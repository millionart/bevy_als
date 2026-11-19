use crate::als::character::AlsCharacter;
use crate::als::dynamics::{AlsMovementBase, AlsMovementBaseDynamicsState, locomotion_velocity};
use crate::als::orientation::world_forward_to_yaw;
use crate::als::state::LocomotionState;
use avian3d::prelude::{ColliderOf, LinearVelocity, RigidBody};
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HorizontalVelocityBlend {
    pub forward: f32,
    pub backward: f32,
    pub left: f32,
    pub right: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HorizontalMotion {
    pub world_velocity: Vec3,
    pub local_velocity: Vec2,
    pub horizontal_speed: f32,
    pub signed_direction_radians: f32,
    pub velocity_blend: HorizontalVelocityBlend,
}

pub fn horizontal_motion_from_world_velocity(
    world_velocity: Vec3,
    character_rotation: Quat,
) -> HorizontalMotion {
    let world_velocity = Vec3::new(world_velocity.x, 0.0, world_velocity.z);
    let forward = character_rotation * Vec3::NEG_Z;
    let right = character_rotation * Vec3::X;
    let local_velocity = Vec2::new(world_velocity.dot(forward), world_velocity.dot(right));
    let horizontal_speed = local_velocity.length();
    if horizontal_speed <= f32::EPSILON {
        return HorizontalMotion {
            world_velocity,
            local_velocity,
            ..HorizontalMotion::default()
        };
    }
    let unit = local_velocity / horizontal_speed;
    let normalized = unit / (unit.x.abs() + unit.y.abs()).max(f32::EPSILON);
    HorizontalMotion {
        world_velocity,
        local_velocity,
        horizontal_speed,
        signed_direction_radians: local_velocity.y.atan2(local_velocity.x),
        velocity_blend: HorizontalVelocityBlend {
            forward: normalized.x.max(0.0),
            backward: (-normalized.x).max(0.0),
            left: (-normalized.y).max(0.0),
            right: normalized.y.max(0.0),
        },
    }
}

const INPUT_DIRECTION_MIN_LENGTH_SQUARED: f32 = 1e-08;

pub(crate) fn consumed_input_direction(input: Vec3) -> Vec3 {
    if input.length_squared() < INPUT_DIRECTION_MIN_LENGTH_SQUARED {
        Vec3::ZERO
    } else {
        input.normalize_or_zero()
    }
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsActualMotion {
    pub world_velocity: Vec3,
    pub world_acceleration: Vec3,
    pub movement_base_snapshot: Option<(Entity, Quat)>,
    pub movement_base_delta_yaw_degrees: f32,
    pub local_velocity: Vec2,
    pub horizontal_speed: f32,
    pub velocity_yaw_degrees: f32,
    pub source_velocity_yaw_degrees: Option<f32>,
    pub has_input: bool,
    pub input_yaw_degrees: f32,
    pub signed_direction_radians: f32,
    pub velocity_blend: HorizontalVelocityBlend,
}

impl AlsActualMotion {
    fn from_physics_velocity(world_velocity: Vec3, character_rotation: Quat) -> Self {
        let motion = horizontal_motion_from_world_velocity(world_velocity, character_rotation);
        let direction =
            crate::als::animation::velocity_blend_direction(world_velocity, character_rotation);
        let horizontal_speed = crate::als::motion::source_horizontal_speed_meters(world_velocity);
        Self {
            world_velocity,
            local_velocity: motion.local_velocity,
            horizontal_speed,
            velocity_yaw_degrees: world_forward_to_yaw(world_velocity).to_degrees(),
            source_velocity_yaw_degrees: None,
            signed_direction_radians: motion.signed_direction_radians,
            velocity_blend: HorizontalVelocityBlend {
                forward: (-direction.z).max(0.0),
                backward: direction.z.max(0.0),
                left: (-direction.x).max(0.0),
                right: direction.x.max(0.0),
            },
            ..default()
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsActualMotionSet;

pub struct AlsActualMotionPlugin;

impl Plugin for AlsActualMotionPlugin {
    fn build(&self, app: &mut App) {
        let outer = crate::AlsPhysicsSchedule::outer(app);
        app.configure_sets(
            outer,
            AlsActualMotionSet.before(crate::gait_layer::AlsGaitLayerSet),
        )
        .add_systems(
            outer,
            snapshot_actual_motion_system.in_set(AlsActualMotionSet),
        );
    }
}

fn snapshot_actual_motion_system(
    time: Res<Time>,
    virtual_time: Res<Time<Virtual>>,
    mut commands: Commands,
    transforms: bevy::transform::helper::TransformHelper,
    mut characters: Query<
        (
            Entity,
            &LinearVelocity,
            &LocomotionState,
            Option<&AlsMovementBaseDynamicsState>,
            Option<&AlsMovementBase>,
            Option<
                &bevy_tnua::prelude::TnuaController<crate::als::control_scheme::AlsControlScheme>,
            >,
            Option<&mut AlsActualMotion>,
        ),
        With<AlsCharacter>,
    >,
    owners: Query<&ColliderOf>,
    bodies: Query<&RigidBody>,
) {
    if virtual_time.is_paused() {
        return;
    }
    for (entity, velocity, locomotion, base, movement_base, controller, current) in &mut characters
    {
        let Ok(transform) = transforms.compute_global_transform(entity) else {
            continue;
        };
        let mut next = AlsActualMotion::from_physics_velocity(
            locomotion_velocity(velocity.0, *locomotion, base),
            transform.rotation(),
        );
        next.movement_base_snapshot = movement_base
            .filter(|base| *locomotion == LocomotionState::Grounded && base.has_relative_rotation)
            .and_then(|base| base.entity.map(|entity| (entity, base.rotation)))
            .filter(|(entity, _)| {
                let owner = owners
                    .get(*entity)
                    .map_or(*entity, |collider| collider.body);
                bodies
                    .get(owner)
                    .is_ok_and(|body| *body != RigidBody::Static)
            });
        next.movement_base_delta_yaw_degrees = match (
            current
                .as_deref()
                .and_then(|motion| motion.movement_base_snapshot),
            next.movement_base_snapshot,
        ) {
            (Some((previous_entity, previous_rotation)), Some((entity, rotation)))
                if previous_entity == entity =>
            {
                -world_forward_to_yaw(rotation * previous_rotation.conjugate() * Vec3::NEG_Z)
                    .to_degrees()
            }
            _ => 0.0,
        };
        if let Some(previous) = current.as_deref().filter(|_| time.delta_secs() > 1e-8) {
            let previous_velocity =
                match (previous.movement_base_snapshot, next.movement_base_snapshot) {
                    (Some((previous_entity, previous_rotation)), Some((entity, rotation)))
                        if previous_entity == entity =>
                    {
                        rotation * previous_rotation.conjugate() * previous.world_velocity
                    }
                    _ => previous.world_velocity,
                };
            next.world_acceleration = (next.world_velocity - previous_velocity) / time.delta_secs();
        }
        let initial_yaw = world_forward_to_yaw(*transform.forward()).to_degrees();
        let input = controller.map_or(Vec3::ZERO, |controller| {
            consumed_input_direction(controller.basis.desired_motion)
        });
        next.has_input = input != Vec3::ZERO;
        next.input_yaw_degrees = if next.has_input {
            world_forward_to_yaw(input).to_degrees()
        } else {
            current
                .as_deref()
                .map_or(initial_yaw, |previous| previous.input_yaw_degrees)
        };
        if crate::als::motion::locomotion_movement_flags(next.has_input, next.horizontal_speed).0 {
            let x = f64::from(next.world_velocity.x) * 100.0;
            let y = f64::from(next.world_velocity.z) * 100.0;
            next.source_velocity_yaw_degrees = Some(y.atan2(x).to_degrees() as f32);
        } else {
            next.velocity_yaw_degrees = current
                .as_deref()
                .map_or(initial_yaw, |previous| previous.velocity_yaw_degrees);
            next.source_velocity_yaw_degrees = current
                .as_deref()
                .and_then(|previous| previous.source_velocity_yaw_degrees);
        }
        if let Some(mut current) = current {
            *current = next;
        } else {
            commands.entity(entity).insert(next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_snapshot_preserves_native_velocity_yaw_before_the_view_difference() {
        let data: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/actions/first_motion_yaw.json",
        ))
        .unwrap();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(PostUpdate, snapshot_actual_motion_system);
        let actor = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::default(),
                LinearVelocity::ZERO,
                LocomotionState::Grounded,
            ))
            .id();
        for row in data.as_array().unwrap() {
            if row["name"].as_str().unwrap().starts_with("actual-reference") {
                continue;
            }
            let velocity = row["velocity_cm_s"].as_array().unwrap();
            let axis = |index: usize| (velocity[index].as_f64().unwrap() * 0.01) as f32;
            app.world_mut()
                .entity_mut(actor)
                .insert(LinearVelocity(Vec3::new(axis(0), axis(2), axis(1))));
            app.world_mut().run_schedule(PostUpdate);
            let motion = app.world().get::<AlsActualMotion>(actor).unwrap();
            let view = (-row["view_yaw_degrees"].as_f64().unwrap().to_radians()
                - std::f64::consts::FRAC_PI_2) as f32;
            let reference = &row["reference"];
            assert_eq!(
                motion.source_velocity_yaw_degrees,
                Some(reference["world_degrees"].as_f64().unwrap() as f32),
                "{}",
                row["name"]
            );
            let native_angle = reference["view_space_degrees"].as_f64().unwrap() as f32;
            let actual = crate::direction_layer::motion_view_space_direction_angle(motion, view);
            if row["name"].as_str().unwrap().starts_with("actual-target") {
                assert_eq!(actual, native_angle, "{}", row["name"]);
            } else {
                assert!(crate::als::head::unwind_degrees(actual - native_angle).abs() <= 1e-5);
            }
        }
        let saved = app
            .world()
            .get::<AlsActualMotion>(actor)
            .unwrap()
            .source_velocity_yaw_degrees;
        app.world_mut()
            .entity_mut(actor)
            .insert(LinearVelocity::ZERO);
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(
            app.world()
                .get::<AlsActualMotion>(actor)
                .unwrap()
                .source_velocity_yaw_degrees,
            saved
        );
    }

    #[test]
    fn actor_motion_reads_current_parent_world_rotation() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(PostUpdate, snapshot_actual_motion_system);
        let parent = app.world_mut().spawn(Transform::IDENTITY).id();
        let character = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::IDENTITY,
                ChildOf(parent),
                LocomotionState::Grounded,
                LinearVelocity::ZERO,
            ))
            .id();
        for yaw in [90.0_f32, -45.0, 170.0] {
            let rotation = Quat::from_rotation_y(yaw.to_radians());
            *app.world_mut().get_mut::<Transform>(parent).unwrap() =
                Transform::from_xyz(5.0, 3.0, -2.0)
                    .with_rotation(rotation)
                    .with_scale(Vec3::new(2.0, 3.0, 0.5));
            app.world_mut()
                .get_mut::<LinearVelocity>(character)
                .unwrap()
                .0 = rotation * Vec3::NEG_Z * 2.0;
            app.world_mut().run_schedule(PostUpdate);
            let actual = app.world().get::<AlsActualMotion>(character).unwrap();
            assert!(
                (actual.local_velocity - Vec2::new(2.0, 0.0)).length() < 1e-5,
                "Source actor forward changed under current parent yaw {yaw}: {:?}",
                actual.local_velocity
            );
        }
    }

    #[test]
    fn consumed_input_validity_uses_the_source_unitless_safe_normal_boundary() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(PostUpdate, snapshot_actual_motion_system);
        let character =
            app.world_mut()
                .spawn((
                    AlsCharacter,
                    Transform::IDENTITY,
                    LocomotionState::Grounded,
                    LinearVelocity::ZERO,
                    bevy_tnua::prelude::TnuaController::<
                        crate::als::control_scheme::AlsControlScheme,
                    >::default(),
                ))
                .id();
        for (amount, expected_input, expected_yaw) in [
            (0.0_f32, false, 0.0_f32),
            (0.00001, false, 0.0),
            (0.000099, false, 0.0),
            (0.000101, true, -90.0),
            (0.0002, true, -90.0),
            (1.0, true, -90.0),
            (0.00001, false, -90.0),
            (0.0, false, -90.0),
        ] {
            app.world_mut().get_mut::<bevy_tnua::prelude::TnuaController<crate::als::control_scheme::AlsControlScheme>>(character)
                .unwrap().basis.desired_motion = Vec3::X * amount;
            app.world_mut().run_schedule(PostUpdate);
            let motion = app.world().get::<AlsActualMotion>(character).unwrap();
            assert_eq!(
                motion.has_input, expected_input,
                "Source unitless input boundary differs: {amount}"
            );
            assert!(
                (motion.input_yaw_degrees - expected_yaw).abs() < 1e-5,
                "Source retained input yaw differs: {amount}, {} != {expected_yaw}",
                motion.input_yaw_degrees
            );
        }
    }

    #[test]
    fn idle_motion_initializes_retained_directions_from_the_actor() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(PostUpdate, snapshot_actual_motion_system);
        let character = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::from_rotation(Quat::from_rotation_y(0.7)),
                LocomotionState::Grounded,
                LinearVelocity::ZERO,
            ))
            .id();
        app.world_mut().run_schedule(PostUpdate);
        let motion = app.world().get::<AlsActualMotion>(character).unwrap();
        assert!(!motion.has_input);
        assert!((motion.input_yaw_degrees - 0.7_f32.to_degrees()).abs() < 1e-5);
        assert!((motion.velocity_yaw_degrees - 0.7_f32.to_degrees()).abs() < 1e-5);
    }

    #[test]
    fn snapshot_excludes_passive_base_transport_and_preserves_air_velocity() {
        use crate::als::{dynamics::AlsMovementBaseDynamicsState, state::LocomotionState};
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(PostUpdate, snapshot_actual_motion_system);
        let base = app.world_mut().spawn_empty().id();
        let character = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::IDENTITY,
                LocomotionState::Grounded,
                LinearVelocity(Vec3::X * 2.0),
                AlsMovementBaseDynamicsState {
                    entity: Some(base),
                    support_velocity: Vec3::X * 2.0,
                    ..default()
                },
            ))
            .id();
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(
            app.world()
                .get::<AlsActualMotion>(character)
                .unwrap()
                .world_velocity,
            Vec3::ZERO
        );
        app.world_mut()
            .get_mut::<LinearVelocity>(character)
            .unwrap()
            .0 += Vec3::NEG_Z;
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(
            app.world()
                .get::<AlsActualMotion>(character)
                .unwrap()
                .world_velocity,
            Vec3::NEG_Z
        );
        *app.world_mut()
            .get_mut::<LocomotionState>(character)
            .unwrap() = LocomotionState::Air;
        app.world_mut()
            .get_mut::<LinearVelocity>(character)
            .unwrap()
            .0 = Vec3::new(2.0, 3.0, -1.0);
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(
            app.world()
                .get::<AlsActualMotion>(character)
                .unwrap()
                .world_velocity,
            Vec3::new(2.0, 3.0, -1.0)
        );
    }

    #[test]
    fn animation_motion_uses_source_centimeter_speed_rounding() {
        for (velocity, expected) in [
            (Vec3::new(1.333_333_4, 7.0, 0.0), 1.333_333_5),
            (Vec3::new(1.485_657_6, -4.0, 0.0), 1.485_657_5),
        ] {
            let motion =
                AlsActualMotion::from_physics_velocity(velocity, Quat::from_rotation_x(1.0));
            assert_eq!(motion.world_velocity, velocity);
            assert_eq!(motion.horizontal_speed, expected);
        }
    }

    #[test]
    fn moving_smooth_uses_world_horizontal_speed_for_pitched_actor() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(PostUpdate, snapshot_actual_motion_system);
        let mut actors = Vec::new();
        for pitch in [
            0.0,
            std::f32::consts::FRAC_PI_3,
            -std::f32::consts::FRAC_PI_3,
        ] {
            actors.push(
                app.world_mut()
                    .spawn((
                        AlsCharacter,
                        Transform::from_rotation(Quat::from_rotation_x(pitch)),
                        LocomotionState::Grounded,
                        LinearVelocity(Vec3::new(0.0, 7.0, -1.6)),
                    ))
                    .id(),
            );
        }
        app.world_mut().run_schedule(PostUpdate);
        for (index, actor) in actors.into_iter().enumerate() {
            let motion = app.world().get::<AlsActualMotion>(actor).unwrap();
            assert_eq!(motion.horizontal_speed, 1.6);
            assert!(crate::standing_layer::source_moving_smooth(
                false,
                motion.horizontal_speed
            ));
            if index > 0 {
                assert!(motion.local_velocity.length() < 1.0);
            }
        }
    }

    #[test]
    fn standing_source_motion_retains_full_velocity_for_three_axis_blend() {
        let velocity = Vec3::new(1.0, 2.0, -1.0);
        let motion = AlsActualMotion::from_physics_velocity(velocity, Quat::IDENTITY);
        assert_eq!(motion.world_velocity, velocity);
        assert!((motion.horizontal_speed - 2.0_f32.sqrt()).abs() < 1e-6);
        assert!((motion.velocity_blend.forward - 0.25).abs() < 1e-6);
        assert!((motion.velocity_blend.right - 0.25).abs() < 1e-6);
    }

    #[test]
    fn snapshot_is_character_local_and_isolated() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(FixedPostUpdate, snapshot_actual_motion_system);
        let first = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::default(),
                LinearVelocity(Vec3::NEG_Z),
                LocomotionState::Grounded,
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
                LinearVelocity(Vec3::NEG_X),
                LocomotionState::Grounded,
            ))
            .id();

        app.world_mut().run_schedule(FixedPostUpdate);

        let first_motion = *app.world().get::<AlsActualMotion>(first).unwrap();
        let second_motion = *app.world().get::<AlsActualMotion>(second).unwrap();
        assert!(first_motion.local_velocity.distance(Vec2::X) < 1e-6);
        assert!(second_motion.local_velocity.distance(Vec2::X) < 1e-6);
        assert_eq!(first_motion.velocity_blend.forward, 1.0);
        #[cfg(target_arch = "aarch64")]
        assert_eq!(
            second_motion.velocity_blend.forward,
            f32::from_bits(0x3f7f_fffe)
        );
        #[cfg(not(target_arch = "aarch64"))]
        assert!((second_motion.velocity_blend.forward - 1.0).abs() <= f32::EPSILON);

        app.world_mut()
            .entity_mut(first)
            .insert(LinearVelocity::default());
        app.world_mut().run_schedule(FixedPostUpdate);

        let stopped_motion = *app.world().get::<AlsActualMotion>(first).unwrap();
        assert_eq!(stopped_motion.horizontal_speed, 0.0);
        assert_eq!(stopped_motion.local_velocity, Vec2::ZERO);
        assert_eq!(
            stopped_motion.velocity_blend,
            HorizontalVelocityBlend::default()
        );
        assert!(second_motion.horizontal_speed > 0.0);
    }

    #[test]
    fn animation_acceleration_compensates_rotating_movement_base() {
        use crate::als::dynamics::{AlsMovementBase, AlsMovementBaseDynamicsState};
        use avian3d::prelude::RigidBody;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(PostUpdate, snapshot_actual_motion_system);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.1));
        let base = app.world_mut().spawn(RigidBody::Kinematic).id();
        let character = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::default(),
                LinearVelocity(Vec3::NEG_Z),
                LocomotionState::Grounded,
                AlsMovementBaseDynamicsState {
                    entity: Some(base),
                    ..default()
                },
                AlsMovementBase {
                    entity: Some(base),
                    has_relative_rotation: true,
                    ..default()
                },
            ))
            .id();
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(
            app.world()
                .get::<AlsActualMotion>(character)
                .unwrap()
                .world_acceleration,
            Vec3::ZERO
        );
        app.world_mut()
            .get_mut::<AlsMovementBase>(character)
            .unwrap()
            .rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        app.world_mut()
            .get_mut::<LinearVelocity>(character)
            .unwrap()
            .0 = Vec3::NEG_X;
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            (app.world()
                .get::<AlsActualMotion>(character)
                .unwrap()
                .movement_base_delta_yaw_degrees
                + 90.0)
                .abs()
                < 1e-5
        );
        assert!(
            app.world()
                .get::<AlsActualMotion>(character)
                .unwrap()
                .world_acceleration
                .length()
                < 1e-5
        );
        app.world_mut()
            .get_mut::<LinearVelocity>(character)
            .unwrap()
            .0 = Vec3::NEG_X * 0.5;
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            (app.world()
                .get::<AlsActualMotion>(character)
                .unwrap()
                .world_acceleration
                - Vec3::X * 5.0)
                .length()
                < 1e-5
        );
        let next_base = app.world_mut().spawn(RigidBody::Kinematic).id();
        app.world_mut()
            .get_mut::<AlsMovementBaseDynamicsState>(character)
            .unwrap()
            .entity = Some(next_base);
        let mut movement_base = app
            .world_mut()
            .get_mut::<AlsMovementBase>(character)
            .unwrap();
        movement_base.entity = Some(next_base);
        movement_base.rotation = Quat::IDENTITY;
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(
            app.world()
                .get::<AlsActualMotion>(character)
                .unwrap()
                .movement_base_delta_yaw_degrees,
            0.0
        );
        assert_eq!(
            app.world()
                .get::<AlsActualMotion>(character)
                .unwrap()
                .world_acceleration,
            Vec3::ZERO
        );
        app.world_mut()
            .entity_mut(next_base)
            .insert(RigidBody::Static);
        app.world_mut()
            .get_mut::<AlsMovementBase>(character)
            .unwrap()
            .rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(
            app.world()
                .get::<AlsActualMotion>(character)
                .unwrap()
                .world_acceleration,
            Vec3::ZERO
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::ZERO);
        app.world_mut()
            .get_mut::<LinearVelocity>(character)
            .unwrap()
            .0 = Vec3::Y;
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(
            app.world()
                .get::<AlsActualMotion>(character)
                .unwrap()
                .world_acceleration,
            Vec3::ZERO
        );
    }

    #[test]
    fn rotating_base_without_relative_rotation_keeps_world_velocity_acceleration_zero() {
        for has_relative_rotation in [false, true] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .add_systems(PostUpdate, snapshot_actual_motion_system);
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            let base = app.world_mut().spawn(RigidBody::Kinematic).id();
            let character = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    Transform::IDENTITY,
                    LinearVelocity(Vec3::X * 3.5),
                    LocomotionState::Grounded,
                    AlsMovementBaseDynamicsState {
                        entity: Some(base),
                        ..default()
                    },
                    AlsMovementBase {
                        entity: Some(base),
                        has_relative_rotation,
                        ..default()
                    },
                ))
                .id();
            app.world_mut().run_schedule(PostUpdate);
            let rotation = Quat::from_rotation_y(1.0_f32.to_radians());
            app.world_mut()
                .get_mut::<AlsMovementBase>(character)
                .unwrap()
                .rotation = rotation;
            if has_relative_rotation {
                app.world_mut()
                    .get_mut::<LinearVelocity>(character)
                    .unwrap()
                    .0 = rotation * Vec3::X * 3.5;
            }
            app.world_mut().run_schedule(PostUpdate);
            let motion = app.world().get::<AlsActualMotion>(character).unwrap();
            assert!(
                motion.world_acceleration.length() < 1e-4,
                "relative_rotation={has_relative_rotation} gained acceleration: {:?}",
                motion.world_acceleration
            );
            assert_eq!(
                motion.movement_base_snapshot.is_some(),
                has_relative_rotation
            );
        }
    }

    #[test]
    fn horizontal_motion_preserves_local_forward_after_yaw_rotation() {
        let motion = horizontal_motion_from_world_velocity(
            Vec3::NEG_X,
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        );
        assert!(motion.local_velocity.distance(Vec2::X) < 1e-6);
        assert!(motion.signed_direction_radians.abs() < 1e-6);
        assert_eq!(
            motion.velocity_blend,
            HorizontalVelocityBlend {
                forward: 1.0,
                ..HorizontalVelocityBlend::default()
            }
        );
    }

    #[test]
    fn horizontal_motion_uses_manhattan_normalized_four_way_weights() {
        let motion =
            horizontal_motion_from_world_velocity(Vec3::new(1.0, 2.0, -1.0), Quat::IDENTITY);
        assert!((motion.horizontal_speed - 2.0_f32.sqrt()).abs() < 1e-6);
        assert!((motion.signed_direction_radians - std::f32::consts::FRAC_PI_4).abs() < 1e-6);
        assert!((motion.velocity_blend.forward - 0.5).abs() < 1e-6);
        assert!((motion.velocity_blend.right - 0.5).abs() < 1e-6);
        assert_eq!(motion.velocity_blend.backward, 0.0);
        assert_eq!(motion.velocity_blend.left, 0.0);
    }

    #[test]
    fn horizontal_motion_zeroes_vertical_and_stationary_velocity() {
        let motion = horizontal_motion_from_world_velocity(Vec3::Y * 3.0, Quat::IDENTITY);
        assert_eq!(motion.world_velocity, Vec3::ZERO);
        assert_eq!(motion.local_velocity, Vec2::ZERO);
        assert_eq!(motion.horizontal_speed, 0.0);
        assert_eq!(motion.velocity_blend, HorizontalVelocityBlend::default());
    }
}
