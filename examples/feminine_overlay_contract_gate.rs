use std::{
    path::Path,
    time::{Duration, Instant},
};

use als_ext::{
    ALS_LAYER_CURVE_NAMES, ALS_LAYER_SIGNAL_NAMES, AlsExtPlugin, ConstantF32Node, FInterpNode,
    GlobalSyncGroupSampleNode, LinkedAnimLayerNode, MultiWayBlendNode, TimePositionNode,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::builtin_nodes::blend_node::{BlendMode, BlendNode};
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
const GRAPH_PATH: &str = "graphs/ab_als/overlays/feminine.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const GAIT_WALKING: &str = "als_overlay_gait_walking_amount";
const STANDING: &str = "als_overlay_standing_amount";
const CROUCHING: &str = "als_overlay_crouching_amount";
const IN_AIR: &str = "als_overlay_in_air_amount";
const GROUND_PREDICTION: &str = "als_overlay_ground_prediction_amount";
const ANIMATION_DELTA_SECONDS: &str = "als_animation_delta_seconds";

fn main() {
    let mut app = App::new();
    app.add_plugins((
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
    .init_resource::<RuntimeProbe>()
    .add_systems(Startup, load_assets)
    .add_systems(Update, (verify_static_contract, auto_quit))
    .add_systems(PostUpdate, observe_runtime.after(AnimationGraphSet::Final));
    app.run();
}

#[derive(Resource)]
struct GateAssets {
    interface: Handle<AnimationGraph>,
    implementation: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource)]
struct AutoQuit(Instant);

#[derive(Resource, Default)]
struct RuntimeProbe {
    player: Option<Entity>,
    phase: ProbePhase,
    default_pose: Option<Pose>,
    walking_pose: Option<Pose>,
    crouching_pose: Option<Pose>,
    air_start_pose: Option<Pose>,
    static_verified: bool,
}

#[derive(Default, PartialEq, Eq)]
enum ProbePhase {
    #[default]
    Pending,
    Default,
    Walking,
    Crouching,
    AirStart,
    AirPrediction,
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        interface: assets.load(INTERFACE_PATH),
        implementation: assets.load(GRAPH_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    commands.insert_resource(AutoQuit(Instant::now() + Duration::from_secs(20)));
    info!("[FEMININE-OVERLAY-CONTRACT] loading linked layer assets");
}

fn close(value: f32, expected: f32) -> bool {
    (value - expected).abs() < 1e-4
}

fn graph_has_node<T: 'static>(graph: &AnimationGraph, name: &str) -> bool {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<T>())
        .is_some()
}

fn node_data_edge(
    graph: &AnimationGraph,
    target_node_name: &str,
    target_pin: &str,
    source_node_name: &str,
    source_pin: &str,
) -> bool {
    let Some((target_id, _)) = graph
        .nodes
        .iter()
        .find(|(_, node)| node.name == target_node_name)
    else {
        return false;
    };
    let Some((source_id, _)) = graph
        .nodes
        .iter()
        .find(|(_, node)| node.name == source_node_name)
    else {
        return false;
    };
    matches!(
        graph
            .edges_inverted
            .get(&TargetPin::NodeData(*target_id, target_pin.to_owned())),
        Some(SourcePin::NodeData(actual_id, actual_pin))
            if actual_id == source_id && actual_pin == source_pin
    )
}

fn build_linked_root(
    interface: Handle<AnimationGraph>,
    implementation: Handle<AnimationGraph>,
) -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new(
        "AB_Als Feminine linked overlay",
        LinkedAnimLayerNode {
            interface,
            implementations: vec![implementation],
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

fn expected_curve(name: &str) -> f32 {
    match name {
        "LayerArmLeftAdditive" | "LayerArmRightAdditive" => 0.74999994,
        "HandLeftIk" | "HandRightIk" | "HipsDirectionLock" | "LayerHandLeft" | "LayerHandRight"
        | "PoseAiming" => 0.0,
        _ => 1.0,
    }
}

fn output_f32(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_outputs().get(name) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
}

fn outputs_match_source(player: &AnimationGraphPlayer) -> bool {
    ALS_LAYER_CURVE_NAMES.iter().all(|name| {
        output_f32(player, name).is_some_and(|value| close(value, expected_curve(name)))
    }) && output_f32(player, "transition_left_request").is_some_and(|value| value.abs() < 1e-6)
}

fn set_inputs(
    player: &mut AnimationGraphPlayer,
    gait_walking: f32,
    standing: f32,
    crouching: f32,
    in_air: f32,
    ground_prediction: f32,
    delta_seconds: f32,
) {
    player.set_global_input_data(GAIT_WALKING, gait_walking);
    player.set_global_input_data(STANDING, standing);
    player.set_global_input_data(CROUCHING, crouching);
    player.set_global_input_data(IN_AIR, in_air);
    player.set_global_input_data(GROUND_PREDICTION, ground_prediction);
    player.set_global_input_data(ANIMATION_DELTA_SECONDS, delta_seconds);
    player.set_global_input_data("als_secondary_motion_phase", 0.0_f32);
}

fn verify_static_contract(
    mut commands: Commands,
    assets: Res<AssetServer>,
    gate_assets: Res<GateAssets>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut probe: ResMut<RuntimeProbe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.static_verified
        || !assets.is_loaded_with_dependencies(&gate_assets.interface)
        || !assets.is_loaded_with_dependencies(&gate_assets.implementation)
        || !assets.is_loaded_with_dependencies(&gate_assets.skeleton)
    {
        return;
    }
    let Some(interface) = graphs.get(&gate_assets.interface) else {
        return;
    };
    let Some(graph) = graphs.get(&gate_assets.implementation) else {
        return;
    };
    let pose_contract = graph
        .io_spec
        .get_input_data(&GraphInputPin::passthrough("base_pose"))
        == Some(DataSpec::Pose)
        && graph
            .io_spec
            .has_input_time(&GraphInputPin::passthrough("base_time"))
        && graph.io_spec.get_output_data(&"pose".to_owned()) == Some(DataSpec::Pose)
        && graph.io_spec.has_output_time()
        && graph.default_data.is_empty();
    let curve_contract = ALS_LAYER_CURVE_NAMES
        .iter()
        .all(|curve| graph.io_spec.get_output_data(&curve.to_string()) == Some(DataSpec::F32));
    let signal_contract = ALS_LAYER_SIGNAL_NAMES
        .iter()
        .all(|signal| graph.io_spec.get_output_data(&signal.to_string()) == Some(DataSpec::F32));
    let interface_contract = interface
        .io_spec
        .get_input_data(&GraphInputPin::passthrough("base_pose"))
        == Some(DataSpec::Pose)
        && interface
            .io_spec
            .has_input_time(&GraphInputPin::passthrough("base_time"))
        && ALS_LAYER_CURVE_NAMES.iter().all(|curve| {
            interface.io_spec.get_output_data(&curve.to_string()) == Some(DataSpec::F32)
        })
        && ALS_LAYER_SIGNAL_NAMES.iter().all(|signal| {
            interface.io_spec.get_output_data(&signal.to_string()) == Some(DataSpec::F32)
        });
    let evaluators = [
        "feminine poses frame zero evaluator",
        "feminine poses frame one evaluator",
        "feminine poses frame two evaluator",
        "feminine poses frame one air evaluator",
        "feminine poses frame zero air evaluator",
    ]
    .into_iter()
    .all(|name| {
        graph
            .nodes
            .values()
            .find(|node| node.name == name)
            .and_then(|node| node.try_inner_downcast_ref::<TimePositionNode>())
            .is_some_and(|node| close(node.duration_fallback, 0.1))
    });
    let f_interp = graph
        .nodes
        .values()
        .find(|node| node.name == "ground prediction interpolation")
        .and_then(|node| node.try_inner_downcast_ref::<FInterpNode>())
        .is_some_and(|node| {
            close(node.increasing_speed, 20.0) && close(node.decreasing_speed, 5.0)
        });
    let topology = graph_has_node::<BlendNode>(graph, "grounded gait blend")
        && graph
            .nodes
            .values()
            .find(|node| node.name == "grounded stance multi way blend")
            .and_then(|node| node.try_inner_downcast_ref::<MultiWayBlendNode>())
            .is_some_and(|node| node.pose_count == 2 && node.normalize_alpha)
        && graph_has_node::<BlendNode>(graph, "air pose blend")
        && graph_has_node::<BlendNode>(graph, "grounded air blend");
    let idle_loop = graph
        .nodes
        .values()
        .find(|node| node.name == "secondary motion idle loop")
        .and_then(|node| node.try_inner_downcast_ref::<GlobalSyncGroupSampleNode>())
        .is_some_and(|node| {
            node.key == "als_secondary_motion_phase"
                && close(node.leader_duration, 2.666667)
                && node.leader_markers.is_empty()
                && close(node.follower_duration, 2.666667)
                && node.follower_markers.is_empty()
                && node.looped
        });
    let additive = graph
        .nodes
        .values()
        .find(|node| node.name == "secondary motion additive")
        .and_then(|node| node.try_inner_downcast_ref::<BlendNode>())
        .is_some_and(|node| matches!(node.mode, BlendMode::Additive))
        && graph
            .nodes
            .values()
            .find(|node| node.name == "secondary motion additive alpha")
            .and_then(|node| node.try_inner_downcast_ref::<ConstantF32Node>())
            .is_some_and(|node| close(node.value, 0.5))
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
    let curves = ALS_LAYER_CURVE_NAMES.iter().all(|curve| {
        graph
            .nodes
            .values()
            .find(|node| node.name == format!("{curve} curve"))
            .and_then(|node| node.try_inner_downcast_ref::<ConstantF32Node>())
            .is_some_and(|node| close(node.value, expected_curve(curve)))
    });
    let source = std::fs::read_to_string(Path::new("assets").join(GRAPH_PATH)).unwrap_or_default();
    let global_inputs = [
        GAIT_WALKING,
        STANDING,
        CROUCHING,
        IN_AIR,
        GROUND_PREDICTION,
        ANIMATION_DELTA_SECONDS,
    ]
    .into_iter()
    .all(|key| source.contains(key));
    let asset_contract = source.matches("A_Als_Feminine_Poses.anim.ron").count() == 5
        && source.contains("animations/Base/A_Als_Idle.anim.ron")
        && !source.contains("AB_Als_Monolithic");
    let pass = pose_contract
        && curve_contract
        && signal_contract
        && interface_contract
        && evaluators
        && f_interp
        && topology
        && idle_loop
        && additive
        && curves
        && global_inputs
        && asset_contract;
    info!(
        "[FEMININE-OVERLAY-CONTRACT] pose_contract={} curve_contract={} signal_contract={} interface_contract={} evaluators={} f_interp_20_5={} topology={} idle_loop={} additive={} curves={} globals={} asset_contract={}",
        pose_contract,
        curve_contract,
        signal_contract,
        interface_contract,
        evaluators,
        f_interp,
        topology,
        idle_loop,
        additive,
        curves,
        global_inputs,
        asset_contract,
    );
    if !pass {
        warn!("[FEMININE-OVERLAY-CONTRACT][VERDICT] static=false -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
        return;
    }
    let root = graphs.add(build_linked_root(
        gate_assets.interface.clone(),
        gate_assets.implementation.clone(),
    ));
    let mut player = AnimationGraphPlayer::new(gate_assets.skeleton.clone()).with_graph(root);
    player.set_input_data("base_pose", DataValue::Pose(Pose::default()));
    player.set_input_data("layer_index", DataValue::F32(0.0));
    set_inputs(&mut player, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.player = Some(
        commands
            .spawn((
                Name::new("feminine_overlay_contract_probe"),
                Children::default(),
                Transform::IDENTITY,
                player,
            ))
            .id(),
    );
    probe.phase = ProbePhase::Default;
    probe.static_verified = true;
    info!("[FEMININE-OVERLAY-CONTRACT] linked runtime probe initialized");
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!("[FEMININE-OVERLAY-CONTRACT][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(code).unwrap()));
}

fn observe_runtime(
    mut probe: ResMut<RuntimeProbe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    if !probe.static_verified {
        return;
    }
    let Some(entity) = probe.player else {
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
    if !outputs_match_source(&player) {
        fail(&mut exit, 5, "source curve or signal value mismatch");
        return;
    }
    match probe.phase {
        ProbePhase::Default => {
            probe.default_pose = Some(pose.clone());
            info!(
                "[FEMININE-OVERLAY-CONTRACT] runtime state=grounded_idle bones={} LayerArmLeftAdditive=0.75 LayerArmRightAdditive=0.75",
                pose.bones.len()
            );
            set_inputs(&mut player, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::Walking;
        }
        ProbePhase::Walking => {
            if !probe
                .default_pose
                .as_ref()
                .is_some_and(|default| default != &pose)
            {
                fail(&mut exit, 6, "gait walking did not change pose");
                return;
            }
            probe.walking_pose = Some(pose.clone());
            info!("[FEMININE-OVERLAY-CONTRACT] runtime state=grounded_walk pose_changed=true");
            set_inputs(&mut player, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::Crouching;
        }
        ProbePhase::Crouching => {
            if !probe
                .walking_pose
                .as_ref()
                .is_some_and(|walking| walking != &pose)
            {
                fail(&mut exit, 7, "crouching did not change pose");
                return;
            }
            probe.crouching_pose = Some(pose.clone());
            info!("[FEMININE-OVERLAY-CONTRACT] runtime state=crouching pose_changed=true");
            set_inputs(&mut player, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::AirStart;
        }
        ProbePhase::AirStart => {
            if !probe
                .crouching_pose
                .as_ref()
                .is_some_and(|crouching| crouching != &pose)
            {
                fail(&mut exit, 8, "air entry did not change pose");
                return;
            }
            probe.air_start_pose = Some(pose.clone());
            info!("[FEMININE-OVERLAY-CONTRACT] runtime state=air_start pose_changed=true");
            set_inputs(&mut player, 0.0, 1.0, 0.0, 1.0, 1.0, 0.01);
            player.queue_time_update(TimeUpdate::Delta(0.01));
            probe.phase = ProbePhase::AirPrediction;
        }
        ProbePhase::AirPrediction => {
            if !probe
                .air_start_pose
                .as_ref()
                .is_some_and(|air_start| air_start != &pose)
            {
                fail(&mut exit, 9, "ground prediction did not change pose");
                return;
            }
            info!(
                "[FEMININE-OVERLAY-CONTRACT] runtime state=air_prediction pose_changed=true f_interp=0.2"
            );
            info!("[FEMININE-OVERLAY-CONTRACT][VERDICT] linked=true -> PASS");
            exit.write(AppExit::Success);
        }
        ProbePhase::Pending => {}
    }
}

fn auto_quit(time: Res<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= time.0 {
        warn!("[FEMININE-OVERLAY-CONTRACT][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(10).unwrap()));
    }
}
