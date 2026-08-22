use std::time::{Duration, Instant};

use bevy::ecs::message::MessageWriter;
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance};
use bevy_animation_graph::core::animation_graph::AnimationGraph;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::AnimationGraphPlugin;

const SPEED_PEAK: f32 = 6.5;
const TRIANGLE_PERIOD: f32 = 12.0;
const LOG_INTERVAL: f32 = 1.0;

fn main() {

    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        AnimationGraphPlugin::default(),
        als_ext::AlsExtPlugin,
    ))
    .add_systems(Startup, setup)
    .add_systems(Update, (drive_inputs, log_status, probe_wrapper_graph, report_graph_events))
    .add_systems(First, apply_timescale);

    if let Ok(secs) = std::env::var("ALS_BOOT_AUTO_QUIT_SECS") {
        let secs: f64 = secs.parse().unwrap_or(25.0);
        app.insert_resource(AutoQuit(Instant::now() + Duration::from_secs_f64(secs)));
        app.add_systems(Update, auto_quit);
    }

    app.run();
}

fn triangle_wave(t: f32) -> f32 {
    let half = TRIANGLE_PERIOD * 0.5;
    let u = t % TRIANGLE_PERIOD;
    if u < half {
        u / half * SPEED_PEAK
    } else {
        (TRIANGLE_PERIOD - u) / half * SPEED_PEAK
    }
}

fn apply_timescale(mut time: ResMut<Time<Virtual>>) {
    let ts: f32 = std::env::var("ALS_TIMESCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    time.set_relative_speed(ts);
}

fn report_graph_events(mut ev: MessageReader<bevy::asset::AssetEvent<bevy_animation_graph::core::animation_graph::AnimationGraph>>) {
    for e in ev.read() {
        if let bevy::asset::AssetEvent::Modified { id } = e {
            info!("[M4GATE][HOTRELOAD] AnimationGraph modified id={id:?}");
        }
    }
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(AnimatedSceneHandle::new(
        assets.load("animated_scenes/motion_graph.animscn.ron"),
    ));

    commands.spawn(GraphProbe(
        assets.load("graphs/graph_node_walk_fwd.animgraph.ron"),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(1.6, 1.1, 2.4).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(2.0, 4.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn drive_inputs(
    time: Res<Time>,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let speed = triangle_wave(time.elapsed_secs_f64() as f32);
    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("speed", DataValue::from(speed));
            player.set_input_data("actual_speed", DataValue::from(speed));
        }
    }
}

#[derive(Default)]
struct LogState {
    last: f32,
}

fn log_status(
    time: Res<Time>,
    instances: Query<&AnimatedSceneInstance>,
    players: Query<&AnimationGraphPlayer>,
    mut state: Local<LogState>,
) {
    let now = time.elapsed_secs_f64() as f32;
    if now - state.last < LOG_INTERVAL {
        return;
    }
    state.last = now;
    let speed = triangle_wave(now);
    for instance in instances.iter() {
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let pose_ts = match player.get_outputs().get("pose") {
            Some(DataValue::Pose(pose)) => format!("{:.3}", pose.timestamp),
            _ => "none".to_string(),
        };
        match player.get_error() {
            Some(err) => error!(
                "MOTION_GRAPH_ERR t={now:.2} speed={speed:.3} elapsed={:.3} error={err:?}",
                player.elapsed()
            ),
            None => info!(
                "MOTION_GRAPH_OK t={now:.2} speed={speed:.3} elapsed={:.3} pose_ts={}",
                player.elapsed(),
                pose_ts
            ),
        }
    }
}

#[derive(Component)]
struct GraphProbe(Handle<AnimationGraph>);

#[derive(Default)]
struct ProbeState {
    done: bool,
}

fn probe_wrapper_graph(
    probes: Query<&GraphProbe>,
    graphs: Res<Assets<AnimationGraph>>,
    mut state: Local<ProbeState>,
) {
    if state.done {
        return;
    }
    for probe in probes.iter() {
        if graphs.contains(&probe.0) {
            info!("MOTION_GRAPH_PROBE wrapper graph graph_node_walk_fwd loaded");
            state.done = true;
        }
    }
}

#[derive(Resource)]
struct AutoQuit(Instant);

fn auto_quit(mut quit: Option<ResMut<AutoQuit>>, mut exit: MessageWriter<AppExit>) {
    let Some(aq) = quit.as_deref_mut() else {
        return;
    };
    if Instant::now() >= aq.0 {
        info!("MOTION_GRAPH auto-quit");
        exit.write(AppExit::Success);
    }
}
