use std::time::{Duration, Instant};

use als_ext::{
    AlsExtPlugin, GlobalF32InputNode, SyncMarker, marker_phase_sample_time, marker_position_at_time,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    context::node_states::StateKey,
    edge_data::DataValue,
    plugin::AnimationGraphSet,
    pose::Pose,
    skeleton::Skeleton,
};

const GRAPH_PATH: &str = "graphs/ab_als/overlays/rifle/state_relaxed.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const PHASE_KEY: &str = "als_movement_marker_phase";
const EPSILON: f32 = 1e-4;

#[derive(Resource)]
struct GateAssets {
    graph: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource, Default)]
struct Probe {
    entity: Option<Entity>,
    phase: u8,
}

#[derive(Resource)]
struct Deadline(Instant);

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "assets".into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                }),
            AnimationGraphPlugin::from_physics_schedule(Update),
            AlsExtPlugin,
        ))
        .init_resource::<Probe>()
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(25)))
        .add_systems(Startup, load_assets)
        .add_systems(Update, (initialize, timeout))
        .add_systems(PostUpdate, observe.after(AnimationGraphSet::Final))
        .run();
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        graph: assets.load(GRAPH_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    info!("[RIFLE-MOVEMENT-SYNC] loading relaxed rifle graph");
}

fn initialize(
    mut commands: Commands,
    server: Res<AssetServer>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    mut probe: ResMut<Probe>,
) {
    if probe.entity.is_some()
        || !server.is_loaded_with_dependencies(&assets.graph)
        || !server.is_loaded_with_dependencies(&assets.skeleton)
    {
        return;
    }
    let Some(graph) = graphs.get(&assets.graph) else {
        return;
    };
    let mut player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.graph.clone());
    player.set_global_input_data("als_root_defaults_enabled", true);
    for node in graph.nodes.values() {
        if let Some(node) = node.try_inner_downcast_ref::<GlobalF32InputNode>() {
            player.set_global_input_data(node.key.clone(), 0.0_f32);
        }
    }
    for (key, value) in [
        (PHASE_KEY, 0.5),
        ("als_overlay_velocity_forward_amount", 1.0),
        ("als_overlay_gait_running_amount", 1.0),
        ("als_overlay_gait_sprinting_amount", 1.0),
        ("als_overlay_standing_amount", 1.0),
        ("als_overlay_sprint_acceleration_amount", 1.0),
    ] {
        player.set_global_input_data(key, value);
    }
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.entity = Some(
        commands
            .spawn((
                Name::new("rifle_movement_sync_gate"),
                Children::default(),
                player,
            ))
            .id(),
    );
    info!("[RIFLE-MOVEMENT-SYNC] graph probe initialized");
}

fn markers(entries: &[(&str, f32)]) -> Vec<SyncMarker> {
    entries
        .iter()
        .map(|(name, time)| SyncMarker {
            name: (*name).to_owned(),
            time: *time,
        })
        .collect()
}

fn template() -> Vec<SyncMarker> {
    markers(&[("Left", 0.0), ("Right", 0.5)])
}

fn node_output(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    name: &str,
    key: &str,
) -> Option<DataValue> {
    let node_id = graph.nodes.values().find(|node| node.name == name)?.id;
    player
        .get_context_arena()?
        .get_toplevel()
        .node_caches
        .get_output_data(node_id, StateKey::Default, key.to_owned())
}

fn sample_time(player: &AnimationGraphPlayer, graph: &AnimationGraph, name: &str) -> Option<f32> {
    match node_output(player, graph, name, "sample_time")? {
        DataValue::F32(value) => Some(value),
        _ => None,
    }
}

fn pose(player: &AnimationGraphPlayer, graph: &AnimationGraph, name: &str) -> Option<Pose> {
    match node_output(player, graph, name, "pose")? {
        DataValue::Pose(value) => Some(value),
        _ => None,
    }
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() <= EPSILON
}

fn pose_is_valid(pose: &Pose) -> bool {
    !pose.bones.is_empty()
        && pose.timestamp.is_finite()
        && pose.bones.iter().all(|bone| {
            bone.translation.is_none_or(Vec3::is_finite)
                && bone.rotation.is_none_or(Quat::is_finite)
                && bone.scale.is_none_or(Vec3::is_finite)
        })
}

fn sample_contract(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    node: &str,
    phase: f32,
    duration: f32,
    follower_markers: &[SyncMarker],
) -> Result<(f32, usize), String> {
    let sample =
        sample_time(player, graph, node).ok_or_else(|| format!("{node} sample missing"))?;
    let pose = pose(player, graph, node).ok_or_else(|| format!("{node} pose missing"))?;
    let expected =
        marker_phase_sample_time(phase, 1.0, &template(), duration, follower_markers, true);
    let expected_marker = marker_position_at_time(&template(), 1.0, phase, true);
    let actual_marker = marker_position_at_time(follower_markers, duration, sample, true);
    if !close(sample, expected)
        || !pose_is_valid(&pose)
        || !expected_marker
            .zip(actual_marker)
            .is_some_and(|(expected, actual)| {
                expected.previous == actual.previous
                    && expected.next == actual.next
                    && close(expected.alpha, actual.alpha)
            })
    {
        return Err(format!(
            "{node} actual={sample:.6} expected={expected:.6} bones={}",
            pose.bones.len(),
        ));
    }
    Ok((sample, pose.bones.len()))
}

fn fail(exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[RIFLE-MOVEMENT-SYNC][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    graphs: Res<Assets<AnimationGraph>>,
    assets: Res<GateAssets>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(entity) = probe.entity else {
        return;
    };
    let Some(graph) = graphs.get(&assets.graph) else {
        return;
    };
    let Ok(mut player) = players.get_mut(entity) else {
        return;
    };
    if let Some(error) = player.get_error() {
        fail(&mut exit, &format!("runtime error={error:?}"));
        return;
    }
    let phase = if probe.phase == 0 { 0.5 } else { 0.25 };
    let run_markers = markers(&[("Left", 0.033401), ("Right", 0.433038)]);
    let sprint_markers = markers(&[("Left", 0.053793), ("Right", 0.353976)]);
    let result = (
        sample_contract(
            &player,
            graph,
            "run arms movement follower",
            phase,
            0.8,
            &run_markers,
        ),
        sample_contract(
            &player,
            graph,
            "sprint arms movement follower",
            phase,
            0.6,
            &sprint_markers,
        ),
        sample_contract(
            &player,
            graph,
            "sprint impulse arms movement follower",
            phase,
            0.6,
            &sprint_markers,
        ),
    );
    let (run, sprint, impulse) = match result {
        (Ok(run), Ok(sprint), Ok(impulse)) => (run, sprint, impulse),
        (run, sprint, impulse) => {
            let message = [run, sprint, impulse]
                .into_iter()
                .find_map(Result::err)
                .unwrap_or_else(|| "movement follower mismatch".to_owned());
            fail(&mut exit, &message);
            return;
        }
    };
    info!(
        "[RIFLE-MOVEMENT-SYNC] phase={phase:.2} run={:.6} sprint={:.6} impulse={:.6} bones={}/{}/{}",
        run.0, sprint.0, impulse.0, run.1, sprint.1, impulse.1,
    );
    if probe.phase == 0 {
        player.set_global_input_data(PHASE_KEY, 0.25_f32);
        player.queue_time_update(TimeUpdate::Delta(0.0));
        probe.phase = 1;
        return;
    }
    info!("[RIFLE-MOVEMENT-SYNC][VERDICT] AlwaysFollower marker phase -> PASS");
    exit.write(AppExit::Success);
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
