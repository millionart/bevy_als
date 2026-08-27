use std::{
    path::Path,
    time::{Duration, Instant},
};

use als_ext::{
    ALS_LAYER_CURVE_NAMES, ALS_LAYER_SIGNAL_NAMES, AlsExtPlugin, ConstantF32Node, FInterpNode,
    LinkedAnimLayerNode, MultiWayBlendNode, TimePositionNode,
};
use bevy::{ecs::message::MessageWriter, prelude::*, window::ExitCondition};
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::builtin_nodes::{
    blend_node::{BlendMode, BlendNode},
    loop_node::LoopNode,
};
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, SourcePin, TargetPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    animation_node::AnimationNode,
    edge_data::{DataSpec, DataValue},
    plugin::AnimationGraphSet,
    pose::Pose,
    skeleton::Skeleton,
};

const INTERFACE_PATH: &str = "graphs/ab_als/ali_overlay.animgraph.ron";
const GRAPH_PATH: &str = "graphs/ab_als/overlays/masculine.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const GAIT: &str = "als_overlay_gait_walking_amount";
const STANDING: &str = "als_overlay_standing_amount";
const CROUCHING: &str = "als_overlay_crouching_amount";
const IN_AIR: &str = "als_overlay_in_air_amount";
const GROUND_PREDICTION: &str = "als_overlay_ground_prediction_amount";
const DELTA_SECONDS: &str = "als_animation_delta_seconds";

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
        .add_systems(Startup, load)
        .add_systems(Update, (verify, timeout))
        .add_systems(PostUpdate, observe.after(AnimationGraphSet::Final))
        .run();
}

#[derive(Resource)]
struct AssetsForGate {
    interface: Handle<AnimationGraph>,
    implementation: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}
#[derive(Resource)]
struct Deadline(Instant);
#[derive(Resource, Default)]
struct Probe {
    entity: Option<Entity>,
    phase: Phase,
    previous: Option<Pose>,
    verified: bool,
}
#[derive(Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Pending,
    Default,
    Walking,
    AirStart,
    AirPrediction,
}

fn load(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(AssetsForGate {
        interface: assets.load(INTERFACE_PATH),
        implementation: assets.load(GRAPH_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    commands.insert_resource(Deadline(Instant::now() + Duration::from_secs(20)));
    info!("[MASCULINE-OVERLAY-CONTRACT] loading linked layer assets");
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}
fn node<T: 'static>(graph: &AnimationGraph, name: &str) -> bool {
    graph
        .nodes
        .values()
        .find(|value| value.name == name)
        .and_then(|value| value.try_inner_downcast_ref::<T>())
        .is_some()
}
fn node_data_edge(
    graph: &AnimationGraph,
    target_name: &str,
    target_pin: &str,
    source_name: &str,
    source_pin: &str,
) -> bool {
    let Some((target_id, _)) = graph
        .nodes
        .iter()
        .find(|(_, value)| value.name == target_name)
    else {
        return false;
    };
    let Some((source_id, _)) = graph
        .nodes
        .iter()
        .find(|(_, value)| value.name == source_name)
    else {
        return false;
    };
    matches!(graph.edges_inverted.get(&TargetPin::NodeData(*target_id, target_pin.to_owned())), Some(SourcePin::NodeData(id, pin)) if id == source_id && pin == source_pin)
}
fn expected_curve(name: &str) -> f32 {
    match name {
        "HandLeftIk" | "HandRightIk" | "HipsDirectionLock" | "LayerHandLeft" | "LayerHandRight"
        | "PoseAiming" => 0.0,
        _ => 1.0,
    }
}
fn output(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_outputs().get(name) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
}
fn source_outputs(player: &AnimationGraphPlayer) -> bool {
    ALS_LAYER_CURVE_NAMES
        .iter()
        .all(|name| output(player, name).is_some_and(|value| close(value, expected_curve(name))))
        && output(player, "transition_left_request").is_some_and(|value| value.abs() < 1e-6)
}
fn set_inputs(
    player: &mut AnimationGraphPlayer,
    gait: f32,
    standing: f32,
    crouching: f32,
    air: f32,
    prediction: f32,
    delta: f32,
) {
    player.set_global_input_data(GAIT, gait);
    player.set_global_input_data(STANDING, standing);
    player.set_global_input_data(CROUCHING, crouching);
    player.set_global_input_data(IN_AIR, air);
    player.set_global_input_data(GROUND_PREDICTION, prediction);
    player.set_global_input_data(DELTA_SECONDS, delta);
}
fn build_root(
    interface: Handle<AnimationGraph>,
    implementation: Handle<AnimationGraph>,
) -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let linked = AnimationNode::new(
        "AB_Als Masculine linked overlay",
        LinkedAnimLayerNode {
            interface,
            implementations: vec![implementation],
            selector: "layer_index".to_owned(),
        },
    );
    let id = linked.id;
    graph.add_node(linked);
    graph.add_input_data(GraphInputPin::passthrough("base_pose"), DataSpec::Pose);
    graph.add_input_time(GraphInputPin::passthrough("base_time"));
    graph.add_input_data(GraphInputPin::passthrough("layer_index"), DataSpec::F32);
    graph.add_output_data("pose".to_owned(), DataSpec::Pose);
    for name in ALS_LAYER_CURVE_NAMES {
        graph.add_output_data(name.to_owned(), DataSpec::F32);
    }
    for name in ALS_LAYER_SIGNAL_NAMES {
        graph.add_output_data(name.to_owned(), DataSpec::F32);
    }
    graph.add_output_time();
    graph.add_input_data_edge(GraphInputPin::passthrough("base_pose"), id, "base_pose");
    graph.add_input_time_edge(GraphInputPin::passthrough("base_time"), id, "base_time");
    graph.add_input_data_edge(GraphInputPin::passthrough("layer_index"), id, "layer_index");
    graph.add_output_data_edge(id, "pose", "pose");
    for name in ALS_LAYER_CURVE_NAMES {
        graph.add_output_data_edge(id, name, name);
    }
    for name in ALS_LAYER_SIGNAL_NAMES {
        graph.add_output_data_edge(id, name, name);
    }
    graph.add_output_pose_edge(id);
    graph
}
fn verify(
    mut commands: Commands,
    assets: Res<AssetServer>,
    gate: Res<AssetsForGate>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut probe: ResMut<Probe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.verified
        || !assets.is_loaded_with_dependencies(&gate.interface)
        || !assets.is_loaded_with_dependencies(&gate.implementation)
        || !assets.is_loaded_with_dependencies(&gate.skeleton)
    {
        return;
    }
    let Some(interface) = graphs.get(&gate.interface) else {
        return;
    };
    let Some(graph) = graphs.get(&gate.implementation) else {
        return;
    };
    let interface_contract = interface
        .io_spec
        .get_input_data(&GraphInputPin::passthrough("base_pose"))
        == Some(DataSpec::Pose)
        && interface
            .io_spec
            .has_input_time(&GraphInputPin::passthrough("base_time"))
        && ALS_LAYER_CURVE_NAMES.iter().all(|name| {
            interface.io_spec.get_output_data(&name.to_string()) == Some(DataSpec::F32)
        });
    let graph_contract = graph
        .io_spec
        .get_input_data(&GraphInputPin::passthrough("base_pose"))
        == Some(DataSpec::Pose)
        && graph
            .io_spec
            .has_input_time(&GraphInputPin::passthrough("base_time"))
        && graph.io_spec.get_output_data(&"pose".to_owned()) == Some(DataSpec::Pose)
        && graph.io_spec.has_output_time()
        && graph.default_data.is_empty()
        && ALS_LAYER_CURVE_NAMES
            .iter()
            .all(|name| graph.io_spec.get_output_data(&name.to_string()) == Some(DataSpec::F32))
        && ALS_LAYER_SIGNAL_NAMES
            .iter()
            .all(|name| graph.io_spec.get_output_data(&name.to_string()) == Some(DataSpec::F32));
    let evaluators = [
        "masculine poses frame zero evaluator",
        "masculine poses frame one evaluator",
        "masculine poses frame two evaluator",
        "masculine poses frame one air evaluator",
        "masculine poses frame zero air evaluator",
    ]
    .into_iter()
    .all(|name| {
        graph
            .nodes
            .values()
            .find(|value| value.name == name)
            .and_then(|value| value.try_inner_downcast_ref::<TimePositionNode>())
            .is_some_and(|value| close(value.duration_fallback, 0.1))
    });
    let topology = node::<BlendNode>(graph, "grounded gait blend")
        && graph
            .nodes
            .values()
            .find(|value| value.name == "grounded stance multi way blend")
            .and_then(|value| value.try_inner_downcast_ref::<MultiWayBlendNode>())
            .is_some_and(|value| value.pose_count == 2 && value.normalize_alpha)
        && node::<BlendNode>(graph, "air pose blend")
        && node::<BlendNode>(graph, "grounded air blend");
    let f_interp = graph
        .nodes
        .values()
        .find(|value| value.name == "ground prediction interpolation")
        .and_then(|value| value.try_inner_downcast_ref::<FInterpNode>())
        .is_some_and(|value| {
            close(value.increasing_speed, 20.0) && close(value.decreasing_speed, 5.0)
        });
    let additive = graph
        .nodes
        .values()
        .find(|value| value.name == "secondary motion additive")
        .and_then(|value| value.try_inner_downcast_ref::<BlendNode>())
        .is_some_and(|value| matches!(value.mode, BlendMode::Additive))
        && graph
            .nodes
            .values()
            .find(|value| value.name == "secondary motion additive alpha")
            .and_then(|value| value.try_inner_downcast_ref::<ConstantF32Node>())
            .is_some_and(|value| close(value.value, 1.0))
        && graph
            .nodes
            .values()
            .find(|value| value.name == "secondary motion idle loop")
            .and_then(|value| value.try_inner_downcast_ref::<LoopNode>())
            .is_some_and(|value| value.interpolation_period.abs() < 1e-6)
        && node_data_edge(
            graph,
            "secondary motion additive",
            "pose_a",
            "grounded air blend",
            "pose",
        )
        && node_data_edge(
            graph,
            "secondary motion additive",
            "pose_b",
            "secondary motion idle loop",
            "pose",
        );
    let curves = ALS_LAYER_CURVE_NAMES.iter().all(|name| {
        graph
            .nodes
            .values()
            .find(|value| value.name == format!("{name} curve"))
            .and_then(|value| value.try_inner_downcast_ref::<ConstantF32Node>())
            .is_some_and(|value| close(value.value, expected_curve(name)))
    });
    let source = std::fs::read_to_string(Path::new("assets").join(GRAPH_PATH)).unwrap_or_default();
    let assets_contract = source.matches("A_Als_Masculine_Poses.anim.ron").count() == 5
        && source.contains("animations/Base/A_Als_Idle.anim.ron")
        && !source.contains("AB_Als_Monolithic")
        && !source.contains("GameplayTagBlendNode")
        && [
            GAIT,
            STANDING,
            CROUCHING,
            IN_AIR,
            GROUND_PREDICTION,
            DELTA_SECONDS,
        ]
        .into_iter()
        .all(|name| source.contains(name));
    let pass = interface_contract
        && graph_contract
        && evaluators
        && topology
        && f_interp
        && additive
        && curves
        && assets_contract;
    info!(
        "[MASCULINE-OVERLAY-CONTRACT] interface_contract={} graph_contract={} evaluators={} topology={} f_interp_20_5={} additive_alpha_1={} curves={} assets_contract={}",
        interface_contract,
        graph_contract,
        evaluators,
        topology,
        f_interp,
        additive,
        curves,
        assets_contract
    );
    if !pass {
        warn!("[MASCULINE-OVERLAY-CONTRACT][VERDICT] static=false -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
        return;
    }
    let root = graphs.add(build_root(
        gate.interface.clone(),
        gate.implementation.clone(),
    ));
    let mut player = AnimationGraphPlayer::new(gate.skeleton.clone()).with_graph(root);
    player.set_input_data("base_pose", DataValue::Pose(Pose::default()));
    player.set_input_data("layer_index", DataValue::F32(0.0));
    set_inputs(&mut player, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.entity = Some(
        commands
            .spawn((
                Name::new("masculine_overlay_contract_probe"),
                Children::default(),
                Transform::IDENTITY,
                player,
            ))
            .id(),
    );
    probe.phase = Phase::Default;
    probe.verified = true;
    info!("[MASCULINE-OVERLAY-CONTRACT] linked runtime probe initialized");
}
fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!("[MASCULINE-OVERLAY-CONTRACT][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(code).unwrap()));
}
fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    if !probe.verified {
        return;
    }
    let Some(entity) = probe.entity else {
        return;
    };
    let Ok(mut player) = players.get_mut(entity) else {
        return;
    };
    if let Some(error) = player.get_error() {
        fail(&mut exit, 4, &format!("runtime error={error:?}"));
        return;
    }
    let Some(pose) = player.get_default_output_pose().cloned() else {
        return;
    };
    if !source_outputs(&player) {
        fail(&mut exit, 5, "source curve or signal value mismatch");
        return;
    }
    match probe.phase {
        Phase::Default => {
            probe.previous = Some(pose);
            info!(
                "[MASCULINE-OVERLAY-CONTRACT] runtime state=grounded_idle bones=79 additive_alpha=1"
            );
            set_inputs(&mut player, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::Walking;
        }
        Phase::Walking => {
            if !probe.previous.as_ref().is_some_and(|value| value != &pose) {
                fail(&mut exit, 6, "gait walking did not change pose");
                return;
            }
            probe.previous = Some(pose);
            info!("[MASCULINE-OVERLAY-CONTRACT] runtime state=grounded_walk pose_changed=true");
            set_inputs(&mut player, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::AirStart;
        }
        Phase::AirStart => {
            if !probe.previous.as_ref().is_some_and(|value| value != &pose) {
                fail(&mut exit, 7, "air entry did not change pose");
                return;
            }
            probe.previous = Some(pose);
            info!("[MASCULINE-OVERLAY-CONTRACT] runtime state=air_start pose_changed=true");
            set_inputs(&mut player, 0.0, 1.0, 0.0, 1.0, 1.0, 0.01);
            player.queue_time_update(TimeUpdate::Delta(0.01));
            probe.phase = Phase::AirPrediction;
        }
        Phase::AirPrediction => {
            if !probe.previous.as_ref().is_some_and(|value| value != &pose) {
                fail(&mut exit, 8, "ground prediction did not change pose");
                return;
            }
            info!(
                "[MASCULINE-OVERLAY-CONTRACT] runtime state=air_prediction pose_changed=true f_interp=0.2"
            );
            info!("[MASCULINE-OVERLAY-CONTRACT][VERDICT] linked=true -> PASS");
            exit.write(AppExit::Success);
        }
        Phase::Pending => {}
    }
}
fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        warn!("[MASCULINE-OVERLAY-CONTRACT][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(9).unwrap()));
    }
}
