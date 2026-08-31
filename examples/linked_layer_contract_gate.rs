use std::time::{Duration, Instant};

use als_ext::{ALS_LAYER_CURVE_NAMES, ALS_LAYER_SIGNAL_NAMES, AlsExtPlugin, LinkedAnimLayerNode};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin},
    animation_graph_player::AnimationGraphPlayer,
    animation_node::{AnimationNode, NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
    plugin::AnimationGraphSet,
    pose::{Pose, RootMotionDelta},
};
use serde_json::Value;

const CONTRACT_PATH: &str = "graphs/ab_als/ali_overlay.animgraph.ron";

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: None,
            exit_condition: ExitCondition::DontExit,
            ..default()
        }),
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
    ))
    .init_resource::<RuntimeProbe>()
    .add_systems(Startup, (verify_native_source, load_contract).chain())
    .add_systems(Update, (verify_contract, auto_quit))
    .add_systems(
        PostUpdate,
        observe_runtime_probe.after(AnimationGraphSet::Final),
    );
    app.run();
}

fn verify_native_source() {
    let Ok(path) = std::env::var("ALI_OVERLAY_SOURCE_PATH") else {
        return;
    };
    let source: Value = serde_json::from_str(
        &std::fs::read_to_string(&path).expect("native ALI_Overlay source readable"),
    )
    .expect("native ALI_Overlay source parses");
    let properties = source["asset_properties"]
        .as_array()
        .expect("native ALI_Overlay properties");
    assert_eq!(native_property(properties, "TargetSkeleton"), "None");
    assert_eq!(
        native_property(properties, "BlueprintType"),
        "BPTYPE_Interface"
    );
    assert_eq!(
        native_property(properties, "PreviewAnimationBlueprintApplicationMethod"),
        "LinkedLayers"
    );
    assert_eq!(
        native_property(properties, "bEnableLinkedAnimLayerInstanceSharing"),
        "False"
    );
    assert!(native_property(properties, "ParentClass").contains("AnimLayerInterface"));
    assert!(native_property(properties, "FunctionGraphs").contains(":Overlay'"));
    assert!(native_property(properties, "CategorySorting").contains("Default Shared Group"));
    let graph = source["graphs"]
        .as_array()
        .and_then(|graphs| graphs.iter().find(|graph| graph["name"] == "Overlay"))
        .expect("native Overlay graph exists");
    let nodes = graph["nodes"].as_array().expect("native Overlay nodes");
    assert_eq!(nodes.len(), 1, "native Overlay graph has only output");
    let root = &nodes[0];
    assert!(
        root["class_path"]
            .as_str()
            .is_some_and(|class| class.contains("AnimGraphNode_Root"))
    );
    assert_eq!(root["position_x"].as_i64(), Some(0));
    assert_eq!(root["position_y"].as_i64(), Some(0));
    assert!(
        native_property(
            root["properties"]
                .as_array()
                .expect("native root properties"),
            "Node"
        )
        .contains("LayerGroup=\"Default Shared Group\"")
    );
    let target = std::fs::read_to_string(format!("assets/{CONTRACT_PATH}"))
        .expect("target ALI_Overlay readable");
    for field in [
        "nodes: []",
        "OutputData(\"pose\"): InputData(Passthrough(\"base_pose\"))",
        "OutputTime: InputTime(Passthrough(\"base_time\"))",
        "input_position: (-320.0, 0.0)",
        "output_position: (0.0, 0.0)",
    ] {
        assert!(target.contains(field), "target interface field {field}");
    }
    info!(
        "[LINKED-LAYER-SOURCE] interface=ALI_Overlay nodes=1 output=(0,0) linked_layers=true shared_instances=false fallback=base_pose -> PASS"
    );
}

fn native_property<'a>(properties: &'a [Value], name: &str) -> &'a str {
    properties
        .iter()
        .find(|property| property["name"] == name)
        .and_then(|property| property["exported_text"].as_str())
        .expect("native ALI_Overlay property")
}

#[derive(Resource)]
struct ContractHandle(Handle<AnimationGraph>);

#[derive(Resource)]
struct AutoQuit(Instant);

#[derive(Resource, Default)]
struct RuntimeProbe {
    player: Option<Entity>,
    phase: ProbePhase,
    static_verified: bool,
}

#[derive(Default, PartialEq, Eq)]
enum ProbePhase {
    #[default]
    Pending,
    First,
    Second,
}

#[derive(Reflect, Clone, Debug)]
#[reflect(NodeLike)]
#[type_path = "linked_layer_contract_gate"]
struct LayerProbeNode {
    marker: f32,
}

impl NodeLike for LayerProbeNode {
    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back("base_time")?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_time_update_back("base_time", ctx.time_update_fwd()?);
        let mut pose = ctx.data_back("base_pose")?.into_pose()?;
        pose.root_motion = Some(RootMotionDelta {
            translation: Vec3::new(self.marker, 0.0, 0.0),
            rotation: Quat::IDENTITY,
        });
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("pose", pose);
        for curve in ALS_LAYER_CURVE_NAMES {
            ctx.set_data_fwd(curve, self.marker);
        }
        for signal in ALS_LAYER_SIGNAL_NAMES {
            ctx.set_data_fwd(signal, self.marker);
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("base_pose".to_owned(), DataSpec::Pose);
        ctx.add_input_time("base_time".to_owned());
        ctx.add_output_data("pose".to_owned(), DataSpec::Pose);
        for curve in ALS_LAYER_CURVE_NAMES {
            ctx.add_output_data(curve.to_owned(), DataSpec::F32);
        }
        for signal in ALS_LAYER_SIGNAL_NAMES {
            ctx.add_output_data(signal.to_owned(), DataSpec::F32);
        }
        ctx.add_output_time();
        Ok(())
    }

    fn display_name(&self) -> String {
        "Layer Probe".into()
    }
}

fn load_contract(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(ContractHandle(assets.load(CONTRACT_PATH)));
    commands.insert_resource(AutoQuit(Instant::now() + Duration::from_secs(15)));
    info!("[LINKED-LAYER-CONTRACT] loading interface=ALI_Overlay");
}

fn verify_contract(
    mut commands: Commands,
    contract: Res<ContractHandle>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut probe: ResMut<RuntimeProbe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.static_verified {
        return;
    }
    let Some(graph) = graphs.get(&contract.0) else {
        return;
    };
    let base_pose = graph
        .io_spec
        .get_input_data(&GraphInputPin::passthrough("base_pose"))
        == Some(DataSpec::Pose);
    let base_time = graph
        .io_spec
        .has_input_time(&GraphInputPin::passthrough("base_time"));
    let pose = graph.io_spec.get_output_data(&"pose".to_owned()) == Some(DataSpec::Pose);
    let time = graph.io_spec.has_output_time();
    let curve_outputs = ALS_LAYER_CURVE_NAMES
        .iter()
        .all(|curve| graph.io_spec.get_output_data(&curve.to_string()) == Some(DataSpec::F32));
    let signal_outputs = ALS_LAYER_SIGNAL_NAMES
        .iter()
        .all(|signal| graph.io_spec.get_output_data(&signal.to_string()) == Some(DataSpec::F32));
    let no_defaults = graph.default_data.is_empty();
    let direct_pose = graph.edges_inverted.contains_key(
        &bevy_animation_graph::core::animation_graph::TargetPin::OutputData("pose".to_owned()),
    );
    let direct_time = graph
        .edges_inverted
        .contains_key(&bevy_animation_graph::core::animation_graph::TargetPin::OutputTime);
    let pass = base_pose
        && base_time
        && pose
        && time
        && curve_outputs
        && signal_outputs
        && no_defaults
        && direct_pose
        && direct_time;
    info!(
        "[LINKED-LAYER-CONTRACT] interface=ALI_Overlay base_pose={} base_time={} pose={} time={} curve_outputs={} signal_outputs={} no_defaults={} direct_pose={} direct_time={}",
        base_pose,
        base_time,
        pose,
        time,
        curve_outputs,
        signal_outputs,
        no_defaults,
        direct_pose,
        direct_time,
    );
    if !pass {
        info!("[LINKED-LAYER-CONTRACT][VERDICT] interface=ALI_Overlay -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
        return;
    }

    let first = graphs.add(build_probe_implementation(1.0));
    let second = graphs.add(build_probe_implementation(2.0));
    let root = graphs.add(build_probe_root(contract.0.clone(), first, second));
    let mut player = AnimationGraphPlayer::new(default()).with_graph(root);
    player.set_input_data("base_pose", DataValue::Pose(Pose::default()));
    player.set_input_data("layer_index", DataValue::F32(0.0));
    probe.player = Some(
        commands
            .spawn((
                Name::new("linked_layer_probe_root"),
                Children::default(),
                Transform::IDENTITY,
                player,
            ))
            .id(),
    );
    probe.phase = ProbePhase::First;
    probe.static_verified = true;
    info!("[LINKED-LAYER-CONTRACT] runtime selector initialized index=0");
}

fn build_probe_implementation(marker: f32) -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new("layer probe", LayerProbeNode { marker });
    let id = node.id;
    graph.add_node(node);
    graph.add_input_data(GraphInputPin::passthrough("base_pose"), DataSpec::Pose);
    graph.add_input_time(GraphInputPin::passthrough("base_time"));
    graph.add_output_data("pose".to_owned(), DataSpec::Pose);
    for curve in ALS_LAYER_CURVE_NAMES {
        graph.add_output_data(curve.to_owned(), DataSpec::F32);
    }
    for signal in ALS_LAYER_SIGNAL_NAMES {
        graph.add_output_data(signal.to_owned(), DataSpec::F32);
    }
    graph.add_output_time();
    graph.add_input_data_edge(GraphInputPin::passthrough("base_pose"), id, "base_pose");
    graph.add_input_time_edge(GraphInputPin::passthrough("base_time"), id, "base_time");
    graph.add_output_data_edge(id, "pose", "pose");
    for curve in ALS_LAYER_CURVE_NAMES {
        graph.add_output_data_edge(id, curve, curve);
    }
    for signal in ALS_LAYER_SIGNAL_NAMES {
        graph.add_output_data_edge(id, signal, signal);
    }
    graph.add_output_pose_edge(id);
    graph
}

fn build_probe_root(
    interface: Handle<AnimationGraph>,
    first: Handle<AnimationGraph>,
    second: Handle<AnimationGraph>,
) -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new(
        "linked overlay layer",
        LinkedAnimLayerNode {
            interface,
            implementations: vec![first, second],
            selector: "layer_index".to_owned(),
        },
    );
    let id = node.id;
    graph.add_node(node);
    graph.add_input_data(GraphInputPin::passthrough("base_pose"), DataSpec::Pose);
    graph.add_input_time(GraphInputPin::passthrough("base_time"));
    graph.add_input_data(GraphInputPin::passthrough("layer_index"), DataSpec::F32);
    graph.add_output_data("pose".to_owned(), DataSpec::Pose);
    for curve in ALS_LAYER_CURVE_NAMES {
        graph.add_output_data(curve.to_owned(), DataSpec::F32);
    }
    for signal in ALS_LAYER_SIGNAL_NAMES {
        graph.add_output_data(signal.to_owned(), DataSpec::F32);
    }
    graph.add_output_time();
    graph.add_input_data_edge(GraphInputPin::passthrough("base_pose"), id, "base_pose");
    graph.add_input_time_edge(GraphInputPin::passthrough("base_time"), id, "base_time");
    graph.add_input_data_edge(GraphInputPin::passthrough("layer_index"), id, "layer_index");
    graph.add_output_data_edge(id, "pose", "pose");
    for curve in ALS_LAYER_CURVE_NAMES {
        graph.add_output_data_edge(id, curve, curve);
    }
    for signal in ALS_LAYER_SIGNAL_NAMES {
        graph.add_output_data_edge(id, signal, signal);
    }
    graph.add_output_pose_edge(id);
    graph
}

fn observe_runtime_probe(
    mut probe: ResMut<RuntimeProbe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(entity) = probe.player else {
        return;
    };
    let Ok(mut player) = players.get_mut(entity) else {
        return;
    };
    if let Some(error) = player.get_error() {
        warn!("[LINKED-LAYER-CONTRACT][VERDICT] runtime error={error:?} -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(5).unwrap()));
        return;
    }
    let marker = player
        .get_default_output_pose()
        .and_then(|pose| pose.root_motion.as_ref())
        .map(|motion| motion.translation.x);
    let Some(marker) = marker else {
        return;
    };
    let curves_match = |expected: f32| {
        ALS_LAYER_CURVE_NAMES.iter().all(|curve| {
            matches!(player.get_outputs().get(*curve), Some(DataValue::F32(value)) if (*value - expected).abs() < 0.001)
        })
    };
    let signals_match = |expected: f32| {
        ALS_LAYER_SIGNAL_NAMES.iter().all(|signal| {
            matches!(player.get_outputs().get(*signal), Some(DataValue::F32(value)) if (*value - expected).abs() < 0.001)
        })
    };
    match probe.phase {
        ProbePhase::First
            if (marker - 1.0).abs() < 0.001 && curves_match(1.0) && signals_match(1.0) =>
        {
            info!(
                "[LINKED-LAYER-CONTRACT] runtime selector index=0 pose_marker={marker:.1} all_curve_markers=1.0 all_signal_markers=1.0"
            );
            player.set_input_data("layer_index", DataValue::F32(1.0));
            probe.phase = ProbePhase::Second;
        }
        ProbePhase::Second
            if (marker - 2.0).abs() < 0.001 && curves_match(2.0) && signals_match(2.0) =>
        {
            info!(
                "[LINKED-LAYER-CONTRACT] runtime selector index=1 pose_marker={marker:.1} all_curve_markers=2.0 all_signal_markers=2.0"
            );
            info!(
                "[LINKED-LAYER-CONTRACT][VERDICT] interface=ALI_Overlay replacement=true -> PASS"
            );
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

fn auto_quit(time: Res<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= time.0 {
        warn!("[LINKED-LAYER-CONTRACT][VERDICT] asset load timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(4).unwrap()));
    }
}
