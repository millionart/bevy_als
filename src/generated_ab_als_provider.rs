use als_core::{
    character::AlsCharacter,
    foot_ik::AlsFootLockInput,
    input::AlsInputCommand,
    motion::AlsViewYaw,
    movement_direction::{
        AlsGroundedMovementIntent, AlsMovementDirectionState, classify_grounded_movement_intent,
    },
    state::{Gait, LocomotionState, RotationMode, Stance},
};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_graph_player::AnimationGraphPlayer,
    plugin::AnimationGraphSet,
};
use bevy_gameplay_tag::GameplayTag;

use crate::{
    ab_als_root::AlsRootGraphSet,
    actual_motion::AlsActualMotion,
    animation_context_projection::AlsRootAnimationContext,
    character_assembly::AlsCharacterAvatar,
    direction_layer::{velocity_yaw_deg, view_space_direction_angle_from_radians},
    grounded_layer::AlsGroundedEntryMode,
    locomotion_layer::AlsMovementBaseAnimationState,
    ragdolling_layer::AlsRagdollingLayerSet,
    standing_layer::{StandingCurveHandles, source_rotation_yaw_offsets},
};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GeneratedAbAlsProviderSet;

pub struct GeneratedAbAlsProviderPlugin;

impl Plugin for GeneratedAbAlsProviderPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            GeneratedAbAlsProviderSet
                .after(AlsRootGraphSet)
                .after(AlsRagdollingLayerSet)
                .before(AnimationGraphSet::PrePhysics),
        )
        .add_systems(
            PostUpdate,
            generated_ab_als_input_writer.in_set(GeneratedAbAlsProviderSet),
        );
    }
}

fn source_direction_inputs(
    rotation_mode: RotationMode,
    gait: Gait,
    velocity_yaw_angle_view_space: f32,
) -> (f32, f32, f32, f32) {
    let direction = if rotation_mode == RotationMode::VelocityDirection || gait == Gait::Sprinting {
        AlsGroundedMovementIntent::Forward
    } else {
        classify_grounded_movement_intent(velocity_yaw_angle_view_space)
    };
    match direction {
        AlsGroundedMovementIntent::Forward => (1.0, 0.0, 0.0, 0.0),
        AlsGroundedMovementIntent::Backward => (0.0, 1.0, 0.0, 0.0),
        AlsGroundedMovementIntent::Left => (0.0, 0.0, 1.0, 0.0),
        AlsGroundedMovementIntent::Right => (0.0, 0.0, 0.0, 1.0),
    }
}

fn set_source_slot_defaults(player: &mut AnimationGraphPlayer) {
    for key in [
        "transition.weight",
        "transition.layer_weight",
        "post_locomotion.weight",
        "post_locomotion.layer_weight",
        "turn_in_place_standing.weight",
        "turn_in_place_standing.layer_weight",
        "turn_in_place_crouching.weight",
        "turn_in_place_crouching.layer_weight",
        "arm_left.weight",
        "arm_left.layer_weight",
        "arm_right.weight",
        "arm_right.layer_weight",
        "curves.weight",
        "curves.layer_weight",
        "head.weight",
        "head.layer_weight",
        "legs.weight",
        "legs.layer_weight",
        "pelvis.weight",
        "pelvis.layer_weight",
        "spine.weight",
        "spine.layer_weight",
    ] {
        if player.get_global_input_data(key).is_none() {
            player.set_global_input_data(key, 0.0_f32);
        }
    }
}

#[allow(clippy::type_complexity)]
fn generated_ab_als_input_writer(
    time: Res<Time>,
    curves: Res<Assets<als_ext::CurveAsset>>,
    curve_handles: Res<StandingCurveHandles>,
    characters: Query<
        (
            Entity,
            &Gait,
            &Stance,
            &LocomotionState,
            &RotationMode,
            &AlsActualMotion,
            &AlsViewYaw,
            Option<&AlsMovementDirectionState>,
            Option<&AlsInputCommand>,
            Option<&AlsFootLockInput>,
            Option<&AlsMovementBaseAnimationState>,
            Option<&AlsGroundedEntryMode>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<(&mut AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
) {
    for (
        character,
        gait,
        stance,
        locomotion,
        rotation_mode,
        motion,
        view_yaw,
        direction_state,
        input,
        feet,
        movement_base,
        grounded_entry_mode,
    ) in &characters
    {
        for (parent, instance) in &avatars {
            if parent.parent() != character {
                continue;
            }
            let Ok((mut player, context)) = players.get_mut(instance.player_entity()) else {
                continue;
            };
            let velocity_yaw_angle_view_space = direction_state.map_or_else(
                || {
                    view_space_direction_angle_from_radians(
                        velocity_yaw_deg(motion.world_velocity),
                        view_yaw.0,
                    )
                },
                |state| state.angle,
            );
            let (forward, backward, left, right) =
                source_direction_inputs(*rotation_mode, *gait, velocity_yaw_angle_view_space);
            let (yaw_forward, yaw_backward, yaw_left, yaw_right) =
                source_rotation_yaw_offsets(&curves, &curve_handles, velocity_yaw_angle_view_space);
            let in_air = *locomotion == LocomotionState::Air;
            let foot_planted = feet.map_or(0.0, |state| state.foot_planted_amount);
            let feet_crossing = feet.map_or(0.0, |state| state.feet_crossing_amount);
            let hips_direction_lock = context
                .and_then(|state| state.f32("HipsDirectionLock"))
                .unwrap_or(0.0)
                .clamp(-1.0, 1.0);
            als_ext::set_gameplay_tag_input(
                instance.player_entity(),
                "grounded_entry_mode",
                GameplayTag::new(
                    if grounded_entry_mode == Some(&AlsGroundedEntryMode::FromRoll) {
                        "Als.GroundedEntryMode.FromRoll"
                    } else {
                        ""
                    },
                ),
            );
            for (key, value) in [
                ("animation.delta_seconds", time.delta_secs()),
                ("grounded_state.movement_direction.b_forward", forward),
                ("grounded_state.movement_direction.b_backward", backward),
                ("grounded_state.movement_direction.b_left", left),
                ("grounded_state.movement_direction.b_right", right),
                (
                    "grounded_state.velocity_blend.forward_amount",
                    motion.velocity_blend.forward,
                ),
                (
                    "grounded_state.velocity_blend.backward_amount",
                    motion.velocity_blend.backward,
                ),
                (
                    "grounded_state.velocity_blend.left_amount",
                    motion.velocity_blend.left,
                ),
                (
                    "grounded_state.velocity_blend.right_amount",
                    motion.velocity_blend.right,
                ),
                (
                    "grounded_state.rotation_yaw_offsets.forward_angle",
                    yaw_forward,
                ),
                (
                    "grounded_state.rotation_yaw_offsets.backward_angle",
                    yaw_backward,
                ),
                ("grounded_state.rotation_yaw_offsets.left_angle", yaw_left),
                ("grounded_state.rotation_yaw_offsets.right_angle", yaw_right),
                (
                    "grounded_state.hips_direction_lock_amount",
                    hips_direction_lock,
                ),
                ("feet_state.foot_planted_amount", foot_planted),
                ("feet_state.feet_crossing_amount", feet_crossing),
                ("locomotion_state.speed", motion.horizontal_speed),
                (
                    "locomotion_state.b_moving_smooth",
                    (motion.horizontal_speed > 0.05) as u8 as f32,
                ),
                (
                    "locomotion_state.b_has_input",
                    input.is_some_and(|value| value.move_dir.length_squared() > 1e-6) as u8 as f32,
                ),
                (
                    "movement_base.b_has_relative_location",
                    movement_base.is_some_and(|state| state.has_relative_location) as u8 as f32,
                ),
                (
                    "pose_state.standing_amount",
                    (*stance == Stance::Standing) as u8 as f32,
                ),
                (
                    "pose_state.crouching_amount",
                    (*stance == Stance::Crouching) as u8 as f32,
                ),
                ("pose_state.in_air_amount", in_air as u8 as f32),
                ("pose_state.grounded_amount", (!in_air) as u8 as f32),
                (
                    "pose_state.gait_walking_amount",
                    (*gait == Gait::Walking) as u8 as f32,
                ),
            ] {
                player.set_global_input_data(key, value);
            }
            set_source_slot_defaults(&mut player);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_grounded_direction_is_exclusive_and_forces_velocity_or_sprint_forward() {
        assert_eq!(
            source_direction_inputs(RotationMode::Aiming, Gait::Running, 90.0),
            (0.0, 0.0, 0.0, 1.0)
        );
        assert_eq!(
            source_direction_inputs(RotationMode::VelocityDirection, Gait::Running, -90.0),
            (1.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(
            source_direction_inputs(RotationMode::Aiming, Gait::Sprinting, 180.0),
            (1.0, 0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn source_slots_default_to_an_inactive_weight_without_a_fabricated_pose() {
        let mut player = AnimationGraphPlayer::new(default());
        set_source_slot_defaults(&mut player);
        for key in [
            "transition.weight",
            "post_locomotion.layer_weight",
            "turn_in_place_standing.weight",
            "turn_in_place_crouching.layer_weight",
            "arm_left.weight",
            "arm_right.layer_weight",
            "curves.weight",
            "head.layer_weight",
            "legs.weight",
            "pelvis.layer_weight",
            "spine.weight",
        ] {
            assert_eq!(
                player
                    .get_global_input_data(key)
                    .and_then(|value| value.as_f32().ok()),
                Some(0.0)
            );
        }
        assert!(player.get_global_input_data("arm_left.pose").is_none());
    }
}
