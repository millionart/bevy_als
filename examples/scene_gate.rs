use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;

use als_ext::playground_materials::PlaygroundMaterialsPlugin;
use als_ext::{
    PlaygroundKinematic, PlaygroundKinematicKind, PlaygroundLandscape, PlaygroundMaterialTint,
    PlaygroundPlugin, PlaygroundStatic, PlaygroundTextLabel,
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
    landscape_seen: bool,
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
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-35.0, 45.0, -60.0).looking_at(Vec3::new(-10.0, 0.0, -20.0), Vec3::Y),
    ));
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
    landscapes: Query<(), With<PlaygroundLandscape>>,
) {
    stats.static_count = statics.iter().count() as u32;
    stats.text_count = texts.iter().count() as u32;
    stats.landscape_seen = landscapes.single().is_ok();
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
            info!("[TINT] root={:?} tinted={} meshes", root, found);
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
    let ok_landscape = stats.landscape_seen;
    let ok_moving = stats.movers_total >= 3 && stats.movers_moving >= 2;
    let ok_spin = stats.spin_yaw_abs_deg > 30.0;

    info!(
        "[SCENEGATE][VERDICT] static={}(>=140) text={}(>=22) landscape={} movers={}(>=3) movers_active={} spin_yaw_abs={:.1}deg -> {}",
        stats.static_count,
        stats.text_count,
        stats.landscape_seen,
        stats.movers_total,
        stats.movers_moving,
        stats.spin_yaw_abs_deg,
        if ok_static && ok_text && ok_landscape && ok_moving && ok_spin {
            "PASS"
        } else {
            "FAIL"
        }
    );
    if !(ok_static && ok_text && ok_landscape && ok_moving && ok_spin) {
        exit.write(AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));
    } else {
        exit.write(AppExit::Success);
    }
}
