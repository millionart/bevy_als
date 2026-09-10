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
use als_ext::{AlsExtPlugin, RagdollCollisionHooks};

use crate::character_runtime::AlsCharacterRuntimePlugin;

pub use crate::character_assembly::{
    ALS_LINKED_ANIMATED_SCENE, AVATAR_MESH_ROOT_WORLD_Y, AlsCharacterAvatar, AlsCharacterBlueprint,
    spawn_als_character,
};

pub struct AlsPlayableCharacterPlugin;

impl Plugin for AlsPlayableCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PhysicsPlugins::new(PostUpdate).with_collision_hooks::<RagdollCollisionHooks>(),
            TnuaAvian3dPlugin::new(Update),
            TnuaControllerPlugin::<AlsControlScheme>::new(Update),
            EnhancedInputPlugin,
            AlsInputPlugin,
            AlsInputRequestPlugin,
            AlsAimPlugin,
            AlsStateTransitionPlugin,
            AlsCharacterPlugin,
            AlsMotionPlugin,
            als_core::air::AlsAirPlugin,
            AnimationGraphPlugin::from_physics_schedule(PostUpdate),
            AlsExtPlugin,
            AlsCharacterRuntimePlugin,
            AlsCameraPlugin,
        )).add_plugins(als_ext::post_physics_animation::PostPhysicsAnimationPlugin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playable_character_animation_phases_initialize_with_the_real_physics_schedule() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            bevy::input::InputPlugin, bevy::gizmos::GizmoPlugin))
            .init_asset::<bevy::world_serialization::WorldAsset>()
            .init_asset::<Mesh>().init_asset::<Image>()
            .add_plugins(AlsPlayableCharacterPlugin);
        app.finish();
        app.cleanup();
        app.world_mut().schedule_scope(PostUpdate, |world, schedule| {
            schedule.initialize(world).expect("playable character animation phases must initialize");
        });
    }
}
