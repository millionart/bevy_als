use std::time::{Duration, Instant};

use als_ext::{
    AlsExtPlugin, CubicVec2FilterNode, NormalizedBlendSpaceNode, NormalizedBlendSpacePoint,
    SyncGroupSampleNode, SyncMarker,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    animation_node::{AnimationNode, NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
    plugin::AnimationGraphSet,
    pose::Pose,
};

const EPSILON: f32 = 1e-4;

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "sync_gate"]
struct FixedDurationPoseNode {
    duration_seconds: f32,
}

impl NodeLike for FixedDurationPoseNode {
    fn display_name(&self) -> String {
        "Fixed Duration Pose".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(Some(self.duration_seconds));
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        let time = update
            .partial_update_basic(ctx.prev_time())
            .unwrap_or_else(|| ctx.prev_time());
        ctx.set_time(time);
        ctx.set_data_fwd(
            "pose",
            Pose {
                timestamp: time,
                ..default()
            },
        );
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data("pose", DataSpec::Pose)
            .add_output_time();
        Ok(())
    }
}

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Initial,
    Filtered,
}

#[derive(Resource)]
struct Probe {
    normalized: Option<Entity>,
    follower: Option<Entity>,
    filter: Option<Entity>,
    phase: Phase,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            normalized: None,
            follower: None,
            filter: None,
            phase: Phase::Initial,
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
    .register_type::<FixedDurationPoseNode>()
    .init_resource::<Probe>()
    .insert_resource(Deadline(Instant::now() + Duration::from_secs(20)))
    .add_systems(Startup, setup)
    .add_systems(Update, timeout)
    .add_systems(PostUpdate, observe.after(AnimationGraphSet::Final));
    app.run();
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

fn fixed_node(name: &str, duration_seconds: f32) -> AnimationNode {
    AnimationNode::new(name, FixedDurationPoseNode { duration_seconds })
}

fn build_normalized_graph() -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let walk = fixed_node("walk", 1.133333);
    let walk_id = walk.id;
    let run = fixed_node("run", 0.8);
    let run_id = run.id;
    let node = AnimationNode::new(
        "normalized blendspace",
        NormalizedBlendSpaceNode {
            looped: true,
            points: vec![
                NormalizedBlendSpacePoint {
                    id: "walk".to_owned(),
                    point: Vec2::new(1.0, 0.0),
                    duration: 1.133333,
                    markers: markers(&[("Left", 0.083333), ("Right", 0.65)]),
                    curves: default(),
                },
                NormalizedBlendSpacePoint {
                    id: "run".to_owned(),
                    point: Vec2::new(1.0, 1.0),
                    duration: 0.8,
                    markers: markers(&[("Left", 0.033333), ("Right", 0.433333)]),
                    curves: default(),
                },
            ],
            ..default()
        },
    );
    let id = node.id;
    graph.add_node(walk);
    graph.add_node(run);
    graph.add_node(node);
    graph.add_input_data(GraphInputPin::passthrough("phase"), DataSpec::F32);
    graph.add_input_data(GraphInputPin::passthrough("position"), DataSpec::Vec2);
    graph.add_input_data_edge(GraphInputPin::passthrough("phase"), id, "phase");
    graph.add_input_data_edge(GraphInputPin::passthrough("position"), id, "position");
    graph.add_node_parameter_edge(walk_id, "pose", id, "pose_walk");
    graph.add_node_pose_edge(walk_id, id, "time_walk");
    graph.add_node_parameter_edge(run_id, "pose", id, "pose_run");
    graph.add_node_pose_edge(run_id, id, "time_run");
    graph.add_output_data("pose".to_owned(), DataSpec::Pose);
    graph.add_output_data_edge(id, "pose", "pose");
    graph.add_output_data("sample_time".to_owned(), DataSpec::F32);
    graph.add_output_data_edge(id, "sample_time", "sample_time");
    graph.add_output_pose_edge(id);
    graph
}

fn build_follower_graph() -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let follower = fixed_node("follower", 0.8);
    let follower_id = follower.id;
    let node = AnimationNode::new(
        "marker follower",
        SyncGroupSampleNode {
            leader_duration: 1.133333,
            leader_markers: markers(&[("Left", 0.083333), ("Right", 0.65)]),
            follower_duration: 0.8,
            follower_markers: markers(&[("Left", 0.033333), ("Right", 0.433333)]),
            looped: true,
        },
    );
    let id = node.id;
    graph.add_node(follower);
    graph.add_node(node);
    graph.add_input_data(GraphInputPin::passthrough("phase"), DataSpec::F32);
    graph.add_input_data_edge(GraphInputPin::passthrough("phase"), id, "phase");
    graph.add_node_parameter_edge(follower_id, "pose", id, "pose");
    graph.add_node_pose_edge(follower_id, id, "time");
    graph.add_output_data("pose".to_owned(), DataSpec::Pose);
    graph.add_output_data_edge(id, "pose", "pose");
    graph.add_output_data("sample_time".to_owned(), DataSpec::F32);
    graph.add_output_data_edge(id, "sample_time", "sample_time");
    graph.add_output_pose_edge(id);
    graph
}

fn build_filter_graph() -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new(
        "cubic filter",
        CubicVec2FilterNode {
            window_seconds: 0.4,
        },
    );
    let id = node.id;
    graph.add_node(node);
    graph.add_input_data(GraphInputPin::passthrough("position"), DataSpec::Vec2);
    graph.add_input_data(GraphInputPin::passthrough("delta_seconds"), DataSpec::F32);
    graph.add_input_data_edge(GraphInputPin::passthrough("position"), id, "position");
    graph.add_input_data_edge(
        GraphInputPin::passthrough("delta_seconds"),
        id,
        "delta_seconds",
    );
    graph.add_output_data("value".to_owned(), DataSpec::Vec2);
    graph.add_output_data_edge(id, "value", "value");
    graph
}

fn setup(mut commands: Commands, mut graphs: ResMut<Assets<AnimationGraph>>) {
    let normalized_graph = graphs.add(build_normalized_graph());
    let follower_graph = graphs.add(build_follower_graph());
    let filter_graph = graphs.add(build_filter_graph());
    let mut normalized = AnimationGraphPlayer::new(default()).with_graph(normalized_graph);
    normalized.set_input_data("phase", DataValue::F32(0.2));
    normalized.set_input_data("position", DataValue::Vec2(Vec2::new(1.0, 0.25)));
    normalized.pause();
    normalized.queue_time_update(TimeUpdate::Delta(0.0));
    let mut follower = AnimationGraphPlayer::new(default()).with_graph(follower_graph);
    follower.set_input_data("phase", DataValue::F32(0.2));
    follower.pause();
    follower.queue_time_update(TimeUpdate::Delta(0.0));
    let mut filter = AnimationGraphPlayer::new(default()).with_graph(filter_graph);
    filter.set_input_data("position", DataValue::Vec2(Vec2::ZERO));
    filter.set_input_data("delta_seconds", DataValue::F32(0.0));
    filter.pause();
    filter.queue_time_update(TimeUpdate::Delta(0.0));
    let normalized = commands
        .spawn((
            Name::new("normalized_blendspace_gate"),
            Children::default(),
            normalized,
        ))
        .id();
    let follower = commands
        .spawn((
            Name::new("marker_follower_gate"),
            Children::default(),
            follower,
        ))
        .id();
    let filter = commands
        .spawn((Name::new("cubic_filter_gate"), Children::default(), filter))
        .id();
    commands.insert_resource(Probe {
        normalized: Some(normalized),
        follower: Some(follower),
        filter: Some(filter),
        phase: Phase::Initial,
    });
    info!("[NORMALIZED-BLENDSPACE] graph probes initialized");
}

fn output_sample_time(player: &AnimationGraphPlayer) -> Option<f32> {
    match player.get_outputs().get("sample_time") {
        Some(DataValue::F32(time)) => Some(*time),
        _ => None,
    }
}

fn output_vec2(player: &AnimationGraphPlayer) -> Option<Vec2> {
    match player.get_outputs().get("value") {
        Some(DataValue::Vec2(value)) => Some(*value),
        _ => None,
    }
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() <= EPSILON
}

fn fail(exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[NORMALIZED-BLENDSPACE][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let (Some(normalized_entity), Some(follower_entity), Some(filter_entity)) =
        (probe.normalized, probe.follower, probe.filter)
    else {
        return;
    };
    let Ok([normalized, follower, mut filter]) =
        players.get_many_mut([normalized_entity, follower_entity, filter_entity])
    else {
        return;
    };
    if let Some(error) = normalized.get_error() {
        fail(&mut exit, &format!("normalized runtime error={error:?}"));
        return;
    }
    if let Some(error) = follower.get_error() {
        fail(&mut exit, &format!("follower runtime error={error:?}"));
        return;
    }
    if let Some(error) = filter.get_error() {
        fail(&mut exit, &format!("filter runtime error={error:?}"));
        return;
    }
    let (Some(leader_time), Some(follower_time), Some(filtered)) = (
        output_sample_time(&normalized),
        output_sample_time(&follower),
        output_vec2(&filter),
    ) else {
        return;
    };
    match probe.phase {
        Phase::Initial => {
            if !close(leader_time, 0.2266666)
                || !close(follower_time, 0.13451)
                || filtered != Vec2::ZERO
            {
                fail(&mut exit, "initial normalized sample times");
                return;
            }
            filter.set_input_data("position", DataValue::Vec2(Vec2::ONE));
            filter.set_input_data("delta_seconds", DataValue::F32(0.2));
            filter.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::Filtered;
        }
        Phase::Filtered => {
            if !close(leader_time, 0.2266666)
                || !close(follower_time, 0.13451)
                || !close(filtered.x, 1.0 / 1.875)
                || !close(filtered.y, 1.0 / 1.875)
            {
                fail(&mut exit, "cubic filter response");
                return;
            }
            info!(
                "[NORMALIZED-BLENDSPACE] phase=.2 leader_time={leader_time:.6} follower_time={follower_time:.6} cubic={:.6}",
                filtered.x
            );
            info!("[NORMALIZED-BLENDSPACE][VERDICT] phase/marker/filter -> PASS");
            exit.write(AppExit::Success);
        }
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
