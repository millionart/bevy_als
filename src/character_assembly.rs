use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;

use als_camera::{AlsCameraRig, AlsCameraTarget};
use als_core::character::AlsCharacterBundle;
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::input::{AlsInputMap, spawn_als_actions};
use als_core::motion::AlsViewYaw;
use als_core::params::AlsMovementParams;
use als_core::view::AlsViewPitch;
use als_ext::overlay_materials::{AVATAR_MAIN_MATERIAL_SLOT, AvatarMaterialOverride};

pub const AVATAR_FORWARD_YAW: f32 = std::f32::consts::PI;
pub const AVATAR_CAPSULE_BOTTOM_OFFSET: f32 = -0.92;
pub const ALS_CHARACTER_MATERIAL_LINEAR_RGB: [f32; 3] = [0.2, 0.2, 1.0];
pub const ALS_CHARACTER_MAIN_MATERIAL_LINEAR_RGB: [f32; 3] = [1.0, 1.0, 1.0];
pub const ALS_CHARACTER_MATERIAL_PERCEPTUAL_ROUGHNESS: f32 = 1.0;
pub const ALS_CHARACTER_MATERIAL_REFLECTANCE: f32 = 0.5;
pub const ALS_CHARACTER_MATERIAL_SLOT_COUNT: usize = 15;
pub const ALS_CHARACTER_EYES_MATERIAL_SLOT: usize = AVATAR_MAIN_MATERIAL_SLOT;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsCharacterAvatar {
    pub character: Entity,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsCharacterCamera {
    pub character: Entity,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsPlayerCharacter;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsCharacterAssembly {
    pub avatar: Entity,
    pub camera: Option<Entity>,
}

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
            AlsCharacterAvatar { character },
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
            AlsCharacterAvatar { character },
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
        perceptual_roughness: ALS_CHARACTER_MATERIAL_PERCEPTUAL_ROUGHNESS,
        reflectance: ALS_CHARACTER_MATERIAL_REFLECTANCE,
        ..default()
    })
}

pub fn make_als_main_material(
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::linear_rgb(
            ALS_CHARACTER_MAIN_MATERIAL_LINEAR_RGB[0],
            ALS_CHARACTER_MAIN_MATERIAL_LINEAR_RGB[1],
            ALS_CHARACTER_MAIN_MATERIAL_LINEAR_RGB[2],
        ),
        perceptual_roughness: ALS_CHARACTER_MATERIAL_PERCEPTUAL_ROUGHNESS,
        reflectance: ALS_CHARACTER_MATERIAL_REFLECTANCE,
        ..default()
    })
}

pub fn als_character_material_slots(
    body_material: Handle<StandardMaterial>,
    main_material: Handle<StandardMaterial>,
) -> Vec<Handle<StandardMaterial>> {
    let mut slots = vec![body_material; ALS_CHARACTER_MATERIAL_SLOT_COUNT];
    slots[ALS_CHARACTER_EYES_MATERIAL_SLOT] = main_material;
    slots
}

pub fn attach_als_character_avatar(
    commands: &mut Commands,
    assets: &AssetServer,
    character: Entity,
    scene: &str,
    body_material: Handle<StandardMaterial>,
    main_material: Handle<StandardMaterial>,
) -> Entity {
    let avatar = commands
        .spawn((
            AnimatedSceneHandle::new(assets.load(scene.to_owned())),
            AvatarMaterialOverride(als_character_material_slots(body_material, main_material)),
            AlsCharacterAvatar { character },
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
        commands.entity(character).insert(AlsPlayerCharacter);
    }
    let character_material = make_als_character_material(materials);
    let main_material = make_als_main_material(materials);
    let avatar = attach_als_character_avatar(
        commands,
        assets,
        character,
        blueprint.animscn,
        character_material,
        main_material,
    );
    let camera = if blueprint.spawn_camera {
        let eye = blueprint.transform.translation + Vec3::new(0.0, 1.8, 5.0);
        Some(
            commands
                .spawn((
                    Camera3d::default(),
                    Transform::from_translation(eye)
                        .looking_at(blueprint.transform.translation + Vec3::Y, Vec3::Y),
                    AlsCameraRig::default(),
                    AlsCharacterCamera { character },
                ))
                .id(),
        )
    } else {
        None
    };
    commands
        .entity(character)
        .insert(AlsCharacterAssembly { avatar, camera });
    character
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy_animation_graph::core::animated_scene::AnimatedScene;

    fn spawn_test_character(app: &mut App, spawn_camera: bool) -> Entity {
        let assets = app.world().resource::<AssetServer>().clone();
        let character = app.world_mut().resource_scope(
            |world, mut materials: Mut<Assets<StandardMaterial>>| {
                world.resource_scope(|world, mut params_assets: Mut<Assets<AlsMovementParams>>| {
                    world.resource_scope(
                        |world, mut config_assets: Mut<Assets<AlsControlSchemeConfig>>| {
                            let mut commands = world.commands();
                            spawn_als_character(
                                &mut commands,
                                &assets,
                                &mut materials,
                                &mut params_assets,
                                &mut config_assets,
                                AlsCharacterBlueprint {
                                    spawn_camera,
                                    ..AlsCharacterBlueprint::new(
                                        Transform::IDENTITY,
                                        "test.animscn.ron",
                                    )
                                },
                            )
                        },
                    )
                })
            },
        );
        app.world_mut().flush();
        character
    }

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
        assert_eq!(
            material.perceptual_roughness,
            ALS_CHARACTER_MATERIAL_PERCEPTUAL_ROUGHNESS
        );
        assert_eq!(material.reflectance, ALS_CHARACTER_MATERIAL_REFLECTANCE);
    }

    #[test]
    fn eyes_slot_uses_main_material() {
        let mut materials = Assets::<StandardMaterial>::default();
        let body = make_als_character_material(&mut materials);
        let main = make_als_main_material(&mut materials);
        let slots = als_character_material_slots(body.clone(), main.clone());
        assert_eq!(slots.len(), ALS_CHARACTER_MATERIAL_SLOT_COUNT);
        assert_eq!(slots[ALS_CHARACTER_EYES_MATERIAL_SLOT], main);
        assert!(
            slots
                .iter()
                .enumerate()
                .filter(|(slot, _)| *slot != ALS_CHARACTER_EYES_MATERIAL_SLOT)
                .all(|(_, material)| *material == body)
        );
    }

    #[test]
    fn default_blueprint_requests_the_local_player_input_path() {
        let blueprint = AlsCharacterBlueprint::new(Transform::IDENTITY, "test.animscn.ron");
        assert!(blueprint.spawn_input);
        assert!(!blueprint.spawn_camera);
    }

    #[test]
    fn assembly_records_avatar_and_optional_camera_relationships() {
        let avatar = Entity::from_raw_u32(17).unwrap();
        let camera = Entity::from_raw_u32(23).unwrap();
        let with_camera = AlsCharacterAssembly {
            avatar,
            camera: Some(camera),
        };
        let without_camera = AlsCharacterAssembly {
            avatar,
            camera: None,
        };
        assert_eq!(with_camera.avatar, avatar);
        assert_eq!(with_camera.camera, Some(camera));
        assert_eq!(without_camera.camera, None);
    }

    #[test]
    fn spawned_character_has_explicit_avatar_camera_and_input_ownership() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<StandardMaterial>()
            .init_asset::<AlsMovementParams>()
            .init_asset::<AlsControlSchemeConfig>()
            .init_asset::<AnimatedScene>();
        let character = spawn_test_character(&mut app, true);
        let assembly = *app.world().get::<AlsCharacterAssembly>(character).unwrap();
        assert!(app.world().get::<AlsPlayerCharacter>(character).is_some());
        assert!(app.world().get::<AlsCameraTarget>(character).is_some());
        assert_eq!(
            app.world()
                .get::<AlsCharacterAvatar>(assembly.avatar)
                .unwrap()
                .character,
            character
        );
        let camera = assembly.camera.unwrap();
        assert_eq!(
            app.world()
                .get::<AlsCharacterCamera>(camera)
                .unwrap()
                .character,
            character
        );
        assert!(app.world().get::<Camera3d>(camera).is_some());
        assert!(app.world().get::<AlsCameraRig>(camera).is_some());
    }
}
