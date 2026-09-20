#[cfg(test)]
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use als_core::{
    character::{AlsCharacter, AlsMovementParamsHandle},
    dynamics::{AlsGroundedDynamics, AlsMovementBase},
    foot_ik::AlsFootLockState,
    input::AlsInputCommand,
    input_request::AlsFirstPersonView,
    motion::{AlsInputOverride, AlsViewYaw},
    movement_direction::{AlsGroundedMovementIntent, classify_grounded_movement_intent},
    params::AlsMovementParams,
    state::{Gait, RotationMode, Stance},
};
use bevy::ecs::hierarchy::ChildOf;
use avian3d::prelude::{ColliderOf, RigidBody};
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_graph_player::AnimationGraphPlayer,
};
use bevy_gameplay_tag::GameplayTag;
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
struct AnimationComponentDefinition {
    root_bone: String,
    teleport_distance_threshold: f32,
}

static ANIMATION_COMPONENT: LazyLock<AnimationComponentDefinition> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../assets/character/animation_component.runtime.json"))
        .expect("Generated animation component definition must be valid")
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
                .before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
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

fn source_input_yaw(
    input: Option<&AlsInputCommand>,
    input_override: Option<&AlsInputOverride>,
    view_yaw: f32,
) -> Option<f32> {
    if !source_has_input(input, input_override) { return None; }
    let input = input_override.map(|value| &value.0).or(input)?;
    Some(-als_core::orientation::world_forward_to_yaw(
        als_core::motion::view_relative_move_direction(input.move_dir, view_yaw),
    ).to_degrees())
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
fn generated_ab_als_input_writer(
    mut source_inputs: Query<&mut als_ext::SourceAnimationInputs>,
    time: Res<Time>,
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
            Option<&AlsInputCommand>,
            Option<&AlsInputOverride>,
            Option<&AlsMovementBase>,
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
    owners: Query<&ColliderOf>,
    bodies: Query<&RigidBody>,
) {
    for (
        character,
        gait,
        stance,
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
            if let Some(skeleton) = skeletons.get(player.skeleton()) {
                write_animation_component_inputs(&mut player, skeleton);
            }
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
            if let Some(yaw) = source_input_yaw(input, input_override, view_yaw.0) {
                player.set_global_input_data("locomotion_state.input_yaw_angle_world_space", yaw);
            } else if player.get_global_input_data("locomotion_state.input_yaw_angle_world_space").is_none() {
                player.set_global_input_data("locomotion_state.input_yaw_angle_world_space", -body_degrees);
            }
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
            let has_relative_location = movement_base.and_then(|base| base.entity).is_some_and(|entity| {
                let owner = owners.get(entity).map_or(entity, |collider| collider.body);
                bodies.get(owner).is_ok_and(|body| *body != RigidBody::Static)
            });
            let hips_direction_lock = context
                .and_then(|state| state.f32("HipsDirectionLock"))
                .unwrap_or(0.0)
                .clamp(-1.0, 1.0);
            source_inputs.get_mut(instance.player_entity()).unwrap().set_gameplay_tag_input("grounded_entry_mode",
                GameplayTag::new(
                    if grounded_entry_mode == Some(&AlsGroundedEntryMode::FromRoll) {
                        "Als.GroundedEntryMode.FromRoll"
                    } else {
                        ""
                    },
                ),);
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
    fn animation_input_projection_uses_source_state_through_the_actual_writers() {
        use als_core::{motion::AlsRotationResponseState, state::LocomotionState};
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::{AnimationGraphPlugin, core::animated_scene::AnimatedSceneHandle};
        let mut app = App::new();
        app.add_plugins(als_ext::SourceAnimationInputsPlugin);
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            bevy::mesh::MeshPlugin, bevy::gizmos::GizmoPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            AnimationGraphPlugin::from_physics_schedule(PostUpdate),
            crate::ab_als_root::AlsRootGraphPlugin, GeneratedAbAlsProviderPlugin,
            crate::layering_layer::LayeringLayerPlugin, crate::head_layer::HeadLayerPlugin))
            .init_asset::<WorldAsset>().init_asset::<AlsMovementParams>();
        app.finish();
        app.cleanup();
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
        let character = app.world_mut().spawn((AlsCharacter, Transform::IDENTITY,
            Gait::Sprinting, Stance::Standing, RotationMode::VelocityDirection,
            LocomotionState::Air, als_core::state::OverlayState::Default,
            AlsActualMotion { horizontal_speed: 0.2, ..default() }, AlsViewYaw(0.0),
            AlsInputCommand { move_dir: Vec2::Y, ..default() },
            AlsRotationResponseState { initialized: true, target_yaw: 55.0_f32.to_radians(), ..default() },
            AlsMovementParamsHandle(params))).id();
        let avatar = app.world_mut().spawn((AlsCharacterAvatar { character }, ChildOf(character),
            Transform::IDENTITY, AnimatedSceneHandle::default(),
            crate::stance_layer::AlsStanceBlend::with_weight(0.9),
            crate::gait_layer::AlsGaitBlend { weights: als_core::gait::GaitWeights::from_gait_amount(3.0) })).id();
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
            let values = curve_values.unwrap_or([0.0; 5]);
            let curves = curve_values.map(|values| ["PoseStanding", "PoseCrouching", "PoseInAir", "PoseGait", "AllowTransitions"]
                .into_iter().zip(values).map(|(key, value)| (key.to_owned(), value)).collect()).unwrap_or_default();
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
    }

    #[test]
    fn animation_movement_base_input_reads_the_actual_physics_owner() {
        use als_core::{dynamics::AlsMovementBase, state::LocomotionState};
        use avian3d::prelude::*;
        use bevy::{ecs::system::RunSystemOnce, world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner}};
        use bevy_animation_graph::{AnimationGraphPlugin, core::animated_scene::AnimatedSceneHandle};
        let mut app = App::new();
        app.add_plugins(als_ext::SourceAnimationInputsPlugin);
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
                    app.world_mut().run_system_once(generated_ab_als_input_writer).unwrap();
                    let actual = app.world().get::<AnimationGraphPlayer>(player).unwrap()
                        .get_global_input_data("movement_base.b_has_relative_location").unwrap().as_bool().unwrap();
                    let expected = kind != RigidBody::Static && target.is_some_and(|entity| entity == body || entity == child);
                    assert_eq!(actual, expected, "Source movement-base input differs: {mode:?}, {kind:?}, {target:?}");
                }
            }
        }
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
    fn source_input_yaw_uses_current_control_input_and_the_native_yaw_basis() {
        let input = AlsInputCommand { move_dir: Vec2::Y, ..default() };
        assert!((source_input_yaw(Some(&input), None, 45.0_f32.to_radians()).unwrap() + 45.0).abs() < 1e-5);
        let override_input = AlsInputOverride(AlsInputCommand { move_dir: Vec2::X, ..default() });
        assert!((source_input_yaw(Some(&input), Some(&override_input), 0.0).unwrap() - 90.0).abs() < 1e-5);
        assert!(source_input_yaw(Some(&AlsInputCommand::default()), None, 1.0).is_none());
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
                avian3d::prelude::LinearVelocity::default(),
                als_core::state::LocomotionState::Grounded,
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
        use als_core::motion::AlsActualVelocity;
        use avian3d::prelude::{LinearVelocity, PhysicsSystems};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, crate::actual_motion::AlsActualMotionPlugin));
        app.configure_sets(
            PostUpdate,
            (
                GeneratedAbAlsProviderSet
                    .after(crate::gait_layer::AlsGaitLayerSet)
                    .before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
                AnimationGraphSet::PrePhysics.before(PhysicsSystems::First),
                PhysicsSystems::First.before(PhysicsSystems::Last),
            ),
        );
        app.world_mut().spawn((
            AlsCharacter,
            Transform::default(),
            LinearVelocity(Vec3::NEG_Z * 1.315),
            als_core::state::LocomotionState::Grounded,
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
            }).in_set(GeneratedAbAlsProviderSet),
        );
        app.update();
    }

    #[test]
    fn generated_overlay_gait_inputs_follow_pose_curve_weights() {
        for gait in [-1.0_f32, 0.0, 1.4, 2.4, 4.0] {
            let context = AlsRootAnimationContext { curves: [("PoseGait".to_owned(), gait)].into_iter().collect() };
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
