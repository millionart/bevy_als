use std::{
    path::Path,
    time::{Duration, Instant},
};

use als_ext::{
    ALS_LAYER_CURVE_NAMES, ALS_LAYER_SIGNAL_NAMES, AlsExtPlugin, ConstantF32Node, CurveModifyNode,
    CurveSampleSetNode, CurveWeightedSamplesNode, GameplayTagBlendMode, GameplayTagLayerBlendNode,
    LinkedAnimLayerNode, MultiWayBlendNode, TimePositionNode,
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
    animation_graph::{AnimationGraph, GraphInputPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    animation_node::AnimationNode,
    edge_data::{DataSpec, DataValue},
    plugin::AnimationGraphSet,
    pose::Pose,
    skeleton::Skeleton,
};

const INTERFACE_PATH: &str = "graphs/ab_als/ali_overlay.animgraph.ron";
const TORCH_GRAPH_PATH: &str = "graphs/ab_als/overlays/torch.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const GAIT_WALKING: &str = "als_overlay_gait_walking_amount";
const STANDING: &str = "als_overlay_standing_amount";
const CROUCHING: &str = "als_overlay_crouching_amount";
const PITCH: &str = "als_overlay_pitch_amount";
const ROTATION_MODE: &str = "als_overlay_rotation_mode_index";
const LOCOMOTION_ACTION: &str = "als_overlay_locomotion_action_index";

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
    last_pose: Option<Pose>,
    static_verified: bool,
}

#[derive(Default, PartialEq, Eq)]
enum ProbePhase {
    #[default]
    Pending,
    Normal,
    AimingStanding,
    AimingWalking,
    AimingCrouching,
    Mantling,
    GettingUp,
    Rolling,
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        interface: assets.load(INTERFACE_PATH),
        implementation: assets.load(TORCH_GRAPH_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    commands.insert_resource(AutoQuit(Instant::now() + Duration::from_secs(30)));
    info!("[TORCH-OVERLAY-CONTRACT] loading linked layer assets");
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

fn node<'a, T: 'static>(graph: &'a AnimationGraph, name: &str) -> Option<&'a T> {
    graph
        .nodes
        .values()
        .find(|candidate| candidate.name == name)
        .and_then(|candidate| candidate.try_inner_downcast_ref::<T>())
}

fn output_f32(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_outputs().get(name) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
}

fn outputs_match(player: &AnimationGraphPlayer, expected: &[(&str, f32)]) -> bool {
    expected
        .iter()
        .all(|(name, value)| output_f32(player, name).is_some_and(|actual| close(actual, *value)))
}

fn build_linked_root(
    interface: Handle<AnimationGraph>,
    implementation: Handle<AnimationGraph>,
) -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new(
        "AB_Als Torch linked overlay",
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

fn set_inputs(
    player: &mut AnimationGraphPlayer,
    gait_walking: f32,
    standing: f32,
    crouching: f32,
    pitch: f32,
    rotation_mode: f32,
    locomotion_action: f32,
) {
    player.set_global_input_data(GAIT_WALKING, gait_walking);
    player.set_global_input_data(STANDING, standing);
    player.set_global_input_data(CROUCHING, crouching);
    player.set_global_input_data(PITCH, pitch);
    player.set_global_input_data(ROTATION_MODE, rotation_mode);
    player.set_global_input_data(LOCOMOTION_ACTION, locomotion_action);
}

fn modify_contract(graph: &AnimationGraph, name: &str, curves: &[&str], values: &[f32]) -> bool {
    node::<CurveModifyNode>(graph, name).is_some_and(|node| {
        node.curve_names
            .iter()
            .map(String::as_str)
            .eq(curves.iter().copied())
            && node.values == values
            && close(node.alpha, 1.0)
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
    if probe.static_verified
        || !assets.is_loaded_with_dependencies(&gate_assets.interface)
        || !assets.is_loaded_with_dependencies(&gate_assets.implementation)
        || !assets.is_loaded_with_dependencies(&gate_assets.skeleton)
    {
        return;
    }
    let Some(torch_graph) = graphs.get(&gate_assets.implementation) else {
        return;
    };
    let pose_contract = torch_graph
        .io_spec
        .get_input_data(&GraphInputPin::passthrough("base_pose"))
        == Some(DataSpec::Pose)
        && torch_graph
            .io_spec
            .has_input_time(&GraphInputPin::passthrough("base_time"))
        && torch_graph.io_spec.get_output_data(&"pose".to_owned()) == Some(DataSpec::Pose)
        && torch_graph.io_spec.has_output_time()
        && torch_graph.default_data.is_empty();
    let curve_contract = ALS_LAYER_CURVE_NAMES.iter().all(|curve| {
        torch_graph.io_spec.get_output_data(&curve.to_string()) == Some(DataSpec::F32)
    });
    let signal_contract = ALS_LAYER_SIGNAL_NAMES.iter().all(|signal| {
        torch_graph.io_spec.get_output_data(&signal.to_string()) == Some(DataSpec::F32)
    });
    let evaluators = [
        "default frame zero evaluator",
        "default walking frame zero evaluator",
        "default frame four evaluator",
        "aim frame two evaluator",
        "aim frame three evaluator",
        "aim crouch frame five evaluator",
        "mantling frame zero evaluator",
        "getting up frame four evaluator",
        "rolling frame zero evaluator",
    ]
    .into_iter()
    .all(|name| {
        node::<TimePositionNode>(torch_graph, name)
            .is_some_and(|node| close(node.duration_fallback, 0.2) && node.loop_position)
    }) && node::<TimePositionNode>(torch_graph, "torch aim evaluator")
        .is_some_and(|node| close(node.duration_fallback, 1.0) && node.loop_position)
        && node::<TimePositionNode>(torch_graph, "torch crouch aim evaluator")
            .is_some_and(|node| close(node.duration_fallback, 1.0) && node.loop_position);
    let layers = graph_has_node::<BlendNode>(torch_graph, "default gait blend")
        && graph_has_node::<MultiWayBlendNode>(torch_graph, "default stance blend")
        && graph_has_node::<BlendNode>(torch_graph, "aim gait blend")
        && graph_has_node::<MultiWayBlendNode>(torch_graph, "aim stance blend")
        && graph_has_node::<LoopNode>(torch_graph, "secondary motion idle loop")
        && node::<BlendNode>(torch_graph, "secondary motion additive")
            .is_some_and(|node| matches!(node.mode, BlendMode::Additive))
        && node::<ConstantF32Node>(torch_graph, "secondary motion additive alpha")
            .is_some_and(|node| close(node.value, 0.5));
    let samples = node::<CurveSampleSetNode>(torch_graph, "torch action pose curve samples")
        .is_some_and(|node| {
            node.sample_positions == [0.0, 0.13333334]
                && node.curves.len() == ALS_LAYER_CURVE_NAMES.len()
        })
        && node::<CurveWeightedSamplesNode>(torch_graph, "torch normal pose curves").is_some_and(
            |node| {
                node.sample_positions == [0.0, 0.13333334]
                    && node.normalize_weights
                    && node.curves.len() == ALS_LAYER_CURVE_NAMES.len()
            },
        )
        && node::<CurveWeightedSamplesNode>(torch_graph, "torch aiming pose curves").is_some_and(
            |node| {
                node.sample_positions == [0.06666667, 0.1, 0.16666667]
                    && node.normalize_weights
                    && node.curves.len() == ALS_LAYER_CURVE_NAMES.len()
            },
        );
    let rotation_layer =
        node::<GameplayTagLayerBlendNode>(torch_graph, "rotation mode layer blend").is_some_and(
            |node| {
                node.tags == ["Als.RotationMode.Aiming"]
                    && node.blend_times == [0.75, 0.2]
                    && node.blend_mode == GameplayTagBlendMode::HermiteCubic
                    && node.normalize_alpha
                    && !node.normalize_curve_weights
            },
        );
    let action_layer =
        node::<GameplayTagLayerBlendNode>(torch_graph, "locomotion action layer blend")
            .is_some_and(|node| {
                node.tags
                    == [
                        "Als.LocomotionAction.Mantling",
                        "Als.LocomotionAction.GettingUp",
                        "Als.LocomotionAction.Rolling",
                    ]
                    && node.blend_times == [0.5, 0.2, 0.0, 0.2]
                    && node.blend_mode == GameplayTagBlendMode::Linear
                    && node.normalize_alpha
                    && !node.normalize_curve_weights
                    && node
                        .curve_names
                        .iter()
                        .map(String::as_str)
                        .eq(ALS_LAYER_CURVE_NAMES.iter().copied())
                    && node
                        .signal_names
                        .iter()
                        .map(String::as_str)
                        .eq(ALS_LAYER_SIGNAL_NAMES.iter().copied())
            });
    let modifiers = modify_contract(
        torch_graph,
        "getting up left arm curve modify",
        &["LayerArmLeft"],
        &[3.0],
    ) && modify_contract(
        torch_graph,
        "rolling left arm curve modify",
        &["LayerArmLeft", "LayerArmLeftAdditive"],
        &[3.0, 0.0],
    );
    let source =
        std::fs::read_to_string(Path::new("assets").join(TORCH_GRAPH_PATH)).unwrap_or_default();
    let asset_contract = source.matches("A_Als_Torch_Poses.anim.ron").count() == 9
        && source.contains("A_Als_Torch_Aim.anim.ron")
        && source.contains("A_Als_Torch_Aim_Crouch.anim.ron")
        && source.contains("A_Als_Idle.anim.ron")
        && !source.contains("AB_Als_Monolithic");
    let pass = pose_contract
        && curve_contract
        && signal_contract
        && evaluators
        && layers
        && samples
        && rotation_layer
        && action_layer
        && modifiers
        && asset_contract;
    info!(
        "[TORCH-OVERLAY-CONTRACT] pose_contract={} curve_contract={} signal_contract={} evaluators={} layers={} samples={} rotation_layer={} action_layer={} modifiers={} asset_contract={}",
        pose_contract,
        curve_contract,
        signal_contract,
        evaluators,
        layers,
        samples,
        rotation_layer,
        action_layer,
        modifiers,
        asset_contract,
    );
    if !pass {
        warn!("[TORCH-OVERLAY-CONTRACT][VERDICT] static=false -> FAIL");
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
                Name::new("torch_overlay_contract_probe"),
                Children::default(),
                Transform::IDENTITY,
                player,
            ))
            .id(),
    );
    probe.phase = ProbePhase::Normal;
    probe.static_verified = true;
    info!("[TORCH-OVERLAY-CONTRACT] linked runtime probe initialized");
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!("[TORCH-OVERLAY-CONTRACT][VERDICT] {message} -> FAIL");
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
    match probe.phase {
        ProbePhase::Normal => {
            let expected = [
                ("LayerArmLeft", 1.0),
                ("LayerArmLeftAdditive", 0.5),
                ("LayerArmLeftLocalSpace", 1.0),
                ("LayerArmRight", 1.0),
                ("LayerArmRightAdditive", 1.0),
                ("LayerArmRightLocalSpace", 1.0),
                ("LayerHeadAdditive", 1.0),
                ("LayerSpineAdditive", 1.0),
                ("HandLeftIk", 0.0),
                ("HandRightIk", 0.0),
                ("HipsDirectionLock", 0.0),
                ("LayerHandLeft", 1.0),
                ("LayerHandRight", 0.0),
                ("PoseAiming", 0.0),
                ("transition_left_request", 0.0),
            ];
            if !outputs_match(&player, &expected) {
                fail(&mut exit, 5, "normal source curve values");
                return;
            }
            probe.last_pose = Some(pose);
            info!("[TORCH-OVERLAY-CONTRACT] normal f0/f4 blend curves verified");
            set_inputs(&mut player, 0.0, 1.0, 0.0, 0.25, 1.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(1.0));
            probe.phase = ProbePhase::AimingStanding;
        }
        ProbePhase::AimingStanding => {
            let expected = [
                ("LayerArmLeftAdditive", 0.5),
                ("LayerArmLeftLocalSpace", 0.0),
                ("LayerArmRightAdditive", 1.0),
                ("LayerHeadAdditive", 0.75),
                ("LayerSpineAdditive", 0.75),
                ("HipsDirectionLock", 1.0),
                ("PoseAiming", 1.0),
            ];
            if !outputs_match(&player, &expected)
                || !probe
                    .last_pose
                    .as_ref()
                    .is_some_and(|previous| previous != &pose)
            {
                fail(&mut exit, 6, "aiming standing pose or curves");
                return;
            }
            probe.last_pose = Some(pose);
            info!("[TORCH-OVERLAY-CONTRACT] aiming standing f2 curves verified");
            set_inputs(&mut player, 1.0, 1.0, 0.0, 0.25, 1.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::AimingWalking;
        }
        ProbePhase::AimingWalking => {
            if !outputs_match(
                &player,
                &[
                    ("LayerArmLeftAdditive", 0.15),
                    ("LayerArmRightAdditive", 0.5),
                ],
            ) {
                fail(&mut exit, 7, "aiming walking f3 curve values");
                return;
            }
            info!("[TORCH-OVERLAY-CONTRACT] aiming walking f3 curves verified");
            set_inputs(&mut player, 0.0, 0.0, 1.0, 0.25, 1.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::AimingCrouching;
        }
        ProbePhase::AimingCrouching => {
            if !outputs_match(
                &player,
                &[
                    ("LayerArmLeftAdditive", 0.2),
                    ("LayerArmRightAdditive", 0.5),
                ],
            ) {
                fail(&mut exit, 8, "aiming crouching f5 curve values");
                return;
            }
            probe.last_pose = Some(pose);
            info!("[TORCH-OVERLAY-CONTRACT] aiming crouching f5 curves verified");
            set_inputs(&mut player, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
            player.queue_time_update(TimeUpdate::Delta(1.0));
            probe.phase = ProbePhase::Mantling;
        }
        ProbePhase::Mantling => {
            if !outputs_match(&player, &[("LayerArmLeft", 1.0), ("PoseAiming", 0.0)])
                || !probe
                    .last_pose
                    .as_ref()
                    .is_some_and(|previous| previous != &pose)
            {
                fail(&mut exit, 9, "mantling f0 pose or curves");
                return;
            }
            probe.last_pose = Some(pose);
            info!("[TORCH-OVERLAY-CONTRACT] action=Mantling f0 verified");
            set_inputs(&mut player, 0.0, 1.0, 0.0, 0.0, 0.0, 2.0);
            player.queue_time_update(TimeUpdate::Delta(1.0));
            probe.phase = ProbePhase::GettingUp;
        }
        ProbePhase::GettingUp => {
            if !outputs_match(
                &player,
                &[("LayerArmLeft", 3.0), ("LayerArmLeftAdditive", 0.5)],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(&mut exit, 10, "getting up f4 pose or curves");
                return;
            }
            probe.last_pose = Some(pose);
            info!("[TORCH-OVERLAY-CONTRACT] action=GettingUp LayerArmLeft=3 verified");
            set_inputs(&mut player, 0.0, 1.0, 0.0, 0.0, 0.0, 3.0);
            player.queue_time_update(TimeUpdate::Delta(1.0));
            probe.phase = ProbePhase::Rolling;
        }
        ProbePhase::Rolling => {
            if !outputs_match(
                &player,
                &[("LayerArmLeft", 3.0), ("LayerArmLeftAdditive", 0.0)],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(&mut exit, 11, "rolling f0 pose or curves");
                return;
            }
            info!(
                "[TORCH-OVERLAY-CONTRACT] action=Rolling LayerArmLeft=3 LayerArmLeftAdditive=0 verified"
            );
            info!("[TORCH-OVERLAY-CONTRACT][VERDICT] linked=true -> PASS");
            exit.write(AppExit::Success);
        }
        ProbePhase::Pending => {}
    }
}

fn auto_quit(time: Res<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= time.0 {
        warn!("[TORCH-OVERLAY-CONTRACT][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(12).unwrap()));
    }
}
