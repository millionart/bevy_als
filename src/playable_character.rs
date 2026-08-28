use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_camera::AlsCameraPlugin;
use als_core::aim::AlsAimPlugin;
use als_core::character::AlsCharacterPlugin;
use als_core::control_scheme::AlsControlScheme;
use als_core::input::AlsInputPlugin;
use als_core::input_request::AlsInputRequestPlugin;
use als_core::motion::AlsMotionPlugin;
use als_core::transition::AlsStateTransitionPlugin;
use als_ext::AlsExtPlugin;

use crate::character_runtime::AlsCharacterRuntimePlugin;

pub use crate::character_assembly::{
    ALS_LINKED_ANIMATED_SCENE, AVATAR_MESH_ROOT_WORLD_Y, AlsCharacterAvatar, AlsCharacterBlueprint,
    spawn_als_character,
};

pub struct AlsPlayableCharacterPlugin;

impl Plugin for AlsPlayableCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PhysicsPlugins::new(FixedPostUpdate),
            TnuaAvian3dPlugin::new(FixedUpdate),
            TnuaControllerPlugin::<AlsControlScheme>::new(FixedUpdate),
            EnhancedInputPlugin,
            AlsInputPlugin,
            AlsInputRequestPlugin,
            AlsAimPlugin,
            AlsStateTransitionPlugin,
            AlsCharacterPlugin,
            AlsMotionPlugin,
            als_core::air::AlsAirPlugin,
            AnimationGraphPlugin::default(),
            AlsExtPlugin,
            AlsCharacterRuntimePlugin,
            AlsCameraPlugin,
        ));
    }
}
