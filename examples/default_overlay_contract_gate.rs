use std::{
    path::Path,
    time::{Duration, Instant},
};

use als_ext::{
    ALS_LAYER_CURVE_NAMES, AlsExtPlugin, ConstantF32Node, FInterpNode, MultiWayBlendNode,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
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

const GRAPH_PATH: &str = "graphs/ab_als/overlays/default.animgraph.ron";
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
    .add_systems(Startup, load_graph)
    .add_systems(Update, (verify_static_contract, auto_quit))
    .add_systems(PostUpdate, observe_runtime.after(AnimationGraphSet::Final));
    app.run();
}

#[derive(Resource)]
struct GateAssets {
    graph: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource)]
struct AutoQuit(Instant);

#[derive(Resource, Default)]
struct RuntimeProbe {
    overlay_player: Option<Entity>,
    f_interp_player: Option<Entity>,
    overlay_phase: OverlayPhase,
    f_interp_phase: FInterpPhase,
    grounded_idle: Option<Pose>,
    grounded_walk: Option<Pose>,
    crouching: Option<Pose>,
    air_start: Option<Pose>,
    static_verified: bool,
    overlay_done: bool,
    f_interp_done: bool,
}

#[derive(Default, PartialEq, Eq)]
enum OverlayPhase {
    #[default]
    Pending,
    GroundedIdle,
    GroundedWalk,
    Crouching,
    AirStart,
    AirPrediction,
}

#[derive(Default, PartialEq, Eq)]
enum FInterpPhase {
    #[default]
    Pending,
    Initial,
    Rising,
    Falling,
}

fn load_graph(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        graph: assets.load(GRAPH_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    commands.insert_resource(AutoQuit(Instant::now() + Duration::from_secs(20)));
    info!("[DEFAULT-OVERLAY-CONTRACT] loading graph=AB_Als_Default");
}

fn graph_has_node(graph: &AnimationGraph, name: &str) -> bool {
    graph.nodes.values().any(|node| node.name == name)
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

fn set_overlay_inputs(
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
}

fn expected_default_curve(name: &str) -> f32 {
    match name {
        "HandLeftIk" | "HandRightIk" | "HipsDirectionLock" | "LayerHandLeft" | "LayerHandRight"
        | "PoseAiming" => 0.0,
        _ => 1.0,
    }
}

fn all_layer_curves_match_default(player: &AnimationGraphPlayer) -> bool {
    ALS_LAYER_CURVE_NAMES.iter().all(|name| {
        matches!(player.get_outputs().get(*name), Some(DataValue::F32(value)) if (*value - expected_default_curve(name)).abs() < 1e-6)
    })
}

fn verify_static_contract(
    mut commands: Commands,
    assets: Res<AssetServer>,
    gate_assets: Res<GateAssets>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut probe: ResMut<RuntimeProbe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.static_verified {
        return;
    }
    if !assets.is_loaded_with_dependencies(&gate_assets.graph)
        || !assets.is_loaded_with_dependencies(&gate_assets.skeleton)
    {
        return;
    }
    let Some(graph) = graphs.get(&gate_assets.graph) else {
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
        && graph.io_spec.has_output_time();
    let curve_contract = ALS_LAYER_CURVE_NAMES
        .iter()
        .all(|curve| graph.io_spec.get_output_data(&curve.to_string()) == Some(DataSpec::F32));
    let evaluator_count = [
        "default poses frame zero evaluator",
        "default poses frame one evaluator",
        "default poses frame two evaluator",
        "default poses frame one air evaluator",
        "default poses frame zero air evaluator",
    ]
    .into_iter()
    .filter(|name| graph_has_node(graph, name))
    .count()
        == 5;
    let f_interp = graph
        .nodes
        .values()
        .find(|node| node.name == "ground prediction interpolation")
        .and_then(|node| node.try_inner_downcast_ref::<FInterpNode>())
        .is_some_and(|node| {
            (node.increasing_speed - 20.0).abs() < 1e-6
                && (node.decreasing_speed - 5.0).abs() < 1e-6
        });
    let normalized_stance = graph
        .nodes
        .values()
        .find(|node| node.name == "grounded stance multi way blend")
        .and_then(|node| node.try_inner_downcast_ref::<MultiWayBlendNode>())
        .is_some_and(|node| node.pose_count == 2 && node.normalize_alpha);
    let idle_loop = graph
        .nodes
        .values()
        .find(|node| node.name == "secondary motion idle loop")
        .and_then(|node| node.try_inner_downcast_ref::<LoopNode>())
        .is_some_and(|node| node.interpolation_period.abs() < 1e-6);
    let additive = graph
        .nodes
        .values()
        .find(|node| node.name == "secondary motion additive")
        .and_then(|node| node.try_inner_downcast_ref::<BlendNode>())
        .is_some_and(|node| matches!(node.mode, BlendMode::Additive));
    let additive_alpha = graph
        .nodes
        .values()
        .find(|node| node.name == "secondary motion additive alpha")
        .and_then(|node| node.try_inner_downcast_ref::<ConstantF32Node>())
        .is_some_and(|node| (node.value - 0.75).abs() < 1e-6);
    let additive_order = node_data_edge(
        graph,
        "secondary motion additive",
        "pose_a",
        "grounded air blend",
        "pose",
    ) && node_data_edge(
        graph,
        "secondary motion additive",
        "pose_b",
        "secondary motion idle loop",
        "pose",
    );
    let source = std::fs::read_to_string(Path::new("assets").join(GRAPH_PATH)).unwrap_or_default();
    let global_inputs = [
        GAIT_WALKING,
        STANDING,
        CROUCHING,
        IN_AIR,
        GROUND_PREDICTION,
        ANIMATION_DELTA_SECONDS,
    ]
    .iter()
    .all(|name| source.contains(name))
        && [
            "overlay gait walking input",
            "overlay standing input",
            "overlay crouching input",
            "overlay in air input",
            "overlay ground prediction input",
            "animation delta seconds input",
        ]
        .into_iter()
        .all(|name| graph_has_node(graph, name));
    let curve_nodes = ALS_LAYER_CURVE_NAMES.iter().all(|curve| {
        graph
            .nodes
            .values()
            .find(|node| node.name == format!("{curve} curve"))
            .and_then(|node| node.try_inner_downcast_ref::<ConstantF32Node>())
            .is_some_and(|node| (node.value - expected_default_curve(curve)).abs() < 1e-6)
    });
    let asset_paths = source.matches("A_Als_Default_Poses.anim.ron").count() == 5
        && source.contains("animations/Base/A_Als_Idle.anim.ron");
    let pass = pose_contract
        && curve_contract
        && evaluator_count
        && f_interp
        && normalized_stance
        && idle_loop
        && additive
        && additive_alpha
        && additive_order
        && global_inputs
        && curve_nodes
        && asset_paths;
    info!(
        "[DEFAULT-OVERLAY-CONTRACT] pose_contract={} curve_contract={} evaluators={} f_interp_20_5={} normalized_stance={} idle_loop={} additive={} additive_alpha={} additive_order={} globals={} curve_nodes={} asset_paths={}",
        pose_contract,
        curve_contract,
        evaluator_count,
        f_interp,
        normalized_stance,
        idle_loop,
        additive,
        additive_alpha,
        additive_order,
        global_inputs,
        curve_nodes,
        asset_paths,
    );
    if !pass {
        info!("[DEFAULT-OVERLAY-CONTRACT][VERDICT] static=false -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
        return;
    }

    let mut overlay_player = AnimationGraphPlayer::new(gate_assets.skeleton.clone())
        .with_graph(gate_assets.graph.clone());
    overlay_player.set_input_data("base_pose", DataValue::Pose(Pose::default()));
    set_overlay_inputs(&mut overlay_player, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0);
    overlay_player.pause();
    overlay_player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.overlay_player = Some(
        commands
            .spawn((
                Name::new("default_overlay_contract_probe"),
                Children::default(),
                Transform::IDENTITY,
                overlay_player,
            ))
            .id(),
    );

    let f_interp_graph = graphs.add(build_f_interp_graph());
    let mut f_interp_player = AnimationGraphPlayer::new(default()).with_graph(f_interp_graph);
    f_interp_player.set_input_data("target", DataValue::F32(0.0));
    f_interp_player.set_input_data("delta_seconds", DataValue::F32(0.0));
    f_interp_player.pause();
    f_interp_player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.f_interp_player = Some(
        commands
            .spawn((
                Name::new("default_overlay_f_interp_probe"),
                Children::default(),
                Transform::IDENTITY,
                f_interp_player,
            ))
            .id(),
    );
    probe.overlay_phase = OverlayPhase::GroundedIdle;
    probe.f_interp_phase = FInterpPhase::Initial;
    probe.static_verified = true;
    info!("[DEFAULT-OVERLAY-CONTRACT] runtime probes initialized");
}

fn build_f_interp_graph() -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new(
        "f interp probe",
        FInterpNode {
            increasing_speed: 20.0,
            decreasing_speed: 5.0,
        },
    );
    let id = node.id;
    graph.add_node(node);
    graph.add_input_data(GraphInputPin::passthrough("target"), DataSpec::F32);
    graph.add_input_data(GraphInputPin::passthrough("delta_seconds"), DataSpec::F32);
    graph.add_output_data("value".to_owned(), DataSpec::F32);
    graph.add_input_data_edge(GraphInputPin::passthrough("target"), id, "target");
    graph.add_input_data_edge(
        GraphInputPin::passthrough("delta_seconds"),
        id,
        "delta_seconds",
    );
    graph.add_output_data_edge(id, "value", "value");
    graph
}

fn observe_overlay(
    probe: &mut RuntimeProbe,
    players: &mut Query<&mut AnimationGraphPlayer>,
    exit: &mut MessageWriter<AppExit>,
) {
    let Some(entity) = probe.overlay_player else {
        return;
    };
    let Ok(mut player) = players.get_mut(entity) else {
        return;
    };
    if let Some(error) = player.get_error() {
        warn!("[DEFAULT-OVERLAY-CONTRACT][VERDICT] overlay runtime error={error:?} -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(4).unwrap()));
        return;
    }
    let Some(pose) = player.get_default_output_pose().cloned() else {
        return;
    };
    if !all_layer_curves_match_default(&player) {
        warn!(
            "[DEFAULT-OVERLAY-CONTRACT][VERDICT] layer curves do not match source presence -> FAIL"
        );
        exit.write(AppExit::Error(std::num::NonZeroU8::new(5).unwrap()));
        return;
    }
    match probe.overlay_phase {
        OverlayPhase::GroundedIdle => {
            info!(
                "[DEFAULT-OVERLAY-CONTRACT] runtime state=grounded_idle bones={} standard_curves=1.0 absent_curves=0.0",
                pose.bones.len()
            );
            probe.grounded_idle = Some(pose);
            set_overlay_inputs(&mut player, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.overlay_phase = OverlayPhase::GroundedWalk;
        }
        OverlayPhase::GroundedWalk => {
            let changed = probe
                .grounded_idle
                .as_ref()
                .is_some_and(|idle| idle != &pose);
            if !changed {
                warn!(
                    "[DEFAULT-OVERLAY-CONTRACT][VERDICT] gait walking did not change pose -> FAIL"
                );
                exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
                return;
            }
            info!(
                "[DEFAULT-OVERLAY-CONTRACT] runtime state=grounded_walk pose_changed=true standard_curves=1.0 absent_curves=0.0"
            );
            probe.grounded_walk = Some(pose);
            set_overlay_inputs(&mut player, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.overlay_phase = OverlayPhase::Crouching;
        }
        OverlayPhase::Crouching => {
            let changed = probe
                .grounded_walk
                .as_ref()
                .is_some_and(|walk| walk != &pose);
            if !changed {
                warn!("[DEFAULT-OVERLAY-CONTRACT][VERDICT] crouching did not change pose -> FAIL");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));
                return;
            }
            info!(
                "[DEFAULT-OVERLAY-CONTRACT] runtime state=crouching pose_changed=true standard_curves=1.0 absent_curves=0.0"
            );
            probe.crouching = Some(pose);
            set_overlay_inputs(&mut player, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.overlay_phase = OverlayPhase::AirStart;
        }
        OverlayPhase::AirStart => {
            let changed = probe
                .crouching
                .as_ref()
                .is_some_and(|crouching| crouching != &pose);
            if !changed {
                warn!("[DEFAULT-OVERLAY-CONTRACT][VERDICT] air entry did not change pose -> FAIL");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(8).unwrap()));
                return;
            }
            info!(
                "[DEFAULT-OVERLAY-CONTRACT] runtime state=air_start pose_changed=true standard_curves=1.0 absent_curves=0.0"
            );
            probe.air_start = Some(pose);
            set_overlay_inputs(&mut player, 0.0, 1.0, 0.0, 1.0, 1.0, 0.01);
            player.queue_time_update(TimeUpdate::Delta(0.01));
            probe.overlay_phase = OverlayPhase::AirPrediction;
        }
        OverlayPhase::AirPrediction => {
            let changed = probe
                .air_start
                .as_ref()
                .is_some_and(|air_start| air_start != &pose);
            if !changed {
                warn!(
                    "[DEFAULT-OVERLAY-CONTRACT][VERDICT] ground prediction did not change pose -> FAIL"
                );
                exit.write(AppExit::Error(std::num::NonZeroU8::new(9).unwrap()));
                return;
            }
            info!(
                "[DEFAULT-OVERLAY-CONTRACT] runtime state=air_prediction pose_changed=true standard_curves=1.0 absent_curves=0.0"
            );
            probe.overlay_done = true;
            probe.overlay_phase = OverlayPhase::Pending;
        }
        OverlayPhase::Pending => {}
    }
}

fn observe_f_interp(
    probe: &mut RuntimeProbe,
    players: &mut Query<&mut AnimationGraphPlayer>,
    exit: &mut MessageWriter<AppExit>,
) {
    let Some(entity) = probe.f_interp_player else {
        return;
    };
    let Ok(mut player) = players.get_mut(entity) else {
        return;
    };
    if let Some(error) = player.get_error() {
        warn!("[DEFAULT-OVERLAY-CONTRACT][VERDICT] f_interp runtime error={error:?} -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(10).unwrap()));
        return;
    }
    let Some(DataValue::F32(value)) = player.get_outputs().get("value") else {
        return;
    };
    match probe.f_interp_phase {
        FInterpPhase::Initial if value.abs() < 1e-6 => {
            player.set_input_data("target", DataValue::F32(1.0));
            player.set_input_data("delta_seconds", DataValue::F32(0.01));
            player.queue_time_update(TimeUpdate::Delta(0.01));
            probe.f_interp_phase = FInterpPhase::Rising;
        }
        FInterpPhase::Rising if (*value - 0.2).abs() < 1e-6 => {
            info!("[DEFAULT-OVERLAY-CONTRACT] f_interp rise dt=0.01 speed=20 value={value:.3}");
            player.set_input_data("target", DataValue::F32(0.0));
            player.set_input_data("delta_seconds", DataValue::F32(0.1));
            player.queue_time_update(TimeUpdate::Delta(0.1));
            probe.f_interp_phase = FInterpPhase::Falling;
        }
        FInterpPhase::Falling if (*value - 0.1).abs() < 1e-6 => {
            info!("[DEFAULT-OVERLAY-CONTRACT] f_interp fall dt=0.1 speed=5 value={value:.3}");
            probe.f_interp_done = true;
            probe.f_interp_phase = FInterpPhase::Pending;
        }
        FInterpPhase::Initial | FInterpPhase::Rising | FInterpPhase::Falling => {
            warn!("[DEFAULT-OVERLAY-CONTRACT][VERDICT] f_interp value={value:.6} -> FAIL");
            exit.write(AppExit::Error(std::num::NonZeroU8::new(11).unwrap()));
        }
        FInterpPhase::Pending => {}
    }
}

fn observe_runtime(
    mut probe: ResMut<RuntimeProbe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    if !probe.static_verified {
        return;
    }
    observe_overlay(&mut probe, &mut players, &mut exit);
    observe_f_interp(&mut probe, &mut players, &mut exit);
    if probe.overlay_done && probe.f_interp_done {
        info!("[DEFAULT-OVERLAY-CONTRACT][VERDICT] graph=AB_Als_Default -> PASS");
        exit.write(AppExit::Success);
    }
}

fn auto_quit(time: Res<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= time.0 {
        warn!("[DEFAULT-OVERLAY-CONTRACT][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(12).unwrap()));
    }
}
