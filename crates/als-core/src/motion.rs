use avian3d::prelude::{
    Collider, Collisions, ContactPair, ContactPairFlags, Gravity, GravityScale, LinearVelocity, PhysicsSystems, Position, Rotation,
    RigidBody, ShapeCastConfig, ShapeHitData, SpatialQuery, SpatialQueryFilter, SpeculativeMargin,
};
use bevy::asset::Assets;
use bevy::prelude::*;
use bevy_tnua::builtins::{
    TnuaBuiltinCrouch, TnuaBuiltinJump, TnuaBuiltinWalk, TnuaBuiltinWalkHeadroom,
};
use bevy_tnua::controller::TnuaConfig;
use bevy_tnua::basis_capabilities::TnuaBasisWithDisplacement;
use bevy_tnua::{TnuaController, TnuaGravity, TnuaMotor, TnuaPipelineSystems, TnuaProximitySensor, TnuaRigidBodyTracker, TnuaSensorsEntities};

use crate::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_FLOOR_CLEARANCE,
    ALS_CHARACTER_TNUA_FLOAT_EPSILON, AlsCharacter, AlsCmcAdapterRuntime, AlsEffectiveGravity, AlsMovementParamsHandle,
    als_character_capsule,
};
use crate::control_scheme::{AlsControlScheme, AlsControlSchemeActionDiscriminant, AlsControlSchemeConfig};
use crate::gait::AlsMaxAllowedGait;
use crate::generated_movement_constants::{
    EDGE_REJECT_DISTANCE, FLOOR_RETRY_SHRINK_FRACTION, FLOOR_SWEEP_SHRINK_FRACTION,
    MIN_FLOOR_DISTANCE, MAX_FLOOR_DISTANCE, MAX_SLOPE_DEGREES, MAX_STEP_HEIGHT, PULLBACK_MAX_DISTANCE,
    PULLBACK_MIN_DISTANCE, PULLBACK_PROPORTION, PULLBACK_TIME_BIAS, SWEEP_EDGE_MIN_RADIUS,
};
use crate::input::AlsInputCommand;
use crate::mantle::{
    AlsMantleControlSettings, AlsMantleStartTimeSettings, AlsMantleState, AlsMantleTargetBase,
    AlsMantleTargetPermission, MantleDetectionInput, auto_mantle_start_allowed,
    configured_mantle_start_time, detect_mantle, mantle_start_allowed,
    scaled_mantle_capsule_dimensions, yaw_from_direction,
};
use crate::orientation::{world_forward_to_yaw, yaw_to_world_forward};
use crate::params::AlsMovementParams;
use crate::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use crate::transition::{
    AlsDesiredStance, AlsPhysicalStanceOverride, AlsStateRequest, Dimension, StateTransitionEvent,
    StateValue,
};

const WALK_CLING_DISTANCE: f32 = ALS_CHARACTER_TNUA_FLOAT_EPSILON;
const AIR_CONTROL: f32 = 0.15;
const AIR_CONTROL_BOOST_MULTIPLIER: f32 = 2.0;
const AIR_CONTROL_BOOST_SPEED: f32 = 0.25;
const IN_AIR_ROTATION_MODE: u8 = include!("../data/in_air_rotation_mode.rs");

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsActualVelocity {
    pub world_velocity: Vec3,
    pub horizontal_speed: f32,
}

impl AlsActualVelocity {
    fn from_physics_velocity(world_velocity: Vec3) -> Self {
        Self {
            horizontal_speed: Vec2::new(world_velocity.x, world_velocity.z).length(),
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
    let total_half = if stance == Stance::Crouching { params.crouch.crouched_capsule_half_height } else { params.crouch.standing_capsule_half_height };
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
) {
    config.basis.speed =
        gait_max_speed_for_velocity(params, &stance, &gait, rotation_mode, velocity, view_yaw);
    config.basis.float_height = params.crouch.standing_capsule_half_height
        + ALS_CHARACTER_FLOOR_CLEARANCE
        + ALS_CHARACTER_TNUA_FLOAT_EPSILON;
    config.basis.acceleration = 0.0;
    config.basis.air_acceleration =
        air_acceleration_for_horizontal_speed(params, Vec2::new(velocity.x, velocity.z).length());
    config.basis.coyote_time = params.jump.coyote_time;
    config.basis.free_fall_extra_gravity = 0.0;
    config.basis.headroom = Some(TnuaBuiltinWalkHeadroom {
        distance_to_collider_top: if capsule_crouched {
            params.crouch.crouched_capsule_half_height
        } else {
            params.crouch.standing_capsule_half_height
        },
        ..Default::default()
    });
    config.basis.turning_angvel = turn_angvel(rotation_mode);
    config.basis.max_slope = deg_to_rad(MAX_SLOPE_DEGREES);
    config.basis.cling_distance = WALK_CLING_DISTANCE;
    config.basis.spring_dampening = 1.0;
    let gravity_scale = params.jump.gravity_scale.max(1e-3);
    config.jump.height =
        params.jump.jump_z_velocity * params.jump.jump_z_velocity / (2.0 * 9.81 * gravity_scale);
    config.jump.upslope_extra_gravity = 0.0;
    config.jump.takeoff_extra_gravity = 0.0;
    config.jump.takeoff_above_velocity = f32::INFINITY;
    config.jump.fall_extra_gravity = 0.0;
    config.jump.shorten_extra_gravity = 0.0;
    config.jump.peak_prevention_at_upward_velocity = f32::INFINITY;
    config.jump.peak_prevention_extra_gravity = 0.0;
    config.jump.input_buffer_time = params.jump.jump_buffer_time;
    config.crouch.float_offset =
        -(params.crouch.standing_capsule_half_height - params.crouch.crouched_capsule_half_height);
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

pub fn view_relative_move_direction(move_dir: Vec2, view_yaw: f32) -> Vec3 {
    let axes = view_relative_move_axes(move_dir, view_yaw);
    Vec3::new(axes.x, 0.0, -axes.y).normalize_or_zero()
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
        view_space_yaw = damper_yaw(view_space_yaw, target, delta_seconds, 0.1);
    }
    if view_space_yaw.abs() > limit + tolerance {
        view_space_yaw = view_space_yaw.clamp(-limit, limit);
    } else {
        limit = view_space_yaw.abs().max(minimum_limit);
    }
    (wrap_yaw_radians(view_yaw - view_space_yaw), limit.to_degrees())
}

fn counterclockwise_yaw_delta(delta: f32) -> f32 {
    -crate::transitions::remap_counterclockwise_degrees(-wrap_yaw_radians(delta).to_degrees()).to_radians()
}

fn damper_yaw(current: f32, target: f32, delta_seconds: f32, half_life: f32) -> f32 {
    let delta = wrap_yaw_radians(target - current);
    if delta.abs() <= 1e-4_f32.to_radians() {
        return target;
    }
    let alpha = 1.0 - crate::animation::inv_exp_approx(std::f32::consts::LN_2 / (half_life + 1e-8) * delta_seconds);
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
        include!("../data/locomotion_thresholds.rs");
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
    let crouching = crate::params::GaitHalfLives::normal();
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
    if rotation_mode == RotationMode::Aiming {
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
    gait_amounts: Query<&crate::gait::AlsGaitAmount>,
    first_person: Query<(), With<crate::input_request::AlsFirstPersonView>>,
    actions: Query<&crate::state::AlsNotifyLocomotionAction>,
    collision_owners: Query<&crate::character::AlsCollisionLayerOwnership>,
    stances: Query<&Stance>,
    mut q: Query<
        (
            Entity,
            &mut Transform,
            Option<&mut Rotation>,
            &Gait,
            &LocomotionState,
            &RotationMode,
            (&AlsViewYaw, Option<&crate::view::AlsPlayerViewYaw>),
            &AlsMovementParamsHandle,
            (&LinearVelocity, Option<&crate::dynamics::AlsMovementBaseDynamicsState>),
            Option<&AlsInputOverride>,
            &AlsInputCommand,
            Option<&AlsRotationAnimationState>,
            Option<&mut AlsRotationResponseState>,
            Option<&crate::roll::AlsRollState>,
            &mut AlsAimingYawConstraint,
        ),
        With<AlsCharacter>,
    >,
) {
    let delta_seconds = time.delta_secs();
    for (
        entity,
        mut transform,
        rotation,
        gait,
        locomotion,
        rotation_mode,
        (view_yaw, player_view),
        params_handle,
        (velocity, base),
        input_override,
        input,
        animation,
        current_runtime,
        roll,
        mut constraint,
    ) in &mut q
    {
        if blocked.contains(entity)
            || !matches!(*rotation_mode, RotationMode::VelocityDirection | RotationMode::ViewDirection | RotationMode::Aiming)
            || collision_owners.get(entity).is_ok_and(|owner| owner.mantle)
            || roll.is_some_and(|state| state.active)
            || !matches!(*locomotion, LocomotionState::Grounded | LocomotionState::Air)
            || actions.get(entity).is_ok_and(|action| *action != crate::state::AlsNotifyLocomotionAction::None)
        {
            continue;
        }
        let Some(params) = params_assets.get(params_handle.id()) else {
            continue;
        };
        let current_yaw = world_forward_to_yaw(transform.rotation * Vec3::NEG_Z);
        let mut runtime = current_runtime.as_deref().copied().unwrap_or_default();
        let view_yaw_speed = if runtime.initialized && delta_seconds > f32::EPSILON {
            wrap_yaw_radians(view_yaw.0 - runtime.previous_view_yaw).abs().to_degrees() / delta_seconds
        } else {
            0.0
        };
        let effective = effective_input(input, input_override);
        let has_input = effective.move_dir.length_squared() > f32::EPSILON;
        let input_yaw = (effective.move_dir.length_squared() > f32::EPSILON).then(|| {
            world_forward_to_yaw(view_relative_move_direction(effective.move_dir, player_view.map_or(view_yaw.0, |view| view.0)))
        });
        let velocity = LinearVelocity(crate::dynamics::locomotion_velocity(velocity.0, *locomotion, base));
        let horizontal_speed = Vec2::new(velocity.x, velocity.z).length();
        let (has_velocity, moving) = locomotion_movement_flags(has_input, horizontal_speed);
        let velocity_yaw = has_velocity.then(|| world_forward_to_yaw(velocity.0));
        let animation = animation.copied().unwrap_or_default();
        if runtime.initialized
            && (runtime.previous_locomotion == LocomotionState::Air
                && *locomotion == LocomotionState::Grounded
                || runtime
                    .previous_rotation_mode
                    .is_some_and(|previous| previous != *rotation_mode))
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
            RotationMode::VelocityDirection => velocity_yaw.unwrap_or(
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
        let next_rotation = Quat::from_rotation_y(next_yaw);
        transform.rotation = next_rotation;
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
    use crate::character::{
        AlsCharacterBundle, AlsCharacterPlugin, als_character_capsule,
    };
    use crate::transition::AlsStateTransitionPlugin;
    use avian3d::prelude::{GravityScale, PhysicsPlugins, Position, RigidBody};
    use bevy::asset::{AssetEvent, AssetPlugin};
    use bevy::transform::TransformPlugin;
    use bevy_tnua::TnuaControllerPlugin;
    use bevy_tnua_avian3d::TnuaAvian3dPlugin;

    use super::*;

    #[test]
    fn custom_mode_preserves_actor_rotation_and_physical_stance() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(16));
        app.insert_resource(time)
            .init_resource::<Assets<AlsMovementParams>>()
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
        app.world_mut().entity_mut(actor).insert((LocomotionState::Grounded, crate::state::AlsNotifyLocomotionAction::Mantling));
        app.update();
        assert_eq!(*app.world().get::<Transform>(actor).unwrap(), transform);
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
            .add_systems(Update, rotation_response_system);
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
            .add(AlsMovementParams::default());
        let alpha = 1.0 - crate::animation::inv_exp_approx(std::f32::consts::LN_2 / (0.1 + 1e-8) * 0.016);
        for stance in [Stance::Standing, Stance::Crouching] {
            for (input, speed, moving) in [(false, 0.1, false), (false, 0.5, false),
                (false, 0.5001, true), (true, 0.009, false), (true, 0.01, true), (true, 0.05, true)] {
                let character = app.world_mut().spawn((
                    AlsCharacter, stance, Transform::IDENTITY, Gait::Walking,
                    LocomotionState::Grounded, RotationMode::Aiming, AlsViewYaw(30.0_f32.to_radians()),
                    AlsMovementParamsHandle(params.clone()), LinearVelocity(Vec3::NEG_Z * speed),
                    AlsInputCommand { move_dir: if input { Vec2::Y } else { Vec2::ZERO }, ..default() },
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
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(16));
        app.insert_resource(time)
            .init_resource::<Assets<AlsMovementParams>>()
            .add_plugins(crate::transition::AlsStateTransitionPlugin)
            .add_message::<crate::aim::AimStateChanged>()
            .add_systems(Update, (crate::input_request::input_state_request_system, crate::aim::aim_rotation_mode_system)
                .chain().before(crate::transition::state_transition_system))
            .add_systems(Update, rotation_response_system.after(crate::transition::state_transition_system));
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
        let characters = [Stance::Standing, Stance::Crouching].map(|stance| app.world_mut().spawn((
            AlsCharacter, stance, Transform::IDENTITY, Rotation::IDENTITY, Gait::Running,
            LocomotionState::Grounded, RotationMode::ViewDirection, AlsViewYaw(0.0),
            AlsMovementParamsHandle(params.clone()), LinearVelocity::ZERO, AlsMaxAllowedGait(Gait::Running),
            crate::aim::AlsLastFreeRotationMode(RotationMode::ViewDirection),
            OverlayState::Default, AlsStateRequest::default(),
            AlsInputCommand { aim_held: true, ..default() },
        )).id());
        app.update();
        for character in characters {
            assert_eq!(*app.world().get::<RotationMode>(character).unwrap(), RotationMode::Aiming);
            app.world_mut().entity_mut(character).insert((
                AlsInputCommand { aim_held: true, move_dir: Vec2::Y, ..default() },
                LinearVelocity(Vec3::NEG_Z * 1.75), AlsViewYaw(30.0_f32.to_radians()),
            ));
        }
        app.update();
        let observed = characters.map(|character| (
            world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward()),
            app.world().get::<AlsRotationResponseState>(character).unwrap().target_yaw,
        ));
        let expected_yaw = 16.0_f32.to_radians() * (1.0 - crate::animation::inv_exp_approx(std::f32::consts::LN_2 / (0.1 + 1e-8) * 0.016));
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
    fn actual_velocity_snapshot_retains_full_velocity_and_horizontal_speed() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(FixedPostUpdate, snapshot_actual_velocity_system);
        let entity = app
            .world_mut()
            .spawn((
                AlsCharacter,
                LinearVelocity(Vec3::new(3.0, 7.0, -4.0)),
                LocomotionState::Air,
                AlsActualVelocity::default(),
            ))
            .id();

        app.world_mut().run_schedule(FixedPostUpdate);

        let motion = *app.world().get::<AlsActualVelocity>(entity).unwrap();
        assert_eq!(motion.world_velocity, Vec3::new(3.0, 7.0, -4.0));
        assert!((motion.horizontal_speed - 5.0).abs() < 1e-6);

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
    }

    #[test]
    fn rotation_response_uses_view_yaw_degrees_per_second() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(100));
        app.insert_resource(time)
            .init_resource::<Assets<AlsMovementParams>>()
            .add_systems(Update, rotation_response_system);
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
            .add(AlsMovementParams::default());
        let character = app.world_mut().spawn((
            AlsCharacter, Stance::Standing, Transform::IDENTITY, Gait::Running,
            LocomotionState::Grounded, RotationMode::ViewDirection,
            AlsViewYaw(90.0_f32.to_radians()), AlsMovementParamsHandle(params),
            LinearVelocity(Vec3::NEG_Z * 1.75),
            AlsInputCommand { move_dir: Vec2::Y, ..default() },
            AlsRotationResponseState { initialized: true, previous_view_yaw: 0.0,
                previous_rotation_mode: Some(RotationMode::ViewDirection), ..default() },
        )).id();
        app.update();
        let yaw = world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward());
        let expected = 50.0_f32.to_radians()
            * (1.0 - crate::animation::inv_exp_approx(std::f32::consts::LN_2 / (0.05 + 1e-8) * 0.1));
        assert!((yaw - expected).abs() < 1e-5, "yaw={yaw} expected={expected}");
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
                .add_systems(Update, rotation_response_system);
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
                .add(AlsMovementParams::default());
            let view = view_degrees.to_radians();
            let character = app.world_mut().spawn((
                AlsCharacter, crate::input_request::AlsFirstPersonView, Stance::Standing,
                Transform::IDENTITY, Gait::Running, locomotion, mode, AlsViewYaw(view),
                AlsMovementParamsHandle(params), LinearVelocity(velocity),
                AlsInputCommand { move_dir: input, ..default() },
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
                .add_systems(Update, rotation_response_system);
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
                .add(AlsMovementParams::default());
            let view = 90.0_f32.to_radians();
            let character = app.world_mut().spawn((
                AlsCharacter, stance, Transform::IDENTITY, Gait::Running,
                crate::gait::AlsGaitAmount(amount), LocomotionState::Grounded,
                mode, AlsViewYaw(view), AlsMovementParamsHandle(params),
                LinearVelocity(velocity), AlsInputCommand { move_dir: Vec2::Y, ..default() },
                AlsRotationResponseState { initialized: true, previous_view_yaw: view,
                    previous_rotation_mode: Some(mode), ..default() },
            )).id();
            app.update();
            let yaw = world_forward_to_yaw(*app.world().get::<Transform>(character).unwrap().forward());
            let expected = target_degrees.to_radians()
                * (1.0 - crate::animation::inv_exp_approx(std::f32::consts::LN_2 / (half_life + 1e-8) * 0.1));
            assert!((yaw - expected).abs() < 1e-5, "mode={mode:?} stance={stance:?} yaw={yaw} expected={expected}");
        }
    }

    #[test]
    fn landing_residual_speed_keeps_rotation_blocked_until_new_input() {
        for mode in [RotationMode::VelocityDirection, RotationMode::ViewDirection] {
            let mut app = App::new();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_millis(100));
            app.insert_resource(time)
                .init_resource::<Assets<AlsMovementParams>>()
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
                AlsInputCommand { move_dir: Vec2::Y, ..default() }));
            app.update();
            assert!(!app.world().get::<AlsRotationResponseState>(character).unwrap()
                .rotation_towards_last_input_direction_blocked);
        }
    }

    #[test]
    #[ignore = "BF124.A07 precollision desired velocity yaw; enable after implementation"]
    fn velocity_direction_uses_the_precollision_motor_heading() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(100));
        app.insert_resource(time)
            .init_resource::<Assets<AlsMovementParams>>()
            .add_systems(Update, rotation_response_system);
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>()
            .add(AlsMovementParams::default());
        let sideways = Vec3::X * 3.75;
        let intended = Vec3::NEG_Z * 3.75;
        let character = app.world_mut().spawn((
            AlsCharacter, Stance::Standing, Transform::IDENTITY, Gait::Running,
            LocomotionState::Grounded, RotationMode::VelocityDirection, AlsViewYaw(0.0),
            AlsMovementParamsHandle(params), LinearVelocity(sideways),
            AlsInputCommand { move_dir: Vec2::Y, ..default() },
            crate::dynamics::AlsMovementBaseDynamicsState {
                world_velocity: sideways, motor_boost: intended - sideways, ..default()
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
        );
        let restored_center = capsule_center_for_stance(
            &params,
            crouching_center,
            Stance::Crouching,
            Stance::Standing,
        );
        let standing_base = standing_center.y - params.crouch.standing_capsule_half_height;
        let crouching_base = crouching_center.y - params.crouch.crouched_capsule_half_height;
        assert!(standing_base.abs() < 1e-6);
        assert!((standing_base - crouching_base).abs() < 1e-6);
        assert!(restored_center.distance(standing_center) < 1e-6);
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
        input.jump_pressed = false;
        input.jump_held = false;
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
    pub mantle_probe: Option<crate::mantle::MantleProbeTrace>,
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

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsActualVelocitySet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsRotationResponseSet;

impl Plugin for AlsMotionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<StateTransitionEvent>()
            .add_plugins((
                crate::view::AlsViewInputPlugin,
                crate::gait::AlsGaitPlugin,
                crate::dynamics::AlsGroundedDynamicsPlugin,
            ))
            .add_systems(
                Update,
                (
                    crate::gait::refresh_gait_system
                        .after(crate::gait::gait_request_system).after(crate::view::AlsViewInputSet),
                    tnua_config_sync_system
                        .after(crate::gait::refresh_gait_system)
                        .after(crate::transition::state_transition_system)
                        .after(capsule_reshape_system)
                        .before(TnuaPipelineSystems::Sensors),
                    locomotion_drive_system
                        .after(crate::view::AlsViewInputSet)
                        .before(crate::transition::state_transition_system),
                    crouch_controller_transaction_system
                        .after(locomotion_drive_system)
                        .after(capsule_reshape_system),
                    grounded_step_up_system.after(crate::dynamics::grounded_dynamics_motor_system)
                        .before(TnuaPipelineSystems::Motors),
                    (|| false).pipe(follow_movement_base_system)
                        .after(capsule_reshape_system).before(TnuaPipelineSystems::Sensors),
                    refine_ground_sensor_system.after(TnuaPipelineSystems::Sensors)
                        .before(TnuaPipelineSystems::Logic),
                    capsule_reshape_system.after(crate::transition::state_transition_system),
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
                ((|| true).pipe(follow_movement_base_system), locomotion_state_sync_system, snapshot_actual_velocity_system)
                    .chain().in_set(AlsActualVelocitySet),
            )
            .add_systems(
                PostUpdate,
                rotation_response_system.in_set(AlsRotationResponseSet),
            );
    }
}

fn snapshot_actual_velocity_system(
    mut characters: Query<(&mut LinearVelocity, &mut AlsActualVelocity, Option<&Position>, &AlsCmcAdapterRuntime,
        &LocomotionState, Option<&mut crate::dynamics::AlsMovementBaseDynamicsState>), With<AlsCharacter>>,
) {
    for (mut velocity, mut actual, position, runtime, locomotion, mut base) in &mut characters {
        if let Some(start) = runtime.grounded_move_start
            && let Some(position) = position
            && runtime.simulation_delta_seconds > 0.0 {
            velocity.x = (position.x - start.x) / runtime.simulation_delta_seconds;
            velocity.z = (position.z - start.z) / runtime.simulation_delta_seconds;
        }
        if *locomotion != LocomotionState::Grounded
            && let Some(base) = base.as_deref_mut() {
            *base = default();
        }
        *actual = AlsActualVelocity::from_physics_velocity(
            crate::dynamics::locomotion_velocity(velocity.0, *locomotion, base.as_deref()));
    }
}

pub fn movement_sweep_travel(distance: f32, hit_distance: f32) -> f32 {
    let pullback = (distance * PULLBACK_PROPORTION).clamp(PULLBACK_MIN_DISTANCE, PULLBACK_MAX_DISTANCE)
        + distance * PULLBACK_TIME_BIAS;
    (hit_distance - pullback).max(0.0)
}

fn follow_movement_base_system(
    In(after_physics): In<bool>,
    time: Res<Time<avian3d::prelude::Physics>>,
    mut characters: Query<(Entity, &LocomotionState, &mut crate::dynamics::AlsMovementBase,
        Option<&AlsMantleState>, Option<&crate::dynamics::AlsMovementBaseDynamicsState>,
        &mut AlsCmcAdapterRuntime, &Collider, Has<avian3d::prelude::CustomPositionIntegration>), With<AlsCharacter>>,
    mut space: ParamSet<(bevy::transform::helper::TransformHelper, SpatialQuery,
        Query<(&mut Position, &mut Transform), With<AlsCharacter>>)>,
    owners: Query<&avian3d::prelude::ColliderOf>,
    bodies: Query<&RigidBody>,
    children: Query<&Children>,
) {
    for (entity, mode, mut base, mantle, dynamics, mut runtime, capsule, custom_position) in &mut characters {
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
        if *mode == LocomotionState::Grounded
            && bodies.get(owner).is_ok_and(|body| *body != RigidBody::Static) {
            let carried = if !after_physics || custom_position { Vec3::ZERO } else {
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
                let hit = space.p1().cast_shape(capsule, origin, Quat::IDENTITY, direction, &config, &filter);
                let applied = *direction * hit.map_or(distance, |hit| movement_sweep_travel(distance, hit.distance));
                let mut query = space.p2();
                let (mut position, mut transform) = query.get_mut(entity).unwrap();
                position.0 += applied;
                transform.translation = position.0;
                if let Some(start) = runtime.grounded_move_start.as_mut() { *start += applied; }
            }
        }
        base.entity = Some(target);
        base.location = current.translation();
        base.rotation = current.rotation();
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
    q: Query<(
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
        Option<&crate::dynamics::AlsMovementBaseDynamicsState>,
    )>,
    mut configs: ResMut<Assets<AlsControlSchemeConfig>>,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut sensors: Query<&mut TnuaProximitySensor>,
) {
    for (config, handle, max_allowed, stance, rotation_mode, view_yaw, velocity, locomotion, controller, sensor_entities, base) in &q {
        let Some(params) = params_assets.get(handle.id()) else {
            continue;
        };
        let Some(mut cfg) = configs.get_mut(&config.0) else {
            continue;
        };
        apply_als_control_scheme_config(
            &mut cfg,
            params,
            max_allowed.0,
            *stance,
            *rotation_mode,
            *stance == Stance::Crouching,
            crate::dynamics::locomotion_velocity(velocity.0, *locomotion, base),
            view_yaw.0,
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

type DriveQueryItem<'a> = (
    Entity,
    &'a mut TnuaController<AlsControlScheme>,
    (&'a AlsViewYaw, Option<&'a crate::view::AlsPlayerViewYaw>),
    &'a GlobalTransform,
    &'a LocomotionState,
    &'a LinearVelocity,
    Option<&'a mut crate::mantle::AlsMantleState>,
    Option<&'a AlsInputOverride>,
    &'a AlsInputCommand,
    &'a Stance,
    &'a OverlayState,
    &'a AlsMovementParamsHandle,
    &'a mut AlsStateRequest,
    Option<&'a mut AlsJumpPrev>,
    Option<&'a mut AlsCharacterDebug>,
);

#[allow(clippy::type_complexity)]
fn locomotion_drive_system(
    mut commands: Commands,
    spatial: SpatialQuery,
    velocities: Query<(&RigidBody, &LinearVelocity, Option<&crate::dynamics::AlsComponentVelocity>), Without<AlsCharacter>>,
    target_permissions: Query<Option<&AlsMantleTargetPermission>, Without<AlsCharacter>>,
    target_transforms: Query<&GlobalTransform, Without<AlsCharacter>>,
    target_bodies: Query<&RigidBody, Without<AlsCharacter>>,
    target_colliders: Query<&avian3d::prelude::ColliderOf>,
    mantle_controls: Query<Option<&AlsMantleControlSettings>, With<AlsCharacter>>,
    mantle_start_time_settings: Query<Option<&AlsMantleStartTimeSettings>, With<AlsCharacter>>,
    params_assets: Res<Assets<AlsMovementParams>>,
    rolls: Query<&crate::roll::AlsRollState, With<AlsCharacter>>,
    actions: Query<&crate::state::AlsNotifyLocomotionAction>,
    blocked: Query<(), With<AlsMovementInputBlocked>>,
    mut q: Query<DriveQueryItem, With<AlsCharacter>>,
) {
    for (
        entity,
        mut controller,
        (view_yaw, player_view),
        gtf,
        loco,
        velocity,
        mut mantle,
        override_cmd,
        cmd,
        stance,
        overlay,
        params_handle,
        mut request,
        jump_prev,
        character_debug,
    ) in &mut q
    {
        let roll = rolls.get(entity).ok();
        controller.initiate_action_feeding();
        let ragdolling = actions.get(entity).ok() == Some(&crate::state::AlsNotifyLocomotionAction::Ragdolling);
        let input_blocked = blocked.contains(entity) && !ragdolling;
        let effective =
            movement_input_for_action(*effective_input(cmd, override_cmd), input_blocked);

        let move_len = effective.move_dir.length().min(1.0);
        let world_dir = if move_len > 1e-4 {
            view_relative_move_direction(effective.move_dir, player_view.map_or(view_yaw.0, |view| view.0))
        } else {
            Vec3::ZERO
        };

        let desired_forward = roll
            .filter(|state| state.active)
            .and_then(|state| Dir3::new(yaw_to_world_forward(state.direction_yaw)).ok())
            .or_else(|| Dir3::new(gtf.forward().as_vec3()).ok());

        controller.basis = TnuaBuiltinWalk {
            desired_motion: world_dir * move_len,
            desired_forward,
        };

        if *stance == Stance::Crouching {
            controller.action(AlsControlScheme::Crouch(TnuaBuiltinCrouch));
        }

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
            |action| *action != crate::state::AlsNotifyLocomotionAction::None,
        );
        let mantle_ready = mantle.as_deref().is_some_and(|m| !m.active)
            && mantle_start_allowed(mantle_control, action_active);
        let auto_mantle = mantle.as_deref().is_some_and(|m| !m.active)
            && auto_mantle_start_allowed(mantle_control, action_active, *loco);
        let should_probe_mantle = mantle_ready && (jump_edge || auto_mantle);
        let detection = if should_probe_mantle {
            let origin = gtf.translation();
            let fwd = gtf.forward().as_vec3();
            let (base_half_height, base_radius) = params_assets
                .get(params_handle.id())
                .map(|params| {
                    (
                        if *stance == Stance::Crouching { params.crouch.crouched_capsule_half_height } else { params.crouch.standing_capsule_half_height },
                        params.crouch.standing_capsule_radius,
                    )
                })
                .unwrap_or((
                    ALS_CHARACTER_CAPSULE_HALF_HEIGHT,
                    crate::character::ALS_CHARACTER_CAPSULE_RADIUS,
                ));
            let (scale, _, _) = gtf.to_scale_rotation_translation();
            let (capsule_half_height, capsule_radius, vertical_scale) =
                scaled_mantle_capsule_dimensions(base_half_height, base_radius, scale);
            let mut detection = detect_mantle(
                &spatial,
                entity,
                MantleDetectionInput {
                    origin,
                    actor_yaw: yaw_from_direction(fwd),
                    velocity: velocity.0,
                    input_direction: world_dir,
                    locomotion: *loco,
                    stance: *stance,
                    overlay: *overlay,
                    capsule_half_height,
                    capsule_radius,
                    capsule_scale: vertical_scale,
                },
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
        let intent = detection.and_then(|detection| {
            detection.intent.and_then(|intent| {
                target_transforms
                    .get(intent.target_entity)
                    .ok()
                    .map(|transform| {
                        let body = target_colliders.get(intent.target_entity)
                            .map_or(intent.target_entity, |collider| collider.body);
                        (
                            intent,
                            AlsMantleTargetBase {
                                entity: intent.target_entity,
                                translation: transform.translation(),
                                rotation: transform.rotation(),
                                scale: transform.scale(),
                                relative_location: target_bodies.get(body).is_ok_and(|body| *body != RigidBody::Static),
                            },
                        )
                    })
            })
        });
        if jump_edge {
            match jump_chain(ragdolling, *stance, intent.is_some()) {
                JumpChain::ExitRagdoll => {
                    request.ragdoll_exit = true;
                }
                JumpChain::Mantle => {
                    if let (Some(m), Some((i, target_base))) = (mantle.as_deref_mut(), intent) {
                        let origin = gtf.translation();
                        let yaw_start = yaw_from_direction(gtf.forward().as_vec3());
                        let start_time = configured_mantle_start_time(
                            mantle_start_time_settings,
                            i.kind,
                            i.mantling_height,
                        );
                        m.begin_with_target_base_and_start_time(
                            i.kind,
                            origin,
                            i.target,
                            start_time,
                            yaw_start,
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
                    if !action_active && *loco == LocomotionState::Grounded {
                        controller.action(AlsControlScheme::Jump(TnuaBuiltinJump { allow_in_air: true, ..default() }));
                    }
                }
            }
        } else if auto_mantle
            && let (Some(m), Some((i, target_base))) = (mantle.as_deref_mut(), intent)
        {
            let origin = gtf.translation();
            let yaw_start = yaw_from_direction(gtf.forward().as_vec3());
            let start_time =
                configured_mantle_start_time(mantle_start_time_settings, i.kind, i.mantling_height);
            m.begin_with_target_base_and_start_time(
                i.kind,
                origin,
                i.target,
                start_time,
                yaw_start,
                i.yaw_target,
                target_base,
            );
            info!(
                "[MOTION] entity={entity:?} auto mantle begin kind={:?} height={:.2} start_time={start_time:.3}",
                i.kind, i.mantling_height,
            );
        } else if effective.jump_held && controller.action_flow_status().ongoing().is_some() {
            controller.prolong_action();
        }
    }
}

type ReshapeQueryItem<'a> = (
    Entity,
    &'a mut Stance,
    &'a AlsDesiredStance,
    Option<&'a AlsPhysicalStanceOverride>,
    &'a LocomotionState,
    Option<&'a AlsMantleState>,
    Option<&'a crate::state::AlsNotifyLocomotionAction>,
    &'a Transform,
    &'a AlsMovementParamsHandle,
    Has<CapsuleReshapeDeferred>,
    &'a mut AlsCrouchTransaction,
);

fn capsule_reshape_system(
    mut commands: Commands,
    spatial_query: SpatialQuery,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut transitions: MessageWriter<StateTransitionEvent>,
    mut q: Query<ReshapeQueryItem, With<AlsCharacter>>,
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
        handle,
        already_deferred,
        mut transaction,
    ) in &mut q
    {
        let Some(params) = params_assets.get(handle.id()) else {
            continue;
        };
        let (half_height, radius) = capsule_dimensions(params, *stance);
        let action_active = action.map_or_else(
            || mantle.is_some_and(|mantle| mantle.active),
            |action| *action != crate::state::AlsNotifyLocomotionAction::None,
        );
        transaction.synchronize(
            desired.0,
            *stance,
            half_height,
            radius,
            transform.translation,
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
        apply_capsule_for_stance(
            &mut commands,
            &spatial_query,
            params,
            entity,
            target,
            &mut stance,
            &mut transaction,
            transform.translation,
            transform.rotation,
            transform,
            already_deferred,
            &mut transitions,
        );
    }
}

fn apply_capsule_for_stance(
    commands: &mut Commands,
    spatial_query: &SpatialQuery,
    params: &AlsMovementParams,
    entity: Entity,
    target: Stance,
    stance: &mut Stance,
    transaction: &mut AlsCrouchTransaction,
    center: Vec3,
    rotation: Quat,
    transform: &Transform,
    already_deferred: bool,
    transitions: &mut MessageWriter<StateTransitionEvent>,
) {
    let target_center = capsule_center_for_stance(params, center, *stance, target);
    match target {
        Stance::Crouching => {
            let (half_height, radius) = capsule_dimensions(params, target);
            commands
                .entity(entity)
                .remove::<CapsuleReshapeDeferred>()
                .insert(AlsCapsuleCrouched)
                .insert(capsule_for(params, target));
            apply_actual_stance(entity, stance, target, transitions);
            apply_capsule_center(commands, entity, transform, target_center);
            transaction.crouch_applied(half_height, radius, target_center);
        }
        Stance::Standing => {
            let filter = SpatialQueryFilter::from_excluded_entities([entity]);
            let standing_capsule = capsule_for(params, Stance::Standing);
            let blocked = !spatial_query
                .shape_intersections(&standing_capsule, target_center, rotation, &filter)
                .is_empty();
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
                .insert(standing_capsule);
            apply_actual_stance(entity, stance, target, transitions);
            let (half_height, radius) = capsule_dimensions(params, Stance::Standing);
            apply_capsule_center(commands, entity, transform, target_center);
            transaction.standing_applied(half_height, radius, target_center);
            if was_deferred {
                info!("[MOTION] entity={entity:?} clearance regained: standing capsule restored");
            }
        }
        _ => {}
    }
}

fn capsule_dimensions(params: &AlsMovementParams, stance: Stance) -> (f32, f32) {
    let half_height = if stance == Stance::Crouching { params.crouch.crouched_capsule_half_height } else { params.crouch.standing_capsule_half_height };
    (half_height, params.crouch.standing_capsule_radius)
}

fn capsule_center_for_stance(
    params: &AlsMovementParams,
    current_center: Vec3,
    current_stance: Stance,
    target_stance: Stance,
) -> Vec3 {
    let (current_half_height, _) = capsule_dimensions(params, current_stance);
    let (target_half_height, _) = capsule_dimensions(params, target_stance);
    current_center + Vec3::Y * (target_half_height - current_half_height)
}

fn apply_capsule_center(
    commands: &mut Commands,
    entity: Entity,
    transform: &Transform,
    center: Vec3,
) {
    let mut next_transform = transform.clone();
    next_transform.translation = center;
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
    mut transactions: Query<(&Stance, &mut AlsCrouchTransaction), With<AlsCharacter>>,
) {
    for (stance, mut transaction) in &mut transactions {
        transaction.controller_crouch_active = *stance == Stance::Crouching;
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

fn grounded_step_up_system(
    time: Res<Time>,
    mut commands: Commands,
    spatial_query: SpatialQuery,
    params_assets: Res<Assets<AlsMovementParams>>,
    blocked: Query<(), With<AlsMovementInputBlocked>>,
    mut q: Query<
        (
            Entity,
            &mut Transform,
            &Position,
            &Collider,
            &LinearVelocity,
            &mut TnuaMotor,
            &mut SpeculativeMargin,
            &mut AlsCmcAdapterRuntime,
            &AlsEffectiveGravity,
            &LocomotionState,
            &TnuaController<AlsControlScheme>,
            &AlsMovementParamsHandle,
            &Stance,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, mut transform, position, capsule, velocity, mut motor, mut margin, mut runtime, gravity, loco, controller, handle, stance) in &mut q {
        *margin = SpeculativeMargin::MAX;
        runtime.grounded_move_start = None;
        if *loco != LocomotionState::Grounded || blocked.contains(entity)
            || controller.action_flow_status().ongoing() == Some(AlsControlSchemeActionDiscriminant::Jump) {
            continue;
        }
        let Some(params) = params_assets.get(handle.id()) else {
            continue;
        };
        let half = if *stance == Stance::Crouching { params.crouch.crouched_capsule_half_height } else { params.crouch.standing_capsule_half_height } * transform.scale.y;
        let radius = params.crouch.standing_capsule_radius * transform.scale.x.min(transform.scale.z);
        let next_velocity = velocity.0 + motor.lin.boost + motor.lin.acceleration * time.delta_secs();
        let delta = Vec3::new(next_velocity.x, 0.0, next_velocity.z) * time.delta_secs();
        let Ok((direction, distance)) = Dir3::new_and_length(delta) else {
            continue;
        };
        let center = position.0;
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let mut forward_config = ShapeCastConfig {
            ignore_origin_penetration: true,
            ..ShapeCastConfig::from_max_distance(distance)
        };
        let clearance = ALS_CHARACTER_FLOOR_CLEARANCE + ALS_CHARACTER_TNUA_FLOAT_EPSILON;
        let mut raised = center;
        let mut remaining_delta = delta;
        let mut initial_floor_y = None;
        if let Some(hit) = spatial_query.cast_shape(capsule, center, Quat::IDENTITY, direction, &forward_config, &filter) {
            let advance = *direction * movement_sweep_travel(distance, hit.distance);
            raised += advance;
            remaining_delta -= advance;
            forward_config.max_distance = remaining_delta.length();
            runtime.grounded_move_start = Some(center);
            transform.translation = raised;
            commands.entity(entity).insert(Position(raised));
            let slide_normal = Vec3::new(hit.normal1.x, 0.0, hit.normal1.z).normalize_or_zero();
            let slide = remaining_delta - slide_normal * remaining_delta.dot(slide_normal);
            let remaining_velocity = slide / time.delta_secs();
            motor.lin.boost.x = remaining_velocity.x - velocity.x;
            motor.lin.boost.z = remaining_velocity.z - velocity.z;
            motor.lin.acceleration.x = 0.0;
            motor.lin.acceleration.z = 0.0;
            if hit.point1.y <= center.y - half - clearance || hit.point1.y > center.y + half - radius {
                continue;
            }
            let Some((_, impact_y, _)) = find_ground_floor(&spatial_query, center, half, radius, &filter) else { continue };
            initial_floor_y = Some(impact_y);
            let up_config = ShapeCastConfig {
                ignore_origin_penetration: true,
                ..ShapeCastConfig::from_max_distance(MAX_STEP_HEIGHT - clearance)
            };
            let lift = spatial_query.cast_shape(capsule, raised, Quat::IDENTITY, Dir3::Y, &up_config, &filter)
                .map_or(up_config.max_distance, |hit| hit.distance);
            raised.y += lift;
            if spatial_query.cast_shape(capsule, raised, Quat::IDENTITY, direction, &forward_config, &filter).is_some() {
                continue;
            }
        }
        let destination = raised + remaining_delta;
        let Some(floor) = walkable_floor_hit(&spatial_query, capsule, radius, destination,
            MAX_STEP_HEIGHT + clearance * 2.0, &filter) else {
            continue;
        };
        let next_y = destination.y - floor.distance + clearance;
        if (next_y - center.y).abs() > MAX_STEP_HEIGHT
            || initial_floor_y.is_some_and(|initial| floor.point1.y - initial > MAX_STEP_HEIGHT) {
            continue;
        }
        if next_y > center.y {
            transform.translation.y = next_y;
            commands.entity(entity).insert(Position(transform.translation));
        }
        if runtime.grounded_move_start.is_some() {
            let remaining_velocity = remaining_delta / time.delta_secs();
            motor.lin.boost.x = remaining_velocity.x - velocity.x;
            motor.lin.boost.z = remaining_velocity.z - velocity.z;
        }
        motor.lin.boost.y = -velocity.y;
        motor.lin.acceleration.y = -gravity.value.y;
        *margin = SpeculativeMargin::ZERO;
    }
}

fn walkable_floor_hit(spatial: &SpatialQuery, capsule: &Collider, radius: f32, origin: Vec3,
    distance: f32, filter: &SpatialQueryFilter) -> Option<ShapeHitData> {
    let hit = spatial.cast_shape(capsule, origin, Quat::IDENTITY, Dir3::NEG_Y,
        &ShapeCastConfig::from_max_distance(distance), filter)?;
    let reduced_radius = (radius - EDGE_REJECT_DISTANCE).max(SWEEP_EDGE_MIN_RADIUS);
    if Vec2::new(hit.point1.x - origin.x, hit.point1.z - origin.z).length_squared() >= reduced_radius * reduced_radius {
        return None;
    }
    let mut normal = hit.normal1;
    if normal.y < deg_to_rad(MAX_SLOPE_DEGREES).cos() {
        let ray_origin = hit.point1 + (Vec3::Y - Vec3::new(normal.x, 0.0, normal.z)) * EDGE_REJECT_DISTANCE;
        if let Some(surface) = spatial.cast_ray(ray_origin, Dir3::NEG_Y, EDGE_REJECT_DISTANCE * 2.0, false, filter)
            .filter(|surface| surface.entity == hit.entity) {
            normal = surface.normal;
        }
    }
    (normal.y >= deg_to_rad(MAX_SLOPE_DEGREES).cos()).then_some(hit)
}

pub fn find_ground_floor(spatial: &SpatialQuery, origin: Vec3, half: f32, radius: f32,
    filter: &SpatialQueryFilter) -> Option<(f32, f32, Entity)> {
    for (shrink_scale, radius_reduction) in [(FLOOR_SWEEP_SHRINK_FRACTION, 0.0),
        (FLOOR_RETRY_SHRINK_FRACTION, SWEEP_EDGE_MIN_RADIUS)] {
        let shrink_height = (half - radius) * shrink_scale;
        let sweep_radius = radius - radius_reduction;
        let capsule = als_character_capsule((half - shrink_height).max(sweep_radius), sweep_radius);
        if let Some(hit) = walkable_floor_hit(spatial, &capsule, sweep_radius, origin,
            MAX_STEP_HEIGHT + MAX_FLOOR_DISTANCE + shrink_height, filter) {
            return Some((origin.y - hit.distance + shrink_height, hit.point1.y, hit.entity));
        }
    }
    None
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
    spatial: SpatialQuery,
    params_assets: Res<Assets<AlsMovementParams>>,
    blocked: Query<(), With<AlsMovementInputBlocked>>,
    collisions: Collisions,
    mut transitions: MessageWriter<StateTransitionEvent>,
    transforms: bevy::transform::helper::TransformHelper,
    mut movement_bases: Query<(&mut crate::dynamics::AlsMovementBase, Option<&mut crate::dynamics::AlsMovementBaseDynamicsState>), With<AlsCharacter>>,
    mut q: Query<
        (
            Entity,
            &Transform,
            &Position,
            &mut LinearVelocity,
            &TnuaController<AlsControlScheme>,
            &mut LocomotionState,
            &AlsStateRequest,
            Has<AlsRagdollAirExit>,
            Option<&crate::state::AlsNotifyLocomotionAction>,
            &AlsMovementParamsHandle,
            &Stance,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, transform, position, mut velocity, controller, mut current, request, ragdoll_air_exit, action, handle, stance) in &mut q {
        if !matches!(*current, LocomotionState::Grounded | LocomotionState::Air) || ragdoll_air_exit
            || action == Some(&crate::state::AlsNotifyLocomotionAction::Ragdolling) {
            continue;
        }
        let Ok(basis) = controller.basis_access() else {
            continue;
        };
        let jumping = controller.action_flow_status().ongoing() == Some(AlsControlSchemeActionDiscriminant::Jump);
        let mut floor_supported = false;
        let mut floor_entity = None;
        if *current == LocomotionState::Grounded && !blocked.contains(entity) && !jumping
            && let Some(params) = params_assets.get(handle.id()) {
            let half = if *stance == Stance::Crouching { params.crouch.crouched_capsule_half_height } else { params.crouch.standing_capsule_half_height } * transform.scale.y;
            let radius = params.crouch.standing_capsule_radius * transform.scale.x.min(transform.scale.z);
            let filter = SpatialQueryFilter::from_excluded_entities([entity]);
            if let Some((floor_center_y, _, floor)) = find_ground_floor(&spatial, position.0, half, radius, &filter) {
                let floor_distance = position.y - floor_center_y;
                let next_y = if (MIN_FLOOR_DISTANCE..=MAX_FLOOR_DISTANCE).contains(&floor_distance) {
                    position.y
                } else {
                    floor_center_y + (MIN_FLOOR_DISTANCE + MAX_FLOOR_DISTANCE) * 0.5
                };
                if (next_y - position.y).abs() <= MAX_STEP_HEIGHT {
                    let mut next_transform = *transform;
                    next_transform.translation = Vec3::new(position.x, next_y, position.z);
                    commands.entity(entity).insert((Position(next_transform.translation), next_transform));
                    velocity.0.y = 0.0;
                    floor_supported = true;
                    floor_entity = Some(floor);
                }
            }
        }
        let airborne = TnuaBuiltinWalk::displacement(&basis).is_none()
            || jumping;
        let landed_on_contact = airborne && collisions.collisions_with(entity)
            .any(|contact| resolved_walkable_contact(contact, entity));
        let landed_on_capsule = airborne && !landed_on_contact && !floor_supported
            && collisions.collisions_with(entity).any(|contact| {
                contact.flags.contains(ContactPairFlags::GENERATE_CONSTRAINTS)
                    && contact.manifolds.iter().any(|manifold| {
                        let normal = if contact.collider1 == entity { -manifold.normal } else { manifold.normal };
                        normal.y > 0.0 && manifold.points.iter().any(|point| point.normal_impulse > 0.0)
                    })
            }) && params_assets.get(handle.id()).is_some_and(|params| {
                let half = if *stance == Stance::Crouching { params.crouch.crouched_capsule_half_height } else { params.crouch.standing_capsule_half_height } * transform.scale.y;
                let radius = params.crouch.standing_capsule_radius * transform.scale.x.min(transform.scale.z);
                find_ground_floor(&spatial, position.0, half, radius, &SpatialQueryFilter::from_excluded_entities([entity]))
                    .is_some_and(|(center, impact, _)| (position.y - center).abs() <= MAX_FLOOR_DISTANCE
                        && impact < position.y - half + radius)
            });
        let target = if airborne && !landed_on_contact && !floor_supported && !landed_on_capsule {
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
            *current = target;
        }
        if let Ok((mut base, dynamics)) = movement_bases.get_mut(entity) {
            if *current == LocomotionState::Grounded && floor_entity.is_none()
                && let Some(params) = params_assets.get(handle.id()) {
                let half = if *stance == Stance::Crouching { params.crouch.crouched_capsule_half_height } else { params.crouch.standing_capsule_half_height } * transform.scale.y;
                let radius = params.crouch.standing_capsule_radius * transform.scale.x.min(transform.scale.z);
                floor_entity = find_ground_floor(&spatial, position.0, half, radius,
                    &SpatialQueryFilter::from_excluded_entities([entity])).map(|(_, _, entity)| entity);
            }
            base.entity = if *current == LocomotionState::Grounded { floor_entity } else { None };
            if let Some(entity) = base.entity && let Ok(global) = transforms.compute_global_transform(entity) {
                base.location = global.translation();
                base.rotation = global.rotation();
            }
            if let Some(mut dynamics) = dynamics {
                dynamics.entity = base.entity;
            } else {
                commands.entity(entity).insert(crate::dynamics::AlsMovementBaseDynamicsState {
                    entity: base.entity, world_velocity: velocity.0, relative_velocity: velocity.0, ..default()
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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


    fn stair_motion_fixture(rise: f32, ceiling: bool, descending: bool, yaw: f32) -> (App, Entity, f32) {
        use crate::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use crate::transition::AlsStateTransitionPlugin;
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
            AlsStateTransitionPlugin, AlsCharacterPlugin, AlsMotionPlugin))
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
        app.world_mut().get_mut::<crate::gait::AlsDesiredGait>(character).unwrap().0 = Gait::Walking;
        *app.world_mut().resource_mut::<TimeUpdateStrategy>() = TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.0093024));
        for _ in 0..30 { app.update(); }
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y * 0.11500549;
        for _ in 0..240 { app.update(); }
        let position = app.world().get::<Position>(character).unwrap().0;
        let base = *app.world().get::<crate::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
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
                        app.world_mut().get_mut::<crate::gait::AlsDesiredGait>(character).unwrap().0 = gait;
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
                                let supported = app.world_mut().run_system_once(move |spatial: SpatialQuery, collisions: Collisions| {
                                    let floor = find_ground_floor(&spatial, position, 0.9, 0.3, &SpatialQueryFilter::from_excluded_entities([character]));
                                    let contact = collisions.collisions_with(character).any(|pair| pair.manifolds.iter().any(|manifold| {
                                        let normal = if pair.collider1 == character { -manifold.normal } else { manifold.normal };
                                        normal.y > 0.0 && manifold.points.iter().any(|point| point.normal_impulse > 0.0)
                                    }));
                                    contact && floor.is_some_and(|(center, _, _)| (position.y - center).abs() <= MAX_FLOOR_DISTANCE)
                                }).unwrap();
                                supported_air_frames = if supported { supported_air_frames + 1 } else { 0 };
                                if supported_air_frames >= 2 {
                                    let evidence = app.world_mut().run_system_once(move |spatial: SpatialQuery, collisions: Collisions| {
                                        let floor = find_ground_floor(&spatial, position, 0.9, 0.3, &SpatialQueryFilter::from_excluded_entities([character]));
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
            let base = app.world().get::<crate::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
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
                assert_eq!(app.world().get::<crate::dynamics::AlsMovementBase>(character).unwrap().entity, Some(collider));
                assert!(app.world().get::<Transform>(character).unwrap().rotation.angle_between(Quat::IDENTITY) < 1e-4);
                maximum = maximum.max((actual - rotation * offset).with_y(0.0).length());
            }
            if maximum > 1e-4 { failures.push((child, step, maximum)); }
        }
        assert!(failures.is_empty(), "Component movement was discarded without a velocity: {failures:?}");
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
        app.world_mut().run_system_once((|| false).pipe(follow_movement_base_system)).unwrap();
        let after = app.world().get::<Position>(character).unwrap().0 - target - Vec3::Y * ALS_CHARACTER_CAPSULE_HALF_HEIGHT;
        assert!((rotation.conjugate() * after).abs_diff_eq(before, 1e-5));
        assert_eq!(app.world().get::<Transform>(character).unwrap().rotation, Quat::IDENTITY);
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
        assert_eq!(app.world().get::<crate::dynamics::AlsMovementBase>(character).unwrap().entity, Some(replacement));
        let velocity = app.world().get::<AlsActualVelocity>(character).unwrap().world_velocity;
        assert!(velocity.with_y(0.0).length() < 1e-4, "Replacing the base changed the character velocity: {velocity:?}");
        app.world_mut().despawn(replacement);
        app.update();
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Air);
        assert!(app.world().get::<crate::dynamics::AlsMovementBase>(character).unwrap().entity.is_none());
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
            assert_eq!(app.world().get::<crate::dynamics::AlsMovementBaseDynamicsState>(character).unwrap().entity, None);
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
            let base = app.world().get::<crate::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
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
        app.world_mut().get_mut::<crate::gait::AlsDesiredGait>(character).unwrap().0 = Gait::Walking;
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
        assert!(app.world().get::<LinearVelocity>(character).unwrap().0.abs_diff_eq(Vec3::NEG_Z * 1.75, 1e-4));
    }

    #[test]
    fn movement_uses_player_view_without_overwriting_the_control_target() {
        let (mut app, character, _) = stair_motion_fixture(0.18, false, false, 0.0);
        let control = 60.0_f32.to_radians();
        let rendered = 30.0_f32.to_radians();
        app.world_mut().entity_mut(character).insert((AlsViewYaw(control), crate::view::AlsPlayerViewYaw(rendered)));
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
        app.update();
        let desired = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion;
        assert!(desired.abs_diff_eq(Vec3::new(-0.5, 0.0, -3.0_f32.sqrt() * 0.5), 1e-5), "Movement used the control target instead of the player view: {desired:?}");
        assert_eq!(app.world().get::<AlsViewYaw>(character).unwrap().0, control);
        app.world_mut().entity_mut(character).remove::<crate::view::AlsPlayerViewYaw>();
        app.update();
        let desired = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion;
        assert!(desired.abs_diff_eq(Vec3::new(-3.0_f32.sqrt() * 0.5, 0.0, -0.5), 1e-5));
        app.world_mut().entity_mut(character).insert((crate::input_request::AlsFirstPersonView, crate::view::AlsPlayerViewYaw(0.0)));
        app.world_mut().get_mut::<crate::gait::AlsDesiredGait>(character).unwrap().0 = Gait::Sprinting;
        app.update();
        assert_eq!(app.world().get::<crate::gait::AlsMaxAllowedGait>(character).unwrap().0, Gait::Running);
        app.world_mut().get_mut::<crate::view::AlsPlayerViewYaw>(character).unwrap().0 = control;
        app.update();
        assert_eq!(app.world().get::<crate::gait::AlsMaxAllowedGait>(character).unwrap().0, Gait::Sprinting);
    }

    #[test]
    fn mantle_targets_resolve_motion_and_permissions_from_compound_bodies() {
        use crate::mantle::AlsMantleTargetPermission;
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
                    app.world_mut().entity_mut(body).insert(crate::dynamics::AlsComponentVelocity(Vec3::X * speed));
                }
                let target = if child {
                    app.world_mut().spawn((ChildOf(body), Transform::from_translation(offset), Collider::cuboid(2.4, 0.8, 1.2))).id()
                } else {
                    app.world_mut().entity_mut(body).insert(Collider::cuboid(2.4, 0.8, 1.2));
                    body
                };
                app.world_mut().entity_mut(target).insert(avian3d::prelude::CollisionLayers::new(
                    [crate::character::AlsCollisionLayer::Default, crate::character::AlsCollisionLayer::Visibility], avian3d::prelude::LayerMask::ALL));
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
                    && (if kind == RigidBody::Dynamic { speed } else { reported.unwrap_or(0.0) }) <= crate::mantle::MANTLE_TARGET_SPEED_THRESHOLD;
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
        let base = app.world().get::<crate::dynamics::AlsMovementBase>(character).unwrap().entity;
        assert!(base.is_some(), "The current floor did not establish a movement base");
        assert_eq!(app.world().get::<crate::dynamics::AlsMovementBaseDynamicsState>(character).unwrap().entity, base);
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
                app.world_mut().get_mut::<crate::gait::AlsDesiredGait>(character).unwrap().0 = Gait::Walking;
                app.world_mut().get_mut::<AlsViewYaw>(character).unwrap().0 = std::f32::consts::PI;
                app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().move_dir = Vec2::Y;
                for frame in 0..600 {
                    app.update();
                    let position = app.world().get::<Position>(character).unwrap().0;
                    if position.z > 0.5 { break; }
                    if *app.world().get::<LocomotionState>(character).unwrap() == LocomotionState::Air {
                        let floor = app.world_mut().run_system_once(move |spatial: SpatialQuery| {
                            find_ground_floor(&spatial, position, 0.9, 0.3, &SpatialQueryFilter::from_excluded_entities([character]))
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
                            let dynamics = app.world().get::<crate::dynamics::AlsMovementBaseDynamicsState>(character).unwrap();
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
        use crate::character::{AlsCharacterBundle, AlsCharacterPlugin};
        use crate::transition::AlsStateTransitionPlugin;
        use avian3d::prelude::{PhysicsPlugins, RigidBody};
        use bevy::asset::AssetEvent;
        use bevy::time::TimeUpdateStrategy;
        use bevy_tnua::TnuaControllerPlugin;
        use bevy_tnua_avian3d::TnuaAvian3dPlugin;

        for fixed_delta in [Some(0.013_f32), None] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                PhysicsPlugins::new(PostUpdate), TnuaAvian3dPlugin::new(Update),
                TnuaControllerPlugin::<AlsControlScheme>::new(Update),
                AlsStateTransitionPlugin, AlsCharacterPlugin, AlsMotionPlugin))
                .init_resource::<Assets<Mesh>>()
                .add_message::<AssetEvent<Mesh>>()
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
                let impact = previous == LocomotionState::Air && previous_vertical < -1.0
                    && vertical.abs() < 0.1 && position.y <= ALS_CHARACTER_CAPSULE_HALF_HEIGHT + 0.001;
                if impact {
                    impact_seen = true;
                    assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(), LocomotionState::Grounded,
                        "ground impact must publish the mode in its physics frame={frame}");
                    let contact = app.world().resource::<avian3d::prelude::ContactGraph>()
                        .contact_pairs_with(character).find(|contact| resolved_walkable_contact(contact, character)).unwrap().clone();
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
                }
                if let Ok(basis) = app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().basis_access() {
                    let airborne = TnuaBuiltinWalk::displacement(&basis).is_none();
                    landed |= !airborne && seen[1];
                    assert!(!landed || !airborne, "landing rebounded at frame={frame}");
                    let expected = if airborne && !impact { LocomotionState::Air } else { LocomotionState::Grounded };
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
            app.world_mut().entity_mut(character).insert((LocomotionState::Grounded,
                crate::state::AlsNotifyLocomotionAction::Mantling, AlsInputCommand::default()));
            app.update();
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
            app.update();
            assert!(app.world().get::<TnuaController<AlsControlScheme>>(character).unwrap().action_flow_status().ongoing().is_none());
            app.world_mut().get_mut::<AlsStateRequest>(character).unwrap().stance = Some(Stance::Crouching);
            app.update();
            assert_eq!(app.world().get::<Stance>(character), Some(&Stance::Standing));
            assert_eq!(app.world().get::<AlsDesiredStance>(character).unwrap().0, Stance::Crouching);
            app.world_mut().entity_mut(character).insert(crate::state::AlsNotifyLocomotionAction::None);
            app.update();
            assert_eq!(app.world().get::<Stance>(character), Some(&Stance::Crouching));
            *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Custom;
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = false;
            app.update();
            app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().jump_pressed = true;
            app.update();
            assert_eq!(app.world().get::<AlsDesiredStance>(character).unwrap().0, Stance::Standing);
            assert_eq!(app.world().get::<Stance>(character), Some(&Stance::Crouching));
            app.world_mut().get_mut::<AlsStateRequest>(character).unwrap().ragdoll_start = true;
            app.update();
            assert_eq!(app.world().get::<LocomotionState>(character), Some(&LocomotionState::Custom));
            assert_eq!(app.world().get::<crate::state::AlsNotifyLocomotionAction>(character), Some(&crate::state::AlsNotifyLocomotionAction::Ragdolling));
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
        assert!(!blocked.jump_pressed && !blocked.roll_pressed);
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
    fn initial_control_scheme_matches_the_first_runtime_sync() {
        let params = AlsMovementParams::default();
        let config = initial_als_control_scheme_config(&params);
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
}
