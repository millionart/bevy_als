use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;
use bevy_animation_graph::core::plugin::AnimationGraphSet;

use als_camera::{AlsCameraRig, AlsCameraTarget};
use als_core::character::{AlsCharacter, AlsCharacterBundle};
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::input::{AlsInputMap, spawn_als_actions};
use als_core::motion::AlsViewYaw;
use als_core::params::AlsMovementParams;
use als_core::state::LocomotionState;
use als_core::view::AlsViewPitch;

pub const AVATAR_FORWARD_YAW: f32 = std::f32::consts::PI;
pub const AVATAR_CAPSULE_BOTTOM_OFFSET: f32 = -0.92;
pub const AVATAR_MESH_ROOT_WORLD_Y: f32 = -0.02;
pub const ALS_LINKED_ANIMATED_SCENE: &str = "animated_scenes/als_ragdoll.animscn.ron";

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AlsCharacterAvatar;

pub fn avatar_transform() -> Transform {
    Transform::from_xyz(0.0, AVATAR_CAPSULE_BOTTOM_OFFSET, 0.0)
        .with_rotation(Quat::from_rotation_y(AVATAR_FORWARD_YAW))
}

pub struct AvatarGroundCompensationPlugin;

impl Plugin for AvatarGroundCompensationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            avatar_ground_compensation_system
                .before(AnimationGraphSet::Final)
                .before(TransformSystems::Propagate),
        );
    }
}

type CompensatedAvatar = (Entity, &'static ChildOf, &'static mut Transform);

fn avatar_ground_compensation_system(
    mut avatars: Query<CompensatedAvatar, (With<AlsCharacterAvatar>, Without<AlsCharacter>)>,
    characters: Query<(&Transform, &LocomotionState), With<AlsCharacter>>,
) {
    for (_entity, child_of, mut transform) in avatars.iter_mut() {
        let Ok((body, loco)) = characters.get(child_of.parent()) else {
            continue;
        };
        if matches!(loco, LocomotionState::Ragdolling) {
            continue;
        }
        let glued_offset = AVATAR_MESH_ROOT_WORLD_Y - body.translation.y;
        transform.translation.y = glued_offset.max(AVATAR_CAPSULE_BOTTOM_OFFSET);
    }
}

fn attach_avatar(
    commands: &mut Commands,
    assets: &AssetServer,
    character: Entity,
    scene: &str,
) -> Entity {
    let avatar = commands
        .spawn((
            AnimatedSceneHandle::new(assets.load(scene.to_owned())),
            avatar_transform(),
            AlsCharacterAvatar,
        ))
        .id();
    commands.entity(character).add_child(avatar);
    avatar
}

pub struct AlsCharacterBlueprint {
    pub transform: Transform,
    pub input_map: AlsInputMap,
    pub spawn_input: bool,
    pub spawn_camera: bool,
}

impl AlsCharacterBlueprint {
    pub fn new(transform: Transform) -> Self {
        Self {
            transform,
            input_map: AlsInputMap::default(),
            spawn_input: true,
            spawn_camera: false,
        }
    }
}

pub fn spawn_als_character(
    commands: &mut Commands,
    assets: &AssetServer,
    params_assets: &mut Assets<AlsMovementParams>,
    config_assets: &mut Assets<AlsControlSchemeConfig>,
    blueprint: AlsCharacterBlueprint,
) -> Entity {
    let params = params_assets.add(AlsMovementParams::default());
    let tnua_config_handle = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, tnua_config_handle);
    bundle.transform = blueprint.transform;
    let fwd = bundle.transform.forward();
    let character = commands
        .spawn(bundle)
        .insert((
            AlsViewYaw(f32::atan2(fwd.x, -fwd.z)),
            AlsViewPitch(0.0),
            AlsCameraTarget,
        ))
        .id();
    if blueprint.spawn_input {
        spawn_als_actions(commands, character, &blueprint.input_map);
    }
    attach_avatar(commands, assets, character, ALS_LINKED_ANIMATED_SCENE);
    if blueprint.spawn_camera {
        let eye = blueprint.transform.translation + Vec3::new(0.0, 1.8, 5.0);
        commands.spawn((
            Camera3d::default(),
            Transform::from_translation(eye)
                .looking_at(blueprint.transform.translation + Vec3::Y, Vec3::Y),
            AlsCameraRig::default(),
        ));
    }
    character
}

#[cfg(test)]
mod tests {
    #[test]
    fn canonical_scene_uses_linked_graph_composition() {
        let scene = include_str!("../assets/animated_scenes/als_ragdoll.animscn.ron");
        let graph = include_str!("../assets/graphs/locomotion_overlay.animgraph.ron");
        assert!(scene.contains("graphs/locomotion_overlay.animgraph.ron"));
        assert!(!scene.contains("Monolithic"));
        assert_eq!(graph.matches("als_ext::LayeredGraphNode").count(), 4);
        for layer in [
            "graphs/base_locomotion.animgraph.ron",
            "graphs/air.animgraph.ron",
            "graphs/actions.animgraph.ron",
            "graphs/overlay_view.animgraph.ron",
        ] {
            assert!(graph.contains(layer));
        }
    }
}
