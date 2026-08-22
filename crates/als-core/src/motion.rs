use avian3d::prelude::Collider;
use bevy::asset::Assets;
use bevy::prelude::*;
use bevy_tnua::builtins::{
    TnuaBuiltinCrouch, TnuaBuiltinJump, TnuaBuiltinWalk, TnuaBuiltinWalkConfig,
    TnuaBuiltinWalkHeadroom,
};
use bevy_tnua::controller::TnuaConfig;
use bevy_tnua::TnuaController;

use crate::character::{AlsCharacter, AlsMovementParamsHandle};
use crate::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use crate::input::AlsInputCommand;
use crate::params::AlsMovementParams;
use crate::state::{Gait, LocomotionState, RotationMode, Stance};
use crate::transition::AlsStateRequest;

fn deg_to_rad(deg: f32) -> f32 {
    deg * std::f32::consts::PI / 180.0
}

pub fn gait_max_speed(params: &AlsMovementParams, stance: &Stance, gait: &Gait) -> f32 {
    let s = params.stance_speeds(*stance);
    match gait {
        Gait::Walking => s.max_walk_speed,
        Gait::Running => s.max_run_speed,
        Gait::Sprinting => s.max_sprint_speed,
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

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsViewYaw(pub f32);

#[derive(Component, Debug, Default, Clone)]
pub struct AlsInputOverride(pub AlsInputCommand);

pub struct AlsMotionPlugin;

impl Plugin for AlsMotionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                tnua_config_sync_system,
                locomotion_drive_system,
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
    )>,
    mut configs: ResMut<Assets<AlsControlSchemeConfig>>,
    params_assets: Res<Assets<AlsMovementParams>>,
) {
    for (config, handle, gait, stance, rotation_mode) in &q {
        let Some(params) = params_assets.get(handle.id()) else {
            continue;
        };
                let Some(mut cfg) = configs.get_mut(&config.0) else {
            continue;
        };
        let cfg: &mut AlsControlSchemeConfig = &mut cfg;
        cfg.basis = TnuaBuiltinWalkConfig {
            speed: gait_max_speed(params, stance, gait),
            float_height: params.crouch.standing_capsule_half_height,
            acceleration: params.acceleration.max_acceleration,
            coyote_time: params.jump.coyote_time,
            headroom: Some(TnuaBuiltinWalkHeadroom {
                distance_to_collider_top: params.crouch.standing_capsule_half_height,
                ..Default::default()
            }),
            turning_angvel: turn_angvel(*rotation_mode),
            max_slope: deg_to_rad(50.0),
            ..Default::default()
        };
        let gravity_scale = params.jump.gravity_scale.max(1e-3);
        cfg.jump.height =
            params.jump.jump_z_velocity * params.jump.jump_z_velocity / (2.0 * 9.81 * gravity_scale);
        cfg.crouch.float_offset = -(params.crouch.standing_capsule_half_height
            - params.crouch.crouched_capsule_half_height);
    }
}

fn locomotion_drive_system(
    mut q: Query<
        (
            &mut TnuaController<AlsControlScheme>,
            &mut AlsViewYaw,
            Option<&AlsInputOverride>,
            &AlsInputCommand,
            &Stance,
            &RotationMode,
        ),
        With<AlsCharacter>,
    >,
) {
    for (mut controller, mut view_yaw, override_cmd, cmd, stance, rotation_mode) in &mut q {
        controller.initiate_action_feeding();
        let effective: &AlsInputCommand = override_cmd.map(|o| &o.0).unwrap_or(cmd);

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

        if effective.jump_pressed {
            controller.action(AlsControlScheme::Jump(TnuaBuiltinJump::default()));
        } else if effective.jump_held && controller.action_flow_status().ongoing().is_some() {
            controller.prolong_action();
        }
    }
}

fn capsule_reshape_system(
    mut commands: Commands,
    q: Query<(Entity, &Stance), (With<AlsCharacter>, Changed<Stance>)>,
    handles: Query<&crate::character::AlsMovementParamsHandle>,
    params_assets: Res<Assets<AlsMovementParams>>,
) {
    for (entity, stance) in q.iter() {
        let Some(params) = handles.iter().next().and_then(|h| params_assets.get(h.id())) else {
            continue;
        };
        commands.entity(entity).insert(capsule_for(params, *stance));
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
