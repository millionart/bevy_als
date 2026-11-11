use std::time::Duration;

use crate::als::{character::AlsCharacter, state::{AlsNotifyLocomotionAction, LocomotionState}};
use avian3d::dynamics::{integrator::{IntegrationSystems, VelocityIntegrationData}, solver::solver_body::SolverBody};
use avian3d::dynamics::solver::{PhysicsLengthUnit, SolverConfig};
use avian3d::prelude::{
    Collider, Collisions, ContactPairFlags, CustomPositionIntegration, Physics, PhysicsSchedule,
    PhysicsSystems, Position, RigidBody, Rotation, ShapeCastConfig, SolverSystems, SpatialQueryFilter,
    SubstepCount, SubstepSchedule,
};
use crate::extensions::RagdollSpatialQuery as SpatialQuery;
use bevy::prelude::*;
use bevy_animation_graph::core::plugin::AnimationGraphSet;

pub const SOURCE_MAX_PHYSICS_DELTA: Duration = Duration::from_nanos(33_333_333);
const SOURCE_PHYSICS_SUBSTEPS: u32 = include!("../../settings/physics_substeps.rs");
const SOURCE_RESTITUTION_ACCELERATION: f32 = include!("../../settings/restitution_acceleration.rs");
const SOURCE_CONTACT_PUSHOUT_SPEED: f32 = include!("../../settings/contact_pushout_speed.rs");
const SOURCE_WORLD_SOLVER_ITERATIONS: [u32; 3] = include!("../../settings/world_solver_iterations.rs");

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourcePhysicsTimingSet {
    Clamp,
    Restore,
}

#[derive(Resource, Default)]
struct SourcePhysicsClockRestore {
    previous: Option<Time>,
}

#[derive(Component, Default)]
struct SourceCharacterFullFrame {
    velocity_before_step: Vec3,
    landed: bool,
}

pub struct SourcePhysicsTimingPlugin;

impl Plugin for SourcePhysicsTimingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SubstepCount(SOURCE_PHYSICS_SUBSTEPS))
            .init_resource::<SourcePhysicsClockRestore>()
            .configure_sets(
                PostUpdate,
                (
                    SourcePhysicsTimingSet::Clamp
                        .after(AnimationGraphSet::PrePhysics)
                        .before(PhysicsSystems::First),
                    SourcePhysicsTimingSet::Restore
                        .after(PhysicsSystems::Last)
                        .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs)
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
                    .after(avian3d::dynamics::integrator::pre_process_velocity_increments)
                    .after(crate::extensions::ragdoll_physics_profile::apply_source_linear_damping),
            )
            .add_systems(PhysicsSchedule, preserve_unobstructed_grounded_displacement
                .in_set(SolverSystems::PostSubstep)
                .after(crate::extensions::source_physics_joint::project_source_joints))
            .add_systems(PhysicsSchedule, update_source_contact_limits.in_set(SolverSystems::PreSubstep))
            .add_systems(
                SubstepSchedule,
                record_character_velocity
                    .before(IntegrationSystems::Velocity),
            )
            .add_systems(
                SubstepSchedule,
                integrate_character_position
                    .in_set(IntegrationSystems::Position)
                    .after(avian3d::dynamics::integrator::integrate_positions)
                    .after(crate::extensions::ragdoll_physics_profile::integrate_source_ragdoll_rotation),
            );
    }
}

fn update_source_contact_limits(
    time: Res<Time<Physics>>,
    length_unit: Res<PhysicsLengthUnit>,
    mut solver: ResMut<SolverConfig>,
) {
    solver.restitution_threshold = (SOURCE_RESTITUTION_ACCELERATION * time.delta_secs() / length_unit.0).next_up();
    solver.max_overlap_solve_speed = SOURCE_CONTACT_PUSHOUT_SPEED / length_unit.0;
    solver.restitution_iterations = SOURCE_WORLD_SOLVER_ITERATIONS[1] as usize;
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
        Option<&crate::als::dynamics::AlsMovementBaseDynamicsState>, &crate::als::character::AlsCmcAdapterRuntime,
        Option<&crate::als::roll::AlsRollState>),
        (With<AlsCharacter>, Without<CustomPositionIntegration>)>,
) {
    for (entity, locomotion, body, action, base, runtime, roll) in &characters {
        if frame_time.delta() <= SOURCE_MAX_PHYSICS_DELTA
            && !(*locomotion == LocomotionState::Grounded && runtime.simulation_substeps > 1
                && runtime.grounded_movement_delta.is_some() && runtime.grounded_move_start.is_none())
            && !(*locomotion == LocomotionState::Air && roll.is_some_and(crate::als::roll::AlsRollState::root_motion_active)) {
            continue;
        }
        if base.is_some_and(|base| base.entity.is_some() && base.support_velocity.length_squared() > 0.0) {
            continue;
        }
        if *body == RigidBody::Dynamic && action != Some(&AlsNotifyLocomotionAction::Ragdolling)
            && matches!(*locomotion, LocomotionState::Grounded | LocomotionState::Air) {
            commands.entity(entity).insert((CustomPositionIntegration, SourceCharacterFullFrame::default()));
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

fn record_character_velocity(mut characters: Query<(&SolverBody, &mut SourceCharacterFullFrame)>) {
    for (body, mut frame) in &mut characters {
        frame.velocity_before_step = body.linear_velocity;
    }
}

fn preserve_unobstructed_grounded_displacement(
    frame_time: Res<Time<Virtual>>,
    physics_time: Res<Time<Physics>>,
    substeps: Res<SubstepCount>,
    collisions: Collisions,
    mut characters: Query<(Entity, &mut SolverBody, &RigidBody, &LocomotionState,
        &crate::als::character::AlsCmcAdapterRuntime, &crate::als::dynamics::AlsMovementBaseDynamicsState,
        Has<SourceCharacterFullFrame>, Has<CustomPositionIntegration>), With<AlsCharacter>>,
) {
    if substeps.0 == 0 { return; }
    for (entity, mut body, rigid_body, locomotion, runtime, base, full_frame, custom) in &mut characters {
        let Some(planned) = base.pre_collision_velocity else { continue; };
        let Some(delta) = runtime.grounded_movement_delta else { continue; };
        if *rigid_body != RigidBody::Dynamic || *locomotion != LocomotionState::Grounded
            || full_frame != custom || runtime.grounded_move_start.is_none()
            || runtime.grounded_collision_velocity.is_some() || runtime.simulation_substeps != 1
            || runtime.simulation_delta_seconds != frame_time.delta_secs()
            || runtime.simulation_delta_seconds != physics_time.delta_secs()
            || base.support_velocity != Vec3::ZERO || body.delta_rotation != Rotation::IDENTITY
            || body.linear_velocity.x != planned.x || body.linear_velocity.z != planned.z
            || delta.with_y(0.0) != (planned * runtime.simulation_delta_seconds).with_y(0.0)
            || collisions.collisions_with(entity).any(|pair| pair.flags.contains(ContactPairFlags::GENERATE_CONSTRAINTS)) {
            continue;
        }
        let step = if full_frame {
            frame_time.delta_secs() / substeps.0 as f32
        } else {
            physics_time.delta().div_f64(f64::from(substeps.0)).as_secs_f32()
        };
        let mut accumulated = Vec3::ZERO;
        for _ in 0..substeps.0 { accumulated += planned * step; }
        if body.delta_position.x == accumulated.x && body.delta_position.z == accumulated.z {
            body.delta_position.x = delta.x;
            body.delta_position.z = delta.z;
        }
    }
}

fn integrate_character_position(
    frame_time: Res<Time<Virtual>>,
    physics_time: Res<Time<Physics>>,
    substeps: Res<SubstepCount>,
    spatial: SpatialQuery,
    impact_bodies: crate::als::motion::FallingImpactBodies,
    params_assets: Option<Res<Assets<crate::als::params::AlsMovementParams>>>,
    step_permissions: Query<&crate::als::mantle::AlsMantleTargetPermission>,
    candidates: Query<crate::als::motion::CharacterCollisionCandidates>,
    shapes: Query<(&Collider, &Position, &Rotation)>,
    mut characters: Query<(Entity, &mut SolverBody, &Position, &Collider, &LocomotionState,
        Option<&AlsNotifyLocomotionAction>, &RigidBody, Option<&crate::als::dynamics::AlsMovementBaseDynamicsState>,
        Option<&mut SourceCharacterFullFrame>, Has<CustomPositionIntegration>,
        &mut crate::als::character::AlsCmcAdapterRuntime,
        Option<&crate::als::character::AlsMovementParamsHandle>), With<AlsCharacter>>,
) {
    let frame_step = frame_time.delta_secs() / substeps.0 as f32;
    let extra_step = (frame_time.delta_secs() - physics_time.delta_secs()) / substeps.0 as f32;
    for (entity, mut body, position, collider, locomotion, action, rigid_body, base, mut full_frame, custom_position, mut runtime, handle) in &mut characters {
        let movement = if let Some(frame) = full_frame.as_deref() {
            if *locomotion == LocomotionState::Grounded && runtime.grounded_move_start.is_none()
                && let Some(delta) = runtime.grounded_movement_delta {
                delta / substeps.0 as f32 + Vec3::Y * body.linear_velocity.y * frame_step
            } else if *locomotion == LocomotionState::Air {
                (frame.velocity_before_step + body.linear_velocity) * 0.5 * frame_step
            } else { body.linear_velocity * frame_step }
        } else if !custom_position && *rigid_body == RigidBody::Dynamic
            && *locomotion == LocomotionState::Grounded
            && action != Some(&AlsNotifyLocomotionAction::Ragdolling)
            && base.is_some_and(|base| base.entity.is_some() && base.support_velocity.length_squared() > 0.0) {
            let relative_velocity = crate::als::dynamics::locomotion_velocity(body.linear_velocity, *locomotion, base);
            if runtime.grounded_move_start.is_none() && let Some(delta) = runtime.grounded_movement_delta {
                (delta - base.unwrap().support_velocity * frame_time.delta_secs()
                    - relative_velocity * physics_time.delta_secs()).with_y(0.0) / substeps.0 as f32
                    + Vec3::Y * relative_velocity.y * extra_step
            } else { relative_velocity * extra_step }
        } else {
            continue;
        };
        let start = position.0 + body.delta_position;
        if *locomotion == LocomotionState::Air {
            let params = params_assets.as_ref().and_then(|assets| handle.and_then(|handle| assets.get(handle.id())));
            let root_landing = runtime.has_animation_root_velocity && full_frame.is_some() && params.is_some();
            let mut walking_time = full_frame.as_deref().filter(|frame| frame.landed).map(|_| frame_step);
            if walking_time.is_none() {
                let max_speed = runtime.grounded_max_speed;
                let landing = if root_landing {
                    let params = params.unwrap();
                    Some((&params.cmc, params.jump.jump_z_velocity, max_speed, &mut runtime.random_seed))
                } else { None };
                let (applied, velocity, contact, remaining, _) = crate::als::motion::falling_movement_sweep(&spatial, collider,
                    start, Quat::IDENTITY, movement, body.linear_velocity, frame_step, None, landing, entity, &candidates, &shapes, &impact_bodies, None);
                body.delta_position += applied;
                body.linear_velocity = velocity;
                runtime.airborne_sweep_contact |= contact;
                if root_landing && let Some((remaining, last_impact)) = remaining {
                    full_frame.as_deref_mut().unwrap().landed = true;
                    walking_time = Some(if last_impact { 0.0 } else { remaining });
                }
            }
            if let (Some(walking_time), Some(params)) = (walking_time, params) {
                body.linear_velocity.y = 0.0;
                let mut transform = Transform::from_translation(position.0 + body.delta_position);
                let extent = collider.shape_scaled().compute_local_aabb().half_extents();
                crate::als::motion::adjust_walking_floor_height(&spatial, collider, &mut transform,
                    extent.y, extent.x.min(extent.z), &params.cmc, entity, &candidates);
                let requested = body.linear_velocity * walking_time;
                if let Some((advanced, remaining, next_y, _, stepped_up, _)) = crate::als::motion::grounded_movement_transaction(
                    &spatial, collider, transform.translation, Quat::IDENTITY, extent.y, extent.x.min(extent.z),
                    requested, &params.cmc, LocomotionState::Grounded, entity, &candidates, &step_permissions, None) {
                    let previous_y = transform.translation.y;
                    transform.translation = advanced + remaining.unwrap_or(requested);
                    if let Some(next_y) = next_y && (stepped_up || next_y > previous_y) { transform.translation.y = next_y; }
                }
                let floor = crate::als::motion::adjust_walking_floor_height(&spatial, collider, &mut transform,
                    extent.y, extent.x.min(extent.z), &params.cmc, entity, &candidates);
                full_frame.as_deref_mut().unwrap().landed = floor.is_some();
                body.delta_position = transform.translation - position.0;
            }
            continue;
        }
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let accepts = |candidate| crate::als::motion::character_blocks_collider(entity, candidate, &candidates);
        let mut remaining = movement;
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
            let travel = crate::als::motion::movement_sweep_travel(distance, hit.distance);
            applied += *direction * travel;
            let Some(normal) = hit.normal1.try_normalize() else { break; };
            let mut slide = remaining - *direction * travel;
            slide -= normal * slide.dot(normal).min(0.0);
            let inward_velocity = body.linear_velocity.dot(normal).min(0.0);
            body.linear_velocity -= normal * inward_velocity;
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
    fn source_restitution_threshold_follows_clamped_physics_time() {
        use avian3d::dynamics::solver::{PhysicsLengthUnit, SolverConfig};
        let (mut app, _, _) = crate::roll_drive::tests::loaded_roll_character();
        for unit in [1.0_f32, 2.0] {
            app.world_mut().insert_resource(PhysicsLengthUnit(unit));
            for frame_dt in [1.0_f32 / 120.0, 1.0 / 60.0, 1.0 / 30.0, 0.1] {
                app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(frame_dt)));
                app.update();
                let physics_dt = app.world().resource::<Time<Physics>>().delta_secs();
                let threshold = 10.0 * physics_dt;
                let actual = app.world().resource::<SolverConfig>().restitution_threshold * unit;
                assert_eq!(app.world().resource::<SolverConfig>().max_overlap_solve_speed * unit, 10.0);
                assert_eq!(actual, threshold.next_up(), "unit={unit} frame_dt={frame_dt}");
                for speed in [threshold.next_down(), threshold, threshold.next_up()] {
                    assert_eq!(speed >= actual, speed > threshold,
                        "unit={unit} frame_dt={frame_dt} speed={speed} threshold={threshold}");
                }
                assert!((physics_dt - frame_dt.min(SOURCE_MAX_PHYSICS_DELTA.as_secs_f32())).abs() < 1e-6);
                assert!((app.world().resource::<Time<Virtual>>().delta_secs() - frame_dt).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn world_ragdoll_uses_one_clamped_physics_step_without_fixing_game_frames() {
        use avian3d::prelude::{AngularDamping, AngularVelocity, ColliderDisabled, Gravity, JointDisabled, LinearDamping, Sleeping, SleepingDisabled};
        use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
        #[derive(Resource, Default)]
        struct ObservedSteps { count: u32, delta: f32, physics: f32 }
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
        for _ in 0..12 { app.update(); }
        let spawned = app.world().get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap();
        let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
        let joints = spawned.joints.values().copied().collect::<Vec<_>>();
        for entity in colliders { app.world_mut().entity_mut(entity).insert(ColliderDisabled); }
        for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
        let body = bodies.iter().find(|(label, _)| label == "spine_01").unwrap().1;
        app.world_mut().entity_mut(body).insert((SleepingDisabled, LinearDamping(0.0), AngularDamping(0.0))).remove::<Sleeping>();
        app.init_resource::<ObservedSteps>();
        app.add_systems(PhysicsSchedule, (|time: Res<Time<Physics>>, mut observed: ResMut<ObservedSteps>| {
            observed.physics = time.delta_secs();
        }).in_set(SolverSystems::PreSubstep));
        app.add_systems(SubstepSchedule, |time: Res<Time>, mut observed: ResMut<ObservedSteps>| {
            observed.count += 1;
            observed.delta = time.delta_secs();
        });
        assert_eq!(app.world().resource::<SubstepCount>().0, 1);
        for frame_dt in [1.0_f32 / 60.0, 1.0 / 30.0, 0.1] {
            for gravity in [Vec3::ZERO, Vec3::NEG_Y * 9.81] {
                app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(frame_dt)));
                app.world_mut().insert_resource(Gravity(gravity));
                app.world_mut().resource_mut::<ObservedSteps>().count = 0;
                app.world_mut().entity_mut(body).insert((LinearVelocity(Vec3::X), AngularVelocity::ZERO)).remove::<Sleeping>();
                let before = app.world().get::<Position>(body).unwrap().0;
                app.update();
                let observed = app.world().resource::<ObservedSteps>();
                let dt = frame_dt.min(SOURCE_MAX_PHYSICS_DELTA.as_secs_f32());
                assert_eq!(observed.count, 1);
                assert!((observed.delta - dt).abs() < 1e-6 && (observed.physics - dt).abs() < 1e-6);
                assert!((app.world().resource::<Time<Virtual>>().delta_secs() - frame_dt).abs() < 1e-6);
                let velocity = Vec3::X + gravity * dt;
                assert!(app.world().get::<LinearVelocity>(body).unwrap().0.abs_diff_eq(velocity, 1e-5));
                let displacement = app.world().get::<Position>(body).unwrap().0 - before;
                assert!(displacement.abs_diff_eq(velocity * dt, 2e-5),
                    "frame_dt={frame_dt} gravity={gravity:?} displacement={displacement:?}");
            }
        }
    }

    #[test]
    fn short_airborne_roll_enables_source_midpoint_integration() {
        use bevy::ecs::system::RunSystemOnce;
        for (mode, active, delta, expected) in [(LocomotionState::Air, true, 1.0 / 60.0, true),
            (LocomotionState::Air, false, 1.0 / 60.0, false),
            (LocomotionState::Grounded, true, 1.0 / 60.0, false),
            (LocomotionState::Air, false, 0.05, true)] {
            let mut world = World::new();
            let mut time = Time::<Virtual>::default();
            time.advance_by(std::time::Duration::from_secs_f32(delta));
            world.insert_resource(time);
            let actor = world.spawn((AlsCharacter, mode, RigidBody::Dynamic,
                crate::als::roll::AlsRollState { active, time: 0.1, ..default() })).id();
            world.run_system_once(enable_character_full_frame).unwrap();
            assert_eq!(world.get::<CustomPositionIntegration>(actor).is_some(), expected,
                "mode={mode:?} active={active} delta={delta}");
        }
    }

    #[test]
    fn free_grounded_plan_survives_six_physics_substeps() {
        use avian3d::prelude::{ContactEdge, ContactGraph, ContactPairFlags};
        use bevy::ecs::system::RunSystemOnce;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins).init_resource::<Time<Physics>>()
            .init_resource::<ContactGraph>().insert_resource(SubstepCount(6));
        let duration = Duration::from_secs_f32(1.0 / 30.0);
        app.world_mut().resource_mut::<Time<Virtual>>().advance_by(duration);
        app.world_mut().resource_mut::<Time<Physics>>().advance_by(duration);
        let dt = app.world().resource::<Time<Physics>>().delta_secs();
        let planned = Vec3::X * 0.666_666_7;
        let delta = Vec3::X * 0.022_222_225;
        let runtime = crate::als::character::AlsCmcAdapterRuntime {
            simulation_substeps: 1, simulation_delta_seconds: dt,
            grounded_move_start: Some(Vec3::ZERO), grounded_movement_delta: Some(delta), ..default()
        };
        let base = crate::als::dynamics::AlsMovementBaseDynamicsState { pre_collision_velocity: Some(planned), ..default() };
        let mut accumulated = Vec3::ZERO;
        for _ in 0..6 { accumulated += planned * (dt / 6.0); }
        assert_ne!(accumulated.x, delta.x);
        let mut body = SolverBody::DUMMY;
        body.linear_velocity = planned;
        body.delta_position = accumulated + Vec3::Y;
        let custom = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, LocomotionState::Grounded,
            body.clone(), runtime, base, CustomPositionIntegration, SourceCharacterFullFrame::default())).id();
        let ordinary = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, LocomotionState::Grounded,
            body.clone(), runtime, base)).id();
        let mut blocked_body = body.clone();
        blocked_body.delta_position.x *= 0.5;
        let blocked = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, LocomotionState::Grounded,
            blocked_body.clone(), runtime, base)).id();
        let moving = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, LocomotionState::Grounded,
            body.clone(), runtime, crate::als::dynamics::AlsMovementBaseDynamicsState {
                entity: Some(Entity::PLACEHOLDER), support_velocity: Vec3::X, ..base
            })).id();
        let other_custom = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, LocomotionState::Grounded,
            body.clone(), runtime, base, CustomPositionIntegration)).id();
        let touching = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, LocomotionState::Grounded,
            body.clone(), runtime, base)).id();
        let obstacle = app.world_mut().spawn_empty().id();
        app.world_mut().resource_mut::<ContactGraph>().add_edge_with(ContactEdge::new(touching, obstacle),
            |pair| pair.flags.insert(ContactPairFlags::TOUCHING | ContactPairFlags::GENERATE_CONSTRAINTS)).unwrap();
        let sensor_touching = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, LocomotionState::Grounded,
            body.clone(), runtime, base)).id();
        let sensor = app.world_mut().spawn_empty().id();
        app.world_mut().resource_mut::<ContactGraph>().add_edge_with(ContactEdge::new(sensor_touching, sensor),
            |pair| pair.flags.insert(ContactPairFlags::TOUCHING)).unwrap();
        let mut rotated_body = body.clone();
        rotated_body.delta_rotation = Rotation(Quat::from_rotation_y(0.1));
        let rotated = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, LocomotionState::Grounded,
            rotated_body, runtime, base)).id();
        app.world_mut().run_system_once(preserve_unobstructed_grounded_displacement).unwrap();
        for actor in [custom, ordinary, sensor_touching] {
            assert_eq!(app.world().get::<SolverBody>(actor).unwrap().delta_position, delta + Vec3::Y);
        }
        assert_eq!(app.world().get::<SolverBody>(blocked).unwrap().delta_position, blocked_body.delta_position);
        assert_eq!(app.world().get::<SolverBody>(moving).unwrap().delta_position, body.delta_position);
        assert_eq!(app.world().get::<SolverBody>(other_custom).unwrap().delta_position, body.delta_position);
        assert_eq!(app.world().get::<SolverBody>(touching).unwrap().delta_position, body.delta_position);
        assert_eq!(app.world().get::<SolverBody>(rotated).unwrap().delta_position, body.delta_position);
    }

    #[test]
    fn spawn_first_physics_step_uses_initial_floor_mode() {
        use crate::als::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use crate::als::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
        use crate::als::motion::{AlsMotionPlugin, initial_als_control_scheme_config};
        use crate::als::params::AlsMovementParams;
        use crate::als::transition::AlsStateTransitionPlugin;
        use bevy::asset::AssetPlugin;
        use bevy_tnua::TnuaControllerPlugin;
        use bevy_tnua_avian3d::TnuaAvian3dPlugin;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            TnuaAvian3dPlugin::new(Update), TnuaControllerPlugin::<AlsControlScheme>::new(Update),
            AlsStateTransitionPlugin, AlsCharacterPlugin, AlsMotionPlugin, SourcePhysicsTimingPlugin))
            .init_asset::<Mesh>().add_message::<bevy::asset::AssetEvent<Mesh>>()
            .insert_resource(SubstepCount(6))
            .insert_resource(avian3d::prelude::Gravity(Vec3::NEG_Y * 9.81))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 30.0)));
        app.finish();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(100.0, 0.2, 100.0),
            Transform::from_xyz(0.0, -0.1, 0.0)));
        app.update();
        app.update();
        let values = AlsMovementParams::default();
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(values.clone());
        let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>()
            .add(initial_als_control_scheme_config(&values));
        let mut supported_bundle = AlsCharacterBundle::new(params.clone(), config.clone());
        supported_bundle.transform = Transform::from_xyz(-2.0, 0.9215, 0.0);
        let supported = app.world_mut().spawn(supported_bundle).id();
        let mut bundle = AlsCharacterBundle::new(params, config);
        bundle.transform = Transform::from_xyz(2.0, 1.5, 0.0);
        let actor = app.world_mut().spawn(bundle).id();
        app.update();
        assert_eq!(*app.world().get::<LocomotionState>(supported).unwrap(), LocomotionState::Grounded);
        assert!((app.world().get::<Position>(supported).unwrap().y - 0.9215).abs() < 1e-4);
        assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), LocomotionState::Air);
        assert!((app.world().get::<Position>(actor).unwrap().y - 1.494_55).abs() < 1e-6);
        assert!((app.world().get::<LinearVelocity>(actor).unwrap().y + 0.327).abs() < 1e-6);
    }

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
                crate::als::character::AlsCharacter,
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
    fn accepted_moving_base_jump_integrates_the_entire_takeoff_frame() {
        use crate::als::character::{AlsCharacterBundle, AlsCharacterPlugin, AlsEffectiveGravity};
        use crate::als::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
        use crate::als::motion::{AlsMotionPlugin, initial_als_control_scheme_config};
        use crate::als::params::AlsMovementParams;
        use crate::als::transition::AlsStateTransitionPlugin;
        use bevy_tnua::TnuaControllerPlugin;
        use bevy_tnua_avian3d::TnuaAvian3dPlugin;
        #[derive(Resource, Default)]
        struct PostMotorSample(Option<(Vec3, Vec3, Vec3, bool, LocomotionState, Option<Entity>)>);
        for delta_ms in [100_u64, 16, 33, 50] {
            println!("Takeoff frame progress: delta={delta_ms} ms");
            for substeps in [1, 2, 6] {
                for (support_body, support_speed, takeoff_speed) in [(RigidBody::Kinematic, 2.0_f32, 2.0_f32),
                    (RigidBody::Kinematic, 2.0, -2.0), (RigidBody::Kinematic, 0.0, 0.0), (RigidBody::Static, 0.0, 0.0)] {
                    for changed in [false, true] {
                        for (gravity_strength, horizontal_input) in [(9.8, Vec2::ZERO), (9.8, Vec2::Y * 0.4),
                            (19.6, Vec2::ZERO), (19.6, Vec2::Y * 0.4)] {
                        let mut app = App::new();
                        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
                            TnuaAvian3dPlugin::new(Update), TnuaControllerPlugin::<AlsControlScheme>::new(Update),
                            AlsStateTransitionPlugin, AlsCharacterPlugin, AlsMotionPlugin, SourcePhysicsTimingPlugin))
                            .init_asset::<Mesh>().add_message::<bevy::asset::AssetEvent<Mesh>>()
                            .init_resource::<PostMotorSample>().insert_resource(SubstepCount(substeps))
                            .insert_resource(avian3d::prelude::Gravity(Vec3::NEG_Y * gravity_strength))
                            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)))
                            .add_systems(Update, (|mut sample: ResMut<PostMotorSample>,
                                characters: Query<(&LinearVelocity, &bevy_tnua::TnuaMotor, &Position, &AlsEffectiveGravity, Ref<Collider>, &bevy_tnua::TnuaController<AlsControlScheme>, &LocomotionState, Option<&crate::als::dynamics::AlsMovementBaseDynamicsState>)>| {
                                for (velocity, motor, position, gravity, collider, controller, mode, base) in &characters {
                                    if controller.action_flow_status().just_starting() == Some(crate::als::control_scheme::AlsControlSchemeActionDiscriminant::Jump) {
                                        sample.0 = Some((velocity.0 + motor.lin.boost, position.0, gravity.value, collider.is_changed(), *mode, base.and_then(|base| base.entity)));
                                    }
                                }
                            }).after(crate::als::dynamics::grounded_dynamics_motor_system)
                                .before(crate::als::motion::cmc_movement_transaction_system)
                                .before(bevy_tnua::TnuaPipelineSystems::Motors));
                        app.finish(); app.update();
                        let value = AlsMovementParams::default();
                        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(value.clone());
                        let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>().add(initial_als_control_scheme_config(&value));
                        let mut bundle = AlsCharacterBundle::new(params, config);
                        bundle.transform.translation = Vec3::new(0.0, 3.174, 0.0);
                        let character = app.world_mut().spawn(bundle).id();
                        let platform = app.world_mut().spawn((support_body, Collider::cuboid(8.0, 0.5, 8.0),
                            Transform::from_xyz(0.0, 2.0, 0.0), LinearVelocity(Vec3::X * support_speed))).id();
                        let ordinary = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(0.2, 0.2, 0.2),
                            Transform::from_xyz(100.0, 100.0, 0.0))).id();
                        for _ in 0..60 { app.update(); }
                        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Grounded);
                        assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character).unwrap().support_velocity.x, support_speed);
                        app.world_mut().get_mut::<LinearVelocity>(platform).unwrap().0 = Vec3::X * takeoff_speed;
                        if changed { *app.world_mut().get_mut::<Collider>(character).unwrap() = Collider::capsule(0.29, 1.22); }
                        *app.world_mut().resource_mut::<TimeUpdateStrategy>() = TimeUpdateStrategy::ManualDuration(Duration::from_millis(delta_ms));
                        {
                            let mut input = app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap();
                            input.jump_pressed = true;
                            input.move_dir = horizontal_input;
                        }
                        let ordinary_velocity = app.world().get::<LinearVelocity>(ordinary).unwrap().y;
                        let platform_start = app.world().get::<Position>(platform).unwrap().0;
                        app.update();
                        let (before, position, gravity, collider_changed, mode, base) = app.world().resource::<PostMotorSample>().0.expect("The real jump must start during the sampled frame");
                        assert_eq!(collider_changed, changed);
                        if takeoff_speed != support_speed && horizontal_input == Vec2::ZERO {
                            assert!((before.x - takeoff_speed).abs() < 0.0001,
                                "Accepted jump lost the current base velocity before clearing: {before:?}, expected={takeoff_speed}");
                        }
                        let delta = delta_ms as f32 / 1000.0;
                        let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
                        let acceleration = crate::als::motion::air_acceleration_for_horizontal_speed(&value, before.with_y(0.0).length())
                            .min(value.acceleration.max_acceleration);
                        let input = Vec3::new(horizontal_input.x, 0.0, -horizontal_input.y);
                        let max_speed = crate::als::dynamics::max_allowed_speed(&value,
                            crate::als::state::Stance::Standing, crate::als::state::RotationMode::ViewDirection,
                            crate::als::state::Gait::Running, before.with_y(0.0), 0.0);
                        let target_speed = (max_speed * input.length()).max(value.cmc.min_analog_walk_speed);
                        let mut horizontal = before.with_y(0.0);
                        let mut horizontal_movement = Vec3::ZERO;
                        let mut remaining = delta;
                        let mut iteration = 0;
                        while remaining >= crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS
                            && iteration < value.cmc.max_simulation_iterations {
                            iteration += 1;
                            let step = if remaining > value.cmc.max_simulation_time_step
                                && iteration < value.cmc.max_simulation_iterations {
                                value.cmc.max_simulation_time_step.min(remaining * 0.5)
                            } else { remaining };
                            let old_horizontal = horizontal;
                            if input != Vec3::ZERO {
                                let limit = if horizontal.length_squared() > target_speed.powi(2) * 1.01 {
                                    horizontal.length()
                                } else { target_speed };
                                horizontal = (horizontal + input * acceleration * step).clamp_length_max(limit);
                            }
                            horizontal_movement += (old_horizontal + horizontal) * (0.5 * step);
                            remaining -= step;
                        }
                        let expected = horizontal + Vec3::Y * before.y + gravity * delta;
                        assert!(velocity.distance(expected) < 0.0001,
                            "delta={delta_ms} substeps={substeps} support={support_speed} changed={changed}: post_motor={before:?} actual={velocity:?} expected={expected:?}");
                        assert_eq!(mode, LocomotionState::Air);
                        assert_eq!(base, None);
                        let physics_delta = delta.min(SOURCE_MAX_PHYSICS_DELTA.as_secs_f32());
                        assert!((app.world().get::<LinearVelocity>(ordinary).unwrap().y - ordinary_velocity + gravity_strength * physics_delta).abs() < 0.0001);
                        assert!((app.world().get::<Position>(platform).unwrap().x - platform_start.x - takeoff_speed * physics_delta).abs() < 0.0001);
                        assert!(((app.world().get::<Position>(character).unwrap().0 - position).with_y(0.0)
                            - horizontal_movement).length() < 0.0001);
                        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn ordinary_free_fall_respects_the_source_terminal_component() {
        for gravity in [Vec3::NEG_Y * 9.8, Vec3::NEG_X * 5.0, Vec3::ZERO] {
            for initial in [Vec3::new(30.0, -39.95, 0.0), Vec3::new(100.0, -20.0, 3.0),
                Vec3::new(1.0, 45.0, 2.0), Vec3::new(-60.0, -60.0, 7.0)] {
                for delta in [1.0 / 120.0, 1.0 / 60.0, 0.1] {
                    for (default_limit, actor_limit) in [(40.0, None), (25.0, None),
                        (25.0, Some(10.0)), (40.0, Some(-20.0))] {
                        let mut app = App::new();
                        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                            avian3d::prelude::PhysicsPlugins::new(PostUpdate), SourcePhysicsTimingPlugin,
                            crate::als::character::AlsCharacterPlugin))
                            .init_asset::<Mesh>().add_message::<bevy::asset::AssetEvent<Mesh>>()
                            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(delta)))
                            .insert_resource(avian3d::prelude::Gravity(gravity))
                            .insert_resource(crate::als::dynamics::AlsDefaultPhysicsVolume(
                                crate::als::dynamics::AlsPhysicsVolume { terminal_velocity: default_limit }));
                        app.finish();
                        app.update();
                        let character = app.world_mut().spawn((AlsCharacter, LocomotionState::Air, RigidBody::Dynamic,
                            Collider::capsule(0.3, 1.2), Position(Vec3::Y * 100.0), Transform::from_xyz(0.0, 100.0, 0.0),
                            LinearVelocity(initial))).id();
                        if let Some(limit) = actor_limit {
                            app.world_mut().entity_mut(character).insert(crate::als::dynamics::AlsPhysicsVolume { terminal_velocity: limit });
                        }
                        app.update();
                        let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
                        let mut expected = initial + gravity * delta;
                        let limit = actor_limit.unwrap_or(default_limit).abs();
                        if let Some(direction) = gravity.try_normalize() {
                            let falling = expected.dot(direction);
                            if falling > limit { expected += direction * (limit - falling); }
                        }
                        assert!(velocity.distance(expected) < 1e-4,
                            "gravity={gravity:?} initial={initial:?} delta={delta} default={default_limit} actor={actor_limit:?}: {velocity:?} != {expected:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn terminal_volume_preserves_root_motion_and_scaled_gravity() {
        for root_motion in [false, true] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate), SourcePhysicsTimingPlugin,
                crate::als::character::AlsCharacterPlugin))
                .init_asset::<Mesh>().add_message::<bevy::asset::AssetEvent<Mesh>>()
                .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)))
                .insert_resource(avian3d::prelude::Gravity(Vec3::NEG_Y * 9.8));
            app.finish();
            app.update();
            let character = app.world_mut().spawn((AlsCharacter, LocomotionState::Air, RigidBody::Dynamic,
                Collider::capsule(0.3, 1.2), Position(Vec3::Y * 1000.0), Transform::from_xyz(0.0, 1000.0, 0.0),
                LinearVelocity(Vec3::new(100.0, -60.0, 3.0)), avian3d::prelude::GravityScale(0.5),
                crate::als::dynamics::AlsPhysicsVolume { terminal_velocity: 20.0 })).id();
            if root_motion {
                app.world_mut().entity_mut(character).insert(crate::als::roll::AlsRollState { active: true, time: 0.0, ..default() });
            }
            app.update();
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            let expected = if root_motion { -60.0 - 4.9 / 60.0 } else { -20.0 };
            assert!((velocity.y - expected).abs() < 1e-4);
            assert_eq!(velocity.x, 100.0);
            assert_eq!(velocity.z, 3.0);
            app.world_mut().entity_mut(character).remove::<crate::als::roll::AlsRollState>();
            app.world_mut().get_mut::<LinearVelocity>(character).unwrap().0 = Vec3::ZERO;
            for _ in 0..300 { app.update(); }
            assert!((app.world().get::<LinearVelocity>(character).unwrap().y + 20.0).abs() < 1e-4);
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
                crate::als::character::AlsCharacter, mode, RigidBody::Dynamic,
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
            crate::als::character::AlsCharacter,
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
    fn browser_review_movement_full_frame_air_matches_source_midpoint_displacement() {
        for initial_y in [0.0, 5.0, -5.0] {
            let (position, velocity) = simulate_air_wall_motion(&[], 0.0, Vec3::Y * initial_y);
            let expected_position = 2.0 + initial_y * 0.1 - 0.5 * 9.81 * 0.1 * 0.1;
            assert!((position.y - expected_position).abs() < 1e-4,
                "Full-frame falling displacement differs: initial={initial_y}, {} != {expected_position}", position.y);
            assert!((velocity.y - (initial_y - 0.981)).abs() < 1e-4,
                "Full-frame gravity changed while correcting displacement");
        }
    }

    #[test]
    fn full_frame_midpoint_keeps_terminal_falling_displacement() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate), SourcePhysicsTimingPlugin))
            .init_asset::<Mesh>()
            .add_message::<bevy::asset::AssetEvent<Mesh>>()
            .insert_resource(SubstepCount(6))
            .insert_resource(crate::als::dynamics::AlsDefaultPhysicsVolume(crate::als::dynamics::AlsPhysicsVolume {
                terminal_velocity: 2.0, ..default()
            }))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)))
            .add_systems(SubstepSchedule, crate::als::dynamics::limit_falling_velocity
                .after(IntegrationSystems::Velocity)
                .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::WarmStart));
        app.finish();
        app.update();
        let character = app.world_mut().spawn((crate::als::character::AlsCharacter, LocomotionState::Air,
            RigidBody::Dynamic, Collider::capsule(0.3, 1.2), avian3d::prelude::LockedAxes::ROTATION_LOCKED,
            Position(Vec3::Y * 2.0), LinearVelocity(Vec3::NEG_Y * 2.0), Transform::from_xyz(0.0, 2.0, 0.0))).id();
        app.update();
        let position = app.world().get::<Position>(character).unwrap().0;
        assert!((position.y - 1.8).abs() < 1e-4, "Terminal falling displacement differs: {position:?}");
        assert!((app.world().get::<LinearVelocity>(character).unwrap().y + 2.0).abs() < 1e-4);
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
    fn browser_feedback_air_horizontal_acceleration_keeps_source_displacement() {
        use crate::als::input::AlsInputCommand;
        for input in [Vec2::ZERO, Vec2::X] {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            assert!(app.is_plugin_added::<SourcePhysicsTimingPlugin>());
            let start = Vec3::Y * 10.0;
            app.world_mut().entity_mut(character).insert((Position(start), Transform::from_translation(start),
                LinearVelocity(Vec3::new(1.0, -2.0, 0.0)), LocomotionState::Air,
                crate::als::transition::AlsStateRequest { locomotion_state: Some(LocomotionState::Air), ..default() },
                crate::als::dynamics::AlsMovementBaseDynamicsState::default(),
                AlsInputCommand { move_dir: input, ..default() }));
            app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(200)));
            app.update();
            let actual = app.world().get::<Position>(character).unwrap().0 - start;
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            let acceleration = if input == Vec2::ZERO { 0.0 } else { 3.0 };
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Air));
            assert!((velocity.x - (1.0 + acceleration * 0.2)).abs() < 1e-4,
                "Air fixture must retain the source final velocity: input={input:?} velocity={velocity:?}");
            let expected = 0.2 + 0.5 * acceleration * 0.2 * 0.2;
            assert!((actual.x - expected).abs() < 1e-4,
                "Air horizontal acceleration was applied before the source old velocity: input={input:?} actual={actual:?} expected_x={expected}");
        }
    }

    #[test]
    fn browser_feedback_air_wall_slide_keeps_full_gravity_and_remaining_movement() {
        for wall in [false, true] {
            let mut app = App::new();
            let delta = 0.046875_f32;
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate), SourcePhysicsTimingPlugin))
                .init_asset::<Mesh>().add_message::<bevy::asset::AssetEvent<Mesh>>()
                .insert_resource(avian3d::prelude::Gravity(Vec3::NEG_Y * 9.8))
                .insert_resource(SubstepCount(1))
                .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(delta)));
            app.finish(); app.update();
            let actor = app.world_mut().spawn((crate::als::character::AlsCharacter, LocomotionState::Air,
                RigidBody::Dynamic, Collider::capsule(0.3, 1.2),
                avian3d::prelude::SpeculativeMargin::MAX, avian3d::prelude::LockedAxes::ROTATION_LOCKED,
                avian3d::prelude::Friction::ZERO, Position(Vec3::Y * 2.0),
                LinearVelocity(Vec3::new(3.75, -2.0, 1.0)), Transform::from_xyz(0.0, 2.0, 0.0))).id();
            if wall {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.02, 20.0, 20.0),
                    Transform::from_xyz(0.46, 2.0, 0.0), avian3d::prelude::Friction::ZERO));
            }
            app.update();
            let position = app.world().get::<Position>(actor).unwrap().0;
            let velocity = app.world().get::<LinearVelocity>(actor).unwrap().0;
            let final_y = -2.0 - 9.8 * delta;
            let initial_movement = Vec3::new(3.75, (-2.0 + final_y) * 0.5, 1.0) * delta;
            let distance = initial_movement.length();
            let hit_fraction = if wall {
                crate::als::motion::movement_sweep_travel(distance, distance * 0.15 / initial_movement.x) / distance
            } else { 1.0 };
            let expected_y = 2.0 + (-2.0 + final_y) * 0.5 * delta * hit_fraction
                + final_y * delta * (1.0 - hit_fraction);
            assert!((position.x - initial_movement.x * hit_fraction).abs() < 1e-4
                && (position.z - delta).abs() < 1e-4,
                "Air wall fixture lost its source hit or tangent: wall={wall} position={position:?}");
            assert!((velocity.y - final_y).abs() < 1e-4 && (position.y - expected_y).abs() < 1e-4
                && (velocity.x - if wall { 0.0 } else { 3.75 }).abs() < 1e-4,
                "Air collision reused midpoint movement after impact: wall={wall} position={position:?} velocity={velocity:?} expected_y={expected_y} expected_velocity_y={final_y}");
        }
    }

    #[test]
    fn browser_feedback_air_source_steps_keep_caps_boost_analog_and_momentum() {
        use crate::als::{input::AlsInputCommand, state::{Gait, Stance, RotationMode}};
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let handle = app.world().get::<crate::als::character::AlsMovementParamsHandle>(actor).unwrap().0.clone();
        let settings = app.world().resource::<Assets<crate::als::params::AlsMovementParams>>().get(&handle).unwrap().clone();
        let limit = crate::als::dynamics::max_allowed_speed(&settings, Stance::Standing,
            RotationMode::ViewDirection, Gait::Running, Vec3::X, 0.0);
        assert!(limit > 1.6);
        for (label, initial, input, delta, iterations, expected_velocity, expected_delta) in [
            ("cap", limit - 0.05, Vec2::X, 0.2, 8, Vec3::X * limit, Vec3::X * (limit * 0.2 - 0.00125)),
            ("cap_single_step", limit - 0.05, Vec2::X, 0.2, 1, Vec3::X * limit, Vec3::X * (limit * 0.2 - 0.005)),
            ("overspeed", 5.0, Vec2::ZERO, 0.2, 8, Vec3::X * 5.0, Vec3::X),
            ("boost", 0.0, Vec2::X, 0.1, 8, Vec3::X * 0.6, Vec3::X * 0.03),
            ("analog", 0.5, Vec2::X * 0.4, 0.2, 8, Vec3::X * 0.74, Vec3::X * 0.124),
            ("reverse", 1.0, Vec2::NEG_X, 0.2, 8, Vec3::X * 0.4, Vec3::X * 0.14),
            ("side", 1.0, Vec2::Y, 0.2, 8, Vec3::new(1.0, 0.0, -0.6), Vec3::new(0.2, 0.0, -0.06)),
            ("short_frame", 1.0, Vec2::X, 0.02, 8, Vec3::X * 1.06, Vec3::X * 0.0206),
        ] {
            let mut params = settings.clone();
            params.cmc.max_simulation_iterations = iterations;
            let replacement = app.world_mut().resource_mut::<Assets<crate::als::params::AlsMovementParams>>().add(params);
            let start = Vec3::Y * 10.0;
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LinearVelocity(Vec3::new(initial, -2.0, 0.0)), LocomotionState::Air,
                crate::als::character::AlsMovementParamsHandle(replacement),
                crate::als::transition::AlsStateRequest { locomotion_state: Some(LocomotionState::Air), ..default() },
                crate::als::dynamics::AlsMovementBaseDynamicsState::default(),
                AlsInputCommand { move_dir: input, ..default() }));
            app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(delta)));
            app.update();
            let velocity = app.world().get::<LinearVelocity>(actor).unwrap().0.with_y(0.0);
            let actual = (app.world().get::<Position>(actor).unwrap().0 - start).with_y(0.0);
            assert!(velocity.distance(expected_velocity) < 1e-4 && actual.distance(expected_delta) < 1e-4,
                "Source falling step budget differs: case={label} velocity={velocity:?} position={actual:?} expected_velocity={expected_velocity:?} expected_position={expected_delta:?}");
        }
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
                    let source_deflected_y = free_velocity.y;
                    assert!(result.y > 0.0 && (result.y - source_deflected_y).abs() < 0.002
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
                let source_deflected_y = free_velocity.y;
                assert!((result.y - source_deflected_y).abs() < 0.002,
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
        use crate::als::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use crate::als::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
        use crate::als::motion::{AlsMotionPlugin, initial_als_control_scheme_config};
        use crate::als::params::AlsMovementParams;
        use crate::als::transition::AlsStateTransitionPlugin;
        use bevy_tnua::TnuaControllerPlugin;
        use bevy_tnua_avian3d::TnuaAvian3dPlugin;
        use bevy::ecs::system::RunSystemOnce;
        for edge_start in [0.3, 0.417345] {
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
        bundle.transform.translation = Vec3::new(0.0, 5.0, edge_start);
        let character = app.world_mut().spawn(bundle).id();
        *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
        app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(10.0, 4.0, 20.0),
            Transform::from_xyz(0.0, -2.0, -10.0)));
        for _ in 0..3 { app.update(); }
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air);
        let start = Vec3::new(0.0, 1.0146527, edge_start);
        app.world_mut().get_mut::<Position>(character).unwrap().0 = start;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation = start;
        app.world_mut().get_mut::<LinearVelocity>(character).unwrap().0 = Vec3::new(0.0, -1.686, -2.3968756);
        for frame in 0..12 {
            app.update();
            if frame == 0 {
                let position = app.world().get::<Position>(character).unwrap().0;
                let (floor, contact) = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                    collisions: avian3d::prelude::Collisions,
                    candidates: Query<crate::als::motion::CharacterCollisionCandidates>| {
                    let floor = crate::als::motion::find_ground_floor(&spatial, position, 0.9, 0.3, &crate::als::params::CmcAdapterParams::default(), LocomotionState::Air,
                        &SpatialQueryFilter::from_excluded_entities([character]), character, &candidates);
                    let contact = collisions.collisions_with(character).any(|pair| pair.manifolds.iter()
                        .any(|manifold| manifold.points.iter().any(|point| point.normal_impulse > 0.0)));
                    (floor, contact)
                }).unwrap();
                let valid_perch = edge_start == 0.3;
                assert!(!contact);
                assert_eq!(floor.is_some_and(|(center, _, _)| (position.y - center).abs() <= 0.024), valid_perch,
                    "sweep-only perch eligibility at {position:?}: {floor:?}");
                assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(),
                    if valid_perch { LocomotionState::Grounded } else { LocomotionState::Air },
                    "first blocking sweep ignored perch eligibility: {position:?}");
            }
        }
        let final_position = app.world().get::<Position>(character).unwrap().0;
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(),
            if edge_start == 0.3 { LocomotionState::Grounded } else { LocomotionState::Air },
            "edge_start={edge_start} final_position={final_position:?}");
        if edge_start != 0.3 { assert!(final_position.z > 0.15, "{final_position:?}"); }
        }
    }

    #[test]
    fn swept_landing_ignores_nonblocking_floor_candidates() {
        use crate::als::character::{AlsCharacterBundle, AlsCharacterPlugin, AlsCollisionLayer};
        use crate::als::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
        use crate::als::motion::{AlsMotionPlugin, initial_als_control_scheme_config};
        use crate::als::params::AlsMovementParams;
        use crate::als::transition::AlsStateTransitionPlugin;
        use bevy_tnua::{TnuaControllerPlugin, TnuaPipelineSystems, TnuaProximitySensor, TnuaSensorsEntities};
        use bevy_tnua_avian3d::TnuaAvian3dPlugin;
        let blocking = CollisionLayers::new(AlsCollisionLayer::Default, AlsCollisionLayer::Default);
        let mut failures = Vec::new();
        for case in ["none", "sensor", "membership", "response", "attached", "blocking"] {
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
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)))
            .add_systems(Update, (|entities: Query<&TnuaSensorsEntities<AlsControlScheme>>,
                mut sensors: Query<&mut TnuaProximitySensor>| {
                for entities in &entities {
                    if let Some(sensor) = entities.ground {
                        sensors.get_mut(sensor).unwrap().output = None;
                    }
                }
            }).after(TnuaPipelineSystems::Sensors).before(TnuaPipelineSystems::Logic));
            app.finish();
            app.update();
            let mut value = AlsMovementParams::default();
            value.acceleration.max_acceleration = 0.0;
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(value.clone());
            let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>()
                .add(initial_als_control_scheme_config(&value));
            let mut bundle = AlsCharacterBundle::new(params, config);
            bundle.transform.translation = Vec3::Y * 5.0;
            let character = app.world_mut().spawn((bundle, blocking)).id();
            for _ in 0..3 { app.update(); }
            assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air);
            let start = Vec3::Y * 2.0;
            app.world_mut().get_mut::<Position>(character).unwrap().0 = start;
            app.world_mut().get_mut::<Transform>(character).unwrap().translation = start;
            app.world_mut().get_mut::<LinearVelocity>(character).unwrap().0 = Vec3::new(3.75, 0.0, 3.75);
            let normal = Vec3::new(-3.0_f32.sqrt() * 0.5, 0.5, 0.0);
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.02, 20.0, 20.0), blocking,
                Transform::from_translation(start - normal * 0.87)
                    .with_rotation(Quat::from_rotation_arc(Vec3::X, normal))));
            let auxiliary = (case != "none").then(|| {
                let layers = match case {
                    "membership" => CollisionLayers::new(AlsCollisionLayer::Visibility, LayerMask::ALL),
                    "response" => CollisionLayers::new(AlsCollisionLayer::Default, AlsCollisionLayer::Visibility),
                    _ => blocking,
                };
                let auxiliary = app.world_mut().spawn((Collider::cuboid(10.0, 0.02, 10.0), layers,
                    Transform::from_xyz(0.0, if case == "attached" { -0.911 } else { 1.053723 }, 0.0))).id();
                if case == "attached" {
                    app.world_mut().entity_mut(character).add_child(auxiliary);
                } else {
                    app.world_mut().entity_mut(auxiliary).insert(RigidBody::Static);
                }
                if case == "sensor" { app.world_mut().entity_mut(auxiliary).insert(Sensor); }
                auxiliary
            });
            app.update();
            let position = app.world().get::<Position>(character).unwrap().0;
            let mode = *app.world().get::<LocomotionState>(character).unwrap();
            let base = app.world().get::<crate::als::dynamics::AlsMovementBase>(character).unwrap().entity;
            let expected = if case == "blocking" { LocomotionState::Grounded } else { LocomotionState::Air };
            if case != "blocking" {
                assert!(position.x < 0.35, "{case} missed the real slope: {position:?}");
            }
            if mode != expected || base != if case == "blocking" { auxiliary } else { None } {
                failures.push((case, mode, base, auxiliary, position));
            }
        }
        assert!(failures.is_empty(), "swept landing accepted nonblocking floors: {failures:?}");
    }

    #[test]
    fn full_frame_crouch_keeps_the_capsule_on_the_floor() {
        use crate::als::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use crate::als::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
        use crate::als::motion::{AlsMotionPlugin, initial_als_control_scheme_config};
        use crate::als::params::AlsMovementParams;
        use crate::als::state::Stance;
        use crate::als::transition::AlsStateTransitionPlugin;
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
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().stance = Some(Stance::Crouching);
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
                crate::als::character::AlsCharacter,
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
            crate::als::character::AlsCharacter,
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
            crate::als::character::AlsCharacter,
            LocomotionState::Grounded,
            crate::als::state::AlsNotifyLocomotionAction::Ragdolling,
            RigidBody::Kinematic,
            Collider::cuboid(0.2, 0.2, 0.2),
            GravityScale(0.0),
            Position(Vec3::Y * 6.0),
            LinearVelocity(Vec3::X * 3.75),
            Transform::from_xyz(0.0, 6.0, 0.0),
        )).id();
        let ragdoll_pending = app.world_mut().spawn((
            crate::als::character::AlsCharacter,
            LocomotionState::Grounded,
            crate::als::state::AlsNotifyLocomotionAction::Ragdolling,
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
            crate::als::state::AlsNotifyLocomotionAction::None,
        ));
        app.world_mut().entity_mut(ragdoll_pending)
            .insert(crate::als::state::AlsNotifyLocomotionAction::None);
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
        use crate::als::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use crate::als::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
        use crate::als::input::AlsInputCommand;
        use crate::als::motion::{AlsMotionPlugin, initial_als_control_scheme_config};
        use crate::als::params::AlsMovementParams;
        use crate::als::transition::AlsStateTransitionPlugin;
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
        let base = app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
        assert_eq!(base.entity, Some(platform));
        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(platform).unwrap().0 = Vec3::X;
        assert_eq!(app.world().get::<avian3d::prelude::LinearVelocity>(character).unwrap().0.with_y(0.0), Vec3::ZERO);
        let carried_before = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x;
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app.update();
        let carried_after = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x;
        assert!((carried_after - carried_before).abs() < 1e-3,
            "moving base changed idle character offset: {carried_before} -> {carried_after}");

        let braking_before = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x;
        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(character).unwrap().0 = Vec3::X * 2.0;
        app.update();
        let braking_after = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x;
        assert!((braking_after - braking_before - 0.0125).abs() < 1e-4,
            "Source relative braking moves before tail velocity reaches zero: {braking_before} -> {braking_after}");
        assert!((app.world().get::<avian3d::prelude::LinearVelocity>(character).unwrap().x - 1.0).abs() < 1e-4);

        app.world_mut().get_mut::<avian3d::prelude::LinearVelocity>(character).unwrap().0 = Vec3::X * 4.75;
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::X;
        let offset_before = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x;
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app.update();
        let offset_after = app.world().get::<Position>(character).unwrap().x
            - app.world().get::<Position>(platform).unwrap().x;
        let relative_motion = offset_after - offset_before;
        let base = app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
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
