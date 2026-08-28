use std::time::{Duration, Instant};

use als_ext::{
    AlsExtPlugin, SyncMarker, SyncMarkerPhaseInput, SyncPhaseState, advance_marker_sync_phase,
    marker_phase_sample_time, marker_position_at_time,
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

const GRAPH_PATH: &str = "graphs/ab_als/stances/crouching/state_movement_direction.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const DURATION: f32 = 1.133333;
const EPSILON: f32 = 1e-4;

#[derive(Resource)]
struct GateAssets {
    graph: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource)]
struct Probe {
    entity: Option<Entity>,
    step: usize,
    phase: f32,
    state: SyncPhaseState,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            entity: None,
            step: 0,
            phase: 0.0,
            state: default(),
        }
    }
}

#[derive(Resource)]
struct Deadline(Instant);

struct Direction {
    label: &'static str,
    node_name: &'static str,
    index: f32,
    markers: Vec<SyncMarker>,
}

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
    info!("[CROUCHING-SYNC] loading movement direction graph");
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

fn directions() -> [Direction; 6] {
    [
        Direction {
            label: "Forward",
            node_name: "crouching forward marker player",
            index: 0.0,
            markers: markers(&[("Left", 0.091667), ("Right", 0.65)]),
        },
        Direction {
            label: "RightForward",
            node_name: "crouching right forward marker player",
            index: 1.0,
            markers: markers(&[("Left", 0.016667), ("Right", 0.65)]),
        },
        Direction {
            label: "RightBackward",
            node_name: "crouching right backward marker player",
            index: 2.0,
            markers: markers(&[("Right", 0.483333), ("Left", 1.041667)]),
        },
        Direction {
            label: "Backward",
            node_name: "crouching backward marker player",
            index: 3.0,
            markers: markers(&[("Right", 0.5), ("Left", 1.066667)]),
        },
        Direction {
            label: "LeftBackward",
            node_name: "crouching left backward marker player",
            index: 4.0,
            markers: markers(&[("Right", 0.466667), ("Left", 1.041667)]),
        },
        Direction {
            label: "LeftForward",
            node_name: "crouching left forward marker player",
            index: 5.0,
            markers: markers(&[("Left", 0.091667), ("Right", 0.591667)]),
        },
    ]
}

fn initialize(
    mut commands: Commands,
    assets: Res<AssetServer>,
    gate_assets: Res<GateAssets>,
    mut probe: ResMut<Probe>,
) {
    if probe.entity.is_some()
        || !assets.is_loaded_with_dependencies(&gate_assets.graph)
        || !assets.is_loaded_with_dependencies(&gate_assets.skeleton)
    {
        return;
    }
    let direction = directions().into_iter().next().unwrap();
    probe.phase = advance_phase(&mut probe.state, &direction, true, 0.0, 1.0, 0.0);
    let mut player = AnimationGraphPlayer::new(gate_assets.skeleton.clone())
        .with_graph(gate_assets.graph.clone());
    set_frame(&mut player, probe.phase, direction.index);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.entity = Some(
        commands
            .spawn((
                Name::new("crouching_sync_gate"),
                Children::default(),
                player,
            ))
            .id(),
    );
    info!("[CROUCHING-SYNC] graph probe initialized");
}

fn set_frame(player: &mut AnimationGraphPlayer, phase: f32, direction: f32) {
    player.set_input_data("direction_index", DataValue::F32(direction));
    for (key, value) in [
        ("als_crouching_direction_index", direction),
        ("als_crouching_movement_marker_phase", phase),
        ("als_crouching_play_rate", 1.0),
    ] {
        player.set_global_input_data(key, value);
    }
    player.queue_time_update(TimeUpdate::Delta(0.0));
}

fn node_pose(player: &AnimationGraphPlayer, graph: &AnimationGraph, name: &str) -> Option<Pose> {
    let node_id = graph.nodes.values().find(|node| node.name == name)?.id;
    let arena = player.get_context_arena()?;
    match arena.get_toplevel().node_caches.get_output_data(
        node_id,
        StateKey::Default,
        "pose".to_owned(),
    )? {
        DataValue::Pose(pose) => Some(pose),
        _ => None,
    }
}

fn node_sample_time(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    name: &str,
) -> Option<f32> {
    let node_id = graph.nodes.values().find(|node| node.name == name)?.id;
    let arena = player.get_context_arena()?;
    match arena.get_toplevel().node_caches.get_output_data(
        node_id,
        StateKey::Default,
        "sample_time".to_owned(),
    )? {
        DataValue::F32(time) => Some(time),
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

fn marker_matches(phase: f32, direction: &Direction, sample_time: f32) -> bool {
    let expected = marker_position_at_time(&template(), 1.0, phase, true);
    let actual = marker_position_at_time(&direction.markers, DURATION, sample_time, true);
    expected.zip(actual).is_some_and(|(expected, actual)| {
        expected.previous == actual.previous
            && expected.next == actual.next
            && close(expected.alpha, actual.alpha)
    })
}

fn verify(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    phase: f32,
    direction: &Direction,
) -> Result<(f32, usize), String> {
    let pose = node_pose(player, graph, direction.node_name)
        .ok_or_else(|| format!("{} pose missing", direction.label))?;
    let sample_time = node_sample_time(player, graph, direction.node_name)
        .ok_or_else(|| format!("{} sample time missing", direction.label))?;
    let expected_time =
        marker_phase_sample_time(phase, 1.0, &template(), DURATION, &direction.markers, true);
    if !pose_is_valid(&pose)
        || !close(sample_time, expected_time)
        || !marker_matches(phase, direction, sample_time)
    {
        return Err(format!(
            "{} phase={phase:.6} actual={sample_time:.6} expected={expected_time:.6} bones={}",
            direction.label,
            pose.bones.len(),
        ));
    }
    Ok((sample_time, pose.bones.len()))
}

fn advance_phase(
    state: &mut SyncPhaseState,
    direction: &Direction,
    active: bool,
    start_time: f32,
    rate: f32,
    delta_seconds: f32,
) -> f32 {
    advance_marker_sync_phase(
        state,
        SyncMarkerPhaseInput {
            active,
            reset: false,
            rate,
            leader_duration: DURATION,
            leader_markers: &direction.markers,
            phase_duration: 1.0,
            phase_markers: &template(),
            start_time,
            looped: true,
            delta_seconds,
        },
    )
}

fn fail(exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[CROUCHING-SYNC][VERDICT] {message} -> FAIL");
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
    let directions = directions();
    let index = probe.step.min(directions.len() - 1);
    let direction = &directions[index];
    let phase = probe.phase;
    let (sample_time, bones) = match verify(&player, graph, phase, direction) {
        Ok(result) => result,
        Err(message) => {
            fail(&mut exit, &message);
            return;
        }
    };
    if probe.step == 0 && !close(sample_time, 0.0) {
        fail(&mut exit, "source crouching start time");
        return;
    }
    info!(
        "[CROUCHING-SYNC] direction={} phase={phase:.6} sample_time={sample_time:.6} bones={bones}",
        direction.label,
    );
    if probe.step + 1 == directions.len() {
        info!("[CROUCHING-SYNC][VERDICT] directions/rate/markers/poses -> PASS");
        exit.write(AppExit::Success);
        return;
    }
    probe.step += 1;
    let next = &directions[probe.step];
    let (rate, delta_seconds) = if probe.step == 1 {
        (1.5, 0.1)
    } else {
        (1.0, 0.0)
    };
    probe.phase = advance_phase(&mut probe.state, next, true, 0.0, rate, delta_seconds);
    set_frame(&mut player, probe.phase, next.index);
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
