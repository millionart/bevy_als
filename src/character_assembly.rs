use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;

use als_camera::{
    AlsCameraFieldOfView,
    AlsCameraSocketBinding, AlsCameraSocketLocations,
    AlsCameraTarget,
};
use als_core::character::AlsCharacterBundle;
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::input::{AlsInputMap, spawn_als_actions};
use als_core::motion::{AlsViewYaw, initial_als_control_scheme_config};
use als_core::orientation::world_forward_to_yaw;
use als_core::params::AlsMovementParams;
use als_core::view::AlsViewPitch;
use als_ext::overlay_materials::{AVATAR_MAIN_MATERIAL_SLOT, AvatarMaterialOverride};

use crate::animation_events::{
    AlsAnimationEventPlaybacks, AlsNotifyEarlyBlendOut, AlsNotifyLocomotionAction,
    AlsNotifyRootMotionScale, AlsStandingEventClock,
};

pub const AVATAR_FORWARD_YAW: f32 = std::f32::consts::PI;
pub const AVATAR_CAPSULE_BOTTOM_OFFSET: f32 = -0.92;
pub const AVATAR_MESH_ROOT_WORLD_Y: f32 = -0.02;
pub const ALS_LINKED_ANIMATED_SCENE: &str = "animated_scenes/als_generated_ab_als.animscn.ron";
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

pub use als_camera::AlsCharacterCamera;

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

pub(crate) fn sync_avatar_stance_offset(
    params_assets: Res<Assets<AlsMovementParams>>,
    characters: Query<(Ref<als_core::state::Stance>, &als_core::character::AlsMovementParamsHandle), With<als_core::character::AlsCharacter>>,
    mut avatars: Query<(Ref<AlsCharacterAvatar>, &ChildOf, &mut Transform)>,
) {
    for (avatar, parent, mut transform) in &mut avatars {
        let Ok((stance, handle)) = characters.get(parent.parent()) else { continue };
        if !avatar.is_added() && !stance.is_changed() { continue; }
        let Some(params) = params_assets.get(handle.id()) else { continue };
        transform.translation.y = AVATAR_CAPSULE_BOTTOM_OFFSET + if *stance == als_core::state::Stance::Crouching {
            params.crouch.standing_capsule_half_height - params.crouch.crouched_capsule_half_height
        } else { 0.0 };
    }
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
    spawn_als_character_internal(
        commands,
        assets,
        None,
        params_assets,
        config_assets,
        blueprint,
    )
}

pub fn spawn_als_character_with_materials(
    commands: &mut Commands,
    assets: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    params_assets: &mut Assets<AlsMovementParams>,
    config_assets: &mut Assets<AlsControlSchemeConfig>,
    blueprint: AlsCharacterBlueprint,
) -> Entity {
    spawn_als_character_internal(
        commands,
        assets,
        Some(materials),
        params_assets,
        config_assets,
        blueprint,
    )
}

fn spawn_als_character_internal(
    commands: &mut Commands,
    assets: &AssetServer,
    materials: Option<&mut Assets<StandardMaterial>>,
    params_assets: &mut Assets<AlsMovementParams>,
    config_assets: &mut Assets<AlsControlSchemeConfig>,
    blueprint: AlsCharacterBlueprint,
) -> Entity {
    let params_value = AlsMovementParams::default();
    let tnua_config_handle = config_assets.add(initial_als_control_scheme_config(&params_value));
    let params = params_assets.add(params_value);
    let mut bundle = AlsCharacterBundle::new(params, tnua_config_handle);
    bundle.transform = blueprint.transform;
    let fwd = bundle.transform.forward();
    let character = commands
        .spawn(bundle)
        .insert((
            AlsViewYaw(world_forward_to_yaw(*fwd)),
            AlsViewPitch(0.0),
            AlsCameraTarget,
            als_camera::AlsCameraCurveValues::default(),
            als_camera::AlsCameraAnimationFrame::default(),
            als_camera::AlsCameraDefinitionHandle(assets.load("params/als_camera.camera_component.ron")),
            AlsStandingEventClock::default(),
            AlsAnimationEventPlaybacks::default(),
            AlsNotifyLocomotionAction::None,
            AlsNotifyRootMotionScale::default(),
            AlsNotifyEarlyBlendOut::default(),
        ))
        .id();
    if blueprint.spawn_input {
        spawn_als_actions(commands, character, &blueprint.input_map);
        commands.entity(character).insert(AlsPlayerCharacter);
    }
    let avatar = attach_avatar(commands, assets, character, ALS_LINKED_ANIMATED_SCENE);
    let camera_material = if let Some(materials) = materials {
        let character_material = make_als_character_material(materials);
        let main_material = make_als_main_material(materials);
        commands
            .entity(avatar)
            .insert(AvatarMaterialOverride(als_character_material_slots(
                character_material,
                main_material.clone(),
            )));
        Some(main_material)
    } else { None };
    commands.entity(character).insert((
        AlsCameraSocketBinding::new(avatar),
        AlsCameraSocketLocations::default(),
    ));
    let mut component = commands.spawn((
        AnimatedSceneHandle::new(assets.load("animated_scenes/als_camera.static.animscn.ron")),
        crate::camera_component::AlsSourceCameraComponent::default(),
        AlsCharacterCamera { character },
        Name::new("Camera"),
        ChildOf(avatar),
        Visibility::Hidden,
    ));
    if let Some(material) = camera_material { component.insert(AvatarMaterialOverride(vec![material])); }
    let component_entity = component.id();
    let camera = if blueprint.spawn_camera {
        let eye = blueprint.transform.translation + Vec3::new(0.0, 1.8, 5.0);
        Some(
            commands
                .spawn((
                    Camera3d::default(),
                    Transform::from_translation(eye)
                        .looking_at(blueprint.transform.translation + Vec3::Y, Vec3::Y),
                    als_camera::AlsCameraViewSource(component_entity),
                    AlsCameraFieldOfView::default(),
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
                            spawn_als_character_with_materials(
                                &mut commands,
                                &assets,
                                &mut materials,
                                &mut params_assets,
                                &mut config_assets,
                                AlsCharacterBlueprint {
                                    spawn_camera,
                                    ..AlsCharacterBlueprint::new(Transform::IDENTITY)
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
        let blueprint = AlsCharacterBlueprint::new(Transform::IDENTITY);
        assert!(blueprint.spawn_input);
        assert!(!blueprint.spawn_camera);
    }

    #[test]
    fn avatar_uses_the_source_capsule_relative_mesh_offset() {
        let transform = avatar_transform();
        assert!((transform.translation.y - AVATAR_CAPSULE_BOTTOM_OFFSET).abs() < f32::EPSILON);
        assert!((AVATAR_MESH_ROOT_WORLD_Y - (-0.02)).abs() < f32::EPSILON);
    }

    #[test]
    fn stance_changes_preserve_the_mesh_world_origin() {
        use als_core::{character::AlsCharacter, state::Stance};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::transform::TransformPlugin, crate::stance_layer::StanceLayerPlugin))
            .init_resource::<Assets<AlsMovementParams>>();
        let params = AlsMovementParams::default();
        let handle = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(params.clone());
        let actors = [0.0, 2.0].map(|floor| {
            let character = app.world_mut().spawn((
                AlsCharacter, Stance::Standing,
                als_core::character::AlsMovementParamsHandle(handle.clone()),
                Transform::from_xyz(floor, floor + params.crouch.standing_capsule_half_height, 0.0),
            )).id();
            let avatar = app.world_mut().spawn((AlsCharacterAvatar { character }, ChildOf(character), avatar_transform())).id();
            (character, avatar, floor)
        });
        for stance in [Stance::Standing, Stance::Crouching, Stance::Standing, Stance::Crouching] {
            for (character, _, floor) in actors {
                app.world_mut().entity_mut(character).insert(stance);
                app.world_mut().get_mut::<Transform>(character).unwrap().translation.y = floor + if stance == Stance::Crouching { params.crouch.crouched_capsule_half_height } else { params.crouch.standing_capsule_half_height };
            }
            app.update();
            let observed = actors.map(|(_, avatar, floor)| app.world().get::<GlobalTransform>(avatar).unwrap().translation().y - floor);
            assert!(observed.iter().all(|height| (*height - AVATAR_MESH_ROOT_WORLD_Y).abs() < 1e-6), "{stance:?}: {observed:?}");
        }
        let avatar = actors[0].1;
        let original = app.world().get::<Transform>(avatar).unwrap().translation;
        app.world_mut().get_mut::<Transform>(avatar).unwrap().translation.y += 0.2;
        app.update();
        assert!((app.world().get::<Transform>(avatar).unwrap().translation.y - original.y - 0.2).abs() < 1e-6);
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
        app.init_asset::<als_camera::AlsCameraDefinition>();
        let character = spawn_test_character(&mut app, true);
        let assembly = *app.world().get::<AlsCharacterAssembly>(character).unwrap();
        assert!(app.world().get::<AlsPlayerCharacter>(character).is_some());
        assert!(app.world().get::<AlsCameraTarget>(character).is_some());
        let source_camera = app.world_mut().query_filtered::<(Entity, &ChildOf, &AlsCharacterCamera), With<crate::camera_component::AlsSourceCameraComponent>>()
            .iter(app.world()).find(|(_, _, owner)| owner.character == character).map(|(entity, parent, _)| (entity, parent.parent())).unwrap();
        assert_eq!(source_camera.1, assembly.avatar);
        assert!(app.world().get::<avian3d::prelude::Collider>(source_camera.0).is_none());
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
        assert!(app.world().get::<als_camera::AlsCameraRig>(source_camera.0).is_some());
        assert_eq!(app.world().get::<als_camera::AlsCameraViewSource>(camera).unwrap().0, source_camera.0);
    }
}
