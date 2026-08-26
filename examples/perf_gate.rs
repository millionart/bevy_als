use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::ecs::{message::MessageWriter, schedule::IntoScheduleConfigs};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;

use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::params::AlsMovementParams;

use bevy_als::playable_character::{
    AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};

const BUDGET_MS_DEBUG: f64 = 1.5;
const SAMPLE_FRAMES: usize = 600;

#[derive(Resource, Default)]
struct EvalTimer {
    start: Option<Instant>,
    samples: Vec<f64>,
}

#[derive(Resource)]
struct Deadline(Instant);

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        file_path: "assets".into(),
        ..default()
    }))
    .add_plugins(AlsPlayableCharacterPlugin);

    app.init_resource::<EvalTimer>()
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(90)))
        .add_systems(
            FixedPostUpdate,
            (
                mark_start
                    .before(bevy_animation_graph::core::plugin::AnimationGraphSet::PrePhysics),
                collect_end
                    .after(bevy_animation_graph::core::plugin::AnimationGraphSet::PrePhysics),
            ),
        )
        .add_systems(Startup, spawn_character)
        .add_systems(Update, finish_or_report);

    app.run();
}

fn mark_start(mut timer: ResMut<EvalTimer>, q: Query<&AnimatedSceneHandle>) {
    if q.is_empty() {
        timer.start = None;
        return;
    }
    timer.start = Some(Instant::now());
}

fn collect_end(
    mut timer: ResMut<EvalTimer>,
    q_players: Query<&AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    if timer.samples.len() >= SAMPLE_FRAMES {
        return;
    }
    let has_scene = !q_players.is_empty();
    match timer.start.take() {
        Some(start) => {
            let us = start.elapsed().as_secs_f64() * 1000.0;
            if us < 50.0 {
                timer.samples.push(us);
            }
        }
        None => {
            if has_scene && timer.samples.len() >= SAMPLE_FRAMES {
                return;
            }
            let _ = exit;
        }
    }
}

#[allow(clippy::type_complexity)]
fn finish_or_report(
    time: Res<Time>,
    mut timer: ResMut<EvalTimer>,
    deadline: Res<Deadline>,
    mut exit: MessageWriter<AppExit>,
) {
    if timer.samples.len() < SAMPLE_FRAMES {
        if Instant::now() >= deadline.0 || time.elapsed_secs_f64() > 45.0 {
            error!(
                "[PERF] only {} of {} samples collected before deadline",
                timer.samples.len(),
                SAMPLE_FRAMES
            );
            exit.write(AppExit::Error(std::num::NonZeroU8::new(5).unwrap()));
        }
        return;
    }
    let mut sorted = timer.samples.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = sorted.len();
    let avg = sorted.iter().sum::<f64>() / n as f64;
    let p95 = sorted[(n as f32 * 0.95) as usize % n];
    let max = sorted[n - 1];
    info!(
        "[PERF][RESULT] n={n} avg_eval_ms={avg:.3} p95={p95:.3} max={max:.3} budget(avg,debug x3)={BUDGET_MS_DEBUG}"
    );
    let ok = avg < BUDGET_MS_DEBUG;
    info!("[PERF][VERDICT] {}", if ok { "PASS" } else { "FAIL" });
    exit.write(if ok {
        AppExit::Success
    } else {
        AppExit::Error(std::num::NonZeroU8::new(6).unwrap())
    });
}

fn spawn_character(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    assets: Res<AssetServer>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 0.3, 40.0),
        Transform::from_xyz(0.0, -0.15, 0.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(40.0, 0.3, 40.0)))),
        MeshMaterial3d(materials.add(Color::srgb(0.24, 0.27, 0.24))),
    ));

    let _character = spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 0.95, 0.0)),
    );
    info!("[PERF] character spawned");
}
