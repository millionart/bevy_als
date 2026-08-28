use avian3d::prelude::{
    Collider, LinearVelocity, LockedAxes, PhysicsSystems, Position, RigidBody, SweptCcd,
};
use bevy::asset::Handle;
use bevy::prelude::*;
use bevy_tnua::TnuaController;
use bevy_tnua::controller::TnuaConfig;
use bevy_tnua_avian3d::TnuaAvian3dSensorShape;

use crate::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use crate::input::AlsInputCommand;
use crate::params::AlsMovementParams;
use crate::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use crate::transition::AlsStateRequest;

#[derive(Component, Debug, Default, Clone)]
pub struct AlsMovementParamsHandle(pub Handle<AlsMovementParams>);

impl std::ops::Deref for AlsMovementParamsHandle {
    type Target = Handle<AlsMovementParams>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
#[require(crate::motion::AlsViewYaw)]
#[require(AlsCrushRecovery)]
pub struct AlsCharacter;

pub const ALS_CHARACTER_CAPSULE_HALF_HEIGHT: f32 = 0.9;
pub const ALS_CHARACTER_CAPSULE_RADIUS: f32 = 0.3;
pub const ALS_CHARACTER_FLOOR_CLEARANCE: f32 = 0.019;
pub const ALS_CHARACTER_TNUA_FLOAT_EPSILON: f32 = 0.005;
const CRUSH_RECOVERY_SKIN: f32 = 0.001;

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsCrushRecoveryFloor {
    pub half_extents: Vec3,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsCrushRecovery {
    pub corrections: u32,
    pub max_penetration: f32,
}

pub fn als_character_capsule(half_height: f32, radius: f32) -> Collider {
    Collider::capsule(radius, (2.0 * (half_height - radius)).max(0.0))
}

pub struct AlsCharacterPlugin;

impl Plugin for AlsCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<AlsMovementParams>()
            .init_asset::<AlsControlSchemeConfig>()
            .add_systems(
                FixedPostUpdate,
                character_crush_recovery_system.in_set(PhysicsSystems::Last),
            );
    }
}

fn character_half_height(params: &AlsMovementParams, stance: Stance) -> f32 {
    match stance {
        Stance::Standing => params.crouch.standing_capsule_half_height,
        Stance::Crouching => params.crouch.crouched_capsule_half_height,
    }
}

fn character_crush_recovery_system(
    floors: Query<(&GlobalTransform, &AlsCrushRecoveryFloor)>,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut characters: Query<
        (
            &mut Position,
            &mut Transform,
            &mut LinearVelocity,
            &Stance,
            &AlsMovementParamsHandle,
            &mut AlsCrushRecovery,
        ),
        With<AlsCharacter>,
    >,
) {
    for (mut position, mut transform, mut velocity, stance, params_handle, mut recovery) in
        &mut characters
    {
        let half_height = params_assets
            .get(params_handle.0.id())
            .map(|params| character_half_height(params, *stance))
            .unwrap_or(ALS_CHARACTER_CAPSULE_HALF_HEIGHT);
        let capsule_bottom = position.0.y - half_height;
        let mut recovery_top = None;
        for (floor_transform, floor) in &floors {
            let transform = floor_transform.compute_transform();
            let local_position = transform.to_matrix().inverse().transform_point3(position.0);
            if local_position.x.abs() > floor.half_extents.x
                || local_position.z.abs() > floor.half_extents.z
            {
                continue;
            }
            let top = transform
                .to_matrix()
                .transform_point3(Vec3::Y * floor.half_extents.y)
                .y;
            if capsule_bottom < top {
                recovery_top = Some(recovery_top.map_or(top, |current: f32| current.max(top)));
            }
        }
        let Some(top) = recovery_top else {
            continue;
        };
        let penetration = top - capsule_bottom;
        if penetration <= 0.0 {
            continue;
        }
        let corrected_y = top + half_height + CRUSH_RECOVERY_SKIN;
        position.0.y = corrected_y;
        transform.translation.y = corrected_y;
        velocity.0.y = velocity.0.y.max(0.0);
        recovery.corrections += 1;
        recovery.max_penetration = recovery.max_penetration.max(penetration);
    }
}

#[derive(Bundle)]
pub struct AlsCharacterBundle {
    pub character: AlsCharacter,
    pub transform: Transform,
    pub rigid_body: RigidBody,
    pub collider: Collider,
    pub sensor_shape: TnuaAvian3dSensorShape,
    pub locked_axes: LockedAxes,
    pub swept_ccd: SweptCcd,
    pub controller: TnuaController<AlsControlScheme>,
    pub tnua_config: TnuaConfig<AlsControlScheme>,
    pub params: AlsMovementParamsHandle,
    pub input_command: AlsInputCommand,
    pub locomotion_state: LocomotionState,
    pub rotation_mode: RotationMode,
    pub stance: Stance,
    pub gait: Gait,
    pub overlay_state: OverlayState,
    pub request: AlsStateRequest,
}

impl AlsCharacterBundle {
    pub fn new(
        params: Handle<AlsMovementParams>,
        tnua_config: Handle<AlsControlSchemeConfig>,
    ) -> Self {
        Self {
            character: AlsCharacter,
            transform: Transform::default(),
            rigid_body: RigidBody::Dynamic,
            collider: als_character_capsule(
                ALS_CHARACTER_CAPSULE_HALF_HEIGHT,
                ALS_CHARACTER_CAPSULE_RADIUS,
            ),
            sensor_shape: TnuaAvian3dSensorShape(Collider::cylinder(
                ALS_CHARACTER_CAPSULE_RADIUS - 0.01,
                0.0,
            )),
            locked_axes: LockedAxes::ROTATION_LOCKED,
            swept_ccd: SweptCcd::LINEAR,
            controller: TnuaController::default(),
            tnua_config: TnuaConfig(tnua_config),
            params: AlsMovementParamsHandle(params),
            input_command: AlsInputCommand::default(),
            locomotion_state: LocomotionState::Grounded,
            rotation_mode: RotationMode::ViewDirection,
            stance: Stance::Standing,
            gait: Gait::Running,
            overlay_state: OverlayState::default(),
            request: AlsStateRequest::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_initial_state_matches_b_als_character() {
        let bundle = AlsCharacterBundle::new(
            Handle::<AlsMovementParams>::default(),
            Handle::<AlsControlSchemeConfig>::default(),
        );
        assert_eq!(bundle.rotation_mode, RotationMode::ViewDirection);
        assert_eq!(bundle.gait, Gait::Running);
        assert_eq!(bundle.stance, Stance::Standing);
    }
}
