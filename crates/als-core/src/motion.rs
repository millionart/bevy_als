use avian3d::prelude::{
    Collider, LinearVelocity, PhysicsSystems, Rotation, SpatialQuery, SpatialQueryFilter,
};
use bevy::asset::{AssetId, Assets};
use bevy::prelude::*;
use bevy_tnua::TnuaController;
use bevy_tnua::builtins::{
    TnuaBuiltinCrouch, TnuaBuiltinJump, TnuaBuiltinWalk, TnuaBuiltinWalkHeadroom,
};
use bevy_tnua::controller::TnuaConfig;

use crate::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_FLOOR_CLEARANCE,
    ALS_CHARACTER_TNUA_FLOAT_EPSILON, AlsCharacter, AlsMovementParamsHandle, als_character_capsule,
};
use crate::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use crate::gait::AlsMaxAllowedGait;
use crate::input::AlsInputCommand;
use crate::input_request::AlsFirstPersonView;
use crate::mantle::{MantleDetectionInput, detect_mantle, yaw_from_direction};
use crate::params::AlsMovementParams;
use crate::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use crate::transition::AlsStateRequest;

const MAX_SLOPE_DEGREES: f32 = 44.765_083;
const WALK_CLING_DISTANCE: f32 = ALS_CHARACTER_TNUA_FLOAT_EPSILON;
const AIR_CONTROL: f32 = 0.15;
const AIR_CONTROL_BOOST_MULTIPLIER: f32 = 2.0;
const AIR_CONTROL_BOOST_SPEED: f32 = 0.25;
const HEADROOM_SKIN: f32 = 0.01;
const FOOTROOM_SKIN: f32 = 0.03;
const MAX_STEP_HEIGHT: f32 = 0.45;
const STEP_ASSIST_COOLDOWN: f32 = 0.3;
const STALL_SPEED_EPSILON: f32 = 1.2;

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
    let speeds = params.gait_speeds(*stance, rotation_mode);
    match *gait {
        Gait::Walking => speeds.max_walk_speed,
        Gait::Running => speeds.max_run_speed,
        Gait::Sprinting => speeds.max_sprint_speed,
    }
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

pub fn apply_als_control_scheme_config(
    config: &mut AlsControlSchemeConfig,
    params: &AlsMovementParams,
    gait: Gait,
    stance: Stance,
    rotation_mode: RotationMode,
    capsule_crouched: bool,
    horizontal_speed: f32,
) {
    config.basis.speed = gait_max_speed_for_rotation(params, &stance, &gait, rotation_mode);
    config.basis.float_height = params.crouch.standing_capsule_half_height
        + ALS_CHARACTER_FLOOR_CLEARANCE
        + ALS_CHARACTER_TNUA_FLOAT_EPSILON;
    config.basis.acceleration = 0.0;
    config.basis.air_acceleration = air_acceleration_for_horizontal_speed(params, horizontal_speed);
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

fn aiming_yaw_constraint_system(
    params_assets: Res<Assets<AlsMovementParams>>,
    mut q: Query<
        (
            &RotationMode,
            Has<AlsFirstPersonView>,
            &AlsViewYaw,
            &AlsMovementParamsHandle,
            &mut Rotation,
            &mut Transform,
            &mut AlsAimingYawConstraint,
        ),
        With<AlsCharacter>,
    >,
) {
    for (
        rotation_mode,
        first_person,
        view_yaw,
        params_handle,
        mut rotation,
        mut transform,
        mut constraint,
    ) in &mut q
    {
        let applies = *rotation_mode == RotationMode::Aiming || first_person;
        if !applies {
            constraint.applied_last_frame = false;
            constraint.active_limit_degrees = 180.0;
            continue;
        }
        let Some(params) = params_assets.get(params_handle.id()) else {
            continue;
        };
        let (actor_yaw, _, _) = rotation.0.to_euler(EulerRot::YXZ);
        let view_direction = view_relative_move_direction(Vec2::Y, view_yaw.0);
        let view_target_yaw = f32::atan2(view_direction.x, -view_direction.z);
        let (constrained_yaw, active_limit_degrees) = constrained_aiming_yaw(
            view_target_yaw,
            actor_yaw,
            params.cmc.aiming_yaw_angle_limit_degrees,
            constraint.active_limit_degrees,
            !constraint.applied_last_frame,
        );
        let next_rotation = Quat::from_rotation_y(constrained_yaw);
        rotation.0 = next_rotation;
        transform.rotation = next_rotation;
        constraint.active_limit_degrees = active_limit_degrees;
        constraint.applied_last_frame = true;
    }
}

#[cfg(test)]
mod rotation_cap_tests {
    use super::*;

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

#[derive(Component, Debug, Default, Clone)]
pub struct AlsInputOverride(pub AlsInputCommand);

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsCapsuleCrouched;

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

fn effective_input<'a>(
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

impl Plugin for AlsMotionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
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
                    .after(crate::transition::state_transition_system),
                locomotion_drive_system
                    .after(crate::view::AlsViewInputSet)
                    .before(crate::transition::state_transition_system),
                step_up_assist_system,
                capsule_reshape_system,
            ),
        )
        .add_systems(
            FixedPostUpdate,
            aiming_yaw_constraint_system.after(PhysicsSystems::Last),
        );
    }
}

fn tnua_config_sync_system(
    q: Query<(
        &TnuaConfig<AlsControlScheme>,
        &AlsMovementParamsHandle,
        &AlsMaxAllowedGait,
        &Stance,
        &RotationMode,
        Has<AlsCapsuleCrouched>,
        &LinearVelocity,
    )>,
    mut configs: ResMut<Assets<AlsControlSchemeConfig>>,
    params_assets: Res<Assets<AlsMovementParams>>,
) {
    for (config, handle, max_allowed, stance, rotation_mode, capsule_crouched, velocity) in &q {
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
            capsule_crouched,
            Vec2::new(velocity.x, velocity.z).length(),
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
    params_assets: Res<Assets<AlsMovementParams>>,
    rotation_modes: Query<&RotationMode, With<AlsCharacter>>,
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
        let Ok(rotation_mode) = rotation_modes.get(entity) else {
            continue;
        };
        controller.initiate_action_feeding();
        let effective = effective_input(cmd, override_cmd);

        let move_len = effective.move_dir.length().min(1.0);
        let world_dir = if move_len > 1e-4 {
            view_relative_move_direction(effective.move_dir, view_yaw.0)
        } else {
            Vec3::ZERO
        };

        let desired_forward: Option<Dir3> = match *rotation_mode {
            RotationMode::VelocityDirection => {
                if move_len > 1e-4 {
                    Dir3::new(world_dir).ok()
                } else {
                    None
                }
            }
            RotationMode::ViewDirection | RotationMode::Aiming => {
                Dir3::new(view_relative_move_direction(Vec2::Y, view_yaw.0)).ok()
            }
        };

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

        let mantle_ready = mantle.as_deref().is_some_and(|m| !m.active);
        let auto_mantle = mantle_ready && *loco == LocomotionState::Air;
        let should_probe_mantle = mantle_ready && (jump_edge || auto_mantle);
        let detection = if should_probe_mantle {
            let origin = gtf.translation();
            let fwd = gtf.forward().as_vec3();
            let (capsule_half_height, capsule_radius) = params_assets
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
                },
                &velocities,
            ))
        } else {
            None
        };
        if let (Some(detection), Some(mut debug)) = (detection, character_debug) {
            debug.0.mantle_probe = Some(detection.trace);
        }
        let intent = detection.and_then(|detection| detection.intent);
        if jump_edge {
            match jump_chain(*loco, *stance, intent.is_some()) {
                JumpChain::ExitRagdoll => {
                    request.locomotion_state = Some(LocomotionState::Grounded);
                }
                JumpChain::Mantle => {
                    if let (Some(m), Some(i)) = (mantle.as_deref_mut(), intent) {
                        let origin = gtf.translation();
                        let yaw_start = yaw_from_direction(gtf.forward().as_vec3());
                        m.begin(
                            i.kind,
                            origin,
                            i.target,
                            i.mantling_height,
                            yaw_start,
                            i.yaw_target,
                        );
                        info!(
                            "[MOTION] entity={entity:?} mantle begin kind={:?} height={:.2}",
                            i.kind, i.mantling_height
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
        } else if auto_mantle && let (Some(m), Some(i)) = (mantle.as_deref_mut(), intent) {
            let origin = gtf.translation();
            let yaw_start = yaw_from_direction(gtf.forward().as_vec3());
            m.begin(
                i.kind,
                origin,
                i.target,
                i.mantling_height,
                yaw_start,
                i.yaw_target,
            );
            info!(
                "[MOTION] entity={entity:?} auto mantle begin kind={:?} height={:.2}",
                i.kind, i.mantling_height
            );
        } else if effective.jump_held && controller.action_flow_status().ongoing().is_some() {
            controller.prolong_action();
        }
    }
}

type ReshapeQueryItem<'a> = (
    Entity,
    &'a Stance,
    &'a Transform,
    &'a AlsMovementParamsHandle,
    Has<CapsuleReshapeDeferred>,
);

fn capsule_reshape_system(
    mut commands: Commands,
    spatial_query: SpatialQuery,
    params_assets: Res<Assets<AlsMovementParams>>,
    q_changed: Query<ReshapeQueryItem, (With<AlsCharacter>, Changed<Stance>)>,
    q_retry: Query<ReshapeQueryItem, (With<AlsCharacter>, With<AlsCapsuleCrouched>)>,
) {
    let mut changed_entities: Vec<Entity> = Vec::new();
    for (entity, stance, transform, handle, already_deferred) in q_changed.iter() {
        changed_entities.push(entity);
        apply_capsule_for_stance(
            &mut commands,
            &spatial_query,
            &params_assets,
            entity,
            *stance,
            transform.translation,
            handle.id(),
            already_deferred,
        );
    }
    for (entity, stance, transform, handle, already_deferred) in q_retry.iter() {
        if *stance != Stance::Standing || changed_entities.contains(&entity) {
            continue;
        }
        apply_capsule_for_stance(
            &mut commands,
            &spatial_query,
            &params_assets,
            entity,
            *stance,
            transform.translation,
            handle.id(),
            already_deferred,
        );
    }
}

fn apply_capsule_for_stance(
    commands: &mut Commands,
    spatial_query: &SpatialQuery,
    params_assets: &Assets<AlsMovementParams>,
    entity: Entity,
    stance: Stance,
    center: Vec3,
    params_handle: AssetId<AlsMovementParams>,
    already_deferred: bool,
) {
    let Some(params) = params_assets.get(params_handle) else {
        return;
    };
    match stance {
        Stance::Crouching => {
            commands
                .entity(entity)
                .remove::<CapsuleReshapeDeferred>()
                .insert(AlsCapsuleCrouched)
                .insert(capsule_for(params, stance));
        }
        Stance::Standing => {
            let half = params.crouch.standing_capsule_half_height;
            let filter = SpatialQueryFilter::from_excluded_entities([entity]);
            let headroom_hit = spatial_query.cast_ray(center, Dir3::Y, half, true, &filter);
            let headroom_clear = headroom_hit
                .as_ref()
                .is_none_or(|hit| hit.distance >= half - HEADROOM_SKIN);
            if !headroom_clear {
                if !already_deferred {
                    info!(
                        "[MOTION] entity={entity:?} stand-up deferred: headroom blocked (ceiling at {:.2}, need {:.2})",
                        headroom_hit.map(|hit| hit.distance).unwrap_or(f32::NAN),
                        half
                    );
                    commands.entity(entity).insert(CapsuleReshapeDeferred);
                }
                return;
            }
            let footroom_clear = spatial_query
                .cast_ray(center, Dir3::NEG_Y, half, false, &filter)
                .is_none_or(|hit| hit.distance >= half - FOOTROOM_SKIN);
            if !footroom_clear {
                return;
            }
            let was_deferred = already_deferred;
            commands
                .entity(entity)
                .remove::<CapsuleReshapeDeferred>()
                .remove::<AlsCapsuleCrouched>()
                .insert(capsule_for(params, stance));
            if was_deferred {
                info!("[MOTION] entity={entity:?} clearance regained: standing capsule restored");
            }
        }
    }
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
            Option<&mut StepAssistCooldown>,
            &AlsMovementParamsHandle,
            Has<AlsCapsuleCrouched>,
            Option<&AlsInputOverride>,
            &AlsInputCommand,
        ),
        With<AlsCharacter>,
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, vel, loco, cooldown, handle, capsule_crouched, override_cmd, cmd) in
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
        let half = if capsule_crouched {
            params.crouch.crouched_capsule_half_height
        } else {
            params.crouch.standing_capsule_half_height
        };
        let center = transform.translation;
        let world_dir = Vec3::new(want.x, 0.0, -want.y).normalize();
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
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, controller, current, mut request) in &mut q {
        if *current == LocomotionState::Ragdolling {
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
