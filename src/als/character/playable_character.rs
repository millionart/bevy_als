use avian3d::prelude::PhysicsPlugins;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use crate::als::aim::AlsAimPlugin;
use crate::als::character::AlsCharacterPlugin;
use crate::als::control_scheme::AlsControlScheme;
use crate::als::input::AlsInputPlugin;
use crate::als::input_request::AlsInputRequestPlugin;
use crate::als::motion::AlsMotionPlugin;
use crate::als::transition::AlsStateTransitionPlugin;
use crate::als_camera::AlsCameraPlugin;
use crate::extensions::{AlsExtPlugin, RagdollCollisionHooks};

use crate::character_runtime::AlsCharacterRuntimePlugin;

pub use crate::character_assembly::{
    ALS_LINKED_ANIMATED_SCENE, AVATAR_MESH_ROOT_WORLD_Y, AlsCharacterAvatar, AlsCharacterBlueprint,
    spawn_als_character,
};

pub struct AlsPlayableCharacterPlugin;

impl Plugin for AlsPlayableCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            PhysicsPlugins::new(PostUpdate).with_collision_hooks::<RagdollCollisionHooks>(),
        );
        app.add_plugins((
            TnuaAvian3dPlugin::new(Update),
            TnuaControllerPlugin::<AlsControlScheme>::new(Update),
            EnhancedInputPlugin,
            AlsInputPlugin,
            AlsInputRequestPlugin,
            AlsAimPlugin,
            AlsStateTransitionPlugin,
            AlsCharacterPlugin,
            AlsMotionPlugin,
            crate::als::air::AlsAirPlugin,
            AnimationGraphPlugin::from_physics_schedule(PostUpdate),
            AlsExtPlugin,
            AlsCharacterRuntimePlugin,
            AlsCameraPlugin,
        ))
        .add_plugins(crate::extensions::post_physics_animation::PostPhysicsAnimationPlugin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playable_character_samples_air_velocity_and_refreshes_the_sensor_before_tnua() {
        use bevy::ecs::schedule::LogLevel;
        use bevy_tnua_avian3d::TnuaAvian3dSensorShape;

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy::input::InputPlugin,
            bevy::gizmos::GizmoPlugin,
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_plugins(AlsPlayableCharacterPlugin);
        app.finish();
        app.cleanup();
        app.world_mut().schedule_scope(Update, |world, schedule| {
            let mut settings = schedule.get_build_settings();
            settings.ambiguity_detection = LogLevel::Warn;
            schedule.set_build_settings(settings);
            schedule.initialize(world).unwrap();
            let systems: std::collections::HashMap<_, _> = schedule
                .systems()
                .unwrap()
                .map(|(id, system)| (id, system.name().to_string()))
                .collect();
            let shape = world.component_id::<TnuaAvian3dSensorShape>().unwrap();
            let impact = world
                .component_id::<crate::als::air::AlsPreImpactVelocity>()
                .unwrap();
            let conflicts: Vec<_> = schedule
                .graph()
                .conflicting_systems()
                .0
                .iter()
                .filter(|(_, _, components)| {
                    components.contains(&shape) || components.contains(&impact)
                })
                .map(|(a, b, _)| (&systems[a], &systems[b]))
                .collect();
            assert!(
                conflicts.is_empty(),
                "Unordered sensor shape or pre-impact accesses: {conflicts:?}"
            );
        });
    }

    #[test]
    fn playable_character_animation_phases_initialize_with_the_real_physics_schedule() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy::input::InputPlugin,
            bevy::gizmos::GizmoPlugin,
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_plugins(AlsPlayableCharacterPlugin);
        app.finish();
        app.cleanup();
        app.world_mut()
            .schedule_scope(PostUpdate, |world, schedule| {
                schedule
                    .initialize(world)
                    .expect("playable character animation phases must initialize");
            });
    }
}
