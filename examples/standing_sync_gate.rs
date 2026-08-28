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

const GRAPH_PATH: &str = "graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const EPSILON: f32 = 1e-4;

#[derive(Resource)]
struct GateAssets {
    graph: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    ForwardWalk,
    ForwardRun,
    Direction,
    SprintEnter,
    SprintInitial,
    SprintExit,
}

#[derive(Resource)]
struct Probe {
    entity: Option<Entity>,
    stage: Stage,
    direction_step: usize,
    phase: f32,
    sync_state: SyncPhaseState,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            entity: None,
            stage: Stage::ForwardWalk,
            direction_step: 0,
            phase: 0.0,
            sync_state: default(),
        }
    }
}

#[derive(Resource)]
struct Deadline(Instant);

struct Leader {
    label: &'static str,
    node_name: &'static str,
    direction_index: f32,
    duration: f32,
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
    info!("[STANDING-SYNC] loading Standing phase graph");
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

fn phase_markers() -> Vec<SyncMarker> {
    markers(&[("Left", 0.0), ("Right", 0.5)])
}

fn forward_walk() -> Leader {
    Leader {
        label: "ForwardWalk",
        node_name: "Forward walk run blendspace",
        direction_index: 0.0,
        duration: 1.133333,
        markers: markers(&[("Left", 0.083333), ("Right", 0.65)]),
    }
}

fn forward_run() -> Leader {
    Leader {
        label: "ForwardRun",
        node_name: "Forward walk run blendspace",
        direction_index: 0.0,
        duration: 0.8,
        markers: markers(&[("Left", 0.033333), ("Right", 0.433333)]),
    }
}

fn run_directions() -> [Leader; 5] {
    [
        Leader {
            label: "RightForward",
            node_name: "Right Forward walk run blendspace",
            direction_index: 1.0,
            duration: 0.8,
            markers: markers(&[("Left", 0.05), ("Right", 0.433333)]),
        },
        Leader {
            label: "RightBackward",
            node_name: "Right Backward walk run blendspace",
            direction_index: 2.0,
            duration: 0.666667,
            markers: markers(&[("Right", 0.316667), ("Left", 0.65)]),
        },
        Leader {
            label: "Backward",
            node_name: "Backward walk run blendspace",
            direction_index: 3.0,
            duration: 0.666667,
            markers: markers(&[("Left", 0.05), ("Right", 0.383333)]),
        },
        Leader {
            label: "LeftBackward",
            node_name: "Left Backward walk run blendspace",
            direction_index: 4.0,
            duration: 0.666667,
            markers: markers(&[("Right", 0.316667), ("Left", 0.65)]),
        },
        Leader {
            label: "LeftForward",
            node_name: "Left Forward walk run blendspace",
            direction_index: 5.0,
            duration: 0.8,
            markers: markers(&[("Left", 0.033333), ("Right", 0.45)]),
        },
    ]
}

fn sprint() -> Leader {
    Leader {
        label: "Sprint",
        node_name: "sprint marker phase player",
        direction_index: 0.0,
        duration: 0.6,
        markers: markers(&[("Left", 0.016667), ("Right", 0.316667)]),
    }
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
    let mut player = AnimationGraphPlayer::new(gate_assets.skeleton.clone())
        .with_graph(gate_assets.graph.clone());
    set_globals(&mut player);
    let leader = forward_walk();
    let phase_template = phase_markers();
    probe.phase = advance_marker_sync_phase(
        &mut probe.sync_state,
        SyncMarkerPhaseInput {
            active: true,
            reset: false,
            rate: 1.0,
            leader_duration: leader.duration,
            leader_markers: &leader.markers,
            phase_duration: 1.0,
            phase_markers: &phase_template,
            start_time: 0.2 * leader.duration,
            looped: true,
            delta_seconds: 0.0,
        },
    );
    set_frame(
        &mut player,
        probe.phase,
        leader.direction_index,
        0.0,
        0.0,
        0.0,
        0.0,
    );
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.entity = Some(
        commands
            .spawn((Name::new("standing_sync_gate"), Children::default(), player))
            .id(),
    );
    info!("[STANDING-SYNC] graph probe initialized");
}

fn set_globals(player: &mut AnimationGraphPlayer) {
    for (key, value) in [
        ("als_standing_stride_amount", 1.0),
        ("als_standing_walk_run_amount", 0.0),
        ("als_standing_play_rate", 1.0),
        ("als_standing_sprint_amount", 0.0),
        ("als_standing_sprint_acceleration_amount", 0.0),
        ("als_standing_sprint_play_rate", 1.0),
        ("als_standing_movement_marker_phase", 0.0),
        ("als_animation_delta_seconds", 0.0),
    ] {
        player.set_global_input_data(key, value);
    }
}

fn set_frame(
    player: &mut AnimationGraphPlayer,
    phase: f32,
    direction_index: f32,
    walk_run_amount: f32,
    sprint_amount: f32,
    sprint_acceleration_amount: f32,
    delta_seconds: f32,
) {
    player.set_input_data("direction_index", DataValue::F32(direction_index));
    for (key, value) in [
        ("als_standing_walk_run_amount", walk_run_amount),
        ("als_standing_sprint_amount", sprint_amount),
        (
            "als_standing_sprint_acceleration_amount",
            sprint_acceleration_amount,
        ),
        ("als_standing_movement_marker_phase", phase),
        ("als_animation_delta_seconds", delta_seconds),
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

fn node_vec2(player: &AnimationGraphPlayer, graph: &AnimationGraph, name: &str) -> Option<Vec2> {
    let node_id = graph.nodes.values().find(|node| node.name == name)?.id;
    let arena = player.get_context_arena()?;
    match arena.get_toplevel().node_caches.get_output_data(
        node_id,
        StateKey::Default,
        "value".to_owned(),
    )? {
        DataValue::Vec2(value) => Some(value),
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

fn marker_phase_matches(phase: f32, leader: &Leader, sample_time: f32) -> bool {
    let template = phase_markers();
    let phase_time = phase;
    let expected = marker_position_at_time(&template, 1.0, phase_time, true);
    let actual = marker_position_at_time(&leader.markers, leader.duration, sample_time, true);
    expected.zip(actual).is_some_and(|(expected, actual)| {
        expected.previous == actual.previous
            && expected.next == actual.next
            && close(expected.alpha, actual.alpha)
    })
}

fn verify_leader(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    phase: f32,
    leader: &Leader,
) -> Result<(f32, usize), String> {
    let pose = node_pose(player, graph, leader.node_name)
        .ok_or_else(|| format!("{} pose missing", leader.label))?;
    let sample_time = node_sample_time(player, graph, leader.node_name)
        .ok_or_else(|| format!("{} sample time missing", leader.label))?;
    let expected_time = marker_phase_sample_time(
        phase,
        1.0,
        &phase_markers(),
        leader.duration,
        &leader.markers,
        true,
    );
    if !pose_is_valid(&pose)
        || !close(sample_time, expected_time)
        || !marker_phase_matches(phase, leader, sample_time)
    {
        return Err(format!(
            "{} phase={phase:.6} actual={:.6} expected={expected_time:.6} bones={}",
            leader.label,
            sample_time,
            pose.bones.len(),
        ));
    }
    Ok((sample_time, pose.bones.len()))
}

fn advance_phase(
    probe: &mut Probe,
    leader: &Leader,
    active: bool,
    start_time: f32,
    delta_seconds: f32,
) -> f32 {
    probe.phase = advance_marker_sync_phase(
        &mut probe.sync_state,
        SyncMarkerPhaseInput {
            active,
            reset: false,
            rate: 1.0,
            leader_duration: leader.duration,
            leader_markers: &leader.markers,
            phase_duration: 1.0,
            phase_markers: &phase_markers(),
            start_time,
            looped: true,
            delta_seconds,
        },
    );
    probe.phase
}

fn fail(exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[STANDING-SYNC][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    graphs: Res<Assets<AnimationGraph>>,
    gate_assets: Res<GateAssets>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(entity) = probe.entity else {
        return;
    };
    let Some(graph) = graphs.get(&gate_assets.graph) else {
        return;
    };
    let Ok(mut player) = players.get_mut(entity) else {
        return;
    };
    if let Some(error) = player.get_error() {
        fail(&mut exit, &format!("runtime error={error:?}"));
        return;
    }
    match probe.stage {
        Stage::ForwardWalk => {
            let leader = forward_walk();
            let walk_phase = probe.phase;
            let (time, bones) = match verify_leader(&player, graph, walk_phase, &leader) {
                Ok(result) => result,
                Err(message) => {
                    let phase_input = player
                        .get_global_input_data("als_standing_movement_marker_phase")
                        .and_then(|value| value.as_f32().ok());
                    fail(
                        &mut exit,
                        &format!("{message} global_phase={phase_input:?}"),
                    );
                    return;
                }
            };
            if !close(time, 0.2266666) {
                fail(&mut exit, "forward walk source start");
                return;
            }
            let leader = forward_run();
            let phase = advance_phase(&mut probe, &leader, true, 0.2 * leader.duration, 0.4);
            set_frame(
                &mut player,
                phase,
                leader.direction_index,
                1.0,
                0.0,
                0.0,
                0.4,
            );
            probe.stage = Stage::ForwardRun;
            info!("[STANDING-SYNC] walk phase={walk_phase:.6} time={time:.6} bones={bones}");
        }
        Stage::ForwardRun => {
            let leader = forward_run();
            let run_phase = probe.phase;
            let Ok((time, bones)) = verify_leader(&player, graph, run_phase, &leader) else {
                fail(&mut exit, "forward run marker phase");
                return;
            };
            if node_vec2(&player, graph, "walk run cubic filter")
                .is_none_or(|value| !close(value.x, 1.0) || !close(value.y, 1.0))
            {
                fail(&mut exit, "standing cubic filter");
                return;
            }
            let directions = run_directions();
            let leader = &directions[probe.direction_step];
            let phase = advance_phase(&mut probe, leader, true, 0.2 * leader.duration, 0.0);
            set_frame(
                &mut player,
                phase,
                leader.direction_index,
                1.0,
                0.0,
                0.0,
                0.0,
            );
            probe.stage = Stage::Direction;
            info!("[STANDING-SYNC] run phase={run_phase:.6} time={time:.6} bones={bones}");
        }
        Stage::Direction => {
            let directions = run_directions();
            let leader = &directions[probe.direction_step];
            let Ok((time, bones)) = verify_leader(&player, graph, probe.phase, leader) else {
                fail(&mut exit, "direction marker phase");
                return;
            };
            info!(
                "[STANDING-SYNC] direction={} phase={:.6} time={time:.6} bones={bones}",
                leader.label, probe.phase,
            );
            probe.direction_step += 1;
            if let Some(next) = directions.get(probe.direction_step) {
                let phase = advance_phase(&mut probe, next, true, 0.2 * next.duration, 0.0);
                set_frame(&mut player, phase, next.direction_index, 1.0, 0.0, 0.0, 0.0);
                return;
            }
            let leader = sprint();
            let phase = advance_phase(&mut probe, &leader, true, 0.166667, 0.0);
            set_frame(
                &mut player,
                phase,
                leader.direction_index,
                1.0,
                1.0,
                0.0,
                0.0,
            );
            probe.stage = Stage::SprintEnter;
        }
        Stage::SprintEnter => {
            let leader = sprint();
            let sprint_entry_phase = probe.phase;
            let Ok((time, bones)) = verify_leader(&player, graph, sprint_entry_phase, &leader)
            else {
                fail(&mut exit, "sprint entry marker phase");
                return;
            };
            let _ = advance_phase(&mut probe, &leader, false, 0.166667, 0.0);
            let phase = advance_phase(&mut probe, &leader, true, 0.166667, 0.0);
            set_frame(
                &mut player,
                phase,
                leader.direction_index,
                1.0,
                1.0,
                0.0,
                0.0,
            );
            probe.stage = Stage::SprintInitial;
            info!(
                "[STANDING-SYNC] sprint-enter phase={sprint_entry_phase:.6} time={time:.6} bones={bones}"
            );
        }
        Stage::SprintInitial => {
            let leader = sprint();
            let Ok((time, bones)) = verify_leader(&player, graph, probe.phase, &leader) else {
                fail(&mut exit, "sprint source start marker phase");
                return;
            };
            if !close(time, 0.166667) {
                fail(&mut exit, "sprint source start time");
                return;
            }
            let leader = forward_run();
            let phase = advance_phase(&mut probe, &leader, true, 0.2 * leader.duration, 0.0);
            set_frame(
                &mut player,
                phase,
                leader.direction_index,
                1.0,
                0.0,
                0.0,
                0.0,
            );
            probe.stage = Stage::SprintExit;
            info!(
                "[STANDING-SYNC] sprint-start phase={:.6} time={time:.6} bones={bones}",
                probe.phase
            );
        }
        Stage::SprintExit => {
            let leader = forward_run();
            let Ok((time, bones)) = verify_leader(&player, graph, probe.phase, &leader) else {
                fail(&mut exit, "sprint exit marker phase");
                return;
            };
            info!(
                "[STANDING-SYNC] sprint-exit phase={:.6} time={time:.6} bones={bones}",
                probe.phase
            );
            info!("[STANDING-SYNC][VERDICT] walk/run/directions/sprint/markers/poses -> PASS");
            exit.write(AppExit::Success);
        }
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
