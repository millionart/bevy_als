use std::time::Duration;

use als_core::{character::AlsCharacter, state::{AlsNotifyLocomotionAction, LocomotionState}};
use avian3d::dynamics::{integrator::{IntegrationSystems, VelocityIntegrationData}, solver::solver_body::SolverBody};
use avian3d::prelude::{
    Collider, ColliderOf, CollisionLayers, CustomPositionIntegration, Physics, PhysicsSchedule,
    PhysicsSystems, Position, RigidBody, Sensor, ShapeCastConfig, SpatialQuery, SpatialQueryFilter,
    SubstepCount, SubstepSchedule,
};
use bevy::prelude::*;
use bevy_animation_graph::core::plugin::AnimationGraphSet;

pub const SOURCE_MAX_PHYSICS_DELTA: Duration = Duration::from_nanos(33_333_333);

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourcePhysicsTimingSet {
    Clamp,
    Restore,
}

#[derive(Resource, Default)]
struct SourcePhysicsClockRestore {
    previous: Option<Time>,
}

#[derive(Component)]
struct SourceCharacterFullFrame;

pub struct SourcePhysicsTimingPlugin;

impl Plugin for SourcePhysicsTimingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SourcePhysicsClockRestore>()
            .configure_sets(
                PostUpdate,
                (
                    SourcePhysicsTimingSet::Clamp
                        .after(AnimationGraphSet::PrePhysics)
                        .before(PhysicsSystems::First),
                    SourcePhysicsTimingSet::Restore
                        .after(PhysicsSystems::Last)
                        .before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Inputs)
                        .before(AnimationGraphSet::PostPhysics),
                ),
            )
            .add_systems(
                PostUpdate,
                (clamp_source_physics_delta, enable_character_full_frame)
                    .chain()
                    .in_set(SourcePhysicsTimingSet::Clamp),
            )
            .add_systems(
                PostUpdate,
                (disable_character_full_frame, restore_source_physics_delta)
                    .chain()
                    .in_set(SourcePhysicsTimingSet::Restore),
            )
            .add_systems(
                PhysicsSchedule,
                integrate_character_velocity
                    .in_set(IntegrationSystems::UpdateVelocityIncrements)
                    .after(avian3d::dynamics::integrator::pre_process_velocity_increments),
            )
            .add_systems(
                SubstepSchedule,
                integrate_character_position
                    .in_set(IntegrationSystems::Position)
                    .after(avian3d::dynamics::integrator::integrate_positions),
            );
    }
}

fn integrate_character_velocity(
    frame_time: Res<Time<Virtual>>,
    physics_time: Res<Time<Physics>>,
    mut characters: Query<(&mut VelocityIntegrationData, &LocomotionState, Ref<Collider>), With<SourceCharacterFullFrame>>,
) {
    if physics_time.delta().is_zero() || frame_time.delta() <= SOURCE_MAX_PHYSICS_DELTA {
        return;
    }
    let ratio = frame_time.delta_secs() / physics_time.delta_secs();
    for (mut integration, locomotion, collider) in &mut characters {
        if *locomotion == LocomotionState::Grounded && collider.is_changed() {
            integration.linear_increment.x *= ratio;
            integration.linear_increment.z *= ratio;
        } else {
            integration.linear_increment *= ratio;
        }
        integration.angular_increment *= ratio;
    }
}

fn source_physics_delta(delta: Duration) -> Duration {
    delta.min(SOURCE_MAX_PHYSICS_DELTA)
}

fn clamp_source_physics_delta(
    mut restore: ResMut<SourcePhysicsClockRestore>,
    mut clock: ResMut<Time>,
) {
    let previous = clock.clone();
    let mut capped = previous.clone();
    capped.advance_by(source_physics_delta(previous.delta()));
    restore.previous = Some(previous);
    *clock = capped;
}

fn restore_source_physics_delta(
    mut restore: ResMut<SourcePhysicsClockRestore>,
    mut clock: ResMut<Time>,
) {
    let Some(previous) = restore.previous.take() else {
        return;
    };
    *clock = previous;
}

fn enable_character_full_frame(
    frame_time: Res<Time<Virtual>>,
    mut commands: Commands,
    characters: Query<(Entity, &LocomotionState, &RigidBody, Option<&AlsNotifyLocomotionAction>,
        Option<&als_core::dynamics::AlsMovementBaseDynamicsState>),
        (With<AlsCharacter>, Without<CustomPositionIntegration>)>,
) {
    if frame_time.delta() <= SOURCE_MAX_PHYSICS_DELTA {
        return;
    }
    for (entity, locomotion, body, action, base) in &characters {
        if base.is_some_and(|base| base.entity.is_some() && base.support_velocity.length_squared() > 0.0) {
            continue;
        }
        if *body == RigidBody::Dynamic && action != Some(&AlsNotifyLocomotionAction::Ragdolling)
            && matches!(*locomotion, LocomotionState::Grounded | LocomotionState::Air) {
            commands.entity(entity).insert((CustomPositionIntegration, SourceCharacterFullFrame));
        }
    }
}

fn disable_character_full_frame(
    mut commands: Commands,
    characters: Query<Entity, With<SourceCharacterFullFrame>>,
) {
    for entity in &characters {
        commands.entity(entity).remove::<(CustomPositionIntegration, SourceCharacterFullFrame)>();
    }
}

fn integrate_character_position(
    frame_time: Res<Time<Virtual>>,
    physics_time: Res<Time<Physics>>,
    substeps: Res<SubstepCount>,
    spatial: SpatialQuery,
    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>)>,
    mut characters: Query<(Entity, &mut SolverBody, &Position, &Collider, &LocomotionState,
        Option<&AlsNotifyLocomotionAction>, &RigidBody, Option<&als_core::dynamics::AlsMovementBaseDynamicsState>,
        Option<&CollisionLayers>, Has<SourceCharacterFullFrame>, Has<CustomPositionIntegration>,
        &mut als_core::character::AlsCmcAdapterRuntime), With<AlsCharacter>>,
) {
    if frame_time.delta() <= SOURCE_MAX_PHYSICS_DELTA {
        return;
    }
    let frame_step = frame_time.delta_secs() / substeps.0 as f32;
    let extra_step = (frame_time.delta_secs() - physics_time.delta_secs()) / substeps.0 as f32;
    for (entity, mut body, position, collider, locomotion, action, rigid_body, base, layers, full_frame, custom_position, mut runtime) in &mut characters {
        let movement = if full_frame {
            body.linear_velocity * frame_step
        } else if !custom_position && extra_step > 0.0 && *rigid_body == RigidBody::Dynamic
            && *locomotion == LocomotionState::Grounded
            && action != Some(&AlsNotifyLocomotionAction::Ragdolling)
            && base.is_some_and(|base| base.entity.is_some() && base.support_velocity.length_squared() > 0.0) {
            als_core::dynamics::locomotion_velocity(body.linear_velocity, *locomotion, base) * extra_step
        } else {
            continue;
        };
        let start = position.0 + body.delta_position;
        let layers = layers.copied().unwrap_or_default();
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let accepts = |candidate| candidates.get(candidate).is_ok_and(|(candidate_layers, sensor, owner)| {
            !sensor && !owner.is_some_and(|owner| owner.body == entity)
                && layers.interacts_with(candidate_layers.copied().unwrap_or_default())
        });
        let mut remaining = movement;
        let mut remaining_time = frame_step;
        let mut previous_normal = None;
        let mut applied = Vec3::ZERO;
        for _ in 0..3 {
            let Ok((direction, distance)) = Dir3::new_and_length(remaining) else { break; };
            let config = ShapeCastConfig { ignore_origin_penetration: true,
                ..ShapeCastConfig::from_max_distance(distance) };
            let Some(hit) = spatial.cast_shape_predicate(collider, start + applied, Quat::IDENTITY,
                direction, &config, &filter, &accepts) else {
                applied += remaining;
                break;
            };
            let travel = als_core::motion::movement_sweep_travel(distance, hit.distance);
            applied += *direction * travel;
            let Some(normal) = hit.normal1.try_normalize() else { break; };
            let mut slide = remaining - *direction * travel;
            if *locomotion == LocomotionState::Air {
                if normal.y > 0.0 && remaining.dot(normal) < 0.0 {
                    runtime.airborne_sweep_contact = Some(hit.entity);
                }
                slide = als_ext::motion::falling_slide(slide, normal, previous_normal);
                previous_normal = Some(normal);
                remaining_time *= 1.0 - travel / distance;
                if remaining_time > f32::EPSILON {
                    body.linear_velocity = slide / remaining_time;
                }
            } else {
                slide -= normal * slide.dot(normal).min(0.0);
                let inward_velocity = body.linear_velocity.dot(normal).min(0.0);
                body.linear_velocity -= normal * inward_velocity;
            }
            if slide.dot(remaining) <= 0.0 { break; }
            remaining = slide;
        }
        body.delta_position += applied;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::prelude::{Collider, CollisionLayers, GravityScale, LayerMask, LinearVelocity, Position, RigidBody, Sensor};
    use bevy::time::TimeUpdateStrategy;

    #[test]
    fn source_physics_delta_is_a_per_frame_cap_not_a_fixed_timestep() {
        assert_eq!(
            source_physics_delta(Duration::from_millis(12)),
            Duration::from_millis(12)
        );
        assert_eq!(
            source_physics_delta(Duration::from_millis(50)),
            SOURCE_MAX_PHYSICS_DELTA
        );
    }

    #[test]
    fn airborne_gravity_consumes_the_full_frame() {
        for millis in [16, 20, 50, 100] {
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins,
                AssetPlugin::default(),
                TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate),
                SourcePhysicsTimingPlugin,
            ))
            .init_asset::<Mesh>()
            .add_message::<bevy::asset::AssetEvent<Mesh>>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(millis)));
            app.finish();
            app.update();
            let character = app.world_mut().spawn((
                als_core::character::AlsCharacter,
                LocomotionState::Air,
                RigidBody::Dynamic,
                Collider::cuboid(0.2, 0.2, 0.2),
                Position(Vec3::ZERO),
                LinearVelocity(Vec3::ZERO),
                Transform::default(),
            )).id();
            app.update();
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().y;
            let expected = -9.81 * millis as f32 / 1000.0;
            assert!((velocity - expected).abs() < 1e-4,
                "airborne gravity at {millis} ms: {velocity} != {expected}");
        }
    }

    #[test]
    fn horizontal_acceleration_consumes_the_full_character_frame() {
        for mode in [LocomotionState::Grounded, LocomotionState::Air] {
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate), SourcePhysicsTimingPlugin,
            ))
            .init_asset::<Mesh>()
            .add_message::<bevy::asset::AssetEvent<Mesh>>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
            app.finish();
            app.update();
            let character = app.world_mut().spawn((
                als_core::character::AlsCharacter, mode, RigidBody::Dynamic,
                Collider::cuboid(0.2, 0.2, 0.2),
                avian3d::prelude::GravityScale(0.0),
                avian3d::prelude::ConstantLinearAcceleration(Vec3::X * 4.0),
                Position(Vec3::ZERO), LinearVelocity(Vec3::ZERO), Transform::default(),
            )).id();
            app.update();
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().x;
            assert!((velocity - 0.4).abs() < 1e-4,
                "{mode:?} horizontal acceleration used {velocity} instead of 0.4");
        }
    }

    fn simulate_air_wall_motion(walls: &[(Vec3, f32)], start_x: f32, velocity: Vec3) -> (Vec3, Vec3) {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            SourcePhysicsTimingPlugin,
        ))
        .init_asset::<Mesh>()
        .add_message::<bevy::asset::AssetEvent<Mesh>>()
        .insert_resource(SubstepCount(6))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app.finish();
        app.update();
        let character = app.world_mut().spawn((
            als_core::character::AlsCharacter,
            LocomotionState::Air,
            RigidBody::Dynamic,
            Collider::capsule(0.3, 1.2),
            avian3d::prelude::SpeculativeMargin::MAX,
            avian3d::prelude::LockedAxes::ROTATION_LOCKED,
            Position(Vec3::new(start_x, 2.0, 0.0)),
            LinearVelocity(velocity),
            Transform::from_xyz(start_x, 2.0, 0.0),
        )).id();
        for &(normal, gap) in walls {
            let support = 0.3 + 0.6 * normal.y.abs();
            app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(0.02, 20.0, 20.0),
                Transform::from_translation(Vec3::Y * 2.0 - normal * (support + gap + 0.01))
                    .with_rotation(Quat::from_rotation_arc(Vec3::X, normal)),
            ));
        }
        app.update();
        (app.world().get::<Position>(character).unwrap().0,
            app.world().get::<LinearVelocity>(character).unwrap().0)
    }

    #[test]
    fn first_air_wall_impact_keeps_the_remaining_tangential_motion() {
        let velocity = Vec3::new(3.75, 0.0, 3.75);
        let (free_position, free_velocity) = simulate_air_wall_motion(&[], 0.0, velocity);
        let (wall_position, wall_velocity) = simulate_air_wall_motion(&[(Vec3::NEG_X, 0.29)], 0.0, velocity);
        assert!((free_position.z - 0.375).abs() < 1e-4, "free air control: {free_position:?}");
        assert!((free_velocity.y + 0.981).abs() < 1e-4, "free air gravity: {free_velocity:?}");
        assert!(wall_position.x < 0.31, "first air impact crossed the wall: {wall_position:?}");
        assert!(wall_position.z >= free_position.z - 0.005,
            "first air impact lost tangent: wall={wall_position:?} free={free_position:?}");
        assert!(wall_velocity.x <= 0.1 && wall_velocity.z >= 3.7,
            "first air impact retained inward velocity: {wall_velocity:?}");
    }

    #[test]
    fn air_sweep_handles_a_second_wall_and_origin_overlap() {
        let (free_position, _) = simulate_air_wall_motion(
            &[], 0.0, Vec3::new(3.75, 0.0, 3.75));
        let (corner_position, corner_velocity) = simulate_air_wall_motion(
            &[(Vec3::NEG_X, 0.29), (Vec3::NEG_Z, 0.29)], 0.0, Vec3::new(3.75, 0.0, 3.75));
        assert!(corner_position.x < 0.31 && corner_position.z < 0.31,
            "air motion crossed a two-wall corner: {corner_position:?}");
        assert!(corner_position.y <= free_position.y + 0.002,
            "two-wall corner discarded falling motion: {corner_position:?} free={free_position:?}");
        assert!(corner_velocity.x <= 0.1 && corner_velocity.z <= 0.1,
            "two-wall corner retained inward velocity: {corner_velocity:?}");
        let (outward_position, outward_velocity) = simulate_air_wall_motion(
            &[(Vec3::NEG_X, 0.29)], 0.32, Vec3::new(-3.75, 0.0, 3.75));
        assert!(outward_position.is_finite() && outward_velocity.is_finite()
            && outward_position.x < 0.2 && outward_position.z > 0.34,
            "origin overlap trapped outward air motion: {outward_position:?} {outward_velocity:?}");
        let (inward_position, inward_velocity) = simulate_air_wall_motion(
            &[(Vec3::NEG_X, 0.29)], 0.32, Vec3::new(3.75, 0.0, 3.75));
        assert!(inward_position.is_finite() && inward_velocity.is_finite()
            && inward_position.x < 0.31 && inward_position.z > 0.05,
            "origin overlap failed to bound inward air motion: {inward_position:?} {inward_velocity:?}");
    }

    #[test]
    fn first_air_slope_impact_does_not_create_upward_motion() {
        for side in [-1.0, 1.0] {
            let normal = Vec3::new(-side * 3.0_f32.sqrt() * 0.5, 0.5, 0.0);
            for upward in [0.0, 2.0] {
                let velocity = Vec3::new(side * 3.75, upward, 3.75);
                let gap = if upward > 0.0 { 0.22 } else { 0.26 };
                let (free_position, free_velocity) = simulate_air_wall_motion(&[], 0.0, velocity);
                let (position, result) = simulate_air_wall_motion(&[(normal, gap)], 0.0, velocity);
                assert!(position.is_finite() && result.is_finite());
                assert!(position.x.abs() < free_position.x.abs() - 0.005,
                    "slope fixture missed its first impact: {position:?} {free_position:?}");
                assert!((position - Vec3::Y * 2.0).dot(normal) >= -gap - 0.002,
                    "slope penetration: {position:?} {normal:?}");
                assert!((position.z - free_position.z).abs() < 0.002,
                    "slope lost parallel motion: {position:?} {free_position:?}");
                if upward == 0.0 {
                    assert!(result.y <= 0.0 && position.y <= 2.0,
                        "falling slope created upward motion: {position:?} {result:?}");
                } else {
                    assert!(result.y > 0.0 && result.y <= free_velocity.y + 0.002
                        && position.y <= free_position.y + 0.002,
                        "slope changed legal jump ascent: {position:?} {result:?} free={free_position:?} {free_velocity:?}");
                }
            }
        }
    }

    #[test]
    fn first_air_nonorthogonal_corner_keeps_falling_motion() {
        for side in [1.0, -1.0] {
            for degrees in [60.0_f32, 75.0, 120.0] {
                let angle = degrees.to_radians();
                let normals = [Vec3::NEG_X * side, Vec3::new(side * angle.cos(), 0.0, -angle.sin())];
                let velocity = Vec3::new(side * 3.75, -3.0,
                    if degrees == 60.0 { 6.5 } else { 3.75 * (1.0 + angle.cos()) / angle.sin() });
                assert!(velocity.length() * SOURCE_MAX_PHYSICS_DELTA.as_secs_f32() < 0.29);
                let (free_position, free_velocity) = simulate_air_wall_motion(&[], 0.0, velocity);
                let (position, result) = simulate_air_wall_motion(
                    &[(normals[0], 0.29), (normals[1], 0.29)], 0.0, velocity);
                assert!((position.y - free_position.y).abs() < 0.002,
                    "{degrees} degree corner lost falling motion: {position:?} free={free_position:?}");
                assert!((result.y - free_velocity.y).abs() < 0.002,
                    "corner changed falling velocity: {result:?} free={free_velocity:?}");
                for normal in normals {
                    assert!((position - Vec3::Y * 2.0).dot(normal) >= -0.292,
                        "corner penetration: {position:?} {normal:?}");
                    assert!(result.dot(normal) >= -0.01,
                        "corner retained inward velocity: {result:?} {normal:?}");
                }
                if degrees > 90.0 {
                    assert!(Vec2::new(result.x, result.z).length() > 0.5,
                        "obtuse corner erased valid horizontal slide: {result:?}");
                }
            }
        }
    }

    #[test]
    fn air_sweep_lands_on_a_walkable_edge_without_solver_contact() {
        use als_core::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
        use als_core::motion::{AlsMotionPlugin, initial_als_control_scheme_config};
        use als_core::params::AlsMovementParams;
        use als_core::transition::AlsStateTransitionPlugin;
        use bevy_tnua::TnuaControllerPlugin;
        use bevy_tnua_avian3d::TnuaAvian3dPlugin;
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            TnuaAvian3dPlugin::new(Update), TnuaControllerPlugin::<AlsControlScheme>::new(Update),
            AlsStateTransitionPlugin, AlsCharacterPlugin, AlsMotionPlugin, SourcePhysicsTimingPlugin,
        ))
        .init_asset::<Mesh>()
        .add_message::<bevy::asset::AssetEvent<Mesh>>()
        .insert_resource(SubstepCount(6))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app.finish();
        app.update();
        let value = AlsMovementParams::default();
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(value.clone());
        let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>()
            .add(initial_als_control_scheme_config(&value));
        let mut bundle = AlsCharacterBundle::new(params, config);
        bundle.transform.translation = Vec3::new(0.0, 5.0, 0.417345);
        let character = app.world_mut().spawn(bundle).id();
        *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
        app.world_mut().get_mut::<als_core::input::AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(10.0, 4.0, 20.0),
            Transform::from_xyz(0.0, -2.0, -10.0)));
        for _ in 0..3 { app.update(); }
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air);
        let start = Vec3::new(0.0, 1.0146527, 0.417345);
        app.world_mut().get_mut::<Position>(character).unwrap().0 = start;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation = start;
        app.world_mut().get_mut::<LinearVelocity>(character).unwrap().0 = Vec3::new(0.0, -1.686, -2.3968756);
        for frame in 0..12 {
            app.update();
            if frame == 0 {
                let position = app.world().get::<Position>(character).unwrap().0;
                let (floor, contact) = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                    collisions: avian3d::prelude::Collisions| {
                    let floor = als_core::motion::find_ground_floor(&spatial, position, 0.9, 0.3,
                        &SpatialQueryFilter::from_excluded_entities([character]));
                    let contact = collisions.collisions_with(character).any(|pair| pair.manifolds.iter()
                        .any(|manifold| manifold.points.iter().any(|point| point.normal_impulse > 0.0)));
                    (floor, contact)
                }).unwrap();
                assert!(!contact && floor.is_some_and(|(center, _, _)| (position.y - center).abs() <= 0.024),
                    "fixture requires a sweep-only walkable contact: {floor:?} {contact}");
                assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Grounded,
                    "first blocking sweep did not land: {position:?}");
            }
        }
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Grounded);
    }

    #[test]
    fn full_frame_crouch_keeps_the_capsule_on_the_floor() {
        use als_core::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
        use als_core::motion::{AlsMotionPlugin, initial_als_control_scheme_config};
        use als_core::params::AlsMovementParams;
        use als_core::state::Stance;
        use als_core::transition::AlsStateTransitionPlugin;
        use bevy_tnua::TnuaControllerPlugin;
        use bevy_tnua_avian3d::TnuaAvian3dPlugin;
        #[derive(Resource, Default)]
        struct GeometryChanges(Vec<(bool, bool)>);

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            TnuaAvian3dPlugin::new(Update),
            TnuaControllerPlugin::<AlsControlScheme>::new(Update),
            AlsStateTransitionPlugin, AlsCharacterPlugin, AlsMotionPlugin, SourcePhysicsTimingPlugin,
        ))
        .init_asset::<Mesh>()
        .add_message::<bevy::asset::AssetEvent<Mesh>>()
        .init_resource::<GeometryChanges>()
        .add_systems(PhysicsSchedule, (|mut changes: ResMut<GeometryChanges>,
            colliders: Query<(Ref<Collider>, Ref<Stance>), With<AlsCharacter>>| {
            for (collider, stance) in &colliders {
                changes.0.push((collider.is_changed(), stance.is_changed()));
            }
        }).before(integrate_character_velocity))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        let params_value = AlsMovementParams::default();
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(params_value.clone());
        let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>()
            .add(initial_als_control_scheme_config(&params_value));
        let mut bundle = AlsCharacterBundle::new(params, config);
        bundle.transform.translation = Vec3::Y * 0.924;
        let character = app.world_mut().spawn(bundle).id();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(10.0, 0.2, 10.0),
            Transform::from_xyz(0.0, -0.1, 0.0)));
        for _ in 0..30 { app.update(); }
        assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app.update();
        assert_eq!(app.world().resource::<GeometryChanges>().0.last(), Some(&(false, true)));
        app.world_mut().get_mut::<als_core::transition::AlsStateRequest>(character).unwrap().stance = Some(Stance::Crouching);
        for frame in 0..2 {
            app.update();
            assert_eq!(app.world().resource::<GeometryChanges>().0.last(), Some(&(frame == 0, true)));
            let position = app.world().get::<Position>(character).unwrap().0;
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            let mode = app.world().get::<LocomotionState>(character).unwrap();
            assert!(position.y <= 1.0 && *mode == LocomotionState::Grounded,
                "crouch launched character at frame {frame}: {position:?} {velocity:?} {mode:?}");
        }
    }

    #[test]
    fn full_frame_air_sweep_only_blocks_colliding_external_solids() {
        let default = CollisionLayers::new(LayerMask::DEFAULT, LayerMask::DEFAULT);
        let cases = [
            ("other_membership", CollisionLayers::new(LayerMask(2), LayerMask(2)), false, false, false),
            ("other_filter", CollisionLayers::new(LayerMask::DEFAULT, LayerMask(2)), false, false, false),
            ("sensor", default, true, false, false),
            ("same_body", default, false, true, false),
            ("blocking", default, false, false, true),
        ];
        for (name, layers, sensor, attached, blocking) in cases {
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins,
                AssetPlugin::default(),
                TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate),
                SourcePhysicsTimingPlugin,
            ))
            .init_asset::<Mesh>()
            .add_message::<bevy::asset::AssetEvent<Mesh>>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
            app.finish();
            app.update();
            let character = app.world_mut().spawn((
                als_core::character::AlsCharacter,
                LocomotionState::Air,
                RigidBody::Dynamic,
                Collider::cuboid(0.2, 0.2, 0.2),
                CollisionLayers::new(LayerMask::DEFAULT, LayerMask::DEFAULT),
                GravityScale(0.0),
                Position(Vec3::ZERO),
                LinearVelocity(Vec3::X * 3.75),
                Transform::default(),
            )).id();
            let obstacle = app.world_mut().spawn((
                Collider::cuboid(0.05, 0.4, 0.4),
                layers,
                Transform::from_xyz(0.3, 0.0, 0.0),
            )).id();
            if sensor {
                app.world_mut().entity_mut(obstacle).insert(Sensor);
            }
            if attached {
                app.world_mut().entity_mut(character).add_child(obstacle);
            } else {
                app.world_mut().entity_mut(obstacle).insert(RigidBody::Static);
            }
            app.update();
            let position = app.world().get::<Position>(character).unwrap().x;
            if blocking {
                assert!(position < 0.3, "{name} must stop the air sweep: {position}");
            } else {
                assert!((position - 0.375).abs() < 1e-4,
                    "{name} changed full-frame air motion: {position}");
            }
        }
    }

    #[test]
    fn free_character_motion_consumes_the_full_frame() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            SourcePhysicsTimingPlugin,
        ))
        .init_asset::<Mesh>()
        .add_message::<bevy::asset::AssetEvent<Mesh>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app.finish();
        app.update();
        let body = app.world_mut().spawn((
            als_core::character::AlsCharacter,
            LocomotionState::Grounded,
            RigidBody::Dynamic,
            Collider::cuboid(0.2, 0.2, 0.2),
            GravityScale(0.0),
            Position(Vec3::ZERO),
            LinearVelocity(Vec3::X * 3.75),
            Transform::default(),
        )).id();
        let rigid = app.world_mut().spawn((
            RigidBody::Dynamic,
            Collider::cuboid(0.2, 0.2, 0.2),
            GravityScale(0.0),
            Position(Vec3::Y * 3.0),
            LinearVelocity(Vec3::X * 3.75),
            Transform::from_xyz(0.0, 3.0, 0.0),
        )).id();
        let ragdoll = app.world_mut().spawn((
            als_core::character::AlsCharacter,
            LocomotionState::Grounded,
            als_core::state::AlsNotifyLocomotionAction::Ragdolling,
            RigidBody::Kinematic,
            Collider::cuboid(0.2, 0.2, 0.2),
            GravityScale(0.0),
            Position(Vec3::Y * 6.0),
            LinearVelocity(Vec3::X * 3.75),
            Transform::from_xyz(0.0, 6.0, 0.0),
        )).id();
        let ragdoll_pending = app.world_mut().spawn((
            als_core::character::AlsCharacter,
            LocomotionState::Grounded,
            als_core::state::AlsNotifyLocomotionAction::Ragdolling,
            RigidBody::Dynamic,
            Collider::cuboid(0.2, 0.2, 0.2),
            GravityScale(0.0),
            Position(Vec3::Y * 9.0),
            LinearVelocity(Vec3::X * 3.75),
            Transform::from_xyz(0.0, 9.0, 0.0),
        )).id();
        for seconds in [1.0 / 120.0, 1.0 / 60.0, 1.0 / 30.0, 1.0 / 20.0, 1.0 / 15.0, 0.1, 0.2] {
            let before = app.world().get::<Position>(body).unwrap().0.x;
            let rigid_before = app.world().get::<Position>(rigid).unwrap().0.x;
            let ragdoll_before = app.world().get::<Position>(ragdoll).unwrap().0.x;
            let pending_before = app.world().get::<Position>(ragdoll_pending).unwrap().0.x;
            let delta = Duration::from_secs_f32(seconds);
            app.insert_resource(TimeUpdateStrategy::ManualDuration(delta));
            app.update();
            let actual = app.world().get::<Position>(body).unwrap().0.x - before;
            let expected = 3.75 * delta.as_secs_f32();
            assert!((actual - expected).abs() < 1e-4,
                "full-frame movement at {seconds} s: {actual} != {expected}");
            assert!(app.world().get::<CustomPositionIntegration>(body).is_none());
            let rigid_x = app.world().get::<Position>(rigid).unwrap().0.x - rigid_before;
            let capped = 3.75 * source_physics_delta(delta).as_secs_f32();
            assert!((rigid_x - capped).abs() < 1e-4,
                "capped rigid-body movement at {seconds} s: {rigid_x} != {capped}");
            let ragdoll_x = app.world().get::<Position>(ragdoll).unwrap().0.x - ragdoll_before;
            assert!((ragdoll_x - capped).abs() < 1e-4,
                "ragdoll capsule inherited full-frame character movement at {seconds} s: {ragdoll_x} != {capped}");
            let pending_x = app.world().get::<Position>(ragdoll_pending).unwrap().0.x - pending_before;
            assert!((pending_x - capped).abs() < 1e-4,
                "ragdoll entry inherited full-frame character movement at {seconds} s: {pending_x} != {capped}");
        }
        app.world_mut().entity_mut(ragdoll).insert((
            RigidBody::Dynamic,
            als_core::state::AlsNotifyLocomotionAction::None,
        ));
        app.world_mut().entity_mut(ragdoll_pending)
            .insert(als_core::state::AlsNotifyLocomotionAction::None);
        let exited_x = app.world().get::<Position>(ragdoll).unwrap().0.x;
        let pending_x = app.world().get::<Position>(ragdoll_pending).unwrap().0.x;
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app.update();
        for (entity, before) in [(ragdoll, exited_x), (ragdoll_pending, pending_x)] {
            let moved = app.world().get::<Position>(entity).unwrap().0.x - before;
            assert!((moved - 0.375).abs() < 1e-4, "ragdoll exit did not restore full-frame movement: {moved}");
            assert!(app.world().get::<CustomPositionIntegration>(entity).is_none());
        }
    }

    #[test]
    fn full_frame_character_sweep_handles_wall_slide_and_landing() {
        use als_core::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
        use als_core::input::AlsInputCommand;
        use als_core::motion::{AlsMotionPlugin, initial_als_control_scheme_config};
        use als_core::params::AlsMovementParams;
        use als_core::transition::AlsStateTransitionPlugin;
        use bevy_tnua::TnuaControllerPlugin;
        use bevy_tnua_avian3d::TnuaAvian3dPlugin;

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            TnuaAvian3dPlugin::new(Update),
            TnuaControllerPlugin::<AlsControlScheme>::new(Update),
            AlsStateTransitionPlugin,
            AlsCharacterPlugin,
            AlsMotionPlugin,
            SourcePhysicsTimingPlugin,
        ))
        .init_asset::<Mesh>()
        .add_message::<bevy::asset::AssetEvent<Mesh>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        let params_value = AlsMovementParams::default();
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(params_value.clone());
        let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>()
            .add(initial_als_control_scheme_config(&params_value));
        let mut bundle = AlsCharacterBundle::new(params, config);
        bundle.transform.translation = Vec3::Y * 0.924;
        let character = app.world_mut().spawn(bundle).id();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(10.0, 0.2, 10.0), Transform::from_xyz(0.0, -0.1, 0.0)));
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.02, 3.0, 2.0), Transform::from_xyz(0.6, 1.5, 0.0)));
        for _ in 0..30 {
            app.update();
        }
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(character).unwrap().0 = Vec3::X * 3.75;
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::X;
        app.update();
        let at_wall = app.world().get::<Position>(character).unwrap().0;
        assert!(at_wall.x > 0.2 && at_wall.x < 0.31 && at_wall.y >= 0.9,
            "full-frame grounded wall response: {at_wall:?}");

        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(character).unwrap().0 = Vec3::new(3.75, 0.0, 3.75);
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::new(1.0, -1.0).normalize();
        app.update();
        let sliding = app.world().get::<Position>(character).unwrap().0;
        assert!(sliding.x < 0.31 && sliding.z > at_wall.z + 0.05,
            "full-frame grounded wall slide stopped: {at_wall:?} -> {sliding:?}");

        let air_wall_start = Vec3::new(sliding.x, 1.5, sliding.z);
        app.world_mut().get_mut::<Position>(character).unwrap().0 = air_wall_start;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation = air_wall_start;
        *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(character).unwrap().0 =
            Vec3::new(3.75, 0.0, 3.75);
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::ZERO;
        app.update();
        let air_slide = app.world().get::<Position>(character).unwrap().0;
        assert!(air_slide.x < 0.31 && air_slide.z > air_wall_start.z + 0.05,
            "full-frame air wall slide stopped: {air_wall_start:?} -> {air_slide:?}");

        let airborne_start = Vec3::new(4.0, 1.2, 0.0);
        app.world_mut().get_mut::<Position>(character).unwrap().0 = airborne_start;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation = airborne_start;
        *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(character).unwrap().0 = Vec3::NEG_Y * 3.75;
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::ZERO;
        app.update();
        let landed = app.world().get::<Position>(character).unwrap().0;
        assert!(landed.y >= 0.9, "full-frame air landing crossed ground: {landed:?}");
        for _ in 0..5 {
            app.update();
        }
        let settled = app.world().get::<Position>(character).unwrap().0;
        let mode = app.world().get::<LocomotionState>(character).unwrap();
        assert!(settled.y >= 0.9 && settled.y <= 0.96 && *mode == LocomotionState::Grounded,
            "full-frame air landing did not settle: {settled:?} {mode:?}");

        let platform = app.world_mut().spawn((
            RigidBody::Kinematic,
            Collider::cuboid(4.0, 0.5, 4.0),
            Transform::from_xyz(4.0, 2.0, 0.0),
        )).id();
        let platform_start = Vec3::new(4.0, 3.174, 0.0);
        app.world_mut().get_mut::<Position>(character).unwrap().0 = platform_start;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation = platform_start;
        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(character).unwrap().0 = Vec3::ZERO;
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
        for _ in 0..30 {
            app.update();
        }
        let base = app.world().get::<als_core::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
        assert_eq!(base.entity, Some(platform));
        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(platform).unwrap().0 = Vec3::X;
        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(character).unwrap().0 = Vec3::X;
        let carried_before = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x;
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app.update();
        let carried_after = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x;
        assert!((carried_after - carried_before).abs() < 1e-3,
            "moving base changed idle character offset: {carried_before} -> {carried_after}");

        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(character).unwrap().0 = Vec3::X * 4.75;
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::X;
        let offset_before = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x;
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app.update();
        let offset_after = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x;
        let relative_motion = offset_after - offset_before;
        let base = app.world().get::<als_core::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
        let velocity = app.world().get::<avian3d::prelude::LinearVelocity>(character).unwrap().0;
        let expected = (velocity.x - base.support_velocity.x) * 0.1;
        assert!((relative_motion - expected).abs() < 1e-3,
            "full-frame movement relative to moving base: {relative_motion} != {expected}");

        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(character).unwrap().0 = Vec3::X;
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::ZERO;
        app.update();
        let stopped_offset = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x;
        assert!((stopped_offset - offset_after).abs() < 1e-3,
            "moving base changed stopped character offset: {offset_after} -> {stopped_offset}");

        app.world_mut().entity_mut(platform).insert((
            RigidBody::Dynamic,
            avian3d::prelude::GravityScale(0.0),
            avian3d::prelude::LockedAxes::ROTATION_LOCKED,
        ));
        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(platform).unwrap().0 = Vec3::X;
        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(character).unwrap().0 = Vec3::X;
        app.update();
        let dynamic_offset = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x;
        assert!((dynamic_offset - stopped_offset).abs() < 1e-2,
            "simulated moving base changed idle character offset: {stopped_offset} -> {dynamic_offset}");
        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(character).unwrap().0 = Vec3::X * 4.75;
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::X;
        app.update();
        let dynamic_moved = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x - dynamic_offset;
        let dynamic_velocity = app.world().get::<avian3d::prelude::LinearVelocity>(character).unwrap().x
            - app.world().get::<avian3d::prelude::LinearVelocity>(platform).unwrap().x;
        assert!((dynamic_moved - dynamic_velocity * 0.1).abs() < 1e-2,
            "full-frame movement relative to simulated base: {dynamic_moved} != {}", dynamic_velocity * 0.1);
    }
}
