use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;

use als_camera::{AlsCameraRig, AlsCameraTarget};
use als_core::character::AlsCharacterBundle;
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::input::{AlsInputMap, spawn_als_actions};
use als_core::motion::AlsViewYaw;
use als_core::params::AlsMovementParams;
use als_core::view::AlsViewPitch;
use als_ext::overlay_materials::AvatarMaterialOverride;

pub const AVATAR_FORWARD_YAW: f32 = std::f32::consts::PI;
pub const AVATAR_CAPSULE_BOTTOM_OFFSET: f32 = -0.92;
pub const ALS_CHARACTER_MATERIAL_LINEAR_RGB: [f32; 3] = [0.2, 0.2, 1.0];
pub const ALS_CHARACTER_MATERIAL_SLOT_COUNT: usize = 15;

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

pub fn attach_avatar_with_material(
    commands: &mut Commands,
    assets: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    character: Entity,
    scene: &str,
    slot_colors: &[Color],
) -> Entity {
    let handles: Vec<Handle<StandardMaterial>> = slot_colors
        .iter()
        .map(|c| {
            materials.add(StandardMaterial {
                base_color: *c,
                perceptual_roughness: 0.7,
                ..default()
            })
        })
        .collect();
    let avatar = commands
        .spawn((
            AnimatedSceneHandle::new(assets.load(scene.to_owned())),
            AvatarMaterialOverride(handles),
            avatar_transform(),
        ))
        .id();
    commands.entity(character).add_child(avatar);
    avatar
}

pub fn make_als_character_material(
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::linear_rgb(
            ALS_CHARACTER_MATERIAL_LINEAR_RGB[0],
            ALS_CHARACTER_MATERIAL_LINEAR_RGB[1],
            ALS_CHARACTER_MATERIAL_LINEAR_RGB[2],
        ),
        perceptual_roughness: 0.5,
        ..default()
    })
}

pub fn attach_als_character_avatar(
    commands: &mut Commands,
    assets: &AssetServer,
    character: Entity,
    scene: &str,
    material: Handle<StandardMaterial>,
) -> Entity {
    let avatar = commands
        .spawn((
            AnimatedSceneHandle::new(assets.load(scene.to_owned())),
            AvatarMaterialOverride(vec![material; ALS_CHARACTER_MATERIAL_SLOT_COUNT]),
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
    materials: &mut Assets<StandardMaterial>,
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
    let character_material = make_als_character_material(materials);
    attach_als_character_avatar(
        commands,
        assets,
        character,
        blueprint.animscn,
        character_material,
    );
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
    use super::*;

    #[test]
    fn als_character_material_matches_the_instance_color() {
        let mut materials = Assets::<StandardMaterial>::default();
        let handle = make_als_character_material(&mut materials);
        let material = materials.get(&handle).unwrap();
        let color = material.base_color.to_linear();
        assert_eq!(ALS_CHARACTER_MATERIAL_SLOT_COUNT, 15);
        assert_eq!(color.red, ALS_CHARACTER_MATERIAL_LINEAR_RGB[0]);
        assert_eq!(color.green, ALS_CHARACTER_MATERIAL_LINEAR_RGB[1]);
        assert_eq!(color.blue, ALS_CHARACTER_MATERIAL_LINEAR_RGB[2]);
        assert_eq!(color.alpha, 1.0);
        assert_eq!(material.perceptual_roughness, 0.5);
    }
}
