use crate::als::{character::AlsCharacter, foot_ik::AlsFootBones, state::LocomotionState};
use crate::extensions::RagdollSpatialQuery as SpatialQuery;
use crate::extensions::projected_decal::{
    NoDecalReceiver, PackedDecal, PackedDecalMaterial, PhysicalSurface,
};
use crate::{
    animation_events::{AlsAnimationEvent, AlsAnimationEventSet},
    character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar},
};
use avian3d::{collider_tree::ColliderTrees, prelude::SpatialQueryFilter};
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_graph_player::AnimationGraphPlayer,
};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Clone)]
pub struct FootprintDefinition {
    pub source_settings: String,
    pub notify_settings_reference: String,
    pub trace_layer: crate::als::character::AlsCollisionLayer,
    pub trace_distance: f32,
    pub spawn_angle_cos: f32,
    pub foot_y_axes: [Vec3; 2],
    pub foot_up_axes: [Vec3; 2],
    pub notify_defaults: HashMap<String, serde_json::Value>,
    pub effects: Vec<FootprintEffect>,
    pub materials: HashMap<String, FootprintMaterial>,
}

impl FootprintDefinition {
    fn effect_for_surface(&self, surface: &str) -> Option<&FootprintEffect> {
        self.effects
            .iter()
            .find(|effect| effect.surface == surface)
            .or_else(|| self.effects.first())
    }
}

#[derive(Deserialize, Clone)]
pub struct FootprintEffect {
    pub surface: String,
    pub material: Option<String>,
    pub attached: bool,
    pub size: Vec3,
    pub location_offset: Vec3,
    pub rotation_offsets: [Quat; 2],
    pub duration: f32,
    pub fade_out_duration: f32,
}

#[derive(Deserialize, Clone)]
pub struct FootprintMaterial {
    pub base_color: f32,
    pub opacity: f32,
    pub roughness_scale: f32,
    pub metallic: f32,
    pub specular: f32,
    pub texture: String,
    #[serde(skip)]
    pub image: Handle<Image>,
}

#[derive(Asset, TypePath, Deserialize, Clone)]
pub struct FootprintLibrary {
    pub character_receives_decals: bool,
    pub definitions: Vec<FootprintDefinition>,
}

#[derive(Default, TypePath)]
struct FootprintLibraryLoader;

impl bevy::asset::AssetLoader for FootprintLibraryLoader {
    type Asset = FootprintLibrary;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _: &(),
        context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<FootprintLibrary, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let mut library: FootprintLibrary =
            serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
        let mut references = std::collections::HashSet::new();
        for definition in &mut library.definitions {
            if !references.insert(&definition.notify_settings_reference)
                || definition
                    .effects
                    .iter()
                    .filter_map(|effect| effect.material.as_ref())
                    .any(|material| !definition.materials.contains_key(material))
                || ["FootBone", "bSpawnDecal", "bSkipEffectsWhenInAir"]
                    .iter()
                    .any(|key| !definition.notify_defaults.contains_key(*key))
            {
                return Err(std::io::Error::other(
                    "Footprint settings contain duplicate references, missing materials or incomplete notify defaults",
                ));
            }
            for material in definition.materials.values_mut() {
                material.image = context
                    .load_builder()
                    .with_settings(|settings: &mut bevy::image::ImageLoaderSettings| {
                        settings.is_srgb = false;
                        settings.sampler = bevy::image::ImageSampler::linear();
                    })
                    .load(material.texture.clone());
            }
        }
        Ok(library)
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}

#[derive(Component, Debug)]
pub struct Footprint {
    pub character: Entity,
    pub foot: usize,
    pub created_at: f64,
    pub lifetime: Vec2,
    pub surface: Entity,
    pub opacity: f32,
}

#[derive(Component, Default, Debug)]
pub struct FootprintDiagnostics {
    pub notifications: u64,
    pub spawned: [u64; 2],
    pub expired: u64,
    pub rejected_air: u64,
    pub rejected_flag: u64,
    pub missed: u64,
    pub rejected_angle: u64,
    pub invalid: u64,
    pub fallback_hits: u64,
}

#[derive(Debug)]
pub struct FootstepDebugTrace {
    pub created_at: f64,
    pub start: Vec3,
    pub end: Vec3,
    pub hit: Option<Vec3>,
    pub rotation: Option<Quat>,
}

#[derive(Resource, Default)]
pub struct FootstepDebugHistory(pub Vec<FootstepDebugTrace>);

#[derive(Resource)]
struct FootprintAssets {
    library: Handle<FootprintLibrary>,
}

#[derive(SystemSet, Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct FootprintSet;

pub struct FootprintPlugin;

impl Plugin for FootprintPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<crate::debug_target::AlsDebugTargetPlugin>() {
            app.add_plugins(crate::debug_target::AlsDebugTargetPlugin);
        }
        if !app.is_plugin_added::<crate::extensions::projected_decal::PackedDecalPlugin>() {
            app.add_plugins(crate::extensions::projected_decal::PackedDecalPlugin);
        }
        app.init_asset::<FootprintLibrary>()
            .init_resource::<FootstepDebugHistory>()
            .init_asset_loader::<FootprintLibraryLoader>()
            .add_systems(Startup, load_footprint_assets)
            .add_systems(PreUpdate, configure_footprint_owners)
            .configure_sets(
                PostUpdate,
                FootprintSet
                    .after(AlsAnimationEventSet::GraphFootsteps)
                    .after(crate::control_rig_apply::AlsControlRigPropagationSet)
                    .before(crate::extensions::projected_decal::PackedDecalPrepareSet),
            )
            .add_systems(
                PostUpdate,
                (
                    expire_footprints,
                    spawn_footprints.run_if(resource_exists::<ColliderTrees>),
                )
                    .chain()
                    .in_set(FootprintSet),
            );
    }
}

fn load_footprint_assets(mut commands: Commands, server: Res<AssetServer>) {
    commands.insert_resource(FootprintAssets {
        library: server.load("ALS/Data/Footstep/footprint.runtime.json"),
    });
}

fn configure_footprint_owners(
    mut commands: Commands,
    avatars: Query<(Entity, &AlsCharacterAvatar, Has<NoDecalReceiver>)>,
    diagnostics: Query<(), With<FootprintDiagnostics>>,
    assets: Res<FootprintAssets>,
    libraries: Res<Assets<FootprintLibrary>>,
) {
    for (entity, avatar, no_decals) in &avatars {
        let Some(library) = libraries.get(&assets.library) else {
            continue;
        };
        if !library.character_receives_decals && !no_decals {
            commands.entity(entity).insert(NoDecalReceiver);
        }
        if !diagnostics.contains(avatar.character) {
            commands
                .entity(avatar.character)
                .insert(FootprintDiagnostics::default());
        }
    }
}

pub fn footprint_transform(
    settings: &FootprintDefinition,
    effect: &FootprintEffect,
    foot: GlobalTransform,
    side: usize,
    point: Vec3,
    normal: Vec3,
    mesh_scale: f32,
) -> Option<Transform> {
    let rotation = foot.rotation();
    let up = rotation * settings.foot_up_axes[side];
    if normal.dot(up) < settings.spawn_angle_cos {
        return None;
    }
    let rotation = footstep_rotation(settings, foot, side, normal)? * effect.rotation_offsets[side];
    Some(Transform {
        translation: point + rotation * (effect.location_offset * mesh_scale),
        rotation,
        scale: effect.size * (2.0 * mesh_scale),
    })
}

fn footstep_rotation(
    settings: &FootprintDefinition,
    foot: GlobalTransform,
    side: usize,
    normal: Vec3,
) -> Option<Quat> {
    let normal = normal.try_normalize()?;
    let mut tangent = (foot.rotation() * settings.foot_y_axes[side]).try_normalize()?;
    if (normal.dot(tangent).abs() - 1.0).abs() <= 1e-8 {
        tangent = if normal.y.abs() < 1.0 - 1e-4 {
            Vec3::Y
        } else {
            Vec3::X
        };
    }
    let forward = normal.cross(tangent).try_normalize()?;
    Some(Quat::from_mat3(&Mat3::from_cols(
        forward,
        normal,
        forward.cross(normal),
    )))
}

pub fn footprint_opacity(age: f64, lifetime: Vec2) -> f32 {
    if age <= lifetime.x as f64 {
        return 1.0;
    }
    if lifetime.y <= 0.0 {
        return 1.0;
    }
    (1.0 - ((age - lifetime.x as f64) / lifetime.y as f64) as f32).clamp(0.0, 1.0)
}

fn spawn_footprints(
    mut commands: Commands,
    mut events: MessageReader<AlsAnimationEvent>,
    assets: Res<FootprintAssets>,
    libraries: Res<Assets<FootprintLibrary>>,
    time: Res<Time>,
    spatial: SpatialQuery,
    mut characters: Query<
        (
            &AlsFootBones,
            &LocomotionState,
            &AlsCharacterAssembly,
            &mut FootprintDiagnostics,
            Has<crate::debug_target::AlsDebugTarget>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    players: Query<&AnimationGraphPlayer>,
    globals: Query<&GlobalTransform>,
    surfaces: Query<&PhysicalSurface>,
    collider_owners: Query<&avian3d::prelude::ColliderOf>,
    mut materials: ResMut<Assets<PackedDecalMaterial>>,
    mut debug: ResMut<FootstepDebugHistory>,
    show_debug: Option<Res<crate::show_debug_hud::AlsShowDebugHudState>>,
) {
    let Some(library) = libraries.get(&assets.library) else {
        return;
    };
    for event in events.read().filter(|event| {
        event.class_name == "AlsAnimNotify_FootstepEffects"
            && event.phase == crate::extensions::EventPhase::Point
    }) {
        let Ok((bones, locomotion, assembly, mut diagnostics, player)) =
            characters.get_mut(event.character)
        else {
            continue;
        };
        let Some((source_avatar, source_player, source_instance)) = event.source else {
            continue;
        };
        if source_avatar != assembly.avatar {
            continue;
        }
        let Ok((owner, instance)) = avatars.get(source_avatar) else {
            continue;
        };
        if owner.character != event.character || instance.player_entity() != source_player {
            continue;
        }
        let Ok(source_player) = players.get(source_player) else {
            continue;
        };
        if source_player
            .get_context_arena()
            .map_or(0, crate::animation_events::notify_root_instance)
            != source_instance
        {
            continue;
        }
        diagnostics.notifications += 1;
        let reference = event
            .parameters
            .get("FootstepEffectsSettings")
            .and_then(serde_json::Value::as_str);
        let Some(settings) = library
            .definitions
            .iter()
            .find(|settings| Some(settings.notify_settings_reference.as_str()) == reference)
        else {
            diagnostics.invalid += 1;
            continue;
        };
        let parameter = |name: &str| {
            event
                .parameters
                .get(name)
                .unwrap_or(&settings.notify_defaults[name])
        };
        if parameter("bSkipEffectsWhenInAir").as_bool() == Some(true)
            && *locomotion == LocomotionState::Air
        {
            diagnostics.rejected_air += 1;
            continue;
        }
        let mut ignored = vec![event.character];
        for (owner, instance) in &avatars {
            if owner.character == event.character
                && let Ok(player) = players.get(instance.player_entity())
                && let Some(ragdoll) = player.get_spawned_ragdoll()
            {
                ignored.extend(ragdoll.bodies.values().copied());
                ignored.extend(ragdoll.colliders.values().copied());
            }
        }
        let filter =
            SpatialQueryFilter::from_excluded_entities(ignored).with_mask(settings.trace_layer);
        let foot_value = parameter("FootBone");
        let side = usize::from(
            foot_value
                .get("value")
                .or(Some(foot_value))
                .and_then(|value| value.as_str())
                .is_some_and(|value| value == "Right" || value.ends_with("::Right")),
        );
        let bone = if side == 0 { bones.left } else { bones.right };
        let (Ok(foot), Ok(mesh)) = (globals.get(bone), globals.get(assembly.avatar)) else {
            diagnostics.invalid += 1;
            continue;
        };
        let mesh_scale = mesh.to_scale_rotation_translation().0.y;
        let origin = foot.translation();
        let up = foot.rotation() * settings.foot_up_axes[side];
        let distance = settings.trace_distance * mesh_scale;
        if !origin.is_finite() || !up.is_finite() || !distance.is_finite() || distance <= 0.0 {
            diagnostics.invalid += 1;
            continue;
        }
        let mut ray = Dir3::NEG_Y;
        let mut ray_length = distance * up.length();
        let hit = Dir3::new(-up)
            .ok()
            .and_then(|direction| {
                ray = direction;
                spatial.cast_ray(origin, direction, distance * up.length(), true, &filter)
            })
            .or_else(|| {
                ray = Dir3::NEG_Y;
                ray_length = distance;
                let hit = spatial.cast_ray(origin, ray, distance, true, &filter);
                if hit.is_some() {
                    diagnostics.fallback_hits += 1;
                }
                hit
            });
        let debug_index = (player
            && show_debug.as_ref().is_some_and(|state| {
                state.is_active(crate::show_debug_hud::DebugCategory::Traces)
            }))
        .then(|| {
            let index = debug.0.len();
            debug.0.push(FootstepDebugTrace {
                created_at: time.elapsed_secs_f64(),
                start: origin,
                end: origin + ray.as_vec3() * ray_length,
                hit: hit
                    .as_ref()
                    .map(|hit| origin + ray.as_vec3() * hit.distance),
                rotation: None,
            });
            index
        });
        let Some(hit) = hit else {
            diagnostics.missed += 1;
            continue;
        };
        let point = origin + ray.as_vec3() * hit.distance;
        let surface = surfaces.get(hit.entity).ok().or_else(|| {
            collider_owners
                .get(hit.entity)
                .ok()
                .and_then(|owner| surfaces.get(owner.body).ok())
        });
        let Some(effect) = settings.effect_for_surface(
            surface.map_or("SurfaceType_Default", |surface| surface.0.as_str()),
        ) else {
            continue;
        };
        if let Some(index) = debug_index {
            debug.0[index].rotation = footstep_rotation(settings, *foot, side, hit.normal);
        }
        if parameter("bSpawnDecal").as_bool() != Some(true) {
            diagnostics.rejected_flag += 1;
            continue;
        }
        let Some(material_reference) = effect.material.as_ref() else {
            continue;
        };
        let Some(source_material) = settings.materials.get(material_reference) else {
            diagnostics.invalid += 1;
            continue;
        };
        let Some(world) =
            footprint_transform(settings, effect, *foot, side, point, hit.normal, mesh_scale)
        else {
            diagnostics.rejected_angle += 1;
            continue;
        };
        let parent = effect
            .attached
            .then(|| globals.get(hit.entity).ok())
            .flatten();
        let local = parent.map_or(world, |parent| {
            GlobalTransform::from(world).reparented_to(parent)
        });
        let material = materials.add(PackedDecalMaterial {
            color: Vec4::new(
                source_material.base_color,
                source_material.base_color,
                source_material.base_color,
                source_material.opacity,
            ),
            metallic: source_material.metallic,
            specular: source_material.specular,
            roughness_scale: source_material.roughness_scale,
            texture: source_material.image.clone(),
        });
        let mut entity = commands.spawn((
            Name::new(if side == 0 {
                "Footprint Left"
            } else {
                "Footprint Right"
            }),
            Footprint {
                character: event.character,
                foot: side,
                created_at: time.elapsed_secs_f64(),
                lifetime: Vec2::new(effect.duration, effect.fade_out_duration),
                surface: hit.entity,
                opacity: source_material.opacity,
            },
            PackedDecal(material),
            local,
            GlobalTransform::from(world),
        ));
        if parent.is_some() {
            entity.insert(ChildOf(hit.entity));
        }
        diagnostics.spawned[side] += 1;
    }
}

fn expire_footprints(
    mut commands: Commands,
    time: Res<Time>,
    footprints: Query<(Entity, &Footprint, &PackedDecal)>,
    mut materials: ResMut<Assets<PackedDecalMaterial>>,
    mut diagnostics: Query<&mut FootprintDiagnostics>,
    mut debug: ResMut<FootstepDebugHistory>,
) {
    debug
        .0
        .retain(|trace| time.elapsed_secs_f64() - trace.created_at < 10.0);
    for (entity, footprint, material) in &footprints {
        let age = time.elapsed_secs_f64() - footprint.created_at;
        let lifetime = footprint.lifetime;
        if lifetime.element_sum() > 0.0 && age >= lifetime.element_sum() as f64 {
            commands.entity(entity).despawn();
            if let Ok(mut diagnostics) = diagnostics.get_mut(footprint.character) {
                diagnostics.expired += 1;
            }
        } else if age > lifetime.x as f64
            && let Some(mut material) = materials.get_mut(&material.0)
        {
            material.color.w = footprint.opacity * footprint_opacity(age, lifetime);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::{
        asset::RenderAssetUsages,
        image::{CompressedImageFormats, ImageSampler, ImageType},
    };

    fn library() -> FootprintLibrary {
        serde_json::from_str(include_str!(
            "../../../assets/ALS/Data/Footstep/footprint.runtime.json"
        ))
        .unwrap()
    }

    static FOOTPRINT: std::sync::LazyLock<FootprintDefinition> =
        std::sync::LazyLock::new(|| library().definitions.remove(0));

    #[test]
    fn replacement_avatar_preserves_counters_and_receives_the_actor_decal_policy() {
        let mut app = App::new();
        app.init_resource::<Assets<FootprintLibrary>>();
        let mut data = library();
        data.definitions.clear();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<FootprintLibrary>>()
            .add(data);
        app.insert_resource(FootprintAssets { library: handle });
        let character = app
            .world_mut()
            .spawn(FootprintDiagnostics {
                notifications: 7,
                ..default()
            })
            .id();
        for _ in 0..2 {
            let avatar = app.world_mut().spawn(AlsCharacterAvatar { character }).id();
            app.world_mut()
                .run_system_once(configure_footprint_owners)
                .unwrap();
            assert!(
                app.world().get::<NoDecalReceiver>(avatar).is_some(),
                "Replacement avatar did not receive the source actor decal policy"
            );
            assert_eq!(
                app.world()
                    .get::<FootprintDiagnostics>(character)
                    .unwrap()
                    .notifications,
                7
            );
            app.world_mut().despawn(avatar);
        }
    }

    #[test]
    fn source_surface_selection_matches_then_falls_back_and_allows_an_empty_map() {
        let mut settings = FOOTPRINT.clone();
        let mut alternate = settings.effects[0].clone();
        alternate.surface = "SurfaceType1".into();
        alternate.duration = 9.0;
        settings.effects.push(alternate);
        assert_eq!(
            settings
                .effect_for_surface("SurfaceType1")
                .unwrap()
                .duration,
            9.0
        );
        assert_eq!(
            settings.effect_for_surface("SurfaceType2").unwrap().surface,
            "SurfaceType_Default"
        );
        settings.effects.clear();
        assert!(settings.effect_for_surface("SurfaceType_Default").is_none());
    }

    #[test]
    fn footprint_library_loads_multiple_references_and_rejects_invalid_bindings() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "target/footprint-library-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("ALS/Material/Footprint")).unwrap();
        std::fs::write(
            root.join("ALS/Material/Footprint/T_Als_Footprint_M.tga"),
            include_bytes!("../../../assets/ALS/Material/Footprint/T_Als_Footprint_M.tga"),
        )
        .unwrap();
        let mut data: serde_json::Value = serde_json::from_str(include_str!(
            "../../../assets/ALS/Data/Footstep/footprint.runtime.json"
        ))
        .unwrap();
        let mut alternate = data["definitions"][0].clone();
        alternate["source_settings"] = "/ALS/Tests/Alternate.Alternate".into();
        alternate["notify_settings_reference"] = "None//ALS/Tests/Alternate/Alternate".into();
        data["definitions"].as_array_mut().unwrap().push(alternate);
        std::fs::write(
            root.join("settings.json"),
            serde_json::to_vec(&data).unwrap(),
        )
        .unwrap();
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: root.to_string_lossy().into_owned(),
                ..default()
            },
            bevy::image::ImagePlugin::default(),
        ))
        .register_asset_loader(bevy::image::ImageLoader::new(CompressedImageFormats::NONE))
        .init_asset::<FootprintLibrary>()
        .init_asset_loader::<FootprintLibraryLoader>()
        .init_asset::<crate::animation_events::AlsNotifyManifest>()
        .init_asset_loader::<crate::animation_events::AlsNotifyManifestLoader>();
        app.finish();
        app.cleanup();
        let handle: Handle<FootprintLibrary> =
            app.world().resource::<AssetServer>().load("settings.json");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            app.update();
            let server = app.world().resource::<AssetServer>();
            if server.is_loaded_with_dependencies(handle.id()) {
                break;
            }
            assert!(
                !matches!(
                    server.load_state(handle.id()),
                    bevy::asset::LoadState::Failed(_)
                ),
                "Footprint library load failed"
            );
            assert!(
                std::time::Instant::now() < deadline,
                "Footprint library dependencies did not load"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let loaded = app
            .world()
            .resource::<Assets<FootprintLibrary>>()
            .get(&handle)
            .unwrap();
        assert_eq!(loaded.definitions.len(), 2);
        assert_ne!(
            loaded.definitions[0].notify_settings_reference,
            loaded.definitions[1].notify_settings_reference
        );
        for definition in &loaded.definitions {
            for material in definition.materials.values() {
                let image = app
                    .world()
                    .resource::<Assets<Image>>()
                    .get(&material.image)
                    .unwrap();
                assert_eq!(image.size(), UVec2::splat(512));
                assert_eq!(
                    image.texture_descriptor.format,
                    bevy::render::render_resource::TextureFormat::Rgba8Unorm
                );
            }
        }
        for case in ["duplicate", "material", "defaults"] {
            let mut invalid = data.clone();
            match case {
                "duplicate" => {
                    invalid["definitions"][1]["notify_settings_reference"] =
                        invalid["definitions"][0]["notify_settings_reference"].clone()
                }
                "material" => {
                    invalid["definitions"][0]["effects"][0]["material"] = "missing".into()
                }
                _ => {
                    invalid["definitions"][0]["notify_defaults"]
                        .as_object_mut()
                        .unwrap()
                        .remove("FootBone");
                }
            }
            let name = format!("{case}.json");
            std::fs::write(root.join(&name), serde_json::to_vec(&invalid).unwrap()).unwrap();
            let handle: Handle<FootprintLibrary> = app.world().resource::<AssetServer>().load(name);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            loop {
                app.update();
                let server = app.world().resource::<AssetServer>();
                if matches!(
                    server.load_state(handle.id()),
                    bevy::asset::LoadState::Failed(_)
                ) {
                    break;
                }
                assert!(
                    !server.is_loaded_with_dependencies(handle.id()),
                    "Invalid footprint {case} was accepted"
                );
                assert!(
                    std::time::Instant::now() < deadline,
                    "Invalid footprint load did not finish"
                );
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    }

    #[test]
    fn source_footprint_axes_angle_size_and_side_offsets() {
        let left = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let right = left * Quat::from_rotation_y(std::f32::consts::PI);
        for (side, rotation, x) in [(0, left, -0.0175), (1, right, 0.0175)] {
            let foot = GlobalTransform::from(Transform::from_rotation(rotation));
            let decal = footprint_transform(
                &FOOTPRINT,
                &FOOTPRINT.effects[0],
                foot,
                side,
                Vec3::ZERO,
                Vec3::Y,
                1.0,
            )
            .unwrap();
            assert!(decal.translation.distance(Vec3::new(x, 0.0, 0.1)) < 1e-6);
            assert_eq!(decal.scale, Vec3::new(0.4, -0.4, 0.2));
            assert!((decal.rotation * Vec3::Z).dot(Vec3::Y).abs() > 0.99999);
            let tilted = GlobalTransform::from(Transform::from_rotation(
                Quat::from_rotation_x(36.0_f32.to_radians()) * rotation,
            ));
            assert!(
                footprint_transform(
                    &FOOTPRINT,
                    &FOOTPRINT.effects[0],
                    tilted,
                    side,
                    Vec3::ZERO,
                    Vec3::Y,
                    1.0
                )
                .is_none()
            );
            let scaled = footprint_transform(
                &FOOTPRINT,
                &FOOTPRINT.effects[0],
                foot,
                side,
                Vec3::ZERO,
                Vec3::Y,
                2.0,
            )
            .unwrap();
            assert!(scaled.translation.distance(decal.translation * 2.0) < 1e-6);
            assert_eq!(scaled.scale, decal.scale * 2.0);
        }
    }

    #[test]
    fn source_footprint_follows_its_surface_fades_and_only_destroys_itself() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .init_resource::<FootstepDebugHistory>()
            .init_resource::<Assets<PackedDecalMaterial>>();
        let owner = app.world_mut().spawn(FootprintDiagnostics::default()).id();
        let hit = Vec3::new(1.1, 0.0, 2.2);
        app.world_mut()
            .resource_mut::<FootstepDebugHistory>()
            .0
            .push(FootstepDebugTrace {
                created_at: 0.0,
                start: hit + Vec3::Y * 0.05,
                end: hit - Vec3::Y * 0.45,
                hit: Some(hit),
                rotation: Some(Quat::IDENTITY),
            });
        let parent = app
            .world_mut()
            .spawn(Transform::from_xyz(1.0, 0.0, 2.0))
            .id();
        let material = app
            .world_mut()
            .resource_mut::<Assets<PackedDecalMaterial>>()
            .add(PackedDecalMaterial {
                color: Vec4::new(0.0, 0.0, 0.0, 0.8),
                roughness_scale: 0.2,
                metallic: 0.0,
                specular: 0.5,
                texture: default(),
            });
        let entity = app
            .world_mut()
            .spawn((
                Footprint {
                    character: owner,
                    foot: 0,
                    created_at: 0.0,
                    lifetime: Vec2::new(4.0, 2.0),
                    surface: parent,
                    opacity: 0.8,
                },
                PackedDecal(material.clone()),
                Transform::from_xyz(0.1, 0.0, 0.2),
                ChildOf(parent),
            ))
            .id();
        app.update();
        app.world_mut()
            .get_mut::<Transform>(parent)
            .unwrap()
            .rotation = Quat::from_rotation_y(0.7);
        app.world_mut()
            .get_mut::<Transform>(parent)
            .unwrap()
            .translation += Vec3::X;
        app.update();
        let expected = app
            .world()
            .get::<GlobalTransform>(parent)
            .unwrap()
            .transform_point(Vec3::new(0.1, 0.0, 0.2));
        assert!(
            app.world()
                .get::<GlobalTransform>(entity)
                .unwrap()
                .translation()
                .distance(expected)
                < 1e-6
        );
        assert_eq!(footprint_opacity(4.0, Vec2::new(4.0, 2.0)), 1.0);
        assert_eq!(footprint_opacity(5.0, Vec2::new(4.0, 2.0)), 0.5);
        assert_eq!(footprint_opacity(10.0, Vec2::ZERO), 1.0);
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs(5));
        app.insert_resource(time);
        app.world_mut().run_system_once(expire_footprints).unwrap();
        assert!(
            (app.world()
                .resource::<Assets<PackedDecalMaterial>>()
                .get(&material)
                .unwrap()
                .color
                .w
                - 0.4)
                .abs()
                < 1e-6
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(500));
        app.world_mut().run_system_once(expire_footprints).unwrap();
        assert!(
            (app.world()
                .resource::<Assets<PackedDecalMaterial>>()
                .get(&material)
                .unwrap()
                .color
                .w
                - 0.2)
                .abs()
                < 1e-6
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(500));
        app.world_mut().run_system_once(expire_footprints).unwrap();
        assert!(app.world().get_entity(entity).is_err());
        assert!(app.world().get_entity(parent).is_ok() && app.world().get_entity(owner).is_ok());
        assert_eq!(
            app.world()
                .get::<FootprintDiagnostics>(owner)
                .unwrap()
                .expired,
            1
        );
        assert_eq!(
            app.world().resource::<FootstepDebugHistory>().0[0].hit,
            Some(hit),
            "Persistent debug geometry stays in world space"
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(4));
        app.world_mut().run_system_once(expire_footprints).unwrap();
        assert!(
            app.world().resource::<FootstepDebugHistory>().0.is_empty(),
            "Debug traces expire after ten seconds"
        );
    }

    #[test]
    fn source_notifications_use_final_bones_trace_fallback_and_spawn_flags() {
        use crate::als::character::AlsCollisionLayer;
        use avian3d::prelude::{Collider, CollisionLayers, LayerMask, PhysicsPlugins, RigidBody};
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            PhysicsPlugins::default(),
        ))
        .init_asset::<Mesh>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / 60.0),
        ))
        .add_message::<AlsAnimationEvent>()
        .init_resource::<Assets<PackedDecalMaterial>>()
        .init_resource::<Assets<FootprintLibrary>>();
        app.add_plugins((
            bevy::world_serialization::WorldSerializationPlugin,
            bevy::gizmos::GizmoPlugin,
            bevy_animation_graph::AnimationGraphPlugin::default(),
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
        app.init_resource::<FootstepDebugHistory>().insert_resource(
            crate::show_debug_hud::AlsShowDebugHudState {
                visible: true,
                traces: true,
                ..default()
            },
        );
        let library = app
            .world_mut()
            .resource_mut::<Assets<FootprintLibrary>>()
            .add(library());
        app.insert_resource(FootprintAssets { library });
        app.finish();
        app.cleanup();
        let surface_body = app
            .world_mut()
            .spawn((RigidBody::Static, Transform::from_xyz(0.0, -0.5, 0.0)))
            .id();
        let floor = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Collider::cuboid(4.0, 1.0, 4.0),
                Transform::IDENTITY,
                ChildOf(surface_body),
                CollisionLayers::new(
                    [AlsCollisionLayer::Default, AlsCollisionLayer::Visibility],
                    LayerMask::ALL,
                ),
            ))
            .id();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(4.0, 0.01, 4.0),
            Transform::from_xyz(0.0, 0.025, 0.0),
            CollisionLayers::new(AlsCollisionLayer::Default, LayerMask::ALL),
        ));
        let avatar = app.world_mut().spawn(Transform::IDENTITY).id();
        let rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let left = app
            .world_mut()
            .spawn(Transform::from_xyz(-0.1, 0.05, 0.0).with_rotation(rotation))
            .id();
        let right = app
            .world_mut()
            .spawn(
                Transform::from_xyz(0.1, 0.05, 0.0)
                    .with_rotation(rotation * Quat::from_rotation_y(std::f32::consts::PI)),
            )
            .id();
        let character = app
            .world_mut()
            .spawn((
                AlsCharacter,
                crate::character_assembly::AlsPlayerCharacter,
                crate::debug_target::AlsDebugTarget,
                LocomotionState::Grounded,
                FootprintDiagnostics::default(),
                AlsCharacterAssembly {
                    avatar,
                    camera: None,
                },
                AlsFootBones {
                    left_thigh: left,
                    left_calf: left,
                    left,
                    left_target: None,
                    left_virtual_target: None,
                    right_thigh: right,
                    right_calf: right,
                    right,
                    right_target: None,
                    right_virtual_target: None,
                },
            ))
            .id();
        app.world_mut().entity_mut(avatar).insert((
            AlsCharacterAvatar { character },
            bevy_animation_graph::core::animated_scene::AnimatedSceneHandle::default(),
        ));
        let source_player = app
            .world_mut()
            .spawn((
                Name::new("Source player"),
                AnimationGraphPlayer::new(Handle::default()),
                Transform::IDENTITY,
                ChildOf(avatar),
            ))
            .id();
        app.world_mut().spawn((
            Name::new("Source bone"),
            Transform::IDENTITY,
            ChildOf(source_player),
        ));
        app.world_mut()
            .trigger(bevy::world_serialization::WorldInstanceReady {
                entity: avatar,
                instance_id: bevy::world_serialization::WorldInstanceSpawner::default()
                    .spawn(Handle::<bevy::world_serialization::WorldAsset>::default()),
            });
        app.world_mut().flush();
        app.update();
        app.update();
        let emit = |world: &mut World, side: &str, spawn: bool, skip_air: bool, settings: &str| {
            world.resource_mut::<Messages<AlsAnimationEvent>>().write(AlsAnimationEvent {
                character, instance_id: 1, source_asset: "source fixture".into(), graph_clip: "source fixture".into(), event_id: "step".into(),
                source: Some((avatar, source_player, 0)),
                class_name: "AlsAnimNotify_FootstepEffects".into(), name: "Step".into(), phase: crate::extensions::EventPhase::Point,
                time: 0.5, weight: 1.0, percentage: 1.0,
                parameters: serde_json::json!({"FootstepEffectsSettings": settings,
                    "FootBone": {"value": format!("EAlsFootBone::{side}")}, "bSpawnDecal": spawn, "bSkipEffectsWhenInAir": skip_air}),
            });
        };
        emit(
            app.world_mut(),
            "Left",
            true,
            false,
            &FOOTPRINT.notify_settings_reference,
        );
        emit(
            app.world_mut(),
            "Right",
            true,
            false,
            &FOOTPRINT.notify_settings_reference,
        );
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        assert_eq!(
            app.world()
                .get::<FootprintDiagnostics>(character)
                .unwrap()
                .spawned,
            [1, 1],
            "Other characters with a blocking query response must remain traceable"
        );
        let history = &app.world().resource::<FootstepDebugHistory>().0;
        assert_eq!(
            history.len(),
            2,
            "Every source notification must retain its debug trace"
        );
        assert!(
            history
                .iter()
                .all(|trace| trace.hit.is_some() && trace.rotation.is_some())
        );
        for (_, parent) in app
            .world_mut()
            .query::<(&Footprint, &ChildOf)>()
            .iter(app.world())
        {
            assert_eq!(parent.parent(), floor);
        }
        app.world_mut()
            .resource_mut::<Messages<AlsAnimationEvent>>()
            .clear();
        *app.world_mut()
            .get_mut::<LocomotionState>(character)
            .unwrap() = LocomotionState::Air;
        emit(
            app.world_mut(),
            "Left",
            false,
            false,
            &FOOTPRINT.notify_settings_reference,
        );
        emit(
            app.world_mut(),
            "Right",
            true,
            true,
            &FOOTPRINT.notify_settings_reference,
        );
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        assert_eq!(
            app.world()
                .get::<FootprintDiagnostics>(character)
                .unwrap()
                .rejected_flag,
            1
        );
        assert_eq!(
            app.world()
                .get::<FootprintDiagnostics>(character)
                .unwrap()
                .rejected_air,
            1
        );
        assert_eq!(
            app.world().resource::<FootstepDebugHistory>().0.len(),
            3,
            "Disabling the decal must not disable the trace or hit frame"
        );
        app.world_mut()
            .resource_mut::<Messages<AlsAnimationEvent>>()
            .clear();
        *app.world_mut()
            .get_mut::<LocomotionState>(character)
            .unwrap() = LocomotionState::Grounded;
        app.world_mut().get_mut::<Transform>(left).unwrap().rotation = Quat::IDENTITY;
        app.update();
        emit(
            app.world_mut(),
            "Left",
            true,
            false,
            &FOOTPRINT.notify_settings_reference,
        );
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        let diagnostics = app.world().get::<FootprintDiagnostics>(character).unwrap();
        assert_eq!(diagnostics.fallback_hits, 1);
        assert_eq!(diagnostics.rejected_angle, 1);
        assert_eq!(diagnostics.invalid, 0);
        let history = &app.world().resource::<FootstepDebugHistory>().0;
        assert_eq!(history.len(), 4);
        assert!(
            history.last().unwrap().rotation.is_some(),
            "Angle rejection must retain the hit frame"
        );
        assert!((history.last().unwrap().end.y + 0.45).abs() < 1e-6);

        let reference = "None//ALS/Tests/Alternate/Alternate";
        let mut alternate = FOOTPRINT.clone();
        alternate.notify_settings_reference = reference.into();
        alternate.trace_distance = 0.01;
        alternate.foot_up_axes[0] *= 2.0;
        let mut material = alternate.materials.values().next().unwrap().clone();
        material.opacity = 0.8;
        material.roughness_scale = 0.6;
        alternate
            .materials
            .insert("/ALS/Tests/M_Alternate.M_Alternate".into(), material);
        let mut effect = alternate.effects[0].clone();
        effect.surface = "SurfaceType1".into();
        effect.material = Some("/ALS/Tests/M_Alternate.M_Alternate".into());
        effect.duration = 8.0;
        effect.attached = false;
        effect.size *= 2.0;
        effect.location_offset.z += 0.2;
        alternate.effects.push(effect);
        let handle = app.world().resource::<FootprintAssets>().library.clone();
        app.world_mut()
            .resource_mut::<Assets<FootprintLibrary>>()
            .get_mut(&handle)
            .unwrap()
            .definitions
            .push(alternate);
        app.world_mut().get_mut::<Transform>(left).unwrap().rotation = rotation;
        app.world_mut()
            .entity_mut(surface_body)
            .insert(PhysicalSurface("SurfaceType1".into()));
        app.update();
        app.world_mut()
            .resource_mut::<Messages<AlsAnimationEvent>>()
            .clear();
        emit(app.world_mut(), "Left", true, false, reference);
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        assert_eq!(
            app.world()
                .get::<FootprintDiagnostics>(character)
                .unwrap()
                .missed,
            1
        );
        let trace = app
            .world()
            .resource::<FootstepDebugHistory>()
            .0
            .last()
            .unwrap();
        assert!(trace.hit.is_none() && trace.rotation.is_none());
        assert!(
            (trace.end.y - 0.04).abs() < 1e-6,
            "A miss must show the final fallback ray"
        );
        app.world_mut()
            .resource_mut::<Assets<FootprintLibrary>>()
            .get_mut(&handle)
            .unwrap()
            .definitions[1]
            .trace_distance = 0.03;
        app.world_mut()
            .resource_mut::<Messages<AlsAnimationEvent>>()
            .clear();
        emit(app.world_mut(), "Left", true, false, reference);
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        let (decal, _, transform, parent, material) = app
            .world_mut()
            .query::<(
                Entity,
                &Footprint,
                &Transform,
                Option<&ChildOf>,
                &PackedDecal,
            )>()
            .iter(app.world())
            .find(|(_, mark, _, _, _)| mark.opacity == 0.8)
            .expect("Selected source foot axis must set the primary trace length");
        assert!(parent.is_none());
        assert!(transform.scale.abs_diff_eq(Vec3::new(0.8, -0.8, 0.4), 1e-6));
        assert!(
            transform
                .translation
                .abs_diff_eq(Vec3::new(-0.1175, 0.2, 0.1), 1e-6),
            "{transform:?}"
        );
        let material = material.0.clone();
        let rendered = app
            .world()
            .resource::<Assets<PackedDecalMaterial>>()
            .get(&material)
            .unwrap();
        assert_eq!(rendered.roughness_scale, 0.6);
        assert_eq!(rendered.color.w, 0.8);
        assert_eq!(
            app.world().get::<Footprint>(decal).unwrap().lifetime,
            Vec2::new(8.0, 2.0)
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(9));
        app.world_mut().run_system_once(expire_footprints).unwrap();
        assert!(
            (app.world()
                .resource::<Assets<PackedDecalMaterial>>()
                .get(&material)
                .unwrap()
                .color
                .w
                - 0.4)
                .abs()
                < 1e-6
        );
        let before = app
            .world()
            .get::<FootprintDiagnostics>(character)
            .unwrap()
            .spawned;
        app.world_mut()
            .resource_mut::<Assets<FootprintLibrary>>()
            .get_mut(&handle)
            .unwrap()
            .definitions[1]
            .effects[1]
            .material = None;
        app.world_mut()
            .resource_mut::<Messages<AlsAnimationEvent>>()
            .clear();
        emit(app.world_mut(), "Left", true, false, reference);
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        assert!(
            app.world()
                .resource::<FootstepDebugHistory>()
                .0
                .last()
                .unwrap()
                .rotation
                .is_some(),
            "A missing decal material must retain the debug axes"
        );
        app.world_mut()
            .resource_mut::<Assets<FootprintLibrary>>()
            .get_mut(&handle)
            .unwrap()
            .definitions[1]
            .effects
            .clear();
        app.world_mut()
            .resource_mut::<Messages<AlsAnimationEvent>>()
            .clear();
        emit(app.world_mut(), "Left", true, false, reference);
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        assert_eq!(
            app.world()
                .get::<FootprintDiagnostics>(character)
                .unwrap()
                .spawned,
            before
        );
        assert_eq!(
            app.world()
                .get::<FootprintDiagnostics>(character)
                .unwrap()
                .invalid,
            0
        );
        let trace = app
            .world()
            .resource::<FootstepDebugHistory>()
            .0
            .last()
            .unwrap();
        assert!(
            trace.hit.is_some() && trace.rotation.is_none(),
            "An empty effect table retains only the trace and impact point"
        );
        let count = app.world().resource::<FootstepDebugHistory>().0.len();
        for visible in [false, true] {
            app.world_mut()
                .resource_mut::<crate::show_debug_hud::AlsShowDebugHudState>()
                .visible = visible;
            if visible {
                app.world_mut()
                    .entity_mut(character)
                    .remove::<crate::debug_target::AlsDebugTarget>();
            }
            app.world_mut()
                .resource_mut::<Messages<AlsAnimationEvent>>()
                .clear();
            emit(
                app.world_mut(),
                "Left",
                false,
                false,
                &FOOTPRINT.notify_settings_reference,
            );
            app.world_mut().run_system_once(spawn_footprints).unwrap();
            assert_eq!(
                app.world().resource::<FootstepDebugHistory>().0.len(),
                count,
                "Only the current debug target records enabled traces"
            );
        }
        app.world_mut()
            .resource_mut::<Messages<AlsAnimationEvent>>()
            .clear();
        let pending = app
            .world_mut()
            .resource_mut::<Assets<FootprintLibrary>>()
            .remove(handle.id())
            .unwrap();
        let before = app
            .world()
            .get::<FootprintDiagnostics>(character)
            .unwrap()
            .notifications;
        emit(
            app.world_mut(),
            "Left",
            true,
            false,
            &FOOTPRINT.notify_settings_reference,
        );
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        assert_eq!(
            app.world()
                .get::<FootprintDiagnostics>(character)
                .unwrap()
                .notifications,
            before
        );
        let replacement = app.world_mut().spawn(Transform::IDENTITY).id();
        let replacement_foot = app
            .world_mut()
            .spawn(Transform::from_xyz(0.9, 0.05, 0.0).with_rotation(rotation))
            .id();
        app.world_mut()
            .get_mut::<AlsCharacterAssembly>(character)
            .unwrap()
            .avatar = replacement;
        app.world_mut()
            .get_mut::<AlsFootBones>(character)
            .unwrap()
            .left = replacement_foot;
        app.world_mut()
            .resource_mut::<Assets<FootprintLibrary>>()
            .insert(handle.id(), pending)
            .unwrap();
        app.update();
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        assert_eq!(
            app.world()
                .get::<FootprintDiagnostics>(character)
                .unwrap()
                .notifications,
            before,
            "A queued footstep must not consume the replacement avatar's bones"
        );
        app.world_mut()
            .get_mut::<AlsCharacterAssembly>(character)
            .unwrap()
            .avatar = avatar;
        app.world_mut()
            .get_mut::<AlsFootBones>(character)
            .unwrap()
            .left = left;
        let pending = app
            .world_mut()
            .resource_mut::<Assets<FootprintLibrary>>()
            .remove(handle.id())
            .unwrap();
        emit(
            app.world_mut(),
            "Left",
            true,
            false,
            &FOOTPRINT.notify_settings_reference,
        );
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        app.update();
        app.world_mut()
            .resource_mut::<Assets<FootprintLibrary>>()
            .insert(handle.id(), pending)
            .unwrap();
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        assert_eq!(
            app.world()
                .get::<FootprintDiagnostics>(character)
                .unwrap()
                .notifications,
            before + 1,
            "An unchanged binding must still consume a delayed current footstep"
        );
        let pending = app
            .world_mut()
            .resource_mut::<Assets<FootprintLibrary>>()
            .remove(handle.id())
            .unwrap();
        emit(
            app.world_mut(),
            "Left",
            true,
            false,
            &FOOTPRINT.notify_settings_reference,
        );
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        app.world_mut()
            .entity_mut(source_player)
            .remove::<ChildOf>();
        let replacement_player = app
            .world_mut()
            .spawn((
                Name::new("Replacement player"),
                AnimationGraphPlayer::new(Handle::default()),
                Transform::IDENTITY,
                ChildOf(avatar),
            ))
            .id();
        app.world_mut().spawn((
            Name::new("Replacement bone"),
            Transform::IDENTITY,
            ChildOf(replacement_player),
        ));
        app.world_mut()
            .trigger(bevy::world_serialization::WorldInstanceReady {
                entity: avatar,
                instance_id: bevy::world_serialization::WorldInstanceSpawner::default()
                    .spawn(Handle::<bevy::world_serialization::WorldAsset>::default()),
            });
        app.world_mut().flush();
        assert_eq!(
            app.world()
                .get::<AnimatedSceneInstance>(avatar)
                .unwrap()
                .player_entity(),
            replacement_player
        );
        app.update();
        app.world_mut()
            .resource_mut::<Assets<FootprintLibrary>>()
            .insert(handle.id(), pending)
            .unwrap();
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        assert_eq!(
            app.world()
                .get::<FootprintDiagnostics>(character)
                .unwrap()
                .notifications,
            before + 1,
            "Rebuilding the player on the same avatar must reject the old queued source"
        );
        for (generation, expected) in [(1, before + 1), (0, before + 2)] {
            emit(
                app.world_mut(),
                "Left",
                true,
                false,
                &FOOTPRINT.notify_settings_reference,
            );
            let mut event = app
                .world_mut()
                .resource_mut::<Messages<AlsAnimationEvent>>()
                .drain()
                .next()
                .unwrap();
            event.source = Some((avatar, replacement_player, generation));
            app.world_mut()
                .resource_mut::<Messages<AlsAnimationEvent>>()
                .write(event);
            app.world_mut().run_system_once(spawn_footprints).unwrap();
            assert_eq!(
                app.world()
                    .get::<FootprintDiagnostics>(character)
                    .unwrap()
                    .notifications,
                expected,
                "Only the current player generation may consume the footstep"
            );
        }
    }

    #[test]
    fn footstep_hit_frame_matches_source_axes_and_parallel_fallback() {
        let mut settings = FOOTPRINT.clone();
        settings.foot_y_axes[0] = Vec3::NEG_Z;
        let rotation = footstep_rotation(&settings, GlobalTransform::IDENTITY, 0, Vec3::Y).unwrap();
        assert!((rotation * Vec3::X).abs_diff_eq(Vec3::NEG_X, 1e-6));
        assert!((rotation * Vec3::Z).abs_diff_eq(Vec3::NEG_Z, 1e-6));
        assert!((rotation * Vec3::Y).abs_diff_eq(Vec3::Y, 1e-6));
        settings.foot_y_axes[0] = Vec3::Y;
        let parallel = footstep_rotation(&settings, GlobalTransform::IDENTITY, 0, Vec3::Y).unwrap();
        let source_forward = crate::extensions::playground::ue_point_to_bevy(Vec3::X * 100.0);
        let source_up = crate::extensions::playground::ue_point_to_bevy(Vec3::Z * 100.0);
        assert!((parallel * Vec3::Z).abs_diff_eq(source_forward, 1e-6));
        assert!((parallel * Vec3::Y).abs_diff_eq(source_up, 1e-6));
        assert!((parallel * Vec3::X).abs_diff_eq(source_up.cross(source_forward), 1e-6));
        settings.foot_y_axes[0] = Vec3::X;
        let horizontal =
            footstep_rotation(&settings, GlobalTransform::IDENTITY, 0, Vec3::X).unwrap();
        assert!((horizontal * Vec3::Y).abs_diff_eq(source_forward, 1e-6));
        assert!((horizontal * Vec3::Z).abs_diff_eq(source_up, 1e-6));
    }

    #[test]
    fn footprint_generated_settings_and_images_preserve_source_channels() {
        assert_eq!(FOOTPRINT.trace_distance, 0.5);
        assert_eq!(
            FOOTPRINT.effects[0].location_offset,
            Vec3::new(-0.0175, -0.1, 0.0)
        );
        assert_eq!(
            FOOTPRINT.notify_defaults["bSpawnDecal"].as_bool(),
            Some(true)
        );
        let decode = |bytes| {
            Image::from_buffer(
                bytes,
                ImageType::Extension("tga"),
                CompressedImageFormats::NONE,
                false,
                ImageSampler::default(),
                RenderAssetUsages::all(),
            )
            .unwrap()
        };
        let source = decode(include_bytes!(
            "../../../assets/ALS/Material/Footprint/T_Als_Footprint_M.tga"
        ));
        assert_eq!(source.size(), UVec2::splat(512));
        let data = source.data.unwrap();
        assert_eq!(&data[..4], [0, 0, 255, 0]);
        assert_eq!(&data[(256 * 512 + 256) * 4..][..4], [0, 0, 255, 255]);
        assert!(
            data.chunks_exact(4)
                .any(|pixel| pixel[1] == 255 && pixel[3] == 255)
        );
        for rotation in FOOTPRINT.effects[0].rotation_offsets {
            assert!((rotation.length_squared() - 1.0).abs() < 1e-6);
        }
    }
}
