use std::collections::HashMap;

use als_core::character::AlsCharacter;
use als_core::input::AlsInputCommand;
use als_core::mantle::AlsMantleState;
use als_core::roll::AlsRollState;
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_core::view::AlsViewSmoothed;
use als_ext::{SyncPhaseInput, SyncPhaseState, advance_sync_phase};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::pose::Pose;

use crate::actual_motion::AlsActualMotion;
use crate::animation_events::AlsNotifyLocomotionAction;
use crate::character_assembly::AlsCharacterAvatar;
use crate::gait_layer::{AlsGaitBlend, AlsGaitLayerSet};
use crate::locomotion_layer::{AlsGroundPredictionState, AlsLocomotionLayerSet};
use crate::stance_layer::AlsStanceBlend;
use crate::standing_layer::AlsStandingLayerSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsRootGraphSet;

pub struct AlsRootGraphPlugin;

const SECONDARY_MOTION_DURATION: f32 = 2.666667;
const FULL_WEIGHT_EPSILON: f32 = 1e-4;

#[derive(Resource, Default)]
struct SecondaryMotionPhases {
    states: HashMap<Entity, SyncPhaseState>,
}

impl Plugin for AlsRootGraphPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsRootGraphSet
                .after(AlsGaitLayerSet)
                .after(AlsStandingLayerSet)
                .before(AlsLocomotionLayerSet)
                .before(AnimationGraphSet::Final),
        )
        .init_resource::<SecondaryMotionPhases>()
        .add_systems(PostUpdate, root_graph_writer.in_set(AlsRootGraphSet))
        .add_systems(
            FixedPostUpdate,
            root_pose_invariant.after(AnimationGraphSet::Final),
        );
    }
}

fn root_pose_is_valid(pose: &Pose) -> bool {
    pose.bones.iter().all(|bone| {
        bone.translation.is_none_or(Vec3::is_finite)
            && bone
                .rotation
                .is_none_or(|rotation| rotation.is_finite() && rotation.length_squared() > 1e-8)
            && bone.scale.is_none_or(Vec3::is_finite)
            && bone
                .weights
                .as_ref()
                .is_none_or(|weights| weights.iter().all(|weight| weight.is_finite()))
    })
}

fn root_pose_invariant(
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    characters: Query<(), With<AlsCharacter>>,
    players: Query<&AnimationGraphPlayer>,
) {
    for (parent, instance) in &avatars {
        if !characters.contains(parent.parent()) {
            continue;
        }
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let Some(DataValue::Pose(pose)) = player.get_outputs().get("pose") else {
            continue;
        };
        if !root_pose_is_valid(pose) {
            error!(
                "[AB-ALS-POSE] invalid root pose entity={:?} bones={}",
                parent.parent(),
                pose.bones.len()
            );
        }
    }
}

pub fn overlay_layer_index(overlay: OverlayState) -> f32 {
    match overlay {
        OverlayState::Default => 0.0,
        OverlayState::Masculine => 1.0,
        OverlayState::Feminine => 2.0,
        OverlayState::Injured => 3.0,
        OverlayState::HandsTied => 4.0,
        OverlayState::Rifle => 5.0,
        OverlayState::PistolOneHanded => 6.0,
        OverlayState::PistolTwoHanded => 7.0,
        OverlayState::Bow => 8.0,
        OverlayState::Torch => 9.0,
        OverlayState::Binoculars => 10.0,
        OverlayState::Box => 11.0,
        OverlayState::Barrel => 12.0,
    }
}

fn locomotion_action_index(
    notify_action: Option<&AlsNotifyLocomotionAction>,
    mantle: Option<&AlsMantleState>,
    roll: Option<&AlsRollState>,
) -> f32 {
    match notify_action {
        Some(AlsNotifyLocomotionAction::Mantling) => 1.0,
        Some(AlsNotifyLocomotionAction::GettingUp) => 2.0,
        Some(AlsNotifyLocomotionAction::Rolling) => 3.0,
        Some(AlsNotifyLocomotionAction::None) => 0.0,
        None if mantle.is_some_and(|state| state.active) => 1.0,
        None if roll.is_some_and(|state| state.active) => 3.0,
        None => 0.0,
    }
}

fn set_missing_f32(player: &mut AnimationGraphPlayer, key: &str) {
    if player.get_global_input_data(key).is_none() {
        player.set_global_input_data(key, 0.0_f32);
    }
}

fn player_f32(player: &AnimationGraphPlayer, key: &str) -> Option<f32> {
    player
        .get_global_input_data(key)
        .and_then(|value| value.as_f32().ok())
        .filter(|value| value.is_finite())
}

fn output_f32(player: &AnimationGraphPlayer, key: &str) -> Option<f32> {
    player
        .get_outputs()
        .get(key)
        .and_then(|value| value.as_f32().ok())
        .filter(|value| value.is_finite())
}

fn overlay_runtime_inputs(player: &AnimationGraphPlayer, stance: Stance) -> (f32, f32) {
    let sprint_acceleration = if stance == Stance::Standing {
        player_f32(player, "als_standing_sprint_acceleration_amount").unwrap_or(0.0)
    } else {
        0.0
    };
    let allow_curve = if stance == Stance::Standing {
        player_f32(player, "als_standing_curve_allow_transitions")
    } else {
        output_f32(player, "AllowTransitions")
    };
    let transitions_allowed = allow_curve.map_or(1.0, |value| {
        (value >= 1.0 - FULL_WEIGHT_EPSILON) as u8 as f32
    });
    (sprint_acceleration, transitions_allowed)
}

#[allow(clippy::type_complexity)]
fn root_graph_writer(
    time: Res<Time>,
    mut secondary_motion: ResMut<SecondaryMotionPhases>,
    characters: Query<
        (
            Entity,
            &OverlayState,
            &Stance,
            &Gait,
            &LocomotionState,
            &RotationMode,
            &AlsActualMotion,
            Option<&AlsInputCommand>,
            Option<&AlsViewSmoothed>,
            Option<&AlsGroundPredictionState>,
            Option<&AlsNotifyLocomotionAction>,
            Option<&AlsMantleState>,
            Option<&AlsRollState>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<
        (
            &ChildOf,
            &AnimatedSceneInstance,
            Option<&AlsStanceBlend>,
            Option<&AlsGaitBlend>,
        ),
        With<AlsCharacterAvatar>,
    >,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (
        entity,
        overlay,
        stance,
        gait,
        locomotion,
        rotation_mode,
        actual_motion,
        input,
        view,
        ground_prediction,
        notify_action,
        mantle,
        roll,
    ) in &characters
    {
        for (parent, instance, stance_blend, gait_blend) in &avatars {
            if parent.parent() != entity {
                continue;
            }
            let Ok(mut player) = players.get_mut(instance.player_entity()) else {
                continue;
            };
            let secondary_motion_phase = advance_sync_phase(
                secondary_motion
                    .states
                    .entry(instance.player_entity())
                    .or_default(),
                SyncPhaseInput {
                    active: true,
                    reset: false,
                    rate: 1.0,
                    duration: SECONDARY_MOTION_DURATION,
                    start_phase: 0.0,
                    looped: true,
                    delta_seconds: time.delta_secs(),
                },
            );
            let crouching_amount = stance_blend.map_or(
                if *stance == Stance::Crouching {
                    1.0
                } else {
                    0.0
                },
                |value| value.weight,
            );
            let (run_amount, sprint_amount) = gait_blend.map_or_else(
                || match *gait {
                    Gait::Walking => (0.0, 0.0),
                    Gait::Running => (1.0, 0.0),
                    Gait::Sprinting => (1.0, 1.0),
                },
                |value| (value.weights.running_amount(), value.weights.sprint_w),
            );
            let forward = actual_motion.velocity_blend.forward;
            let backward = actual_motion.velocity_blend.backward;
            let left = actual_motion.velocity_blend.left;
            let right = actual_motion.velocity_blend.right;
            let in_air_amount = if *locomotion == LocomotionState::Air {
                1.0
            } else {
                0.0
            };
            let moving = actual_motion.horizontal_speed > 0.05;
            let pitch_amount = view.map_or(0.5, |value| 0.5 - value.0 / std::f32::consts::PI);
            let has_input = input.is_some_and(|value| value.move_dir.length_squared() > 1e-6);
            let (sprint_acceleration, transitions_allowed) =
                overlay_runtime_inputs(&player, *stance);
            for (key, value) in [
                ("als_root_overlay_index", overlay_layer_index(*overlay)),
                ("als_animation_delta_seconds", time.delta_secs()),
                ("als_secondary_motion_phase", secondary_motion_phase),
                ("als_overlay_standing_amount", 1.0 - crouching_amount),
                ("als_overlay_crouching_amount", crouching_amount),
                ("als_overlay_gait_walking_amount", 1.0 - run_amount),
                ("als_overlay_gait_running_amount", run_amount),
                ("als_overlay_gait_sprinting_amount", sprint_amount),
                (
                    "als_overlay_ground_prediction_amount",
                    ground_prediction.map_or(0.0, |value| value.amount),
                ),
                ("als_overlay_in_air_amount", in_air_amount),
                ("als_overlay_is_in_air", in_air_amount),
                ("als_overlay_is_moving", if moving { 1.0 } else { 0.0 }),
                ("als_overlay_is_sprinting", sprint_amount),
                (
                    "als_overlay_locomotion_action_index",
                    locomotion_action_index(notify_action, mantle, roll),
                ),
                ("als_overlay_pitch_amount", pitch_amount),
                (
                    "als_overlay_rotation_mode_index",
                    if *rotation_mode == RotationMode::Aiming {
                        1.0
                    } else {
                        0.0
                    },
                ),
                (
                    "als_overlay_sprint_acceleration_amount",
                    sprint_acceleration,
                ),
                ("als_overlay_transitions_allowed", transitions_allowed),
                ("als_overlay_velocity_forward_amount", forward),
                ("als_overlay_velocity_backward_amount", backward),
                ("als_overlay_velocity_left_amount", left),
                ("als_overlay_velocity_right_amount", right),
            ] {
                player.set_global_input_data(key, value);
            }
            if has_input {
                player.set_global_input_data("als_root_has_input", 1.0_f32);
            } else {
                player.set_global_input_data("als_root_has_input", 0.0_f32);
            }
            for key in [
                "als_root_transition_slot_weight",
                "als_root_post_locomotion_slot_weight",
                "als_root_transition_slot_layer_weight",
                "als_root_post_locomotion_slot_layer_weight",
            ] {
                set_missing_f32(&mut player, key);
            }
            player.set_global_input_data("als_root_defaults_enabled", true);
            let skeleton = player.skeleton().clone();
            let elapsed = player.elapsed();
            player.set_global_input_data(
                "als_root_reference_pose",
                DataValue::Pose(Pose {
                    skeleton,
                    timestamp: elapsed,
                    ..default()
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_indices_follow_the_root_implementation_order() {
        assert_eq!(overlay_layer_index(OverlayState::Default), 0.0);
        assert_eq!(overlay_layer_index(OverlayState::Bow), 8.0);
        assert_eq!(overlay_layer_index(OverlayState::Torch), 9.0);
        assert_eq!(overlay_layer_index(OverlayState::Barrel), 12.0);
    }

    #[test]
    fn notify_action_overrides_legacy_action_inference() {
        assert_eq!(
            locomotion_action_index(Some(&AlsNotifyLocomotionAction::Mantling), None, None),
            1.0
        );
        assert_eq!(
            locomotion_action_index(Some(&AlsNotifyLocomotionAction::GettingUp), None, None),
            2.0
        );
        assert_eq!(
            locomotion_action_index(Some(&AlsNotifyLocomotionAction::Rolling), None, None),
            3.0
        );
        assert_eq!(
            locomotion_action_index(Some(&AlsNotifyLocomotionAction::None), None, None),
            0.0
        );
    }

    #[test]
    fn secondary_motion_phase_uses_the_source_idle_loop_duration() {
        let mut state = SyncPhaseState::default();
        let input = |delta_seconds| SyncPhaseInput {
            active: true,
            reset: false,
            rate: 1.0,
            duration: SECONDARY_MOTION_DURATION,
            start_phase: 0.0,
            looped: true,
            delta_seconds,
        };
        assert!(advance_sync_phase(&mut state, input(0.0)).abs() < 1e-6);
        assert!(
            (advance_sync_phase(&mut state, input(SECONDARY_MOTION_DURATION * 0.25)) - 0.25).abs()
                < 1e-6
        );
        assert!(
            (advance_sync_phase(&mut state, input(SECONDARY_MOTION_DURATION * 0.75))).abs() < 1e-6
        );
    }

    #[test]
    fn overlay_inputs_forward_current_standing_sprint_and_transition_curve() {
        let mut player = AnimationGraphPlayer::new(default());
        player.set_global_input_data("als_standing_sprint_acceleration_amount", 0.35_f32);
        player.set_global_input_data("als_standing_curve_allow_transitions", 1.0_f32);
        assert_eq!(
            overlay_runtime_inputs(&player, Stance::Standing),
            (0.35, 1.0)
        );
    }

    #[test]
    fn overlay_transition_input_requires_a_full_weight_curve() {
        let mut player = AnimationGraphPlayer::new(default());
        player.set_global_input_data("als_standing_sprint_acceleration_amount", 0.2_f32);
        player.set_global_input_data("als_standing_curve_allow_transitions", 0.9998_f32);
        assert_eq!(
            overlay_runtime_inputs(&player, Stance::Standing),
            (0.2, 0.0)
        );
    }

    #[test]
    fn overlay_inputs_do_not_reuse_standing_sprint_while_crouching() {
        let mut player = AnimationGraphPlayer::new(default());
        player.set_global_input_data("als_standing_sprint_acceleration_amount", 0.7_f32);
        player.set_global_input_data("als_standing_curve_allow_transitions", 0.0_f32);
        assert_eq!(
            overlay_runtime_inputs(&player, Stance::Crouching),
            (0.0, 1.0)
        );
    }
}
