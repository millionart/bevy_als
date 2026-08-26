use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::light::CascadeShadowConfig;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::text::TextLayoutInfo;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;

use als_ext::playground_materials::PlaygroundMaterialsPlugin;
use als_ext::{
    PlaygroundKinematic, PlaygroundKinematicKind, PlaygroundLandscape, PlaygroundMaterialTint,
    PlaygroundPlugin, PlaygroundSky, PlaygroundStatic, PlaygroundTextLabel,
    PlaygroundWorldTextMesh,
};

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        PhysicsPlugins::new(FixedPostUpdate),
        EnhancedInputPlugin,
        PlaygroundMaterialsPlugin,
        PlaygroundPlugin {
            manifest: "assets/models/playground/playground_scene.ron".into(),
        },
        AnimationGraphPlugin::default(),
    ))
    .init_resource::<SceneStats>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(Startup, (spawn_camera, arm_schedule))
    .add_systems(
        Update,
        (
            stats_collector,
            kinematic_probe,
            verdict_system,
            screenshot_runner,
            auto_quit,
        ),
    );

    app.run();
}

#[derive(Resource, Default)]
struct SceneStats {
    static_count: u32,
    text_count: u32,
    native_text_count: u32,
    glyph_text_count: u32,
    world_text_mesh_count: u32,
    world_text_masked_count: u32,
    landscape_seen: bool,
    landscape_identity: bool,
    sun_count: u32,
    sun_shadowed: bool,
    sky_count: u32,
    pbr_material_count: u32,
    movers_total: u32,
    movers_moving: u32,
    spin_yaw_abs_deg: f32,
    mover_prev: Vec<Vec3>,
    verdict_done: bool,
}

#[derive(Resource)]
struct Schedule {
    shot_at: Instant,
    shot_done: bool,
    quit_at: Instant,
    shot_path: String,
}

fn spawn_camera(mut commands: Commands) {
    let text_camera = std::env::var("ALS_SCENE_CAMERA").ok().as_deref() == Some("text");
    let transform = if text_camera {
        Transform::from_xyz(3.33, 8.78, -42.0)
            .looking_at(Vec3::new(3.33, 8.78, -46.24), Vec3::NEG_Y)
    } else {
        Transform::from_xyz(-35.0, 45.0, -60.0).looking_at(Vec3::new(-10.0, 0.0, -20.0), Vec3::Y)
    };
    commands.spawn((Camera3d::default(), transform));
}

fn arm_schedule(mut commands: Commands) {
    let path = std::env::var("ALS_SHOT_PATH")
        .unwrap_or_else(|_| "docs/bfscene-shots/gate-overview.png".to_string());
    let after = std::env::var("ALS_SHOT_AFTER_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6.0);
    let quit = std::env::var("ALS_BOOT_AUTO_QUIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20.0);
    commands.insert_resource(Schedule {
        shot_at: Instant::now() + Duration::from_secs_f64(after),
        shot_done: false,
        quit_at: Instant::now() + Duration::from_secs_f64(quit),
        shot_path: path,
    });
}

fn stats_collector(
    mut stats: ResMut<SceneStats>,
    statics: Query<(), With<PlaygroundStatic>>,
    texts: Query<(), With<PlaygroundTextLabel>>,
    native_texts: Query<(), (With<PlaygroundTextLabel>, With<Text2d>, With<Transform>)>,
    text_layouts: Query<&TextLayoutInfo, (With<PlaygroundTextLabel>, With<Text2d>)>,
    world_text_meshes: Query<&MeshMaterial3d<StandardMaterial>, With<PlaygroundWorldTextMesh>>,
    landscapes: Query<&Transform, With<PlaygroundLandscape>>,
    sun_lights: Query<(&DirectionalLight, &CascadeShadowConfig)>,
    skies: Query<(), With<PlaygroundSky>>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    materials: Res<Assets<StandardMaterial>>,
) {
    stats.static_count = statics.iter().count() as u32;
    stats.text_count = texts.iter().count() as u32;
    stats.native_text_count = native_texts.iter().count() as u32;
    stats.glyph_text_count = text_layouts
        .iter()
        .filter(|layout| !layout.glyphs.is_empty())
        .count() as u32;
    stats.world_text_mesh_count = world_text_meshes.iter().count() as u32;
    stats.world_text_masked_count = world_text_meshes
        .iter()
        .filter(|handle| {
            materials.get(&handle.0).is_some_and(|material| {
                material.base_color_texture.is_some()
                    && matches!(material.alpha_mode, AlphaMode::Mask(_))
                    && material.cull_mode.is_none()
            })
        })
        .count() as u32;
    if let Ok(transform) = landscapes.single() {
        stats.landscape_seen = true;
        stats.landscape_identity = transform.translation.length_squared() < 1e-6
            && transform.rotation.dot(Quat::IDENTITY).abs() > 1.0 - 1e-6
            && transform.scale.distance_squared(Vec3::ONE) < 1e-6;
    }
    stats.sun_count = sun_lights.iter().count() as u32;
    stats.sun_shadowed = sun_lights.iter().any(|(light, cascades)| {
        light.shadow_maps_enabled
            && cascades.bounds.len() == 4
            && cascades
                .bounds
                .last()
                .is_some_and(|distance| (*distance - 400.0).abs() < 0.01)
    });
    stats.sky_count = skies.iter().count() as u32;
    stats.pbr_material_count = mesh_materials
        .iter()
        .filter(|handle| {
            materials.get(&handle.0).is_some_and(|material| {
                material.base_color_texture.is_some() && material.normal_map_texture.is_some()
            })
        })
        .count() as u32;
}

fn kinematic_probe(
    time: Res<Time>,
    mut stats: ResMut<SceneStats>,
    movings: Query<(&Transform, &PlaygroundKinematic)>,
) {
    if stats.mover_prev.len() != movings.iter().count() {
        stats.mover_prev = movings.iter().map(|(tf, _)| tf.translation).collect();
        return;
    }
    let mut moving = 0u32;
    for (i, (tf, kin)) in movings.iter().enumerate() {
        match kin.kind {
            PlaygroundKinematicKind::MovePingPong { .. } => {
                if stats.mover_prev[i].distance(tf.translation) > 0.005 {
                    moving += 1;
                }
            }
            PlaygroundKinematicKind::SpinYawDegPerSec { speed_deg } => {
                let dt = time.delta_secs();
                stats.spin_yaw_abs_deg += (speed_deg * dt).abs();
                if stats.spin_yaw_abs_deg > 1.0 {
                    moving += 1;
                }
            }
        }
    }
    stats.movers_moving = moving.max(stats.movers_moving);
    stats.movers_total = movings.iter().count() as u32;
    for (i, (tf, _)) in movings.iter().enumerate() {
        stats.mover_prev[i] = tf.translation;
    }
}

fn screenshot_runner(
    mut commands: Commands,
    mut schedule: ResMut<Schedule>,
    roots: Query<(Entity, &PlaygroundMaterialTint)>,
    children_query: Query<&Children>,
    mesh_material: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    fn walk_and_tint(
        entity: Entity,
        tint: &Handle<StandardMaterial>,
        commands: &mut Commands,
        children_query: &Query<&Children>,
        mesh_material: &Query<&MeshMaterial3d<StandardMaterial>>,
        found: &mut u32,
    ) {
        if let Ok(children) = children_query.get(entity) {
            for child in children.iter() {
                if mesh_material.contains(child) {
                    commands.entity(child).insert(MeshMaterial3d(tint.clone()));
                    *found += 1;
                }
                walk_and_tint(child, tint, commands, children_query, mesh_material, found);
            }
        }
    }

    for (root, tint_handle) in &roots {
        let mut found = 0u32;
        walk_and_tint(
            root,
            &tint_handle.0,
            &mut commands,
            &children_query,
            &mesh_material,
            &mut found,
        );
        if found > 0 {
            debug!("[TINT] root={:?} tinted={} meshes", root, found);
            commands.entity(root).remove::<PlaygroundMaterialTint>();
        }
    }

    if !schedule.shot_done && Instant::now() >= schedule.shot_at {
        let path = schedule.shot_path.clone();
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        info!("[SCENEGATE] screenshot requested");
        schedule.shot_done = true;
    }
}

fn auto_quit(schedule: Res<Schedule>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= schedule.quit_at {
        warn!("[SCENEGATE] wall-clock safety quit");
        exit.write(AppExit::Success);
    }
}

fn verdict_system(
    time: Res<Time>,
    mut stats: ResMut<SceneStats>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs_f64();
    if t < 10.0 || stats.verdict_done {
        return;
    }
    stats.verdict_done = true;

    let ok_static = stats.static_count >= 140;
    let ok_text = stats.text_count >= 22;
    let ok_native_text = stats.native_text_count == stats.text_count;
    let ok_glyph_text = stats.glyph_text_count == stats.text_count;
    let ok_world_text = stats.world_text_mesh_count >= stats.text_count
        && stats.world_text_masked_count == stats.world_text_mesh_count;
    let ok_landscape = stats.landscape_seen && stats.landscape_identity;
    let ok_sun = stats.sun_count == 1 && stats.sun_shadowed;
    let ok_sky = stats.sky_count >= 2;
    let ok_pbr = stats.pbr_material_count >= 1;
    let ok_moving = stats.movers_total >= 3 && stats.movers_moving >= 2;
    let ok_spin = stats.spin_yaw_abs_deg > 30.0;

    info!(
        "[SCENEGATE][VERDICT] static={}(>=140) text={}(>=22) native_text={}/{} glyph_text={}/{} world_mesh_text={} masked_world_mesh_text={} landscape={} identity={} sun={}/shadow={} sky={} pbr={} movers={}(>=3) movers_active={} spin_yaw_abs={:.1}deg -> {}",
        stats.static_count,
        stats.text_count,
        stats.native_text_count,
        stats.text_count,
        stats.glyph_text_count,
        stats.text_count,
        stats.world_text_mesh_count,
        stats.world_text_masked_count,
        stats.landscape_seen,
        stats.landscape_identity,
        stats.sun_count,
        stats.sun_shadowed,
        stats.sky_count,
        stats.pbr_material_count,
        stats.movers_total,
        stats.movers_moving,
        stats.spin_yaw_abs_deg,
        if ok_static
            && ok_text
            && ok_native_text
            && ok_glyph_text
            && ok_world_text
            && ok_landscape
            && ok_sun
            && ok_sky
            && ok_pbr
            && ok_moving
            && ok_spin
        {
            "PASS"
        } else {
            "FAIL"
        }
    );
    if !(ok_static
        && ok_text
        && ok_native_text
        && ok_glyph_text
        && ok_world_text
        && ok_landscape
        && ok_sun
        && ok_sky
        && ok_pbr
        && ok_moving
        && ok_spin)
    {
        exit.write(AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));
    } else {
        exit.write(AppExit::Success);
    }
}
