#[cfg(test)]
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use crate::als::{
    character::{AlsCharacter, AlsMovementParamsHandle},
    dynamics::{AlsGroundedDynamics, AlsMovementBase},
    foot_ik::AlsFootLockState,
    input_request::AlsFirstPersonView,
    motion::AlsViewYaw,
    movement_direction::{AlsGroundedMovementIntent, classify_grounded_movement_intent},
    params::AlsMovementParams,
    state::{Gait, RotationMode, Stance},
};
use bevy::ecs::hierarchy::ChildOf;
use bevy::platform::collections::HashMap;
use avian3d::prelude::{ColliderOf, RigidBody};
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_graph_player::AnimationGraphPlayer,
};
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
pub(crate) struct AnimationComponentDefinition {
    root_bone: String,
    teleport_distance_threshold: f32,
    pub(crate) visibility_tick_option: crate::animation_visibility::AlsAnimationTickOption,
    pub(crate) recently_rendered_seconds: f64,
}

pub(crate) static ANIMATION_COMPONENT: LazyLock<AnimationComponentDefinition> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../../../assets/ALS/Character/animation_component.runtime.json"))
        .expect("Animation component definition must be valid")
});

fn write_animation_component_inputs(
    player: &mut AnimationGraphPlayer,
    skeleton: &bevy_animation_graph::core::skeleton::Skeleton,
) {
    if let Some(root) = skeleton.iter_paths().find(|path| path.parts.last().is_some_and(|name| name.as_str() == ANIMATION_COMPONENT.root_bone)) {
        player.set_global_input_data("component.root_bone", root.clone());
        player.set_global_input_data("component.teleport_distance_threshold", ANIMATION_COMPONENT.teleport_distance_threshold);
    }
}

fn update_attachment_epoch(cursor: &mut (Option<Entity>, u32), parent: Option<Entity>) -> u32 {
    if cursor.0 != parent {
        cursor.0 = parent;
        cursor.1 = cursor.1.wrapping_add(1);
    }
    cursor.1
}

use crate::{
    ab_als_root::AlsRootGraphSet,
    actual_motion::AlsActualMotion,
    animation_context_projection::AlsRootAnimationContext,
    character_assembly::AlsCharacterAvatar,
    direction_layer::view_space_direction_angle_from_radians,
    grounded_layer::AlsGroundedEntryMode,
    ragdolling_layer::AlsRagdollingLayerSet,
    standing_layer::{AlsStandingLayerSet, source_moving_smooth},
    standing_source::standing_rotation_yaw_offsets,
};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbAlsProviderSet;

pub struct AbAlsProviderPlugin;

impl Plugin for AbAlsProviderPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AbAlsProviderSet
                .after(AlsRootGraphSet)
                .after(AlsRagdollingLayerSet)
                .after(AlsStandingLayerSet)
                .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
        )
        .add_systems(
            PostUpdate,
            ab_als_input_writer.in_set(AbAlsProviderSet),
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

fn source_locomotion_flags(has_input: bool, speed: f32) -> [(&'static str, bool); 2] {
    [
        (
            "locomotion_state.b_moving",
            crate::als::motion::locomotion_movement_flags(has_input, speed).1,
        ),
        (
            "locomotion_state.b_moving_smooth",
            source_moving_smooth(has_input, speed),
        ),
    ]
}

fn source_pose_curve_inputs(context: Option<&AlsRootAnimationContext>) -> [(&'static str, f32); 9] {
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
        ("pose_state.gait_running_amount", (curve("PoseGait") - 1.0).clamp(0.0, 1.0)),
        ("pose_state.gait_sprinting_amount", (curve("PoseGait") - 2.0).clamp(0.0, 1.0)),
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
fn ab_als_input_writer(
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
    time: Res<Time>,
    virtual_time: Res<Time<Virtual>>,
    params_assets: Res<Assets<AlsMovementParams>>,
    skeletons: Res<Assets<bevy_animation_graph::core::skeleton::Skeleton>>,
    characters: Query<
        (
            Entity,
            &Gait,
            &Stance,
            &RotationMode,
            &AlsActualMotion,
            &AlsViewYaw,
            Option<&AlsMovementBase>,
            Option<&AlsGroundedEntryMode>,
            &Transform,
            Option<&AlsFirstPersonView>,
            Option<&crate::als_camera::AlsCameraCustomTimeDilation>,
            Option<&AlsFootLockState>,
            (&AlsMovementParamsHandle, Option<&AlsGroundedDynamics>),
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance, &GlobalTransform), With<AlsCharacterAvatar>>,
    mut players: Query<(&mut AnimationGraphPlayer, Option<&AlsRootAnimationContext>, Option<&crate::extensions::post_physics_animation::AnimationPoseUpdateGate>)>,
    owners: Query<&ColliderOf>,
    bodies: Query<&RigidBody>,
    parents: Query<&ChildOf>,
    mut attachment_state: Local<HashMap<Entity, (Option<Entity>, u32)>>,
) {
    for (
        character,
        gait,
        stance,
        rotation_mode,
        motion,
        view_yaw,
        movement_base,
        grounded_entry_mode,
        transform,
        first_person,
        custom_dilation,
        feet,
        (params_handle, dynamics),
    ) in &characters
    {
        let Some(params) = params_assets.get(&params_handle.0) else {
            continue;
        };
        let attach_parent = parents.get(character).ok().map(ChildOf::parent);
        let cursor = attachment_state.entry(character).or_insert((attach_parent, 0));
        let attachment_epoch = update_attachment_epoch(cursor, attach_parent) as f32;
        for (parent, instance, avatar_transform) in &avatars {
            if parent.parent() != character {
                continue;
            }
            let Ok((mut player, context, tick)) = players.get_mut(instance.player_entity()) else {
                continue;
            };
            if let Some(skeleton) = skeletons.get(player.skeleton()) {
                write_animation_component_inputs(&mut player, skeleton);
            }
            player.set_global_input_data("component.attach_parent_epoch", attachment_epoch);
            player.set_global_input_data("component.attached_to_actor", attach_parent.is_some());
            let velocity_yaw_angle_view_space =
                source_velocity_yaw_angle_view_space(motion, view_yaw.0);
            let (forward, backward, left, right) =
                source_direction_inputs(*rotation_mode, *gait, velocity_yaw_angle_view_space);
            let (yaw_forward, yaw_backward, yaw_left, yaw_right) =
                standing_rotation_yaw_offsets(-velocity_yaw_angle_view_space);
            let has_input = motion.has_input;
            let delta_seconds = time.delta_secs() * custom_dilation.map_or(1.0, |value| value.0);
            let view_degrees = crate::als::turn_in_place::wrap_angle_degrees(view_yaw.0.to_degrees());
            let previous_view = player
                .get_global_input_data("view_state.rotation_yaw")
                .and_then(|value| value.as_f32().ok())
                .unwrap_or(view_degrees);
            if delta_seconds > 1e-8 {
                player.set_global_input_data(
                    "view_state.yaw_speed",
                    (view_degrees - previous_view).abs() / delta_seconds,
                );
            }
            player.set_global_input_data("view_state.rotation_yaw", view_degrees);
            player.set_global_input_data("movement_base.b_has_relative_rotation", motion.movement_base_snapshot.is_some());
            player.set_global_input_data("movement_base.delta_yaw", if !virtual_time.is_paused() {
                motion.movement_base_delta_yaw_degrees
            } else { 0.0 });
            let body_degrees =
                crate::als::orientation::world_forward_to_yaw(*transform.forward()).to_degrees();
            player.set_global_input_data("locomotion_state.input_yaw_angle_world_space", -motion.input_yaw_degrees);
            let action_active = source_inputs.get(instance.player_entity()).ok().and_then(|inputs| inputs.gameplay_tag_input("locomotion_action")).is_some_and(|tag| tag.is_valid());
            if !action_active {
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
                .is_none_or(|arena| arena.frame_index() == 0) || tick.is_some_and(|tick| tick.pending_update);
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
                if pending_update || delta_seconds <= 1e-8 {
                    Vec3::ZERO
                } else {
                    motion.world_acceleration * (time.delta_secs() / delta_seconds)
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
            let has_relative_location = movement_base.and_then(|base| base.entity).is_some_and(|entity| {
                let owner = owners.get(entity).map_or(entity, |collider| collider.body);
                bodies.get(owner).is_ok_and(|body| *body != RigidBody::Static)
            });
            let pose_gait = context.and_then(|state| state.f32("PoseGait")).unwrap_or(0.0).clamp(0.0, 3.0);
            let pose_grounded = context.and_then(|state| state.f32("PoseGrounded")).unwrap_or(0.0);
            let (running, sprinting) = crate::standing_layer::unweighted_gait_amounts(pose_gait, pose_grounded);
            let speed = motion.horizontal_speed / avatar_transform.to_scale_rotation_translation().0.y;
            let stride = crate::standing_source::standing_stride_amount(running, speed);
            for (key, value) in [
                ("standing_state.walk_run_blend_amount", if *gait == Gait::Walking { 0.0 } else { 1.0 }),
                ("standing_state.stride_blend_amount", stride),
                ("standing_state.sprint_block_amount", context.and_then(|state| state.f32("SprintBlock")).unwrap_or(0.0).clamp(0.0, 1.0)),
                ("standing_state.play_rate", crate::standing_layer::standing_play_rate(params, speed, stride, running, sprinting)),
            ] {
                crate::extensions::source_instance::set_source_frame_input(&mut player, key, value);
            }
            player.set_global_input_data("als_standing_gait_is_sprinting", (*gait == Gait::Sprinting) as u8 as f32);
            let hips_direction_lock = context
                .and_then(|state| state.f32("HipsDirectionLock"))
                .unwrap_or(0.0)
                .clamp(-1.0, 1.0);
            source_inputs.get_mut(instance.player_entity()).unwrap().set_gameplay_tag_input("grounded_entry_mode",
                grounded_entry_mode.copied().unwrap_or_default().tag());
            for (key, value) in [
                ("animation.delta_seconds", delta_seconds),
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
    attachment_state.retain(|entity, _| characters.contains(*entity));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::als::{input::AlsInputCommand, motion::AlsInputOverride};

    #[test]
    fn generated_grounded_lean_uses_the_source_animation_acceleration_clock() {
        use crate::actual_motion::AlsActualMotionSet;
        use crate::extensions::{post_physics_animation::PostPhysicsAnimationSet, source_instance::source_player_property};

        #[derive(Resource)]
        struct Fixture {
            actor: Entity,
            velocity: Vec3,
            previous_velocity: Vec3,
            expected_lean: f32,
        }
        fn set_velocity(fixture: Res<Fixture>, mut actors: Query<&mut avian3d::prelude::LinearVelocity>) {
            actors.get_mut(fixture.actor).unwrap().0 = fixture.velocity;
        }
        fn source_lean(mut fixture: ResMut<Fixture>, actors: Query<(&crate::character_assembly::AlsCharacterAssembly, &crate::als::state::LocomotionState)>, avatars: Query<&AnimatedSceneInstance>, players: Query<&AnimationGraphPlayer>) {
            let (assembly, state) = actors.get(fixture.actor).unwrap();
            assert_eq!(*state, crate::als::state::LocomotionState::Grounded);
            let player = players.get(avatars.get(assembly.avatar).unwrap().player_entity()).unwrap();
            let scalar = |key| player.get_global_input_data(key).and_then(|value| value.as_f32().ok()).unwrap();
            assert_eq!(player.get_global_input_data("animation.pending_update").and_then(|value| value.as_bool().ok()), Some(false));
            let delta = scalar("animation.delta_seconds");
            let acceleration = if delta > 1e-8 {
                (fixture.velocity - fixture.previous_velocity) / delta
            } else { Vec3::ZERO };
            let rotation = player.get_global_input_data("locomotion_state.rotation_quaternion_world_space").unwrap().as_quat().unwrap();
            let target = crate::als::animation::acceleration_amount(acceleration, fixture.velocity, rotation,
                scalar("locomotion_state.max_acceleration"), scalar("locomotion_state.max_braking_deceleration")).x;
            let previous = source_player_property(player, "lean_state.forward_amount").and_then(|value| value.as_f32().ok()).unwrap();
            let alpha = 1.0 - crate::als::animation::inv_exp_approx(std::f32::consts::LN_2 / (0.2 + 1e-8) * delta);
            fixture.expected_lean = previous + (target - previous) * alpha;
            fixture.previous_velocity = fixture.velocity;
        }
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().entity_mut(actor).insert(crate::als::transition::AlsStateRequest::default().with_rotation_mode(RotationMode::ViewDirection));
        for _ in 0..3 { app.update(); }
        let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        let previous_velocity = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap()
            .get_global_input_data("locomotion_state.velocity_world_space").unwrap().as_vec3().unwrap();
        app.insert_resource(Fixture { actor, velocity: Vec3::NEG_Z, previous_velocity, expected_lean: 0.0 });
        app.add_systems(PostUpdate, set_velocity.after(crate::als::motion::AlsActualVelocitySet).before(AlsActualMotionSet));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut stable_frames = 0;
        for _ in 0..240 {
            app.update();
            let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            let arena = player.get_context_arena().unwrap();
            let properties = arena.get_toplevel().node_states.get::<crate::extensions::source_instance::SourceInstanceProperties>(
                bevy_animation_graph::core::animation_graph::NodeId::default(), bevy_animation_graph::core::context::node_states::StateKey::Default).unwrap();
            if properties.write_frames.get("standing_source.acceleration_forward") == Some(&arena.frame_index()) {
                stable_frames += 1;
                if stable_frames == 3 { break; }
            } else { stable_frames = 0; }
            assert!(std::time::Instant::now() < deadline, "Grounded native lean fixture did not leave its landing state");
        }
        assert_eq!(stable_frames, 3, "Grounded native lean fixture did not update on three consecutive frames");
        {
            let mut fixture = app.world_mut().resource_mut::<Fixture>();
            fixture.previous_velocity = fixture.velocity;
        }
        app.add_systems(PostUpdate, source_lean.after(crate::head_layer::AlsHeadLayerSet).before(PostPhysicsAnimationSet::Evaluate));
        for (custom_dilation, speed) in [(1.0_f32, 1.01_f32), (0.5, 1.02)] {
            app.world_mut().entity_mut(actor).insert(crate::als_camera::AlsCameraCustomTimeDilation(custom_dilation));
            app.world_mut().resource_mut::<Fixture>().velocity = Vec3::NEG_Z * speed;
            app.update();
            let expected = app.world().resource::<Fixture>().expected_lean;
            let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            assert!(player.get_error().is_none());
            let actual = source_player_property(player, "lean_state.forward_amount").and_then(|value| value.as_f32().ok()).unwrap();
            assert!((actual - expected).abs() < 1e-6,
                "Native grounded lean uses the wrong acceleration clock: actual={actual}, source={expected}, custom={custom_dilation}");
        }
        app.world_mut().entity_mut(actor).insert(crate::als_camera::AlsCameraCustomTimeDilation(0.0));
        app.world_mut().resource_mut::<Fixture>().velocity = Vec3::NEG_Z * 1.03;
        app.update();
        let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
        assert_eq!(player.get_global_input_data("locomotion_state.acceleration_world_space")
            .and_then(|value| value.as_vec3().ok()), Some(Vec3::ZERO),
            "Native animation rate-of-change must be zero when its delta is zero");
    }

    #[test]
    fn animation_input_follows_consumed_motion_through_the_actual_motor_and_writer() {
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        app.world_mut().get_mut::<Transform>(character).unwrap().rotation = Quat::IDENTITY;
        for (blocked, command, override_input, control_yaw, expected_input, expected_yaw) in [
            (true, Vec2::Y, None, 0.0_f32, false, 0.0_f32),
            (false, Vec2::X, None, 0.0, true, 90.0),
            (true, Vec2::Y, None, 0.0, false, 90.0),
            (false, Vec2::Y, Some(Vec2::ZERO), 0.0, false, 90.0),
            (false, Vec2::ZERO, Some(Vec2::X * 0.0002), 0.0, true, 90.0),
            (false, Vec2::Y, None, 0.0, true, 0.0),
            (false, Vec2::ZERO, None, 0.0, false, 0.0),
            (false, Vec2::Y, None, 45.0, true, -45.0),
        ] {
            app.world_mut().entity_mut(character).insert((
                AlsInputCommand { move_dir: command, ..default() }, AlsViewYaw(control_yaw.to_radians()),
                crate::als::view::AlsPlayerViewYaw(control_yaw.to_radians()),
            ));
            if blocked {
                app.world_mut().entity_mut(character).insert(crate::als::motion::AlsMovementInputBlocked);
            } else {
                app.world_mut().entity_mut(character).remove::<crate::als::motion::AlsMovementInputBlocked>();
            }
            if let Some(move_dir) = override_input {
                app.world_mut().entity_mut(character).insert(AlsInputOverride(AlsInputCommand { move_dir, ..default() }));
            } else {
                app.world_mut().entity_mut(character).remove::<AlsInputOverride>();
            }
            app.update();
            let motion = app.world().get::<AlsActualMotion>(character).unwrap();
            assert_eq!(motion.has_input, expected_input, "Consumed motion differs: blocked={blocked}, command={command:?}, override={override_input:?}");
            let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            assert_eq!(player.get_global_input_data("locomotion_state.b_has_input").unwrap().as_bool().unwrap(),
                expected_input, "Animation input differs: blocked={blocked}, command={command:?}, override={override_input:?}");
            let yaw = player.get_global_input_data("locomotion_state.input_yaw_angle_world_space").unwrap().as_f32().unwrap();
            assert!((yaw - expected_yaw).abs() < 1e-4, "Consumed input yaw differs: {yaw} != {expected_yaw}");
        }
    }

    #[test]
    fn animation_input_projection_uses_source_state_through_the_actual_writers() {
        use crate::als::{motion::AlsRotationResponseState, state::LocomotionState};
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::{AnimationGraphPlugin, core::animated_scene::AnimatedSceneHandle};
        let mut app = App::new();
        app.add_plugins(crate::extensions::SourceAnimationInputsPlugin);
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            bevy::mesh::MeshPlugin, bevy::gizmos::GizmoPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            AnimationGraphPlugin::from_physics_schedule(PostUpdate),
            crate::ab_als_root::AlsRootGraphPlugin, AbAlsProviderPlugin,
            crate::layering_layer::LayeringLayerPlugin, crate::head_layer::HeadLayerPlugin))
            .init_asset::<WorldAsset>().init_asset::<AlsMovementParams>();
        app.finish();
        app.cleanup();
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
        let character = app.world_mut().spawn((AlsCharacter, Transform::IDENTITY,
            Gait::Sprinting, Stance::Standing, RotationMode::VelocityDirection,
            LocomotionState::Air, crate::als::state::OverlayState::Default,
            AlsActualMotion { horizontal_speed: 0.2, ..default() }, AlsViewYaw(0.0),
            AlsInputCommand { move_dir: Vec2::Y, ..default() },
            AlsRotationResponseState { initialized: true, target_yaw: 55.0_f32.to_radians(), ..default() },
            AlsMovementParamsHandle(params))).id();
        let avatar = app.world_mut().spawn((AlsCharacterAvatar { character }, ChildOf(character),
            Transform::IDENTITY, AnimatedSceneHandle::default(),
            crate::stance_layer::AlsStanceBlend::with_weight(0.9),
            crate::gait_layer::AlsGaitBlend { weights: crate::als::gait::GaitWeights::from_gait_amount(3.0) })).id();
        let player = app.world_mut().spawn((AnimationGraphPlayer::new(Handle::default()),
            Name::new("root"), Transform::IDENTITY, ChildOf(avatar))).id();
        app.world_mut().spawn((Name::new("bone"), Transform::IDENTITY, ChildOf(player)));
        app.world_mut().trigger(WorldInstanceReady { entity: avatar,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()) });
        app.world_mut().flush();
        assert_eq!(app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity(), player);
        for (stance, moving, curve_values) in [
            (Stance::Standing, false, Some([0.35, 0.2, 0.25, 1.25, 0.9998])),
            (Stance::Crouching, true, Some([0.1, 0.6, 0.3, 2.25, 0.999995])),
            (Stance::Standing, false, None),
        ] {
            app.world_mut().entity_mut(character).insert((stance, AlsInputOverride(AlsInputCommand {
                move_dir: if moving { Vec2::Y } else { Vec2::ZERO }, ..default()
            })));
            app.world_mut().get_mut::<AlsActualMotion>(character).unwrap().has_input = moving;
            let values = curve_values.unwrap_or([0.0; 5]);
            let curves = curve_values.map(|values| ["PoseStanding", "PoseCrouching", "PoseInAir", "PoseGait", "AllowTransitions"]
                .into_iter().zip(values).map(|(key, value)| (key.into(), value)).collect()).unwrap_or_default();
            app.world_mut().entity_mut(player).insert(AlsRootAnimationContext { curves });
            app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap()
                .set_global_input_data("als_standing_curve_allow_transitions", 1.0_f32);
            app.world_mut().run_schedule(PostUpdate);
            let player = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            for (key, expected) in [
                ("pose_state.standing_amount", values[0]),
                ("pose_state.crouching_amount", values[1]),
                ("pose_state.in_air_amount", values[2]),
                ("pose_state.gait_walking_amount", values[3].clamp(0.0, 1.0)),
                ("pose_state.gait_running_amount", (values[3] - 1.0).clamp(0.0, 1.0)),
                ("pose_state.gait_sprinting_amount", (values[3] - 2.0).clamp(0.0, 1.0)),
                ("locomotion_state.target_yaw_angle_world_space", -55.0),
            ] {
                let actual = player.get_global_input_data(key).unwrap().as_f32().unwrap();
                assert!((actual - expected).abs() < 1e-5, "{key}: {actual} != {expected}");
            }
            for (key, expected) in [
                ("locomotion_state.b_has_input", moving),
                ("locomotion_state.b_moving", moving),
                ("transitions_state.b_transitions_allowed", values[4] >= 1.0 - 1e-5),
            ] {
                assert_eq!(player.get_global_input_data(key).unwrap().as_bool().unwrap(), expected, "{key}");
            }
            for key in ["als_overlay_standing_amount", "als_overlay_crouching_amount",
                "als_overlay_gait_walking_amount", "als_overlay_gait_running_amount",
                "als_overlay_gait_sprinting_amount", "als_overlay_in_air_amount", "als_overlay_is_in_air",
                "als_overlay_is_moving", "als_overlay_is_sprinting", "als_overlay_transitions_allowed",
                "als_overlay_velocity_forward_amount", "als_overlay_velocity_backward_amount",
                "als_overlay_velocity_left_amount", "als_overlay_velocity_right_amount", "als_root_has_input"] {
                assert!(player.get_global_input_data(key).is_none(), "Obsolete projection remains: {key}");
            }
        }
        app.world_mut().entity_mut(character).insert(crate::als_camera::AlsCameraCustomTimeDilation(0.5));
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.1));
        app.world_mut().resource_mut::<Time<Virtual>>().advance_by(std::time::Duration::from_secs_f32(0.1));
        app.world_mut().run_schedule(PostUpdate);
        let player = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        let unscaled = player.get_global_input_data("animation.delta_seconds_without_time_dilation").unwrap().as_f32().unwrap();
        let scaled = player.get_global_input_data("animation.delta_seconds").unwrap().as_f32().unwrap();
        assert!((unscaled - 0.1).abs() < 1e-6, "unscaled={unscaled}");
        assert!((scaled - 0.05).abs() < 1e-6, "scaled={scaled}");
        app.world_mut().get_mut::<AlsViewYaw>(character).unwrap().0 = 10.0_f32.to_radians();
        app.world_mut().run_schedule(PostUpdate);
        let player = app.world().get::<AnimationGraphPlayer>(
            app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity()).unwrap();
        let yaw_speed = player.get_global_input_data("view_state.yaw_speed").unwrap().as_f32().unwrap();
        assert!((yaw_speed - 200.0).abs() < 1e-4, "Actor view yaw speed differs: {yaw_speed}");
    }

    #[test]
    fn animation_movement_base_input_reads_the_actual_physics_owner() {
        use crate::als::{dynamics::AlsMovementBase, state::LocomotionState};
        use avian3d::prelude::*;
        use bevy::{ecs::system::RunSystemOnce, world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner}};
        use bevy_animation_graph::{AnimationGraphPlugin, core::animated_scene::AnimatedSceneHandle};
        let mut app = App::new();
        app.add_plugins(crate::extensions::SourceAnimationInputsPlugin);
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin, bevy::mesh::MeshPlugin,
            bevy::gizmos::GizmoPlugin, PhysicsPlugins::new(PostUpdate), AnimationGraphPlugin::from_physics_schedule(PostUpdate)))
            .init_asset::<WorldAsset>().init_asset::<AlsMovementParams>();
        app.finish();
        app.cleanup();
        let body = app.world_mut().spawn((RigidBody::Kinematic, Transform::IDENTITY)).id();
        let child = app.world_mut().spawn((Collider::cuboid(3.0, 1.0, 3.0), ChildOf(body), Transform::IDENTITY)).id();
        app.update();
        assert_eq!(app.world().get::<ColliderOf>(child).unwrap().body, body);
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
        let character = app.world_mut().spawn((AlsCharacter, Transform::IDENTITY, Gait::Walking,
            Stance::Standing, RotationMode::ViewDirection, AlsActualMotion::default(), AlsViewYaw(0.0),
            AlsMovementParamsHandle(params))).id();
        let avatar = app.world_mut().spawn((AlsCharacterAvatar { character }, ChildOf(character),
            Transform::IDENTITY, AnimatedSceneHandle::default())).id();
        let player = app.world_mut().spawn((AnimationGraphPlayer::new(Handle::default()), ChildOf(avatar))).id();
        app.world_mut().trigger(WorldInstanceReady { entity:avatar,
            instance_id:WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()) });
        app.world_mut().flush();
        assert_eq!(app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity(), player);
        let invalid = app.world_mut().spawn(Transform::IDENTITY).id();
        for mode in [LocomotionState::Grounded, LocomotionState::Custom] {
            app.world_mut().entity_mut(character).insert(mode);
            for kind in [RigidBody::Static, RigidBody::Kinematic, RigidBody::Dynamic] {
                app.world_mut().entity_mut(body).insert(kind);
                for target in [None, Some(body), Some(child), Some(invalid)] {
                    app.world_mut().get_mut::<AlsMovementBase>(character).unwrap().entity = target;
                    app.world_mut().run_system_once(ab_als_input_writer).unwrap();
                    let actual = app.world().get::<AnimationGraphPlayer>(player).unwrap()
                        .get_global_input_data("movement_base.b_has_relative_location").unwrap().as_bool().unwrap();
                    let expected = kind != RigidBody::Static && target.is_some_and(|entity| entity == body || entity == child);
                    assert_eq!(actual, expected, "Source movement-base input differs: {mode:?}, {kind:?}, {target:?}");
                }
            }
        }
        let player = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        assert_eq!(player.get_global_input_data("component.attach_parent_epoch").unwrap().as_f32().unwrap(), 0.0);
        assert!(!player.get_global_input_data("component.attached_to_actor").unwrap().as_bool().unwrap());
    }

    #[test]
    fn component_inputs_bind_the_source_bone_below_the_scene_container() {
        use bevy_animation_graph::core::{animation_clip::EntityPath, skeleton::Skeleton};
        let container = EntityPath::default().child("SceneContainer");
        let root = container.child("root");
        let mut skeleton = Skeleton::default();
        skeleton.set_root(container.id());
        skeleton.add_bone(container, Transform::IDENTITY, Transform::IDENTITY);
        skeleton.add_bone(root.clone(), Transform::IDENTITY, Transform::IDENTITY);
        let mut player = AnimationGraphPlayer::new(Handle::default());
        write_animation_component_inputs(&mut player, &skeleton);
        assert_eq!(player.get_global_input_data("component.root_bone").unwrap().as_entity_path().unwrap(), &root);
        assert_eq!(player.get_global_input_data("component.teleport_distance_threshold").unwrap().as_f32().unwrap(), 3.0);
    }

    #[test]
    fn grounded_entry_tags_reach_the_authored_roll_entry_condition() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{animation_graph::AnimationGraph, edge_data::DataValue, context::{graph_context_arena::GraphContextArena, deferred_gizmos::DeferredGizmos, system_resources::SystemResources}};
        fn query(In((root, entity, source, moving, rotating)): In<(Handle<AnimationGraph>, Entity, crate::extensions::SourceAnimationInputs, bool, bool)>, resources: SystemResources) -> bool {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let globals = HashMap::from([
                ("locomotion_state.b_moving_smooth".to_owned(), DataValue::Bool(moving)),
                ("rotate_in_place_state.b_rotating_left".to_owned(), DataValue::Bool(rotating)),
                ("rotate_in_place_state.b_rotating_right".to_owned(), DataValue::Bool(false)),
            ]);
            source.evaluate(entity, || graph.query(bevy_animation_graph::core::animation_graph::TimeUpdate::Delta(0.0), &mut arena, &resources, entity, &HashMap::new(), &mut DeferredGizmos::default(), &globals).unwrap()["bCanEnterTransition"].as_bool().unwrap())
        }
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let condition: Handle<AnimationGraph> = app.world().resource::<AssetServer>().load("ALS/Character/AnimationInstances/AB_Als_Grounded/AnimGraph/SM11_Grounded_States/T2_Transition.condition.animgraph.ron");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while app.world().resource::<Assets<AnimationGraph>>().get(&condition).is_none() {
            app.update();
            assert!(std::time::Instant::now() < deadline, "Grounded entry condition did not load");
        }
        let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        for tag in ["Als.GroundedEntryMode.FromRoll", "", "Als.GroundedEntryMode.Custom", "Als.GroundedEntryMode.FromRoll", ""] {
            app.world_mut().entity_mut(character).insert(AlsGroundedEntryMode::new(tag).unwrap());
            app.world_mut().run_system_once(ab_als_input_writer).unwrap();
            let source = app.world().get::<crate::extensions::SourceAnimationInputs>(player).unwrap().clone();
            assert_eq!(source.gameplay_tag_input("grounded_entry_mode").unwrap().as_str(), tag);
            for (moving, rotating) in [(false, false), (true, false), (false, true), (true, true)] {
                let actual = app.world_mut().run_system_once_with(query, (condition.clone(), player, source.clone(), moving, rotating)).unwrap();
                assert_eq!(actual, tag == "Als.GroundedEntryMode.FromRoll" && !moving && !rotating, "Authored T2 differs for tag={tag}, moving={moving}, rotating={rotating}");
            }
        }
        for (action, changed) in [
            (crate::als::state::AlsNotifyLocomotionAction::None, false),
            (crate::als::state::AlsNotifyLocomotionAction::GettingUp, true),
            (crate::als::state::AlsNotifyLocomotionAction::GettingUp, false),
            (crate::als::state::AlsNotifyLocomotionAction::None, true),
        ] {
            app.world_mut().entity_mut(character).insert((AlsGroundedEntryMode::FromRoll, action));
            app.update();
            app.world_mut().run_system_once(ab_als_input_writer).unwrap();
            let source = app.world().get::<crate::extensions::SourceAnimationInputs>(player).unwrap().clone();
            assert_eq!(app.world_mut().run_system_once_with(query, (condition.clone(), player, source, false, false)).unwrap(), !changed,
                "Source NativeUpdateAnimation clears GroundedEntryMode only when the actual locomotion action changes before the authored Roll entry condition: action={action:?}");
        }
    }

    #[test]
    fn source_action_snapshot_preserves_the_later_roll_entry_point_notify() {
        #[derive(Resource, Default)]
        struct ObservedRollEntry(Vec<(String, String)>);
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        app.init_resource::<ObservedRollEntry>().add_systems(PostUpdate,
            (move |source: Query<&crate::extensions::SourceAnimationInputs>, mut observed: ResMut<ObservedRollEntry>| {
                let source = source.get(player).unwrap();
                observed.0.push((source.gameplay_tag_input("locomotion_action").unwrap().as_str().to_owned(),
                    source.gameplay_tag_input("grounded_entry_mode").unwrap().as_str().to_owned()));
            }).after(AbAlsProviderSet).before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));
        for _ in 0..30 { app.update(); }
        app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().roll_pressed = true;
        app.update();
        app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().roll_pressed = false;
        for _ in 0..100 { app.update(); }
        let rows = &app.world().resource::<ObservedRollEntry>().0;
        assert!(rows.iter().any(|(action, _)| action == "Als.LocomotionAction.Rolling"));
        assert!(rows.iter().any(|(action, entry)| action.is_empty() && entry == "Als.GroundedEntryMode.FromRoll"),
            "Source branching action end precedes NativeUpdate reset, then the queued FromRoll point reaches the actual graph consumer");
    }

    #[test]
    fn attachment_epoch_changes_only_when_actor_parent_identity_changes() {
        let first = Entity::from_raw_u32(301).unwrap();
        let second = Entity::from_raw_u32(302).unwrap();
        let mut cursor = (None, 0);
        assert_eq!(update_attachment_epoch(&mut cursor, None), 0);
        assert_eq!(update_attachment_epoch(&mut cursor, Some(first)), 1);
        assert_eq!(update_attachment_epoch(&mut cursor, Some(first)), 1);
        assert_eq!(update_attachment_epoch(&mut cursor, Some(second)), 2);
        assert_eq!(update_attachment_epoch(&mut cursor, None), 3);
        assert_eq!(update_attachment_epoch(&mut cursor, None), 3);
    }

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
    fn standing_source_velocity_yaw_survives_stop_and_tracks_view_changes() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, crate::actual_motion::AlsActualMotionPlugin));
        let entity = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::default(),
                avian3d::prelude::LinearVelocity::default(),
                crate::als::state::LocomotionState::Grounded,
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
                .insert(avian3d::prelude::LinearVelocity(velocity));
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
    fn generated_locomotion_observes_the_current_motor_velocity_before_graph_update() {
        use crate::als::motion::AlsActualVelocity;
        use avian3d::prelude::{LinearVelocity, PhysicsSystems};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, crate::actual_motion::AlsActualMotionPlugin));
        app.configure_sets(
            PostUpdate,
            (
                AbAlsProviderSet
                    .after(crate::gait_layer::AlsGaitLayerSet)
                    .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
                AnimationGraphSet::PrePhysics.before(PhysicsSystems::First),
                PhysicsSystems::First.before(PhysicsSystems::Last),
            ),
        );
        app.world_mut().spawn((
            AlsCharacter,
            Transform::default(),
            LinearVelocity(Vec3::NEG_Z * 1.315),
            crate::als::state::LocomotionState::Grounded,
            AlsActualVelocity {
                world_velocity: Vec3::NEG_Z * 1.689,
                horizontal_speed: 1.689,
            },
            AlsActualMotion {
                world_velocity: Vec3::NEG_Z * 1.689,
                horizontal_speed: 1.689,
                ..default()
            },
        ));
        app.add_systems(
            PostUpdate,
            (|motion: Query<&AlsActualMotion>| {
                let motion = motion.single().unwrap();
                assert!((motion.horizontal_speed - 1.315).abs() < 1e-5);
                assert!(!source_locomotion_flags(false, motion.horizontal_speed)[1].1);
            }).in_set(AbAlsProviderSet),
        );
        app.update();
    }

    #[test]
    fn generated_overlay_gait_inputs_follow_pose_curve_weights() {
        for gait in [-1.0_f32, 0.0, 1.4, 2.4, 4.0] {
            let context = AlsRootAnimationContext { curves: [("PoseGait".into(), gait)].into_iter().collect() };
            let inputs: std::collections::HashMap<_,_> = source_pose_curve_inputs(Some(&context)).into_iter().collect();
            assert!((inputs["pose_state.gait_running_amount"] - (gait.clamp(0.0,3.0) - 1.0).clamp(0.0,1.0)).abs() < 1e-6);
            assert!((inputs["pose_state.gait_sprinting_amount"] - (gait.clamp(0.0,3.0) - 2.0).clamp(0.0,1.0)).abs() < 1e-6);
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
            [-1.0, 0.3, 0.4, 0.2, 0.4, 0.6, 1.0, 1.0, 2.4 - 2.0]
        );
        assert_eq!(
            source_pose_curve_inputs(None).map(|(_, value)| value),
            [0.0; 9]
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
