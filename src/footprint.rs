use bevy::prelude::*;
use serde::Deserialize;
use std::{collections::HashMap, sync::LazyLock};
use als_core::{character::AlsCharacter, foot_ik::AlsFootBones, state::LocomotionState};
use als_ext::projected_decal::{NoDecalReceiver, PackedDecalExtension, PackedDecalMaterial};
use avian3d::{collider_tree::ColliderTrees, prelude::{SpatialQuery, SpatialQueryFilter}};
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy_animation_graph::core::{animated_scene::AnimatedSceneInstance, animation_graph_player::AnimationGraphPlayer};
use crate::{animation_events::{AlsAnimationEvent, AlsAnimationEventSet}, character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar}};

#[derive(Deserialize)]
pub struct FootprintDefinition {
    pub source_settings: String,
    pub notify_settings_reference: String,
    pub character_receives_decals: bool,
    pub trace_layer: als_core::character::AlsCollisionLayer,
    pub trace_distance: f32,
    pub spawn_angle_cos: f32,
    pub foot_y_axes: [Vec3; 2],
    pub foot_up_axes: [Vec3; 2],
    pub notify_defaults: HashMap<String, serde_json::Value>,
    pub effects: Vec<FootprintEffect>,
    pub material: FootprintMaterial,
}

#[derive(Deserialize)]
pub struct FootprintEffect {
    pub surface: String,
    pub attached: bool,
    pub size: Vec3,
    pub location_offset: Vec3,
    pub rotation_offsets: [Quat; 2],
    pub duration: f32,
    pub fade_out_duration: f32,
}

#[derive(Deserialize)]
pub struct FootprintMaterial {
    pub base_color: f32,
    pub opacity: f32,
    pub roughness_scale: f32,
    pub metallic: f32,
    pub specular: f32,
    pub texture: String,
}

pub static FOOTPRINT: LazyLock<FootprintDefinition> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../assets/effects/footprint.runtime.json"))
        .expect("Generated footprint definition must be valid")
});

#[derive(Component, Debug)]
pub struct Footprint {
    pub character: Entity,
    pub foot: usize,
    pub created_at: f64,
    pub lifetime: Vec2,
    pub surface: Entity,
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

#[derive(Resource)]
struct FootprintAssets {
    mesh: Handle<Mesh>,
    texture: Handle<Image>,
}

#[derive(SystemSet, Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct FootprintSet;

pub struct FootprintPlugin;

impl Plugin for FootprintPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(als_ext::projected_decal::PackedDecalPlugin)
            .add_systems(Startup, load_footprint_assets)
            .add_systems(PreUpdate, configure_footprint_owners)
            .configure_sets(PostUpdate, FootprintSet
                .after(AlsAnimationEventSet::GraphFootsteps)
                .after(crate::control_rig_apply::AlsControlRigPropagationSet))
            .add_systems(PostUpdate, (expire_footprints, spawn_footprints.run_if(resource_exists::<ColliderTrees>))
                .chain().in_set(FootprintSet));
    }
}

fn load_footprint_assets(mut commands: Commands, server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>) {
    let texture = server.load_builder().with_settings(|settings: &mut bevy::image::ImageLoaderSettings| {
        settings.is_srgb = false;
        settings.sampler = bevy::image::ImageSampler::linear();
    }).load(FOOTPRINT.material.texture.clone());
    commands.insert_resource(FootprintAssets { texture, mesh: meshes.add(Cuboid::from_size(Vec3::ONE)) });
}

fn configure_footprint_owners(
    mut commands: Commands,
    avatars: Query<(Entity, &AlsCharacterAvatar), Added<AlsCharacterAvatar>>,
    cameras: Query<Entity, (With<Camera3d>, Or<(Without<DepthPrepass>, Without<NormalPrepass>)>)>,
) {
    for (entity, avatar) in &avatars {
        if !FOOTPRINT.character_receives_decals { commands.entity(entity).insert(NoDecalReceiver); }
        commands.entity(avatar.character).insert(FootprintDiagnostics::default());
    }
    for camera in &cameras { commands.entity(camera).insert((DepthPrepass, NormalPrepass)); }
}

pub fn footprint_transform(foot: GlobalTransform, side: usize, point: Vec3, normal: Vec3, mesh_scale: f32) -> Option<Transform> {
    let rotation = foot.rotation();
    let up = rotation * FOOTPRINT.foot_up_axes[side];
    if normal.dot(up) < FOOTPRINT.spawn_angle_cos { return None; }
    let y = rotation * FOOTPRINT.foot_y_axes[side];
    let tangent = (y - normal * y.dot(normal)).try_normalize()?;
    let frame = Quat::from_mat3(&Mat3::from_cols(normal.cross(tangent), normal, tangent));
    let effect = &FOOTPRINT.effects[0];
    let rotation = frame * effect.rotation_offsets[side];
    Some(Transform {
        translation: point + rotation * (effect.location_offset * mesh_scale),
        rotation,
        scale: effect.size * (2.0 * mesh_scale),
    })
}

pub fn footprint_opacity(age: f64, lifetime: Vec2) -> f32 {
    if age <= lifetime.x as f64 { return 1.0; }
    if lifetime.y <= 0.0 { return 1.0; }
    (1.0 - ((age - lifetime.x as f64) / lifetime.y as f64) as f32).clamp(0.0, 1.0)
}

fn spawn_footprints(
    mut commands: Commands,
    mut events: MessageReader<AlsAnimationEvent>,
    assets: Res<FootprintAssets>,
    time: Res<Time>,
    spatial: SpatialQuery,
    mut characters: Query<(&AlsFootBones, &LocomotionState, &AlsCharacterAssembly, &mut FootprintDiagnostics), With<AlsCharacter>>,
    owners: Query<Entity, With<AlsCharacter>>,
    avatars: Query<&AnimatedSceneInstance, With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    globals: Query<&GlobalTransform>,
    mut materials: ResMut<Assets<PackedDecalMaterial>>,
) {
    let mut ignored = owners.iter().collect::<Vec<_>>();
    for instance in &avatars {
        if let Ok(player) = players.get(instance.player_entity()) && let Some(ragdoll) = player.get_spawned_ragdoll() {
            ignored.extend(ragdoll.bodies.values().copied());
            ignored.extend(ragdoll.colliders.values().copied());
        }
    }
    let filter = SpatialQueryFilter::from_excluded_entities(ignored).with_mask(FOOTPRINT.trace_layer);
    for event in events.read().filter(|event| event.class_name == "AlsAnimNotify_FootstepEffects" && event.phase == als_ext::EventPhase::Point) {
        let Ok((bones, locomotion, assembly, mut diagnostics)) = characters.get_mut(event.character) else { continue; };
        diagnostics.notifications += 1;
        let parameter = |name: &str| event.parameters.get(name).unwrap_or(&FOOTPRINT.notify_defaults[name]);
        if parameter("FootstepEffectsSettings").as_str() != Some(FOOTPRINT.notify_settings_reference.as_str()) {
            diagnostics.invalid += 1;
            continue;
        }
        if parameter("bSkipEffectsWhenInAir").as_bool() == Some(true) && *locomotion == LocomotionState::Air {
            diagnostics.rejected_air += 1;
            continue;
        }
        if parameter("bSpawnDecal").as_bool() != Some(true) {
            diagnostics.rejected_flag += 1;
            continue;
        }
        let foot_value = parameter("FootBone");
        let side = usize::from(foot_value.get("value").or(Some(foot_value)).and_then(|value| value.as_str())
            .is_some_and(|value| value == "Right" || value.ends_with("::Right")));
        let bone = if side == 0 { bones.left } else { bones.right };
        let (Ok(foot), Ok(mesh)) = (globals.get(bone), globals.get(assembly.avatar)) else { diagnostics.invalid += 1; continue; };
        let mesh_scale = mesh.to_scale_rotation_translation().0.y;
        let origin = foot.translation();
        let up = foot.rotation() * FOOTPRINT.foot_up_axes[side];
        let distance = FOOTPRINT.trace_distance * mesh_scale;
        if !origin.is_finite() || !up.is_finite() || !distance.is_finite() || distance <= 0.0 {
            diagnostics.invalid += 1;
            continue;
        }
        let direction = Dir3::new(-up).unwrap();
        let mut ray = direction;
        let hit = spatial.cast_ray(origin, direction, distance, true, &filter).or_else(|| {
            ray = Dir3::NEG_Y;
            let hit = spatial.cast_ray(origin, ray, distance, true, &filter);
            if hit.is_some() { diagnostics.fallback_hits += 1; }
            hit
        });
        let Some(hit) = hit else { diagnostics.missed += 1; continue; };
        let point = origin + ray.as_vec3() * hit.distance;
        let Some(world) = footprint_transform(*foot, side, point, hit.normal, mesh_scale) else { diagnostics.rejected_angle += 1; continue; };
        let effect = &FOOTPRINT.effects[0];
        let parent = effect.attached.then(|| globals.get(hit.entity).ok()).flatten();
        let local = parent.map_or(world, |parent| GlobalTransform::from(world).reparented_to(parent));
        let source_material = &FOOTPRINT.material;
        let material = materials.add(PackedDecalMaterial {
            base: StandardMaterial {
                base_color: Color::linear_rgba(source_material.base_color, source_material.base_color, source_material.base_color, source_material.opacity),
                metallic: source_material.metallic, reflectance: source_material.specular, ..default()
            },
            extension: PackedDecalExtension { roughness_scale: source_material.roughness_scale, texture: assets.texture.clone() },
        });
        let mut entity = commands.spawn((
            Name::new(if side == 0 { "Footprint Left" } else { "Footprint Right" }),
            Footprint { character: event.character, foot: side, created_at: time.elapsed_secs_f64(), lifetime: Vec2::new(effect.duration, effect.fade_out_duration), surface: hit.entity },
            Mesh3d(assets.mesh.clone()), MeshMaterial3d(material), local, GlobalTransform::from(world),
            bevy::light::NotShadowCaster,
        ));
        if parent.is_some() { entity.insert(ChildOf(hit.entity)); }
        diagnostics.spawned[side] += 1;
    }
}

fn expire_footprints(
    mut commands: Commands, time: Res<Time>,
    footprints: Query<(Entity, &Footprint, &MeshMaterial3d<PackedDecalMaterial>)>,
    mut materials: ResMut<Assets<PackedDecalMaterial>>,
    mut diagnostics: Query<&mut FootprintDiagnostics>,
) {
    for (entity, footprint, material) in &footprints {
        let age = time.elapsed_secs_f64() - footprint.created_at;
        let lifetime = footprint.lifetime;
        if lifetime.element_sum() > 0.0 && age >= lifetime.element_sum() as f64 {
            commands.entity(entity).despawn();
            if let Ok(mut diagnostics) = diagnostics.get_mut(footprint.character) { diagnostics.expired += 1; }
        } else if age > lifetime.x as f64 && let Some(mut material) = materials.get_mut(&material.0) {
            material.base.base_color.set_alpha(FOOTPRINT.material.opacity * footprint_opacity(age, lifetime));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::RenderAssetUsages, image::{CompressedImageFormats, ImageSampler, ImageType}};
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn source_footprint_axes_angle_size_and_side_offsets() {
        let left = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let right = left * Quat::from_rotation_y(std::f32::consts::PI);
        for (side, rotation, x) in [(0, left, -0.0175), (1, right, 0.0175)] {
            let foot = GlobalTransform::from(Transform::from_rotation(rotation));
            let decal = footprint_transform(foot, side, Vec3::ZERO, Vec3::Y, 1.0).unwrap();
            assert!(decal.translation.distance(Vec3::new(x, 0.0, 0.1)) < 1e-6);
            assert_eq!(decal.scale, Vec3::new(0.2, 0.4, 0.4));
            assert!((decal.rotation * Vec3::X).dot(Vec3::Y).abs() > 0.99999);
            let tilted = GlobalTransform::from(Transform::from_rotation(Quat::from_rotation_x(36.0_f32.to_radians()) * rotation));
            assert!(footprint_transform(tilted, side, Vec3::ZERO, Vec3::Y, 1.0).is_none());
            let scaled = footprint_transform(foot, side, Vec3::ZERO, Vec3::Y, 2.0).unwrap();
            assert!(scaled.translation.distance(decal.translation * 2.0) < 1e-6);
            assert_eq!(scaled.scale, decal.scale * 2.0);
        }
    }

    #[test]
    fn source_footprint_follows_its_surface_fades_and_only_destroys_itself() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .init_resource::<Assets<PackedDecalMaterial>>();
        let owner = app.world_mut().spawn(FootprintDiagnostics::default()).id();
        let parent = app.world_mut().spawn(Transform::from_xyz(1.0, 0.0, 2.0)).id();
        let material = app.world_mut().resource_mut::<Assets<PackedDecalMaterial>>().add(PackedDecalMaterial {
            base: StandardMaterial { base_color: Color::linear_rgba(0.0, 0.0, 0.0, 0.3), ..default() },
            extension: PackedDecalExtension { roughness_scale: 0.2, texture: default() },
        });
        let entity = app.world_mut().spawn((
            Footprint { character: owner, foot: 0, created_at: 0.0, lifetime: Vec2::new(4.0, 2.0), surface: parent },
            MeshMaterial3d(material.clone()), Transform::from_xyz(0.1, 0.0, 0.2), ChildOf(parent),
        )).id();
        app.update();
        app.world_mut().get_mut::<Transform>(parent).unwrap().rotation = Quat::from_rotation_y(0.7);
        app.world_mut().get_mut::<Transform>(parent).unwrap().translation += Vec3::X;
        app.update();
        let expected = app.world().get::<GlobalTransform>(parent).unwrap().transform_point(Vec3::new(0.1, 0.0, 0.2));
        assert!(app.world().get::<GlobalTransform>(entity).unwrap().translation().distance(expected) < 1e-6);
        assert_eq!(footprint_opacity(4.0, Vec2::new(4.0, 2.0)), 1.0);
        assert_eq!(footprint_opacity(5.0, Vec2::new(4.0, 2.0)), 0.5);
        assert_eq!(footprint_opacity(10.0, Vec2::ZERO), 1.0);
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs(5));
        app.insert_resource(time);
        app.world_mut().run_system_once(expire_footprints).unwrap();
        assert!((app.world().resource::<Assets<PackedDecalMaterial>>().get(&material).unwrap().base.base_color.alpha() - 0.15).abs() < 1e-6);
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
        app.world_mut().run_system_once(expire_footprints).unwrap();
        assert!(app.world().get_entity(entity).is_err());
        assert!(app.world().get_entity(parent).is_ok() && app.world().get_entity(owner).is_ok());
        assert_eq!(app.world().get::<FootprintDiagnostics>(owner).unwrap().expired, 1);
    }

    #[test]
    fn source_notifications_use_final_bones_trace_fallback_and_spawn_flags() {
        use avian3d::prelude::{Collider, CollisionLayers, LayerMask, PhysicsPlugins, RigidBody};
        use als_core::character::AlsCollisionLayer;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin, PhysicsPlugins::default()))
            .init_asset::<Mesh>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
            .add_message::<AlsAnimationEvent>()
            .init_resource::<Assets<PackedDecalMaterial>>()
            .insert_resource(FootprintAssets { mesh: default(), texture: default() });
        app.finish();
        app.cleanup();
        let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 1.0, 4.0), Transform::from_xyz(0.0, -0.5, 0.0),
            CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL))).id();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.01, 4.0), Transform::from_xyz(0.0, 0.025, 0.0),
            CollisionLayers::new(AlsCollisionLayer::Default, LayerMask::ALL)));
        let avatar = app.world_mut().spawn(Transform::IDENTITY).id();
        let rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let left = app.world_mut().spawn(Transform::from_xyz(-0.1, 0.05, 0.0).with_rotation(rotation)).id();
        let right = app.world_mut().spawn(Transform::from_xyz(0.1, 0.05, 0.0).with_rotation(rotation * Quat::from_rotation_y(std::f32::consts::PI))).id();
        let character = app.world_mut().spawn((AlsCharacter, LocomotionState::Grounded, FootprintDiagnostics::default(),
            AlsCharacterAssembly { avatar, camera: None }, AlsFootBones { left_thigh: left, left_calf: left, left,
                left_target: None, left_virtual_target: None, right_thigh: right, right_calf: right, right,
                right_target: None, right_virtual_target: None })).id();
        app.update();
        app.update();
        let emit = |world: &mut World, side: &str, spawn: bool, skip_air: bool| {
            world.resource_mut::<Messages<AlsAnimationEvent>>().write(AlsAnimationEvent {
                character, instance_id: 1, source_asset: "source fixture".into(), graph_clip: "source fixture".into(), event_id: "step".into(),
                class_name: "AlsAnimNotify_FootstepEffects".into(), name: "Step".into(), phase: als_ext::EventPhase::Point,
                time: 0.5, weight: 1.0, percentage: 1.0,
                parameters: serde_json::json!({"FootstepEffectsSettings": FOOTPRINT.notify_settings_reference,
                    "FootBone": {"value": format!("EAlsFootBone::{side}")}, "bSpawnDecal": spawn, "bSkipEffectsWhenInAir": skip_air}),
            });
        };
        emit(app.world_mut(), "Left", true, false);
        emit(app.world_mut(), "Right", true, false);
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        assert_eq!(app.world().get::<FootprintDiagnostics>(character).unwrap().spawned, [1, 1]);
        for (_, parent) in app.world_mut().query::<(&Footprint, &ChildOf)>().iter(app.world()) { assert_eq!(parent.parent(), floor); }
        app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
        *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Air;
        emit(app.world_mut(), "Left", false, false);
        emit(app.world_mut(), "Right", true, true);
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        assert_eq!(app.world().get::<FootprintDiagnostics>(character).unwrap().rejected_flag, 1);
        assert_eq!(app.world().get::<FootprintDiagnostics>(character).unwrap().rejected_air, 1);
        app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
        *app.world_mut().get_mut::<LocomotionState>(character).unwrap() = LocomotionState::Grounded;
        app.world_mut().get_mut::<Transform>(left).unwrap().rotation = Quat::IDENTITY;
        app.update();
        emit(app.world_mut(), "Left", true, false);
        app.world_mut().run_system_once(spawn_footprints).unwrap();
        let diagnostics = app.world().get::<FootprintDiagnostics>(character).unwrap();
        assert_eq!(diagnostics.fallback_hits, 1);
        assert_eq!(diagnostics.rejected_angle, 1);
        assert_eq!(diagnostics.invalid, 0);
    }

    #[test]
    fn footprint_generated_settings_and_images_preserve_source_channels() {
        assert_eq!(FOOTPRINT.trace_distance, 0.5);
        assert_eq!(FOOTPRINT.effects[0].location_offset, Vec3::new(0.0, -0.0175, -0.1));
        assert_eq!(FOOTPRINT.notify_defaults["bSpawnDecal"].as_bool(), Some(true));
        let decode = |bytes| Image::from_buffer(bytes, ImageType::Extension("tga"),
            CompressedImageFormats::NONE, false, ImageSampler::default(), RenderAssetUsages::all()).unwrap();
        let source = decode(include_bytes!("../assets/textures/footprint/T_Als_Footprint_M.tga"));
        assert_eq!(source.size(), UVec2::splat(512));
        let data = source.data.unwrap();
        assert_eq!(&data[..4], [0, 0, 255, 0]);
        assert_eq!(&data[(256 * 512 + 256) * 4..][..4], [0, 0, 255, 255]);
        assert!(data.chunks_exact(4).any(|pixel| pixel[1] == 255 && pixel[3] == 255));
        for rotation in FOOTPRINT.effects[0].rotation_offsets {
            assert!((rotation.length_squared() - 1.0).abs() < 1e-6);
        }
    }
}
