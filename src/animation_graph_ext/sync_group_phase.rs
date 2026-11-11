use std::collections::HashMap;

use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, NodeId, TimeUpdate},
    animation_node::{NodeLike, ReflectNodeLike},
    context::{
        graph_context_arena::{GraphContextArena, GraphContextId},
        new_context::NodeContext,
        node_states::StateKey,
        spec_context::SpecContext,
    },
    edge_data::{DataSpec, DataValue, events::{AnimationEvent, EventQueue, SampledEvent}},
    errors::GraphError,
};
use serde::{Deserialize, Serialize};

use crate::extensions::{animation_context::context_time_update, global_f32_input::global_f32_or_root_default};

const EPSILON: f32 = 1e-6;

pub const IN_ACTIVE: &str = "active";
pub const IN_RATE: &str = "rate";
pub const IN_DURATION: &str = "duration";
pub const IN_DELTA_SECONDS: &str = "delta_seconds";
pub const IN_RESET: &str = "reset";
pub const IN_PHASE: &str = "phase";
pub const IN_POSE: &str = "pose";
pub const IN_TIME: &str = "time";
pub const OUT_PHASE: &str = "phase";
pub const OUT_POSE: &str = "pose";
pub const OUT_SAMPLE_TIME: &str = "sample_time";

#[derive(Reflect, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[reflect(Default, Serialize, Deserialize)]
pub struct SyncMarker {
    pub name: String,
    pub time: f32,
}

#[derive(Reflect, Clone, Debug, Default, PartialEq)]
pub struct SyncMarkerPosition {
    pub previous: String,
    pub next: String,
    pub alpha: f32,
}

#[derive(Reflect, Clone, Debug, Default, PartialEq)]
pub struct SyncPhaseState {
    pub phase: f32,
    pub active: bool,
    pub reset_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyncPhaseInput {
    pub active: bool,
    pub reset: bool,
    pub rate: f32,
    pub duration: f32,
    pub start_phase: f32,
    pub looped: bool,
    pub delta_seconds: f32,
}

#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[reflect(Default, Serialize, Deserialize)]
pub enum SourceSyncGroupRole {
    #[default]
    CanBeLeader,
    AlwaysFollower,
    AlwaysLeader,
    TransitionLeader,
    TransitionFollower,
    ExclusiveAlwaysLeader,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceSyncGroupCandidate {
    pub role: SourceSyncGroupRole,
    pub blend_weight: f32,
    pub source_order: usize,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct SourceSyncPlayerBinding {
    pub key: String,
    pub role: SourceSyncGroupRole,
    pub start_position: f32,
    pub play_rate_key: String,
    pub delta_seconds_key: String,
}

#[derive(Reflect, Clone, Debug, Default)]
pub struct SourceSyncPlayerTick {
    pub identity: (GraphContextId, NodeId),
    pub key: String,
    pub role: SourceSyncGroupRole,
    pub source_asset: Option<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>,
    pub override_position_when_joining_sync_group_as_leader: bool,
    pub weight: f32,
    pub has_been_full_weight: bool,
    pub requested_inertialization: bool,
    pub selected_leader: bool,
    pub ticked_as_leader: bool,
    pub phase: f32,
    pub previous_phase: f32,
    pub previous_marker_record: Option<SourceMarkerRecord>,
    pub marker_advance: Option<SourceMarkerAdvance>,
    pub sample_time_hint: Option<f32>,
    pub phase_duration: f32,
    pub phase_markers: Vec<SyncMarker>,
    pub duration: f32,
    pub markers: Vec<SyncMarker>,
    pub looped: bool,
    pub delta_seconds: f32,
    pub play_rate: f32,
    pub length_play_rate: Option<f32>,
}

#[derive(Reflect, Clone, Debug, Default, PartialEq)]
pub struct SourceMarkerAdvance {
    pub forward: bool,
    pub markers: Vec<String>,
    pub start: Option<SyncMarkerPosition>,
    pub end: Option<SyncMarkerPosition>,
}

impl SourceMarkerAdvance {
    pub fn into_inputs(self, group: &str) -> Vec<(String, DataValue)> {
        let key = format!("{SOURCE_SYNC_RESOLVED}.marker_advance.{group}");
        let mut inputs = vec![(format!("{key}.forward"), DataValue::Bool(self.forward)),
            (key.clone(), DataValue::EventQueue(EventQueue::with_events(self.markers.into_iter()
                .map(|name| SampledEvent::instant(AnimationEvent::StringId(name))).collect::<Vec<_>>())))];
        for (suffix, position) in [("start", self.start), ("end", self.end)] {
            if let Some(position) = position {
                inputs.push((format!("{key}.{suffix}"), DataValue::EventQueue(EventQueue::with_events(
                    [position.previous, position.next].into_iter().map(|name|
                        SampledEvent::instant(AnimationEvent::StringId(name))).collect::<Vec<_>>()))));
                inputs.push((format!("{key}.{suffix}.alpha"), DataValue::F32(position.alpha)));
            }
        }
        inputs
    }
}

pub struct SourceSyncPlayerStep {
    pub phase: f32,
    pub advance: SourceMarkerAdvance,
    pub time: f32,
    pub previous_time: f32,
    pub record: Option<SourceMarkerRecord>,
}

pub const SOURCE_SYNC_RESOLVED: &str = "__source_sync_resolved";
pub const SOURCE_SYNC_INERTIAL_SCOPE: &str = "__source_sync_inertial_scope";

pub fn source_sync_excluded_key(identity: (GraphContextId, NodeId)) -> String {
    format!(
        "{SOURCE_SYNC_RESOLVED}.ungrouped.{:?}.{:?}",
        identity.0, identity.1
    )
}

pub fn source_sync_sample_time_key(identity: (GraphContextId, NodeId)) -> String {
    format!("{SOURCE_SYNC_RESOLVED}.sample_time.{:?}.{:?}", identity.0, identity.1)
}

pub fn source_sync_marker_reset_key(identity: (GraphContextId, NodeId)) -> String {
    format!("{}.reset_markers", source_sync_sample_time_key(identity))
}

pub fn source_sync_markers_reset(ctx: &NodeContext) -> bool {
    ctx.graph_context.global_input_data.contains_key(&source_sync_marker_reset_key((ctx.graph_context.context_id, ctx.node_id)))
}

pub fn resolved_source_sync_previous_time(ctx: &NodeContext) -> Result<Option<f32>, GraphError> {
    let key = source_sync_sample_time_key((ctx.graph_context.context_id, ctx.node_id));
    ctx.graph_context.global_input_data.get(&format!("{key}.previous"))
        .map(|value| value.as_f32()).transpose()
}

pub fn resolved_source_sync_sample_time(ctx: &NodeContext) -> Result<Option<f32>, GraphError> {
    ctx.graph_context.global_input_data
        .get(&source_sync_sample_time_key((ctx.graph_context.context_id, ctx.node_id)))
        .map(|value| value.as_f32()).transpose()
}

pub fn source_sync_full_weight_key(identity: (GraphContextId, NodeId)) -> String {
    format!("{SOURCE_SYNC_RESOLVED}.full_weight.{:?}.{:?}", identity.0, identity.1)
}

pub fn source_sync_marker_set_key(key: &str) -> String {
    format!("{SOURCE_SYNC_RESOLVED}.markers.{key}")
}

pub fn source_sync_marker_name_key(key: &str, name: &str) -> String {
    format!("{SOURCE_SYNC_RESOLVED}.markers.{key}.{name}")
}

pub fn resolved_source_marker_advance(ctx: &NodeContext, group: &str) -> Result<Option<SourceMarkerAdvance>, GraphError> {
    let key = format!("{SOURCE_SYNC_RESOLVED}.marker_advance.{group}");
    let inputs = ctx.graph_context.global_input_data;
    let Some(value) = inputs.get(&key) else { return Ok(None); };
    let names = |value: &DataValue| value.as_event_queue()?.events.iter().map(|event| match &event.event {
        AnimationEvent::StringId(name) => Ok(name.clone()),
        _ => Err(GraphError::MissingStateValue),
    }).collect::<Result<Vec<_>, _>>();
    let position = |suffix: &str| -> Result<Option<SyncMarkerPosition>, GraphError> {
        let Some(value) = inputs.get(&format!("{key}.{suffix}")) else { return Ok(None); };
        let names = names(value)?;
        let [previous, next] = names.as_slice() else { return Err(GraphError::MissingStateValue); };
        let alpha = inputs.get(&format!("{key}.{suffix}.alpha")).ok_or(GraphError::MissingStateValue)?.as_f32()?;
        Ok(Some(SyncMarkerPosition { previous: previous.clone(), next: next.clone(), alpha }))
    };
    let forward = inputs.get(&format!("{key}.forward")).ok_or(GraphError::MissingStateValue)?.as_bool()?;
    let markers = names(value)?;
    Ok(Some(SourceMarkerAdvance { forward, markers, start: position("start")?, end: position("end")? }))
}

pub fn resolved_source_marker_record(ctx: &NodeContext, markers: &[SyncMarker], duration: f32,
    asset: Option<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>) -> Result<Option<SourceMarkerRecord>, GraphError> {
    let key = source_sync_sample_time_key((ctx.graph_context.context_id, ctx.node_id));
    let inputs = ctx.graph_context.global_input_data;
    let Some(indices) = inputs.get(&format!("{key}.record.indices")) else { return Ok(None); };
    let indices = indices.as_vec2()?;
    let distances = inputs.get(&format!("{key}.record.distances")).ok_or(GraphError::MissingStateValue)?.as_vec2()?;
    if !indices.is_finite() || !distances.is_finite() || [indices.x, indices.y].iter()
        .any(|index| index.fract() != 0.0 || *index < -1.0 || *index >= markers.len() as f32) {
        return Err(GraphError::MissingStateValue);
    }
    Ok(Some(SourceMarkerRecord { previous: indices.x as i64, next: indices.y as i64,
        previous_distance: distances.x, next_distance: distances.y,
        markers: sorted_markers(markers, duration), duration, asset }))
}

pub fn resolved_source_sync_markers<'a>(ctx: &NodeContext, key: &str, markers: &'a [SyncMarker]) -> std::borrow::Cow<'a, [SyncMarker]> {
    let inputs = ctx.graph_context.global_input_data;
    if !inputs.contains_key(&source_sync_marker_set_key(key))
        || inputs.contains_key(&source_sync_excluded_key((ctx.graph_context.context_id, ctx.node_id)))
    {
        return std::borrow::Cow::Borrowed(markers);
    }
    std::borrow::Cow::Owned(markers.iter().filter(|marker|
        inputs.contains_key(&source_sync_marker_name_key(key, &marker.name))).cloned().collect())
}

pub fn source_sync_delta_seconds(ctx: &NodeContext, key: &str) -> Result<f32, GraphError> {
    let delta = global_f32_or_root_default(ctx, key)?;
    let player_update = match ctx.graph_context.context_arena.get_ref()
        .get_toplevel().query_output_time.get(StateKey::Default) {
        Some(update) => update,
        None => context_time_update(ctx)?,
    };
    Ok(if matches!(player_update, TimeUpdate::Delta(value) if value > 0.0) {
        delta
    } else {
        0.0
    })
}

pub fn source_sync_phase(
    arena: &GraphContextArena,
    graphs: &Assets<AnimationGraph>,
    key: &str,
) -> Option<f32> {
    source_sync_leader(arena, graphs, key)
        .map(|player| player.phase)
        .filter(|phase| phase.is_finite())
}

pub fn source_sync_leader<'a>(
    arena: &'a GraphContextArena,
    graphs: &Assets<AnimationGraph>,
    key: &str,
) -> Option<&'a SourceSyncPlayerTick> {
    source_sync_players(arena, graphs, key)?.iter()
        .find(|player| player.key == key && player.selected_leader)
}

pub fn source_sync_players<'a>(
    arena: &'a GraphContextArena,
    graphs: &Assets<AnimationGraph>,
    key: &str,
) -> Option<&'a [SourceSyncPlayerTick]> {
    for context_id in arena.iter_context_ids() {
        let context = arena.get_context(context_id)?;
        let Some(graph) = graphs.get(context.get_graph_id()) else {
            continue;
        };
        for node in graph.nodes.values() {
            if !node
                .try_inner_downcast_ref::<crate::extensions::SourcePosePassthroughNode>()
                .is_some_and(|node| node.synchronize_groups)
                || !context.node_caches.is_updated(node.id, StateKey::Default)
            {
                continue;
            }
            let Ok(state) = context
                .node_states
                .get::<crate::extensions::AnimationContextState>(node.id, StateKey::Default)
            else {
                continue;
            };
            if state.sync_players.iter().any(|player| player.key == key) {
                return Some(&state.sync_players);
            }
        }
    }
    None
}

pub fn resolved_source_sync_phase(ctx: &NodeContext, key: &str) -> Result<Option<f32>, GraphError> {
    if !ctx
        .graph_context
        .global_input_data
        .get(SOURCE_SYNC_RESOLVED)
        .is_some_and(|value| value.as_bool().unwrap_or(false))
    {
        return Ok(None);
    }
    if ctx
        .graph_context
        .global_input_data
        .contains_key(&source_sync_excluded_key((
            ctx.graph_context.context_id,
            ctx.node_id,
        )))
    {
        return Ok(None);
    }
    ctx.graph_context
        .global_input_data
        .get(&format!("{SOURCE_SYNC_RESOLVED}.{key}"))
        .map(|value| value.as_f32())
        .transpose()
}

pub fn resolved_source_sync_length_ratios(ctx: &NodeContext, key: &str) -> Result<Option<Vec2>, GraphError> {
    if resolved_source_sync_phase(ctx, key)?.is_none() { return Ok(None); }
    ctx.graph_context.global_input_data.get(&format!("{SOURCE_SYNC_RESOLVED}.length_ratios.{key}"))
        .map(|value| value.as_vec2()).transpose()
}

fn ordered_group_players<'a>(players: &[&'a SourceSyncPlayerTick]) -> Vec<&'a SourceSyncPlayerTick> {
    let candidates = players.iter().enumerate().filter(|(_, player)| player.sync_group_eligible()).map(|(index, player)| SourceSyncGroupCandidate {
        role: player.role, blend_weight: player.weight, source_order: index,
    }).collect::<Vec<_>>();
    ordered_source_sync_candidates(&candidates).into_iter().map(|index| players[candidates[index].source_order]).collect()
}

fn group_leader<'a>(players: &[&'a SourceSyncPlayerTick]) -> Option<&'a SourceSyncPlayerTick> {
    players.iter().copied().find(|player| player.selected_leader)
        .or_else(|| ordered_group_players(players).first().copied())
}

fn common_marker_names(players: &[&SourceSyncPlayerTick], leader: &SourceSyncPlayerTick) -> Vec<String> {
    let mut common = leader.markers.iter().map(|marker| marker.name.clone()).collect::<Vec<_>>();
    common.sort();
    common.dedup();
    for candidate in players {
        if candidate.sync_group_eligible() && !candidate.markers.is_empty()
        {
            common.retain(|name| candidate.markers.iter().any(|marker| &marker.name == name));
        }
    }
    common
}

pub struct SourceSyncGroupResolution {
    pub key: String,
    pub phase: Option<f32>,
    pub length_ratios: Option<Vec2>,
    pub ungrouped: Vec<(GraphContextId, NodeId)>,
    pub markers: Vec<String>,
    pub marker_advance: Option<SourceMarkerAdvance>,
    pub leader_samples: Vec<((GraphContextId, NodeId), SourceSyncPlayerStep)>,
    pub marker_resets: Vec<(GraphContextId, NodeId)>,
}

pub fn resolve_source_sync_phases(
    current: &[SourceSyncPlayerTick],
    previous: &[SourceSyncPlayerTick],
) -> Vec<SourceSyncGroupResolution> {
    let mut groups: Vec<SourceSyncGroupResolution> = Vec::new();
    for player in current {
        if groups.iter().any(|group| group.key == player.key) {
            continue;
        }
        let players = current
            .iter()
            .filter(|candidate| candidate.key == player.key)
            .collect::<Vec<_>>();
        let ordered = ordered_group_players(&players);
        let common = ordered.first().map_or_else(Vec::new, |leader| common_marker_names(&players, leader));
        let previous_players = previous
            .iter()
            .filter(|prior| prior.key == player.key)
            .collect::<Vec<_>>();
        let previous_leader = group_leader(&previous_players);
        let previous_common = previous_leader.map_or_else(Vec::new, |prior|
            common_marker_names(&previous_players, prior));
        let mut marker_advance = None;
        let mut leader_samples = Vec::new();
        let mut phase = None;
        let mut length_ratios = None;
        for leader in ordered {
            let joined = previous_leader.is_some_and(|prior| {
                prior.identity != leader.identity || prior.source_asset != leader.source_asset
            });
            let mut grouped = leader.clone();
            grouped.markers.retain(|marker| common.contains(&marker.name));
            grouped.phase_markers.retain(|marker| common.contains(&marker.name));
            if common.is_empty() {
                grouped.play_rate = grouped.length_play_rate.unwrap_or(grouped.play_rate);
            }
            let length_resync = (common.is_empty() || previous_common.is_empty())
                && !leader.override_position_when_joining_sync_group_as_leader && leader.requested_inertialization;
            let prior_phase = if length_resync {
                previous_leader.filter(|prior| source_sync_group_leader_score(SourceSyncGroupCandidate {
                        role: prior.role, blend_weight: prior.weight, source_order: 0,
                    }) >= source_sync_group_leader_score(SourceSyncGroupCandidate {
                        role: leader.role, blend_weight: leader.weight, source_order: 0,
                    })).map(|prior| marker_phase_sample_time(prior.phase, prior.phase_duration,
                        &prior.phase_markers, prior.duration, &prior.markers, prior.looped) / prior.duration.max(EPSILON))
            } else if common.is_empty() {
                None
            } else if leader.override_position_when_joining_sync_group_as_leader && joined {
                None
            } else if common == previous_common {
                previous_leader.map(|prior| prior.phase)
            } else {
                previous_leader.and_then(|prior| {
                    let prior_time = marker_phase_sample_time(prior.phase, prior.phase_duration,
                        &prior.phase_markers, prior.duration, &prior.markers, prior.looped);
                    let position = marker_position_at_time(&prior.markers, prior.duration, prior_time, prior.looped)?;
                    if !common.contains(&position.previous) || !common.contains(&position.next) {
                        return None;
                    }
                    time_for_marker_position(&grouped.phase_markers, grouped.phase_duration,
                        &position, prior.phase * grouped.phase_duration, grouped.looped)
                        .map(|time| time_to_phase(time, grouped.phase_duration, grouped.looped))
                })
            };
            let step = if let Some(phase) = prior_phase {
                let continuing = common == previous_common && previous_players.iter().any(|prior|
                    prior.identity == leader.identity && prior.source_asset == leader.source_asset
                        && (prior.duration - leader.duration).abs() <= EPSILON);
                if let Some(time) = grouped.sample_time_hint.filter(|_| continuing) {
                    grouped.advance_from_time(time)
                } else {
                    grouped.previous_marker_record = None;
                    let position = previous_leader.and_then(|prior| prior.marker_advance.as_ref())
                        .and_then(|advance| advance.end.as_ref()).filter(|position| position.is_valid());
                    if let Some(position) = position {
                        grouped.advance_from_position(grouped.sample_time_hint.unwrap_or(phase * grouped.duration), Some(position))
                    } else { grouped.advance(phase) }
                }
            } else {
                let previous_time = leader.sample_time_hint.or_else(|| previous.iter()
                    .find(|prior| prior.identity == leader.identity && prior.source_asset == leader.source_asset
                        && (prior.duration - leader.duration).abs() <= EPSILON)
                    .map(|prior| marker_phase_sample_time(prior.phase, prior.phase_duration,
                        &prior.phase_markers, prior.duration, &prior.markers, prior.looped)));
                if let Some(time) = previous_time {
                    grouped.advance_from_time(time)
                } else {
                    let time = marker_phase_sample_time(leader.phase, leader.phase_duration,
                        &leader.phase_markers, leader.duration, &leader.markers, leader.looped);
                    let mut initial = grouped.clone();
                    initial.delta_seconds = 0.0;
                    initial.advance_from_time(time)
                }
            };
            let valid_end = step.advance.end.as_ref().is_some_and(|position| position.is_valid());
            phase = Some(step.phase);
            length_ratios = Some(Vec2::new(step.previous_time, step.time) / leader.duration.max(EPSILON));
            marker_advance = Some(step.advance.clone());
            leader_samples.push((leader.identity, step));
            if common.is_empty() || valid_end { break; }
        }
        let previous_order = ordered_group_players(&previous_players);
        let previous_leader_index = previous_leader.and_then(|leader|
            previous_order.iter().position(|player| player.identity == leader.identity));
        let leader_changed = previous_leader_index != leader_samples.len().checked_sub(1);
        let marker_resets = players.iter().filter(|player| {
            let prior = previous_players.iter().find(|prior| prior.identity == player.identity
                && prior.sync_group_eligible()
                && (prior.role != SourceSyncGroupRole::ExclusiveAlwaysLeader || prior.ticked_as_leader));
            common != previous_common || prior.is_none_or(|prior| prior.source_asset != player.source_asset)
                || (leader_changed && !leader_samples.iter().any(|(identity, ..)| *identity == player.identity))
        }).map(|player| player.identity).collect();
        let ungrouped = players
            .iter()
            .filter(|candidate| {
                !candidate.sync_group_eligible()
                    || (candidate.role == SourceSyncGroupRole::ExclusiveAlwaysLeader
                        && !leader_samples.iter().any(|(identity, ..)| *identity == candidate.identity))
            })
            .map(|candidate| candidate.identity)
            .collect();
        groups.push(SourceSyncGroupResolution { key: player.key.clone(), phase, length_ratios, ungrouped, markers: common, marker_advance, leader_samples, marker_resets });
    }
    groups
}

impl SourceSyncPlayerTick {
    pub fn sync_group_eligible(&self) -> bool {
        !matches!(self.role, SourceSyncGroupRole::TransitionLeader | SourceSyncGroupRole::TransitionFollower)
            || (!self.requested_inertialization
                && (self.has_been_full_weight || self.weight >= 1.0 - 1e-5))
    }

    pub fn advance(&self, previous_phase: f32) -> SourceSyncPlayerStep {
        let position = marker_position_at_time(&self.phase_markers, self.phase_duration,
            phase_to_time(previous_phase, self.phase_duration, self.looped), self.looped);
        let time = phase_to_time(previous_phase, self.duration, self.looped);
        self.advance_from_position(if position.is_some() { self.sample_time_hint.unwrap_or(time) } else { time }, position.as_ref())
    }

    pub fn advance_from_position(&self, hint: f32, position: Option<&SyncMarkerPosition>) -> SourceSyncPlayerStep {
        let markers = sorted_markers(&self.markers, self.duration);
        let (time, record) = if let Some(position) = position.filter(|position| position.is_valid()) {
            SourceMarkerRecord::from_position(&markers, self.duration, self.looped, hint, position, self.source_asset.clone())
        } else { (hint, SourceMarkerRecord::at_time(&markers, self.duration, self.looped, hint, self.source_asset.clone())) };
        self.advance_with_record(time, record)
    }

    pub fn advance_from_time(&self, previous_time: f32) -> SourceSyncPlayerStep {
        let markers = sorted_markers(&self.markers, self.duration);
        let record = self.previous_marker_record.clone().filter(|record| record.valid(&markers, self.duration, self.looped))
            .unwrap_or_else(|| SourceMarkerRecord::at_time(&markers, self.duration, self.looped, previous_time, self.source_asset.clone()));
        self.advance_with_record(previous_time, record)
    }

    fn advance_with_record(&self, previous_time: f32, mut record: SourceMarkerRecord) -> SourceSyncPlayerStep {
        let delta = self.delta_seconds * self.play_rate;
        let (time, advance) = if record.markers.is_empty() {
            (crate::extensions::source_sequence_player::bound_source_sample_time(previous_time + delta, self.duration, self.looped),
                SourceMarkerAdvance { forward: delta >= 0.0, ..default() })
        } else { record.advance(previous_time, delta, self.looped) };
        let phase = advance.end.as_ref().filter(|position| position.is_valid())
            .and_then(|position| time_for_marker_position(&self.phase_markers, self.phase_duration,
                position, time / self.duration.max(EPSILON) * self.phase_duration, self.looped))
            .map(|time| time_to_phase(time, self.phase_duration, self.looped))
            .unwrap_or_else(|| time_to_phase(time, self.duration, self.looped));
        SourceSyncPlayerStep { phase, advance, time, previous_time,
            record: (!record.markers.is_empty()).then_some(record) }
    }
}


pub fn source_sync_group_leader_score(candidate: SourceSyncGroupCandidate) -> f32 {
    match candidate.role {
        SourceSyncGroupRole::CanBeLeader | SourceSyncGroupRole::TransitionLeader => {
            candidate.blend_weight
        }
        SourceSyncGroupRole::AlwaysLeader | SourceSyncGroupRole::ExclusiveAlwaysLeader => 2.0,
        SourceSyncGroupRole::AlwaysFollower | SourceSyncGroupRole::TransitionFollower => {
            -2.0 + candidate.blend_weight
        }
    }
}

fn source_sync_group_tie_prefers_last(candidate: SourceSyncGroupCandidate) -> bool {
    matches!(
        candidate.role,
        SourceSyncGroupRole::AlwaysLeader | SourceSyncGroupRole::ExclusiveAlwaysLeader
    )
}

fn ordered_source_sync_candidates(candidates: &[SourceSyncGroupCandidate]) -> Vec<usize> {
    let mut ordered: Vec<usize> = (0..candidates.len()).collect();
    ordered.sort_by(|left, right| {
        let left_candidate = candidates[*left];
        let right_candidate = candidates[*right];
        let score = source_sync_group_leader_score(right_candidate)
            .total_cmp(&source_sync_group_leader_score(left_candidate));
        if !score.is_eq() {
            return score;
        }
        if source_sync_group_tie_prefers_last(left_candidate)
            && source_sync_group_tie_prefers_last(right_candidate)
        {
            return right_candidate
                .source_order
                .cmp(&left_candidate.source_order);
        }
        left_candidate
            .source_order
            .cmp(&right_candidate.source_order)
    });
    ordered
}

#[derive(Resource, Default)]
pub struct SyncGroupPhaseRegistry {
    phases: HashMap<String, SyncPhaseState>,
}

impl SyncGroupPhaseRegistry {
    pub fn advance(&mut self, group: impl Into<String>, input: SyncPhaseInput) -> f32 {
        let state = self.phases.entry(group.into()).or_default();
        advance_sync_phase(state, input)
    }

    pub fn phase(&self, group: &str) -> Option<f32> {
        self.phases.get(group).map(|state| state.phase)
    }

    pub fn clear(&mut self, group: &str) {
        self.phases.remove(group);
    }
}

pub fn normalize_phase(phase: f32, looped: bool) -> f32 {
    if !phase.is_finite() {
        return 0.0;
    }
    if looped {
        phase.rem_euclid(1.0)
    } else {
        phase.clamp(0.0, 1.0)
    }
}

pub fn phase_to_time(phase: f32, duration: f32, looped: bool) -> f32 {
    let duration = duration.max(EPSILON);
    normalize_phase(phase, looped) * duration
}

pub fn time_to_phase(time: f32, duration: f32, looped: bool) -> f32 {
    let duration = duration.max(EPSILON);
    normalize_phase(time / duration, looped)
}

pub fn advance_sync_phase(state: &mut SyncPhaseState, input: SyncPhaseInput) -> f32 {
    if !input.active {
        state.active = false;
        state.reset_active = false;
        return state.phase;
    }
    let reset = !state.active || (input.reset && !state.reset_active);
    state.active = true;
    state.reset_active = input.reset;
    if reset {
        state.phase = normalize_phase(input.start_phase, input.looped);
        return state.phase;
    }
    let duration = input.duration.max(EPSILON);
    state.phase = normalize_phase(
        state.phase + input.delta_seconds.max(0.0) * input.rate / duration,
        input.looped,
    );
    state.phase
}

fn sorted_markers(markers: &[SyncMarker], duration: f32) -> Vec<SyncMarker> {
    let mut result = markers
        .iter()
        .filter(|marker| {
            !marker.name.is_empty()
                && marker.time.is_finite()
                && marker.time >= 0.0
                && marker.time <= duration
        })
        .cloned()
        .collect::<Vec<_>>();
    result.sort_by(|left, right| left.time.total_cmp(&right.time));
    result
}

pub fn marker_position_at_time(
    markers: &[SyncMarker],
    duration: f32,
    time: f32,
    looped: bool,
) -> Option<SyncMarkerPosition> {
    marker_position_at_time_in_direction(markers, duration, time, looped, false)
}

fn marker_position_at_time_in_direction(
    markers: &[SyncMarker], duration: f32, time: f32, looped: bool, forward: bool,
) -> Option<SyncMarkerPosition> {
    let duration = duration.max(EPSILON);
    let markers = sorted_markers(markers, duration);
    if markers.is_empty() || (!looped && markers.len() < 2) {
        return None;
    }
    let time = if looped {
        time.rem_euclid(duration)
    } else {
        time.clamp(0.0, duration)
    };
    let previous_index = markers.iter().rposition(|marker| if forward { marker.time < time } else { marker.time <= time });
    let (previous, previous_time, next, next_time) = match previous_index {
        Some(index) if index + 1 < markers.len() => (
            &markers[index],
            markers[index].time,
            &markers[index + 1],
            markers[index + 1].time,
        ),
        Some(index) if looped => (
            &markers[index],
            markers[index].time,
            &markers[0],
            markers[0].time + duration,
        ),
        Some(_) => return None,
        None if looped => (
            markers.last()?,
            markers.last()?.time - duration,
            &markers[0],
            markers[0].time,
        ),
        None => return None,
    };
    let current_time = if looped && time < previous_time {
        time + duration
    } else {
        time
    };
    let span = (next_time - previous_time).max(EPSILON);
    Some(SyncMarkerPosition {
        previous: previous.name.clone(),
        next: next.name.clone(),
        alpha: ((current_time - previous_time) / span).clamp(0.0, 1.0),
    })
}

pub fn time_for_marker_position(
    markers: &[SyncMarker],
    duration: f32,
    position: &SyncMarkerPosition,
    current_time: f32,
    looped: bool,
) -> Option<f32> {
    let duration = duration.max(EPSILON);
    let markers = sorted_markers(markers, duration);
    if markers.is_empty() || (!looped && markers.len() < 2) {
        return None;
    }
    let current_time = if looped { current_time.rem_euclid(duration) } else { current_time.clamp(0.0, duration) };
    let mut closest = None;
    for index in 0..markers.len() {
        let next_index = if index + 1 == markers.len() {
            if !looped {
                continue;
            }
            0
        } else {
            index + 1
        };
        if markers[index].name != position.previous || markers[next_index].name != position.next {
            continue;
        }
        let start = markers[index].time;
        let end = markers[next_index].time + if next_index == 0 { duration } else { 0.0 };
        let time = start + (end - start).max(EPSILON) * position.alpha.clamp(0.0, 1.0);
        let distance = if looped && next_index == 0 {
            (time - current_time).abs().min((time - duration - current_time).abs())
        } else {
            (time - current_time).abs()
        };
        if closest.is_none_or(|(best, _)| distance < best) {
            closest = Some((distance, if looped { time.rem_euclid(duration) } else { time }));
        }
    }
    closest.map(|(_, time)| time)
}

pub fn follower_sample_time(
    phase: f32,
    leader_duration: f32,
    leader_markers: &[SyncMarker],
    follower_duration: f32,
    follower_markers: &[SyncMarker],
    looped: bool,
) -> f32 {
    follower_sample_time_near(phase, leader_duration, leader_markers, follower_duration,
        follower_markers, phase_to_time(phase, follower_duration, looped), looped)
}

pub fn follower_sample_time_near(
    phase: f32,
    leader_duration: f32,
    leader_markers: &[SyncMarker],
    follower_duration: f32,
    follower_markers: &[SyncMarker],
    current_time: f32,
    looped: bool,
) -> f32 {
    let leader_time = phase_to_time(phase, leader_duration, looped);
    marker_position_at_time(leader_markers, leader_duration, leader_time, looped)
        .and_then(|position| {
            time_for_marker_position(follower_markers, follower_duration, &position,
                current_time, looped)
        })
        .unwrap_or_else(|| phase_to_time(phase, follower_duration, looped))
}

#[derive(Clone, Debug)]
pub struct SyncMarkerPhaseInput<'a> {
    pub active: bool,
    pub reset: bool,
    pub rate: f32,
    pub leader_duration: f32,
    pub leader_markers: &'a [SyncMarker],
    pub phase_duration: f32,
    pub phase_markers: &'a [SyncMarker],
    pub start_time: f32,
    pub looped: bool,
    pub delta_seconds: f32,
}

pub fn marker_phase_sample_time(
    phase: f32,
    phase_duration: f32,
    phase_markers: &[SyncMarker],
    sample_duration: f32,
    sample_markers: &[SyncMarker],
    looped: bool,
) -> f32 {
    marker_phase_sample_time_near(phase, phase_duration, phase_markers, sample_duration,
        sample_markers, phase_to_time(phase, sample_duration, looped), looped)
}

pub fn marker_phase_sample_time_near(
    phase: f32,
    phase_duration: f32,
    phase_markers: &[SyncMarker],
    sample_duration: f32,
    sample_markers: &[SyncMarker],
    current_time: f32,
    looped: bool,
) -> f32 {
    let fallback = phase_to_time(phase, sample_duration, looped);
    let phase_time = phase_to_time(phase, phase_duration, looped);
    marker_position_at_time(phase_markers, phase_duration, phase_time, looped)
        .and_then(|position| {
            time_for_marker_position(sample_markers, sample_duration, &position, current_time, looped)
        })
        .unwrap_or(fallback)
}

impl SyncMarkerPosition {
    fn is_valid(&self) -> bool { !self.previous.is_empty() && !self.next.is_empty() }
}

#[derive(Reflect, Clone, Debug)]
pub struct SourceMarkerRecord {
    previous: i64,
    next: i64,
    previous_distance: f32,
    next_distance: f32,
    markers: Vec<SyncMarker>,
    duration: f32,
    pub asset: Option<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>,
}

impl SourceMarkerRecord {
    fn valid(&self, markers: &[SyncMarker], duration: f32, looped: bool) -> bool {
        self.markers == markers && self.duration == duration && [self.previous, self.next].iter()
            .all(|index| *index >= if looped { 0 } else { -1 } && *index < markers.len() as i64)
    }

    fn at_time(markers: &[SyncMarker], duration: f32, looped: bool, time: f32,
        asset: Option<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>) -> Self {
        let previous = markers.iter().rposition(|marker| marker.time < time);
        let next = markers.iter().position(|marker| marker.time >= time);
        let mut record = Self { previous: previous.map_or(-1, |index| index as i64),
            next: next.map_or(-1, |index| index as i64), previous_distance: -time, next_distance: duration - time,
            markers: markers.to_vec(), duration, asset };
        if let Some(index) = previous { record.previous_distance = markers[index].time - time; }
        else if looped && !markers.is_empty() {
            record.previous = markers.len() as i64 - 1;
            record.previous_distance = markers.last().unwrap().time - duration - time;
        }
        if let Some(index) = next { record.next_distance = markers[index].time - time; }
        else if looped && !markers.is_empty() {
            record.next = 0;
            record.next_distance = markers[0].time + duration - time;
        }
        record
    }

    fn from_position(markers: &[SyncMarker], duration: f32, looped: bool, hint: f32,
        position: &SyncMarkerPosition, asset: Option<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>) -> (f32, Self) {
        let mut record = Self::at_time(markers, duration, looped, hint, asset);
        if !looped || !position.is_valid() {
            record.previous = markers.iter().rposition(|marker| marker.time < hint && marker.name == position.previous).map_or(-1, |index| index as i64);
            record.next = markers.iter().position(|marker| marker.time > hint && marker.name == position.next).map_or(-1, |index| index as i64);
            record.previous_distance = record.previous_time() - hint;
            record.next_distance = record.next_time() - hint;
            return (hint, record);
        }
        let mut best = None;
        for previous in 0..markers.len() {
            if markers[previous].name != position.previous { continue; }
            for offset in 1..=markers.len() {
                let next = (previous + offset) % markers.len();
                if markers[next].name != position.next { continue; }
                let wrapped = markers[next].time < markers[previous].time;
                let end = markers[next].time + if wrapped { duration } else { 0.0 };
                let time = markers[previous].time + position.alpha * (end - markers[previous].time);
                let distance = (time - hint).abs();
                if best.is_none_or(|(old, _, _)| distance < old) { best = Some((distance, previous, next)); }
                else if wrapped {
                    let distance = (time - duration - hint).abs();
                    if best.is_none_or(|(old, _, _)| distance < old) { best = Some((distance, previous, next)); }
                }
                break;
            }
        }
        if let Some((_, previous, next)) = best {
            record.previous = previous as i64;
            record.next = next as i64;
            (record.sample(position.alpha), record)
        } else { (hint, record) }
    }

    fn previous_time(&self) -> f32 { if self.previous < 0 { 0.0 } else { self.markers[self.previous as usize].time } }
    fn next_time(&self) -> f32 { if self.next < 0 { self.duration } else { self.markers[self.next as usize].time } }

    fn position(&self, mut time: f32) -> SyncMarkerPosition {
        let mut previous = self.previous_time();
        let mut next = self.next_time();
        if previous > next {
            if previous > time { previous -= self.duration; }
            if next < time { next += self.duration; }
        } else if previous > time { time += self.duration; }
        if previous == next { previous -= self.duration; }
        SyncMarkerPosition {
            previous: if self.previous < 0 { String::new() } else { self.markers[self.previous as usize].name.clone() },
            next: if self.next < 0 { String::new() } else { self.markers[self.next as usize].name.clone() },
            alpha: (time - previous) / (next - previous).max(EPSILON),
        }
    }

    fn sample(&mut self, alpha: f32) -> f32 {
        let mut previous = self.previous_time();
        let next = self.next_time();
        if previous >= next { previous -= self.duration; }
        let time = previous + alpha * (next - previous);
        self.previous_distance = previous - time;
        self.next_distance = next - time;
        (if time < 0.0 { time + self.duration } else { time }).clamp(0.0, self.duration)
    }

    fn advance(&mut self, time: f32, delta: f32, looped: bool) -> (f32, SourceMarkerAdvance) {
        let mut advance = SourceMarkerAdvance { forward: delta >= 0.0,
            start: Some(self.position(time)), ..default() };
        let mut time = time.clamp(0.0, self.duration);
        let mut remaining = delta;
        while remaining != 0.0 {
            if advance.forward {
                if self.next < 0 {
                    let old = time;
                    time = (time + remaining).min(self.duration);
                    self.next_distance = self.duration - time;
                    self.previous_distance -= time - old;
                    break;
                }
                if remaining > self.next_distance {
                    time = self.next_time();
                    remaining -= self.next_distance;
                    self.previous = self.next;
                    self.previous_distance = 0.0;
                    advance.markers.push(self.markers[self.next as usize].name.clone());
                    self.next += 1;
                    let mut offset = 0.0;
                    if self.next == self.markers.len() as i64 {
                        self.next = if looped { offset = self.duration; 0 } else { -1 };
                    }
                    if self.next >= 0 { self.next_distance = offset + self.next_time() - time; }
                    continue;
                }
            } else {
                if self.previous < 0 {
                    let old = time;
                    time = (time + remaining).max(0.0);
                    self.previous_distance = time;
                    self.next_distance -= time - old;
                    break;
                }
                if remaining < self.previous_distance {
                    time = self.previous_time();
                    remaining -= self.previous_distance;
                    self.next = self.previous;
                    self.next_distance = 0.0;
                    advance.markers.push(self.markers[self.previous as usize].name.clone());
                    self.previous -= 1;
                    let mut offset = 0.0;
                    if self.previous < 0 && looped { self.previous = self.markers.len() as i64 - 1; offset = -self.duration; }
                    if self.previous >= 0 { self.previous_distance = offset + self.previous_time() - time; }
                    continue;
                }
            }
            time = (time + remaining).rem_euclid(self.duration.max(EPSILON));
            self.previous_distance -= remaining;
            self.next_distance -= remaining;
            break;
        }
        advance.end = Some(self.position(time));
        (time, advance)
    }

    pub fn into_inputs(&self, key: &str) -> [(String, DataValue); 2] {
        [(format!("{key}.record.indices"), DataValue::Vec2(Vec2::new(self.previous as f32, self.next as f32))),
            (format!("{key}.record.distances"), DataValue::Vec2(Vec2::new(self.previous_distance, self.next_distance)))]
    }
}

pub fn marker_phase_sample_time_continuous(
    phase: f32,
    advance: &SourceMarkerAdvance,
    phase_duration: f32,
    phase_markers: &[SyncMarker],
    sample_duration: f32,
    sample_markers: &[SyncMarker],
    previous_sample_time: f32,
    looped: bool,
    record: &mut Option<SourceMarkerRecord>,
    asset: Option<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>,
) -> (f32, f32) {
    let fallback = || marker_phase_sample_time_near(phase, phase_duration, phase_markers,
        sample_duration, sample_markers, previous_sample_time, looped);
    let phase_markers = sorted_markers(phase_markers, phase_duration);
    let sample_markers = sorted_markers(sample_markers, sample_duration);
    let saved = record.take().filter(|record| record.asset == asset && record.valid(&sample_markers, sample_duration, looped));
    let mut previous_time = previous_sample_time;
    if phase_markers.is_empty() || sample_markers.is_empty() { return (fallback(), previous_time); }
    let phase_time = phase_to_time(phase, phase_duration, looped);
    let mut current = if let Some(record) = saved { record }
        else if let Some(start) = advance.start.as_ref().filter(|position| position.is_valid()) {
            let (time, record) = SourceMarkerRecord::from_position(&sample_markers, sample_duration, looped, previous_sample_time, start, asset.clone());
            previous_time = time;
            record
        } else {
            SourceMarkerRecord::at_time(&sample_markers, sample_duration, looped, previous_sample_time, asset.clone())
        };
    let (mut previous, mut next) = (current.previous, current.next);
    let position = advance.end.clone().or_else(|| marker_position_at_time_in_direction(&phase_markers,
        phase_duration, phase_time, looped, advance.forward));
    let Some(position) = position else { return (fallback(), previous_time); };
    let count = sample_markers.len() as i64;
    let find = |start: i64, name: &str, direction: i64| (0..count)
        .map(|offset| start + direction * offset)
        .take_while(|index| looped || (0..count).contains(index))
        .find(|index| sample_markers[index.rem_euclid(count) as usize].name == name);
    if let Some(start) = advance.start.as_ref().filter(|position| position.is_valid()) {
        let matches = |index: i64, name: &str| index < 0
            || sample_markers[index.rem_euclid(count) as usize].name == name;
        if advance.forward {
            if !matches(previous, &start.previous) {
                previous = find(previous, &start.previous, 1).map_or(-1, |index| index.rem_euclid(count));
                next = previous + 1;
                if next >= count { next = if looped { 0 } else { -1 }; }
            }
            if !matches(next, &start.next) {
                next = find(next, &start.next, 1).map_or(-1, |index| index.rem_euclid(count));
            }
        } else {
            if !matches(next, &start.next) {
                next = find(next, &start.next, -1).map_or(-1, |index| index.rem_euclid(count));
                previous = if next < 0 || (next == 0 && looped) { count - 1 } else { next - 1 };
            }
            if !matches(previous, &start.previous) {
                previous = find(previous, &start.previous, -1).map_or(-1, |index| index.rem_euclid(count));
            }
        }
    }
    if !advance.markers.is_empty() {
        let direction = if advance.forward { 1 } else { -1 };
        let mut cursor = if advance.forward { next } else { previous };
        for marker in &advance.markers {
            let Some(index) = find(cursor, marker, direction) else { return (fallback(), previous_time); };
            cursor = index;
        }
        if advance.forward {
            previous = cursor;
            let Some(index) = find(cursor, &position.next, 1) else { return (fallback(), previous_time); };
            next = index;
        } else {
            next = cursor;
            let Some(index) = find(cursor, &position.previous, -1) else { return (fallback(), previous_time); };
            previous = index;
        }
    }
    if !looped && (previous < 0 || next >= count) { return (fallback(), previous_time); }
    if sample_markers[previous.rem_euclid(count) as usize].name != position.previous
        || sample_markers[next.rem_euclid(count) as usize].name != position.next {
        return (fallback(), previous_time);
    }
    current.previous = previous.rem_euclid(count);
    current.next = next.rem_euclid(count);
    let time = current.sample(position.alpha);
    *record = Some(current);
    (time, previous_time)
}

pub fn marker_phase_from_sample_time(
    sample_time: f32,
    sample_duration: f32,
    sample_markers: &[SyncMarker],
    phase_duration: f32,
    phase_markers: &[SyncMarker],
    looped: bool,
) -> f32 {
    let fallback = time_to_phase(sample_time, sample_duration, looped);
    marker_position_at_time(sample_markers, sample_duration, sample_time, looped)
        .and_then(|position| {
            time_for_marker_position(phase_markers, phase_duration, &position,
                phase_to_time(fallback, phase_duration, looped), looped)
        })
        .map(|phase_time| time_to_phase(phase_time, phase_duration, looped))
        .unwrap_or(fallback)
}

pub fn advance_marker_sync_phase(
    state: &mut SyncPhaseState,
    input: SyncMarkerPhaseInput<'_>,
) -> f32 {
    if !input.active {
        state.active = false;
        state.reset_active = false;
        return state.phase;
    }
    let reset = !state.active || (input.reset && !state.reset_active);
    state.active = true;
    state.reset_active = input.reset;
    if reset {
        state.phase = marker_phase_from_sample_time(
            input.start_time,
            input.leader_duration,
            input.leader_markers,
            input.phase_duration,
            input.phase_markers,
            input.looped,
        );
        return state.phase;
    }
    let duration = input.leader_duration.max(EPSILON);
    let current_time = marker_phase_sample_time(
        state.phase,
        input.phase_duration,
        input.phase_markers,
        duration,
        input.leader_markers,
        input.looped,
    );
    let rate = input.rate.is_finite().then_some(input.rate).unwrap_or(0.0);
    let next_time = if input.looped {
        (current_time + input.delta_seconds.max(0.0) * rate).rem_euclid(duration)
    } else {
        (current_time + input.delta_seconds.max(0.0) * rate).clamp(0.0, duration)
    };
    state.phase = marker_phase_from_sample_time(
        next_time,
        duration,
        input.leader_markers,
        input.phase_duration,
        input.phase_markers,
        input.looped,
    );
    state.phase
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SyncGroupPhaseNode {
    pub looped: bool,
    pub start_phase: f32,
    pub activation_threshold: f32,
}

impl NodeLike for SyncGroupPhaseNode {
    fn display_name(&self) -> String {
        "ALS Sync Group Phase".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let active = ctx.data_back(IN_ACTIVE)?.as_f32()? > self.activation_threshold;
        let rate = ctx.data_back(IN_RATE)?.as_f32()?;
        let duration = ctx.data_back(IN_DURATION)?.as_f32()?;
        let delta_seconds = ctx.data_back(IN_DELTA_SECONDS)?.as_f32()?;
        let reset = ctx.data_back(IN_RESET)?.as_f32()? > self.activation_threshold;
        let phase = {
            let state = ctx.state_mut::<SyncPhaseState>()?;
            advance_sync_phase(
                state,
                SyncPhaseInput {
                    active,
                    reset,
                    rate,
                    duration,
                    start_phase: self.start_phase,
                    looped: self.looped,
                    delta_seconds,
                },
            )
        };
        ctx.set_data_fwd(OUT_PHASE, phase);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_ACTIVE, DataSpec::F32)
            .add_input_data(IN_RATE, DataSpec::F32)
            .add_input_data(IN_DURATION, DataSpec::F32)
            .add_input_data(IN_DELTA_SECONDS, DataSpec::F32)
            .add_input_data(IN_RESET, DataSpec::F32)
            .add_output_data(OUT_PHASE, DataSpec::F32);
        Ok(())
    }
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SyncGroupSampleNode {
    pub leader_duration: f32,
    pub leader_markers: Vec<SyncMarker>,
    pub follower_duration: f32,
    pub follower_markers: Vec<SyncMarker>,
    pub looped: bool,
}

impl NodeLike for SyncGroupSampleNode {
    fn display_name(&self) -> String {
        "ALS Sync Group Sample".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let duration = if self.follower_duration > EPSILON {
            Some(self.follower_duration)
        } else {
            ctx.duration_back(IN_TIME)?
        };
        ctx.set_duration_fwd(duration);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let graph_time = ctx
            .time_update_fwd()?
            .partial_update_basic(ctx.prev_time())
            .unwrap_or_else(|| ctx.prev_time());
        let phase = ctx.data_back(IN_PHASE)?.as_f32()?;
        let duration = if self.follower_duration > EPSILON {
            self.follower_duration
        } else {
            ctx.duration_back(IN_TIME)?.unwrap_or(EPSILON)
        }
        .max(EPSILON);
        let sample_time = follower_sample_time(
            phase,
            self.leader_duration,
            &self.leader_markers,
            duration,
            &self.follower_markers,
            self.looped,
        );
        ctx.set_time_update_back(IN_TIME, TimeUpdate::Absolute(sample_time));
        let mut pose = ctx.data_back(IN_POSE)?.into_pose()?;
        pose.timestamp = graph_time;
        ctx.set_time(graph_time);
        ctx.set_data_fwd(OUT_POSE, pose);
        ctx.set_data_fwd(OUT_SAMPLE_TIME, sample_time);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_PHASE, DataSpec::F32)
            .add_input_data(IN_POSE, DataSpec::Pose)
            .add_input_time(IN_TIME)
            .add_output_data(OUT_POSE, DataSpec::Pose)
            .add_output_data(OUT_SAMPLE_TIME, DataSpec::F32)
            .add_output_time();
        Ok(())
    }
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct GlobalSyncGroupSampleNode {
    pub key: String,
    pub leader_duration: f32,
    pub leader_markers: Vec<SyncMarker>,
    pub follower_duration: f32,
    pub follower_markers: Vec<SyncMarker>,
    pub looped: bool,
}

impl NodeLike for GlobalSyncGroupSampleNode {
    fn display_name(&self) -> String {
        "ALS Global Sync Group Sample".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let duration = if self.follower_duration > EPSILON {
            Some(self.follower_duration)
        } else {
            ctx.duration_back(IN_TIME)?
        };
        ctx.set_duration_fwd(duration);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let graph_time = context_time_update(&ctx)?
            .partial_update_basic(ctx.prev_time())
            .unwrap_or_else(|| ctx.prev_time());
        let phase = global_f32_or_root_default(&ctx, &self.key)?;
        let duration = if self.follower_duration > EPSILON {
            self.follower_duration
        } else {
            ctx.duration_back(IN_TIME)?.unwrap_or(EPSILON)
        }
        .max(EPSILON);
        let sample_time = follower_sample_time(
            phase,
            self.leader_duration,
            &self.leader_markers,
            duration,
            &self.follower_markers,
            self.looped,
        );
        ctx.set_time_update_back(IN_TIME, TimeUpdate::Absolute(sample_time));
        let mut pose = ctx.data_back(IN_POSE)?.into_pose()?;
        pose.timestamp = graph_time;
        ctx.set_time(graph_time);
        ctx.set_data_fwd(OUT_POSE, pose);
        ctx.set_data_fwd(OUT_SAMPLE_TIME, sample_time);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_POSE, DataSpec::Pose)
            .add_input_time(IN_TIME)
            .add_output_data(OUT_POSE, DataSpec::Pose)
            .add_output_data(OUT_SAMPLE_TIME, DataSpec::F32)
            .add_output_time();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rifle_movement_follows_the_outer_linked_instance_sync_group() {
        use bevy::prelude::*;
        use bevy_animation_graph::core::{animated_scene::AnimatedSceneInstance, animation_graph::AnimationGraph,
            animation_graph_player::AnimationGraphPlayer};
        use crate::als::{state::{Gait, Stance, LocomotionState, OverlayState}, input::AlsInputCommand};
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().entity_mut(avatar).insert(crate::animation_visibility::AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones);
        app.world_mut().entity_mut(character).insert((avian3d::prelude::RigidBody::Kinematic, bevy_tnua::TnuaToggle::Disabled));
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().overlay_state = Some(OverlayState::Rifle);
        app.add_systems(PostUpdate, (move |mut actors: Query<(&mut crate::actual_motion::AlsActualMotion,
            &mut Gait, &mut Stance, &mut LocomotionState, &mut AlsInputCommand)>| {
            let (mut motion, mut gait, mut stance, mut mode, mut input) = actors.get_mut(character).unwrap();
            *gait = Gait::Running;
            *stance = Stance::Standing;
            *mode = LocomotionState::Grounded;
            motion.horizontal_speed = 3.5;
            motion.world_velocity = Vec3::NEG_Z * 3.5;
            motion.local_velocity = Vec2::new(0.0, 3.5);
            motion.has_input = true;
            input.move_dir = Vec2::Y;
        }).after(crate::standing_layer::AlsStandingLayerSet).before(crate::ab_als_root::AlsRootGraphSet));
        for _ in 0..150 { app.update(); }
        let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        let instance = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        assert!(instance.get_error().is_none(), "Actual Rifle graph failed: {:?}", instance.get_error());
        let arena = instance.get_context_arena().unwrap();
        let graphs = app.world().resource::<Assets<AnimationGraph>>();
        let mut ticks = Vec::<SourceSyncPlayerTick>::new();
        for id in arena.iter_context_ids() {
            let context = arena.get_context(id).unwrap();
            let graph = graphs.get(context.get_graph_id()).unwrap();
            for node in graph.nodes.values() {
                if !node.try_inner_downcast_ref::<crate::extensions::SourcePosePassthroughNode>().is_some_and(|node| node.synchronize_groups)
                    || !context.node_caches.is_updated(node.id, bevy_animation_graph::core::context::node_states::StateKey::Default) { continue; }
                let state = context.node_states.get::<crate::extensions::AnimationContextState>(node.id,
                    bevy_animation_graph::core::context::node_states::StateKey::Default).unwrap();
                for tick in state.sync_players.iter().filter(|tick| tick.key == "sync_groups.movement.phase") {
                    if !ticks.iter().any(|prior| prior.identity == tick.identity) { ticks.push(tick.clone()); }
                }
            }
        }
        let standing = ticks.iter().filter(|tick| tick.role == SourceSyncGroupRole::CanBeLeader)
            .max_by(|a,b| a.weight.total_cmp(&b.weight)).expect("Actual Standing Movement leader is missing");
        let follower = ticks.iter().filter(|tick| tick.role == SourceSyncGroupRole::AlwaysFollower)
            .max_by(|a,b| a.weight.total_cmp(&b.weight)).unwrap();
        assert!(follower.role == SourceSyncGroupRole::AlwaysFollower && standing.role == SourceSyncGroupRole::CanBeLeader);
        let actual_context = arena.get_context(follower.identity.0).unwrap();
        let actual_states = actual_context.node_states.get_all_upcoming_states::<crate::extensions::source_sequence_player::SourceSequencePlayerState>(follower.identity.1)
            .unwrap().filter(|state| state.context.sync_players.iter().any(|tick| tick.identity == follower.identity)).collect::<Vec<_>>();
        assert_eq!(actual_states.len(), 1, "Fixture did not reach a unique active Rifle sampling state");
        let actual_state = actual_states[0];
        let actual = actual_state.context.relevant_asset.as_ref().unwrap().position;
        let expected = standing.marker_advance.as_ref().and_then(|advance| advance.end.as_ref())
            .and_then(|position| time_for_marker_position(&follower.markers, follower.duration, position, actual, follower.looped))
            .unwrap_or_else(|| marker_phase_sample_time(standing.phase, standing.phase_duration, &standing.phase_markers,
                follower.duration, &follower.markers, follower.looped));
        let clips = app.world().resource::<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>();
        let clip = clips.get(follower.source_asset.as_ref().unwrap()).unwrap();
        let actual_pose = crate::extensions::source_sequence_player::sample_pose(clip, actual);
        let expected_pose = crate::extensions::source_sequence_player::sample_pose(clip, expected);
        let mut rotation_error = 0.0_f32;
        let mut translation_error = 0.0_f32;
        for bone in actual_pose.paths.keys() {
            let a = actual_pose.get_bone(*bone).unwrap();
            let b = expected_pose.get_bone(*bone).unwrap();
            if let (Some(a), Some(b)) = (a.rotation, b.rotation) { rotation_error = rotation_error.max(a.angle_between(b)); }
            if let (Some(a), Some(b)) = (a.translation, b.translation) { translation_error = translation_error.max(a.distance(b)); }
        }
        assert!((actual - expected).abs() < 1e-4,
            "Rifle Movement was isolated from the outer Standing leader: actual={actual}, expected={expected}, standing_phase={}, rifle_phase={}, selected_as_leader={}, rotation_degrees={}, translation_meters={translation_error}",
            standing.phase, follower.phase, follower.selected_leader, rotation_error.to_degrees());
    }

    fn markers(entries: &[(&str, f32)]) -> Vec<SyncMarker> {
        entries
            .iter()
            .map(|(name, time)| SyncMarker {
                name: (*name).to_owned(),
                time: *time,
            })
            .collect()
    }

    #[test]
    fn leader_resets_on_join_and_wraps_phase() {
        let mut state = SyncPhaseState::default();
        let input = SyncPhaseInput {
            active: true,
            reset: false,
            rate: 1.0,
            duration: 1.0,
            start_phase: 0.2,
            looped: true,
            delta_seconds: 0.5,
        };
        assert!((advance_sync_phase(&mut state, input) - 0.2).abs() < EPSILON);
        assert!((advance_sync_phase(&mut state, input) - 0.7).abs() < EPSILON);
        assert!(
            (advance_sync_phase(
                &mut state,
                SyncPhaseInput {
                    delta_seconds: 0.5,
                    ..input
                }
            ) - 0.2)
                .abs()
                < EPSILON
        );
        advance_sync_phase(
            &mut state,
            SyncPhaseInput {
                active: false,
                ..input
            },
        );
        assert!((advance_sync_phase(&mut state, input) - 0.2).abs() < EPSILON);
    }

    #[test]
    fn new_override_leader_uses_its_own_marker_phase() {
        let mut clips = Assets::<bevy_animation_graph::core::animation_clip::GraphClip>::default();
        let old_clip = clips.add(bevy_animation_graph::core::animation_clip::GraphClip::default());
        let new_clip = clips.add(bevy_animation_graph::core::animation_clip::GraphClip::default());
        let old_id: NodeId = serde_json::from_str("\"10000000-0000-4000-8000-000000000001\"").unwrap();
        let new_id: NodeId = serde_json::from_str("\"10000000-0000-4000-8000-000000000002\"").unwrap();
        let tick = |id, clip, phase, override_position| SourceSyncPlayerTick {
            identity: (GraphContextId::default(), id),
            key: "Movement".to_owned(),
            role: SourceSyncGroupRole::CanBeLeader,
            source_asset: Some(clip),
            override_position_when_joining_sync_group_as_leader: override_position,
            weight: 1.0,
            phase,
            previous_phase: phase,
            previous_marker_record: None,
            marker_advance: None,
            sample_time_hint: None,
            length_play_rate: None,
            phase_duration: 1.0,
            phase_markers: markers(&[("Left", 0.0), ("Right", 0.5)]),
            duration: 1.0,
            markers: markers(&[("Left", 0.0), ("Right", 0.5)]),
            looped: true,
            delta_seconds: 0.1,
            play_rate: 1.0,
            has_been_full_weight: false,
            requested_inertialization: false,
            selected_leader: false,
            ticked_as_leader: false,
        };
        let old = tick(old_id, old_clip.clone(), 0.8, false);
        let mut joining = tick(new_id, new_clip.clone(), 0.2, true);
        joining.phase = 0.3;
        let phase = |current: &SourceSyncPlayerTick, previous: &[SourceSyncPlayerTick]| {
            resolve_source_sync_phases(&[current.clone()], previous)[0].phase.unwrap()
        };
        assert!((phase(&joining, &[old.clone()]) - 0.3).abs() < EPSILON);
        joining.override_position_when_joining_sync_group_as_leader = false;
        assert!((phase(&joining, &[old]) - 0.9).abs() < EPSILON);
        joining.override_position_when_joining_sync_group_as_leader = true;
        let mut same = joining.clone();
        same.phase = 0.8;
        assert!((phase(&joining, &[same.clone()]) - 0.9).abs() < EPSILON);
        same.source_asset = Some(old_clip);
        assert!((phase(&joining, &[same]) - 0.3).abs() < EPSILON);
        assert!((phase(&joining, &[]) - 0.3).abs() < EPSILON);
        joining.phase_markers.clear();
        joining.markers.clear();
        let mut old_length = tick(old_id, new_clip, 0.8, false);
        old_length.phase_markers.clear();
        old_length.markers.clear();
        assert!((phase(&joining, &[old_length.clone()]) - 0.3).abs() < EPSILON);
        joining.override_position_when_joining_sync_group_as_leader = false;
        assert!((phase(&joining, &[old_length]) - 0.3).abs() < EPSILON);
    }

    #[test]
    fn non_looping_phase_clamps_at_the_end() {
        let mut state = SyncPhaseState {
            phase: 0.9,
            active: true,
            reset_active: false,
        };
        let phase = advance_sync_phase(
            &mut state,
            SyncPhaseInput {
                active: true,
                reset: false,
                rate: 2.0,
                duration: 1.0,
                start_phase: 0.0,
                looped: false,
                delta_seconds: 0.1,
            },
        );
        assert_eq!(phase, 1.0);
    }

    #[test]
    fn marker_follower_matches_the_leader_marker_interval() {
        let leader = markers(&[("Left", 0.083333), ("Right", 0.65)]);
        let follower = markers(&[("Left", 0.033333), ("Right", 0.433333)]);
        let time = follower_sample_time(0.2, 1.133333, &leader, 0.8, &follower, true);
        assert!((time - 0.13451).abs() < 1e-3);
    }

    #[test]
    fn marker_records_preserve_endpoints_and_reset_for_asset_and_group_changes() {
        let adjacent = markers(&[("A", 0.125), ("A", 0.375), ("B", 0.75)]);
        let mut clips = Assets::<bevy_animation_graph::core::animation_clip::GraphClip>::default();
        let first = clips.add(bevy_animation_graph::core::animation_clip::GraphClip::default());
        let second = clips.add(bevy_animation_graph::core::animation_clip::GraphClip::default());
        let passed = SourceMarkerAdvance { forward: true, markers: vec!["A".into()], ..default() };
        let empty = SourceMarkerAdvance { forward: true, markers: vec![], ..default() };
        let mut record = None;
        assert_eq!(marker_phase_sample_time_continuous(0.1875, &passed, 1.0, &adjacent,
            1.0, &adjacent, 0.875, true, &mut record, Some(first.clone())), (0.375, 0.875));
        let saved = record.clone();
        assert_eq!((record.as_ref().unwrap().previous, record.as_ref().unwrap().next), (0, 0));
        for looped in [true, false] {
            let mut record = saved.clone();
            assert_eq!(marker_phase_sample_time_continuous(0.203125, &empty, 1.0, &adjacent,
                1.0, &adjacent, 0.375, looped, &mut record, Some(first.clone())), (0.4375, 0.375));
        }
        assert_eq!(marker_phase_sample_time_continuous(0.203125, &empty, 1.0, &adjacent,
            1.0, &adjacent, 0.375, true, &mut record, Some(second.clone())), (0.203125, 0.375));

        let ids: [NodeId; 4] = std::array::from_fn(|index| serde_json::from_value(
            serde_json::Value::String(format!("10000000-0000-4000-8000-00000000000{}", index + 1))).unwrap());
        let ticks = |delta| [0, 1, 2].map(|index| SourceSyncPlayerTick {
            identity: (GraphContextId::default(), ids[index]), key: "Movement".into(),
            role: if index == 0 { SourceSyncGroupRole::AlwaysLeader } else { SourceSyncGroupRole::AlwaysFollower },
            weight: 1.0 / (index + 1) as f32, selected_leader: index == 0, ticked_as_leader: index == 0,
            phase: 0.625, previous_phase: 0.625, duration: if index == 0 { 1.0 } else { 2.0 },
            sample_time_hint: Some(if index == 0 { 0.625 } else { 1.25 }),
            phase_duration: 1.0, phase_markers: markers(&[("A", 0.25), ("B", 0.75)]),
            markers: markers(if index == 0 { &[("A", 0.25), ("B", 0.75)] } else { &[("A", 0.5), ("B", 1.5)] }),
            source_asset: Some(first.clone()), looped: false, delta_seconds: delta, play_rate: 1.0, ..default()
        });
        let previous = ticks(0.0);
        assert!(resolve_source_sync_phases(&ticks(0.0625), &previous)[0].marker_resets.is_empty());
        let result = resolve_source_sync_phases(&ticks(0.25), &previous);
        assert_eq!(result[0].marker_resets, vec![previous[2].identity]);
        let mut current = ticks(0.0625);
        current[2].identity.1 = ids[3];
        assert_eq!(resolve_source_sync_phases(&current, &previous)[0].marker_resets, vec![current[2].identity]);
        current = ticks(0.0625);
        current[2].source_asset = Some(second);
        assert_eq!(resolve_source_sync_phases(&current, &previous)[0].marker_resets, vec![current[2].identity]);
        current = ticks(0.0625);
        current[2].markers.retain(|marker| marker.name == "A");
        assert_eq!(resolve_source_sync_phases(&current, &previous)[0].marker_resets,
            current.iter().map(|player| player.identity).collect::<Vec<_>>());
        current = ticks(0.0625);
        current[2].role = SourceSyncGroupRole::AlwaysLeader;
        assert!(resolve_source_sync_phases(&current, &previous)[0].marker_resets.is_empty());
    }

    #[test]
    fn markerless_sync_leader_keeps_native_seconds_and_ratios() {
        #[derive(serde::Deserialize)]
        struct Trace {
            asset: String,
            name: String,
            duration: f32,
            rate: f32,
            looped: bool,
            frames: Vec<[f32; 4]>,
        }
        #[derive(serde::Deserialize)]
        struct Reference {
            cases: Vec<Trace>,
        }

        let reference: Reference = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d201/clock.replay.json"
        )).unwrap();
        for trace in reference.cases {
            let mut position = trace.frames[0][1];
            let mut previous = None;
            for [delta, native_previous, native_position, ratio] in trace.frames {
                let tick = SourceSyncPlayerTick {
                    key: "fixture".into(), role: SourceSyncGroupRole::CanBeLeader, weight: 1.0,
                    duration: trace.duration, phase_duration: trace.duration, play_rate: trace.rate,
                    delta_seconds: delta, looped: trace.looped, sample_time_hint: Some(position),
                    ..default()
                };
                let group = resolve_source_sync_phases(std::slice::from_ref(&tick), previous.as_slice()).remove(0);
                let (_, step) = &group.leader_samples[0];
                assert_eq!((step.previous_time, step.time), (native_previous, native_position),
                    "{} {} at {native_previous}", trace.asset, trace.name);
                assert_eq!(group.length_ratios.unwrap().y, ratio);
                position = step.time;
                previous = Some(tick);
            }
        }
    }

    #[test]
    fn markerless_blendspace_sync_uses_the_weighted_rate_when_group_markers_are_filtered_out() {
        let leader = SourceSyncPlayerTick {
            key: "Movement".into(), role: SourceSyncGroupRole::AlwaysLeader, weight: 1.0,
            duration: 2.0, phase_duration: 2.0, sample_time_hint: Some(0.0), delta_seconds: 0.1,
            play_rate: 1.0, length_play_rate: Some(1.4), looped: true,
            markers: markers(&[("A", 0.2), ("B", 1.2)]), phase_markers: markers(&[("A", 0.2), ("B", 1.2)]),
            ..default()
        };
        for filtered in [false, true] {
        let follower = SourceSyncPlayerTick {
            key: "Movement".into(), role: SourceSyncGroupRole::AlwaysFollower, weight: 0.5,
            duration: 1.0, phase_duration: 1.0, markers: markers(if filtered { &[("X", 0.1), ("Y", 0.6)] } else { &[("A", 0.1), ("B", 0.6)] }),
            ..default()
        };
        let result = resolve_source_sync_phases(&[leader.clone(), follower], &[]);
        assert_eq!(result[0].markers.is_empty(), filtered);
        assert!((result[0].phase.unwrap() - if filtered { 0.07 } else { 0.05 }).abs() < 1e-6);
        assert!((result[0].leader_samples[0].1.time - if filtered { 0.14 } else { 0.1 }).abs() < 1e-6);
        }
    }

    #[test]
    fn leader_resync_without_markers_preserves_the_new_players_time() {
        let mut clips = Assets::<bevy_animation_graph::core::animation_clip::GraphClip>::default();
        let prior_clip = clips.add(bevy_animation_graph::core::animation_clip::GraphClip::default());
        let clip = clips.add(bevy_animation_graph::core::animation_clip::GraphClip::default());
        let prior = SourceSyncPlayerTick {
            key: "Movement".into(), source_asset: Some(prior_clip), selected_leader: true,
            phase: 0.8, previous_phase: 0.8, duration: 1.0, phase_duration: 1.0, weight: 1.0,
            ..default()
        };
        let current = SourceSyncPlayerTick {
            key: "Movement".into(), source_asset: Some(clip), phase: 0.2, previous_phase: 0.2,
            sample_time_hint: Some(0.2), duration: 1.0, phase_duration: 1.0, weight: 1.0,
            delta_seconds: 0.1, play_rate: 1.0, ..default()
        };
        let result = resolve_source_sync_phases(&[current], &[prior]);
        assert!((result[0].leader_samples[0].1.previous_time - 0.2).abs() < 1e-6);
        assert!((result[0].leader_samples[0].1.time - 0.3).abs() < 1e-6);
    }

    #[test]
    fn leader_resync_length_rules_preserve_override_inertialization_and_scores() {
        let mut clips = Assets::<bevy_animation_graph::core::animation_clip::GraphClip>::default();
        let old = clips.add(bevy_animation_graph::core::animation_clip::GraphClip::default());
        let new = clips.add(bevy_animation_graph::core::animation_clip::GraphClip::default());
        for previous_weight in [0.25_f32, 0.5, 0.75] {
            for weight in [0.25, 0.5, 0.75] {
                for requested in [false, true] {
                    for override_position in [false, true] {
                        for rate in [-1.0_f32, 0.0, 1.0] {
                            let previous = SourceSyncPlayerTick {
                                key: "Movement".into(), source_asset: Some(old.clone()), selected_leader: true,
                                phase: 0.8, previous_phase: 0.8, duration: 1.0, phase_duration: 1.0, weight: previous_weight,
                                ..default()
                            };
                            let current = SourceSyncPlayerTick {
                                key: "Movement".into(), source_asset: Some(new.clone()), phase: 0.2, previous_phase: 0.2,
                                sample_time_hint: Some(0.2), duration: 1.0, phase_duration: 1.0, weight,
                                requested_inertialization: requested, override_position_when_joining_sync_group_as_leader: override_position,
                                delta_seconds: 0.1, play_rate: rate, ..default()
                            };
                            let result = resolve_source_sync_phases(&[current], &[previous]);
                            let start = if requested && !override_position && previous_weight >= weight { 0.8 } else { 0.2 };
                            let step = &result[0].leader_samples[0].1;
                            assert!((step.previous_time - start).abs() < 1e-6);
                            assert!((step.time - (start + 0.1 * rate)).abs() < 1e-6);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn leader_resync_marker_changes_use_valid_marker_sync_or_qualified_length_resync() {
        let mut clips = Assets::<bevy_animation_graph::core::animation_clip::GraphClip>::default();
        let old = clips.add(bevy_animation_graph::core::animation_clip::GraphClip::default());
        let new = clips.add(bevy_animation_graph::core::animation_clip::GraphClip::default());
        for (previous_markers, current_markers, expected_marker_sync) in [
            (vec![], markers(&[("A", 0.1), ("B", 0.6)]), false),
            (markers(&[("A", 0.1), ("B", 0.6)]), vec![], false),
            (markers(&[("A", 0.1), ("B", 0.6)]), markers(&[("A", 0.1), ("B", 0.6)]), true),
        ] {
            for requested in [false, true] {
                for override_position in [false, true] {
                    let previous = SourceSyncPlayerTick { key: "Movement".into(), source_asset: Some(old.clone()),
                        selected_leader: true, phase: 0.8, previous_phase: 0.8, markers: previous_markers.clone(),
                        phase_markers: previous_markers.clone(), duration: 1.0, phase_duration: 1.0, weight: 1.0, ..default() };
                    let current = SourceSyncPlayerTick { key: "Movement".into(), source_asset: Some(new.clone()),
                        phase: 0.2, previous_phase: 0.2, sample_time_hint: Some(0.2), markers: current_markers.clone(),
                        phase_markers: current_markers.clone(), duration: 1.0, phase_duration: 1.0, weight: 1.0,
                        requested_inertialization: requested, override_position_when_joining_sync_group_as_leader: override_position,
                        delta_seconds: 0.1, play_rate: 1.0, ..default() };
                    let result = resolve_source_sync_phases(&[current], &[previous]);
                    let start = if !override_position && (expected_marker_sync || requested) { 0.8 } else { 0.2 };
                    assert!((result[0].leader_samples[0].1.previous_time - start).abs() < 1e-6,
                        "Wrong source qualification: markers={expected_marker_sync}, inertial={requested}, override={override_position}");
                }
            }
        }
    }

    #[test]
    fn repeated_marker_continuity_preserves_multi_interval_self_mapping() {
        let markers = [("A", 0.1), ("B", 0.3), ("A", 0.6), ("B", 0.8)]
            .map(|(name, time)| SyncMarker { name: name.into(), time });
        for looped in [true, false] {
            for (previous, next, forward, passed) in [(0.15, 0.75, true, vec!["B", "A"]),
                (0.75, 0.15, false, vec!["A", "B"]), (0.15, 0.8, true, vec!["B", "A"]),
                (0.8, 0.1, false, vec!["A", "B"])] {
                let advance = SourceMarkerAdvance { forward, markers: passed.into_iter().map(str::to_owned).collect(), ..default() };
                let actual = marker_phase_sample_time_continuous(next, &advance, 1.0, &markers,
                    1.0, &markers, previous, looped, &mut None, None).0;
                assert!((actual - next).abs() < 1e-5, "{previous} -> {next}, looped={looped}: {actual}");
            }
        }
    }

    #[test]
    fn repeated_marker_steps_preserve_signed_cycles_and_nonuniform_spacing() {
        let markers = [("A", 0.1), ("B", 0.3), ("A", 0.6), ("B", 0.8)]
            .map(|(name, time)| SyncMarker { name: name.into(), time }).to_vec();
        for (previous, rate, phase, steps) in [(0.15, 3.0, 0.75, 2), (0.75, -3.0, 0.15, -2),
            (0.15, 13.0, 0.75, 10), (0.75, -13.0, 0.15, -10), (0.1, 2.5, 0.6, 2)] {
            let player = SourceSyncPlayerTick { phase_duration: 1.0, duration: 1.0,
                phase_markers: markers.clone(), markers: markers.clone(), looped: true,
                sample_time_hint: Some(previous), delta_seconds: 0.2, play_rate: rate, ..default() };
            let SourceSyncPlayerStep { phase: actual, advance, .. } = player.advance_from_time(previous);
            assert!((actual - phase).abs() < 1e-5, "{previous} at {rate}: {actual}");
            assert_eq!(advance.forward, steps > 0);
            assert_eq!(advance.markers.len() as i64, i64::abs(steps));
        }
        let phase_markers = [("A", 0.1), ("B", 0.6)]
            .map(|(name, time)| SyncMarker { name: name.into(), time }).to_vec();
        let sample_markers = [("A", 0.05), ("B", 0.2), ("A", 0.35), ("B", 0.5), ("A", 0.7), ("B", 0.9)]
            .map(|(name, time)| SyncMarker { name: name.into(), time });
        for (rate, previous_sample, expected, steps) in [(10.0, 0.08, 0.74, 4), (-10.0, 0.74, 0.08, -4)] {
            let player = SourceSyncPlayerTick { key: "Movement".into(), phase: 0.2,
                phase_duration: 1.0, duration: 1.0, phase_markers: phase_markers.clone(),
                markers: phase_markers.clone(), looped: true, sample_time_hint: Some(0.2),
                delta_seconds: 0.2, play_rate: rate, ..default() };
            let resolved = resolve_source_sync_phases(&[player.clone()], &[player]);
            let advance = resolved[0].marker_advance.as_ref().unwrap();
            assert_eq!(advance.forward, steps > 0);
            assert_eq!(advance.markers.len() as i64, i64::abs(steps));
            let actual = marker_phase_sample_time_continuous(resolved[0].phase.unwrap(), advance,
                1.0, &phase_markers, 1.0, &sample_markers, previous_sample, true, &mut None, None).0;
            assert!((actual - expected).abs() < 1e-5, "rate={rate}: {actual} != {expected}");
        }
        let source = [("A", 0.1), ("B", 0.2), ("A", 0.4), ("C", 0.6), ("A", 0.8), ("B", 0.9)]
            .map(|(name, time)| SyncMarker { name: name.into(), time });
        let target = [("A", 0.05), ("B", 0.15), ("A", 0.25), ("B", 0.35), ("A", 0.45), ("C", 0.65), ("A", 0.75), ("B", 0.85)]
            .map(|(name, time)| SyncMarker { name: name.into(), time });
        for (previous, rate, hint, expected, passed) in [(0.12, 7.0, 0.07, 0.77, vec!["B", "A", "C", "A"]),
            (0.82, -7.0, 0.77, 0.27, vec!["A", "C", "A", "B"])] {
            let player = SourceSyncPlayerTick { phase_duration: 1.0, duration: 1.0,
                phase_markers: source.to_vec(), markers: source.to_vec(), looped: true,
                sample_time_hint: Some(previous), delta_seconds: 0.1, play_rate: rate, ..default() };
            let SourceSyncPlayerStep { phase, advance, .. } = player.advance(previous);
            assert_eq!(advance.markers, passed);
            let actual = marker_phase_sample_time_continuous(phase, &advance, 1.0, &source, 1.0, &target, hint, true, &mut None, None).0;
            assert!((actual - expected).abs() < 1e-5, "Mixed repeated marker path: {actual} != {expected}");
        }
    }

    #[test]
    fn repeated_marker_pairs_follow_the_nearest_timeline_occurrence() {
        let phase = markers(&[("A", 0.1), ("B", 0.3), ("A", 0.6), ("B", 0.8)]);
        let sample = markers(&[("A", 0.05), ("B", 0.25), ("A", 0.55), ("B", 0.95)]);
        for (phase_time, expected) in [(0.65, 0.65), (0.62, 0.59), (0.61, 0.57), (0.95, 1.0)] {
            let actual = marker_phase_sample_time(phase_time, 1.0, &phase, 1.0, &sample, true);
            assert!((actual - expected).abs() < 1e-5, "phase={phase_time} time={actual} expected={expected}");
        }
        let player = SourceSyncPlayerTick {
            phase_duration: 1.0, phase_markers: phase,
            duration: 1.0, markers: sample,
            sample_time_hint: Some(0.65), delta_seconds: 0.1, play_rate: 1.0,
            looped: true, ..default()
        };
        assert!((player.advance(0.2).phase - 0.75).abs() < 1e-5);
    }

    #[test]
    fn follower_uses_normalized_duration_when_markers_are_absent() {
        let time = follower_sample_time(0.2, 1.133333, &[], 0.8, &[], true);
        assert!((time - 0.16).abs() < EPSILON);
    }

    #[test]
    fn marker_phase_preserves_the_foot_interval_across_leader_changes() {
        let template = markers(&[("Left", 0.0), ("Right", 0.5)]);
        let forward = markers(&[("Left", 0.083333), ("Right", 0.65)]);
        let backward = markers(&[("Right", 0.516667), ("Left", 1.083333)]);
        let mut state = SyncPhaseState::default();
        let forward_phase = advance_marker_sync_phase(
            &mut state,
            SyncMarkerPhaseInput {
                active: true,
                reset: false,
                rate: 1.0,
                leader_duration: 1.133333,
                leader_markers: &forward,
                phase_duration: 1.0,
                phase_markers: &template,
                start_time: 0.2 * 1.133333,
                looped: true,
                delta_seconds: 0.0,
            },
        );
        let forward_time =
            marker_phase_sample_time(forward_phase, 1.0, &template, 1.133333, &forward, true);
        let backward_time =
            marker_phase_sample_time(forward_phase, 1.0, &template, 1.133333, &backward, true);
        let forward_position =
            marker_position_at_time(&forward, 1.133333, forward_time, true).unwrap();
        let backward_position =
            marker_position_at_time(&backward, 1.133333, backward_time, true).unwrap();
        assert!((forward_time - 0.2266666).abs() < 1e-5);
        assert_eq!(forward_position.previous, backward_position.previous);
        assert_eq!(forward_position.next, backward_position.next);
        assert!(
            (forward_position.alpha - backward_position.alpha).abs() < EPSILON,
            "forward={forward_position:?} backward={backward_position:?}"
        );
    }

    #[test]
    fn marker_phase_uses_the_sequence_start_time_on_activation() {
        let template = markers(&[("Left", 0.0), ("Right", 0.5)]);
        let sprint = markers(&[("Left", 0.016667), ("Right", 0.316667)]);
        let mut state = SyncPhaseState::default();
        let phase = advance_marker_sync_phase(
            &mut state,
            SyncMarkerPhaseInput {
                active: true,
                reset: false,
                rate: 1.0,
                leader_duration: 0.6,
                leader_markers: &sprint,
                phase_duration: 1.0,
                phase_markers: &template,
                start_time: 0.166667,
                looped: true,
                delta_seconds: 0.0,
            },
        );
        let sprint_time = marker_phase_sample_time(phase, 1.0, &template, 0.6, &sprint, true);
        assert!((sprint_time - 0.166667).abs() < 1e-5);
    }

    #[test]
    fn registry_keeps_group_phase_across_ecs_callers() {
        let mut registry = SyncGroupPhaseRegistry::default();
        let input = SyncPhaseInput {
            active: true,
            reset: false,
            rate: 1.0,
            duration: 1.0,
            start_phase: 0.2,
            looped: true,
            delta_seconds: 0.0,
        };
        assert!((registry.advance("Movement", input) - 0.2).abs() < EPSILON);
        assert!(
            (registry.advance(
                "Movement",
                SyncPhaseInput {
                    delta_seconds: 0.25,
                    ..input
                },
            ) - 0.45)
                .abs()
                < EPSILON
        );
        assert_eq!(registry.phase("Movement"), Some(0.45));
    }

    #[test]
    fn explicit_asset_reset_uses_the_source_start_phase_once() {
        let mut state = SyncPhaseState {
            phase: 0.7,
            active: true,
            reset_active: false,
        };
        let input = SyncPhaseInput {
            active: true,
            reset: true,
            rate: 1.0,
            duration: 1.0,
            start_phase: 0.2,
            looped: true,
            delta_seconds: 0.25,
        };
        assert!((advance_sync_phase(&mut state, input) - 0.2).abs() < EPSILON);
        assert!((advance_sync_phase(&mut state, input) - 0.45).abs() < EPSILON);
    }

    #[test]
    fn source_group_leader_scoring_matches_ue_roles() {
        let candidates = [
            SourceSyncGroupCandidate {
                role: SourceSyncGroupRole::AlwaysFollower,
                blend_weight: 1.0,
                source_order: 0,
            },
            SourceSyncGroupCandidate {
                role: SourceSyncGroupRole::CanBeLeader,
                blend_weight: 0.75,
                source_order: 1,
            },
            SourceSyncGroupCandidate {
                role: SourceSyncGroupRole::AlwaysLeader,
                blend_weight: 0.0,
                source_order: 2,
            },
        ];
        assert_eq!(source_sync_group_leader_score(candidates[0]), -1.0);
        assert_eq!(source_sync_group_leader_score(candidates[1]), 0.75);
        assert_eq!(source_sync_group_leader_score(candidates[2]), 2.0);
        assert_eq!(ordered_source_sync_candidates(&candidates)[0], 2);
    }

    #[test]
    fn transition_roles_join_only_after_full_weight() {
        let first: NodeId = serde_json::from_str("\"10000000-0000-4000-8000-000000000001\"").unwrap();
        let second: NodeId = serde_json::from_str("\"10000000-0000-4000-8000-000000000002\"").unwrap();
        let third: NodeId = serde_json::from_str("\"10000000-0000-4000-8000-000000000003\"").unwrap();
        let tick = |id, role, weight, phase| SourceSyncPlayerTick {
            identity: (GraphContextId::default(), id), key: "Movement".into(), role, weight,
            phase, previous_phase: phase, phase_duration: 1.0, duration: 1.0,
            looped: true, delta_seconds: 0.0, play_rate: 1.0, ..default()
        };
        let entering = tick(first, SourceSyncGroupRole::TransitionLeader, 0.4, 0.8);
        let follower = tick(second, SourceSyncGroupRole::TransitionFollower, 0.8, 0.6);
        let normal = tick(third, SourceSyncGroupRole::CanBeLeader, 0.2, 0.2);
        let result = resolve_source_sync_phases(&[entering.clone(), follower.clone(), normal.clone()], &[]);
        assert_eq!(result.len(), 1);
        assert!((result[0].phase.unwrap() - 0.2).abs() < EPSILON);
        assert_eq!(result[0].ungrouped, vec![entering.identity, follower.identity]);
        let alone = resolve_source_sync_phases(&[entering.clone(), follower.clone()], &[]);
        assert_eq!(alone[0].phase, None);
        assert_eq!(alone[0].ungrouped, vec![entering.identity, follower.identity]);
        let mut full = entering.clone();
        full.weight = 1.0 - 1e-5;
        let joined = resolve_source_sync_phases(&[full.clone(), normal.clone()], &[]);
        assert!((joined[0].phase.unwrap() - 0.8).abs() < EPSILON);
        assert!(joined[0].ungrouped.is_empty());
        full.weight = 0.4;
        full.has_been_full_weight = true;
        let fading_out = resolve_source_sync_phases(&[full.clone(), normal.clone()], &[]);
        assert!((fading_out[0].phase.unwrap() - 0.8).abs() < EPSILON);
        assert!(fading_out[0].ungrouped.is_empty());
        let mut joined_follower = follower;
        joined_follower.has_been_full_weight = true;
        let followers = resolve_source_sync_phases(&[joined_follower, normal], &[]);
        assert_eq!(followers[0].ungrouped.len(), 0);
        assert!((followers[0].phase.unwrap() - 0.2).abs() < EPSILON);
        full.requested_inertialization = true;
        let inertialized = resolve_source_sync_phases(&[full, entering], &[]);
        assert_eq!(inertialized[0].phase, None);
        assert_eq!(inertialized[0].ungrouped.len(), 2);
    }

    #[test]
    fn sync_group_uses_only_marker_names_shared_by_marked_players() {
        let leader = SourceSyncPlayerTick {
            key: "Movement".into(),
            role: SourceSyncGroupRole::AlwaysLeader,
            weight: 0.6,
            phase_duration: 1.0,
            phase_markers: markers(&[("A", 0.0), ("B", 0.8)]),
            duration: 1.0,
            markers: markers(&[("A", 0.0), ("Extra", 0.25), ("B", 0.5)]),
            phase: 0.3,
            previous_phase: 0.2,
            delta_seconds: 0.1,
            play_rate: 1.0,
            looped: true,
            ..default()
        };
        let follower = SourceSyncPlayerTick {
            key: "Movement".into(),
            role: SourceSyncGroupRole::AlwaysFollower,
            weight: 0.4,
            phase_duration: 1.0,
            phase_markers: markers(&[("A", 0.0), ("B", 0.8)]),
            duration: 1.0,
            markers: markers(&[("A", 0.0), ("B", 0.8)]),
            ..default()
        };
        let shared = resolve_source_sync_phases(&[leader.clone(), follower.clone()], &[]);
        assert!((shared[0].phase.unwrap() - 0.48).abs() < EPSILON, "{:?}", shared[0].phase);
        assert_eq!(shared[0].markers, ["A", "B"]);
        let mut unmarked = follower.clone();
        unmarked.markers.clear();
        let unmarked_group = resolve_source_sync_phases(&[leader.clone(), unmarked], &[]);
        assert_eq!(unmarked_group[0].markers, ["A", "B", "Extra"]);
        let mut disjoint = follower.clone();
        disjoint.markers = markers(&[("C", 0.0), ("D", 0.8)]);
        let length_sync = resolve_source_sync_phases(&[leader.clone(), disjoint], &[]);
        assert!(length_sync[0].markers.is_empty());
        assert!((length_sync[0].phase.unwrap() - 0.3).abs() < EPSILON);
        let mut single = follower.clone();
        single.markers = markers(&[("A", 0.0), ("C", 0.8)]);
        let single_group = resolve_source_sync_phases(&[leader.clone(), single], &[]);
        assert_eq!(single_group[0].markers, ["A"]);
        let single_clock = marker_phase_sample_time(
            0.3, 1.0, &markers(&[("A", 0.2)]), 1.0, &markers(&[("A", 0.5)]), true);
        assert!((single_clock - 0.6).abs() < EPSILON);
        let mut next = leader.clone();
        next.phase = 0.4;
        next.previous_phase = 0.3;
        let changed = resolve_source_sync_phases(&[next.clone(), follower.clone()], &[leader.clone()]);
        assert!((changed[0].phase.unwrap() - 0.64).abs() < EPSILON, "{:?}", changed[0].phase);
        let mut prior_valid = leader;
        prior_valid.phase = 0.9;
        let retained = resolve_source_sync_phases(&[next, follower], &[prior_valid]);
        assert!((retained[0].phase.unwrap() - 0.94).abs() < EPSILON, "{:?}", retained[0].phase);
    }

    #[test]
    fn browser_review_losing_exclusive_markers_force_length_sync() {
        let ids: [NodeId; 3] = std::array::from_fn(|index| serde_json::from_value(
            serde_json::Value::String(format!("13500000-0000-4000-8000-00000000000{}", index + 1))).unwrap());
        let tick = |index, role, weight, names: &[(&str, f32)]| SourceSyncPlayerTick {
            identity: (GraphContextId::default(), ids[index]), key: "Movement".into(),
            role, weight, phase: 0.6, phase_duration: 1.0, phase_markers: markers(names),
            duration: 1.0, markers: markers(names), looped: true, ..default()
        };
        let exclusive = tick(0, SourceSyncGroupRole::ExclusiveAlwaysLeader, 0.4, &[("X", 0.1), ("Y", 0.6)]);
        let follower = tick(1, SourceSyncGroupRole::AlwaysFollower, 0.2, &[("A", 0.2), ("B", 0.9)]);
        let leader = tick(2, SourceSyncGroupRole::AlwaysLeader, 0.4, &[("A", 0.1), ("B", 0.6)]);
        let resolved = resolve_source_sync_phases(&[exclusive.clone(), follower.clone(), leader.clone()], &[]);
        let group = &resolved[0];
        assert_eq!(group.ungrouped, [exclusive.identity]);
        assert!(group.markers.is_empty(), "A losing exclusive still participates in Prepare");
        let filtered = leader.phase_markers.iter().filter(|marker| group.markers.contains(&marker.name)).cloned().collect::<Vec<_>>();
        let sample = marker_phase_sample_time(group.phase.unwrap(), 1.0, &filtered, 1.0, &follower.markers, true);
        assert!((sample - 0.6).abs() < EPSILON, "Length-sync follower differs: {sample}");
    }

    #[test]
    fn browser_review_valid_follower_record_aligns_to_the_directional_start_pair() {
        let source = markers(&[("A", 0.0625), ("B", 0.25), ("A", 0.5625), ("B", 0.75)]);
        for (start, hint, delta) in [(0.828125, 0.578125, 0.015625), (0.984375, 0.234375, -0.015625)] {
            let mut leader = SourceMarkerRecord::at_time(&source, 1.0, true, start, None);
            let mut record = Some(SourceMarkerRecord::at_time(&source, 1.0, true, hint, None));
            let mut leader_time = start;
            let mut sample_time = hint;
            for step in 0..24 {
                let (next, advance) = leader.advance(leader_time, delta, true);
                let (sample, previous) = marker_phase_sample_time_continuous(next, &advance, 1.0, &source,
                    1.0, &source, sample_time, true, &mut record, None);
                assert!((sample - next).abs() < EPSILON,
                    "Directional start alignment differs: delta={delta}, step={step}, {sample} != {next}");
                assert!((previous - sample_time).abs() < EPSILON, "A valid record changed the previous tick time");
                if step == 0 {
                    let record = record.as_ref().unwrap();
                    assert_eq!((record.previous, record.next), (3, 0));
                }
                leader_time = next;
                sample_time = sample;
            }
        }
    }

    #[test]
    fn losing_exclusive_player_still_reduces_common_markers() {
        let first: NodeId = serde_json::from_str("\"10000000-0000-4000-8000-000000000001\"").unwrap();
        let second: NodeId = serde_json::from_str("\"10000000-0000-4000-8000-000000000002\"").unwrap();
        let tick = |id, names: &[(&str, f32)]| SourceSyncPlayerTick {
            identity: (GraphContextId::default(), id),
            key: "Movement".into(),
            role: SourceSyncGroupRole::ExclusiveAlwaysLeader,
            weight: 0.5,
            phase_duration: 1.0,
            phase_markers: markers(names),
            duration: 1.0,
            markers: markers(names),
            looped: true,
            ..default()
        };
        let loser = tick(first, &[("C", 0.0), ("D", 0.5)]);
        let winner = tick(second, &[("A", 0.0), ("B", 0.5)]);
        let result = resolve_source_sync_phases(&[loser.clone(), winner], &[]);
        assert_eq!(result[0].ungrouped, [loser.identity]);
        assert!(result[0].markers.is_empty());
    }

    #[test]
    fn source_group_leader_ties_follow_ue_role_ordering() {
        let normal = [
            SourceSyncGroupCandidate {
                role: SourceSyncGroupRole::CanBeLeader,
                blend_weight: 0.5,
                source_order: 0,
            },
            SourceSyncGroupCandidate {
                role: SourceSyncGroupRole::CanBeLeader,
                blend_weight: 0.5,
                source_order: 1,
            },
        ];
        let always = [
            SourceSyncGroupCandidate {
                role: SourceSyncGroupRole::AlwaysLeader,
                blend_weight: 0.0,
                source_order: 0,
            },
            SourceSyncGroupCandidate {
                role: SourceSyncGroupRole::ExclusiveAlwaysLeader,
                blend_weight: 0.0,
                source_order: 1,
            },
        ];
        assert_eq!(ordered_source_sync_candidates(&normal)[0], 0);
        assert_eq!(ordered_source_sync_candidates(&always)[0], 1);
    }

    #[test]
    fn nodes_register_node_like_dispatch_keys() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<SyncGroupPhaseNode>();
        registry.register::<SyncGroupSampleNode>();
        registry.register::<GlobalSyncGroupSampleNode>();
        for type_id in [
            std::any::TypeId::of::<SyncGroupPhaseNode>(),
            std::any::TypeId::of::<SyncGroupSampleNode>(),
            std::any::TypeId::of::<GlobalSyncGroupSampleNode>(),
        ] {
            assert!(
                registry
                    .get(type_id)
                    .unwrap()
                    .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                    .is_some()
            );
        }
    }
}
