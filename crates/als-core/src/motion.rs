use avian3d::prelude::{Collider, LinearVelocity, SpatialQuery, SpatialQueryFilter};
use bevy::asset::{AssetId, Assets};
use bevy::prelude::*;
use bevy_tnua::builtins::{
    TnuaBuiltinCrouch, TnuaBuiltinJump, TnuaBuiltinWalk, TnuaBuiltinWalkHeadroom,
};
use bevy_tnua::controller::TnuaConfig;
use bevy_tnua::TnuaController;

use crate::character::{AlsCharacter, AlsMovementParamsHandle};
use crate::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use crate::input::AlsInputCommand;
use crate::mantle::{detect_mantle, yaw_from_direction, MantleIntent};
use crate::params::AlsMovementParams;
use crate::state::{Gait, LocomotionState, RotationMode, Stance};
use crate::transition::AlsStateRequest;

const MAX_SLOPE_DEGREES: f32 = 50.0;
const WALK_CLING_DISTANCE: f32 = 0.5;
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
    match stance {
        Stance::Crouching => params.crouch.max_walk_speed_crouched,
        Stance::Standing => {
            let s = params.stance_speeds(*stance);
            match effective_gait(*stance, *gait) {
                Gait::Walking => s.max_walk_speed,
                Gait::Running => s.max_run_speed,
                Gait::Sprinting => s.max_sprint_speed,
            }
        }
    }
}

fn capsule_for(params: &AlsMovementParams, stance: Stance) -> Collider {
    let total_half = match stance {
        Stance::Standing => params.crouch.standing_capsule_half_height,
        Stance::Crouching => params.crouch.crouched_capsule_half_height,
    };
    let radius = params.crouch.standing_capsule_radius;
    Collider::capsule((total_half - radius).max(0.0), radius)
}

fn turn_angvel(mode: RotationMode) -> f32 {
    match mode {
        RotationMode::VelocityDirection => deg_to_rad(800.0),
        RotationMode::ViewDirection => deg_to_rad(500.0),
        RotationMode::Aiming => deg_to_rad(1000.0),
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
        assert!(turn_angvel(RotationMode::VelocityDirection) > turn_angvel(RotationMode::ViewDirection));
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsViewYaw(pub f32);

#[derive(Component, Debug, Default, Clone)]
pub struct AlsInputOverride(pub AlsInputCommand);

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsCapsuleCrouched;

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
        app.add_systems(
            Update,
            (
                tnua_config_sync_system,
                locomotion_drive_system.before(crate::transition::state_transition_system),
                step_up_assist_system,
                capsule_reshape_system,
                locomotion_state_sync_system,
            ),
        );
    }
}

fn tnua_config_sync_system(
    q: Query<(
        &TnuaConfig<AlsControlScheme>,
        &AlsMovementParamsHandle,
        &Gait,
        &Stance,
        &RotationMode,
        Has<AlsCapsuleCrouched>,
    )>,
    mut configs: ResMut<Assets<AlsControlSchemeConfig>>,
    params_assets: Res<Assets<AlsMovementParams>>,
) {
    for (config, handle, gait, stance, rotation_mode, capsule_crouched) in &q {
        let Some(params) = params_assets.get(handle.id()) else {
            continue;
        };
        let Some(mut cfg) = configs.get_mut(&config.0) else {
            continue;
        };
        cfg.basis.speed = gait_max_speed(params, stance, gait);
        cfg.basis.float_height = params.crouch.standing_capsule_half_height;
        cfg.basis.acceleration = params.acceleration.max_acceleration;
        cfg.basis.coyote_time = params.jump.coyote_time;
        cfg.basis.headroom = Some(TnuaBuiltinWalkHeadroom {
            distance_to_collider_top: if capsule_crouched {
                params.crouch.crouched_capsule_half_height
            } else {
                params.crouch.standing_capsule_half_height
            },
            ..Default::default()
        });
        cfg.basis.turning_angvel = turn_angvel(*rotation_mode);
        cfg.basis.max_slope = deg_to_rad(MAX_SLOPE_DEGREES);
        cfg.basis.cling_distance = WALK_CLING_DISTANCE;
        let gravity_scale = params.jump.gravity_scale.max(1e-3);
        cfg.jump.height =
            params.jump.jump_z_velocity * params.jump.jump_z_velocity / (2.0 * 9.81 * gravity_scale);
        cfg.jump.shorten_extra_gravity = 0.0;
        cfg.crouch.float_offset = -(params.crouch.standing_capsule_half_height
            - params.crouch.crouched_capsule_half_height);
    }
}

type DriveQueryItem<'a> = (
    Entity,
    &'a mut TnuaController<AlsControlScheme>,
    &'a mut AlsViewYaw,
    &'a GlobalTransform,
    &'a LocomotionState,
    Option<&'a mut crate::mantle::AlsMantleState>,
    Option<&'a AlsInputOverride>,
    &'a AlsInputCommand,
    &'a Stance,
    &'a RotationMode,
    &'a mut AlsStateRequest,
    Option<&'a mut AlsJumpPrev>,
);

#[allow(clippy::type_complexity)]
fn locomotion_drive_system(
    mut commands: Commands,
    spatial: SpatialQuery,
    velocities: Query<&LinearVelocity, Without<AlsCharacter>>,
    mut q: Query<DriveQueryItem, With<AlsCharacter>>,
) {
    for (
        entity,
        mut controller,
        mut view_yaw,
        gtf,
        loco,
        mut mantle,
        override_cmd,
        cmd,
        stance,
        rotation_mode,
        mut request,
        jump_prev,
    ) in &mut q
    {
        controller.initiate_action_feeding();
        let effective = effective_input(cmd, override_cmd);

        view_yaw.0 -= effective.look_delta.x * 0.0025;

        let move_len = effective.move_dir.length().min(1.0);
        let world_dir = if move_len > 1e-4 {
            Vec3::new(effective.move_dir.x, 0.0, -effective.move_dir.y).normalize()
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
                Dir3::new(Vec3::new(view_yaw.0.sin(), 0.0, -view_yaw.0.cos())).ok()
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

        if jump_edge {
            let mantle_ready = mantle.as_deref().is_some_and(|m| {
                !m.active
                    && m.cooldown <= 0.0
                    && *loco == LocomotionState::Grounded
                    && *stance == Stance::Standing
            });
            let mut intent: Option<MantleIntent> = None;
            if mantle_ready {
                let origin = gtf.translation();
                let f = gtf.forward();
                let fwd = Quat::from_rotation_y(f32::atan2(f.x, -f.z))
                    .mul_vec3(Vec3::NEG_Z)
                    .normalize_or_zero();
                let foot_y = origin.y - 0.90;
                intent = detect_mantle(&spatial, entity, origin, foot_y, fwd, &velocities);
            }
            match jump_chain(*loco, *stance, intent.is_some()) {
                JumpChain::ExitRagdoll => {
                    request.locomotion_state = Some(LocomotionState::Grounded);
                }
                JumpChain::Mantle => {
                    if let (Some(m), Some(i)) = (mantle.as_deref_mut(), intent) {
                        let origin = gtf.translation();
                        let f = gtf.forward();
                        let fwd = Quat::from_rotation_y(f32::atan2(f.x, -f.z))
                            .mul_vec3(Vec3::NEG_Z)
                            .normalize_or_zero();
                        let yaw_start = yaw_from_direction(fwd);
                        m.begin(i.kind, origin, i.target, i.top_height, yaw_start, i.yaw_target);
                        info!(
                            "[MOTION] entity={entity:?} mantle begin kind={:?} top_h={:.2}",
                            i.kind, i.top_height
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
    for (
        entity,
        mut transform,
        vel,
        loco,
        cooldown,
        handle,
        capsule_crouched,
        override_cmd,
        cmd,
    ) in &mut q
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
        assert_eq!(effective_gait(Stance::Crouching, Gait::Walking), Gait::Walking);
        assert_eq!(effective_gait(Stance::Crouching, Gait::Running), Gait::Walking);
        assert_eq!(effective_gait(Stance::Crouching, Gait::Sprinting), Gait::Walking);
        assert_eq!(effective_gait(Stance::Standing, Gait::Running), Gait::Running);
        assert_eq!(effective_gait(Stance::Standing, Gait::Sprinting), Gait::Sprinting);
    }

    #[test]
    fn standing_gait_speeds_match_als_table() {
        let p = AlsMovementParams::default();
        assert!((gait_max_speed(&p, &Stance::Standing, &Gait::Walking) - 1.75).abs() < 1e-6);
        assert!((gait_max_speed(&p, &Stance::Standing, &Gait::Running) - 3.75).abs() < 1e-6);
        assert!((gait_max_speed(&p, &Stance::Standing, &Gait::Sprinting) - 6.50).abs() < 1e-6);
    }

    #[test]
    fn crouching_speed_clamped_to_als_crouch_walk() {
        let p = AlsMovementParams::default();
        for gait in [Gait::Walking, Gait::Running, Gait::Sprinting] {
            assert!(
                (gait_max_speed(&p, &Stance::Crouching, &gait) - p.crouch.max_walk_speed_crouched)
                    .abs()
                    < 1e-6
            );
        }
        assert!((p.crouch.max_walk_speed_crouched - 1.50).abs() < 1e-6);
    }
}
