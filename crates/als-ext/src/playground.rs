use std::path::PathBuf;

use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use ron::de::from_str;
use serde::Deserialize;

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

    let placeholder_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.55, 0.55, 0.6, 0.45),
        alpha_mode: AlphaMode::Blend,
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
                    .and_then(|c| {
                        body_plan_from_comp(
                            c,
                            actor.location,
                            actor.scale,
                            actor_rotator(actor),
                        )
                    })
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
                    let entity = spawn_box_visual(
                        &mut commands,
                        &mut meshes,
                        placeholder_mat.clone(),
                        transform,
                        plan.half_extents_model,
                        format!("MovingHook:{}", actor.name),
                    );
                    commands.entity(entity).insert(PlaygroundMovingObject);
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
                        placeholder_mat.clone(),
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
                        placeholder_mat.clone(),
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
    }

    for actor in &manifest.actors {
        if actor.class == "Landscape" || actor.class == "LandscapeProxy" {
            spawn_landscape(&mut commands, &mut meshes, &mut materials, actor.location);
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

/// Oscillates a moving object between its spawn point and a target offset.
pub fn playground_moving_object_system(
    time: Res<Time>,
    mut q: Query<(&mut Transform, &PlaygroundMovingOffset), With<PlaygroundMovingObject>>,
) {
    let t = time.elapsed_secs_f64() as f32;
    for (mut tf, offset) in q.iter_mut() {
        let phase = (t / offset.period_secs * std::f32::consts::PI).sin();
        let disp = offset.axis * offset.distance_m * phase;
        tf.translation = offset.base + disp;
    }
}

#[derive(Component)]
pub struct PlaygroundMovingOffset {
    pub axis: Vec3,
    pub distance_m: f32,
    pub period_secs: f32,
    pub base: Vec3,
}

pub fn spawn_landscape(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    location_ue: [f32; 3],
) {
    let pos = ue_point_to_bevy(Vec3::from(location_ue));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.45, 0.28),
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((
        Name::new("Landscape1"),
        Transform::from_translation(pos),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(100.0, 0.2, 100.0)))),
        MeshMaterial3d(mat),
        PlaygroundLandscape,
    ));
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
    let color_arr = lighting.sun_color.unwrap_or([1.0, 0.96, 0.9, 1.0]);
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
