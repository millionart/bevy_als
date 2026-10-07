use avian3d::prelude::{AngularVelocity, ColliderOf, LinearVelocity, Position, RigidBody};
use bevy::prelude::*;
use bevy_tnua::{TnuaController, TnuaMotor, TnuaPipelineSystems};

use crate::als::character::{AlsCharacter, AlsCmcAdapterRuntime, AlsMovementParamsHandle};
use crate::als::control_scheme::AlsControlScheme;
use crate::als::gait::{AlsGaitAmount, AlsMaxAllowedGait, calculate_gait_amount_for_thresholds};
use crate::als::movement_constants::{BRAKE_TO_STOP_VELOCITY, BRAKING_SMALL_VELOCITY_SQUARED, MAX_SPEED_SQUARED_RATIO, MIN_MOVEMENT_DELTA_SECONDS};
use crate::actual_motion::AlsActualMotion;
use crate::als::mantle::AlsMantleState;
use crate::als::motion::AlsViewYaw;
use crate::als::params::{AlsMovementParams, CmcAdapterParams, GroundedDynamics};
use crate::als::roll::AlsRollState;
use crate::als::state::{Gait, LocomotionState, RotationMode, Stance};

pub const BRAKING_FRICTION_FACTOR: f32 = 0.0;
const PHYSICAL_VECTOR_MIN_LENGTH_SQUARED: f32 = 1e-12;
const MAX_ACCELERATION_MIN_FOR_ANALOG_MODIFIER: f32 = 1e-10;
pub const LANDING_BRAKING_FRICTION_WITH_INPUT: f32 = 0.5;
pub const LANDING_BRAKING_FRICTION_WITHOUT_INPUT: f32 = 3.0;
pub const LANDING_BRAKING_FRICTION_DURATION: f32 = 0.5;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsGroundedDynamics(pub GroundedDynamics);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsLandingBrakingFriction {
    pub factor: f32,
    pub remaining_seconds: f64,
    pub previous_locomotion: LocomotionState,
}

impl Default for AlsLandingBrakingFriction {
    fn default() -> Self {
        Self {
            factor: BRAKING_FRICTION_FACTOR,
            remaining_seconds: 0.0,
            previous_locomotion: LocomotionState::Grounded,
        }
    }
}

impl Default for AlsGroundedDynamics {
    fn default() -> Self {
        Self(GroundedDynamics {
            acceleration: 20.0,
            braking_deceleration: 15.0,
            ground_friction: 5.0,
        })
    }
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsMovementBaseDynamicsState {
    pub entity: Option<Entity>,
    pub world_velocity: Vec3,
    pub support_velocity: Vec3,
    pub relative_velocity: Vec3,
    pub motor_boost: Vec3,
    pub pre_collision_velocity: Option<Vec3>,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsMovementBase {
    pub entity: Option<Entity>,
    pub location: Vec3,
    pub rotation: Quat,
    pub has_relative_rotation: bool,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsComponentVelocity(pub Vec3);

pub fn component_velocity(body: &RigidBody, physical: &LinearVelocity, component: Option<&AlsComponentVelocity>) -> Vec3 {
    if *body == RigidBody::Dynamic { physical.0 } else { component.map_or(Vec3::ZERO, |velocity| velocity.0) }
}

pub fn locomotion_velocity(
    world_velocity: Vec3,
    locomotion: LocomotionState,
    base: Option<&AlsMovementBaseDynamicsState>,
) -> Vec3 {
    world_velocity - base.filter(|base| locomotion == LocomotionState::Grounded && base.entity.is_some())
        .map_or(Vec3::ZERO, |base| base.support_velocity.with_y(0.0))
}

pub fn support_point_velocity(
    base_position: Vec3,
    base_linear_velocity: Vec3,
    base_angular_velocity: Vec3,
    point: Vec3,
) -> Vec3 {
    base_linear_velocity + base_angular_velocity.cross(point - base_position)
}

fn character_contact_point(params: &AlsMovementParams, stance: Stance, position: Vec3) -> Vec3 {
    let half_height = if stance == Stance::Crouching { params.crouch.crouched_capsule_half_height } else { params.crouch.standing_capsule_half_height };
    position - Vec3::Y * half_height
}

fn support_velocity_from_base(
    base_entity: Option<Entity>,
    contact_point: Vec3,
    bases: &Query<(&Position, &LinearVelocity, Option<&AngularVelocity>), Without<AlsCharacter>>,
    colliders: &Query<&ColliderOf>,
) -> (Option<Entity>, Vec3) {
    let Some(entity) = base_entity else {
        return (None, Vec3::ZERO);
    };
    let body = colliders.get(entity).map_or(entity, |collider| collider.body);
    let Ok((position, linear_velocity, angular_velocity)) = bases.get(body) else {
        return (None, Vec3::ZERO);
    };
    (
        Some(entity),
        support_point_velocity(
            position.0,
            linear_velocity.0,
            angular_velocity.map_or(Vec3::ZERO, |velocity| velocity.0),
            contact_point,
        ),
    )
}

pub fn max_allowed_speed(
    params: &AlsMovementParams,
    stance: Stance,
    rotation_mode: RotationMode,
    gait: Gait,
    velocity: Vec3,
    view_yaw: f32,
) -> f32 {
    crate::als::motion::gait_max_speed_for_velocity(params, &stance, &gait, rotation_mode, velocity, view_yaw)
}

pub fn grounded_velocity_step(
    velocity: Vec3,
    desired_motion: Vec3,
    max_speed: f32,
    dynamics: GroundedDynamics,
    min_analog_walk_speed: f32,
    delta_seconds: f32,
) -> Vec3 {
    grounded_velocity_step_with_braking_friction(
        velocity,
        desired_motion,
        max_speed,
        dynamics,
        min_analog_walk_speed,
        BRAKING_FRICTION_FACTOR,
        CmcAdapterParams::default().braking_substep_time,
        delta_seconds,
    )
}

pub fn grounded_velocity_step_with_braking_friction(
    velocity: Vec3,
    desired_motion: Vec3,
    max_speed: f32,
    dynamics: GroundedDynamics,
    min_analog_walk_speed: f32,
    braking_friction_factor: f32,
    braking_substep_time: f32,
    delta_seconds: f32,
) -> Vec3 {
    let horizontal_velocity = Vec3::new(velocity.x, 0.0, velocity.z);
    let desired_horizontal = Vec3::new(desired_motion.x, 0.0, desired_motion.z);
    let desired_amount = desired_horizontal.length().min(1.0);
    let speed = horizontal_velocity.length();
    let delta_seconds = delta_seconds.max(0.0);
    if delta_seconds < MIN_MOVEMENT_DELTA_SECONDS { return horizontal_velocity; }
    let acceleration = desired_horizontal.clamp_length_max(1.0) * dynamics.acceleration.max(0.0);
    let has_acceleration = acceleration != Vec3::ZERO;
    let analog_modifier = if has_acceleration && dynamics.acceleration > MAX_ACCELERATION_MIN_FOR_ANALOG_MODIFIER { desired_amount } else { 0.0 };
    let target_speed = analog_target_speed(max_speed, analog_modifier, min_analog_walk_speed);
    let overspeed = horizontal_velocity.length_squared() > target_speed * target_speed * MAX_SPEED_SQUARED_RATIO;
    let acceleration_direction = if acceleration.length_squared() < PHYSICAL_VECTOR_MIN_LENGTH_SQUARED {
        Vec3::ZERO
    } else {
        acceleration.normalize_or_zero()
    };
    let mut next = horizontal_velocity;
    if !has_acceleration || overspeed {
        let friction = dynamics.ground_friction.max(0.0) * braking_friction_factor.max(0.0);
        let deceleration = dynamics.braking_deceleration.max(0.0);
        if friction > 0.0 || deceleration > 0.0 {
            let reverse_acceleration = -horizontal_velocity.normalize_or_zero() * deceleration;
            let mut remaining = delta_seconds;
            let max_step = braking_substep_time.clamp(1.0 / 75.0, 1.0 / 20.0);
            while remaining >= MIN_MOVEMENT_DELTA_SECONDS {
                let step = if remaining > max_step && friction > 0.0 {
                    max_step.min(remaining * 0.5)
                } else { remaining };
                remaining -= step;
                next += (-next * friction + reverse_acceleration) * step;
                if next.dot(horizontal_velocity) <= 0.0 {
                    next = Vec3::ZERO;
                    break;
                }
            }
            if next.length_squared() <= BRAKING_SMALL_VELOCITY_SQUARED
                || deceleration > 0.0 && next.length_squared() <= BRAKE_TO_STOP_VELOCITY.powi(2)
            {
                next = Vec3::ZERO;
            }
        }
        if overspeed && next.length_squared() < target_speed * target_speed
            && acceleration.dot(horizontal_velocity) > 0.0
        {
            next = horizontal_velocity.normalize_or_zero() * target_speed;
        }
    } else {
        let friction_alpha = (dynamics.ground_friction.max(0.0) * delta_seconds).clamp(0.0, 1.0);
        next -= (next - acceleration_direction * speed) * friction_alpha;
    }
    if !has_acceleration { return next; }
    let input_limit = if next.length_squared() > target_speed * target_speed * MAX_SPEED_SQUARED_RATIO {
        next.length()
    } else { target_speed };
    (next + acceleration * delta_seconds)
        .clamp_length_max(input_limit)
}

pub fn analog_target_speed(max_speed: f32, desired_amount: f32, min_analog_walk_speed: f32) -> f32 {
    let max_speed = max_speed.max(0.0);
    (max_speed * desired_amount.clamp(0.0, 1.0)).max(min_analog_walk_speed.max(0.0))
}

pub fn simulate_grounded_velocity(
    velocity: Vec3,
    desired_motion: Vec3,
    max_speed: f32,
    dynamics: GroundedDynamics,
    cmc: &CmcAdapterParams,
    delta_seconds: f32,
) -> (Vec3, u32) {
    simulate_grounded_velocity_with_braking_friction(
        velocity,
        desired_motion,
        max_speed,
        dynamics,
        cmc,
        BRAKING_FRICTION_FACTOR,
        delta_seconds,
    )
}

pub fn simulate_grounded_velocity_with_braking_friction(
    velocity: Vec3,
    desired_motion: Vec3,
    max_speed: f32,
    dynamics: GroundedDynamics,
    cmc: &CmcAdapterParams,
    braking_friction_factor: f32,
    delta_seconds: f32,
) -> (Vec3, u32) {
    let mut remaining = delta_seconds.max(0.0);
    let max_iterations = cmc.max_simulation_iterations;
    let mut current = velocity;
    let mut substeps = 0;
    while remaining >= MIN_MOVEMENT_DELTA_SECONDS && substeps < max_iterations {
        let step = if remaining > cmc.max_simulation_time_step && substeps + 1 < max_iterations {
            cmc.max_simulation_time_step.min(remaining * 0.5)
        } else {
            remaining
        }.max(MIN_MOVEMENT_DELTA_SECONDS);
        current = grounded_velocity_step_with_braking_friction(
            current,
            desired_motion,
            max_speed,
            dynamics,
            cmc.min_analog_walk_speed,
            braking_friction_factor,
            cmc.braking_substep_time,
            step,
        );
        remaining -= step;
        substeps += 1;
    }
    (current, substeps)
}

pub fn predict_grounded_stop_distance(
    params: &AlsMovementParams,
    stance: Stance,
    rotation_mode: RotationMode,
    initial_speed: f32,
) -> f32 {
    predict_grounded_stop_distance_with_braking_friction(
        params,
        stance,
        rotation_mode,
        initial_speed,
        BRAKING_FRICTION_FACTOR,
    )
}

pub fn predict_grounded_stop_distance_with_braking_friction(
    params: &AlsMovementParams,
    stance: Stance,
    rotation_mode: RotationMode,
    initial_speed: f32,
    braking_friction_factor: f32,
) -> f32 {
    let mut velocity = Vec3::new(initial_speed.max(0.0), 0.0, 0.0);
    let mut distance = 0.0;
    let delta_seconds = 1.0 / 128.0;
    for _ in 0..4096 {
        let speed = velocity.length();
        if speed <= 1e-4 {
            break;
        }
        let thresholds = params.gait_speed_thresholds(stance, rotation_mode, velocity, 0.0);
        let gait_amount = calculate_gait_amount_for_thresholds(speed, thresholds);
        let dynamics = params.grounded_dynamics(stance, rotation_mode, gait_amount);
        let (next, _) = simulate_grounded_velocity_with_braking_friction(
            velocity,
            Vec3::ZERO,
            0.0,
            dynamics,
            &params.cmc,
            braking_friction_factor,
            delta_seconds,
        );
        distance += next.length() * delta_seconds;
        velocity = next;
    }
    distance
}

pub(crate) fn landing_braking_friction_system(
    mut q: Query<
        (
            &LocomotionState,
            &AlsActualMotion,
            &mut AlsLandingBrakingFriction,
            Option<&mut crate::als::motion::AlsRotationResponseState>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (locomotion, motion, mut braking, rotation) in &mut q {
        let ordinary_landing = update_landing_braking_friction(
            &mut braking,
            *locomotion,
            motion.has_input,
            motion.world_velocity.y <= -crate::als::roll::ROLL_ON_LAND_SPEED_THRESHOLD,
        );
        if ordinary_landing && let Some(mut rotation) = rotation {
            rotation.rotation_towards_last_input_direction_blocked = true;
        }
    }
}

fn advance_landing_braking_timer_system(
    time: Res<Time>,
    mut characters: Query<&mut AlsLandingBrakingFriction, With<AlsCharacter>>,
) {
    for mut braking in &mut characters {
        if braking.factor != BRAKING_FRICTION_FACTOR {
            braking.remaining_seconds -= f64::from(time.delta_secs().max(0.0));
        }
    }
}

fn update_landing_braking_friction(
    braking: &mut AlsLandingBrakingFriction,
    locomotion: LocomotionState,
    has_input: bool,
    suppress: bool,
) -> bool {
    let landed = braking.previous_locomotion == LocomotionState::Air
        && locomotion == LocomotionState::Grounded;
    if landed && !suppress {
        braking.factor = if has_input {
            LANDING_BRAKING_FRICTION_WITH_INPUT
        } else {
            LANDING_BRAKING_FRICTION_WITHOUT_INPUT
        };
        braking.remaining_seconds = f64::from(LANDING_BRAKING_FRICTION_DURATION);
    }
    braking.previous_locomotion = locomotion;
    landed && !suppress
}

fn reset_landing_braking_friction_system(
    mut characters: Query<&mut AlsLandingBrakingFriction, With<AlsCharacter>>,
) {
    for mut braking in &mut characters {
        if braking.remaining_seconds < 0.0 {
            braking.factor = BRAKING_FRICTION_FACTOR;
            braking.remaining_seconds = 0.0;
        }
    }
}

pub fn grounded_dynamics_motor_system(
    time: Res<Time>,
    mut commands: Commands,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut q: Query<
        (
            Entity,
            &LocomotionState,
            &Stance,
            &RotationMode,
            &AlsViewYaw,
            &AlsMaxAllowedGait,
            &mut AlsGroundedDynamics,
            &mut AlsGaitAmount,
            &Position,
            &LinearVelocity,
            &AlsMovementParamsHandle,
            &TnuaController<AlsControlScheme>,
            &mut TnuaMotor,
            &mut AlsCmcAdapterRuntime,
            Option<&mut AlsMovementBaseDynamicsState>,
        ),
        With<AlsCharacter>,
    >,
    actions: Query<(Option<&AlsRollState>, Option<&AlsMantleState>), With<AlsCharacter>>,
    landing_braking: Query<&AlsLandingBrakingFriction, With<AlsCharacter>>,
    bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>), Without<AlsCharacter>>,
    colliders: Query<&ColliderOf>,
) {
    for (
        entity,
        locomotion,
        stance,
        rotation_mode,
        view_yaw,
        max_allowed_gait,
        mut dynamics,
        mut gait_amount,
        position,
        velocity,
        params_handle,
        controller,
        mut motor,
        mut cmc_runtime,
        current_base_state,
    ) in &mut q
    {
        let Ok((roll, mantle)) = actions.get(entity) else {
            continue;
        };
        if !matches!(*locomotion, LocomotionState::Grounded | LocomotionState::Air)
            || roll.is_some_and(AlsRollState::root_motion_active)
            || mantle.is_some_and(|state| state.active)
        {
            continue;
        }
        let Some(params) = params_assets.get(params_handle.id()) else {
            continue;
        };
        let world_velocity = Vec3::new(velocity.x, 0.0, velocity.z);
        if *locomotion == LocomotionState::Air {
            let max_speed = max_allowed_speed(params, *stance, *rotation_mode,
                max_allowed_gait.0, world_velocity, view_yaw.0);
            let air = GroundedDynamics {
                acceleration: crate::als::motion::air_acceleration_for_horizontal_speed(params, world_velocity.length())
                    .min(params.acceleration.max_acceleration),
                braking_deceleration: 0.0, ground_friction: 0.0,
            };
            let (next, _) = simulate_grounded_velocity_with_braking_friction(world_velocity,
                controller.basis.desired_motion, max_speed, air, &params.cmc, 0.0, time.delta_secs());
            motor.lin.acceleration.x = 0.0;
            motor.lin.acceleration.z = 0.0;
            motor.lin.boost.x = next.x - world_velocity.x;
            motor.lin.boost.z = next.z - world_velocity.z;
            continue;
        }
        let contact_point = character_contact_point(params, *stance, position.0);
        let (base_entity, support_velocity) = support_velocity_from_base(
            controller.basis_memory.standing_on_entity(),
            contact_point,
            &bases,
            &colliders,
        );
        let current = locomotion_velocity(world_velocity, *locomotion,
            current_base_state.as_deref().filter(|_| base_entity.is_some()));
        let thresholds = params.gait_speed_thresholds(*stance, *rotation_mode, current, view_yaw.0);
        gait_amount.0 = calculate_gait_amount_for_thresholds(current.length(), thresholds);
        if AlsMovementParams::has_gait_settings(*stance, *rotation_mode) {
            dynamics.0 = params.grounded_dynamics(*stance, *rotation_mode, gait_amount.0);
        }
        let max_speed = max_allowed_speed(
            params,
            *stance,
            *rotation_mode,
            max_allowed_gait.0,
            current,
            view_yaw.0,
        );
        let desired_motion = controller.basis.desired_motion;
        let braking_friction_factor = landing_braking
            .get(entity)
            .map_or(BRAKING_FRICTION_FACTOR, |state| state.factor);
        let (next, simulation_substeps) = simulate_grounded_velocity_with_braking_friction(
            current,
            desired_motion,
            max_speed,
            dynamics.0,
            &params.cmc,
            braking_friction_factor,
            time.delta_secs(),
        );
        cmc_runtime.simulation_delta_seconds = time.delta_secs();
        cmc_runtime.simulation_substeps = simulation_substeps;
        let analog_modifier = if dynamics.0.acceleration > MAX_ACCELERATION_MIN_FOR_ANALOG_MODIFIER { desired_motion.length().min(1.0) } else { 0.0 };
        cmc_runtime.min_analog_speed_applied = desired_motion.with_y(0.0) != Vec3::ZERO && dynamics.0.acceleration > 0.0
            && max_speed * analog_modifier < params.cmc.min_analog_walk_speed;
        let motor_boost = next - current;
        motor.lin.boost.x = next.x + support_velocity.x - world_velocity.x;
        motor.lin.boost.z = next.z + support_velocity.z - world_velocity.z;
        let base_state = AlsMovementBaseDynamicsState {
            entity: base_entity,
            world_velocity,
            support_velocity,
            relative_velocity: current,
            motor_boost,
            pre_collision_velocity: Some(next),
        };
        if let Some(mut current_base_state) = current_base_state {
            *current_base_state = base_state;
        } else {
            commands.entity(entity).insert(base_state);
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsPhysicsVolume {
    pub terminal_velocity: f32,
}

impl Default for AlsPhysicsVolume {
    fn default() -> Self {
        Self {
            terminal_velocity: crate::als::movement_constants::DEFAULT_TERMINAL_VELOCITY,
        }
    }
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct AlsDefaultPhysicsVolume(pub AlsPhysicsVolume);

pub(crate) fn limit_falling_velocity(
    time: Res<Time>,
    gravity: Res<avian3d::prelude::Gravity>,
    default_volume: Res<AlsDefaultPhysicsVolume>,
    mut characters: Query<(
        &LocomotionState, &RigidBody, &mut avian3d::dynamics::solver::solver_body::SolverBody,
        Option<&AlsPhysicsVolume>, Option<&avian3d::prelude::GravityScale>, Option<&bevy_tnua::TnuaGravity>,
        Option<&AlsRollState>, Option<&AlsMantleState>,
    ), With<AlsCharacter>>,
) {
    if time.delta().is_zero() { return; }
    for (mode, body, mut velocity, volume, scale, tnua_gravity, roll, mantle) in &mut characters {
        if *mode != LocomotionState::Air || *body != RigidBody::Dynamic
            || roll.is_some_and(AlsRollState::root_motion_active)
            || mantle.is_some_and(|state| state.active) { continue; }
        let gravity = crate::als::motion::effective_tnua_gravity(gravity.0,
            scale.map_or(1.0, |scale| scale.0), tnua_gravity.map(|gravity| gravity.0));
        let Some(direction) = gravity.try_normalize() else { continue; };
        let limit = volume.unwrap_or(&default_volume.0).terminal_velocity.abs();
        let falling = velocity.linear_velocity.dot(direction);
        if falling > limit {
            velocity.linear_velocity += direction * (limit - falling);
        }
    }
}

pub struct AlsGroundedDynamicsPlugin;

impl Plugin for AlsGroundedDynamicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                advance_landing_braking_timer_system
                    .before(grounded_dynamics_motor_system),
                grounded_dynamics_motor_system
                    .after(TnuaPipelineSystems::Logic)
                    .before(TnuaPipelineSystems::Motors),
            ),
        )
        .add_systems(PostUpdate, (
            landing_braking_friction_system
                .after(crate::als::motion::AlsActualVelocitySet)
                .before(crate::actual_motion::AlsActualMotionSet),
            reset_landing_braking_friction_system
                .after(avian3d::prelude::PhysicsSystems::Last)
                .after(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_stance_noop_and_settings_replacement_follow_source_consumers() {
        use bevy::ecs::system::RunSystemOnce;
        for stance in [Stance::Standing, Stance::Crouching] {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().stance = Some(stance);
            for _ in 0..3 { app.update(); }
            assert_eq!(app.world().get::<Stance>(character), Some(&stance));
            app.world_mut().resource_mut::<Messages<crate::als::transition::StateTransitionEvent>>().clear();
            let capsule = app.world().get::<crate::als::motion::AlsCrouchTransaction>(character).unwrap().collider_half_height;
            app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().stance = Some(stance);
            app.update();
            let stance_events = app.world_mut().run_system_once(|mut events: MessageReader<crate::als::transition::StateTransitionEvent>| {
                events.read().filter(|event| event.dimension == crate::als::transition::Dimension::Stance).count()
            }).unwrap();
            assert_eq!(stance_events, 0, "Source same-value stance does not publish a changed event");
            assert_eq!(app.world().get::<crate::als::motion::AlsCrouchTransaction>(character).unwrap().collider_half_height, capsule);
            let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
            let mut source_settings = app.world().resource::<Assets<AlsMovementParams>>().get(&handle).unwrap().clone();
            source_settings.acceleration.grounded.normal.braking_deceleration.keys = vec![crate::als::params::HermiteKey { time: 0.0, value: 2.0, arrive_tangent: 0.0, leave_tangent: 0.0 }];
            source_settings.acceleration.grounded.responsive.braking_deceleration = source_settings.acceleration.grounded.normal.braking_deceleration.clone();
            let replacement = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(source_settings);
            app.world_mut().entity_mut(character).insert((AlsMovementParamsHandle(replacement), LinearVelocity(Vec3::X * 3.75)));
            for _ in 0..35 { app.update(); }
            app.world_mut().entity_mut(character).insert(LinearVelocity(Vec3::X * 3.75));
            let delta = app.world().resource::<Time>().delta_secs();
            app.update();
            let actual = app.world().get::<LinearVelocity>(character).unwrap().x;
            assert!((actual - (3.75 - 2.0 * delta)).abs() < 1e-4,
                "Source SetMovementSettings refreshes the next actual braking consumer: stance={stance:?}, actual={actual}, delta={delta}");
        }
    }

    #[test]
    fn actual_landing_stance_waits_for_the_next_source_movement_tick() {
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        let mut transform = *app.world().get::<Transform>(character).unwrap();
        transform.translation.y += 0.5;
        app.world_mut().entity_mut(character).insert((Position(transform.translation), transform,
            GlobalTransform::from(transform), LinearVelocity(Vec3::ZERO)));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)));
        app.update();
        assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Air));
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().stance = Some(Stance::Crouching);
        let mut landed = false;
        for _ in 0..80 {
            app.update();
            assert_eq!(app.world().get::<Stance>(character), Some(&Stance::Standing));
            assert_eq!(app.world().get::<crate::als::transition::AlsDesiredStance>(character).unwrap().0, Stance::Crouching);
            if app.world().get::<LocomotionState>(character) == Some(&LocomotionState::Grounded) { landed = true; break; }
        }
        assert!(landed);
        let capsule_height = app.world().get::<crate::als::motion::AlsCrouchTransaction>(character).unwrap().collider_half_height;
        app.update();
        assert_eq!(app.world().get::<Stance>(character), Some(&Stance::Crouching));
        assert!(app.world().get::<crate::als::motion::AlsCrouchTransaction>(character).unwrap().collider_half_height < capsule_height);
    }

    #[test]
    fn actual_get_up_ordinary_landing_blocks_old_velocity_rotation() {
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().rotation_mode_pressed = true;
        app.update();
        app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().rotation_mode_pressed = false;
        assert_eq!(app.world().get::<RotationMode>(character), Some(&RotationMode::VelocityDirection));
        app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
        for _ in 0..45 { app.update(); }
        assert!(!app.world().get::<crate::als::motion::AlsRotationResponseState>(character).unwrap().rotation_towards_last_input_direction_blocked);
        app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().move_dir = Vec2::ZERO;
        app.world_mut().entity_mut(character).insert(crate::get_up::AlsGetUpRequest(false));
        for _ in 0..4 { app.update(); }
        assert!(app.world().get::<crate::get_up::AlsGetUpState>(character).unwrap().pose_ready);
        let mut transform = *app.world().get::<Transform>(character).unwrap();
        transform.translation.y += 0.5;
        app.world_mut().entity_mut(character).insert((Position(transform.translation), transform,
            GlobalTransform::from(transform), LinearVelocity(Vec3::X * 0.2)));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)));
        let mut airborne = false;
        let mut landed = false;
        for _ in 0..80 {
            app.update();
            let mode = *app.world().get::<LocomotionState>(character).unwrap();
            airborne |= mode == LocomotionState::Air;
            if airborne && mode == LocomotionState::Grounded { landed = true; break; }
        }
        assert!(landed);
        assert_eq!(app.world().get::<crate::als::state::AlsNotifyLocomotionAction>(character), Some(&crate::als::state::AlsNotifyLocomotionAction::GettingUp));
        let mut ended = false;
        for _ in 0..80 {
            app.update();
            if app.world().get::<crate::als::state::AlsNotifyLocomotionAction>(character) == Some(&crate::als::state::AlsNotifyLocomotionAction::None) { ended = true; break; }
        }
        assert!(ended);
        assert_eq!(app.world().get::<RotationMode>(character), Some(&RotationMode::VelocityDirection));
        let before = app.world().get::<Transform>(character).unwrap().rotation;
        app.world_mut().entity_mut(character).insert(LinearVelocity(Vec3::X * 3.75));
        app.update();
        let after = app.world().get::<Transform>(character).unwrap().rotation;
        assert!(after.abs_diff_eq(before, 1e-5),
            "Source ordinary landing block must survive GetUp until new input; before={before:?}, actual={after:?}");
    }

    #[test]
    fn landing_automatic_action_condition_preserves_the_existing_timer() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        for saved_vertical_speed in [-6.999_f32, -7.0, -7.001] {
            let original = AlsLandingBrakingFriction {
                factor: 0.5, remaining_seconds: 0.25,
                previous_locomotion: LocomotionState::Air,
            };
            let character = world.spawn((AlsCharacter, LocomotionState::Grounded,
                AlsActualMotion { has_input: false, world_velocity: Vec3::Y * saved_vertical_speed, ..default() },
                original)).id();
            world.run_system_once(landing_braking_friction_system).unwrap();
            let actual = world.get::<AlsLandingBrakingFriction>(character).unwrap();
            if saved_vertical_speed <= -7.0 {
                assert_eq!(actual.factor, original.factor);
                assert_eq!(actual.remaining_seconds, original.remaining_seconds);
            } else {
                assert_eq!(actual.factor, 3.0);
                assert_eq!(actual.remaining_seconds, 0.5);
            }
            assert_eq!(actual.previous_locomotion, LocomotionState::Grounded);
            world.despawn(character);
        }
    }

    #[test]
    fn actual_rejected_landing_roll_skips_the_ordinary_friction_branch() {
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        for _ in 0..60 { app.update(); }
        assert_eq!(app.world().get::<AlsLandingBrakingFriction>(character).unwrap().factor, 0.0,
            "Previous ordinary landing timer must expire before testing the rejected Roll callback");
        app.world_mut().entity_mut(character).insert(crate::get_up::AlsGetUpRequest(false));
        for _ in 0..4 { app.update(); }
        assert!(app.world().get::<crate::get_up::AlsGetUpState>(character).unwrap().pose_ready);
        let mut transform = *app.world().get::<Transform>(character).unwrap();
        transform.translation.y += 0.5;
        app.world_mut().entity_mut(character).insert((Position(transform.translation), transform,
            GlobalTransform::from(transform), LinearVelocity(Vec3::new(6.5, -8.0, 0.0))));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)));
        let mut airborne = false;
        let mut landed = false;
        let mut saved = Vec3::ZERO;
        for _ in 0..40 {
            let previous = app.world().get::<AlsActualMotion>(character).unwrap().world_velocity;
            app.update();
            let mode = *app.world().get::<LocomotionState>(character).unwrap();
            airborne |= mode == LocomotionState::Air;
            if airborne && mode == LocomotionState::Grounded {
                saved = previous;
                landed = true;
                break;
            }
        }
        assert!(landed && saved.y <= -7.0 && saved.y > -10.0,
            "Actual GetUp landing must reach the source rejected medium Roll branch: {saved:?}");
        assert_eq!(app.world().get::<crate::als::state::AlsNotifyLocomotionAction>(character),
            Some(&crate::als::state::AlsNotifyLocomotionAction::GettingUp));
        assert!(!app.world().get::<crate::als::roll::AlsRollState>(character).unwrap().active);
        let factor = app.world().get::<AlsLandingBrakingFriction>(character).unwrap().factor;
        let before = app.world().get::<LinearVelocity>(character).unwrap().0.with_y(0.0);
        let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap();
        let params = app.world().resource::<Assets<AlsMovementParams>>().get(handle.id()).unwrap();
        let stance = *app.world().get::<Stance>(character).unwrap();
        let rotation = *app.world().get::<RotationMode>(character).unwrap();
        let thresholds = params.gait_speed_thresholds(stance, rotation, before, app.world().get::<AlsViewYaw>(character).unwrap().0);
        let dynamics = params.grounded_dynamics(stance, rotation, calculate_gait_amount_for_thresholds(before.length(), thresholds));
        let source = simulate_grounded_velocity_with_braking_friction(before, Vec3::ZERO, 3.75,
            dynamics, &params.cmc, 0.0, app.world().resource::<Time>().delta_secs()).0;
        assert!(source.length() > 0.1, "Source baseline must retain measurable momentum at the friction consumer: before={before:?}, source={source:?}");
        app.update();
        let actual = app.world().get::<LinearVelocity>(character).unwrap().0.with_y(0.0);
        assert!(actual.abs_diff_eq(source, 1e-4),
            "Source rejected landing Roll cannot enter ordinary friction: factor={factor}, source={source:?}, actual={actual:?}");
    }

    #[test]
    fn actual_landing_callback_retains_source_input_and_publishes_friction() {
        let release_frame = 6;
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        let mut transform = *app.world().get::<Transform>(character).unwrap();
        transform.translation.y += 0.5;
        app.world_mut().entity_mut(character).insert((Position(transform.translation), transform,
            GlobalTransform::from(transform), LinearVelocity(Vec3::X * 0.2)));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)));
        let mut airborne = false;
        let mut captured = None;
        for frame in 0..80 {
            let saved_input = app.world().get::<AlsActualMotion>(character).unwrap().has_input;
            app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().move_dir =
                if frame < release_frame { Vec2::Y } else { Vec2::ZERO };
            app.update();
            let mode = *app.world().get::<LocomotionState>(character).unwrap();
            airborne |= mode == LocomotionState::Air;
            if airborne && mode == LocomotionState::Grounded {
                if frame == release_frame && saved_input {
                    captured = Some((frame, if saved_input { 0.5 } else { 3.0 }));
                }
                break;
            }
        }
        let (frame, expected_factor) = captured.expect("Actual low landing must reach the held-to-released source callback");
        assert_eq!(app.world().get::<crate::als::state::AlsNotifyLocomotionAction>(character),
            Some(&crate::als::state::AlsNotifyLocomotionAction::None));
        assert!(!app.world().get::<AlsActualMotion>(character).unwrap().has_input);
        let braking = *app.world().get::<AlsLandingBrakingFriction>(character).unwrap();
        let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
        let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap();
        let params = app.world().resource::<Assets<AlsMovementParams>>().get(handle.id()).unwrap();
        assert_eq!(braking.factor, expected_factor, "Landing callback must publish friction from saved input in its creation frame");
        assert_eq!(braking.remaining_seconds, 0.5, "Source pending timer must keep its entire creation-frame delay");
        let horizontal_velocity = velocity.with_y(0.0);
        let thresholds = params.gait_speed_thresholds(Stance::Standing, RotationMode::ViewDirection, horizontal_velocity, app.world().get::<AlsViewYaw>(character).unwrap().0);
        let dynamics = params.grounded_dynamics(Stance::Standing, RotationMode::ViewDirection, calculate_gait_amount_for_thresholds(horizontal_velocity.length(), thresholds));
        let expected_velocity = simulate_grounded_velocity_with_braking_friction(velocity,
            Vec3::ZERO, 3.75, dynamics, &params.cmc, expected_factor, 0.05).0;
        app.update();
        let actual_velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
        assert!(actual_velocity.with_y(0.0).abs_diff_eq(expected_velocity.with_y(0.0), 1e-4),
            "First grounded motor must retain callback friction from saved input: expected={expected_velocity:?}, actual={actual_velocity:?}");

    }

    #[test]
    fn actual_landing_braking_keeps_the_source_timer_window() {
        for delta in [0.05_f32, 0.1, 0.125] {
        for input in [Vec2::ZERO, Vec2::Y] {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            let mut transform = *app.world().get::<Transform>(character).unwrap();
            transform.translation.y += 0.5;
            app.world_mut().entity_mut(character).insert((Position(transform.translation), transform,
                GlobalTransform::from(transform), LinearVelocity(Vec3::X * 0.2)));
            app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().move_dir = input;
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)));
            let mut airborne = false;
            let mut landed = false;
            for _ in 0..80 {
                app.update();
                let mode = *app.world().get::<LocomotionState>(character).unwrap();
                airborne |= mode == LocomotionState::Air;
                if airborne && mode == LocomotionState::Grounded {
                    landed = true;
                    break;
                }
            }
            assert!(landed, "Actual low landing must reach the source ordinary landing callback");
            assert_eq!(app.world().get::<crate::als::state::AlsNotifyLocomotionAction>(character), Some(&crate::als::state::AlsNotifyLocomotionAction::None));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
            let expected = if input == Vec2::ZERO { 3.0 } else { 0.5 };
            let expiry_frame = (0.5_f64 / f64::from(delta)).floor() as usize + 1;
            for frame in 1..=expiry_frame {
                let before = app.world().get::<LinearVelocity>(character).unwrap().0;
                let dynamics = app.world().get::<AlsGroundedDynamics>(character).unwrap().0;
                let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap();
                let params = app.world().resource::<Assets<AlsMovementParams>>().get(handle.id()).unwrap();
                let source_velocity = simulate_grounded_velocity_with_braking_friction(before,
                    Vec3::ZERO, 3.75, dynamics, &params.cmc, expected, delta).0;
                if input != Vec2::ZERO && frame == expiry_frame {
                    app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().move_dir = Vec2::ZERO;
                }
                app.update();
                let braking = app.world().get::<AlsLandingBrakingFriction>(character).unwrap();
                if frame < expiry_frame {
                    assert_eq!(braking.factor, expected);
                } else {
                    assert_eq!(braking.factor, 0.0);
                    if input != Vec2::ZERO {
                        let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
                        assert!(velocity.with_y(0.0).abs_diff_eq(source_velocity.with_y(0.0), 1e-4),
                            "Source landing friction is consumed before the 0.5s timer resets after physics: delta={delta} before={before:?} expected={source_velocity:?} actual={velocity:?}");
                    }
                }
            }
            app.update();
            assert_eq!(app.world().get::<AlsLandingBrakingFriction>(character).unwrap().factor, 0.0);
        }
        }
    }

    #[test]
    fn analog_modifier_uses_the_source_max_acceleration_scalar_boundary() {
        use bevy::ecs::system::RunSystemOnce;
        for (maximum_acceleration, expected, expected_minimum) in [(1e-11_f32, 0.75_f32, true), (2e-10, 1.125, false), (0.0, 0.75, false)] {
            let mut world = World::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_secs_f32(0.05));
            world.insert_resource(time);
            let mut value = AlsMovementParams::default();
            for key in &mut value.acceleration.grounded.normal.acceleration.keys {
                key.value = maximum_acceleration;
                key.arrive_tangent = 0.0;
                key.leave_tangent = 0.0;
            }
            let mut params = Assets::<AlsMovementParams>::default();
            let handle = params.add(value);
            world.insert_resource(params);
            let mut controller = TnuaController::<AlsControlScheme>::default();
            controller.basis.desired_motion = Vec3::X;
            let character = world.spawn((AlsCharacter, LocomotionState::Grounded, Stance::Standing,
                RotationMode::ViewDirection, AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Running),
                AlsGroundedDynamics::default(), AlsGaitAmount::default(), Position(Vec3::Y * 0.92),
                LinearVelocity(Vec3::X * 1.5), AlsMovementParamsHandle(handle), controller,
                TnuaMotor::default(), AlsCmcAdapterRuntime::default())).id();
            world.run_system_once(grounded_dynamics_motor_system).unwrap();
            let actual = 1.5 + world.get::<TnuaMotor>(character).unwrap().lin.boost.x;
            assert!((actual - expected).abs() < 1e-6,
                "Source analog modifier scalar boundary: max_acceleration={maximum_acceleration}, {actual} != {expected}");
            assert_eq!(world.get::<AlsCmcAdapterRuntime>(character).unwrap().min_analog_speed_applied, expected_minimum,
                "Source analog modifier floor diagnostic differs: {maximum_acceleration}");
        }
    }

    #[test]
    fn source_minimum_analog_speed_survives_a_lower_configured_gait_speed() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.05));
        world.insert_resource(time);
        let mut value = AlsMovementParams::default();
        value.speeds.standing.max_walk_speed = 0.1;
        let mut params = Assets::<AlsMovementParams>::default();
        let handle = params.add(value);
        world.insert_resource(params);
        let mut controller = TnuaController::<AlsControlScheme>::default();
        controller.basis.desired_motion = Vec3::X * 0.01;
        let character = world.spawn((AlsCharacter, LocomotionState::Grounded, Stance::Standing,
            RotationMode::ViewDirection, AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Walking),
            AlsGroundedDynamics::default(), AlsGaitAmount::default(), Position(Vec3::Y * 0.92),
            LinearVelocity(Vec3::X * 0.2), AlsMovementParamsHandle(handle), controller,
            TnuaMotor::default(), AlsCmcAdapterRuntime::default())).id();
        world.run_system_once(grounded_dynamics_motor_system).unwrap();
        let actual = 0.2 + world.get::<TnuaMotor>(character).unwrap().lin.boost.x;
        assert!((actual - 0.21).abs() < 1e-6,
            "Source minimum analog floor must avoid braking below 0.25 m/s: {actual}");
        assert!(world.get::<AlsCmcAdapterRuntime>(character).unwrap().min_analog_speed_applied);
        for (max_speed, amount, minimum, expected) in [
            (0.0_f32, 1.0, 0.25, 0.25), (0.1, 0.01, 0.25, 0.25),
            (0.25, 1.0, 0.25, 0.25), (3.75, 0.01, 0.25, 0.25),
            (3.75, 1.0, 0.25, 3.75), (0.1, 1.0, 0.0, 0.1),
        ] {
            assert_eq!(analog_target_speed(max_speed, amount, minimum), expected);
        }
    }

    #[test]
    fn source_nonzero_acceleration_and_physical_normalization_reach_the_actual_motor() {
        #[derive(Resource, Default)]
        struct Samples(Vec<(f32, f32, bool)>);
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)))
            .init_resource::<Samples>()
            .add_systems(Update, (move |mut samples: ResMut<Samples>, q: Query<(&TnuaController<AlsControlScheme>, &LinearVelocity, &TnuaMotor, &AlsCmcAdapterRuntime)>| {
                let (controller, velocity, motor, runtime) = q.get(character).unwrap();
                samples.0.push((controller.basis.desired_motion.x, velocity.x + motor.lin.boost.x, runtime.min_analog_speed_applied));
            }).after(grounded_dynamics_motor_system).before(TnuaPipelineSystems::Motors));
        for (amount, expected) in [(0.0000001_f32, 0.2000001_f32), (0.00000002, 0.15000002)] {
            app.world_mut().entity_mut(character).insert((AlsLandingBrakingFriction::default(),
                LinearVelocity(Vec3::X * 0.2), AlsViewYaw(0.0),
                crate::als::input::AlsInputCommand { move_dir: Vec2::X * amount, ..default() }));
            app.update();
            let observed = *app.world().resource::<Samples>().0.last().unwrap();
            assert!((observed.1 - expected).abs() < 1e-8,
                "Source acceleration must remain nonzero while its direction uses physical tolerance: {amount}, {observed:?} != {expected}");
            assert!((observed.0 - amount).abs() < 1e-12);
            assert!(observed.2, "Source minimum analog speed must be reported for nonzero acceleration");
        }
    }

    #[test]
    fn zero_max_acceleration_brakes_even_when_the_move_command_is_nonzero() {
        let result = grounded_velocity_step(Vec3::X * 0.2, Vec3::X, 3.75,
            GroundedDynamics { acceleration: 0.0, braking_deceleration: 15.0, ground_friction: 5.0 },
            0.25, 0.05);
        assert_eq!(result, Vec3::ZERO, "Source Acceleration.IsZero must select braking");
    }

    #[test]
    fn tiny_nonzero_move_reaches_the_actual_drive_and_motor() {
        #[derive(Resource, Default)]
        struct Samples(Vec<(f32, f32)>);
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)))
            .init_resource::<Samples>()
            .add_systems(Update, (move |mut samples: ResMut<Samples>, q: Query<(&TnuaController<AlsControlScheme>, &LinearVelocity, &TnuaMotor)>| {
                let (controller, velocity, motor) = q.get(character).unwrap();
                samples.0.push((controller.basis.desired_motion.x, velocity.x + motor.lin.boost.x));
            }).after(grounded_dynamics_motor_system).before(TnuaPipelineSystems::Motors));
        app.world_mut().entity_mut(character).insert((
            AlsLandingBrakingFriction::default(), LinearVelocity(Vec3::X * 0.2), AlsViewYaw(0.0),
            crate::als::input::AlsInputCommand { move_dir: Vec2::X * 0.00001, ..default() },
        ));
        app.update();
        let observed = app.world().resource::<Samples>().0[0];
        assert!((observed.1 - 0.20001).abs() < 1e-6,
            "Source nonzero acceleration must avoid braking: {observed:?}");
        assert!((observed.0 - 0.00001).abs() < 1e-9);
        assert!(!app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap().has_input,
            "Source locomotion input validity must remain separate from nonzero acceleration");
    }

    #[test]
    fn gait_changes_take_effect_after_the_current_source_movement_tick() {
        #[derive(Resource, Default)]
        struct Samples(Vec<(Gait, f32)>);
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)))
            .init_resource::<Samples>()
            .add_systems(Update, (move |mut samples: ResMut<Samples>, q: Query<(&AlsMaxAllowedGait, &LinearVelocity, &TnuaMotor)>| {
                let (gait, velocity, motor) = q.get(character).unwrap();
                samples.0.push((gait.0, velocity.x + motor.lin.boost.x));
            }).after(grounded_dynamics_motor_system).before(TnuaPipelineSystems::Motors));
        app.world_mut().entity_mut(character).insert((
            crate::als::gait::AlsDesiredGait(Gait::Walking), AlsMaxAllowedGait(Gait::Running),
            LinearVelocity(Vec3::X * 3.75), AlsViewYaw(0.0),
            AlsLandingBrakingFriction::default(),
            crate::als::input::AlsInputCommand { move_dir: Vec2::X, ..default() },
        ));
        app.update();
        let first = app.world().resource::<Samples>().0[0];
        assert!((first.1 - 3.75).abs() < 1e-5, "The current source tick must keep 3.75 m/s: {first:?}");
        assert_eq!(first.0, Gait::Running, "The current motor must use the previous owner gait");
        assert_eq!(app.world().get::<AlsMaxAllowedGait>(character).unwrap().0, Gait::Walking);
        app.world_mut().get_mut::<LinearVelocity>(character).unwrap().0 = Vec3::X * 3.75;
        app.update();
        let second = app.world().resource::<Samples>().0[1];
        assert_eq!(second.0, Gait::Walking);
        assert!((second.1 - 3.125).abs() < 1e-5, "The next source tick must use walking braking: {second:?}");
    }

    #[test]
    fn landing_braking_reads_retained_character_input_before_the_owner_refresh() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::als::{input::AlsInputCommand, motion::AlsInputOverride};
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.05));
        world.insert_resource(time);
        let mut value = AlsMovementParams::default();
        value.cmc.max_simulation_iterations = 1;
        let mut params = Assets::<AlsMovementParams>::default();
        let handle = params.add(value);
        world.insert_resource(params);
        let velocity = Vec3::X * 3.75;
        let character = world.spawn((
            AlsCharacter, LocomotionState::Grounded, Stance::Standing, RotationMode::ViewDirection,
            AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Walking), AlsGroundedDynamics::default(),
            AlsGaitAmount::default(), Position(Vec3::Y * 0.92), LinearVelocity(velocity),
            AlsMovementParamsHandle(handle), TnuaController::<AlsControlScheme>::default(),
            TnuaMotor::default(), AlsCmcAdapterRuntime::default(),
        )).id();
        for (retained_input, command, override_input, expected_factor) in [
            (true, Vec2::ZERO, None, 0.5_f32),
            (false, Vec2::Y, None, 3.0),
            (true, Vec2::Y * 0.0002, None, 0.5),
            (false, Vec2::Y, Some(Vec2::ZERO), 3.0),
            (true, Vec2::Y, Some(Vec2::ZERO), 0.5),
            (false, Vec2::ZERO, Some(Vec2::Y), 3.0),
            (true, Vec2::Y, None, 0.5),
            (false, Vec2::ZERO, None, 3.0),
        ] {
            world.entity_mut(character).insert((
                AlsActualMotion { has_input: retained_input, ..default() },
                AlsInputCommand { move_dir: command, ..default() },
                AlsLandingBrakingFriction { previous_locomotion: LocomotionState::Air, ..default() },
            ));
            if let Some(move_dir) = override_input {
                world.entity_mut(character).insert(AlsInputOverride(AlsInputCommand { move_dir, ..default() }));
            } else {
                world.entity_mut(character).remove::<AlsInputOverride>();
            }
            world.run_system_once(landing_braking_friction_system).unwrap();
            if retained_input && command == Vec2::ZERO && override_input.is_none() {
                world.run_system_once(grounded_dynamics_motor_system).unwrap();
                let actual = velocity + world.get::<TnuaMotor>(character).unwrap().lin.boost;
                assert!(actual.distance(Vec3::X * 2.775) < 1e-5,
                    "Landing release must keep the source input factor and 2.775 m/s: {actual:?}");
            }
            assert_eq!(world.get::<AlsLandingBrakingFriction>(character).unwrap().factor, expected_factor,
                "Landing input differs: retained={retained_input}, command={command:?}, override={override_input:?}");
        }
    }

    #[test]
    fn airborne_motor_preserves_momentum_while_applying_input_acceleration() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_tnua::{TnuaBasis, TnuaBasisContext, TnuaProximitySensor, TnuaRigidBodyTracker};
        for speed in [0.0, 0.2, 0.25, 3.75, 5.0] {
            for direction in [Vec3::Z, Vec3::X, Vec3::NEG_X, Vec3::ZERO] {
                for amount in [1.0, 0.4, 0.01] {
                    for delta in [1.0 / 120.0, 1.0 / 60.0, 0.02] {
                        let mut world = World::new();
                        let mut time = Time::<()>::default();
                        time.advance_by(std::time::Duration::from_secs_f32(delta));
                        world.insert_resource(time);
                        let params_value = AlsMovementParams::default();
                        let mut assets = Assets::<AlsMovementParams>::default();
                        let handle = assets.add(params_value.clone());
                        world.insert_resource(assets);
                        let velocity = Vec3::X * speed + Vec3::NEG_Y;
                        let input = direction * amount;
                        let mut controller = TnuaController::<AlsControlScheme>::default();
                        controller.basis.desired_motion = input;
                        let max_speed = max_allowed_speed(&params_value, Stance::Standing,
                            RotationMode::ViewDirection, Gait::Running, velocity.with_y(0.0), 0.0);
                        let mut config = crate::als::motion::initial_als_control_scheme_config(&params_value).basis;
                        config.speed = max_speed;
                        config.air_acceleration = if input == Vec3::ZERO { 0.0 } else if speed < 0.25 { 6.0 } else { 3.0 };
                        let mut motor = TnuaMotor::default();
                        controller.basis.apply(&config, &mut controller.basis_memory,
                            &bevy_tnua::builtins::TnuaBuiltinWalkSensors { ground: &TnuaProximitySensor::default(), headroom: None },
                            TnuaBasisContext { frame_duration: delta,
                                tracker: &TnuaRigidBodyTracker { velocity, gravity: Vec3::NEG_Y * 9.81, ..default() },
                                up_direction: Dir3::Y }, &mut motor);
                        let vertical = (motor.lin.boost.y, motor.lin.acceleration.y);
                        let character = world.spawn((AlsCharacter, LocomotionState::Air, Stance::Standing,
                            RotationMode::ViewDirection, AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Running),
                            AlsGroundedDynamics::default(), AlsGaitAmount::default(), Position(Vec3::Y * 5.0),
                            LinearVelocity(velocity), AlsMovementParamsHandle(handle), controller, motor, AlsCmcAdapterRuntime::default())).id();
                        let mut expected = velocity.with_y(0.0);
                        for _ in 0..15 {
                            world.get_mut::<LinearVelocity>(character).unwrap().0 = expected + Vec3::NEG_Y;
                            world.run_system_once(grounded_dynamics_motor_system).unwrap();
                            let motor = world.get::<TnuaMotor>(character).unwrap();
                            let actual = (expected + motor.lin.boost + motor.lin.acceleration * delta).with_y(0.0);
                            if input != Vec3::ZERO {
                                let target_speed = (max_speed * amount).max(0.25);
                                let limit = if expected.length_squared() > target_speed * target_speed * 1.01 {
                                    expected.length() } else { target_speed };
                                let acceleration = if expected.length() < 0.25 { 6.0 } else { 3.0 };
                                expected = (expected + input * acceleration * delta).clamp_length_max(limit);
                            }
                            assert!(actual.distance(expected) < 2e-6,
                                "speed={speed} input={input:?} delta={delta}: {actual:?} != {expected:?}");
                            assert_eq!((motor.lin.boost.y, motor.lin.acceleration.y), vertical);
                            assert_eq!(world.get::<LinearVelocity>(character).unwrap().y, -1.0);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn precollision_motor_sample_keeps_own_velocity_on_moving_support() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_tnua::{
            TnuaBasis, TnuaBasisContext, TnuaProximitySensor, TnuaProximitySensorOutput,
            TnuaRigidBodyTracker,
        };
        for own in [Vec3::NEG_Z * 3.75, Vec3::X * 3.75, Vec3::ZERO] {
            for (linear, angular) in [(Vec3::ZERO, Vec3::ZERO), (Vec3::X * 2.0, Vec3::ZERO),
                (Vec3::ZERO, Vec3::Y * 2.0)] {
                let mut world = World::new();
                let mut time = Time::<()>::default();
                time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
                world.insert_resource(time);
                let mut params = Assets::<AlsMovementParams>::default();
                let handle = params.add(AlsMovementParams::default());
                world.insert_resource(params);
                let support = support_point_velocity(Vec3::ZERO, linear, angular, Vec3::X);
                let platform = world.spawn((Position(Vec3::ZERO), LinearVelocity(linear), AngularVelocity(angular))).id();
                let mut controller = TnuaController::<AlsControlScheme>::default();
                controller.basis.desired_motion = own.normalize_or_zero();
                let sensor = TnuaProximitySensor {
                    output: Some(TnuaProximitySensorOutput {
                        entity: platform, proximity: 0.92, normal: Dir3::Y,
                        entity_linvel: support, entity_angvel: angular,
                    }), ..default()
                };
                controller.basis.apply(&default(), &mut controller.basis_memory,
                    &bevy_tnua::builtins::TnuaBuiltinWalkSensors { ground: &sensor, headroom: None },
                    TnuaBasisContext { frame_duration: 1.0 / 60.0,
                        tracker: &TnuaRigidBodyTracker { velocity: own + support, ..default() },
                        up_direction: Dir3::Y }, &mut TnuaMotor::default());
                assert_eq!(controller.basis_memory.standing_on_entity(), Some(platform));
                let character = world.spawn((
                    AlsCharacter, LocomotionState::Grounded, Stance::Standing, RotationMode::VelocityDirection,
                    AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Running), AlsGroundedDynamics::default(),
                    AlsGaitAmount::default(), Position(Vec3::X + Vec3::Y * 0.92), LinearVelocity(own + support),
                    AlsMovementParamsHandle(handle), controller, TnuaMotor::default(), AlsCmcAdapterRuntime::default(),
                    AlsMovementBaseDynamicsState { entity: Some(platform), world_velocity: own + support,
                        support_velocity: support, relative_velocity: own, ..default() },
                )).id();
                world.run_system_once(grounded_dynamics_motor_system).unwrap();
                let state = world.get::<AlsMovementBaseDynamicsState>(character).unwrap();
                assert_eq!(state.support_velocity, support);
                assert_eq!(state.relative_velocity, own);
                assert_eq!(state.pre_collision_velocity, Some(state.relative_velocity + state.motor_boost),
                    "The producer must publish own pre-collision velocity, not world velocity");
                assert!(state.pre_collision_velocity.unwrap().distance(
                    own + support + world.get::<TnuaMotor>(character).unwrap().lin.boost - support) < 1e-5);
            }
        }
    }

    #[test]
    fn overspeed_turn_reaches_the_actual_motor_without_rewriting_world_velocity() {
        use bevy::ecs::system::RunSystemOnce;
        for stance in [Stance::Standing, Stance::Crouching] {
            for amount in [1.0, 0.4] {
                let mut world = World::new();
                let delta = 1.0 / 60.0;
                let mut time = Time::<()>::default();
                time.advance_by(std::time::Duration::from_secs_f32(delta));
                world.insert_resource(time);
                let mut params = Assets::<AlsMovementParams>::default();
                let handle = params.add(AlsMovementParams::default());
                world.insert_resource(params);
                let mut controller = TnuaController::<AlsControlScheme>::default();
                controller.basis.desired_motion = Vec3::Z * amount;
                let velocity = Vec3::X * 3.75;
                let character = world.spawn((AlsCharacter, LocomotionState::Grounded, stance, RotationMode::ViewDirection,
                    AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Walking), AlsGroundedDynamics::default(),
                    AlsGaitAmount::default(), Position(Vec3::Y * 0.92), LinearVelocity(velocity),
                    AlsMovementParamsHandle(handle.clone()), controller, TnuaMotor::default(), AlsCmcAdapterRuntime::default())).id();
                world.run_system_once(grounded_dynamics_motor_system).unwrap();
                let dynamics = world.get::<AlsGroundedDynamics>(character).unwrap().0;
                let max_speed = max_allowed_speed(world.resource::<Assets<AlsMovementParams>>().get(&handle).unwrap(),
                    stance, RotationMode::ViewDirection, Gait::Walking, velocity, 0.0);
                let braked = Vec3::X * (3.75 - dynamics.braking_deceleration * delta).max(0.0);
                let expected = (braked + Vec3::Z * dynamics.acceleration * amount * delta).clamp_length_max(braked.length());
                assert!(braked.length() > analog_target_speed(max_speed, amount, 0.25));
                let actual = velocity + world.get::<TnuaMotor>(character).unwrap().lin.boost;
                assert!(actual.distance(expected) < 1e-5, "stance={stance:?} amount={amount}: {actual:?} != {expected:?}");
                assert!(actual.z > 0.0);
                assert_eq!(world.get::<LinearVelocity>(character).unwrap().0, velocity);
                assert_eq!(world.get::<AlsMovementBaseDynamicsState>(character).unwrap().pre_collision_velocity, Some(actual));
            }
        }
    }

    #[test]
    fn overspeed_braking_keeps_same_frame_turn_acceleration() {
        let dynamics = GroundedDynamics { acceleration: 20.0, braking_deceleration: 15.0, ground_friction: 4.0 };
        let next = grounded_velocity_step_with_braking_friction(Vec3::X * 3.75, Vec3::Z, 1.5,
            dynamics, 0.25, 0.0, 1.0 / 33.0, 1.0 / 60.0);
        let expected = Vec3::new(3.4842343, 0.0, 0.33183184);
        assert!(next.distance(expected) < 1e-6, "Overspeed input must follow braking: {next:?}");
        for factor in [0.0_f32, 0.5, 3.0] {
            for delta in [1.0_f32 / 120.0, 1.0 / 60.0, 0.02] {
                let braked = (3.75 * (1.0 - 4.0 * factor * delta) - 15.0 * delta).max(0.0);
                for direction in [Vec3::X, Vec3::NEG_X, Vec3::Z] {
                    let expected = (Vec3::X * braked + direction * 20.0 * delta).clamp_length_max(braked);
                    let actual = grounded_velocity_step_with_braking_friction(Vec3::X * 3.75, direction, 1.5,
                        dynamics, 0.25, factor, 1.0 / 33.0, delta);
                    assert!(actual.distance(expected) < 1e-5, "factor={factor} delta={delta} direction={direction:?}: {actual:?} != {expected:?}");
                }
            }
        }
    }

    #[test]
    fn source_speed_tolerance_does_not_brake_below_the_requested_limit() {
        let dynamics = GroundedDynamics { acceleration: 20.0, braking_deceleration: 15.0, ground_friction: 4.0 };
        for speed in [f32::from_bits(3.75_f32.to_bits() + 1), 4.0] {
            let next = grounded_velocity_step(Vec3::X * speed, Vec3::X, 3.75, dynamics, 0.25, 0.05);
            assert!(next.distance(Vec3::X * 3.75) < 0.00001, "speed={speed:?} next={next:?}");
        }
    }

    #[test]
    fn braking_uses_source_stop_thresholds_in_the_actual_motor() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.001));
        world.insert_resource(time);
        let mut params = Assets::<AlsMovementParams>::default();
        let handle = params.add(AlsMovementParams::default());
        world.insert_resource(params);
        let character = world.spawn((
            AlsCharacter, LocomotionState::Grounded, Stance::Standing, RotationMode::ViewDirection,
            AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Walking), AlsGroundedDynamics::default(),
            AlsGaitAmount::default(), Position(Vec3::Y * 0.92), LinearVelocity(Vec3::X * 0.08),
            AlsMovementParamsHandle(handle), TnuaController::<AlsControlScheme>::default(),
            TnuaMotor::default(), AlsCmcAdapterRuntime::default(),
        )).id();
        world.run_system_once(grounded_dynamics_motor_system).unwrap();
        assert_eq!(world.get::<TnuaMotor>(character).unwrap().lin.boost, Vec3::X * -0.08);
        assert_eq!(world.get::<AlsMovementBaseDynamicsState>(character).unwrap().pre_collision_velocity, Some(Vec3::ZERO));

        assert_eq!(simulate_grounded_velocity(Vec3::X * 0.08, Vec3::ZERO, 1.0,
            GroundedDynamics { acceleration: 20.0, braking_deceleration: 15.0, ground_friction: 5.0 },
            &CmcAdapterParams::default(), 0.000001), (Vec3::ZERO, 1));

        let dynamics = GroundedDynamics { acceleration: 0.0, braking_deceleration: 1.0, ground_friction: 0.0 };
        for speed in [0.099, 0.1, 0.101] {
            assert_eq!(grounded_velocity_step(Vec3::X * speed, Vec3::ZERO, 1.0, dynamics, 0.0, 0.002), Vec3::ZERO);
        }
        assert!(grounded_velocity_step(Vec3::X * 0.103, Vec3::ZERO, 1.0, dynamics, 0.0, 0.002).x > 0.1);
        assert_eq!(grounded_velocity_step(Vec3::X * 0.08, Vec3::ZERO, 1.0, dynamics, 0.0, 0.0), Vec3::X * 0.08);
        let no_braking = GroundedDynamics { braking_deceleration: 0.0, ..dynamics };
        assert_eq!(grounded_velocity_step(Vec3::X * 0.08, Vec3::ZERO, 1.0, no_braking, 0.0, 0.002), Vec3::X * 0.08);
        let friction_only = GroundedDynamics { ground_friction: 1.0, ..no_braking };
        assert_eq!(grounded_velocity_step_with_braking_friction(Vec3::X * 0.00005, Vec3::ZERO, 1.0, friction_only, 0.0, 1.0, 1.0 / 33.0, 0.002), Vec3::ZERO);
        assert!(grounded_velocity_step_with_braking_friction(Vec3::X * 0.08, Vec3::ZERO, 1.0, friction_only, 0.0, 1.0, 1.0 / 33.0, 0.002).x > 0.079);
        assert!(grounded_velocity_step(Vec3::X * 0.08, Vec3::ZERO, 1.0, no_braking, 0.0, 0.0000001).x > 0.079);
        assert!(grounded_velocity_step(Vec3::X * 0.103, Vec3::X, 0.05, GroundedDynamics { acceleration: 1.0, ..dynamics }, 0.0, 0.01).distance(Vec3::X * 0.05) < 1e-6);
    }

    #[test]
    fn roll_blend_out_restores_the_grounded_velocity_motor() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.05));
        world.insert_resource(time);
        let mut params = Assets::<AlsMovementParams>::default();
        let handle = params.add(AlsMovementParams::default());
        world.insert_resource(params);
        let character = world.spawn((
            AlsCharacter, LocomotionState::Grounded, Stance::Standing, RotationMode::ViewDirection,
            AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Walking), AlsGroundedDynamics::default(),
            AlsGaitAmount::default(), Position(Vec3::Y * 0.92), LinearVelocity(Vec3::X),
            AlsMovementParamsHandle(handle), TnuaController::<AlsControlScheme>::default(),
            TnuaMotor::default(), AlsCmcAdapterRuntime::default(),
            AlsRollState { active: true, time: 0.8, ..default() },
        )).id();
        world.run_system_once(grounded_dynamics_motor_system).unwrap();
        assert_eq!(world.get::<TnuaMotor>(character).unwrap().lin.boost, Vec3::ZERO);
        world.get_mut::<AlsRollState>(character).unwrap().time = 0.9;
        world.run_system_once(grounded_dynamics_motor_system).unwrap();
        assert!(world.get::<TnuaMotor>(character).unwrap().lin.boost.x < 0.0);
        let dynamics = world.get::<AlsMovementBaseDynamicsState>(character).unwrap();
        assert_eq!(dynamics.pre_collision_velocity, Some(dynamics.world_velocity + dynamics.motor_boost));
    }

    #[test]
    fn max_allowed_speed_consumes_direction_dependent_thresholds() {
        let mut params = AlsMovementParams::default();
        params
            .speeds
            .standing
            .allow_direction_dependent_movement_speed = true;
        params.speeds.standing.max_run_backward_speed = 2.0;
        let speed = max_allowed_speed(
            &params,
            Stance::Standing,
            RotationMode::ViewDirection,
            Gait::Running,
            Vec3::Z,
            0.0,
        );
        println!("[BF101.5] dynamics_backward_run_speed={speed:.6}");
        assert!((speed - 2.0).abs() < 1e-6);
    }

    #[test]
    fn braking_uses_the_source_curve_without_braking_friction() {
        assert_eq!(BRAKING_FRICTION_FACTOR, 0.0);
        let current = Vec3::new(4.0, 0.0, 0.0);
        let next = grounded_velocity_step(
            current,
            Vec3::ZERO,
            6.5,
            GroundedDynamics {
                acceleration: 20.0,
                braking_deceleration: 12.5,
                ground_friction: 4.0,
            },
            0.25,
            0.02,
        );
        assert!((next.x - 3.75).abs() < 1e-6);
    }

    #[test]
    fn landing_braking_friction_uses_source_window_and_updates_prediction() {
        use bevy::ecs::system::RunSystemOnce;
        let mut with_input = AlsLandingBrakingFriction {
            previous_locomotion: LocomotionState::Air,
            ..Default::default()
        };
        update_landing_braking_friction(&mut with_input, LocomotionState::Grounded, true, false);
        let mut without_input = AlsLandingBrakingFriction {
            previous_locomotion: LocomotionState::Air,
            ..Default::default()
        };
        update_landing_braking_friction(&mut without_input, LocomotionState::Grounded, false, false);
        let initial_factor = without_input.factor;
        let mut world = World::new();
        let actor = world.spawn((AlsCharacter, without_input)).id();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(LANDING_BRAKING_FRICTION_DURATION));
        world.insert_resource(time);
        world.run_system_once(advance_landing_braking_timer_system).unwrap();
        world.run_system_once(reset_landing_braking_friction_system).unwrap();
        assert_eq!(world.get::<AlsLandingBrakingFriction>(actor).unwrap().factor, initial_factor);
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.01));
        world.run_system_once(advance_landing_braking_timer_system).unwrap();
        world.run_system_once(reset_landing_braking_friction_system).unwrap();
        without_input = *world.get::<AlsLandingBrakingFriction>(actor).unwrap();
        let params = AlsMovementParams::default();
        let no_friction_distance = predict_grounded_stop_distance_with_braking_friction(
            &params,
            Stance::Standing,
            RotationMode::ViewDirection,
            4.0,
            0.0,
        );
        let landing_friction_distance = predict_grounded_stop_distance_with_braking_friction(
            &params,
            Stance::Standing,
            RotationMode::ViewDirection,
            4.0,
            LANDING_BRAKING_FRICTION_WITHOUT_INPUT,
        );
        let passed = (with_input.factor - LANDING_BRAKING_FRICTION_WITH_INPUT).abs() < 1e-6
            && (initial_factor - LANDING_BRAKING_FRICTION_WITHOUT_INPUT).abs() < 1e-6
            && without_input.factor == BRAKING_FRICTION_FACTOR
            && without_input.remaining_seconds == 0.0
            && landing_friction_distance < no_friction_distance;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.18][VERDICT] input_factor={:.3} no_input_factor={initial_factor:.3} reset_factor={:.3} stop_distance=({no_friction_distance:.3},{landing_friction_distance:.3}) -> {verdict}",
            with_input.factor, without_input.factor,
        );
        assert!(passed);
    }

    #[test]
    fn braking_uses_velocity_relative_to_the_support_base() {
        let support_velocity = Vec3::X * 2.0;
        let world_velocity = support_velocity;
        let relative_velocity = world_velocity - support_velocity;
        let next = grounded_velocity_step(
            relative_velocity,
            Vec3::ZERO,
            6.5,
            GroundedDynamics {
                acceleration: 20.0,
                braking_deceleration: 15.0,
                ground_friction: 5.0,
            },
            0.25,
            0.02,
        );
        assert_eq!(next, Vec3::ZERO);
        assert_eq!(next - relative_velocity, Vec3::ZERO);
    }

    #[test]
    fn support_velocity_includes_angular_velocity_at_the_contact_point() {
        let velocity = support_point_velocity(Vec3::ZERO, Vec3::X * 2.0, Vec3::Y, Vec3::X * 3.0);
        assert!((velocity.x - 2.0).abs() < 1e-6);
        assert!((velocity.z + 3.0).abs() < 1e-6);
    }

    #[test]
    fn friction_rotates_velocity_toward_the_input_direction() {
        let next = grounded_velocity_step(
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::NEG_Z,
            6.5,
            GroundedDynamics {
                acceleration: 1.0,
                braking_deceleration: 10.0,
                ground_friction: 5.0,
            },
            0.25,
            0.02,
        );
        assert!(next.distance(Vec3::new(2.7, 0.0, -0.32)) < 1e-6,
            "Source nonzero acceleration must rotate velocity through friction: {next:?}");
    }

    #[test]
    fn source_braking_step_configuration_and_same_frame_input_are_preserved() {
        let dynamics = GroundedDynamics { acceleration: 20.0, braking_deceleration: 12.5, ground_friction: 4.0 };
        for (configured, expected) in [(0.001, 1.458921), (1.0 / 33.0, 1.30625), (0.2, 0.875)] {
            let cmc = CmcAdapterParams { braking_substep_time: configured, max_simulation_iterations: 1, ..default() };
            let (next, steps) = simulate_grounded_velocity_with_braking_friction(Vec3::X * 3.75,
                Vec3::ZERO, 6.5, dynamics, &cmc, 3.0, 0.05);
            assert!((next.x - expected).abs() < 1e-5, "Source braking setting={configured}: {next:?} != {expected}");
            assert_eq!(steps, 1);
        }
        for (direction, expected) in [(Vec3::X, Vec3::X * 1.5), (Vec3::NEG_X, Vec3::X * 0.30625)] {
            let next = grounded_velocity_step_with_braking_friction(Vec3::X * 3.75, direction, 1.5,
                dynamics, 0.25, 3.0, 1.0 / 33.0, 0.05);
            assert!(next.distance(expected) < 1e-5, "Source post-braking input for {direction:?}: {next:?} != {expected:?}");
        }
        let next = grounded_velocity_step_with_braking_friction(Vec3::X * 3.75, Vec3::Z, 1.5,
            dynamics, 0.25, 3.0, 1.0 / 33.0, 0.05);
        assert!(next.distance(Vec3::new(1.1910513, 0.0, 0.9118096)) < 1e-5);
        for configured in [0.001, 1.0 / 33.0, 0.2] {
            assert_eq!(grounded_velocity_step_with_braking_friction(Vec3::X * 3.75, Vec3::ZERO, 6.5,
                dynamics, 0.25, 0.0, configured, 0.05), Vec3::X * 3.125);
        }
        assert_eq!(grounded_velocity_step_with_braking_friction(Vec3::X * 0.5, Vec3::ZERO, 6.5,
            dynamics, 0.25, 3.0, 1.0 / 33.0, 0.1), Vec3::ZERO);
    }

    #[test]
    fn source_braking_substeps_reach_the_actual_motor_without_sharing_the_cmc_budget() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.05));
        world.insert_resource(time);
        let mut value = AlsMovementParams::default();
        value.cmc.max_simulation_iterations = 1;
        let mut params = Assets::<AlsMovementParams>::default();
        let handle = params.add(value);
        world.insert_resource(params);
        let velocity = Vec3::X * 3.75;
        let character = world.spawn((
            AlsCharacter, LocomotionState::Grounded, Stance::Standing, RotationMode::ViewDirection,
            AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Walking), AlsGroundedDynamics::default(),
            AlsGaitAmount::default(), Position(Vec3::Y * 0.92), LinearVelocity(velocity),
            AlsMovementParamsHandle(handle), TnuaController::<AlsControlScheme>::default(),
            TnuaMotor::default(), AlsCmcAdapterRuntime::default(),
            AlsLandingBrakingFriction { factor: 3.0, ..default() },
        )).id();
        world.run_system_once(grounded_dynamics_motor_system).unwrap();
        let dynamics = world.get::<AlsGroundedDynamics>(character).unwrap().0;
        assert_eq!((dynamics.braking_deceleration, dynamics.ground_friction), (12.5, 4.0));
        let actual = velocity + world.get::<TnuaMotor>(character).unwrap().lin.boost;
        assert!(actual.distance(Vec3::X * 1.30625) < 1e-5,
            "Source independent braking steps must produce 1.30625 m/s, actual {actual:?}");
        assert_eq!(world.get::<AlsCmcAdapterRuntime>(character).unwrap().simulation_substeps, 1);
        assert_eq!(world.get::<LinearVelocity>(character).unwrap().0, velocity);
    }

    #[test]
    fn source_cmc_tail_steps_reach_the_actual_motor() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.075));
        world.insert_resource(time);
        let mut params = Assets::<AlsMovementParams>::default();
        let handle = params.add(AlsMovementParams::default());
        world.insert_resource(params);
        let mut controller = TnuaController::<AlsControlScheme>::default();
        controller.basis.desired_motion = Vec3::Z;
        let velocity = Vec3::X * 1.5;
        let character = world.spawn((
            AlsCharacter, LocomotionState::Grounded, Stance::Standing, RotationMode::ViewDirection,
            AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Sprinting), AlsGroundedDynamics::default(),
            AlsGaitAmount::default(), Position(Vec3::Y * 0.92), LinearVelocity(velocity),
            AlsMovementParamsHandle(handle), controller, TnuaMotor::default(), AlsCmcAdapterRuntime::default(),
        )).id();
        world.run_system_once(grounded_dynamics_motor_system).unwrap();
        let dynamics = world.get::<AlsGroundedDynamics>(character).unwrap().0;
        assert_eq!((dynamics.acceleration, dynamics.ground_friction), (20.0, 5.0));
        let actual = velocity + world.get::<TnuaMotor>(character).unwrap().lin.boost;
        let expected = Vec3::new(0.9902344, 0.0, 1.8872353);
        assert!(actual.distance(expected) < 1e-5, "Source tail-step motor velocity: {actual:?} != {expected:?}");
        assert_eq!(world.get::<AlsCmcAdapterRuntime>(character).unwrap().simulation_substeps, 2);
        assert_eq!(world.get::<LinearVelocity>(character).unwrap().0, velocity);
        assert_eq!(world.get::<AlsMovementBaseDynamicsState>(character).unwrap().pre_collision_velocity, Some(actual));
    }

    #[test]
    fn source_cmc_outer_budget_is_independent_of_braking_settings() {
        for (delta, max_step, iterations, factor, expected_steps) in [
            (0.0, 0.05, 8, 0.0, 0), (0.0000001, 0.05, 8, 0.0, 0),
            (0.000001, 0.05, 8, 0.0, 1), (0.075, 0.05, 8, 0.0, 2),
            (0.075, 0.05, 8, 3.0, 2), (0.075, 0.05, 1, 3.0, 1),
            (0.075, 0.05, 0, 3.0, 0), (0.12, 0.05, 8, 0.0, 3),
            (0.00012, 0.00005, 8, 0.0, 3),
        ] {
            let cmc = CmcAdapterParams { max_simulation_time_step: max_step,
                max_simulation_iterations: iterations, ..default() };
            let dynamics = GroundedDynamics { acceleration: 0.0, braking_deceleration: 0.0, ground_friction: 0.0 };
            let (actual, steps) = simulate_grounded_velocity_with_braking_friction(
                Vec3::X, Vec3::ZERO, 6.5, dynamics, &cmc, factor, delta);
            assert_eq!(steps, expected_steps, "Source outer budget: delta={delta} max={max_step} iterations={iterations} factor={factor}");
            assert_eq!(actual, Vec3::X);
        }
    }

    #[test]
    fn simulation_adapter_uses_source_step_and_iteration_limits() {
        let cmc = CmcAdapterParams::default();
        let (next, substeps) = simulate_grounded_velocity(
            Vec3::ZERO,
            Vec3::new(0.01, 0.0, 0.0),
            3.75,
            GroundedDynamics {
                acceleration: 20.0,
                braking_deceleration: 15.0,
                ground_friction: 5.0,
            },
            &cmc,
            0.12,
        );
        assert_eq!(substeps, 3);
        assert!(next.length() > 0.0);
        assert!((analog_target_speed(3.75, 0.01, cmc.min_analog_walk_speed) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn dynamic_stop_prediction_consumes_the_speed_dependent_braking_curve() {
        let params = AlsMovementParams::default();
        let dynamic_distance = predict_grounded_stop_distance(
            &params,
            Stance::Standing,
            RotationMode::ViewDirection,
            6.25,
        );
        let static_distance = 6.25 * 6.25 / (2.0 * 5.0);
        assert!(dynamic_distance > 0.0);
        assert!(dynamic_distance < static_distance);
    }
}
