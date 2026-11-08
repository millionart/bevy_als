use avian3d::prelude::{AngularVelocity, ColliderOf, LinearVelocity, Position, RigidBody};
use bevy::prelude::*;
use bevy_tnua::{TnuaController, TnuaMotor, TnuaPipelineSystems};

use crate::als::character::{AlsCharacter, AlsCmcAdapterRuntime, AlsMovementParamsHandle};
use crate::als::control_scheme::AlsControlScheme;
use crate::als::gait::{AlsGaitAmount, AlsMaxAllowedGait, calculate_gait_amount_for_velocity};
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

fn source_grounded_normal(vector: Vec3) -> Vec3 {
    let vector = vector.as_dvec3();
    let length_squared = vector.length_squared();
    if length_squared < f64::from(PHYSICAL_VECTOR_MIN_LENGTH_SQUARED) {
        Vec3::ZERO
    } else {
        (vector * length_squared.sqrt().recip()).as_vec3()
    }
}

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

pub(crate) fn support_velocity_from_base(
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
    let speed = f64::from(crate::als::motion::source_horizontal_speed_centimeters(horizontal_velocity)) / 100.0;
    let delta_seconds = delta_seconds.max(0.0);
    if delta_seconds < MIN_MOVEMENT_DELTA_SECONDS { return horizontal_velocity; }
    let acceleration = desired_horizontal.clamp_length_max(1.0) * dynamics.acceleration.max(0.0);
    let has_acceleration = acceleration != Vec3::ZERO;
    let analog_modifier = if has_acceleration && dynamics.acceleration > MAX_ACCELERATION_MIN_FOR_ANALOG_MODIFIER { desired_amount } else { 0.0 };
    let target_speed = analog_target_speed(max_speed, analog_modifier, min_analog_walk_speed);
    let overspeed = horizontal_velocity.length_squared() > target_speed * target_speed * MAX_SPEED_SQUARED_RATIO;
    let acceleration_direction = source_grounded_normal(acceleration);
    let mut next = horizontal_velocity.as_dvec3();
    if !has_acceleration || overspeed {
        let friction = dynamics.ground_friction.max(0.0) * braking_friction_factor.max(0.0);
        let deceleration = dynamics.braking_deceleration.max(0.0);
        if friction > 0.0 || deceleration > 0.0 {
            let reverse_acceleration = -source_grounded_normal(horizontal_velocity).as_dvec3() * f64::from(deceleration);
            let mut remaining = delta_seconds;
            let max_step = braking_substep_time.clamp(1.0 / 75.0, 1.0 / 20.0);
            while remaining >= MIN_MOVEMENT_DELTA_SECONDS {
                let step = if remaining > max_step && friction > 0.0 {
                    max_step.min(remaining * 0.5)
                } else { remaining };
                remaining -= step;
                next += (-next * f64::from(friction) + reverse_acceleration) * f64::from(step);
                if next.as_vec3().dot(horizontal_velocity) <= 0.0 {
                    next = bevy::math::DVec3::ZERO;
                    break;
                }
            }
            if next.as_vec3().length_squared() <= BRAKING_SMALL_VELOCITY_SQUARED
                || deceleration > 0.0 && next.as_vec3().length_squared() <= BRAKE_TO_STOP_VELOCITY.powi(2)
            {
                next = bevy::math::DVec3::ZERO;
            }
        }
        if overspeed && next.as_vec3().length_squared() < target_speed * target_speed
            && acceleration.dot(horizontal_velocity) > 0.0
        {
            next = (source_grounded_normal(horizontal_velocity) * target_speed).as_dvec3();
        }
    }
    let next = if has_acceleration && !overspeed {
        let friction_alpha = (dynamics.ground_friction.max(0.0) * delta_seconds).clamp(0.0, 1.0);
        next - (next - acceleration_direction.as_dvec3() * speed) * f64::from(friction_alpha)
    } else { next };
    if !has_acceleration { return next.as_vec3(); }
    let input_limit = if next.length_squared() > f64::from(target_speed * target_speed * MAX_SPEED_SQUARED_RATIO) {
        f64::from((next * 100.0).length() as f32) / 100.0
    } else { f64::from(target_speed) };
    (next + acceleration.as_dvec3() * f64::from(delta_seconds))
        .clamp_length_max(input_limit).as_vec3()
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
    let (velocity, substeps, _) = simulate_grounded_motion_with_braking_friction(velocity, desired_motion,
        max_speed, dynamics, cmc, braking_friction_factor, delta_seconds);
    (velocity, substeps)
}

pub(crate) fn simulate_grounded_motion_with_braking_friction(
    velocity: Vec3, desired_motion: Vec3, max_speed: f32, dynamics: GroundedDynamics,
    cmc: &CmcAdapterParams, braking_friction_factor: f32, delta_seconds: f32,
) -> (Vec3, u32, Vec3) {
    simulate_grounded_motion_with_feedback(velocity, desired_motion, max_speed, dynamics, cmc,
        braking_friction_factor, delta_seconds, |velocity, step| (velocity, velocity * step))
}

pub(crate) fn simulate_grounded_motion_with_feedback(
    velocity: Vec3, desired_motion: Vec3, max_speed: f32, dynamics: GroundedDynamics,
    cmc: &CmcAdapterParams, braking_friction_factor: f32, delta_seconds: f32,
    mut movement: impl FnMut(Vec3, f32) -> (Vec3, Vec3),
) -> (Vec3, u32, Vec3) {
    let (velocity, steps, displacement, _) = simulate_grounded_motion_until_mode_change(velocity,
        desired_motion, max_speed, dynamics, cmc, braking_friction_factor, delta_seconds,
        |velocity, step| { let (actual, delta) = movement(velocity, step); (actual, delta, true) });
    (velocity, steps, displacement)
}

pub(crate) fn source_movement_time_step(remaining: f32, cmc: &CmcAdapterParams, iteration: u32) -> f32 {
    (if remaining > cmc.max_simulation_time_step && iteration < cmc.max_simulation_iterations {
        cmc.max_simulation_time_step.min(remaining * 0.5)
    } else { remaining }).max(MIN_MOVEMENT_DELTA_SECONDS)
}

pub(crate) fn walking_fall_remaining(remaining: f32, step: f32, desired_distance: f32, applied: Vec3) -> f32 {
    if desired_distance < crate::als::movement_constants::SMALL_MOVEMENT_DISTANCE {
        0.0
    } else {
        remaining + step * (1.0 - (applied.with_y(0.0).length() / desired_distance).min(1.0))
    }
}

pub(crate) fn simulate_grounded_motion_until_mode_change(
    velocity: Vec3, desired_motion: Vec3, max_speed: f32, dynamics: GroundedDynamics,
    cmc: &CmcAdapterParams, braking_friction_factor: f32, delta_seconds: f32,
    mut movement: impl FnMut(Vec3, f32) -> (Vec3, Vec3, bool),
) -> (Vec3, u32, Vec3, f32) {
    let mut remaining = delta_seconds.max(0.0);
    let max_iterations = cmc.max_simulation_iterations;
    let mut current = velocity;
    let mut substeps = 0;
    let mut displacement = Vec3::ZERO;
    while remaining >= MIN_MOVEMENT_DELTA_SECONDS && substeps < max_iterations {
        let step = source_movement_time_step(remaining, cmc, substeps + 1);
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
        let (actual_velocity, applied, continue_walking) = movement(current, step);
        remaining -= step;
        substeps += 1;
        displacement += applied;
        if !continue_walking {
            remaining = walking_fall_remaining(remaining, step, current.length() * step, applied);
            break;
        }
        current = actual_velocity;
        if applied == Vec3::ZERO {
            remaining = 0.0;
            break;
        }
    }
    (current, substeps, displacement, remaining)
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
        let gait_amount = calculate_gait_amount_for_velocity(velocity, thresholds);
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
    mut transitions: MessageReader<crate::als::transition::StateTransitionEvent>,
    mut q: Query<
        (
            Entity,
            &LocomotionState,
            &AlsActualMotion,
            &mut AlsLandingBrakingFriction,
            Option<&mut crate::als::motion::AlsRotationResponseState>,
        ),
        With<AlsCharacter>,
    >,
) {
    let landings = transitions.read().filter(|event|
        event.dimension == crate::als::transition::Dimension::LocomotionState
            && event.from == crate::als::transition::StateValue::LocomotionState(LocomotionState::Air)
            && event.to == crate::als::transition::StateValue::LocomotionState(LocomotionState::Grounded))
        .map(|event| event.entity).collect::<Vec<_>>();
    for (entity, locomotion, motion, mut braking, rotation) in &mut q {
        let mode = if landings.contains(&entity) {
            braking.previous_locomotion = LocomotionState::Air;
            LocomotionState::Grounded
        } else { *locomotion };
        let ordinary_landing = update_landing_braking_friction(
            &mut braking,
            mode,
            motion.has_input,
            motion.world_velocity.y <= -crate::als::roll::ROLL_ON_LAND_SPEED_THRESHOLD,
        );
        braking.previous_locomotion = *locomotion;
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

pub(crate) fn update_landing_braking_friction(
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
    custom_dilations: Query<&crate::als_camera::AlsCameraCustomTimeDilation>,
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
        let delta_seconds = time.delta_secs() * custom_dilations.get(entity).map_or(1.0, |value| value.0);
        cmc_runtime.grounded_movement_delta = None;
        cmc_runtime.grounded_collision_velocity = None;
        let Ok((roll, mantle)) = actions.get(entity) else {
            continue;
        };
        if !matches!(*locomotion, LocomotionState::Grounded | LocomotionState::Air) {
            continue;
        }
        let root_motion_active = roll.is_some_and(AlsRollState::root_motion_active)
            || mantle.is_some_and(|state| state.active);
        let Some(params) = params_assets.get(params_handle.id()) else {
            continue;
        };
        cmc_runtime.controlled_input_max_acceleration = Some(if *locomotion == LocomotionState::Grounded {
            dynamics.0.acceleration
        } else {
            params.acceleration.max_acceleration
        });
        let world_velocity = Vec3::new(velocity.x, 0.0, velocity.z);
        if *locomotion == LocomotionState::Air {
            if root_motion_active { continue; }
            let inherited = if let Some(mut base) = current_base_state {
                if let Some(previous) = base.entity {
                    let (_, support) = support_velocity_from_base(Some(previous),
                        character_contact_point(params, *stance, position.0), &bases, &colliders);
                    let inherited = support - base.support_velocity;
                    *base = default();
                    inherited.with_y(0.0)
                } else { Vec3::ZERO }
            } else { Vec3::ZERO };
            motor.lin.acceleration.x = 0.0;
            motor.lin.acceleration.z = 0.0;
            motor.lin.boost.x = inherited.x;
            motor.lin.boost.z = inherited.z;
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
        let max_speed = max_allowed_speed(
            params, *stance, *rotation_mode, max_allowed_gait.0, current, view_yaw.0,
        );
        cmc_runtime.grounded_max_speed = max_speed;
        if root_motion_active { continue; }
        let thresholds = params.gait_speed_thresholds(*stance, *rotation_mode, current, view_yaw.0);
        gait_amount.0 = calculate_gait_amount_for_velocity(current, thresholds);
        if AlsMovementParams::has_gait_settings(*stance, *rotation_mode) {
            dynamics.0 = params.grounded_dynamics(*stance, *rotation_mode, gait_amount.0);
        }
        let desired_motion = controller.basis.desired_motion;
        let braking_friction_factor = landing_braking
            .get(entity)
            .map_or(BRAKING_FRICTION_FACTOR, |state| state.factor);
        let (next, simulation_substeps, displacement) = simulate_grounded_motion_with_braking_friction(
            current,
            desired_motion,
            max_speed,
            dynamics.0,
            &params.cmc,
            braking_friction_factor,
            delta_seconds,
        );
        cmc_runtime.simulation_delta_seconds = delta_seconds;
        cmc_runtime.simulation_substeps = simulation_substeps;
        cmc_runtime.grounded_movement_delta = Some(displacement + support_velocity * time.delta_secs());
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
        let limit = volume.unwrap_or(&default_volume.0).terminal_velocity.abs();
        velocity.linear_velocity = clamp_falling_velocity(velocity.linear_velocity, gravity, limit);
    }
}

pub(crate) fn clamp_falling_velocity(mut velocity: Vec3, gravity: Vec3, limit: f32) -> Vec3 {
    if let Some(direction) = gravity.try_normalize() {
        let falling = velocity.dot(direction);
        if falling > limit.abs() { velocity += direction * (limit.abs() - falling); }
    }
    velocity
}

pub struct AlsGroundedDynamicsPlugin;

impl Plugin for AlsGroundedDynamicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<crate::als::transition::StateTransitionEvent>().add_systems(
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
    fn grounded_direction_normalization_keeps_source_double_precision() {
        let velocity = Vec3::new(0.985679566860199, 0.0, 4.308531842411867e-8);
        let normal = source_grounded_normal(velocity);
        assert_eq!(normal.z, 4.3711281705327565e-8_f32);
        assert_eq!(source_grounded_normal(Vec3::ZERO), Vec3::ZERO);
        assert_eq!(source_grounded_normal(Vec3::X * 0.5e-6), Vec3::ZERO);
        assert_eq!(source_grounded_normal(Vec3::X * 2e-6), Vec3::X);
        let dynamics = GroundedDynamics { acceleration: 20.0, braking_deceleration: 15.0, ground_friction: 5.0 };
        let actual = grounded_velocity_step_with_braking_friction(
            velocity, Vec3::ZERO, 3.75, dynamics, 0.0, 0.0, 1.0 / 33.0, 1.0 / 30.0,
        );
        assert_eq!(actual.z, 2.1229675795098046e-8_f32);
    }

    #[test]
    fn turning_velocity_retains_double_precision_through_acceleration() {
        let dynamics = GroundedDynamics { acceleration: 20.0, braking_deceleration: 15.0, ground_friction: 5.0 };
        let delta = 0.031250014901161194_f32;
        let actual = grounded_velocity_step_with_braking_friction(
            Vec3::new(1.0, 0.0, 0.75), Vec3::X, 3.75, dynamics, 0.0, 0.0, 0.03030303120613098, delta,
        );
        assert_eq!(actual, Vec3::new(1.6640628576278687, 0.0, 0.6328124403953552));
        let no_turn = grounded_velocity_step_with_braking_friction(
            Vec3::new(1.0, 0.0, 0.75), Vec3::X, 3.75,
            GroundedDynamics { ground_friction: 0.0, ..dynamics }, 0.0, 0.0, 0.03030303120613098, delta,
        );
        assert_eq!(no_turn, Vec3::new(1.625000238418579, 0.0, 0.75));
        let capped = grounded_velocity_step_with_braking_friction(
            Vec3::new(3.0, 0.0, 2.0), Vec3::X, 3.75,
            GroundedDynamics { ground_friction: 0.0, ..dynamics }, 0.0, 0.0, 0.03030303120613098, 0.05,
        );
        assert_eq!(capped, Vec3::new(3.3541018962860107, 0.0, 1.6770509481430054));
    }

    #[test]
    fn grounded_acceleration_preserves_source_double_product() {
        let dynamics = GroundedDynamics { acceleration: 20.0, braking_deceleration: 0.0, ground_friction: 0.0 };
        for (velocity, input, max_speed, delta, expected) in [
            (Vec3::X * 0.666_666_6, Vec3::X, 6.5, 1.0 / 30.0, Vec3::X * 1.333_333_4),
            (Vec3::NEG_X * 0.666_666_6, Vec3::NEG_X, 6.5, 1.0 / 30.0, Vec3::NEG_X * 1.333_333_4),
            (Vec3::X * 0.666_666_6, Vec3::X, 6.5, 0.0, Vec3::X * 0.666_666_6),
            (Vec3::X * 3.75, Vec3::X, 3.75, 1.0 / 30.0, Vec3::X * 3.75),
        ] {
            let actual = grounded_velocity_step_with_braking_friction(velocity, input, max_speed,
                dynamics, 0.0, 0.0, 0.030_303_031, delta);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn grounded_braking_preserves_source_double_product() {
        let dynamics = GroundedDynamics { acceleration: 20.0, braking_deceleration: 15.0, ground_friction: 0.0 };
        for (velocity, delta, expected) in [
            (Vec3::X * 0.985_669_7, 1.0 / 30.0, Vec3::X * 0.485_669_64),
            (Vec3::NEG_X * 0.985_669_7, 1.0 / 30.0, Vec3::NEG_X * 0.485_669_64),
            (Vec3::X * 0.985_669_7, 0.0, Vec3::X * 0.985_669_7),
            (Vec3::X * 0.1, 1.0 / 30.0, Vec3::ZERO),
        ] {
            let actual = grounded_velocity_step_with_braking_friction(velocity, Vec3::ZERO, 6.5,
                dynamics, 0.0, 0.0, 0.030_303_031, delta);
            assert_eq!(actual, expected);
        }
        let substeps = grounded_velocity_step_with_braking_friction(Vec3::X, Vec3::ZERO, 6.5,
            GroundedDynamics { acceleration: 20.0, braking_deceleration: 2.0, ground_friction: 4.0 },
            0.0, 1.0, 0.03, 0.1);
        assert_eq!(substeps, Vec3::X * 0.483_178_26);
    }

    #[test]
    fn grounded_turning_and_overspeed_keep_source_centimeter_speed() {
        for (velocity, friction, max_speed, delta, expected) in [
            (Vec3::new(-3.775_452_4, 0.0, -3.199_061_2), 8.0, 10.0, 0.03125,
                Vec3::new(-0.844_454_65, 0.0, -2.399_295_8)),
            (Vec3::new(-1.800_328_4, 0.0, 2.208_680_6), 8.0, 10.0, 0.03125,
                Vec3::new(0.112_120_055, 0.0, 1.656_510_5)),
            (Vec3::new(-0.246_269_57, 0.0, -3.380_010_8), 8.0, 10.0, 0.03125,
                Vec3::new(1.412_540_4, 0.0, -2.535_008_2)),
            (Vec3::new(4.945_411_7, 0.0, -2.311_210_2), 0.0, 4.0, 0.03125,
                Vec3::new(5.058_210_4, 0.0, -2.052_632_6)),
            (Vec3::new(4.283_014_3, 0.0, -0.256_706_3), 0.0, 4.0, 0.03125,
                Vec3::new(4.285_13, 0.0, -0.218_560_86)),
            (Vec3::new(-3.775_452_4, 0.0, -3.199_061_2), 0.0, 10.0, 0.03125,
                Vec3::new(-3.025_452_4, 0.0, -3.199_061_2)),
            (Vec3::new(-3.775_452_4, 0.0, -3.199_061_2), 8.0, 10.0, 0.0,
                Vec3::new(-3.775_452_4, 0.0, -3.199_061_2)),
        ] {
            let actual = grounded_velocity_step_with_braking_friction(velocity, Vec3::X, max_speed,
                GroundedDynamics { acceleration: 24.0, braking_deceleration: 0.0, ground_friction: friction },
                0.0, 0.0, 0.05, delta);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn grounded_motor_consumes_source_turning_velocity() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        let initial = Vec3::new(1.0, 0.0, 0.75);
        app.world_mut().entity_mut(character).insert((LinearVelocity(initial), LocomotionState::Grounded,
            Stance::Standing, RotationMode::ViewDirection, AlsMaxAllowedGait(Gait::Running),
            TnuaMotor::default(), AlsCmcAdapterRuntime::default(), AlsMovementBaseDynamicsState::default(),
            AlsLandingBrakingFriction::default()));
        app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion = Vec3::X;
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.031250014901161194));
        app.world_mut().run_system_once(grounded_dynamics_motor_system).unwrap();
        let motor = app.world().get::<TnuaMotor>(character).unwrap();
        let planned = initial + motor.lin.boost;
        assert_eq!(planned.with_y(0.0), Vec3::new(1.6640628576278687, 0.0, 0.6328124403953552));
        assert_eq!(app.world().get::<AlsCmcAdapterRuntime>(character).unwrap().simulation_substeps, 1);
    }

    #[test]
    fn actual_walking_displacement_integrates_each_source_velocity_substep() {
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        for _ in 0..35 { app.update(); }
        let before = app.world().get::<Position>(character).unwrap().0;
        app.world_mut().entity_mut(character).insert((LinearVelocity::ZERO,
            crate::als::input::AlsInputCommand { move_dir: Vec2::Y, ..default() }));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
        app.update();
        let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
        let after = app.world().get::<Position>(character).unwrap().0;
        let dynamics = app.world().get::<AlsGroundedDynamics>(character).unwrap().0;
        let runtime = app.world().get::<AlsCmcAdapterRuntime>(character).unwrap();
        assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded));
        assert!((dynamics.acceleration - 20.0).abs() < 1e-5);
        assert_eq!(runtime.simulation_substeps, 2);
        assert!((velocity.with_y(0.0).length() - 2.0).abs() < 1e-4,
            "Source walking tail velocity must remain2m/s: {velocity:?}");
        let source_distance = 1.0 * 0.05 + 2.0 * 0.05;
        let actual_distance = (after - before).with_y(0.0).length();
        assert!((actual_distance - source_distance).abs() < 1e-4,
            "Source PhysWalking moves with each substep velocity: before={before:?} after={after:?} actual={actual_distance} expected={source_distance}");
    }

    #[test]
    fn actual_walking_displacement_keeps_source_tail_velocity_across_step_budgets() {
        for (delta, max_step, iterations, initial, input) in [
            (0.075, 0.05, 8, Vec3::ZERO, Vec2::Y),
            (0.12, 0.05, 8, Vec3::ZERO, Vec2::Y),
            (0.1, 0.05, 1, Vec3::ZERO, Vec2::Y),
            (0.02, 0.005, 8, Vec3::ZERO, Vec2::Y),
            (0.1, 0.05, 8, Vec3::X * 1.5, Vec2::Y),
            (0.1, 0.05, 8, Vec3::NEG_Z * 3.75, Vec2::ZERO),
        ] {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            for _ in 0..35 { app.update(); }
            let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
            let mut settings = app.world().resource::<Assets<AlsMovementParams>>().get(&handle).unwrap().clone();
            settings.cmc.max_simulation_time_step = max_step;
            settings.cmc.max_simulation_iterations = iterations;
            let constant = |value| crate::als::params::HermiteCurve { keys: vec![crate::als::params::HermiteKey {
                time: 0.0, value, arrive_tangent: 0.0, leave_tangent: 0.0 }] };
            settings.acceleration.grounded.normal.acceleration = constant(20.0);
            settings.acceleration.grounded.normal.braking_deceleration = constant(12.5);
            settings.acceleration.grounded.normal.ground_friction = constant(5.0);
            settings.acceleration.grounded.responsive = settings.acceleration.grounded.normal.clone();
            let replacement = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(settings.clone());
            let before = app.world().get::<Position>(character).unwrap().0;
            app.world_mut().entity_mut(character).insert((AlsMovementParamsHandle(replacement), LinearVelocity(initial),
                AlsLandingBrakingFriction::default(), crate::als::input::AlsInputCommand { move_dir: input, ..default() }));
            let delta = std::time::Duration::from_secs_f32(delta).as_secs_f32();
            let mut remaining = delta;
            let mut expected_velocity = initial;
            let mut expected_delta = Vec3::ZERO;
            let mut count = 0;
            while remaining >= MIN_MOVEMENT_DELTA_SECONDS && count < iterations {
                count += 1;
                let step = if remaining > max_step && count < iterations { max_step.min(remaining * 0.5) } else { remaining };
                expected_velocity = grounded_velocity_step_with_braking_friction(expected_velocity,
                    Vec3::new(input.x, 0.0, -input.y), 3.75,
                    GroundedDynamics { acceleration: 20.0, braking_deceleration: 12.5, ground_friction: 5.0 },
                    settings.cmc.min_analog_walk_speed, 0.0, settings.cmc.braking_substep_time, step);
                expected_delta += expected_velocity * step;
                remaining -= step;
            }
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
            app.update();
            let actual_velocity = app.world().get::<LinearVelocity>(character).unwrap().0.with_y(0.0);
            let actual_delta = (app.world().get::<Position>(character).unwrap().0 - before).with_y(0.0);
            assert!(actual_velocity.distance(expected_velocity) < 1e-4,
                "Source tail velocity differs: delta={delta} max_step={max_step} iterations={iterations} initial={initial:?} input={input:?} actual={actual_velocity:?} expected={expected_velocity:?}");
            assert!(actual_delta.distance(expected_delta) < 1e-4,
                "Source per-step walking displacement differs: delta={delta} max_step={max_step} iterations={iterations} initial={initial:?} input={input:?} actual={actual_delta:?} expected={expected_delta:?}");
        }
    }

    #[test]
    fn actual_walking_on_moving_support_integrates_source_relative_substeps() {
        use avian3d::prelude::Collider;
        for (delta, max_step) in [(0.1_f32, 0.05_f32), (0.075, 0.05), (0.02, 0.005)] {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            let platform = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(8.0, 0.5, 8.0),
                Transform::from_xyz(0.0, 2.0, 0.0), LinearVelocity::ZERO)).id();
            app.world_mut().entity_mut(character).insert((Position(Vec3::new(0.0, 3.174, 0.0)),
                Transform::from_xyz(0.0, 3.174, 0.0), LinearVelocity::ZERO));
            for _ in 0..60 { app.update(); }
            app.world_mut().entity_mut(platform).insert(LinearVelocity(Vec3::X * 2.0));
            app.world_mut().entity_mut(character).insert(LinearVelocity(Vec3::X * 2.0));
            for _ in 0..20 { app.update(); }
            let base = app.world().get::<AlsMovementBaseDynamicsState>(character).unwrap();
            assert_eq!(base.entity, Some(platform));
            assert!((base.support_velocity.x - 2.0).abs() < 1e-5);
            let before = app.world().get::<Position>(character).unwrap().0;
            let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
            app.world_mut().resource_mut::<Assets<AlsMovementParams>>().get_mut(&handle).unwrap().cmc.max_simulation_time_step = max_step;
            app.world_mut().entity_mut(character).insert((LinearVelocity(Vec3::X * 2.0),
                crate::als::input::AlsInputCommand { move_dir: Vec2::Y, ..default() }));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
            app.update();
            let after = app.world().get::<Position>(character).unwrap().0;
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded));
            let mut remaining = delta;
            let mut source_velocity = 0.0;
            let mut source_distance = 0.0;
            while remaining >= MIN_MOVEMENT_DELTA_SECONDS {
                let step = if remaining > max_step { max_step.min(remaining * 0.5) } else { remaining };
                source_velocity += 20.0 * step;
                source_distance += source_velocity * step;
                remaining -= step;
            }
            assert!((velocity.z + source_velocity).abs() < 1e-4, "Source moving-support tail velocity differs: {velocity:?}");
            assert!((after.z - before.z + source_distance).abs() < 1e-4,
                "Source walking integrates relative substep movement independently of perpendicular base carry: delta={delta} max_step={max_step} before={before:?} after={after:?} velocity={velocity:?} expected={source_distance}");
        }
    }

    #[test]
    fn actual_walking_forward_sweep_does_not_reach_beyond_source_substep_displacement() {
        use avian3d::prelude::Collider;
        use bevy::ecs::system::RunSystemOnce;
        for gap in [0.18_f32, 0.14] {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            for _ in 0..35 { app.update(); }
            let before = app.world().get::<Position>(character).unwrap().0;
            let wall = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 3.0, 0.1),
                Transform::from_xyz(before.x, 1.5, before.z - 0.35 - gap))).id();
            app.update();
            let before = app.world().get::<Position>(character).unwrap().0;
            app.world_mut().run_system_once(move |spatial: crate::extensions::RagdollSpatialQuery,
                colliders: Query<&Collider>| {
                let capsule = colliders.get(character).unwrap();
                let filter = avian3d::prelude::SpatialQueryFilter::from_excluded_entities([character]);
                let source_hit = spatial.cast_shape(capsule, before, Quat::IDENTITY, Dir3::NEG_Z,
                    &avian3d::prelude::ShapeCastConfig::from_max_distance(0.15), &filter);
                assert_eq!(source_hit.is_some(), gap < 0.15,
                    "The real wall must match source accumulated movement: gap={gap} hit={source_hit:?}");
                let overextended = spatial.cast_shape(capsule, before, Quat::IDENTITY, Dir3::NEG_Z,
                    &avian3d::prelude::ShapeCastConfig::from_max_distance(0.2), &filter).unwrap();
                assert_eq!(overextended.entity, wall);
                assert!((overextended.distance - gap).abs() < 0.002,
                    "The real wall must be beyond source displacement but within the old full-tail sweep: {overextended:?}");
            }).unwrap();
            app.world_mut().entity_mut(character).insert((LinearVelocity::ZERO,
                crate::als::input::AlsInputCommand { move_dir: Vec2::Y, ..default() }));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
            app.update();
            let after = app.world().get::<Position>(character).unwrap().0;
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded));
            if gap > 0.15 {
                assert!((after.z - before.z + 0.15).abs() < 1e-4,
                    "Source walking does not hit a wall.18m away when source substeps only move.15m: before={before:?} after={after:?} velocity={velocity:?}");
                assert!((velocity.z + 2.0).abs() < 1e-4,
                    "A wall beyond source displacement cannot change the source tail velocity: {velocity:?}");
            } else {
                assert!(after.z - before.z >= -gap - 1e-4,
                    "A real wall within source movement must still block the capsule: gap={gap} before={before:?} after={after:?}");
            }
        }
    }

    #[test]
    fn actual_walking_collision_velocity_uses_the_final_source_substep() {
        use avian3d::prelude::Collider;
        use bevy::ecs::system::RunSystemOnce;
        #[derive(Resource, Default)]
        struct LastCollisionInput(Option<Vec3>);
        for delta_ms in [100_u64, 125] {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            for _ in 0..35 { app.update(); }
            let before = app.world().get::<Position>(character).unwrap().0;
            let wall = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 3.0, 0.1),
                Transform::from_xyz(before.x, 1.5, before.z - 0.5))).id();
            app.update();
            let before = app.world().get::<Position>(character).unwrap().0;
            app.world_mut().run_system_once(move |spatial: crate::extensions::RagdollSpatialQuery,
                colliders: Query<&Collider>| {
                let capsule = colliders.get(character).unwrap();
                let filter = avian3d::prelude::SpatialQueryFilter::from_excluded_entities([character]);
                assert!(spatial.cast_shape(capsule, before, Quat::IDENTITY, Dir3::NEG_Z,
                    &avian3d::prelude::ShapeCastConfig::from_max_distance(0.1), &filter).is_none());
                let hit = spatial.cast_shape(capsule, before, Quat::IDENTITY, Dir3::NEG_Z,
                    &avian3d::prelude::ShapeCastConfig::from_max_distance(0.2), &filter).unwrap();
                assert_eq!(hit.entity, wall);
                assert!((before.z - hit.point1.z - 0.45).abs() < 1e-4 && hit.normal1.z > 0.99,
                    "The geometric wall face must leave source capsule clearance0.15m: before={before:?} hit={hit:?}");
                assert!(hit.distance > 0.1 && hit.distance <= 0.15,
                    "The real wall must be reached during the final source substep: {hit:?}");
            }).unwrap();
            let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
            let mut settings = app.world().resource::<Assets<AlsMovementParams>>().get(&handle).unwrap().clone();
            settings.speeds.standing.max_walk_speed = 2.0;
            settings.speeds.standing.max_walk_backward_speed = 2.0;
            settings.speeds.standing.max_run_speed = 2.0;
            settings.speeds.standing.max_run_backward_speed = 2.0;
            settings.speeds.standing.max_sprint_speed = 2.0;
            if delta_ms == 125 { settings.cmc.max_simulation_time_step = 0.03125; }
            let replacement = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(settings);
            app.init_resource::<LastCollisionInput>().add_systems(PostUpdate,
                (move |mut sampled: ResMut<LastCollisionInput>, states: Query<&AlsMovementBaseDynamicsState>| {
                    sampled.0 = states.get(character).ok().and_then(|state| state.pre_collision_velocity);
                }).after(crate::als::motion::AlsActualVelocitySet).before(crate::als::motion::AlsRotationResponseSet));
            app.world_mut().entity_mut(character).insert((AlsMovementParamsHandle(replacement),
                LinearVelocity(Vec3::NEG_Z * 2.0), crate::als::input::AlsInputCommand { move_dir: Vec2::Y, ..default() }));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(delta_ms)));
            app.update();
            let after = app.world().get::<Position>(character).unwrap().0;
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            let runtime = app.world().get::<AlsCmcAdapterRuntime>(character).unwrap();
            assert_eq!(runtime.simulation_substeps, if delta_ms == 100 { 2 } else { 4 });
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded));
            assert!(before.z - after.z > 0.1 && before.z - after.z < 0.15,
                "The real capsule must complete the first source step and collide during the final step: {before:?} -> {after:?}");
            let source_speed_upper_bound = if delta_ms == 100 { 1.0 } else { 0.2 };
            assert!(velocity.z >= -source_speed_upper_bound - 1e-4,
                "Source final step only has at most0.05m remaining before collision in0.05s; frame-average velocity must not replace the final step: before={before:?} after={after:?} velocity={velocity:?}");
            if delta_ms == 100 {
                assert!(velocity.z <= -0.75,
                    "Source final step advances remaining wall clearance minus bounded pullback; zero velocity cannot replace that actual movement: {velocity:?}");
            }
            if delta_ms == 125 {
                let last_input = app.world().resource::<LastCollisionInput>().0.unwrap();
                assert!(last_input.z >= -1.5,
                    "The final source step must consume prior collision velocity before applying fresh acceleration: {last_input:?}");
            }
        }
    }

    #[test]
    fn actual_walking_without_progress_keeps_source_final_velocity_and_attempt_input() {
        use avian3d::prelude::Collider;
        #[derive(Resource, Default)]
        struct SampledAttempt(Option<Vec3>);
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        for _ in 0..35 { app.update(); }
        let origin = app.world().get::<Position>(character).unwrap().0;
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 3.0, 0.1),
            Transform::from_xyz(origin.x, 1.5, origin.z - 0.5)));
        app.init_resource::<SampledAttempt>().add_systems(PostUpdate,
            (move |mut sample: ResMut<SampledAttempt>, states: Query<&AlsMovementBaseDynamicsState>| {
                sample.0 = states.get(character).ok().and_then(|state| state.pre_collision_velocity);
            }).after(crate::als::motion::AlsActualVelocitySet).before(crate::als::motion::AlsRotationResponseSet));
        app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
        for _ in 0..45 { app.update(); }
        let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
        app.world_mut().resource_mut::<Assets<AlsMovementParams>>().get_mut(&handle).unwrap().cmc.max_simulation_time_step = 0.03125;
        app.world_mut().entity_mut(character).insert(LinearVelocity::ZERO);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(125)));
        let before = app.world().get::<Position>(character).unwrap().0;
        app.update();
        let after = app.world().get::<Position>(character).unwrap().0;
        let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
        let attempt = app.world().resource::<SampledAttempt>().0.unwrap();
        assert!(velocity.with_y(0.0).length() < 0.05 && (after - before).with_y(0.0).length() < 0.002,
            "Source blocked walking must not create motion: before={before:?} after={after:?} velocity={velocity:?}");
        assert!(attempt.z.abs() <= 0.625 + 1e-3,
            "Source no-progress exit keeps the first source acceleration attempt: {attempt:?}");
    }

    #[test]
    fn actual_turning_walk_checks_each_source_segment_before_aggregate_chord() {
        use avian3d::prelude::{Collider, ShapeCastConfig, SpatialQueryFilter};
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        for _ in 0..35 { app.update(); }
        let origin = app.world().get::<Position>(character).unwrap().0;
        let initial_velocity = Vec3::X * 1.5;
        let first_velocity = initial_velocity - (initial_velocity - Vec3::NEG_Z * initial_velocity.length()) * 0.25 + Vec3::NEG_Z;
        let first = first_velocity * 0.05;
        let next_velocity = first_velocity - (first_velocity - Vec3::NEG_Z * first_velocity.length()) * 0.25 + Vec3::NEG_Z;
        let chord = first + next_velocity * 0.05;
        let away = Vec3::new(-chord.z, 0.0, chord.x).normalize();
        let center = origin + first + away * 0.3;
        let obstacle = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.004, 3.0, 0.004),
            Transform::from_xyz(center.x, 1.5, center.z))).id();
        app.update();
        let origin = app.world().get::<Position>(character).unwrap().0;
        let source_first_velocity = app.world_mut().run_system_once(move |spatial: crate::extensions::RagdollSpatialQuery,
            colliders: Query<&Collider>| {
            let capsule = colliders.get(character).unwrap();
            let filter = SpatialQueryFilter::from_excluded_entities([character]);
            let (first_direction, first_distance) = Dir3::new_and_length(first).unwrap();
            assert!(spatial.shape_intersections(capsule, origin, Quat::IDENTITY, &filter).is_empty(),
                "The source first walking sweep must start without penetration");
            let source_hit = spatial.cast_shape(capsule, origin, Quat::IDENTITY, first_direction,
                &ShapeCastConfig { ignore_origin_penetration: true, ..ShapeCastConfig::from_max_distance(first_distance) }, &filter).unwrap();
            assert_eq!(source_hit.entity, obstacle);
            assert!(source_hit.distance > 0.0,
                "Source first segment must hit after a free start: {source_hit:?}");
            let (chord_direction, chord_distance) = Dir3::new_and_length(chord).unwrap();
            assert!(spatial.cast_shape(capsule, origin, Quat::IDENTITY, chord_direction,
                &ShapeCastConfig::from_max_distance(chord_distance), &filter).is_none(),
                "The aggregate chord must miss the source first-segment obstacle");
            let travel = crate::als::motion::movement_sweep_travel(first_distance, source_hit.distance);
            let advance = *first_direction * travel;
            let remaining = first - advance;
            let slide = remaining - source_hit.normal1 * remaining.dot(source_hit.normal1);
            if let Ok((direction, distance)) = Dir3::new_and_length(slide) {
                assert!(spatial.cast_shape(capsule, origin + advance, Quat::IDENTITY, direction,
                    &ShapeCastConfig { ignore_origin_penetration: true, ..ShapeCastConfig::from_max_distance(distance) }, &filter).is_none(),
                    "The source tangent continuation must remain unobstructed");
            }
            let first_actual = (advance + slide).with_y(0.0) / 0.05;
            let second = first_actual - (first_actual - Vec3::NEG_Z * first_actual.length()) * 0.25 + Vec3::NEG_Z;
            let second_delta = second * 0.05;
            let (direction, distance) = Dir3::new_and_length(second_delta).unwrap();
            assert!(spatial.cast_shape(capsule, origin + advance + slide, Quat::IDENTITY, direction,
                &ShapeCastConfig { ignore_origin_penetration: true, ..ShapeCastConfig::from_max_distance(distance) }, &filter).is_none(),
                "The next source step must remain unobstructed after first collision feedback");
            first_actual
        }).unwrap();
        app.world_mut().entity_mut(character).insert((LinearVelocity(Vec3::X * 1.5),
            crate::als::input::AlsInputCommand { move_dir: Vec2::Y, ..default() }));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
        app.update();
        let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
        let source_speed = source_first_velocity.length();
        let source_next = source_first_velocity
            - (source_first_velocity - Vec3::NEG_Z * source_speed) * 0.25 + Vec3::NEG_Z;
        assert!(velocity.distance(source_next) < 0.03,
            "The source first-segment impact must affect subsequent walking velocity: first={source_first_velocity:?} actual={velocity:?} source_next={source_next:?}");
    }

    #[test]
    fn actual_source_walking_stair_contact_consumes_each_segment_once() {
        use avian3d::prelude::{CollisionLayers, ColliderOf, Sensor, SpatialQueryFilter};
        use bevy::ecs::system::RunSystemOnce;
        for (mesh_name, source, bytes, mesh_origin, center, input, gait, speed) in [
            ("SM_Als_Stairs_01", include_str!("../../../../assets/ALSExtras/Environment/Meshes/SM_Als_Stairs_01.gltf"), &include_bytes!("../../../../assets/ALSExtras/Environment/Meshes/SM_Als_Stairs_01.bin")[..],
                Vec3::new(-3.3333, 0.18, -30.0), Vec3::new(0.0016979529, 1.2816019, -30.404602), Vec2::Y, Gait::Walking, 1.75),
            ("SM_Als_Stairs_03", include_str!("../../../../assets/ALSExtras/Environment/Meshes/SM_Als_Stairs_03.gltf"), &include_bytes!("../../../../assets/ALSExtras/Environment/Meshes/SM_Als_Stairs_03.bin")[..],
                Vec3::new(-30.0, 0.36, -30.0), Vec3::new(-26.67559, 13.69611, -42.672752), Vec2::NEG_Y, Gait::Walking, 1.75),
            ("SM_Als_Stairs_03", include_str!("../../../../assets/ALSExtras/Environment/Meshes/SM_Als_Stairs_03.gltf"), &include_bytes!("../../../../assets/ALSExtras/Environment/Meshes/SM_Als_Stairs_03.bin")[..],
                Vec3::new(-30.0, 0.36, -30.0), Vec3::new(-26.67559, 13.69611, -42.672752), Vec2::NEG_Y, Gait::Sprinting, 6.5),
        ] {
            let data: serde_json::Value = serde_json::from_str(source).unwrap();
            let primitive = &data["meshes"][0]["primitives"][0];
            let accessor_bytes = |index: usize, width: usize| {
                let accessor = &data["accessors"][index];
                let view = &data["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
                let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
                assert_eq!(view["byteStride"].as_u64().unwrap_or(width as u64) as usize, width);
                &bytes[offset..offset + accessor["count"].as_u64().unwrap() as usize * width]
            };
            let vertices: Vec<Vec3> = accessor_bytes(primitive["attributes"]["POSITION"].as_u64().unwrap() as usize, 12)
                .chunks_exact(12).map(|point| Vec3::from_array(std::array::from_fn(|i| f32::from_le_bytes(point[i*4..i*4+4].try_into().unwrap())))).collect();
            let triangles: Vec<[u32; 3]> = accessor_bytes(primitive["indices"].as_u64().unwrap() as usize, 2).chunks_exact(6)
                .map(|triangle| std::array::from_fn(|i| u16::from_le_bytes(triangle[i*2..i*2+2].try_into().unwrap()) as u32)).collect();
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            crate::playground_collision::PlaygroundCollisionPlugin.build(&mut app);
            let mut mesh = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList, default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices.iter().map(|vertex| vertex.to_array()).collect::<Vec<_>>());
            mesh.insert_indices(bevy::render::mesh::Indices::U32(triangles.into_iter().flatten().collect()));
            let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
            let source_transform = Transform::from_translation(mesh_origin);
            let stairs = app.world_mut().spawn((crate::extensions::PlaygroundCollisionSource::new(mesh_name), source_transform, GlobalTransform::from(source_transform))).id();
            app.world_mut().spawn((ChildOf(stairs), Mesh3d(mesh), Transform::IDENTITY, GlobalTransform::from(source_transform)));
            app.world_mut().entity_mut(character).insert((Position(center), Transform::from_translation(center), LinearVelocity::ZERO));
            for _ in 0..8 { app.update(); }
            assert!(app.world().entity(stairs).contains::<crate::playground_collision::PlaygroundMeshColliderBuilt>());
            let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
            let cmc = app.world().resource::<Assets<AlsMovementParams>>().get(&handle).unwrap().cmc.clone();
            let center = if gait == Gait::Sprinting {
                let source_cmc = cmc.clone();
                app.world_mut().run_system_once(move |spatial: crate::extensions::RagdollSpatialQuery,
                    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&crate::als::character::AlsCollisionLayerOwnership>)>| {
                    let (floor, _, _) = crate::als::motion::find_ground_floor(&spatial, center, 0.9, 0.3,
                        &source_cmc, LocomotionState::Grounded, &SpatialQueryFilter::from_excluded_entities([character]), character, &candidates).unwrap();
                    center.with_y(floor + (crate::als::movement_constants::MIN_FLOOR_DISTANCE + crate::als::movement_constants::MAX_FLOOR_DISTANCE) * 0.5)
                }).unwrap()
            } else { center };
            app.world_mut().get_mut::<crate::als::gait::AlsDesiredGait>(character).unwrap().0 = gait;
            app.world_mut().get_mut::<crate::als::gait::AlsMaxAllowedGait>(character).unwrap().0 = gait;
            if gait == Gait::Sprinting {
                *app.world_mut().get_mut::<RotationMode>(character).unwrap() = RotationMode::VelocityDirection;
                app.world_mut().entity_mut(character).insert(crate::als::aim::AlsLastFreeRotationMode(RotationMode::VelocityDirection));
            }
            app.world_mut().entity_mut(character).insert((Position(center), Transform::from_translation(center), LinearVelocity(Vec3::NEG_Z * input.y * speed),
                crate::als::input::AlsInputCommand { move_dir: input, ..default() }));
            if gait == Gait::Sprinting {
                for _ in 0..3 { app.update(); }
                assert_eq!(app.world().get::<AlsMaxAllowedGait>(character).unwrap().0, Gait::Sprinting);
                app.world_mut().entity_mut(character).insert((Position(center), Transform::from_translation(center), LinearVelocity(Vec3::NEG_Z * input.y * speed)));
            }
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.07051209)));
            for frame in 0..8 {
                let before = app.world().get::<Position>(character).unwrap().0;
                let cmc = cmc.clone();
                app.world_mut().run_system_once(move |spatial: crate::extensions::RagdollSpatialQuery,
                    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&crate::als::character::AlsCollisionLayerOwnership>)>| {
                    let filter = SpatialQueryFilter::from_excluded_entities([character]);
                    let mut source_center = before;
                    for step in 0..=2 {
                        if step > 0 { source_center += Vec3::NEG_Z * input.y * (speed * 0.07051209 * 0.5); }
                        let (floor, _, entity) = crate::als::motion::find_ground_floor(&spatial, source_center, 0.9, 0.3,
                            &cmc, LocomotionState::Grounded, &filter, character, &candidates)
                            .expect("Source current and following walking steps must keep a valid stair floor");
                        assert_eq!(entity, stairs);
                        let distance = source_center.y - floor;
                        if step > 0 && !(crate::als::movement_constants::MIN_FLOOR_DISTANCE..=crate::als::movement_constants::MAX_FLOOR_DISTANCE).contains(&distance) {
                            source_center.y = floor + (crate::als::movement_constants::MIN_FLOOR_DISTANCE + crate::als::movement_constants::MAX_FLOOR_DISTANCE) * 0.5;
                        }
                    }
                }).unwrap();
                app.update();
                let after = app.world().get::<Position>(character).unwrap().0;
                let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
                let runtime = app.world().get::<AlsCmcAdapterRuntime>(character).unwrap();
                assert!(runtime.simulation_substeps > 1);
                assert!((after - before).with_y(0.0).length() <= speed * 0.07051209 + 0.003,
                    "Source each walking segment advances once: gait={gait:?} frame={frame} before={before:?} after={after:?} velocity={velocity:?} source_tail={:?}", runtime.grounded_collision_velocity);
                assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded),
                    "Source valid stair floors must stay grounded: gait={gait:?} frame={frame} before={before:?} after={after:?} velocity={velocity:?}");
                assert!((velocity.with_y(0.0).length() - speed).abs() < 0.03,
                    "Source walkable stair contact must preserve horizontal speed: gait={gait:?} frame={frame} before={before:?} after={after:?} velocity={velocity:?} source_tail={:?}", runtime.grounded_collision_velocity);
            }
        }
    }

    #[test]
    fn walking_stops_after_an_exactly_unchanged_substep() {
        let dynamics = GroundedDynamics { acceleration: 0.0, braking_deceleration: 0.0, ground_friction: 0.0 };
        for (applied, feedback, expected_steps) in [
            (Vec3::ZERO, Vec3::ZERO, 1),
            (Vec3::ZERO, Vec3::X * 6.25, 1),
            (Vec3::X * 0.001, Vec3::X * 6.25, 2),
        ] {
            let mut calls = 0;
            let (velocity, steps, displacement, remaining) = simulate_grounded_motion_until_mode_change(
                Vec3::X * 6.25, Vec3::X, 6.25, dynamics, &CmcAdapterParams::default(), 0.0, 0.1,
                |_, _| { calls += 1; (feedback, applied, true) });
            assert_eq!(calls, expected_steps);
            assert_eq!(steps, expected_steps);
            assert_eq!(velocity, feedback);
            assert_eq!(displacement, applied * expected_steps as f32);
            assert_eq!(remaining, 0.0);
        }
    }

    #[test]
    fn source_falling_refunds_the_unconsumed_fraction_of_a_shortened_walking_step() {
        let velocity = Vec3::NEG_Z * 6.0;
        let dynamics = GroundedDynamics { acceleration: 0.0, braking_deceleration: 0.0, ground_friction: 0.0 };
        let (actual, steps, displacement, remaining) = simulate_grounded_motion_until_mode_change(
            velocity, Vec3::NEG_Z, 6.0, dynamics, &CmcAdapterParams::default(), 0.0, 0.1,
            |planned, step| (planned * 0.8, planned * (step * 0.8), false));
        assert_eq!(steps, 1);
        assert!(displacement.abs_diff_eq(Vec3::NEG_Z * 0.24, 1e-6));
        assert!((remaining - 0.06).abs() < 1e-6,
            "Source StartFalling refunds the unconsumed20percent of the current50ms step: remaining={remaining}");
        assert!(actual.abs_diff_eq(velocity, 1e-6),
            "Source CheckFall returns before replacing the planned velocity with walking displacement velocity: actual={actual:?} source={velocity:?}");
    }

    #[test]
    fn source_falling_refund_uses_horizontal_actual_distance_and_clamps_overshoot() {
        let dynamics = GroundedDynamics { acceleration: 0.0, braking_deceleration: 0.0, ground_friction: 0.0 };
        for (fraction, vertical, expected) in [(0.0, 0.0, 0.1), (0.8, 0.2, 0.06), (1.0, 0.0, 0.05), (1.2, 0.0, 0.05)] {
            let (_, steps, _, remaining) = simulate_grounded_motion_until_mode_change(
                Vec3::NEG_Z * 6.0, Vec3::NEG_Z, 6.0, dynamics, &CmcAdapterParams::default(), 0.0, 0.1,
                |planned, step| (planned * fraction, planned * (step * fraction) + Vec3::Y * vertical, false));
            assert_eq!(steps, 1);
            assert!((remaining - expected).abs() < 1e-6,
                "Source refund uses horizontal travel and never negative refund: fraction={fraction} vertical={vertical} remaining={remaining} expected={expected}");
        }
        for speed in [0.0, crate::als::movement_constants::SMALL_MOVEMENT_DISTANCE * 10.0] {
            let (_, _, _, remaining) = simulate_grounded_motion_until_mode_change(
                Vec3::NEG_Z * speed, Vec3::ZERO, 0.0, dynamics, &CmcAdapterParams::default(), 0.0, 0.1,
                |_, _| (Vec3::ZERO, Vec3::ZERO, false));
            assert_eq!(remaining, 0.0, "Source near-zero desired movement discards the unused time: speed={speed}");
        }
    }

    #[test]
    fn single_step_refund_matches_recorded_wall_and_unblocked_travel() {
        let step = 1.0 / 30.0;
        let desired = 0.194159105;
        let blocked = walking_fall_remaining(0.0, step, desired, Vec3::X * 0.14146172);
        assert!((blocked - 0.009047115).abs() < 1e-8);
        assert_eq!(walking_fall_remaining(0.0, step, desired, Vec3::X * desired), 0.0);
    }

    #[test]
    fn actual_shortened_walking_step_refunds_time_to_falling() {
        use avian3d::prelude::{Collider, CollisionLayers, ColliderOf, Sensor, SpatialQueryFilter, ShapeCastConfig};
        use bevy::ecs::system::RunSystemOnce;
        let source_input = std::sync::Arc::new(std::sync::Mutex::new(None));
        let source_sample = source_input.clone();
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        let platform = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(8.0, 0.5, 4.0),
            Transform::from_xyz(0.0, 2.0, 2.0))).id();
        let center = Vec3::new(0.0, 3.174, 0.12);
        app.world_mut().entity_mut(character).insert((Position(center), Transform::from_translation(center), LinearVelocity::ZERO));
        for _ in 0..60 { app.update(); }
        assert_eq!(app.world().get::<AlsMovementBaseDynamicsState>(character).unwrap().entity, Some(platform));
        let wall = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 4.0, 0.1),
            Transform::from_xyz(0.0, 3.0, -0.52))).id();
        app.update();
        let origin = app.world().get::<Position>(character).unwrap().0;
        let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
        let cmc = app.world().resource::<Assets<AlsMovementParams>>().get(&handle).unwrap().cmc.clone();
        let source_distance = 6.25 * 0.05;
        let applied_distance = app.world_mut().run_system_once(move |spatial: crate::extensions::RagdollSpatialQuery,
            colliders: Query<&Collider>, candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&crate::als::character::AlsCollisionLayerOwnership>)>| {
            let filter = SpatialQueryFilter::from_excluded_entities([character]);
            assert!(crate::als::motion::find_ground_floor(&spatial, origin, 0.9, 0.3,
                &cmc, LocomotionState::Grounded, &filter, character, &candidates).is_some());
            let hit = spatial.cast_shape(colliders.get(character).unwrap(), origin, Quat::IDENTITY, Dir3::NEG_Z,
                &ShapeCastConfig { ignore_origin_penetration: true, ..ShapeCastConfig::from_max_distance(source_distance) }, &filter).unwrap();
            assert_eq!(hit.entity, wall);
            let applied = crate::als::motion::movement_sweep_travel(source_distance, hit.distance);
            assert!(applied > 0.27 && applied < source_distance,
                "The source first walking sweep must be shortened after passing the perch boundary: applied={applied} hit={hit:?}");
            let end = origin + Vec3::NEG_Z * applied;
            assert!(crate::als::motion::find_ground_floor(&spatial, end, 0.9, 0.3,
                &cmc, LocomotionState::Grounded, &filter, character, &candidates).is_none(),
                "The collision-shortened first source step must lose its floor");
            applied
        }).unwrap();
        app.add_systems(Update, (move |states: Query<&AlsMovementBaseDynamicsState>| {
            *source_sample.lock().unwrap() = states.get(character).unwrap().pre_collision_velocity;
        }).after(TnuaPipelineSystems::Motors));
        app.world_mut().get_mut::<crate::als::gait::AlsDesiredGait>(character).unwrap().0 = Gait::Running;
        app.world_mut().get_mut::<AlsMaxAllowedGait>(character).unwrap().0 = Gait::Running;
        app.world_mut().entity_mut(character).insert((LinearVelocity(Vec3::NEG_Z * 6.5),
            crate::als::input::AlsInputCommand { move_dir: Vec2::Y, ..default() }));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
        app.update();
        let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
        let gravity = app.world().get::<crate::als::character::AlsEffectiveGravity>(character).unwrap().value.y;
        let source_first_velocity = source_input.lock().unwrap().expect("The source walking step must publish its input before the Air snapshot clears based state");
        assert!((source_first_velocity.z + 6.25).abs() < 1e-4,
            "The real first walking source input must match the measured0.3125m sweep: planned={source_first_velocity:?}");
        let source_remaining = 0.05 + 0.05 * (1.0 - applied_distance / source_distance);
        assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Air));
        assert!((velocity.y - gravity * source_remaining).abs() < 0.001,
            "Source shortened Walking step refunds time before Falling gravity: applied={applied_distance} desired={source_distance} remaining={source_remaining} velocity={velocity:?} source_y={}", gravity * source_remaining);
        let params = app.world().resource::<Assets<AlsMovementParams>>().get(&handle).unwrap();
        let maximum_air_acceleration = crate::als::motion::air_acceleration_for_horizontal_speed(params, 0.0)
            .min(params.acceleration.max_acceleration);
        assert!(velocity.z.abs() <= maximum_air_acceleration * source_remaining + 1e-4,
            "The falling tail must consume the wall-deflected source velocity before remaining air control: {velocity:?}");
    }

    #[test]
    fn actual_walking_off_a_ledge_gives_remaining_source_time_to_falling() {
        use avian3d::prelude::{Collider, CollisionLayers, ColliderOf, Sensor, SpatialQueryFilter};
        use bevy::ecs::system::RunSystemOnce;
        #[derive(Resource, Default)]
        struct FirstWalkingSample(Option<(f32, Vec3, GroundedDynamics)>);
        for delta_ms in [100_u64, 50] {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            let platform = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(8.0, 0.5, 4.0),
                Transform::from_xyz(0.0, 2.0, 2.0))).id();
            let center = Vec3::new(0.0, 3.174, 0.12);
            app.world_mut().entity_mut(character).insert((Position(center), Transform::from_translation(center), LinearVelocity::ZERO));
            for _ in 0..60 { app.update(); }
            assert_eq!(app.world().get::<AlsMovementBaseDynamicsState>(character).unwrap().entity, Some(platform));
            let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
            let cmc = app.world().resource::<Assets<AlsMovementParams>>().get(&handle).unwrap().cmc.clone();
            let origin = app.world().get::<Position>(character).unwrap().0;
            app.init_resource::<FirstWalkingSample>().add_systems(Update,
                (move |mut sample: ResMut<FirstWalkingSample>, states: Query<(&AlsMovementBaseDynamicsState, &AlsGroundedDynamics, &crate::als::character::AlsEffectiveGravity)>| {
                    let (base, dynamics, gravity) = states.get(character).unwrap();
                    sample.0 = Some((gravity.value.y, base.relative_velocity, dynamics.0));
                }).after(grounded_dynamics_motor_system).before(TnuaPipelineSystems::Motors));
            app.world_mut().run_system_once(move |spatial: crate::extensions::RagdollSpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&crate::als::character::AlsCollisionLayerOwnership>)>| {
                let filter = SpatialQueryFilter::from_excluded_entities([character]);
                assert!(crate::als::motion::find_ground_floor(&spatial, origin, 0.9, 0.3,
                    &cmc, LocomotionState::Grounded, &filter, character, &candidates).is_some());
                let source_first_end = origin + Vec3::NEG_Z * (5.75 * 0.05);
                assert!(crate::als::motion::find_ground_floor(&spatial, source_first_end, 0.9, 0.3,
                    &cmc, LocomotionState::Grounded, &filter, character, &candidates).is_none(),
                    "The source first walking step must lose the ledge floor before the remaining step");
            }).unwrap();
            app.world_mut().get_mut::<crate::als::gait::AlsDesiredGait>(character).unwrap().0 = Gait::Sprinting;
            app.world_mut().entity_mut(character).insert((LinearVelocity(Vec3::NEG_Z * 6.5),
                crate::als::input::AlsInputCommand { move_dir: Vec2::Y, ..default() }));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(delta_ms)));
            app.update();
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            let gravity = app.world().get::<crate::als::character::AlsEffectiveGravity>(character).unwrap().value.y;
            let sampled = app.world().resource::<FirstWalkingSample>().0.unwrap();
            assert!((sampled.1.z + 6.5).abs() < 1e-4 && sampled.2.braking_deceleration <= 15.0,
                "Source first walking step must retain enough horizontal speed to cross the proved floor boundary: {sampled:?}");
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Air));
            let falling_seconds = if delta_ms == 100 { 0.05 } else { 0.0 };
            assert!((velocity.y - gravity * falling_seconds).abs() < if delta_ms == 100 { 1e-3 } else { 0.01 },
                "Source StartFalling must consume only the remaining50ms after a complete first walking step: velocity={velocity:?} gravity={gravity}");
            if delta_ms == 100 {
                let position = app.world().get::<Position>(character).unwrap().0;
                let source_y = origin.y + gravity * (0.5 * falling_seconds * falling_seconds);
                assert!((position.y - source_y).abs() < 0.003,
                    "Source remaining falling position uses midpoint velocity once: actual={position:?} source_y={source_y} origin={origin:?}");
            }
        }
    }

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
        world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
        for saved_vertical_speed in [-6.999_f32, -7.0, -7.001] {
            let original = AlsLandingBrakingFriction {
                factor: 0.5, remaining_seconds: 0.25,
                previous_locomotion: LocomotionState::Air,
            };
            let character = world.spawn((AlsCharacter, LocomotionState::Grounded,
                AlsActualMotion { has_input: false, world_velocity: Vec3::Y * saved_vertical_speed, ..default() },
                original)).id();
            world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
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
        app.world_mut().entity_mut(character).insert(LinearVelocity(Vec3::X * 3.75));
        let before = app.world().get::<LinearVelocity>(character).unwrap().0.with_y(0.0);
        let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap();
        let params = app.world().resource::<Assets<AlsMovementParams>>().get(handle.id()).unwrap();
        let stance = *app.world().get::<Stance>(character).unwrap();
        let rotation = *app.world().get::<RotationMode>(character).unwrap();
        let thresholds = params.gait_speed_thresholds(stance, rotation, before, app.world().get::<AlsViewYaw>(character).unwrap().0);
        let dynamics = params.grounded_dynamics(stance, rotation, calculate_gait_amount_for_velocity(before, thresholds));
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
        let dynamics = params.grounded_dynamics(Stance::Standing, RotationMode::ViewDirection, calculate_gait_amount_for_velocity(horizontal_velocity, thresholds));
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
                key.value = f64::from(maximum_acceleration);
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
            world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
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
    fn grounded_speed_cap_refreshes_during_root_motion_and_not_in_air() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.05));
        world.insert_resource(time);
        let mut assets = Assets::<AlsMovementParams>::default();
        let handle = assets.add(AlsMovementParams::default());
        world.insert_resource(assets);
        let character = world.spawn((AlsCharacter, LocomotionState::Grounded, Stance::Crouching,
            RotationMode::ViewDirection, AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Running),
            AlsGroundedDynamics::default(), AlsGaitAmount::default(), Position(Vec3::Y),
            LinearVelocity(Vec3::X * 2.0), AlsMovementParamsHandle(handle),
            TnuaController::<AlsControlScheme>::default(), TnuaMotor::default(), AlsCmcAdapterRuntime::default(),
            AlsRollState { active: true, ..default() })).id();
        world.run_system_once(grounded_dynamics_motor_system).unwrap();
        assert!((world.get::<AlsCmcAdapterRuntime>(character).unwrap().grounded_max_speed - 2.0).abs() < 1e-5);
        world.entity_mut(character).remove::<AlsRollState>();
        world.entity_mut(character).insert((LocomotionState::Air, Stance::Standing));
        world.run_system_once(grounded_dynamics_motor_system).unwrap();
        assert!((world.get::<AlsCmcAdapterRuntime>(character).unwrap().grounded_max_speed - 2.0).abs() < 1e-5);
        world.entity_mut(character).insert(LocomotionState::Grounded);
        world.run_system_once(grounded_dynamics_motor_system).unwrap();
        assert!((world.get::<AlsCmcAdapterRuntime>(character).unwrap().grounded_max_speed - 3.75).abs() < 1e-5);
    }

    #[test]
    fn jumping_uses_the_previous_base_when_the_sensor_switches() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_tnua::{TnuaBasis, TnuaBasisContext, TnuaProximitySensor,
            TnuaProximitySensorOutput, TnuaRigidBodyTracker};
        for (switched, prior_support) in [(false, 0.0), (true, 0.0), (true, 0.25)] {
            let mut world = World::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_secs_f32(0.05));
            world.insert_resource(time);
            let params = AlsMovementParams::default();
            let mut assets = Assets::<AlsMovementParams>::default();
            let handle = assets.add(params.clone());
            world.insert_resource(assets);
            let old = world.spawn((Position::default(), LinearVelocity(Vec3::X))).id();
            let new = world.spawn((Position::default(), LinearVelocity(Vec3::NEG_X))).id();
            let selected = if switched { new } else { old };
            let selected_speed = if switched { Vec3::NEG_X } else { Vec3::X };
            let config = crate::als::motion::initial_als_control_scheme_config(&params).basis;
            let mut controller = TnuaController::<AlsControlScheme>::default();
            let sensor = TnuaProximitySensor { output: Some(TnuaProximitySensorOutput {
                entity: selected, proximity: config.float_height, normal: Dir3::Y,
                entity_linvel: selected_speed, entity_angvel: Vec3::ZERO,
            }), ..default() };
            controller.basis.apply(&config, &mut controller.basis_memory,
                &bevy_tnua::builtins::TnuaBuiltinWalkSensors { ground: &sensor, headroom: None },
                TnuaBasisContext { frame_duration: 0.05,
                    tracker: &TnuaRigidBodyTracker { gravity: Vec3::NEG_Y * 9.81, ..default() }, up_direction: Dir3::Y },
                &mut TnuaMotor::default());
            assert_eq!(controller.basis_memory.standing_on_entity(), Some(selected));
            let character = world.spawn((AlsCharacter, LocomotionState::Air, Stance::Standing,
                RotationMode::ViewDirection, AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Running),
                AlsGroundedDynamics::default(), AlsGaitAmount::default(), Position(Vec3::Y),
                LinearVelocity(Vec3::X * prior_support), AlsMovementParamsHandle(handle), controller,
                TnuaMotor::default(), AlsCmcAdapterRuntime::default(), AlsMovementBaseDynamicsState {
                    entity: Some(old), support_velocity: Vec3::X * prior_support, ..default()
                })).id();
            world.run_system_once(grounded_dynamics_motor_system).unwrap();
            let velocity = world.get::<LinearVelocity>(character).unwrap().x + world.get::<TnuaMotor>(character).unwrap().lin.boost.x;
            assert!((velocity - 1.0).abs() < 1e-5, "switched={switched} prior={prior_support} actual={velocity}");
        }
    }

    #[test]
    fn browser_review_movement_jump_frame_keeps_the_current_base_velocity_change() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_tnua::{TnuaBasis, TnuaBasisContext, TnuaProximitySensor,
            TnuaProximitySensorOutput, TnuaRigidBodyTracker};
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
        world.insert_resource(time);
        let params = AlsMovementParams::default();
        let mut assets = Assets::<AlsMovementParams>::default();
        let handle = assets.add(params.clone());
        world.insert_resource(assets);
        let base = world.spawn((Position::default(), LinearVelocity(Vec3::X * 2.0))).id();
        let config = crate::als::motion::initial_als_control_scheme_config(&params).basis;
        let mut controller = TnuaController::<AlsControlScheme>::default();
        let tracker = TnuaRigidBodyTracker { velocity: Vec3::X * 2.0,
            gravity: Vec3::NEG_Y * 9.81, ..default() };
        let mut sensor = TnuaProximitySensor { output: Some(TnuaProximitySensorOutput {
            entity: base, proximity: config.float_height, normal: Dir3::Y,
            entity_linvel: Vec3::X * 2.0, entity_angvel: Vec3::ZERO,
        }), ..default() };
        let mut motor = TnuaMotor::default();
        controller.basis.apply(&config, &mut controller.basis_memory,
            &bevy_tnua::builtins::TnuaBuiltinWalkSensors { ground: &sensor, headroom: None },
            TnuaBasisContext { frame_duration: 1.0 / 60.0, tracker: &tracker, up_direction: Dir3::Y }, &mut motor);
        sensor.output.as_mut().unwrap().entity_linvel = Vec3::NEG_X * 2.0;
        world.get_mut::<LinearVelocity>(base).unwrap().0 = Vec3::NEG_X * 2.0;
        controller.basis.apply(&config, &mut controller.basis_memory,
            &bevy_tnua::builtins::TnuaBuiltinWalkSensors { ground: &sensor, headroom: None },
            TnuaBasisContext { frame_duration: 1.0 / 60.0, tracker: &tracker, up_direction: Dir3::Y }, &mut motor);
        assert!((motor.lin.boost.x + 4.0).abs() < 1e-5, "The real walking basis did not emit the base change");
        let character = world.spawn((AlsCharacter, LocomotionState::Air, Stance::Standing,
            RotationMode::ViewDirection, AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Running),
            AlsGroundedDynamics::default(), AlsGaitAmount::default(), Position(Vec3::Y),
            LinearVelocity(Vec3::X * 2.0), AlsMovementParamsHandle(handle), controller, motor,
            AlsCmcAdapterRuntime::default(), AlsMovementBaseDynamicsState {
                entity: Some(base), support_velocity: Vec3::X * 2.0,
                world_velocity: Vec3::X * 2.0, ..default()
            })).id();
        world.run_system_once(grounded_dynamics_motor_system).unwrap();
        let actual = world.get::<LinearVelocity>(character).unwrap().x + world.get::<TnuaMotor>(character).unwrap().lin.boost.x;
        assert!((actual + 2.0).abs() < 1e-5, "The airborne motor erased the inherited base change: {actual}");
        world.get_mut::<LinearVelocity>(character).unwrap().0 = Vec3::NEG_X * 2.0;
        *world.get_mut::<TnuaMotor>(character).unwrap() = TnuaMotor::default();
        world.run_system_once(grounded_dynamics_motor_system).unwrap();
        assert!(world.get::<TnuaMotor>(character).unwrap().lin.boost.x.abs() < 1e-5,
            "A following airborne update repeated the inherited base change");
    }

    #[test]
    fn airborne_cmc_does_not_inherit_the_basis_slipping_motor() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_tnua::{TnuaBasis, TnuaBasisContext, TnuaProximitySensor,
            TnuaProximitySensorOutput, TnuaRigidBodyTracker};
        let mut world = World::new();
        let delta = 1.0 / 60.0;
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(delta));
        world.insert_resource(time);
        let params = AlsMovementParams::default();
        let mut assets = Assets::<AlsMovementParams>::default();
        let handle = assets.add(params.clone());
        world.insert_resource(assets);
        let surface = world.spawn_empty().id();
        let velocity = Vec3::NEG_Z * 3.0;
        let mut config = crate::als::motion::initial_als_control_scheme_config(&params).basis;
        config.air_acceleration = crate::als::motion::air_acceleration_for_horizontal_speed(&params, 3.0);
        let mut controller = TnuaController::<AlsControlScheme>::default();
        controller.basis.desired_motion = Vec3::NEG_Z;
        let tracker = TnuaRigidBodyTracker { velocity, gravity: Vec3::NEG_Y * 9.81, ..default() };
        let sensor = TnuaProximitySensor { output: Some(TnuaProximitySensorOutput {
            entity: surface, proximity: config.float_height,
            normal: Dir3::new(Vec3::new(0.0, 0.5, 3.0_f32.sqrt() * 0.5)).unwrap(),
            entity_linvel: Vec3::ZERO, entity_angvel: Vec3::ZERO,
        }), ..default() };
        let mut motor = TnuaMotor::default();
        controller.basis.apply(&config, &mut controller.basis_memory,
            &bevy_tnua::builtins::TnuaBuiltinWalkSensors { ground: &sensor, headroom: None },
            TnuaBasisContext { frame_duration: delta, tracker: &tracker, up_direction: Dir3::Y }, &mut motor);
        assert!(motor.lin.boost.with_y(0.0).length() > 1e-4, "The fixture must emit a real slipping boost");
        let character = world.spawn((AlsCharacter, LocomotionState::Air, Stance::Standing,
            RotationMode::ViewDirection, AlsViewYaw::default(), AlsMaxAllowedGait(Gait::Running),
            AlsGroundedDynamics::default(), AlsGaitAmount::default(), Position(Vec3::Y),
            LinearVelocity(velocity), AlsMovementParamsHandle(handle), controller, motor,
            AlsCmcAdapterRuntime::default())).id();
        world.run_system_once(grounded_dynamics_motor_system).unwrap();
        let actual = velocity + world.get::<TnuaMotor>(character).unwrap().lin.boost.with_y(0.0);
        let expected = velocity;
        assert!(actual.distance(expected) < 1e-5,
            "The CMC consumed an unrelated slipping boost: {actual:?} != {expected:?}");
    }

    #[test]
    fn airborne_motor_preserves_the_source_old_velocity_until_the_movement_transaction() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_tnua::{TnuaBasis, TnuaBasisContext, TnuaProximitySensor, TnuaRigidBodyTracker};
        for speed in [0.0, 5.0] {
            for input in [Vec3::X, Vec3::Z * 0.4, Vec3::ZERO] {
                let mut world = World::new();
                let delta = 0.2;
                let mut time = Time::<()>::default();
                time.advance_by(std::time::Duration::from_secs_f32(delta));
                world.insert_resource(time);
                let params_value = AlsMovementParams::default();
                let mut assets = Assets::<AlsMovementParams>::default();
                let handle = assets.add(params_value.clone());
                world.insert_resource(assets);
                let velocity = Vec3::X * speed + Vec3::NEG_Y;
                let mut controller = TnuaController::<AlsControlScheme>::default();
                controller.basis.desired_motion = input;
                let config = crate::als::motion::initial_als_control_scheme_config(&params_value).basis;
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
                    LinearVelocity(velocity), AlsMovementParamsHandle(handle), controller, motor,
                    AlsCmcAdapterRuntime::default())).id();
                world.run_system_once(grounded_dynamics_motor_system).unwrap();
                let motor = world.get::<TnuaMotor>(character).unwrap();
                assert_eq!(motor.lin.boost.with_y(0.0), Vec3::ZERO,
                    "Air control must not replace the source old velocity before movement");
                assert_eq!(motor.lin.acceleration.with_y(0.0), Vec3::ZERO);
                assert_eq!((motor.lin.boost.y, motor.lin.acceleration.y), vertical);
                assert_eq!(world.get::<LinearVelocity>(character).unwrap().0, velocity);
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
