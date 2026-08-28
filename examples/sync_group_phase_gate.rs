use std::time::{Duration, Instant};

use als_ext::{AlsExtPlugin, SyncGroupPhaseNode, SyncMarker, follower_sample_time};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    animation_node::AnimationNode,
    edge_data::{DataSpec, DataValue},
    plugin::AnimationGraphSet,
};

const EPSILON: f32 = 1e-4;

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Join,
    Advance,
    Reset,
    PostReset,
    Wrap,
    Leave,
    Rejoin,
}

#[derive(Resource)]
struct Probe {
    entity: Option<Entity>,
    phase: Phase,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            entity: None,
            phase: Phase::Join,
        }
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: None,
            exit_condition: ExitCondition::DontExit,
            ..default()
        }),
        AnimationGraphPlugin::from_physics_schedule(Update),
        AlsExtPlugin,
    ))
    .init_resource::<Probe>()
    .insert_resource(Deadline(Instant::now() + Duration::from_secs(20)))
    .add_systems(Startup, setup)
    .add_systems(Update, timeout)
    .add_systems(PostUpdate, observe.after(AnimationGraphSet::Final));
    app.run();
}

fn build_graph() -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new(
        "sync phase leader",
        SyncGroupPhaseNode {
            looped: true,
            start_phase: 0.2,
            activation_threshold: 1e-6,
        },
    );
    let id = node.id;
    graph.add_node(node);
    for input in ["active", "rate", "duration", "delta_seconds", "reset"] {
        graph.add_input_data(GraphInputPin::passthrough(input), DataSpec::F32);
        graph.add_input_data_edge(GraphInputPin::passthrough(input), id, input);
    }
    graph.add_output_data("phase".to_owned(), DataSpec::F32);
    graph.add_output_data_edge(id, "phase", "phase");
    graph
}

fn setup(mut commands: Commands, mut graphs: ResMut<Assets<AnimationGraph>>) {
    let graph = graphs.add(build_graph());
    let mut player = AnimationGraphPlayer::new(default()).with_graph(graph);
    set_inputs(&mut player, 1.0, 0.0);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    let entity = commands
        .spawn((
            Name::new("sync_group_phase_gate"),
            Children::default(),
            player,
        ))
        .id();
    commands.insert_resource(Probe {
        entity: Some(entity),
        phase: Phase::Join,
    });
    info!("[SYNC-PHASE] phase graph initialized");
}

fn set_inputs(player: &mut AnimationGraphPlayer, active: f32, delta_seconds: f32) {
    player.set_input_data("active", DataValue::F32(active));
    player.set_input_data("rate", DataValue::F32(1.0));
    player.set_input_data("duration", DataValue::F32(1.0));
    player.set_input_data("delta_seconds", DataValue::F32(delta_seconds));
    player.set_input_data("reset", DataValue::F32(0.0));
}

fn output_phase(player: &AnimationGraphPlayer) -> Option<f32> {
    match player.get_outputs().get("phase") {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() <= EPSILON
}

fn fail(exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[SYNC-PHASE][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(entity) = probe.entity else {
        return;
    };
    let Ok(mut player) = players.get_mut(entity) else {
        return;
    };
    if let Some(error) = player.get_error() {
        fail(&mut exit, &format!("runtime error={error:?}"));
        return;
    }
    let Some(phase) = output_phase(&player) else {
        return;
    };
    match probe.phase {
        Phase::Join => {
            if !close(phase, 0.2) {
                fail(&mut exit, "join phase");
                return;
            }
            set_inputs(&mut player, 1.0, 0.5);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::Advance;
        }
        Phase::Advance => {
            if !close(phase, 0.7) {
                fail(&mut exit, "advance phase");
                return;
            }
            set_inputs(&mut player, 1.0, 0.25);
            player.set_input_data("reset", DataValue::F32(1.0));
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::Reset;
        }
        Phase::Reset => {
            if !close(phase, 0.2) {
                fail(&mut exit, "asset reset phase");
                return;
            }
            set_inputs(&mut player, 1.0, 0.5);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::PostReset;
        }
        Phase::PostReset => {
            if !close(phase, 0.7) {
                fail(&mut exit, "post reset phase");
                return;
            }
            set_inputs(&mut player, 1.0, 0.5);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::Wrap;
        }
        Phase::Wrap => {
            if !close(phase, 0.2) {
                fail(&mut exit, "wrap phase");
                return;
            }
            set_inputs(&mut player, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::Leave;
        }
        Phase::Leave => {
            if !close(phase, 0.2) {
                fail(&mut exit, "leave phase");
                return;
            }
            set_inputs(&mut player, 1.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::Rejoin;
        }
        Phase::Rejoin => {
            let leader_markers = vec![
                SyncMarker {
                    name: "Left".to_owned(),
                    time: 0.083333,
                },
                SyncMarker {
                    name: "Right".to_owned(),
                    time: 0.65,
                },
            ];
            let follower_markers = vec![
                SyncMarker {
                    name: "Left".to_owned(),
                    time: 0.033333,
                },
                SyncMarker {
                    name: "Right".to_owned(),
                    time: 0.433333,
                },
            ];
            let follower_time = follower_sample_time(
                phase,
                1.133333,
                &leader_markers,
                0.8,
                &follower_markers,
                true,
            );
            if !close(phase, 0.2) || !close(follower_time, 0.13451) {
                fail(&mut exit, "rejoin or marker follower phase");
                return;
            }
            info!(
                "[SYNC-PHASE] join=.2 advance=.7 asset_reset=.2 wrap=.2 rejoin=.2 marker_follower={follower_time:.6}"
            );
            info!("[SYNC-PHASE][VERDICT] leader/follower/marker -> PASS");
            exit.write(AppExit::Success);
        }
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
