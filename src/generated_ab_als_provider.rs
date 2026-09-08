use als_core::{
    character::AlsCharacter,
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
    standing_layer::{
        AlsStandingLayerSet, StandingCurveHandles, source_moving_smooth,
        source_rotation_yaw_offsets,
    },
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
                .after(AlsStandingLayerSet)
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
) -> (bool, bool, bool, bool) {
    let direction = if rotation_mode == RotationMode::VelocityDirection || gait == Gait::Sprinting {
        AlsGroundedMovementIntent::Forward
    } else {
        classify_grounded_movement_intent(velocity_yaw_angle_view_space)
    };
    match direction {
        AlsGroundedMovementIntent::Forward => (true, false, false, false),
        AlsGroundedMovementIntent::Backward => (false, true, false, false),
        AlsGroundedMovementIntent::Left => (false, false, true, false),
        AlsGroundedMovementIntent::Right => (false, false, false, true),
    }
}

fn source_velocity_blend(
    player: &AnimationGraphPlayer,
    use_standing_runtime: bool,
    fallback: als_ext::HorizontalVelocityBlend,
) -> als_ext::HorizontalVelocityBlend {
    if !use_standing_runtime {
        return fallback;
    }
    let value = |key, fallback| {
        player
            .get_global_input_data(key)
            .and_then(|value| value.as_f32().ok())
            .filter(|value| value.is_finite())
            .unwrap_or(fallback)
    };
    als_ext::HorizontalVelocityBlend {
        forward: value("als_standing_velocity_forward_amount", fallback.forward),
        backward: value("als_standing_velocity_backward_amount", fallback.backward),
        left: value("als_standing_velocity_left_amount", fallback.left),
        right: value("als_standing_velocity_right_amount", fallback.right),
    }
}

fn source_pose_curve_inputs(context: Option<&AlsRootAnimationContext>) -> [(&'static str, f32); 7] {
    let curve = |name| context.and_then(|state| state.f32(name)).unwrap_or(0.0);
    [
        (
            "feet_state.foot_planted_amount",
            curve("FootPlanted").clamp(-1.0, 1.0),
        ),
        (
            "feet_state.feet_crossing_amount",
            curve("FeetCrossing").clamp(0.0, 1.0),
        ),
        ("pose_state.standing_amount", curve("PoseStanding")),
        ("pose_state.crouching_amount", curve("PoseCrouching")),
        ("pose_state.in_air_amount", curve("PoseInAir")),
        ("pose_state.grounded_amount", curve("PoseGrounded")),
        (
            "pose_state.gait_walking_amount",
            curve("PoseGait").clamp(0.0, 1.0),
        ),
    ]
}

fn set_source_slot_defaults(player: &mut AnimationGraphPlayer) {
    for key in [
        "transition.weight",
        "transition.layer_weight",
        "transition.mesh_space_additive",
        "post_locomotion.weight",
        "post_locomotion.layer_weight",
        "post_locomotion.mesh_space_additive",
        "turn_in_place_standing.weight",
        "turn_in_place_standing.layer_weight",
        "turn_in_place_standing.mesh_space_additive",
        "turn_in_place_crouching.weight",
        "turn_in_place_crouching.layer_weight",
        "turn_in_place_crouching.mesh_space_additive",
        "arm_left.weight",
        "arm_left.layer_weight",
        "arm_left.mesh_space_additive",
        "arm_right.weight",
        "arm_right.layer_weight",
        "arm_right.mesh_space_additive",
        "curves.weight",
        "curves.layer_weight",
        "curves.mesh_space_additive",
        "head.weight",
        "head.layer_weight",
        "head.mesh_space_additive",
        "legs.weight",
        "legs.layer_weight",
        "legs.mesh_space_additive",
        "pelvis.weight",
        "pelvis.layer_weight",
        "pelvis.mesh_space_additive",
        "spine.weight",
        "spine.layer_weight",
        "spine.mesh_space_additive",
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
            let has_input =
                input.is_some_and(|value| value.move_dir.length_squared() > f32::EPSILON);
            let moving_smooth = source_moving_smooth(has_input, motion.horizontal_speed);
            let has_relative_location =
                movement_base.is_some_and(|state| state.has_relative_location);
            let velocity_blend = source_velocity_blend(
                &player,
                *stance == Stance::Standing && *locomotion == LocomotionState::Grounded,
                motion.velocity_blend,
            );
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
                (
                    "grounded_state.velocity_blend.forward_amount",
                    velocity_blend.forward,
                ),
                (
                    "grounded_state.velocity_blend.backward_amount",
                    velocity_blend.backward,
                ),
                (
                    "grounded_state.velocity_blend.left_amount",
                    velocity_blend.left,
                ),
                (
                    "grounded_state.velocity_blend.right_amount",
                    velocity_blend.right,
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
                ("locomotion_state.speed", motion.horizontal_speed),
            ]
            .into_iter()
            .chain(source_pose_curve_inputs(context))
            {
                player.set_global_input_data(key, value);
            }
            for (key, value) in [
                ("grounded_state.movement_direction.b_forward", forward),
                ("grounded_state.movement_direction.b_backward", backward),
                ("grounded_state.movement_direction.b_left", left),
                ("grounded_state.movement_direction.b_right", right),
                ("locomotion_state.b_moving_smooth", moving_smooth),
                ("locomotion_state.b_has_input", has_input),
                (
                    "movement_base.b_has_relative_location",
                    has_relative_location,
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
    fn generated_pose_inputs_preserve_actual_curve_weights_and_source_defaults() {
        let context = AlsRootAnimationContext {
            curves: [
                ("FootPlanted", -1.2),
                ("FeetCrossing", 0.3),
                ("PoseStanding", 0.4),
                ("PoseCrouching", 0.2),
                ("PoseInAir", 0.4),
                ("PoseGrounded", 0.6),
                ("PoseGait", 2.4),
            ]
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect(),
        };
        assert_eq!(
            source_pose_curve_inputs(Some(&context)).map(|(_, value)| value),
            [-1.0, 0.3, 0.4, 0.2, 0.4, 0.6, 1.0]
        );
        assert_eq!(
            source_pose_curve_inputs(None).map(|(_, value)| value),
            [0.0; 7]
        );
    }

    #[test]
    fn source_grounded_direction_is_exclusive_and_forces_velocity_or_sprint_forward() {
        assert_eq!(
            source_direction_inputs(RotationMode::Aiming, Gait::Running, 90.0),
            (false, false, false, true)
        );
        assert_eq!(
            source_direction_inputs(RotationMode::VelocityDirection, Gait::Running, -90.0),
            (true, false, false, false)
        );
        assert_eq!(
            source_direction_inputs(RotationMode::Aiming, Gait::Sprinting, 180.0),
            (true, false, false, false)
        );
    }

    #[test]
    fn source_velocity_blend_uses_standing_runtime_only_for_standing_grounded() {
        let mut player = AnimationGraphPlayer::new(default());
        player.set_global_input_data("als_standing_velocity_forward_amount", 0.75_f32);
        player.set_global_input_data("als_standing_velocity_right_amount", 0.25_f32);
        let fallback = als_ext::HorizontalVelocityBlend {
            backward: 1.0,
            ..default()
        };
        let standing = source_velocity_blend(&player, true, fallback);
        assert_eq!(standing.forward, 0.75);
        assert_eq!(standing.right, 0.25);
        assert_eq!(standing.backward, 1.0);
        assert_eq!(source_velocity_blend(&player, false, fallback), fallback);
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
