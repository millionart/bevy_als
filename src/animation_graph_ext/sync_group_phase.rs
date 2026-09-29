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

#[derive(Clone, Debug, PartialEq)]
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
    pub sample_time_hint: Option<f32>,
    pub phase_duration: f32,
    pub phase_markers: Vec<SyncMarker>,
    pub duration: f32,
    pub markers: Vec<SyncMarker>,
    pub looped: bool,
    pub delta_seconds: f32,
    pub play_rate: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SourceMarkerAdvance {
    pub forward: bool,
    pub markers: Vec<String>,
}

impl SourceMarkerAdvance {
    pub fn into_inputs(self, key: &str) -> [(String, DataValue); 2] {
        let key = format!("{SOURCE_SYNC_RESOLVED}.marker_advance.{key}");
        [(format!("{key}.forward"), DataValue::Bool(self.forward)),
            (key, DataValue::EventQueue(EventQueue::with_events(self.markers.into_iter()
                .map(|name| SampledEvent::instant(AnimationEvent::StringId(name))).collect::<Vec<_>>()))) ]
    }
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

pub fn resolved_source_marker_advance(ctx: &NodeContext, key: &str) -> Result<Option<SourceMarkerAdvance>, GraphError> {
    let key = format!("{SOURCE_SYNC_RESOLVED}.marker_advance.{key}");
    let inputs = ctx.graph_context.global_input_data;
    let Some(value) = inputs.get(&key) else { return Ok(None); };
    let forward = inputs.get(&format!("{key}.forward")).ok_or(GraphError::MissingStateValue)?.as_bool()?;
    let markers = value.as_event_queue()?.events.iter().map(|event| match &event.event {
        AnimationEvent::StringId(name) => Ok(name.clone()),
        _ => Err(GraphError::MissingStateValue),
    }).collect::<Result<Vec<_>, _>>()?;
    Ok(Some(SourceMarkerAdvance { forward, markers }))
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
            && (candidate.role != SourceSyncGroupRole::ExclusiveAlwaysLeader
                || candidate.identity == leader.identity)
        {
            common.retain(|name| candidate.markers.iter().any(|marker| &marker.name == name));
        }
    }
    common
}

pub struct SourceSyncGroupResolution {
    pub key: String,
    pub phase: Option<f32>,
    pub ungrouped: Vec<(GraphContextId, NodeId)>,
    pub markers: Vec<String>,
    pub marker_advance: Option<SourceMarkerAdvance>,
    pub leader_samples: Vec<((GraphContextId, NodeId), f32)>,
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
        for leader in ordered {
            let joined = previous_leader.is_some_and(|prior| {
                prior.identity != leader.identity || prior.source_asset != leader.source_asset
            });
            let mut grouped = leader.clone();
            grouped.markers.retain(|marker| common.contains(&marker.name));
            grouped.phase_markers.retain(|marker| common.contains(&marker.name));
            let prior_phase = if leader.override_position_when_joining_sync_group_as_leader && joined {
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
            let (mut candidate_phase, advance, time) = if let Some(phase) = prior_phase {
                let continuing = common == previous_common && previous_players.iter().any(|prior|
                    prior.identity == leader.identity && prior.source_asset == leader.source_asset
                        && (prior.duration - leader.duration).abs() <= EPSILON);
                if let Some(time) = grouped.sample_time_hint.filter(|_| continuing) {
                    grouped.advance_from_time(time)
                } else {
                    grouped.advance(phase)
                }
            } else {
                let independent_time = leader.sample_time_hint.map(|time|
                    phase_to_time((time + leader.delta_seconds * leader.play_rate) / leader.duration.max(EPSILON), leader.duration, leader.looped))
                    .or_else(|| previous.iter()
                    .find(|prior| prior.identity == leader.identity && prior.source_asset == leader.source_asset
                        && (prior.duration - leader.duration).abs() <= EPSILON)
                    .map(|prior| {
                        let time = marker_phase_sample_time(prior.phase, prior.phase_duration,
                            &prior.phase_markers, prior.duration, &prior.markers, prior.looped);
                        let duration = leader.duration.max(EPSILON);
                        phase_to_time((time + leader.delta_seconds * leader.play_rate) / duration,
                            duration, leader.looped)
                    }))
                    .unwrap_or_else(|| marker_phase_sample_time(leader.phase, leader.phase_duration,
                        &leader.phase_markers, leader.duration, &leader.markers, leader.looped));
                (marker_phase_from_sample_time(independent_time, grouped.duration, &grouped.markers,
                    grouped.phase_duration, &grouped.phase_markers, grouped.looped),
                    SourceMarkerAdvance { forward: leader.play_rate >= 0.0, markers: Vec::new() }, independent_time)
            };
            let position = marker_position_at_time_in_direction(&grouped.markers, grouped.duration, time, grouped.looped, advance.forward);
            if let Some(position) = &position {
                if let Some(time) = time_for_marker_position(&grouped.phase_markers, grouped.phase_duration,
                    position, candidate_phase * grouped.phase_duration, grouped.looped) {
                    candidate_phase = time_to_phase(time, grouped.phase_duration, grouped.looped);
                }
            }
            phase = Some(candidate_phase);
            marker_advance = prior_phase.map(|_| advance);
            leader_samples.push((leader.identity, time));
            if common.is_empty() || position.is_some() { break; }
        }
        let ungrouped = players
            .iter()
            .filter(|candidate| {
                !candidate.sync_group_eligible()
                    || (candidate.role == SourceSyncGroupRole::ExclusiveAlwaysLeader
                        && !leader_samples.iter().any(|(identity, _)| *identity == candidate.identity))
            })
            .map(|candidate| candidate.identity)
            .collect();
        groups.push(SourceSyncGroupResolution { key: player.key.clone(), phase, ungrouped, markers: common, marker_advance, leader_samples });
    }
    groups
}

impl SourceSyncPlayerTick {
    pub fn sync_group_eligible(&self) -> bool {
        !matches!(self.role, SourceSyncGroupRole::TransitionLeader | SourceSyncGroupRole::TransitionFollower)
            || (!self.requested_inertialization
                && (self.has_been_full_weight || self.weight >= 1.0 - 1e-5))
    }

    pub fn advance(&self, previous_phase: f32) -> (f32, SourceMarkerAdvance, f32) {
        let previous_time = marker_phase_sample_time_near(
            previous_phase,
            self.phase_duration,
            &self.phase_markers,
            self.duration,
            &self.markers,
            self.sample_time_hint.unwrap_or_else(|| phase_to_time(previous_phase, self.duration, self.looped)),
            self.looped,
        );
        self.advance_from_time(previous_time)
    }

    pub fn advance_from_time(&self, previous_time: f32) -> (f32, SourceMarkerAdvance, f32) {
        let duration = self.duration.max(EPSILON);
        let next_unwrapped = previous_time + self.delta_seconds * self.play_rate;
        let markers = sorted_markers(&self.markers, duration);
        let marker_index = |time: f32, forward: bool| {
            let (time, cycle) = if self.looped { (time.rem_euclid(duration), (time / duration).floor() as i64) }
                else { (time.clamp(0.0, duration), 0) };
            cycle * markers.len() as i64 + markers.iter().rposition(|marker| if forward { marker.time < time } else { marker.time <= time })
                .map_or(-1, |index| index as i64)
        };
        let forward = next_unwrapped >= previous_time;
        let previous_index = marker_index(previous_time, forward);
        let steps = marker_index(next_unwrapped, forward) - previous_index;
        let passed = (0..steps.unsigned_abs()).map(|offset| {
            let index = if forward { previous_index + 1 + offset as i64 } else { previous_index - offset as i64 };
            markers[index.rem_euclid(markers.len() as i64) as usize].name.clone()
        }).collect();
        let next_time = phase_to_time(
            next_unwrapped / duration,
            self.duration,
            self.looped,
        );
        (marker_phase_from_sample_time(
            next_time,
            self.duration,
            &self.markers,
            self.phase_duration,
            &self.phase_markers,
            self.looped,
        ), SourceMarkerAdvance { forward, markers: passed }, next_time)
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

pub fn marker_phase_sample_time_continuous(
    phase: f32,
    advance: &SourceMarkerAdvance,
    phase_duration: f32,
    phase_markers: &[SyncMarker],
    sample_duration: f32,
    sample_markers: &[SyncMarker],
    previous_sample_time: f32,
    looped: bool,
) -> f32 {
    let fallback = || marker_phase_sample_time_near(phase, phase_duration, phase_markers,
        sample_duration, sample_markers, previous_sample_time, looped);
    let phase_markers = sorted_markers(phase_markers, phase_duration);
    let sample_markers = sorted_markers(sample_markers, sample_duration);
    if phase_markers.len() < 2 || sample_markers.len() < 2 {
        return fallback();
    }
    let phase_time = phase_to_time(phase, phase_duration, looped);
    let previous_sample_time = if looped { previous_sample_time.rem_euclid(sample_duration.max(EPSILON)) }
        else { previous_sample_time.clamp(0.0, sample_duration.max(EPSILON)) };
    let mut previous = sample_markers.iter().rposition(|marker|
        if advance.forward { marker.time < previous_sample_time } else { marker.time <= previous_sample_time })
        .map_or(-1, |index| index as i64);
    let mut next = previous + 1;
    let Some(position) = marker_position_at_time_in_direction(&phase_markers, phase_duration, phase_time, looped, advance.forward) else {
        return fallback();
    };
    let count = sample_markers.len() as i64;
    let find = |start: i64, name: &str, direction: i64| (0..count)
        .map(|offset| start + direction * offset)
        .take_while(|index| looped || (0..count).contains(index))
        .find(|index| sample_markers[index.rem_euclid(count) as usize].name == name);
    if !advance.markers.is_empty() {
        let direction = if advance.forward { 1 } else { -1 };
        let mut cursor = if advance.forward { next } else { previous };
        for marker in &advance.markers {
            let Some(index) = find(cursor, marker, direction) else { return fallback(); };
            cursor = index;
        }
        if advance.forward {
            previous = cursor;
            let Some(index) = find(cursor, &position.next, 1) else { return fallback(); };
            next = index;
        } else {
            next = cursor;
            let Some(index) = find(cursor, &position.previous, -1) else { return fallback(); };
            previous = index;
        }
    }
    if !looped && (previous < 0 || next >= count) { return fallback(); }
    if sample_markers[previous.rem_euclid(count) as usize].name != position.previous
        || sample_markers[next.rem_euclid(count) as usize].name != position.next {
        return fallback();
    }
    let mut start = sample_markers[previous.rem_euclid(count) as usize].time;
    let end = sample_markers[next.rem_euclid(count) as usize].time;
    if start >= end { start -= sample_duration; }
    let time = start + (end - start) * position.alpha.clamp(0.0, 1.0);
    (if time < 0.0 { time + sample_duration } else { time }).clamp(0.0, sample_duration)
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
            sample_time_hint: None,
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
        assert!((phase(&joining, &[old_length]) - 0.9).abs() < EPSILON);
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
    fn repeated_marker_continuity_preserves_multi_interval_self_mapping() {
        let markers = [("A", 0.1), ("B", 0.3), ("A", 0.6), ("B", 0.8)]
            .map(|(name, time)| SyncMarker { name: name.into(), time });
        for looped in [true, false] {
            for (previous, next, forward, passed) in [(0.15, 0.75, true, vec!["B", "A"]),
                (0.75, 0.15, false, vec!["A", "B"]), (0.15, 0.8, true, vec!["B", "A"]),
                (0.8, 0.1, false, vec!["A", "B"])] {
                let advance = SourceMarkerAdvance { forward, markers: passed.into_iter().map(str::to_owned).collect() };
                let actual = marker_phase_sample_time_continuous(next, &advance, 1.0, &markers,
                    1.0, &markers, previous, looped);
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
            let (actual, advance, _) = player.advance(previous);
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
                1.0, &phase_markers, 1.0, &sample_markers, previous_sample, true);
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
            let (phase, advance, _) = player.advance(previous);
            assert_eq!(advance.markers, passed);
            let actual = marker_phase_sample_time_continuous(phase, &advance, 1.0, &source, 1.0, &target, hint, true);
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
        assert!((player.advance(0.2).0 - 0.75).abs() < 1e-5);
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
    fn losing_exclusive_player_does_not_reduce_common_markers() {
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
        assert_eq!(result[0].markers, ["A", "B"]);
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
