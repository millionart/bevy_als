use als_core::{
    character::{AlsCharacter, AlsMovementParamsHandle},
    dynamics::AlsGroundedDynamics,
    foot_ik::AlsFootLockState,
    input::AlsInputCommand,
    input_request::AlsFirstPersonView,
    motion::{AlsInputOverride, AlsViewYaw},
    movement_direction::{AlsGroundedMovementIntent, classify_grounded_movement_intent},
    params::AlsMovementParams,
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
    direction_layer::view_space_direction_angle_from_radians,
    grounded_layer::AlsGroundedEntryMode,
    locomotion_layer::AlsMovementBaseAnimationState,
    ragdolling_layer::AlsRagdollingLayerSet,
    standing_layer::{AlsStandingLayerSet, source_moving_smooth},
    standing_source::standing_rotation_yaw_offsets,
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

fn source_crouching_movement_inputs(
    speed: f32,
    mesh_scale: f32,
    animated_speed: f32,
) -> [(&'static str, f32); 2] {
    let speed = speed / mesh_scale;
    let stride = crate::standing_source::standing_stride_amount(0.0, speed);
    [
        ("crouching_state.stride_blend_amount", stride),
        (
            "crouching_state.play_rate",
            (speed / (animated_speed * stride)).clamp(1e-4, 2.0),
        ),
    ]
}

fn source_velocity_yaw_angle_view_space(motion: &AlsActualMotion, view_yaw: f32) -> f32 {
    view_space_direction_angle_from_radians(motion.velocity_yaw_degrees, view_yaw)
}

fn source_has_input(
    input: Option<&AlsInputCommand>,
    input_override: Option<&AlsInputOverride>,
) -> bool {
    input_override
        .map(|value| &value.0)
        .or(input)
        .is_some_and(|value| value.move_dir.length_squared() > f32::EPSILON)
}

fn source_locomotion_flags(has_input: bool, speed: f32) -> [(&'static str, bool); 2] {
    [
        (
            "locomotion_state.b_moving",
            als_core::motion::locomotion_movement_flags(has_input, speed).1,
        ),
        (
            "locomotion_state.b_moving_smooth",
            source_moving_smooth(has_input, speed),
        ),
    ]
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
    params_assets: Res<Assets<AlsMovementParams>>,
    characters: Query<
        (
            Entity,
            &Gait,
            &Stance,
            &LocomotionState,
            &RotationMode,
            &AlsActualMotion,
            &AlsViewYaw,
            Option<&AlsInputCommand>,
            Option<&AlsInputOverride>,
            Option<&AlsMovementBaseAnimationState>,
            Option<&AlsGroundedEntryMode>,
            &Transform,
            Option<&AlsFirstPersonView>,
            Option<&AlsFootLockState>,
            (&AlsMovementParamsHandle, Option<&AlsGroundedDynamics>),
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance, &GlobalTransform), With<AlsCharacterAvatar>>,
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
        input,
        input_override,
        movement_base,
        grounded_entry_mode,
        transform,
        first_person,
        feet,
        (params_handle, dynamics),
    ) in &characters
    {
        let Some(params) = params_assets.get(&params_handle.0) else {
            continue;
        };
        for (parent, instance, avatar_transform) in &avatars {
            if parent.parent() != character {
                continue;
            }
            let Ok((mut player, context)) = players.get_mut(instance.player_entity()) else {
                continue;
            };
            let velocity_yaw_angle_view_space =
                source_velocity_yaw_angle_view_space(motion, view_yaw.0);
            let (forward, backward, left, right) =
                source_direction_inputs(*rotation_mode, *gait, velocity_yaw_angle_view_space);
            let (yaw_forward, yaw_backward, yaw_left, yaw_right) =
                standing_rotation_yaw_offsets(-velocity_yaw_angle_view_space);
            let has_input = source_has_input(input, input_override);
            let view_degrees = als_core::turn_in_place::wrap_angle_degrees(view_yaw.0.to_degrees());
            let previous_view = player
                .get_global_input_data("view_state.rotation_yaw")
                .and_then(|value| value.as_f32().ok())
                .unwrap_or(view_degrees);
            if time.delta_secs() > 1e-8 {
                player.set_global_input_data(
                    "view_state.yaw_speed",
                    (view_degrees - previous_view).abs() / time.delta_secs(),
                );
            }
            player.set_global_input_data("view_state.rotation_yaw", view_degrees);
            let body_degrees =
                als_core::orientation::world_forward_to_yaw(*transform.forward()).to_degrees();
            let action = player
                .get_global_input_data("als_overlay_locomotion_action_index")
                .and_then(|value| value.as_f32().ok())
                .unwrap_or(0.0);
            if *locomotion != LocomotionState::Ragdolling && action == 0.0 {
                player.set_global_input_data(
                    "view_state.yaw_angle",
                    crate::turn_layer::character_turn_angle_degrees(view_degrees, body_degrees),
                );
            }
            player.set_global_input_data(
                "locomotion_state.scale_world_space",
                avatar_transform.to_scale_rotation_translation().0.y,
            );
            let pending_update = player
                .get_context_arena()
                .is_none_or(|arena| arena.frame_index() == 0);
            player.set_global_input_data("animation.pending_update", pending_update);
            player.set_global_input_data(
                "locomotion_state.velocity_world_space",
                motion.world_velocity,
            );
            player.set_global_input_data(
                "locomotion_state.rotation_quaternion_world_space",
                transform.rotation,
            );
            player.set_global_input_data(
                "locomotion_state.acceleration_world_space",
                if pending_update {
                    Vec3::ZERO
                } else {
                    motion.world_acceleration
                },
            );
            player.set_global_input_data(
                "locomotion_state.max_acceleration",
                dynamics.map_or(params.acceleration.max_acceleration, |value| {
                    value.0.acceleration
                }),
            );
            player.set_global_input_data(
                "locomotion_state.max_braking_deceleration",
                dynamics.map_or(params.acceleration.braking_deceleration, |value| {
                    value.0.braking_deceleration
                }),
            );
            for (key, value) in [
                (
                    "rotate_in_place.allowed",
                    *rotation_mode == RotationMode::Aiming || first_person.is_some(),
                ),
                (
                    "rotate_in_place.first_person_threshold",
                    first_person.is_some() && *rotation_mode != RotationMode::Aiming,
                ),
                (
                    "turn_in_place.allowed",
                    *rotation_mode == RotationMode::ViewDirection && first_person.is_none(),
                ),
                ("stance.crouching", *stance == Stance::Crouching),
                (
                    "transitions_state.b_transitions_allowed",
                    context
                        .and_then(|state| state.f32("AllowTransitions"))
                        .unwrap_or(0.0)
                        >= 1.0 - 1e-5,
                ),
            ] {
                player.set_global_input_data(key, value);
            }
            for (side, foot) in [
                ("left", feet.map(|state| &state.left)),
                ("right", feet.map(|state| &state.right)),
            ] {
                player.set_global_input_data(
                    format!("feet_state.{side}.lock_amount"),
                    foot.map_or(0.0, |foot| foot.lock_amount),
                );
                player.set_global_input_data(
                    format!("feet_state.{side}.lock_distance_squared"),
                    foot.map_or(0.0, |foot| {
                        foot.target_location.distance_squared(foot.lock_location)
                    }),
                );
            }
            let has_relative_location =
                movement_base.is_some_and(|state| state.has_relative_location);
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
            .chain(source_crouching_movement_inputs(
                motion.horizontal_speed,
                avatar_transform.to_scale_rotation_translation().0.y,
                params.crouch.animated_crouch_speed,
            )) {
                player.set_global_input_data(key, value);
            }
            for (key, value) in [
                ("grounded_state.movement_direction.b_forward", forward),
                ("grounded_state.movement_direction.b_backward", backward),
                ("grounded_state.movement_direction.b_left", left),
                ("grounded_state.movement_direction.b_right", right),
                ("locomotion_state.b_has_input", has_input),
                (
                    "movement_base.b_has_relative_location",
                    has_relative_location,
                ),
            ]
            .into_iter()
            .chain(source_locomotion_flags(has_input, motion.horizontal_speed))
            {
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
    fn source_locomotion_flags_keep_moving_and_moving_smooth_distinct() {
        assert_eq!(
            source_locomotion_flags(false, 0.75),
            [
                ("locomotion_state.b_moving", true),
                ("locomotion_state.b_moving_smooth", false),
            ]
        );
        assert_eq!(
            source_locomotion_flags(false, 0.5).map(|(_, value)| value),
            [false, false]
        );
        assert_eq!(
            source_locomotion_flags(true, 0.01).map(|(_, value)| value),
            [true, true]
        );
    }

    #[test]
    fn crouching_movement_uses_the_native_stride_curve_and_mesh_world_scale() {
        let mesh = GlobalTransform::from(Transform::from_scale(Vec3::new(2.0, 3.0, 4.0)));
        let inputs = source_crouching_movement_inputs(
            0.583 * 3.0,
            mesh.to_scale_rotation_translation().0.y,
            1.5,
        );
        assert!((inputs[0].1 - 0.55).abs() < 1e-6, "{:?}", inputs);
        assert!((inputs[1].1 - 0.583 / (1.5 * 0.55)).abs() < 1e-6);
        assert_eq!(source_crouching_movement_inputs(0.0, 1.0, 1.5)[1].1, 1e-4);
        assert_eq!(source_crouching_movement_inputs(10.0, 1.0, 1.5)[1].1, 2.0);
    }

    #[test]
    fn source_has_input_uses_the_same_override_as_character_movement() {
        let idle = AlsInputCommand::default();
        let moving = AlsInputCommand {
            move_dir: Vec2::Y,
            ..default()
        };
        assert!(source_has_input(
            Some(&idle),
            Some(&AlsInputOverride(moving))
        ));
        assert!(!source_has_input(
            Some(&moving),
            Some(&AlsInputOverride(idle))
        ));
        assert!(source_has_input(Some(&moving), None));
        assert!(!source_has_input(None, None));
    }

    #[test]
    fn standing_source_velocity_yaw_survives_stop_and_tracks_view_changes() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, crate::actual_motion::AlsActualMotionPlugin));
        let entity = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::default(),
                als_core::motion::AlsActualVelocity::default(),
            ))
            .id();
        for (velocity, view_yaw, expected) in [
            (Vec3::new(1.0, 0.0, -1.0), 0.0, 45.0),
            (Vec3::ZERO, 0.0, 45.0),
            (Vec3::NEG_Z * 0.009, std::f32::consts::FRAC_PI_2, 135.0),
            (Vec3::NEG_Z * 0.01, 0.0, 0.0),
            (Vec3::ZERO, 0.0, 0.0),
        ] {
            app.world_mut()
                .entity_mut(entity)
                .insert(als_core::motion::AlsActualVelocity {
                    world_velocity: velocity,
                    horizontal_speed: Vec2::new(velocity.x, velocity.z).length(),
                });
            app.update();
            let motion = app.world().get::<AlsActualMotion>(entity).unwrap();
            assert_eq!(motion.world_velocity, velocity);
            let actual = source_velocity_yaw_angle_view_space(motion, view_yaw);
            assert!(
                (actual - expected).abs() < 1e-5,
                "velocity={velocity:?} view={view_yaw} angle={actual} expected={expected}"
            );
        }
    }

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
