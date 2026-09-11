#[cfg(test)]
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use std::sync::LazyLock;

use als_core::{
    character::AlsCharacter,
    control_scheme::{
        AlsControlScheme, AlsControlSchemeActionDiscriminant, AlsControlSchemeConfig,
    },
};
use als_ext::source_instance::{set_source_frame_input, source_player_property};
use avian3d::prelude::{Collider, ShapeCastConfig, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_graph_player::AnimationGraphPlayer,
    ragdoll::definition::ColliderLabel,
};
use bevy_tnua::{TnuaConfig, TnuaController, controller::TnuaActionFlowStatus};

use crate::{
    actual_motion::AlsActualMotion,
    animation_context_projection::{AlsRootAnimationContext, AlsRootAnimationContextSet},
    character_assembly::AlsCharacterAvatar,
    generated_ab_als_provider::GeneratedAbAlsProviderSet,
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
    lean: als_ext::als_curves::AlsFloatCurve,
    ground_prediction: als_ext::als_curves::AlsFloatCurve,
}

static IN_AIR_SETTINGS: LazyLock<InAirSettings> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../assets/character/als_in_air_settings.json"))
        .expect("generated in-air settings are valid")
});

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AlsMovementBaseAnimationState {
    pub has_relative_location: bool,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AlsGroundPredictionState {
    pub amount: f32,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsLocomotionLayerSet;

pub struct LocomotionLayerPlugin;

impl Plugin for LocomotionLayerPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsLocomotionLayerSet
                .after(GeneratedAbAlsProviderSet)
                .before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
        )
        .add_systems(
            PostUpdate,
            locomotion_input_writer.in_set(AlsLocomotionLayerSet),
        )
        .add_systems(
            PostUpdate,
            project_in_air_state
                .after(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate)
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
) -> f32 {
    let settings = &*IN_AIR_SETTINGS;
    if velocity.y > settings.prediction_velocity_threshold || allowance <= 1e-4 {
        return 0.0;
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
    hit.filter(|hit| hit.normal1.y >= walkable_floor_cos)
        .map_or(0.0, |hit| {
            settings.ground_prediction.sample(hit.distance / distance) * allowance
        })
}

#[allow(clippy::type_complexity)]
fn locomotion_input_writer(
    spatial: SpatialQuery,
    configs: Res<Assets<AlsControlSchemeConfig>>,
    characters: Query<
        (
            &TnuaController<AlsControlScheme>,
            &TnuaConfig<AlsControlScheme>,
            &Transform,
            &GlobalTransform,
            &Collider,
            &AlsActualMotion,
        ),
        With<AlsCharacter>,
    >,
    non_world_colliders: Query<Entity, Or<(With<AlsCharacter>, With<ColliderLabel>)>>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance, &GlobalTransform), With<AlsCharacterAvatar>>,
    mut players: Query<(&mut AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
) {
    let filter = SpatialQueryFilter::from_excluded_entities(non_world_colliders.iter());
    for (parent, instance, mesh) in &avatars {
        let Ok((controller, config, transform, global, capsule, motion)) =
            characters.get(parent.parent())
        else {
            continue;
        };
        let Some(config) = configs.get(&config.0) else {
            continue;
        };
        let Ok((mut player, context)) = players.get_mut(instance.player_entity()) else {
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
        let prediction = ground_prediction_amount(
            &spatial,
            global.translation(),
            motion.world_velocity,
            capsule,
            mesh.to_scale_rotation_translation().0.y,
            config.basis.max_slope.cos(),
            &filter,
            allowance,
        );
        player.set_global_input_data("in_air_source.ground_prediction_amount", prediction);
        player.set_global_input_data(
            "in_air_source.lean_target",
            in_air_lean_target(motion.world_velocity, transform.rotation),
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
    use avian3d::prelude::*;
    use bevy::{asset::AssetEvent, ecs::system::RunSystemOnce};

    #[test]
    fn generated_air_inputs_initialize_with_their_actual_neighbor_plugins() {
        let mut app = App::new();
        app.add_plugins((
            LocomotionLayerPlugin,
            crate::locomotion_lifecycle::LocomotionLifecyclePlugin,
            crate::layering_layer::LayeringLayerPlugin,
            crate::head_layer::HeadLayerPlugin,
            crate::ragdolling_layer::RagdollingLayerPlugin,
            crate::ab_als_root::AlsRootGraphPlugin,
            crate::generated_ab_als_provider::GeneratedAbAlsProviderPlugin,
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
        fn sample(spatial: SpatialQuery, ignored: Query<Entity, With<ColliderLabel>>) -> [f32; 5] {
            let filter = SpatialQueryFilter::from_excluded_entities(ignored.iter());
            let unfiltered = SpatialQueryFilter::default();
            [
                (1.0, 1.0, 0.9, true),
                (3.0, 1.0, 0.9, true),
                (3.0, 0.0, 0.9, true),
                (1.0, 1.0, 0.9, false),
                (3.0, 1.0, 0.4, true),
            ]
            .map(|(scale, allowance, half_height, filtered)| {
                let capsule = als_core::character::als_character_capsule(half_height, 0.3);
                ground_prediction_amount(
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
                )
            })
        }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::new(Update)))
            .init_resource::<Assets<Mesh>>()
            .add_message::<AssetEvent<Mesh>>();
        app.finish();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(20.0, 0.1, 20.0),
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
