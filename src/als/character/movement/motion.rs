use avian3d::prelude::{
    AngularVelocity, Collider, ColliderOf, CollisionLayers, CollisionStart, Collisions, ComputedCenterOfMass, ComputedMass, ContactPair, ContactPairFlags, Gravity, GravityScale, LinearVelocity, PhysicsSchedule, PhysicsStepSystems, PhysicsSystems, Position, Rotation,
    Forces, RigidBody, RigidBodyDisabled, Sensor, ShapeCastConfig, ShapeHitData, SpatialQueryFilter, SpeculativeMargin,
    ReadRigidBodyForces, WriteRigidBodyForces,
};
use crate::extensions::RagdollSpatialQuery as SpatialQuery;
use bevy::asset::Assets;
use bevy::prelude::*;
use bevy_tnua::builtins::{
    TnuaBuiltinCrouch, TnuaBuiltinJump, TnuaBuiltinWalk, TnuaBuiltinWalkHeadroom,
};
use bevy_tnua::controller::TnuaConfig;
use bevy_tnua::basis_capabilities::TnuaBasisWithDisplacement;
use bevy_tnua::{TnuaController, TnuaGravity, TnuaMotor, TnuaPipelineSystems, TnuaProximitySensor, TnuaRigidBodyTracker, TnuaSensorsEntities};

use crate::als::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_FLOOR_CLEARANCE,
    ALS_CHARACTER_TNUA_FLOAT_EPSILON, AlsCharacter, AlsCmcAdapterRuntime, AlsCollisionLayerOwnership, AlsEffectiveGravity, AlsMovementParamsHandle,
    als_character_capsule,
};
use crate::als::control_scheme::{AlsControlScheme, AlsControlSchemeActionDiscriminant, AlsControlSchemeConfig};
use crate::als::dynamics::AlsComponentVelocity;
use crate::als::gait::AlsMaxAllowedGait;
use crate::als::movement_constants::{
    EDGE_REJECT_DISTANCE, FLOOR_RETRY_SHRINK_FRACTION, FLOOR_SWEEP_SHRINK_FRACTION,
    MIN_FLOOR_DISTANCE, MAX_FLOOR_DISTANCE, MAX_SLOPE_DEGREES, MAX_STEP_HEIGHT, PULLBACK_MAX_DISTANCE,
    STAND_UP_SWEEP_INFLATION, STAND_UP_MIN_FLOOR_DISTANCE,
    MAX_DEPENETRATION_WITH_GEOMETRY, MAX_DEPENETRATION_WITH_PAWN,
    PENETRATION_PULLBACK_DISTANCE, PENETRATION_OVERLAP_INFLATION,
    PULLBACK_MIN_DISTANCE, PULLBACK_PROPORTION, PULLBACK_TIME_BIAS, SWEEP_EDGE_MIN_RADIUS,
};
use crate::als::input::AlsInputCommand;
use crate::als::mantle::{
    AlsMantleControlSettings, AlsMantleStartTimeSettings, AlsMantleState, AlsMantleTargetBase,
    AlsMantleTargetPermission, MantleDetectionInput, auto_mantle_start_allowed,
    configured_mantle_start_time, detect_mantle, mantle_start_allowed,
    scaled_mantle_capsule_dimensions, yaw_from_direction,
};
use crate::als::orientation::{world_forward_to_yaw, yaw_to_world_forward};
use crate::als::params::AlsMovementParams;
use crate::als::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use crate::als::transition::{
    AlsDesiredStance, AlsPhysicalStanceOverride, AlsStateRequest, Dimension, StateTransitionEvent,
    StateValue,
};

const WALK_CLING_DISTANCE: f32 = ALS_CHARACTER_TNUA_FLOAT_EPSILON;
const AIR_CONTROL: f32 = 0.15;
const AIR_CONTROL_BOOST_MULTIPLIER: f32 = 2.0;
const AIR_CONTROL_BOOST_SPEED: f32 = 0.25;
const IN_AIR_ROTATION_MODE: u8 = include!("../../settings/in_air_rotation_mode.rs");

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsActualVelocity {
    pub world_velocity: Vec3,
    pub horizontal_speed: f32,
}

pub(crate) fn source_horizontal_speed_centimeters(world_velocity: Vec3) -> f32 {
    (Vec2::new(world_velocity.x, world_velocity.z).as_dvec2() * 100.0).length() as f32
}

pub(crate) fn source_horizontal_speed_meters(world_velocity: Vec3) -> f32 {
    (f64::from(source_horizontal_speed_centimeters(world_velocity)) / 100.0) as f32
}

impl AlsActualVelocity {
    fn from_physics_velocity(world_velocity: Vec3) -> Self {
        Self {
            horizontal_speed: source_horizontal_speed_meters(world_velocity),
            world_velocity,
        }
    }
}

fn deg_to_rad(deg: f32) -> f32 {
    deg * std::f32::consts::PI / 180.0
}

pub fn effective_gait(stance: Stance, gait: Gait) -> Gait {
    if stance == Stance::Crouching { Gait::Walking } else { gait }
}

pub fn gait_max_speed(params: &AlsMovementParams, stance: &Stance, gait: &Gait) -> f32 {
    gait_max_speed_for_rotation(params, stance, gait, RotationMode::VelocityDirection)
}

pub fn gait_max_speed_for_rotation(
    params: &AlsMovementParams,
    stance: &Stance,
    gait: &Gait,
    rotation_mode: RotationMode,
) -> f32 {
    gait_max_speed_for_velocity(params, stance, gait, rotation_mode, Vec3::ZERO, 0.0)
}

pub fn gait_max_speed_for_velocity(
    params: &AlsMovementParams,
    stance: &Stance,
    gait: &Gait,
    rotation_mode: RotationMode,
    velocity: Vec3,
    view_yaw: f32,
) -> f32 {
    if !matches!(*gait, Gait::Walking | Gait::Running | Gait::Sprinting) {
        return params.gait_speeds(*stance, rotation_mode).max_run_speed;
    }
    params
        .gait_speed_thresholds(*stance, rotation_mode, velocity, view_yaw)
        .for_gait(*gait)
}

fn capsule_for(params: &AlsMovementParams, stance: Stance) -> Collider {
    let total_half = if stance == Stance::Crouching { params.crouch.actual_crouched_half_height() } else { params.crouch.standing_capsule_half_height };
    let radius = params.crouch.standing_capsule_radius;
    als_character_capsule(total_half, radius)
}

fn turn_angvel(mode: RotationMode) -> f32 {
    match mode {
        RotationMode::VelocityDirection => deg_to_rad(800.0),
        RotationMode::ViewDirection => deg_to_rad(500.0),
        RotationMode::Aiming => deg_to_rad(1000.0),
        _ => 0.0,
    }
}

pub fn air_acceleration_for_horizontal_speed(
    params: &AlsMovementParams,
    horizontal_speed: f32,
) -> f32 {
    let multiplier = if horizontal_speed < AIR_CONTROL_BOOST_SPEED {
        AIR_CONTROL_BOOST_MULTIPLIER
    } else {
        1.0
    };
    params.acceleration.max_acceleration * AIR_CONTROL * multiplier
}

fn falling_input_acceleration(params: &AlsMovementParams, input: Vec3, speed: f32, cached_max: Option<f32>) -> f32 {
    let amount = input.with_y(0.0).length().min(1.0);
    if amount == 0.0 { return 0.0; }
    let multiplier = if speed < AIR_CONTROL_BOOST_SPEED { AIR_CONTROL_BOOST_MULTIPLIER } else { 1.0 };
    let scaled = cached_max.unwrap_or(params.acceleration.max_acceleration).max(0.0) * AIR_CONTROL * multiplier;
    scaled.min(params.acceleration.max_acceleration.max(0.0) / amount)
}

pub fn effective_tnua_gravity(
    global_gravity: Vec3,
    gravity_scale: f32,
    tnua_gravity: Option<Vec3>,
) -> Vec3 {
    global_gravity * gravity_scale + tnua_gravity.unwrap_or(Vec3::ZERO)
}

pub fn apply_als_control_scheme_config(
    config: &mut AlsControlSchemeConfig,
    params: &AlsMovementParams,
    gait: Gait,
    stance: Stance,
    rotation_mode: RotationMode,
    capsule_crouched: bool,
    velocity: Vec3,
    view_yaw: f32,
    gravity_magnitude: f32,
    capsule_scale: f32,
) {
    config.basis.speed =
        gait_max_speed_for_velocity(params, &stance, &gait, rotation_mode, velocity, view_yaw);
    config.basis.float_height = params.crouch.standing_capsule_half_height * capsule_scale
        + ALS_CHARACTER_FLOOR_CLEARANCE
        + ALS_CHARACTER_TNUA_FLOAT_EPSILON;
    config.basis.acceleration = 0.0;
    config.basis.air_acceleration =
        air_acceleration_for_horizontal_speed(params, Vec2::new(velocity.x, velocity.z).length());
    config.basis.coyote_time = params.jump.coyote_time;
    config.basis.free_fall_extra_gravity = 0.0;
    config.basis.headroom = Some(TnuaBuiltinWalkHeadroom {
        distance_to_collider_top: if capsule_crouched {
            params.crouch.actual_crouched_half_height() * capsule_scale
        } else {
            params.crouch.standing_capsule_half_height * capsule_scale
        },
        ..Default::default()
    });
    config.basis.turning_angvel = turn_angvel(rotation_mode);
    config.basis.max_slope = deg_to_rad(MAX_SLOPE_DEGREES);
    config.basis.cling_distance = WALK_CLING_DISTANCE;
    config.basis.spring_strength = 0.0;
    config.basis.spring_dampening = 1.0;
    config.jump.height = if gravity_magnitude > f32::EPSILON {
        params.jump.jump_z_velocity * params.jump.jump_z_velocity / (2.0 * gravity_magnitude)
    } else { 0.0 };
    config.jump.upslope_extra_gravity = 0.0;
    config.jump.takeoff_extra_gravity = 0.0;
    config.jump.takeoff_above_velocity = f32::INFINITY;
    config.jump.fall_extra_gravity = 0.0;
    config.jump.shorten_extra_gravity = 0.0;
    config.jump.peak_prevention_at_upward_velocity = f32::INFINITY;
    config.jump.peak_prevention_extra_gravity = 0.0;
    config.jump.input_buffer_time = params.jump.jump_buffer_time;
    config.crouch.float_offset =
        -(params.crouch.standing_capsule_half_height - params.crouch.actual_crouched_half_height()) * capsule_scale;
}

pub fn initial_als_control_scheme_config(params: &AlsMovementParams) -> AlsControlSchemeConfig {
    let mut config = AlsControlSchemeConfig::default();
    apply_als_control_scheme_config(
        &mut config,
        params,
        Gait::Running,
        Stance::Standing,
        RotationMode::ViewDirection,
        false,
        Vec3::ZERO,
        0.0,
        Gravity::default().0.length(),
        1.0,
    );
    config
}

pub fn view_relative_move_axes(move_dir: Vec2, view_yaw: f32) -> Vec2 {
    let (sin, cos) = view_yaw.sin_cos();
    Vec2::new(
        cos * move_dir.x - sin * move_dir.y,
        sin * move_dir.x + cos * move_dir.y,
    )
}

pub fn view_relative_move_input(move_dir: Vec2, view_yaw: f32) -> Vec3 {
    let mut move_dir = move_dir.as_dvec2();
    let input_length_squared = move_dir.length_squared();
    if input_length_squared > 1.0 { move_dir *= input_length_squared.sqrt().recip(); }
    let source_degrees = (-(view_yaw as f64) - std::f64::consts::FRAC_PI_2).to_degrees() as f32;
    let (sin, cos) = crate::extensions::control_rig_math::rig_sin_cos(source_degrees.to_radians());
    let (sin, cos) = (f64::from(sin), f64::from(cos));
    let mut world = bevy::math::DVec3::new(cos * move_dir.y - sin * move_dir.x, 0.0,
        sin * move_dir.y + cos * move_dir.x);
    let world_length_squared = world.length_squared();
    if world_length_squared > 1.0 { world *= world_length_squared.sqrt().recip(); }
    world.as_vec3()
}

pub fn view_relative_move_direction(move_dir: Vec2, view_yaw: f32) -> Vec3 {
    view_relative_move_input(move_dir, view_yaw).normalize_or_zero()
}

pub fn wrap_yaw_radians(yaw: f32) -> f32 {
    (yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

pub fn constrained_aiming_yaw(
    view_yaw: f32,
    actor_yaw: f32,
    minimum_limit_degrees: f32,
    active_limit_degrees: f32,
    reset: bool,
    delta_seconds: f32,
    apply_secondary_constraint: bool,
) -> (f32, f32) {
    let tolerance = 1e-4_f32.to_radians();
    let minimum_limit = minimum_limit_degrees.clamp(0.0, 180.0).to_radians();
    let mut limit = if reset {
        std::f32::consts::PI
    } else {
        active_limit_degrees
            .clamp(minimum_limit_degrees, 180.0)
            .to_radians()
    };
    let mut view_space_yaw = wrap_yaw_radians(view_yaw - actor_yaw);
    if view_space_yaw.abs() <= minimum_limit + tolerance {
        limit = minimum_limit;
        return (actor_yaw, limit.to_degrees());
    }
    view_space_yaw = counterclockwise_yaw_delta(view_space_yaw);
    if apply_secondary_constraint {
        let target = view_space_yaw.clamp(-minimum_limit, minimum_limit);
        let delta = unwind_aiming_yaw(target - view_space_yaw);
        view_space_yaw = if delta.abs() <= tolerance {
            target
        } else {
            let alpha = 1.0 - crate::als::animation::inv_exp_approx(std::f32::consts::LN_2 / (0.1 + 1e-8) * delta_seconds);
            unwind_aiming_yaw(view_space_yaw + delta * alpha)
        };
    }
    if view_space_yaw.abs() > limit + tolerance {
        view_space_yaw = view_space_yaw.clamp(-limit, limit);
    } else {
        limit = view_space_yaw.abs().max(minimum_limit);
    }
    (wrap_yaw_radians(view_yaw - view_space_yaw), limit.to_degrees())
}

fn unwind_aiming_yaw(angle: f32) -> f32 {
    if angle > std::f32::consts::PI { angle - std::f32::consts::TAU }
    else if angle < -std::f32::consts::PI { angle + std::f32::consts::TAU }
    else { angle }
}

fn counterclockwise_yaw_delta(delta: f32) -> f32 {
    -crate::als::transitions::remap_counterclockwise_degrees(-wrap_yaw_radians(delta).to_degrees()).to_radians()
}

fn damper_yaw(current: f32, target: f32, delta_seconds: f32, half_life: f32) -> f32 {
    let delta = wrap_yaw_radians(target - current);
    if delta.abs() <= 1e-4_f32.to_radians() {
        return target;
    }
    let alpha = 1.0 - crate::als::animation::inv_exp_approx(std::f32::consts::LN_2 / (half_life + 1e-8) * delta_seconds);
    wrap_yaw_radians(current + counterclockwise_yaw_delta(delta) * alpha)
}

fn advance_yaw(current: f32, target: f32, speed_degrees: f32, delta_seconds: f32) -> f32 {
    let delta = wrap_yaw_radians(target - current);
    let max_delta = speed_degrees.to_radians() * delta_seconds;
    if speed_degrees <= 0.0 || delta.abs() <= max_delta {
        return target;
    }
    wrap_yaw_radians(current + counterclockwise_yaw_delta(delta).signum() * max_delta)
}

pub fn locomotion_movement_flags(has_input: bool, horizontal_speed: f32) -> (bool, bool) {
    let (has_speed_threshold, moving_speed_threshold): (f32, f32) =
        include!("../../settings/locomotion_thresholds.rs");
    let has_velocity = horizontal_speed >= has_speed_threshold;
    (
        has_velocity,
        (has_input && has_velocity) || horizontal_speed > moving_speed_threshold,
    )
}

fn moving_rotation_half_life(
    params: &AlsMovementParams,
    gait_amount: f32,
    rotation_mode: RotationMode,
    moving: bool,
    view_yaw_speed: f32,
    stance: Stance,
) -> f32 {
    let crouching = crate::als::params::GaitHalfLives::normal();
    let velocity_profile = if stance == Stance::Crouching { &crouching } else { &params.rotation.grounded_moving_velocity_direction.interpolation_half_lives };
    let view_profile = if stance == Stance::Crouching { &crouching } else { &params.rotation.grounded_moving_view_direction.interpolation_half_lives };
    let stationary_view_profile = if stance == Stance::Crouching { &crouching } else { &params.rotation.grounded_stationary_view_direction.interpolation_half_lives };
    let base = if !AlsMovementParams::has_gait_settings(stance, rotation_mode)
        && rotation_mode != RotationMode::Aiming && (moving || rotation_mode == RotationMode::ViewDirection) {
        0.2
    } else { match (rotation_mode, moving) {
        (RotationMode::VelocityDirection, true) => velocity_profile.sample(gait_amount),
        (RotationMode::ViewDirection, true) => view_profile.sample(gait_amount),
        (RotationMode::Aiming, _) => {
            params
                .rotation
                .grounded_moving_aiming
                .interpolation_half_life
        }
        (RotationMode::VelocityDirection, false) => {
            params
                .rotation
                .grounded_stationary_velocity_direction
                .interpolation_half_life
        }
        (RotationMode::ViewDirection, false) => stationary_view_profile.sample(gait_amount),
        _ => 0.0,
    }};
    if rotation_mode == RotationMode::Aiming || (rotation_mode == RotationMode::VelocityDirection && !moving) {
        return base;
    }
    let responsiveness =
        (view_yaw_speed.abs() / params.rotation.reference_view_yaw_speed).clamp(0.0, 1.0);
    base * (1.0 + (params.rotation.min_interpolation_half_life_multiplier - 1.0) * responsiveness)
}

fn target_yaw_speed(params: &AlsMovementParams, rotation_mode: RotationMode) -> f32 {
    match rotation_mode {
        RotationMode::VelocityDirection => {
            params
                .rotation
                .grounded_moving_velocity_direction
                .target_yaw_angle_speed
        }
        RotationMode::ViewDirection => {
            params
                .rotation
                .grounded_moving_view_direction
                .target_yaw_angle_speed
        }
        RotationMode::Aiming => {
            params
                .rotation
                .grounded_moving_aiming
                .target_yaw_angle_speed
        }
        _ => 0.0,
    }
}

fn stationary_view_direction_holds_rotation(
    moving: bool,
    has_input: bool,
    blocked: bool,
    rotation_mode: RotationMode,
) -> bool {
    !moving && !has_input && blocked && rotation_mode == RotationMode::ViewDirection
}

#[allow(clippy::type_complexity)]
fn rotation_response_system(
    time: Res<Time>,
    mut commands: Commands,
    params_assets: Res<Assets<AlsMovementParams>>,
    blocked: Query<(), With<AlsMovementInputBlocked>>,
    gait_amounts: Query<&crate::als::gait::AlsGaitAmount>,
    first_person: Query<(), With<crate::als::input_request::AlsFirstPersonView>>,
    actions: Query<&crate::als::state::AlsNotifyLocomotionAction>,
    mantle_frames: Query<(), With<crate::mantle_drive::AlsMantlePhysicsFrame>>,
    stances: Query<&Stance>,
    parents: Query<&ChildOf>,
    mut space: ParamSet<(bevy::transform::helper::TransformHelper, Query<&mut Transform, With<AlsCharacter>>)>,
    mut q: Query<
        (
            Entity,
            Option<&mut Rotation>,
            &Gait,
            &LocomotionState,
            &RotationMode,
            &AlsViewYaw,
            Option<&crate::als_camera::AlsCameraCustomTimeDilation>,
            &AlsMovementParamsHandle,
            (&LinearVelocity, Option<&mut crate::als::dynamics::AlsMovementBaseDynamicsState>),
            &crate::actual_motion::AlsActualMotion,
            Option<&AlsRotationAnimationState>,
            Option<&mut AlsRotationResponseState>,
            Option<&crate::als::roll::AlsRollState>,
            &mut AlsAimingYawConstraint,
        ),
        (With<AlsCharacter>, Without<crate::als::character::AlsBirthAnimationPending>),
    >,
) {
    for (
        entity,
        rotation,
        gait,
        locomotion,
        rotation_mode,
        view_yaw,
        custom_dilation,
        params_handle,
        (velocity, mut base),
        motion,
        animation,
        mut current_runtime,
        roll,
        mut constraint,
    ) in &mut q
    {
        let delta_seconds = time.delta_secs() * custom_dilation.map_or(1.0, |value| value.0);
        let pre_collision_velocity = base.as_deref_mut().and_then(|state| state.pre_collision_velocity.take());
        let Ok(actor_world) = space.p0().compute_global_transform(entity) else { continue; };
        let current_yaw = world_forward_to_yaw(actor_world.rotation() * Vec3::NEG_Z);
        let action_active = mantle_frames.contains(entity)
            || roll.is_some_and(crate::als::roll::AlsRollState::root_motion_active)
            || actions.get(entity).is_ok_and(|action| *action != crate::als::state::AlsNotifyLocomotionAction::None);
        let invalid_mode = !matches!(*locomotion, LocomotionState::Grounded | LocomotionState::Air);
        if action_active || invalid_mode {
            let mut runtime = current_runtime.as_deref().copied().unwrap_or_default();
            runtime.initialized = true;
            runtime.target_yaw = current_yaw;
            runtime.smooth_target_yaw = current_yaw;
            runtime.previous_view_yaw = view_yaw.0;
            runtime.previous_locomotion = *locomotion;
            runtime.previous_rotation_mode = Some(*rotation_mode);
            if let Some(current_runtime) = current_runtime.as_deref_mut() {
                *current_runtime = runtime;
            } else {
                commands.entity(entity).insert(runtime);
            }
        }
        if blocked.contains(entity)
            || !matches!(*rotation_mode, RotationMode::VelocityDirection | RotationMode::ViewDirection | RotationMode::Aiming)
            || action_active
            || invalid_mode
        {
            constraint.applied_last_frame = false;
            continue;
        }
        let Some(params) = params_assets.get(params_handle.id()) else {
            constraint.applied_last_frame = false;
            continue;
        };
        let mut runtime = current_runtime.as_deref().copied().unwrap_or_default();
        if !runtime.initialized { runtime.target_yaw = current_yaw; }
        let view_yaw_speed = if runtime.initialized && delta_seconds > f32::EPSILON {
            (crate::als::turn_in_place::wrap_angle_degrees(view_yaw.0.to_degrees())
                - crate::als::turn_in_place::wrap_angle_degrees(runtime.previous_view_yaw.to_degrees()))
                .abs() / delta_seconds
        } else {
            0.0
        };
        let has_input = motion.has_input;
        let input_yaw = has_input.then(|| motion.input_yaw_degrees.to_radians());
        let velocity = LinearVelocity(crate::als::dynamics::locomotion_velocity(velocity.0, *locomotion, base.as_deref()));
        let horizontal_speed = source_horizontal_speed_meters(velocity.0);
        let (has_velocity, moving) = locomotion_movement_flags(has_input, horizontal_speed);
        let velocity_yaw = has_velocity.then(|| world_forward_to_yaw(velocity.0));
        let desired_velocity_yaw = pre_collision_velocity
            .filter(|value| *locomotion == LocomotionState::Grounded
                && locomotion_movement_flags(false, Vec2::new(value.x, value.z).length()).0)
            .map(world_forward_to_yaw);
        let animation = animation.copied().unwrap_or_default();
        if runtime.initialized
            && runtime.previous_rotation_mode.is_some_and(|previous| previous != *rotation_mode)
        {
            runtime.rotation_towards_last_input_direction_blocked = true;
        }
        let aiming = *rotation_mode == RotationMode::Aiming || first_person.contains(entity);
        if *locomotion == LocomotionState::Grounded && moving && has_input && !aiming {
            runtime.rotation_towards_last_input_direction_blocked = false;
        }
        let blocked_residual_motion = *locomotion == LocomotionState::Grounded
            && moving && !has_input && runtime.rotation_towards_last_input_direction_blocked && !aiming;
        let curve_driven = *locomotion == LocomotionState::Grounded && !moving && animation.yaw_speed_degrees.abs() > f32::EPSILON;
        let current_yaw = if curve_driven {
            wrap_yaw_radians(current_yaw + animation.yaw_speed_degrees.to_radians() * delta_seconds)
        } else {
            current_yaw
        };
        if curve_driven && (animation.yaw_speed_degrees * delta_seconds).abs() > 1e-8 {
            runtime.target_yaw = current_yaw;
        }
        let hold_rotation = stationary_view_direction_holds_rotation(
            moving,
            has_input,
            runtime.rotation_towards_last_input_direction_blocked,
            *rotation_mode,
        );
        if curve_driven || hold_rotation || !runtime.initialized {
            runtime.smooth_target_yaw = current_yaw;
        }
        let hold_air_rotation = *locomotion == LocomotionState::Air && !aiming && matches!(IN_AIR_ROTATION_MODE, 0 | 3);
        let body_rotation_mode = if aiming { RotationMode::Aiming } else { *rotation_mode };
        let target_yaw = wrap_yaw_radians(match body_rotation_mode {
            _ if blocked_residual_motion => current_yaw,
            _ if hold_air_rotation => current_yaw,
            RotationMode::VelocityDirection if !moving && runtime.rotation_towards_last_input_direction_blocked => runtime.target_yaw,
            RotationMode::VelocityDirection => desired_velocity_yaw.or(velocity_yaw).unwrap_or(
                if runtime.initialized
                    && !(curve_driven && runtime.rotation_towards_last_input_direction_blocked)
                {
                    runtime.target_yaw
                } else {
                    current_yaw
                },
            ),
            RotationMode::ViewDirection if moving && *gait == Gait::Sprinting => {
                velocity_yaw.unwrap_or(current_yaw)
            }
            RotationMode::ViewDirection if hold_rotation => current_yaw,
            RotationMode::ViewDirection if !moving && !has_input => {
                if curve_driven || !runtime.initialized {
                    current_yaw
                } else {
                    runtime.target_yaw
                }
            }
            RotationMode::ViewDirection if !moving => view_yaw.0,
            RotationMode::ViewDirection => {
                wrap_yaw_radians(view_yaw.0 + animation.yaw_offset_degrees.to_radians())
            }
            RotationMode::Aiming => view_yaw.0,
            _ => current_yaw,
        });
        let aiming_stationary = aiming && *locomotion == LocomotionState::Grounded && !moving && !has_input;
        if blocked_residual_motion || hold_air_rotation || aiming_stationary || (aiming && *locomotion == LocomotionState::Air) {
            runtime.smooth_target_yaw = target_yaw;
        } else {
            runtime.smooth_target_yaw = advance_yaw(
                runtime.smooth_target_yaw,
                target_yaw,
                target_yaw_speed(params, body_rotation_mode),
                delta_seconds,
            );
        }
        let half_life = if *locomotion == LocomotionState::Air {
            if aiming {
                params.rotation.in_air_aiming_half_life
            } else {
                params.rotation.in_air_half_life
            }
        } else {
            moving_rotation_half_life(params, gait_amounts.get(entity).map_or(0.0, |amount| amount.0), body_rotation_mode, moving, view_yaw_speed, stances.get(entity).copied().unwrap_or_default())
        };
        let mut next_yaw = if hold_air_rotation || aiming_stationary {
            current_yaw
        } else {
            damper_yaw(
                current_yaw,
                runtime.smooth_target_yaw,
                delta_seconds,
                half_life,
            )
        };
        if aiming {
            let (constrained_yaw, active_limit_degrees) = constrained_aiming_yaw(
                view_yaw.0,
                next_yaw,
                params.cmc.aiming_yaw_angle_limit_degrees,
                constraint.active_limit_degrees,
                !constraint.applied_last_frame,
                delta_seconds,
                aiming_stationary,
            );
            if *locomotion == LocomotionState::Grounded && !aiming_stationary
                && wrap_yaw_radians(constrained_yaw - next_yaw).abs() > 1e-4_f32.to_radians()
            {
                runtime.smooth_target_yaw = target_yaw;
            }
            next_yaw = constrained_yaw;
            constraint.active_limit_degrees = active_limit_degrees;
            constraint.applied_last_frame = true;
        } else {
            constraint.active_limit_degrees = 180.0;
            constraint.applied_last_frame = false;
        }
        let (_, pitch, world_roll) = actor_world.rotation().to_euler(EulerRot::YXZ);
        let next_rotation = Quat::from_euler(EulerRot::YXZ, next_yaw, pitch, world_roll);
        let parent_world = if let Ok(parent) = parents.get(entity) {
            let Ok(world) = space.p0().compute_global_transform(parent.parent()) else { continue; };
            Some(world)
        } else { None };
        space.p1().get_mut(entity).unwrap().rotation = parent_world
            .map_or(next_rotation, |parent| parent.rotation().inverse() * next_rotation);
        if let Some(mut rotation) = rotation {
            rotation.0 = next_rotation;
        }
        runtime.initialized = true;
        runtime.previous_view_yaw = view_yaw.0;
        runtime.previous_locomotion = *locomotion;
        runtime.previous_rotation_mode = Some(*rotation_mode);
        runtime.input_yaw = input_yaw;
        runtime.velocity_yaw = velocity_yaw;
        runtime.target_yaw = target_yaw;
        if let Some(mut current_runtime) = current_runtime {
            *current_runtime = runtime;
        } else {
            commands.entity(entity).insert(runtime);
        }
    }
}

#[cfg(test)]
mod rotation_cap_tests {
    use crate::als::character::{
        AlsCharacterBundle, AlsCharacterPlugin, als_character_capsule,
    };
    use crate::als::transition::AlsStateTransitionPlugin;
    use avian3d::prelude::{GravityScale, PhysicsPlugins, Position, RigidBody};
    use bevy::asset::{AssetEvent, AssetPlugin};
    use bevy::transform::TransformPlugin;
    use bevy_tnua::TnuaControllerPlugin;

    #[test]
    fn velocity_direction_initializes_target_from_birth_world_yaw_once() {
        for parent_yaw in [0.0_f32, 30.0] {
            for world_yaw in [0.0_f32, 45.0, -90.0, 170.0] {
                for already_initialized in [false, true] {
                    let mut app = App::new();
                    let mut time = Time::<()>::default();
                    time.advance_by(std::time::Duration::from_secs_f32(1.0 / 30.0));
                    app.insert_resource(time).init_resource::<Assets<AlsMovementParams>>()
                        .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
                        .add_systems(Update, rotation_response_system);
                    let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
                    let parent_rotation = Quat::from_rotation_y(parent_yaw.to_radians());
                    let parent = app.world_mut().spawn(Transform::from_rotation(parent_rotation)).id();
                    let world_rotation = Quat::from_rotation_y(world_yaw.to_radians());
                    let actor = app.world_mut().spawn((AlsCharacter, ChildOf(parent),
                        Transform::from_rotation(parent_rotation.inverse() * world_rotation), Rotation(world_rotation),
                        Stance::Standing, Gait::Running, LocomotionState::Grounded, RotationMode::VelocityDirection,
                        AlsViewYaw(world_yaw.to_radians()), AlsMovementParamsHandle(params), LinearVelocity::ZERO,
                        AlsInputCommand::default())).id();
                    let target = if already_initialized { wrap_yaw_radians(world_yaw.to_radians() + 0.4) }
                        else { world_yaw.to_radians() };
                    if already_initialized {
                        app.world_mut().entity_mut(actor).insert(AlsRotationResponseState {
                            initialized: true, target_yaw: target, smooth_target_yaw: target,
                            previous_view_yaw: world_yaw.to_radians(),
                            previous_rotation_mode: Some(RotationMode::VelocityDirection), ..default()
                        });
                    }
                    app.update();
                    let state = app.world().get::<AlsRotationResponseState>(actor).unwrap();
                    assert!(wrap_yaw_radians(state.target_yaw - target).abs() < 1e-6,
                        "parent={parent_yaw} world={world_yaw} initialized={already_initialized} target={}", state.target_yaw);
                    if !already_initialized {
                        let rotation = app.world().get::<Rotation>(actor).unwrap().0;
                        assert!(rotation.abs_diff_eq(world_rotation, 1e-6) || rotation.abs_diff_eq(-world_rotation, 1e-6));
                        let changed = Quat::from_rotation_y(world_yaw.to_radians() + 0.2);
                        app.world_mut().get_mut::<Transform>(actor).unwrap().rotation = parent_rotation.inverse() * changed;
                        app.world_mut().get_mut::<Rotation>(actor).unwrap().0 = changed;
                        app.update();
                        let state = app.world().get::<AlsRotationResponseState>(actor).unwrap();
                        assert!(wrap_yaw_radians(state.target_yaw - target).abs() < 1e-6);
                    }
                }
            }
        }
    }

    #[test]
    fn turn_curve_updates_velocity_direction_target_before_smoothing() {
        for (curve, delta) in [(90.0_f32, 1.0_f32 / 60.0), (-90.0, 1.0 / 60.0), (0.0, 1.0 / 60.0), (90.0, 0.0)] {
            let mut app = App::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_secs_f32(delta));
            app.insert_resource(time).init_resource::<Assets<AlsMovementParams>>()
                .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
                .add_systems(Update, rotation_response_system);
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
            let actor = app.world_mut().spawn((AlsCharacter, Transform::from_rotation(Quat::from_rotation_y(0.2)),
                Stance::Standing, Gait::Walking, LocomotionState::Grounded, RotationMode::VelocityDirection,
                AlsViewYaw(0.2), AlsMovementParamsHandle(params), LinearVelocity::ZERO, AlsInputCommand::default(),
                AlsRotationAnimationState { yaw_speed_degrees: curve, ..default() },
                AlsRotationResponseState { initialized: true, target_yaw: 1.2, smooth_target_yaw: 1.0,
                    previous_view_yaw: 0.2, previous_rotation_mode: Some(RotationMode::ViewDirection),
                    rotation_towards_last_input_direction_blocked: false, ..default() })).id();
            app.update();
            let state = app.world().get::<AlsRotationResponseState>(actor).unwrap();
            assert!(state.rotation_towards_last_input_direction_blocked);
            let target = if curve != 0.0 && delta != 0.0 { 0.2 + (curve * delta).to_radians() } else { 1.2 };
            assert!(wrap_yaw_radians(state.target_yaw - target).abs() < 1e-6,
                "curve={curve} delta={delta} target={} expected={target}", state.target_yaw);
            if curve != 0.0 && delta != 0.0 {
                assert!(wrap_yaw_radians(state.smooth_target_yaw - target).abs() < 1e-6);
                let yaw = world_forward_to_yaw(app.world().get::<Transform>(actor).unwrap().rotation * Vec3::NEG_Z);
                assert!(wrap_yaw_radians(yaw - target).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn ordinary_rotation_keeps_current_parent_world_and_physics_aligned() {
        for locomotion in [LocomotionState::Grounded, LocomotionState::Air] {
            for mode in [RotationMode::VelocityDirection, RotationMode::ViewDirection, RotationMode::Aiming] {
                let mut app = App::new();
                let mut time = Time::<()>::default();
                time.advance_by(std::time::Duration::from_millis(20));
                app.insert_resource(time).init_resource::<Assets<AlsMovementParams>>()
                    .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
                    .add_systems(Update, rotation_response_system);
                let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
                let parent = app.world_mut().spawn(Transform::IDENTITY).id();
                let actor = app.world_mut().spawn((AlsCharacter, Transform::IDENTITY, ChildOf(parent), Rotation::IDENTITY,
                    Stance::Standing, Gait::Running, locomotion, mode, AlsViewYaw(0.2), AlsMovementParamsHandle(params.clone()),
                    LinearVelocity(Vec3::NEG_Z * 2.0),
                    crate::actual_motion::AlsActualMotion { has_input: true, ..default() })).id();
                for yaw in [90.0_f32, -35.0] {
                    let parent_transform = Transform::from_xyz(5.0, 2.0, -3.0)
                        .with_rotation(Quat::from_euler(EulerRot::YXZ, yaw.to_radians(), 0.15, -0.1))
                        .with_scale(Vec3::splat(2.0));
                    *app.world_mut().get_mut::<Transform>(parent).unwrap() = parent_transform;
                    let before = parent_transform.rotation * app.world().get::<Transform>(actor).unwrap().rotation;
                    let (_, pitch, roll) = before.to_euler(EulerRot::YXZ);
                    let reference = app.world_mut().spawn((AlsCharacter, Transform::from_rotation(before), Rotation(before),
                        Stance::Standing, Gait::Running, locomotion, mode, AlsViewYaw(0.2), AlsMovementParamsHandle(params.clone()),
                        LinearVelocity(Vec3::NEG_Z * 2.0),
                        crate::actual_motion::AlsActualMotion { has_input: true, ..default() })).id();
                    if let Some(response) = app.world().get::<AlsRotationResponseState>(actor).copied() {
                        app.world_mut().entity_mut(reference).insert(response);
                    }
                    app.world_mut().run_schedule(Update);
                    let local = app.world().get::<Transform>(actor).unwrap();
                    let physical = app.world().get::<Rotation>(actor).unwrap().0;
                    let expected = app.world().get::<Rotation>(reference).unwrap().0;
                    assert!(physical.angle_between(expected).abs() < 1e-3,
                        "Source world rotation changed with attachment: {locomotion:?}, {mode:?}, {yaw}");
                    assert!((parent_transform.rotation * local.rotation).angle_between(physical).abs() < 1e-3,
                        "Source actor world and physical rotation differ: {locomotion:?}, {mode:?}, {yaw}");
                    let (_, actual_pitch, actual_roll) = physical.to_euler(EulerRot::YXZ);
                    assert!((actual_pitch - pitch).abs() < 1e-5 && (actual_roll - roll).abs() < 1e-5,
                        "Source world pitch and roll were discarded");
                    assert_eq!(local.translation, Vec3::ZERO);
                    assert_eq!(local.scale, Vec3::ONE);
                }
            }
        }
    }
    use bevy_tnua_avian3d::TnuaAvian3dPlugin;

    use super::*;

    #[test]
    fn paused_character_retains_the_actual_rotation_owner_outputs() {
        for (locomotion, mode, velocity, has_input, first_person) in [
            (LocomotionState::Grounded, RotationMode::Aiming, Vec3::ZERO, false, false),
            (LocomotionState::Grounded, RotationMode::Aiming, Vec3::NEG_Z * 1.75, true, false),
            (LocomotionState::Grounded, RotationMode::ViewDirection, Vec3::NEG_Z * 1.75, true, false),
            (LocomotionState::Grounded, RotationMode::VelocityDirection, Vec3::X * 1.75, true, false),
            (LocomotionState::Grounded, RotationMode::ViewDirection, Vec3::ZERO, false, true),
            (LocomotionState::Air, RotationMode::Aiming, Vec3::X * 1.75, true, false),
            (LocomotionState::Air, RotationMode::ViewDirection, Vec3::X * 1.75, true, false),
        ] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(20)))
                .init_resource::<Assets<AlsMovementParams>>()
                .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
                .add_systems(Update, rotation_response_system);
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
                .add(AlsMovementParams::default());
            let actor = app.world_mut().spawn((
                AlsCharacter, Stance::Standing, Transform::IDENTITY, Gait::Walking,
                locomotion, mode,
                AlsViewYaw(170.0_f32.to_radians()), AlsMovementParamsHandle(params),
                LinearVelocity(velocity),
                crate::actual_motion::AlsActualMotion { has_input, input_yaw_degrees: 170.0, ..default() },
            )).id();
            if first_person { app.world_mut().entity_mut(actor).insert(crate::als::input_request::AlsFirstPersonView); }
            for _ in 0..8 { app.update(); }
            let before_transform = *app.world().get::<Transform>(actor).unwrap();
            let before_response = *app.world().get::<AlsRotationResponseState>(actor).unwrap();
            let before_constraint = *app.world().get::<AlsAimingYawConstraint>(actor).unwrap();
            app.world_mut().resource_mut::<Time<Virtual>>().pause();
            for _ in 0..3 {
                app.update();
                assert_eq!(*app.world().get::<Transform>(actor).unwrap(), before_transform,
                    "Source paused Character must preserve its actual actor rotation");
                let after = app.world().get::<AlsRotationResponseState>(actor).unwrap();
                assert_eq!((after.initialized, after.smooth_target_yaw, after.previous_view_yaw,
                    after.input_yaw, after.velocity_yaw, after.target_yaw,
                    after.rotation_towards_last_input_direction_blocked, after.previous_locomotion, after.previous_rotation_mode),
                    (before_response.initialized, before_response.smooth_target_yaw, before_response.previous_view_yaw,
                    before_response.input_yaw, before_response.velocity_yaw, before_response.target_yaw,
                    before_response.rotation_towards_last_input_direction_blocked, before_response.previous_locomotion, before_response.previous_rotation_mode),
                    "Source paused Character must preserve its rotation owner state");
                assert_eq!(*app.world().get::<AlsAimingYawConstraint>(actor).unwrap(), before_constraint,
                    "Source paused Character must preserve its aiming constraint state");
            }
            app.world_mut().resource_mut::<Time<Virtual>>().unpause();
            app.update();
            let rotation = app.world().get::<Transform>(actor).unwrap().rotation;
            assert!(rotation.is_finite());
            if locomotion == LocomotionState::Grounded {
                assert_ne!(rotation, before_transform.rotation, "Source resumed Character must continue rotating toward its target");
            }
        }
    }

    #[test]
    fn rotation_uses_consumed_input_at_the_source_low_speed_boundary() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(16));
        app.insert_resource(time).init_resource::<Assets<AlsMovementParams>>()
            .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
            .add_systems(Update, rotation_response_system);
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
        let character = app.world_mut().spawn((AlsCharacter, Stance::Standing, Transform::IDENTITY,
            Gait::Walking, LocomotionState::Grounded, RotationMode::ViewDirection,
            AlsViewYaw(30.0_f32.to_radians()), AlsMovementParamsHandle(params),
            LinearVelocity(Vec3::NEG_Z * 0.01),
            AlsInputCommand { move_dir: Vec2::Y * 0.0002, ..default() },
            crate::actual_motion::AlsActualMotion { has_input: true, input_yaw_degrees: 30.0, ..default() },
            AlsRotationResponseState { initialized: true, previous_view_yaw: 30.0_f32.to_radians(),
                previous_rotation_mode: Some(RotationMode::ViewDirection), ..default() },
        )).id();
        app.update();
        let yaw = world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward());
        assert!(yaw > 0.001, "Source valid input must rotate at 0.01 m/s: {yaw}");
        let state = app.world().get::<AlsRotationResponseState>(character).unwrap();
        assert_eq!(state.input_yaw, Some(30.0_f32.to_radians()));
        assert!(!state.rotation_towards_last_input_direction_blocked);
    }

    #[test]
    fn custom_mode_preserves_actor_rotation_and_physical_stance() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(16));
        app.insert_resource(time)
            .init_resource::<Assets<AlsMovementParams>>()
            .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
            .add_systems(Update, rotation_response_system);
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
            .add(AlsMovementParams::default());
        let transform = Transform::IDENTITY.with_rotation(Quat::from_rotation_y(-0.6));
        let actor = app.world_mut().spawn((AlsCharacter, transform, Rotation(transform.rotation),
            Gait::Running, LocomotionState::Custom, RotationMode::Aiming, AlsViewYaw(1.2),
            AlsMovementParamsHandle(params), LinearVelocity(Vec3::X),
            AlsInputCommand { move_dir: Vec2::X, ..default() })).id();
        app.update();
        assert_eq!(*app.world().get::<Transform>(actor).unwrap(), transform);
        assert_eq!(app.world().get::<Rotation>(actor).unwrap().0, transform.rotation);
        app.world_mut().entity_mut(actor).insert((
            LocomotionState::Custom,
            crate::als::state::AlsNotifyLocomotionAction::Mantling,
            AlsRotationResponseState { initialized: true, target_yaw: 1.0, smooth_target_yaw: 1.0,
                previous_view_yaw: -1.0, previous_locomotion: LocomotionState::Air,
                previous_rotation_mode: Some(RotationMode::Aiming), ..default() },
            AlsAimingYawConstraint { active_limit_degrees: 70.0, applied_last_frame: true },
        ));
        app.update();
        assert_eq!(*app.world().get::<Transform>(actor).unwrap(), transform);
        let response = app.world().get::<AlsRotationResponseState>(actor).unwrap();
        assert!((response.target_yaw + 0.6).abs() < 1e-6);
        assert!((response.smooth_target_yaw + 0.6).abs() < 1e-6);
        assert_eq!(response.previous_view_yaw, 1.2);
        assert_eq!(response.previous_locomotion, LocomotionState::Custom);
        assert!(!app.world().get::<AlsAimingYawConstraint>(actor).unwrap().applied_last_frame);
        let moved = Transform::IDENTITY.with_rotation(Quat::from_rotation_y(-1.1));
        app.world_mut().entity_mut(actor).insert((
            LocomotionState::Grounded, crate::als::state::AlsNotifyLocomotionAction::None,
            crate::als::roll::AlsRollState { active: true, ..default() },
            moved, Rotation(moved.rotation), AlsViewYaw(2.0),
        ));
        app.update();
        let response = app.world().get::<AlsRotationResponseState>(actor).unwrap();
        assert!((response.target_yaw + 1.1).abs() < 1e-6);
        assert!((response.smooth_target_yaw + 1.1).abs() < 1e-6);
        assert_eq!(response.previous_view_yaw, 2.0);
        app.world_mut().entity_mut(actor).remove::<crate::als::roll::AlsRollState>();
        app.update();
        let limit = app.world().get::<AlsAimingYawConstraint>(actor).unwrap();
        assert!(limit.applied_last_frame && limit.active_limit_degrees > 100.0);
        for (mode, action, yaw) in [
            (LocomotionState::Grounded, crate::als::state::AlsNotifyLocomotionAction::GettingUp, 0.7),
            (LocomotionState::None, crate::als::state::AlsNotifyLocomotionAction::Ragdolling, -0.8),
            (LocomotionState::None, crate::als::state::AlsNotifyLocomotionAction::None, 0.9),
        ] {
            let transform = Transform::from_rotation(Quat::from_rotation_y(yaw));
            app.world_mut().entity_mut(actor).insert((mode, action, transform, Rotation(transform.rotation),
                AlsAimingYawConstraint { active_limit_degrees: 70.0, applied_last_frame: true }));
            app.update();
            assert_eq!(*app.world().get::<Transform>(actor).unwrap(), transform);
            let response = app.world().get::<AlsRotationResponseState>(actor).unwrap();
            assert!((response.target_yaw - yaw).abs() < 1e-6 && (response.smooth_target_yaw - yaw).abs() < 1e-6);
            assert!(!app.world().get::<AlsAimingYawConstraint>(actor).unwrap().applied_last_frame);
        }
        for (desired, actual) in [(Stance::Crouching, Stance::Standing), (Stance::Standing, Stance::Crouching)] {
            assert_eq!(capsule_target_for_locomotion(desired, actual, LocomotionState::Custom, false, None), actual);
            assert_eq!(capsule_target_for_locomotion(desired, actual, LocomotionState::Grounded, false, None), desired);
        }
    }

    #[test]
    fn grounded_rotation_uses_source_input_and_velocity_thresholds() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(16));
        app.insert_resource(time)
            .init_resource::<Assets<AlsMovementParams>>()
            .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
            .add_systems(Update, rotation_response_system);
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
            .add(AlsMovementParams::default());
        let alpha = 1.0 - crate::als::animation::inv_exp_approx(std::f32::consts::LN_2 / (0.1 + 1e-8) * 0.016);
        for stance in [Stance::Standing, Stance::Crouching] {
            for (input, speed, moving) in [(false, 0.1, false), (false, 0.5, false),
                (false, 0.5001, true), (true, 0.009, false), (true, 0.01, true), (true, 0.05, true)] {
                let character = app.world_mut().spawn((
                    AlsCharacter, stance, Transform::IDENTITY, Gait::Walking,
                    LocomotionState::Grounded, RotationMode::Aiming, AlsViewYaw(30.0_f32.to_radians()),
                    AlsMovementParamsHandle(params.clone()), LinearVelocity(Vec3::NEG_Z * speed),
                    AlsInputCommand { move_dir: if input { Vec2::Y } else { Vec2::ZERO }, ..default() },
                    crate::actual_motion::AlsActualMotion { has_input: input, input_yaw_degrees: 30.0, ..default() },
                    AlsRotationAnimationState { yaw_speed_degrees: 60.0, ..default() },
                )).id();
                app.update();
                let curve_yaw = if moving { 0.0 } else { 60.0_f32.to_radians() * 0.016 };
                let expected = curve_yaw + if moving || input { 16.0_f32.to_radians() * alpha } else { 0.0 };
                let yaw = world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward());
                assert!((yaw - expected).abs() < 1e-6, "{stance:?}, input={input}, speed={speed}: {yaw} != {expected}");
                app.world_mut().despawn(character);
            }
        }
    }

    #[test]
    fn moving_aiming_follows_view_before_constraining_in_both_stances() {
        let mut app = App::new();
        app.init_resource::<Time<Virtual>>();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(16));
        app.insert_resource(time)
            .init_resource::<Assets<AlsMovementParams>>()
            .add_plugins(crate::als::transition::AlsStateTransitionPlugin)
            .add_message::<crate::als::aim::AimStateChanged>()
            .add_systems(Update, (crate::als::input_request::input_state_request_system, crate::als::aim::aim_rotation_mode_system)
                .chain().before(crate::als::transition::state_transition_system))
            .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
            .add_systems(Update, rotation_response_system.after(crate::als::transition::state_transition_system));
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
        let characters = [Stance::Standing, Stance::Crouching].map(|stance| app.world_mut().spawn((
            AlsCharacter, stance, Transform::IDENTITY, Rotation::IDENTITY, Gait::Running,
            LocomotionState::Grounded, RotationMode::ViewDirection, AlsViewYaw(0.0),
            AlsMovementParamsHandle(params.clone()), LinearVelocity::ZERO, AlsMaxAllowedGait(Gait::Running),
            crate::als::aim::AlsLastFreeRotationMode(RotationMode::ViewDirection),
            OverlayState::Default, AlsStateRequest::default(),
            AlsInputCommand { aim_held: true, ..default() },
        )).id());
        app.update();
        for character in characters {
            assert_eq!(*app.world().get::<RotationMode>(character).unwrap(), RotationMode::Aiming);
            app.world_mut().entity_mut(character).insert((
                AlsInputCommand { aim_held: true, move_dir: Vec2::Y, ..default() },
                crate::actual_motion::AlsActualMotion { has_input: true, input_yaw_degrees: 30.0, ..default() },
                LinearVelocity(Vec3::NEG_Z * 1.75), AlsViewYaw(30.0_f32.to_radians()),
            ));
        }
        app.update();
        let observed = characters.map(|character| (
            world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward()),
            app.world().get::<AlsRotationResponseState>(character).unwrap().target_yaw,
        ));
        let expected_yaw = 16.0_f32.to_radians() * (1.0 - crate::als::animation::inv_exp_approx(std::f32::consts::LN_2 / (0.1 + 1e-8) * 0.016));
        assert!(observed.iter().all(|(body, target)| (*body - expected_yaw).abs() < 1e-6 && (*target - 30.0_f32.to_radians()).abs() < 1e-6), "{observed:?}");
        for character in characters {
            app.world_mut().entity_mut(character).insert(AlsViewYaw(160.0_f32.to_radians()));
        }
        app.update();
        for character in characters {
            let response = app.world().get::<AlsRotationResponseState>(character).unwrap();
            let yaw = world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward());
            assert!((yaw - 90.0_f32.to_radians()).abs() < 1e-5);
            assert!((response.target_yaw - 160.0_f32.to_radians()).abs() < 1e-6);
            assert!((response.smooth_target_yaw - response.target_yaw).abs() < 1e-6);
            assert!(app.world().get::<Transform>(character).unwrap().rotation.dot(app.world().get::<Rotation>(character).unwrap().0).abs() > 1.0 - 1e-6);
        }
        let character = characters[0];
        app.world_mut().entity_mut(character).insert((
            LocomotionState::Air, Transform::IDENTITY, Rotation::IDENTITY,
            AlsInputCommand { aim_held: true, ..default() }, LinearVelocity(Vec3::Y),
            crate::actual_motion::AlsActualMotion::default(),
            AlsViewYaw(30.0_f32.to_radians()), AlsRotationResponseState::default(),
            AlsRotationAnimationState { yaw_speed_degrees: 120.0, ..default() },
        ));
        app.update();
        let response = app.world().get::<AlsRotationResponseState>(character).unwrap();
        let yaw = world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward());
        assert!((yaw - expected_yaw * 30.0 / 16.0).abs() < 1e-6);
        assert!((response.target_yaw - 30.0_f32.to_radians()).abs() < 1e-6);
        assert!((response.smooth_target_yaw - response.target_yaw).abs() < 1e-6);
    }

    #[test]
    fn velocity_direction_retains_rotation_target_after_velocity_stops() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(16));
        app.insert_resource(time)
            .init_resource::<Assets<AlsMovementParams>>()
            .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
            .add_systems(Update, rotation_response_system);
        let params = app
            .world_mut()
            .resource_mut::<Assets<AlsMovementParams>>()
            .add(AlsMovementParams::default());
        let character = app.world_mut().spawn((
            AlsCharacter,
            Transform::IDENTITY,
            Gait::Running,
            LocomotionState::Grounded,
            RotationMode::VelocityDirection,
            AlsViewYaw(0.0),
            AlsMovementParamsHandle(params),
            LinearVelocity(Vec3::X * 0.1),
            AlsInputCommand::default(),
            AlsRotationResponseState { initialized: true,
                rotation_towards_last_input_direction_blocked: false,
                previous_rotation_mode: Some(RotationMode::VelocityDirection), ..default() },
        )).id();
        app.update();
        let target = app.world().get::<AlsRotationResponseState>(character).unwrap().target_yaw;
        assert!((target + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        app.world_mut().entity_mut(character).insert(LinearVelocity::ZERO);
        app.update();
        let response = app.world().get::<AlsRotationResponseState>(character).unwrap();
        assert!((response.target_yaw - target).abs() < 1e-6);
        let body = app.world().get::<Transform>(character).unwrap();
        assert!((world_forward_to_yaw(body.rotation * Vec3::NEG_Z) - target).abs() > 0.1);
    }

    #[test]
    fn stationary_turn_curve_starts_at_actor_yaw_and_retains_its_target() {
        for (blocked, curve) in [(false, 0.0_f32), (true, 120.0), (false, -120.0)] {
            let mut app = App::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_millis(16));
            app.insert_resource(time)
                .init_resource::<Assets<AlsMovementParams>>()
                .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
                .add_systems(Update, rotation_response_system);
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
                .add(AlsMovementParams::default());
            let character = app.world_mut().spawn((
                AlsCharacter,
                Transform::IDENTITY,
                Gait::Walking,
                LocomotionState::Grounded,
                RotationMode::ViewDirection,
                AlsViewYaw(1.5),
                AlsMovementParamsHandle(params),
                LinearVelocity::ZERO,
                AlsInputCommand::default(),
                AlsRotationAnimationState { yaw_speed_degrees: curve, ..default() },
                AlsRotationResponseState {
                    initialized: true,
                    smooth_target_yaw: if curve == 0.0 { 0.0 } else { 1.0 },
                    target_yaw: if curve == 0.0 { 0.0 } else { 1.2 },
                    previous_view_yaw: 1.5,
                    previous_rotation_mode: Some(RotationMode::ViewDirection),
                    rotation_towards_last_input_direction_blocked: blocked,
                    ..default()
                },
            )).id();
            app.update();
            let expected = curve.to_radians() * 0.016;
            let yaw = world_forward_to_yaw(app.world().get::<Transform>(character).unwrap().rotation * Vec3::NEG_Z);
            assert!((yaw - expected).abs() < 1e-6, "curve={curve}, blocked={blocked}, yaw={yaw}, expected={expected}");
            let response = app.world().get::<AlsRotationResponseState>(character).unwrap();
            assert!((response.target_yaw - expected).abs() < 1e-6);
            assert!((response.smooth_target_yaw - expected).abs() < 1e-6);
            app.world_mut().entity_mut(character).insert((AlsViewYaw(-1.5), AlsRotationAnimationState::default()));
            app.update();
            let yaw = world_forward_to_yaw(app.world().get::<Transform>(character).unwrap().rotation * Vec3::NEG_Z);
            assert!((yaw - expected).abs() < 1e-6);
            app.world_mut().entity_mut(character).insert((
                AlsViewYaw(1.5),
                AlsInputCommand { move_dir: Vec2::Y, ..default() },
                crate::actual_motion::AlsActualMotion { has_input: true, input_yaw_degrees: 1.5_f32.to_degrees(), ..default() },
                AlsRotationAnimationState { yaw_speed_degrees: 120.0, yaw_offset_degrees: 30.0 },
            ));
            app.update();
            let response = app.world().get::<AlsRotationResponseState>(character).unwrap();
            assert!((response.target_yaw - 1.5).abs() < 1e-6);
            let yaw = world_forward_to_yaw(app.world().get::<Transform>(character).unwrap().rotation * Vec3::NEG_Z);
            assert!(yaw > expected + 120.0_f32.to_radians() * 0.016);
        }
    }

    #[test]
    fn stationary_view_direction_holds_actor_yaw_until_animation_curve_drives_it() {
        assert!(stationary_view_direction_holds_rotation(
            false,
            false,
            true,
            RotationMode::ViewDirection,
        ));
        assert!(!stationary_view_direction_holds_rotation(
            false,
            true,
            true,
            RotationMode::ViewDirection,
        ));
        assert!(!stationary_view_direction_holds_rotation(
            false,
            false,
            false,
            RotationMode::ViewDirection,
        ));
    }

    #[test]
    fn actual_velocity_preserves_source_centimeter_speed_rounding() {
        for (velocity, centimeters, meters) in [
            (Vec3::new(1.333_333_4, 7.0, 0.0), 133.333_34, 1.333_333_5),
            (Vec3::new(1.485_657_6, -4.0, 0.0), 148.565_75, 1.485_657_5),
            (Vec3::ZERO, 0.0, 0.0),
        ] {
            let actual = AlsActualVelocity::from_physics_velocity(velocity);
            assert_eq!(source_horizontal_speed_centimeters(velocity), centimeters);
            assert_eq!(actual.horizontal_speed, meters);
            assert_eq!(actual.world_velocity, velocity);
        }
    }

    #[test]
    fn spawn_movement_mode_sees_same_frame_floor() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
            .insert_resource(Assets::<AlsMovementParams>::default())
            .add_message::<StateTransitionEvent>();
        app.finish();
        let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(100.0, 0.2, 100.0),
            Transform::from_xyz(0.0, -0.1, 0.0))).id();
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
        let character = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, als_character_capsule(0.9, 0.3),
            Transform::from_xyz(0.0, 0.9215, 0.0), LocomotionState::Grounded,
            Stance::Standing, AlsMovementParamsHandle(params))).id();
        app.world_mut().run_system_once(initialize_spawn_movement_mode).unwrap();
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Grounded);
        assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(character).unwrap().entity, Some(floor));
    }

    #[test]
    fn spawn_movement_mode_uses_registered_floor_before_physics() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
            .insert_resource(Assets::<AlsMovementParams>::default())
            .add_message::<StateTransitionEvent>();
        app.finish();
        let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(100.0, 0.2, 100.0),
            Transform::from_xyz(0.0, -0.1, 0.0))).id();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(1.0, 0.1, 1.0), Sensor,
            Transform::from_xyz(3.0, 0.55, 0.0)));
        app.update();
        app.update();
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
        let supported = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, als_character_capsule(0.9, 0.3),
            Transform::from_xyz(0.0, 0.9215, 0.0), LinearVelocity(Vec3::Y * 2.0),
            LocomotionState::Grounded, Stance::Standing, AlsMovementParamsHandle(params.clone()))).id();
        let unsupported = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, als_character_capsule(0.9, 0.3),
            Transform::from_xyz(3.0, 1.5, 0.0), LinearVelocity(Vec3::Y * 2.0),
            LocomotionState::Grounded, Stance::Standing, AlsMovementParamsHandle(params.clone()))).id();
        let explicit_air = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, als_character_capsule(0.9, 0.3),
            Transform::from_xyz(-3.0, 0.9215, 0.0), LocomotionState::Air,
            Stance::Standing, AlsMovementParamsHandle(params.clone()))).id();
        let pending = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, als_character_capsule(0.9, 0.3),
            Transform::from_xyz(6.0, 1.5, 0.0), LocomotionState::Grounded,
            Stance::Standing, AlsMovementParamsHandle::default())).id();
        app.world_mut().run_system_once(initialize_spawn_movement_mode).unwrap();
        assert_eq!(*app.world().get::<LocomotionState>(supported).unwrap(), LocomotionState::Grounded);
        assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(supported).unwrap().entity, Some(floor));
        assert_eq!(app.world().get::<LinearVelocity>(supported).unwrap().y, 0.0);
        assert_eq!(*app.world().get::<LocomotionState>(unsupported).unwrap(), LocomotionState::Air);
        assert_eq!(app.world().get::<LinearVelocity>(unsupported).unwrap().y, 2.0);
        assert_eq!(app.world().get::<Position>(unsupported).unwrap().y, 1.5);
        assert_eq!(*app.world().get::<LocomotionState>(explicit_air).unwrap(), LocomotionState::Air);
        assert!(app.world().get::<crate::als::character::AlsInitialMovementModePending>(supported).is_none());
        assert!(app.world().get::<crate::als::character::AlsInitialMovementModePending>(pending).is_some());
        app.world_mut().entity_mut(pending).insert(AlsMovementParamsHandle(params));
        app.world_mut().get_mut::<Transform>(supported).unwrap().translation.y = 1.5;
        app.world_mut().run_system_once(initialize_spawn_movement_mode).unwrap();
        assert_eq!(*app.world().get::<LocomotionState>(supported).unwrap(), LocomotionState::Grounded);
        assert_eq!(*app.world().get::<LocomotionState>(pending).unwrap(), LocomotionState::Air);
    }

    #[test]
    fn free_walking_snapshot_preserves_plan_at_nonzero_world_origin() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Time<avian3d::prelude::Physics>>()
            .add_systems(FixedPostUpdate, snapshot_actual_velocity_system);
        app.world_mut().resource_mut::<Time<avian3d::prelude::Physics>>()
            .advance_by(std::time::Duration::from_secs_f32(1.0 / 30.0));
        let dt = app.world().resource::<Time<avian3d::prelude::Physics>>().delta_secs();
        let planned = Vec3::X * 0.666_666_7;
        let delta = Vec3::X * 0.022_222_225;
        let mut actors = Vec::new();
        for origin in [0.0, 1.662_102_5] {
            let start = Vec3::X * origin;
            let actor = app.world_mut().spawn((AlsCharacter, LocomotionState::Grounded,
                Position(start + delta), LinearVelocity(planned),
                AlsCmcAdapterRuntime { simulation_substeps: 1, simulation_delta_seconds: dt,
                    grounded_move_start: Some(start), grounded_movement_delta: Some(delta), ..default() },
                crate::als::dynamics::AlsMovementBaseDynamicsState { pre_collision_velocity: Some(planned), ..default() },
            )).id();
            actors.push(actor);
        }
        let observed_start = Vec3::new(-15.0, 0.0, -15.0);
        let observed_plan = Vec3::new(0.666_666_7, 0.0, -0.000_050_912_964);
        let observed_delta = Vec3::new(0.022_222_225, 0.0, -0.000_001_697_098_9);
        let observed = app.world_mut().spawn((AlsCharacter, LocomotionState::Grounded,
            Position(observed_start + observed_delta), LinearVelocity(observed_plan),
            AlsCmcAdapterRuntime { simulation_substeps: 1, simulation_delta_seconds: dt,
                grounded_move_start: Some(observed_start), grounded_movement_delta: Some(observed_delta), ..default() },
            crate::als::dynamics::AlsMovementBaseDynamicsState { pre_collision_velocity: Some(observed_plan), ..default() },
        )).id();
        let start = Vec3::X * 1.662_102_5;
        let blocked_position = start + delta * 0.5;
        let blocked = app.world_mut().spawn((AlsCharacter, LocomotionState::Grounded,
            Position(blocked_position), LinearVelocity(planned),
            AlsCmcAdapterRuntime { simulation_substeps: 1, simulation_delta_seconds: dt,
                grounded_move_start: Some(start), grounded_movement_delta: Some(delta), ..default() },
            crate::als::dynamics::AlsMovementBaseDynamicsState { pre_collision_velocity: Some(planned), ..default() },
        )).id();
        let slow_dt = dt * 0.5;
        let slow_position = start + delta * 0.5;
        let slow = app.world_mut().spawn((AlsCharacter, LocomotionState::Grounded,
            Position(slow_position), LinearVelocity(planned),
            AlsCmcAdapterRuntime { simulation_substeps: 1, simulation_delta_seconds: slow_dt,
                grounded_move_start: Some(start), grounded_movement_delta: Some(delta * 0.5), ..default() },
            crate::als::dynamics::AlsMovementBaseDynamicsState { pre_collision_velocity: Some(planned), ..default() },
        )).id();
        let support = Vec3::X * 1.25;
        let moving_position = start + delta + support * dt;
        let moving = app.world_mut().spawn((AlsCharacter, LocomotionState::Grounded,
            Position(moving_position), LinearVelocity(planned + support),
            AlsCmcAdapterRuntime { simulation_substeps: 1, simulation_delta_seconds: dt,
                grounded_move_start: Some(start), grounded_movement_delta: Some(delta), ..default() },
            crate::als::dynamics::AlsMovementBaseDynamicsState {
                entity: Some(Entity::PLACEHOLDER), support_velocity: support,
                pre_collision_velocity: Some(planned), ..default()
            },
        )).id();
        app.world_mut().run_schedule(FixedPostUpdate);
        for actor in actors {
            assert_eq!(app.world().get::<AlsActualVelocity>(actor).unwrap().world_velocity.x, planned.x);
        }
        assert_eq!(app.world().get::<AlsActualVelocity>(observed).unwrap().world_velocity, observed_plan);
        assert_eq!(app.world().get::<AlsActualVelocity>(blocked).unwrap().world_velocity.x,
            (blocked_position.x - start.x) / dt);
        assert_eq!(app.world().get::<AlsActualVelocity>(slow).unwrap().world_velocity.x,
            (slow_position.x - start.x) / slow_dt);
        assert_eq!(app.world().get::<LinearVelocity>(moving).unwrap().x,
            (moving_position.x - start.x - support.x * dt) / dt + support.x);
    }

    #[test]
    fn actual_velocity_snapshot_retains_full_velocity_and_horizontal_speed() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Time<avian3d::prelude::Physics>>()
            .add_systems(FixedPostUpdate, snapshot_actual_velocity_system);
        let entity = app
            .world_mut()
            .spawn((
                AlsCharacter,
                LinearVelocity(Vec3::new(3.0, 7.0, -4.0)),
                LocomotionState::Air,
                AlsActualVelocity::default(),
                crate::als::dynamics::AlsMovementBaseDynamicsState {
                    pre_collision_velocity: Some(Vec3::NEG_Z), ..default()
                },
            ))
            .id();

        app.world_mut().run_schedule(FixedPostUpdate);

        let motion = *app.world().get::<AlsActualVelocity>(entity).unwrap();
        assert_eq!(motion.world_velocity, Vec3::new(3.0, 7.0, -4.0));
        assert!((motion.horizontal_speed - 5.0).abs() < 1e-6);
        assert!(app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(entity)
            .unwrap().pre_collision_velocity.is_none());

        app.world_mut().entity_mut(entity).insert(LinearVelocity(Vec3::new(1.333_333_4, 7.0, 0.0)));
        app.world_mut().run_schedule(FixedPostUpdate);
        assert_eq!(app.world().get::<AlsActualVelocity>(entity).unwrap().horizontal_speed, 1.333_333_5);

        app.world_mut()
            .entity_mut(entity)
            .insert(LinearVelocity::ZERO);
        app.world_mut().run_schedule(FixedPostUpdate);
        assert_eq!(
            *app.world().get::<AlsActualVelocity>(entity).unwrap(),
            AlsActualVelocity::default()
        );
    }

    #[derive(Component)]
    struct StandingCapsulePhysicsCase {
        beam: Entity,
    }

    #[derive(Component)]
    struct StandingCapsuleBeam;

    #[derive(Resource)]
    struct StandingCapsulePhysicsProbe {
        beam_overlap: bool,
        desired_stance: Stance,
        actual_stance: Stance,
        physical_crouched: bool,
        headroom: f32,
        sync_error: f32,
        phase: AlsCrouchTransactionPhase,
        retry_count: u32,
        transaction_half_height: f32,
        transaction_center: Vec3,
        transaction_controller_crouch: bool,
        transaction_headroom_blocked: bool,
    }

    fn record_standing_capsule_physics_probe(
        spatial_query: SpatialQuery,
        cases: Query<
            (
                Entity,
                &StandingCapsulePhysicsCase,
                &Collider,
                &Position,
                &Transform,
                &Stance,
                &AlsDesiredStance,
                &TnuaConfig<AlsControlScheme>,
                Has<AlsCapsuleCrouched>,
                &AlsCrouchTransaction,
            ),
            With<AlsCharacter>,
        >,
        beams: Query<(), With<StandingCapsuleBeam>>,
        configs: Res<Assets<AlsControlSchemeConfig>>,
        mut probe: ResMut<StandingCapsulePhysicsProbe>,
    ) {
        let Ok((
            entity,
            case,
            collider,
            position,
            transform,
            stance,
            desired_stance,
            config,
            physical_crouched,
            transaction,
        )) = cases.single()
        else {
            return;
        };
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        probe.beam_overlap = spatial_query
            .shape_intersections(collider, transform.translation, transform.rotation, &filter)
            .into_iter()
            .any(|other| other == case.beam && beams.get(other).is_ok());
        probe.desired_stance = desired_stance.0;
        probe.actual_stance = *stance;
        probe.physical_crouched = physical_crouched;
        probe.headroom = configs
            .get(&config.0)
            .and_then(|config| config.basis.headroom.as_ref())
            .map(|headroom| headroom.distance_to_collider_top)
            .unwrap_or(f32::NAN);
        probe.sync_error = transform.translation.distance(position.0);
        probe.phase = transaction.phase;
        probe.retry_count = transaction.blocked_retry_count;
        probe.transaction_half_height = transaction.collider_half_height;
        probe.transaction_center = transaction.collider_center;
        probe.transaction_controller_crouch = transaction.controller_crouch_active;
        probe.transaction_headroom_blocked = transaction.headroom_blocked;
    }

    #[test]
    fn yaw_caps_match_als_rotation_speeds() {
        assert!((turn_angvel(RotationMode::VelocityDirection) - 800f32.to_radians()).abs() < 1e-4);
        assert!((turn_angvel(RotationMode::ViewDirection) - 500f32.to_radians()).abs() < 1e-4);
        assert!((turn_angvel(RotationMode::Aiming) - 1000f32.to_radians()).abs() < 1e-4);
        assert!(turn_angvel(RotationMode::Aiming) > turn_angvel(RotationMode::VelocityDirection));
        assert!(
            turn_angvel(RotationMode::VelocityDirection) > turn_angvel(RotationMode::ViewDirection)
        );
    }

    #[test]
    fn aiming_yaw_constraint_matches_source_reset_and_primary_limit() {
        let (first_yaw, first_limit) =
            constrained_aiming_yaw(180.0_f32.to_radians(), 0.0, 70.0, 70.0, true, 0.0, false);
        assert!(wrap_yaw_radians(first_yaw).abs() < 1e-6);
        assert!((first_limit - 180.0).abs() < 1e-6);

        let (limited_yaw, limited_limit) =
            constrained_aiming_yaw(180.0_f32.to_radians(), 0.0, 70.0, 70.0, false, 0.0, false);
        assert!((limited_yaw.abs() - 110.0_f32.to_radians()).abs() < 1e-6);
        assert!((limited_limit - 70.0).abs() < 1e-6);

        let (near_yaw, near_limit) =
            constrained_aiming_yaw(20.0_f32.to_radians(), 0.0, 70.0, 180.0, false, 0.0, false);
        assert!((near_yaw - 0.0).abs() < 1e-6);
        assert!((near_limit - 70.0).abs() < 1e-6);
    }

    #[test]
    fn aiming_secondary_preserves_the_source_half_turn_side() {
        for (view, expected) in [(180.0_f32, 110.0_f32), (-180.0, 110.0), (179.9, 109.9), (-179.9, -109.9)] {
            let (yaw, limit) = constrained_aiming_yaw(view.to_radians(), 0.0, 70.0, 70.0, false, 0.0, true);
            assert!((yaw.to_degrees() - expected).abs() < 1e-4, "view={view} yaw={} expected={expected}", yaw.to_degrees());
            assert!((limit - 70.0).abs() < 1e-4);
        }
        for delta in [0.0_f32, 0.001 / 60.0, 1.0 / 60.0] {
            let (yaw, limit) = constrained_aiming_yaw(std::f32::consts::PI, 0.0, 70.0, 70.0, false, delta, true);
            assert!((yaw.to_degrees() - 110.0).abs() < 1e-4, "delta={delta} yaw={}", yaw.to_degrees());
            assert!((limit - 70.0).abs() < 1e-4);
        }
    }

    #[test]
    fn frozen_owner_aiming_half_turn_keeps_the_source_side() {
        for dilation in [0.0_f32, 0.001, 1.0] {
            let mut app = App::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            app.insert_resource(time).init_resource::<Assets<AlsMovementParams>>()
                .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
                .add_systems(Update, rotation_response_system);
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
            let actor = app.world_mut().spawn((AlsCharacter, Transform::IDENTITY,
                Stance::Standing, Gait::Walking, LocomotionState::Grounded, RotationMode::Aiming,
                AlsViewYaw(std::f32::consts::PI), AlsMovementParamsHandle(params), LinearVelocity::ZERO,
                AlsInputCommand::default(), crate::als_camera::AlsCameraCustomTimeDilation(dilation),
                AlsAimingYawConstraint { active_limit_degrees: 70.0, applied_last_frame: true },
                AlsRotationResponseState { initialized: true, previous_rotation_mode: Some(RotationMode::Aiming),
                    previous_view_yaw: std::f32::consts::PI, ..default() })).id();
            app.update();
            let yaw = world_forward_to_yaw(app.world().get::<Transform>(actor).unwrap().rotation * Vec3::NEG_Z);
            assert!((yaw.to_degrees() - 110.0).abs() < 1e-4, "dilation={dilation} yaw={}", yaw.to_degrees());
            assert!((app.world().get::<AlsAimingYawConstraint>(actor).unwrap().active_limit_degrees - 70.0).abs() < 1e-4);
        }
    }

    #[test]
    fn stationary_aiming_secondary_constraint_matches_the_source_half_life() {
        let (yaw, limit) =
            constrained_aiming_yaw(100.0_f32.to_radians(), 0.0, 70.0, 70.0, true, 0.1, true);
        assert!((yaw - 15.00338_f32.to_radians()).abs() < 1e-5);
        assert!((limit - 84.99662).abs() < 1e-5);
    }

    #[test]
    fn rotation_response_uses_half_life_and_target_speed_settings() {
        let half = damper_yaw(0.0, std::f32::consts::FRAC_PI_2, 0.1, 0.1);
        assert!((half - 45.01014_f32.to_radians()).abs() < 1e-5);
        let advanced = advance_yaw(0.0, std::f32::consts::FRAC_PI_2, 500.0, 0.1);
        assert!((advanced - 50.0_f32.to_radians()).abs() < 1e-5);
        assert!((advance_yaw(0.0, -176.0_f32.to_radians(), 500.0, 0.1) - advanced).abs() < 1e-5);
        assert!((damper_yaw(0.0, -176.0_f32.to_radians(), 0.1, 0.1) - half * 184.0 / 90.0).abs() < 1e-5);
        assert_eq!(advance_yaw(0.0, 1.0, 0.0, 0.1), 1.0);
        let params = AlsMovementParams::default();
        let normal = moving_rotation_half_life(
            &params,
            2.0,
            RotationMode::ViewDirection,
            true,
            0.0,
            Stance::Standing,
        );
        let responsive = moving_rotation_half_life(
            &params,
            2.0,
            RotationMode::ViewDirection,
            true,
            params.rotation.reference_view_yaw_speed,
            Stance::Standing,
        );
        assert!((normal - 0.15).abs() < 1e-6);
        assert!((responsive - 0.05).abs() < 1e-6);
        for stance in [Stance::Standing, Stance::Crouching] {
            for (moving, expected) in [(false, 0.1), (true, if stance == Stance::Standing { 0.05 / 3.0 } else { 0.05 })] {
                let actual = moving_rotation_half_life(&params, 2.0, RotationMode::VelocityDirection,
                    moving, params.rotation.reference_view_yaw_speed, stance);
                assert!((actual - expected).abs() < 1e-6,
                    "Source VelocityDirection half-life differs: stance={stance:?} moving={moving} actual={actual}");
            }
        }
    }

    #[test]
    fn rotation_response_uses_view_yaw_degrees_per_second() {
        let mut failures = Vec::new();
        for (previous_view_degrees, view_degrees, custom_dilation) in [
            (0.0_f32, 90.0_f32, 1.0_f32),
            (0.0, 10.0, 0.5),
            (179.0, -179.0, 1.0),
        ] {
            let mut app = App::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_millis(100));
            app.insert_resource(time)
                .init_resource::<Assets<AlsMovementParams>>()
                .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
                .add_systems(Update, rotation_response_system);
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
                .add(AlsMovementParams::default());
            let character = app.world_mut().spawn((
                AlsCharacter, Stance::Standing, Transform::IDENTITY, Gait::Running,
                LocomotionState::Grounded, RotationMode::ViewDirection,
                AlsViewYaw(view_degrees.to_radians()), AlsMovementParamsHandle(params),
                crate::als_camera::AlsCameraCustomTimeDilation(custom_dilation),
                LinearVelocity(Vec3::NEG_Z * 1.75),
                AlsInputCommand { move_dir: Vec2::Y, ..default() },
                crate::actual_motion::AlsActualMotion { has_input: true, input_yaw_degrees: 90.0, ..default() },
                AlsRotationResponseState { initialized: true, previous_view_yaw: previous_view_degrees.to_radians(),
                    previous_rotation_mode: Some(RotationMode::ViewDirection), ..default() },
            )).id();
            app.update();
            let yaw = world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward());
            let actor_delta = 0.1 * custom_dilation;
            let source_speed = (view_degrees - previous_view_degrees).abs() / actor_delta;
            let half_life = 0.15 * (1.0 + (0.333333 - 1.0) * (source_speed / 300.0).clamp(0.0, 1.0));
            let source_target = advance_yaw(0.0, view_degrees.to_radians(), 500.0, actor_delta);
            let expected = source_target
                * (1.0 - crate::als::animation::inv_exp_approx(std::f32::consts::LN_2 / (half_life + 1e-8) * actor_delta));
            if (yaw - expected).abs() >= 1e-5 {
                failures.push(format!("Actor rotation differs: yaw={yaw} expected={expected}, dilation={custom_dilation}, previous={previous_view_degrees}, view={view_degrees}"));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn first_person_non_aiming_uses_source_body_aiming_rotation() {
        for (locomotion, mode, velocity, input, view_degrees, expected_degrees, limit_active) in [
            (LocomotionState::Grounded, RotationMode::ViewDirection, Vec3::ZERO, Vec2::ZERO, 100.0_f32, 15.00338_f32, false),
            (LocomotionState::Grounded, RotationMode::ViewDirection, Vec3::NEG_Z * 1.75, Vec2::Y, 90.0, 45.01014, false),
            (LocomotionState::Grounded, RotationMode::VelocityDirection, Vec3::X * 1.75, Vec2::Y, 90.0, 45.01014, false),
            (LocomotionState::Grounded, RotationMode::ViewDirection, Vec3::NEG_Z * 1.75, Vec2::Y, 170.0, 100.0, true),
            (LocomotionState::Air, RotationMode::VelocityDirection, Vec3::X * 3.75, Vec2::Y, 90.0, 45.01014, false),
        ] {
            let mut app = App::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_millis(100));
            app.insert_resource(time)
                .init_resource::<Assets<AlsMovementParams>>()
                .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
                .add_systems(Update, rotation_response_system);
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
                .add(AlsMovementParams::default());
            let view = view_degrees.to_radians();
            let character = app.world_mut().spawn((
                AlsCharacter, crate::als::input_request::AlsFirstPersonView, Stance::Standing,
                Transform::IDENTITY, Gait::Running, locomotion, mode, AlsViewYaw(view),
                AlsMovementParamsHandle(params), LinearVelocity(velocity),
                AlsInputCommand { move_dir: input, ..default() },
                crate::actual_motion::AlsActualMotion { has_input: input != Vec2::ZERO, input_yaw_degrees: view_degrees, ..default() },
                AlsRotationResponseState { initialized: true, previous_view_yaw: view,
                    previous_locomotion: locomotion, previous_rotation_mode: Some(mode), ..default() },
                AlsAimingYawConstraint { active_limit_degrees: if limit_active { 70.0 } else { 180.0 },
                    applied_last_frame: limit_active },
            )).id();
            app.update();
            let yaw = world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward());
            assert!((yaw - expected_degrees.to_radians()).abs() < 1e-4,
                "locomotion={locomotion:?} mode={mode:?} yaw={yaw} expected={expected_degrees}");
            let response = app.world().get::<AlsRotationResponseState>(character).unwrap();
            assert!((response.target_yaw - view).abs() < 1e-6);
            assert!(response.rotation_towards_last_input_direction_blocked);
            assert!(app.world().get::<AlsAimingYawConstraint>(character).unwrap().applied_last_frame);
        }
    }

    #[test]
    fn source_default_air_rotation_keeps_actor_yaw_and_resets_the_target() {
        for (mode, velocity, input) in [
            (RotationMode::VelocityDirection, Vec3::X * 3.75, Vec2::Y),
            (RotationMode::ViewDirection, Vec3::NEG_Z * 3.75, Vec2::Y),
            (RotationMode::ViewDirection, Vec3::ZERO, Vec2::ZERO),
        ] {
            let mut app = App::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_millis(100));
            app.insert_resource(time)
                .init_resource::<Assets<AlsMovementParams>>()
                .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
                .add_systems(Update, rotation_response_system);
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
                .add(AlsMovementParams::default());
            let actor_yaw = 0.3_f32;
            let character = app.world_mut().spawn((
                AlsCharacter, Stance::Standing,
                Transform::IDENTITY.with_rotation(Quat::from_rotation_y(actor_yaw)),
                Gait::Running, LocomotionState::Air, mode, AlsViewYaw(1.0),
                AlsMovementParamsHandle(params), LinearVelocity(velocity),
                AlsInputCommand { move_dir: input, ..default() },
                crate::actual_motion::AlsActualMotion { has_input: input != Vec2::ZERO, input_yaw_degrees: 1.0_f32.to_degrees(), ..default() },
                AlsRotationResponseState { initialized: true, smooth_target_yaw: 1.0,
                    target_yaw: 1.0, previous_locomotion: LocomotionState::Air,
                    previous_rotation_mode: Some(mode), ..default() },
            )).id();
            app.update();
            let yaw = world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward());
            let response = app.world().get::<AlsRotationResponseState>(character).unwrap();
            assert!((yaw - actor_yaw).abs() < 1e-6, "mode={mode:?} input={input:?} yaw={yaw}");
            assert!((response.target_yaw - actor_yaw).abs() < 1e-6);
            assert!((response.smooth_target_yaw - actor_yaw).abs() < 1e-6);
            assert!(response.rotation_towards_last_input_direction_blocked);
        }
    }

    #[test]
    fn grounded_rotation_samples_the_continuous_source_gait_amount() {
        for (mode, stance, velocity, amount, target_degrees, half_life) in [
            (RotationMode::ViewDirection, Stance::Standing, Vec3::NEG_Z * 5.125, 2.5, 50.0_f32, 0.1_f32),
            (RotationMode::VelocityDirection, Stance::Standing, Vec3::X * 2.75, 1.5, -80.0, 0.075),
            (RotationMode::VelocityDirection, Stance::Crouching, Vec3::X * 1.75, 1.5, -80.0, 0.15),
        ] {
            let mut app = App::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_millis(100));
            app.insert_resource(time)
                .init_resource::<Assets<AlsMovementParams>>()
                .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
                .add_systems(Update, rotation_response_system);
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
                .add(AlsMovementParams::default());
            let view = 90.0_f32.to_radians();
            let character = app.world_mut().spawn((
                AlsCharacter, stance, Transform::IDENTITY, Gait::Running,
                crate::als::gait::AlsGaitAmount(amount), LocomotionState::Grounded,
                mode, AlsViewYaw(view), AlsMovementParamsHandle(params),
                LinearVelocity(velocity), AlsInputCommand { move_dir: Vec2::Y, ..default() },
                crate::actual_motion::AlsActualMotion { has_input: true, input_yaw_degrees: 90.0, ..default() },
                AlsRotationResponseState { initialized: true, previous_view_yaw: view,
                    previous_rotation_mode: Some(mode), ..default() },
            )).id();
            app.update();
            let yaw = world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward());
            let expected = target_degrees.to_radians()
                * (1.0 - crate::als::animation::inv_exp_approx(std::f32::consts::LN_2 / (half_life + 1e-8) * 0.1));
            assert!((yaw - expected).abs() < 1e-5, "mode={mode:?} stance={stance:?} yaw={yaw} expected={expected}");
        }
    }

    #[test]
    fn landing_residual_speed_keeps_rotation_blocked_until_new_input() {
        use bevy::ecs::system::RunSystemOnce;
        for mode in [RotationMode::VelocityDirection, RotationMode::ViewDirection] {
            let mut app = App::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_millis(100));
            app.insert_resource(time)
                .init_resource::<Assets<AlsMovementParams>>()
                .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
                .add_systems(Update, rotation_response_system);
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
                .add(AlsMovementParams::default());
            let actor_yaw = 0.4_f32;
            let character = app.world_mut().spawn((
                AlsCharacter, Stance::Standing,
                Transform::IDENTITY.with_rotation(Quat::from_rotation_y(actor_yaw)),
                Gait::Running, LocomotionState::Grounded, mode, AlsViewYaw(-1.0),
                AlsMovementParamsHandle(params), LinearVelocity(Vec3::X),
                AlsInputCommand::default(),
                AlsRotationResponseState { initialized: true, smooth_target_yaw: -1.0,
                    target_yaw: -1.0, previous_locomotion: LocomotionState::Air,
                    previous_rotation_mode: Some(mode), ..default() },
            )).id();
            app.world_mut().entity_mut(character).insert(crate::als::dynamics::AlsLandingBrakingFriction {
                previous_locomotion: LocomotionState::Air, ..default()
            });
            app.init_resource::<Messages<StateTransitionEvent>>();
            app.world_mut().run_system_once(crate::als::dynamics::landing_braking_friction_system).unwrap();
            for velocity in [Vec3::X, Vec3::X * 0.1, Vec3::ZERO] {
                app.world_mut().entity_mut(character).insert(LinearVelocity(velocity));
                app.update();
                let yaw = world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward());
                let response = app.world().get::<AlsRotationResponseState>(character).unwrap();
                assert!(response.rotation_towards_last_input_direction_blocked, "mode={mode:?} velocity={velocity:?}");
                assert!((yaw - actor_yaw).abs() < 1e-6, "mode={mode:?} velocity={velocity:?} yaw={yaw}");
                assert!((response.target_yaw - actor_yaw).abs() < 1e-6);
                assert!((response.smooth_target_yaw - actor_yaw).abs() < 1e-6);
            }
            app.world_mut().entity_mut(character).insert((LinearVelocity(Vec3::X),
                AlsInputCommand { move_dir: Vec2::Y, ..default() },
                crate::actual_motion::AlsActualMotion { has_input: true, input_yaw_degrees: -1.0_f32.to_degrees(), ..default() }));
            app.update();
            assert!(!app.world().get::<AlsRotationResponseState>(character).unwrap()
                .rotation_towards_last_input_direction_blocked);
        }
    }

    #[test]
    fn velocity_direction_uses_the_precollision_motor_heading() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(100));
        app.insert_resource(time)
            .init_resource::<Assets<AlsMovementParams>>()
            .register_required_components::<AlsCharacter, crate::actual_motion::AlsActualMotion>()
            .add_systems(Update, rotation_response_system);
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
            .add(AlsMovementParams::default());
        let support = Vec3::X * 2.0;
        let platform = app.world_mut().spawn_empty().id();
        let sideways = Vec3::X * 3.75;
        let intended = Vec3::NEG_Z * 3.75;
        let character = app.world_mut().spawn((
            AlsCharacter, Stance::Standing, Transform::IDENTITY, Gait::Running,
            LocomotionState::Grounded, RotationMode::VelocityDirection, AlsViewYaw(0.0),
            AlsMovementParamsHandle(params), LinearVelocity(sideways + support),
            AlsInputCommand { move_dir: Vec2::Y, ..default() },
            crate::actual_motion::AlsActualMotion { has_input: true, input_yaw_degrees: 0.0, ..default() },
            crate::als::dynamics::AlsMovementBaseDynamicsState {
                entity: Some(platform), support_velocity: support, relative_velocity: sideways,
                world_velocity: sideways + support, motor_boost: intended - sideways,
                pre_collision_velocity: Some(intended), ..default()
            },
            AlsRotationResponseState { initialized: true,
                rotation_towards_last_input_direction_blocked: false,
                previous_rotation_mode: Some(RotationMode::VelocityDirection), ..default() },
        )).id();
        app.update();
        let response = app.world().get::<AlsRotationResponseState>(character).unwrap();
        let yaw = world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward());
        assert!(response.target_yaw.abs() < 1e-6, "postcollision yaw was selected: {}", response.target_yaw);
        assert!(yaw.abs() < 1e-6, "actor followed collision slide: {yaw}");
        assert!(app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character)
            .unwrap().pre_collision_velocity.is_none());
        app.update();
        let fallback = app.world().get::<AlsRotationResponseState>(character).unwrap().target_yaw;
        assert!((fallback + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character)
            .unwrap().pre_collision_velocity = Some(Vec3::NEG_Z * 0.005);
        app.update();
        let low_speed_fallback = app.world().get::<AlsRotationResponseState>(character).unwrap().target_yaw;
        assert!((low_speed_fallback + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!(app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character)
            .unwrap().pre_collision_velocity.is_none());
        app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character)
            .unwrap().pre_collision_velocity = Some(Vec3::ZERO);
        app.update();
        assert!((app.world().get::<AlsRotationResponseState>(character).unwrap().target_yaw
            + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!(app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character)
            .unwrap().pre_collision_velocity.is_none());
    }

    #[test]
    fn view_relative_move_axes_match_camera_forward_and_right() {
        let cases = [
            (0.0, Vec2::Y, Vec3::NEG_Z),
            (0.0, Vec2::X, Vec3::X),
            (std::f32::consts::FRAC_PI_2, Vec2::Y, Vec3::NEG_X),
            (std::f32::consts::FRAC_PI_2, Vec2::X, Vec3::NEG_Z),
            (std::f32::consts::PI, Vec2::Y, Vec3::Z),
            (-std::f32::consts::FRAC_PI_2, Vec2::Y, Vec3::X),
        ];
        for (yaw, input, expected) in cases {
            let actual = view_relative_move_direction(input, yaw);
            assert!(
                actual.distance(expected) < 1e-5,
                "yaw={yaw} input={input:?}"
            );
        }
    }

    #[test]
    fn movement_direction_rounds_source_degrees_before_radians() {
        for (yaw, expected_z) in [
            (-1.5707480907440186_f32, -4.823604831472039e-5_f32),
            (-1.570765733718872_f32, -3.059307346120477e-5_f32),
        ] {
            let actual = view_relative_move_direction(Vec2::Y, yaw);
            assert_eq!(actual, Vec3::new(1.0, 0.0, expected_z));
        }
        assert_eq!(view_relative_move_direction(Vec2::ZERO, 0.7), Vec3::ZERO);
    }

    #[test]
    fn movement_input_preserves_subunit_rotation_and_analog_magnitude() {
        let yaw = -std::f32::consts::PI * 0.75;
        let unit = view_relative_move_input(Vec2::Y, yaw);
        assert!(unit.length_squared() < 1.0);
        assert_eq!(view_relative_move_input(Vec2::Y * 0.5, yaw), unit * 0.5);
        assert_eq!(view_relative_move_input(Vec2::Y * 2.0, yaw), unit);
        assert_eq!(view_relative_move_input(Vec2::ZERO, yaw), Vec3::ZERO);
        assert!((view_relative_move_direction(Vec2::Y * 0.5, yaw).length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn movement_input_uses_source_float_sine_and_cosine() {
        let yaw = -std::f32::consts::PI * 0.75;
        let actual = view_relative_move_input(Vec2::Y, yaw);
        assert_eq!(actual, Vec3::new(0.7071067094802856, 0.0, 0.7071067690849304));
    }

    #[test]
    fn movement_input_clamps_before_projecting_to_float() {
        for (yaw, expected) in [
            (-30.0_f32.to_radians(), Vec3::new(0.9659258127212524, 0.0, -0.2588191032409668)),
            (-150.0_f32.to_radians(), Vec3::new(-0.2588188946247101, 0.0, 0.9659258723258972)),
        ] {
            assert_eq!(view_relative_move_input(Vec2::ONE, yaw), expected);
        }
    }

    #[test]
    fn view_relative_move_direction_matches_source_axes() {
        let cases = [
            (0.0, Vec3::NEG_Z),
            (std::f32::consts::FRAC_PI_2, Vec3::NEG_X),
            (std::f32::consts::PI, Vec3::Z),
            (-std::f32::consts::FRAC_PI_2, Vec3::X),
        ];
        for (yaw, expected) in cases {
            let locomotion = view_relative_move_direction(Vec2::Y, yaw);
            assert!(locomotion.distance(expected) < 1e-5, "yaw={yaw}");
        }
    }

    #[test]
    fn desired_stance_waits_for_clearance_before_actual_stance_changes() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            PhysicsPlugins::new(FixedPostUpdate),
            TransformPlugin,
            TnuaAvian3dPlugin::new(FixedUpdate),
            TnuaControllerPlugin::<AlsControlScheme>::new(FixedUpdate),
            AlsStateTransitionPlugin,
            AlsCharacterPlugin,
            AlsMotionPlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .add_message::<AssetEvent<Mesh>>()
        .insert_resource(StandingCapsulePhysicsProbe {
            beam_overlap: false,
            desired_stance: Stance::Crouching,
            actual_stance: Stance::Crouching,
            physical_crouched: true,
            headroom: f32::NAN,
            sync_error: f32::NAN,
            phase: AlsCrouchTransactionPhase::Crouching,
            retry_count: 0,
            transaction_half_height: f32::NAN,
            transaction_center: Vec3::ZERO,
            transaction_controller_crouch: false,
            transaction_headroom_blocked: false,
        })
        .add_systems(
            Update,
            record_standing_capsule_physics_probe.after(capsule_reshape_system),
        );
        app.finish();
        let params_value = AlsMovementParams::default();
        let params = app
            .world_mut()
            .resource_mut::<Assets<AlsMovementParams>>()
            .add(params_value.clone());
        let config = app
            .world_mut()
            .resource_mut::<Assets<AlsControlSchemeConfig>>()
            .add(initial_als_control_scheme_config(&params_value));
        let beam = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(0.08, 0.1, 0.4),
                Transform::from_xyz(0.26, 1.65, 0.0),
                StandingCapsuleBeam,
            ))
            .id();
        let center = Vec3::new(0.0, params_value.crouch.crouched_capsule_half_height, 0.0);
        let mut bundle = AlsCharacterBundle::new(params, config);
        bundle.transform = Transform::from_translation(center);
        bundle.rigid_body = RigidBody::Kinematic;
        bundle.collider = als_character_capsule(
            params_value.crouch.crouched_capsule_half_height,
            params_value.crouch.standing_capsule_radius,
        );
        bundle.stance = Stance::Crouching;
        bundle.desired_stance = AlsDesiredStance(Stance::Crouching);
        let character = app
            .world_mut()
            .spawn((
                bundle,
                Position(center),
                LinearVelocity::ZERO,
                AlsCapsuleCrouched,
                StandingCapsulePhysicsCase { beam },
            ))
            .id();

        app.update();
        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().run_schedule(FixedPostUpdate);
        app.world_mut()
            .entity_mut(character)
            .get_mut::<AlsStateRequest>()
            .unwrap()
            .stance = Some(Stance::Standing);
        for _ in 0..2 {
            app.update();
            app.world_mut().run_schedule(FixedUpdate);
            app.world_mut().run_schedule(FixedPostUpdate);
        }

        let blocked_probe = {
            let probe = app.world().resource::<StandingCapsulePhysicsProbe>();
            (
                probe.desired_stance,
                probe.actual_stance,
                probe.physical_crouched,
                probe.headroom,
                probe.beam_overlap,
                probe.sync_error,
                probe.phase,
                probe.retry_count,
                probe.transaction_half_height,
                probe.transaction_center,
                probe.transaction_controller_crouch,
                probe.transaction_headroom_blocked,
            )
        };
        app.world_mut().despawn(beam);
        app.world_mut().run_schedule(FixedPostUpdate);
        for _ in 0..2 {
            app.update();
            app.world_mut().run_schedule(FixedUpdate);
            app.world_mut().run_schedule(FixedPostUpdate);
        }

        let restored_probe = {
            let probe = app.world().resource::<StandingCapsulePhysicsProbe>();
            (
                probe.desired_stance,
                probe.actual_stance,
                probe.physical_crouched,
                probe.headroom,
                probe.beam_overlap,
                probe.sync_error,
                probe.phase,
                probe.retry_count,
                probe.transaction_half_height,
                probe.transaction_center,
                probe.transaction_controller_crouch,
                probe.transaction_headroom_blocked,
            )
        };
        println!(
            "[BF102.02] blocked_desired={:?} blocked_actual={:?} blocked_crouched={} blocked_headroom={:.6} blocked_overlap={} blocked_sync_error={:.6} blocked_phase={:?} blocked_retries={} blocked_half={:.6} blocked_center_y={:.6} blocked_controller={} blocked_headroom_state={} restored_desired={:?} restored_actual={:?} restored_crouched={} restored_headroom={:.6} restored_overlap={} restored_sync_error={:.6} restored_phase={:?} restored_retries={} restored_half={:.6} restored_center_y={:.6} restored_controller={} restored_headroom_state={}",
            blocked_probe.0,
            blocked_probe.1,
            blocked_probe.2,
            blocked_probe.3,
            blocked_probe.4,
            blocked_probe.5,
            blocked_probe.6,
            blocked_probe.7,
            blocked_probe.8,
            blocked_probe.9.y,
            blocked_probe.10,
            blocked_probe.11,
            restored_probe.0,
            restored_probe.1,
            restored_probe.2,
            restored_probe.3,
            restored_probe.4,
            restored_probe.5,
            restored_probe.6,
            restored_probe.7,
            restored_probe.8,
            restored_probe.9.y,
            restored_probe.10,
            restored_probe.11,
        );
        assert_eq!(blocked_probe.0, Stance::Standing);
        assert_eq!(blocked_probe.1, Stance::Crouching);
        assert!(blocked_probe.2);
        assert!((blocked_probe.3 - params_value.crouch.crouched_capsule_half_height).abs() < 1e-4);
        assert!(!blocked_probe.4);
        assert!(blocked_probe.5 < 1e-4);
        assert_eq!(
            blocked_probe.6,
            AlsCrouchTransactionPhase::BlockedFromStanding
        );
        assert!(blocked_probe.7 >= 1);
        assert!((blocked_probe.8 - params_value.crouch.crouched_capsule_half_height).abs() < 1e-4);
        assert!(
            (blocked_probe.9.y - params_value.crouch.crouched_capsule_half_height).abs() < 1e-4
        );
        assert!(blocked_probe.10);
        assert!(blocked_probe.11);
        assert_eq!(restored_probe.0, Stance::Standing);
        assert_eq!(restored_probe.1, Stance::Standing);
        assert!(!restored_probe.2);
        assert!((restored_probe.3 - params_value.crouch.standing_capsule_half_height).abs() < 1e-4);
        assert!(!restored_probe.4);
        assert!(restored_probe.5 < 1e-4);
        assert_eq!(restored_probe.6, AlsCrouchTransactionPhase::Standing);
        assert_eq!(restored_probe.7, 0);
        assert!((restored_probe.8 - params_value.crouch.standing_capsule_half_height).abs() < 1e-4);
        assert!(
            (restored_probe.9.y - params_value.crouch.standing_capsule_half_height).abs() < 1e-4
        );
        assert!(!restored_probe.10);
        assert!(!restored_probe.11);
        println!(
            "[CROUCH-TRANSACTION][PHYSICS] blocked_retries={} restored_phase={:?} -> PASS",
            blocked_probe.7, restored_probe.6,
        );
    }

    #[test]
    fn crouch_transaction_pairs_and_entities_remain_isolated() {
        let mut first = AlsCrouchTransaction::default();
        let mut second = AlsCrouchTransaction::default();
        for _ in 0..100 {
            first.synchronize(
                Stance::Crouching,
                Stance::Standing,
                0.9,
                0.3,
                Vec3::ZERO,
                false,
            );
            first.crouch_applied(0.56, 0.3, Vec3::ZERO);
            first.synchronize(
                Stance::Standing,
                Stance::Crouching,
                0.56,
                0.3,
                Vec3::ZERO,
                false,
            );
            first.standing_applied(0.9, 0.3, Vec3::ZERO);
        }
        second.synchronize(
            Stance::Crouching,
            Stance::Standing,
            0.9,
            0.3,
            Vec3::ZERO,
            false,
        );
        second.crouch_applied(0.56, 0.3, Vec3::ZERO);
        second.synchronize(
            Stance::Standing,
            Stance::Crouching,
            0.56,
            0.3,
            Vec3::ZERO,
            false,
        );
        second.stand_blocked(0.56, 0.3, Vec3::ZERO);
        assert_eq!(first.actual_stance, Stance::Standing);
        assert_eq!(first.phase, AlsCrouchTransactionPhase::ExitingCrouch);
        assert_eq!(first.blocked_retry_count, 0);
        assert_eq!(second.actual_stance, Stance::Crouching);
        assert_eq!(second.phase, AlsCrouchTransactionPhase::BlockedFromStanding);
        assert_eq!(second.blocked_retry_count, 1);
        println!(
            "[CROUCH-TRANSACTION][VERDICT] pairs=100 first={:?} second={:?} retries={} -> PASS",
            first.phase, second.phase, second.blocked_retry_count,
        );
    }

    #[test]
    fn airborne_crouch_keeps_intent_but_targets_a_standing_capsule() {
        assert_eq!(
            capsule_target_for_locomotion(
                Stance::Crouching,
                Stance::Standing,
                LocomotionState::Grounded,
                false,
                None,
            ),
            Stance::Crouching
        );
        assert_eq!(
            capsule_target_for_locomotion(
                Stance::Crouching,
                Stance::Standing,
                LocomotionState::Air,
                false,
                None,
            ),
            Stance::Standing
        );
        assert_eq!(
            capsule_target_for_locomotion(
                Stance::Standing,
                Stance::Crouching,
                LocomotionState::Air,
                true,
                None,
            ),
            Stance::Crouching
        );
        assert_eq!(
            capsule_target_for_locomotion(
                Stance::Standing,
                Stance::Standing,
                LocomotionState::Air,
                false,
                None,
            ),
            Stance::Standing
        );
        assert_eq!(
            capsule_target_for_locomotion(
                Stance::Crouching,
                Stance::Standing,
                LocomotionState::Air,
                true,
                None,
            ),
            Stance::Standing
        );
        assert_eq!(
            capsule_target_for_locomotion(
                Stance::Standing,
                Stance::Standing,
                LocomotionState::Grounded,
                false,
                Some(Stance::Crouching),
            ),
            Stance::Crouching
        );
    }

    #[test]
    fn physical_stance_override_releases_to_unchanged_desired_stance() {
        assert_eq!(
            capsule_target_for_locomotion(
                Stance::Standing,
                Stance::Crouching,
                LocomotionState::Grounded,
                false,
                Some(Stance::Crouching),
            ),
            Stance::Crouching
        );
        assert_eq!(
            capsule_target_for_locomotion(
                Stance::Standing,
                Stance::Crouching,
                LocomotionState::Grounded,
                false,
                None,
            ),
            Stance::Standing
        );
    }

    #[test]
    fn stance_capsule_centers_preserve_the_support_base() {
        let params = AlsMovementParams::default();
        let standing_center = Vec3::Y * params.crouch.standing_capsule_half_height;
        let crouching_center = capsule_center_for_stance(
            &params,
            standing_center,
            Stance::Standing,
            Stance::Crouching,
            1.0,
        );
        let restored_center = capsule_center_for_stance(
            &params,
            crouching_center,
            Stance::Crouching,
            Stance::Standing,
            1.0,
        );
        let standing_base = standing_center.y - params.crouch.standing_capsule_half_height;
        let crouching_base = crouching_center.y - params.crouch.crouched_capsule_half_height;
        assert!(standing_base.abs() < 1e-6);
        assert!((standing_base - crouching_base).abs() < 1e-6);
        assert!(restored_center.distance(standing_center) < 1e-6);
    }

    #[test]
    fn scaled_stance_capsule_centers_preserve_the_source_base() {
        let params = AlsMovementParams::default();
        for scale in [Vec3::splat(2.0), Vec3::new(-2.0, 3.0, 4.0)] {
            let effective_scale = scale.abs().min_element();
            let standing_half = params.crouch.standing_capsule_half_height * effective_scale;
            let crouched_half = params.crouch.crouched_capsule_half_height * effective_scale;
            let standing_center = Vec3::Y * standing_half;
            let crouched_center = capsule_center_for_stance(&params, standing_center,
                Stance::Standing, Stance::Crouching, effective_scale);
            assert!((crouched_center.y - crouched_half).abs() < 1e-6,
                "scale={scale:?} center={crouched_center:?} expected={crouched_half}");
        }
    }

    #[test]
    fn stand_up_uses_blocking_floor_clearance_before_committing() {
        use crate::character_assembly::{AlsCharacterAvatar, avatar_transform, sync_avatar_stance_offset};
        use crate::als::character::AlsCollisionLayer;
        for (scale, mode, gap, ceiling, floor_kind, standing_y) in [
            (1.0, LocomotionState::Grounded, 0.0215, 1.81, "solid", Some(0.90002)),
            (1.0, LocomotionState::Grounded, 0.0215, 1.79, "solid", None),
            (1.0, LocomotionState::Grounded, 0.0215, 2.0, "solid", Some(0.92151)),
            (1.0, LocomotionState::Air, 0.0215, 1.81, "solid", None),
            (1.0, LocomotionState::Grounded, 0.0215, 1.81, "none", None),
            (1.0, LocomotionState::Grounded, 0.0215, 1.81, "sensor", None),
            (1.0, LocomotionState::Grounded, 0.0215, 1.81, "disjoint", None),
            (1.0, LocomotionState::Grounded, 0.000005, 1.800015, "solid", None),
            (2.0, LocomotionState::Grounded, 0.0215, 3.61, "solid", Some(1.80002)),
        ] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), PhysicsPlugins::new(FixedPostUpdate),
                TransformPlugin, AlsStateTransitionPlugin, AlsCharacterPlugin))
                .init_resource::<Assets<Mesh>>().add_message::<AssetEvent<Mesh>>()
                .add_systems(Update, capsule_reshape_system)
                .add_systems(PostUpdate, sync_avatar_stance_offset.before(bevy::transform::TransformSystems::Propagate));
            app.finish();
            if floor_kind != "none" {
                let mut floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.1, 4.0),
                    Transform::from_xyz(0.0, -0.05, 0.0)));
                if floor_kind == "sensor" { floor.insert(Sensor); }
                if floor_kind == "disjoint" {
                    floor.insert(CollisionLayers::new(AlsCollisionLayer::Visibility, AlsCollisionLayer::Visibility));
                }
            }
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.1, 4.0),
                Transform::from_xyz(0.0, ceiling + 0.05, 0.0)));
            let params_value = AlsMovementParams::default();
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(params_value.clone());
            let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>()
                .add(initial_als_control_scheme_config(&params_value));
            let center = Vec3::Y * (params_value.crouch.crouched_capsule_half_height * scale + gap);
            let mut bundle = AlsCharacterBundle::new(params, config);
            bundle.rigid_body = RigidBody::Kinematic;
            bundle.transform = Transform::from_translation(center).with_scale(Vec3::splat(scale));
            bundle.collider = capsule_for(&params_value, Stance::Crouching);
            bundle.stance = Stance::Crouching;
            bundle.desired_stance = AlsDesiredStance(Stance::Crouching);
            let character = app.world_mut().spawn((bundle, Position(center), GravityScale(0.0), AlsCapsuleCrouched,
                CollisionLayers::new(AlsCollisionLayer::Default, AlsCollisionLayer::Default))).id();
            let avatar = app.world_mut().spawn((AlsCharacterAvatar { character }, ChildOf(character), avatar_transform())).id();
            app.update();
            app.world_mut().run_schedule(FixedPostUpdate);
            *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = mode;
            app.world_mut().get_mut::<AlsDesiredStance>(character).unwrap().0 = Stance::Standing;
            app.update();
            let expected_stance = if standing_y.is_some() { Stance::Standing } else { Stance::Crouching };
            let expected_y = standing_y.unwrap_or(center.y);
            let mesh_offset = if standing_y.is_some() { -0.92 } else { -0.58 };
            assert_eq!(app.world().get::<Stance>(character), Some(&expected_stance),
                "mode={mode:?} floor={floor_kind} scale={scale} gap={gap} ceiling={ceiling}");
            assert_eq!(app.world().get::<AlsDesiredStance>(character).unwrap().0, Stance::Standing);
            assert!((app.world().get::<Position>(character).unwrap().y - expected_y).abs() < 2e-6);
            let transaction = app.world().get::<AlsCrouchTransaction>(character).unwrap();
            assert_eq!(transaction.headroom_blocked, standing_y.is_none());
            assert!((transaction.collider_half_height - if standing_y.is_some() { 0.9 } else { 0.56 }).abs() < 1e-6);
            assert!((app.world().get::<Transform>(avatar).unwrap().translation.y - mesh_offset).abs() < 1e-6);
            assert!((app.world().get::<GlobalTransform>(avatar).unwrap().translation().y
                - expected_y - mesh_offset * scale).abs() < 2e-6);
            app.world_mut().run_schedule(FixedPostUpdate);
            assert_eq!(app.world().get::<Stance>(character), Some(&expected_stance));
            assert!((app.world().get::<Position>(character).unwrap().y - expected_y).abs() < 2e-6);
        }
    }

    #[test]
    fn stand_up_overlap_uses_character_blocking_policy() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::als::character::AlsCollisionLayer;
        for forced in [false, true] {
            for kind in ["sensor", "query_sensor", "disjoint", "solid", "foreign", "owned", "self"] {
                let mut app = App::new();
                app.add_plugins((MinimalPlugins, AssetPlugin::default(), PhysicsPlugins::new(FixedPostUpdate),
                    TransformPlugin, AlsStateTransitionPlugin, AlsCharacterPlugin))
                    .init_resource::<Assets<Mesh>>().add_message::<AssetEvent<Mesh>>()
                    .add_systems(Update, capsule_reshape_system);
                app.finish();
                let params_value = AlsMovementParams::default();
                let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(params_value.clone());
                let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>()
                    .add(initial_als_control_scheme_config(&params_value));
                let center = Vec3::Y * (params_value.crouch.crouched_capsule_half_height + 0.019);
                let mut bundle = AlsCharacterBundle::new(params, config);
                bundle.rigid_body = RigidBody::Kinematic;
                bundle.transform = Transform::from_translation(center);
                bundle.collider = capsule_for(&params_value, Stance::Crouching);
                bundle.stance = Stance::Crouching;
                bundle.desired_stance = AlsDesiredStance(Stance::Crouching);
                let character = app.world_mut().spawn((bundle, Position(center), GravityScale(0.0), AlsCapsuleCrouched,
                    CollisionLayers::new(AlsCollisionLayer::Default, AlsCollisionLayer::Default))).id();
                let obstacle = if kind == "self" { character } else {
                    let mut obstacle = app.world_mut().spawn((Collider::cuboid(1.0, 0.05, 1.0),
                        Transform::from_xyz(0.0, 1.55, 0.0)));
                    match kind {
                        "sensor" => { obstacle.insert((RigidBody::Static, Sensor)); }
                        "query_sensor" => { obstacle.insert(Sensor); }
                        "disjoint" => { obstacle.insert((RigidBody::Static,
                            CollisionLayers::new(AlsCollisionLayer::Visibility, AlsCollisionLayer::Visibility))); }
                        "foreign" => { obstacle.insert((RigidBody::Static, AlsCharacter)); }
                        "owned" => { obstacle.insert((ChildOf(character),
                            Transform::from_xyz(0.0, 1.55 - center.y, 0.0))); }
                        _ => { obstacle.insert(RigidBody::Static); }
                    }
                    obstacle.id()
                };
                app.update();
                app.world_mut().run_schedule(FixedPostUpdate);
                let shape = capsule_for(&params_value, Stance::Standing);
                let standing_center = center + Vec3::Y * (params_value.crouch.standing_capsule_half_height
                    - params_value.crouch.crouched_capsule_half_height);
                app.world_mut().run_system_once(move |spatial: SpatialQuery| {
                    assert!(spatial.shape_intersections(&shape, standing_center, Quat::IDENTITY,
                        &SpatialQueryFilter::default()).contains(&obstacle), "Native overlap missing: {kind}");
                }).unwrap();
                if kind == "owned" {
                    assert_eq!(app.world().get::<ColliderOf>(obstacle).unwrap().body, character);
                }
                if forced {
                    *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
                } else {
                    app.world_mut().get_mut::<AlsDesiredStance>(character).unwrap().0 = Stance::Standing;
                }
                app.update();
                let blocked = matches!(kind, "solid" | "foreign");
                let actual = if blocked { Stance::Crouching } else { Stance::Standing };
                let position = if blocked { center } else { standing_center + Vec3::Y * STAND_UP_SWEEP_INFLATION };
                assert_eq!(app.world().get::<Stance>(character), Some(&actual), "kind={kind} forced={forced}");
                assert_eq!(app.world().get::<AlsCrouchTransaction>(character).unwrap().headroom_blocked, blocked);
                assert_eq!(app.world().get::<AlsDesiredStance>(character).unwrap().0,
                    if forced { Stance::Crouching } else { Stance::Standing });
                assert!(app.world().get::<Position>(character).unwrap().0.distance(position) < 1e-5);
                app.update();
                let transaction = app.world().get::<AlsCrouchTransaction>(character).unwrap();
                assert_eq!(transaction.blocked_retry_count, if blocked { 2 } else { 0 });
                assert_eq!(transaction.headroom_blocked, blocked);
            }
        }
    }

    #[test]
    fn scaled_stand_up_query_rejects_a_low_ceiling() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins, AssetPlugin::default(), PhysicsPlugins::new(FixedPostUpdate),
            TransformPlugin, TnuaAvian3dPlugin::new(FixedUpdate),
            TnuaControllerPlugin::<AlsControlScheme>::new(FixedUpdate),
            AlsStateTransitionPlugin, AlsCharacterPlugin, AlsMotionPlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .add_message::<AssetEvent<Mesh>>();
        app.finish();
        app.world_mut().spawn((
            RigidBody::Static, Collider::cuboid(4.0, 0.1, 4.0),
            Transform::from_xyz(0.0, 3.2, 0.0),
        ));
        app.update();
        app.world_mut().run_schedule(FixedPostUpdate);
        let params_value = AlsMovementParams::default();
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
            .add(params_value.clone());
        let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>()
            .add(initial_als_control_scheme_config(&params_value));
        let center = Vec3::Y * params_value.crouch.crouched_capsule_half_height * 2.0;
        let mut bundle = AlsCharacterBundle::new(params, config);
        bundle.transform = Transform::from_translation(center).with_scale(Vec3::splat(2.0));
        bundle.rigid_body = RigidBody::Kinematic;
        bundle.collider = als_character_capsule(
            params_value.crouch.crouched_capsule_half_height,
            params_value.crouch.standing_capsule_radius,
        );
        bundle.stance = Stance::Crouching;
        bundle.desired_stance = AlsDesiredStance(Stance::Standing);
        let character = app.world_mut().spawn((
            bundle, Position(center), LinearVelocity::ZERO, AlsCapsuleCrouched,
        )).id();
        app.update();
        assert_eq!(app.world().get::<Stance>(character), Some(&Stance::Crouching));
        assert!((app.world().get::<Position>(character).unwrap().y - center.y).abs() < 1e-6);
    }

    #[test]
    fn airborne_desired_crouch_keeps_actual_capsule_standing_until_landing() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            PhysicsPlugins::new(FixedPostUpdate),
            TransformPlugin,
            AlsStateTransitionPlugin,
            AlsCharacterPlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .add_message::<AssetEvent<Mesh>>()
        .add_systems(Update, capsule_reshape_system);
        app.finish();

        let params_value = AlsMovementParams::default();
        let params = app
            .world_mut()
            .resource_mut::<Assets<AlsMovementParams>>()
            .add(params_value.clone());
        let config = app
            .world_mut()
            .resource_mut::<Assets<AlsControlSchemeConfig>>()
            .add(initial_als_control_scheme_config(&params_value));
        let center = Vec3::new(0.0, params_value.crouch.standing_capsule_half_height, 0.0);
        let mut bundle = AlsCharacterBundle::new(params, config);
        bundle.rigid_body = RigidBody::Kinematic;
        bundle.transform = Transform::from_translation(center);
        bundle.locomotion_state = LocomotionState::Air;
        bundle.desired_stance = AlsDesiredStance(Stance::Crouching);
        let character = app
            .world_mut()
            .spawn((bundle, Position(center), LinearVelocity::ZERO))
            .id();

        app.update();

        let airborne = app.world().entity(character);
        let airborne_transaction = airborne.get::<AlsCrouchTransaction>().unwrap();
        assert_eq!(airborne.get::<Stance>(), Some(&Stance::Standing));
        assert_eq!(
            airborne.get::<AlsDesiredStance>(),
            Some(&AlsDesiredStance(Stance::Crouching))
        );
        assert!(airborne_transaction.forced_standing);
        assert!(!airborne_transaction.controller_crouch_active);
        assert!(!airborne.contains::<AlsCapsuleCrouched>());

        *app.world_mut()
            .get_mut::<LocomotionState>(character)
            .unwrap() = LocomotionState::Grounded;
        app.update();

        let landed = app.world().entity(character);
        let landed_transaction = landed.get::<AlsCrouchTransaction>().unwrap();
        assert_eq!(landed.get::<Stance>(), Some(&Stance::Crouching));
        assert!(landed.contains::<AlsCapsuleCrouched>());
        assert!(!landed_transaction.forced_standing);
        assert!(landed_transaction.controller_crouch_active);
        assert!(
            (landed_transaction.collider_half_height
                - params_value.crouch.crouched_capsule_half_height)
                .abs()
                < 1e-4
        );
    }

    #[test]
    fn capsule_transactions_repeat_with_two_physical_characters() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            PhysicsPlugins::new(FixedPostUpdate),
            TransformPlugin,
            TnuaAvian3dPlugin::new(FixedUpdate),
            TnuaControllerPlugin::<AlsControlScheme>::new(FixedUpdate),
            AlsStateTransitionPlugin,
            AlsCharacterPlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .add_message::<AssetEvent<Mesh>>()
        .insert_resource(Assets::<AlsMovementParams>::default())
        .insert_resource(Assets::<AlsControlSchemeConfig>::default())
        .add_systems(Update, capsule_reshape_system);
        app.finish();
        let params_value = AlsMovementParams::default();
        let params = app
            .world_mut()
            .resource_mut::<Assets<AlsMovementParams>>()
            .add(params_value.clone());
        let config = app
            .world_mut()
            .resource_mut::<Assets<AlsControlSchemeConfig>>()
            .add(initial_als_control_scheme_config(&params_value));
        let characters: Vec<_> = [Vec3::new(-4.0, 0.9, 0.0), Vec3::new(4.0, 0.9, 0.0)]
            .into_iter()
            .map(|position| {
                let mut bundle = AlsCharacterBundle::new(params.clone(), config.clone());
                bundle.transform = Transform::from_translation(position);
                bundle.rigid_body = RigidBody::Kinematic;
                bundle.collider = als_character_capsule(
                    params_value.crouch.standing_capsule_half_height,
                    params_value.crouch.standing_capsule_radius,
                );
                bundle.stance = Stance::Standing;
                bundle.desired_stance = AlsDesiredStance(Stance::Standing);
                app.world_mut()
                    .spawn((bundle, Position(position), GravityScale(0.0)))
                    .id()
            })
            .collect();
        for _ in 0..100 {
            for entity in &characters {
                app.world_mut()
                    .get_mut::<AlsDesiredStance>(*entity)
                    .unwrap()
                    .0 = Stance::Crouching;
            }
            app.update();
            for entity in &characters {
                let entity_ref = app.world().entity(*entity);
                let transaction = entity_ref.get::<AlsCrouchTransaction>().unwrap();
                assert_eq!(entity_ref.get::<Stance>(), Some(&Stance::Crouching));
                assert!(entity_ref.contains::<AlsCapsuleCrouched>());
                assert!(transaction.controller_crouch_active);
                assert!(
                    (transaction.collider_half_height
                        - params_value.crouch.crouched_capsule_half_height)
                        .abs()
                        < 1e-4
                );
            }
            for entity in &characters {
                app.world_mut()
                    .get_mut::<AlsDesiredStance>(*entity)
                    .unwrap()
                    .0 = Stance::Standing;
            }
            app.update();
            for entity in &characters {
                let entity_ref = app.world().entity(*entity);
                let transaction = entity_ref.get::<AlsCrouchTransaction>().unwrap();
                assert_eq!(entity_ref.get::<Stance>(), Some(&Stance::Standing));
                assert!(!entity_ref.contains::<AlsCapsuleCrouched>());
                assert!(!transaction.controller_crouch_active);
                assert!(
                    (transaction.collider_half_height
                        - params_value.crouch.standing_capsule_half_height)
                        .abs()
                        < 1e-4
                );
            }
        }
        let revisions: Vec<_> = characters
            .iter()
            .map(|entity| {
                app.world()
                    .get::<AlsCrouchTransaction>(*entity)
                    .unwrap()
                    .revision
            })
            .collect();
        println!(
            "[CROUCH-TRANSACTION][PHYSICS-CYCLES] pairs=100 characters={} revisions={revisions:?} -> PASS",
            characters.len(),
        );
        assert_eq!(revisions, vec![200, 200]);
    }

    #[test]
    fn jump_chain_keeps_the_source_priority_order() {
        assert_eq!(
            jump_chain(true, Stance::Crouching, true),
            JumpChain::ExitRagdoll
        );
        assert_eq!(
            jump_chain(false, Stance::Standing, true),
            JumpChain::Mantle
        );
        assert_eq!(
            jump_chain(false, Stance::Crouching, false),
            JumpChain::StandUp
        );
        assert_eq!(
            jump_chain(false, Stance::Standing, false),
            JumpChain::Jump
        );
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsViewYaw(pub f32);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsAimingYawConstraint {
    pub active_limit_degrees: f32,
    pub applied_last_frame: bool,
}

impl Default for AlsAimingYawConstraint {
    fn default() -> Self {
        Self {
            active_limit_degrees: 180.0,
            applied_last_frame: false,
        }
    }
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsRotationAnimationState {
    pub yaw_offset_degrees: f32,
    pub yaw_speed_degrees: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsRotationResponseState {
    pub initialized: bool,
    pub smooth_target_yaw: f32,
    pub previous_view_yaw: f32,
    pub input_yaw: Option<f32>,
    pub velocity_yaw: Option<f32>,
    pub target_yaw: f32,
    pub rotation_towards_last_input_direction_blocked: bool,
    pub previous_locomotion: LocomotionState,
    pub previous_rotation_mode: Option<RotationMode>,
}

impl Default for AlsRotationResponseState {
    fn default() -> Self {
        Self {
            initialized: false,
            smooth_target_yaw: 0.0,
            previous_view_yaw: 0.0,
            input_yaw: None,
            velocity_yaw: None,
            target_yaw: 0.0,
            rotation_towards_last_input_direction_blocked: true,
            previous_locomotion: LocomotionState::Grounded,
            previous_rotation_mode: None,
        }
    }
}

#[derive(Component, Debug, Default, Clone)]
pub struct AlsInputOverride(pub AlsInputCommand);

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsMovementInputBlocked;

pub fn movement_input_for_action(mut input: AlsInputCommand, blocked: bool) -> AlsInputCommand {
    if blocked {
        input.move_dir = Vec2::ZERO;
        input.roll_pressed = false;
    }
    input
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsCapsuleCrouched;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsCrouchTransactionPhase {
    Standing,
    EnteringCrouch,
    Crouching,
    TryingToStand,
    BlockedFromStanding,
    ExitingCrouch,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsCrouchTransaction {
    pub desired_stance: Stance,
    pub actual_stance: Stance,
    pub phase: AlsCrouchTransactionPhase,
    pub blocked_retry_count: u32,
    pub revision: u64,
    pub controller_crouch_active: bool,
    pub collider_half_height: f32,
    pub collider_radius: f32,
    pub collider_center: Vec3,
    pub headroom_blocked: bool,
    pub forced_standing: bool,
}

impl Default for AlsCrouchTransaction {
    fn default() -> Self {
        Self {
            desired_stance: Stance::Standing,
            actual_stance: Stance::Standing,
            phase: AlsCrouchTransactionPhase::Standing,
            blocked_retry_count: 0,
            revision: 0,
            controller_crouch_active: false,
            collider_half_height: 0.0,
            collider_radius: 0.0,
            collider_center: Vec3::ZERO,
            headroom_blocked: false,
            forced_standing: false,
        }
    }
}

impl AlsCrouchTransaction {
    fn synchronize(
        &mut self,
        desired: Stance,
        actual: Stance,
        half_height: f32,
        radius: f32,
        center: Vec3,
        forced_standing: bool,
    ) {
        if self.desired_stance != desired {
            self.revision = self.revision.wrapping_add(1);
            self.blocked_retry_count = 0;
        }
        self.desired_stance = desired;
        self.actual_stance = actual;
        self.collider_half_height = half_height;
        self.collider_radius = radius;
        self.collider_center = center;
        self.controller_crouch_active = actual == Stance::Crouching;
        self.forced_standing = forced_standing;
        if forced_standing {
            self.phase = if actual == Stance::Standing {
                AlsCrouchTransactionPhase::Standing
            } else {
                AlsCrouchTransactionPhase::ExitingCrouch
            };
            self.headroom_blocked = false;
        } else if desired == actual {
            self.phase = if actual == Stance::Crouching { AlsCrouchTransactionPhase::Crouching } else { AlsCrouchTransactionPhase::Standing };
            self.headroom_blocked = false;
        } else if desired == Stance::Crouching {
            self.phase = AlsCrouchTransactionPhase::EnteringCrouch;
            self.headroom_blocked = false;
        } else if self.phase != AlsCrouchTransactionPhase::BlockedFromStanding {
            self.phase = AlsCrouchTransactionPhase::TryingToStand;
        }
    }

    fn crouch_applied(&mut self, half_height: f32, radius: f32, center: Vec3) {
        self.actual_stance = Stance::Crouching;
        self.phase = AlsCrouchTransactionPhase::Crouching;
        self.controller_crouch_active = true;
        self.collider_half_height = half_height;
        self.collider_radius = radius;
        self.collider_center = center;
        self.headroom_blocked = false;
        self.forced_standing = false;
    }

    fn stand_blocked(&mut self, half_height: f32, radius: f32, center: Vec3) {
        self.actual_stance = Stance::Crouching;
        self.phase = AlsCrouchTransactionPhase::BlockedFromStanding;
        self.controller_crouch_active = true;
        self.collider_half_height = half_height;
        self.collider_radius = radius;
        self.collider_center = center;
        self.headroom_blocked = true;
        self.forced_standing = false;
        self.blocked_retry_count = self.blocked_retry_count.saturating_add(1);
    }

    fn standing_applied(&mut self, half_height: f32, radius: f32, center: Vec3) {
        self.actual_stance = Stance::Standing;
        self.phase = AlsCrouchTransactionPhase::ExitingCrouch;
        self.controller_crouch_active = false;
        self.collider_half_height = half_height;
        self.collider_radius = radius;
        self.collider_center = center;
        self.headroom_blocked = false;
        self.forced_standing = false;
        self.blocked_retry_count = 0;
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CharacterDebugFrame {
    pub actor_yaw_deg: f32,
    pub capsule_half_height: f32,
    pub capsule_radius: f32,
    pub max_speed: f32,
    pub smooth_target_yaw_deg: Option<f32>,
}

#[derive(Component, Debug, Default, Clone)]
pub struct AlsCharacterDebug(pub CharacterDebugFrameInner);

#[derive(Debug, Default, Clone, Copy)]
pub struct CharacterDebugFrameInner {
    pub frame: CharacterDebugFrame,
    pub mantle_probe: Option<crate::als::mantle::MantleProbeTrace>,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct CapsuleReshapeDeferred;

pub(crate) fn effective_input<'a>(
    cmd: &'a AlsInputCommand,
    ovr: Option<&'a AlsInputOverride>,
) -> &'a AlsInputCommand {
    ovr.map(|o| &o.0).unwrap_or(cmd)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpChain {
    ExitRagdoll,
    Mantle,
    StandUp,
    Jump,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsJumpPrev(pub bool);

#[derive(Component)]
pub(crate) struct AlsAutoMantlePending;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsRagdollAirExit;

pub fn jump_chain(ragdolling: bool, stance: Stance, mantle_available: bool) -> JumpChain {
    if ragdolling {
        JumpChain::ExitRagdoll
    } else if mantle_available {
        JumpChain::Mantle
    } else if stance == Stance::Crouching {
        JumpChain::StandUp
    } else {
        JumpChain::Jump
    }
}

pub struct AlsMotionPlugin;

#[derive(Message, Clone, Copy)]
pub(crate) struct SourceCharacterImpact {
    collider: Entity,
    point: Vec3,
    normal: Vec3,
    acceleration: Vec3,
    velocity: Vec3,
    max_speed: f32,
}

fn record_character_impact(
    context: &mut Option<(&mut Messages<SourceCharacterImpact>, Vec3, f32, f32, bool)>,
    hit: &ShapeHitData, normal: Vec3, velocity: Vec3, landed: bool,
) {
    if let Some((writer, acceleration, falling_speed, grounded_speed, landing_allowed)) = context.as_mut() {
        if landed && !*landing_allowed { return; }
        writer.write(SourceCharacterImpact { collider: hit.entity, point: hit.point1, normal,
            acceleration: *acceleration, velocity, max_speed: if landed { *grounded_speed } else { *falling_speed } });
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MovementBaseFollowPhase {
    BeforeSensors,
    AfterJump,
    AfterPhysics,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsActualVelocitySet;

#[derive(Component)]
pub(crate) struct AlsCustomMovementPhysicsFrame {
    pub(crate) rigid_body: avian3d::prelude::RigidBody,
    pub(crate) custom_position: bool,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsRotationResponseSet;

impl Plugin for AlsMotionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<StateTransitionEvent>()
            .add_message::<SourceCharacterImpact>()
            .add_plugins((
                crate::als::view::AlsViewInputPlugin,
                crate::als::gait::AlsGaitPlugin,
                crate::als::dynamics::AlsGroundedDynamicsPlugin,
            ))
            .add_systems(
                Update,
                (
                    initialize_spawn_movement_mode
                        .before(locomotion_drive_system)
                        .before(TnuaPipelineSystems::Sensors),
                    tnua_config_sync_system
                        .after(crate::als::transition::state_transition_system)
                        .after(capsule_reshape_system)
                        .before(TnuaPipelineSystems::Sensors),
                    locomotion_drive_system
                        .after(crate::als::view::AlsViewInputSet)
                        .before(crate::als::transition::state_transition_system),
                    crouch_controller_transaction_system
                        .after(locomotion_drive_system)
                        .after(capsule_reshape_system)
                        .before(TnuaPipelineSystems::Logic),
                    accepted_jump_state_system.after(TnuaPipelineSystems::Logic)
                        .before(crate::als::dynamics::grounded_dynamics_motor_system),
                    cmc_movement_transaction_system.after(crate::als::dynamics::grounded_dynamics_motor_system)
                        .before(TnuaPipelineSystems::Motors),
                    auto_mantle_after_movement.after(cmc_movement_transaction_system)
                        .after(TnuaPipelineSystems::Motors),
                    resolve_stationary_floor_state.after(cmc_movement_transaction_system)
                        .before(TnuaPipelineSystems::Motors),
                    apply_standing_downward_force.after(resolve_stationary_floor_state)
                        .before(TnuaPipelineSystems::Motors),
                    apply_character_impact_forces.after(cmc_movement_transaction_system)
                        .before(TnuaPipelineSystems::Motors),
                    (|| MovementBaseFollowPhase::BeforeSensors).pipe(follow_movement_base_system)
                        .after(capsule_reshape_system).before(TnuaPipelineSystems::Sensors),
                    (|| MovementBaseFollowPhase::AfterJump).pipe(follow_movement_base_system)
                        .after(accepted_jump_state_system).before(crate::als::dynamics::grounded_dynamics_motor_system),
                    refine_ground_sensor_system.after(TnuaPipelineSystems::Sensors)
                        .before(TnuaPipelineSystems::Logic),
                    capsule_reshape_system.after(crate::als::transition::state_transition_system),
                ),
            )
            .add_systems(
                Update,
                sync_effective_gravity_system
                    .after(TnuaPipelineSystems::Sensors)
                    .before(TnuaPipelineSystems::Logic),
            )
            .configure_sets(
                PostUpdate,
                (
                    AlsActualVelocitySet.after(PhysicsSystems::Last),
                    AlsRotationResponseSet.after(AlsActualVelocitySet),
                ),
            )
            .add_systems(
                PostUpdate,
                ((|| MovementBaseFollowPhase::AfterPhysics).pipe(follow_movement_base_system), locomotion_state_sync_system, snapshot_actual_velocity_system)
                    .chain().in_set(AlsActualVelocitySet),
            )
            .add_systems(PostUpdate, apply_character_overlap_impulses
                .after(locomotion_state_sync_system).before(snapshot_actual_velocity_system).in_set(AlsActualVelocitySet))
            .add_systems(PhysicsSchedule, apply_character_repulsion
                .after(PhysicsStepSystems::NarrowPhase)
                .before(avian3d::dynamics::integrator::IntegrationSystems::UpdateVelocityIncrements))
            .add_systems(PostUpdate, (
                prepare_subminimum_custom_physics
                    .after(crate::roll_drive::AlsRollDriveSet)
                    .after(crate::mantle_drive::AlsMantleDriveSet)
                    .before(crate::source_physics_timing::SourcePhysicsTimingSet::Clamp),
                release_custom_movement_physics
                    .after(PhysicsSystems::Last)
                    .before(AlsActualVelocitySet),
            ))
            .add_systems(
                PostUpdate,
                (
                    crate::als::gait::refresh_gait_system
                        .after(crate::actual_motion::AlsActualMotionSet)
                        .before(AlsRotationResponseSet)
                        .before(crate::gait_layer::AlsGaitLayerSet),
                    rotation_response_system.in_set(AlsRotationResponseSet),
                ),
            )
            .add_systems(avian3d::prelude::SubstepSchedule, custom_movement_velocity_system
                .after(avian3d::dynamics::integrator::IntegrationSystems::Velocity)
                .after(crate::als::dynamics::limit_falling_velocity)
                .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::WarmStart));
    }
}

fn prepare_subminimum_custom_physics(
    time: Res<Time<Virtual>>,
    mut commands: Commands,
    characters: Query<(Entity, &LocomotionState, Option<&AlsMantleState>, &avian3d::prelude::RigidBody,
        Has<avian3d::prelude::CustomPositionIntegration>, Option<&crate::als::roll::AlsRollState>,
        Option<&crate::als_camera::AlsCameraCustomTimeDilation>),
        (With<AlsCharacter>, Without<AlsCustomMovementPhysicsFrame>)>,
) {
    if time.delta_secs() <= 0.0 { return; }
    for (entity, mode, mantle, body, custom_position, roll, dilation) in &characters {
        if *mode == LocomotionState::Custom && time.delta_secs() < crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS
            && mantle.is_none_or(|state| !state.active && !state.root_motion_ready) {
            commands.entity(entity).insert((avian3d::prelude::RigidBody::Kinematic,
                avian3d::prelude::CustomPositionIntegration,
                AlsCustomMovementPhysicsFrame { rigid_body: *body, custom_position }));
        } else if *mode == LocomotionState::Grounded
            && time.delta_secs() * dilation.map_or(1.0, |value| value.0) < crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS
            && roll.is_some_and(|roll| roll.active && roll.previous_time < crate::als::roll::ROLL_DURATION_SECS - crate::als::roll::ROLL_WEIGHT_OUT_SECS) {
            commands.entity(entity).insert((avian3d::prelude::CustomPositionIntegration,
                AlsCustomMovementPhysicsFrame { rigid_body: *body, custom_position }));
        }
    }
}

fn release_custom_movement_physics(
    mut commands: Commands,
    characters: Query<(Entity, &AlsCustomMovementPhysicsFrame)>,
) {
    for (entity, frame) in &characters {
        let mut character = commands.entity(entity);
        character.insert(frame.rigid_body).remove::<AlsCustomMovementPhysicsFrame>();
        if !frame.custom_position { character.remove::<avian3d::prelude::CustomPositionIntegration>(); }
    }
}

fn custom_movement_velocity_system(
    time: Res<Time<avian3d::prelude::Physics>>,
    mut characters: Query<(&LocomotionState, Option<&AlsMantleState>,
        Has<AlsCustomMovementPhysicsFrame>,
        &mut avian3d::dynamics::solver::solver_body::SolverBody), With<AlsCharacter>>,
) {
    if time.delta_secs() < crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS { return; }
    for (mode, mantle, root_frame, mut body) in &mut characters {
        if *mode == LocomotionState::Custom && mantle.is_none_or(|state| !state.active && !state.root_motion_ready)
            && !root_frame {
            body.linear_velocity = Vec3::ZERO;
        }
    }
}

fn accepted_jump_state_system(
    mut transitions: MessageWriter<StateTransitionEvent>,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut characters: Query<(Entity, &TnuaController<AlsControlScheme>, &mut LocomotionState,
        &mut crate::als::dynamics::AlsMovementBase, &AlsMovementParamsHandle, &mut bevy_tnua::TnuaMotor),
        (With<AlsCharacter>, Without<crate::als::character::AlsBirthAnimationPending>)>,
) {
    for (entity, controller, mut mode, mut base, handle, mut motor) in &mut characters {
        if *mode != LocomotionState::Grounded || controller.action_flow_status().just_starting()
            != Some(AlsControlSchemeActionDiscriminant::Jump) { continue; }
        if let Some(params) = params_assets.get(handle.id())
            && let Ok(basis) = controller.basis_access() {
            use bevy_tnua::basis_capabilities::TnuaBasisWithFrameOfReferenceSurface;
            let relative = TnuaBuiltinWalk::effective_velocity(&basis).y
                - TnuaBuiltinWalk::vertical_velocity(&basis).max(0.0);
            motor.lin.boost.y = params.jump.jump_z_velocity.max(relative) - relative;
        }
        transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
            from: StateValue::LocomotionState(*mode), to: StateValue::LocomotionState(LocomotionState::Air) });
        *mode = LocomotionState::Air;
        base.entity = None;
    }
}

fn snapshot_actual_velocity_system(
    time: Res<Time<avian3d::prelude::Physics>>,
    mut characters: Query<(&mut LinearVelocity, &mut AlsActualVelocity, Option<&Position>, &mut AlsCmcAdapterRuntime,
        &LocomotionState, Option<&mut crate::als::dynamics::AlsMovementBaseDynamicsState>), With<AlsCharacter>>,
) {
    for (mut velocity, mut actual, position, mut runtime, locomotion, mut base) in &mut characters {
        if let Some(collision_velocity) = runtime.grounded_collision_velocity.take() {
            velocity.0 = collision_velocity;
        } else if let Some(start) = runtime.grounded_move_start
            && let Some(position) = position
            && runtime.simulation_delta_seconds > 0.0 {
            let support = base.as_deref().filter(|base| base.entity.is_some())
                .map_or(Vec3::ZERO, |base| base.support_velocity.with_y(0.0));
            let relative = (position.0 - start - support * time.delta_secs()) / runtime.simulation_delta_seconds;
            let planned = base.as_deref().filter(|base| base.support_velocity == Vec3::ZERO)
                .and_then(|base| base.pre_collision_velocity)
                .filter(|planned| *locomotion == LocomotionState::Grounded
                    && runtime.simulation_substeps == 1 && runtime.simulation_delta_seconds == time.delta_secs()
                    && runtime.grounded_movement_delta.is_some_and(|delta| {
                        let expected = start + delta;
                        delta.with_y(0.0) == (*planned * runtime.simulation_delta_seconds).with_y(0.0)
                            && position.0.x == expected.x && position.0.z == expected.z
                    }));
            if let Some(planned) = planned {
                velocity.x = planned.x;
                velocity.z = planned.z;
            } else {
                velocity.x = relative.x + support.x;
                velocity.z = relative.z + support.z;
            }
        }
        if *locomotion != LocomotionState::Grounded
            && let Some(base) = base.as_deref_mut() {
            *base = default();
        }
        *actual = AlsActualVelocity::from_physics_velocity(
            crate::als::dynamics::locomotion_velocity(velocity.0, *locomotion, base.as_deref()));
    }
}

pub fn movement_sweep_travel(distance: f32, hit_distance: f32) -> f32 {
    let pullback = (distance * PULLBACK_PROPORTION).clamp(PULLBACK_MIN_DISTANCE, PULLBACK_MAX_DISTANCE)
        + distance * PULLBACK_TIME_BIAS;
    (hit_distance - pullback).max(0.0)
}

fn box_impact_normal(normal: Vec3, trace: Vec3, rotation: Quat) -> Vec3 {
    let inverse = rotation.inverse();
    let local_normal = inverse * normal;
    let local_trace = inverse * trace;
    let mut best_normal = local_normal;
    let mut best_dot = f32::INFINITY;
    for axis in [0, 2, 1] {
        let sign = if local_normal[axis] > 1e-4 { 1.0 }
            else if local_normal[axis] < -1e-4 { -1.0 } else { continue };
        let dot = local_trace[axis] * sign;
        if dot < best_dot {
            best_dot = dot;
            best_normal = Vec3::ZERO;
            best_normal[axis] = sign;
        }
    }
    rotation * best_normal
}

fn shape_impact_normal(
    shape: &(dyn avian3d::parry::shape::Shape + 'static), pose: avian3d::parry::math::Pose3,
    point: Vec3, normal: Vec3, trace: Vec3, sweep_start: Vec3, sweep_rotation: Quat, capsule: &Collider,
) -> Vec3 {
    use avian3d::parry::{bounding_volume::Aabb, query::{DefaultQueryDispatcher, PointQuery, ShapeCastOptions},
        shape::{CompositeShapeRef, Triangle}};
    if let Some(compound) = shape.as_compound() {
        if let Ok((direction, distance)) = Dir3::new_and_length(trace) {
            let query_pose = avian3d::parry::math::Pose3::from_parts(sweep_start, sweep_rotation);
            let relative = pose.inv_mul(&query_pose);
            let options = ShapeCastOptions { max_time_of_impact: distance, target_distance: 0.0,
                stop_at_penetration: false, compute_impact_geometry_on_penetration: true };
            if let Some((index, _)) = CompositeShapeRef(compound).cast_shape(&DefaultQueryDispatcher, &relative,
                pose.inverse_transform_vector(*direction), capsule.shape_scaled().as_ref(), options) {
                let (child_pose, child_shape) = &compound.shapes()[index as usize];
                return shape_impact_normal(child_shape.as_ref(), pose * *child_pose, point, normal, trace,
                    sweep_start, sweep_rotation, capsule);
            }
        }
        return normal;
    }
    if shape.as_cuboid().is_some() { return box_impact_normal(normal, trace, pose.rotation); }
    let local_point = pose.inverse_transform_point(point);
    let local_trace = pose.inverse_transform_vector(trace);
    if let Some(convex) = shape.as_convex_polyhedron() {
        let search = 0.01_f32.max(shape.compute_local_aabb().half_extents().abs().max_element() * 2e-4);
        let mut selected = None;
        let mut best_dot = f32::INFINITY;
        for face in convex.faces() {
            let index = convex.vertices_adj_to_face()[face.first_vertex_or_edge as usize] as usize;
            let vertex = convex.points()[index];
            let dot = face.normal.dot(local_trace);
            if face.normal.dot(local_point - vertex).abs() < search && dot < best_dot {
                best_dot = dot;
                selected = Some(face.normal);
            }
        }
        return selected.map_or(normal, |face| pose.transform_vector(face));
    }
    if let Some(triangle) = shape.as_triangle() {
        return triangle.normal().map_or(normal, |face| pose.transform_vector(face));
    }
    let search = Vec3::splat(0.01);
    let bounds = Aabb::new(local_point - search, local_point + search);
    let mut selected = None;
    let mut best_dot = f32::INFINITY;
    let mut consider = |triangle: &Triangle| {
        let projection = triangle.project_local_point(local_point, false).point;
        if projection.distance_squared(local_point) < 0.0001 && let Some(face) = triangle.normal() {
            let dot = face.dot(local_trace);
            if dot < best_dot { best_dot = dot; selected = Some(face); }
        }
    };
    if let Some(mesh) = shape.as_trimesh() {
        for index in mesh.bvh().intersect_aabb(&bounds) { consider(&mesh.triangle(index)); }
    } else if let Some(heightfield) = shape.as_heightfield() {
        heightfield.map_elements_in_local_aabb(&bounds, &mut |_, triangle| consider(triangle));
    }
    selected.map_or(normal, |face| pose.transform_vector(face))
}

pub(crate) fn source_sweep_impact_normal(
    spatial: &SpatialQuery, hit: &ShapeHitData, trace: Vec3,
    sweep_start: Vec3, sweep_rotation: Quat, capsule: &Collider,
) -> Vec3 {
    if hit.distance <= 0.0 { return hit.normal1; }
    let Some((position, rotation, shape)) = spatial.collider_geometry(hit.entity) else { return hit.normal1; };
    shape_impact_normal(shape.shape_scaled().as_ref(), avian3d::parry::math::Pose3::from_parts(position.0, rotation.0),
        hit.point1, hit.normal1, trace, sweep_start, sweep_rotation, capsule)
}

fn landing_hit_qualification(
    normal: Vec3, impact_normal: Vec3, point: Vec3, hit_location: Vec3, position: Vec3,
    half: f32, radius: f32, penetrating: bool,
) -> (bool, bool) {
    let within_edge = |location: Vec3| {
        let offset = point - location;
        let reduced = (radius - EDGE_REJECT_DISTANCE).max(SWEEP_EDGE_MIN_RADIUS);
        Vec2::new(offset.x, offset.z).length_squared() < reduced * reduced
    };
    let direct = if penetrating { normal.y >= 1e-4 } else {
        impact_normal.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos()
            && point.y < hit_location.y - half + radius && within_edge(hit_location)
    };
    let edge = !penetrating && normal.y > 1e-4
        && (normal - impact_normal).abs().max_element() > 1e-4 && within_edge(position);
    (direct, edge)
}

fn edge_floor_landing(
    spatial: &SpatialQuery, position: Vec3, half: f32, radius: f32, cmc: &crate::als::params::CmcAdapterParams,
    entity: Entity, candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
) -> bool {
    let filter = SpatialQueryFilter::from_excluded_entities([entity]);
    let Some(floor) = find_ground_floor_with_penetration(spatial, position, half, radius, cmc,
        LocomotionState::Air, &filter, entity, candidates, None, None) else { return false };
    if floor.origin != GroundFloorOrigin::Sweep { return false; }
    let trace = floor.trace;
    let hit_location = trace.origin - Vec3::Y * floor.hit.distance;
    let impact_normal = floor.impact_normal;
    let penetrating = floor.hit.distance == 0.0 && (floor.hit.point1 - floor.hit.point2).dot(floor.hit.normal1) > 0.0;
    if !landing_hit_qualification(floor.hit.normal1, impact_normal, floor.hit.point1, hit_location, position,
        half, radius, penetrating).0 { return false; }
    let supplied = ShapeHitData { normal1: impact_normal, ..floor.hit };
    find_ground_floor_with_penetration(spatial, position, half, radius, cmc, LocomotionState::Air,
        &filter, entity, candidates, Some((supplied, trace)), None).is_some()
}

fn limit_falling_air_control(acceleration: Vec3, normal: Vec3, penetrating: bool) -> Vec3 {
    if penetrating {
        if acceleration.dot(normal) > 0.0 { acceleration } else { Vec3::ZERO }
    } else if normal.y > 0.001 && acceleration.dot(normal) < 0.0 {
        let horizontal_normal = normal.with_y(0.0).normalize_or_zero();
        acceleration - horizontal_normal * acceleration.dot(horizontal_normal)
    } else { acceleration }
}

fn falling_ditch(first_impact: Vec3, second_impact: Vec3, delta: Vec3) -> bool {
    first_impact.y > 0.0 && second_impact.y > 0.0 && delta.y.abs() <= 1e-6
        && first_impact.dot(second_impact) < 0.0
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct FallingImpactBodies<'w, 's> {
    bodies: Query<'w, 's, (&'static RigidBody, &'static Position, &'static LinearVelocity,
        Option<&'static AngularVelocity>, Has<avian3d::prelude::RigidBodyDisabled>)>,
    owners: Query<'w, 's, &'static ColliderOf>,
    pawns: Query<'w, 's, (), With<AlsCharacter>>,
}

impl FallingImpactBodies<'_, '_> {
    fn contact_velocity(&self, collider: Entity, point: Vec3) -> Option<Vec3> {
        let entity = self.owners.get(collider).map_or(collider, |owner| owner.body);
        let (body, position, linear, angular, disabled) = self.bodies.get(entity).ok()?;
        if *body != RigidBody::Dynamic || disabled { return None; }
        Some(crate::als::dynamics::support_point_velocity(position.0, linear.0,
            angular.map_or(Vec3::ZERO, |velocity| velocity.0), point))
    }

    fn is_pawn(&self, collider: Entity) -> bool {
        let entity = self.owners.get(collider).map_or(collider, |owner| owner.body);
        self.pawns.contains(entity)
    }
}

fn falling_penetration_vector(hit: &ShapeHitData, impact_bodies: &FallingImpactBodies) -> Option<Vec3> {
    let depth = (hit.point1 - hit.point2).dot(hit.normal1);
    if hit.distance != 0.0 || depth <= 0.0 { return None; }
    let maximum = if impact_bodies.is_pawn(hit.entity) { MAX_DEPENETRATION_WITH_PAWN } else { MAX_DEPENETRATION_WITH_GEOMETRY };
    Some((hit.normal1 * (depth + PENETRATION_PULLBACK_DISTANCE)).clamp_length_max(maximum))
}

fn falling_penetration_adjustment(spatial: &SpatialQuery, capsule: &Collider, origin: Vec3, rotation: Quat,
    hit: &ShapeHitData, filter: &SpatialQueryFilter, accepts: &dyn Fn(Entity) -> bool,
    impact_bodies: &FallingImpactBodies) -> Option<Vec3> {
    let adjustment = falling_penetration_vector(hit, impact_bodies)?;
    let extent = capsule.shape_scaled().compute_local_aabb().half_extents();
    let inflated = als_character_capsule(extent.y + PENETRATION_OVERLAP_INFLATION, extent.x + PENETRATION_OVERLAP_INFLATION);
    (!spatial.shape_intersections(&inflated, origin + adjustment, rotation, filter).into_iter().any(accepts)).then_some(adjustment)
}

fn falling_resolve_penetration(spatial: &SpatialQuery, capsule: &Collider, origin: Vec3, rotation: Quat,
    hit: &ShapeHitData, movement: Vec3, filter: &SpatialQueryFilter, accepts: &dyn Fn(Entity) -> bool,
    impact_bodies: &FallingImpactBodies) -> Option<Vec3> {
    if let Some(adjustment) = falling_penetration_adjustment(spatial, capsule, origin, rotation,
        hit, filter, accepts, impact_bodies) { return Some(adjustment); }
    let adjustment = falling_penetration_vector(hit, impact_bodies)?;
    let attempt = |delta: Vec3| {
        let Ok((direction, distance)) = Dir3::new_and_length(delta) else { return (Vec3::ZERO, None) };
        let config = ShapeCastConfig { ignore_origin_penetration: true, ..ShapeCastConfig::from_max_distance(distance) };
        let hit = spatial.cast_shape_predicate(capsule, origin, rotation, direction, &config, filter, accepts);
        let travel = hit.map_or(distance, |hit| movement_sweep_travel(distance, hit.distance));
        (*direction * travel, hit)
    };
    let (moved, second) = attempt(adjustment);
    if moved != Vec3::ZERO { return Some(moved); }
    if let Some(second) = second && let Some(second_mtd) = falling_penetration_vector(&second, impact_bodies)
        && second_mtd != adjustment {
        let (moved, _) = attempt(adjustment + second_mtd);
        if moved != Vec3::ZERO { return Some(moved); }
    }
    if movement != Vec3::ZERO {
        let (moved, _) = attempt(adjustment + movement);
        if moved != Vec3::ZERO { return Some(moved); }
        if movement.dot(adjustment) > 0.0 {
            let (moved, _) = attempt(movement);
            if moved != Vec3::ZERO { return Some(moved); }
        }
    }
    None
}

fn falling_safe_sweep(spatial: &SpatialQuery, capsule: &Collider, origin: Vec3, rotation: Quat,
    direction: Dir3, distance: f32, filter: &SpatialQueryFilter, accepts: &dyn Fn(Entity) -> bool,
    impact_bodies: &FallingImpactBodies) -> (Vec3, Option<ShapeHitData>) {
    let config = ShapeCastConfig::from_max_distance(distance);
    let hit = spatial.cast_shape_predicate(capsule, origin, rotation, direction, &config, filter, accepts);
    if let Some(initial) = hit.as_ref() && let Some(adjustment) = falling_resolve_penetration(spatial, capsule, origin,
        rotation, initial, *direction * distance, filter, accepts, impact_bodies) {
        let retry = ShapeCastConfig { ignore_origin_penetration: true, ..config };
        return (adjustment, spatial.cast_shape_predicate(capsule, origin + adjustment, rotation, direction, &retry, filter, accepts));
    }
    (Vec3::ZERO, hit)
}

pub(crate) fn falling_movement_sweep(
    spatial: &SpatialQuery, capsule: &Collider, start: Vec3, rotation: Quat,
    movement: Vec3, mut velocity: Vec3, step: f32, air_control: Option<(Vec3, Vec3)>,
    mut landing: Option<(&crate::als::params::CmcAdapterParams, f32, f32, &mut u32)>, entity: Entity,
    candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    shapes: &Query<(&Collider, &Position, &Rotation)>,
    impact_bodies: &FallingImpactBodies,
    mut impact_context: Option<(&mut Messages<SourceCharacterImpact>, Vec3, f32, f32, bool)>,
) -> (Vec3, Vec3, bool, Option<(f32, bool)>, Option<LocomotionState>) {
    let filter = SpatialQueryFilter::from_excluded_entities([entity]);
    let accepts = |candidate| character_blocks_collider(entity, candidate, candidates);
    let mut remaining = movement;
    let mut remaining_time = step;
    let mut previous_normal = None;
    let mut first_impact_normal: Option<Vec3> = None;
    let mut first_geometry_normal: Option<Vec3> = None;
    let mut ditch = false;
    let mut applied = Vec3::ZERO;
    let mut contact = false;
    let mut collision_mode = None;
    let mut teleported = false;
    for sweep in 0..3 {
        let third_delta = remaining;
        let mut motion = Dir3::new_and_length(remaining).ok();
        let mut hit = motion.as_ref().and_then(|(direction, distance)| {
            let (adjustment, hit) = falling_safe_sweep(spatial, capsule, start + applied, rotation,
                *direction, *distance, &filter, &accepts, impact_bodies);
            applied += adjustment;
            teleported |= adjustment != Vec3::ZERO;
            hit
        });
        let mut sidestepped = false;
        if sweep == 2 && let Some(first) = first_impact_normal
            && let Some(third) = hit.as_ref() && let Some((_, distance)) = motion.as_ref()
            && movement_sweep_travel(*distance, third.distance) == 0.0 {
            let side = (first + third.normal1).with_y(0.0);
            let side = if side.length_squared() > 1e-8 { side.normalize() }
                else { Vec3::new(first.z, 0.0, -first.x) };
            remaining = side * 0.01;
            motion = Dir3::new_and_length(remaining).ok();
            hit = motion.as_ref().and_then(|(direction, distance)| {
                let (adjustment, hit) = falling_safe_sweep(spatial, capsule, start + applied, rotation,
                    *direction, *distance, &filter, &accepts, impact_bodies);
                applied += adjustment;
                hit
            });
            sidestepped = true;
        }
        let Some(hit) = hit else {
            applied += remaining;
            if ditch && landing.is_some() {
                return (applied, velocity, contact, Some((0.0, true)), Some(LocomotionState::Grounded));
            }
            if sidestepped { collision_mode = Some(LocomotionState::Air); }
            if sweep == 2
                && let Some((cmc, jump_z, max_speed, seed)) = landing.as_mut()
                && cmc.perch_radius_threshold > 0.0
                && first_impact_normal.is_some_and(|normal| normal.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos())
                && !first_impact_normal.zip(previous_normal).is_some_and(|(first, second)| {
                    first.y > 0.0 && second.y > 0.0 && third_delta.y.abs() <= 1e-6 && first.dot(second) < 0.0
                })
                && applied.y.abs() <= 0.002 * step
                && Vec2::new(applied.x, applied.z).length() <= 0.04 * step
            {
                let next_random = |seed: &mut u32| {
                    *seed = seed.wrapping_mul(196314165).wrapping_add(907633515);
                    f32::from_bits(0x3f800000 | (*seed >> 9)) - 1.0
                };
                velocity.x += 0.25 * *max_speed * (next_random(seed) - 0.5);
                velocity.z += 0.25 * *max_speed * (next_random(seed) - 0.5);
                velocity.y = (*jump_z * 0.25).max(0.01);
                let kick = velocity * step;
                if let Ok((direction, distance)) = Dir3::new_and_length(kick) {
                    let (adjustment, hit) = falling_safe_sweep(spatial, capsule, start + applied, rotation,
                        direction, distance, &filter, &accepts, impact_bodies);
                    applied += adjustment;
                    applied += *direction * hit.map_or(distance, |hit| movement_sweep_travel(distance, hit.distance));
                }
            }
            break;
        };
        let (direction, distance) = motion.unwrap();
        let travel = movement_sweep_travel(distance, hit.distance);
        applied += *direction * travel;
        if sidestepped && travel == 0.0 {
            return (applied, velocity, contact, Some((0.0, true)), Some(LocomotionState::Grounded));
        }
        if sweep == 2 && ditch && landing.is_some() {
            return (applied, velocity, contact, Some((0.0, true)), Some(LocomotionState::Grounded));
        }
        let Some(normal) = hit.normal1.try_normalize() else { break };
        if sweep == 0 { first_impact_normal = Some(normal); }
        let extent = capsule.shape_scaled().compute_local_aabb().half_extents();
        let position = start + applied;
        let sweep_start = position - *direction * travel;
        let geometry = shapes.get(hit.entity).ok().map(|(shape, shape_position, shape_rotation)| {
            let pose = avian3d::parry::math::Pose3::from_parts(shape_position.0, shape_rotation.0);
            let impact = shape_impact_normal(shape.shape_scaled().as_ref(), pose, hit.point1, normal,
                remaining, sweep_start, rotation, capsule);
            let hit_location = sweep_start + *direction * hit.distance;
            let penetrating = hit.distance == 0.0 && (hit.point1 - hit.point2).dot(normal) > 0.0;
            (landing_hit_qualification(normal, impact, hit.point1, hit_location, position,
                extent.y, extent.x.min(extent.z), penetrating), impact)
        });
        let impact_normal = if hit.distance <= 0.0 { normal } else { geometry.map_or(normal, |(_, impact)| impact) };
        if sweep == 0 { first_geometry_normal = Some(impact_normal); }
        let qualification = geometry.map(|(qualification, _)| qualification);
        let direct_landing = qualification.map_or(normal.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos(), |(direct, _)| direct);
        let candidate_contact = qualification.map_or(normal.y > 0.0, |(direct, edge)| direct || edge);
        contact |= candidate_contact && remaining.dot(normal) < 0.0;
        let previous_time = remaining_time;
        if !sidestepped { remaining_time *= 1.0 - travel / distance; }
        if direct_landing
            && remaining.dot(normal) < 0.0
            && let Some((cmc, ..)) = landing.as_ref() {
            if find_ground_floor(spatial, position, extent.y, extent.x.min(extent.z), cmc,
                LocomotionState::Air, &filter, entity, candidates).is_some() {
                record_character_impact(&mut impact_context, &hit, impact_normal, velocity, true);
                return (applied, velocity, contact, Some((remaining_time, sweep == 2)), None);
            }
        }
        if sweep == 0 && qualification.is_some_and(|(_, edge)| edge)
            && remaining.dot(normal) < 0.0 && let Some((cmc, ..)) = landing.as_ref() {
            if edge_floor_landing(spatial, position, extent.y, extent.x.min(extent.z), cmc, entity, candidates) {
                record_character_impact(&mut impact_context, &hit, impact_normal, velocity, true);
                return (applied, velocity, contact, Some((remaining_time, false)), None);
            }
            contact = false;
            collision_mode = Some(LocomotionState::Air);
        }
        if sweep == 0 && direct_landing && remaining.dot(normal) < 0.0 && landing.is_some() {
            contact = false;
            collision_mode = Some(LocomotionState::Air);
        }
        if sweep == 2 {
            if sidestepped { collision_mode = Some(LocomotionState::Air); }
            break;
        }
        record_character_impact(&mut impact_context, &hit, impact_normal, velocity, false);
        let budget = velocity * remaining_time;
        let penetrating = hit.distance == 0.0 && (hit.point1 - hit.point2).dot(normal) > 0.0;
        let input = if sweep == 0 && let Some((without_air, acceleration)) = air_control {
            (without_air + limit_falling_air_control(acceleration, normal, penetrating) * previous_time) * remaining_time
        } else if sweep == 1 && normal.y > 0.001
            && let Some((without_air, _)) = air_control
            && previous_time > 0.0 && let Some(first) = previous_normal {
            crate::extensions::motion::falling_slide(without_air * previous_time, first, None)
                * (remaining_time / previous_time)
        } else { budget };
        let mut slide = crate::extensions::motion::falling_slide(input, normal, previous_normal);
        if sweep == 1 && let Some((_, acceleration)) = air_control && let Some(first) = previous_normal {
            let limited = limit_falling_air_control(acceleration, normal, penetrating);
            let delta_velocity = limited * remaining_time;
            if delta_velocity.dot(first) > 0.0 { slide += delta_velocity * remaining_time; }
        }
        if sweep == 1 && let Some(first) = first_geometry_normal {
            ditch = falling_ditch(first, impact_normal, slide);
        }
        previous_normal = Some(normal);
        if sweep == 0 && velocity.abs().max_element() > 1e-6
            && let Some(contact_velocity) = impact_bodies.contact_velocity(hit.entity, hit.point1) {
            velocity -= impact_normal * (velocity - contact_velocity).dot(impact_normal);
        } else if remaining_time > f32::EPSILON && !teleported {
            velocity = slide / remaining_time;
        }
        if sweep == 0 && slide.dot(budget) <= 0.0 { break; }
        remaining = slide;
    }
    (applied, velocity, contact, None, collision_mode)
}

fn falling_apex_step(old_velocity: Vec3, new_velocity: Vec3, step: f32, attempts: u32) -> Option<(f32, Vec3)> {
    if old_velocity.y <= 0.0 || new_velocity.y > 0.0 || attempts >= 2 { return None; }
    let acceleration = (new_velocity - old_velocity) / step;
    if acceleration.y.abs() <= 1e-10 { return None; }
    let apex_time = -old_velocity.y / acceleration.y;
    (apex_time >= 0.0001 && apex_time < step)
        .then(|| (apex_time, (old_velocity + acceleration * apex_time).with_y(0.0)))
}

fn pawn_rejects_character_base(
    floor: Entity, pawns: &Query<(), With<AlsCharacter>>, permissions: &Query<&AlsMantleTargetPermission>,
    owners: &Query<&ColliderOf>,
) -> bool {
    let owner = owners.get(floor).map_or(floor, |collider| collider.body);
    pawns.contains(owner) && permissions.get(owner).is_ok_and(|permission| !permission.allows_step_up)
}

fn movement_base_changed(previous: Option<Entity>, floor: Entity, owners: &Query<&ColliderOf>) -> bool {
    let owner = owners.get(floor).map_or(floor, |collider| collider.body);
    previous.is_none_or(|previous| owners.get(previous).map_or(previous, |collider| collider.body) != owner)
}

fn jump_off_velocity(velocity: Vec3, max_speed: f32, jump_z: f32, factor: f32, seed: &mut u32) -> Vec3 {
    *seed = seed.wrapping_mul(196314165).wrapping_add(907633515);
    let angle = (f32::from_bits(0x3f800000 | (*seed >> 9)) - 1.0) * std::f32::consts::TAU;
    let direction = Vec3::new(angle.cos(), 0.5, angle.sin()).normalize();
    let limit = max_speed * 0.85;
    let mut velocity = velocity + direction * limit;
    if velocity.with_y(0.0).length() > limit { velocity = velocity.normalize_or_zero() * limit; }
    velocity.y = factor * jump_z;
    velocity
}

fn falling_movement_transaction(
    spatial: &SpatialQuery, capsule: &Collider, mut transform: Transform, mut velocity: Vec3,
    desired_motion: Vec3, mut max_speed: f32, params: &AlsMovementParams, gravity: Vec3,
    grounded: (Stance, RotationMode, Gait, f32), landing: (f32, bool, bool),
    terminal: f32, mut remaining: f32, mut iterations: u32, entity: Entity, random_seed: &mut u32,
    candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    step_permissions: &Query<&AlsMantleTargetPermission>,
    bases: &Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
    owners: &Query<&ColliderOf>,
    shapes: &Query<(&Collider, &Position, &Rotation)>,
    impact_bodies: &FallingImpactBodies,
    transitions: &mut MessageWriter<StateTransitionEvent>, cached_input_max: Option<f32>,
    mut impacts: Option<&mut Messages<SourceCharacterImpact>>,
) -> (Transform, Vec3, u32, bool, LocomotionState, Option<Entity>, f32, Option<LocomotionState>, Option<crate::als::params::GroundedDynamics>) {
    let mut acceleration = falling_input_acceleration(params, desired_motion, velocity.with_y(0.0).length(), cached_input_max);
    let mut contact = false;
    let mut mode = LocomotionState::Air;
    let mut current_base = None;
    let mut collision_mode = None;
    let mut refreshed_dynamics = None;
    let mut apex_attempts = 0;
    let mut jumped_from_pawn = false;
    while remaining >= crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS
        && iterations < params.cmc.max_simulation_iterations {
        iterations += 1;
        let full_step = crate::als::dynamics::source_movement_time_step(remaining, &params.cmc, iterations);
        let mut step = full_step;
        let old_velocity = velocity;
        let horizontal = crate::als::dynamics::grounded_velocity_step_with_braking_friction(
            velocity.with_y(0.0), desired_motion, max_speed,
            crate::als::params::GroundedDynamics { acceleration, braking_deceleration: 0.0, ground_friction: 0.0 },
            params.cmc.min_analog_walk_speed, 0.0, params.cmc.braking_substep_time, step);
        velocity = crate::als::dynamics::clamp_falling_velocity(horizontal + Vec3::Y * velocity.y + gravity * step,
            gravity, terminal);
        if let Some((apex_time, apex_velocity)) = falling_apex_step(old_velocity, velocity, step, apex_attempts) {
            step = apex_time;
            velocity = apex_velocity;
            iterations -= 1;
            apex_attempts += 1;
        }
        let air_control = if desired_motion.with_y(0.0) != Vec3::ZERO && acceleration > 0.0 {
            let without_air = crate::als::dynamics::clamp_falling_velocity(old_velocity + gravity * full_step, gravity, terminal);
            Some((without_air, (velocity - without_air) / step))
        } else { None };
        let requested = (old_velocity + velocity) * (0.5 * step);
        let impact_acceleration = desired_motion.clamp_length_max(1.0)
            * cached_input_max.unwrap_or(params.acceleration.max_acceleration) + gravity;
        let (stance, rotation, gait, yaw) = grounded;
        let grounded_speed = crate::als::dynamics::max_allowed_speed(params, stance, rotation, gait, velocity.with_y(0.0), yaw);
        let (applied, deflected, hit, landing_remaining, step_collision_mode) = falling_movement_sweep(spatial, capsule, transform.translation,
            transform.rotation, requested, velocity, step, air_control,
            Some((&params.cmc, params.jump.jump_z_velocity, max_speed, &mut *random_seed)), entity, candidates, shapes, impact_bodies,
            impacts.as_deref_mut().map(|writer| (writer, impact_acceleration, max_speed, grounded_speed, !landing.1)));
        collision_mode = step_collision_mode;
        transform.translation += applied;
        contact |= hit;
        let extent = capsule.shape_scaled().compute_local_aabb().half_extents();
        if let Some((landing_remaining, last_impact)) = landing_remaining {
            remaining = if last_impact { 0.0 } else { remaining - step + landing_remaining };
            transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                from: StateValue::LocomotionState(LocomotionState::Air), to: StateValue::LocomotionState(LocomotionState::Grounded) });
            mode = LocomotionState::Grounded;
            current_base = adjust_walking_floor_height(spatial, capsule, &mut transform, extent.y, extent.x.min(extent.z),
                &params.cmc, entity, candidates);
            velocity = deflected.with_y(0.0);
            if landing.1 {
                mode = LocomotionState::None;
                transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                    from: StateValue::LocomotionState(LocomotionState::Grounded), to: StateValue::LocomotionState(mode) });
                break;
            }
            if current_base.is_some_and(|floor| pawn_rejects_character_base(
                floor, &impact_bodies.pawns, step_permissions, owners)) {
                velocity = jump_off_velocity(velocity, max_speed, params.jump.jump_z_velocity,
                    params.cmc.jump_off_jump_z_factor, random_seed);
                mode = LocomotionState::Air;
                jumped_from_pawn = true;
                current_base = None;
                collision_mode = Some(LocomotionState::Air);
                transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                    from: StateValue::LocomotionState(LocomotionState::Grounded), to: StateValue::LocomotionState(mode) });
                acceleration = falling_input_acceleration(params, desired_motion, velocity.with_y(0.0).length(), cached_input_max);
                continue;
            }
            let (stance, rotation, gait, yaw) = grounded;
            let thresholds = params.gait_speed_thresholds(stance, rotation, velocity, yaw);
            let gait_amount = crate::als::gait::calculate_gait_amount_for_velocity(velocity, thresholds);
            let dynamics = params.grounded_dynamics(stance, rotation, gait_amount);
            let speed = crate::als::dynamics::max_allowed_speed(params, stance, rotation, gait, velocity, yaw);
            if remaining >= crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS
                && iterations < params.cmc.max_simulation_iterations {
                max_speed = speed;
                if AlsMovementParams::has_gait_settings(stance, rotation) { refreshed_dynamics = Some(dynamics); }
            }
            let input_dynamics = crate::als::dynamics::with_controlled_input_acceleration(dynamics, cached_input_max);
            let mut cmc = params.cmc.clone();
            cmc.max_simulation_iterations = cmc.max_simulation_iterations.saturating_sub(iterations);
            let (moved, next_velocity, steps, next_remaining, lost_floor, _, last_base, rejected_base) = walking_movement_transaction(
                spatial, capsule, transform, extent.y, extent.x.min(extent.z), velocity, desired_motion,
                speed, input_dynamics, &cmc, landing.0, remaining, entity, candidates, step_permissions, bases, owners,
                &impact_bodies.pawns, current_base,
                impacts.as_deref_mut().map(|writer| (writer, impact_acceleration - gravity, speed, impact_bodies)));
            transform = moved;
            velocity = next_velocity;
            current_base = last_base;
            iterations += steps;
            remaining = next_remaining;
            if !lost_floor && !rejected_base { break; }
            if rejected_base {
                velocity = jump_off_velocity(velocity, speed, params.jump.jump_z_velocity,
                    params.cmc.jump_off_jump_z_factor, random_seed);
                jumped_from_pawn = true;
            }
            velocity += crate::als::dynamics::support_velocity_from_base(last_base,
                transform.translation - Vec3::Y * extent.y, bases, owners).1;
            current_base = None;
            mode = LocomotionState::Air;
            transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                from: StateValue::LocomotionState(LocomotionState::Grounded), to: StateValue::LocomotionState(mode) });
            if landing.2 && params.rolling.interrupt_when_in_air {
                mode = LocomotionState::None;
                transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                    from: StateValue::LocomotionState(LocomotionState::Air), to: StateValue::LocomotionState(mode) });
                break;
            }
            acceleration = falling_input_acceleration(params, desired_motion, velocity.with_y(0.0).length(), cached_input_max);
            continue;
        }
        velocity = deflected;
        if velocity.with_y(0.0).length_squared() <= 1e-7 { velocity = Vec3::new(0.0, velocity.y, 0.0); }
        remaining -= step;
    }
    if jumped_from_pawn && mode == LocomotionState::Air { collision_mode = Some(LocomotionState::Air); }
    (transform, velocity, iterations, contact, mode, current_base, max_speed, collision_mode, refreshed_dynamics)
}

fn follow_movement_base_system(
    In(phase): In<MovementBaseFollowPhase>,
    time: Res<Time<avian3d::prelude::Physics>>,
    virtual_time: Res<Time<Virtual>>,
    mut characters: Query<(Entity, &LocomotionState, &mut crate::als::dynamics::AlsMovementBase,
        Option<&AlsMantleState>, Option<&crate::als::dynamics::AlsMovementBaseDynamicsState>,
        &mut AlsCmcAdapterRuntime, &Collider, Has<avian3d::prelude::CustomPositionIntegration>),
        (With<AlsCharacter>, Without<crate::als::character::AlsBirthAnimationPending>)>,
    mut space: ParamSet<(bevy::transform::helper::TransformHelper, SpatialQuery,
        Query<(&mut Position, &mut Transform), With<AlsCharacter>>)>,
    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    owners: Query<&avian3d::prelude::ColliderOf>,
    bodies: Query<&RigidBody>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
) {
    if virtual_time.is_paused() { return; }
    for (entity, mode, mut base, mantle, dynamics, mut runtime, capsule, custom_position) in &mut characters {
        if phase == MovementBaseFollowPhase::BeforeSensors && runtime.deferred_base_follow { continue; }
        if phase == MovementBaseFollowPhase::AfterJump {
            if !runtime.deferred_base_follow { continue; }
            runtime.deferred_base_follow = false;
        }
        let target = if *mode == LocomotionState::Custom {
            mantle.and_then(|mantle| mantle.target_base.map(|target| target.entity))
        } else if *mode == LocomotionState::Grounded {
            base.entity
        } else {
            None
        };
        let Some(target) = target else { base.entity = None; continue; };
        let Ok(current) = space.p0().compute_global_transform(target) else { base.entity = None; continue; };
        let owner = owners.get(target).map_or(target, |owner| owner.body);
        if *mode == LocomotionState::Custom && phase == MovementBaseFollowPhase::AfterPhysics
            && !bodies.get(owner).is_ok_and(|body| *body == RigidBody::Dynamic) {
            continue;
        }
        let mantle_base = mantle.filter(|state| *mode == LocomotionState::Custom
            && (state.active || state.root_motion_ready))
            .and_then(|state| state.target_base).filter(|target| target.relative_location);
        if let Some(initial) = mantle_base && base.entity != Some(target) {
            base.entity = Some(target);
            base.location = initial.translation;
            base.rotation = initial.rotation;
        }
        if (*mode == LocomotionState::Grounded || mantle_base.is_some())
            && bodies.get(owner).is_ok_and(|body| *body != RigidBody::Static) {
            let carried = if phase != MovementBaseFollowPhase::AfterPhysics || custom_position { Vec3::ZERO } else {
                dynamics.filter(|dynamics| dynamics.entity == Some(target))
                    .map_or(Vec3::ZERO, |dynamics| dynamics.support_velocity) * time.delta_secs()
            };
            let origin = space.p2().get(entity).unwrap().0.0;
            let offset = Vec3::Y * capsule.shape_scaled().compute_local_aabb().half_extents().y;
            let delta = current.rotation() * base.rotation.conjugate()
                * (origin - carried - offset - base.location) + current.translation() + offset - origin;
            if let Ok((direction, distance)) = Dir3::new_and_length(delta) {
                let filter = SpatialQueryFilter::from_excluded_entities(
                    [entity, owner, target].into_iter().chain(children.iter_descendants(owner)));
                let config = ShapeCastConfig { ignore_origin_penetration: true,
                    ..ShapeCastConfig::from_max_distance(distance) };
                let Ok(rotation) = space.p0().compute_global_transform(entity).map(|world| world.rotation()) else { continue; };
                let hit = space.p1().cast_shape_predicate(capsule, origin, rotation, direction, &config, &filter,
                    &|candidate| character_blocks_collider(entity, candidate, &candidates));
                let applied = *direction * hit.map_or(distance, |hit| movement_sweep_travel(distance, hit.distance));
                let Ok(parent_world) = parents.get(entity).ok()
                    .map(|parent| space.p0().compute_global_transform(parent.parent())).transpose() else { continue; };
                let mut query = space.p2();
                let (mut position, mut transform) = query.get_mut(entity).unwrap();
                position.0 += applied;
                transform.translation = parent_world.map_or(position.0, |parent| parent.affine().inverse().transform_point3(position.0));
                if let Some(start) = runtime.grounded_move_start.as_mut() { *start += applied; }
            }
        }
        base.entity = Some(target);
        base.location = current.translation();
        base.rotation = current.rotation();
    }
}

fn bind_walking_base(
    floor: Option<Entity>, base: &mut crate::als::dynamics::AlsMovementBase,
    transforms: &bevy::transform::helper::TransformHelper,
) {
    let Some(floor) = floor else { return };
    if base.entity == Some(floor) { return; }
    let Ok(current) = transforms.compute_global_transform(floor) else { return };
    base.entity = Some(floor);
    base.location = current.translation();
    base.rotation = current.rotation();
}

pub(crate) fn initialize_spawn_movement_mode(
    mut commands: Commands,
    spatial: SpatialQuery,
    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    pawns: Query<(), With<AlsCharacter>>,
    permissions: Query<&AlsMantleTargetPermission>,
    owners: Query<&ColliderOf>,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut transforms: ParamSet<(bevy::transform::helper::TransformHelper, Query<&mut Transform>)>,
    mut transitions: MessageWriter<StateTransitionEvent>,
    mut characters: Query<(Entity, &RigidBody, &mut LocomotionState, &Stance, &AlsMovementParamsHandle,
        Option<&ChildOf>, &Position, &mut LinearVelocity, &mut crate::als::dynamics::AlsMovementBase,
        &mut AlsCmcAdapterRuntime, (Option<&RotationMode>, &AlsMaxAllowedGait, &AlsViewYaw)),
        (With<AlsCharacter>, With<Collider>, With<crate::als::character::AlsInitialMovementModePending>,
            Without<crate::als::character::AlsBirthAnimationPending>)>,
) {
    for (entity, body, mut locomotion, stance, handle, parent, position, mut velocity, mut base,
        mut runtime, (rotation, gait, yaw)) in &mut characters {
        if *body != RigidBody::Dynamic || *locomotion != LocomotionState::Grounded {
            commands.entity(entity).remove::<crate::als::character::AlsInitialMovementModePending>();
            continue;
        }
        let Some(params) = params_assets.get(handle.id()) else { continue };
        let (world, parent_world) = {
            let helper = transforms.p0();
            let Ok(world) = helper.compute_global_transform(entity) else { continue };
            let Ok(parent_world) = parent.map(|parent| helper.compute_global_transform(parent.parent())).transpose() else { continue };
            (world, parent_world)
        };
        let local = {
            let query = transforms.p1();
            let Ok(local) = query.get(entity) else { continue };
            *local
        };
        let scale = character_floor_scale(&local, parent.is_some(), &world);
        let half = if *stance == Stance::Crouching { params.crouch.actual_crouched_half_height() }
            else { params.crouch.standing_capsule_half_height } * scale.y;
        let radius = params.crouch.standing_capsule_radius * scale.x.min(scale.z);
        let capsule = als_character_capsule(half, radius);
        let mut source_world = world.compute_transform();
        if position.0 != Position::PLACEHOLDER.0 { source_world.translation = position.0; }
        let floor = adjust_walking_floor_height(&spatial, &capsule, &mut source_world, half, radius,
            &params.cmc, entity, &candidates);
        commands.entity(entity).insert(Position(source_world.translation));
        {
            let mut query = transforms.p1();
            if let Ok(mut transform) = query.get_mut(entity) {
                transform.translation = parent_world.map_or(source_world.translation,
                    |parent| parent.affine().inverse().transform_point3(source_world.translation));
            }
        }
        if let Some(floor) = floor {
            velocity.y = 0.0;
            if movement_base_changed(base.entity, floor, &owners)
                && pawn_rejects_character_base(floor, &pawns, &permissions, &owners) {
                let max_speed = crate::als::dynamics::max_allowed_speed(params, *stance,
                    rotation.copied().unwrap_or(RotationMode::ViewDirection), gait.0, velocity.0, yaw.0);
                velocity.0 = jump_off_velocity(velocity.0, max_speed, params.jump.jump_z_velocity,
                    params.cmc.jump_off_jump_z_factor, &mut runtime.random_seed);
                *base = default();
                *locomotion = LocomotionState::Air;
                transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                    from: StateValue::LocomotionState(LocomotionState::Grounded),
                    to: StateValue::LocomotionState(LocomotionState::Air) });
            } else {
                bind_walking_base(Some(floor), &mut base, &transforms.p0());
            }
        } else {
            *base = default();
            *locomotion = LocomotionState::Air;
            transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                from: StateValue::LocomotionState(LocomotionState::Grounded),
                to: StateValue::LocomotionState(LocomotionState::Air) });
        }
        commands.entity(entity).remove::<crate::als::character::AlsInitialMovementModePending>();
    }
}

fn sync_effective_gravity_system(
    gravity: Option<Res<Gravity>>,
    mut characters: Query<
        (
            Option<&GravityScale>,
            Option<&TnuaGravity>,
            &mut TnuaRigidBodyTracker,
            &mut AlsEffectiveGravity,
        ),
        With<AlsCharacter>,
    >,
) {
    let Some(gravity) = gravity else {
        return;
    };
    for (gravity_scale, tnua_gravity, mut tracker, mut effective_gravity) in &mut characters {
        let gravity_scale = gravity_scale.map_or(1.0, |scale| scale.0);
        let tnua_gravity = tnua_gravity.map_or(Vec3::ZERO, |gravity| gravity.0);
        let value = effective_tnua_gravity(gravity.0, gravity_scale, Some(tnua_gravity));
        tracker.gravity = value;
        *effective_gravity = AlsEffectiveGravity {
            value,
            global: gravity.0,
            gravity_scale,
            tnua_gravity,
        };
    }
}

fn refine_ground_sensor_system(
    spatial: SpatialQuery,
    characters: Query<(Entity, &Position, &Rotation, &Collider, &LocomotionState, &TnuaSensorsEntities<AlsControlScheme>), With<AlsCharacter>>,
    mut sensors: Query<&mut TnuaProximitySensor>,
) {
    let walkable_y = deg_to_rad(MAX_SLOPE_DEGREES).cos();
    for (entity, position, rotation, collider, locomotion, entities) in &characters {
        let Some(sensor_entity) = entities.ground else { continue; };
        let Ok(mut sensor) = sensors.get_mut(sensor_entity) else { continue; };
        if *locomotion == LocomotionState::Air {
            sensor.output = None;
            continue;
        }
        if *locomotion != LocomotionState::Grounded { continue; }
        let origin = position.0 + rotation.0 * (collider.scale() * sensor.cast_origin);
        let direction = sensor.cast_direction;
        let range = sensor.cast_range;
        let Some(output) = sensor.output.as_mut() else { continue; };
        if output.normal.y >= walkable_y { continue; }
        if let Some(hit) = spatial.cast_ray(origin, direction, range, false, &SpatialQueryFilter::from_excluded_entities([entity]))
            .filter(|hit| hit.entity == output.entity && hit.distance > 0.0 && hit.normal.y >= walkable_y) {
            output.normal = Dir3::new(hit.normal).unwrap();
        }
    }
}

fn tnua_config_sync_system(
    gravity: Option<Res<Gravity>>,
    transforms: bevy::transform::helper::TransformHelper,
    q: Query<(
        Entity,
        &TnuaConfig<AlsControlScheme>,
        &AlsMovementParamsHandle,
        &AlsMaxAllowedGait,
        &Stance,
        &RotationMode,
        &AlsViewYaw,
        &LinearVelocity,
        &LocomotionState,
        &TnuaController<AlsControlScheme>,
        &TnuaSensorsEntities<AlsControlScheme>,
        Option<&crate::als::dynamics::AlsMovementBaseDynamicsState>,
        Option<&GravityScale>,
        Option<&TnuaGravity>,
    )>,
    mut configs: ResMut<Assets<AlsControlSchemeConfig>>,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut sensors: Query<&mut TnuaProximitySensor>,
) {
    for (entity, config, handle, max_allowed, stance, rotation_mode, view_yaw, velocity, locomotion, controller, sensor_entities, base, gravity_scale, tnua_gravity) in &q {
        let Some(params) = params_assets.get(handle.id()) else {
            continue;
        };
        let Some(mut cfg) = configs.get_mut(&config.0) else {
            continue;
        };
        let Ok(world_transform) = transforms.compute_global_transform(entity) else { continue; };
        let capsule_scale = world_transform.to_scale_rotation_translation().0.y;
        apply_als_control_scheme_config(
            &mut cfg,
            params,
            max_allowed.0,
            *stance,
            *rotation_mode,
            *stance == Stance::Crouching,
            crate::als::dynamics::locomotion_velocity(velocity.0, *locomotion, base),
            view_yaw.0,
            effective_tnua_gravity(
                gravity.as_ref().map_or(Gravity::default().0, |gravity| gravity.0),
                gravity_scale.map_or(1.0, |scale| scale.0),
                tnua_gravity.map(|gravity| gravity.0),
            ).length(),
            capsule_scale,
        );
        if controller.basis.desired_motion == Vec3::ZERO {
            cfg.basis.air_acceleration = 0.0;
        }
        if *locomotion == LocomotionState::Grounded {
            cfg.basis.cling_distance = MAX_STEP_HEIGHT;
        }
        if let Some(entity) = sensor_entities.ground
            && let Ok(mut sensor) = sensors.get_mut(entity) {
            let mut basis_config = cfg.basis.clone();
            if let Some(action) = &controller.current_action {
                bevy_tnua::TnuaActionState::modify_basis_config(action, &mut basis_config);
            }
            sensor.cast_range = basis_config.float_height + basis_config.cling_distance;
        }
    }
}

fn mantle_probe_input(
    world: &GlobalTransform, motion: Option<&crate::actual_motion::AlsActualMotion>,
    locomotion: LocomotionState, stance: Stance, overlay: OverlayState, params: Option<&AlsMovementParams>,
) -> MantleDetectionInput {
    let (half, radius) = params.map_or((ALS_CHARACTER_CAPSULE_HALF_HEIGHT,
        crate::als::character::ALS_CHARACTER_CAPSULE_RADIUS), |params| (
        if stance == Stance::Crouching { params.crouch.actual_crouched_half_height() }
        else { params.crouch.standing_capsule_half_height }, params.crouch.standing_capsule_radius));
    let (scale, _, _) = world.to_scale_rotation_translation();
    let (capsule_half_height, capsule_radius, capsule_scale) = scaled_mantle_capsule_dimensions(half, radius, scale);
    MantleDetectionInput {
        origin: world.translation(), actor_yaw: yaw_from_direction(world.forward().as_vec3()),
        velocity_yaw: motion.filter(|motion| locomotion_movement_flags(motion.has_input, motion.horizontal_speed).0)
            .map(|motion| motion.velocity_yaw_degrees.to_radians()),
        input_yaw: motion.and_then(|motion| motion.has_input.then_some(motion.input_yaw_degrees.to_radians())),
        locomotion, stance, overlay, capsule_half_height, capsule_radius, capsule_scale,
    }
}

fn mantle_target_base(
    intent: crate::als::mantle::MantleIntent,
    transforms: &Query<&GlobalTransform, Without<AlsCharacter>>,
    bodies: &Query<&RigidBody, Without<AlsCharacter>>,
    colliders: &Query<&ColliderOf>,
) -> Option<(crate::als::mantle::MantleIntent, AlsMantleTargetBase)> {
    let transform = transforms.get(intent.target_entity).ok()?;
    let body = colliders.get(intent.target_entity).map_or(intent.target_entity, |collider| collider.body);
    Some((intent, AlsMantleTargetBase {
        entity: intent.target_entity, translation: transform.translation(), rotation: transform.rotation(),
        scale: transform.scale(), relative_location: bodies.get(body).is_ok_and(|body| *body != RigidBody::Static),
    }))
}

type DriveQueryItem<'a> = (
    Entity,
    &'a mut TnuaController<AlsControlScheme>,
    (&'a AlsViewYaw, Option<&'a crate::als::view::AlsPlayerViewYaw>, Option<&'a crate::als::view::AlsPlayerViewReadiness>),
    &'a GlobalTransform,
    &'a LocomotionState,
    Option<&'a crate::actual_motion::AlsActualMotion>,
    Option<&'a mut crate::als::mantle::AlsMantleState>,
    Option<&'a AlsInputOverride>,
    &'a AlsInputCommand,
    &'a Stance,
    &'a OverlayState,
    &'a AlsMovementParamsHandle,
    &'a mut AlsStateRequest,
    Option<&'a mut AlsJumpPrev>,
    (Option<&'a mut AlsCharacterDebug>, &'a mut AlsCmcAdapterRuntime),
);

#[allow(clippy::type_complexity)]
pub(crate) fn locomotion_drive_system(
    time: Res<Time<Virtual>>,
    custom_dilations: Query<&crate::als_camera::AlsCameraCustomTimeDilation>,
    mut commands: Commands,
    spatial: SpatialQuery,
    velocities: Query<(Option<&RigidBody>, Option<&LinearVelocity>, Option<&crate::als::dynamics::AlsComponentVelocity>), Without<AlsCharacter>>,
    target_permissions: Query<Option<&AlsMantleTargetPermission>>,
    target_transforms: Query<&GlobalTransform, Without<AlsCharacter>>,
    target_bodies: Query<&RigidBody, Without<AlsCharacter>>,
    target_colliders: Query<&avian3d::prelude::ColliderOf>,
    mantle_controls: Query<Option<&AlsMantleControlSettings>, With<AlsCharacter>>,
    mantle_start_time_settings: Query<Option<&AlsMantleStartTimeSettings>, With<AlsCharacter>>,
    params_assets: Res<Assets<AlsMovementParams>>,
    rolls: Query<&crate::als::roll::AlsRollState, With<AlsCharacter>>,
    actions: Query<&crate::als::state::AlsNotifyLocomotionAction>,
    blocked: Query<(), With<AlsMovementInputBlocked>>,
    mut q: Query<DriveQueryItem, (With<AlsCharacter>, Without<crate::als::character::AlsBirthAnimationPending>)>,
) {
    for (
        entity,
        mut controller,
        (view_yaw, player_view, readiness),
        gtf,
        loco,
        motion,
        mut mantle,
        override_cmd,
        cmd,
        stance,
        overlay,
        params_handle,
        mut request,
        jump_prev,
        (character_debug, mut runtime),
    ) in &mut q
    {
        runtime.deferred_base_follow = false;
        let roll = rolls.get(entity).ok();
        controller.initiate_action_feeding();
        let ragdolling = actions.get(entity).ok() == Some(&crate::als::state::AlsNotifyLocomotionAction::Ragdolling);
        let delta_seconds = time.delta_secs() * custom_dilations.get(entity).map_or(1.0, |value| value.0);
        if let Some(crate::als::control_scheme::AlsControlSchemeActionState::Jump(jump)) = controller.current_action.as_mut() {
            jump.input.delta_seconds = delta_seconds;
            if *loco != LocomotionState::Air || ragdolling {
                jump.memory.hold_elapsed = f32::INFINITY;
            }
        }
        let input_blocked = blocked.contains(entity);
        let effective =
            movement_input_for_action(*effective_input(cmd, override_cmd), input_blocked);

        let movement_yaw = player_view.map_or_else(|| {
            if readiness.is_some_and(|readiness| !readiness.has_published) {
                world_forward_to_yaw(gtf.forward().as_vec3())
            } else { view_yaw.0 }
        }, |view| view.0);
        let world_input = view_relative_move_input(effective.move_dir, movement_yaw);

        let desired_forward = if roll.is_some_and(|state| state.active) {
            None
        } else {
            Dir3::new(gtf.forward().as_vec3()).ok()
        };

        controller.basis = TnuaBuiltinWalk {
            desired_motion: world_input,
            desired_forward,
        };

        let jump_now = effective.jump_pressed;
        let jump_was = jump_prev.as_deref().map(|j| j.0).unwrap_or(false);
        let jump_edge = jump_now && !jump_was;
        match jump_prev {
            Some(mut jp) => jp.0 = jump_now,
            None => {
                commands.entity(entity).insert(AlsJumpPrev(jump_now));
            }
        }

        let mantle_control = mantle_controls
            .get(entity)
            .ok()
            .flatten()
            .copied()
            .unwrap_or_default();
        let mantle_start_time_settings = mantle_start_time_settings.get(entity).ok().flatten();
        let action_active = input_blocked || actions.get(entity).map_or_else(
            |_| roll.is_some_and(|state| state.active),
            |action| *action != crate::als::state::AlsNotifyLocomotionAction::None,
        );
        let mantle_ready = mantle.as_deref().is_some_and(|m| !m.active)
            && mantle_start_allowed(mantle_control, action_active);
        let auto_mantle = !time.is_paused() && mantle.as_deref().is_some_and(|m| !m.active)
            && auto_mantle_start_allowed(mantle_control, action_active, *loco);
        if auto_mantle && !jump_edge { commands.entity(entity).insert(AlsAutoMantlePending); }
        let should_probe_mantle = mantle_ready && jump_edge;
        let detection = if should_probe_mantle {
            let mut detection = detect_mantle(
                &spatial,
                entity,
                mantle_probe_input(gtf, motion, *loco, *stance, *overlay, params_assets.get(params_handle.id())),
                &velocities,
                &target_permissions,
                &target_colliders,
            );
            detection.trace.draw_failed_traces = mantle_control.draw_failed_traces(*loco);
            Some(detection)
        } else {
            None
        };
        if let (Some(detection), Some(mut debug)) = (detection, character_debug) {
            debug.0.mantle_probe = Some(detection.trace);
        }
        let intent = detection.and_then(|detection| detection.intent)
            .and_then(|intent| mantle_target_base(intent, &target_transforms, &target_bodies, &target_colliders));
        if jump_edge {
            match jump_chain(ragdolling, *stance, intent.is_some()) {
                JumpChain::ExitRagdoll => {
                    request.ragdoll_exit = true;
                }
                JumpChain::Mantle => {
                    if let (Some(m), Some((i, target_base))) = (mantle.as_deref_mut(), intent) {
                        let start_time = configured_mantle_start_time(
                            mantle_start_time_settings,
                            i.kind,
                            i.mantling_height,
                        );
                        m.begin_with_target_base_and_start_time(
                            i.kind,
                            gtf.compute_transform(),
                            i.target,
                            start_time,
                            i.yaw_target,
                            target_base,
                        );
                        info!(
                            "[MOTION] entity={entity:?} mantle begin kind={:?} height={:.2} start_time={start_time:.3}",
                            i.kind, i.mantling_height,
                        );
                    }
                }
                JumpChain::StandUp => {
                    request.stance = Some(Stance::Standing);
                }
                JumpChain::Jump => {
                    if *loco == LocomotionState::Grounded
                        && actions.get(entity).ok().is_none_or(|action| *action == crate::als::state::AlsNotifyLocomotionAction::None) {
                        let Some(params) = params_assets.get(params_handle.id()) else { continue; };
                        controller.action(AlsControlScheme::Jump(crate::als::control_scheme::AlsBuiltinJump {
                            builtin: TnuaBuiltinJump { allow_in_air: true, ..default() },
                            max_hold_time: params.jump.max_hold_time,
                            jump_z_velocity: params.jump.jump_z_velocity,
                            delta_seconds,
                        }));
                        runtime.deferred_base_follow = true;
                    }
                }
            }
        } else if effective.jump_held && controller.action_flow_status().ongoing() == Some(AlsControlSchemeActionDiscriminant::Jump) {
            controller.prolong_action();
        }
    }
}

fn auto_mantle_after_movement(
    mut commands: Commands,
    spatial: SpatialQuery,
    velocities: Query<(Option<&RigidBody>, Option<&LinearVelocity>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
    permissions: Query<Option<&AlsMantleTargetPermission>>,
    target_transforms: Query<&GlobalTransform, Without<AlsCharacter>>,
    target_bodies: Query<&RigidBody, Without<AlsCharacter>>,
    colliders: Query<&ColliderOf>,
    params: Res<Assets<AlsMovementParams>>,
    blocked: Query<(), With<AlsMovementInputBlocked>>,
    transforms: bevy::transform::helper::TransformHelper,
    mut characters: Query<(Entity, &mut AlsMantleState, &LocomotionState, &Stance, &OverlayState,
        &AlsMovementParamsHandle, Option<&crate::actual_motion::AlsActualMotion>, Option<&AlsMantleControlSettings>,
        Option<&AlsMantleStartTimeSettings>, Option<&crate::als::roll::AlsRollState>,
        Option<&crate::als::state::AlsNotifyLocomotionAction>, Option<&mut AlsCharacterDebug>),
        (With<AlsCharacter>, With<AlsAutoMantlePending>)>,
) {
    for (entity, mut mantle, locomotion, stance, overlay, handle, motion, control, start_settings, roll, action, debug) in &mut characters {
        if mantle.active { continue; }
        let action_active = blocked.contains(entity) || action.map_or_else(
            || roll.is_some_and(|state| state.active),
            |action| *action != crate::als::state::AlsNotifyLocomotionAction::None);
        let control = control.copied().unwrap_or_default();
        if !auto_mantle_start_allowed(control, action_active, *locomotion) {
            commands.entity(entity).remove::<AlsAutoMantlePending>();
            continue;
        }
        let Ok(world) = transforms.compute_global_transform(entity) else {
            commands.entity(entity).remove::<AlsAutoMantlePending>();
            continue;
        };
        let mut detection = detect_mantle(&spatial, entity,
            mantle_probe_input(&world, motion, *locomotion, *stance, *overlay, params.get(handle.id())),
            &velocities, &permissions, &colliders);
        detection.trace.draw_failed_traces = control.draw_failed_traces(*locomotion);
        if let Some(mut debug) = debug { debug.0.mantle_probe = Some(detection.trace); }
        let Some((intent, base)) = detection.intent
            .and_then(|intent| mantle_target_base(intent, &target_transforms, &target_bodies, &colliders)) else {
            commands.entity(entity).remove::<AlsAutoMantlePending>();
            continue;
        };
        let start_time = configured_mantle_start_time(start_settings, intent.kind, intent.mantling_height);
        mantle.begin_with_target_base_and_start_time(intent.kind, world.compute_transform(), intent.target, start_time,
            intent.yaw_target, base);
    }
}

pub(crate) type ReshapeQueryItem<'a> = (
    Entity,
    &'a mut Stance,
    &'a AlsDesiredStance,
    Option<&'a AlsPhysicalStanceOverride>,
    &'a LocomotionState,
    Option<&'a AlsMantleState>,
    Option<&'a crate::als::state::AlsNotifyLocomotionAction>,
    &'a Transform,
    Option<&'a ChildOf>,
    &'a AlsMovementParamsHandle,
    Has<CapsuleReshapeDeferred>,
    &'a mut AlsCrouchTransaction,
);

pub(crate) fn capsule_reshape_system(
    mut commands: Commands,
    spatial_query: SpatialQuery,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut transitions: MessageWriter<StateTransitionEvent>,
    transforms: bevy::transform::helper::TransformHelper,
    mut q: Query<ReshapeQueryItem, With<AlsCharacter>>,
    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
) {
    for (
        entity,
        mut stance,
        desired,
        physical_override,
        locomotion,
        mantle,
        action,
        transform,
        parent,
        handle,
        already_deferred,
        mut transaction,
    ) in &mut q
    {
        let Some(params) = params_assets.get(handle.id()) else {
            continue;
        };
        let Ok(world) = transforms.compute_global_transform(entity) else { continue; };
        let (half_height, radius) = capsule_dimensions(params, *stance);
        let action_active = action.map_or_else(
            || mantle.is_some_and(|mantle| mantle.active),
            |action| *action != crate::als::state::AlsNotifyLocomotionAction::None,
        );
        transaction.synchronize(
            desired.0,
            *stance,
            half_height,
            radius,
            world.translation(),
            *locomotion == LocomotionState::Air && !action_active,
        );
        let target = capsule_target_for_locomotion(
            desired.0,
            *stance,
            *locomotion,
            action_active,
            physical_override.map(|value| value.0),
        );
        if target == *stance && !already_deferred {
            continue;
        }
        let Ok(parent_world) = parent.map(|parent| transforms.compute_global_transform(parent.parent())).transpose() else { continue; };
        apply_capsule_for_stance(
            &mut commands,
            &spatial_query,
            &candidates,
            params,
            entity,
            target,
            *locomotion,
            &mut stance,
            &mut transaction,
            transform,
            &world,
            parent_world.as_ref(),
            already_deferred,
            &mut transitions,
        );
    }
}

fn apply_capsule_for_stance(
    commands: &mut Commands,
    spatial_query: &SpatialQuery,
    candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    params: &AlsMovementParams,
    entity: Entity,
    target: Stance,
    locomotion: LocomotionState,
    stance: &mut Stance,
    transaction: &mut AlsCrouchTransaction,
    transform: &Transform,
    world: &GlobalTransform,
    parent_world: Option<&GlobalTransform>,
    already_deferred: bool,
    transitions: &mut MessageWriter<StateTransitionEvent>,
) {
    let center = world.translation();
    let rotation = world.rotation();
    let scale = world.to_scale_rotation_translation().0;
    let effective_scale = scale.abs().min_element();
    let mut target_center = capsule_center_for_stance(params, center, *stance, target, effective_scale);
    match target {
        Stance::Crouching => {
            let (half_height, radius) = capsule_dimensions(params, target);
            let (current_half, _) = capsule_dimensions(params, *stance);
            if half_height > current_half {
                let capsule = als_character_capsule(half_height * scale.y, radius * scale.x.min(scale.z));
                let filter = SpatialQueryFilter::from_excluded_entities([entity]);
                if spatial_query.shape_intersections(&capsule, target_center, Quat::IDENTITY, &filter)
                    .into_iter().any(|candidate| character_blocks_collider(entity, candidate, candidates)) {
                    return;
                }
            }
            if let Ok((direction, distance)) = Dir3::new_and_length(target_center - center) {
                let (scaled_radius, axis_half) = crate::als::character::physical_capsule_dimensions(half_height, radius, scale, 0.000001);
                let capsule = Collider::capsule(scaled_radius, axis_half * 2.0);
                let filter = SpatialQueryFilter::from_excluded_entities([entity]);
                let config = ShapeCastConfig { ignore_origin_penetration: true, ..ShapeCastConfig::from_max_distance(distance) };
                let hit = spatial_query.cast_shape_predicate(&capsule, center, rotation, direction, &config,
                    &filter, &|candidate| character_blocks_collider(entity, candidate, candidates));
                target_center = center + *direction * hit.map_or(distance, |hit| movement_sweep_travel(distance, hit.distance));
            }
            commands
                .entity(entity)
                .remove::<CapsuleReshapeDeferred>()
                .insert(AlsCapsuleCrouched)
                .insert(capsule_for(params, target));
            apply_actual_stance(entity, stance, target, transitions);
            apply_capsule_center(commands, entity, transform, parent_world, target_center);
            transaction.crouch_applied(half_height, radius, target_center);
        }
        Stance::Standing => {
            let filter = SpatialQueryFilter::from_excluded_entities([entity]);
            let (standing_half, radius) = capsule_dimensions(params, Stance::Standing);
            let (current_half, _) = capsule_dimensions(params, *stance);
            let standing_capsule = als_character_capsule(current_half * scale.y
                + (standing_half - current_half) * effective_scale + STAND_UP_SWEEP_INFLATION,
                radius * scale.x.min(scale.z));
            target_center.y += STAND_UP_SWEEP_INFLATION;
            let mut blocked = spatial_query
                .shape_intersections(&standing_capsule, target_center, Quat::IDENTITY, &filter)
                .into_iter().any(|candidate| character_blocks_collider(entity, candidate, candidates));
            if blocked && locomotion == LocomotionState::Grounded {
                let (half, radius) = capsule_dimensions(params, *stance);
                if let Some((floor_center, _, _)) = find_ground_floor(spatial_query, center,
                    half * scale.y, radius * scale.x.min(scale.z), &params.cmc, locomotion, &filter, entity, candidates) {
                    let floor_distance = center.y - floor_center;
                    if floor_distance > STAND_UP_MIN_FLOOR_DISTANCE {
                        target_center.y -= floor_distance - STAND_UP_MIN_FLOOR_DISTANCE;
                        blocked = spatial_query
                            .shape_intersections(&standing_capsule, target_center, Quat::IDENTITY, &filter)
                            .into_iter().any(|candidate| character_blocks_collider(entity, candidate, candidates));
                    }
                }
            }
            if blocked {
                let (half_height, radius) = capsule_dimensions(params, Stance::Crouching);
                transaction.stand_blocked(half_height, radius, center);
                if !already_deferred {
                    info!("[MOTION] entity={entity:?} stand-up deferred: target capsule blocked");
                    commands.entity(entity).insert(CapsuleReshapeDeferred);
                }
                return;
            }
            let was_deferred = already_deferred;
            commands
                .entity(entity)
                .remove::<CapsuleReshapeDeferred>()
                .remove::<AlsCapsuleCrouched>()
                .insert(capsule_for(params, Stance::Standing));
            apply_actual_stance(entity, stance, target, transitions);
            let (half_height, radius) = capsule_dimensions(params, Stance::Standing);
            apply_capsule_center(commands, entity, transform, parent_world, target_center);
            transaction.standing_applied(half_height, radius, target_center);
            if was_deferred {
                info!("[MOTION] entity={entity:?} clearance regained: standing capsule restored");
            }
        }
        _ => {}
    }
}

fn capsule_dimensions(params: &AlsMovementParams, stance: Stance) -> (f32, f32) {
    let half_height = if stance == Stance::Crouching { params.crouch.actual_crouched_half_height() } else { params.crouch.standing_capsule_half_height };
    (half_height, params.crouch.standing_capsule_radius)
}

fn capsule_center_for_stance(
    params: &AlsMovementParams,
    current_center: Vec3,
    current_stance: Stance,
    target_stance: Stance,
    effective_scale: f32,
) -> Vec3 {
    let (current_half_height, _) = capsule_dimensions(params, current_stance);
    let (target_half_height, _) = capsule_dimensions(params, target_stance);
    current_center + Vec3::Y * (target_half_height - current_half_height) * effective_scale
}

fn apply_capsule_center(
    commands: &mut Commands,
    entity: Entity,
    transform: &Transform,
    parent_world: Option<&GlobalTransform>,
    center: Vec3,
) {
    let mut next_transform = transform.clone();
    next_transform.translation = parent_world.map_or(center, |parent| parent.affine().inverse().transform_point3(center));
    commands
        .entity(entity)
        .insert((Position(center), next_transform));
}

fn capsule_target_for_locomotion(
    desired: Stance,
    actual: Stance,
    locomotion: LocomotionState,
    action_active: bool,
    physical_override: Option<Stance>,
) -> Stance {
    if let Some(physical_override) = physical_override {
        physical_override
    } else if action_active || !matches!(locomotion, LocomotionState::Grounded | LocomotionState::Air) {
        actual
    } else if locomotion == LocomotionState::Air {
        Stance::Standing
    } else if matches!(desired, Stance::Standing | Stance::Crouching) {
        desired
    } else {
        actual
    }
}

fn crouch_controller_transaction_system(
    mut transactions: Query<(&Stance, &mut AlsCrouchTransaction, &mut TnuaController<AlsControlScheme>), With<AlsCharacter>>,
) {
    for (stance, mut transaction, mut controller) in &mut transactions {
        transaction.controller_crouch_active = *stance == Stance::Crouching;
        if transaction.controller_crouch_active {
            controller.action(AlsControlScheme::Crouch(TnuaBuiltinCrouch));
        }
    }
}

fn apply_actual_stance(
    entity: Entity,
    stance: &mut Stance,
    target: Stance,
    transitions: &mut MessageWriter<StateTransitionEvent>,
) {
    if *stance == target {
        return;
    }
    let from = *stance;
    *stance = target;
    transitions.write(StateTransitionEvent {
        entity,
        dimension: Dimension::Stance,
        from: StateValue::Stance(from),
        to: StateValue::Stance(target),
    });
}

fn walking_step_slide_normal(delta: Vec3, normal: Vec3, impact_normal: Vec3, floor_normal: Vec3, floor_distance: f32) -> Vec3 {
    if normal.y > 0.0 && impact_normal.y < deg_to_rad(MAX_SLOPE_DEGREES).cos() {
        normal.with_y(0.0).normalize_or_zero()
    } else if normal.y < -1e-4 && floor_distance < MIN_FLOOR_DISTANCE {
        let selected = if delta.dot(floor_normal) < 0.0 && floor_normal.y < 1.0 - 1e-5 { floor_normal } else { normal };
        selected.with_y(0.0).normalize_or_zero()
    } else { normal }
}

fn walking_two_wall_adjust(delta: Vec3, normal: Vec3, impact: Vec3, previous: Vec3, remaining: f32, floor_distance: f32) -> Vec3 {
    let dot = previous.dot(normal);
    let mut adjusted = if dot <= 0.0 {
        let crease = normal.cross(previous).normalize_or_zero();
        let result = crease * delta.dot(crease) * remaining;
        if result.dot(delta) < 0.0 { -result } else { result }
    } else {
        let result = (delta - normal * delta.dot(normal)) * remaining;
        if result.dot(delta) <= 0.0 { Vec3::ZERO }
        else if (dot - 1.0).abs() < 1e-4 { result + normal * 0.0001 }
        else { result }
    };
    if adjusted.y > 0.0 {
        if (normal.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos() || impact.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos()) && normal.y > 1e-4 {
            let up = adjusted.normalize_or_zero().y * delta.length() / normal.y;
            adjusted = (delta.with_y(0.0) + Vec3::Y * up) * remaining;
            if adjusted.y > MAX_STEP_HEIGHT { adjusted *= MAX_STEP_HEIGHT / adjusted.y; }
        } else {
            adjusted.y = 0.0;
        }
    } else if adjusted.y < 0.0 && floor_distance < MIN_FLOOR_DISTANCE {
        adjusted.y = 0.0;
    }
    adjusted
}

fn ground_ramp_movement(delta: Vec3, contact: Vec3, impact: Vec3, line: bool) -> Vec3 {
    if impact.y < 1.0 - 1e-4 && impact.y > 1e-4 && contact.y > 1e-4 && !line
        && impact.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos() {
        delta.with_y(-impact.dot(delta) / impact.y)
    } else { delta }
}

fn walking_slide_sweep(spatial: &SpatialQuery, capsule: &Collider, origin: Vec3,
    delta: Vec3, normal: Vec3, floor_distance: f32, filter: &SpatialQueryFilter, accepts: &dyn Fn(Entity) -> bool,
    mut impacts: Option<&mut Vec<(ShapeHitData, Vec3)>>) -> Option<Vec3> {
    let slide = delta - normal * delta.dot(normal);
    if slide.dot(delta) <= 0.0 { return Some(Vec3::ZERO); }
    let Ok((direction, distance)) = Dir3::new_and_length(slide) else { return Some(Vec3::ZERO) };
    let config = ShapeCastConfig { ignore_origin_penetration: true, ..ShapeCastConfig::from_max_distance(distance) };
    let hit = spatial.cast_shape_predicate(capsule, origin, Quat::IDENTITY, direction, &config, filter, accepts);
    if hit.is_some_and(|hit| hit.distance == 0.0 && (hit.point1 - hit.point2).dot(hit.normal1) > 0.0) { return None; }
    let travel = hit.map_or(distance, |hit| movement_sweep_travel(distance, hit.distance));
    let mut progress = *direction * travel;
    if let Some(hit) = hit {
        let impact = source_sweep_impact_normal(spatial, &hit, slide, origin, Quat::IDENTITY, capsule);
        if let Some(impacts) = impacts.as_deref_mut() { impacts.push((hit, impact)); }
        let adjusted = walking_two_wall_adjust(slide, hit.normal1, impact, normal, 1.0 - travel / distance, floor_distance);
        if adjusted.abs().max_element() > 1e-5 && adjusted.dot(delta) > 0.0
            && let Ok((next_direction, next_distance)) = Dir3::new_and_length(adjusted) {
            let next_config = ShapeCastConfig { max_distance: next_distance, ..config };
            let next = spatial.cast_shape_predicate(capsule, origin + progress, Quat::IDENTITY,
                next_direction, &next_config, filter, accepts);
            if let Some(hit) = next && let Some(impacts) = impacts.as_deref_mut() {
                let impact = source_sweep_impact_normal(spatial, &hit, adjusted, origin + progress, Quat::IDENTITY, capsule);
                impacts.push((hit, impact));
            }
            let travel = next.map_or(next_distance, |hit| movement_sweep_travel(next_distance, hit.distance));
            progress += *next_direction * travel;
        }
    }
    Some(progress)
}

fn record_ground_impact(context: &mut Option<(&mut Messages<SourceCharacterImpact>, Vec3, Vec3, f32, &FallingImpactBodies)>,
    hit: &ShapeHitData, normal: Vec3) {
    if let Some((writer, acceleration, velocity, max_speed, _)) = context.as_mut() {
        writer.write(SourceCharacterImpact { collider: hit.entity, point: hit.point1,
            normal, acceleration: *acceleration, velocity: *velocity, max_speed: *max_speed });
    }
}

pub(crate) fn grounded_movement_transaction(
    spatial_query: &SpatialQuery, capsule: &Collider, center: Vec3, rotation: Quat,
    half: f32, radius: f32, delta: Vec3, cmc: &crate::als::params::CmcAdapterParams,
    mode: LocomotionState, entity: Entity,
    candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    step_permissions: &Query<&AlsMantleTargetPermission>,
    mut impact_context: Option<(&mut Messages<SourceCharacterImpact>, Vec3, Vec3, f32, &FallingImpactBodies)>,
) -> Option<(Vec3, Option<Vec3>, Option<f32>, bool, bool, bool)> {
    let mut pending_step_impact = None;
    let mut pending_fallback_impacts = Vec::new();
    let outcome = (|| {
    let filter = SpatialQueryFilter::from_excluded_entities([entity]);
    let accepts = |candidate| character_blocks_collider(entity, candidate, &candidates);
    let can_step_up = |candidate| {
        let body = candidates.get(candidate).ok().and_then(|parts| parts.2).map_or(candidate, |collider| collider.body);
        step_permissions.get(candidate).or_else(|_| step_permissions.get(body)).map_or(true, |permission| permission.allows_step_up)
    };
    let floor = find_ground_floor_with_penetration(spatial_query, center, half, radius, cmc, mode, &filter, entity, candidates, None, None);
    let ramp_delta = floor.map_or(delta, |floor| ground_ramp_movement(delta, floor.hit.normal1, floor.impact_normal,
        floor.origin != GroundFloorOrigin::Sweep));
    let ramp_movement = ramp_delta != delta;
    let (direction, distance) = Dir3::new_and_length(ramp_delta).ok()?;
    let mut forward_config = ShapeCastConfig {
        ignore_origin_penetration: true,
        ..ShapeCastConfig::from_max_distance(distance)
    };
    let clearance = ALS_CHARACTER_FLOOR_CLEARANCE + ALS_CHARACTER_TNUA_FLOAT_EPSILON;
    let mut raised = center;
    let mut remaining_delta = ramp_delta;
    let mut step_delta = delta;
    let mut initial_floor_y = None;
    let raw_config = ShapeCastConfig { ignore_origin_penetration: false, ..forward_config };
    let raw_hit = spatial_query.cast_shape_predicate(capsule, center, Quat::IDENTITY, direction, &raw_config, &filter, &accepts);
    let penetrating = raw_hit.as_ref().is_some_and(|hit| hit.distance == 0.0
        && (hit.point1 - hit.point2).dot(hit.normal1) > 0.0);
    let unresolved = penetrating && impact_context.as_ref().is_some_and(|(_, _, _, _, bodies)| {
        falling_resolve_penetration(spatial_query, capsule, center, Quat::IDENTITY,
            raw_hit.as_ref().unwrap(), ramp_delta, &filter, &accepts, bodies).is_none()
    });
    if unresolved {
        let hit = raw_hit.as_ref().unwrap();
        record_ground_impact(&mut impact_context, hit, hit.normal1);
        impact_context = None;
    }
    let forward_hit = if raw_hit.as_ref().is_some_and(|hit| hit.distance == 0.0) {
        spatial_query.cast_shape_predicate(capsule, center, Quat::IDENTITY, direction, &forward_config, &filter, &accepts)
    } else { raw_hit };
    let mut result = (center, ramp_movement.then_some(ramp_delta), None, penetrating, false, ramp_movement);
    if let Some(first) = forward_hit {
        let forward_travel = movement_sweep_travel(distance, first.distance);
        let advance = *direction * forward_travel;
        raised += advance;
        remaining_delta = if ramp_movement { delta * (1.0 - forward_travel / distance) } else { delta - advance };
        step_delta = remaining_delta;
        result.0 = raised;
        result.5 = false;
        let mut impact_normal = source_sweep_impact_normal(spatial_query, &first, ramp_delta, center, Quat::IDENTITY, capsule);
        let hit = if first.distance > 0.0 && first.normal1.y > 1e-4
            && impact_normal.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos() {
            let projected = ground_ramp_movement(remaining_delta, first.normal1, impact_normal, false);
            let second = Dir3::new_and_length(projected).ok().and_then(|(direction, distance)| {
                spatial_query.cast_shape_predicate(capsule, raised, Quat::IDENTITY, direction,
                    &ShapeCastConfig { max_distance: distance, ..forward_config }, &filter, &accepts)
                    .map(|hit| (hit, direction, distance))
            });
            if let Some((hit, direction, distance)) = second {
                if hit.distance == 0.0 && (hit.point1 - hit.point2).dot(hit.normal1) > 0.0 {
                    result.1 = Some(Vec3::ZERO);
                    result.5 = true;
                    return Some(result);
                }
                impact_normal = source_sweep_impact_normal(spatial_query, &hit, projected, raised, Quat::IDENTITY, capsule);
                let travel = movement_sweep_travel(distance, hit.distance);
                raised += *direction * travel;
                remaining_delta *= 1.0 - travel / distance;
                step_delta = remaining_delta;
                result.0 = raised;
                Some(hit)
            } else {
                remaining_delta = projected;
                result.1 = Some(projected);
                result.5 = true;
                None
            }
        } else { Some(first) };
        if let Some(hit) = hit {
            forward_config.max_distance = remaining_delta.length();
            let (floor_normal, floor_distance) = floor.map_or((Vec3::Y, f32::INFINITY),
                |floor| (floor.hit.normal1, floor.sweep_distance));
            let slide_normal = walking_step_slide_normal(remaining_delta, hit.normal1, impact_normal, floor_normal, floor_distance);
            let mut fallback_impacts = Vec::new();
            result.1 = Some(walking_slide_sweep(spatial_query, capsule, raised, remaining_delta, slide_normal, floor_distance,
                &filter, &accepts, Some(&mut fallback_impacts))
                .unwrap_or(Vec3::ZERO));
            result.5 = true;
            if !can_step_up(hit.entity) {
                record_ground_impact(&mut impact_context, &hit, impact_normal);
                for (hit, normal) in fallback_impacts { record_ground_impact(&mut impact_context, &hit, normal); }
                return Some(result);
            }
            pending_step_impact = Some((hit, impact_normal));
            pending_fallback_impacts = fallback_impacts;
            if hit.point1.y <= center.y - half - clearance || hit.point1.y > center.y + half - radius {
                return Some(result);
            }
            let Some(floor) = floor else { return Some(result) };
            let floor_center = floor.center_y;
            initial_floor_y = Some(floor.hit.point1.y);
            let up_config = ShapeCastConfig {
                ignore_origin_penetration: true,
                ..ShapeCastConfig::from_max_distance((MAX_STEP_HEIGHT - (center.y - floor_center).max(0.0)).max(0.0))
            };
            let up_origin = raised;
            let up_hit = spatial_query.cast_shape_predicate(capsule, raised, Quat::IDENTITY, Dir3::Y, &up_config, &filter, &accepts);
            let lift = up_hit.map_or(up_config.max_distance, |hit| movement_sweep_travel(up_config.max_distance, hit.distance));
            raised.y += lift;
            let step_direction = Dir3::new(remaining_delta).unwrap_or(direction);
            if let Some(hit) = spatial_query.cast_shape_predicate(capsule, raised, Quat::IDENTITY,
                step_direction, &forward_config, &filter, &accepts) {
                if hit.distance == 0.0 && (hit.point1 - hit.point2).dot(hit.normal1) > 0.0 { return Some(result); }
                let forward_travel = movement_sweep_travel(forward_config.max_distance, hit.distance);
                let advance = *step_direction * forward_travel;
                let impact_normal = source_sweep_impact_normal(spatial_query, &hit, *step_direction * forward_config.max_distance,
                    raised, Quat::IDENTITY, capsule);
                let step_impacts_enabled = !up_hit.is_some_and(|hit| hit.distance == 0.0 && (hit.point1 - hit.point2).dot(hit.normal1) > 0.0);
                if step_impacts_enabled {
                    if let Some(up_hit) = up_hit {
                        let normal = source_sweep_impact_normal(spatial_query, &up_hit, Vec3::Y * up_config.max_distance,
                            up_origin, Quat::IDENTITY, capsule);
                        record_ground_impact(&mut impact_context, &up_hit, normal);
                    }
                    record_ground_impact(&mut impact_context, &hit, impact_normal);
                }
                let normal = walking_step_slide_normal(remaining_delta, hit.normal1, impact_normal, floor_normal, floor_distance);
                let mut step_slide_impacts = Vec::new();
                let Some(slide) = walking_slide_sweep(spatial_query, capsule, raised + advance,
                    remaining_delta - advance, normal, floor_distance, &filter, &accepts,
                    Some(&mut step_slide_impacts)) else { return Some(result) };
                if step_impacts_enabled {
                    for (hit, normal) in step_slide_impacts { record_ground_impact(&mut impact_context, &hit, normal); }
                }
                if forward_travel == 0.0 && slide.length_squared() == 0.0 { return Some(result); }
                remaining_delta = advance + slide;
            }
        }
    }
    let destination = raised + remaining_delta;
    let down_distance = MAX_STEP_HEIGHT + MAX_FLOOR_DISTANCE * 2.0;
    let Some((floor, impact_normal)) = floor_sweep_hit(&spatial_query, capsule, radius, destination,
        down_distance, &filter, &accepts) else {
        if initial_floor_y.is_some() && spatial_query.cast_shape_predicate(capsule, destination, Quat::IDENTITY,
            Dir3::NEG_Y, &ShapeCastConfig::from_max_distance(down_distance), &filter, &accepts).is_none() {
            result.1 = Some(remaining_delta);
            result.2 = Some(destination.y - down_distance);
            result.4 = true;
        }
        return Some(result);
    };
    if floor.distance == 0.0 && (floor.point1 - floor.point2).dot(floor.normal1) > 0.0 { return Some(result); }
    let down_travel = movement_sweep_travel(down_distance, floor.distance);
    if impact_normal.y < deg_to_rad(MAX_SLOPE_DEGREES).cos()
        && (initial_floor_y.is_none() || step_delta.dot(floor.normal1) < 0.0
            || destination.y - down_travel > center.y) {
        return Some(result);
    }
    let next_y = if initial_floor_y.is_some() { destination.y - down_travel }
        else if (MIN_FLOOR_DISTANCE..=MAX_FLOOR_DISTANCE).contains(&floor.distance) { destination.y }
        else { destination.y - floor.distance + clearance };
    if initial_floor_y.is_some_and(|initial| floor.point1.y > initial) && !can_step_up(floor.entity) {
        return Some(result);
    }
    if initial_floor_y.is_some_and(|initial| floor.point1.y - initial > MAX_STEP_HEIGHT) {
        return Some(result);
    }
    if initial_floor_y.is_some() && spatial_query.shape_intersections(capsule, destination.with_y(next_y),
        rotation, &filter).into_iter().any(accepts) { return Some(result); }
    if result.1.is_some() { result.1 = Some(remaining_delta); }
    result.2 = Some(next_y);
    result.4 = initial_floor_y.is_some();
    Some(result)
    })();
    if let Some((hit, normal)) = pending_step_impact
        && !outcome.as_ref().is_some_and(|result| result.4) {
        record_ground_impact(&mut impact_context, &hit, normal);
        for (hit, normal) in pending_fallback_impacts { record_ground_impact(&mut impact_context, &hit, normal); }
    }
    outcome
}

fn character_floor_scale(local: &Transform, parented: bool, world: &GlobalTransform) -> Vec3 {
    if parented { world.to_scale_rotation_translation().0 } else { local.scale }
}

pub(crate) fn adjust_walking_floor_height(
    spatial: &SpatialQuery, capsule: &Collider, transform: &mut Transform, half: f32, radius: f32,
    cmc: &crate::als::params::CmcAdapterParams, entity: Entity,
    candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
) -> Option<Entity> {
    let filter = SpatialQueryFilter::from_excluded_entities([entity]);
    let floor = find_ground_floor_with_penetration(spatial, transform.translation, half, radius, cmc,
        LocomotionState::Grounded, &filter, entity, candidates, None, None)?;
    if line_floor_blocks_height_adjustment(floor.sweep_distance, floor.line_distance) { return Some(floor.hit.entity); }
    let center = floor.center_y;
    if !(MIN_FLOOR_DISTANCE..=MAX_FLOOR_DISTANCE).contains(&(transform.translation.y - center)) {
        let adjustment = Vec3::Y * (center + (MIN_FLOOR_DISTANCE + MAX_FLOOR_DISTANCE) * 0.5 - transform.translation.y);
        if let Ok((direction, distance)) = Dir3::new_and_length(adjustment) {
            let hit = spatial.cast_shape_predicate(capsule, transform.translation, transform.rotation,
                direction, &ShapeCastConfig { ignore_origin_penetration: true,
                    ..ShapeCastConfig::from_max_distance(distance) }, &filter,
                &|candidate| character_blocks_collider(entity, candidate, candidates));
            transform.translation += *direction * hit.map_or(distance, |hit| movement_sweep_travel(distance, hit.distance));
        }
    }
    Some(floor.hit.entity)
}

fn walking_movement_transaction(
    spatial: &SpatialQuery, capsule: &Collider, mut transform: Transform, half: f32, radius: f32,
    velocity: Vec3, desired_motion: Vec3, max_speed: f32, dynamics: crate::als::params::GroundedDynamics,
    cmc: &crate::als::params::CmcAdapterParams, friction: f32, delta: f32, entity: Entity,
    candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    step_permissions: &Query<&AlsMantleTargetPermission>,
    bases: &Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
    owners: &Query<&ColliderOf>, pawns: &Query<(), With<AlsCharacter>>, mut last_base: Option<Entity>,
    mut impacts: Option<(&mut Messages<SourceCharacterImpact>, Vec3, f32, &FallingImpactBodies)>,
) -> (Transform, Vec3, u32, f32, bool, Vec3, Option<Entity>, bool) {
    let mut last_input = velocity;
    let mut lost_floor = false;
    let mut rejected_base = false;
    let (velocity, steps, _, remaining_time) = crate::als::dynamics::simulate_grounded_motion_until_mode_change(
        velocity, desired_motion, max_speed, dynamics, cmc, friction, delta, |step_velocity, step_seconds| {
            last_input = step_velocity;
            let center = transform.translation;
            let requested = step_velocity * step_seconds;
            let Some((advanced, remaining, next_y, penetrating, stepped_up, _)) = grounded_movement_transaction(spatial, capsule, center,
                transform.rotation, half, radius, requested, cmc, LocomotionState::Grounded, entity, candidates, step_permissions,
                impacts.as_mut().map(|(writer, acceleration, speed, bodies)| (&mut **writer, *acceleration, step_velocity, *speed, *bodies)))
                else { return (step_velocity, Vec3::ZERO, true) };
            transform.translation = advanced + remaining.unwrap_or(requested);
            if let Some(next_y) = next_y && (stepped_up || next_y > center.y) { transform.translation.y = next_y; }
            let floor = adjust_walking_floor_height(spatial, capsule, &mut transform, half, radius, cmc, entity, candidates);
            lost_floor = floor.is_none();
            rejected_base = floor.is_some_and(|floor| movement_base_changed(last_base, floor, owners)
                && pawn_rejects_character_base(floor, pawns, step_permissions, owners));
            if floor.is_some() { last_base = floor; }
            let applied = transform.translation - center;
            let actual_velocity = if penetrating {
                step_velocity
            } else if remaining.is_none() && !stepped_up && floor.is_some()
                && crate::als::dynamics::support_velocity_from_base(floor,
                    transform.translation - Vec3::Y * half, bases, owners).1 == Vec3::ZERO {
                let origin_cm = center.as_dvec3() * 100.0;
                let destination_cm = origin_cm + requested.as_dvec3() * 100.0;
                ((destination_cm - origin_cm) / f64::from(step_seconds) / 100.0).as_vec3().with_y(0.0)
            } else {
                applied.with_y(0.0) / step_seconds
            };
            (actual_velocity, if rejected_base { requested } else { applied }, !lost_floor && !rejected_base)
        });
    (transform, velocity, steps, remaining_time, lost_floor, last_input, last_base, rejected_base)
}

pub(crate) fn cmc_movement_transaction_system(
    time: Res<Time>,
    custom_dilations: Query<&crate::als_camera::AlsCameraCustomTimeDilation>,
    motion: Query<&crate::actual_motion::AlsActualMotion>,
    mut commands: Commands,
    (mut transitions, mut impacts, ragdoll_eligibility): (MessageWriter<StateTransitionEvent>,
        Option<ResMut<Messages<SourceCharacterImpact>>>, crate::ragdoll_link::AlsRagdollStartEligibility),
    spatial_query: SpatialQuery,
    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    step_permissions: Query<&AlsMantleTargetPermission>,
    params_assets: Res<Assets<AlsMovementParams>>,
    blocked: Query<(), With<AlsMovementInputBlocked>>,
    default_volume: Res<crate::als::dynamics::AlsDefaultPhysicsVolume>,
    physics_modes: Query<(&RigidBody, Option<&crate::als::dynamics::AlsPhysicsVolume>, Has<avian3d::prelude::CustomPositionIntegration>)>,
    motion_roots: Query<(Option<&crate::als::roll::AlsRollState>, Option<&AlsMantleState>, Option<&crate::als::state::AlsNotifyLocomotionAction>)>,
    mut transforms: ParamSet<(bevy::transform::helper::TransformHelper, Query<&mut Transform, With<AlsCharacter>>)>,
    base_queries: (
        Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
        Query<&ColliderOf>,
        Query<&mut crate::als::dynamics::AlsMovementBase, With<AlsCharacter>>,
        Query<(&Collider, &Position, &Rotation)>,
        FallingImpactBodies,
    ),
    mut q: Query<
        (
            Entity,
            &Position,
            &Collider,
            &LinearVelocity,
            &mut TnuaMotor,
            &mut SpeculativeMargin,
            &mut AlsCmcAdapterRuntime,
            &AlsEffectiveGravity,
            &mut LocomotionState,
            &TnuaController<AlsControlScheme>,
            &AlsMovementParamsHandle,
            &Stance,
            Option<&ChildOf>,
            (&RotationMode, &AlsViewYaw, &AlsMaxAllowedGait,
                Option<&mut crate::als::dynamics::AlsGroundedDynamics>,
                Option<&crate::als::dynamics::AlsLandingBrakingFriction>,
                Option<&mut crate::als::dynamics::AlsMovementBaseDynamicsState>),
        ),
        (With<AlsCharacter>, Without<crate::als::character::AlsBirthAnimationPending>),
    >,
) {
    let (bases, owners, mut movement_bases, shapes, impact_bodies) = base_queries;
    for (entity, position, capsule, velocity, mut motor, mut margin, mut runtime, gravity, mut loco, controller, handle, stance, parent, (rotation_mode, view_yaw, max_allowed_gait, mut dynamics, braking, mut base)) in &mut q {
        let Some(params) = params_assets.get(handle.id()) else { continue };
        let cached_input_max = runtime.controlled_input_max_acceleration;
        let Ok(world) = transforms.p0().compute_global_transform(entity) else { continue };
        let Ok(parent_world) = parent.map(|parent| transforms.p0().compute_global_transform(parent.parent())).transpose() else { continue };
        let mut transform_query = transforms.p1();
        let Ok(mut transform) = transform_query.get_mut(entity) else { continue };
        let scale = character_floor_scale(&transform, parent.is_some(), &world);
        let mut landing_braking = braking.copied().unwrap_or_default();
        landing_braking.previous_locomotion = LocomotionState::Air;
        if let Ok(motion) = motion.get(entity) {
            crate::als::dynamics::update_landing_braking_friction(&mut landing_braking, LocomotionState::Grounded,
                motion.has_input, motion.world_velocity.y <= -crate::als::roll::ROLL_ON_LAND_SPEED_THRESHOLD);
        }
        let ragdoll_on_land = crate::get_up::RAGDOLL_SETTINGS.start_on_land
            && motion.get(entity).is_ok_and(|motion| motion.world_velocity.y <= -crate::get_up::RAGDOLL_SETTINGS.on_land_speed_threshold_mps);
        let roll_on_land = !ragdoll_on_land
            && motion.get(entity).is_ok_and(|motion| motion.world_velocity.y <= -crate::als::roll::ROLL_ON_LAND_SPEED_THRESHOLD)
                && motion_roots.get(entity).is_ok_and(|(_, _, action)| action.is_none_or(|action|
                    *action == crate::als::state::AlsNotifyLocomotionAction::None));
        let can_ragdoll = (ragdoll_on_land || (roll_on_land && params.rolling.interrupt_when_in_air))
            && motion_roots.get(entity).is_ok_and(|(_, _, action)| action != Some(&crate::als::state::AlsNotifyLocomotionAction::Ragdolling))
            && ragdoll_eligibility.allows(entity);
        let landing = (landing_braking.factor, ragdoll_on_land && can_ragdoll, roll_on_land && can_ragdoll);
        let grounded = (*stance, *rotation_mode, max_allowed_gait.0, view_yaw.0);
        let root_motion_active = motion_roots.get(entity).is_ok_and(|(roll, mantle, _)|
            roll.is_some_and(crate::als::roll::AlsRollState::root_motion_active)
                || mantle.is_some_and(|state| state.active));
        let delta_seconds = time.delta_secs() * custom_dilations.get(entity).map_or(1.0, |value| value.0);
        *margin = SpeculativeMargin::MAX;
        runtime.grounded_move_start = None;
        if *loco == LocomotionState::Air
            && let Ok((RigidBody::Dynamic, volume, false)) = physics_modes.get(entity)
            && !root_motion_active {
            let mut initial_transform = *transform;
            initial_transform.translation = position.0;
            initial_transform.rotation = world.rotation();
            let initial = velocity.0 + motor.lin.boost;
            let max_speed = runtime.grounded_max_speed;
            let (moved, final_velocity, steps, contact, mode, floor, grounded_max_speed, collision_mode, refreshed) = falling_movement_transaction(&spatial_query, capsule,
                initial_transform, initial, controller.basis.desired_motion, max_speed, params,
                gravity.value + motor.lin.acceleration, grounded, landing, volume.unwrap_or(&default_volume.0).terminal_velocity,
                delta_seconds, 0, entity, &mut runtime.random_seed, &candidates, &step_permissions,
                &bases, &owners, &shapes, &impact_bodies, &mut transitions, cached_input_max, impacts.as_deref_mut());
            if let (Some(refreshed), Some(dynamics)) = (refreshed, dynamics.as_deref_mut()) { dynamics.0 = refreshed; }
            runtime.grounded_max_speed = grounded_max_speed;
            runtime.post_collision_mode = collision_mode.filter(|collision_mode| *collision_mode == mode);
            if *loco != mode { *loco = mode; }
            if mode == LocomotionState::None {
                commands.entity(entity).insert((crate::als::state::AlsNotifyLocomotionAction::Ragdolling,
                    crate::als::dynamics::AlsMovementBase::default(), LinearVelocity::ZERO));
                if let Some(base) = base.as_deref_mut() { *base = default(); }
            }
            runtime.simulation_delta_seconds = delta_seconds;
            runtime.simulation_substeps = steps;
            runtime.grounded_movement_delta = None;
            runtime.grounded_collision_velocity = Some(if mode == LocomotionState::None { Vec3::ZERO } else { final_velocity });
            runtime.airborne_sweep_contact |= contact;
            let world_position = moved.translation;
            transform.translation = parent_world.map_or(world_position, |parent| parent.affine().inverse().transform_point3(world_position));
            commands.entity(entity).insert(Position(moved.translation));
            motor.lin.boost = -velocity.0;
            motor.lin.acceleration = -gravity.value;
            if mode == LocomotionState::None { *motor = default(); }
            *margin = SpeculativeMargin::ZERO;
            drop(transform);
            drop(transform_query);
            if mode == LocomotionState::Grounded && let Ok(mut binding) = movement_bases.get_mut(entity) {
                bind_walking_base(floor, &mut binding, &transforms.p0());
            }
            continue;
        }
        if *loco != LocomotionState::Grounded || blocked.contains(entity)
            || controller.action_flow_status().ongoing() == Some(AlsControlSchemeActionDiscriminant::Jump) {
            continue;
        }
        let half = if *stance == Stance::Crouching { params.crouch.actual_crouched_half_height() } else { params.crouch.standing_capsule_half_height } * scale.y;
        let radius = params.crouch.standing_capsule_radius * scale.x.min(scale.z);
        if root_motion_active { runtime.simulation_delta_seconds = time.delta_secs(); }
        let next_velocity = velocity.0 + motor.lin.boost + motor.lin.acceleration * delta_seconds;
        let delta = runtime.grounded_movement_delta
            .unwrap_or(next_velocity * delta_seconds).with_y(0.0);
        if delta_seconds < crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS {
            runtime.grounded_collision_velocity = Some((velocity.0 + motor.lin.boost).with_y(0.0));
            if delta.length_squared() <= f32::EPSILON {
                motor.lin.boost = -velocity.0;
                motor.lin.acceleration = -gravity.value;
                *margin = SpeculativeMargin::ZERO;
                continue;
            }
        }
        if runtime.simulation_substeps > 1 && runtime.grounded_movement_delta.is_some()
            && let (Some(base), Some(dynamics)) = (base.as_deref_mut(), dynamics.as_deref_mut()) {
            let stationary_delta = runtime.grounded_movement_delta.filter(|delta|
                delta.with_y(0.0).abs().max_element() <= crate::als::movement_constants::SMALL_MOVEMENT_DISTANCE);
            let initial = position.0;
            let mut current_transform = *transform;
            current_transform.translation = initial;
            let max_speed = crate::als::dynamics::max_allowed_speed(params, *stance, *rotation_mode,
                max_allowed_gait.0, base.relative_velocity, view_yaw.0);
            let factor = braking.map_or(crate::als::dynamics::BRAKING_FRICTION_FACTOR, |state| state.factor);
            let support = base.support_velocity.with_y(0.0);
            let initial_base = movement_bases.get(entity).ok().and_then(|binding| binding.entity).or(base.entity);
            let input_dynamics = crate::als::dynamics::with_controlled_input_acceleration(dynamics.0, cached_input_max);
            let (moved, mut final_velocity, iterations, remaining_time, lost_floor, last_input, mut floor, rejected_base) = walking_movement_transaction(
                &spatial_query, capsule, current_transform, half, radius, base.relative_velocity,
                controller.basis.desired_motion, max_speed, input_dynamics, &params.cmc, factor, delta_seconds, entity,
                &candidates, &step_permissions, &bases, &owners, &impact_bodies.pawns, initial_base,
                impacts.as_deref_mut().map(|writer| (writer,
                    controller.basis.desired_motion.clamp_length_max(1.0) * input_dynamics.acceleration, max_speed, &impact_bodies)));
            current_transform = moved;
            if lost_floor || rejected_base {
                *loco = LocomotionState::Air;
                transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                    from: StateValue::LocomotionState(LocomotionState::Grounded),
                    to: StateValue::LocomotionState(LocomotionState::Air) });
                let current_support = crate::als::dynamics::support_velocity_from_base(floor,
                    current_transform.translation - Vec3::Y * half, &bases, &owners).1;
                let leaving_support = if floor == initial_base { support.with_y(current_support.y) } else { current_support };
                if rejected_base {
                    final_velocity = jump_off_velocity(final_velocity, max_speed, params.jump.jump_z_velocity,
                        params.cmc.jump_off_jump_z_factor, &mut runtime.random_seed);
                }
                final_velocity += leaving_support;
                let terminal = physics_modes.get(entity).ok().and_then(|(_, volume, _)| volume)
                    .unwrap_or(&default_volume.0).terminal_velocity;
                let mut falling_transform = current_transform;
                falling_transform.rotation = world.rotation();
                let (moved, falling_velocity, _, contact, mode, landing_base, grounded_max_speed, collision_mode, refreshed) = falling_movement_transaction(&spatial_query,
                    capsule, falling_transform, final_velocity, controller.basis.desired_motion, max_speed,
                    params, gravity.value, grounded, landing, terminal, remaining_time, iterations, entity,
                    &mut runtime.random_seed, &candidates, &step_permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, cached_input_max, impacts.as_deref_mut());
                if let Some(refreshed) = refreshed { dynamics.0 = refreshed; }
                runtime.grounded_max_speed = grounded_max_speed;
                runtime.post_collision_mode = if rejected_base && mode == LocomotionState::Air { Some(mode) }
                    else { collision_mode.filter(|collision_mode| *collision_mode == mode) };
                floor = landing_base;
                if *loco != mode { *loco = mode; }
                if mode == LocomotionState::None {
                    commands.entity(entity).insert((crate::als::state::AlsNotifyLocomotionAction::Ragdolling,
                        crate::als::dynamics::AlsMovementBase::default(), LinearVelocity::ZERO));
                    *base = default();
                }
                current_transform.translation = moved.translation;
                final_velocity = falling_velocity;
                runtime.airborne_sweep_contact |= contact;
            }
            runtime.grounded_movement_delta = stationary_delta;
            runtime.grounded_collision_velocity = Some(if *loco == LocomotionState::None { Vec3::ZERO }
                else if lost_floor || rejected_base { final_velocity } else { final_velocity + support });
            runtime.grounded_move_start = None;
            if (lost_floor || rejected_base) && *loco == LocomotionState::Air { base.entity = None; }
            base.pre_collision_velocity = Some(last_input);
            let world_position = current_transform.translation;
            *transform = current_transform;
            transform.translation = parent_world.map_or(world_position, |parent| parent.affine().inverse().transform_point3(world_position));
            commands.entity(entity).insert(Position(world_position));
            motor.lin.boost.x = support.x - velocity.x;
            motor.lin.boost.z = support.z - velocity.z;
            motor.lin.acceleration.x = 0.0;
            motor.lin.acceleration.z = 0.0;
            motor.lin.boost.y = -velocity.y;
            motor.lin.acceleration.y = -gravity.value.y;
            if *loco == LocomotionState::None { *motor = default(); }
            *margin = SpeculativeMargin::ZERO;
            drop(transform);
            drop(transform_query);
            if *loco == LocomotionState::Grounded && let Ok(mut binding) = movement_bases.get_mut(entity) {
                bind_walking_base(floor, &mut binding, &transforms.p0());
            }
            continue;
        }
        let center = position.0;
        let impact_velocity = base.as_deref().and_then(|state| state.pre_collision_velocity)
            .unwrap_or(next_velocity.with_y(0.0));
        let impact_acceleration = controller.basis.desired_motion.clamp_length_max(1.0)
            * cached_input_max.unwrap_or_else(|| dynamics.as_deref().map_or(params.acceleration.max_acceleration, |value| value.0.acceleration));
        let Some((advanced, remaining, next_y, penetrating, stepped_up, swept_movement)) = grounded_movement_transaction(&spatial_query, capsule,
            center, transform.rotation, half, radius, delta, &params.cmc, *loco, entity, &candidates, &step_permissions,
            impacts.as_deref_mut().map(|writer| (writer, impact_acceleration, impact_velocity, runtime.grounded_max_speed, &impact_bodies))) else { continue };
        runtime.grounded_move_start = (!penetrating).then_some(center);
        if penetrating { runtime.grounded_collision_velocity = Some(next_velocity.with_y(0.0)); }
        let consumed = stepped_up || swept_movement;
        if consumed { runtime.grounded_movement_delta = None; }
        let advanced = if consumed { advanced + remaining.unwrap_or(delta) } else { advanced };
        let world_position = next_y.filter(|height| stepped_up || *height > center.y).map_or(advanced, |height| advanced.with_y(height));
        transform.translation = parent_world.map_or(world_position, |parent| parent.affine().inverse().transform_point3(world_position));
        commands.entity(entity).insert(Position(world_position));
        let planned_velocity = if !consumed && !penetrating && remaining.is_none() && next_y.is_some()
            && runtime.simulation_substeps == 1 && runtime.grounded_movement_delta.is_some()
            && delta_seconds == time.delta_secs() {
            base.as_deref().filter(|base| base.support_velocity == Vec3::ZERO)
                .and_then(|base| base.pre_collision_velocity)
        } else { None };
        let remaining_velocity = if consumed {
            base.as_deref().filter(|base| base.entity.is_some())
                .map_or(Vec3::ZERO, |base| base.support_velocity.with_y(0.0))
        } else { planned_velocity.unwrap_or_else(|| remaining.unwrap_or(delta) / time.delta_secs()) };
        motor.lin.boost.x = remaining_velocity.x - velocity.x;
        motor.lin.boost.z = remaining_velocity.z - velocity.z;
        motor.lin.acceleration.x = 0.0;
        motor.lin.acceleration.z = 0.0;
        let Some(next_y) = next_y else {
            let destination = if consumed { advanced } else { advanced + remaining.unwrap_or(delta) };
            if find_ground_floor(&spatial_query, destination, half, radius, &params.cmc, *loco,
                &SpatialQueryFilter::from_excluded_entities([entity]), entity, &candidates).is_none() {
                motor.lin.acceleration.y = -gravity.value.y;
                if !root_motion_active
                    && physics_modes.get(entity).is_ok_and(|(body, _, custom)| *body == RigidBody::Dynamic && !custom)
                    && !walking_floor_penetrates(&spatial_query, destination, half, radius, entity, &candidates) {
                    let remaining_time = crate::als::dynamics::walking_fall_remaining(0.0, delta_seconds,
                        delta.length(), destination - center);
                    let support = base.as_deref().filter(|base| base.entity.is_some())
                        .map_or(Vec3::ZERO, |base| {
                            let current = crate::als::dynamics::support_velocity_from_base(base.entity,
                                destination - Vec3::Y * half, &bases, &owners).1;
                            base.support_velocity.with_y(current.y)
                        });
                    let initial_velocity = base.as_deref().and_then(|base| base.pre_collision_velocity)
                        .map_or(next_velocity.with_y(0.0) + Vec3::Y * support.y,
                            |relative| relative.with_y(0.0) + support);
                    let terminal = physics_modes.get(entity).ok().and_then(|(_, volume, _)| volume)
                        .unwrap_or(&default_volume.0).terminal_velocity;
                    let max_speed = runtime.grounded_max_speed;
                    let mut falling_start = *transform;
                    falling_start.translation = destination;
                    falling_start.rotation = world.rotation();
                    *loco = LocomotionState::Air;
                    transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                        from: StateValue::LocomotionState(LocomotionState::Grounded),
                        to: StateValue::LocomotionState(LocomotionState::Air) });
                    let (moved, falling_velocity, _, contact, mode, floor, grounded_max_speed, collision_mode, refreshed) =
                        falling_movement_transaction(&spatial_query, capsule, falling_start, initial_velocity,
                            controller.basis.desired_motion, max_speed, params, gravity.value,
                            grounded, landing, terminal, remaining_time, 1, entity, &mut runtime.random_seed,
                            &candidates, &step_permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, cached_input_max, impacts.as_deref_mut());
                    if let (Some(refreshed), Some(dynamics)) = (refreshed, dynamics.as_deref_mut()) { dynamics.0 = refreshed; }
                    runtime.grounded_max_speed = grounded_max_speed;
                    runtime.post_collision_mode = collision_mode.filter(|collision_mode| *collision_mode == mode);
                    runtime.grounded_movement_delta = None;
                    runtime.grounded_move_start = None;
                    runtime.grounded_collision_velocity = Some(if mode == LocomotionState::None { Vec3::ZERO } else { falling_velocity });
                    runtime.airborne_sweep_contact |= contact;
                    *loco = mode;
                    if mode == LocomotionState::Air && let Some(base) = base.as_deref_mut() { base.entity = None; }
                    if mode == LocomotionState::None {
                        commands.entity(entity).insert((crate::als::state::AlsNotifyLocomotionAction::Ragdolling,
                            crate::als::dynamics::AlsMovementBase::default(), LinearVelocity::ZERO));
                        if let Some(base) = base.as_deref_mut() { *base = default(); }
                    }
                    let world_position = moved.translation;
                    transform.translation = parent_world.map_or(world_position, |parent| parent.affine().inverse().transform_point3(world_position));
                    commands.entity(entity).insert(Position(world_position));
                    motor.lin.boost = -velocity.0;
                    motor.lin.acceleration = -gravity.value;
                    if mode == LocomotionState::None { *motor = default(); }
                    *margin = SpeculativeMargin::ZERO;
                    drop(transform);
                    drop(transform_query);
                    if mode == LocomotionState::Grounded && let Ok(mut binding) = movement_bases.get_mut(entity) {
                        bind_walking_base(floor, &mut binding, &transforms.p0());
                    }
                }
            }
            continue;
        };
        motor.lin.boost.y = -velocity.y;
        motor.lin.acceleration.y = -gravity.value.y;
        *margin = SpeculativeMargin::ZERO;
        let destination = if stepped_up { advanced.with_y(next_y) }
            else { (advanced + remaining.unwrap_or(delta)).with_y(next_y) };
        let floor = find_ground_floor(&spatial_query, destination, half, radius, &params.cmc, *loco,
            &SpatialQueryFilter::from_excluded_entities([entity]), entity, &candidates).map(|(_, _, floor)| floor);
        let previous = movement_bases.get(entity).ok().and_then(|binding| binding.entity)
            .or_else(|| base.as_deref().and_then(|state| state.entity));
        if floor.is_some_and(|floor| movement_base_changed(previous, floor, &owners)
            && pawn_rejects_character_base(floor, &impact_bodies.pawns, &step_permissions, &owners)) {
            let support = base.as_deref().filter(|state| state.entity.is_some())
                .map_or(Vec3::ZERO, |state| state.support_velocity.with_y(0.0));
            let walking_velocity = base.as_deref().and_then(|state| state.pre_collision_velocity)
                .unwrap_or(next_velocity.with_y(0.0) - support);
            let max_speed = crate::als::dynamics::max_allowed_speed(params, *stance, *rotation_mode,
                max_allowed_gait.0, walking_velocity, view_yaw.0);
            let jump_velocity = jump_off_velocity(walking_velocity, max_speed, params.jump.jump_z_velocity,
                params.cmc.jump_off_jump_z_factor, &mut runtime.random_seed);
            runtime.grounded_collision_velocity = Some(jump_velocity);
            runtime.grounded_movement_delta = None;
            runtime.grounded_move_start = None;
            runtime.post_collision_mode = Some(LocomotionState::Air);
            *loco = LocomotionState::Air;
            transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                from: StateValue::LocomotionState(LocomotionState::Grounded),
                to: StateValue::LocomotionState(LocomotionState::Air) });
            if let Some(base) = base.as_deref_mut() { base.entity = None; }
            if let Ok(mut binding) = movement_bases.get_mut(entity) { *binding = default(); }
            transform.translation = parent_world.map_or(destination, |parent| parent.affine().inverse().transform_point3(destination));
            commands.entity(entity).insert(Position(destination));
            motor.lin.boost = -velocity.0;
            motor.lin.acceleration = -gravity.value;
            continue;
        }
        drop(transform);
        drop(transform_query);
        if let Ok(mut binding) = movement_bases.get_mut(entity) {
            bind_walking_base(floor, &mut binding, &transforms.p0());
        }
    }
}

pub fn character_blocks_collider(character: Entity, candidate: Entity,
    candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>) -> bool {
    let layers = candidates.get(character).ok().map(|(layers, _, _, ownership)| {
        ownership.filter(|owner| owner.active()).map(|owner| owner.original_layers())
            .or(layers.copied()).unwrap_or_default()
    }).unwrap_or_default();
    candidate != character && candidates.get(candidate).is_ok_and(|(candidate_layers, sensor, owner, _)| {
        !sensor && !owner.is_some_and(|owner| owner.body == character)
            && layers.interacts_with(candidate_layers.copied().unwrap_or_default())
    })
}

fn floor_sweep_hit(spatial: &SpatialQuery, capsule: &Collider, radius: f32, origin: Vec3,
    distance: f32, filter: &SpatialQueryFilter, accepts: &dyn Fn(Entity) -> bool) -> Option<(ShapeHitData, Vec3)> {
    let hit = spatial.cast_shape_predicate(capsule, origin, Quat::IDENTITY, Dir3::NEG_Y,
        &ShapeCastConfig::from_max_distance(distance), filter, accepts)?;
    let reduced_radius = (radius - EDGE_REJECT_DISTANCE).max(SWEEP_EDGE_MIN_RADIUS);
    if Vec2::new(hit.point1.x - origin.x, hit.point1.z - origin.z).length_squared() >= reduced_radius * reduced_radius {
        return None;
    }
    let impact = source_sweep_impact_normal(spatial, &hit, Vec3::NEG_Y * distance, origin, Quat::IDENTITY, capsule);
    Some((hit, impact))
}

fn walking_floor_penetrates(spatial: &SpatialQuery, origin: Vec3, half: f32, radius: f32, character: Entity,
    candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>) -> bool {
    let filter = SpatialQueryFilter::from_excluded_entities([character]);
    let accepts = |candidate| character_blocks_collider(character, candidate, candidates);
    let distance = MAX_FLOOR_DISTANCE.max(MAX_STEP_HEIGHT + MAX_FLOOR_DISTANCE
        + crate::als::movement_constants::SMALL_MOVEMENT_DISTANCE);
    let mut penetrating = false;
    for (shrink, reduction) in [(FLOOR_SWEEP_SHRINK_FRACTION, 0.0),
        (FLOOR_RETRY_SHRINK_FRACTION, SWEEP_EDGE_MIN_RADIUS)] {
        let shrink_height = (half - radius) * shrink;
        let sweep_radius = (radius - reduction).max(SWEEP_EDGE_MIN_RADIUS);
        let capsule = als_character_capsule((half - shrink_height).max(sweep_radius), sweep_radius);
        if let Some((hit, _)) = floor_sweep_hit(spatial, &capsule, sweep_radius, origin,
            distance + shrink_height, &filter, &accepts) {
            penetrating = hit.distance == 0.0 && (hit.point1 - hit.point2).dot(hit.normal1) > 0.0;
            if hit.distance != 0.0 { break; }
        }
    }
    penetrating
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GroundFloorOrigin {
    Sweep,
    Line,
    PerchLine,
}

#[derive(Clone, Copy, Debug)]
struct GroundFloorTrace {
    origin: Vec3,
    distance: f32,
}

#[derive(Clone, Copy, Debug)]
struct GroundFloorResult {
    center_y: f32,
    hit: ShapeHitData,
    impact_normal: Vec3,
    penetration: Option<ShapeHitData>,
    sweep_distance: f32,
    line_distance: Option<f32>,
    origin: GroundFloorOrigin,
    trace: GroundFloorTrace,
}

fn line_floor_blocks_height_adjustment(sweep_distance: f32, line_distance: Option<f32>) -> bool {
    line_distance.is_some_and(|line| sweep_distance < MIN_FLOOR_DISTANCE && line >= MIN_FLOOR_DISTANCE)
}

fn source_character_impact_force(impact: &SourceCharacterImpact, body_velocity: Vec3) -> Vec3 {
    use crate::als::movement_constants::{INITIAL_PUSH_FORCE, INITIAL_PUSH_FORCE_RATE, MOVING_PUSH_FORCE,
        PUSH_FORCE_NEAR_ZERO_VELOCITY, PUSH_FORCE_ZERO_VELOCITY_TOLERANCE, SCALE_PUSH_FORCE_TO_VELOCITY};
    let mut modifier = 1.0;
    if SCALE_PUSH_FORCE_TO_VELOCITY && body_velocity.abs().max_element() > PUSH_FORCE_NEAR_ZERO_VELOCITY {
        let virtual_velocity = if impact.acceleration == Vec3::ZERO { impact.velocity }
            else { impact.acceleration.normalize_or_zero() * impact.max_speed };
        let dot = body_velocity.dot(virtual_velocity) / crate::als::params::UE_CM_TO_M.powi(2);
        if dot > 0.0 && dot < 1.0 { modifier = dot; }
    }
    let magnitude = if body_velocity.abs().max_element() <= PUSH_FORCE_ZERO_VELOCITY_TOLERANCE {
        INITIAL_PUSH_FORCE * INITIAL_PUSH_FORCE_RATE
    } else { MOVING_PUSH_FORCE };
    -impact.normal * (magnitude * modifier)
}

fn source_touch_impulse(character: Vec3, other: Vec3, velocity: Vec3, mass: f32) -> Vec3 {
    use crate::als::movement_constants::{MAX_TOUCH_IMPULSE, MIN_TOUCH_IMPULSE, TOUCH_DIRECTION_VERTICAL_OFFSET,
        TOUCH_FORCE_FACTOR, TOUCH_FORCE_SCALED_TO_MASS};
    let horizontal = velocity.with_y(0.0);
    let direction = ((other - character).with_y(TOUCH_DIRECTION_VERTICAL_OFFSET).normalize_or_zero()
        + horizontal.normalize_or_zero()).normalize_or_zero();
    let mut strength = horizontal.length() * TOUCH_FORCE_FACTOR * if TOUCH_FORCE_SCALED_TO_MASS { mass } else { 1.0 };
    if MIN_TOUCH_IMPULSE > 0.0 { strength = strength.max(MIN_TOUCH_IMPULSE); }
    if MAX_TOUCH_IMPULSE > 0.0 { strength = strength.min(MAX_TOUCH_IMPULSE); }
    direction * strength
}

fn source_repulsion_response(
    capsule: &Collider, center: Vec3, rotation: Quat, half: f32, radius: f32,
    body_position: Vec3, body_com: Vec3, velocity: Vec3, delta_seconds: f32,
) -> (bool, Vec3) {
    use crate::als::movement_constants::{CHARACTER_MOVEMENT_MASS, REPULSION_ACCELERATION,
        REPULSION_COINCIDENT_DISTANCE, REPULSION_RADIUS_SCALE, REPULSION_STOP_DISTANCE_SQUARED};
    let toward_center = (center - body_position).with_y(0.0);
    let hit = Dir3::new_and_length(toward_center).ok().and_then(|(direction, distance)| {
        capsule.cast_ray(center, rotation, body_position, *direction, distance, true)
            .map(|(travel, _)| (body_position + *direction * travel, travel == 0.0))
    });
    let (hit_location, penetrating) = hit.unwrap_or((body_position, true));
    let distance_now = (hit_location - body_position).with_y(0.0).length_squared();
    let distance_later = (hit_location - (body_position + velocity * delta_seconds)).with_y(0.0).length_squared();
    if hit.is_some() && distance_now < REPULSION_STOP_DISTANCE_SQUARED && !penetrating {
        return (true, Vec3::ZERO);
    }
    if distance_later > distance_now && !penetrating { return (false, Vec3::ZERO); }
    let force_center = center.with_y(if hit.is_some() { hit_location.y }
        else { body_position.y.clamp(center.y - half, center.y + half) });
    let offset = body_com - force_center;
    let distance = offset.length();
    if distance > radius * REPULSION_RADIUS_SCALE { return (false, Vec3::ZERO); }
    let direction = if distance < REPULSION_COINCIDENT_DISTANCE { Vec3::X } else { offset / distance };
    (false, direction * (REPULSION_ACCELERATION * CHARACTER_MOVEMENT_MASS))
}

fn apply_character_repulsion(
    time: Res<Time<Virtual>>,
    collisions: Collisions,
    sensors: Query<Has<Sensor>>,
    transforms: bevy::transform::helper::TransformHelper,
    params_assets: Res<Assets<AlsMovementParams>>,
    characters: Query<(&Position, &Rotation, &Collider, &Transform, Option<&ChildOf>, &Stance,
        &AlsMovementParamsHandle, Option<&crate::als_camera::AlsCameraCustomTimeDilation>), With<AlsCharacter>>,
    mut bodies: ParamSet<(
        Query<(&RigidBody, &Position, &Rotation, &ComputedCenterOfMass, &mut LinearVelocity, Has<RigidBodyDisabled>)>,
        Query<Forces>,
    )>,
) {
    use crate::als::movement_constants::{CHARACTER_GENERATES_OVERLAP_EVENTS, ENABLE_PHYSICS_INTERACTION, REPULSION_ACCELERATION};
    if !CHARACTER_GENERATES_OVERLAP_EVENTS || !ENABLE_PHYSICS_INTERACTION || REPULSION_ACCELERATION <= 0.0 { return; }
    for pair in collisions.iter() {
        for (actor, other, body) in [(pair.collider1, pair.collider2, pair.body2), (pair.collider2, pair.collider1, pair.body1)] {
            let Ok((position, rotation, capsule, local, parent, stance, handle, dilation)) = characters.get(actor) else { continue };
            if !sensors.get(actor).unwrap_or(false) && !sensors.get(other).unwrap_or(false) { continue; }
            let body = body.unwrap_or(other);
            if body == actor { continue; }
            let Some(params) = params_assets.get(handle.id()) else { continue };
            let Ok(world) = transforms.compute_global_transform(actor) else { continue };
            let scale = character_floor_scale(local, parent.is_some(), &world);
            let half = if *stance == Stance::Crouching { params.crouch.actual_crouched_half_height() }
                else { params.crouch.standing_capsule_half_height } * scale.y;
            let radius = params.crouch.standing_capsule_radius * scale.x.min(scale.z);
            let force = {
                let mut velocities = bodies.p0();
                let Ok((RigidBody::Dynamic, body_position, body_rotation, center_of_mass, mut velocity, false)) = velocities.get_mut(body) else { continue };
                let com = body_position.0 + body_rotation.0 * center_of_mass.0;
                let (stop, force) = source_repulsion_response(capsule, position.0, rotation.0, half, radius,
                    body_position.0, com, velocity.0, time.delta_secs() * dilation.map_or(1.0, |value| value.0));
                if stop { velocity.0 = Vec3::ZERO; }
                force
            };
            if force != Vec3::ZERO && let Ok(mut body_forces) = bodies.p1().get_mut(body) {
                body_forces.apply_force(force);
            }
        }
    }
}

fn apply_character_overlap_impulses(
    mut starts: MessageReader<CollisionStart>,
    colliders: Query<(&Position, Has<Sensor>)>,
    mut access: ParamSet<(
        Query<(&Position, &LinearVelocity), With<AlsCharacter>>,
        Query<(&RigidBody, &ComputedMass, Forces, Has<RigidBodyDisabled>)>,
    )>,
) {
    use crate::als::movement_constants::{CHARACTER_GENERATES_OVERLAP_EVENTS, ENABLE_PHYSICS_INTERACTION};
    if !CHARACTER_GENERATES_OVERLAP_EVENTS || !ENABLE_PHYSICS_INTERACTION { return; }
    for start in starts.read() {
        for (actor, other, body) in [(start.collider1, start.collider2, start.body2),
            (start.collider2, start.collider1, start.body1)] {
            let actor_motion = {
                let characters = access.p0();
                characters.get(actor).ok().map(|(position, velocity)| (position.0, velocity.0))
            };
            let Some((actor_position, velocity)) = actor_motion else { continue };
            let Ok((other_position, other_sensor)) = colliders.get(other) else { continue };
            let actor_sensor = colliders.get(actor).is_ok_and(|(_, sensor)| sensor);
            if !other_sensor && !actor_sensor { continue; }
            let body = body.unwrap_or(other);
            if body == actor { continue; }
            let mut bodies = access.p1();
            if let Ok((RigidBody::Dynamic, mass, mut forces, false)) = bodies.get_mut(body) {
                forces.apply_linear_impulse(source_touch_impulse(actor_position, other_position.0, velocity, mass.value()));
            }
        }
    }
}

fn apply_character_impact_forces(
    mut impacts: MessageReader<SourceCharacterImpact>,
    owners: Query<&ColliderOf>,
    mut bodies: Query<(&RigidBody, Forces, Has<RigidBodyDisabled>)>,
) {
    if !crate::als::movement_constants::ENABLE_PHYSICS_INTERACTION { return; }
    for impact in impacts.read() {
        let body = owners.get(impact.collider).map_or(impact.collider, |owner| owner.body);
        if let Ok((RigidBody::Dynamic, mut forces, false)) = bodies.get_mut(body) {
            let force = source_character_impact_force(impact, forces.linear_velocity());
            forces.apply_force_at_point(force, impact.point);
        }
    }
}

fn apply_standing_downward_force(
    time: Res<Time<Virtual>>,
    spatial: SpatialQuery,
    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    owners: Query<&ColliderOf>,
    transforms: bevy::transform::helper::TransformHelper,
    params_assets: Res<Assets<AlsMovementParams>>,
    characters: Query<(Entity, &Position, &Transform, Option<&ChildOf>, &Stance,
        &AlsMovementParamsHandle, &LocomotionState, &AlsEffectiveGravity), With<AlsCharacter>>,
    mut bodies: Query<(&RigidBody, Forces, Has<RigidBodyDisabled>)>,
) {
    use crate::als::movement_constants::{CHARACTER_MOVEMENT_MASS, ENABLE_PHYSICS_INTERACTION, STANDING_DOWNWARD_FORCE_SCALE};
    if time.is_paused() || !ENABLE_PHYSICS_INTERACTION || STANDING_DOWNWARD_FORCE_SCALE == 0.0 { return; }
    for (entity, position, local, parent, stance, handle, mode, gravity) in &characters {
        if *mode != LocomotionState::Grounded { continue; }
        let force = gravity.global * gravity.gravity_scale * (CHARACTER_MOVEMENT_MASS * STANDING_DOWNWARD_FORCE_SCALE);
        if force == Vec3::ZERO { continue; }
        let Some(params) = params_assets.get(handle.id()) else { continue };
        let Ok(world) = transforms.compute_global_transform(entity) else { continue };
        let scale = character_floor_scale(local, parent.is_some(), &world);
        let half = if *stance == Stance::Crouching { params.crouch.actual_crouched_half_height() }
            else { params.crouch.standing_capsule_half_height } * scale.y;
        let radius = params.crouch.standing_capsule_radius * scale.x.min(scale.z);
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let Some(floor) = find_ground_floor_with_penetration(&spatial, position.0, half, radius,
            &params.cmc, *mode, &filter, entity, &candidates, None, None) else { continue };
        let body = owners.get(floor.hit.entity).map_or(floor.hit.entity, |owner| owner.body);
        if let Ok((RigidBody::Dynamic, mut forces, false)) = bodies.get_mut(body) {
            forces.apply_force_at_point(force, floor.hit.point1);
        }
    }
}

fn resolve_stationary_floor_state(
    time: Res<Time<Virtual>>,
    mut commands: Commands,
    mut transitions: MessageWriter<StateTransitionEvent>,
    spatial: SpatialQuery,
    impact_bodies: FallingImpactBodies,
    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    transforms: bevy::transform::helper::TransformHelper,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut characters: Query<(Entity, &Position, &Transform, Option<&ChildOf>, &Collider, &mut LocomotionState,
        &Stance, &AlsMovementParamsHandle, &LinearVelocity, &AlsEffectiveGravity,
        &mut TnuaMotor, &mut SpeculativeMargin, &mut AlsCmcAdapterRuntime,
        (Option<&mut crate::als::dynamics::AlsMovementBase>, Option<&mut crate::als::dynamics::AlsMovementBaseDynamicsState>),
        Option<&crate::als_camera::AlsCameraCustomTimeDilation>), With<AlsCharacter>>,
) {
    if time.is_paused() { return; }
    for (entity, position, local, parent, capsule, mut mode, stance, handle, velocity, gravity,
        mut motor, mut margin, mut runtime, (base, dynamics), dilation) in &mut characters {
        if *mode != LocomotionState::Grounded
            || time.delta_secs() * dilation.map_or(1.0, |value| value.0) < crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS
            || !runtime.grounded_movement_delta.is_some_and(|delta|
                delta.with_y(0.0).abs().max_element() <= crate::als::movement_constants::SMALL_MOVEMENT_DISTANCE) { continue; }
        let Some(params) = params_assets.get(handle.id()) else { continue };
        let Ok(world) = transforms.compute_global_transform(entity) else { continue };
        let Ok(parent_world) = parent.map(|parent| transforms.compute_global_transform(parent.parent())).transpose() else { continue };
        let scale = character_floor_scale(local, parent.is_some(), &world);
        let half = if *stance == Stance::Crouching { params.crouch.actual_crouched_half_height() }
            else { params.crouch.standing_capsule_half_height } * scale.y;
        let radius = params.crouch.standing_capsule_radius * scale.x.min(scale.z);
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let mut rejected = None;
        let floor = find_ground_floor_with_penetration(&spatial, position.0, half, radius, &params.cmc,
            *mode, &filter, entity, &candidates, None, Some(&mut rejected));
        if floor.is_some() { continue; }
        if rejected.is_none() && dynamics.as_deref().is_some_and(|state|
            state.entity.is_some() && state.support_velocity != Vec3::ZERO) { continue; }
        if let Some(hit) = rejected {
            let accepts = |candidate| character_blocks_collider(entity, candidate, &candidates);
            if let Some(adjustment) = falling_resolve_penetration(&spatial, capsule, position.0, world.rotation(),
                &hit, Vec3::Y * MAX_FLOOR_DISTANCE, &filter, &accepts, &impact_bodies) {
                let world_position = position.0 + adjustment;
                let mut next = *local;
                next.translation = parent_world.map_or(world_position, |parent| parent.affine().inverse().transform_point3(world_position));
                commands.entity(entity).insert((Position(world_position), next));
            }
        } else {
            *mode = LocomotionState::Air;
            transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                from: StateValue::LocomotionState(LocomotionState::Grounded),
                to: StateValue::LocomotionState(LocomotionState::Air) });
            if let Some(mut base) = base { *base = default(); }
            if let Some(mut dynamics) = dynamics { *dynamics = default(); }
        }
        runtime.grounded_collision_velocity = Some(velocity.0.with_y(0.0));
        runtime.grounded_move_start = None;
        runtime.post_collision_mode = Some(*mode);
        motor.lin.boost.y = -velocity.y;
        motor.lin.acceleration.y = -gravity.value.y;
        *margin = SpeculativeMargin::ZERO;
    }
}

pub fn find_ground_floor(spatial: &SpatialQuery, origin: Vec3, half: f32, radius: f32,
    cmc: &crate::als::params::CmcAdapterParams, mode: LocomotionState,
    filter: &SpatialQueryFilter, character: Entity,
    candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>) -> Option<(f32, f32, Entity)> {
    find_ground_floor_with_penetration(spatial, origin, half, radius, cmc, mode, filter, character, candidates, None, None)
        .map(|floor| (floor.center_y, floor.hit.point1.y, floor.hit.entity))
}

fn find_ground_floor_with_penetration(spatial: &SpatialQuery, origin: Vec3, half: f32, radius: f32,
    cmc: &crate::als::params::CmcAdapterParams, mode: LocomotionState,
    filter: &SpatialQueryFilter, character: Entity,
    candidates: &Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    supplied: Option<(ShapeHitData, GroundFloorTrace)>,
    mut rejected_penetration: Option<&mut Option<ShapeHitData>>)
    -> Option<GroundFloorResult> {
    let accepts = |candidate| character_blocks_collider(character, candidate, candidates);
    let walkable = |hit: &ShapeHitData, impact_normal: Vec3| impact_normal.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos()
        && !(hit.distance == 0.0 && (hit.point1 - hit.point2).dot(hit.normal1) > 0.0);
    let mut compute = |at: Vec3, test_radius: f32, line_distance: f32, sweep_distance: f32| {
        let mut result = None;
        for (shrink_scale, radius_reduction) in [(FLOOR_SWEEP_SHRINK_FRACTION, 0.0),
            (FLOOR_RETRY_SHRINK_FRACTION, SWEEP_EDGE_MIN_RADIUS)] {
            let shrink_height = (half - radius) * shrink_scale;
            let sweep_radius = (test_radius - radius_reduction).max(SWEEP_EDGE_MIN_RADIUS);
            let capsule_half = (half - shrink_height).max(sweep_radius);
            let capsule = als_character_capsule(capsule_half, sweep_radius);
            let trace = GroundFloorTrace { origin: at, distance: sweep_distance + shrink_height };
            if let Some((hit, impact_normal)) = floor_sweep_hit(spatial, &capsule, sweep_radius, at,
                sweep_distance + shrink_height, filter, &accepts) {
                let floor_distance = (hit.distance - shrink_height).max(-MAX_FLOOR_DISTANCE.max(radius));
                let valid = walkable(&hit, impact_normal) && floor_distance <= sweep_distance;
                if !valid && hit.distance == 0.0 && (hit.point1 - hit.point2).dot(hit.normal1) > 0.0
                    && let Some(rejected) = rejected_penetration.as_deref_mut() { *rejected = Some(hit); }
                result = Some((hit, floor_distance, valid, false, None, floor_distance, trace, impact_normal));
                if valid { return result; }
                if hit.distance != 0.0 { break; }
            }
        }
        if result.is_some() && line_distance > 0.0
            && let Some(ray) = spatial.cast_ray_predicate(at, Dir3::NEG_Y,
                line_distance + half, true, filter, &accepts)
            && ray.distance > 0.0 && ray.normal.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos() {
            let floor_distance = (ray.distance - half).max(-MAX_FLOOR_DISTANCE.max(radius));
            if floor_distance <= line_distance {
                let (sweep, sweep_floor_distance, _, _, _, _, trace, _) = result.unwrap();
                let retained = (sweep.distance == 0.0 && (sweep.point1 - sweep.point2).dot(sweep.normal1) > 0.0).then_some(sweep);
                let hit = ShapeHitData { entity: ray.entity, normal1: ray.normal, normal2: -ray.normal, ..sweep };
                return Some((hit, floor_distance, true, true, retained, sweep_floor_distance, trace, ray.normal));
            }
        }
        result
    };
    let height_adjust = if mode == LocomotionState::Grounded {
        MAX_FLOOR_DISTANCE + crate::als::movement_constants::SMALL_MOVEMENT_DISTANCE
    } else { -MAX_FLOOR_DISTANCE };
    let max_floor = MAX_FLOOR_DISTANCE.max(MAX_STEP_HEIGHT + height_adjust);
    let supplied = supplied.and_then(|(hit, trace)| {
        let location = trace.origin - Vec3::Y * hit.distance;
        let offset = hit.point1 - location;
        let reduced = (radius - EDGE_REJECT_DISTANCE).max(SWEEP_EDGE_MIN_RADIUS);
        if trace.distance > 0.0 && walkable(&hit, hit.normal1)
            && Vec2::new(offset.x, offset.z).length_squared() < reduced * reduced {
            let distance = origin.y - location.y;
            Some((hit, distance, true, false, None, distance, trace, hit.normal1))
        } else { None }
    });
    let (mut hit, mut floor_distance, valid, line, retained_penetration, sweep_floor_distance, trace, mut impact_normal) =
        supplied.or_else(|| compute(origin, radius, max_floor, max_floor))?;
    let mut floor_origin = if line { GroundFloorOrigin::Line } else { GroundFloorOrigin::Sweep };
    let threshold = cmc.perch_radius_threshold.max(0.0);
    let perch_radius = (radius - threshold).max(crate::als::movement_constants::MIN_PERCH_RADIUS).min(radius);
    let edge_distance = Vec2::new(hit.point1.x - origin.x, hit.point1.z - origin.z).length_squared();
    if !line && threshold > EDGE_REJECT_DISTANCE && edge_distance > perch_radius * perch_radius
        && !(hit.distance == 0.0 && (hit.point1 - hit.point2).dot(hit.normal1) > 0.0) {
        let at = origin - Vec3::Y * hit.distance;
        let above_base = (hit.point1.y - at.y + half).max(0.0);
        let max_perch = max_floor + if mode == LocomotionState::Grounded { cmc.perch_additional_height.max(0.0) } else { 0.0 };
        let (perch, perch_distance, perch_valid, _, _, _, _, perch_impact) = compute(at, perch_radius,
            (max_perch - above_base).max(0.0), max_perch + radius)?;
        if !perch_valid || above_base + perch_distance > max_perch { return None; }
        let average = (MIN_FLOOR_DISTANCE + MAX_FLOOR_DISTANCE) * 0.5;
        if average - floor_distance + perch_distance >= max_perch { floor_distance = average; }
        if !valid {
            hit = ShapeHitData { entity: perch.entity, normal1: perch.normal1, normal2: perch.normal2, ..hit };
            impact_normal = perch_impact;
            floor_origin = GroundFloorOrigin::PerchLine;
        }
    } else if !valid { return None; }
    let source_floor_distance = if line { sweep_floor_distance } else { floor_distance };
    let line_distance = match floor_origin {
        GroundFloorOrigin::Sweep => None,
        GroundFloorOrigin::Line => Some(floor_distance),
        GroundFloorOrigin::PerchLine => Some(floor_distance.max(MIN_FLOOR_DISTANCE)),
    };
    Some(GroundFloorResult { center_y: origin.y - line_distance.unwrap_or(floor_distance), hit, impact_normal,
        penetration: retained_penetration, sweep_distance: source_floor_distance, line_distance, origin: floor_origin, trace })
}

fn resolved_walkable_contact(contact: &ContactPair, character: Entity) -> bool {
    contact.flags.contains(ContactPairFlags::GENERATE_CONSTRAINTS)
        && contact.manifolds.iter().any(|manifold| {
            let normal = if contact.collider1 == character { -manifold.normal } else { manifold.normal };
            normal.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos()
                && manifold.points.iter().any(|point| point.normal_speed < 0.0 && point.normal_impulse > 0.0)
        })
}

pub(crate) fn locomotion_state_sync_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    custom_dilations: Query<&crate::als_camera::AlsCameraCustomTimeDilation>,
    motion: Query<&crate::actual_motion::AlsActualMotion, With<AlsCharacter>>,
    spatial: SpatialQuery,
    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    params_assets: Res<Assets<AlsMovementParams>>,
    blocked: Query<(), With<AlsMovementInputBlocked>>,
    pawns: Query<(), With<AlsCharacter>>,
    permissions: Query<&AlsMantleTargetPermission>,
    owners: Query<&ColliderOf>,
    (collisions, ragdoll_eligibility): (Collisions, crate::ragdoll_link::AlsRagdollStartEligibility),
    mut transitions: MessageWriter<StateTransitionEvent>,
    transforms: bevy::transform::helper::TransformHelper,
    mut movement_bases: Query<(&mut crate::als::dynamics::AlsMovementBase, Option<&mut crate::als::dynamics::AlsMovementBaseDynamicsState>), With<AlsCharacter>>,
    mut q: Query<
        (
            Entity,
            &Transform,
            &Position,
            &Collider,
            &mut LinearVelocity,
            &TnuaController<AlsControlScheme>,
            &mut LocomotionState,
            &AlsStateRequest,
            Has<AlsRagdollAirExit>,
            Option<&crate::als::state::AlsNotifyLocomotionAction>,
            &AlsMovementParamsHandle,
            &Stance,
            Option<&ChildOf>,
            &mut AlsCmcAdapterRuntime,
            (Option<&RotationMode>, &AlsMaxAllowedGait, &AlsViewYaw),
        ),
        (With<AlsCharacter>, Without<crate::als::character::AlsBirthAnimationPending>),
    >,
) {
    if time.is_paused() { return; }
    for (entity, transform, position, capsule, mut velocity, controller, mut current, request, ragdoll_air_exit,
        action, handle, stance, parent, mut runtime, (rotation, gait, yaw)) in &mut q {
        let sweep_contact = std::mem::take(&mut runtime.airborne_sweep_contact);
        let has_animation_root_velocity = std::mem::take(&mut runtime.has_animation_root_velocity);
        let collision_mode = std::mem::take(&mut runtime.post_collision_mode);
        let mantle_exit_grounded = std::mem::take(&mut runtime.mantle_exit_grounded);
        if let Some(mut source_velocity) = runtime.grounded_collision_velocity.take() {
            if *current == LocomotionState::Air && has_animation_root_velocity {
                source_velocity.y = velocity.y;
            }
            velocity.0 = source_velocity;
        }
        if !matches!(*current, LocomotionState::Grounded | LocomotionState::Air) || ragdoll_air_exit
            || action == Some(&crate::als::state::AlsNotifyLocomotionAction::Ragdolling) {
            continue;
        }
        if collision_mode == Some(*current) { continue; }
        let actor_delta = time.delta_secs() * custom_dilations.get(entity).map_or(1.0, |value| value.0);
        if *current == LocomotionState::Grounded && actor_delta < crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS {
            continue;
        }
        let Ok(basis) = controller.basis_access() else {
            continue;
        };
        let jumping = controller.action_flow_status().ongoing() == Some(AlsControlSchemeActionDiscriminant::Jump);
        let Ok(world) = transforms.compute_global_transform(entity) else { continue };
        let scale = character_floor_scale(transform, parent.is_some(), &world);
        let Ok(parent_world) = parent.map(|parent| transforms.compute_global_transform(parent.parent())).transpose() else { continue };
        let floor_result = params_assets.get(handle.id()).and_then(|params| {
            let half = if *stance == Stance::Crouching { params.crouch.actual_crouched_half_height() }
                else { params.crouch.standing_capsule_half_height } * scale.y;
            let radius = params.crouch.standing_capsule_radius * scale.x.min(scale.z);
            find_ground_floor_with_penetration(&spatial, position.0, half, radius, &params.cmc, *current,
                &SpatialQueryFilter::from_excluded_entities([entity]), entity, &candidates, None, None)
        });
        let validated_floor = floor_result.map(|floor| (floor.center_y, floor.hit.point1.y, floor.hit.entity));
        let floor_penetration = floor_result.and_then(|floor| floor.penetration);
        let capsule_floor_is_sweep = floor_result.is_none_or(|floor| floor.origin == GroundFloorOrigin::Sweep);
        let mut floor_supported = false;
        let mut floor_entity = None;
        if *current == LocomotionState::Grounded && !blocked.contains(entity) && !jumping
            && params_assets.get(handle.id()).is_some() {
            let filter = SpatialQueryFilter::from_excluded_entities([entity]);
            if let Some((floor_center_y, _, floor)) = validated_floor {
                let floor_distance = position.y - floor_center_y;
                let next_y = if floor_result.is_some_and(|floor| line_floor_blocks_height_adjustment(floor.sweep_distance, floor.line_distance))
                    || (MIN_FLOOR_DISTANCE..=MAX_FLOOR_DISTANCE).contains(&floor_distance) {
                    position.y
                } else {
                    floor_center_y + (MIN_FLOOR_DISTANCE + MAX_FLOOR_DISTANCE) * 0.5
                };
                let mut next_transform = *transform;
                next_transform.translation = position.0;
                let (direction, distance) = Dir3::new_and_length(Vec3::Y * (next_y - position.y))
                    .unwrap_or((Dir3::NEG_Y, 0.0));
                let config = ShapeCastConfig { ignore_origin_penetration: true,
                    ..ShapeCastConfig::from_max_distance(distance) };
                let accepts = |candidate| character_blocks_collider(entity, candidate, &candidates);
                let mut hit = spatial.cast_shape_predicate(capsule, position.0, transform.rotation,
                    direction, &config, &filter, &accepts);
                let initial_penetration = floor_penetration.or(hit.filter(|hit| hit.distance == 0.0));
                if let Some(initial) = initial_penetration {
                    let depth = (initial.point1 - initial.point2).dot(initial.normal1);
                    if depth > 0.0 {
                        let body = candidates.get(initial.entity).ok().and_then(|(_, _, owner, _)| owner)
                            .map_or(initial.entity, |owner| owner.body);
                        let maximum = if pawns.contains(body) { MAX_DEPENETRATION_WITH_PAWN }
                            else { MAX_DEPENETRATION_WITH_GEOMETRY };
                        let adjustment = (initial.normal1 * (depth + PENETRATION_PULLBACK_DISTANCE))
                            .clamp_length_max(maximum);
                        let extent = capsule.shape_scaled().compute_local_aabb().half_extents();
                        let inflated = als_character_capsule(extent.y + PENETRATION_OVERLAP_INFLATION,
                            extent.x + PENETRATION_OVERLAP_INFLATION);
                        let encroached = spatial.shape_intersections(&inflated, position.0 + adjustment,
                            transform.rotation, &filter).into_iter().any(accepts);
                        let applied = if !encroached {
                            adjustment
                        } else if let Ok((exit, exit_distance)) = Dir3::new_and_length(adjustment) {
                            let exit_config = ShapeCastConfig { max_distance: exit_distance, ..config };
                            let exit_hit = spatial.cast_shape_predicate(capsule, position.0, transform.rotation,
                                exit, &exit_config, &filter, &accepts);
                            *exit * exit_hit.map_or(exit_distance,
                                |hit| movement_sweep_travel(exit_distance, hit.distance))
                        } else { Vec3::ZERO };
                        if applied.length_squared() > 0.0 {
                            next_transform.translation += applied;
                            if let Some(start) = runtime.grounded_move_start.as_mut() { *start += applied; }
                            hit = spatial.cast_shape_predicate(capsule, next_transform.translation,
                                transform.rotation, direction, &config, &filter, &accepts);
                        }
                    }
                }
                next_transform.translation += *direction
                    * hit.map_or(distance, |hit| movement_sweep_travel(distance, hit.distance));
                let world_position = next_transform.translation;
                next_transform.translation = parent_world.map_or(world_position, |parent| parent.affine().inverse().transform_point3(world_position));
                commands.entity(entity).insert((Position(world_position), next_transform));
                velocity.0.y = 0.0;
                floor_supported = true;
                floor_entity = Some(floor);
            }
        }
        let airborne = TnuaBuiltinWalk::displacement(&basis).is_none()
            || jumping || validated_floor.is_none();
        let landed_on_contact = airborne && validated_floor.is_some() && collisions.collisions_with(entity)
            .any(|contact| resolved_walkable_contact(contact, entity));
        let capsule_contact = airborne && !landed_on_contact && !floor_supported
            && collisions.collisions_with(entity).any(|contact| {
                contact.flags.contains(ContactPairFlags::GENERATE_CONSTRAINTS)
                    && contact.manifolds.iter().any(|manifold| {
                        let normal = if contact.collider1 == entity { -manifold.normal } else { manifold.normal };
                        normal.y > 0.0 && manifold.points.iter().any(|point| point.normal_impulse > 0.0)
                    })
            });
        let landed_on_capsule = airborne && !landed_on_contact && !floor_supported
            && (capsule_contact || sweep_contact) && capsule_floor_is_sweep
            && params_assets.get(handle.id()).is_some_and(|params| {
                let half = if *stance == Stance::Crouching { params.crouch.actual_crouched_half_height() } else { params.crouch.standing_capsule_half_height } * scale.y;
                let radius = params.crouch.standing_capsule_radius * scale.x.min(scale.z);
                validated_floor.is_some_and(|(center, impact, _)| (position.y - center).abs() <= MAX_FLOOR_DISTANCE
                        && impact < position.y - half + radius)
            });
        let target = if airborne && !landed_on_contact && !floor_supported && !landed_on_capsule
            && !(mantle_exit_grounded && *current == LocomotionState::Grounded) {
            LocomotionState::Air
        } else {
            LocomotionState::Grounded
        };
        if *current != target && request.locomotion_state.is_none() {
            info!(
                "[MOTION] entity={entity:?} locomotion sync {:?} -> {:?}",
                current, target
            );
            transitions.write(StateTransitionEvent {
                entity,
                dimension: Dimension::LocomotionState,
                from: StateValue::LocomotionState(*current),
                to: StateValue::LocomotionState(target),
            });
            let ragdoll_on_land = *current == LocomotionState::Air && target == LocomotionState::Grounded
                && crate::get_up::RAGDOLL_SETTINGS.start_on_land
                && motion.get(entity).is_ok_and(|state| state.world_velocity.y
                    <= -crate::get_up::RAGDOLL_SETTINGS.on_land_speed_threshold_mps);
            let ragdoll_on_air_roll = target == LocomotionState::Air
                && action == Some(&crate::als::state::AlsNotifyLocomotionAction::Rolling)
                && params_assets.get(handle.id()).is_some_and(|params| params.rolling.interrupt_when_in_air);
            *current = target;
            if target == LocomotionState::Grounded { velocity.y = 0.0; }
            if (ragdoll_on_land || ragdoll_on_air_roll) && ragdoll_eligibility.allows(entity) {
                *current = LocomotionState::None;
                transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                    from: StateValue::LocomotionState(target), to: StateValue::LocomotionState(LocomotionState::None) });
                commands.entity(entity).insert(crate::als::state::AlsNotifyLocomotionAction::Ragdolling);
            }
        }
        if let Ok((mut base, dynamics)) = movement_bases.get_mut(entity) {
            if floor_entity.is_none() { floor_entity = validated_floor.map(|(_, _, entity)| entity); }
            if *current == LocomotionState::Grounded && floor_entity.is_some_and(|floor|
                movement_base_changed(base.entity, floor, &owners)
                    && pawn_rejects_character_base(floor, &pawns, &permissions, &owners))
                && let Some(params) = params_assets.get(handle.id()) {
                let max_speed = crate::als::dynamics::max_allowed_speed(params, *stance,
                    rotation.copied().unwrap_or(RotationMode::ViewDirection), gait.0, velocity.0, yaw.0);
                velocity.0 = jump_off_velocity(velocity.0, max_speed, params.jump.jump_z_velocity,
                    params.cmc.jump_off_jump_z_factor, &mut runtime.random_seed);
                *current = LocomotionState::Air;
                transitions.write(StateTransitionEvent { entity, dimension: Dimension::LocomotionState,
                    from: StateValue::LocomotionState(LocomotionState::Grounded),
                    to: StateValue::LocomotionState(LocomotionState::Air) });
                *base = default();
                if let Some(mut dynamics) = dynamics { *dynamics = default(); }
                continue;
            }
            base.entity = if *current == LocomotionState::Grounded { floor_entity } else { None };
            if let Some(entity) = base.entity && let Ok(global) = transforms.compute_global_transform(entity) {
                base.location = global.translation();
                base.rotation = global.rotation();
            }
            if let Some(mut dynamics) = dynamics {
                dynamics.entity = base.entity;
            } else {
                commands.entity(entity).insert(crate::als::dynamics::AlsMovementBaseDynamicsState {
                    entity: base.entity, world_velocity: velocity.0, relative_velocity: velocity.0, ..default()
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn subminimum_grounded_roll_suppresses_one_physics_position_step() {
        use bevy::ecs::system::RunSystemOnce;
        for (dilation, suppressed) in [(0.0, true), (0.00001, true), (0.0001, false), (1.0, false)] {
            let mut world = World::new();
            let mut time = Time::<Virtual>::default();
            time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            world.insert_resource(time);
            let actor = world.spawn((AlsCharacter, LocomotionState::Grounded, RigidBody::Dynamic,
                crate::als::roll::AlsRollState { active: true, time: 0.3, previous_time: 0.29, ..default() },
                crate::als_camera::AlsCameraCustomTimeDilation(dilation))).id();
            world.run_system_once(prepare_subminimum_custom_physics).unwrap();
            assert_eq!(world.get::<avian3d::prelude::CustomPositionIntegration>(actor).is_some(), suppressed);
            world.run_system_once(release_custom_movement_physics).unwrap();
            assert!(world.get::<avian3d::prelude::CustomPositionIntegration>(actor).is_none());
            assert_eq!(*world.get::<RigidBody>(actor).unwrap(), RigidBody::Dynamic);
        }
    }

    #[test]
    fn normal_aiming_rotation_resumes_during_the_roll_blend_tail() {
        for (time, active, expected) in [(0.3, true, 0.0), (0.9, true, 0.0318133),
            (1.05, true, 0.0318133), (1.2, false, 0.0318133)] {
            let mut app = App::new();
            let mut clock = Time::<()>::default();
            clock.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            app.insert_resource(clock).init_resource::<Assets<AlsMovementParams>>()
                .add_systems(Update, rotation_response_system);
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
            let actor = app.world_mut().spawn((AlsCharacter, Transform::IDENTITY, Stance::Standing, Gait::Running,
                LocomotionState::Grounded, RotationMode::Aiming, AlsViewYaw(1.2), AlsMovementParamsHandle(params),
                LinearVelocity(Vec3::NEG_Z * 3.75),
                crate::actual_motion::AlsActualMotion { has_input: true, input_yaw_degrees: 0.0, ..default() },
                AlsRotationResponseState { initialized: true, previous_view_yaw: 1.2,
                    previous_rotation_mode: Some(RotationMode::Aiming), ..default() },
                crate::als::roll::AlsRollState { active, time, ..default() },
                crate::als::state::AlsNotifyLocomotionAction::None)).id();
            app.update();
            let actual = world_forward_to_yaw(*app.world().get::<Transform>(actor).unwrap().forward());
            assert!((actual - expected).abs() < 1e-5,
                "time={time} active={active} actual={actual} expected={expected}");
        }
    }

    #[test]
    fn loaded_roll_blend_tail_restores_normal_aiming_rotation() {
        use bevy::ecs::system::RunSystemOnce;
        for target in [0.9_f32, 1.05, 1.2] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            crate::roll_drive::tests::play_loaded_roll(&mut app, actor);
            let current = app.world().get::<crate::als::roll::AlsRollState>(actor).unwrap().time;
            let steps = ((target - current).max(0.0) * 60.0).round() as usize;
            for _ in 0..steps { app.update(); }
            let roll = *app.world().get::<crate::als::roll::AlsRollState>(actor).unwrap();
            assert!(!roll.root_motion_active(), "target={target} roll={roll:?}");
            assert_eq!(app.world().get::<crate::als::state::AlsNotifyLocomotionAction>(actor),
                Some(&crate::als::state::AlsNotifyLocomotionAction::None));
            let mut transform = *app.world().get::<Transform>(actor).unwrap();
            transform.rotation = Quat::IDENTITY;
            app.world_mut().entity_mut(actor).insert((transform, Rotation::IDENTITY, RotationMode::Aiming,
                AlsViewYaw(1.2), LinearVelocity(Vec3::NEG_Z * 3.75), LocomotionState::Grounded,
                crate::actual_motion::AlsActualMotion { has_input: true, input_yaw_degrees: 0.0, ..default() },
                AlsRotationResponseState { initialized: true, previous_view_yaw: 1.2,
                    previous_rotation_mode: Some(RotationMode::Aiming), ..default() }));
            *app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBaseDynamicsState>(actor).unwrap() = default();
            app.world_mut().run_system_once(rotation_response_system).unwrap();
            let actual = world_forward_to_yaw(*app.world().get::<Transform>(actor).unwrap().forward());
            assert!((actual - 0.0318133).abs() < 1e-5, "target={target} roll={roll:?} actual={actual}");
        }
    }

    #[test]
    fn rolling_does_not_feed_tnua_a_second_turn_target() {
        use bevy::ecs::system::RunSystemOnce;
        for active in [false, true] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().entity_mut(actor).insert(crate::als::roll::AlsRollState {
                active, direction_yaw: 0.0, ..default()
            });
            app.world_mut().run_system_once(locomotion_drive_system).unwrap();
            let forward = app.world().get::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_forward;
            assert_eq!(forward.is_none(), active, "active={active} forward={forward:?}");
        }
    }

    #[test]
    fn unwalkable_floor_query_preserves_initial_penetration() {
        use bevy::ecs::system::RunSystemOnce;
        for (wall, floor, expect_walkable, expect_penetration) in [(true, false, false, true),
            (true, true, true, true), (false, true, true, false), (false, false, false, false)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            let wall_entity = wall.then(|| app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.5, 4.0, 4.0),
                Transform::from_xyz(0.35, 100.0, 0.0))).id());
            if floor {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                    Transform::from_xyz(0.0, 98.9785, 0.0)));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let (found, rejected) = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
                let mut rejected = None;
                let found = find_ground_floor_with_penetration(&spatial, Vec3::Y * 100.0, 0.9, 0.3,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                    &SpatialQueryFilter::from_excluded_entities([actor]), actor, &candidates, None, Some(&mut rejected));
                (found, rejected)
            }).unwrap();
            assert_eq!(found.is_some(), expect_walkable, "wall={wall} floor={floor}");
            assert_eq!(rejected.is_some(), expect_penetration, "wall={wall} floor={floor}");
            if let Some(hit) = rejected {
                assert_eq!(Some(hit.entity), wall_entity);
                assert_eq!(hit.distance, 0.0);
                assert!(hit.normal1.x < -0.9 && (hit.point1 - hit.point2).dot(hit.normal1) > 0.19);
            }
        }
    }

    #[test]
    fn stationary_invalid_floor_penetration_is_resolved_before_physics() {
        use bevy::ecs::system::RunSystemOnce;
        for (wall, floor, expect_recovery) in [(true, false, true), (true, true, false),
            (false, true, false), (false, false, false)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            if wall {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.5, 4.0, 4.0),
                    Transform::from_xyz(0.35, 100.0, 0.0)));
            }
            if floor {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                    Transform::from_xyz(0.0, 98.9785, 0.0)));
            }
            app.update();
            app.update();
            let start = Vec3::Y * 100.0;
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LinearVelocity::ZERO, LocomotionState::Grounded, TnuaMotor::default()));
            {
                let mut runtime = app.world_mut().get_mut::<AlsCmcAdapterRuntime>(actor).unwrap();
                runtime.grounded_movement_delta = Some(Vec3::ZERO);
                runtime.grounded_collision_velocity = None;
                runtime.post_collision_mode = None;
            }
            app.world_mut().run_system_once(resolve_stationary_floor_state).unwrap();
            let position = app.world().get::<Position>(actor).unwrap().0;
            let runtime = app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap();
            if expect_recovery {
                assert!(position.x < -0.19 && position.y == start.y, "position={position:?}");
                assert_eq!(runtime.post_collision_mode, Some(LocomotionState::Grounded));
                assert_eq!(runtime.grounded_collision_velocity, Some(Vec3::ZERO));
            } else {
                assert_eq!(position, start, "wall={wall} floor={floor}");
                assert_eq!(runtime.post_collision_mode,
                    if floor { None } else { Some(LocomotionState::Air) });
            }
        }
    }

    #[test]
    fn loaded_stationary_character_keeps_grounded_during_unwalkable_penetration() {
        for wall in [false, true] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            if wall {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.5, 4.0, 4.0),
                    Transform::from_xyz(0.35, 100.0, 0.0)));
            }
            app.update();
            app.update();
            let start = Vec3::Y * 100.0;
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LinearVelocity::ZERO, LocomotionState::Grounded, TnuaMotor::default()));
            *app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBase>(actor).unwrap() = default();
            *app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBaseDynamicsState>(actor).unwrap() = default();
            app.world_mut().get_mut::<AlsInputCommand>(actor).unwrap().move_dir = Vec2::ZERO;
            app.update();
            let position = app.world().get::<Position>(actor).unwrap().0;
            let mode = *app.world().get::<LocomotionState>(actor).unwrap();
            if wall {
                assert!(position.x < -0.19 && (position.y - 100.0).abs() < 1e-5, "position={position:?}");
                assert_eq!(mode, LocomotionState::Grounded);
                assert!(app.world().get::<LinearVelocity>(actor).unwrap().y.abs() < 1e-5);
            } else {
                assert_eq!(mode, LocomotionState::Air);
            }
        }
    }

    #[test]
    fn stationary_floor_loss_defers_gravity_until_the_next_frame() {
        use bevy::time::TimeUpdateStrategy;
        for stance in [Stance::Standing, Stance::Crouching] {
            for delta in [1.0 / 60.0, 0.05] {
                for remove_floor in [false, true] {
                    let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
                    let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                        Transform::from_xyz(0.0, 9.0, 0.0))).id();
                    app.update();
                    app.update();
                    let half = if stance == Stance::Crouching { 0.56 } else { 0.9 };
                    let start = Vec3::Y * (9.1 + half + 0.0215);
                    app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                        LinearVelocity::ZERO, LocomotionState::Grounded, stance, AlsDesiredStance(stance),
                        als_character_capsule(half, 0.3), TnuaMotor::default()));
                    *app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBase>(actor).unwrap() = default();
                    *app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBaseDynamicsState>(actor).unwrap() = default();
                    app.world_mut().get_mut::<AlsInputCommand>(actor).unwrap().move_dir = Vec2::ZERO;
                    for _ in 0..4 { app.update(); }
                    assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), LocomotionState::Grounded);
                    let initial = app.world().get::<Position>(actor).unwrap().0;
                    app.world_mut().get_mut::<LinearVelocity>(actor).unwrap().0 = Vec3::ZERO;
                    *app.world_mut().resource_mut::<TimeUpdateStrategy>() =
                        TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta));
                    if remove_floor { app.world_mut().despawn(floor); }
                    app.update();
                    let position = app.world().get::<Position>(actor).unwrap().0;
                    let velocity = app.world().get::<LinearVelocity>(actor).unwrap().0;
                    let mode = *app.world().get::<LocomotionState>(actor).unwrap();
                    assert_eq!(mode, if remove_floor { LocomotionState::Air } else { LocomotionState::Grounded });
                    assert!((position.y - initial.y).abs() < 1e-5 && velocity.y.abs() < 1e-5,
                        "stance={stance:?} delta={delta} remove={remove_floor} position={position:?} velocity={velocity:?}");
                    if remove_floor {
                        app.update();
                        let next = app.world().get::<LinearVelocity>(actor).unwrap().y;
                        assert!((next + 9.81 * delta).abs() < 0.01,
                            "Gravity must start on the following frame: stance={stance:?} delta={delta} velocity={next}");
                    }
                }
            }
        }
    }

    #[test]
    fn source_repulsion_uses_capsule_trace_distance_and_body_center_of_mass() {
        let capsule = als_character_capsule(0.9, 0.3);
        for (distance, speed, expected_stop, expected_force) in [(0.2, 0.0, false, 2.5),
            (0.34, 0.0, false, 2.5), (0.31, 1.0, true, 0.0), (0.32, -0.1, false, 2.5),
            (0.34, 1.0, false, 0.0), (0.4, 0.0, false, 0.0)] {
            let body = Vec3::X * distance;
            let (stop, force) = source_repulsion_response(&capsule, Vec3::ZERO, Quat::IDENTITY,
                0.9, 0.3, body, body, Vec3::X * speed, 1.0 / 60.0);
            assert_eq!(stop, expected_stop, "distance={distance} speed={speed} force={force:?}");
            assert!((force.x - expected_force).abs() < 1e-4,
                "distance={distance} speed={speed} force={force:?} expected={expected_force}");
        }
        let body = Vec3::X * 0.2;
        let (_, force) = source_repulsion_response(&capsule, Vec3::ZERO, Quat::IDENTITY,
            0.9, 0.3, body, Vec3::X * 0.5, Vec3::ZERO, 1.0 / 60.0);
        assert_eq!(force, Vec3::ZERO, "A center of mass outside the source radius must not receive force");
        let (_, force) = source_repulsion_response(&capsule, Vec3::ZERO, Quat::IDENTITY,
            0.9, 0.15, body, body, Vec3::ZERO, 1.0 / 60.0);
        assert_eq!(force, Vec3::ZERO, "The source radius must follow capsule scale");
        let body = Vec3::X * 0.34;
        let (_, force) = source_repulsion_response(&capsule, Vec3::ZERO, Quat::IDENTITY,
            0.9, 0.3, body, body, Vec3::X, 0.0);
        assert!((force.x - 2.5).abs() < 1e-4, "Zero owner delta must preserve the source distance comparison");
    }

    #[test]
    fn loaded_character_repels_overlaps_and_stops_nearby_bodies() {
        use avian3d::prelude::{Mass, NoAutoMass};
        for (kind, distance, speed, expected) in [(RigidBody::Dynamic, 0.2, 0.0, 0.041667),
            (RigidBody::Dynamic, 0.34, 0.0, 0.041667), (RigidBody::Dynamic, 0.31, 1.0, 0.0),
            (RigidBody::Dynamic, 0.32, -0.1, -0.058333), (RigidBody::Dynamic, 0.34, 1.0, 1.0),
            (RigidBody::Dynamic, 0.4, 0.0, 0.0), (RigidBody::Kinematic, 0.2, 0.0, 0.0)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let start = app.world().get::<Position>(actor).unwrap().0;
            let body = app.world_mut().spawn((kind, Collider::sphere(0.2), Sensor,
                Transform::from_translation(start + Vec3::X * distance), GravityScale(0.0),
                Mass(1.0), NoAutoMass, LinearVelocity(Vec3::X * speed))).id();
            app.world_mut().get_mut::<LinearVelocity>(actor).unwrap().0 = Vec3::ZERO;
            app.world_mut().get_mut::<AlsInputCommand>(actor).unwrap().move_dir = Vec2::ZERO;
            app.update();
            let actual = app.world().get::<LinearVelocity>(body).unwrap().x;
            assert!((actual - expected).abs() < 0.006,
                "Repulsion differs: kind={kind:?} distance={distance} speed={speed} actual={actual} expected={expected}");
        }
    }

    #[test]
    fn source_touch_impulse_scales_and_clamps_by_body_mass() {
        for (mass, expected_speed) in [(100.0, 0.025), (10.0, 0.25), (0.001, 3.75)] {
            let impulse = source_touch_impulse(Vec3::ZERO, Vec3::X * 0.45, Vec3::X * 3.75, mass);
            assert!((impulse.length() / mass - expected_speed).abs() < 1e-5);
            assert!(impulse.x > 0.0 && impulse.y > 0.0);
        }
        assert_eq!(source_touch_impulse(Vec3::ZERO, Vec3::X, Vec3::ZERO, 100.0), Vec3::ZERO);
    }

    #[test]
    fn overlap_impulse_only_runs_for_new_dynamic_sensor_contacts() {
        use avian3d::prelude::{Mass, NoAutoMass};
        for (body, sensor, actor_speed, expected) in [(RigidBody::Dynamic, true, 3.75, 0.025),
            (RigidBody::Dynamic, false, 3.75, 0.0), (RigidBody::Kinematic, true, 3.75, 0.0),
            (RigidBody::Dynamic, true, 0.0, 0.0)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            let actor = app.world_mut().spawn((AlsCharacter, Position(Vec3::ZERO), LinearVelocity(Vec3::X * actor_speed))).id();
            let other = app.world_mut().spawn((body, Collider::sphere(0.2), Transform::from_xyz(0.45, 0.0, 0.0),
                GravityScale(0.0), Mass(100.0), NoAutoMass, LinearVelocity::ZERO)).id();
            if sensor { app.world_mut().entity_mut(other).insert(Sensor); }
            app.update();
            app.update();
            app.world_mut().get_mut::<LinearVelocity>(actor).unwrap().0 = Vec3::X * actor_speed;
            app.world_mut().resource_mut::<Messages<CollisionStart>>().write(CollisionStart {
                collider1: actor, collider2: other, body1: None, body2: Some(other),
            });
            let system = app.world_mut().register_system(apply_character_overlap_impulses);
            app.world_mut().run_system(system).unwrap();
            let first = app.world().get::<LinearVelocity>(other).unwrap().0;
            assert!((first.x - expected).abs() < 1e-4,
                "Overlap impulse differs: body={body:?} sensor={sensor} speed={actor_speed} actual={first:?}");
            app.world_mut().run_system(system).unwrap();
            assert_eq!(app.world().get::<LinearVelocity>(other).unwrap().0, first,
                "A retained overlap must not apply the initial impulse again");
        }
    }

    #[test]
    fn loaded_character_first_overlap_applies_the_source_impulse() {
        use avian3d::prelude::{Mass, NoAutoMass};
        for (mass, expected) in [(100.0, 0.025), (10.0, 0.25), (0.001, 3.75)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let start = app.world().get::<Position>(actor).unwrap().0;
            let other = app.world_mut().spawn((RigidBody::Dynamic, Collider::sphere(0.2), Sensor,
                Transform::from_translation(start + Vec3::X * 0.45), GravityScale(0.0),
                Mass(mass), NoAutoMass, LinearVelocity::ZERO)).id();
            app.world_mut().get_mut::<LinearVelocity>(actor).unwrap().0 = Vec3::X * 3.75;
            app.world_mut().get_mut::<AlsInputCommand>(actor).unwrap().move_dir = Vec2::X;
            app.update();
            let actual = app.world().get::<LinearVelocity>(other).unwrap().x;
            assert!((actual - expected).abs() < 0.01,
                "First overlap impulse differs: mass={mass} actual={actual} expected={expected}");
            app.update();
            let repeated = app.world().get::<LinearVelocity>(other).unwrap().x;
            assert!((repeated - actual).abs() < 0.01,
                "A continuing overlap repeated its initial impulse: first={actual} second={repeated}");
        }
    }

    #[test]
    fn impact_force_uses_source_speed_branches_and_velocity_scaling() {
        let impact = SourceCharacterImpact { collider: Entity::PLACEHOLDER, point: Vec3::ZERO,
            normal: Vec3::NEG_X, acceleration: Vec3::NEG_Y, velocity: Vec3::X * 5.0, max_speed: 3.75 };
        for (body_velocity, expected) in [(Vec3::ZERO, 150.0), (Vec3::X * 0.005, 150.0),
            (Vec3::X * 0.1, 7500.0), (Vec3::NEG_X * 0.1, 7500.0)] {
            let force = source_character_impact_force(&impact, body_velocity);
            assert!((force.x - expected).abs() < 1e-4, "body={body_velocity:?} force={force:?}");
        }
        let scaled = SourceCharacterImpact { acceleration: Vec3::X, ..impact };
        assert!((source_character_impact_force(&scaled, Vec3::X * 0.00001).x - 56.25).abs() < 1e-4);
    }

    #[test]
    fn falling_blocking_hit_applies_the_source_force_to_dynamic_bodies() {
        use avian3d::dynamics::integrator::VelocityIntegrationData;
        use avian3d::prelude::{LockedAxes, Mass, NoAutoMass};
        use bevy::ecs::system::RunSystemOnce;
        for (body, speed, expected) in [(RigidBody::Dynamic, 0.0, 1.5),
            (RigidBody::Dynamic, 0.005, 1.5), (RigidBody::Dynamic, 0.1, 75.0),
            (RigidBody::Dynamic, -0.1, 75.0), (RigidBody::Static, 0.0, 0.0),
            (RigidBody::Kinematic, 0.0, 0.0)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .add_message::<SourceCharacterImpact>();
            app.finish();
            let wall = app.world_mut().spawn((body, Collider::cuboid(0.1, 4.0, 4.0),
                Transform::from_xyz(0.42, 0.0, 0.0), GravityScale(0.0), Mass(100.0), NoAutoMass,
                LockedAxes::ROTATION_LOCKED, LinearVelocity::ZERO)).id();
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            app.world_mut().get_mut::<LinearVelocity>(wall).unwrap().0 = Vec3::X * speed;
            let hit = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                shapes: Query<(&Collider, &Position, &Rotation)>, mut impacts: ResMut<Messages<SourceCharacterImpact>>| {
                falling_movement_sweep(&spatial, &als_character_capsule(0.9, 0.3), Vec3::ZERO, Quat::IDENTITY,
                    Vec3::X * (5.0 / 60.0), Vec3::X * 5.0, 1.0 / 60.0, None, None,
                    actor, &candidates, &shapes, &impact_bodies, Some((&mut impacts, Vec3::NEG_Y * 9.81, 3.75, 3.75, true)))
            }).unwrap();
            assert!(hit.0.x < 5.0 / 60.0, "The wall must block the capsule: {hit:?}");
            app.world_mut().run_system_once(apply_character_impact_forces).unwrap();
            let actual = app.world().get::<VelocityIntegrationData>(wall).map_or(0.0, |value| value.linear_increment.x);
            assert!((actual - expected).abs() < 1e-3,
                "Impact force differs: body={body:?} speed={speed} actual={actual} expected={expected}");
        }
    }

    #[test]
    fn non_step_ground_blocking_hit_applies_source_impact_force() {
        use avian3d::dynamics::integrator::VelocityIntegrationData;
        use avian3d::prelude::{LockedAxes, Mass, NoAutoMass};
        use bevy::ecs::system::RunSystemOnce;
        for (body, wall_x, expected) in [(RigidBody::Dynamic, 0.42, 1.5),
            (RigidBody::Static, 0.42, 0.0), (RigidBody::Dynamic, 0.8, 0.0)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .add_message::<SourceCharacterImpact>();
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(0.0, -0.1, 0.0)));
            let wall = app.world_mut().spawn((body, Collider::cuboid(0.1, 4.0, 4.0),
                Transform::from_xyz(wall_x, 1.0, 0.0), GravityScale(0.0), Mass(100.0), NoAutoMass,
                LockedAxes::ROTATION_LOCKED, LinearVelocity::ZERO,
                AlsMantleTargetPermission { allows_step_up: false })).id();
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>, mut impacts: ResMut<Messages<SourceCharacterImpact>>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3), Vec3::Y * 0.9215,
                    Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.1, &crate::als::params::CmcAdapterParams::default(),
                    LocomotionState::Grounded, actor, &candidates, &permissions,
                    Some((&mut impacts, Vec3::X * 20.0, Vec3::X * 5.0, 3.75, &impact_bodies)))
            }).unwrap();
            app.world_mut().run_system_once(apply_character_impact_forces).unwrap();
            let actual = app.world().get::<VelocityIntegrationData>(wall).map_or(0.0, |value| value.linear_increment.x);
            assert!((actual - expected).abs() < 1e-4,
                "Ground impact differs: body={body:?} wall={wall_x} actual={actual} expected={expected}");
        }
    }

    #[test]
    fn loaded_walking_character_pushes_a_non_step_dynamic_wall() {
        use avian3d::dynamics::integrator::VelocityIntegrationData;
        use avian3d::prelude::{LockedAxes, Mass, NoAutoMass};
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let wall = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(0.1, 4.0, 4.0),
            Transform::from_xyz(0.42, 2.0, 1.0), GravityScale(0.0), Mass(100.0), NoAutoMass,
            LockedAxes::ROTATION_LOCKED, LinearVelocity::ZERO,
            AlsMantleTargetPermission { allows_step_up: false })).id();
        app.update();
        app.update();
        let start = Vec3::new(0.0, 0.9215, 1.0);
        app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
            LinearVelocity(Vec3::X * 3.75), LocomotionState::Grounded, TnuaMotor::default()));
        app.world_mut().get_mut::<LinearVelocity>(wall).unwrap().0 = Vec3::ZERO;
        app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::X;
        {
            let mut runtime = app.world_mut().get_mut::<AlsCmcAdapterRuntime>(actor).unwrap();
            runtime.simulation_substeps = 1;
            runtime.simulation_delta_seconds = 0.05;
            runtime.grounded_movement_delta = Some(Vec3::X * 0.1875);
            runtime.controlled_input_max_acceleration = Some(20.0);
        }
        app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBaseDynamicsState>(actor).unwrap()
            .pre_collision_velocity = Some(Vec3::X * 3.75);
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
        app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
        app.world_mut().run_system_once(apply_character_impact_forces).unwrap();
        let actual = app.world().get::<VelocityIntegrationData>(wall).unwrap().linear_increment.x;
        assert!((actual - 1.5).abs() < 1e-4, "Walking impact must push the wall: {actual}");
    }

    #[test]
    fn step_success_does_not_push_the_initial_blocker_but_failure_does() {
        use avian3d::dynamics::integrator::VelocityIntegrationData;
        use avian3d::prelude::{LockedAxes, Mass, NoAutoMass};
        use bevy::ecs::system::RunSystemOnce;
        for (height, center_y, stepped, expected) in [(0.2, 0.1, true, 0.0), (0.1, 1.7, false, 1.5)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .add_message::<SourceCharacterImpact>();
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(0.0, -0.1, 0.0)));
            let blocker = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(0.2, height, 2.0),
                Transform::from_xyz(0.47, center_y, 0.0), GravityScale(0.0), Mass(100.0), NoAutoMass,
                LockedAxes::ROTATION_LOCKED, LinearVelocity::ZERO)).id();
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let outcome = app.world_mut().run_system_once(move |spatia impact_bodies: FallingImpactBodies,l: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>, mut impacts: ResMut<Messages<SourceCharacterImpact>>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3), Vec3::Y * 0.9215,
                    Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.5, &crate::als::params::CmcAdapterParams::default(),
                    LocomotionState::Grounded, actor, &candidates, &permissions,
                    Some((&mut impacts, Vec3::X * 20.0, Vec3::X * 5.0, 3.75, &impact_bodies)))
            }).unwrap().unwrap();
            assert_eq!(outcome.4, stepped, "height={height} outcome={outcome:?}");
            app.world_mut().run_system_once(apply_character_impact_forces).unwrap();
            let actual = app.world().get::<VelocityIntegrationData>(blocker).unwrap().linear_increment.x;
            assert!((actual - expected).abs() < 1e-4,
                "Step impact differs: height={height} actual={actual} expected={expected}");
        }
    }

    #[test]
    fn blocked_step_reports_both_ceiling_and_forward_impacts() {
        use avian3d::dynamics::integrator::VelocityIntegrationData;
        use avian3d::prelude::{LockedAxes, Mass, NoAutoMass};
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
            .add_message::<SourceCharacterImpact>();
        app.finish();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
            Transform::from_xyz(0.0, -0.1, 0.0)));
        let wall = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(0.1, 1.0, 2.0),
            Transform::from_xyz(0.42, 0.5, 0.0), GravityScale(0.0), Mass(100.0), NoAutoMass,
            LockedAxes::ROTATION_LOCKED, LinearVelocity::ZERO)).id();
        let ceiling = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(4.0, 0.1, 2.0),
            Transform::from_xyz(0.0, 1.95, 0.0), GravityScale(0.0), Mass(100.0), NoAutoMass,
            LockedAxes::ROTATION_LOCKED, LinearVelocity::ZERO)).id();
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        app.update();
        app.world_mut().run_system_o impact_bodies: FallingImpactBodies,nce(move |spatial: SpatialQuery,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
            permissions: Query<&AlsMantleTargetPermission>, mut impacts: ResMut<Messages<SourceCharacterImpact>>| {
            grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3), Vec3::Y * 0.9215,
                Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.5, &crate::als::params::CmcAdapterParams::default(),
                LocomotionState::Grounded, actor, &candidates, &permissions,
                Some((&mut impacts, Vec3::X * 20.0, Vec3::X * 5.0, 3.75, &impact_bodies)))
        }).unwrap();
        app.world_mut().run_system_once(apply_character_impact_forces).unwrap();
        let wall_acceleration = app.world().get::<VelocityIntegrationData>(wall).unwrap().linear_increment;
        let ceiling_acceleration = app.world().get::<VelocityIntegrationData>(ceiling).unwrap().linear_increment;
        assert!(wall_acceleration.x > 0.0, "Forward step impact must push the wall: {wall_acceleration:?}");
        assert!(ceiling_acceleration.y > 0.0, "Blocked lift must push the ceiling: {ceiling_acceleration:?}");
    }

    #[test]
    fn slide_records_only_the_sweeps_that_hit() {
        use bevy::ecs::system::RunSystemOnce;
        for third_wall in [false, true] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            let first = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 2.0, 0.1),
                Transform::from_xyz(0.0, 1.0, -0.6).with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4)))).id();
            let third = third_wall.then(|| app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 2.0, 4.0),
                Transform::from_xyz(-0.5, 1.0, 0.0))).id());
            app.update();
            app.update();
            let impacts = app.world_mut().run_system_once(|spatial: SpatialQuery| {
                let mut impacts = Vec::new();
                walking_slide_sweep(&spatial, &als_character_capsule(0.9, 0.3), Vec3::Y,
                    Vec3::new(1.0, 0.0, -1.0), Vec3::NEG_X, 0.0215,
                    &SpatialQueryFilter::default(), &|_| true, Some(&mut impacts)).unwrap();
                impacts
            }).unwrap();
            assert_eq!(impacts.first().map(|(hit, _)| hit.entity), Some(first));
            assert_eq!(impacts.get(1).map(|(hit, _)| hit.entity), third);
            assert_eq!(impacts.len(), if third_wall { 2 } else { 1 });
        }
    }

    #[test]
    fn successful_step_discards_speculative_slide_impacts() {
        use avian3d::dynamics::integrator::VelocityIntegrationData;
        use avian3d::prelude::{LockedAxes, Mass, NoAutoMass};
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
            .add_message::<SourceCharacterImpact>();
        app.finish();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
            Transform::from_xyz(0.0, -0.1, 0.0)));
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.2, 0.2, 1.0),
            Transform::from_xyz(0.47, 0.1, 0.0)));
        let witness = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(2.0, 0.2, 0.1),
            Transform::from_xyz(0.0, 0.1, 0.6), GravityScale(0.0), Mass(100.0), NoAutoMass,
            LockedAxes::ROTATION_LOCKED, LinearVelocity::ZERO)).id();
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        app.update();
        let (witnessed, outcome) = app.world_mu impact_bodies: FallingImpactBodies,t().run_system_once(move |spatial: SpatialQuery,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
            permissions: Query<&AlsMantleTargetPermission>, mut impacts: ResMut<Messages<SourceCharacterImpact>>| {
            let capsule = als_character_capsule(0.9, 0.3);
            let mut preview = Vec::new();
            walking_slide_sweep(&spatial, &capsule, Vec3::new(0.07, 0.9215, 0.07),
                Vec3::new(0.43, 0.0, 0.43), Vec3::NEG_X, 0.0215,
                &SpatialQueryFilter::default(), &|_| true, Some(&mut preview)).unwrap();
            let outcome = grounded_movement_transaction(&spatial, &capsule, Vec3::Y * 0.9215,
                Quat::IDENTITY, 0.9, 0.3, Vec3::new(0.5, 0.0, 0.5), &crate::als::params::CmcAdapterParams::default(),
                LocomotionState::Grounded, actor, &candidates, &permissions,
                Some((&mut impacts, Vec3::X * 20.0, Vec3::X * 5.0, 3.75, &impact_bodies))).unwrap();
            (preview.iter().any(|(hit, _)| hit.entity == witness), outcome)
        }).unwrap();
        assert!(witnessed, "The discarded slide must actually hit its witness");
        assert!(outcome.4, "The actual step must succeed: {outcome:?}");
        app.world_mut().run_system_once(apply_character_impact_forces).unwrap();
        let acceleration = app.world().get::<VelocityIntegrationData>(witness).unwrap().linear_increment;
        assert_eq!(acceleration, Vec3::ZERO, "A speculative slide must not push the witness");
    }

    #[test]
    fn failed_step_commits_the_actual_fallback_slide_impact() {
        use avian3d::dynamics::integrator::VelocityIntegrationData;
        use avian3d::prelude::{LockedAxes, Mass, NoAutoMass};
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
            .add_message::<SourceCharacterImpact>();
        app.finish();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0), Transform::from_xyz(0.0, -0.1, 0.0)));
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 2.0, 4.0), Transform::from_xyz(0.42, 1.0, 0.0),
            AlsMantleTargetPermission { allows_step_up: false }));
        let witness = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(1.0, 2.0, 0.1),
            Transform::from_xyz(-0.3, 1.0, 0.6), GravityScale(0.0), Mass(100.0), NoAutoMass,
            LockedAxes::ROTATION_LOCKED, LinearVelocity::ZERO)).id();
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        app.update();
     impact_bodies: FallingImpactBodies,    app.world_mut().run_system_once(move |spatial: SpatialQuery,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
            permissions: Query<&AlsMantleTargetPermission>, mut impacts: ResMut<Messages<SourceCharacterImpact>>| {
            grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3), Vec3::Y * 0.9215,
                Quat::IDENTITY, 0.9, 0.3, Vec3::new(0.5, 0.0, 0.5), &crate::als::params::CmcAdapterParams::default(),
                LocomotionState::Grounded, actor, &candidates, &permissions,
                Some((&mut impacts, Vec3::X * 20.0, Vec3::X * 5.0, 3.75, &impact_bodies)))
        }).unwrap();
        app.world_mut().run_system_once(apply_character_impact_forces).unwrap();
        let acceleration = app.world().get::<VelocityIntegrationData>(witness).unwrap().linear_increment;
        assert!(acceleration.z > 0.0, "The committed slide must push its blocking witness: {acceleration:?}");
    }

    #[test]
    fn unresolved_ground_penetration_reports_an_impact_once() {
        use avian3d::prelude::{LockedAxes, Mass, NoAutoMass};
        use bevy::ecs::system::RunSystemOnce;
        for blocked in [false, true] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .add_message::<SourceCharacterImpact>();
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0), Transform::from_xyz(0.0, -0.1, 0.0)));
            let wall = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(0.5, 2.0, 2.0),
                Transform::from_xyz(0.525, 1.7, 0.0), GravityScale(0.0), Mass(100.0), NoAutoMass,
                LockedAxes::ROTATION_LOCKED, LinearVelocity::ZERO,
                AlsMantleTargetPermission { allows_step_up: false })).id();
            if blocked {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.5, 2.0, 2.0),
                    Transform::from_xyz(-0.5501, 1.7, 0.0)));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let (initial, recovered) = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>, mut impacts: ResMut<Messages<SourceCharacterImpact>>| {
                let capsule = als_character_capsule(0.9, 0.3);
                let center = Vec3::Y * 0.9215;
                let delta = Vec3::NEG_X * 0.3125;
                let filter = SpatialQueryFilter::from_excluded_entities([actor]);
                let accepts = |candidate| character_blocks_collider(actor, candidate, &candidates);
                let raw = spatial.cast_shape_predicate(&capsule, center, Quat::IDENTITY, Dir3::NEG_X,
                    &ShapeCastConfig::from_max_distance(delta.length()), &filter, &accepts).unwrap();
                let recovered = falling_resolve_penetration(&spatial, &capsule, center, Quat::IDENTITY,
                    &raw, delta, &filter, &accepts, &impact_bodies).is_some();
                grounded_movement_transaction(&spatial, &capsule, center, Quat::IDENTITY, 0.9, 0.3, delta,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                    actor, &candidates, &permissions, Some((&mut impacts, Vec3::NEG_X * 20.0,
                        Vec3::NEG_X * 5.0, 3.75, &impact_bodies))).unwrap();
                (raw.entity, recovered)
            }).unwrap();
            assert_eq!(initial, wall, "The dynamic wall must be the initial overlapping shape");
            assert_eq!(recovered, !blocked, "The opposing wall must prevent penetration recovery");
            assert_eq!(app.world().resource::<Messages<SourceCharacterImpact>>().len(), usize::from(blocked));
        }
    }

    #[test]
    fn loaded_character_pushes_a_dynamic_wall_on_falling_impact() {
        use avian3d::prelude::{LockedAxes, Mass, NoAutoMass};
        for (speed, expected) in [(0.0, 0.025), (0.1, 1.35), (-0.1, 1.15)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let wall = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(0.1, 4.0, 4.0),
                Transform::from_xyz(0.42, 5.0, 0.0), GravityScale(0.0), Mass(100.0), NoAutoMass,
                LockedAxes::ROTATION_LOCKED, LinearVelocity::ZERO)).id();
            app.update();
            app.update();
            let start = Vec3::Y * 5.0;
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LinearVelocity(Vec3::X * 5.0), LocomotionState::Air));
            app.world_mut().get_mut::<LinearVelocity>(wall).unwrap().0 = Vec3::X * speed;
            app.update();
            let actual = app.world().get::<LinearVelocity>(wall).unwrap().x;
            assert!((actual - expected).abs() < 0.02,
                "Falling impact must push the wall: speed={speed} actual={actual} expected={expected}");
        }
    }

    #[test]
    fn normal_landing_applies_source_impact_force_to_dynamic_bases() {
        use avian3d::dynamics::integrator::VelocityIntegrationData;
        use avian3d::prelude::{LockedAxes, Mass, NoAutoMass};
        use bevy::ecs::system::RunSystemOnce;
        for (body, landing_allowed, expected) in [(RigidBody::Dynamic, true, -1.5),
            (RigidBody::Dynamic, false, 0.0), (RigidBody::Static, true, 0.0),
            (RigidBody::Kinematic, true, 0.0)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .add_message::<SourceCharacterImpact>();
            app.finish();
            let floor = app.world_mut().spawn((body, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(0.0, -0.1, 0.0), GravityScale(0.0), Mass(100.0), NoAutoMass,
                LockedAxes::ROTATION_LOCKED, LinearVelocity::ZERO)).id();
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let hit = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                shapes: Query<(&Collider, &Position, &Rotation)>, mut impacts: ResMut<Messages<SourceCharacterImpact>>| {
                let params = crate::als::params::CmcAdapterParams::default();
                let mut seed = 123;
                falling_movement_sweep(&spatial, &als_character_capsule(0.9, 0.3), Vec3::Y * 0.925, Quat::IDENTITY,
                    Vec3::NEG_Y * 0.05, Vec3::NEG_Y * 2.0, 0.025, None, Some((&params, 4.2, 3.75, &mut seed)),
                    actor, &candidates, &shapes, &impact_bodies,
                    Some((&mut impacts, Vec3::NEG_Y * 9.81, 3.75, 3.75, landing_allowed)))
            }).unwrap();
            assert!(hit.3.is_some(), "The capsule must land: {hit:?}");
            app.world_mut().run_system_once(apply_character_impact_forces).unwrap();
            let actual = app.world().get::<VelocityIntegrationData>(floor).map_or(0.0, |value| value.linear_increment.y);
            assert!((actual - expected).abs() < 1e-4,
                "Landing impact differs: body={body:?} allowed={landing_allowed} actual={actual} expected={expected}");
        }
    }

    #[test]
    fn loaded_landing_pushes_dynamic_base_before_standing_force() {
        use avian3d::dynamics::integrator::VelocityIntegrationData;
        use avian3d::prelude::{LockedAxes, Mass, NoAutoMass};
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let floor = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(4.0, 0.2, 4.0),
            Transform::from_xyz(0.0, 1.9, 0.0), GravityScale(0.0), Mass(100.0), NoAutoMass,
            LockedAxes::ROTATION_LOCKED, LinearVelocity::ZERO)).id();
        app.update();
        app.update();
        let start = Vec3::Y * 2.925;
        app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
            LinearVelocity(Vec3::NEG_Y * 2.0), LocomotionState::Air, TnuaMotor::default()));
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.025));
        app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
        assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), LocomotionState::Grounded);
        app.world_mut().run_system_once(apply_character_impact_forces).unwrap();
        let actual = app.world().get::<VelocityIntegrationData>(floor).unwrap().linear_increment.y;
        assert!((actual + 1.5).abs() < 1e-4, "Landing must apply its own impact force: {actual}");
    }

    #[test]
    fn standing_force_uses_source_mass_gravity_and_contact_point() {
        use avian3d::dynamics::integrator::VelocityIntegrationData;
        use avian3d::prelude::{Mass, NoAutoMass};
        use bevy::ecs::system::RunSystemOnce;
        for (body, mass, owner_dilation, gravity_scale, offset) in [
            (RigidBody::Dynamic, 100.0, 1.0, 1.0, 0.0),
            (RigidBody::Dynamic, 1000.0, 1.0, 1.0, 0.0),
            (RigidBody::Dynamic, 100.0, 0.0, 1.0, 0.0),
            (RigidBody::Dynamic, 100.0, 1.0, 0.0, 0.0),
            (RigidBody::Dynamic, 100.0, 1.0, 1.0, 0.4),
            (RigidBody::Static, 100.0, 1.0, 1.0, 0.0),
            (RigidBody::Kinematic, 100.0, 1.0, 1.0, 0.0),
        ] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .insert_resource(Assets::<AlsMovementParams>::default());
            app.finish();
            let platform = app.world_mut().spawn((body, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(0.0, -0.1, 0.0), GravityScale(0.0), Mass(mass), NoAutoMass)).id();
            app.update();
            app.update();
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
            let start = Vec3::new(offset, 0.9215, 0.0);
            app.world_mut().spawn((AlsCharacter, Transform::from_translation(start), Position(start),
                Stance::Standing, AlsMovementParamsHandle(params), LocomotionState::Grounded,
                AlsEffectiveGravity { global: Vec3::NEG_Y * 9.81, gravity_scale, ..default() },
                crate::als_camera::AlsCameraCustomTimeDilation(owner_dilation)));
            app.world_mut().run_system_once(apply_standing_downward_force).unwrap();
            let integration = app.world().get::<VelocityIntegrationData>(platform);
            let acceleration = integration.map_or(Vec3::ZERO, |value| value.linear_increment);
            let expected = if body == RigidBody::Dynamic { -981.0 * gravity_scale / mass } else { 0.0 };
            assert!((acceleration.y - expected).abs() < 1e-4,
                "Standing force differs: {body:?} mass={mass} owner={owner_dilation} gravity={gravity_scale} acceleration={acceleration:?}");
            if body == RigidBody::Dynamic && gravity_scale != 0.0 && offset != 0.0 {
                assert!(integration.unwrap().angular_increment.z < 0.0, "Off-center standing force must apply torque");
            }
        }
    }

    #[test]
    fn loaded_character_pushes_dynamic_base_with_world_physics_time() {
        use avian3d::prelude::{LockedAxes, Mass, NoAutoMass};
        for (mass, owner_dilation) in [(100.0, 1.0), (1000.0, 1.0), (100.0, 0.0)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let platform = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(0.0, 1.9, 0.0), GravityScale(0.0), Mass(mass), NoAutoMass,
                LockedAxes::ROTATION_LOCKED)).id();
            app.update();
            app.update();
            let start = Vec3::new(0.0, 2.9215, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LinearVelocity::ZERO, LocomotionState::Grounded,
                crate::als_camera::AlsCameraCustomTimeDilation(owner_dilation)));
            app.world_mut().get_mut::<LinearVelocity>(platform).unwrap().0 = Vec3::ZERO;
            app.update();
            let actual = app.world().get::<LinearVelocity>(platform).unwrap().y;
            let expected = -0.1635 * 100.0 / mass;
            assert!((actual - expected).abs() < 0.005,
                "Standing force must use world time: mass={mass} owner={owner_dilation} actual={actual} expected={expected}");
        }
    }

    #[test]
    fn jump_off_rejects_only_forbidden_pawn_bases() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let forbidden = world.spawn(AlsCharacter).id();
        let allowed = world.spawn((AlsCharacter, AlsMantleTargetPermission { allows_step_up: true })).id();
        let prop = world.spawn(AlsMantleTargetPermission { allows_step_up: false }).id();
        let child = world.spawn(ColliderOf { body: forbidden }).id();
        world.run_system_once(move |pawns: Query<(), With<AlsCharacter>>,
            permissions: Query<&AlsMantleTargetPermission>, owners: Query<&ColliderOf>| {
            assert!(pawn_rejects_character_base(forbidden, &pawns, &permissions, &owners));
            assert!(pawn_rejects_character_base(child, &pawns, &permissions, &owners));
            assert!(!pawn_rejects_character_base(allowed, &pawns, &permissions, &owners));
            assert!(!pawn_rejects_character_base(prop, &pawns, &permissions, &owners));
        }).unwrap();
    }

    #[test]
    fn base_change_compares_physics_owners() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let body = world.spawn_empty().id();
        let child = world.spawn(ColliderOf { body }).id();
        let other = world.spawn_empty().id();
        world.run_system_once(move |owners: Query<&ColliderOf>| {
            assert!(movement_base_changed(None, child, &owners));
            assert!(!movement_base_changed(Some(body), child, &owners));
            assert!(!movement_base_changed(Some(child), body, &owners));
            assert!(movement_base_changed(Some(other), child, &owners));
        }).unwrap();
    }

    #[test]
    fn jump_off_uses_source_speed_and_configured_vertical_velocity() {
        let mut seed = 123;
        let stationary = jump_off_velocity(Vec3::ZERO, 4.0, 4.2, 0.0, &mut seed);
        assert!((stationary.with_y(0.0).length() - 3.4 / 1.25_f32.sqrt()).abs() < 1e-5);
        assert_eq!(stationary.y, 0.0);
        let limited = jump_off_velocity(Vec3::X * 50.0, 4.0, 4.2, 0.5, &mut seed);
        assert!(limited.with_y(0.0).length() <= 3.4 + 1e-5);
        assert!((limited.y - 2.1).abs() < 1e-5);
    }

    #[test]
    fn falling_landing_jumps_off_forbidden_pawns() {
        use bevy::ecs::system::RunSystemOnce;
        for (pawn, allowed) in [(true, false), (true, true), (false, false)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .add_message::<StateTransitionEvent>();
            app.finish();
            let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(0.0, 1.9, 0.0), AlsMantleTargetPermission { allows_step_up: allowed })).id();
            if pawn { app.world_mut().entity_mut(floor).insert(AlsCharacter); }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>,
                bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
                owners: Query<&ColliderOf>, shapes: Query<(&Collider, &Position, &Rotation)>,
                mut transitions: MessageWriter<StateTransitionEvent>| {
                let mut params = AlsMovementParams::default();
                params.cmc.max_simulation_iterations = 1;
                let mut seed = 123;
                falling_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Transform::from_xyz(0.0, 2.925, 0.0), Vec3::NEG_Y, Vec3::ZERO,
                    3.75, &params, Vec3::NEG_Y * 9.81,
                    (Stance::Standing, RotationMode::ViewDirection, Gait::Running, 0.0),
                    (0.0, false, false), 40.0, 0.05, 0, actor, &mut seed,
                    &candidates, &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, None, None)
            }).unwrap();
            if pawn && !allowed {
                assert_eq!(result.4, LocomotionState::Air, "The forbidden pawn must trigger JumpOff: {result:?}");
                assert!(result.1.with_y(0.0).length() > 0.0 && result.1.y == 0.0);
                assert!(result.5.is_none());
            } else {
                assert_eq!(result.4, LocomotionState::Grounded, "The permitted base must remain grounded: {result:?}");
                assert_eq!(result.5, Some(floor));
            }
        }
    }

    #[test]
    fn spawn_rejects_forbidden_pawn_bases() {
        use bevy::ecs::system::RunSystemOnce;
        for (pawn, allowed) in [(true, false), (true, true), (false, false)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .insert_resource(Assets::<AlsMovementParams>::default()).add_message::<StateTransitionEvent>();
            app.finish();
            let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(0.0, -0.1, 0.0), AlsMantleTargetPermission { allows_step_up: allowed })).id();
            if pawn { app.world_mut().entity_mut(floor).insert(AlsCharacter); }
            app.update();
            app.update();
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
            let actor = app.world_mut().spawn((AlsCharacter, RigidBody::Dynamic, als_character_capsule(0.9, 0.3),
                Transform::from_xyz(0.0, 0.9215, 0.0), LocomotionState::Grounded,
                Stance::Standing, AlsMovementParamsHandle(params))).id();
            app.world_mut().run_system_once(initialize_spawn_movement_mode).unwrap();
            let mode = *app.world().get::<LocomotionState>(actor).unwrap();
            let base = app.world().get::<crate::als::dynamics::AlsMovementBase>(actor).unwrap().entity;
            if pawn && !allowed {
                assert_eq!(mode, LocomotionState::Air);
                assert!(base.is_none());
                assert!(app.world().get::<LinearVelocity>(actor).unwrap().0.with_y(0.0).length() > 0.0);
            } else {
                assert_eq!(mode, LocomotionState::Grounded);
                assert_eq!(base, Some(floor));
            }
        }
    }

    #[test]
    fn state_sync_rejects_new_forbidden_pawn_bases() {
        use bevy::ecs::system::RunSystemOnce;
        for (pawn, allowed, already_based) in [(true, false, false), (true, true, false),
            (false, false, false), (true, false, true)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(0.0, 1.9, 0.0), AlsMantleTargetPermission { allows_step_up: allowed })).id();
            if pawn { app.world_mut().entity_mut(floor).insert(AlsCharacter); }
            app.update();
            app.update();
            let position = Vec3::new(0.0, 2.9215, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(position), Transform::from_translation(position),
                LinearVelocity::ZERO, LocomotionState::Grounded));
            *app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBase>(actor).unwrap() = default();
            if already_based {
                app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBase>(actor).unwrap().entity = Some(floor);
            }
            app.world_mut().get_mut::<AlsCmcAdapterRuntime>(actor).unwrap().post_collision_mode = None;
            app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
            let mode = *app.world().get::<LocomotionState>(actor).unwrap();
            let base = app.world().get::<crate::als::dynamics::AlsMovementBase>(actor).unwrap().entity;
            if pawn && !allowed && !already_based {
                assert_eq!(mode, LocomotionState::Air);
                assert!(base.is_none());
                assert!(app.world().get::<LinearVelocity>(actor).unwrap().0.with_y(0.0).length() > 0.0);
            } else {
                assert_eq!(mode, LocomotionState::Grounded);
                assert_eq!(base, Some(floor));
            }
        }
    }

    #[test]
    fn walking_base_change_jumps_off_forbidden_pawns() {
        use bevy::ecs::system::RunSystemOnce;
        for (pawn, allowed, steps) in [(true, false, 1), (true, false, 2),
            (true, true, 1), (false, false, 1)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let old = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.0, 0.2, 2.0),
                Transform::from_xyz(-1.0, 1.9, 0.0))).id();
            let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.05, 0.2, 2.0),
                Transform::from_xyz(0.1875, 1.9, 0.0), AlsMantleTargetPermission { allows_step_up: allowed })).id();
            if pawn { app.world_mut().entity_mut(floor).insert(AlsCharacter); }
            app.update();
            app.update();
            let position = Vec3::new(0.0, 2.9215, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(position), Transform::from_translation(position),
                LinearVelocity(Vec3::X * 3.75), LocomotionState::Grounded, TnuaMotor::default()));
            {
                let mut base = app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBase>(actor).unwrap();
                base.entity = Some(old);
                base.location = Vec3::new(-1.0, 1.9, 0.0);
                base.rotation = Quat::IDENTITY;
            }
            {
                let mut base = app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBaseDynamicsState>(actor).unwrap();
                base.entity = Some(old);
                base.relative_velocity = Vec3::X * 3.75;
                base.support_velocity = Vec3::ZERO;
                base.pre_collision_velocity = Some(Vec3::X * 3.75);
            }
            app.world_mut().get_mut::<crate::als::dynamics::AlsGroundedDynamics>(actor).unwrap().0 =
                crate::als::params::GroundedDynamics { acceleration: 0.0, braking_deceleration: 0.0, ground_friction: 0.0 };
            app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::ZERO;
            let delta = if steps == 1 { 0.05 } else { 0.1 };
            {
                let mut runtime = app.world_mut().get_mut::<AlsCmcAdapterRuntime>(actor).unwrap();
                runtime.simulation_substeps = steps;
                runtime.simulation_delta_seconds = delta;
                runtime.grounded_movement_delta = Some(Vec3::X * (3.75 * delta));
                runtime.random_seed = 123;
            }
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(delta));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            let mode = *app.world().get::<LocomotionState>(actor).unwrap();
            if pawn && !allowed {
                assert_eq!(mode, LocomotionState::Air, "steps={steps}");
                let velocity = app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().grounded_collision_velocity.unwrap();
                assert!(velocity.with_y(0.0).length() > 0.0);
                assert!(app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(actor).unwrap().entity.is_none());
                app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
                assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), LocomotionState::Air);
                assert_eq!(app.world().get::<LinearVelocity>(actor).unwrap().0, velocity,
                    "Frame-end synchronization must preserve the first JumpOff");
            } else {
                assert_eq!(mode, LocomotionState::Grounded);
                assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(actor).unwrap().entity, Some(floor));
            }
        }
    }

    #[test]
    fn root_floor_dimensions_use_the_stored_scale() {
        let local = Transform::from_rotation(Quat::from_xyzw(0.0, -0.6227078437805176, 0.0, 0.7824543714523315));
        let world = GlobalTransform::from(local);
        let extracted = world.to_scale_rotation_translation().0;
        assert_eq!(extracted, Vec3::new(0.9999998807907104, 1.0, 0.9999998807907104));
        let scale = character_floor_scale(&local, false, &world);
        assert_eq!(scale, Vec3::ONE);
        let radius = 0.29999998211860657_f32;
        assert_eq!(radius * scale.x.min(scale.z), 0.29999998211860657_f32);
        assert_eq!(radius * extracted.x.min(extracted.z), 0.2999999523162842_f32);

        let scaled = local.with_scale(Vec3::new(1.25, 0.8, 0.6));
        assert_eq!(character_floor_scale(&scaled, false, &GlobalTransform::from(scaled)), scaled.scale);
        let parented_world = GlobalTransform::from(Transform::from_scale(Vec3::new(2.0, 3.0, 4.0)));
        assert_eq!(character_floor_scale(&scaled, true, &parented_world), parented_world.to_scale_rotation_translation().0);
    }

    #[test]
    fn loaded_scaled_character_uses_the_circular_collider_for_movement() {
        use bevy::ecs::system::RunSystemOnce;
        for parented in [false, true] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            if parented {
                let parent = app.world_mut().spawn(Transform::from_scale(Vec3::new(2.0, 1.0, 1.0))).id();
                app.world_mut().entity_mut(actor).insert(ChildOf(parent));
            } else {
                app.world_mut().get_mut::<Transform>(actor).unwrap().scale = Vec3::new(2.0, 1.0, 1.0);
            }
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 4.0, 0.1),
                Transform::from_xyz(0.0, 5.0, -1.05)));
            app.update();
            app.update();
            let (walking, falling) = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                capsules: Query<&Collider>,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>, shapes: Query<(&Collider, &Position, &Rotation)>| {
                let capsule = capsules.get(actor).unwrap();
                let start = Vec3::Y * 5.0;
                let walking = grounded_movement_transaction(&spatial, capsule, start, Quat::IDENTITY, 0.9, 0.3,
                    Vec3::NEG_Z, &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                    actor, &candidates, &permissions, None).unwrap();
                let falling = falling_movement_sweep(&spatial, capsule, start, Quat::IDENTITY, Vec3::NEG_Z,
                    Vec3::NEG_Z * 10.0, 0.1, None, None, actor, &candidates, &shapes, &impact_bodies, None);
                (walking.0.z, falling.0.z)
            }).unwrap();
            for distance in [walking, falling] {
                assert!((-0.4..-0.37).contains(&distance), "parented={parented} distance={distance}");
            }
        }
    }

    #[test]
    fn grounded_floor_adjustment_uses_the_owner_tick_threshold() {
        use bevy::ecs::system::RunSystemOnce;
        for (world_delta, dilation, start_y, adjusts) in [
            (0.05_f32, 0.0_f32, 1.0_f32, false),
            (0.05, 0.0, 0.9215, false),
            (0.0000005, 1.0, 1.0, false),
            (0.000001, 1.0, 1.0, true),
            (0.05, 1.0, 1.0, true),
        ] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let start = Vec3::Y * start_y;
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LocomotionState::Grounded, AlsStateRequest::default(), AlsCmcAdapterRuntime::default(),
                crate::als_camera::AlsCameraCustomTimeDilation(dilation)));
            app.world_mut().resource_mut::<Time<Virtual>>().advance_by(std::time::Duration::from_secs_f32(world_delta));
            app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
            let actual = app.world().get::<Position>(actor).unwrap().y;
            let expected = if adjusts { 0.9215 } else { start_y };
            assert!((actual - expected).abs() < 1e-4,
                "world_delta={world_delta} dilation={dilation} start={start_y} actual={actual} expected={expected}");
            assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), LocomotionState::Grounded);
            if !adjusts && start_y == 1.0 {
                app.world_mut().get_mut::<crate::als_camera::AlsCameraCustomTimeDilation>(actor).unwrap().0 = 1.0;
                app.world_mut().resource_mut::<Time<Virtual>>().advance_by(std::time::Duration::from_secs_f32(0.05));
                app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
                assert!((app.world().get::<Position>(actor).unwrap().y - 0.9215).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn crouch_base_shift_sweeps_the_resized_world_capsule() {
        use bevy::ecs::system::RunSystemOnce;
        for (angle, parented, floor) in [(0.0_f32, false, true), (60.0, false, true),
            (60.0, true, true), (90.0, false, true), (90.0, true, true), (60.0, false, false)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let rotation = Quat::from_rotation_x(angle.to_radians());
            let start = Vec3::new(if floor { 0.0 } else { 200.0 }, 0.3 + 0.6 * angle.to_radians().cos().abs() + 0.0215, 0.0);
            let local = if parented {
                let parent = app.world_mut().spawn(Transform::from_rotation(rotation)).id();
                app.world_mut().entity_mut(actor).insert(ChildOf(parent));
                Transform::from_translation(rotation.inverse() * start)
            } else {
                Transform::from_translation(start).with_rotation(rotation)
            };
            app.world_mut().entity_mut(actor).insert((Position(start), local, Stance::Standing,
                AlsDesiredStance(Stance::Crouching), LocomotionState::Grounded, AlsCrouchTransaction::default()));
            app.world_mut().run_system_once(capsule_reshape_system).unwrap();
            let position = app.world().get::<Position>(actor).unwrap().0;
            let available = 0.34 * angle.to_radians().cos().abs() + 0.0215;
            let expected_travel = if floor && available < 0.34 { movement_sweep_travel(0.34, available) } else { 0.34 };
            assert!((position.y - (start.y - expected_travel)).abs() < 0.002,
                "angle={angle} parented={parented} floor={floor} position={position:?}");
            assert_eq!(*app.world().get::<Stance>(actor).unwrap(), Stance::Crouching);
            assert_eq!(app.world().get::<Transform>(actor).unwrap().rotation, local.rotation);
        }
    }

    #[test]
    fn uncrouch_clearance_uses_gravity_orientation_instead_of_actor_rotation() {
        use bevy::ecs::system::RunSystemOnce;
        for (angle, parented, obstacle) in [(60.0_f32, false, "side"), (60.0, false, "ceiling"),
            (60.0, true, "side"), (60.0, true, "ceiling"), (0.0, false, "side"), (60.0, false, "none")] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            if obstacle == "side" {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 4.0, 0.1), Transform::from_xyz(0.0, 10.34, 0.65)));
            } else if obstacle == "ceiling" {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.1, 4.0), Transform::from_xyz(0.0, 11.15, 0.0)));
            }
            app.update();
            let rotation = Quat::from_rotation_x(angle.to_radians());
            let start = Vec3::Y * 10.0;
            let local = if parented {
                let parent = app.world_mut().spawn(Transform::from_rotation(rotation)).id();
                app.world_mut().entity_mut(actor).insert(ChildOf(parent));
                Transform::from_translation(rotation.inverse() * start)
            } else { Transform::from_translation(start).with_rotation(rotation) };
            app.world_mut().entity_mut(actor).insert((Position(start), local, Stance::Crouching,
                AlsDesiredStance(Stance::Standing), LocomotionState::Air, AlsCrouchTransaction::default(),
                als_character_capsule(0.56, 0.3)));
            let (upright_blocked, actor_blocked) = app.world_mut().run_system_once(move |spatial: SpatialQuery| {
                let capsule = als_character_capsule(0.9 + STAND_UP_SWEEP_INFLATION, 0.3);
                let center = start + Vec3::Y * (0.34 + STAND_UP_SWEEP_INFLATION);
                let filter = SpatialQueryFilter::from_excluded_entities([actor]);
                (!spatial.shape_intersections(&capsule, center, Quat::IDENTITY, &filter).is_empty(),
                    !spatial.shape_intersections(&capsule, center, rotation, &filter).is_empty())
            }).unwrap();
            let blocked = obstacle == "ceiling";
            assert_eq!(upright_blocked, blocked, "angle={angle} obstacle={obstacle}");
            if angle == 60.0 && obstacle != "none" { assert_ne!(actor_blocked, upright_blocked); }
            app.world_mut().run_system_once(capsule_reshape_system).unwrap();
            assert_eq!(*app.world().get::<Stance>(actor).unwrap(), if blocked { Stance::Crouching } else { Stance::Standing },
                "angle={angle} parented={parented} obstacle={obstacle}");
            let position = app.world().get::<Position>(actor).unwrap().0;
            assert!((position.y - if blocked { start.y } else { start.y + 0.34 + STAND_UP_SWEEP_INFLATION }).abs() < 1e-5);
            assert_eq!(app.world().get::<Transform>(actor).unwrap().rotation, local.rotation);
        }
    }

    #[test]
    fn grounded_uncrouch_retry_uses_gravity_orientation() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.1, 4.0), Transform::from_xyz(0.0, 1.86, 0.0)));
        app.update();
        let start = Vec3::Y * 0.5815;
        let rotation = Quat::from_rotation_x(60.0_f32.to_radians());
        app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start).with_rotation(rotation),
            Stance::Crouching, AlsDesiredStance(Stance::Standing), LocomotionState::Grounded,
            AlsCrouchTransaction::default(), als_character_capsule(0.56, 0.3)));
        app.world_mut().run_system_once(capsule_reshape_system).unwrap();
        let position = app.world().get::<Position>(actor).unwrap().0;
        assert_eq!(*app.world().get::<Stance>(actor).unwrap(), Stance::Standing);
        assert!((position.y - 0.90002).abs() < 1e-4, "position={position:?}");
        assert_eq!(app.world().get::<Transform>(actor).unwrap().rotation, rotation);
    }

    #[test]
    fn crouch_dimensions_clamp_only_the_applied_height() {
        for (configured, actual) in [(0.2_f32, 0.3_f32), (0.3, 0.3), (0.56, 0.56), (0.9, 0.9), (1.1, 1.1)] {
            let mut params = AlsMovementParams::default();
            params.crouch.crouched_capsule_half_height = configured;
            assert!((params.crouch.actual_crouched_half_height() - actual).abs() < 1e-6);
            assert_eq!(params.crouch.crouched_capsule_half_height, configured);
            assert!((capsule_for(&params, Stance::Crouching).shape_scaled().compute_local_aabb().half_extents().y - actual).abs() < 1e-6);
            assert!((capsule_center_for_stance(&params, Vec3::Y * 100.0, Stance::Standing, Stance::Crouching, 1.0).y
                - (100.0 - 0.9 + actual)).abs() < 1e-5);
            let mut config = AlsControlSchemeConfig::default();
            apply_als_control_scheme_config(&mut config, &params, Gait::Running, Stance::Crouching,
                RotationMode::ViewDirection, true, Vec3::ZERO, 0.0, 9.81, 1.0);
            assert!((config.basis.headroom.unwrap().distance_to_collider_top - actual).abs() < 1e-6);
            assert!((config.crouch.float_offset - (actual - 0.9)).abs() < 1e-6);
        }
    }

    #[test]
    fn loaded_crouch_uses_the_clamped_height_for_center_mesh_and_transaction() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let handle = app.world().get::<AlsMovementParamsHandle>(actor).unwrap().0.clone();
        for (configured, actual) in [(0.2_f32, 0.3_f32), (0.3, 0.3), (0.56, 0.56), (0.9, 0.9), (1.1, 1.1)] {
            app.world_mut().resource_mut::<Assets<AlsMovementParams>>().get_mut(&handle).unwrap()
                .crouch.crouched_capsule_half_height = configured;
            let start = Vec3::Y * 100.0;
            app.world_mut().entity_mut(actor).remove::<AlsCapsuleCrouched>().insert((Position(start), Transform::from_translation(start),
                Stance::Standing, AlsDesiredStance(Stance::Crouching), LocomotionState::Grounded,
                AlsCrouchTransaction::default(), als_character_capsule(0.9, 0.3)));
            app.world_mut().run_system_once(capsule_reshape_system).unwrap();
            app.world_mut().run_system_once(crate::character_assembly::sync_avatar_stance_offset).unwrap();
            let position = app.world().get::<Position>(actor).unwrap().0;
            let collider = app.world().get::<Collider>(actor).unwrap().shape_scaled().compute_local_aabb().half_extents();
            let transaction = app.world().get::<AlsCrouchTransaction>(actor).unwrap();
            let mesh_y = app.world().get::<Transform>(avatar).unwrap().translation.y;
            assert!((position.y - (100.0 - 0.9 + actual)).abs() < 1e-5, "configured={configured} position={position:?}");
            assert!((collider.y - actual).abs() < 1e-5 && (transaction.collider_half_height - actual).abs() < 1e-6);
            assert!((mesh_y - (crate::character_assembly::AVATAR_CAPSULE_BOTTOM_OFFSET + 0.9 - actual)).abs() < 1e-6);
            assert_eq!(app.world().resource::<Assets<AlsMovementParams>>().get(&handle).unwrap().crouch.crouched_capsule_half_height, configured);
        }
    }

    #[test]
    fn growing_crouch_rejects_blocked_clearance_and_retries_when_clear() {
        use bevy::ecs::system::RunSystemOnce;
        for (height, ceiling, angle, parented, accepted) in [
            (0.2_f32, true, 0.0_f32, false, true), (0.56, true, 0.0, false, true),
            (1.1, false, 0.0, false, true), (1.1, true, 0.0, false, false),
            (1.1, true, 60.0, false, false), (1.1, true, 60.0, true, false),
        ] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let roof = ceiling.then(|| app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.1, 4.0),
                Transform::from_xyz(0.0, 11.03, 0.0))).id());
            app.update();
            let handle = app.world().get::<AlsMovementParamsHandle>(actor).unwrap().0.clone();
            app.world_mut().resource_mut::<Assets<AlsMovementParams>>().get_mut(&handle).unwrap()
                .crouch.crouched_capsule_half_height = height;
            let start = Vec3::Y * 10.0;
            let rotation = Quat::from_rotation_x(angle.to_radians());
            let local = if parented {
                let parent = app.world_mut().spawn(Transform::from_rotation(rotation)).id();
                app.world_mut().entity_mut(actor).insert(ChildOf(parent));
                Transform::from_translation(rotation.inverse() * start)
            } else { Transform::from_translation(start).with_rotation(rotation) };
            app.world_mut().entity_mut(actor).insert((Position(start), local, Stance::Standing,
                AlsDesiredStance(Stance::Crouching), LocomotionState::Grounded, AlsCrouchTransaction::default()));
            app.world_mut().run_system_once(capsule_reshape_system).unwrap();
            let stance = *app.world().get::<Stance>(actor).unwrap();
            let position = app.world().get::<Position>(actor).unwrap().0;
            let half = app.world().get::<Collider>(actor).unwrap().shape_scaled().compute_local_aabb().half_extents().y;
            assert_eq!(stance, if accepted { Stance::Crouching } else { Stance::Standing }, "height={height} angle={angle}");
            let actual_height = if accepted { height.max(0.3) } else { 0.9 };
            assert!((half - actual_height).abs() < 1e-5, "height={height} half={half}");
            assert!((position.y - if accepted { start.y - 0.9 + actual_height } else { start.y }).abs() < 1e-5);
            assert_eq!(app.world().get::<Transform>(actor).unwrap().rotation, local.rotation);
            if !accepted {
                app.world_mut().despawn(roof.unwrap());
                app.world_mut().run_system_once(capsule_reshape_system).unwrap();
                assert_eq!(*app.world().get::<Stance>(actor).unwrap(), Stance::Crouching);
                assert!((app.world().get::<Position>(actor).unwrap().y - 10.2).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn ground_ramp_projection_uses_source_normal_and_line_conditions() {
        let delta = Vec3::X * 0.25;
        let normal = Vec3::new(-40.0_f32.to_radians().sin(), 40.0_f32.to_radians().cos(), 0.0);
        let projected = ground_ramp_movement(delta, normal, normal, false);
        assert!((projected.y - 0.25 * 40.0_f32.to_radians().tan()).abs() < 1e-6);
        assert_eq!(ground_ramp_movement(delta, normal, normal, true), delta);
        assert_eq!(ground_ramp_movement(delta, Vec3::NEG_Y, normal, false), delta);
        assert_eq!(ground_ramp_movement(delta, normal, Vec3::Y, false), delta);
        let steep = Vec3::new(-60.0_f32.to_radians().sin(), 60.0_f32.to_radians().cos(), 0.0);
        assert_eq!(ground_ramp_movement(delta, steep, steep, false), delta);
    }

    #[test]
    fn walking_sweeps_along_the_current_ramp_under_a_parallel_ceiling() {
        use bevy::ecs::system::RunSystemOnce;
        for angle in [20.0_f32, 40.0] {
            for ceiling in [false, true] {
                let mut app = App::new();
                app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
                app.finish();
                let rotation = Quat::from_rotation_z(angle.to_radians());
                let normal = rotation * Vec3::Y;
                let support = 0.3 + 0.6 * normal.y;
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 3.0),
                    Transform::from_translation(-normal * 0.1).with_rotation(rotation)));
                if ceiling {
                    let bottom = 2.0 * support + 0.0215 * normal.y + 0.012;
                    app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 3.0),
                        Transform::from_translation(normal * (bottom + 0.1)).with_rotation(rotation)));
                }
                let actor = app.world_mut().spawn_empty().id();
                app.update();
                app.update();
                let center = Vec3::Y * (support / normal.y + 0.0215);
                let result = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                    permissions: Query<&AlsMantleTargetPermission>| {
                    grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3), center,
                        Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.25,
                        &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                        actor, &candidates, &permissions, None).unwrap()
                }).unwrap();
                let movement = result.1.expect("Source ramp sweep must retain its tangent movement");
                assert!(result.5 && !result.4, "angle={angle} ceiling={ceiling} result={result:?}");
                assert!((movement.x - 0.25).abs() < 1e-5);
                assert!((movement.y - 0.25 * angle.to_radians().tan()).abs() < 1e-4,
                    "angle={angle} ceiling={ceiling} movement={movement:?}");
            }
        }
    }

    #[test]
    fn loaded_walking_commits_the_ramp_sweep_without_replaying_it_in_physics() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let angle = 40.0_f32.to_radians();
        let rotation = Quat::from_rotation_z(angle);
        let normal = rotation * Vec3::Y;
        let support = 0.3 + 0.6 * normal.y;
        let floor_center = Vec3::Y * 10.0;
        let ceiling_bottom = 2.0 * support + 0.0215 * normal.y + 0.012;
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 3.0),
            Transform::from_translation(floor_center - normal * 0.1).with_rotation(rotation)));
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 3.0),
            Transform::from_translation(floor_center + normal * (ceiling_bottom + 0.1)).with_rotation(rotation)));
        app.update();
        let start = floor_center + Vec3::Y * (support / normal.y + 0.0215);
        app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
            LocomotionState::Grounded, LinearVelocity(Vec3::X * 5.0), TnuaMotor::default(),
            AlsCmcAdapterRuntime { simulation_delta_seconds: 0.05, simulation_substeps: 1,
                grounded_movement_delta: Some(Vec3::X * 0.25), ..default() }));
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
        app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
        let actual = app.world().get::<Position>(actor).unwrap().0;
        assert!((actual.x - start.x - 0.25).abs() < 1e-4, "start={start:?} actual={actual:?}");
        assert!((actual.y - start.y - 0.25 * angle.tan()).abs() < 0.005,
            "start={start:?} actual={actual:?}");
        assert!((app.world().get::<TnuaMotor>(actor).unwrap().lin.boost.x + 5.0).abs() < 1e-4);
    }

    #[test]
    fn walking_sweeps_the_remaining_time_on_a_new_walkable_ramp() {
        use bevy::ecs::system::RunSystemOnce;
        for (angle, allows_step, wall) in [(40.0_f32, false, true), (40.0, true, true),
            (40.0, false, false), (60.0, false, true)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(-15.0, 1.9, -15.0)));
            let rotation = Quat::from_rotation_z(angle.to_radians());
            let toe = Vec3::new(-13.8, 2.0, -15.0);
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(1.2, 0.1, 2.0),
                Transform::from_translation(toe + rotation * Vec3::new(0.6, -0.05, 0.0)).with_rotation(rotation),
                AlsMantleTargetPermission { allows_step_up: allows_step }));
            if wall {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 3.0, 2.0),
                    Transform::from_xyz(-13.35, 3.5, -15.0), AlsMantleTargetPermission { allows_step_up: false }));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let start = Vec3::new(-13.98, 2.9215, -15.0);
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3), start,
                    Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.3125,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                    actor, &candidates, &permissions, None).unwrap()
            }).unwrap();
            if angle == 60.0 {
                assert!((result.0.x - -13.9711046).abs() < 0.004, "result={result:?}");
                assert!(result.1.unwrap().length() < 1e-5, "result={result:?}");
            } else if wall {
                assert!((result.0.x - -13.7078868).abs() < 0.004,
                    "allows_step={allows_step} result={result:?}");
                assert!((result.0.y - 3.0775676).abs() < 0.006,
                    "allows_step={allows_step} result={result:?}");
            } else {
                let remaining = result.1.expect("Source second sweep must retain the remaining ramp movement");
                assert!(result.5 && !result.4, "result={result:?}");
                assert!((result.0.x + remaining.x - (start.x + 0.3125)).abs() < 0.001, "result={result:?}");
                assert!(remaining.y > 0.1, "result={result:?}");
            }
        }
    }

    #[test]
    fn loaded_walking_reaches_the_witness_wall_after_the_new_ramp() {
        use bevy::ecs::system::RunSystemOnce;
        for allows_step in [false, true] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(-15.0, 1.9, -15.0)));
            let rotation = Quat::from_rotation_z(40.0_f32.to_radians());
            let toe = Vec3::new(-13.8, 2.0, -15.0);
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(1.2, 0.1, 2.0),
                Transform::from_translation(toe + rotation * Vec3::new(0.6, -0.05, 0.0)).with_rotation(rotation),
                AlsMantleTargetPermission { allows_step_up: allows_step }));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 3.0, 2.0),
                Transform::from_xyz(-13.35, 3.5, -15.0), AlsMantleTargetPermission { allows_step_up: false }));
            app.update();
            let start = Vec3::new(-13.98, 2.9215, -15.0);
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LocomotionState::Grounded, LinearVelocity(Vec3::X * 6.25), TnuaMotor::default(),
                AlsCmcAdapterRuntime { simulation_delta_seconds: 0.05, simulation_substeps: 1,
                    grounded_movement_delta: Some(Vec3::X * 0.3125), ..default() }));
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            let position = app.world().get::<Position>(actor).unwrap().0;
            assert!((position.x - -13.7078868).abs() < 0.004, "allows_step={allows_step} position={position:?}");
            assert!((position.y - 3.0775676).abs() < 0.006, "allows_step={allows_step} position={position:?}");
            app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
            let adjusted = app.world().get::<Position>(actor).unwrap().0;
            assert!((adjusted.y - 3.0904143).abs() < 0.006, "allows_step={allows_step} position={adjusted:?}");
        }
    }

    #[test]
    fn walking_respects_character_step_permission_for_root_and_child_colliders() {
        use bevy::ecs::system::RunSystemOnce;
        for case in ["character", "child", "prop", "override"] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let far = Vec3::new(10.0, 0.9215, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(far), Transform::from_translation(far)));
            let target_position = Vec3::new(0.0, 0.18, -0.8);
            if case == "character" || case == "override" {
                let params = app.world().get::<AlsMovementParamsHandle>(actor).unwrap().0.clone();
                let config = app.world().get::<TnuaConfig<AlsControlScheme>>(actor).unwrap().0.clone();
                let mut bundle = crate::als::character::AlsCharacterBundle::new(params, config);
                bundle.rigid_body = RigidBody::Static;
                bundle.transform = Transform::from_translation(target_position).with_scale(Vec3::splat(0.2));
                let target = app.world_mut().spawn(bundle).id();
                if case == "override" {
                    app.world_mut().entity_mut(target).insert(AlsMantleTargetPermission { allows_step_up: true });
                }
            } else if case == "child" {
                let target = app.world_mut().spawn((AlsCharacter, RigidBody::Static, Transform::from_translation(target_position))).id();
                app.world_mut().spawn((ChildOf(target), Collider::capsule(0.06, 0.24), Transform::IDENTITY));
            } else {
                app.world_mut().spawn((RigidBody::Static, Collider::capsule(0.06, 0.24), Transform::from_translation(target_position)));
            }
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3), Vec3::Y * 0.9215,
                    Quat::IDENTITY, 0.9, 0.3, Vec3::NEG_Z,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                    actor, &candidates, &permissions, None).unwrap()
            }).unwrap();
            assert_eq!(result.4, matches!(case, "prop" | "override"), "case={case} result={result:?}");
        }
    }

    #[test]
    fn rejected_step_sweeps_the_first_slide_before_each_walking_substep() {
        use bevy::ecs::system::RunSystemOnce;
        for (upper_hit, second_wall) in [(false, true), (true, true), (false, false)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(0.0, -0.1, 0.0)));
            let wall = if upper_hit {
                (Vec3::new(0.1, 0.2, 4.0), Vec3::new(0.42, 1.8, 0.0))
            } else {
                (Vec3::new(0.1, 2.0, 4.0), Vec3::new(0.48, 1.0, 0.0))
            };
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(wall.0.x, wall.0.y, wall.0.z),
                Transform::from_translation(wall.1), AlsMantleTargetPermission { allows_step_up: upper_hit }));
            if second_wall {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 2.0, 0.1),
                    Transform::from_xyz(0.0, 1.0, -0.53), AlsMantleTargetPermission { allows_step_up: false }));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let positions = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                let mut center = Vec3::new(0.12, 0.9215, -0.155);
                let mut positions = Vec::new();
                for _ in 0..2 {
                    let result = grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3), center,
                        Quat::IDENTITY, 0.9, 0.3, Vec3::new(0.3, 0.0, -0.3),
                        &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                        actor, &candidates, &permissions, None).unwrap();
                    assert!(result.5 && !result.4, "upper={upper_hit} result={result:?}");
                    center = result.0 + result.1.unwrap();
                    positions.push(center);
                }
                positions
            }).unwrap();
            if second_wall {
                assert!(positions.iter().all(|position| position.z >= -0.1801),
                    "upper={upper_hit} positions={positions:?}");
            } else {
                assert!(positions[0].z < -0.3, "unblocked slide was shortened: {positions:?}");
            }
        }
    }

    #[test]
    fn loaded_walking_substeps_do_not_enter_the_second_wall() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let far = Vec3::new(10.0, 0.9215, 0.0);
        app.world_mut().entity_mut(actor).insert((Position(far), Transform::from_translation(far)));
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 2.0, 4.0),
            Transform::from_xyz(0.48, 1.0, 0.0), AlsMantleTargetPermission { allows_step_up: false }));
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 2.0, 0.1),
            Transform::from_xyz(0.0, 1.0, -0.53), AlsMantleTargetPermission { allows_step_up: false }));
        app.update();
        let start = Vec3::new(0.12, 0.9215, -0.155);
        app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
            LocomotionState::Grounded, LinearVelocity(Vec3::new(3.0, 0.0, -3.0)), TnuaMotor::default(),
            AlsCmcAdapterRuntime { simulation_delta_seconds: 0.1, simulation_substeps: 2,
                grounded_movement_delta: Some(Vec3::new(0.3, 0.0, -0.3)), ..default() }));
        app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion =
            Vec3::new(1.0, 0.0, -1.0).normalize();
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.1));
        app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
        let position = app.world().get::<Position>(actor).unwrap().0;
        assert!(position.x <= 0.1301 && position.z >= -0.1801, "position={position:?}");
    }

    #[test]
    fn ground_line_rejects_a_ray_starting_inside_the_floor() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let (inside, normal, missing, solid, exit) = app.world_mut().run_system_once(move |spatial: SpatialQuery,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
            let filter = SpatialQueryFilter::from_excluded_entities([actor]);
            let cmc = crate::als::params::CmcAdapterParams::default();
            let floor = |origin| find_ground_floor_with_penetration(&spatial, origin, 0.9, 0.3, &cmc,
                LocomotionState::Grounded, &filter, actor, &candidates, None, None);
            let origin = Vec3::NEG_Y * 0.2;
            let solid = spatial.cast_ray(origin, Dir3::NEG_Y, 2.0, true, &filter).unwrap();
            let exit = spatial.cast_ray(origin, Dir3::NEG_Y, 2.0, false, &filter).unwrap();
            (floor(origin), floor(Vec3::Y * 0.9215), floor(Vec3::new(200.0, 0.9215, 0.0)), solid, exit)
        }).unwrap();
        assert_eq!(solid.distance, 0.0);
        assert!(exit.distance > 0.0 && exit.normal.y > 0.99);
        assert!(inside.is_none(), "An exit surface cannot be a source floor: {inside:?}");
        assert!(normal.is_some() && missing.is_none());
    }

    #[test]
    fn rotated_root_floor_recheck_uses_the_stored_radius() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let start = Vec3::new(-14.041662, 0.924094, -15.000024);
        let rotation = Quat::from_xyzw(0.0, -0.6227078437805176, 0.0, 0.7824543714523315);
        let local = Transform::from_translation(start).with_rotation(rotation);
        app.world_mut().entity_mut(actor).insert((Position(start), local, LocomotionState::Grounded,
            AlsCmcAdapterRuntime::default()));
        let handle = app.world().get::<AlsMovementParamsHandle>(actor).unwrap().0.clone();
        let params = app.world().resource::<Assets<AlsMovementParams>>().get(&handle).unwrap().clone();
        let center = app.world_mut().run_system_once(move |spatial: SpatialQuery,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
            find_ground_floor(&spatial, start, params.crouch.standing_capsule_half_height,
                params.crouch.standing_capsule_radius, &params.cmc, LocomotionState::Grounded,
                &SpatialQueryFilter::from_excluded_entities([actor]), actor, &candidates).unwrap().0
        }).unwrap();
        let distance = start.y - center;
        let expected = if (MIN_FLOOR_DISTANCE..=MAX_FLOOR_DISTANCE).contains(&distance) { start.y }
            else { center + (MIN_FLOOR_DISTANCE + MAX_FLOOR_DISTANCE) * 0.5 };
        app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
        assert_eq!(app.world().get::<Position>(actor).unwrap().y, expected);
    }

    #[test]
    fn free_walking_preserves_the_valid_floor_distance_band() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
        app.finish();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(10.0, 0.2, 10.0), Transform::from_xyz(0.0, -0.1, 0.0)));
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        app.update();
        for (gap, comparison) in [(0.015, 1), (0.020, 0), (0.0215, 0), (0.023, 0), (0.031, -1)] {
            let center = Vec3::new(0.0, 0.9 + gap, 0.0);
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3), center,
                    Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.1, &crate::als::params::CmcAdapterParams::default(),
                    LocomotionState::Grounded, actor, &candidates, &permissions, None).unwrap()
            }).unwrap();
            assert!(!result.4);
            let height = result.2.unwrap();
            if comparison == 0 { assert_eq!(height, center.y, "gap={gap}"); }
            else if comparison > 0 { assert!(height > center.y, "gap={gap}"); }
            else { assert!(height < center.y, "gap={gap}"); }
        }
    }

    #[test]
    fn free_walking_keeps_the_planned_velocity_before_physics() {
        use bevy::ecs::system::RunSystemOnce;
        for (wall, dilation) in [(false, 1.0_f32), (true, 1.0), (false, 0.5)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            if wall {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.04, 3.0, 5.0),
                    Transform::from_xyz(0.36, 1.5, 0.0)));
                app.update();
                app.update();
            }
            let center = Vec3::new(0.0, 0.9215, 0.0);
            let dt = 1.0_f32 / 30.0;
            app.world_mut().entity_mut(actor).insert((Position(center), Transform::from_translation(center),
                LinearVelocity(Vec3::X * 3.75), LocomotionState::Grounded, TnuaMotor::default(),
                crate::als_camera::AlsCameraCustomTimeDilation(dilation),
                AlsCmcAdapterRuntime { simulation_substeps: 1, simulation_delta_seconds: dt * dilation,
                    grounded_movement_delta: Some(Vec3::X * (3.75 * dt * dilation)), ..default() },
                crate::als::dynamics::AlsMovementBaseDynamicsState {
                    pre_collision_velocity: Some(Vec3::X * 3.75), ..default() }));
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(dt));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            let boost = app.world().get::<TnuaMotor>(actor).unwrap().lin.boost.x;
            if wall { assert!(boost < -1.0); }
            else if dilation == 1.0 { assert_eq!(boost, 0.0); }
            else { assert!((boost + 1.875).abs() < 1e-6); }
        }
    }

    #[test]
    fn landing_roll_that_leaves_ground_interrupts_before_remaining_falling() {
        use bevy::ecs::system::RunSystemOnce;
        for interrupt in [false, true] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.6, 0.1), Transform::from_xyz(5.0, 0.3, 0.0)));
            app.update();
            let handle = app.world().get::<AlsMovementParamsHandle>(actor).unwrap().0.clone();
            app.world_mut().resource_mut::<Assets<AlsMovementParams>>().get_mut(&handle).unwrap().rolling.interrupt_when_in_air = interrupt;
            let start = Vec3::new(5.0, 1.6, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LinearVelocity(Vec3::new(0.0, -8.0, -3.75)), LocomotionState::Air,
                AlsStateRequest::default(), TnuaMotor::default()));
            app.world_mut().get_mut::<crate::actual_motion::AlsActualMotion>(actor).unwrap().world_velocity = Vec3::new(0.0, -8.0, -3.75);
            app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::NEG_Z;
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.2));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            assert_eq!(app.world().get::<LocomotionState>(actor), Some(&if interrupt { LocomotionState::None } else { LocomotionState::Air }));
            if interrupt {
                assert_eq!(app.world().get::<crate::als::state::AlsNotifyLocomotionAction>(actor),
                    Some(&crate::als::state::AlsNotifyLocomotionAction::Ragdolling));
                assert_eq!(app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().grounded_collision_velocity.unwrap().y, 0.0,
                    "Source Roll interruption must stop the remaining Falling budget");
            }
        }
    }

    #[test]
    fn landing_callback_arbitration_precedes_remaining_walking() {
        use bevy::ecs::system::RunSystemOnce;
        for (vertical, getting_up, expected_travel) in [(-3.0, false, true), (-8.0, true, true), (-11.0, false, false)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let start = Vec3::Y;
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LinearVelocity(Vec3::new(0.0, vertical, -3.75)), LocomotionState::Air,
                AlsStateRequest::default(), TnuaMotor::default()));
            if getting_up { app.world_mut().entity_mut(actor).insert(crate::als::state::AlsNotifyLocomotionAction::GettingUp); }
            {
                let mut motion = app.world_mut().get_mut::<crate::actual_motion::AlsActualMotion>(actor).unwrap();
                motion.world_velocity = Vec3::new(0.0, vertical, -3.75);
                motion.has_input = true;
            }
            app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::NEG_Z;
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.2));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            let travel = -app.world().get::<Position>(actor).unwrap().z;
            if expected_travel { assert!((travel - 0.75).abs() < 0.001, "Rejected Roll must retain the source walking budget: {travel}"); }
            else {
                assert!(travel < 0.1, "Ragdoll landing must stop before remaining Walking: {travel}");
                assert_eq!(app.world().get::<LinearVelocity>(actor).unwrap().0, Vec3::ZERO);
                let motor = app.world().get::<TnuaMotor>(actor).unwrap();
                assert_eq!(motor.lin.boost, Vec3::ZERO);
                assert_eq!(motor.lin.acceleration, Vec3::ZERO);
            }
        }
    }

    #[test]
    fn automatic_landing_and_air_roll_require_ragdoll_bodies() {
        use bevy::ecs::system::RunSystemOnce;
        for (landing, missing) in [(true, None), (true, Some("pelvis")),
            (false, None), (false, Some("spine_03"))] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let (_, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
            if let Some(label) = missing { crate::ragdoll_link::tests::remove_ragdoll_body(&mut app, &bodies, label); }
            if landing {
                let start = Vec3::Y;
                app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                    LinearVelocity(Vec3::new(0.0, -11.0, -3.75)), LocomotionState::Air,
                    AlsStateRequest::default(), TnuaMotor::default(), crate::als::state::AlsNotifyLocomotionAction::None));
                app.world_mut().get_mut::<crate::actual_motion::AlsActualMotion>(actor).unwrap().world_velocity = Vec3::new(0.0, -11.0, -3.75);
                app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.2));
                app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            } else {
                let handle = app.world().get::<AlsMovementParamsHandle>(actor).unwrap().0.clone();
                app.world_mut().resource_mut::<Assets<AlsMovementParams>>().get_mut(&handle).unwrap().rolling.interrupt_when_in_air = true;
                let start = Vec3::Y * 10.0;
                app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                    LocomotionState::Grounded, AlsCmcAdapterRuntime::default(), crate::als::state::AlsNotifyLocomotionAction::Rolling));
                app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
            }
            let expected = if missing.is_none() { LocomotionState::None }
                else if landing { LocomotionState::Grounded } else { LocomotionState::Air };
            assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), expected,
                "landing={landing} missing={missing:?}");
            assert_eq!(app.world().get::<crate::als::state::AlsNotifyLocomotionAction>(actor)
                == Some(&crate::als::state::AlsNotifyLocomotionAction::Ragdolling), missing.is_none());
        }
    }

    #[test]
    fn frozen_roll_collision_prepass_consumes_the_owner_clock() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(1.0, 0.4, 0.6), Transform::from_xyz(0.0, 0.2, -0.615)));
        app.update();
        let before = app.world().get::<Position>(actor).unwrap().0;
        app.world_mut().entity_mut(actor).insert((LinearVelocity(Vec3::NEG_Z * 3.75),
            crate::als_camera::AlsCameraCustomTimeDilation(0.0), bevy_tnua::TnuaMotor::default(),
            crate::als::roll::AlsRollState { active: true, time: 0.3, previous_time: 0.3, ..default() },
            AlsCmcAdapterRuntime::default()));
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.1));
        app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
        let actual = app.world().get::<Position>(actor).unwrap().0;
        assert!(actual.abs_diff_eq(before, 1e-5),
            "Source frozen Root collision prepass consumes no displacement: before={before:?} actual={actual:?}");
    }

    #[test]
    fn root_motion_collision_prepass_bridges_source_velocity_to_the_world_frame() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        for dilation in [0.0, 0.5, 1.0, 2.0] {
            app.world_mut().entity_mut(actor).insert((
                LinearVelocity(Vec3::new(0.0, 0.0, -2.0)), LocomotionState::Grounded,
                crate::als_camera::AlsCameraCustomTimeDilation(dilation),
                bevy_tnua::TnuaMotor::default(),
                crate::als::roll::AlsRollState { active: true, time: 0.3, previous_time: 0.3, ..default() },
                AlsCmcAdapterRuntime { simulation_delta_seconds: 0.025, ..default() }));
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            assert_eq!(app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().simulation_delta_seconds, 0.05);
            assert!((app.world().get::<bevy_tnua::TnuaMotor>(actor).unwrap().lin.boost.z - 2.0 * (1.0 - dilation)).abs() < 0.001,
                "Source Root velocity must consume the owner clock before its world physics bridge");
        }
    }

    #[test]
    fn falling_transaction_defers_to_the_active_animation_root_motion_owner() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let position = Vec3::Y * 10.0;
        app.world_mut().entity_mut(actor).insert((Position(position), Transform::from_translation(position),
            LinearVelocity(Vec3::new(1.0, -2.0, 0.0)), LocomotionState::Air,
            crate::als::roll::AlsRollState { active: true, time: 0.3, previous_time: 0.3, ..default() },
            AlsCmcAdapterRuntime::default()));
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
        app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
        assert_eq!(app.world().get::<Position>(actor).unwrap().0, position,
            "Ordinary falling movement replaced an active animation root motion owner");
        assert!(app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().grounded_collision_velocity.is_none());
    }

    #[test]
    fn falling_transaction_consumes_the_actor_terminal_velocity() {
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let start = Vec3::Y * 10.0;
        app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
            LinearVelocity(Vec3::NEG_Y * 1.9), LocomotionState::Air,
            AlsStateRequest { locomotion_state: Some(LocomotionState::Air), ..default() },
            crate::als::dynamics::AlsPhysicsVolume { terminal_velocity: 2.0 }, AlsInputCommand::default()));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.1)));
        app.update();
        let actual = app.world().get::<Position>(actor).unwrap().y - start.y;
        let velocity = app.world().get::<LinearVelocity>(actor).unwrap().y;
        let expected = (-1.9 - 2.0) * 0.5 * 0.05 - 2.0 * 0.05;
        assert!((actual - expected).abs() < 1e-4 && (velocity + 2.0).abs() < 1e-4,
            "Source falling transaction ignored the actor physics volume: position={actual} velocity={velocity} expected={expected}");
    }

    use super::*;

    #[test]
    fn falling_rotated_wall_consumes_native_capsule_sweep_normal() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
            .add_message::<StateTransitionEvent>();
        app.finish();
        let wall = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.04, 4.0, 2.0),
            Transform::from_xyz(-13.38, 3.0, -15.0)
                .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4)))).id();
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        app.update();
        let (hit, result) = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
            shapes: Query<(&Collider, &Position, &Rotation)>| {
            let capsule = als_character_capsule(0.9, 0.3);
            let start = Vec3::new(-13.7582617, 2.9012036, -15.0849314);
            let movement = Vec3::new(0.24020796, -0.0073499945, -4.47101e-9);
            let velocity = Vec3::new(6.2309957, -0.37974584, -1.2050123e-7);
            let (direction, distance) = Dir3::new_and_length(movement).unwrap();
            let hit = spatial.cast_shape(&capsule, start, Quat::IDENTITY, direction,
                &ShapeCastConfig { ignore_origin_penetration: true, ..ShapeCastConfig::from_max_distance(distance) },
                &SpatialQueryFilter::from_excluded_entities([actor]));
            let result = falling_movement_sweep(&spatial, &capsule, start, Quat::IDENTITY, movement,
                velocity, 0.038710073, None, None, actor, &candidates, &shapes, &impact_bodies, None);
            (hit, result)
        }).unwrap();
        let hit = hit.unwrap();
        assert_eq!(hit.entity, wall);
        assert!((hit.distance - 0.010649537).abs() <= 1e-6);
        let source_normal = Vec3::new(-std::f32::consts::FRAC_1_SQRT_2, 0.0, -std::f32::consts::FRAC_1_SQRT_2);
        assert!(hit.normal1.abs_diff_eq(source_normal, 1e-6), "Wall normal differs: {:?}", hit.normal1);
        let expected = Vec3::new(3.1154974, -0.37974584, -3.1154983);
        assert!(result.1.abs_diff_eq(expected, 1e-5), "Falling slide differs: {:?}", result.1);
    }

    #[test]
    fn falling_edge_rechecks_floor_at_the_impact_before_returning_time() {
        use bevy::ecs::system::RunSystemOnce;
        for (wall_top, deck_x, deck_top, lands) in [
            (2.16, -12.7, 1.973, true), (2.16, -12.52, 1.973, false),
            (2.16, -12.36, 1.973, false), (2.215, -12.7, 1.973, false),
            (2.215, -12.7, 1.978, true), (2.29, -12.7, 1.973, false),
        ] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .add_message::<StateTransitionEvent>();
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.0, 0.2, 3.0), Transform::from_xyz(-15.0, 1.9, -15.0)));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.0, 0.2, 1.3), Transform::from_xyz(deck_x, deck_top - 0.1, -15.3)));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.04, wall_top - 1.0, 2.0),
                Transform::from_xyz(-13.38, (wall_top + 1.0) * 0.5, -15.0)
                    .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4))));
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                shapes: Query<(&Collider, &Position, &Rotation)>| {
                let cmc = crate::als::params::CmcAdapterParams::default();
                let mut seed = 123;
                falling_movement_sweep(&spatial, &als_character_capsule(0.9, 0.3),
                    Vec3::new(-13.7582617, 2.9012036, -15.0849314), Quat::IDENTITY,
                    Vec3::new(0.24020796, -0.00735, 0.0), Vec3::new(6.230996, -0.379746, 0.0),
                    0.03871007, None, Some((&cmc, 4.2, 6.5, &mut seed)), actor, &candidates, &shapes, &impact_bodies, None)
            }).unwrap();
            assert_eq!(result.3.is_some(), lands, "wall={wall_top} deck={deck_x}/{deck_top} result={result:?}");
            if lands {
                assert!(result.3.unwrap().0 > 0.02, "the walking budget must be returned: {result:?}");
            } else if wall_top != 2.29 {
                assert_eq!(result.4, Some(LocomotionState::Air), "a later floor query must not replace the stop result");
            }
            if wall_top == 2.16 && deck_x == -12.7 {
                let frame = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                    permissions: Query<&AlsMantleTargetPermission>,
                    bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
                    owners: Query<&ColliderOf>, shapes: Query<(&Collider, &Position, &Rotation)>,
                    mut transitions: MessageWriter<StateTransitionEvent>| {
                    let mut params = AlsMovementParams::default();
                    params.acceleration.max_acceleration = 8.847457;
                    let mut seed = 123;
                    falling_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                        Transform::from_xyz(-13.7582617, 2.9012036, -15.0849314), Vec3::X * 6.1796227,
                        Vec3::X, 6.5, &params, Vec3::NEG_Y * 9.81,
                        (Stance::Standing, RotationMode::ViewDirection, Gait::Running, 0.0),
                        (0.0, false, false), 40.0, 0.03871007, 0, actor, &mut seed,
                        &candidates, &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, None, None)
                }).unwrap();
                assert_eq!(frame.4, LocomotionState::Grounded);
                assert!(frame.0.translation.x > -13.57, "the returned time did not advance Walking: {:?}", frame.0);
            }
        }
    }

    #[test]
    fn box_impact_and_edge_qualification_follow_source_face_rules() {
        let rotation = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4);
        let normal = Vec3::new(-0.6260943, 0.4647709, -0.6260943);
        let impact = box_impact_normal(normal, Vec3::new(0.24, -0.007, 0.0), rotation);
        assert!(impact.distance(rotation * Vec3::NEG_X) < 1e-6);
        let point = Vec3::new(0.1, -0.5, 0.0);
        assert_eq!(landing_hit_qualification(normal, impact, point, Vec3::ZERO, Vec3::ZERO, 0.9, 0.3, false), (false, true));
        assert_eq!(landing_hit_qualification(normal, impact, Vec3::new(0.299, -0.5, 0.0),
            Vec3::ZERO, Vec3::ZERO, 0.9, 0.3, false), (false, false));
        let shallow = Vec3::new(-1.0, 0.000052, 0.0).normalize();
        assert_eq!(landing_hit_qualification(shallow, shallow, point, Vec3::ZERO, Vec3::ZERO, 0.9, 0.3, false), (false, false));
        assert_eq!(landing_hit_qualification(Vec3::Y, Vec3::Y, Vec3::NEG_Y * 0.9,
            Vec3::ZERO, Vec3::ZERO, 0.9, 0.3, false), (true, false));
    }

    #[test]
    fn impact_face_selection_covers_convex_mesh_heightfield_and_compound() {
        let points: Vec<_> = [-0.5, 0.5].into_iter().flat_map(|x| {
            [-0.5, 0.5].into_iter().flat_map(move |y| [-0.5, 0.5].map(move |z| Vec3::new(x, y, z)))
        }).collect();
        let convex = Collider::convex_hull(points).unwrap();
        let mesh = Collider::trimesh(vec![
            Vec3::new(-0.5, -0.5, -0.5), Vec3::new(-0.5, -0.5, 0.5),
            Vec3::new(-0.5, 0.5, 0.5), Vec3::new(-0.5, 0.5, -0.5), Vec3::new(0.5, 0.5, 0.5),
        ], vec![[0, 1, 2], [0, 2, 3], [3, 2, 4]]);
        let compound = Collider::compound(vec![(Vec3::ZERO, Quat::IDENTITY, Collider::cuboid(1.0, 1.0, 1.0))]);
        let capsule = als_character_capsule(0.9, 0.3);
        let normal = Vec3::new(-1.0, 1.0, 0.0).normalize();
        for shape in [&convex, &mesh, &compound] {
            let impact = shape_impact_normal(shape.shape_scaled().as_ref(), avian3d::parry::math::Pose3::IDENTITY,
                Vec3::new(-0.5, 0.5, 0.0), normal, Vec3::new(2.0, -0.4, 0.0),
                Vec3::new(-2.0, 1.0, 0.0), Quat::IDENTITY, &capsule);
            assert!(impact.distance(Vec3::NEG_X) < 1e-5, "shape={shape:?} impact={impact:?}");
        }
        let heightfield = Collider::heightfield(vec![vec![0.0, 0.0], vec![0.0, 0.0]], Vec3::splat(2.0));
        let impact = shape_impact_normal(heightfield.shape_scaled().as_ref(), avian3d::parry::math::Pose3::IDENTITY,
            Vec3::ZERO, Vec3::new(0.2, 0.98, 0.0).normalize(), Vec3::NEG_Y,
            Vec3::Y * 2.0, Quat::IDENTITY, &capsule);
        assert!(impact.distance(Vec3::Y) < 1e-5, "heightfield impact={impact:?}");
        let sphere = Collider::sphere(0.5);
        assert_eq!(shape_impact_normal(sphere.shape_scaled().as_ref(), avian3d::parry::math::Pose3::IDENTITY,
            Vec3::NEG_X * 0.5, normal, Vec3::X, Vec3::NEG_X * 2.0, Quat::IDENTITY, &capsule), normal);
    }

    #[test]
    fn floor_sweep_uses_convex_impact_face_at_a_steep_edge() {
        use bevy::ecs::system::RunSystemOnce;
        for angle in [30.0_f32, 60.0] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            let slope = angle.to_radians().tan();
            let mut points = Vec::new();
            for z in [-1.0_f32, 1.0] {
                points.extend([Vec3::new(-1.0, 0.08 - slope, z), Vec3::new(0.0, 0.08, z),
                    Vec3::new(-1.0, -3.0, z), Vec3::new(0.0, -3.0, z)]);
            }
            app.world_mut().spawn((RigidBody::Static, Collider::convex_hull(points).unwrap(), Transform::IDENTITY));
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let (hit, impact, overlap_impact, floor, line_hit) = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
                let origin = Vec3::new(0.1, 1.02, 0.0);
                let filter = SpatialQueryFilter::from_excluded_entities([actor]);
                let shrink = (0.9 - 0.3) * FLOOR_SWEEP_SHRINK_FRACTION;
                let capsule = als_character_capsule(0.9 - shrink, 0.3);
                let (hit, impact) = floor_sweep_hit(&spatial, &capsule, 0.3, origin, 1.0, &filter, &|_| true).unwrap();
                let overlap = ShapeHitData { distance: 0.0, ..hit };
                let overlap_impact = source_sweep_impact_normal(&spatial, &overlap, Vec3::NEG_Y, origin, Quat::IDENTITY, &capsule);
                let floor = find_ground_floor_with_penetration(&spatial, origin, 0.9, 0.3,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                    &filter, actor, &candidates, None, None);
                let line_hit = spatial.cast_ray(origin, Dir3::NEG_Y, 2.0, false, &filter).is_some();
                (hit, impact, overlap_impact, floor, line_hit)
            }).unwrap();
            assert!(!line_hit);
            assert!(hit.distance > 0.0 && hit.normal1.y > deg_to_rad(MAX_SLOPE_DEGREES).cos());
            assert!((impact.y - angle.to_radians().cos()).abs() < 1e-5, "angle={angle} impact={impact:?}");
            assert_eq!(overlap_impact, hit.normal1);
            assert_eq!(floor.is_some(), angle == 30.0, "angle={angle} hit={hit:?} impact={impact:?}");
        }
    }

    #[test]
    fn falling_wall_contact_requires_the_source_edge_qualification() {
        use bevy::ecs::system::RunSystemOnce;
        for (top, pitch, expected) in [(2.16, 0.0_f32, true), (2.29, 0.0, false),
            (5.0, -0.003, false), (5.0, -0.03, false)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            let bottom = 1.0;
            let rotation = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4) * Quat::from_rotation_z(pitch.to_radians());
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.04, top - bottom, 2.0),
                Transform::from_xyz(-13.38, (top + bottom) * 0.5, -15.0).with_rotation(rotation)));
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let contact = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                shapes: Query<(&Collider, &Position, &Rotation)>| {
                falling_movement_sweep(&spatial, &als_character_capsule(0.9, 0.3),
                    Vec3::new(-13.7582617, 2.9012036, -15.0849314), Quat::IDENTITY,
                    Vec3::new(0.24020796, -0.00735, 0.0), Vec3::new(6.230996, -0.379746, 0.0),
                    0.03871007, None, None, actor, &candidates, &shapes, &impact_bodies, None).2
            }).unwrap();
            assert_eq!(contact, expected, "top={top} pitch={pitch}");
        }
    }

    #[test]
    fn line_floor_keeps_the_wide_hit_and_does_not_publish_a_wall_landing() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_tnua::{TnuaBasis, TnuaBasisContext, TnuaProximitySensor, TnuaRigidBodyTracker};
        for (deck_x, blocker, expected_origin) in [
            (-12.61, true, GroundFloorOrigin::PerchLine), (-12.61, false, GroundFloorOrigin::Sweep),
            (-12.76, true, GroundFloorOrigin::Line), (-12.76, false, GroundFloorOrigin::Sweep),
        ] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let far = Vec3::new(20.0, 10.0, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(far), Transform::from_translation(far), LocomotionState::Air));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.0, 0.2, 3.0), Transform::from_xyz(-15.0, 1.9, -15.0)));
            let deck = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.0, 0.2, 1.3),
                Transform::from_xyz(deck_x, 1.878, -15.3))).id();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.04, 1.205, 2.0),
                Transform::from_xyz(-13.38, 1.6025, -15.0).with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4))));
            if blocker {
                let rotation = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4) * Quat::from_rotation_z(-std::f32::consts::FRAC_PI_4);
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.14, 0.03, 1.6),
                    Transform::from_xyz(-13.9164686, 2.077465, -15.2431383).with_rotation(rotation)));
            }
            app.update();
            app.update();
            let position = Vec3::new(-13.6265402, 2.9005203, -15.1943197);
            let floor = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
                find_ground_floor_with_penetration(&spatial, position, 0.9, 0.3,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Air,
                    &SpatialQueryFilter::from_excluded_entities([actor]), actor, &candidates, None, None)
            }).unwrap().unwrap();
            if blocker {
                assert_eq!(floor.origin, expected_origin);
                assert_eq!(floor.hit.entity, deck);
                assert!((floor.hit.point1.y - 2.07356).abs() < 0.002, "the wide impact must be retained: {floor:?}");
                assert!((floor.hit.distance - 0.0771).abs() < 0.002 && (floor.hit.point1.x + 13.778).abs() < 0.002);
                assert!((floor.sweep_distance - 0.0171).abs() < 0.002);
                let expected_line_distance = if expected_origin == GroundFloorOrigin::Line { 0.02252 } else { MIN_FLOOR_DISTANCE };
                assert!((floor.line_distance.unwrap() - expected_line_distance).abs() < 1e-4);
            } else {
                assert_eq!(floor.origin, expected_origin);
            }
            let params = AlsMovementParams::default();
            let config = initial_als_control_scheme_config(&params).basis;
            let mut controller = TnuaController::<AlsControlScheme>::default();
            controller.basis.apply(&config, &mut controller.basis_memory,
                &bevy_tnua::builtins::TnuaBuiltinWalkSensors { ground: &TnuaProximitySensor::default(), headroom: None },
                TnuaBasisContext { frame_duration: 0.05,
                    tracker: &TnuaRigidBodyTracker { gravity: Vec3::NEG_Y * 9.81, ..default() }, up_direction: Dir3::Y },
                &mut TnuaMotor::default());
            app.world_mut().entity_mut(actor).insert((Position(position), Transform::from_translation(position),
                LocomotionState::Air, AlsStateRequest::default(), controller));
            app.world_mut().get_mut::<AlsCmcAdapterRuntime>(actor).unwrap().airborne_sweep_contact = true;
            app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
            assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(),
                if blocker { LocomotionState::Air } else { LocomotionState::Grounded });
        }
    }

    #[test]
    fn step_down_commits_a_miss_without_replaying_the_forward_move() {
        use bevy::ecs::system::RunSystemOnce;
        for width in [2.0, 2.9] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(width, 0.2, 3.0), Transform::from_xyz(-15.0, 1.9, -15.0)));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.02, 0.2, 0.02), Transform::from_xyz(-13.855, 2.08, -14.73)));
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Vec3::new(-13.98, 2.9215, -15.0), Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.3125,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded, actor, &candidates, &permissions, None)
            }).unwrap().unwrap();
            assert!(result.4 && result.2.is_some(), "width={width} result={result:?}");
            let end = (result.0 + result.1.unwrap()).with_y(result.2.unwrap());
            assert!((end.x + 13.6675).abs() < 0.001 && (end.z + 15.0).abs() < 0.001);
            if width == 2.0 {
                assert!((end.y - 2.8522).abs() < 0.003, "the miss must consume the full down sweep: {end:?}");
            } else {
                assert!((end.y - 2.9105).abs() < 0.003, "the supported step must stop at the floor: {end:?}");
            }
        }
    }

    #[test]
    fn successful_step_commits_forward_motion_without_replaying_it_in_the_motor() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let far = Vec3::new(10.0, 0.9215, 0.0);
        app.world_mut().entity_mut(actor).insert((Position(far), Transform::from_translation(far)));
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.9, 0.2, 3.0), Transform::from_xyz(-15.0, 1.9, -15.0)));
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.02, 0.2, 0.02), Transform::from_xyz(-13.855, 2.08, -14.73)));
        app.update();
        let start = Vec3::new(-13.98, 2.9215, -15.0);
        app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
            LinearVelocity(Vec3::X * 6.5), LocomotionState::Grounded, TnuaMotor::default()));
        {
            let mut base = app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBaseDynamicsState>(actor).unwrap();
            base.support_velocity = Vec3::ZERO;
        }
        {
            let mut runtime = app.world_mut().get_mut::<AlsCmcAdapterRuntime>(actor).unwrap();
            runtime.simulation_substeps = 1;
            runtime.simulation_delta_seconds = 0.05;
            runtime.grounded_movement_delta = Some(Vec3::X * 0.3125);
        }
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
        app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
        let position = app.world().get::<Position>(actor).unwrap().0;
        let motor = app.world().get::<TnuaMotor>(actor).unwrap();
        let runtime = app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap();
        assert!((position.x - (start.x + 0.3125)).abs() < 0.001 && (position.z - start.z).abs() < 1e-5,
            "the StepUp transaction did not commit the raised forward sweep: {position:?}");
        assert!((motor.lin.boost.x + 6.5).abs() < 1e-5 && motor.lin.boost.z.abs() < 1e-5);
        assert!(runtime.grounded_movement_delta.is_none(), "completed step movement must not be replayed");
    }

    #[test]
    fn step_down_preserves_the_swept_gap_before_floor_adjustment() {
        use bevy::ecs::system::RunSystemOnce;
        for roof_bottom in [4.256, 4.28] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 3.0), Transform::from_xyz(-15.0, 1.9, -15.0)));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.5, 0.44, 2.0), Transform::from_xyz(-13.4, 2.22, -15.0)));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(1.3, 0.1, 2.0),
                Transform::from_xyz(-13.85, roof_bottom + 0.05, -15.0)));
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Vec3::new(-13.98, 2.9215, -15.0), Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.3125,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded, actor, &candidates, &permissions, None)
            }).unwrap().unwrap();
            let gap = result.2.expect("the raw StepDown position must pass beneath the roof") - 2.44 - 0.9;
            assert!((0.008..0.013).contains(&gap), "roof={roof_bottom} raw_gap={gap}");
        }
    }

    #[test]
    fn unwalkable_step_down_rejects_only_opposed_or_high_hits() {
        use bevy::ecs::system::RunSystemOnce;
        for (yaw, contact_y, accepted) in [(0.0_f32, 2.88, true), (15.0, 2.88, false), (0.0, 2.94, false)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.0, 0.2, 3.0), Transform::from_xyz(-15.0, 1.9, -15.0)));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.02, 0.2, 0.02), Transform::from_xyz(-13.855, 2.08, -14.73)));
            let rotation = Quat::from_rotation_y(-yaw.to_radians()) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_3);
            let center = Vec3::new(-13.6675, contact_y - 0.6, -15.0) - rotation * Vec3::Y * 0.315;
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.24, 0.03, 0.24),
                Transform::from_translation(center).with_rotation(rotation)));
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Vec3::new(-13.98, 2.9215, -15.0), Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.3125,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded, actor, &candidates, &permissions, None)
            }).unwrap().unwrap();
            assert_eq!(result.2.is_some(), accepted, "yaw={yaw} contact_y={contact_y} result={result:?}");
        }
    }

    #[test]
    fn step_down_keeps_a_blocking_hit_when_the_perch_is_not_walkable() {
        use bevy::ecs::system::RunSystemOnce;
        for (deck_x, walkable) in [(-12.7, true), (-12.63, false)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.0, 0.2, 3.0), Transform::from_xyz(-15.0, 1.9, -15.0)));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.0, 0.2, 1.3), Transform::from_xyz(deck_x, 2.08, -15.3)));
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                let cmc = crate::als::params::CmcAdapterParams::default();
                let result = grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Vec3::new(-14.01, 2.9215, -15.0), Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.17427273,
                    &cmc, LocomotionState::Grounded, actor, &candidates, &permissions, None).unwrap();
                let height = result.2.expect("the blocking StepDown must be committed");
                let destination = (result.0 + result.1.unwrap()).with_y(height);
                let floor = find_ground_floor(&spatial, destination, 0.9, 0.3, &cmc, LocomotionState::Grounded,
                    &SpatialQueryFilter::from_excluded_entities([actor]), actor, &candidates);
                assert_eq!(floor.is_some(), walkable, "deck={deck_x} destination={destination:?} floor={floor:?}");
            }).unwrap();
        }
    }

    #[test]
    fn walking_two_wall_adjust_preserves_source_projection_and_ground_limits() {
        let first = Vec3::NEG_X;
        let obtuse = Vec3::new(-0.5, 0.0, -3.0_f32.sqrt() * 0.5);
        let result = walking_two_wall_adjust(Vec3::Z * 0.2, obtuse, obtuse, first, 0.5, 0.0215);
        assert!(result.distance(Vec3::new(-0.04330127, 0.0, 0.025)) < 1e-6);
        let repeated = walking_two_wall_adjust(Vec3::Z * 0.2, first, first, first, 0.5, 0.0215);
        assert!(repeated.distance(Vec3::new(-0.0001, 0.0, 0.1)) < 1e-6);
        let acute = Vec3::new(0.5, 0.0, -3.0_f32.sqrt() * 0.5);
        assert!(walking_two_wall_adjust(Vec3::NEG_Y * 0.1, acute, acute, first, 0.5, 0.0215)
            .distance(Vec3::NEG_Y * 0.05) < 1e-6);
        assert_eq!(walking_two_wall_adjust(Vec3::NEG_Y * 0.1, acute, acute, first, 0.5, 0.018), Vec3::ZERO);
    }

    #[test]
    fn two_wall_adjust_uses_the_walkable_impact_face() {
        let normal = Vec3::new(-0.8, 0.6, 0.0);
        let walkable = walking_two_wall_adjust(Vec3::X * 0.2, normal, Vec3::Y, Vec3::Z, 0.5, 0.0215);
        let steep = walking_two_wall_adjust(Vec3::X * 0.2, normal, normal, Vec3::Z, 0.5, 0.0215);
        assert!(walkable.y > 0.1, "walkable={walkable:?}");
        assert_eq!(steep.y, 0.0);
    }

    #[test]
    fn two_wall_slide_sweeps_the_adjusted_direction_against_a_third_wall() {
        use bevy::ecs::system::RunSystemOnce;
        for third_wall in [false, true] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 2.0, 0.1),
                Transform::from_xyz(0.0, 1.0, -0.6).with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4))));
            if third_wall {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 2.0, 4.0), Transform::from_xyz(-0.5, 1.0, 0.0)));
            }
            app.update();
            app.update();
            let progress = app.world_mut().run_system_once(|spatial: SpatialQuery| {
                walking_slide_sweep(&spatial, &als_character_capsule(0.9, 0.3), Vec3::Y,
                    Vec3::new(1.0, 0.0, -1.0), Vec3::NEG_X, 0.0215,
                    &SpatialQueryFilter::default(), &|_| true, None).unwrap()
            }).unwrap();
            assert!(progress.z < -0.2, "third_wall={third_wall} progress={progress:?}");
            if third_wall { assert!(progress.x >= -0.1501 && progress.x < -0.1, "progress={progress:?}"); }
            else { assert!(progress.x < -0.3, "progress={progress:?}"); }
        }
    }

    #[test]
    fn step_up_continues_sliding_after_a_second_wall() {
        use bevy::ecs::system::RunSystemOnce;
        for (two_walls, expected) in [(false, Vec2::new(-13.773913, -14.893581)),
            (true, Vec2::new(-13.802099, -14.883323))] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 3.0), Transform::from_xyz(-15.0, 1.9, -15.0)));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.02, 0.2, 0.02), Transform::from_xyz(-13.855, 2.08, -14.73)));
            for (offset, yaw) in [(1.13, 45.0_f32), (1.16, 20.0_f32)].into_iter().take(if two_walls { 2 } else { 1 }) {
                let rotation = Quat::from_rotation_y(yaw.to_radians());
                let center = Vec3::new(-15.0 + offset, 3.5, -15.0) + rotation * Vec3::X * 0.32;
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.04, 1.0, 3.0),
                    Transform::from_translation(center).with_rotation(rotation)));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Vec3::new(-13.98, 2.9215, -15.0), Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.3125,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded, actor, &candidates, &permissions, None)
            }).unwrap().unwrap();
            assert!(result.2.is_some(), "the step must reach the downward sweep: {result:?}");
            let destination = result.0 + result.1.unwrap();
            assert!(Vec2::new(destination.x, destination.z).distance(expected) < 0.001,
                "two_walls={two_walls} destination={destination:?} expected={expected:?}");
        }
    }

    #[test]
    fn walking_step_slide_uses_the_near_floor_normal_only_when_opposed() {
        let wall = Vec3::new(-0.61237246, -0.5, -0.61237246);
        let opposed_floor = Vec3::new(-4.0_f32.to_radians().sin(), 4.0_f32.to_radians().cos(), 0.0);
        let following_floor = Vec3::new(-opposed_floor.x, opposed_floor.y, 0.0);
        let wall_flat = wall.with_y(0.0).normalize();
        assert!(walking_step_slide_normal(Vec3::X, wall, wall, Vec3::Y, 0.018).distance(wall_flat) < 1e-6);
        assert_eq!(walking_step_slide_normal(Vec3::X, wall, wall, Vec3::Y, 0.0215), wall);
        assert!(walking_step_slide_normal(Vec3::X, wall, wall, opposed_floor, 0.018).distance(Vec3::NEG_X) < 1e-6);
        assert!(walking_step_slide_normal(Vec3::X, wall, wall, following_floor, 0.018).distance(wall_flat) < 1e-6);
        let nearly_vertical = Vec3::new(-0.001, 0.9999995, 0.0);
        assert!(walking_step_slide_normal(Vec3::X, wall, wall, nearly_vertical, 0.018).distance(wall_flat) < 1e-6);
    }

    #[test]
    fn walking_step_slide_uses_the_impact_face_for_positive_contact() {
        for contact in [Vec3::new(-0.62927973, 0.77717894, 0.0), Vec3::new(-0.73775566, 0.67506796, 0.0)] {
            assert_eq!(walking_step_slide_normal(Vec3::X, contact, Vec3::NEG_X, Vec3::Y, 0.0215), Vec3::NEG_X);
            assert_eq!(walking_step_slide_normal(Vec3::X, contact, Vec3::Y, Vec3::Y, 0.0215), contact);
        }
    }

    #[test]
    fn step_up_uses_the_source_edge_face_under_both_ceilings() {
        use bevy::ecs::system::RunSystemOnce;
        for (ceiling_bottom, expected_x) in [(4.07, -13.790219), (4.04, -13.821881)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            for (size, position) in [
                (Vec3::new(4.0, 0.2, 4.0), Vec3::new(-15.0, 1.9, -15.0)),
                (Vec3::new(0.02, 0.2, 0.02), Vec3::new(-13.855, 2.08, -14.73)),
                (Vec3::new(1.0, 0.1, 1.0), Vec3::new(-14.65, ceiling_bottom + 0.05, -15.0)),
                (Vec3::new(0.08, 0.43, 1.0), Vec3::new(-13.55, 2.215, -15.0)),
            ] {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(size.x, size.y, size.z),
                    Transform::from_translation(position)));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Vec3::new(-13.98, 2.9215, -15.0), Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.3125,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded, actor, &candidates, &permissions, None)
            }).unwrap().unwrap();
            let movement = result.1.expect("Source StepUp must commit the forward movement");
            assert!(result.4 && result.2.is_some(), "ceiling={ceiling_bottom} result={result:?}");
            assert!(movement.y.abs() < 1e-5, "ceiling={ceiling_bottom} movement={movement:?}");
            assert!((result.0.x + movement.x - expected_x).abs() < 0.002,
                "ceiling={ceiling_bottom} result={result:?} source_x={expected_x}");
        }
    }

    #[test]
    fn line_floor_retains_the_sweep_gap_used_by_the_slide_rule() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
        app.finish();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(8.0, 0.2, 8.0), Transform::from_xyz(0.0, -0.1, 0.0)));
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 0.5, 4.0), Transform::from_xyz(0.15, 0.9215, 0.0)));
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        app.update();
        let floor = app.world_mut().run_system_once(move |spatial: SpatialQuery,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
            find_ground_floor_with_penetration(&spatial, Vec3::Y * 0.9215, 0.9, 0.3,
                &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                &SpatialQueryFilter::from_excluded_entities([actor]), actor, &candidates, None, None)
        }).unwrap().unwrap();
        assert!(floor.penetration.is_some() && floor.hit.normal1.y > 0.99);
        assert!(floor.sweep_distance < MIN_FLOOR_DISTANCE && 0.9215 - floor.center_y > MIN_FLOOR_DISTANCE);
    }

    #[test]
    fn step_up_flattens_a_downward_wall_only_near_the_old_floor() {
        use bevy::ecs::system::RunSystemOnce;
        for (gap, flattened) in [(0.018, true), (0.0215, false)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 3.0), Transform::from_xyz(-15.0, 1.9, -15.0)));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.02, 0.2, 0.02), Transform::from_xyz(-13.855, 2.08, -14.73)));
            let rotation = Quat::from_rotation_y(45.0_f32.to_radians()) * Quat::from_rotation_z(30.0_f32.to_radians());
            let center = Vec3::new(-13.87, 3.95, -15.0) + rotation * Vec3::X * 0.32;
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.04, 1.0, 1.5),
                Transform::from_translation(center).with_rotation(rotation)));
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Vec3::new(-13.98, 2.9 + gap, -15.0), Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.3125,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded, actor, &candidates, &permissions, None)
            }).unwrap().unwrap();
            assert!(result.2.is_some(), "the source fixture must complete StepUp: gap={gap} result={result:?}");
            let slide = result.1.expect("the step must produce a slide");
            assert_eq!(slide.y.abs() < 1e-5, flattened, "gap={gap} slide={slide:?}");
        }
    }

    #[test]
    fn step_up_retains_nonzero_submillimeter_progress() {
        use bevy::ecs::system::RunSystemOnce;
        for (front, accepted) in [(1.3845, false), (1.38638, true), (1.387, true)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            for (center, size) in [
                (Vec3::new(-15.0, 1.9, -15.0), Vec3::new(4.0, 0.2, 3.0)),
                (Vec3::new(-13.855, 2.08, -14.73), Vec3::new(0.02, 0.2, 0.02)),
                (Vec3::new(-15.0 + front + 0.04, 4.15, -15.0), Vec3::new(0.08, 1.0, 0.7)),
            ] {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(size.x, size.y, size.z), Transform::from_translation(center)));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Vec3::new(-13.98, 2.9215, -15.0), Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.3125,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded, actor, &candidates, &permissions, None)
            }).unwrap().unwrap();
            assert_eq!(result.2.is_some(), accepted, "front={front} result={result:?}");
        }
    }

    #[test]
    fn step_height_uses_the_floor_impact_instead_of_the_adjusted_capsule_center() {
        use bevy::ecs::system::RunSystemOnce;
        for (height, gap, accepted) in [(0.446, 0.0215, true), (0.449, 0.0215, true),
            (0.449, 0.003, true), (0.451, 0.0215, false)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 3.0),
                Transform::from_xyz(-15.0, 1.9, -15.0)));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.5, height, 2.0),
                Transform::from_xyz(-13.4, 2.0 + 0.5 * height, -15.0)));
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                grounded_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Vec3::new(-13.98, 2.9 + gap, -15.0), Quat::IDENTITY, 0.9, 0.3, Vec3::X * 0.3125,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded, actor, &candidates, &permissions, None)
            }).unwrap().unwrap();
            assert_eq!(result.2.is_some(), accepted, "height={height} gap={gap} result={result:?}");
        }
    }

    #[test]
    fn walking_adjusts_to_a_valid_low_floor_beyond_the_step_height() {
        use bevy::ecs::system::RunSystemOnce;
        for (drop, valid) in [(0.447, true), (0.4505, true), (0.46, false)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let far = Vec3::new(10.0, 0.9215, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(far), Transform::from_translation(far)));
            let top = 2.0 - drop;
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_xyz(0.0, top - 0.1, 0.0)));
            app.update();
            let start = Vec3::new(0.0, 2.9215, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start), LocomotionState::Grounded));
            app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
            let position = app.world().get::<Position>(actor).unwrap().0;
            let mode = *app.world().get::<LocomotionState>(actor).unwrap();
            if valid {
                assert_eq!(mode, LocomotionState::Grounded, "drop={drop}");
                assert!((position.y - (top + 0.9 + 0.0215)).abs() < 1e-4, "drop={drop} position={position:?}");
            } else {
                assert_eq!(mode, LocomotionState::Air, "a floor beyond the query range must not be accepted");
            }
        }
    }

    #[test]
    fn second_sloped_impact_recomputes_air_control_and_respects_the_first_wall() {
        use bevy::ecs::system::RunSystemOnce;
        for (desired, expected) in [
            (Vec3::ZERO, Vec3::new(0.0, -0.98087, -0.57504)),
            (Vec3::Z, Vec3::new(0.0, -0.98087, -0.57504)),
            (Vec3::new(-0.5, 0.0, 0.75_f32.sqrt()), Vec3::new(-0.03079, -0.98087, -0.58047)),
        ] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .add_message::<StateTransitionEvent>();
            app.finish();
            let rotation = Quat::from_rotation_y(-10.0_f32.to_radians()) * Quat::from_rotation_x(-60.0_f32.to_radians());
            let slope = Vec3::new(-13.0, 3.4, -15.0) - rotation * Vec3::Y * 0.385;
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 3.0, 2.0), Transform::from_xyz(-12.62, 4.0, -15.0)));
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.0, 0.04, 1.2),
                Transform::from_translation(slope).with_rotation(rotation)));
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>,
                bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
                owners: Query<&ColliderOf>, shapes: Query<(&Collider, &Position, &Rotation)>, mut transitions: MessageWriter<StateTransitionEvent>| {
                let mut seed = 123;
                falling_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Transform::from_xyz(-13.0, 4.0, -15.0), Vec3::new(4.0, -2.0, 2.0), desired,
                    6.5, &AlsMovementParams::default(), Vec3::NEG_Y * 9.81,
                    (Stance::Standing, RotationMode::ViewDirection, Gait::Running, 0.0),
                    (0.0, false, false), 40.0, 0.05, 0, actor, &mut seed,
                    &candidates, &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, None, None)
            }).unwrap();
            assert_eq!(result.4, LocomotionState::Air);
            assert!(result.1.distance(expected) < 0.002, "input={desired:?} actual={:?} expected={expected:?}", result.1);
        }
    }

    #[test]
    fn falling_air_control_uses_the_source_slope_and_penetration_rules() {
        let normal = Vec3::new(-3.0_f32.sqrt() * 0.5, 0.5, 0.0);
        assert!(limit_falling_air_control(Vec3::X * 6.0, normal, false).length() < 1e-6);
        assert_eq!(limit_falling_air_control(Vec3::NEG_X, normal, false), Vec3::NEG_X);
        assert_eq!(limit_falling_air_control(Vec3::Z, normal, false), Vec3::Z);
        assert_eq!(limit_falling_air_control(Vec3::X, normal, true), Vec3::ZERO);
        assert_eq!(limit_falling_air_control(Vec3::NEG_X, normal, true), Vec3::NEG_X);
        assert_eq!(limit_falling_air_control(Vec3::X, Vec3::NEG_X, false), Vec3::X);
    }

    #[test]
    fn falling_first_slope_hit_limits_only_inward_air_control() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
            .add_message::<StateTransitionEvent>();
        app.finish();
        let rotation = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4) * Quat::from_rotation_z(30.0_f32.to_radians());
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.04, 8.0, 2.0),
            Transform::from_xyz(0.0, 3.0, 0.0).with_rotation(rotation)));
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        app.update();
        let n = 0.5_f32.sqrt();
        for (desired, expected) in [
            (Vec3::new(-n, 0.0, -n), Vec3::new(0.3562477, -0.8726250, 0.3562477)),
            (Vec3::new(n, 0.0, n), Vec3::new(0.3739253, -0.9159262, 0.3739253)),
            (Vec3::new(-n, 0.0, n), Vec3::new(0.2855370, -0.8726250, 0.4269584)),
            (Vec3::ZERO, Vec3::new(0.3562477, -0.8726250, 0.3562477)),
        ] {
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>,
                bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
                owners: Query<&ColliderOf>, shapes: Query<(&Collider, &Position, &Rotation)>,
                mut transitions: MessageWriter<StateTransitionEvent>| {
                let mut seed = 123;
                falling_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Transform::from_xyz(0.38273278, 3.3125, 0.38273278), Vec3::NEG_Y, desired, 6.5,
                    &AlsMovementParams::default(), Vec3::NEG_Y * 9.81,
                    (Stance::Standing, RotationMode::ViewDirection, Gait::Running, 0.0),
                    (0.0, false, false), 40.0, 1.0 / 60.0, 0, actor, &mut seed,
                    &candidates, &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, Some(20.0), None)
            }).unwrap();
            assert_eq!(result.4, LocomotionState::Air);
            assert!(result.1.distance(expected) < 0.003, "input={desired:?} actual={:?} source={expected:?}", result.1);
        }
    }

    #[test]
    fn loaded_falling_character_uses_limited_input_on_the_first_slope_hit() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let rotation = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4) * Quat::from_rotation_z(30.0_f32.to_radians());
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.04, 8.0, 2.0),
            Transform::from_xyz(0.0, 13.0, 0.0).with_rotation(rotation)));
        app.update();
        let start = Vec3::new(0.38273278, 13.3125, 0.38273278);
        app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
            LocomotionState::Air, LinearVelocity(Vec3::NEG_Y), TnuaMotor::default(),
            crate::als::character::AlsEffectiveGravity { value: Vec3::NEG_Y * 9.81, ..default() },
            AlsCmcAdapterRuntime { controlled_input_max_acceleration: Some(20.0), ..default() }));
        app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion =
            Vec3::new(-1.0, 0.0, -1.0).normalize();
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
        app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
        let actual = app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().grounded_collision_velocity.unwrap();
        let expected = Vec3::new(0.3562477, -0.8726250, 0.3562477);
        assert!(actual.distance(expected) < 0.003, "actual={actual:?} source={expected:?}");
    }

    #[test]
    fn falling_capsule_uses_world_rotation_without_changing_local_rotation() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 4.0, 0.1),
            Transform::from_xyz(0.0, 5.0, -0.05)));
        app.update();
        for (world_rotation, expected_z) in [
            (Quat::IDENTITY, 0.3106),
            (Quat::from_rotation_y(60.0_f32.to_radians()), 0.3106),
            (Quat::from_rotation_x(60.0_f32.to_radians()), 0.8302),
        ] {
            for parented in [false, true] {
                let start = Vec3::new(0.0, 5.0, 1.0);
                let local = if parented {
                    let parent = app.world_mut().spawn(Transform::from_rotation(world_rotation)).id();
                    app.world_mut().entity_mut(actor).insert(ChildOf(parent));
                    Transform::from_translation(world_rotation.inverse() * start)
                } else {
                    app.world_mut().entity_mut(actor).remove::<ChildOf>();
                    Transform::from_translation(start).with_rotation(world_rotation)
                };
                app.world_mut().entity_mut(actor).insert((Position(start), local, LocomotionState::Air,
                    LinearVelocity(Vec3::NEG_Z * 12.0), TnuaMotor::default(),
                    crate::als::character::AlsEffectiveGravity::default(), AlsCmcAdapterRuntime::default()));
                app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::ZERO;
                app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
                app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
                let actual = app.world().get::<Position>(actor).unwrap().0;
                let local_after = app.world().get::<Transform>(actor).unwrap();
                assert!((actual.z - expected_z).abs() < 0.003,
                    "parented={parented} rotation={world_rotation:?} position={actual:?}");
                assert_eq!(local_after.rotation, local.rotation, "parented={parented}");
            }
        }
    }

    #[test]
    fn movement_base_carry_sweeps_with_the_character_world_rotation() {
        use bevy::ecs::system::RunSystemOnce;
        for angle in [0.0_f32, 60.0] {
            for parented in [false, true] {
                for wall in [false, true] {
                    let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
                    let base_origin = Vec3::Y * 10.0;
                    let base = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(4.0, 0.2, 4.0),
                        Transform::from_translation(base_origin))).id();
                    if wall {
                        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 4.0, 0.1),
                            Transform::from_xyz(0.0, 11.0, 1.25)));
                    }
                    app.update();
                    let start = Vec3::Y * 11.0;
                    let rotation = Quat::from_rotation_x(angle.to_radians());
                    let local = if parented {
                        let parent = app.world_mut().spawn(Transform::from_rotation(rotation)).id();
                        app.world_mut().entity_mut(actor).insert(ChildOf(parent));
                        Transform::from_translation(rotation.inverse() * start)
                    } else { Transform::from_translation(start).with_rotation(rotation) };
                    app.world_mut().entity_mut(actor).insert((Position(start), local, LocomotionState::Grounded,
                        crate::als::dynamics::AlsMovementBase { entity: Some(base), location: base_origin,
                            rotation: Quat::IDENTITY, has_relative_rotation: false },
                        AlsCmcAdapterRuntime { grounded_move_start: Some(start), ..default() }));
                    let target = base_origin + Vec3::Z;
                    app.world_mut().entity_mut(base).insert((Position(target), Transform::from_translation(target)));
                    app.world_mut().run_system_once((|| MovementBaseFollowPhase::BeforeSensors).pipe(follow_movement_base_system)).unwrap();
                    let position = app.world().get::<Position>(actor).unwrap().0;
                    let extent = 0.3 + 0.6 * angle.to_radians().sin().abs();
                    let expected = if wall { movement_sweep_travel(1.0, 1.2 - extent) } else { 1.0 };
                    assert!((position.z - expected).abs() < 0.003,
                        "angle={angle} parented={parented} wall={wall} position={position:?} expected={expected}");
                    assert_eq!(app.world().get::<Transform>(actor).unwrap().rotation, local.rotation);
                    assert!(app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().grounded_move_start.unwrap().distance(position) < 1e-5);
                }
            }
        }
    }

    #[test]
    fn mantle_base_carry_uses_the_existing_feet_anchor_before_movement() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::als::mantle::MantleKind;
        for (relative, distance, had_base) in [(true, 0.0, true), (true, 0.02, true),
            (true, 0.02, false), (false, 0.02, true)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let base_start = Vec3::new(0.0, 4.0, 0.0);
            let target = app.world_mut().spawn((RigidBody::Kinematic, Transform::from_translation(base_start))).id();
            app.update();
            let actor_start = Vec3::new(0.0, 5.0, 0.0);
            let mut mantle = AlsMantleState::default();
            mantle.begin_with_target_base(MantleKind::High, actor_start, actor_start + Vec3::Y, 1.5, 0.0, 0.0,
                AlsMantleTargetBase { entity: target, translation: base_start, rotation: Quat::IDENTITY,
                    scale: Vec3::ONE, relative_location: relative });
            let velocity = Vec3::new(1.0, 0.2, -0.4);
            app.world_mut().entity_mut(actor).insert((Position(actor_start), Transform::from_translation(actor_start),
                LocomotionState::Custom, mantle, LinearVelocity(velocity),
                crate::als::dynamics::AlsMovementBase { entity: had_base.then_some(target), location: if had_base { base_start } else { Vec3::ZERO },
                    rotation: Quat::IDENTITY, has_relative_rotation: false }));
            app.world_mut().get_mut::<Transform>(target).unwrap().translation.x += distance;
            app.world_mut().run_system_once((|| MovementBaseFollowPhase::BeforeSensors).pipe(follow_movement_base_system)).unwrap();
            let expected = actor_start + Vec3::X * if relative { distance } else { 0.0 };
            assert!(app.world().get::<Position>(actor).unwrap().0.abs_diff_eq(expected, 1e-5));
            assert_eq!(app.world().get::<LinearVelocity>(actor).unwrap().0, velocity);
            assert_eq!(app.world().get::<Transform>(actor).unwrap().rotation, Quat::IDENTITY);
        }
    }

    #[test]
    fn mantle_moving_base_writes_the_current_actor_parent_space() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::als::mantle::MantleKind;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
        app.finish();
        app.update();
        for parented in [false, true] {
            for dilation in [0.0, 1.0] {
                for (simulated, translation, rotation, scale) in [
                    (true, Vec3::ZERO, Quat::IDENTITY, Vec3::ONE),
                    (true, Vec3::X * 0.2, Quat::IDENTITY, Vec3::ONE),
                    (true, Vec3::ZERO, Quat::from_rotation_y(0.2), Vec3::ONE),
                    (true, Vec3::ZERO, Quat::from_rotation_x(0.15), Vec3::ONE),
                    (true, Vec3::ZERO, Quat::IDENTITY, Vec3::new(1.2, 0.8, 1.1)),
                    (false, Vec3::X * 0.2, Quat::IDENTITY, Vec3::ONE),
                ] {
                    let base_start = Vec3::new(0.0, 4.0, 0.0);
                    let base_transform = Transform::from_translation(base_start + translation).with_rotation(rotation).with_scale(scale);
                    let base = app.world_mut().spawn((if simulated { RigidBody::Dynamic } else { RigidBody::Kinematic },
                        base_transform, GlobalTransform::from(base_transform))).id();
                    let start = Transform::from_xyz(0.4, 5.0, 0.7).with_rotation(Quat::from_rotation_y(0.35));
                    let mut mantle = AlsMantleState::default();
                    mantle.begin_with_target_base(MantleKind::High, start.translation, Vec3::new(0.0, 6.0, -1.0),
                        1.5, 0.35, 0.0, AlsMantleTargetBase { entity: base, translation: base_start,
                            rotation: Quat::IDENTITY, scale: Vec3::ONE, relative_location: true });
                    mantle.root_motion_ready = true;
                    let parent_transform = Transform::from_xyz(3.0, -1.0, 2.0)
                        .with_rotation(Quat::from_rotation_y(0.7)).with_scale(Vec3::splat(1.5));
                    let local = if parented { GlobalTransform::from(start).reparented_to(&GlobalTransform::from(parent_transform)) }
                        else { start };
                    let actor = app.world_mut().spawn((AlsCharacter, mantle, LocomotionState::Custom,
                        Collider::capsule(0.3, 1.2), Position(start.translation), Rotation(start.rotation), local,
                        crate::als::dynamics::AlsMovementBase { entity: Some(base), location: base_start,
                            rotation: Quat::IDENTITY, has_relative_rotation: false },
                        avian3d::prelude::CustomPositionIntegration,
                        crate::als_camera::AlsCameraCustomTimeDilation(dilation))).id();
                    let parent = if parented {
                        let parent = app.world_mut().spawn((parent_transform, GlobalTransform::from(parent_transform))).id();
                        app.world_mut().entity_mut(actor).insert(ChildOf(parent));
                        Some(parent)
                    } else { None };
                    app.world_mut().run_system_once((|| MovementBaseFollowPhase::AfterPhysics).pipe(follow_movement_base_system)).unwrap();
                    let offset = Vec3::Y * 0.9;
                    let expected = if simulated {
                        rotation * (start.translation - offset - base_start) + base_start + translation + offset
                    } else { start.translation };
                    let actual = app.world().get::<Position>(actor).unwrap().0;
                    let world = app.world_mut().run_system_once(move |helper: bevy::transform::helper::TransformHelper| {
                        helper.compute_global_transform(actor).unwrap()
                    }).unwrap();
                    assert!(actual.abs_diff_eq(expected, 1e-5),
                        "parented={parented} dilation={dilation} simulated={simulated} actual={actual:?} expected={expected:?}");
                    assert!(world.translation().abs_diff_eq(expected, 1e-5));
                    assert!(world.rotation().angle_between(start.rotation) < 1e-4);
                    assert_eq!(app.world().get::<Rotation>(actor).unwrap().0, start.rotation);
                    app.world_mut().despawn(actor);
                    if let Some(parent) = parent { app.world_mut().despawn(parent); }
                    app.world_mut().despawn(base);
                }
            }
        }
    }

    #[test]
    fn movement_base_follow_pauses_only_with_the_world_clock() {
        use bevy::ecs::system::RunSystemOnce;
        for (paused, owner_zero, distance) in [(true, false, 0.1_f32), (true, false, 0.0),
            (false, true, 0.1), (false, false, 0.1)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let base_origin = Vec3::Y * 10.0;
            let base = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(4.0, 0.2, 4.0),
                Transform::from_translation(base_origin))).id();
            app.update();
            let start = Vec3::Y * 11.0;
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start), LocomotionState::Grounded,
                crate::als::dynamics::AlsMovementBase { entity: Some(base), location: base_origin,
                    rotation: Quat::IDENTITY, has_relative_rotation: false },
                AlsCmcAdapterRuntime { grounded_move_start: Some(start), ..default() },
                crate::als_camera::AlsCameraCustomTimeDilation(if owner_zero { 0.0 } else { 1.0 })));
            let moved_base = base_origin + Vec3::X * distance;
            app.world_mut().entity_mut(base).insert((Position(moved_base), Transform::from_translation(moved_base)));
            if paused { app.world_mut().resource_mut::<Time<Virtual>>().pause(); }
            app.world_mut().run_system_once((|| MovementBaseFollowPhase::BeforeSensors).pipe(follow_movement_base_system)).unwrap();
            app.world_mut().run_system_once((|| MovementBaseFollowPhase::AfterPhysics).pipe(follow_movement_base_system)).unwrap();
            let expected = if paused { start } else { start + Vec3::X * distance };
            assert!(app.world().get::<Position>(actor).unwrap().0.abs_diff_eq(expected, 1e-6));
            assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(actor).unwrap().location,
                if paused { base_origin } else { moved_base });
            assert!(app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().grounded_move_start.unwrap().abs_diff_eq(expected, 1e-6));
            if paused {
                app.world_mut().resource_mut::<Time<Virtual>>().unpause();
                app.world_mut().run_system_once((|| MovementBaseFollowPhase::BeforeSensors).pipe(follow_movement_base_system)).unwrap();
                assert!(app.world().get::<Position>(actor).unwrap().0.abs_diff_eq(start + Vec3::X * distance, 1e-6));
                assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(actor).unwrap().location, moved_base);
            }
        }
    }

    #[test]
    fn grounded_float_config_does_not_apply_a_position_spring() {
        let config = initial_als_control_scheme_config(&AlsMovementParams::default());
        assert_eq!(config.basis.spring_strength, 0.0);
        assert_eq!(config.basis.spring_dampening, 1.0);
    }

    #[test]
    fn loaded_grounded_character_keeps_source_floor_distance_band() {
        for (gap, expected) in [(0.018_f32, 0.0215_f32), (0.02, 0.02), (0.0215, 0.0215),
            (0.0235, 0.0235), (0.025, 0.0215)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let start = Vec3::Y * (0.9 + gap);
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LocomotionState::Grounded, LinearVelocity::ZERO, TnuaMotor::default(), AlsCmcAdapterRuntime::default()));
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            for _ in 0..30 {
                app.update();
                assert!(std::time::Instant::now() < deadline, "Grounded floor fixture made no progress");
            }
            let position = app.world().get::<Position>(actor).unwrap().0;
            let velocity = app.world().get::<LinearVelocity>(actor).unwrap().0;
            let handle = app.world().get::<TnuaConfig<AlsControlScheme>>(actor).unwrap().0.clone();
            let config = app.world().resource::<Assets<AlsControlSchemeConfig>>().get(&handle).unwrap();
            assert_eq!(config.basis.spring_strength, 0.0);
            assert!((position.y - (0.9 + expected)).abs() < 1e-4, "gap={gap} position={position:?}");
            assert!(velocity.y.abs() < 1e-4, "gap={gap} velocity={velocity:?}");
        }
    }

    #[test]
    fn falling_ditch_uses_opposing_upward_impact_normals_and_source_tolerance() {
        let left = Vec3::new(3.0_f32.sqrt() * 0.5, 0.5, 0.0);
        let right = Vec3::new(-3.0_f32.sqrt() * 0.5, 0.5, 0.0);
        assert!(falling_ditch(left, right, Vec3::Z));
        assert!(falling_ditch(left, right, Vec3::Y * 1e-6));
        assert!(!falling_ditch(left, right, Vec3::Y * 1.01e-6));
        assert!(!falling_ditch(left, left, Vec3::ZERO));
        assert!(!falling_ditch(left, right.with_y(-0.5), Vec3::ZERO));
    }

    #[test]
    fn falling_between_two_steep_slopes_lands_without_remaining_time() {
        use bevy::ecs::system::RunSystemOnce;
        for opposing in [false, true] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .add_message::<StateTransitionEvent>();
            app.finish();
            for angle in [-60.0_f32, 60.0].into_iter().take(if opposing { 2 } else { 1 }) {
                let rotation = Quat::from_rotation_z(angle.to_radians());
                let normal = rotation * Vec3::Y;
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.02, 2.0),
                    Transform::from_translation(-normal * 0.01).with_rotation(rotation)));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let (result, forced_landing) = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>,
                bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
                owners: Query<&ColliderOf>, shapes: Query<(&Collider, &Position, &Rotation)>,
                mut transitions: MessageWriter<StateTransitionEvent>| {
                let mut seed = 123;
                let params = AlsMovementParams::default();
                let capsule = als_character_capsule(0.9, 0.3);
                let final_velocity = Vec3::NEG_Y * (2.0 + 9.81 * 0.05);
                let forced_landing = falling_movement_sweep(&spatial, &capsule, Vec3::Y * 1.25, Quat::IDENTITY,
                    (Vec3::NEG_Y * 2.0 + final_velocity) * 0.025, final_velocity, 0.05, None,
                    Some((&params.cmc, 4.2, 6.5, &mut seed)), actor, &candidates, &shapes, &impact_bodies, None).3;
                seed = 123;
                let result = falling_movement_transaction(&spatial, &capsule,
                    Transform::from_xyz(0.0, 1.25, 0.0), Vec3::NEG_Y * 2.0, Vec3::ZERO, 6.5,
                    &params, Vec3::NEG_Y * 9.81,
                    (Stance::Standing, RotationMode::ViewDirection, Gait::Running, 0.0),
                    (0.0, false, false), 40.0, 0.05, 0, actor, &mut seed,
                    &candidates, &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, None, None);
                (result, forced_landing)
            }).unwrap();
            assert_eq!(result.4, if opposing { LocomotionState::Grounded } else { LocomotionState::Air }, "result={result:?}");
            if opposing {
                assert_eq!(forced_landing, Some((0.0, true)), "A ditch must consume the remaining falling time");
                assert_eq!(result.7, Some(LocomotionState::Grounded));
            }
        }
    }

    #[test]
    fn loaded_falling_character_keeps_the_ditch_landing_for_the_current_frame() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let origin = Vec3::Y * 10.0;
        for angle in [-60.0_f32, 60.0] {
            let rotation = Quat::from_rotation_z(angle.to_radians());
            let normal = rotation * Vec3::Y;
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.02, 2.0),
                Transform::from_translation(origin - normal * 0.01).with_rotation(rotation)));
        }
        app.update();
        let start = origin + Vec3::Y * 1.25;
        app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
            LocomotionState::Air, LinearVelocity(Vec3::NEG_Y * 2.0), TnuaMotor::default(),
            crate::als::character::AlsEffectiveGravity { value: Vec3::NEG_Y * 9.81, ..default() },
            AlsCmcAdapterRuntime::default()));
        app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::ZERO;
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
        app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
        assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), LocomotionState::Grounded);
        assert_eq!(app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().post_collision_mode, Some(LocomotionState::Grounded));
        app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
        assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), LocomotionState::Grounded);
    }

    #[test]
    fn falling_impact_uses_only_simulated_body_contact_velocity() {
        use bevy::ecs::system::RunSystemOnce;
        for (kind, speed, angular, child, actor_speed) in [
            (RigidBody::Dynamic, 1.0_f32, 0.0_f32, false, 5.0_f32),
            (RigidBody::Dynamic, -1.0, 0.0, false, 5.0),
            (RigidBody::Dynamic, 0.0, 0.0, false, 5.0),
            (RigidBody::Kinematic, 1.0, 0.0, false, 5.0),
            (RigidBody::Static, 1.0, 0.0, false, 5.0),
            (RigidBody::Dynamic, 1.0, 1.0, true, 5.0),
            (RigidBody::Dynamic, 1.0, 0.0, false, 0.0),
        ] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            let body = app.world_mut().spawn((kind, Transform::from_xyz(0.0, 2.0, 0.0),
                LinearVelocity::ZERO, AngularVelocity::ZERO, GravityScale(0.0))).id();
            if child {
                app.world_mut().spawn((ChildOf(body), Collider::cuboid(0.1, 4.0, 4.0), Transform::IDENTITY));
            } else {
                app.world_mut().entity_mut(body).insert(Collider::cuboid(0.1, 4.0, 4.0));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            app.world_mut().entity_mut(body).insert((LinearVelocity(Vec3::X * speed), AngularVelocity(Vec3::Z * angular)));
            let (result, hit, body_position) = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                shapes: Query<(&Collider, &Position, &Rotation)>, positions: Query<&Position>| {
                let capsule = als_character_capsule(0.9, 0.3);
                let start = Vec3::new(-1.0, 3.0, 0.0);
                let filter = SpatialQueryFilter::from_excluded_entities([actor]);
                let hit = spatial.cast_shape(&capsule, start, Quat::IDENTITY, Dir3::X,
                    &ShapeCastConfig::from_max_distance(1.0), &filter).unwrap();
                let result = falling_movement_sweep(&spatial, &capsule, start, Quat::IDENTITY,
                    Vec3::X, Vec3::X * actor_speed, 0.2, None, None, actor, &candidates, &shapes, &impact_bodies, None);
                (result, hit, positions.get(body).unwrap().0)
            }).unwrap();
            let expected = if kind == RigidBody::Dynamic && actor_speed != 0.0 {
                speed - angular * (hit.point1.y - body_position.y)
            } else { 0.0 };
            assert!((result.1.x - expected).abs() < 0.002,
                "kind={kind:?} speed={speed} angular={angular} child={child} actual={:?} expected={expected}", result.1);
        }
    }

    #[test]
    fn loaded_falling_impact_publishes_dynamic_wall_velocity() {
        use bevy::ecs::system::RunSystemOnce;
        for (kind, speed, expected) in [(RigidBody::Dynamic, 1.0_f32, 1.0_f32),
            (RigidBody::Dynamic, -1.0, -1.0), (RigidBody::Kinematic, 1.0, 0.0), (RigidBody::Static, 1.0, 0.0)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let wall = app.world_mut().spawn((kind, Collider::cuboid(0.1, 4.0, 4.0),
                Transform::from_xyz(0.0, 10.0, 0.0), LinearVelocity::ZERO, GravityScale(0.0))).id();
            app.update();
            app.world_mut().entity_mut(wall).insert(LinearVelocity(Vec3::X * speed));
            let start = Vec3::new(-0.4, 10.0, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LocomotionState::Air, LinearVelocity(Vec3::X * 5.0), TnuaMotor::default(),
                crate::als::character::AlsEffectiveGravity::default(), AlsCmcAdapterRuntime::default()));
            app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::ZERO;
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            let actual = app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().grounded_collision_velocity.unwrap();
            assert!((actual.x - expected).abs() < 0.002, "kind={kind:?} speed={speed} actual={actual:?}");
        }
    }

    #[test]
    fn falling_initial_overlap_resolves_before_retrying_original_motion() {
        use bevy::ecs::system::RunSystemOnce;
        for (inside, blocked, direction) in [(true, false, 1.0_f32), (true, false, -1.0),
            (false, false, 1.0), (false, false, -1.0), (true, true, 1.0)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 4.0, 4.0), Transform::IDENTITY));
            if blocked {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 4.0, 4.0), Transform::from_xyz(-0.7, 0.0, 0.0)));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let start = Vec3::X * if inside { -0.15 } else { -0.6 };
            let (result, adjustment) = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                shapes: Query<(&Collider, &Position, &Rotation)>| {
                let capsule = als_character_capsule(0.9, 0.3);
                let filter = SpatialQueryFilter::from_excluded_entities([actor]);
                let accepts = |candidate| character_blocks_collider(actor, candidate, &candidates);
                let cast_direction = if direction > 0.0 { Dir3::X } else { Dir3::NEG_X };
                let hit = spatial.cast_shape_predicate(&capsule, start, Quat::IDENTITY, cast_direction,
                    &ShapeCastConfig::from_max_distance(1.0 / 60.0), &filter, &accepts);
                let adjustment = hit.as_ref().and_then(|hit| falling_penetration_adjustment(&spatial, &capsule,
                    start, Quat::IDENTITY, hit, &filter, &accepts, &impact_bodies));
                let result = falling_movement_sweep(&spatial, &capsule, start, Quat::IDENTITY,
                    Vec3::X * (direction / 60.0), Vec3::X * direction, 1.0 / 60.0,
                    None, None, actor, &candidates, &shapes, &impact_bodies, None);
                (result, adjustment)
            }).unwrap();
            if inside && !blocked {
                assert!((adjustment.unwrap().x + 0.20125).abs() < 1e-4, "adjustment={adjustment:?}");
                let end = start + result.0;
                assert!(end.x <= -0.35, "direction={direction} end={end:?}");
                if direction < 0.0 { assert!(end.x < -0.366, "end={end:?}"); }
                assert_eq!(result.1.x, direction);
            } else if blocked {
                assert!(adjustment.is_none(), "A blocked adjustment must not teleport into the other wall");
                let end = start + result.0;
                assert!(end.x < -0.3 && end.x >= -0.3501, "The source sweep-out must make only available progress: {end:?}");
            } else {
                assert!(adjustment.is_none());
                assert!((result.0.x - direction / 60.0).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn loaded_falling_character_resolves_wall_overlap_before_motion() {
        use bevy::ecs::system::RunSystemOnce;
        for direction in [-1.0_f32, 1.0] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 4.0, 4.0), Transform::from_xyz(0.0, 10.0, 0.0)));
            app.update();
            let start = Vec3::new(-0.15, 10.0, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LocomotionState::Air, LinearVelocity(Vec3::X * direction), TnuaMotor::default(),
                crate::als::character::AlsEffectiveGravity::default(), AlsCmcAdapterRuntime::default()));
            app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::ZERO;
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            let position = app.world().get::<Position>(actor).unwrap().0;
            assert!(position.x <= -0.35, "direction={direction} position={position:?}");
        }
    }

    #[test]
    fn falling_substeps_clear_only_source_small_horizontal_velocity() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
            .add_message::<StateTransitionEvent>();
        app.finish();
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        for (horizontal, clear) in [
            (Vec3::ZERO, true),
            (Vec3::X * 0.0002, true),
            (Vec3::new(0.00022, 0.0, 0.00022), true),
            (Vec3::new(0.00023, 0.0, 0.00023), false),
            (Vec3::X * 0.000316, true),
            (Vec3::X * 1e-7_f32.sqrt(), true),
            (Vec3::X * 0.0003165, false),
        ] {
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>,
                bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
                owners: Query<&ColliderOf>, shapes: Query<(&Collider, &Position, &Rotation)>,
                mut transitions: MessageWriter<StateTransitionEvent>| {
                let mut seed = 123;
                falling_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3), Transform::from_xyz(0.0, 10.0, 0.0),
                    horizontal + Vec3::NEG_Y * 0.2, Vec3::ZERO, 6.5, &AlsMovementParams::default(), Vec3::NEG_Y * 9.81,
                    (Stance::Standing, RotationMode::ViewDirection, Gait::Running, 0.0),
                    (0.0, false, false), 40.0, 0.06, 0, actor, &mut seed,
                    &candidates, &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, None, None)
            }).unwrap();
            assert_eq!(result.2, 2);
            let expected = if clear { Vec3::ZERO } else { horizontal };
            assert!(result.1.with_y(0.0).abs_diff_eq(expected, 1e-8), "input={horizontal:?} velocity={:?}", result.1);
            assert!((result.1.y + 0.7886).abs() < 1e-5, "velocity={:?}", result.1);
            let displacement = horizontal * if clear { 0.03 } else { 0.06 };
            assert!(result.0.translation.with_y(0.0).abs_diff_eq(displacement, 1e-8),
                "input={horizontal:?} position={:?}", result.0.translation);
        }
    }

    #[test]
    fn loaded_falling_substeps_publish_the_cleared_velocity() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let start = Vec3::Y * 10.0;
        app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
            LocomotionState::Air, LinearVelocity(Vec3::new(0.0002, -0.2, 0.0)), TnuaMotor::default(),
            crate::als::character::AlsEffectiveGravity { value: Vec3::NEG_Y * 9.81, ..default() },
            AlsCmcAdapterRuntime::default()));
        app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::ZERO;
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.06));
        app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
        let runtime = app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap();
        assert_eq!(runtime.simulation_substeps, 2);
        assert_eq!(runtime.grounded_collision_velocity.unwrap().x, 0.0);
        assert!((app.world().get::<Position>(actor).unwrap().x - 0.000006).abs() < 1e-8);
    }

    #[test]
    fn falling_apex_split_obeys_source_time_and_attempt_limits() {
        let old = Vec3::new(1.0, 0.2, 0.0);
        let next = Vec3::new(1.1, 0.2 - 9.81 * 0.05, 0.0);
        let (time, velocity) = falling_apex_step(old, next, 0.05, 0).unwrap();
        assert!((time - 0.2 / 9.81).abs() < 1e-6);
        assert_eq!(velocity.y, 0.0);
        assert!((velocity.x - (1.0 + 0.1 * time / 0.05)).abs() < 1e-6);
        assert!(falling_apex_step(old, next, 0.05, 2).is_none());
        assert!(falling_apex_step(Vec3::Y * 0.0005, Vec3::NEG_Y, 0.05, 0).is_none());
        assert!(falling_apex_step(old, Vec3::X, 0.05, 0).is_none());
        assert!(falling_apex_step(Vec3::NEG_Y, next, 0.05, 0).is_none());
    }

    #[test]
    fn falling_apex_substep_sweeps_the_upward_path_before_descending() {
        use bevy::ecs::system::RunSystemOnce;
        for (ceiling, duration, expected_y) in [(true, 0.05_f32, 1.9961968_f32),
            (false, 0.05, 1.9977375), (true, 0.1, 1.9694), (false, 0.1, 1.97095)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .add_message::<StateTransitionEvent>();
            app.finish();
            if ceiling {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.1, 4.0), Transform::from_xyz(0.0, 2.9515, 0.0)));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>,
                bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
                owners: Query<&ColliderOf>, shapes: Query<(&Collider, &Position, &Rotation)>,
                mut transitions: MessageWriter<StateTransitionEvent>| {
                let mut seed = 123;
                falling_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3), Transform::from_xyz(0.0, 2.0, 0.0),
                    Vec3::Y * 0.2, Vec3::ZERO, 6.5, &AlsMovementParams::default(), Vec3::NEG_Y * 9.81,
                    (Stance::Standing, RotationMode::ViewDirection, Gait::Running, 0.0),
                    (0.0, false, false), 40.0, duration, 0, actor, &mut seed,
                    &candidates, &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, None, None)
            }).unwrap();
            assert!((result.0.translation.y - expected_y).abs() < 0.0002,
                "ceiling={ceiling} duration={duration} position={:?} source_y={expected_y}", result.0.translation);
            assert!((result.1.y - (0.2 - 9.81 * duration)).abs() < 1e-4);
            assert_eq!(result.4, LocomotionState::Air);
        }
    }

    #[test]
    fn loaded_falling_character_preserves_the_apex_ceiling_contact() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.1, 4.0), Transform::from_xyz(0.0, 10.9515, 0.0)));
        app.update();
        let start = Vec3::Y * 10.0;
        app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
            LocomotionState::Air, LinearVelocity(Vec3::Y * 0.2), TnuaMotor::default(),
            crate::als::character::AlsEffectiveGravity { value: Vec3::NEG_Y * 9.81, ..default() },
            AlsCmcAdapterRuntime::default()));
        app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::ZERO;
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
        app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
        let position = app.world().get::<Position>(actor).unwrap().0;
        assert!((position.y - 9.9961968).abs() < 0.0002, "position={position:?}");
    }

    #[test]
    fn apex_ceiling_collision_preserves_source_air_control_timing() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
            .add_message::<StateTransitionEvent>();
        app.finish();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.1, 4.0), Transform::from_xyz(0.0, 2.9515, 0.0)));
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        app.update();
        let result = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
            permissions: Query<&AlsMantleTargetPermission>,
            bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
            owners: Query<&ColliderOf>, shapes: Query<(&Collider, &Position, &Rotation)>,
            mut transitions: MessageWriter<StateTransitionEvent>| {
            let mut seed = 123;
            falling_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3), Transform::from_xyz(0.0, 2.0, 0.0),
                Vec3::Y * 0.2, Vec3::X, 6.5, &AlsMovementParams::default(), Vec3::NEG_Y * 9.81,
                (Stance::Standing, RotationMode::ViewDirection, Gait::Running, 0.0),
                (0.0, false, false), 40.0, 0.05, 0, actor, &mut seed,
                &candidates, &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, Some(20.0), None)
        }).unwrap();
        let expected_position = Vec3::new(0.00835248, 1.9963437, 0.0);
        let expected_velocity = Vec3::new(0.3, -0.2905, 0.0);
        assert!(result.0.translation.distance(expected_position) < 0.0003,
            "position={:?} source={expected_position:?}", result.0.translation);
        assert!(result.1.distance(expected_velocity) < 0.002, "velocity={:?} source={expected_velocity:?}", result.1);
    }

    #[test]
    fn falling_overlap_combines_competing_mtds_when_the_first_sweep_is_blocked() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
        app.finish();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 4.0, 4.0), Transform::IDENTITY));
        let normal = Vec3::new(0.5_f32.sqrt(), 0.0, -0.5_f32.sqrt());
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 4.0, 4.0),
            Transform::from_translation(normal * -0.256066).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4))));
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        app.update();
        let (first, second, resolved) = app.world_mut().run_system_once(move |spatial: SpatialQuery,
            impact_bodies: FallingImpactBodies,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
            let capsule = als_character_capsule(0.9, 0.3);
            let origin = Vec3::new(-0.15, 0.0, 0.0);
            let filter = SpatialQueryFilter::from_excluded_entities([actor]);
            let accepts = |candidate| character_blocks_collider(actor, candidate, &candidates);
            let hit = spatial.cast_shape_predicate(&capsule, origin, Quat::IDENTITY, Dir3::X,
                &ShapeCastConfig::from_max_distance(0.02), &filter, &accepts).unwrap();
            assert!(falling_penetration_adjustment(&spatial, &capsule, origin, Quat::IDENTITY,
                &hit, &filter, &accepts, &impact_bodies).is_none());
            let first = falling_penetration_vector(&hit, &impact_bodies).unwrap();
            let (direction, distance) = Dir3::new_and_length(first).unwrap();
            let second_hit = spatial.cast_shape_predicate(&capsule, origin, Quat::IDENTITY, direction,
                &ShapeCastConfig { ignore_origin_penetration: true, ..ShapeCastConfig::from_max_distance(distance) },
                &filter, &accepts).unwrap();
            let second = falling_penetration_vector(&second_hit, &impact_bodies).unwrap();
            let resolved = falling_resolve_penetration(&spatial, &capsule, origin, Quat::IDENTITY,
                &hit, Vec3::X * 0.02, &filter, &accepts, &impact_bodies).unwrap();
            (first, second, resolved)
        }).unwrap();
        assert_ne!(first, second);
        assert!(resolved.distance(first + second) < 1e-4, "first={first:?} second={second:?} resolved={resolved:?}");
        assert!(resolved.dot(first) > 0.0 && resolved.dot(second) > 0.0);
    }

    #[test]
    fn third_zero_time_hit_uses_the_side_sweep_to_choose_locomotion() {
        use bevy::ecs::system::RunSystemOnce;
        for (case, value) in [("floor-stuck", 3.0803), ("floor-positive", 3.077),
            ("slope-blocked", 0.0015), ("slope-escape", 0.00188)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            let start = Vec3::new(-13.0, 4.0, -15.0);
            let slope_case = case.starts_with("slope");
            let wall_x = if slope_case { -12.649 } else { -12.62 };
            let second_z = if slope_case { -14.6455 } else { -14.595 };
            for (center, size) in [(Vec3::new(wall_x, 4.0, -15.0), Vec3::new(0.1, 3.0, 2.0)),
                (Vec3::new(-13.0, 4.0, second_z), Vec3::new(2.0, 3.0, 0.1))] {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(size.x, size.y, size.z), Transform::from_translation(center)));
            }
            if slope_case {
                let rotation = Quat::from_rotation_z(-60.0_f32.to_radians());
                let center = Vec3::new(-13.0, 3.4, -15.0) - rotation * Vec3::Y * (0.32 + 0.5 * value);
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(1.3, 0.04, 2.0),
                    Transform::from_translation(center).with_rotation(rotation)));
            } else {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.0, 0.2, 2.0),
                    Transform::from_xyz(-13.0, value - 0.1, -15.0)));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                shapes: Query<(&Collider, &Position, &Rotation)>| {
                let cmc = crate::als::params::CmcAdapterParams::default();
                let mut seed = 123;
                let old_y = if slope_case { 0.0 } else { -1.0 };
                let step = 0.05;
                let velocity = Vec3::new(4.0, old_y - 9.81 * step, 4.0);
                let movement = Vec3::new(0.2, (old_y + velocity.y) * 0.5 * step, 0.2);
                falling_movement_sweep(&spatial, &als_character_capsule(0.9, 0.3), start, Quat::IDENTITY,
                    movement, velocity, step, None, Some((&cmc, 4.2, 3.75, &mut seed)), actor, &candidates, &shapes, &impact_bodies, None)
            }).unwrap();
            match case {
                "floor-stuck" => {
                    assert!(result.3.is_none() && result.4 == Some(LocomotionState::Air), "{result:?}");
                    assert!((0.01..0.016).contains(&result.0.x));
                    assert!((result.1.y + 1.4905).abs() < 0.002);
                }
                "floor-positive" => assert!(result.3.is_some() && result.4.is_none(), "{result:?}"),
                "slope-blocked" => assert!(result.3.is_some() && result.4 == Some(LocomotionState::Grounded), "{result:?}"),
                _ => {
                    assert!(result.3.is_none() && result.4 == Some(LocomotionState::Air), "{result:?}");
                    assert!(result.0.x < 0.0);
                }
            }
        }
    }

    #[test]
    fn side_sweep_locomotion_survives_one_post_physics_sync() {
        use bevy::ecs::system::RunSystemOnce;
        for mode in [LocomotionState::Grounded, LocomotionState::Air] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            if mode == LocomotionState::Grounded {
                let position = Vec3::new(0.0, 10.0, 0.0);
                app.world_mut().entity_mut(actor).insert((Position(position), Transform::from_translation(position)));
            }
            *app.world_mut().get_mut::<LocomotionState>(actor).unwrap() = mode;
            app.world_mut().get_mut::<AlsCmcAdapterRuntime>(actor).unwrap().post_collision_mode = Some(mode);
            app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
            assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), mode);
            assert!(app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().post_collision_mode.is_none());
        }
    }

    #[test]
    fn third_impact_landing_discards_the_entire_remaining_frame() {
        use bevy::ecs::system::RunSystemOnce;
        for (floor_top, third_impact) in [(3.04, true), (3.065, false)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .add_message::<StateTransitionEvent>();
            app.finish();
            let slope_rotation = Quat::from_rotation_y(-10.0_f32.to_radians()) * Quat::from_rotation_x(-60.0_f32.to_radians());
            let slope = Vec3::new(0.0, 3.4, 0.0) - slope_rotation * Vec3::Y * 0.385;
            for (center, rotation, size) in [
                (Vec3::new(0.38, 4.0, 0.0), Quat::IDENTITY, Vec3::new(0.1, 3.0, 2.0)),
                (slope, slope_rotation, Vec3::new(2.0, 0.04, 1.2)),
                (Vec3::new(0.0, floor_top - 0.1, 0.0), Quat::IDENTITY, Vec3::new(2.0, 0.2, 2.0)),
            ] {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(size.x, size.y, size.z),
                    Transform::from_translation(center).with_rotation(rotation)));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>,
                bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
                owners: Query<&ColliderOf>, shapes: Query<(&Collider, &Position, &Rotation)>, mut transitions: MessageWriter<StateTransitionEvent>| {
                let mut seed = 123;
                falling_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Transform::from_xyz(0.0, 4.0, 0.0), Vec3::new(4.0, -2.0, 2.0), Vec3::ZERO,
                    7.0, &AlsMovementParams::default(), Vec3::NEG_Y * 9.81,
                    (Stance::Standing, RotationMode::ViewDirection, Gait::Running, 0.0),
                    (0.0, false, false), 40.0, 0.1, 0, actor, &mut seed,
                    &candidates, &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, None, None)
            }).unwrap();
            assert_eq!(result.4, LocomotionState::Grounded, "floor={floor_top}");
            let expected_cap = if third_impact { 7.0 } else { 3.75 };
            assert!((result.6 - expected_cap).abs() < 1e-4,
                "floor={floor_top} walking cap={} expected={expected_cap}", result.6);
            if third_impact {
                assert!(result.1.x.abs() < 0.001 && (result.1.z + 0.57524).abs() < 0.002,
                    "third-impact landing kept the pre-collision velocity: {:?}", result.1);
            }
        }
    }

    #[test]
    fn falling_keeps_the_last_grounded_speed_cap_after_stance_changes() {
        use bevy::ecs::system::RunSystemOnce;
        for (stance, expected_cap, expected_speed) in [
            (Stance::Crouching, 2.0, 2.0),
            (Stance::Standing, 3.75, 2.15),
        ] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().entity_mut(actor).insert((stance, LocomotionState::Grounded,
                AlsMaxAllowedGait(Gait::Running), LinearVelocity(Vec3::X * 2.0)));
            app.world_mut().run_system_once(crate::als::dynamics::grounded_dynamics_motor_system).unwrap();
            assert!((app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().grounded_max_speed - expected_cap).abs() < 1e-5);
            let start = Vec3::new(0.0, 10.0, 0.0);
            app.world_mut().entity_mut(actor).insert((Stance::Standing, LocomotionState::Air,
                Position(start), Transform::from_translation(start), LinearVelocity(Vec3::X * 2.0), TnuaMotor::default()));
            app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::X;
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            let runtime = app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap();
            assert!((runtime.grounded_max_speed - expected_cap).abs() < 1e-5);
            let actual = runtime.grounded_collision_velocity.unwrap();
            assert!((actual.x - expected_speed).abs() < 1e-4, "stance={stance:?} velocity={actual:?}");
        }
    }

    #[test]
    fn landing_then_walking_off_inherits_vertical_base_velocity() {
        use bevy::ecs::system::RunSystemOnce;
        let mut baseline = None;
        for speed in [0.0, -2.0, 2.0] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .add_message::<StateTransitionEvent>();
            app.finish();
            let platform = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(1.0, 0.2, 2.0),
                Transform::from_xyz(0.0, 1.9, 0.0), LinearVelocity::ZERO)).id();
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            app.world_mut().get_mut::<LinearVelocity>(platform).unwrap().0 = Vec3::Y * speed;
            let result = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>,
                bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
                owners: Query<&ColliderOf>, shapes: Query<(&Collider, &Position, &Rotation)>,
                mut transitions: MessageWriter<StateTransitionEvent>| {
                let mut seed = 123;
                falling_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Transform::from_xyz(0.2, 3.02, 0.0), Vec3::new(3.75, -1.0, 0.0), Vec3::X,
                    3.75, &AlsMovementParams::default(), Vec3::NEG_Y * 9.81,
                    (Stance::Standing, RotationMode::ViewDirection, Gait::Running, 0.0),
                    (0.0, false, false), 40.0, 0.2, 0, actor, &mut seed,
                    &candidates, &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, None, None)
            }).unwrap();
            assert_eq!(result.4, LocomotionState::Air, "The character must land and leave the platform: {result:?}");
            let initial = *baseline.get_or_insert(result.1.y);
            assert!((result.1.y - initial - speed).abs() < 1e-4,
                "Landing and walk-off must inherit vertical base velocity: speed={speed} result={result:?} baseline={initial}");
        }
    }

    #[test]
    fn walking_off_inherits_vertical_base_velocity_before_falling() {
        use bevy::ecs::system::RunSystemOnce;
        for (speed, steps) in [(-2.0, 1), (0.0, 1), (2.0, 1), (-2.0, 2), (0.0, 2), (2.0, 2)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let platform = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(1.0, 0.2, 2.0),
                Transform::from_xyz(0.0, 1.9, 0.0), LinearVelocity::ZERO)).id();
            app.update();
            app.update();
            app.world_mut().get_mut::<LinearVelocity>(platform).unwrap().0 = Vec3::Y * speed;
            let start = Vec3::new(0.55, 2.9215, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LinearVelocity(Vec3::X * 3.75), LocomotionState::Grounded, TnuaMotor::default()));
            {
                let mut base = app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBase>(actor).unwrap();
                base.entity = Some(platform);
                base.location = Vec3::new(0.0, 1.9, 0.0);
                base.rotation = Quat::IDENTITY;
            }
            {
                let mut base = app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBaseDynamicsState>(actor).unwrap();
                base.entity = Some(platform);
                base.relative_velocity = Vec3::X * 3.75;
                base.support_velocity = Vec3::Y * speed;
                base.pre_collision_velocity = Some(Vec3::X * 3.75);
            }
            app.world_mut().get_mut::<crate::als::dynamics::AlsGroundedDynamics>(actor).unwrap().0 =
                crate::als::params::GroundedDynamics { acceleration: 0.0, braking_deceleration: 0.0, ground_friction: 0.0 };
            app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::ZERO;
            let delta = if steps == 1 { 0.05 } else { 0.1 };
            {
                let mut runtime = app.world_mut().get_mut::<AlsCmcAdapterRuntime>(actor).unwrap();
                runtime.simulation_substeps = steps;
                runtime.simulation_delta_seconds = delta;
                runtime.grounded_movement_delta = Some(Vec3::X * (3.75 * delta));
            }
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(delta));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), LocomotionState::Air);
            let velocity = app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().grounded_collision_velocity.unwrap();
            let expected = speed - if steps == 1 { 0.0 } else { 0.4905 };
            assert!((velocity.y - expected).abs() < 1e-4,
                "Walk-off vertical velocity differs: speed={speed} steps={steps} actual={velocity:?} expected={expected}");
            app.world_mut().run_system_once(crate::als::dynamics::grounded_dynamics_motor_system).unwrap();
            assert_eq!(app.world().get::<TnuaMotor>(actor).unwrap().lin.boost.y, 0.0,
                "The following airborne update must not inherit the base twice");
        }
    }

    #[test]
    fn walking_substeps_bind_the_new_floor_before_falling_or_physics_follow() {
        use bevy::ecs::system::RunSystemOnce;
        for (width, side_speed, falls, steps) in [
            (0.14, 1.0, true, 2), (0.14, 0.0, true, 2),
            (0.64, 0.0, false, 2), (0.64, 0.0, false, 1),
        ] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let old = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(2.0, 0.2, 3.0),
                Transform::from_xyz(0.0, 1.9, 0.0), LinearVelocity::ZERO)).id();
            let new = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(width, 0.2, 2.0),
                Transform::from_xyz(1.27, 1.88, 0.0), LinearVelocity::ZERO)).id();
            app.update();
            app.update();
            app.world_mut().get_mut::<LinearVelocity>(new).unwrap().0 = Vec3::Z * side_speed;
            let start = Vec3::new(1.02, 2.9215, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LinearVelocity(Vec3::X * 6.5), LocomotionState::Grounded));
            {
                let mut base = app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBase>(actor).unwrap();
                base.entity = Some(old);
                base.location = Vec3::new(0.0, 1.9, 0.0);
                base.rotation = Quat::IDENTITY;
            }
            {
                let mut base = app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBaseDynamicsState>(actor).unwrap();
                base.entity = Some(old);
                base.relative_velocity = Vec3::X * 6.5;
                base.support_velocity = Vec3::ZERO;
            }
            app.world_mut().get_mut::<crate::als::dynamics::AlsGroundedDynamics>(actor).unwrap().0 =
                crate::als::params::GroundedDynamics { acceleration: 0.0, braking_deceleration: 5.0, ground_friction: 0.0 };
            app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::ZERO;
            {
                let mut runtime = app.world_mut().get_mut::<AlsCmcAdapterRuntime>(actor).unwrap();
                runtime.simulation_substeps = steps;
                runtime.simulation_delta_seconds = if steps == 1 { 0.05 } else { 0.1 };
                runtime.grounded_movement_delta = Some(Vec3::X * if steps == 1 { 0.3125 } else { 0.6125 });
            }
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(if steps == 1 { 0.05 } else { 0.1 }));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            let mode = *app.world().get::<LocomotionState>(actor).unwrap();
            assert_eq!(mode, if falls { LocomotionState::Air } else { LocomotionState::Grounded });
            if falls {
                let velocity = app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().grounded_collision_velocity.unwrap();
                assert!((velocity.z - side_speed).abs() < 1e-4, "speed={side_speed} velocity={velocity:?}");
            } else {
                assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(actor).unwrap().entity, Some(new));
                if steps == 1 { app.world_mut().get_mut::<Position>(actor).unwrap().0.x += 0.3125; }
                let before = app.world().get::<Position>(actor).unwrap().0;
                app.world_mut().get_mut::<Position>(old).unwrap().0.z += 0.2;
                app.world_mut().get_mut::<Transform>(old).unwrap().translation.z += 0.2;
                app.world_mut().run_system_once((|| MovementBaseFollowPhase::AfterPhysics).pipe(follow_movement_base_system)).unwrap();
                let after = app.world().get::<Position>(actor).unwrap().0;
                assert!((after.z - before.z).abs() < 1e-5, "the previous base carried the actor: {before:?} -> {after:?}");
            }
        }
    }

    #[test]
    fn walking_substep_keeps_unobstructed_velocity_at_landing_origins() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
        app.finish();
        let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(100.0, 0.2, 100.0),
            Transform::from_xyz(0.0, -0.1, 0.0))).id();
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        app.update();
        for (start, velocity, delta) in [
            (Vec3::new(-7.866_695, 0.921_545_5, -15.298_997), Vec3::new(6.499_999_5, 0.0, 0.001_433_724_1), 0.019_570_196),
            (Vec3::new(-7.867_078_3, 0.921_545_27, -15.301_003), Vec3::new(6.499_999_5, 0.0, -0.001_433_586_1), 0.019_629_141),
        ] {
            app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>,
                bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
                owners: Query<&ColliderOf>, pawns: Query<(), With<AlsCharacter>>| {
                let capsule = als_character_capsule(0.9, 0.3);
                let dynamics = crate::als::params::GroundedDynamics { acceleration: 0.0, braking_deceleration: 0.0, ground_friction: 0.0 };
                let (_, actual, steps, _, lost_floor, _, _, _) = walking_movement_transaction(&spatial, &capsule,
                    Transform::from_translation(start), 0.9, 0.3, velocity, Vec3::ZERO, 6.5, dynamics,
                    &crate::als::params::CmcAdapterParams::default(), 0.0, delta, actor,
                    &candidates, &permissions, &bases, &owners, &pawns, None, None);
                assert_eq!(steps, 1);
                assert!(!lost_floor);
                assert_eq!(actual, velocity);
            }).unwrap();
        }
        app.world_mut().entity_mut(floor).insert(LinearVelocity(Vec3::X));
        app.world_mut().run_system_once(move |spatial: SpatialQuery,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
            permissions: Query<&AlsMantleTargetPermission>,
            bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
            owners: Query<&ColliderOf>, pawns: Query<(), With<AlsCharacter>>| {
            let start = Vec3::new(-7.866_695, 0.921_545_5, -15.298_997);
            let velocity = Vec3::new(6.499_999_5, 0.0, 0.001_433_724_1);
            let delta = 0.019_570_196;
            let capsule = als_character_capsule(0.9, 0.3);
            let dynamics = crate::als::params::GroundedDynamics { acceleration: 0.0, braking_deceleration: 0.0, ground_friction: 0.0 };
            let (_, actual, _, _, _, _, _, _) = walking_movement_transaction(&spatial, &capsule,
                Transform::from_translation(start), 0.9, 0.3, velocity, Vec3::ZERO, 6.5, dynamics,
                &crate::als::params::CmcAdapterParams::default(), 0.0, delta, actor,
                &candidates, &permissions, &bases, &owners, &pawns, Some(floor), None);
            assert_eq!(actual, ((start + velocity * delta) - start).with_y(0.0) / delta);
            assert_ne!(actual.z, velocity.z);
        }).unwrap();
    }

    #[test]
    fn walking_initial_penetration_preserves_the_planned_step_velocity() {
        use bevy::ecs::system::RunSystemOnce;
        for (gap, left_wall, seconds, expected_penetration) in [
            (Some(0.55), true, 0.1, true),
            (Some(0.55), false, 0.05, true),
            (Some(0.64), true, 0.1, false),
            (None, false, 0.1, false),
        ] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 20.0),
                Transform::from_xyz(0.0, -0.1, 0.0)));
            if let Some(gap) = gap {
                let sides: &[f32] = if left_wall { &[-1.0, 1.0] } else { &[1.0] };
                for side in sides {
                    app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.5, 2.0, 2.0),
                        Transform::from_xyz(side * (gap * 0.5 + 0.25), 1.7, 0.0)));
                }
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>,
                bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
                owners: Query<&ColliderOf>, pawns: Query<(), With<AlsCharacter>>| {
                let capsule = als_character_capsule(0.9, 0.3);
                let cmc = crate::als::params::CmcAdapterParams::default();
                let start = Transform::from_xyz(0.0, 0.9215, 0.0);
                let outcome = grounded_movement_transaction(&spatial, &capsule, start.translation, Quat::IDENTITY,
                    0.9, 0.3, Vec3::X * 0.3125, &cmc, LocomotionState::Grounded, actor, &candidates, &permissions, None).unwrap();
                assert_eq!(outcome.3, expected_penetration, "gap={gap:?} left={left_wall}");
                let dynamics = crate::als::params::GroundedDynamics { acceleration: 0.0, braking_deceleration: 5.0, ground_friction: 0.0 };
                let (_, velocity, _, _, _, _, _, _) = walking_movement_transaction(&spatial, &capsule, start, 0.9, 0.3,
                    Vec3::X * 6.5, Vec3::ZERO, 6.5, dynamics, &cmc, 0.0, seconds, actor, &candidates, &permissions, &bases, &owners, &pawns, None, None);
                if expected_penetration {
                    assert!((velocity.x - 6.25).abs() < 1e-4, "gap={gap:?} velocity={velocity:?}");
                } else if gap.is_some() {
                    assert!(velocity.x < 1.0, "a positive-time wall must still limit velocity: {velocity:?}");
                } else {
                    assert!(velocity.x > 5.9, "free walking lost forward velocity: {velocity:?}");
                }
            }).unwrap();
        }
    }

    #[test]
    fn single_walking_step_keeps_penetration_velocity_after_physics() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let start = app.world().get::<Position>(actor).unwrap().0;
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.5, 2.0, 2.0),
            Transform::from_translation(start + Vec3::new(0.525, 0.775, 0.0))));
        app.update();
        app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
            LinearVelocity(Vec3::X * 6.5), LocomotionState::Grounded));
        {
            let mut motor = app.world_mut().get_mut::<TnuaMotor>(actor).unwrap();
            motor.lin.boost = Vec3::X * -0.25;
            motor.lin.acceleration = Vec3::ZERO;
        }
        {
            let mut runtime = app.world_mut().get_mut::<AlsCmcAdapterRuntime>(actor).unwrap();
            runtime.simulation_substeps = 1;
            runtime.simulation_delta_seconds = 0.05;
            runtime.grounded_movement_delta = Some(Vec3::X * 0.3125);
        }
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
        app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
        let runtime = app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap();
        assert!(runtime.grounded_move_start.is_none());
        assert!((runtime.grounded_collision_velocity.unwrap().x - 6.25).abs() < 1e-4);
        app.world_mut().get_mut::<Position>(actor).unwrap().0.x -= 0.0125;
        app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
        app.world_mut().run_system_once(snapshot_actual_velocity_system).unwrap();
        assert!((app.world().get::<LinearVelocity>(actor).unwrap().x - 6.25).abs() < 1e-4);
    }

    #[test]
    fn single_walking_step_drops_stale_vertical_spring_after_leaving_floor() {
        use bevy::ecs::system::RunSystemOnce;
        for (supported, spring) in [(false, 9.11858), (false, -169.39), (true, 9.11858)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let (width, center) = if supported { (4.0, 0.0) } else { (1.0, -0.5) };
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(width, 0.2, 2.0),
                Transform::from_xyz(center, 1.05, 0.0)));
            app.update();
            let start = Vec3::new(0.2, 2.0715, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LinearVelocity(Vec3::X * 6.0), LocomotionState::Grounded));
            let gravity = app.world().get::<AlsEffectiveGravity>(actor).unwrap().value.y;
            {
                let mut motor = app.world_mut().get_mut::<TnuaMotor>(actor).unwrap();
                motor.lin.acceleration.y = -gravity + spring;
                motor.lin.boost.y = 0.0;
            }
            {
                let mut runtime = app.world_mut().get_mut::<AlsCmcAdapterRuntime>(actor).unwrap();
                runtime.simulation_substeps = 1;
                runtime.simulation_delta_seconds = 1.0 / 30.0;
                runtime.grounded_movement_delta = Some(Vec3::X * 0.2);
            }
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(1.0 / 30.0));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            let motor = app.world().get::<TnuaMotor>(actor).unwrap();
            assert!((motor.lin.acceleration.y + gravity).abs() <= 1e-6,
                "The previous floor spring survived: supported={supported} spring={spring}");
        }
    }

    #[test]
    fn single_walking_step_returns_blocked_distance_to_falling() {
        use bevy::ecs::system::RunSystemOnce;
        for (wall, supported) in [(true, false), (false, false), (false, true)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let width = if supported { 4.0 } else { 2.0 };
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(width, 0.2, 3.0),
                Transform::from_xyz(-15.0, 1.9, -15.0)));
            if wall {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.04, 4.0, 2.0),
                    Transform::from_xyz(-13.38, 3.0, -15.0)
                        .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4))));
            }
            app.update();
            let start = Vec3::new(-13.8911314, 2.9012036, -15.0);
            let desired = Vec3::X * 0.1941591;
            let seconds = 1.0 / 30.0;
            let pre_collision = desired / seconds;
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                LinearVelocity(Vec3::X * 5.5550895), LocomotionState::Grounded));
            app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::ZERO;
            {
                let mut motor = app.world_mut().get_mut::<TnuaMotor>(actor).unwrap();
                motor.lin.boost = Vec3::X * (pre_collision.x - 5.5550895);
                motor.lin.acceleration = Vec3::Y * 18.928581;
            }
            {
                let mut runtime = app.world_mut().get_mut::<AlsCmcAdapterRuntime>(actor).unwrap();
                runtime.simulation_substeps = 1;
                runtime.simulation_delta_seconds = seconds;
                runtime.grounded_movement_delta = Some(desired);
                runtime.grounded_max_speed = 6.5;
            }
            if let Some(mut base) = app.world_mut().get_mut::<crate::als::dynamics::AlsMovementBaseDynamicsState>(actor) {
                base.pre_collision_velocity = Some(pre_collision);
                base.entity = None;
                base.support_velocity = Vec3::ZERO;
            }
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(seconds));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            let mode = *app.world().get::<LocomotionState>(actor).unwrap();
            if supported {
                assert_eq!(mode, LocomotionState::Grounded);
            } else {
                assert_eq!(mode, LocomotionState::Air);
                let velocity = app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().grounded_collision_velocity.unwrap();
                if wall { assert!(velocity.y < -0.03 && velocity.y > -0.15, "Refunded time was not integrated: {velocity:?}"); }
                else { assert!(velocity.y.abs() < 1e-6, "An unobstructed step must not refund time: {velocity:?}"); }
            }
        }
    }

    #[test]
    fn walking_floor_penetration_is_not_treated_as_a_missing_floor() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
        app.finish();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
            Transform::from_xyz(0.0, -0.1, 0.0)));
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        app.update();
        let penetrating = app.world_mut().run_system_once(move |spatial: SpatialQuery,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
            walking_floor_penetrates(&spatial, Vec3::Y * 0.2, 0.9, 0.3, actor, &candidates)
        }).unwrap();
        assert!(penetrating);
    }

    #[test]
    fn falling_virtual_ditch_kicks_only_after_a_stalled_two_wall_sweep() {
        use bevy::ecs::system::RunSystemOnce;
        for (gap, perch, jump_z, expected_y) in [
            (Some(0.001), 0.15, 4.2, Some(1.05)),
            (Some(0.001), 0.15, 0.0, Some(0.01)),
            (Some(0.014), 0.15, 4.2, None),
            (None, 0.15, 4.2, None),
            (Some(0.001), 0.0, 4.2, None),
        ] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
            app.finish();
            let rotation = Quat::from_rotation_z(-40.0_f32.to_radians());
            let ledge = Vec3::new(0.0, 3.4, 0.0) - rotation * Vec3::Y * 0.311;
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.06, 0.02, 0.6),
                Transform::from_translation(ledge).with_rotation(rotation)));
            if let Some(gap) = gap {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 3.0, 2.0),
                    Transform::from_xyz(0.35 + gap, 4.0, 0.0)));
            }
            let actor = app.world_mut().spawn_empty().id();
            app.update();
            app.update();
            let (result, seed) = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                shapes: Query<(&Collider, &Position, &Rotation)>| {
                let cmc = crate::als::params::CmcAdapterParams { perch_radius_threshold: perch, ..default() };
                let mut seed = 123_u32;
                let result = falling_movement_sweep(&spatial, &als_character_capsule(0.9, 0.3),
                    Vec3::new(0.0, 4.0, 0.0), Quat::IDENTITY, Vec3::new(-0.025, -0.1122625, 0.001),
                    Vec3::new(-0.5, -2.4905, 0.02), 0.05, None, Some((&cmc, jump_z, 3.75, &mut seed)),
                    actor, &candidates, &shapes, &impact_bodies, None);
                (result, seed)
            }).unwrap();
            assert_eq!(seed, if expected_y.is_some() { 1238442549 } else { 123 }, "gap={gap:?} perch={perch}");
            if let Some(expected_y) = expected_y {
                assert!((result.1.y - expected_y).abs() < 1e-5,
                    "gap={gap:?} perch={perch} velocity={:?}", result.1);
                assert!(result.3.is_none(), "the virtual ditch must stay airborne");
            } else {
                assert!((result.1.y - 1.05).abs() > 1e-5, "gap={gap:?} perch={perch}");
            }
        }
    }

    #[test]
    fn custom_tags_follow_source_movement_fallbacks() {
        let stance = Stance::new("Als.Stance.Custom").unwrap();
        let gait = Gait::new("Als.Gait.Custom").unwrap();
        let rotation = RotationMode::new("Als.RotationMode.Custom").unwrap();
        assert_eq!(capsule_target_for_locomotion(stance, Stance::Crouching, LocomotionState::Grounded, false, None), Stance::Crouching);
        assert_eq!(capsule_target_for_locomotion(stance, Stance::Crouching, LocomotionState::Air, false, None), Stance::Standing);
        assert_eq!(turn_angvel(rotation), 0.0);
        assert_eq!(moving_rotation_half_life(&AlsMovementParams::default(), 2.0, RotationMode::ViewDirection, true, 0.0, stance), 0.2);
        let mut params = AlsMovementParams::default();
        params.speeds.standing.allow_direction_dependent_movement_speed = true;
        params.speeds.standing.max_walk_speed = 9.0;
        params.speeds.standing.max_run_backward_speed = 1.0;
        assert_eq!(params.gait_speeds(stance, RotationMode::ViewDirection).max_walk_speed, 1.75);
        assert_eq!(params.gait_speeds(Stance::Standing, rotation).max_run_speed, 3.75);
        assert_eq!(gait_max_speed_for_velocity(&params, &Stance::Standing, &gait, RotationMode::ViewDirection, Vec3::Z, 0.0), params.speeds.standing.max_run_speed);
    }

    #[test]
    fn native_custom_mode_below_source_minimum_keeps_velocity_without_moving() {
        for custom_position in [false, true] {
        for delta in [0.5e-6_f32, 1.0e-6] {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
            for _ in 0..30 { app.update(); }
            let before_velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            assert!(before_velocity.length() > 1.0, "Actual source character must enter Custom from existing movement");
            let mut transform = *app.world().get::<Transform>(character).unwrap();
            transform.translation.y += 5.0;
            app.world_mut().entity_mut(character).insert((Position(transform.translation), transform, GlobalTransform::from(transform)));
            if custom_position { app.world_mut().entity_mut(character).insert(avian3d::prelude::CustomPositionIntegration); }
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::ZERO;
            app.world_mut().get_mut::<AlsStateRequest>(character).unwrap().locomotion_state = Some(LocomotionState::Custom);
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
            app.update();
            assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Custom);
            let expected = if delta < crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS { before_velocity } else { Vec3::ZERO };
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            assert!(velocity.abs_diff_eq(expected, 1e-6),
                "Source StartNewPhysics only clears Custom velocity at Delta >= MIN: delta={delta} expected={expected:?} actual={velocity:?}");
            let position = app.world().get::<Position>(character).unwrap().0;
            assert!(position.abs_diff_eq(transform.translation, 1e-7),
                "Source below-min Custom and legal no-root Custom do not move: delta={delta} expected={:?} actual={position:?}", transform.translation);
            assert_eq!(*app.world().get::<RigidBody>(character).unwrap(), RigidBody::Dynamic);
            assert!(app.world().get::<AlsCustomMovementPhysicsFrame>(character).is_none());
            assert_eq!(app.world().get::<avian3d::prelude::CustomPositionIntegration>(character).is_some(), custom_position);
            if delta < crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS {
                app.world_mut().resource_mut::<Time<Virtual>>().pause();
                for _ in 0..4 {
                    app.update();
                    assert_eq!(app.world().get::<Position>(character).unwrap().0, position);
                    assert_eq!(app.world().get::<LinearVelocity>(character).unwrap().0, velocity);
                    assert!(app.world().get::<AlsCustomMovementPhysicsFrame>(character).is_none());
                }
                app.world_mut().resource_mut::<Time<Virtual>>().unpause();
                app.update();
                assert!(app.world().get::<Position>(character).unwrap().0.abs_diff_eq(position, 1e-7));
                app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0e-6)));
                app.update();
                assert!(app.world().get::<LinearVelocity>(character).unwrap().0.abs_diff_eq(Vec3::ZERO, 1e-6));
                assert_eq!(*app.world().get::<RigidBody>(character).unwrap(), RigidBody::Dynamic);
                assert!(app.world().get::<AlsCustomMovementPhysicsFrame>(character).is_none());
                assert_eq!(app.world().get::<avian3d::prelude::CustomPositionIntegration>(character).is_some(), custom_position);
            }
        }
        }
    }

    #[test]
    fn native_custom_mode_without_root_motion_keeps_actual_velocity_and_position_zero() {
        for airborne in [true, false] {
        for input in [Vec2::ZERO, Vec2::X] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            for _ in 0..30 { app.update(); }
            if airborne {
                let position = app.world().get::<Position>(character).unwrap().0 + Vec3::Y * 5.0;
                let mut transform = *app.world().get::<Transform>(character).unwrap();
                transform.translation = position;
                app.world_mut().entity_mut(character).insert((Position(position), transform, GlobalTransform::from(transform), LinearVelocity::ZERO));
            }
            app.world_mut().get_mut::<AlsStateRequest>(character).unwrap().locomotion_state = Some(LocomotionState::Custom);
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = input;
            app.update();
            assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Custom);
            assert!(app.world().get::<crate::als::mantle::AlsMantleState>(character).is_none_or(|state| !state.active));
            let position = app.world().get::<Position>(character).unwrap().0;
            let mut maximum_speed = 0.0_f32;
            for _ in 0..4 {
                app.update();
                maximum_speed = maximum_speed.max(app.world().get::<LinearVelocity>(character).unwrap().0.length());
            }
            let final_position = app.world().get::<Position>(character).unwrap().0;
            assert!(maximum_speed < 1e-5,
                "Source PhysCustom without root motion resets actual velocity: airborne={airborne} input={input:?} speed={maximum_speed}");
            assert!(final_position.abs_diff_eq(position, 1e-5),
                "Source PhysCustom without root motion cannot move the actor: airborne={airborne} input={input:?} actual={:?}", final_position - position);
        }
        }
    }


    fn stair_motion_fixture(rise: f32, ceiling: bool, descending: bool, yaw: f32) -> (App, Entity, f32) {
        use crate::als::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use crate::als::transition::AlsStateTransitionPlugin;
        use avian3d::prelude::PhysicsPlugins;
        use bevy::asset::AssetEvent;
        use bevy::time::TimeUpdateStrategy;
        use bevy_tnua::TnuaControllerPlugin;
        use bevy_tnua_avian3d::TnuaAvian3dPlugin;
        let frame_rotation = Quat::from_rotation_y(yaw);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            PhysicsPlugins::new(PostUpdate), TnuaAvian3dPlugin::new(Update),
            TnuaControllerPlugin::<AlsControlScheme>::new(Update),
            AlsStateTransitionPlugin, AlsCharacterPlugin, AlsMotionPlugin, crate::actual_motion::AlsActualMotionPlugin))
            .configure_sets(PostUpdate, crate::actual_motion::AlsActualMotionSet.after(AlsActualVelocitySet))
            .init_resource::<Assets<Mesh>>()
            .add_message::<AssetEvent<Mesh>>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        let params_value = AlsMovementParams::default();
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(params_value.clone());
        let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>().add(initial_als_control_scheme_config(&params_value));
        let mut bundle = AlsCharacterBundle::new(params, config);
        let start_z = if descending { -5.44 } else { 1.0 };
        let floor_y = if descending { 12.0 * rise } else { 0.0 };
        bundle.transform = Transform::from_translation(frame_rotation * Vec3::new(0.0, floor_y + 0.924, start_z))
            .with_rotation(frame_rotation);
        let character = app.world_mut().spawn(bundle).id();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 30.0), Transform::from_xyz(0.0, -0.1, 0.0)));
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for step in 0..12 {
            let front = -(step as f32) * 0.37;
            let bottom = step as f32 * rise;
            let top = bottom + rise;
            let index = vertices.len() as u32;
            vertices.extend([
                Vec3::new(-3.335, bottom, front), Vec3::new(3.335, bottom, front),
                Vec3::new(-3.335, top, front), Vec3::new(3.335, top, front),
                Vec3::new(-3.335, top, front - 0.37), Vec3::new(3.335, top, front - 0.37),
            ].map(|vertex| frame_rotation * vertex));
            triangles.extend([[index, index + 1, index + 2], [index + 1, index + 3, index + 2],
                [index + 2, index + 3, index + 4], [index + 3, index + 5, index + 4]]);
        }
        app.world_mut().spawn((RigidBody::Static, Collider::try_trimesh(vertices, triangles).unwrap(), Transform::default()));
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(6.67, 12.0 * rise, 6.0),
            Transform::from_translation(frame_rotation * Vec3::new(0.0, 6.0 * rise, -7.44))
                .with_rotation(frame_rotation)));
        if ceiling {
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(6.67, 0.2, 6.0),
                Transform::from_translation(frame_rotation * Vec3::new(0.0, 2.0, -3.0))
                    .with_rotation(frame_rotation)));
        }
        for _ in 0..30 {
            app.update();
        }
        (app, character, start_z)
    }

    #[test]
    fn loaded_jump_inherits_signed_vertical_platform_velocity() {
        #[derive(Resource, Default)]
        struct TakeoffVelocity(Option<f32>);
        for speed in [-2.0, 0.0, 2.0] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            app.init_resource::<TakeoffVelocity>().add_systems(Update,
                (move |mut sample: ResMut<TakeoffVelocity>, actors: Query<(&LinearVelocity, &TnuaMotor,
                    &TnuaController<AlsControlScheme>)>| {
                    let (velocity, motor, controller) = actors.get(character).unwrap();
                    if controller.action_flow_status().just_starting() == Some(AlsControlSchemeActionDiscriminant::Jump) {
                        sample.0 = Some(velocity.y + motor.lin.boost.y);
                    }
                }).after(crate::als::dynamics::grounded_dynamics_motor_system)
                    .before(cmc_movement_transaction_system));
            let platform = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(4.0, 0.4, 4.0),
                Transform::from_xyz(0.0, 0.0, 1.0), GravityScale(0.0))).id();
            let start = Vec3::new(0.0, 1.124, 1.0);
            app.world_mut().entity_mut(character).insert((Position(start), Transform::from_translation(start), LinearVelocity::ZERO));
            for _ in 0..30 { app.update(); }
            assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character).unwrap().entity,
                Some(platform), "The character must stand on the test platform");
            app.world_mut().get_mut::<LinearVelocity>(platform).unwrap().y = speed;
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
            app.update();
            let actual = app.world().resource::<TakeoffVelocity>().0.expect("The jump must start");
            let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap();
            let expected = app.world().resource::<Assets<AlsMovementParams>>().get(handle.id()).unwrap().jump.jump_z_velocity + speed;
            assert!((actual - expected).abs() < 1e-4,
                "Jump must add signed platform velocity: speed={speed} actual={actual} expected={expected}");
        }
    }

    #[test]
    fn browser_review_scaled_capsule_keeps_the_source_jump_velocity() {
        use crate::als::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use crate::als::transition::AlsStateTransitionPlugin;
        use bevy::asset::AssetEvent;
        use bevy::time::TimeUpdateStrategy;
        use bevy_tnua::TnuaControllerPlugin;
        use bevy_tnua_avian3d::TnuaAvian3dPlugin;
        #[derive(Resource, Default)]
        struct TakeoffSample(Option<(f32, Option<f32>)>);
        for scale in [0.5, 1.0, 2.0] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate), TnuaAvian3dPlugin::new(Update),
                TnuaControllerPlugin::<AlsControlScheme>::new(Update), AlsStateTransitionPlugin,
                AlsCharacterPlugin, AlsMotionPlugin, crate::actual_motion::AlsActualMotionPlugin))
                .configure_sets(PostUpdate, crate::actual_motion::AlsActualMotionSet.after(AlsActualVelocitySet))
                .init_resource::<Assets<Mesh>>().add_message::<AssetEvent<Mesh>>()
                .init_resource::<TakeoffSample>()
                .add_systems(Update, (|mut sample: ResMut<TakeoffSample>,
                    actors: Query<(&LinearVelocity, &TnuaMotor, &TnuaController<AlsControlScheme>)>| {
                    for (velocity, motor, controller) in &actors {
                        if controller.action_flow_status().just_starting() == Some(AlsControlSchemeActionDiscriminant::Jump) {
                            sample.0 = Some((velocity.y + motor.lin.boost.y, controller.basis_access().ok()
                                .and_then(|basis| TnuaBuiltinWalk::displacement(&basis).map(|value| value.y))));
                        }
                    }
                }).after(accepted_jump_state_system).before(cmc_movement_transaction_system)
                    .before(TnuaPipelineSystems::Motors))
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
            app.finish();
            let params = AlsMovementParams::default();
            let params_handle = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(params.clone());
            let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>().add(initial_als_control_scheme_config(&params));
            let expected_height = params.crouch.standing_capsule_half_height * scale
                + ALS_CHARACTER_FLOOR_CLEARANCE + ALS_CHARACTER_TNUA_FLOAT_EPSILON;
            let mut bundle = AlsCharacterBundle::new(params_handle, config.clone());
            bundle.transform = Transform::from_xyz(0.0, expected_height, 0.0).with_scale(Vec3::splat(scale));
            let character = app.world_mut().spawn(bundle).id();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 20.0), Transform::from_xyz(0.0, -0.1, 0.0)));
            for _ in 0..30 { app.update(); }
            let actual_height = app.world().resource::<Assets<AlsControlSchemeConfig>>().get(&config).unwrap().basis.float_height;
            assert!((actual_height - expected_height).abs() < 1e-5,
                "Scaled capsule floating height differs: scale={scale}, {actual_height} != {expected_height}");
            let standing_half = app.world().get::<Collider>(character).unwrap().shape_scaled().compute_local_aabb().half_extents().y;
            let top = app.world().resource::<Assets<AlsControlSchemeConfig>>().get(&config).unwrap()
                .basis.headroom.as_ref().unwrap().distance_to_collider_top;
            assert!((top - standing_half).abs() < 1e-5, "Scaled standing headroom differs from the physical capsule");
            app.world_mut().get_mut::<AlsStateRequest>(character).unwrap().stance = Some(Stance::Crouching);
            for _ in 0..10 { app.update(); }
            assert_eq!(*app.world().get::<Stance>(character).unwrap(), Stance::Crouching);
            let crouched_half = app.world().get::<Collider>(character).unwrap().shape_scaled().compute_local_aabb().half_extents().y;
            {
                let cfg = app.world().resource::<Assets<AlsControlSchemeConfig>>().get(&config).unwrap();
                assert!((cfg.crouch.float_offset - (crouched_half - standing_half)).abs() < 1e-5,
                    "Scaled crouch offset differs from the physical capsule change");
                assert!((cfg.basis.headroom.as_ref().unwrap().distance_to_collider_top - crouched_half).abs() < 1e-5,
                    "Scaled crouched headroom differs from the physical capsule");
            }
            app.world_mut().get_mut::<AlsStateRequest>(character).unwrap().stance = Some(Stance::Standing);
            for _ in 0..10 { app.update(); }
            assert_eq!(*app.world().get::<Stance>(character).unwrap(), Stance::Standing);
            app.world_mut().get_mut::<crate::als::input::AlsInputCommand>(character).unwrap().jump_pressed = true;
            app.update();
            let (velocity, displacement) = app.world().resource::<TakeoffSample>().0.expect("The real jump must start");
            let expected = params.jump.jump_z_velocity;
            assert!((velocity - expected).abs() < 1e-4,
                "Scaled capsule changed source takeoff velocity: scale={scale}, {velocity} != {expected}, displacement={displacement:?}");
        }
    }

    #[test]
    fn paused_move_events_do_not_refresh_source_actual_character_input() {
        use crate::als::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use crate::als::input::{AlsInputMap, AlsInputPlugin, spawn_als_actions};
        use crate::als::transition::AlsStateTransitionPlugin;
        use avian3d::prelude::PhysicsPlugins;
        use bevy::asset::AssetEvent;
        use bevy::time::TimeUpdateStrategy;
        use bevy::input::gamepad::{RawGamepadAxisChangedEvent, RawGamepadEvent};
        use bevy_enhanced_input::prelude::EnhancedInputPlugin;
        use bevy_tnua::TnuaControllerPlugin;
        use bevy_tnua_avian3d::TnuaAvian3dPlugin;
        for (initial, gamepad_input) in [(Vec2::ZERO,false),(Vec2::Y,false),(Vec2::ZERO,true),(Vec2::Y,true)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin, bevy::input::InputPlugin,
                EnhancedInputPlugin, AlsInputPlugin, PhysicsPlugins::new(PostUpdate), TnuaAvian3dPlugin::new(Update),
                TnuaControllerPlugin::<AlsControlScheme>::new(Update), AlsStateTransitionPlugin,
                AlsCharacterPlugin, AlsMotionPlugin, crate::actual_motion::AlsActualMotionPlugin))
                .configure_sets(PostUpdate, crate::actual_motion::AlsActualMotionSet.after(AlsActualVelocitySet))
                .init_resource::<Assets<Mesh>>().add_message::<AssetEvent<Mesh>>()
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
            app.finish();
            let params = AlsMovementParams::default();
            let handle = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(params.clone());
            let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>().add(initial_als_control_scheme_config(&params));
            let mut bundle = AlsCharacterBundle::new(handle, config);
            bundle.transform.translation = Vec3::new(0.0, 0.924, 0.0);
            let character = app.world_mut().spawn(bundle).id();
            let gamepad = app.world_mut().spawn(Gamepad::default()).id();
            spawn_als_actions(&mut app.world_mut().commands(), character, &AlsInputMap::default());
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 20.0), Transform::from_xyz(0.0, -0.1, 0.0)));
            for _ in 0..30 { app.update(); }
            if initial == Vec2::Y {
                if gamepad_input { app.world_mut().write_message(RawGamepadEvent::Axis(
                    RawGamepadAxisChangedEvent::new(gamepad, GamepadAxis::LeftStickY, -1.0))); }
                else { app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyW); }
            }
            for _ in 0..5 { app.update(); }
            let before = *app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap();
            app.world_mut().resource_mut::<Time<Virtual>>().pause();
            if gamepad_input {
                for (axis,value) in [(GamepadAxis::LeftStickY,0.0),(GamepadAxis::LeftStickX,1.0)] {
                    app.world_mut().write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(gamepad, axis, value)));
                }
            } else {
                let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                keys.release(KeyCode::KeyW);
                keys.press(KeyCode::KeyD);
            }
            for _ in 0..3 { app.update(); }
            let actual = app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap();
            assert_eq!((actual.has_input, actual.input_yaw_degrees), (before.has_input, before.input_yaw_degrees),
                "Source paused Character owner must preserve its last refreshed input for initial={initial:?}");
            assert_eq!(*actual, before, "Source paused owner must retain its complete motion snapshot");
            app.world_mut().resource_mut::<Time<Virtual>>().unpause();
            app.update();
            let resumed = app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap();
            assert!(resumed.has_input && (resumed.input_yaw_degrees + 90.0).abs() < 1e-5,
                "Source resumed Character must refresh the current held movement for device={gamepad_input}");
            let retained_yaw = resumed.input_yaw_degrees;
            if gamepad_input { app.world_mut().write_message(RawGamepadEvent::Axis(
                RawGamepadAxisChangedEvent::new(gamepad, GamepadAxis::LeftStickX, 0.0))); }
            else { app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::KeyD); }
            for _ in 0..3 {
                app.update();
                let released = app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap();
                assert!(!released.has_input && released.input_yaw_degrees == retained_yaw,
                    "Source Move release must consume zero each tick without clearing its retained direction");
                assert_eq!(app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion, Vec3::ZERO);
            }
            if gamepad_input { app.world_mut().write_message(RawGamepadEvent::Axis(
                RawGamepadAxisChangedEvent::new(gamepad, GamepadAxis::LeftStickX, 1.0))); }
            else { app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyD); }
            app.update();
            assert!(app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap().has_input);
            app.world_mut().resource_mut::<Time<Virtual>>().pause();
            if gamepad_input { app.world_mut().write_message(RawGamepadEvent::Axis(
                RawGamepadAxisChangedEvent::new(gamepad, GamepadAxis::LeftStickX, 0.0))); }
            else { app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::KeyD); }
            app.update();
            assert!(app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap().has_input);
            app.world_mut().resource_mut::<Time<Virtual>>().unpause();
            for _ in 0..3 {
                app.update();
                assert!(!app.world().get::<crate::actual_motion::AlsActualMotion>(character).unwrap().has_input,
                    "Source Move released while paused must not leave pending input on resume");
                assert_eq!(app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion, Vec3::ZERO);
            }
        }
    }
    #[test]
    fn admitted_spawn_bootstrap_preserves_source_grounded_velocity() {
        use bevy::time::TimeUpdateStrategy;
        for delta in [1.0_f32 / 120.0, 1.0 / 60.0, 1.0 / 30.0, 0.075, 0.12, 0.2, 0.25] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            crate::source_physics_timing::SourcePhysicsTimingPlugin.build(&mut app);
            let observed = std::sync::Arc::new(std::sync::Mutex::new(0.0_f32));
            let sampler = observed.clone();
            app.add_systems(PostUpdate, (move |velocities: Query<&LinearVelocity, With<AlsCharacter>>| {
                let mut peak = sampler.lock().unwrap();
                for velocity in &velocities { *peak = peak.max(velocity.0.y.abs()); }
            }).after(AlsActualVelocitySet));
            app.world_mut().insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
            let fit = Vec3::new(0.0, 0.904, 1.0);
            app.world_mut().entity_mut(character).insert((Position(fit), Transform::from_translation(fit),
                bevy_tnua::TnuaToggle::SenseOnly, GravityScale(0.0), LinearVelocity::ZERO,
                TnuaMotor::default(), TnuaController::<AlsControlScheme>::default()));
            for _ in 0..3 { app.update(); }
            app.world_mut().entity_mut(character).insert((bevy_tnua::TnuaToggle::Enabled,
                GravityScale(1.0), LinearVelocity::ZERO, TnuaMotor::default()));
            let mut peak = 0.0_f32;
            for _ in 0..20 {
                app.update();
                let world = app.world();
                let velocity = world.get::<LinearVelocity>(character).unwrap().0;
                peak = peak.max(velocity.y.abs());
            }
            assert!(peak <= 0.15, "Source grounded spawn velocity exceeded the unchanged actual gate: delta={delta}, peak={peak}");
            let sampled_peak = *observed.lock().unwrap();
            eprintln!("{}", serde_json::json!({"delta":delta,"completed_peak":peak,"sampled_peak":sampled_peak}));
            assert!(sampled_peak <= 0.15,
                "Source spawn observation must use the completed movement velocity: delta={delta}, peak={sampled_peak}");
        }
    }
    #[test]
    fn airborne_side_input_reaches_physics_without_losing_horizontal_momentum() {
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let start = Vec3::new(0.0, 10.0, 1.0);
        app.world_mut().get_mut::<Position>(character).unwrap().0 = start;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation = start;
        app.world_mut().get_mut::<LinearVelocity>(character).unwrap().0 = Vec3::NEG_Z * 3.75;
        *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
        app.world_mut().entity_mut(character).insert(AlsInputOverride(AlsInputCommand {
            move_dir: Vec2::X, ..default()
        }));
        let mut expected = Vec3::NEG_Z * 3.75;
        for frame in 0..15 {
            expected = (expected + Vec3::X * 3.0 / 60.0).clamp_length_max(3.75);
            app.update();
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            assert!(velocity.with_y(0.0).distance(expected) < 2e-5,
                "Air steering differed after physical integration: frame={frame} actual={velocity:?} expected={expected:?}");
            assert!(velocity.y < 0.0);
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Air));
        }
        app.world_mut().get_mut::<AlsInputOverride>(character).unwrap().0.move_dir = Vec2::ZERO;
        for _ in 0..10 {
            app.update();
            assert!(app.world().get::<LinearVelocity>(character).unwrap().0.with_y(0.0).distance(expected) < 2e-5);
        }
    }

    #[test]
    fn floor_height_adjustment_sweeps_the_actual_capsule_below_a_ceiling() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::als::character::AlsCollisionLayer;
        for (origin_y, ceiling, kind) in [
            (0.905, 1.81, "solid"), (0.905, 2.0, "solid"),
            (0.920, 2.0, "solid"), (1.005, 2.0, "solid"),
            (0.890, 2.0, "solid"), (0.905, 1.81, "sensor"),
            (0.905, 1.81, "disjoint"), (0.5815, 1.81, "uncrouch"),
        ] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            let origin = Vec3::new(0.0, origin_y, 1.0);
            app.world_mut().get_mut::<Position>(character).unwrap().0 = origin;
            app.world_mut().get_mut::<Transform>(character).unwrap().translation = origin;
            app.world_mut().get_mut::<LinearVelocity>(character).unwrap().0 = Vec3::ZERO;
            app.world_mut().entity_mut(character).insert(CollisionLayers::new(
                AlsCollisionLayer::Default, AlsCollisionLayer::Default));
            if kind == "uncrouch" {
                let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
                let params = app.world().resource::<Assets<AlsMovementParams>>().get(&handle).unwrap();
                let crouched = capsule_for(params, Stance::Crouching);
                app.world_mut().entity_mut(character).insert((crouched, Stance::Crouching,
                    AlsDesiredStance(Stance::Crouching), AlsCapsuleCrouched));
            }
            let mut roof = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.1, 1.0),
                Transform::from_xyz(0.0, ceiling + 0.05, 1.0)));
            if kind == "sensor" { roof.insert(Sensor); }
            if kind == "disjoint" {
                roof.insert(CollisionLayers::new(AlsCollisionLayer::Visibility, AlsCollisionLayer::Visibility));
            }
            let roof = roof.id();
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded));
            if kind == "uncrouch" {
                app.update();
                app.world_mut().get_mut::<AlsDesiredStance>(character).unwrap().0 = Stance::Standing;
            }
            for frame in 0..4 {
                app.update();
                let position = app.world().get::<Position>(character).unwrap().0;
                let constrained = ceiling < 1.82 && !matches!(kind, "sensor" | "disjoint");
                if constrained {
                    assert!(position.y <= ceiling - 0.9 + 1e-6,
                        "Post-physics ceiling penetration: kind={kind} frame={frame} position={position:?}");
                } else {
                    assert!((MIN_FLOOR_DISTANCE - 1e-4..=MAX_FLOOR_DISTANCE + 1e-4).contains(&(position.y - 0.9)),
                        "Floor clearance differs: kind={kind} frame={frame} position={position:?}");
                }
                assert_eq!(app.world().get::<Stance>(character), Some(&Stance::Standing));
                assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded));
                assert!(position.distance(app.world().get::<Transform>(character).unwrap().translation) < 1e-6);
                assert!(app.world().get::<LinearVelocity>(character).unwrap().length() < 1e-4,
                    "Height adjustment must not create locomotion velocity");
                app.world_mut().run_system_once(move |spatial: SpatialQuery, colliders: Query<&Collider>| {
                    let overlaps = spatial.shape_intersections(colliders.get(character).unwrap(), position,
                        Quat::IDENTITY, &SpatialQueryFilter::from_excluded_entities([character]));
                    if !matches!(kind, "sensor" | "disjoint") {
                        assert!(!overlaps.contains(&roof), "Actual capsule overlaps the roof: kind={kind} frame={frame}");
                    }
                }).unwrap();
            }
        }
    }

    #[test]
    fn step_up_checks_initial_and_higher_landing_permissions() {
        use bevy::ecs::system::RunSystemOnce;
        for (front_allowed, landing_allowed) in [(true, true), (true, false), (false, true), (false, false)] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            let old: Vec<_> = app.world_mut().query::<(Entity, &RigidBody)>().iter(app.world())
                .filter_map(|(entity, body)| (*body == RigidBody::Static).then_some(entity)).collect();
            for entity in old { app.world_mut().despawn(entity); }
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 20.0), Transform::from_xyz(0.0, -0.1, 0.0)));
            let front = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.2, 0.1, 4.0),
                Transform::from_xyz(0.5, 0.05, 0.0), AlsMantleTargetPermission { allows_step_up: front_allowed })).id();
            let landing = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(1.2, 0.2, 4.0),
                Transform::from_xyz(1.2, 0.1, 0.0), AlsMantleTargetPermission { allows_step_up: landing_allowed })).id();
            app.update();
            app.world_mut().run_system_once(move |spatial: SpatialQuery, colliders: Query<&Collider>,
                params: Res<Assets<AlsMovementParams>>, handles: Query<&AlsMovementParamsHandle>,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
                permissions: Query<&AlsMantleTargetPermission>| {
                let capsule = colliders.get(character).unwrap();
                let params = params.get(handles.get(character).unwrap().id()).unwrap();
                let start = Vec3::new(0.0, 0.924, 0.0);
                let filter = SpatialQueryFilter::from_excluded_entities([character]);
                let hit = spatial.cast_shape(capsule, start, Quat::IDENTITY, Dir3::X,
                    &ShapeCastConfig::from_max_distance(1.0), &filter).unwrap();
                assert_eq!(hit.entity, front, "The first surface must be separate from the landing");
                let floor = spatial.cast_shape(capsule, start + Vec3::new(1.0, 0.45, 0.0), Quat::IDENTITY,
                    Dir3::NEG_Y, &ShapeCastConfig::from_max_distance(0.6), &filter).unwrap();
                assert_eq!(floor.entity, landing, "The raised landing must belong to the separate surface");
                let result = grounded_movement_transaction(&spatial, capsule, start, Quat::IDENTITY,
                    params.crouch.standing_capsule_half_height, params.crouch.standing_capsule_radius,
                    Vec3::X, &params.cmc, LocomotionState::Grounded, character, &candidates, &permissions, None).unwrap();
                assert_eq!(result.2.is_some_and(|height| height > start.y + 0.1), front_allowed && landing_allowed,
                    "Source StepUp checks the initial surface and the higher downward hit: front={front_allowed} landing={landing_allowed} result={result:?}");
            }).unwrap();
        }
    }

    #[test]
    fn partial_forward_step_keeps_progress_before_a_blocking_wall() {
        use bevy::ecs::system::RunSystemOnce;
        for prior_advance in [0.0, 0.07] {
            for (delta_seconds, distance) in [(1.0 / 60.0, 0.325), (0.1, 0.325), (0.1, 0.2)] {
                for case in ["close", "none", "far", "zero", "ceiling", "low_ceiling", "too_high", "slide"] {
                    let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
                    for _ in 0..30 { app.update(); }
                    let old: Vec<_> = app.world_mut().query::<(Entity, &RigidBody)>().iter(app.world())
                        .filter_map(|(entity, body)| (*body == RigidBody::Static).then_some(entity)).collect();
                    for entity in old { app.world_mut().despawn(entity); }
                    app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 20.0), Transform::from_xyz(0.0, -0.1, 0.0)));
                    let height = if case == "too_high" { 0.55 } else { 0.2 };
                    app.world_mut().spawn((RigidBody::Static, Collider::cuboid(1.0, height, 20.0), Transform::from_xyz(0.9, height * 0.5, 0.0)));
                    let mut blockers = Vec::new();
                    if case != "none" {
                        let face = if case == "far" { 0.9 } else if case == "zero" { 0.4 } else { 0.58 };
                        blockers.push(app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.2, 4.0, 20.0), Transform::from_xyz(face + 0.1, 2.0, 0.0))).id());
                    }
                    if matches!(case, "ceiling" | "low_ceiling") {
                        let bottom = if case == "ceiling" { 1.825 } else { 1.85 };
                        blockers.push(app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 20.0), Transform::from_xyz(0.0, bottom + 0.1, 0.0))).id());
                    }
                    app.update();
                    let start = Vec3::new(prior_advance, 0.924, 0.0);
                    app.world_mut().entity_mut(character).insert((Position(start), Transform::from_translation(start), LinearVelocity::ZERO, LocomotionState::Grounded));
                    let direction = if case == "slide" { Vec3::new(1.0, 0.0, 0.3).normalize() } else { Vec3::X };
                    let request = direction * distance;
                    app.world_mut().get_mut::<AlsCmcAdapterRuntime>(character).unwrap().grounded_movement_delta = Some(request);
                    let mut time = Time::<()>::default(); time.advance_by(std::time::Duration::from_secs_f32(delta_seconds)); app.insert_resource(time);
                    let mut motor = app.world_mut().get_mut::<TnuaMotor>(character).unwrap();
                    motor.lin.boost = request / delta_seconds; motor.lin.acceleration = Vec3::ZERO;
                    app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
                    let position = app.world().get::<Position>(character).unwrap().0;
                    let motor = app.world().get::<TnuaMotor>(character).unwrap();
                    let endpoint = position + (motor.lin.boost + motor.lin.acceleration * delta_seconds).with_y(0.0) * delta_seconds;
                    if matches!(case, "zero" | "ceiling" | "too_high") {
                        assert!((endpoint.y - start.y).abs() < 0.0001, "Failed step did not roll back: {case} {endpoint:?}");
                    } else if case == "low_ceiling" {
                        assert!(endpoint.y >= start.y && endpoint.y <= 0.95, "Partial rise exceeded available headroom: {endpoint:?}");
                    } else {
                        assert!(endpoint.y > 1.02, "Valid step lost its landing: {case} {endpoint:?}");
                        if matches!(case, "none" | "far") {
                            assert!((endpoint.x - start.x - request.x).abs() < 0.0001);
                        } else if start.x + request.x > 0.28 {
                            assert!(endpoint.x > 0.25 && endpoint.x < 0.28, "Partial step progress lost: {case} {endpoint:?}");
                        } else { assert!((endpoint.x - start.x - request.x).abs() < 0.011); }
                        if case == "slide" { assert!((endpoint.z - request.z).abs() < 0.0001, "Step slide discarded progress: {endpoint:?}"); }
                    }
                    let checked = if matches!(case, "zero" | "ceiling" | "too_high") { position } else { endpoint };
                    app.world_mut().run_system_once(move |spatial: SpatialQuery, colliders: Query<&Collider>| {
                        let overlaps = spatial.shape_intersections(colliders.get(character).unwrap(), checked, Quat::IDENTITY,
                            &SpatialQueryFilter::from_excluded_entities([character]));
                        assert!(blockers.iter().all(|entity| !overlaps.contains(entity)), "Committed step position overlaps a blocker: {case} {checked:?}");
                    }).unwrap();
                }
            }
        }
    }

    #[test]
    fn stair_contact_reproduces_the_terminal_low_input_boundary() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::time::TimeUpdateStrategy;
        let (mut app, character, _) = stair_motion_fixture(0.36, false, false, 0.0);
        let offset = Vec3::new(-26.665, 15.48, -45.91);
        let mut roots = app.world_mut().query_filtered::<(&mut Transform, &mut Position), Without<ChildOf>>();
        for (mut transform, mut position) in roots.iter_mut(app.world_mut()) {
            transform.translation += offset;
            position.0 += offset;
        }
        let start = offset + Vec3::new(0.0, 0.924, 0.3);
        app.world_mut().get_mut::<Transform>(character).unwrap().translation = start;
        app.world_mut().get_mut::<Position>(character).unwrap().0 = start;
        app.world_mut().get_mut::<crate::als::gait::AlsDesiredGait>(character).unwrap().0 = Gait::Walking;
        *app.world_mut().resource_mut::<TimeUpdateStrategy>() = TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.0093024));
        for _ in 0..30 { app.update(); }
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y * 0.11500549;
        for _ in 0..240 { app.update(); }
        let position = app.world().get::<Position>(character).unwrap().0;
        let base = *app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
        let delta = (base.world_velocity + base.motor_boost) * 0.0093024;
        let (forward, floor) = app.world_mut().run_system_once(move |spatial: SpatialQuery, colliders: Query<&Collider>| {
            let capsule = colliders.get(character).unwrap();
            let filter = SpatialQueryFilter::from_excluded_entities([character]);
            let forward = spatial.cast_shape(capsule, position, Quat::IDENTITY, Dir3::NEG_Z,
                &ShapeCastConfig::from_max_distance(delta.length()), &filter);
            let destination = position + delta + Vec3::Y * (MAX_STEP_HEIGHT - 0.024);
            let floor = spatial.cast_shape(capsule, destination, Quat::IDENTITY, Dir3::NEG_Y,
                &ShapeCastConfig::from_max_distance(MAX_STEP_HEIGHT + 0.048), &filter);
            (forward, floor)
        }).unwrap();
        let floor = floor.unwrap();
        let floor_offset = (floor.point1 - position - delta).with_y(0.0).length();
        assert!(floor_offset >= 0.3 - EDGE_REJECT_DISTANCE);
        assert!(forward.is_some_and(|hit| hit.distance == 0.0));
        assert!((position.z - start.z).abs() < 0.001);
        println!("[STAIR-CONTACT] source_edge_rejected distance={floor_offset} delta={delta:?}");
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Grounded);
        let after_low = position;
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
        for _ in 0..120 { app.update(); }
        let after_full = app.world().get::<Position>(character).unwrap().0;
        println!("[STAIR-CONTACT] full_input position={after_full:?} distance={}", after_low.z - after_full.z);
        assert!(after_low.z - after_full.z > 0.37, "Full keyboard input did not clear the next stair");
    }

    #[test]
    fn uphill_jump_landing_keeps_supported_capsules_out_of_air() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::time::TimeUpdateStrategy;
        let mut failures = Vec::new();
        for rise in [0.18, 0.26, 0.36] {
            for gait in [Gait::Running, Gait::Sprinting] {
                for launch_z in [0.55, 0.15, -0.22] {
                    for delta in [1.0 / 60.0, 1.0 / 120.0] {
                        let (mut app, character, _) = stair_motion_fixture(rise, false, false, 0.0);
                        *app.world_mut().resource_mut::<TimeUpdateStrategy>() = TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta));
                        app.world_mut().get_mut::<crate::als::gait::AlsDesiredGait>(character).unwrap().0 = gait;
                        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
                        for _ in 0..180 {
                            app.update();
                            if app.world().get::<Position>(character).unwrap().z <= launch_z { break; }
                        }
                        let launch = app.world().get::<Position>(character).unwrap().0;
                        let mut airborne = false;
                        let mut landed = false;
                        let mut supported_air_frames = 0;
                        for frame in 0..180 {
                            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = frame == 0;
                            app.update();
                            let mode = *app.world().get::<LocomotionState>(character).unwrap();
                            airborne |= mode == LocomotionState::Air;
                            let position = app.world().get::<Position>(character).unwrap().0;
                            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
                            if airborne && mode == LocomotionState::Grounded {
                                landed = position.y > launch.y + 0.05;
                                break;
                            }
                            if airborne && velocity.y.abs() < 0.001 {
                                let supported = app.world_mut().run_system_once(move |spatial: SpatialQuery, collisions: Collisions,
                                    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
                                    let floor = find_ground_floor(&spatial, position, 0.9, 0.3, &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded, &SpatialQueryFilter::from_excluded_entities([character]), character, &candidates);
                                    let contact = collisions.collisions_with(character).any(|pair| pair.manifolds.iter().any(|manifold| {
                                        let normal = if pair.collider1 == character { -manifold.normal } else { manifold.normal };
                                        normal.y > 0.0 && manifold.points.iter().any(|point| point.normal_impulse > 0.0)
                                    }));
                                    contact && floor.is_some_and(|(center, _, _)| (position.y - center).abs() <= MAX_FLOOR_DISTANCE)
                                }).unwrap();
                                supported_air_frames = if supported { supported_air_frames + 1 } else { 0 };
                                if supported_air_frames >= 2 {
                                    let evidence = app.world_mut().run_system_once(move |spatial: SpatialQuery, collisions: Collisions,
                                    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
                                        let floor = find_ground_floor(&spatial, position, 0.9, 0.3, &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded, &SpatialQueryFilter::from_excluded_entities([character]), character, &candidates);
                                        let normals: Vec<_> = collisions.collisions_with(character).flat_map(|pair| pair.manifolds.iter().map(|manifold| {
                                            let normal = if pair.collider1 == character { -manifold.normal } else { manifold.normal };
                                            (normal, manifold.points.iter().map(|point| (point.normal_speed, point.normal_impulse)).collect::<Vec<_>>())
                                        })).collect();
                                        (floor, normals)
                                    }).unwrap();
                                    println!("[UPHILL-SUPPORT] rise={rise} gait={gait:?} launch_z={launch_z} position={position:?} evidence={evidence:?}");
                                    failures.push((rise, gait, launch_z, delta, frame, position, velocity, "supported capsule remained Air"));
                                    break;
                                }
                            }
                        }
                        if !airborne || (!landed && supported_air_frames < 2) {
                            failures.push((rise, gait, launch_z, delta, 180, app.world().get::<Position>(character).unwrap().0,
                                app.world().get::<LinearVelocity>(character).unwrap().0, "uphill jump or landing was not observed"));
                        }
                        if landed {
                            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::ZERO;
                            for rest in 0..30 {
                                app.update();
                                if *app.world().get::<LocomotionState>(character).unwrap() != LocomotionState::Grounded {
                                    failures.push((rise, gait, launch_z, delta, 180 + rest, app.world().get::<Position>(character).unwrap().0,
                                        app.world().get::<LinearVelocity>(character).unwrap().0, "landed capsule returned to Air"));
                                    break;
                                }
                            }
                            let controller = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap();
                            if controller.action_flow_status().ongoing() == Some(AlsControlSchemeActionDiscriminant::Jump) {
                                failures.push((rise, gait, launch_z, delta, 210, app.world().get::<Position>(character).unwrap().0,
                                    app.world().get::<LinearVelocity>(character).unwrap().0, "landed controller remained in Jump action"));
                            }
                            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
                            app.update();
                            if app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().action_flow_status().ongoing()
                                != Some(AlsControlSchemeActionDiscriminant::Jump) {
                                println!("[UPHILL-REJUMP] rise={rise} gait={gait:?} launch_z={launch_z} delta={delta} mantle={:?} mode={:?} controller={:?}",
                                    app.world().get::<AlsMantleState>(character), app.world().get::<LocomotionState>(character),
                                    app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().action_flow_status());
                                failures.push((rise, gait, launch_z, delta, 211, app.world().get::<Position>(character).unwrap().0,
                                    app.world().get::<LinearVelocity>(character).unwrap().0, "jump was unavailable after landing"));
                            }
                            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = false;
                            app.update();
                            let was_air = *app.world().get::<LocomotionState>(character).unwrap() == LocomotionState::Air;
                            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
                            app.update();
                            if was_air {
                                assert_ne!(app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().action_flow_status().just_starting(),
                                    Some(AlsControlSchemeActionDiscriminant::Jump),
                                    "An airborne input started an extra jump: rise={rise} gait={gait:?} launch_z={launch_z} delta={delta}");
                            }
                        }
                        println!("[UPHILL-JUMP] rise={rise} gait={gait:?} launch_z={launch_z} delta={delta} airborne={airborne} landed={landed} supported_air_frames={supported_air_frames}");
                    }
                }
            }
        }
        assert!(failures.is_empty(), "Uphill landing failed: {failures:?}");
    }

    #[test]
    fn ground_floor_query_keeps_the_response_while_mantle_owns_physics() {
        use crate::als::character::{AlsCollisionLayer, AlsCollisionLayerOwner};
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let original = Some(CollisionLayers::new(AlsCollisionLayer::Default, AlsCollisionLayer::Default));
        app.world_mut().entity_mut(character).insert(original.unwrap());
        let position = app.world().get::<Position>(character).unwrap().0;
        let query_floor = move |spatial: SpatialQuery,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
            find_ground_floor(&spatial, position, 0.9, 0.3, &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                &SpatialQueryFilter::from_excluded_entities([character]), character, &candidates)
        };
        let original_floor = app.world_mut().run_system_once(query_floor).unwrap().unwrap().2;
        let masked = app.world_mut().get_mut::<AlsCollisionLayerOwnership>(character).unwrap()
            .set_owner(AlsCollisionLayerOwner::Mantle, original, true).unwrap().unwrap();
        app.world_mut().entity_mut(character).insert(masked);
        let masked_floor = app.world_mut().run_system_once(query_floor).unwrap();
        assert_eq!(masked_floor.map(|(_, _, entity)| entity), Some(original_floor),
            "temporary physics masking erased the query response");
        let restored = app.world_mut().get_mut::<AlsCollisionLayerOwnership>(character).unwrap()
            .set_owner(AlsCollisionLayerOwner::Mantle, Some(masked), false).unwrap().unwrap();
        app.world_mut().entity_mut(character).insert(restored);
        assert_eq!(app.world_mut().run_system_once(query_floor).unwrap().map(|(_, _, entity)| entity), Some(original_floor));
        app.world_mut().entity_mut(character).insert(CollisionLayers::NONE);
        assert!(app.world_mut().run_system_once(query_floor).unwrap().is_none(),
            "an inactive ownership record overrode the current response");
    }

    #[test]
    fn rejected_perch_clears_cached_support_and_prevents_contact_landing() {
        for mode in [LocomotionState::Grounded, LocomotionState::Air] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            for _ in 0..30 { app.update(); }
            let old_base = app.world().get::<crate::als::dynamics::AlsMovementBase>(character).unwrap().entity;
            assert!(old_base.is_some());
            let obstacles: Vec<_> = app.world_mut().query::<(Entity, &RigidBody)>().iter(app.world())
                .filter_map(|(entity, body)| (*body == RigidBody::Static).then_some(entity)).collect();
            for obstacle in obstacles { app.world_mut().despawn(obstacle); }
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 4.0, 4.0),
                Transform::from_xyz(-2.0, -2.0, 0.0)));
            let center = Vec3::new(0.2, 0.83, -1.0);
            app.world_mut().entity_mut(character).insert((Transform::from_translation(center), Position(center),
                LinearVelocity(Vec3::NEG_Y * 0.3), mode, AlsInputOverride(AlsInputCommand::default())));
            for frame in 0..20 {
                app.update();
                assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air,
                    "A rejected perch regained Grounded through contact at frame {frame}, starting {mode:?}");
                assert!(app.world().get::<crate::als::dynamics::AlsMovementBase>(character).unwrap().entity.is_none());
            }
        }
    }

    #[test]
    fn actual_walkable_line_floor_keeps_source_side_penetration_recovery() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        let mut transform = *app.world().get::<Transform>(character).unwrap();
        transform.translation.y = 0.9215;
        let origin = transform.translation;
        app.world_mut().entity_mut(character).insert((Position(origin), transform,
            GlobalTransform::from(transform), LinearVelocity::ZERO));
        let wall_x = origin.x + 0.1;
        let wall = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 0.5, 4.0),
            Transform::from_xyz(wall_x + 0.05, origin.y, origin.z))).id();
        app.update();
        let before = app.world().get::<Position>(character).unwrap().0;
        let remaining = app.world_mut().run_system_once(move |spatial: SpatialQuery| {
            let capsule = als_character_capsule(0.36, 0.298499);
            spatial.cast_shape(&capsule, origin, Quat::IDENTITY, Dir3::NEG_Y,
                &ShapeCastConfig::from_max_distance(1.015), &SpatialQueryFilter::from_excluded_entities([character]))
        }).unwrap().unwrap();
        assert_eq!(remaining.entity, wall);
        assert_eq!(remaining.distance, 0.0);
        assert!((remaining.point1 - remaining.point2).dot(remaining.normal1) > 0.0,
            "Source retry sweep must retain a real side penetration");
        let ray = app.world_mut().run_system_once(move |spatial: SpatialQuery| {
            spatial.cast_ray(before, Dir3::NEG_Y, 1.375, false, &SpatialQueryFilter::from_excluded_entities([character]))
        }).unwrap().unwrap();
        assert!(ray.normal.y > 0.9 && ray.distance > 0.9,
            "Source line floor must remain a valid center ray while the sweep is penetrating: {ray:?}");
        let found = app.world_mut().run_system_once(move |spatial: SpatialQuery,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
            find_ground_floor(&spatial, before, 0.9, 0.3, &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                &SpatialQueryFilter::from_excluded_entities([character]), character, &candidates)
        }).unwrap();
        assert!(found.is_some(), "Source current line floor must remain valid while side sweep starts penetrating: {found:?}");
        let source_minimum_x = wall_x - 0.3;
        app.update();
        let actual = app.world().get::<Position>(character).unwrap().0;
        assert!(actual.x <= source_minimum_x + 0.002,
            "Source walkable line floor still resolves side penetration before the final walking iteration: before={before:?}, actual={actual:?}, maximum_x={source_minimum_x}");
    }

    #[test]
    fn line_floor_height_guard_uses_both_source_distances() {
        let min = MIN_FLOOR_DISTANCE;
        for (sweep, line, expected) in [
            (-0.3_f32, Some(0.095_f32), true),
            (min - 0.0001, Some(min), true),
            (min, Some(min), false),
            (min - 0.0001, Some(min - 0.0001), false),
            (-0.3, None, false),
        ] {
            assert_eq!(line_floor_blocks_height_adjustment(sweep, line), expected, "sweep={sweep} line={line:?}");
        }
    }

    #[test]
    fn line_floor_preserves_height_while_resolving_side_penetration() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        let wall_x = 0.1;
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.1, 0.5, 4.0),
            Transform::from_xyz(wall_x + 0.05, 1.0, 0.0)));
        app.update();
        let start = Vec3::new(0.0, 1.0, 0.0);
        app.world_mut().entity_mut(character).insert((Position(start), Transform::from_translation(start),
            GlobalTransform::from_translation(start), LinearVelocity::ZERO, LocomotionState::Grounded,
            AlsCmcAdapterRuntime::default()));
        let (floor, adjusted) = app.world_mut().run_system_once(move |spatial: SpatialQuery,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
            let cmc = crate::als::params::CmcAdapterParams::default();
            let filter = SpatialQueryFilter::from_excluded_entities([character]);
            let floor = find_ground_floor_with_penetration(&spatial, start, 0.9, 0.3, &cmc,
                LocomotionState::Grounded, &filter, character, &candidates, None, None).unwrap();
            let mut transform = Transform::from_translation(start);
            let support = adjust_walking_floor_height(&spatial, &als_character_capsule(0.9, 0.3),
                &mut transform, 0.9, 0.3, &cmc, character, &candidates);
            assert_eq!(support, Some(floor.hit.entity));
            (floor, transform.translation)
        }).unwrap();
        assert_eq!(floor.origin, GroundFloorOrigin::Line);
        assert!(floor.sweep_distance < MIN_FLOOR_DISTANCE);
        assert!(floor.line_distance.unwrap() >= MIN_FLOOR_DISTANCE);
        assert!(floor.penetration.is_some());
        assert_eq!(adjusted.y, start.y);
        app.world_mut().run_system_once(locomotion_state_sync_system).unwrap();
        let actual = app.world().get::<Position>(character).unwrap().0;
        assert!((actual.y - start.y).abs() < 1e-6, "before={start:?} after={actual:?}");
        assert!(actual.x <= wall_x - 0.3 + 0.002, "Side penetration was not resolved: {actual:?}");
    }

    #[test]
    fn source_valid_downward_floor_sweep_matches_current_capsule_recheck() {
        use bevy::ecs::system::RunSystemOnce;
        for scale in [0.5, 1.0, 1.5] {
            for mode in [LocomotionState::Grounded, LocomotionState::Air] {
                let mut app = App::new();
                app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                    avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                    .init_asset::<Mesh>().add_message::<bevy::asset::AssetEvent<Mesh>>();
                app.finish();
                let character = app.world_mut().spawn((RigidBody::Kinematic, Transform::default())).id();
                let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(8.0, 0.1, 8.0),
                    Transform::from_xyz(0.0, -0.05, 0.0))).id();
                for _ in 0..2 { app.update(); }
                app.world_mut().run_system_once(move |spatial: SpatialQuery,
                    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
                    let half = 0.9 * scale;
                    let radius = 0.3 * scale;
                    let filter = SpatialQueryFilter::from_excluded_entities([character]);
                    let origin = Vec3::Y * (half + 0.1);
                    let capsule = als_character_capsule(half, radius);
                    let supplied = spatial.cast_shape(&capsule, origin, Quat::IDENTITY, Dir3::NEG_Y,
                        &ShapeCastConfig::from_max_distance(0.2), &filter).unwrap();
                    assert_eq!(supplied.entity, floor);
                    assert!(supplied.distance > 0.0 && supplied.normal1.y > 0.99);
                    let location = origin - Vec3::Y * supplied.distance;
                    assert!((supplied.point1 - location).with_y(0.0).length() < radius - EDGE_REJECT_DISTANCE);
                    let query_origin = location + Vec3::Y * 0.0215;
                    let found = find_ground_floor(&spatial, query_origin, half, radius,
                        &crate::als::params::CmcAdapterParams::default(), mode, &filter, character, &candidates).unwrap();
                    assert_eq!(found.2, supplied.entity);
                    assert!((found.0 - location.y).abs() < 1e-4,
                        "Supplied downward walkable sweep and current floor recheck differ: scale={scale} mode={mode:?} supplied={supplied:?} found={found:?}");
                }).unwrap();
            }
        }
    }

    #[test]
    fn source_floor_edge_rejection_retries_the_capsule_before_accepting_no_floor() {
        use bevy::ecs::system::RunSystemOnce;
        for edge_x in [0.2986_f32, 0.299, 0.2995] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .init_asset::<Mesh>().add_message::<bevy::asset::AssetEvent<Mesh>>();
            app.finish();
            let character = app.world_mut().spawn((RigidBody::Kinematic, Transform::default())).id();
            let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(8.0, 0.1, 8.0),
                Transform::from_xyz(0.0, -0.25, 0.0))).id();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.004, 1.5, 0.2),
                Transform::from_xyz(edge_x + 0.002, 0.5, 0.0)));
            for _ in 0..2 { app.update(); }
            let found = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
                let cmc = crate::als::params::CmcAdapterParams { perch_radius_threshold: 0.0, ..default() };
                let origin = Vec3::new(0.0, 0.919, 0.0);
                let capsule = als_character_capsule(0.84, 0.3);
                let raw = spatial.cast_shape(&capsule, origin, Quat::IDENTITY, Dir3::NEG_Y,
                    &ShapeCastConfig::from_max_distance(0.535), &SpatialQueryFilter::from_excluded_entities([character])).unwrap();
                let edge_distance = Vec2::new(raw.point1.x, raw.point1.z).length();
                assert!(edge_distance >= 0.2985,
                    "Source edge-retry fixture must reject its first capsule hit: edge={edge_x}, distance={edge_distance}, raw={raw:?}");
                find_ground_floor(&spatial, Vec3::new(0.0, 0.919, 0.0), 0.9, 0.3, &cmc, LocomotionState::Grounded,
                    &SpatialQueryFilter::from_excluded_entities([character]), character, &candidates)
            }).unwrap();
            assert_eq!(found.map(|found| found.2), Some(floor),
                "Source rejected near-radius hit retries its reduced capsule and keeps the lower walkable floor: edge={edge_x}, found={found:?}");
        }
    }

    #[test]
    fn perch_queries_observe_radius_distance_and_locomotion_bounds() {
        use bevy::ecs::system::RunSystemOnce;
        for offset in [0.1, 0.14, 0.16, 0.2] {
            for threshold in [0.0, 0.15] {
                for (lower, mode, additional) in [(None, LocomotionState::Grounded, 0.0),
                    (Some(0.2), LocomotionState::Grounded, 0.0),
                    (Some(0.43), LocomotionState::Grounded, 0.0),
                    (Some(0.43), LocomotionState::Air, 0.0),
                    (Some(0.6), LocomotionState::Grounded, 0.0),
                    (Some(0.6), LocomotionState::Grounded, 0.3),
                    (Some(0.6), LocomotionState::Air, 0.3)] {
                    let mut app = App::new();
                    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                        avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                        .init_asset::<Mesh>().add_message::<bevy::asset::AssetEvent<Mesh>>();
                    app.finish();
                    let character = app.world_mut().spawn((RigidBody::Kinematic, Transform::default())).id();
                    let ledge = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 4.0, 4.0),
                        Transform::from_xyz(-2.0, -2.0, 0.0))).id();
                    if let Some(drop) = lower {
                        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(8.0, 0.1, 8.0),
                            Transform::from_xyz(0.0, -drop - 0.05, 0.0)));
                    }
                    for _ in 0..2 { app.update(); }
                    let found = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                        candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
                        let cmc = crate::als::params::CmcAdapterParams { perch_radius_threshold: threshold,
                            perch_additional_height: additional, ..default() };
                        find_ground_floor(&spatial, Vec3::new(offset, 0.919, 0.0), 0.9, 0.3,
                            &cmc, mode, &SpatialQueryFilter::from_excluded_entities([character]), character, &candidates)
                    }).unwrap();
                    let limit = 0.45 + if mode == LocomotionState::Grounded { 0.024 + additional } else { -0.024 };
                    let valid = threshold == 0.0 || offset <= 0.15 || lower.is_some_and(|drop| drop <= limit);
                    assert_eq!(found.is_some(), valid,
                        "offset={offset} threshold={threshold} lower={lower:?} mode={mode:?} additional={additional}: {found:?}");
                    if valid { assert_eq!(found.unwrap().2, ledge); }
                }
            }
        }
    }

    #[test]
    fn perch_floor_keeps_the_source_walkable_normal_boundary() {
        use bevy::ecs::system::RunSystemOnce;
        for angle in [44.7_f32, 44.8, 50.0] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .init_asset::<Mesh>().add_message::<bevy::asset::AssetEvent<Mesh>>();
            app.finish();
            let character = app.world_mut().spawn((RigidBody::Kinematic, Transform::default())).id();
            let rotation = Quat::from_rotation_z(angle.to_radians());
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 20.0),
                Transform::from_translation(rotation * Vec3::NEG_Y * 0.1).with_rotation(rotation)));
            for _ in 0..2 { app.update(); }
            let found = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
                find_ground_floor(&spatial, Vec3::new(0.0, 1.2, 0.0), 0.9, 0.3,
                    &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                    &SpatialQueryFilter::from_excluded_entities([character]), character, &candidates)
            }).unwrap();
            assert_eq!(found.is_some(), angle <= 44.765083, "angle={angle}: {found:?}");
        }
    }

    #[test]
    fn full_radius_floor_must_not_restore_a_rejected_perch() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
            .init_asset::<Mesh>().add_message::<bevy::asset::AssetEvent<Mesh>>();
        app.finish();
        let character = app.world_mut().spawn((RigidBody::Kinematic, Transform::default())).id();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 4.0, 4.0),
            Transform::from_xyz(-2.0, -2.0, 0.0)));
        for _ in 0..2 { app.update(); }
        let found = app.world_mut().run_system_once(move |spatial: SpatialQuery,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
            find_ground_floor(&spatial, Vec3::new(0.2, 0.919, 0.0), 0.9, 0.3, &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded,
                &SpatialQueryFilter::from_excluded_entities([character]), character, &candidates)
        }).unwrap();
        assert!(found.is_none(), "The full capsule edge cannot restore failed reduced-radius perch: {found:?}");
    }

    #[test]
    fn ground_floor_queries_reject_nonblocking_candidates_and_preserve_edge_normals() {
        use crate::als::character::AlsCollisionLayer;
        use bevy::ecs::system::RunSystemOnce;
        let blocking = CollisionLayers::new(AlsCollisionLayer::Default, AlsCollisionLayer::Default);
        for case in ["sensor", "membership", "response", "attached", "edge_sensor"] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
                .init_asset::<Mesh>().add_message::<bevy::asset::AssetEvent<Mesh>>();
            app.finish();
            let character = app.world_mut().spawn((RigidBody::Kinematic, blocking, Transform::default())).id();
            let edge = case == "edge_sensor";
            let (shape, transform) = if edge {
                (Collider::cuboid(4.0, 4.0, 4.0), Transform::from_xyz(-2.0, -2.0, 0.0))
            } else {
                (Collider::cuboid(8.0, 0.1, 8.0), Transform::from_xyz(0.0, -0.05, 0.0))
            };
            let floor = app.world_mut().spawn((RigidBody::Static, shape, transform, blocking)).id();
            let layers = match case {
                "membership" => CollisionLayers::new(AlsCollisionLayer::Visibility, avian3d::prelude::LayerMask::ALL),
                "response" => CollisionLayers::new(AlsCollisionLayer::Default, AlsCollisionLayer::Visibility),
                _ => blocking,
            };
            let (shape, transform) = if edge {
                (Collider::cuboid(0.03, EDGE_REJECT_DISTANCE * 0.1, 0.1),
                    Transform::from_xyz(-0.005, EDGE_REJECT_DISTANCE * 0.45, 0.0))
            } else {
                (Collider::cuboid(6.0, 0.002, 6.0), Transform::from_xyz(0.0, 0.019, 0.0))
            };
            let auxiliary = app.world_mut().spawn((shape, transform, layers)).id();
            if case == "attached" {
                app.world_mut().entity_mut(character).add_child(auxiliary);
            } else {
                app.world_mut().entity_mut(auxiliary).insert(RigidBody::Static);
            }
            if matches!(case, "sensor" | "edge_sensor") {
                app.world_mut().entity_mut(auxiliary).insert(Sensor);
            }
            for _ in 0..2 { app.update(); }
            let query_floor = move |spatial: SpatialQuery,
                candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
                find_ground_floor(&spatial, Vec3::new(if edge { 0.25 } else { 0.0 }, 1.0, 0.0),
                    0.9, 0.3, &crate::als::params::CmcAdapterParams { perch_radius_threshold: 0.0, ..default() }, LocomotionState::Grounded, &SpatialQueryFilter::from_excluded_entities([character]), character, &candidates)
            };
            let found = app.world_mut().run_system_once(query_floor).unwrap();
            assert_eq!(found.map(|(_, _, entity)| entity), Some(floor),
                "{case} intercepted a blocking floor or its normal ray: {found:?}");
            let mut ownership = AlsCollisionLayerOwnership::default();
            ownership.set_owner(crate::als::character::AlsCollisionLayerOwner::Mantle, Some(blocking), true);
            app.world_mut().entity_mut(character).insert((ownership, CollisionLayers::NONE));
            assert_eq!(app.world_mut().run_system_once(query_floor).unwrap().map(|(_, _, entity)| entity), Some(floor),
                "{case} bypassed the original response during physics ownership");
        }
    }

    #[test]
    fn sweep_landing_requires_a_current_blocking_contact() {
        for contact in [false, true] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            app.add_systems(Update, (move |entities: Query<&TnuaSensorsEntities<AlsControlScheme>>,
                mut sensors: Query<&mut TnuaProximitySensor>| {
                let sensor = entities.get(character).unwrap().ground.unwrap();
                sensors.get_mut(sensor).unwrap().output = None;
            }).after(TnuaPipelineSystems::Sensors).before(TnuaPipelineSystems::Logic));
            *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
            app.world_mut().get_mut::<AlsCmcAdapterRuntime>(character).unwrap().airborne_sweep_contact =
                contact;
            app.update();
            let expected = if contact { LocomotionState::Grounded } else { LocomotionState::Air };
            assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), expected,
                "landing requires a blocking contact: {contact}");
            assert!(!app.world().get::<AlsCmcAdapterRuntime>(character).unwrap().airborne_sweep_contact,
                "a consumed sweep contact survived into the next frame");
        }
    }

    #[test]
    fn steep_contact_does_not_pass_capsule_landing_validation() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let mut bodies = app.world_mut().query::<(Entity, &RigidBody)>();
        let remove: Vec<_> = bodies.iter(app.world()).filter_map(|(entity, body)| (*body == RigidBody::Static).then_some(entity)).collect();
        for entity in remove { app.world_mut().despawn(entity); }
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 20.0),
            Transform::from_rotation(Quat::from_rotation_x(60.0_f32.to_radians()))));
        let start = Vec3::Y * 4.0;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation = start;
        app.world_mut().get_mut::<Position>(character).unwrap().0 = start;
        *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
        let mut contact_seen = false;
        for _ in 0..180 {
            app.update();
            let has_contact = app.world_mut().run_system_once(move |collisions: Collisions| {
                collisions.collisions_with(character).any(|pair| pair.manifolds.iter().any(|manifold| {
                    let normal = if pair.collider1 == character { -manifold.normal } else { manifold.normal };
                    normal.y > 0.0 && manifold.points.iter().any(|point| point.normal_impulse > 0.0)
                }))
            }).unwrap();
            if has_contact {
                contact_seen = true;
                assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air,
                    "An unwalkable surface was classified as a landing");
                break;
            }
        }
        assert!(contact_seen, "The steep-surface fixture did not contact the capsule");
    }

    #[test]
    fn moving_support_keeps_an_idle_character_on_root_and_child_colliders() {
        let mut failures = Vec::new();
        for (child, velocity) in [false, true].into_iter().flat_map(|child| {
            [Vec3::X * 2.0, Vec3::Y * 50.0, Vec3::NEG_Y * 50.0].map(move |velocity| (child, velocity))
        }) {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            let transform = Transform::from_xyz(0.0, 20.0, 5.0);
            let platform = app.world_mut().spawn((RigidBody::Kinematic, transform, GlobalTransform::from(transform))).id();
            let collider = if child {
                app.world_mut().spawn((ChildOf(platform), Collider::cuboid(4.0,0.5,4.0), Transform::IDENTITY, GlobalTransform::from(transform))).id()
            } else {
                app.world_mut().entity_mut(platform).insert(Collider::cuboid(4.0,0.5,4.0));
                platform
            };
            let initial = Vec3::new(0.0, 21.174, 5.0);
            app.world_mut().get_mut::<Position>(character).unwrap().0 = initial;
            app.world_mut().get_mut::<Transform>(character).unwrap().translation = initial;
            for _ in 0..60 { app.update(); }
            let controller = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap();
            assert_eq!(controller.basis_memory.standing_on_entity(), Some(collider));
            let initial_offset = app.world().get::<Position>(character).unwrap().0 - app.world().get::<Position>(platform).unwrap().0;
            app.world_mut().get_mut::<LinearVelocity>(platform).unwrap().0 = velocity;
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)));
            let mut max_error = 0.0_f32;
            for frame in 0..if velocity.y == 0.0 { 120 } else { 4 } {
                app.update();
                let offset = app.world().get::<Position>(character).unwrap().0 - app.world().get::<Position>(platform).unwrap().0;
                max_error = max_error.max((offset - initial_offset).with_y(0.0).length());
                let clearance = offset.y - 0.25 - ALS_CHARACTER_CAPSULE_HALF_HEIGHT;
                assert!(app.world().get::<Position>(collider).unwrap().0.abs_diff_eq(
                    app.world().get::<Position>(platform).unwrap().0, 1e-5));
                assert!(app.world().get::<AlsActualVelocity>(character).unwrap().world_velocity.y.abs() < 1e-4);
                if !(MIN_FLOOR_DISTANCE - 1e-5..=MAX_FLOOR_DISTANCE + 1e-5).contains(&clearance)
                    || *app.world().get::<LocomotionState>(character).unwrap() != LocomotionState::Grounded {
                    failures.push((child, velocity, frame, clearance, max_error));
                    break;
                }
            }
            let base = app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
            println!("[MOVING-BASE] child={child} max_relative_error={max_error} dynamics={base:?}");
            if max_error > 1e-4 { failures.push((child, velocity, 120, 0.0, max_error)); }
        }
        assert!(failures.is_empty(), "Idle characters lost their moving support: {failures:?}");
    }

    #[test]
    fn moving_support_uses_component_displacement_without_reported_velocity() {
        let mut failures = Vec::new();
        for (child, step) in [false, true].into_iter().flat_map(|child| {
            [Vec3::new(0.02, 0.0, -0.01), Vec3::Y * 0.01, Vec3::NEG_Y * 0.01, Vec3::ZERO].map(move |step| (child, step))
        }) {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            let start = Vec3::new(0.0, 2.0, 5.0);
            let platform = app.world_mut().spawn((RigidBody::Kinematic, Transform::from_translation(start))).id();
            let collider = if child {
                app.world_mut().spawn((ChildOf(platform), Collider::cuboid(4.0, 0.5, 4.0), Transform::IDENTITY)).id()
            } else {
                app.world_mut().entity_mut(platform).insert(Collider::cuboid(4.0, 0.5, 4.0));
                platform
            };
            let position = start + Vec3::new(0.5, 1.174, 0.0);
            app.world_mut().entity_mut(character).insert((Position(position), Transform::from_translation(position)));
            for _ in 0..60 { app.update(); }
            assert_eq!(app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis_memory.standing_on_entity(), Some(collider));
            let offset = app.world().get::<Position>(character).unwrap().0 - start;
            let mut maximum = 0.0_f32;
            for frame in 1..=30 {
                let target = start + step * frame as f32;
                let rotation = if step == Vec3::ZERO { Quat::from_rotation_y(frame as f32 * 0.02) } else { Quat::IDENTITY };
                app.world_mut().entity_mut(platform).insert((Position(target), Rotation(rotation), Transform::from_translation(target).with_rotation(rotation)));
                app.update();
                assert_eq!(app.world().get::<LinearVelocity>(platform).unwrap().0, Vec3::ZERO);
                let actual = app.world().get::<Position>(character).unwrap().0 - target;
                let clearance = actual.y - 0.25 - ALS_CHARACTER_CAPSULE_HALF_HEIGHT;
                assert!((MIN_FLOOR_DISTANCE - 1e-5..=MAX_FLOOR_DISTANCE + 1e-5).contains(&clearance),
                    "Movement-base floor clearance differs: child={child} step={step:?} frame={frame} clearance={clearance}");
                assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Grounded);
                assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(character).unwrap().entity, Some(collider));
                assert!(app.world().get::<Transform>(character).unwrap().rotation.angle_between(Quat::IDENTITY) < 1e-4);
                maximum = maximum.max((actual - rotation * offset).with_y(0.0).length());
            }
            if maximum > 1e-4 { failures.push((child, step, maximum)); }
        }
        assert!(failures.is_empty(), "Component movement was discarded without a velocity: {failures:?}");
    }

    #[test]
    fn mantle_probe_uses_own_locomotion_velocity_on_a_moving_base() {
        use avian3d::prelude::{CollisionLayers, LayerMask};
        use bevy::ecs::system::RunSystemOnce;
        use crate::als::character::AlsCollisionLayer;
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        let platform = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(8.0, 0.5, 8.0),
            Transform::from_xyz(0.0, 2.0, 0.0), LinearVelocity::ZERO)).id();
        let origin = Vec3::new(0.0, 3.174, 0.0);
        app.world_mut().entity_mut(character).insert((Position(origin), Transform::from_translation(origin), LinearVelocity::ZERO));
        for _ in 0..60 { app.update(); }
        app.world_mut().entity_mut(platform).insert(LinearVelocity(Vec3::X * 2.0));
        app.world_mut().entity_mut(character).insert(LinearVelocity(Vec3::X * 2.0));
        for _ in 0..20 { app.update(); }
        let base = *app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
        assert_eq!(base.entity, Some(platform));
        assert!(base.relative_velocity.with_y(0.0).length() < 0.01);
        let origin = app.world().get::<Position>(character).unwrap().0;
        let target = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(1.0, 1.0, 0.8),
            Transform::from_xyz(origin.x, 2.75, origin.z - 0.9),
            CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL))).id();
        app.update();
        app.world_mut().entity_mut(character).insert(AlsCharacterDebug::default());
        let origin = app.world().get::<Position>(character).unwrap().0;
        let source_detection = app.world_mut().run_system_once(move |spatial: SpatialQuery,
            velocities: Query<(Option<&RigidBody>, Option<&LinearVelocity>, Option<&crate::als::dynamics::AlsComponentVelocity>), Without<AlsCharacter>>,
            permissions: Query<Option<&AlsMantleTargetPermission>>, colliders: Query<&ColliderOf>| {
            detect_mantle(&spatial, character, MantleDetectionInput { origin, actor_yaw: 0.0,
                velocity_yaw: None, input_yaw: Some(0.0), locomotion: LocomotionState::Grounded,
                capsule_half_height: 0.9, capsule_radius: 0.3, capsule_scale: 1.0, ..default() },
                &velocities, &permissions, &colliders)
        }).unwrap();
        assert_eq!(source_detection.intent.map(|intent| intent.target_entity), Some(target),
            "Source own-velocity input must detect the forward static ledge before the public attempt: trace={:?}", source_detection.trace);
        {
            let mut input = app.world_mut().get_mut::<AlsInputCommand>(character).unwrap();
            input.move_dir = Vec2::Y;
            input.jump_pressed = true;
        }
        app.update();
        let trace = app.world().get::<AlsCharacterDebug>(character).unwrap().0.mantle_probe.unwrap();
        let forward = trace.forward_end.unwrap() - trace.forward_start.unwrap();
        assert!(forward.normalize_or_zero().dot(Vec3::NEG_Z) > 0.999,
            "Source stationary-on-base Mantle trace follows character input rather than perpendicular base velocity: origin={origin:?} target={target:?} forward={forward:?} base={base:?}");
        assert!(app.world().get::<AlsMantleState>(character).unwrap().active,
            "The actual public Jump Mantle attempt must reach the forward static ledge");
    }

    #[test]
    fn loaded_mantle_attempt_uses_hit_component_velocity() {
        use avian3d::prelude::{CollisionLayers, LayerMask};
        use crate::als::character::AlsCollisionLayer;
        use crate::als::dynamics::AlsComponentVelocity;
        for (body_speed, component_speed, allowed) in [(0.0, 0.2, false), (0.2, 0.0, true)] {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            let origin = app.world().get::<Position>(character).unwrap().0;
            let body = app.world_mut().spawn((RigidBody::Kinematic,
                Transform::from_xyz(origin.x, 0.4, origin.z - 1.4), LinearVelocity::ZERO)).id();
            let ledge = app.world_mut().spawn((Collider::cuboid(2.4, 0.8, 1.2), ChildOf(body), Transform::IDENTITY,
                CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL))).id();
            app.update();
            app.update();
            app.world_mut().entity_mut(body).insert(AlsComponentVelocity(Vec3::X * body_speed));
            app.world_mut().entity_mut(ledge).insert(AlsComponentVelocity(Vec3::X * component_speed));
            app.world_mut().entity_mut(character).insert(AlsCharacterDebug::default());
            {
                let mut input = app.world_mut().get_mut::<AlsInputCommand>(character).unwrap();
                input.move_dir = Vec2::Y;
                input.jump_pressed = true;
            }
            app.update();
            let trace = app.world().get::<AlsCharacterDebug>(character).unwrap().0.mantle_probe.unwrap();
            assert_eq!(app.world().get::<AlsMantleState>(character).unwrap().active, allowed,
                "body_speed={body_speed} component_speed={component_speed} trace={trace:?}");
        }
    }

    #[test]
    fn loaded_mantle_corner_keeps_the_wall_face_direction() {
        use avian3d::prelude::{CollisionLayers, LayerMask};
        use crate::als::character::AlsCollisionLayer;
        for lateral in [-1.3, 0.0, 1.3] {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.4, 0.8, 1.2),
                Transform::from_xyz(0.0, 0.4, -1.4),
                CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL)));
            let position = app.world().get::<Position>(character).unwrap().0.with_x(lateral);
            app.world_mut().entity_mut(character).insert((Position(position), Transform::from_translation(position),
                LinearVelocity::ZERO, AlsCharacterDebug::default()));
            app.update();
            app.update();
            {
                let mut input = app.world_mut().get_mut::<AlsInputCommand>(character).unwrap();
                input.move_dir = Vec2::Y;
                input.jump_pressed = true;
            }
            app.update();
            let state = app.world().get::<AlsMantleState>(character).unwrap();
            let trace = app.world().get::<AlsCharacterDebug>(character).unwrap().0.mantle_probe.unwrap();
            assert!(state.active, "lateral={lateral} trace={trace:?}");
            assert!((state.target.x - trace.forward_hit.unwrap().x).abs() < 0.0001,
                "lateral={lateral} target={:?} trace={trace:?}", state.target);
            assert!(state.yaw_target.abs() < 0.0001, "lateral={lateral} yaw={}", state.yaw_target);
        }
    }

    #[test]
    fn loaded_mantle_captures_the_actor_world_transform() {
        use avian3d::prelude::{CollisionLayers, LayerMask};
        use bevy::ecs::system::RunSystemOnce;
        use crate::als::character::AlsCollisionLayer;
        for (scale, pitch, roll) in [(0.75, 0.0, 0.0), (1.0, 0.0, 0.0), (1.5, 0.0, 0.0),
            (1.0, 0.2, 0.0), (1.0, 0.0, 0.2), (1.0, 0.2, -0.2)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.4 * scale, 0.8 * scale, 1.2 * scale),
                Transform::from_xyz(0.0, 0.4 * scale, -1.4 * scale),
                CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL)));
            app.update();
            app.update();
            let transform = Transform::from_xyz(0.0, 0.95 * scale, 0.0)
                .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.0, pitch, roll)).with_scale(Vec3::splat(scale));
            app.world_mut().entity_mut(actor).insert((transform, GlobalTransform::from(transform), Position(transform.translation),
                Rotation(transform.rotation), LocomotionState::Grounded, AlsMantleState::default(), AlsJumpPrev(false),
                AlsInputCommand { jump_pressed: true, ..default() }, crate::als::state::AlsNotifyLocomotionAction::None));
            app.world_mut().run_system_once(locomotion_drive_system).unwrap();
            let mantle = app.world().get::<AlsMantleState>(actor).unwrap();
            assert!(mantle.active, "scale={scale} pitch={pitch} roll={roll}");
            assert!(mantle.warp_start.translation.abs_diff_eq(transform.translation, 1e-5));
            assert!(mantle.warp_start.scale.abs_diff_eq(transform.scale, 1e-5));
            assert!(mantle.warp_start.rotation.angle_between(transform.rotation) < 1e-4);
        }
    }

    #[test]
    fn transform_base_follow_only_sweeps_character_blocking_shapes() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::als::character::AlsCollisionLayer;
        let blocking = CollisionLayers::new(AlsCollisionLayer::Default, AlsCollisionLayer::Default);
        for case in ["sensor", "membership", "response", "solid", "physics_only", "zero_members", "sensor_wall", "base_child", "character_child"] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            let base_origin = Vec3::new(0.0, 2.0, 5.0);
            let platform = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(4.0, 0.5, 4.0),
                Transform::from_translation(base_origin))).id();
            let position = base_origin + Vec3::new(0.5, 1.174, 0.0);
            app.world_mut().entity_mut(character).insert((Position(position), Transform::from_translation(position), blocking));
            for _ in 0..60 { app.update(); }
            assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(character).unwrap().entity, Some(platform));
            assert_eq!(app.world().get::<LinearVelocity>(platform).unwrap().0, Vec3::ZERO);
            let origin = app.world().get::<Position>(character).unwrap().0;
            let layers = match case {
                "membership" => CollisionLayers::new(AlsCollisionLayer::Visibility, avian3d::prelude::LayerMask::ALL),
                "response" => CollisionLayers::new(AlsCollisionLayer::Default, AlsCollisionLayer::Visibility),
                "zero_members" => CollisionLayers::new(avian3d::prelude::LayerMask::NONE, avian3d::prelude::LayerMask::ALL),
                _ => blocking,
            };
            let obstacle = app.world_mut().spawn((Collider::cuboid(0.05, 1.0, 4.0), layers,
                Transform::from_translation(origin + Vec3::X * 0.35))).id();
            match case {
                "sensor" | "sensor_wall" => { app.world_mut().entity_mut(obstacle).insert((RigidBody::Static, Sensor)); }
                "physics_only" => { app.world_mut().entity_mut(obstacle).insert((RigidBody::Static,
                    crate::extensions::RagdollShapeCollisionMode::PhysicsOnly)); }
                "base_child" | "character_child" => {
                    let parent = if case == "base_child" { platform } else { character };
                    let parent_origin = app.world().get::<Transform>(parent).unwrap().translation;
                    app.world_mut().entity_mut(obstacle).insert((ChildOf(parent),
                        Transform::from_translation(origin + Vec3::X * 0.35 - parent_origin)));
                }
                _ => { app.world_mut().entity_mut(obstacle).insert(RigidBody::Static); }
            }
            if case == "sensor_wall" {
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.05, 1.0, 4.0), blocking,
                    Transform::from_translation(origin + Vec3::X * 0.4)));
            }
            app.update();
            let origin = app.world().get::<Position>(character).unwrap().0;
            let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
            assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character).unwrap().support_velocity, Vec3::ZERO);
            app.world_mut().get_mut::<AlsCmcAdapterRuntime>(character).unwrap().grounded_move_start = Some(origin);
            let target = base_origin + Vec3::X * 0.1;
            app.world_mut().entity_mut(platform).insert((Position(target), Transform::from_translation(target)));
            app.world_mut().run_system_once((|| MovementBaseFollowPhase::BeforeSensors).pipe(follow_movement_base_system)).unwrap();
            let after = app.world().get::<Position>(character).unwrap().0;
            let applied = (after - origin).x;
            if case == "solid" { assert!(applied > 0.0 && applied < 0.025, "{case}: {applied}"); }
            else if case == "sensor_wall" { assert!(applied > 0.05 && applied < 0.075, "{case}: {applied}"); }
            else { assert!((applied - 0.1).abs() < 0.0001, "{case} clipped nonblocking carry: {applied}"); }
            assert_eq!(app.world().get::<LinearVelocity>(character).unwrap().0, velocity);
            assert!(app.world().get::<AlsCmcAdapterRuntime>(character).unwrap().grounded_move_start.unwrap().distance(after) < 0.0001);
            assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(character).unwrap().location, target);
            app.world_mut().run_system_once((|| MovementBaseFollowPhase::BeforeSensors).pipe(follow_movement_base_system)).unwrap();
            assert_eq!(app.world().get::<Position>(character).unwrap().0, after);
        }
    }

    #[test]
    fn movement_base_rotation_keeps_the_capsule_bottom_anchor() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let center = Vec3::new(0.0, 2.0, 5.0);
        let platform = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(4.0, 0.5, 4.0), Transform::from_translation(center))).id();
        let position = center + Vec3::new(0.5, 1.174, 0.0);
        app.world_mut().entity_mut(character).insert((Position(position), Transform::from_translation(position)));
        for _ in 0..60 { app.update(); }
        let before = app.world().get::<Position>(character).unwrap().0 - center - Vec3::Y * ALS_CHARACTER_CAPSULE_HALF_HEIGHT;
        let target = center + Vec3::new(0.1, 0.05, -0.05);
        let rotation = Quat::from_rotation_x(0.3);
        app.world_mut().entity_mut(platform).insert((Position(target), Rotation(rotation),
            Transform::from_translation(target).with_rotation(rotation).with_scale(Vec3::splat(2.0))));
        app.world_mut().run_system_once((|| MovementBaseFollowPhase::BeforeSensors).pipe(follow_movement_base_system)).unwrap();
        let after = app.world().get::<Position>(character).unwrap().0 - target - Vec3::Y * ALS_CHARACTER_CAPSULE_HALF_HEIGHT;
        assert!((rotation.conjugate() * after).abs_diff_eq(before, 1e-5));
        assert_eq!(app.world().get::<Transform>(character).unwrap().rotation, Quat::IDENTITY);
    }

    #[test]
    fn deferred_base_follow_runs_only_when_jump_is_not_accepted() {
        use bevy::ecs::system::RunSystemOnce;
        for accepted in [false, true] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            let center = Vec3::new(0.0, 2.0, 5.0);
            let platform = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(4.0, 0.5, 4.0),
                Transform::from_translation(center))).id();
            let position = center + Vec3::Y * 1.174;
            app.world_mut().entity_mut(character).insert((Position(position), Transform::from_translation(position)));
            for _ in 0..60 { app.update(); }
            assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(character).unwrap().entity, Some(platform));
            let start = app.world().get::<Position>(character).unwrap().0;
            let target = center + Vec3::X * 0.2;
            app.world_mut().entity_mut(platform).insert((Position(target), Transform::from_translation(target)));
            app.world_mut().get_mut::<AlsCmcAdapterRuntime>(character).unwrap().deferred_base_follow = true;
            app.world_mut().run_system_once((|| MovementBaseFollowPhase::BeforeSensors).pipe(follow_movement_base_system)).unwrap();
            assert_eq!(app.world().get::<Position>(character).unwrap().0, start);
            if accepted { *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air; }
            app.world_mut().run_system_once((|| MovementBaseFollowPhase::AfterJump).pipe(follow_movement_base_system)).unwrap();
            let actual = app.world().get::<Position>(character).unwrap().0;
            let expected = start + Vec3::X * if accepted { 0.0 } else { 0.2 };
            assert!(actual.abs_diff_eq(expected, 1e-4), "accepted={accepted} actual={actual:?} expected={expected:?}");
            assert!(!app.world().get::<AlsCmcAdapterRuntime>(character).unwrap().deferred_base_follow);
        }
    }

    #[test]
    fn accepted_jump_does_not_follow_the_old_base_transform() {
        for (owner_dilation, jump, shift) in [(1.0, true, 0.2), (0.0, true, 0.2),
            (1.0, true, 0.0), (1.0, false, 0.2)] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            let center = Vec3::new(0.0, 2.0, 5.0);
            let platform = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(4.0, 0.5, 4.0),
                Transform::from_translation(center))).id();
            let position = center + Vec3::Y * 1.174;
            app.world_mut().entity_mut(character).insert((Position(position), Transform::from_translation(position)));
            for _ in 0..60 { app.update(); }
            assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(character).unwrap().entity, Some(platform));
            let start = app.world().get::<Position>(character).unwrap().0;
            let target = center + Vec3::X * shift;
            app.world_mut().entity_mut(platform).insert((Position(target), Transform::from_translation(target), LinearVelocity::ZERO));
            app.world_mut().entity_mut(character).insert(crate::als_camera::AlsCameraCustomTimeDilation(owner_dilation));
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = jump;
            app.update();
            let actual = app.world().get::<Position>(character).unwrap().0;
            let expected_x = start.x + if jump { 0.0 } else { shift };
            assert!((actual.x - expected_x).abs() < 1e-4,
                "owner={owner_dilation} jump={jump} shift={shift} actual={actual:?} expected_x={expected_x}");
            if jump { assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air); }
        }
    }

    #[test]
    fn component_transport_precedes_the_character_movement_sweep() {
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let center = Vec3::new(0.0, 2.0, 5.0);
        let platform = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(4.0, 0.5, 4.0), Transform::from_translation(center))).id();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.2, 4.0, 4.0), Transform::from_xyz(1.0, 3.0, 5.0)));
        let start = Vec3::new(0.5, 3.174, 5.0);
        app.world_mut().entity_mut(character).insert((Position(start), Transform::from_translation(start)));
        for _ in 0..60 { app.update(); }
        let start = app.world().get::<Position>(character).unwrap().0.with_x(0.6);
        app.world_mut().entity_mut(character).insert((Position(start), Transform::from_translation(start), LinearVelocity(Vec3::X * 3.75)));
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::X;
        let target = center - Vec3::X * 0.5;
        app.world_mut().entity_mut(platform).insert((Position(target), Transform::from_translation(target)));
        app.update();
        let actual = app.world().get::<Position>(character).unwrap().0;
        let expected_x = start.x - 0.5 + 3.75 * app.world().resource::<Time>().delta_secs();
        assert!((actual.x - expected_x).abs() < 1e-4,
            "Character movement ran before its base displacement: actual={actual:?} expected_x={expected_x}");
        assert!((app.world().get::<AlsActualVelocity>(character).unwrap().horizontal_speed - 3.75).abs() < 1e-4);
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::ZERO;
        app.world_mut().get_mut::<LinearVelocity>(character).unwrap().0 = Vec3::ZERO;
        let target = center + Vec3::X * 0.5;
        app.world_mut().entity_mut(platform).insert((Position(target), Transform::from_translation(target)));
        app.update();
        let blocked = app.world().get::<Position>(character).unwrap().0;
        assert!(blocked.x > actual.x + 0.3 && blocked.x <= 0.6001,
            "Movement-base transport did not sweep the blocking wall: {blocked:?}");
    }

    #[test]
    fn deferred_base_motion_does_not_enter_the_character_sweep_velocity() {
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let platform = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(4.0, 0.5, 4.0),
            avian3d::prelude::LockedAxes::ALL_LOCKED, GravityScale(0.0), Transform::from_xyz(0.0, 2.0, 5.0))).id();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.2, 4.0, 4.0), Transform::from_xyz(1.0, 3.0, 5.0)));
        let position = Vec3::new(0.5, 3.174, 5.0);
        app.world_mut().entity_mut(character).insert((Position(position), Transform::from_translation(position)));
        for _ in 0..60 { app.update(); }
        let start = app.world().get::<Position>(character).unwrap().0.with_x(0.6);
        app.world_mut().entity_mut(character).insert((Position(start), Transform::from_translation(start), LinearVelocity(Vec3::X * 3.75)));
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::X;
        app.add_systems(PostUpdate, (move |mut bodies: Query<(&mut Position, &mut Transform), Without<AlsCharacter>>| {
            let (mut position, mut transform) = bodies.get_mut(platform).unwrap();
            position.x -= 0.5;
            transform.translation = position.0;
        }).after(PhysicsSystems::Last).before(AlsActualVelocitySet));
        app.update();
        assert!(app.world().get::<AlsCmcAdapterRuntime>(character).unwrap().grounded_move_start.is_some());
        let velocity = app.world().get::<AlsActualVelocity>(character).unwrap().world_velocity;
        assert!(velocity.x.abs() < 1e-4, "Deferred movement-base displacement entered the character velocity: {velocity:?}");
    }

    #[test]
    fn replacing_moving_support_keeps_the_character_velocity_and_updates_the_base() {
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let platform = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(4.0, 0.5, 4.0),
            Transform::from_xyz(0.0, 2.0, 5.0))).id();
        let position = Vec3::new(0.0, 3.174, 5.0);
        app.world_mut().entity_mut(character).insert((Position(position), Transform::from_translation(position)));
        for _ in 0..60 { app.update(); }
        app.world_mut().get_mut::<LinearVelocity>(platform).unwrap().0 = Vec3::X * 2.0;
        for _ in 0..30 { app.update(); }
        let center = app.world().get::<Position>(platform).unwrap().0;
        app.world_mut().despawn(platform);
        let replacement = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(4.0, 0.5, 4.0),
            Transform::from_translation(center))).id();
        app.update();
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Grounded);
        assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBase>(character).unwrap().entity, Some(replacement));
        let velocity = app.world().get::<AlsActualVelocity>(character).unwrap().world_velocity;
        assert!(velocity.with_y(0.0).length() < 1e-4, "Replacing the base changed the character velocity: {velocity:?}");
        app.world_mut().despawn(replacement);
        app.update();
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air);
        assert!(app.world().get::<crate::als::dynamics::AlsMovementBase>(character).unwrap().entity.is_none());
        for _ in 0..5 { app.update(); }
        assert!(app.world().get::<AlsActualVelocity>(character).unwrap().horizontal_speed < 1e-4);
    }

    #[test]
    fn accelerating_support_preserves_idle_position_and_character_velocity() {
        let mut failures = Vec::new();
        for child in [false, true] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            *app.world_mut().get_mut::<RotationMode>(character).unwrap() = RotationMode::VelocityDirection;
            let transform = Transform::from_xyz(0.0, 2.0, 5.0);
            let platform = app.world_mut().spawn((RigidBody::Kinematic, transform, GlobalTransform::from(transform))).id();
            if child {
                app.world_mut().spawn((ChildOf(platform), Collider::cuboid(4.0, 0.5, 4.0), Transform::IDENTITY, GlobalTransform::from(transform)));
            } else {
                app.world_mut().entity_mut(platform).insert(Collider::cuboid(4.0, 0.5, 4.0));
            }
            let initial = Vec3::new(0.0, 3.174, 5.0);
            app.world_mut().get_mut::<Position>(character).unwrap().0 = initial;
            app.world_mut().get_mut::<Transform>(character).unwrap().translation = initial;
            for _ in 0..60 { app.update(); }
            let initial_offset = app.world().get::<Position>(character).unwrap().0 - app.world().get::<Position>(platform).unwrap().0;
            let mut max_error = 0.0_f32;
            let mut max_speed = 0.0_f32;
            let initial_rotation = app.world().get::<Transform>(character).unwrap().rotation;
            let mut max_yaw_error = 0.0_f32;
            for frame in 0..180 {
                let velocity = match frame / 30 {
                    0 => Vec3::X * 2.0,
                    1 => Vec3::new(1.0, 0.0, -1.0),
                    2 | 5 => Vec3::ZERO,
                    3 => Vec3::NEG_X * 2.0,
                    _ => Vec3::X * ((frame - 120) as f32 / 30.0),
                };
                app.world_mut().get_mut::<LinearVelocity>(platform).unwrap().0 = velocity;
                app.update();
                let offset = app.world().get::<Position>(character).unwrap().0 - app.world().get::<Position>(platform).unwrap().0;
                max_error = max_error.max((offset - initial_offset).with_y(0.0).length());
                max_speed = max_speed.max(app.world().get::<AlsActualVelocity>(character).unwrap().horizontal_speed);
                max_yaw_error = max_yaw_error.max(initial_rotation.angle_between(app.world().get::<Transform>(character).unwrap().rotation));
            }
            assert!(max_yaw_error < 1e-4, "Passive transport turned the character: child={child} error={max_yaw_error}");
            if max_error > 1e-4 || max_speed > 1e-4 { failures.push((child, max_error, max_speed)); }
            app.world_mut().get_mut::<LinearVelocity>(platform).unwrap().0 = Vec3::X * 2.0;
            app.update();
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
            app.update();
            assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air);
            assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character).unwrap().entity, None);
            assert!((app.world().get::<AlsActualVelocity>(character).unwrap().world_velocity.x - 2.0).abs() < 1e-4);
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = false;
            app.world_mut().get_mut::<LinearVelocity>(platform).unwrap().0 = Vec3::NEG_X * 2.0;
            for _ in 0..5 { app.update(); }
            assert!((app.world().get::<AlsActualVelocity>(character).unwrap().world_velocity.x - 2.0).abs() < 1e-4,
                "Detached character lost its imparted velocity: {:?}", app.world().get::<AlsActualVelocity>(character));
        }
        assert!(failures.is_empty(), "Support transport changed the idle character motion: {failures:?}");
    }

    #[test]
    fn landing_on_moving_support_preserves_the_character_velocity() {
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let platform = app.world_mut().spawn((RigidBody::Kinematic, Collider::cuboid(8.0, 0.5, 4.0),
            Transform::from_xyz(0.0, 2.0, 5.0), LinearVelocity(Vec3::X * 2.0),
            avian3d::prelude::Friction::new(0.8).with_combine_rule(avian3d::prelude::CoefficientCombine::Max))).id();
        let prop = app.world_mut().spawn((RigidBody::Dynamic, Collider::cuboid(0.5, 0.5, 0.5),
            Transform::from_xyz(2.0, 2.5, 6.2))).id();
        let initial = Vec3::new(0.0, 4.5, 5.0);
        app.world_mut().get_mut::<Position>(character).unwrap().0 = initial;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation = initial;
        *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
        app.world_mut().get_mut::<LinearVelocity>(character).unwrap().0 = Vec3::ZERO;
        let mut based_frames = 0;
        for frame in 0..100 {
            app.update();
            let base = app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
            if base.entity == Some(platform) {
                based_frames += 1;
                let velocity = app.world().get::<AlsActualVelocity>(character).unwrap().world_velocity;
                assert!(velocity.with_y(0.0).length() < 1e-4,
                    "Landing changed the character velocity: frame={frame} velocity={velocity:?}");
            }
            if based_frames == 20 { break; }
        }
        assert_eq!(based_frames, 20, "The character did not remain on its acquired movement base");
        assert!(app.world().get::<LinearVelocity>(prop).unwrap().x > 0.5,
            "The character adapter removed friction from an ordinary physics object");
    }

    #[test]
    fn an_ascending_capsule_does_not_land_from_the_controller_sensor_alone() {
        let (mut app, character, _) = stair_motion_fixture(0.26, false, false, 0.0);
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.26, 4.0), Transform::from_xyz(0.0, 0.13, 5.0)));
        app.update();
        let position = Vec3::new(0.0, 1.1873851, 5.0);
        app.world_mut().entity_mut(character).insert((Position(position),
            Transform::from_translation(position), LocomotionState::Air,
            LinearVelocity(Vec3::new(0.0, 4.192001, -3.75))));
        let controller = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap();
        assert!(TnuaBuiltinWalk::displacement(&controller.basis_access().unwrap()).is_some());
        app.update();
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air,
            "The capsule is still ascending above its physical floor");
    }

    #[test]
    fn ground_sensor_surface_fallback_prevents_the_recorded_slipping_impulse() {
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        app.world_mut().get_mut::<crate::als::gait::AlsDesiredGait>(character).unwrap().0 = Gait::Walking;
        app.update();
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
        app.world_mut().get_mut::<LinearVelocity>(character).unwrap().0 = Vec3::NEG_Z * 1.75;
        app.add_systems(Update, (move |entities: Query<&TnuaSensorsEntities<AlsControlScheme>>, mut sensors: Query<&mut TnuaProximitySensor>| {
            let sensor = entities.get(character).unwrap().ground.unwrap();
            sensors.get_mut(sensor).unwrap().output.as_mut().unwrap().normal =
                Dir3::new(Vec3::new(-0.046057902, 0.5598525, 0.8273112)).unwrap();
        }).after(TnuaPipelineSystems::Sensors).before(refine_ground_sensor_system));
        app.update();
        let sensor = app.world().get::<TnuaSensorsEntities<AlsControlScheme>>(character).unwrap().ground.unwrap();
        assert_eq!(app.world().get::<TnuaProximitySensor>(sensor).unwrap().output.as_ref().unwrap().normal, Dir3::Y);
        let actual = app.world().get::<LinearVelocity>(character).unwrap().0;
        assert!(actual.abs_diff_eq(Vec3::NEG_Z * 1.75, 1e-4),
            "Refined ground speed differs: actual={actual:?}, max_gait={:?}, desired={:?}, motor={:?}",
            app.world().get::<crate::als::gait::AlsMaxAllowedGait>(character),
            app.world().get::<crate::als::gait::AlsDesiredGait>(character),
            app.world().get::<TnuaMotor>(character));
    }

    #[test]
    fn mantle_probe_uses_consumed_input_and_velocity_yaw() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let mut failures = Vec::new();
        for (has_input, input_yaw, speed, velocity_yaw, expected) in [
            (false, -90.0_f32, 0.0, 70.0, 0.0), (true, -90.0, 0.0, 70.0, -50.0),
            (false, -90.0, 1.0, 70.0, 50.0), (true, 100.0, 1.0, -10.0, 40.0),
        ] {
            app.world_mut().entity_mut(actor).insert((AlsMantleState::default(), AlsJumpPrev(false),
                AlsStateRequest::default(), AlsInputOverride(AlsInputCommand { jump_pressed: true, move_dir: Vec2::X * 0.00001, ..default() }),
                crate::als::view::AlsPlayerViewYaw(0.4), LinearVelocity::ZERO,
                crate::actual_motion::AlsActualMotion { has_input, input_yaw_degrees: input_yaw,
                    horizontal_speed: speed, velocity_yaw_degrees: velocity_yaw, ..default() }));
            app.world_mut().run_system_once(locomotion_drive_system).unwrap();
            let trace = app.world().get::<AlsCharacterDebug>(actor).unwrap().0.mantle_probe.unwrap();
            let direction = (trace.forward_end.unwrap() - trace.forward_start.unwrap()).normalize();
            let yaw = crate::als::orientation::world_forward_to_yaw(direction).to_degrees();
            if (yaw - expected).abs() > 1e-4 { failures.push((has_input, input_yaw, speed, velocity_yaw, yaw, expected)); }
        }
        assert!(failures.is_empty(), "Source Mantle uses consumed input and velocity yaw: {failures:?}");
    }

    #[test]
    fn movement_uses_player_view_without_overwriting_the_control_target() {
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let control = 60.0_f32.to_radians();
        let rendered = 30.0_f32.to_radians();
        app.world_mut().entity_mut(character).insert((AlsViewYaw(control), crate::als::view::AlsPlayerViewYaw(rendered)));
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
        app.update();
        let desired = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion;
        assert!(desired.abs_diff_eq(Vec3::new(-0.5, 0.0, -3.0_f32.sqrt() * 0.5), 1e-5), "Movement used the control target instead of the player view: {desired:?}");
        assert_eq!(app.world().get::<AlsViewYaw>(character).unwrap().0, control);
        app.world_mut().entity_mut(character).remove::<crate::als::view::AlsPlayerViewYaw>();
        app.update();
        let desired = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion;
        assert!(desired.abs_diff_eq(Vec3::new(-3.0_f32.sqrt() * 0.5, 0.0, -0.5), 1e-5));
        app.world_mut().entity_mut(character).insert((crate::als::input_request::AlsFirstPersonView, crate::als::view::AlsPlayerViewYaw(0.0)));
        app.world_mut().get_mut::<crate::als::gait::AlsDesiredGait>(character).unwrap().0 = Gait::Sprinting;
        app.update();
        assert_eq!(app.world().get::<crate::als::gait::AlsMaxAllowedGait>(character).unwrap().0, Gait::Running);
        app.world_mut().get_mut::<crate::als::view::AlsPlayerViewYaw>(character).unwrap().0 = control;
        app.update();
        assert_eq!(app.world().get::<crate::als::gait::AlsMaxAllowedGait>(character).unwrap().0, Gait::Sprinting);
    }

    #[test]
    fn cold_player_view_uses_actor_yaw_until_a_view_is_published() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::als::view::{AlsPlayerViewReadiness, AlsPlayerViewYaw};
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let control = -150.0_f32.to_radians();
        let mut transform = *app.world().get::<Transform>(character).unwrap();
        transform.rotation = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
        app.world_mut().entity_mut(character).insert((transform, GlobalTransform::from(transform), AlsViewYaw(control),
            AlsPlayerViewReadiness { source: Entity::PLACEHOLDER, has_published: false }));
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
        app.world_mut().run_system_once(locomotion_drive_system).unwrap();
        let desired = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion;
        assert!(desired.abs_diff_eq(Vec3::X, 1e-6));

        let published = -120.0_f32.to_radians();
        app.world_mut().entity_mut(character).insert(AlsPlayerViewYaw(published));
        app.world_mut().run_system_once(locomotion_drive_system).unwrap();
        let desired = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion;
        assert!(desired.abs_diff_eq(view_relative_move_direction(Vec2::Y, published), 1e-6));
        app.world_mut().entity_mut(character).remove::<AlsPlayerViewYaw>();
        app.world_mut().get_mut::<AlsPlayerViewReadiness>(character).unwrap().has_published = true;
        app.world_mut().run_system_once(locomotion_drive_system).unwrap();
        let desired = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion;
        assert!(desired.abs_diff_eq(view_relative_move_direction(Vec2::Y, control), 1e-6));
        app.world_mut().entity_mut(character).remove::<AlsPlayerViewReadiness>();
        app.world_mut().run_system_once(locomotion_drive_system).unwrap();
        let desired = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion;
        assert!(desired.abs_diff_eq(view_relative_move_direction(Vec2::Y, control), 1e-6));
    }

    #[test]
    fn player_analog_motion_is_scaled_once() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        app.world_mut().entity_mut(character).insert(crate::als::view::AlsPlayerViewYaw(-std::f32::consts::PI * 0.75));
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y * 0.5;
        app.world_mut().run_system_once(locomotion_drive_system).unwrap();
        let desired = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion;
        assert!(desired.abs_diff_eq(Vec3::new(0.35355338, 0.0, 0.35355338), 1e-6));
    }

    #[test]
    fn player_diagonal_motion_preserves_source_clamp_precision() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        app.world_mut().entity_mut(character).insert(crate::als::view::AlsPlayerViewYaw(-30.0_f32.to_radians()));
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::ONE;
        app.world_mut().run_system_once(locomotion_drive_system).unwrap();
        let desired = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion;
        assert_eq!(desired, Vec3::new(0.9659258127212524, 0.0, -0.2588191032409668));
    }

    #[test]
    fn mantle_targets_resolve_motion_and_permissions_from_compound_bodies() {
        use crate::als::mantle::AlsMantleTargetPermission;
        let mut failures = Vec::new();
        for child in [false, true] {
            for (kind, speed, allowed, override_permission, reported) in [(RigidBody::Static, 0.0, true, None, None),
                (RigidBody::Kinematic, 0.04, true, None, None), (RigidBody::Dynamic, 0.04, true, None, None),
                (RigidBody::Kinematic, 0.1, true, None, None), (RigidBody::Kinematic, 0.2, true, None, None),
                (RigidBody::Kinematic, 4.0, true, None, None), (RigidBody::Dynamic, 0.2, true, None, None),
                (RigidBody::Dynamic, 4.0, true, None, None), (RigidBody::Kinematic, 4.0, true, None, Some(0.04)),
                (RigidBody::Kinematic, 4.0, true, None, Some(0.2)), (RigidBody::Dynamic, 0.04, true, None, Some(4.0)),
                (RigidBody::Dynamic, 4.0, true, None, Some(0.0)),
                (RigidBody::Kinematic, 0.04, false, None, None), (RigidBody::Kinematic, 0.04, false, Some(true), None),
                (RigidBody::Kinematic, 0.04, true, Some(false), None)] {
                let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
                let offset = if child { Vec3::X * 0.25 } else { Vec3::ZERO };
                let body = app.world_mut().spawn((kind, Transform::from_translation(Vec3::new(0.0, 0.4, 4.6) - offset),
                    avian3d::prelude::GravityScale(0.0), AlsMantleTargetPermission { allows_step_up: allowed })).id();
                if let Some(speed) = reported {
                    app.world_mut().entity_mut(body).insert(crate::als::dynamics::AlsComponentVelocity(Vec3::X * speed));
                }
                let target = if child {
                    app.world_mut().spawn((ChildOf(body), Transform::from_translation(offset), Collider::cuboid(2.4, 0.8, 1.2))).id()
                } else {
                    app.world_mut().entity_mut(body).insert(Collider::cuboid(2.4, 0.8, 1.2));
                    body
                };
                app.world_mut().entity_mut(target).insert(avian3d::prelude::CollisionLayers::new(
                    [crate::als::character::AlsCollisionLayer::Default, crate::als::character::AlsCollisionLayer::Visibility], avian3d::prelude::LayerMask::ALL));
                app.world_mut().entity_mut(character).insert(AlsMantleControlSettings { draw_failed_grounded_traces: child, ..default() });
                if let Some(allows_step_up) = override_permission {
                    app.world_mut().entity_mut(target).insert(AlsMantleTargetPermission { allows_step_up });
                }
                let start = Vec3::new(0.0, 0.924, 6.0);
                app.world_mut().entity_mut(character).insert((Position(start), Transform::from_translation(start), AlsMantleState::default()));
                for _ in 0..3 { app.update(); }
                app.world_mut().get_mut::<LinearVelocity>(body).unwrap().0 = Vec3::X * speed;
                let input = &mut *app.world_mut().get_mut::<AlsInputCommand>(character).unwrap();
                input.move_dir = Vec2::Y;
                input.jump_pressed = true;
                app.update();
                assert_eq!(app.world().get::<AlsCharacterDebug>(character).unwrap().0.mantle_probe.unwrap().draw_failed_traces, child);
                let mantle = app.world().get::<AlsMantleState>(character).unwrap();
                let expected = override_permission.unwrap_or(allowed)
                    && (if kind == RigidBody::Dynamic { speed } else { reported.unwrap_or(0.0) }) <= crate::als::mantle::MANTLE_TARGET_SPEED_THRESHOLD;
                if mantle.active != expected {
                    failures.push(format!("child={child} kind={kind:?} speed={speed} reported={reported:?} allowed={allowed} active={}", mantle.active));
                    continue;
                }
                if !expected { continue; }
                let base = mantle.target_base.unwrap();
                assert_eq!(base.entity, target);
                if base.relative_location != (kind != RigidBody::Static) {
                    failures.push(format!("child={child} kind={kind:?} relative_location={}", base.relative_location));
                    continue;
                }
                let mut moving = mantle.clone();
                let rotation = Quat::from_rotation_y(0.3);
                let latest = AlsMantleTargetBase { translation: base.translation + Vec3::new(0.4, 0.7, -0.2), rotation, ..base };
                let expected_target = if base.relative_location {
                    latest.translation + rotation * (moving.target - base.translation)
                } else { moving.target };
                assert!(moving.refresh_target_base(Some(latest)));
                assert!(moving.target.abs_diff_eq(expected_target, 1e-5));
            }
        }
        assert!(failures.is_empty(), "Mantle component ownership differs from the source: {failures:?}");
    }

    #[test]
    fn ground_sensor_rejects_air_and_preserves_foreign_hits_and_steep_surfaces() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let sensor = app.world().get::<TnuaSensorsEntities<AlsControlScheme>>(character).unwrap().ground.unwrap();
        let mut original = app.world().get::<TnuaProximitySensor>(sensor).unwrap().output.clone().unwrap();
        original.normal = Dir3::new(Vec3::new(-0.046057902, 0.5598525, 0.8273112)).unwrap();
        let foreign = app.world_mut().spawn_empty().id();
        for (mode, entity, normal) in [(LocomotionState::Air, original.entity, original.normal),
            (LocomotionState::Grounded, foreign, original.normal),
            (LocomotionState::Grounded, original.entity, Dir3::new(Vec3::new(0.6, 0.8, 0.0)).unwrap())] {
            *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = mode;
            let mut output = original.clone();
            output.entity = entity;
            output.normal = normal;
            app.world_mut().get_mut::<TnuaProximitySensor>(sensor).unwrap().output = Some(output);
            app.world_mut().run_system_once(refine_ground_sensor_system).unwrap();
            if mode == LocomotionState::Air {
                assert!(app.world().get::<TnuaProximitySensor>(sensor).unwrap().output.is_none());
                continue;
            }
            let output = app.world().get::<TnuaProximitySensor>(sensor).unwrap().output.as_ref().unwrap();
            assert_eq!(output.normal, normal);
            assert_eq!(output.entity, entity);
        }
        app.world_mut().get_mut::<Rotation>(original.entity).unwrap().0 = Quat::from_rotation_z(60.0_f32.to_radians());
        app.world_mut().get_mut::<Transform>(original.entity).unwrap().rotation = Quat::from_rotation_z(60.0_f32.to_radians());
        app.update();
        *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Grounded;
        app.world_mut().get_mut::<TnuaProximitySensor>(sensor).unwrap().output = Some(original.clone());
        let position = app.world().get::<Position>(character).unwrap().0;
        let hit = app.world_mut().run_system_once(move |spatial: SpatialQuery| spatial.cast_ray(position, Dir3::NEG_Y, 1.374, false,
            &SpatialQueryFilter::from_excluded_entities([character]))).unwrap().unwrap();
        assert_eq!(hit.entity, original.entity);
        assert!((hit.normal.y - 0.5).abs() < 1e-4);
        app.world_mut().run_system_once(refine_ground_sensor_system).unwrap();
        assert_eq!(app.world().get::<TnuaProximitySensor>(sensor).unwrap().output.as_ref().unwrap().normal, original.normal);
    }

    #[test]
    fn current_capsule_floor_wins_over_missing_pre_move_sensor_feedback() {
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        app.add_systems(Update, (move |entities: Query<&TnuaSensorsEntities<AlsControlScheme>>, mut sensors: Query<&mut TnuaProximitySensor>, mut injected: Local<bool>| {
            if *injected { return; }
            *injected = true;
            let sensor = entities.get(character).unwrap().ground.unwrap();
            sensors.get_mut(sensor).unwrap().output = None;
        }).after(TnuaPipelineSystems::Sensors).before(TnuaPipelineSystems::Logic));
        app.update();
        let position = app.world().get::<Position>(character).unwrap().0;
        let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
        assert!(TnuaBuiltinWalk::displacement(&app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis_access().unwrap()).is_none());
        assert!((MIN_FLOOR_DISTANCE..=MAX_FLOOR_DISTANCE).contains(&(position.y - 0.9)) && velocity.y.abs() < 1e-5);
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Grounded,
            "Current capsule floor was discarded after the floor adjustment");
        let base = app.world().get::<crate::als::dynamics::AlsMovementBase>(character).unwrap().entity;
        assert!(base.is_some(), "The current floor did not establish a movement base");
        assert_eq!(app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character).unwrap().entity, base);
        for _ in 0..30 { app.update(); }
        let sensor = app.world().get::<TnuaSensorsEntities<AlsControlScheme>>(character).unwrap().ground.unwrap();
        let controller = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap();
        assert!(TnuaBuiltinWalk::displacement(&app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis_access().unwrap()).is_some(),
            "The walking basis did not reacquire the supported capsule: position={:?} float_height={} sensor={:?}",
            app.world().get::<Position>(character).unwrap().0, controller.basis_access().unwrap().config.float_height,
            app.world().get::<TnuaProximitySensor>(sensor).unwrap());
    }

    #[test]
    fn walking_down_stairs_keeps_current_capsule_support_grounded() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::time::TimeUpdateStrategy;
        let mut failures = Vec::new();
        for rise in [0.18, 0.26, 0.36] {
            for delta in [1.0 / 60.0, 1.0 / 120.0, 0.0119908] {
                let (mut app, character, _) = stair_motion_fixture(rise, false, true, 0.0);
                *app.world_mut().resource_mut::<TimeUpdateStrategy>() = TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta));
                app.world_mut().get_mut::<crate::als::gait::AlsDesiredGait>(character).unwrap().0 = Gait::Walking;
                app.world_mut().get_mut::<AlsViewYaw>(character).unwrap().0 = std::f32::consts::PI;
                app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
                for frame in 0..600 {
                    app.update();
                    let position = app.world().get::<Position>(character).unwrap().0;
                    if position.z > 0.5 { break; }
                    if *app.world().get::<LocomotionState>(character).unwrap() == LocomotionState::Air {
                        let floor = app.world_mut().run_system_once(move |spatial: SpatialQuery,
                            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>| {
                            find_ground_floor(&spatial, position, 0.9, 0.3, &crate::als::params::CmcAdapterParams::default(), LocomotionState::Grounded, &SpatialQueryFilter::from_excluded_entities([character]), character, &candidates)
                        }).unwrap();
                        let velocity = app.world().get::<LinearVelocity>(character).unwrap().0;
                        failures.push((rise, delta, frame, position, velocity, floor));
                        break;
                    }
                }
            }
        }
        assert!(failures.is_empty(), "Supported stair descent became airborne: {failures:?}");
    }

    #[test]
    fn continuous_stairs_preserve_grounded_horizontal_motion() {
        use bevy::time::TimeUpdateStrategy;

        let mut failures = Vec::new();
        for (rise, ceiling) in [(0.18_f32, false), (0.26, false), (0.36, false), (0.6, false), (0.18, true)] {
            for descending in [false, true] {
                let blocked = rise > MAX_STEP_HEIGHT || ceiling;
                if descending && blocked { continue }
                for (yaw, variable_step) in [(0.0_f32, false), (std::f32::consts::FRAC_PI_2, false),
                    (std::f32::consts::PI, false), (-std::f32::consts::FRAC_PI_2, false),
                    (0.0, true), (-std::f32::consts::FRAC_PI_2, true)] {
                    let frame_rotation = Quat::from_rotation_y(yaw);
                    let (mut app, character, start_z) = stair_motion_fixture(rise, ceiling, descending, yaw);
                    app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
                    app.world_mut().get_mut::<AlsViewYaw>(character).unwrap().0 = yaw + if descending { std::f32::consts::PI } else { 0.0 };
                    let mut samples = 0;
                    let mut air_frames = 0;
                    let mut max_speed_error = 0.0_f32;
                    let mut max_displacement_speed_error = 0.0_f32;
                    let mut source_motion_violation = false;
                    for frame in 0..180 {
                        if variable_step {
                            *app.world_mut().resource_mut::<TimeUpdateStrategy>() = TimeUpdateStrategy::ManualDuration(
                                std::time::Duration::from_secs_f32([1.0 / 120.0, 1.0 / 60.0, 1.0 / 30.0, 0.05][frame % 4]));
                        }
                        let previous = frame_rotation.inverse() * app.world().get::<Position>(character).unwrap().0;
                        app.update();
                        let world_position = app.world().get::<Position>(character).unwrap().0;
                        let position = frame_rotation.inverse() * world_position;
                        assert!(world_position.distance(app.world().get::<Transform>(character).unwrap().translation) < 0.001);
                        if (-3.7..-0.74).contains(&position.z) {
                            samples += 1;
                            air_frames += usize::from(*app.world().get::<LocomotionState>(character).unwrap() != LocomotionState::Grounded);
                            let speed = app.world().get::<AlsActualVelocity>(character).unwrap().horizontal_speed;
                            max_speed_error = max_speed_error.max((speed - 3.75).abs());
                            let moved_speed = (position.z - previous.z).abs() / app.world().resource::<Time>().delta_secs();
                            max_displacement_speed_error = max_displacement_speed_error.max((moved_speed - 3.75).abs());
                            let source_sweep = app.world().get::<AlsCmcAdapterRuntime>(character).unwrap().grounded_move_start.is_some();
                            let dynamics = app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
                            let requested_speed = (dynamics.world_velocity + dynamics.motor_boost).with_y(0.0).length();
                            source_motion_violation |= speed > 3.85 || moved_speed > 3.85
                                || (!source_sweep && ((speed - requested_speed).abs() > 0.1 || (moved_speed - requested_speed).abs() > 0.1))
                                || (source_sweep && (speed - moved_speed).abs() > 0.001);
                        }
                    }
                    let position = frame_rotation.inverse() * app.world().get::<Position>(character).unwrap().0;
                    let distance = (position.z - start_z).abs();
                    println!("[STAIRS] rise={rise} descending={descending} ceiling={ceiling} yaw={yaw} variable_step={variable_step} distance={distance:.4} samples={samples} air_frames={air_frames} max_speed_error={max_speed_error:.4} max_displacement_speed_error={max_displacement_speed_error:.4} position={position:?}");
                    let failed = if blocked { distance > 1.5 } else {
                        distance < 6.44 || samples < 20 || air_frames != 0 || source_motion_violation
                    };
                    if failed {
                        failures.push((rise, descending, ceiling, yaw, distance, air_frames, max_speed_error, max_displacement_speed_error));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "Continuous stair movement failed: {failures:?}");
    }

    #[test]
    fn physical_locomotion_matches_the_current_controller_before_animation() {
        use crate::als::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use crate::als::transition::AlsStateTransitionPlugin;
        use avian3d::prelude::{PhysicsPlugins, RigidBody};
        use bevy::asset::AssetEvent;
        use bevy::time::TimeUpdateStrategy;
        use bevy_tnua::TnuaControllerPlugin;
        use bevy_tnua_avian3d::TnuaAvian3dPlugin;

        #[derive(Resource, Default)]
        struct SweptLanding(bool);

        for fixed_delta in [Some(0.013_f32), None] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                PhysicsPlugins::new(PostUpdate), TnuaAvian3dPlugin::new(Update),
                TnuaControllerPlugin::<AlsControlScheme>::new(Update),
                AlsStateTransitionPlugin, AlsCharacterPlugin, AlsMotionPlugin))
                .init_resource::<Assets<Mesh>>()
                .add_message::<AssetEvent<Mesh>>()
                .init_resource::<SweptLanding>()
                .add_systems(PostUpdate, (|mut sample: ResMut<SweptLanding>,
                    actors: Query<&AlsCmcAdapterRuntime, With<AlsCharacter>>| {
                    sample.0 = actors.iter().any(|runtime| runtime.airborne_sweep_contact);
                }).after(PhysicsSystems::Last).before(locomotion_state_sync_system))
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
            app.finish();
            let params_value = AlsMovementParams::default();
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(params_value.clone());
            let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>().add(initial_als_control_scheme_config(&params_value));
            let mut bundle = AlsCharacterBundle::new(params, config);
            bundle.transform = Transform::from_xyz(0.0,
                ALS_CHARACTER_CAPSULE_HALF_HEIGHT + ALS_CHARACTER_FLOOR_CLEARANCE
                    + 6.2 * 6.2 / (2.0 * 9.81 * params_value.jump.gravity_scale), 0.0);
            let character = app.world_mut().spawn(bundle).id();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 20.0), Transform::from_xyz(0.0, -0.1, 0.0)));
            let mut seen = [false; 2];
            let mut landed = false;
            let mut impact_seen = false;
            for frame in 0..120 {
                *app.world_mut().resource_mut::<TimeUpdateStrategy>() = TimeUpdateStrategy::ManualDuration(
                    std::time::Duration::from_secs_f32(fixed_delta.unwrap_or([1.0 / 120.0, 1.0 / 60.0, 1.0 / 30.0, 0.05][frame % 4])));
                let previous = *app.world().get::<LocomotionState>(character).unwrap();
                let previous_vertical = app.world().get::<LinearVelocity>(character).unwrap().y;
                app.update();
                let position = app.world().get::<Transform>(character).unwrap().translation;
                let vertical = app.world().get::<LinearVelocity>(character).unwrap().y;
                let swept_landing = app.world().resource::<SweptLanding>().0;
                let impact = previous == LocomotionState::Air && previous_vertical < -1.0
                    && vertical.abs() < 0.1
                    && (position.y <= ALS_CHARACTER_CAPSULE_HALF_HEIGHT + 0.001 || swept_landing);
                if impact {
                    impact_seen = true;
                    assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Grounded,
                        "ground impact must publish the mode in its physics frame={frame}");
                    if let Some(contact) = app.world().resource::<avian3d::prelude::ContactGraph>()
                        .contact_pairs_with(character).find(|contact| resolved_walkable_contact(contact, character)).cloned() {
                    let mut reversed = contact.clone();
                    std::mem::swap(&mut reversed.collider1, &mut reversed.collider2);
                    for manifold in &mut reversed.manifolds { manifold.normal = -manifold.normal; }
                    assert!(resolved_walkable_contact(&reversed, character));
                    let mut sensor = contact.clone();
                    sensor.flags.remove(ContactPairFlags::GENERATE_CONSTRAINTS);
                    assert!(!resolved_walkable_contact(&sensor, character));
                    for normal in [Vec3::X, Vec3::NEG_Y] {
                        let mut other = contact.clone();
                        for manifold in &mut other.manifolds {
                            manifold.normal = if other.collider1 == character { -normal } else { normal };
                        }
                        assert!(!resolved_walkable_contact(&other, character));
                    }
                    for (speed, impulse) in [(-1.0, 0.0), (1.0, 1.0)] {
                        let mut other = contact.clone();
                        for manifold in &mut other.manifolds {
                            for point in &mut manifold.points { point.normal_speed = speed; point.normal_impulse = impulse; }
                        }
                        assert!(!resolved_walkable_contact(&other, character));
                    }
                    } else { assert!(swept_landing, "Landing requires an actual source sweep or solver contact"); }
                }
                if let Ok(basis) = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis_access() {
                    let airborne = TnuaBuiltinWalk::displacement(&basis).is_none();
                    landed |= !airborne && seen[1];
                    assert!(!landed || !airborne, "landing rebounded at frame={frame}");
                    let floor_limit = MAX_STEP_HEIGHT + if previous == LocomotionState::Grounded {
                        MAX_FLOOR_DISTANCE + crate::als::movement_constants::SMALL_MOVEMENT_DISTANCE
                    } else { -MAX_FLOOR_DISTANCE };
                    let supported = position.y - ALS_CHARACTER_CAPSULE_HALF_HEIGHT <= floor_limit;
                    let expected = if (airborne || !supported) && !impact { LocomotionState::Air } else { LocomotionState::Grounded };
                    assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), expected, "frame={frame}");
                    let events: Vec<_> = app.world_mut().resource_mut::<Messages<StateTransitionEvent>>().drain()
                        .filter(|event| event.entity == character && event.dimension == Dimension::LocomotionState).collect();
                    assert_eq!(events.len(), usize::from(previous != expected));
                    if let Some(event) = events.first() {
                        assert_eq!(event.from, StateValue::LocomotionState(previous));
                        assert_eq!(event.to, StateValue::LocomotionState(expected));
                    }
                    seen[usize::from(airborne)] = true;
                }
            }
            assert_eq!(seen, [true, true]);
            assert!(impact_seen || fixed_delta.is_none());
            for hold_frames in [4, 0] {
                let mut jump_landed = false;
                for frame in 0..120 {
                    {
                        let mut input = app.world_mut().get_mut::<AlsInputCommand>(character).unwrap();
                        input.jump_pressed = frame == 0;
                        input.jump_held = frame < hold_frames;
                    }
                    app.update();
                    let mode = *app.world().get::<LocomotionState>(character).unwrap();
                    if frame < 4 {
                        if frame == 0 || frame < hold_frames {
                            assert_eq!(app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().action_flow_status().ongoing(),
                                Some(AlsControlSchemeActionDiscriminant::Jump));
                        }
                        assert_eq!(mode, LocomotionState::Air,
                            "an executing jump must publish Air in its first frame={frame}");
                    } else {
                        jump_landed |= mode == LocomotionState::Grounded;
                        assert!(!jump_landed || mode == LocomotionState::Grounded, "jump landing changed back to Air");
                    }
                }
                assert!(jump_landed);
            }
            app.world_mut().get_mut::<AlsStateRequest>(character).unwrap().locomotion_state = Some(LocomotionState::Custom);
            app.update();
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Custom));
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
            app.update();
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Custom));
            assert!(app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().action_flow_status().ongoing().is_none());
            app.world_mut().entity_mut(character).insert((LocomotionState::Custom,
                crate::als::state::AlsNotifyLocomotionAction::Mantling, AlsInputCommand::default()));
            app.update();
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
            app.update();
            assert!(app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().action_flow_status().ongoing().is_none());
            app.world_mut().get_mut::<AlsStateRequest>(character).unwrap().stance = Some(Stance::Crouching);
            app.update();
            assert_eq!(app.world().get::<Stance>(character), Some(&Stance::Standing));
            assert_eq!(app.world().get::<AlsDesiredStance>(character).unwrap().0, Stance::Crouching);
            app.world_mut().entity_mut(character).insert((LocomotionState::Grounded,
                crate::als::state::AlsNotifyLocomotionAction::None));
            app.update();
            assert_eq!(app.world().get::<Stance>(character), Some(&Stance::Crouching));
            *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Custom;
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = false;
            app.update();
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
            app.update();
            assert_eq!(app.world().get::<AlsDesiredStance>(character).unwrap().0, Stance::Standing);
            assert_eq!(app.world().get::<Stance>(character), Some(&Stance::Crouching));
            app.world_mut().entity_mut(character).insert((LocomotionState::None,
                crate::als::state::AlsNotifyLocomotionAction::Ragdolling));
            *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Grounded;
            let lifted = app.world().get::<Transform>(character).unwrap().with_translation(Vec3::Y * 10.0);
            app.world_mut().entity_mut(character).insert((lifted, Position(lifted.translation)));
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = false;
            app.update();
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
            app.update();
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded));
            let controller = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap();
            assert!(TnuaBuiltinWalk::displacement(&controller.basis_access().unwrap()).is_none());
            assert!(app.world().get::<AlsStateRequest>(character).unwrap().ragdoll_exit);
        }
    }

    #[test]
    fn blocked_movement_keeps_the_original_command_and_view_controls() {
        let input = AlsInputCommand {
            move_dir: Vec2::Y,
            jump_pressed: true,
            roll_pressed: true,
            look_delta: Vec2::X,
            ragdoll_pressed: true,
            ..default()
        };
        let blocked = movement_input_for_action(input, true);
        assert_eq!(blocked.move_dir, Vec2::ZERO);
        assert!(blocked.jump_pressed && !blocked.roll_pressed);
        assert_eq!(blocked.look_delta, input.look_delta);
        assert!(blocked.ragdoll_pressed);
        assert_eq!(input.move_dir, Vec2::Y);
        assert_eq!(movement_input_for_action(input, false), input);
    }

    #[test]
    fn crouching_stance_forces_effective_walking_gait() {
        assert_eq!(
            effective_gait(Stance::Crouching, Gait::Walking),
            Gait::Walking
        );
        assert_eq!(
            effective_gait(Stance::Crouching, Gait::Running),
            Gait::Walking
        );
        assert_eq!(
            effective_gait(Stance::Crouching, Gait::Sprinting),
            Gait::Walking
        );
        assert_eq!(
            effective_gait(Stance::Standing, Gait::Running),
            Gait::Running
        );
        assert_eq!(
            effective_gait(Stance::Standing, Gait::Sprinting),
            Gait::Sprinting
        );
    }

    #[test]
    fn effective_tnua_gravity_matches_physics_gravity_and_overrides() {
        let global = Vec3::new(0.0, -12.0, 0.0);
        assert_eq!(effective_tnua_gravity(global, 0.5, None), Vec3::Y * -6.0);
        assert_eq!(effective_tnua_gravity(global, 2.0, None), Vec3::Y * -24.0);
        assert_eq!(
            effective_tnua_gravity(global, 0.25, Some(Vec3::new(1.0, 5.0, -2.0))),
            Vec3::new(1.0, 2.0, -2.0)
        );
    }

    #[test]
    fn standing_gait_speeds_match_als_table() {
        let p = AlsMovementParams::default();
        assert!((gait_max_speed(&p, &Stance::Standing, &Gait::Walking) - 1.75).abs() < 1e-6);
        assert!((gait_max_speed(&p, &Stance::Standing, &Gait::Running) - 3.75).abs() < 1e-6);
        assert!((gait_max_speed(&p, &Stance::Standing, &Gait::Sprinting) - 6.50).abs() < 1e-6);
    }

    #[test]
    fn aiming_walk_speed_matches_the_source_table() {
        let p = AlsMovementParams::default();
        assert!(
            (gait_max_speed_for_rotation(
                &p,
                &Stance::Standing,
                &Gait::Walking,
                RotationMode::Aiming,
            ) - 1.65)
                .abs()
                < 1e-6
        );
        assert!(
            (gait_max_speed_for_rotation(
                &p,
                &Stance::Standing,
                &Gait::Running,
                RotationMode::Aiming,
            ) - 3.75)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn default_source_jump_trajectory_is_independent_of_physical_hold_duration() {
        let mut baseline: Option<Vec<(f32, f32, LocomotionState)>> = None;
        for hold_frames in [0, 4, 30, 60] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            let mut samples = Vec::new();
            let start_y = app.world().get::<Position>(character).unwrap().y;
            let mut launched = false;
            let mut landed = false;
            for frame in 0..90 {
                let mut input = app.world_mut().get_mut::<AlsInputCommand>(character).unwrap();
                input.jump_pressed = frame == 0;
                input.jump_held = frame < hold_frames;
                app.update();
                let position = app.world().get::<Position>(character).unwrap().y - start_y;
                let velocity = app.world().get::<LinearVelocity>(character).unwrap().y;
                let mode = *app.world().get::<LocomotionState>(character).unwrap();
                if frame == 0 {
                    assert!((velocity - (4.2 - 9.81 / 60.0)).abs() < 0.02,
                        "Source default Jump launch after gravity for hold={hold_frames}: velocity={velocity}");
                }
                launched |= mode == LocomotionState::Air;
                landed |= frame > 4 && launched && mode == LocomotionState::Grounded;
                samples.push((position, velocity, mode));
            }
            assert!(launched && landed, "Actual default Jump must launch and land for hold={hold_frames}");
            if let Some(ref reference) = baseline {
                for (frame, (actual, expected)) in samples.iter().zip(reference.iter()).enumerate() {
                    assert!((actual.0 - expected.0).abs() < 1e-5 && (actual.1 - expected.1).abs() < 1e-5
                        && actual.2 == expected.2,
                        "Source MaxHoldTime zero trajectory changed with hold={hold_frames}, frame={frame}: actual={actual:?}, expected={expected:?}");
                }
            } else {
                baseline = Some(samples);
            }
        }
    }

    #[test]
    fn active_jump_hold_resets_when_custom_movement_interrupts_flight() {
        for (mode, action) in [
            (LocomotionState::Custom, crate::als::state::AlsNotifyLocomotionAction::Mantling),
            (LocomotionState::Air, crate::als::state::AlsNotifyLocomotionAction::Ragdolling),
            (LocomotionState::Grounded, crate::als::state::AlsNotifyLocomotionAction::None),
        ] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
            app.world_mut().resource_mut::<Assets<AlsMovementParams>>().get_mut(&handle).unwrap().jump.max_hold_time = 0.2;
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_held = true;
            app.update();
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Air));
            app.world_mut().entity_mut(character).insert((mode, action,
                bevy_tnua::TnuaToggle::Disabled, RigidBody::Kinematic,
                AlsInputCommand { jump_held: true, ..default() },
                Position(Vec3::new(0.0, 10.0, 1.0)), Transform::from_xyz(0.0, 10.0, 1.0),
                LinearVelocity::ZERO));
            for _ in 0..4 { app.update(); }
            app.world_mut().entity_mut(character).insert((LocomotionState::Air,
                crate::als::state::AlsNotifyLocomotionAction::None,
                bevy_tnua::TnuaToggle::Enabled, RigidBody::Dynamic, LinearVelocity::ZERO));
            app.update();
            let actual = app.world().get::<LinearVelocity>(character).unwrap().y;
            assert!((actual + 9.81 / 60.0).abs() < 0.02,
                "Source non-Falling mode resets active Jump force; later Falling cannot resume physical hold without a new Jump: mode={mode:?}, action={action:?}, actual={actual}");
        }
    }

    #[test]
    fn paused_source_owner_does_not_auto_start_a_valid_air_mantle() {
        for (allow_mantling, auto_start_in_air, action_active) in [
            (true, true, false), (false, true, false), (true, false, false), (true, true, true),
        ] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            app.world_mut().spawn((RigidBody::Static,
                Collider::cuboid(2.4, 0.8, 1.2), Transform::from_xyz(0.0, 0.4, 4.6),
                crate::als::mantle::AlsMantleTargetPermission::default(),
                CollisionLayers::new([crate::als::character::AlsCollisionLayer::Default,
                    crate::als::character::AlsCollisionLayer::Visibility], avian3d::prelude::LayerMask::ALL)));
            app.world_mut().entity_mut(character).insert((AlsMantleState::default(),
                Position(Vec3::new(0.0, 0.924, 6.0)), Transform::from_xyz(0.0, 0.924, 6.0)));
            for _ in 0..10 { app.update(); }
            assert!(!app.world().get::<AlsMantleState>(character).unwrap().active);
            app.world_mut().entity_mut(character).insert(AlsMantleControlSettings {
                allow_mantling: false, ..default()
            });
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
            app.update();
            assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air,
                "Actual native Jump must reach the source valid air mode before pause");
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = false;
            app.world_mut().entity_mut(character).insert(AlsMantleControlSettings {
                allow_mantling, auto_start_in_air, ..default()
            });
            if action_active {
                app.world_mut().entity_mut(character).insert(crate::als::state::AlsNotifyLocomotionAction::Rolling);
            }
            app.world_mut().resource_mut::<Time<Virtual>>().pause();
            app.update();
            assert!(!app.world().get::<AlsMantleState>(character).unwrap().active,
                "Source paused Character must not run AutoStartMantling even when its air trace is valid");
            app.world_mut().resource_mut::<Time<Virtual>>().unpause();
            app.update();
            assert_eq!(app.world().get::<AlsMantleState>(character).unwrap().active,
                allow_mantling && auto_start_in_air && !action_active,
                "Source resumed Character must auto start the same valid air mantle");
        }
    }

    #[test]
    fn auto_mantle_does_not_start_after_the_movement_tick_lands() {
        use bevy::time::TimeUpdateStrategy;
        use crate::als::character::AlsCollisionLayer;
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.4, 0.8, 1.2),
            Transform::from_xyz(0.0, 0.4, -1.4), CollisionLayers::new(
                [AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], avian3d::prelude::LayerMask::ALL)));
        app.update();
        app.update();
        let start = Vec3::new(0.0, 0.94, 0.0);
        app.world_mut().entity_mut(character).insert((Position(start), Transform::from_translation(start),
            LocomotionState::Air, LinearVelocity(Vec3::NEG_Y * 4.0), TnuaMotor::default(),
            crate::als::dynamics::AlsMovementBase::default(), crate::als::dynamics::AlsMovementBaseDynamicsState::default(),
            AlsCmcAdapterRuntime::default(), crate::als::state::AlsNotifyLocomotionAction::None));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.05)));
        app.update();
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Grounded);
        assert!(!app.world().get::<AlsMantleState>(character).unwrap().active);
        assert!(app.world().get::<AlsAutoMantlePending>(character).is_none());
    }

    #[test]
    fn actual_jump_priority_uses_ragdoll_mantle_stance_then_native_jump() {
        for (ragdolling, crouching, allows_mantle) in [
            (true, true, true), (false, true, true), (false, true, false),
            (false, false, true), (false, false, false),
        ] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            let target = app.world_mut().spawn((RigidBody::Static,
                Collider::cuboid(2.4, 0.8, 1.2), Transform::from_xyz(0.0, 0.4, 4.6),
                crate::als::mantle::AlsMantleTargetPermission { allows_step_up: allows_mantle },
                CollisionLayers::new([crate::als::character::AlsCollisionLayer::Default,
                    crate::als::character::AlsCollisionLayer::Visibility], avian3d::prelude::LayerMask::ALL))).id();
            app.world_mut().entity_mut(character).insert((AlsMantleState::default(),
                Position(Vec3::new(0.0, 0.924, 6.0)), Transform::from_xyz(0.0, 0.924, 6.0)));
            if crouching { app.world_mut().get_mut::<AlsDesiredStance>(character).unwrap().0 = Stance::Crouching; }
            for _ in 0..10 { app.update(); }
            if ragdolling {
                app.world_mut().entity_mut(character).insert(crate::als::state::AlsNotifyLocomotionAction::Ragdolling);
            }
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
            app.update();
            let exit = app.world().get::<AlsStateRequest>(character).unwrap().ragdoll_exit;
            let mantle = app.world().get::<AlsMantleState>(character).unwrap();
            let flow = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().action_flow_status();
            assert_eq!(exit, ragdolling, "Actual source stopRagdolling priority differs");
            assert_eq!(mantle.active, !ragdolling && allows_mantle, "Actual source StartMantling return priority differs");
            if mantle.active { assert_eq!(mantle.target_base.unwrap().entity, target); }
            if !ragdolling && !allows_mantle && crouching {
                assert_eq!(app.world().get::<AlsDesiredStance>(character).unwrap().0, Stance::Standing);
            }
            assert_eq!(flow.just_starting() == Some(AlsControlSchemeActionDiscriminant::Jump),
                !ragdolling && !allows_mantle && !crouching,
                "Actual native Jump must only run after earlier source branches return false");
        }
    }

    #[test]
    fn native_mode_reset_does_not_replay_jump_pressed_during_mantle() {
        for max_hold_time in [0.0, 0.2] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            let handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
            app.world_mut().resource_mut::<Assets<AlsMovementParams>>().get_mut(&handle).unwrap().jump.max_hold_time = max_hold_time;
            app.world_mut().entity_mut(character).insert((LocomotionState::Custom,
                crate::als::state::AlsNotifyLocomotionAction::Mantling,
                AlsInputCommand { jump_pressed: true, jump_held: true, ..default() }));
            app.update();
            assert!(app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().action_flow_status().ongoing().is_none());
            app.world_mut().entity_mut(character).insert((LocomotionState::Grounded,
                crate::als::state::AlsNotifyLocomotionAction::None,
                AlsInputCommand { jump_held: true, ..default() }));
            for _ in 0..6 { app.update(); }
            assert!(app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().action_flow_status().ongoing().is_none(),
                "Source entering Walking resets pending native Jump: hold={max_hold_time}");
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded));
        }
    }

    #[test]
    fn source_crouched_jump_stands_without_physical_hold_affecting_crouch() {
        let mut baseline: Option<Vec<(f32, f32, Stance)>> = None;
        for held_frames in [0, 30] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            app.world_mut().get_mut::<AlsDesiredStance>(character).unwrap().0 = Stance::Crouching;
            for _ in 0..10 { app.update(); }
            assert_eq!(app.world().get::<Stance>(character), Some(&Stance::Crouching));
            assert_eq!(app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().action_flow_status().ongoing(),
                Some(AlsControlSchemeActionDiscriminant::Crouch));
            let mut samples = Vec::new();
            for frame in 0..35 {
                let mut input = app.world_mut().get_mut::<AlsInputCommand>(character).unwrap();
                input.jump_pressed = frame == 0;
                input.jump_held = frame < held_frames;
                app.update();
                assert_eq!(app.world().get::<AlsDesiredStance>(character).unwrap().0, Stance::Standing);
                let sample = (app.world().get::<Position>(character).unwrap().y,
                    app.world().get::<LinearVelocity>(character).unwrap().y,
                    *app.world().get::<Stance>(character).unwrap());
                samples.push(sample);
            }
            if let Some(ref reference) = baseline {
                for (frame, (actual, expected)) in samples.iter().zip(reference).enumerate() {
                    assert!((actual.0 - expected.0).abs() < 1e-5 && (actual.1 - expected.1).abs() < 1e-5 && actual.2 == expected.2,
                        "Source crouched Jump only sets DesiredStanding; held must not alter physical crouch exit: frame={frame}, actual={actual:?}, expected={expected:?}");
                }
            } else { baseline = Some(samples); }
        }
    }

    #[test]
    fn getting_up_consumes_zero_movement_and_rejects_native_jump() {
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        app.world_mut().entity_mut(character).insert((
            crate::get_up::AlsGetUpState::new(false),
            crate::als::state::AlsNotifyLocomotionAction::GettingUp,
            AlsMovementInputBlocked,
            AlsInputCommand { move_dir: Vec2::Y, jump_pressed: true, jump_held: true, ..default() },
        ));
        app.update();
        let controller = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap();
        assert_eq!(controller.basis.desired_motion, Vec3::ZERO,
            "Source GetUp InputBlocked must consume zero movement");
        assert_eq!(controller.action_flow_status().just_starting(), None,
            "Source nonempty GetUp action rejects the final native Jump even when its input event is dispatched");
        let velocity = app.world().get::<LinearVelocity>(character).unwrap().y;
        assert!(velocity.abs() < 0.02, "Source rejected Jump keeps the grounded vertical velocity: {velocity}");
        assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Grounded));
    }

    #[test]
    fn configured_source_jump_hold_maintains_launch_velocity_until_release() {
        for (delta, speed, held_frames, max_hold_time) in [
            (1.0 / 60.0, 4.2, 6, 0.2),
            (1.0 / 60.0, 4.2, 24, 0.2),
            (1.0 / 30.0, 5.0, 16, 0.2),
            (1.0 / 120.0, 3.5, 32, 0.07),
        ] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            *app.world_mut().resource_mut::<bevy::time::TimeUpdateStrategy>() =
                bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta));
            let params_handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
            let source_override: crate::als::params::AlsMovementParamsOverride = serde_json::from_value(
                serde_json::json!({"jump":{"max_hold_time":max_hold_time,"jump_z_velocity":speed}})).unwrap();
            app.world_mut().resource_mut::<Assets<AlsMovementParams>>().get_mut(&params_handle).unwrap()
                .apply_override(&source_override);
            let mut elapsed = 0.0_f32;
            let mut expected_velocity = 0.0_f32;
            for frame in 0..held_frames + 4 {
                let mut input = app.world_mut().get_mut::<AlsInputCommand>(character).unwrap();
                input.jump_pressed = frame == 0;
                input.jump_held = frame < held_frames;
                if frame == 0 || (frame < held_frames && elapsed < max_hold_time) {
                    expected_velocity = expected_velocity.max(speed);
                }
                expected_velocity -= 9.81 * delta;
                app.update();
                let velocity = app.world().get::<LinearVelocity>(character).unwrap().y;
                assert!((velocity - expected_velocity).abs() < 0.02,
                    "Source CheckJumpInput/ClearJumpInput hold timing differs: hold={max_hold_time}, held_frames={held_frames}, delta={delta}, frame={frame}, actual={velocity}, expected={expected_velocity}");
                elapsed += delta;
            }
        }
    }

    #[test]
    fn configured_source_jump_hold_uses_source_speed_without_gravity_or_velocity_loss() {
        for (max_hold_time, gravity, initial_velocity) in [(0.2, 0.0, 0.0), (0.2, 9.81, 6.2), (0.0, 0.0, 0.0), (0.0, 9.81, 6.2)] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            let params_handle = app.world().get::<AlsMovementParamsHandle>(character).unwrap().0.clone();
            app.world_mut().resource_mut::<Assets<AlsMovementParams>>().get_mut(&params_handle).unwrap()
                .jump.max_hold_time = max_hold_time;
            app.world_mut().resource_mut::<Gravity>().0 = Vec3::NEG_Y * gravity;
            app.world_mut().get_mut::<LinearVelocity>(character).unwrap().y = initial_velocity;
            let mut expected = initial_velocity;
            for frame in 0..6 {
                let mut input = app.world_mut().get_mut::<AlsInputCommand>(character).unwrap();
                input.jump_pressed = frame == 0;
                input.jump_held = true;
                expected = expected.max(4.2) - gravity / 60.0;
                app.update();
                let actual = app.world().get::<LinearVelocity>(character).unwrap().y;
                assert!((actual - expected).abs() < 0.02,
                    "Source DoJump keeps max of current and source launch velocity: hold={max_hold_time}, gravity={gravity}, initial={initial_velocity}, frame={frame}, actual={actual}, expected={expected}, mode={:?}, position={:?}, motor={:?}, action={:?}",
                    app.world().get::<LocomotionState>(character), app.world().get::<Position>(character),
                    app.world().get::<TnuaMotor>(character),
                    app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().action_flow_status());
            }
        }
    }

    #[test]
    fn jump_height_keeps_source_launch_speed_when_world_gravity_changes() {
        for (global, scale, extra) in [
            (Vec3::Y * -12.0, 1.0, Vec3::ZERO),
            (Vec3::Y * -9.81, 2.0, Vec3::ZERO),
            (Vec3::Y * -9.81, 1.0, Vec3::new(3.0, -2.0, 0.0)),
        ] {
            let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
            app.world_mut().resource_mut::<Gravity>().0 = global;
            app.world_mut().entity_mut(character).insert((GravityScale(scale), TnuaGravity(extra)));
            app.update();
            let world = app.world();
            let gravity = world.get::<TnuaRigidBodyTracker>(character).unwrap().gravity;
            assert!(gravity.abs_diff_eq(effective_tnua_gravity(global, scale, Some(extra)), 1e-4));
            let config_handle = world.get::<TnuaConfig<AlsControlScheme>>(character).unwrap();
            let config = world.resource::<Assets<AlsControlSchemeConfig>>().get(&config_handle.0).unwrap();
            let params_handle = world.get::<AlsMovementParamsHandle>(character).unwrap();
            let params = world.resource::<Assets<AlsMovementParams>>().get(params_handle.id()).unwrap();
            let launch_speed = (2.0 * gravity.length() * config.jump.height).sqrt();
            assert!((launch_speed - params.jump.jump_z_velocity).abs() < 1e-3,
                "gravity={gravity:?} launch_speed={launch_speed} source_jump_z_velocity={}", params.jump.jump_z_velocity);
        }
    }

    #[test]
    fn initial_control_scheme_matches_the_first_runtime_sync() {
        let params = AlsMovementParams::default();
        let mut config = initial_als_control_scheme_config(&params);
        assert!((config.jump.height - params.jump.jump_z_velocity.powi(2)
            / (2.0 * Gravity::default().0.length())).abs() < 1e-6);
        assert!((config.basis.float_height - 0.924).abs() < 1e-6);
        assert!((config.basis.speed - 3.75).abs() < 1e-6);
        assert!((config.basis.cling_distance - WALK_CLING_DISTANCE).abs() < 1e-6);
        assert!((config.basis.air_acceleration - 6.0).abs() < 1e-6);
        assert_eq!(config.basis.acceleration, 0.0);
        assert_eq!(config.basis.coyote_time, 0.0);
        assert_eq!(config.basis.free_fall_extra_gravity, 0.0);
        assert_eq!(config.jump.upslope_extra_gravity, 0.0);
        assert_eq!(config.jump.takeoff_extra_gravity, 0.0);
        assert_eq!(config.jump.fall_extra_gravity, 0.0);
        assert_eq!(config.jump.shorten_extra_gravity, 0.0);
        assert_eq!(config.jump.peak_prevention_extra_gravity, 0.0);
        assert_eq!(config.jump.input_buffer_time, 0.0);
        assert_eq!(
            config
                .basis
                .headroom
                .as_ref()
                .unwrap()
                .distance_to_collider_top,
            params.crouch.standing_capsule_half_height
        );
        apply_als_control_scheme_config(&mut config, &params, Gait::Running,
            Stance::Standing, RotationMode::ViewDirection, false, Vec3::ZERO, 0.0, 0.0, 1.0);
        assert_eq!(config.jump.height, 0.0);
    }

    #[test]
    fn direction_dependent_speed_updates_the_tnua_config() {
        let mut params = AlsMovementParams::default();
        params
            .speeds
            .standing
            .allow_direction_dependent_movement_speed = true;
        params.speeds.standing.max_run_backward_speed = 2.0;
        let mut config = AlsControlSchemeConfig::default();
        apply_als_control_scheme_config(
            &mut config,
            &params,
            Gait::Running,
            Stance::Standing,
            RotationMode::ViewDirection,
            false,
            Vec3::Z,
            0.0,
            Gravity::default().0.length(),
            1.0,
        );
        println!(
            "[BF101.5] tnua_backward_run_speed={:.6}",
            config.basis.speed
        );
        assert!((config.basis.speed - 2.0).abs() < 1e-6);
    }

    #[test]
    fn crouching_gait_speeds_match_the_source_table() {
        let p = AlsMovementParams::default();
        assert!((gait_max_speed(&p, &Stance::Crouching, &Gait::Walking) - 1.50).abs() < 1e-6);
        assert!((gait_max_speed(&p, &Stance::Crouching, &Gait::Running) - 2.00).abs() < 1e-6);
        assert!((gait_max_speed(&p, &Stance::Crouching, &Gait::Sprinting) - 3.00).abs() < 1e-6);
    }

    #[test]
    fn air_control_matches_source_base_and_low_speed_boost() {
        let p = AlsMovementParams::default();
        assert!((air_acceleration_for_horizontal_speed(&p, 0.0) - 6.0).abs() < 1e-6);
        assert!((air_acceleration_for_horizontal_speed(&p, 0.25) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn falling_uses_the_frame_input_before_applying_air_control_limits() {
        let params = AlsMovementParams::default();
        let cached = 12.007459;
        assert!((falling_input_acceleration(&params, Vec3::X, 5.0, Some(cached)) - cached * 0.15).abs() < 1e-6);
        assert!((falling_input_acceleration(&params, Vec3::X, 0.0, Some(cached)) - cached * 0.3).abs() < 1e-6);
        assert_eq!(falling_input_acceleration(&params, Vec3::ZERO, 5.0, Some(cached)), 0.0);
        assert!((falling_input_acceleration(&params, Vec3::X * 0.1, 5.0, Some(200.0)) * 0.1 - 3.0).abs() < 1e-6);
        assert!((falling_input_acceleration(&params, Vec3::X, 5.0, Some(200.0)) - 20.0).abs() < 1e-6);
    }

    #[test]
    fn falling_transaction_integrates_cached_walking_input_and_refreshes_in_air() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
            .add_message::<StateTransitionEvent>();
        app.finish();
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        let (cached, airborne) = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
            permissions: Query<&AlsMantleTargetPermission>,
            bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
            owners: Query<&ColliderOf>, shapes: Query<(&Collider, &Position, &Rotation)>,
            mut transitions: MessageWriter<StateTransitionEvent>| {
            let capsule = als_character_capsule(0.9, 0.3);
            let params = AlsMovementParams::default();
            let mut seed = 123;
            let cached = falling_movement_transaction(&spatial, &capsule, Transform::from_xyz(0.0, 4.0, 0.0),
                Vec3::X * 5.0, Vec3::X, 6.5, &params, Vec3::NEG_Y * 9.81,
                (Stance::Standing, RotationMode::ViewDirection, Gait::Running, 0.0),
                (0.0, false, false), 40.0, 0.03, 1, actor, &mut seed, &candidates,
                &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, Some(12.007459), None);
            let airborne = falling_movement_transaction(&spatial, &capsule, Transform::from_xyz(0.0, 4.0, 0.0),
                Vec3::X * 5.0, Vec3::X, 6.5, &params, Vec3::NEG_Y * 9.81,
                (Stance::Standing, RotationMode::ViewDirection, Gait::Running, 0.0),
                (0.0, false, false), 40.0, 0.03, 1, actor, &mut seed, &candidates,
                &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, None, None);
            (cached, airborne)
        }).unwrap();
        assert!((cached.1.x - (5.0 + 12.007459 * 0.15 * 0.03)).abs() < 1e-5);
        assert!((airborne.1.x - 5.09).abs() < 1e-5);
        assert!((cached.1.y - airborne.1.y).abs() < 1e-6);
    }

    #[test]
    fn landing_reuses_controlled_acceleration_and_reports_only_executed_walking_refresh() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)))
            .add_message::<StateTransitionEvent>();
        app.finish();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(100.0, 0.2, 100.0), Transform::from_xyz(0.0, -0.1, 0.0)));
        let actor = app.world_mut().spawn_empty().id();
        app.update();
        app.update();
        let (slow, fast, exhausted) = app.world_mut().run_system_once(move |spatial: SpatialQuery, impact_bodies: FallingImpactBodies,
            candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
            permissions: Query<&AlsMantleTargetPermission>,
            bases: Query<(&Position, &LinearVelocity, Option<&AngularVelocity>, Option<&RigidBody>, Option<&AlsComponentVelocity>), Without<AlsCharacter>>,
            owners: Query<&ColliderOf>, shapes: Query<(&Collider, &Position, &Rotation)>,
            mut transitions: MessageWriter<StateTransitionEvent>| {
            let mut run = |fresh: f64, max_iterations: u32| {
                let mut params = AlsMovementParams::default();
                params.cmc.max_simulation_iterations = max_iterations;
                params.acceleration.grounded.normal.acceleration.keys = vec![crate::als::params::HermiteKey {
                    time: 0.0, value: fresh, arrive_tangent: 0.0, leave_tangent: 0.0,
                }];
                let mut seed = 123;
                falling_movement_transaction(&spatial, &als_character_capsule(0.9, 0.3),
                    Transform::from_xyz(0.0, 1.0, 0.0), Vec3::new(4.0, -2.0, 0.0), Vec3::X,
                    6.5, &params, Vec3::NEG_Y * 9.81,
                    (Stance::Standing, RotationMode::ViewDirection, Gait::Sprinting, 0.0),
                    (0.0, false, false), 40.0, 0.1, 0, actor, &mut seed, &candidates,
                    &permissions, &bases, &owners, &shapes, &impact_bodies, &mut transitions, Some(20.0), None)
            };
            (run(7.5, 8), run(20.0, 8), run(7.5, 1))
        }).unwrap();
        assert_eq!(slow.4, LocomotionState::Grounded);
        assert_eq!(fast.4, LocomotionState::Grounded);
        assert_eq!(slow.1, fast.1);
        assert_eq!(slow.0.translation, fast.0.translation);
        assert_eq!(slow.8.unwrap().acceleration, 7.5);
        assert_eq!(fast.8.unwrap().acceleration, 20.0);
        assert_eq!(exhausted.4, LocomotionState::Grounded);
        assert!(exhausted.8.is_none());
    }

    #[test]
    fn landing_publishes_the_last_walking_curve_for_the_next_input_frame() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::als::dynamics::{AlsGroundedDynamics, AlsMovementBase, AlsMovementBaseDynamicsState, grounded_dynamics_motor_system};

        for (iterations, expected) in [(8, 7.5_f32), (1, 3.0_f32)] {
            let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
            let handle = app.world().get::<AlsMovementParamsHandle>(actor).unwrap().0.clone();
            let mut params = app.world().resource::<Assets<AlsMovementParams>>().get(&handle).unwrap().clone();
            params.cmc.max_simulation_iterations = iterations;
            params.acceleration.grounded.normal.acceleration.keys = vec![crate::als::params::HermiteKey {
                time: 0.0, value: 7.5, arrive_tangent: 0.0, leave_tangent: 0.0,
            }];
            let handle = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(params);
            let start = Vec3::new(0.0, 1.0, 0.0);
            app.world_mut().entity_mut(actor).insert((Position(start), Transform::from_translation(start),
                GlobalTransform::from_translation(start), LinearVelocity(Vec3::new(4.0, -2.0, 0.0)),
                LocomotionState::Air, Stance::Standing, RotationMode::ViewDirection, AlsViewYaw(0.0),
                AlsMaxAllowedGait(Gait::Sprinting), AlsMovementParamsHandle(handle), TnuaMotor::default(),
                AlsGroundedDynamics(crate::als::params::GroundedDynamics { acceleration: 3.0, ..default() }),
                AlsMovementBase::default(), AlsMovementBaseDynamicsState::default(),
                AlsCmcAdapterRuntime { controlled_input_max_acceleration: Some(20.0), grounded_max_speed: 6.5, ..default() }));
            app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(actor).unwrap().basis.desired_motion = Vec3::X;
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.1));
            app.world_mut().run_system_once(cmc_movement_transaction_system).unwrap();
            assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), LocomotionState::Grounded);
            assert_eq!(app.world().get::<AlsGroundedDynamics>(actor).unwrap().0.acceleration, expected);
            assert_eq!(app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().controlled_input_max_acceleration, Some(20.0));
            app.world_mut().run_system_once(grounded_dynamics_motor_system).unwrap();
            assert_eq!(app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().controlled_input_max_acceleration, Some(expected));
        }
    }

    #[test]
    fn input_acceleration_is_captured_before_walking_refresh_and_reset_next_frame() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().get_mut::<crate::als::dynamics::AlsGroundedDynamics>(actor).unwrap().0.acceleration = 12.007459;
        *app.world_mut().get_mut::<LocomotionState>(actor).unwrap() = LocomotionState::Grounded;
        app.world_mut().run_system_once(crate::als::dynamics::grounded_dynamics_motor_system).unwrap();
        assert_eq!(app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().controlled_input_max_acceleration, Some(12.007459));
        *app.world_mut().get_mut::<LocomotionState>(actor).unwrap() = LocomotionState::Air;
        app.world_mut().run_system_once(crate::als::dynamics::grounded_dynamics_motor_system).unwrap();
        let handle = app.world().get::<AlsMovementParamsHandle>(actor).unwrap();
        let air_max = app.world().resource::<Assets<AlsMovementParams>>().get(&handle.0).unwrap().acceleration.max_acceleration;
        assert_eq!(app.world().get::<AlsCmcAdapterRuntime>(actor).unwrap().controlled_input_max_acceleration, Some(air_max));
    }
}
