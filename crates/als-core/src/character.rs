use avian3d::prelude::{Collider, RigidBody};
use bevy::asset::Handle;
use bevy::prelude::*;
use bevy_tnua::TnuaController;
use bevy_tnua::controller::TnuaConfig;

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
pub struct AlsCharacter;

pub struct AlsCharacterPlugin;

impl Plugin for AlsCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<AlsMovementParams>()
            .init_asset::<AlsControlSchemeConfig>();
    }
}

#[derive(Bundle)]
pub struct AlsCharacterBundle {
    pub character: AlsCharacter,
    pub transform: Transform,
    pub rigid_body: RigidBody,
    pub collider: Collider,
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
            collider: Collider::capsule(0.6, 0.3),
            controller: TnuaController::default(),
            tnua_config: TnuaConfig(tnua_config),
            params: AlsMovementParamsHandle(params),
            input_command: AlsInputCommand::default(),
            locomotion_state: LocomotionState::Grounded,
            rotation_mode: RotationMode::VelocityDirection,
            stance: Stance::Standing,
            gait: Gait::Walking,
            overlay_state: OverlayState::default(),
            request: AlsStateRequest::default(),
        }
    }
}
