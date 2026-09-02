use avian3d::prelude::{
    Collider, Gravity, GravityScale, LinearVelocity, PhysicsSystems, Position, Rotation,
    SpatialQuery, SpatialQueryFilter,
};
use bevy::asset::Assets;
use bevy::prelude::*;
use bevy_tnua::builtins::{
    TnuaBuiltinCrouch, TnuaBuiltinJump, TnuaBuiltinWalk, TnuaBuiltinWalkHeadroom,
};
use bevy_tnua::controller::TnuaConfig;
use bevy_tnua::{TnuaController, TnuaGravity, TnuaPipelineSystems, TnuaRigidBodyTracker};

use crate::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_FLOOR_CLEARANCE,
    ALS_CHARACTER_TNUA_FLOAT_EPSILON, AlsCharacter, AlsEffectiveGravity, AlsMovementParamsHandle,
    als_character_capsule,
};
use crate::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use crate::gait::AlsMaxAllowedGait;
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
    AlsDesiredStance, AlsStateRequest, Dimension, StateTransitionEvent, StateValue,
};

const MAX_SLOPE_DEGREES: f32 = 44.765_083;
const WALK_CLING_DISTANCE: f32 = ALS_CHARACTER_TNUA_FLOAT_EPSILON;
const AIR_CONTROL: f32 = 0.15;
const AIR_CONTROL_BOOST_MULTIPLIER: f32 = 2.0;
const AIR_CONTROL_BOOST_SPEED: f32 = 0.25;
const HEADROOM_SKIN: f32 = 0.01;
const MAX_STEP_HEIGHT: f32 = 0.45;
const STEP_ASSIST_COOLDOWN: f32 = 0.3;
const STALL_SPEED_EPSILON: f32 = 1.2;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsActualVelocity {
    pub world_velocity: Vec3,
    pub horizontal_speed: f32,
}

impl AlsActualVelocity {
    fn from_physics_velocity(world_velocity: Vec3) -> Self {
        let world_velocity = Vec3::new(world_velocity.x, 0.0, world_velocity.z);
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
    match stance {
        Stance::Standing => gait,
        Stance::Crouching => Gait::Walking,
    }
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
    params
        .gait_speed_thresholds(*stance, rotation_mode, velocity, view_yaw)
        .for_gait(*gait)
}

fn capsule_for(params: &AlsMovementParams, stance: Stance) -> Collider {
    let total_half = match stance {
        Stance::Standing => params.crouch.standing_capsule_half_height,
        Stance::Crouching => params.crouch.crouched_capsule_half_height,
    };
    let radius = params.crouch.standing_capsule_radius;
    als_character_capsule(total_half, radius)
}

fn turn_angvel(mode: RotationMode) -> f32 {
    match mode {
        RotationMode::VelocityDirection => deg_to_rad(800.0),
        RotationMode::ViewDirection => deg_to_rad(500.0),
        RotationMode::Aiming => deg_to_rad(1000.0),
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

fn step_assist_world_direction(move_dir: Vec2, view_yaw: f32) -> Vec3 {
    view_relative_move_direction(move_dir, view_yaw)
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
) -> (f32, f32) {
    let minimum_limit = minimum_limit_degrees.clamp(0.0, 180.0).to_radians();
    let mut limit = if reset {
        std::f32::consts::PI
    } else {
        active_limit_degrees
            .clamp(minimum_limit_degrees, 180.0)
            .to_radians()
    };
    let view_space_yaw = wrap_yaw_radians(view_yaw - actor_yaw);
    if view_space_yaw.abs() <= minimum_limit {
        limit = minimum_limit;
    } else if !reset && view_space_yaw.abs() <= limit {
        limit = view_space_yaw.abs().max(minimum_limit);
    }
    let constrained = view_space_yaw.clamp(-limit, limit);
    (wrap_yaw_radians(view_yaw - constrained), limit.to_degrees())
}

fn damper_yaw(current: f32, target: f32, delta_seconds: f32, half_life: f32) -> f32 {
    if half_life <= f32::EPSILON {
        return target;
    }
    let alpha = 1.0 - 2.0_f32.powf(-delta_seconds.max(0.0) / half_life);
    wrap_yaw_radians(current + wrap_yaw_radians(target - current) * alpha)
}

fn advance_yaw(current: f32, target: f32, speed_degrees: f32, delta_seconds: f32) -> f32 {
    let max_delta = speed_degrees.max(0.0).to_radians() * delta_seconds.max(0.0);
    wrap_yaw_radians(current + wrap_yaw_radians(target - current).clamp(-max_delta, max_delta))
}

fn moving_rotation_half_life(
    params: &AlsMovementParams,
    gait: Gait,
    rotation_mode: RotationMode,
    moving: bool,
    view_yaw_speed: f32,
) -> f32 {
    let base = match (rotation_mode, moving) {
        (RotationMode::VelocityDirection, true) => params
            .rotation
            .grounded_moving_velocity_direction
            .interpolation_half_lives
            .half_life(gait),
        (RotationMode::ViewDirection, true) => params
            .rotation
            .grounded_moving_view_direction
            .interpolation_half_lives
            .half_life(gait),
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
        (RotationMode::ViewDirection, false) => params
            .rotation
            .grounded_stationary_view_direction
            .interpolation_half_lives
            .half_life(gait),
    };
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
    mut q: Query<
        (
            Entity,
            &mut Transform,
            Option<&mut Rotation>,
            &Gait,
            &LocomotionState,
            &RotationMode,
            &AlsViewYaw,
            &AlsMovementParamsHandle,
            &LinearVelocity,
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
        view_yaw,
        params_handle,
        velocity,
        input_override,
        input,
        animation,
        current_runtime,
        roll,
        mut constraint,
    ) in &mut q
    {
        if roll.is_some_and(|state| state.active) || *locomotion == LocomotionState::Ragdolling {
            continue;
        }
        let Some(params) = params_assets.get(params_handle.id()) else {
            continue;
        };
        let current_yaw = world_forward_to_yaw(transform.rotation * Vec3::NEG_Z);
        let mut runtime = current_runtime.as_deref().copied().unwrap_or_default();
        let view_yaw_speed = if runtime.initialized && delta_seconds > f32::EPSILON {
            wrap_yaw_radians(view_yaw.0 - runtime.previous_view_yaw).abs() / delta_seconds
        } else {
            0.0
        };
        let effective = effective_input(input, input_override);
        let has_input = effective.move_dir.length_squared() > f32::EPSILON;
        let input_yaw = (effective.move_dir.length_squared() > f32::EPSILON).then(|| {
            world_forward_to_yaw(view_relative_move_direction(effective.move_dir, view_yaw.0))
        });
        let horizontal_speed = Vec2::new(velocity.x, velocity.z).length();
        let moving = horizontal_speed > 0.05;
        let velocity_yaw = moving.then(|| world_forward_to_yaw(velocity.0));
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
        if moving || has_input {
            runtime.rotation_towards_last_input_direction_blocked = false;
        }
        let mut target_yaw = match *rotation_mode {
            RotationMode::VelocityDirection if moving => velocity_yaw.unwrap_or(current_yaw),
            RotationMode::ViewDirection if moving && *gait == Gait::Sprinting => {
                velocity_yaw.unwrap_or(current_yaw)
            }
            RotationMode::ViewDirection
                if stationary_view_direction_holds_rotation(
                    moving,
                    has_input,
                    runtime.rotation_towards_last_input_direction_blocked,
                    *rotation_mode,
                ) =>
            {
                current_yaw
            }
            RotationMode::ViewDirection => {
                wrap_yaw_radians(view_yaw.0 + animation.yaw_offset_degrees.to_radians())
            }
            RotationMode::Aiming => view_yaw.0,
            _ => current_yaw,
        };
        let aiming = *rotation_mode == RotationMode::Aiming;
        if aiming {
            let (constrained_yaw, active_limit_degrees) = constrained_aiming_yaw(
                view_yaw.0,
                current_yaw,
                params.cmc.aiming_yaw_angle_limit_degrees,
                constraint.active_limit_degrees,
                !constraint.applied_last_frame,
            );
            target_yaw = constrained_yaw;
            constraint.active_limit_degrees = active_limit_degrees;
            constraint.applied_last_frame = true;
        } else {
            constraint.active_limit_degrees = 180.0;
            constraint.applied_last_frame = false;
        }
        let curve_driven = !moving && !aiming && animation.yaw_speed_degrees.abs() > f32::EPSILON;
        if curve_driven {
            if !runtime.initialized {
                runtime.smooth_target_yaw = current_yaw;
            }
            runtime.smooth_target_yaw = wrap_yaw_radians(
                runtime.smooth_target_yaw
                    + animation.yaw_speed_degrees.to_radians() * delta_seconds,
            );
        } else {
            if !runtime.initialized {
                runtime.smooth_target_yaw = current_yaw;
            }
            runtime.smooth_target_yaw = advance_yaw(
                runtime.smooth_target_yaw,
                target_yaw,
                target_yaw_speed(params, *rotation_mode),
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
            moving_rotation_half_life(params, *gait, *rotation_mode, moving, view_yaw_speed)
        };
        let next_yaw = if curve_driven {
            runtime.smooth_target_yaw
        } else {
            damper_yaw(
                current_yaw,
                runtime.smooth_target_yaw,
                delta_seconds,
                half_life,
            )
        };
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
        ALS_CHARACTER_CAPSULE_RADIUS, AlsCharacterBundle, AlsCharacterPlugin, als_character_capsule,
    };
    use crate::transition::AlsStateTransitionPlugin;
    use avian3d::prelude::{GravityScale, PhysicsPlugins, Position, RigidBody};
    use bevy::asset::{AssetEvent, AssetPlugin};
    use bevy::transform::TransformPlugin;
    use bevy_tnua::{TnuaController, TnuaControllerPlugin};
    use bevy_tnua_avian3d::TnuaAvian3dPlugin;

    use super::*;

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
    fn actual_velocity_snapshot_uses_horizontal_physics_velocity() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(FixedPostUpdate, snapshot_actual_velocity_system);
        let entity = app
            .world_mut()
            .spawn((
                AlsCharacter,
                LinearVelocity(Vec3::new(3.0, 7.0, -4.0)),
                AlsActualVelocity::default(),
            ))
            .id();

        app.world_mut().run_schedule(FixedPostUpdate);

        let motion = *app.world().get::<AlsActualVelocity>(entity).unwrap();
        assert_eq!(motion.world_velocity, Vec3::new(3.0, 0.0, -4.0));
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
    struct StepAssistPhysicsCase {
        origin: Vec3,
        expected: Vec3,
    }

    #[derive(Resource)]
    struct StepAssistPhysicsProbe {
        ground_distance: f32,
        step_distance: f32,
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

    fn record_step_assist_physics_probe(
        spatial_query: SpatialQuery,
        cases: Query<(Entity, &StepAssistPhysicsCase)>,
        mut probe: ResMut<StepAssistPhysicsProbe>,
    ) {
        let Ok((entity, case)) = cases.single() else {
            return;
        };
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        probe.ground_distance = spatial_query
            .cast_ray(case.origin, Dir3::NEG_Y, 2.0, false, &filter)
            .map(|hit| hit.distance)
            .unwrap_or(f32::NAN);
        let probe_origin = case.origin + case.expected * 0.45 + Vec3::Y * MAX_STEP_HEIGHT;
        probe.step_distance = spatial_query
            .cast_ray(probe_origin, Dir3::NEG_Y, 2.0, false, &filter)
            .map(|hit| hit.distance)
            .unwrap_or(f32::NAN);
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
            constrained_aiming_yaw(180.0_f32.to_radians(), 0.0, 70.0, 70.0, true);
        assert!(wrap_yaw_radians(first_yaw).abs() < 1e-6);
        assert!((first_limit - 180.0).abs() < 1e-6);

        let (limited_yaw, limited_limit) =
            constrained_aiming_yaw(180.0_f32.to_radians(), 0.0, 70.0, 70.0, false);
        assert!((limited_yaw.abs() - 110.0_f32.to_radians()).abs() < 1e-6);
        assert!((limited_limit - 70.0).abs() < 1e-6);

        let (near_yaw, near_limit) =
            constrained_aiming_yaw(20.0_f32.to_radians(), 0.0, 70.0, 180.0, false);
        assert!((near_yaw - 0.0).abs() < 1e-6);
        assert!((near_limit - 70.0).abs() < 1e-6);
    }

    #[test]
    fn rotation_response_uses_half_life_and_target_speed_settings() {
        let half = damper_yaw(0.0, std::f32::consts::FRAC_PI_2, 0.1, 0.1);
        assert!((half - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        let advanced = advance_yaw(0.0, std::f32::consts::FRAC_PI_2, 500.0, 0.1);
        assert!((advanced - 50.0_f32.to_radians()).abs() < 1e-5);
        let params = AlsMovementParams::default();
        let normal = moving_rotation_half_life(
            &params,
            Gait::Running,
            RotationMode::ViewDirection,
            true,
            0.0,
        );
        let responsive = moving_rotation_half_life(
            &params,
            Gait::Running,
            RotationMode::ViewDirection,
            true,
            params.rotation.reference_view_yaw_speed,
        );
        assert!((normal - 0.15).abs() < 1e-6);
        assert!((responsive - 0.05).abs() < 1e-6);
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
    fn step_assist_direction_matches_locomotion_across_view_yaws() {
        let cases = [
            (0.0, Vec3::NEG_Z),
            (std::f32::consts::FRAC_PI_2, Vec3::NEG_X),
            (std::f32::consts::PI, Vec3::Z),
            (-std::f32::consts::FRAC_PI_2, Vec3::X),
        ];
        for (yaw, expected) in cases {
            let locomotion = view_relative_move_direction(Vec2::Y, yaw);
            let assist = step_assist_world_direction(Vec2::Y, yaw);
            assert!(assist.distance(expected) < 1e-5, "yaw={yaw}");
            assert!(assist.distance(locomotion) < 1e-5, "yaw={yaw}");
        }
    }

    #[test]
    #[ignore]
    fn step_assist_uses_default_motion_plugin_with_physics_queries() {
        let cases = [
            (0.0, Vec3::NEG_Z),
            (std::f32::consts::FRAC_PI_2, Vec3::NEG_X),
            (std::f32::consts::PI, Vec3::Z),
            (-std::f32::consts::FRAC_PI_2, Vec3::X),
        ];
        for (index, (yaw, expected)) in cases.into_iter().enumerate() {
            let origin = Vec3::new(index as f32 * 4.0, ALS_CHARACTER_CAPSULE_HALF_HEIGHT, 0.0);
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins,
                AssetPlugin::default(),
                PhysicsPlugins::new(FixedPostUpdate),
                TransformPlugin,
                TnuaAvian3dPlugin::new(FixedUpdate),
                TnuaControllerPlugin::<AlsControlScheme>::new(FixedUpdate),
                AlsMotionPlugin,
            ))
            .init_resource::<Assets<Mesh>>()
            .add_message::<AssetEvent<Mesh>>()
            .insert_resource(Assets::<AlsMovementParams>::default())
            .insert_resource(StepAssistPhysicsProbe {
                ground_distance: f32::NAN,
                step_distance: f32::NAN,
            })
            .add_systems(
                Update,
                record_step_assist_physics_probe.after(step_up_assist_system),
            );
            app.finish();
            let params = app
                .world_mut()
                .resource_mut::<Assets<AlsMovementParams>>()
                .add(AlsMovementParams::default());
            let mut input = AlsInputCommand::default();
            input.move_dir = Vec2::Y;
            app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(40.0, 0.2, 20.0),
                Transform::from_xyz(0.0, -0.1, 0.0),
            ));
            let step_center = Vec3::new(origin.x, 0.15, origin.z) + expected * 0.7;
            app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(0.6, 0.3, 0.6),
                Transform::from_translation(step_center),
            ));
            let character = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    RigidBody::Dynamic,
                    GravityScale(0.0),
                    Collider::capsule(ALS_CHARACTER_CAPSULE_RADIUS, 1.2),
                    Position(origin),
                    Transform::from_translation(origin),
                    LinearVelocity::ZERO,
                    TnuaController::<AlsControlScheme>::default(),
                    LocomotionState::Grounded,
                    Stance::Standing,
                    AlsViewYaw(yaw),
                    StepAssistCooldown(0.0),
                    AlsMovementParamsHandle(params),
                    input,
                    StepAssistPhysicsCase { origin, expected },
                ))
                .id();

            for _ in 0..8 {
                app.update();
                app.world_mut().run_schedule(FixedUpdate);
                app.world_mut().run_schedule(FixedPostUpdate);
            }
            app.update();

            let probe = app.world().resource::<StepAssistPhysicsProbe>();
            let ground_distance = probe.ground_distance;
            let step_distance = probe.step_distance;
            let entity = app.world().entity(character);
            let transform = entity.get::<Transform>().unwrap();
            let position = entity.get::<Position>().unwrap();
            let displacement = transform.translation - origin;
            let horizontal = Vec3::new(displacement.x, 0.0, displacement.z);
            let direction = horizontal.normalize_or_zero();
            let direction_dot = direction.dot(expected);
            let sync_error = transform.translation.distance(position.0);
            let capsule_bottom = transform.translation.y - ALS_CHARACTER_CAPSULE_HALF_HEIGHT;
            let step_top = step_center.y + 0.15;
            println!(
                "[BF102.01] yaw={yaw:.6} ground_distance={ground_distance:.6} step_distance={step_distance:.6} direction_dot={direction_dot:.6} rise={:.6} sync_error={sync_error:.6} floor_gap={:.6}",
                displacement.y,
                capsule_bottom - step_top
            );
            assert!(direction_dot > 0.99, "yaw={yaw}");
            assert!(displacement.y > 0.25, "yaw={yaw}");
            assert!(sync_error < 1e-4, "yaw={yaw}");
            assert!(capsule_bottom >= step_top - 0.01, "yaw={yaw}");
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
            ),
            Stance::Crouching
        );
        assert_eq!(
            capsule_target_for_locomotion(
                Stance::Crouching,
                Stance::Standing,
                LocomotionState::Air,
                false,
            ),
            Stance::Standing
        );
        assert_eq!(
            capsule_target_for_locomotion(
                Stance::Standing,
                Stance::Crouching,
                LocomotionState::Air,
                true,
            ),
            Stance::Crouching
        );
        assert_eq!(
            capsule_target_for_locomotion(
                Stance::Standing,
                Stance::Standing,
                LocomotionState::Air,
                false,
            ),
            Stance::Standing
        );
        assert_eq!(
            capsule_target_for_locomotion(
                Stance::Crouching,
                Stance::Standing,
                LocomotionState::Air,
                true,
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
            jump_chain(LocomotionState::Ragdolling, Stance::Crouching, true),
            JumpChain::ExitRagdoll
        );
        assert_eq!(
            jump_chain(LocomotionState::Grounded, Stance::Standing, true),
            JumpChain::Mantle
        );
        assert_eq!(
            jump_chain(LocomotionState::Grounded, Stance::Crouching, false),
            JumpChain::StandUp
        );
        assert_eq!(
            jump_chain(LocomotionState::Air, Stance::Standing, false),
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
            self.phase = match actual {
                Stance::Standing => AlsCrouchTransactionPhase::Standing,
                Stance::Crouching => AlsCrouchTransactionPhase::Crouching,
            };
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

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct StepAssistCooldown(pub f32);

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

pub fn jump_chain(loco: LocomotionState, stance: Stance, mantle_available: bool) -> JumpChain {
    if loco == LocomotionState::Ragdolling {
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
                    locomotion_state_sync_system,
                    crate::gait::refresh_gait_system
                        .after(locomotion_state_sync_system)
                        .after(crate::gait::gait_request_system),
                    tnua_config_sync_system
                        .after(crate::gait::refresh_gait_system)
                        .after(crate::transition::state_transition_system)
                        .after(capsule_reshape_system),
                    locomotion_drive_system
                        .after(crate::view::AlsViewInputSet)
                        .before(crate::transition::state_transition_system),
                    crouch_controller_transaction_system
                        .after(locomotion_drive_system)
                        .after(capsule_reshape_system),
                    step_up_assist_system,
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
                snapshot_actual_velocity_system.in_set(AlsActualVelocitySet),
            )
            .add_systems(
                PostUpdate,
                rotation_response_system.in_set(AlsRotationResponseSet),
            );
    }
}

fn snapshot_actual_velocity_system(
    mut characters: Query<(&LinearVelocity, &mut AlsActualVelocity), With<AlsCharacter>>,
) {
    for (velocity, mut actual) in &mut characters {
        *actual = AlsActualVelocity::from_physics_velocity(velocity.0);
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

fn tnua_config_sync_system(
    q: Query<(
        &TnuaConfig<AlsControlScheme>,
        &AlsMovementParamsHandle,
        &AlsMaxAllowedGait,
        &Stance,
        &RotationMode,
        &AlsViewYaw,
        &LinearVelocity,
    )>,
    mut configs: ResMut<Assets<AlsControlSchemeConfig>>,
    params_assets: Res<Assets<AlsMovementParams>>,
) {
    for (config, handle, max_allowed, stance, rotation_mode, view_yaw, velocity) in &q {
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
            velocity.0,
            view_yaw.0,
        );
    }
}

type DriveQueryItem<'a> = (
    Entity,
    &'a mut TnuaController<AlsControlScheme>,
    &'a AlsViewYaw,
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
    velocities: Query<&LinearVelocity, Without<AlsCharacter>>,
    target_permissions: Query<Option<&AlsMantleTargetPermission>, Without<AlsCharacter>>,
    target_transforms: Query<&GlobalTransform, Without<AlsCharacter>>,
    mantle_controls: Query<Option<&AlsMantleControlSettings>, With<AlsCharacter>>,
    mantle_start_time_settings: Query<Option<&AlsMantleStartTimeSettings>, With<AlsCharacter>>,
    params_assets: Res<Assets<AlsMovementParams>>,
    rolls: Query<&crate::roll::AlsRollState, With<AlsCharacter>>,
    mut q: Query<DriveQueryItem, With<AlsCharacter>>,
) {
    for (
        entity,
        mut controller,
        view_yaw,
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
        let effective = effective_input(cmd, override_cmd);

        let move_len = effective.move_dir.length().min(1.0);
        let world_dir = if move_len > 1e-4 {
            view_relative_move_direction(effective.move_dir, view_yaw.0)
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
        let action_active = roll.is_some_and(|state| state.active);
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
                        match *stance {
                            Stance::Standing => params.crouch.standing_capsule_half_height,
                            Stance::Crouching => params.crouch.crouched_capsule_half_height,
                        },
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
            Some(detect_mantle(
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
            ))
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
                        (
                            intent,
                            AlsMantleTargetBase {
                                entity: intent.target_entity,
                                translation: transform.translation(),
                                rotation: transform.rotation(),
                            },
                        )
                    })
            })
        });
        if jump_edge {
            match jump_chain(*loco, *stance, intent.is_some()) {
                JumpChain::ExitRagdoll => {
                    request.locomotion_state = Some(LocomotionState::Grounded);
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
                    controller.action(AlsControlScheme::Jump(TnuaBuiltinJump::default()));
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
    &'a LocomotionState,
    Option<&'a AlsMantleState>,
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
        locomotion,
        mantle,
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
        let action_active = mantle.is_some_and(|mantle| mantle.active);
        transaction.synchronize(
            desired.0,
            *stance,
            half_height,
            radius,
            transform.translation,
            *locomotion == LocomotionState::Air && !action_active,
        );
        let target = capsule_target_for_locomotion(desired.0, *stance, *locomotion, action_active);
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
    }
}

fn capsule_dimensions(params: &AlsMovementParams, stance: Stance) -> (f32, f32) {
    let half_height = match stance {
        Stance::Standing => params.crouch.standing_capsule_half_height,
        Stance::Crouching => params.crouch.crouched_capsule_half_height,
    };
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
) -> Stance {
    if action_active {
        actual
    } else if locomotion == LocomotionState::Air {
        Stance::Standing
    } else {
        desired
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

fn step_up_assist_system(
    time: Res<Time>,
    mut commands: Commands,
    spatial_query: SpatialQuery,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut q: Query<
        (
            Entity,
            &mut Transform,
            &LinearVelocity,
            &LocomotionState,
            &AlsViewYaw,
            Option<&mut StepAssistCooldown>,
            &AlsMovementParamsHandle,
            &Stance,
            Option<&AlsInputOverride>,
            &AlsInputCommand,
        ),
        With<AlsCharacter>,
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, vel, loco, view_yaw, cooldown, handle, stance, override_cmd, cmd) in
        &mut q
    {
        if cooldown.is_none() {
            commands.entity(entity).insert(StepAssistCooldown(0.0));
            continue;
        }
        let mut cd = cooldown.unwrap();
        cd.0 -= dt;
        if cd.0 > 0.0 || *loco != LocomotionState::Grounded {
            continue;
        }
        let input = effective_input(cmd, override_cmd);
        let want = input.move_dir;
        if want.length() < 0.7 || Vec2::new(vel.x, vel.z).length() > STALL_SPEED_EPSILON {
            continue;
        }
        let Some(params) = params_assets.get(handle.id()) else {
            continue;
        };
        let radius = params.crouch.standing_capsule_radius;
        let half = match *stance {
            Stance::Crouching => params.crouch.crouched_capsule_half_height,
            Stance::Standing => params.crouch.standing_capsule_half_height,
        };
        let center = transform.translation;
        let world_dir = step_assist_world_direction(want, view_yaw.0);
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let Some(cur_hit) =
            spatial_query.cast_ray(center, Dir3::NEG_Y, MAX_STEP_HEIGHT + 1.5, false, &filter)
        else {
            continue;
        };
        let cur_ground_y = center.y - cur_hit.distance;
        let ahead_xz = center + world_dir * (radius + 0.15);
        let probe_origin = Vec3::new(ahead_xz.x, center.y + MAX_STEP_HEIGHT, ahead_xz.z);
        let Some(probe_hit) = spatial_query.cast_ray(
            probe_origin,
            Dir3::NEG_Y,
            cur_hit.distance + MAX_STEP_HEIGHT + 0.05,
            false,
            &filter,
        ) else {
            continue;
        };
        let next_ground_y = probe_origin.y - probe_hit.distance;
        let dh = next_ground_y - cur_ground_y;
        if dh <= 0.03 || dh > MAX_STEP_HEIGHT {
            continue;
        }
        let dest_center_y = next_ground_y + half;
        let headroom_ok = spatial_query
            .cast_ray(
                Vec3::new(ahead_xz.x, dest_center_y, ahead_xz.z),
                Dir3::Y,
                half,
                true,
                &filter,
            )
            .is_none_or(|hit| hit.distance >= half - HEADROOM_SKIN);
        if !headroom_ok {
            continue;
        }
        transform.translation = Vec3::new(
            center.x + world_dir.x * 0.04,
            dest_center_y + 0.005,
            center.z + world_dir.z * 0.04,
        );
        cd.0 = STEP_ASSIST_COOLDOWN;
        info!("[MOTION] entity={entity:?} step-up assist +{dh:.2}m");
    }
}

fn locomotion_state_sync_system(
    mut q: Query<
        (
            Entity,
            &TnuaController<AlsControlScheme>,
            &LocomotionState,
            &mut AlsStateRequest,
            Has<AlsRagdollAirExit>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, controller, current, mut request, ragdoll_air_exit) in &mut q {
        if *current == LocomotionState::Ragdolling || ragdoll_air_exit {
            continue;
        }
        let Ok(airborne) = controller.is_airborne() else {
            continue;
        };
        let target = if airborne {
            LocomotionState::Air
        } else {
            LocomotionState::Grounded
        };
        if *current != target && request.locomotion_state.is_none() {
            info!(
                "[MOTION] entity={entity:?} locomotion sync {:?} -> {:?}",
                current, target
            );
            request.locomotion_state = Some(target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
