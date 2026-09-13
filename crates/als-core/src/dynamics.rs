use avian3d::prelude::{AngularVelocity, ColliderOf, LinearVelocity, Position};
use bevy::prelude::*;
use bevy_tnua::{TnuaController, TnuaMotor, TnuaPipelineSystems};

use crate::character::{AlsCharacter, AlsCmcAdapterRuntime, AlsMovementParamsHandle};
use crate::control_scheme::AlsControlScheme;
use crate::gait::{AlsGaitAmount, AlsMaxAllowedGait, calculate_gait_amount_for_thresholds};
use crate::generated_movement_constants::MAX_SPEED_SQUARED_RATIO;
use crate::input::AlsInputCommand;
use crate::mantle::AlsMantleState;
use crate::motion::{AlsInputOverride, AlsViewYaw};
use crate::params::{AlsMovementParams, CmcAdapterParams, GroundedDynamics};
use crate::roll::AlsRollState;
use crate::state::{Gait, LocomotionState, RotationMode, Stance};

pub const BRAKING_FRICTION_FACTOR: f32 = 0.0;
pub const LANDING_BRAKING_FRICTION_WITH_INPUT: f32 = 0.5;
pub const LANDING_BRAKING_FRICTION_WITHOUT_INPUT: f32 = 3.0;
pub const LANDING_BRAKING_FRICTION_DURATION: f32 = 0.5;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsGroundedDynamics(pub GroundedDynamics);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsLandingBrakingFriction {
    pub factor: f32,
    pub remaining_seconds: f32,
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
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsMovementBase {
    pub entity: Option<Entity>,
    pub location: Vec3,
    pub rotation: Quat,
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
    let half_height = match stance {
        Stance::Standing => params.crouch.standing_capsule_half_height,
        Stance::Crouching => params.crouch.crouched_capsule_half_height,
    };
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
    params
        .gait_speed_thresholds(stance, rotation_mode, velocity, view_yaw)
        .for_gait(gait)
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
    delta_seconds: f32,
) -> Vec3 {
    let horizontal_velocity = Vec3::new(velocity.x, 0.0, velocity.z);
    let desired_horizontal = Vec3::new(desired_motion.x, 0.0, desired_motion.z);
    let desired_amount = desired_horizontal.length().min(1.0);
    let speed = horizontal_velocity.length();
    let delta_seconds = delta_seconds.max(0.0);
    if desired_amount <= 1e-6 {
        let friction_alpha =
            (dynamics.ground_friction.max(0.0) * braking_friction_factor.max(0.0) * delta_seconds)
                .clamp(0.0, 1.0);
        let frictioned_velocity = horizontal_velocity * (1.0 - friction_alpha);
        let braking_delta = (dynamics.braking_deceleration.max(0.0) * delta_seconds)
            .min(frictioned_velocity.length());
        return frictioned_velocity - frictioned_velocity.normalize_or_zero() * braking_delta;
    }
    let desired_direction = desired_horizontal.normalize_or_zero();
    let friction_alpha = (dynamics.ground_friction.max(0.0) * delta_seconds).clamp(0.0, 1.0);
    let direction_adjusted =
        horizontal_velocity - (horizontal_velocity - desired_direction * speed) * friction_alpha;
    let accelerated = direction_adjusted
        + desired_direction * dynamics.acceleration.max(0.0) * desired_amount * delta_seconds;
    let target_speed = analog_target_speed(max_speed, desired_amount, min_analog_walk_speed);
    if horizontal_velocity.length_squared() > target_speed * target_speed * MAX_SPEED_SQUARED_RATIO
        && accelerated.dot(horizontal_velocity) > 0.0 {
        let braking_delta = (dynamics.braking_deceleration.max(0.0) * delta_seconds).min(speed);
        let minimum_speed = if desired_direction.dot(horizontal_velocity) > 0.0 { target_speed } else { 0.0 };
        return horizontal_velocity.normalize_or_zero() * (speed - braking_delta).max(minimum_speed);
    }
    accelerated.clamp_length_max(target_speed)
}

pub fn analog_target_speed(max_speed: f32, desired_amount: f32, min_analog_walk_speed: f32) -> f32 {
    let max_speed = max_speed.max(0.0);
    (max_speed * desired_amount.clamp(0.0, 1.0)).max(min_analog_walk_speed.max(0.0).min(max_speed))
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
    let braking_step = cmc.braking_substep_time.clamp(1.0 / 75.0, 1.0 / 20.0);
    let max_step = if desired_motion.length_squared() <= 1e-6 && braking_friction_factor > 0.0 {
        cmc.max_simulation_time_step.min(braking_step).max(1e-4)
    } else {
        cmc.max_simulation_time_step.max(1e-4)
    };
    let max_iterations = cmc.max_simulation_iterations.max(1);
    let mut current = velocity;
    let mut substeps = 0;
    while remaining > 1e-6 && substeps < max_iterations {
        let step = if substeps + 1 == max_iterations {
            remaining
        } else {
            remaining.min(max_step)
        };
        current = grounded_velocity_step_with_braking_friction(
            current,
            desired_motion,
            max_speed,
            dynamics,
            cmc.min_analog_walk_speed,
            braking_friction_factor,
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

fn landing_braking_friction_system(
    time: Res<Time>,
    mut q: Query<
        (
            &LocomotionState,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            Option<&AlsRollState>,
            Option<&AlsMantleState>,
            &mut AlsLandingBrakingFriction,
        ),
        With<AlsCharacter>,
    >,
) {
    for (locomotion, input, input_override, roll, mantle, mut braking) in &mut q {
        let effective_input = input_override.map_or(input, |input| &input.0);
        update_landing_braking_friction(
            &mut braking,
            *locomotion,
            effective_input.move_dir.length_squared() > 1e-6,
            roll.is_some_and(|state| state.active) || mantle.is_some_and(|state| state.active),
            time.delta_secs(),
        );
    }
}

fn update_landing_braking_friction(
    braking: &mut AlsLandingBrakingFriction,
    locomotion: LocomotionState,
    has_input: bool,
    suppress: bool,
    delta_seconds: f32,
) {
    let landed = braking.previous_locomotion == LocomotionState::Air
        && locomotion == LocomotionState::Grounded;
    if landed {
        if suppress {
            braking.factor = BRAKING_FRICTION_FACTOR;
            braking.remaining_seconds = 0.0;
        } else {
            braking.factor = if has_input {
                LANDING_BRAKING_FRICTION_WITH_INPUT
            } else {
                LANDING_BRAKING_FRICTION_WITHOUT_INPUT
            };
            braking.remaining_seconds = LANDING_BRAKING_FRICTION_DURATION;
        }
    }
    if braking.remaining_seconds > 0.0 {
        braking.remaining_seconds = (braking.remaining_seconds - delta_seconds.max(0.0)).max(0.0);
        if braking.remaining_seconds <= 0.0 {
            braking.factor = BRAKING_FRICTION_FACTOR;
        }
    }
    braking.previous_locomotion = locomotion;
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
        if *locomotion != LocomotionState::Grounded
            || roll.is_some_and(AlsRollState::root_motion_active)
            || mantle.is_some_and(|state| state.active)
        {
            continue;
        }
        let Some(params) = params_assets.get(params_handle.id()) else {
            continue;
        };
        let world_velocity = Vec3::new(velocity.x, 0.0, velocity.z);
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
        dynamics.0 = params.grounded_dynamics(*stance, *rotation_mode, gait_amount.0);
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
        cmc_runtime.min_analog_speed_applied = desired_motion.length_squared() > 1e-6
            && max_speed * desired_motion.length().min(1.0) < params.cmc.min_analog_walk_speed;
        let motor_boost = next - current;
        motor.lin.boost.x = next.x + support_velocity.x - world_velocity.x;
        motor.lin.boost.z = next.z + support_velocity.z - world_velocity.z;
        let base_state = AlsMovementBaseDynamicsState {
            entity: base_entity,
            world_velocity,
            support_velocity,
            relative_velocity: current,
            motor_boost,
        };
        if let Some(mut current_base_state) = current_base_state {
            *current_base_state = base_state;
        } else {
            commands.entity(entity).insert(base_state);
        }
    }
}

pub struct AlsGroundedDynamicsPlugin;

impl Plugin for AlsGroundedDynamicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                landing_braking_friction_system
                    .after(crate::motion::locomotion_state_sync_system)
                    .before(grounded_dynamics_motor_system),
                grounded_dynamics_motor_system
                    .after(crate::gait::refresh_gait_system)
                    .after(TnuaPipelineSystems::Logic)
                    .before(TnuaPipelineSystems::Motors),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_speed_tolerance_does_not_brake_below_the_requested_limit() {
        let dynamics = GroundedDynamics { acceleration: 20.0, braking_deceleration: 15.0, ground_friction: 4.0 };
        for speed in [f32::from_bits(3.75_f32.to_bits() + 1), 4.0] {
            let next = grounded_velocity_step(Vec3::X * speed, Vec3::X, 3.75, dynamics, 0.25, 0.05);
            assert!(next.distance(Vec3::X * 3.75) < 0.00001, "speed={speed:?} next={next:?}");
        }
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
        let mut with_input = AlsLandingBrakingFriction {
            previous_locomotion: LocomotionState::Air,
            ..Default::default()
        };
        update_landing_braking_friction(
            &mut with_input,
            LocomotionState::Grounded,
            true,
            false,
            0.0,
        );
        let mut without_input = AlsLandingBrakingFriction {
            previous_locomotion: LocomotionState::Air,
            ..Default::default()
        };
        update_landing_braking_friction(
            &mut without_input,
            LocomotionState::Grounded,
            false,
            false,
            0.0,
        );
        let initial_factor = without_input.factor;
        update_landing_braking_friction(
            &mut without_input,
            LocomotionState::Grounded,
            false,
            false,
            LANDING_BRAKING_FRICTION_DURATION,
        );
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
                acceleration: 0.0,
                braking_deceleration: 10.0,
                ground_friction: 5.0,
            },
            0.25,
            0.02,
        );
        assert!(next.x < 3.0);
        assert!(next.z < 0.0);
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
