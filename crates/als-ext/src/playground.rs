use std::path::PathBuf;

use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use ron::de::from_str;
use serde::Deserialize;

use crate::playground_materials::PlaygroundMaterialLibrary;

pub struct PlaygroundPlugin {
    pub manifest: PathBuf,
}

impl Plugin for PlaygroundPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PlaygroundManifestPath(self.manifest.clone()))
            .add_systems(Startup, playground_setup_system)
            .add_systems(Update, playground_moving_object_system);
    }
}

#[derive(Resource)]
pub struct PlaygroundManifestPath(pub PathBuf);

#[derive(Debug, Deserialize)]
pub struct PlaygroundManifest {
    #[serde(default)]
    pub actors: Vec<PlaygroundActor>,
    #[serde(default)]
    pub lighting: Option<SceneLighting>,
    #[serde(default)]
    pub sun_rotator: Option<PlaygroundRotator>,
    #[serde(default)]
    pub terrain: Option<SceneTerrain>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SceneTerrain {
    pub origin_ue_x_cm: f32,
    pub origin_ue_y_cm: f32,
    pub step_cm: f32,
    pub heights_cm: Vec<Vec<f32>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SceneLighting {
    pub sun_direction: Option<[f32; 3]>,
    pub sun_intensity: Option<f32>,
    pub sun_color: Option<[f32; 4]>,
    pub ambient_brightness: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct PlaygroundActor {
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub location: [f32; 3],
    #[serde(default)]
    pub rotation_yaw: f32,
    #[serde(default)]
    pub rotation: Option<PlaygroundRotator>,
    #[serde(default)]
    pub scale: [f32; 3],
    #[serde(default)]
    pub bounds_origin: Option<[f32; 3]>,
    #[serde(default)]
    pub bounds_extent: Option<[f32; 3]>,
    #[serde(default, rename = "mesh")]
    pub mesh_path: Option<String>,
    #[serde(default)]
    pub components: Vec<PlaygroundComponent>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub text_color: Option<[f32; 4]>,
    #[serde(default)]
    pub text_world_size: Option<f32>,
    #[serde(default)]
    pub top_color: Option<[f32; 3]>,
    #[serde(default)]
    pub side_color: Option<[f32; 3]>,
    #[serde(default)]
    pub move_duration: Option<f32>,
    #[serde(default)]
    pub rotate_speed_deg: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct PlaygroundComponent {
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "mesh")]
    pub mesh_path: Option<String>,
    #[serde(default)]
    pub location: Option<[f32; 3]>,
    #[serde(default)]
    pub rotation: Option<PlaygroundRotator>,
    #[serde(default)]
    pub scale: Option<[f32; 3]>,
    #[serde(default)]
    pub local_box_origin: Option<[f32; 3]>,
    #[serde(default)]
    pub local_box_extent: Option<[f32; 3]>,
    #[serde(default)]
    pub world_aabb_center: Option<[f32; 3]>,
    #[serde(default)]
    pub world_aabb_extent: Option<[f32; 3]>,
}

#[derive(Debug, Deserialize)]
pub struct PlaygroundRotator {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

#[derive(Component, Debug)]
pub struct PlaygroundStatic;

#[derive(Component, Debug)]
pub struct PlaygroundBoxCollider {
    pub half_extents: Vec3,
}

#[derive(Component, Debug)]
pub struct PlaygroundMovingObject;

#[derive(Debug, Clone, Copy)]
pub enum PlaygroundKinematicKind {
    MovePingPong { duration: f32 },
    SpinYawDegPerSec { speed_deg: f32 },
}

#[derive(Component, Debug)]
pub struct PlaygroundKinematic {
    pub base_transform: Transform,
    pub kind: PlaygroundKinematicKind,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct PlaygroundSpawnPoint {
    pub position: Vec3,
    pub rotation: Quat,
}

const CM_TO_M: f32 = 0.01;

const SKIP_CLASSES: &[&str] = &[
    "WorldSettings",
    "RecastNavMesh",
    "NavMeshBoundsVolume",
    "LandscapeGizmoActiveActor",
    "BP_Sky_Sphere_C",
    "SkyAtmosphere",
    "SkyLight",
    "AmbientSound",
    "PostProcessVolume",
    "DirectionalLight",
    "BuoyancyManager",
    "DefaultPhysicsVolume",
    "GameplayDebuggerPlayerManager",
    "AbstractNavData",
    "L_Als_Playground_C",
    "B_Als_AICharacter_C",
];

fn is_skipped(class: &str) -> bool {
    SKIP_CLASSES.contains(&class)
}

pub fn ue_point_to_bevy(p_cm: Vec3) -> Vec3 {
    Vec3::new(p_cm.x * CM_TO_M, p_cm.z * CM_TO_M, p_cm.y * CM_TO_M)
}

pub fn ue_extents_to_bevy(e_cm: Vec3) -> Vec3 {
    Vec3::new(e_cm.x * CM_TO_M, e_cm.z * CM_TO_M, e_cm.y * CM_TO_M)
}

pub fn ue_quat_to_bevy(q: Quat) -> Quat {
    Quat::from_xyzw(-q.x, -q.z, -q.y, q.w)
}

pub fn ue_rotator_to_bevy_quat(pitch_deg: f32, yaw_deg: f32, roll_deg: f32) -> Quat {
    let pitch = pitch_deg.to_radians();
    let yaw = yaw_deg.to_radians();
    let roll = roll_deg.to_radians();
    let qx = Quat::from_rotation_x(roll);
    let qy = Quat::from_rotation_y(-pitch);
    let qz = Quat::from_rotation_z(yaw);
    ue_quat_to_bevy(qz * qy * qx)
}

pub fn ue_yaw_to_bevy_quat(yaw_deg: f32) -> Quat {
    ue_rotator_to_bevy_quat(0.0, yaw_deg, 0.0)
}

fn gltf_asset_base(mesh_path: &str) -> Option<String> {
    if !mesh_path.contains(".SM_Als_") {
        return None;
    }
    let file = mesh_path.rsplit('/').next()?;
    let base = file.split('.').next()?;
    Some(base.to_string())
}

pub fn is_engine_basic(mesh_path: &str) -> bool {
    mesh_path.starts_with("/Engine/BasicShapes/")
}

struct BodyPlan {
    loc_cm: Vec3,
    quat: Quat,
    half_extents_model: Vec3,
    center_offset_model: Vec3,
    center_world: Vec3,
    obb: bool,
    mesh_base: Option<String>,
}

fn actor_rotator(actor: &PlaygroundActor) -> (f32, f32, f32) {
    match &actor.rotation {
        Some(r) => (r.pitch, r.yaw, r.roll),
        None => (0.0, actor.rotation_yaw, 0.0),
    }
}

fn comp_rotator(comp: &PlaygroundComponent, fallback: (f32, f32, f32)) -> (f32, f32, f32) {
    match &comp.rotation {
        Some(r) => (r.pitch, r.yaw, r.roll),
        None => fallback,
    }
}

fn body_plan_from_comp(
    comp: &PlaygroundComponent,
    actor_loc: [f32; 3],
    actor_scale: [f32; 3],
    rotator: (f32, f32, f32),
) -> Option<BodyPlan> {
    let loc_arr = comp.location.unwrap_or(actor_loc);
    let scale_arr = comp.scale.unwrap_or(actor_scale);
    let loc_cm = Vec3::from(loc_arr);
    let scale = Vec3::from(scale_arr);
    let rot = comp_rotator(comp, rotator);
    let quat = ue_rotator_to_bevy_quat(rot.0, rot.1, rot.2);
    let mesh_base = comp.mesh_path.as_deref().and_then(gltf_asset_base);

    if let (Some(org), Some(ext)) = (comp.local_box_origin, comp.local_box_extent) {
        return Some(BodyPlan {
            loc_cm,
            quat,
            half_extents_model: ue_extents_to_bevy(Vec3::from(ext) * scale),
            center_offset_model: ue_point_to_bevy(Vec3::from(org) * scale),
            center_world: ue_point_to_bevy(loc_cm),
            obb: true,
            mesh_base,
        });
    }

    if let (Some(c), Some(_e)) = (comp.world_aabb_center, comp.world_aabb_extent) {
        return Some(BodyPlan {
            loc_cm,
            quat,
            half_extents_model: comp
                .world_aabb_extent
                .map(|e| ue_extents_to_bevy(Vec3::from(e)))
                .unwrap_or(Vec3::ONE),
            center_offset_model: Vec3::ZERO,
            center_world: ue_point_to_bevy(Vec3::from(c)),
            obb: false,
            mesh_base,
        });
    }

    None
}

fn body_plan_from_actor_bounds(actor: &PlaygroundActor) -> Option<BodyPlan> {
    let org = actor.bounds_origin?;
    let ext = actor.bounds_extent?;
    let rotator = actor_rotator(actor);
    Some(BodyPlan {
        loc_cm: Vec3::from(org),
        quat: ue_rotator_to_bevy_quat(rotator.0, rotator.1, rotator.2),
        half_extents_model: ue_extents_to_bevy(Vec3::from(ext)),
        center_offset_model: Vec3::ZERO,
        center_world: ue_point_to_bevy(Vec3::from(org)),
        obb: true,
        mesh_base: None,
    })
}

fn infer_block_mesh(actor: &PlaygroundActor) -> Option<String> {
    let ext = actor.bounds_extent?;
    let max_cm = ext[0].max(ext[1]).max(ext[2]);
    if max_cm <= 125.0 {
        Some("SM_Als_Block_2x2".to_string())
    } else {
        Some("SM_Als_Block_3x3".to_string())
    }
}

fn spawn_box_visual(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    placeholder: Handle<StandardMaterial>,
    transform: Transform,
    half_extents: Vec3,
    name: String,
) -> Entity {
    commands
        .spawn((
            Name::new(name),
            transform,
            Mesh3d(meshes.add(Mesh::from(Cuboid::new(
                half_extents.x * 2.0,
                half_extents.y * 2.0,
                half_extents.z * 2.0,
            )))),
            MeshMaterial3d(placeholder),
            PlaygroundStatic,
            PlaygroundBoxCollider { half_extents },
        ))
        .id()
}

pub fn playground_setup_system(
    manifest_path: Res<PlaygroundManifestPath>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    material_library: Res<PlaygroundMaterialLibrary>,
) {
    let text = match std::fs::read_to_string(&manifest_path.0) {
        Ok(t) => t,
        Err(e) => {
            error!(
                "PLAYGROUND manifest read failed path={:?} err={e}",
                manifest_path.0
            );
            return;
        }
    };
    let manifest: PlaygroundManifest = match from_str(&text) {
        Ok(m) => m,
        Err(e) => {
            error!("PLAYGROUND manifest parse failed err={e}");
            return;
        }
    };


    let tiles_lookup = material_library.clone();

    let default_solid_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.92, 0.94),
        base_color_texture: tiles_lookup.tiles_base_color.clone(),
        normal_map_texture: tiles_lookup.tiles_normal.clone(),
        perceptual_roughness: 0.85,
        ..default()
    });

    let mut spawned_static = 0usize;
    let mut spawned_moving = 0usize;
    let mut skipped = 0usize;
    let mut spawn_point_found = false;
    let mut spawned_landscape = 0usize;
    let mut spawned_text = 0usize;

    for actor in &manifest.actors {
        if is_skipped(&actor.class) {
            skipped += 1;
            continue;
        }
        match actor.class.as_str() {
            "PlayerStart" => {
                commands.insert_resource(PlaygroundSpawnPoint {
                    position: ue_point_to_bevy(Vec3::from(actor.location)),
                    rotation: ue_yaw_to_bevy_quat(actor.rotation_yaw),
                });
                spawn_point_found = true;
                continue;
            }
            "B_Als_MovingObject_C" => {
                let plan = actor
                    .components
                    .first()
                    .and_then(|c| body_plan_from_comp(c, actor.location, actor.scale, actor_rotator(actor)))
                    .or_else(|| body_plan_from_actor_bounds(actor));
                if let Some(plan) = plan {
                    let transform = if plan.obb {
                        Transform::from_translation(
                            ue_point_to_bevy(plan.loc_cm) + plan.quat * plan.center_offset_model,
                        )
                        .with_rotation(plan.quat)
                    } else {
                        Transform::from_translation(plan.center_world)
                    };
                    let mesh_base = plan
                        .mesh_base
                        .clone()
                        .or_else(|| infer_block_mesh(actor));
                    let mesh_scene =
                        mesh_base
                            .as_deref()
                            .map(|base| assets.load(format!("models/playground/{base}.gltf#Scene0")));
                    let duration = actor.move_duration.unwrap_or(2.0);
                    let entity = commands
                        .spawn((
                            Name::new(format!("Moving:{}", actor.name)),
                            transform,
                            PlaygroundStatic,
                            PlaygroundMovingObject,
                            PlaygroundKinematic {
                                base_transform: transform,
                                kind: PlaygroundKinematicKind::MovePingPong { duration },
                            },
                        ))
                        .id();
                    match mesh_scene {
                        Some(scene) => {
                            commands.entity(entity).insert(WorldAssetRoot(scene));
                        }
                        None => {
                            commands.entity(entity).insert((
                                Mesh3d(meshes.add(Mesh::from(Cuboid::new(
                                    plan.half_extents_model.x * 2.0,
                                    plan.half_extents_model.y * 2.0,
                                    plan.half_extents_model.z * 2.0,
                                )))),
                                MeshMaterial3d(default_solid_mat.clone()),
                            ));
                        }
                    }
                    commands.entity(entity).with_children(|p| {
                        p.spawn((Name::new("Moving:collider"), Transform::IDENTITY, PlaygroundBoxCollider {
                            half_extents: plan.half_extents_model,
                        }));
                    });
                    spawned_moving += 1;
                }
                continue;
            }
            "B_Als_RotatingObject_C" => {
                let plan = actor
                    .components
                    .first()
                    .and_then(|c| body_plan_from_comp(c, actor.location, actor.scale, actor_rotator(actor)))
                    .or_else(|| body_plan_from_actor_bounds(actor));
                if let Some(plan) = plan {
                    let center = plan.center_world + plan.quat * plan.center_offset_model;
                    let speed_deg = actor.rotate_speed_deg.unwrap_or(100.0);
                    let mesh_base = plan
                        .mesh_base
                        .clone()
                        .or_else(|| infer_block_mesh(actor));
                    let mesh_scene =
                        mesh_base
                            .as_deref()
                            .map(|base| assets.load(format!("models/playground/{base}.gltf#Scene0")));
                    let entity = commands
                        .spawn((
                            Name::new(format!("Rotating:{}", actor.name)),
                            Transform::from_translation(center).with_rotation(plan.quat),
                            PlaygroundStatic,
                            PlaygroundMovingObject,
                            PlaygroundKinematic {
                                base_transform: Transform::from_translation(center)
                                    .with_rotation(plan.quat),
                                kind: PlaygroundKinematicKind::SpinYawDegPerSec { speed_deg },
                            },
                        ))
                        .id();
                    match mesh_scene {
                        Some(scene) => {
                            commands.entity(entity).insert(WorldAssetRoot(scene));
                        }
                        None => {
                            commands.entity(entity).insert((
                                Mesh3d(meshes.add(Mesh::from(Cuboid::new(
                                    plan.half_extents_model.x * 2.0,
                                    plan.half_extents_model.y * 2.0,
                                    plan.half_extents_model.z * 2.0,
                                )))),
                                MeshMaterial3d(default_solid_mat.clone()),
                            ));
                        }
                    }
                    commands.entity(entity).with_children(|p| {
                        p.spawn((Name::new("Rotating:collider"), Transform::IDENTITY, PlaygroundBoxCollider {
                            half_extents: plan.half_extents_model,
                        }));
                    });
                    spawned_moving += 1;
                }
                continue;
            }
            _ => {}
        }

        let rotator = actor_rotator(actor);
        let mut plans: Vec<BodyPlan> = actor
            .components
            .iter()
            .filter_map(|c| body_plan_from_comp(c, actor.location, actor.scale, rotator))
            .collect();
        if plans.is_empty() {
            if let Some(p) = body_plan_from_actor_bounds(actor) {
                plans.push(p);
            }
        }
        if plans.is_empty() {
            warn!("PLAYGROUND no geometry for {} {}", actor.class, actor.name);
            skipped += 1;
            continue;
        }

        for plan in &plans {
            let instance_mat = actor.top_color.map(|tc| {
                materials.add(StandardMaterial {
                    base_color: Color::srgb(tc[0], tc[1], tc[2]),
                    base_color_texture: tiles_lookup.tiles_base_color.clone(),
                    normal_map_texture: tiles_lookup.tiles_normal.clone(),
                    perceptual_roughness: 0.85,
                    ..default()
                })
            });
            let visual_mat = instance_mat.unwrap_or_else(|| default_solid_mat.clone());
            match (&plan.mesh_base, plan.obb) {
                (Some(base), true) => {
                    let scene = assets.load(format!("models/playground/{base}.gltf#Scene0"));
                    let root_tf =
                        Transform::from_translation(ue_point_to_bevy(plan.loc_cm))
                            .with_rotation(plan.quat);
                    let offset = plan.center_offset_model;
                    commands
                        .spawn((
                            Name::new(format!("{base}:{}", actor.name)),
                            root_tf,
                            WorldAssetRoot(scene),
                            PlaygroundStatic,
                            PlaygroundMaterialTint(visual_mat),
                        ))
                        .with_children(|p| {
                            p.spawn((
                                Name::new(format!("{base}:collider")),
                                Transform::from_translation(offset),
                                PlaygroundBoxCollider {
                                    half_extents: plan.half_extents_model,
                                },
                            ));
                        });
                    spawned_static += 1;
                }
                (Some(base), false) => {
                    let scene = assets.load(format!("models/playground/{base}.gltf#Scene0"));
                    let root_tf =
                        Transform::from_translation(ue_point_to_bevy(plan.loc_cm))
                            .with_rotation(plan.quat);
                    commands
                        .spawn((
                            Name::new(format!("{base}:{}", actor.name)),
                            root_tf,
                            WorldAssetRoot(scene),
                            PlaygroundStatic,
                            PlaygroundMaterialTint(visual_mat),
                        ))
                        .with_children(|p| {
                            p.spawn((
                                Name::new(format!("{}:aabb", actor.name)),
                                Transform::from_translation(
                                    plan.center_world - ue_point_to_bevy(plan.loc_cm),
                                ),
                                PlaygroundBoxCollider {
                                    half_extents: plan.half_extents_model,
                                },
                            ));
                        });
                    spawned_static += 1;
                }
                (None, true) => {
                    let center = plan.center_world + plan.quat * plan.center_offset_model;
                    spawn_box_visual(
                        &mut commands,
                        &mut meshes,
                        default_solid_mat.clone(),
                        Transform::from_translation(center).with_rotation(plan.quat),
                        plan.half_extents_model,
                        format!("{}:box", actor.name),
                    );
                    spawned_static += 1;
                }
                (None, false) => {
                    spawn_box_visual(
                        &mut commands,
                        &mut meshes,
                        default_solid_mat.clone(),
                        Transform::from_translation(plan.center_world),
                        plan.half_extents_model,
                        format!("{}:box", actor.name),
                    );
                    spawned_static += 1;
                }
            }
        }
    }

    if let Some(lighting) = &manifest.lighting {
        spawn_sun_light(&mut commands, lighting);
    } else if let Some(rot) = &manifest.sun_rotator {
        commands.spawn((
            Name::new("SunLight"),
            DirectionalLight {
                illuminance: 30_000.0,
                ..default()
            },
            Transform::from_rotation(ue_rotator_to_bevy_quat(rot.pitch, rot.yaw, rot.roll)),
        ));
    }

    for actor in &manifest.actors {
        if actor.class == "Landscape" || actor.class == "LandscapeProxy" {
            let terrain = manifest.terrain.as_ref().map(|t| PlaygroundTerrainData {
                origin_ue: [t.origin_ue_x_cm, t.origin_ue_y_cm],
                step_cm: t.step_cm,
                rows_ue_y: t.heights_cm.len(),
                cols_ue_x: t.heights_cm.first().map(|r| r.len()).unwrap_or(0),
                heights_cm: t.heights_cm.clone(),
            });
            let pos = ue_point_to_bevy(Vec3::from(actor.location));
            let mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.25, 0.5, 0.125),
                base_color_texture: material_library.tiles_base_color.clone(),
                normal_map_texture: material_library.tiles_normal.clone(),
                perceptual_roughness: 0.95,
                ..default()
            });
            let entity = commands
                .spawn((Name::new(format!("Landscape:{}", actor.name)),))
                .id();
            let grid = terrain.as_ref();
            spawn_landscape_grid(&mut commands, &mut meshes, &mut materials, mat, entity, grid, pos);
            spawned_landscape += 1;
            continue;
        }
        if actor.class.starts_with("TextRenderActor") || actor.class == "TextRenderActor" {
            spawn_text_label(&mut commands, actor);
            spawned_text += 1;
            continue;
        }
    }

    info!(
        "PLAYGROUND setup done actors={} static_bodies={spawned_static} moving_hooks={spawned_moving} skipped={skipped} spawn_point={spawn_point_found} landscape={spawned_landscape} text={spawned_text}",
        manifest.actors.len()
    );
}

#[derive(Component)]
pub struct PlaygroundTextLabel;

#[derive(Component)]
pub struct PlaygroundLandscape;

/// Ping-pong spline-like motion and constant spin, per B_Als_MovingObject /
/// B_Als_RotatingObject event graphs (ServerWorldTime-driven, modulo ping-pong).
pub fn playground_moving_object_system(
    time: Res<Time>,
    mut q: Query<(&mut Transform, &PlaygroundKinematic), With<PlaygroundMovingObject>>,
) {
    let t = time.elapsed_secs_f64() as f32;
    for (mut tf, kin) in q.iter_mut() {
        match kin.kind {
            PlaygroundKinematicKind::SpinYawDegPerSec { speed_deg } => {
                let yaw = speed_deg * t;
                tf.rotation = kin.base_transform.rotation
                    * Quat::from_rotation_y(yaw.to_radians());
            }
            PlaygroundKinematicKind::MovePingPong { duration } => {
                if duration <= 0.0 {
                    continue;
                }
                let total = t / duration;
                let frac = total % 2.0;
                let s = if frac > 1.0 { 2.0 - frac } else { frac };
                let eased = s * s * (3.0 - 2.0 * s);
                let base = kin.base_transform.translation;
                let axis = Vec3::Z;
                let span_m = 4.0;
                tf.translation =
                    base + axis * (eased * span_m * 0.5);
            }
        }
    }
}

#[derive(Component, Debug)]
pub struct PlaygroundMaterialTint(pub Handle<StandardMaterial>);

#[derive(Component, Debug, Clone, Copy)]
pub struct PlaygroundTintApplied;

#[derive(Resource, Debug, Clone)]
pub struct PlaygroundTerrainData {
    pub origin_ue: [f32; 2],
    pub step_cm: f32,
    pub rows_ue_y: usize,
    pub cols_ue_x: usize,
    pub heights_cm: Vec<Vec<f32>>,
}

pub fn spawn_landscape(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    location_ue: [f32; 3],
) {
    let pos = ue_point_to_bevy(Vec3::from(location_ue));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.5, 0.125),
        perceptual_roughness: 0.95,
        ..default()
    });
    let data = commands
        .spawn((Name::new("Landscape1"), Transform::from_translation(pos)))
        .id();
    spawn_landscape_grid(commands, meshes, materials, mat, data, None, pos);
}

#[allow(clippy::too_many_arguments)]
fn spawn_landscape_grid(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    mat: Handle<StandardMaterial>,
    entity: Entity,
    terrain: Option<&PlaygroundTerrainData>,
    pos: Vec3,
) {
    let rows = terrain.map(|t| t.rows_ue_y).unwrap_or(64);
    let cols = terrain.map(|t| t.cols_ue_x).unwrap_or(64);
    let step_m = terrain
        .map(|t| (t.step_cm * CM_TO_M) as f32)
        .unwrap_or(120.0 / 64.0);
    let height_at = |ri: usize, ci: usize| -> f32 {
        match terrain {
            Some(t) => {
                let r = ri.min(t.heights_cm.len() - 1);
                let row = &t.heights_cm[r];
                let c = ci.min(row.len() - 1);
                row[c] * CM_TO_M
            }
            None => terrain_height(
                -60.0 + ci as f32 * 120.0 / 64.0,
                -60.0 + ri as f32 * 120.0 / 64.0,
            ),
        }
    };
    let mut positions = Vec::with_capacity(rows * cols);
    let mut uvs = Vec::with_capacity(rows * cols);
    for ri in 0..rows {
        for ci in 0..cols {
            let lx = ci as f32 * step_m;
            let lz = ri as f32 * step_m;
            let h = height_at(ri, ci);
            positions.push([lx, h, lz]);
            uvs.push([ci as f32 / (cols - 1).max(1) as f32, ri as f32 / (rows - 1).max(1) as f32]);
        }
    }
    let mut indices: Vec<u32> = Vec::with_capacity((rows - 1) * (cols - 1) * 6);
    for ri in 0..rows - 1 {
        for ci in 0..cols - 1 {
            let a = (ri * cols + ci) as u32;
            let b = a + 1;
            let c = ((ri + 1) * cols + ci) as u32;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    let per_tri = 3usize;
    let tri_count = indices.len() / 3;
    let mut normals = vec![[0.0f32, 1.0, 0.0]; positions.len()];
    for ti in 0..tri_count {
        let ia = indices[ti * per_tri] as usize;
        let ib = indices[ti * per_tri + 1] as usize;
        let ic = indices[ti * per_tri + 2] as usize;
        let pa = Vec3::from_array(positions[ia]);
        let pb = Vec3::from_array(positions[ib]);
        let pc = Vec3::from_array(positions[ic]);
        let n = (pb - pa).cross(pc - pa).normalize_or_zero();
        for idx in [ia, ib, ic] {
            let existing = Vec3::from_array(normals[idx]);
            normals[idx] = (existing + n).normalize_or_zero().to_array();
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

    commands.entity(entity).insert((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(mat),
        PlaygroundLandscape,
        Transform::from_translation(pos),
    ));
}

pub fn terrain_height(x: f32, z: f32) -> f32 {
    let h = 0.35 * ((x * 0.06).sin() * (z * 0.05).cos())
        + 0.20 * ((x * 0.12 + z * 0.09).sin() + (z * 0.15).sin())
        + 0.08 * ((x * 0.3).sin() * (z * 0.25).cos());
    h.max(-0.5)
}

fn spawn_text_label(
    commands: &mut Commands,
    actor: &PlaygroundActor,
) {
    let text_str = actor.text.clone().unwrap_or_default();
    if text_str.is_empty() {
        return;
    }
    let pos = ue_point_to_bevy(Vec3::from(actor.location));
    let color_arr = actor.text_color.unwrap_or([1.0, 1.0, 1.0, 1.0]);
    let size = actor.text_world_size.unwrap_or(30.0) * CM_TO_M * 8.0;
    commands.spawn((
        Name::new(format!("Text:{}", actor.name)),
        Text2d::new(text_str),
        Transform::from_translation(pos).with_rotation(
            ue_rotator_to_bevy_quat(
                actor.rotation.as_ref().map(|r| r.pitch).unwrap_or(0.0),
                actor.rotation_yaw,
                actor.rotation.as_ref().map(|r| r.roll).unwrap_or(0.0),
            ),
        ),
        TextFont { font_size: bevy::text::FontSize::Px(size.max(12.0)), ..default() },
        TextColor(Color::srgba(color_arr[0], color_arr[1], color_arr[2], color_arr[3])),
        PlaygroundTextLabel,
    ));
}

fn spawn_sun_light(commands: &mut Commands, lighting: &SceneLighting) {
    let dir = lighting
        .sun_direction
        .map(|v| Vec3::from(v).normalize_or_zero())
        .unwrap_or(Vec3::new(-0.4, -0.8, 0.3).normalize_or_zero());
    let intensity = lighting.sun_intensity.unwrap_or(80_000.0);
    let _color_arr = lighting.sun_color.unwrap_or([1.0, 0.96, 0.9, 1.0]);
    let rotation = Quat::from_rotation_arc(Vec3::NEG_Z, dir);
    commands.spawn((
        Name::new("SunLight"),
        DirectionalLight {
            illuminance: intensity,
            ..default()
        },
        Transform::default().with_rotation(rotation),
    ));
    let ambient = lighting.ambient_brightness.unwrap_or(400.0);
    commands.insert_resource(GlobalAmbientLight {
        brightness: ambient,
        ..default()
    });
}


#[cfg(test)]
mod tests {
    use super::*;

    fn map_dir(v: Vec3) -> Vec3 {
        ue_point_to_bevy(v) * 100.0
    }

    fn assert_vec3_eq(a: Vec3, b: Vec3, eps: f32) {
        assert!(a.distance(b) < eps, "vec mismatch left={a:?} right={b:?}");
    }

    fn assert_quat_eq(a: Quat, b: Quat, eps: f32) {
        let d = a.dot(b).abs();
        assert!(
            (d - 1.0).abs() < eps,
            "quat mismatch left={a:?} right={b:?} dot={d}"
        );
    }

    #[test]
    fn point_mapping_swaps_yz_and_scales() {
        assert_vec3_eq(
            ue_point_to_bevy(Vec3::new(100.0, 200.0, 300.0)),
            Vec3::new(1.0, 3.0, 2.0),
            1e-5,
        );
    }

    #[test]
    fn extents_mapping_matches_point_mapping() {
        assert_vec3_eq(
            ue_extents_to_bevy(Vec3::new(25.0, 1000.0, 100.0)),
            ue_point_to_bevy(Vec3::new(25.0, 1000.0, 100.0)),
            1e-6,
        );
    }

    #[test]
    fn quat_conjugation_preserves_mapped_rotations() {
        let samples: [(f32, f32, f32); 5] = [
            (10.0, 30.0, 0.0),
            (90.0, 90.0, 0.0),
            (0.0, 45.0, 15.0),
            (-20.0, -120.0, 60.0),
            (0.0, 0.0, 25.95),
        ];
        for (pitch, yaw, roll) in samples {
            let q_ue = Quat::from_rotation_z(yaw.to_radians())
                * Quat::from_rotation_y(-pitch.to_radians())
                * Quat::from_rotation_x(roll.to_radians());
            let q_bevy = ue_quat_to_bevy(q_ue);
            for v in [
                Vec3::X,
                Vec3::Y,
                Vec3::Z,
                Vec3::new(1.0, 2.0, 3.0),
                Vec3::new(-4.0, 0.5, 2.0),
            ] {
                let via_ue = map_dir(q_ue.mul_vec3(v));
                let via_bevy = q_bevy.mul_vec3(map_dir(v));
                assert_vec3_eq(via_ue, via_bevy, 1e-4);
            }
        }
    }

    #[test]
    fn rotator_conversion_agrees_with_quat_rule() {
        for yaw in [-180.0_f32, -90.0, -45.0, 0.0, 30.0, 90.0, 179.5] {
            let expected = ue_quat_to_bevy(Quat::from_axis_angle(Vec3::Z, yaw.to_radians()));
            assert_quat_eq(ue_yaw_to_bevy_quat(yaw), expected, 1e-5);
            assert_quat_eq(ue_rotator_to_bevy_quat(0.0, yaw, 0.0), expected, 1e-5);
        }
    }

    #[test]
    fn player_start_lands_on_floor() {
        let pos = ue_point_to_bevy(Vec3::new(-1500.0, -1500.0, 92.0));
        assert_vec3_eq(pos, Vec3::new(-15.0, 0.92, -15.0), 1e-4);
    }

    #[test]
    fn wall_collider_bottom_rests_on_ground() {
        let half = ue_extents_to_bevy(Vec3::new(25.0, 1000.0, 100.0));
        let offset = ue_point_to_bevy(Vec3::new(0.0, 0.0, 100.0));
        let center_y = offset.y;
        assert!((center_y - half.y).abs() < 1e-5);
    }

    #[test]
    fn gltf_base_extraction() {
        assert_eq!(
            gltf_asset_base("/ALS/ALSExtras/Environment/Meshes/SM_Als_Rock.SM_Als_Rock")
                .as_deref(),
            Some("SM_Als_Rock")
        );
        assert!(gltf_asset_base("/Engine/BasicShapes/Cube.Cube").is_none());
    }

    #[test]
    fn engine_basic_detection() {
        assert!(is_engine_basic("/Engine/BasicShapes/Cube.Cube"));
        assert!(!is_engine_basic(
            "/ALS/ALSExtras/Environment/Meshes/SM_Als_Rock.SM_Als_Rock"
        ));
    }

    #[test]
    fn skip_classification() {
        assert!(!is_skipped("TextRenderActor"));
        assert!(!is_skipped("Landscape"));
        assert!(is_skipped("LandscapeGizmoActiveActor"));
        assert!(!is_skipped("B_Als_StaticObject_C"));
        assert!(!is_skipped("StaticMeshActor"));
        assert!(!is_skipped("Brush"));
    }

    #[test]
    fn manifest_deserializes_ron_snippet() {
        let src = "#![enable(implicit_some)]\n(actors: [(class: \"Brush\", name: \"Brush_0\", location: (1.0, 2.0, 3.0), rotation_yaw: 45.0, rotation: (pitch: 0.0, yaw: 45.0, roll: 0.0), scale: (1.0, 1.0, 1.0), bounds_origin: (1.0, 2.0, 103.0), bounds_extent: (150.0, 100.0, 100.0))])";
        let m: PlaygroundManifest = from_str(src).expect("parse");
        assert_eq!(m.actors.len(), 1);
        assert_eq!(m.actors[0].class, "Brush");
        assert_eq!(m.actors[0].bounds_extent.unwrap(), [150.0, 100.0, 100.0]);
        assert!(m.actors[0].components.is_empty());
    }
}
