#[cfg(test)]
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use std::sync::LazyLock;

use crate::als::{
    character::AlsCharacter,
    control_scheme::{
        AlsControlScheme, AlsControlSchemeActionDiscriminant, AlsControlSchemeConfig,
    },
};
use crate::extensions::source_instance::{set_source_frame_input, source_player_property};
use avian3d::prelude::{Collider, ShapeCastConfig, SpatialQueryFilter};
use crate::extensions::RagdollSpatialQuery as SpatialQuery;
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_graph_player::AnimationGraphPlayer,
};
#[cfg(test)]
use bevy_animation_graph::core::ragdoll::definition::ColliderLabel;
use bevy_tnua::{TnuaConfig, TnuaController, controller::TnuaActionFlowStatus};

use crate::{
    actual_motion::AlsActualMotion,
    animation_context_projection::{AlsRootAnimationContext, AlsRootAnimationContextSet},
    character_assembly::AlsCharacterAvatar,
    ab_als_provider::AbAlsProviderSet,
};

#[derive(serde::Deserialize)]
struct InAirSettings {
    lean_interpolation_half_life: f32,
    jump_reference_speed: f32,
    jump_min_play_rate: f32,
    jump_max_play_rate: f32,
    lean_reference_speed: f32,
    prediction_velocity_threshold: f32,
    prediction_min_velocity: f32,
    prediction_max_velocity: f32,
    prediction_min_distance: f32,
    prediction_max_distance: f32,
    prediction_trace_layer: crate::als::character::AlsCollisionLayer,
    lean: crate::extensions::als_curves::AlsFloatCurve,
    ground_prediction: crate::extensions::als_curves::AlsFloatCurve,
}

static IN_AIR_SETTINGS: LazyLock<InAirSettings> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../../../assets/ALS/Data/AnimationInstance/Air/als_in_air_settings.json"))
        .expect("generated in-air settings are valid")
});

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AlsGroundPredictionState {
    pub amount: f32,
}

#[derive(Component, Clone, Debug)]
pub struct AlsGroundPredictionTrace {
    pub start: Vec3,
    pub end: Vec3,
    pub shape: Collider,
    pub hit: Option<(Vec3, Vec3)>,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsLocomotionLayerSet;

pub struct LocomotionLayerPlugin;

impl Plugin for LocomotionLayerPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsLocomotionLayerSet
                .after(AbAlsProviderSet)
                .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
        )
        .add_systems(
            PostUpdate,
            locomotion_input_writer.in_set(AlsLocomotionLayerSet),
        )
        .add_systems(
            PostUpdate,
            project_in_air_state
                .after(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate)
                .before(AlsRootAnimationContextSet),
        );
    }
}

fn write_jump_request(
    player: &mut AnimationGraphPlayer,
    flow: &TnuaActionFlowStatus<AlsControlSchemeActionDiscriminant>,
    pending_update: bool,
) {
    let jumped = !pending_update
        && (source_player_property(player, "in_air_state.b_jumped")
            .and_then(|value| value.as_bool().ok())
            .unwrap_or(false)
            || flow.just_starting() == Some(AlsControlSchemeActionDiscriminant::Jump));
    set_source_frame_input(player, "in_air_state.b_jumped", jumped);
}

fn in_air_lean_target(velocity: Vec3, rotation: Quat) -> Vec2 {
    let local = rotation.inverse() * velocity;
    Vec2::new(-local.z, local.x) / IN_AIR_SETTINGS.lean_reference_speed
        * IN_AIR_SETTINGS.lean.sample(velocity.y)
}

fn ground_prediction_amount(
    spatial: &SpatialQuery,
    origin: Vec3,
    velocity: Vec3,
    capsule: &Collider,
    mesh_scale: f32,
    walkable_floor_cos: f32,
    filter: &SpatialQueryFilter,
    allowance: f32,
) -> (f32, Option<AlsGroundPredictionTrace>) {
    let settings = &*IN_AIR_SETTINGS;
    if velocity.y > settings.prediction_velocity_threshold || allowance <= 1e-4 {
        return (0.0, None);
    }
    let vertical = velocity.y.clamp(
        settings.prediction_min_velocity,
        settings.prediction_max_velocity,
    );
    let direction = Dir3::new(Vec3::new(velocity.x, vertical, velocity.z)).unwrap();
    let amount = (vertical - settings.prediction_max_velocity)
        / (settings.prediction_min_velocity - settings.prediction_max_velocity);
    let distance = settings
        .prediction_min_distance
        .lerp(settings.prediction_max_distance, amount)
        * mesh_scale;
    let hit = spatial.cast_shape(
        capsule,
        origin,
        Quat::IDENTITY,
        direction,
        &ShapeCastConfig::from_max_distance(distance),
        filter,
    );
    let hit = hit.as_ref().filter(|hit| hit.normal1.y >= walkable_floor_cos);
    (hit.map_or(0.0, |hit| settings.ground_prediction.sample(hit.distance / distance) * allowance),
        Some(AlsGroundPredictionTrace { start: origin, end: origin + direction * distance, shape: capsule.clone(),
            hit: hit.map(|hit| (origin + direction * hit.distance, hit.point1)) }))
}

#[allow(clippy::type_complexity)]
fn locomotion_input_writer(
    mut commands: Commands,
    spatial: SpatialQuery,
    configs: Res<Assets<AlsControlSchemeConfig>>,
    transforms: bevy::transform::helper::TransformHelper,
    characters: Query<
        (
            &TnuaController<AlsControlScheme>,
            &TnuaConfig<AlsControlScheme>,
            &avian3d::prelude::Position,
            &Collider,
            &AlsActualMotion,
        ),
        With<AlsCharacter>,
    >,
    non_world_colliders: Query<Entity, With<AlsCharacter>>,
    avatars: Query<(Entity, &ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<(&mut AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
) {
    let filter = SpatialQueryFilter::from_excluded_entities(non_world_colliders.iter()).with_mask(IN_AIR_SETTINGS.prediction_trace_layer);
    for (avatar, parent, instance) in &avatars {
        let Ok(mesh) = transforms.compute_global_transform(avatar) else { continue; };
        let Ok((controller, config, position, capsule, motion)) =
            characters.get(parent.parent())
        else {
            commands.entity(avatar).remove::<AlsGroundPredictionTrace>();
            continue;
        };
        let Ok(transform) = transforms.compute_global_transform(parent.parent()) else { continue; };
        let Some(config) = configs.get(&config.0) else {
            commands.entity(avatar).remove::<AlsGroundPredictionTrace>();
            continue;
        };
        let Ok((mut player, context)) = players.get_mut(instance.player_entity()) else {
            commands.entity(avatar).remove::<AlsGroundPredictionTrace>();
            continue;
        };
        let pending_update = player
            .get_global_input_data("animation.pending_update")
            .and_then(|value| value.as_bool().ok())
            .unwrap_or(true);
        write_jump_request(&mut player, controller.action_flow_status(), pending_update);
        let allowance = 1.0
            - context
                .and_then(|context| context.f32("GroundPredictionBlock"))
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
        let (prediction, trace) = ground_prediction_amount(
            &spatial,
            position.0,
            motion.world_velocity,
            capsule,
            mesh.to_scale_rotation_translation().0.y,
            config.basis.max_slope.cos(),
            &filter,
            allowance,
        );
        if let Some(trace) = trace { commands.entity(avatar).insert(trace); }
        else { commands.entity(avatar).remove::<AlsGroundPredictionTrace>(); }
        player.set_global_input_data("in_air_source.ground_prediction_amount", prediction);
        player.set_global_input_data(
            "in_air_source.lean_target",
            in_air_lean_target(motion.world_velocity, transform.rotation()),
        );
        player.set_global_input_data(
            "in_air_source.lean_interpolation_half_life",
            IN_AIR_SETTINGS.lean_interpolation_half_life,
        );
        player.set_global_input_data(
            "in_air_source.jump_play_rate",
            IN_AIR_SETTINGS.jump_min_play_rate.lerp(
                IN_AIR_SETTINGS.jump_max_play_rate,
                (motion.horizontal_speed / IN_AIR_SETTINGS.jump_reference_speed).clamp(0.0, 1.0),
            ),
        );
    }
}

fn project_in_air_state(
    mut commands: Commands,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
) {
    for (parent, instance) in &avatars {
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let amount = source_player_property(player, "in_air_state.ground_prediction_amount")
            .and_then(|value| value.as_f32().ok())
            .unwrap_or(0.0);
        commands
            .entity(parent.parent())
            .insert(AlsGroundPredictionState { amount });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn air_lean_reads_current_actor_world_rotation() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let parent = app.world_mut().spawn(Transform::IDENTITY).id();
        let velocity = Vec3::new(1.0, IN_AIR_SETTINGS.prediction_min_velocity, -3.0);
        app.world_mut().entity_mut(actor).insert((ChildOf(parent), Transform::IDENTITY,
            AlsActualMotion { world_velocity: velocity, ..default() }));
        let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        app.world_mut().entity_mut(player_entity).remove::<AlsRootAnimationContext>();
        for yaw in [90.0_f32, -45.0] {
            let rotation = Quat::from_rotation_y(yaw.to_radians());
            *app.world_mut().get_mut::<Transform>(parent).unwrap() = Transform::from_xyz(4.0, 2.0, -1.0)
                .with_rotation(rotation).with_scale(Vec3::new(2.0, 3.0, 0.5));
            app.world_mut().run_system_once(locomotion_input_writer).unwrap();
            let actual = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap()
                .get_global_input_data("in_air_source.lean_target").unwrap().as_vec2().unwrap();
            assert!(actual.abs_diff_eq(in_air_lean_target(velocity, rotation), 1e-5),
                "Source Air lean world basis differs at {yaw}: {actual:?}");
            let trace = app.world().get::<AlsGroundPredictionTrace>(avatar).expect("Source falling prediction must publish its trace");
            let expected_distance = IN_AIR_SETTINGS.prediction_max_distance * 3.0
                * app.world().get::<Transform>(avatar).unwrap().scale.y;
            assert!((trace.end.distance(trace.start) - expected_distance).abs() < 1e-4,
                "Source Air prediction must use current mesh world scale: {} != {expected_distance}", trace.end.distance(trace.start));
        }
    }

    #[test]
    fn falling_ground_prediction_starts_at_the_current_actor_position() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(actor).unwrap().jump_pressed = true;
        app.update();
        app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(actor).unwrap().jump_pressed = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut falling_samples = 0;
        let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        for _ in 0..180 {
            let allowance = 1.0 - app.world().get::<AlsRootAnimationContext>(player_entity)
                .and_then(|context| context.f32("GroundPredictionBlock")).unwrap_or(0.0).clamp(0.0, 1.0);
            app.update();
            let motion = app.world().get::<crate::actual_motion::AlsActualMotion>(actor).unwrap();
            if motion.world_velocity.y <= IN_AIR_SETTINGS.prediction_velocity_threshold {
                assert_eq!(*app.world().get::<crate::als::state::LocomotionState>(actor).unwrap(), crate::als::state::LocomotionState::Air);
                let trace = app.world().get::<AlsGroundPredictionTrace>(avatar).expect("Falling source prediction must execute its sweep");
                let position = app.world().get::<avian3d::prelude::Position>(actor).unwrap().0;
                let transform = app.world().get::<Transform>(actor).unwrap();
                assert!(position.abs_diff_eq(transform.translation, 1e-6), "Actor pose must be synchronized after physics");
                let trace_start = trace.start;
                let velocity = motion.world_velocity;
                let actual_prediction = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap()
                    .get_global_input_data("in_air_source.ground_prediction_amount").unwrap().as_f32().unwrap();
                let expected_prediction = app.world_mut().run_system_once(move |spatial: crate::extensions::RagdollSpatialQuery,
                    actors: Query<(&Collider, &TnuaConfig<AlsControlScheme>)>,
                    avatars: Query<&GlobalTransform>, configs: Res<Assets<AlsControlSchemeConfig>>,
                    characters: Query<Entity, With<AlsCharacter>>| {
                    let (capsule, config) = actors.get(actor).unwrap();
                    let config = configs.get(&config.0).unwrap();
                    let filter = SpatialQueryFilter::from_excluded_entities(characters.iter())
                        .with_mask(IN_AIR_SETTINGS.prediction_trace_layer);
                    ground_prediction_amount(&spatial, position, velocity, capsule,
                        avatars.get(avatar).unwrap().to_scale_rotation_translation().0.y,
                        config.basis.max_slope.cos(), &filter, allowance).0
                }).unwrap();
                assert!((actual_prediction - expected_prediction).abs() < 1e-6,
                    "Ground prediction uses a stale actor origin: actual={actual_prediction}, source={expected_prediction}, trace={trace_start:?}, actor={position:?}, velocity={velocity:?}");
                assert!(trace_start.abs_diff_eq(position, 1e-6), "Source sweep origin must use the current actor position");
                falling_samples += 1;
                if falling_samples == 3 { break; }
            }
            assert!(std::time::Instant::now() < deadline, "Falling prediction fixture made no progress");
        }
        assert_eq!(falling_samples, 3, "The accepted jump did not reach source prediction velocity");
    }
    use avian3d::prelude::*;
    use bevy::{asset::AssetEvent, ecs::system::RunSystemOnce};

    #[test]
    fn prediction_trace_is_published_by_the_actual_query_and_cleared_when_blocked_or_unavailable() {
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        app.world_mut().entity_mut(actor).insert(AlsActualMotion { world_velocity: Vec3::NEG_Y * 10.0, ..default() });
        for blocked in [false, true, false] {
            app.world_mut().entity_mut(player).insert(AlsRootAnimationContext { curves: [("GroundPredictionBlock".into(), if blocked { 1.0 } else { 0.0 })].into() });
            app.world_mut().run_system_once(locomotion_input_writer).unwrap();
            app.world_mut().flush();
            assert_eq!(app.world().get::<AlsGroundPredictionTrace>(avatar).is_some(), !blocked);
        }
        app.world_mut().entity_mut(player).remove::<AnimationGraphPlayer>();
        app.world_mut().run_system_once(locomotion_input_writer).unwrap();
        app.world_mut().flush();
        assert!(app.world().get::<AlsGroundPredictionTrace>(avatar).is_none(), "Unavailable player must not retain a previous prediction trace");
    }

    #[test]
    fn generated_air_inputs_initialize_with_their_actual_neighbor_plugins() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .insert_resource(crate::overlay_object::OverlayLibraryHandle(default()));
        app.add_plugins((
            LocomotionLayerPlugin,
            crate::locomotion_lifecycle::LocomotionLifecyclePlugin,
            crate::layering_layer::LayeringLayerPlugin,
            crate::head_layer::HeadLayerPlugin,
            crate::ragdolling_layer::RagdollingLayerPlugin,
            crate::ab_als_root::AlsRootGraphPlugin,
            crate::ab_als_provider::AbAlsProviderPlugin,
            crate::control_rig_apply::ControlRigApplyPlugin,
            crate::foot_ik_apply::FootIkApplyPlugin,
        ))
        .configure_sets(
            PostUpdate,
            (AnimationGraphSet::PrePhysics, AnimationGraphSet::PostPhysics, AnimationGraphSet::Final).chain(),
        );
        app.world_mut().schedule_scope(PostUpdate, |world, schedule| {
            schedule.initialize(world).expect("source input schedule must initialize");
        });
    }

    #[test]
    fn source_jump_signal_comes_from_the_accepted_action_and_latches_until_consumed() {
        use AlsControlSchemeActionDiscriminant::{Crouch, Jump};
        use TnuaActionFlowStatus::*;
        for (flow, expected) in [
            (NoAction, false),
            (ActionOngoing(Jump), false),
            (ActionStarted(Crouch), false),
            (ActionStarted(Jump), true),
            (
                Cancelled {
                    old: Crouch,
                    new: Jump,
                },
                true,
            ),
        ] {
            let mut player = AnimationGraphPlayer::new(default());
            write_jump_request(&mut player, &flow, false);
            let value = source_player_property(&player, "in_air_state.b_jumped")
                .unwrap()
                .as_bool()
                .unwrap();
            assert_eq!(value, expected, "{flow:?}");
            write_jump_request(&mut player, &NoAction, false);
            assert_eq!(
                source_player_property(&player, "in_air_state.b_jumped")
                    .unwrap()
                    .as_bool()
                    .unwrap(),
                expected
            );
            write_jump_request(&mut player, &flow, true);
            assert!(
                !source_player_property(&player, "in_air_state.b_jumped")
                    .unwrap()
                    .as_bool()
                    .unwrap()
            );
        }
    }

    #[test]
    fn in_air_lean_samples_the_native_key_in_runtime_velocity_units() {
        let amount = in_air_lean_target(Vec3::new(0.0, -22.519_31, -3.5), Quat::IDENTITY);
        assert!((amount.x + 0.932_24).abs() < 1e-6, "{amount:?}");
        assert_eq!(amount.y, 0.0);
        assert_eq!(IN_AIR_SETTINGS.ground_prediction.sample(0.0), 1.0);
        assert_eq!(IN_AIR_SETTINGS.ground_prediction.sample(1.0), 0.0);
        let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let velocity = rotation * Vec3::new(3.5, 0.0, -3.5);
        assert!(in_air_lean_target(velocity, rotation).distance(Vec2::ONE) < 1e-6);
    }

    #[test]
    fn ground_prediction_uses_the_source_mesh_scaled_sweep() {
        fn sample(spatial: crate::extensions::RagdollSpatialQuery) -> [f32; 5] {
            let filter = SpatialQueryFilter::default().with_mask(IN_AIR_SETTINGS.prediction_trace_layer);
            let unfiltered = SpatialQueryFilter::default();
            [
                (1.0, 1.0, 0.9, true),
                (3.0, 1.0, 0.9, true),
                (3.0, 0.0, 0.9, true),
                (1.0, 1.0, 0.9, false),
                (3.0, 1.0, 0.4, true),
            ]
            .map(|(scale, allowance, half_height, filtered)| {
                let capsule = crate::als::character::als_character_capsule(half_height, 0.3);
                let (amount, trace) = ground_prediction_amount(
                    &spatial,
                    Vec3::Y * 3.0,
                    Vec3::NEG_Y * 2.0,
                    &capsule,
                    scale,
                    0.71,
                    if filtered {
                        &filter
                    } else {
                        &unfiltered
                    },
                    allowance,
                );
                assert_eq!(trace.is_some(), allowance > 1e-4);
                if let Some(trace) = trace {
                    assert!((trace.end.y - (3.0 - 1.5 * scale)).abs() < 1e-6);
                    assert!((trace.shape.shape_scaled().compute_local_aabb().half_extents().y - half_height).abs() < 1e-6);
                }
                amount
            })
        }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::new(Update)))
            .init_resource::<Assets<Mesh>>()
            .add_message::<AssetEvent<Mesh>>();
        app.finish();
        app.world_mut().spawn((
            RigidBody::Static,
            ColliderLabel("visible_world".to_owned()),
            Collider::cuboid(20.0, 0.1, 20.0),
            CollisionLayers::new([crate::als::character::AlsCollisionLayer::Default, crate::als::character::AlsCollisionLayer::Visibility], LayerMask::ALL),
            Transform::from_xyz(0.0, -0.05, 0.0),
        ));
        app.world_mut().spawn((
            RigidBody::Static,
            ColliderLabel("ignored_body".to_owned()),
            Collider::cuboid(20.0, 0.1, 20.0),
            Transform::from_xyz(0.0, 1.5, 0.0),
        ));
        for _ in 0..3 {
            app.update();
        }
        let amounts = app.world_mut().run_system_once(sample).unwrap();
        assert_eq!(amounts[0], 0.0);
        assert!(
            amounts[1] > 0.1,
            "source mesh scale 3 should reach the floor: {amounts:?}"
        );
        assert_eq!(amounts[2], 0.0);
        assert!(
            amounts[3] > amounts[1],
            "omitting the source filter should hit the ignored body: {amounts:?}"
        );
        assert!(
            amounts[4] < amounts[1],
            "the actual capsule shape must affect hit time: {amounts:?}"
        );
    }
}
