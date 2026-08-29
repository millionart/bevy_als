use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use als_ext::{AlsExtPlugin, SkeletonAwareLayeredGraphNode};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::{
    animation_graph::AnimationGraph,
    animation_graph_player::AnimationGraphPlayer,
    animation_node::{AnimationNode, NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
    plugin::AnimationGraphSet,
};

const SLOT_WEIGHT: &str = "slot_curve_probe_weight";
const SLOT_PREFIX: &str = "slot_curve_probe_";
const FOOT_LOCK: &str = "FootLeftLock";
const VIEW_BLOCK: &str = "ViewBlock";

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource)]
struct Probe {
    player: Entity,
    phase: Phase,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Quarter,
    ThreeQuarters,
}

#[derive(Reflect, Clone, Debug)]
#[reflect(NodeLike)]
#[type_path = "slot_curve_gate"]
struct BaseCurveNode;

impl NodeLike for BaseCurveNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_data_fwd(FOOT_LOCK, 1.0_f32);
        ctx.set_data_fwd(VIEW_BLOCK, 0.0_f32);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data(FOOT_LOCK, DataSpec::F32)
            .add_output_data(VIEW_BLOCK, DataSpec::F32);
        Ok(())
    }

    fn display_name(&self) -> String {
        "Slot Curve Base".into()
    }
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            }),
            AnimationGraphPlugin::from_physics_schedule(Update),
            AlsExtPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, observe.after(AnimationGraphSet::Final))
        .add_systems(Update, timeout)
        .run();
}

fn setup(mut commands: Commands, mut graphs: ResMut<Assets<AnimationGraph>>) {
    let inner = graphs.add(build_inner_graph());
    let root = graphs.add(build_root_graph(inner));
    let mut player = AnimationGraphPlayer::new(default()).with_graph(root);
    player.set_global_input_data(SLOT_WEIGHT, 0.25_f32);
    player.set_global_input_data(format!("{SLOT_PREFIX}{FOOT_LOCK}"), 0.0_f32);
    player.set_global_input_data(format!("{SLOT_PREFIX}{VIEW_BLOCK}"), 1.0_f32);
    let entity = commands
        .spawn((Name::new("slot_curve_probe"), Children::default(), player))
        .id();
    commands.insert_resource(Probe {
        player: entity,
        phase: Phase::Quarter,
    });
    commands.insert_resource(Deadline(Instant::now() + Duration::from_secs(10)));
}

fn build_inner_graph() -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new("base curves", BaseCurveNode);
    let id = node.id;
    graph.add_node(node);
    graph.add_output_data(FOOT_LOCK.to_owned(), DataSpec::F32);
    graph.add_output_data(VIEW_BLOCK.to_owned(), DataSpec::F32);
    graph.add_output_data_edge(id, FOOT_LOCK, FOOT_LOCK);
    graph.add_output_data_edge(id, VIEW_BLOCK, VIEW_BLOCK);
    graph
}

fn build_root_graph(inner: Handle<AnimationGraph>) -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new(
        "curve slot",
        SkeletonAwareLayeredGraphNode {
            graph: inner,
            fallback_pose_key: "unused".to_owned(),
            slot_name: Some("CurveProbe".to_owned()),
            slot_pose_key: "unused".to_owned(),
            slot_weight_key: SLOT_WEIGHT.to_owned(),
            slot_curve_prefix: Some(SLOT_PREFIX.to_owned()),
            slot_curve_names: vec![FOOT_LOCK.to_owned(), VIEW_BLOCK.to_owned()],
            always_update_source_pose: false,
        },
    );
    let id = node.id;
    graph.add_node(node);
    graph.add_output_data(FOOT_LOCK.to_owned(), DataSpec::F32);
    graph.add_output_data(VIEW_BLOCK.to_owned(), DataSpec::F32);
    graph.add_output_data_edge(id, FOOT_LOCK, FOOT_LOCK);
    graph.add_output_data_edge(id, VIEW_BLOCK, VIEW_BLOCK);
    graph
}

fn output_f32(player: &AnimationGraphPlayer, key: &str) -> Option<f32> {
    match player.get_outputs().get(key) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(mut player) = players.get_mut(probe.player) else {
        return;
    };
    if let Some(error) = player.get_error() {
        error!("[BF102.09][VERDICT] runtime error={error:?} -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(9).unwrap()));
        return;
    }
    let (Some(foot_lock), Some(view_block)) = (
        output_f32(&player, FOOT_LOCK),
        output_f32(&player, VIEW_BLOCK),
    ) else {
        return;
    };
    match probe.phase {
        Phase::Quarter if close(foot_lock, 0.75) && close(view_block, 0.25) => {
            info!("[BF102.09] weight=0.25 foot_lock={foot_lock:.3} view_block={view_block:.3}");
            player.set_global_input_data(SLOT_WEIGHT, 0.75_f32);
            probe.phase = Phase::ThreeQuarters;
        }
        Phase::ThreeQuarters if close(foot_lock, 0.25) && close(view_block, 0.75) => {
            info!("[BF102.09] weight=0.75 foot_lock={foot_lock:.3} view_block={view_block:.3}");
            info!("[BF102.09][VERDICT] pose_and_curves_share_slot_weight -> PASS");
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        error!("[BF102.09][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(10).unwrap()));
    }
}
