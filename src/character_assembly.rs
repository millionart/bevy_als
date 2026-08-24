use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;

use als_camera::{AlsCameraRig, AlsCameraTarget};
use als_core::character::AlsCharacterBundle;
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::input::{spawn_als_actions, AlsInputMap};
use als_core::motion::AlsViewYaw;
use als_core::params::AlsMovementParams;
use als_core::view::AlsViewPitch;

pub const AVATAR_FORWARD_YAW: f32 = std::f32::consts::PI;
pub const AVATAR_CAPSULE_BOTTOM_OFFSET: f32 = -0.92;

pub fn avatar_transform() -> Transform {
    Transform::from_xyz(0.0, AVATAR_CAPSULE_BOTTOM_OFFSET, 0.0)
        .with_rotation(Quat::from_rotation_y(AVATAR_FORWARD_YAW))
}

pub fn attach_avatar(
    commands: &mut Commands,
    assets: &AssetServer,
    character: Entity,
    scene: &str,
) -> Entity {
    let avatar = commands
        .spawn((
            AnimatedSceneHandle::new(assets.load(scene.to_owned())),
            avatar_transform(),
        ))
        .id();
    commands.entity(character).add_child(avatar);
    avatar
}

pub struct AlsCharacterBlueprint<'a> {
    pub transform: Transform,
    pub animscn: &'a str,
    pub input_map: AlsInputMap,
    pub spawn_input: bool,
    pub spawn_camera: bool,
}

impl<'a> AlsCharacterBlueprint<'a> {
    pub fn new(transform: Transform, animscn: &'a str) -> Self {
        Self {
            transform,
            animscn,
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
    blueprint: AlsCharacterBlueprint<'_>,
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
    attach_avatar(commands, assets, character, blueprint.animscn);
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
