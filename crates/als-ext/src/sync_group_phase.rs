use std::collections::HashMap;

use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};
use serde::{Deserialize, Serialize};

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
    let duration = duration.max(EPSILON);
    let markers = sorted_markers(markers, duration);
    if markers.len() < 2 {
        return None;
    }
    let time = if looped {
        time.rem_euclid(duration)
    } else {
        time.clamp(0.0, duration)
    };
    let previous_index = markers.iter().rposition(|marker| marker.time <= time);
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
    looped: bool,
) -> Option<f32> {
    let duration = duration.max(EPSILON);
    let markers = sorted_markers(markers, duration);
    if markers.len() < 2 {
        return None;
    }
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
        return Some(if looped {
            time.rem_euclid(duration)
        } else {
            time
        });
    }
    None
}

pub fn follower_sample_time(
    phase: f32,
    leader_duration: f32,
    leader_markers: &[SyncMarker],
    follower_duration: f32,
    follower_markers: &[SyncMarker],
    looped: bool,
) -> f32 {
    let leader_time = phase_to_time(phase, leader_duration, looped);
    marker_position_at_time(leader_markers, leader_duration, leader_time, looped)
        .and_then(|position| {
            time_for_marker_position(follower_markers, follower_duration, &position, looped)
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
    let fallback = phase_to_time(phase, sample_duration, looped);
    let phase_time = phase_to_time(phase, phase_duration, looped);
    marker_position_at_time(phase_markers, phase_duration, phase_time, looped)
        .and_then(|position| {
            time_for_marker_position(sample_markers, sample_duration, &position, looped)
        })
        .unwrap_or(fallback)
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
            time_for_marker_position(phase_markers, phase_duration, &position, looped)
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
    fn nodes_register_node_like_dispatch_keys() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<SyncGroupPhaseNode>();
        registry.register::<SyncGroupSampleNode>();
        for type_id in [
            std::any::TypeId::of::<SyncGroupPhaseNode>(),
            std::any::TypeId::of::<SyncGroupSampleNode>(),
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
