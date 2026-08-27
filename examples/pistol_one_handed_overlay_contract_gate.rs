use std::time::{Duration, Instant};

use als_ext::{
    ALS_LAYER_CURVE_NAMES, ALS_LAYER_SIGNAL_NAMES, AlphaBlendMode, AlsExtPlugin, ConstantF32Node,
    CurveModifyNode, CurveSampleSetNode, CurveWeightedSamplesNode, GameplayTagBlendMode,
    GameplayTagLayerBlendNode, GlobalF32InputNode, LinkedAnimLayerNode, MeshSpaceAdditiveNode,
    ProfiledBlendNode, SuppressFirstUpdateNode, TimePositionNode, transition_alpha,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::builtin_nodes::{
    blend_node::{BlendMode, BlendNode},
    fsm_node::FsmNode,
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
    state_machine::high_level::{StateMachine, TransitionKind},
};

const INTERFACE_PATH: &str = "graphs/ab_als/ali_overlay.animgraph.ron";
const PISTOL_PATH: &str = "graphs/ab_als/overlays/pistol_one_handed.animgraph.ron";
const FSM_PATH: &str = "graphs/ab_als/overlays/pistol_one_handed/pistol_states.fsm.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const GAIT_WALKING: &str = "als_overlay_gait_walking_amount";
const STANDING: &str = "als_overlay_standing_amount";
const CROUCHING: &str = "als_overlay_crouching_amount";
const ROTATION_MODE: &str = "als_overlay_rotation_mode_index";
const LOCOMOTION_ACTION: &str = "als_overlay_locomotion_action_index";
const MOVING: &str = "als_overlay_is_moving";
const SPRINTING: &str = "als_overlay_is_sprinting";
const IN_AIR: &str = "als_overlay_is_in_air";
const TRANSITIONS_ALLOWED: &str = "als_overlay_transitions_allowed";
const PITCH_AMOUNT: &str = "als_overlay_pitch_amount";

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
    pistol: Handle<AnimationGraph>,
    fsm: Handle<StateMachine>,
    skeleton: Handle<Skeleton>,
    relaxed: Handle<AnimationGraph>,
    ready: Handle<AnimationGraph>,
    aiming: Handle<AnimationGraph>,
    aim_in: Handle<AnimationGraph>,
    aim_out: Handle<AnimationGraph>,
    relaxed_to_ready: Handle<AnimationGraph>,
    ready_to_relaxed_allowed: Handle<AnimationGraph>,
    ready_to_relaxed_moving: Handle<AnimationGraph>,
    ready_to_relaxed_interrupt: Handle<AnimationGraph>,
}

#[derive(Resource)]
struct AutoQuit(Instant);

#[derive(Resource, Default)]
struct RuntimeProbe {
    player: Option<Entity>,
    phase: ProbePhase,
    static_verified: bool,
    frames: u8,
    relaxed: Option<Pose>,
    walking: Option<Pose>,
}

#[derive(Default, PartialEq, Eq)]
enum ProbePhase {
    #[default]
    Pending,
    Mantling,
    GettingUp,
    Rolling,
    Relaxed,
    Walking,
    Crouching,
    BeginAim,
    Aiming,
    BeginAimOut,
    Ready,
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        interface: assets.load(INTERFACE_PATH),
        pistol: assets.load(PISTOL_PATH),
        fsm: assets.load(FSM_PATH),
        skeleton: assets.load(SKELETON_PATH),
        relaxed: assets
            .load("graphs/ab_als/overlays/pistol_one_handed/state_relaxed.animgraph.ron"),
        ready: assets.load("graphs/ab_als/overlays/pistol_one_handed/state_ready.animgraph.ron"),
        aiming: assets.load("graphs/ab_als/overlays/pistol_one_handed/state_aiming.animgraph.ron"),
        aim_in: assets.load("graphs/ab_als/overlays/bow/transition_ready_to_aiming.animgraph.ron"),
        aim_out: assets.load("graphs/ab_als/overlays/bow/transition_aiming_to_ready.animgraph.ron"),
        relaxed_to_ready: assets
            .load("graphs/ab_als/overlays/bow/transition_relaxed_to_ready.animgraph.ron"),
        ready_to_relaxed_allowed: assets
            .load("graphs/ab_als/overlays/bow/transition_ready_to_relaxed_allowed.animgraph.ron"),
        ready_to_relaxed_moving: assets
            .load("graphs/ab_als/overlays/bow/transition_ready_to_relaxed_moving.animgraph.ron"),
        ready_to_relaxed_interrupt: assets
            .load("graphs/ab_als/overlays/bow/transition_ready_to_relaxed_interrupt.animgraph.ron"),
    });
    commands.insert_resource(AutoQuit(Instant::now() + Duration::from_secs(30)));
    info!("[PISTOL-ONE-HANDED-CONTRACT] loading linked overlay assets");
}

fn close(value: f32, expected: f32) -> bool {
    (value - expected).abs() < 1e-4
}

fn output_f32(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_outputs().get(name) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
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
        graph.edges_inverted.get(&TargetPin::NodeData(*target_id, target_pin.to_owned())),
        Some(SourcePin::NodeData(actual_id, actual_pin)) if actual_id == source_id && actual_pin == source_pin
    )
}

fn build_linked_root(
    interface: Handle<AnimationGraph>,
    implementation: Handle<AnimationGraph>,
) -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new(
        "AB_Als PistolOneHanded linked overlay",
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
    action: f32,
    rotation_mode: f32,
) {
    player.set_global_input_data(GAIT_WALKING, gait_walking);
    player.set_global_input_data(STANDING, standing);
    player.set_global_input_data(CROUCHING, crouching);
    player.set_global_input_data(LOCOMOTION_ACTION, action);
    player.set_global_input_data(ROTATION_MODE, rotation_mode);
    player.set_global_input_data(MOVING, 0.0);
    player.set_global_input_data(SPRINTING, 0.0);
    player.set_global_input_data(IN_AIR, 0.0);
    player.set_global_input_data(TRANSITIONS_ALLOWED, 0.0);
    player.set_global_input_data(PITCH_AMOUNT, 0.5);
}

fn has_time_position(graph: &AnimationGraph, name: &str, position: f32) -> bool {
    graph.nodes.values().any(|node| {
        node.name == name
            && node
                .try_inner_downcast_ref::<TimePositionNode>()
                .is_some_and(|node| close(node.duration_fallback, 0.3))
    }) && graph.nodes.values().any(|node| {
        node.name == name.replace("poses ", "").replace("evaluator", "position")
            && node
                .try_inner_downcast_ref::<ConstantF32Node>()
                .is_some_and(|node| close(node.value, position))
    })
}

fn additive_contract(graph: &AnimationGraph, name: &str, alpha: f32, base: &str) -> bool {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<BlendNode>())
        .is_some_and(|node| matches!(node.mode, BlendMode::Additive))
        && graph.nodes.values().any(|node| {
            node.name == "idle loop"
                && node
                    .try_inner_downcast_ref::<LoopNode>()
                    .is_some_and(|node| close(node.interpolation_period, 0.0))
        })
        && graph.nodes.values().any(|node| {
            node.name == "idle additive alpha"
                && node
                    .try_inner_downcast_ref::<ConstantF32Node>()
                    .is_some_and(|node| close(node.value, alpha))
        })
        && node_data_edge(graph, name, "pose_a", base, "pose")
        && node_data_edge(graph, name, "pose_b", "idle loop", "pose")
}

fn profile_contract(
    graph: &AnimationGraph,
    name: &str,
    blend_mode: AlphaBlendMode,
    custom_key_count: usize,
    bone_scale_count: usize,
) -> bool {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<ProfiledBlendNode>())
        .is_some_and(|node| {
            node.alpha_blend_mode == blend_mode
                && node.custom_curve.len() == custom_key_count
                && node.bone_scales.len() == bone_scale_count
        })
}

fn curve_key(node: &CurveSampleSetNode, name: &str, time: f32, value: f32) -> bool {
    node.curves
        .iter()
        .find(|curve| curve.name == name)
        .is_some_and(|curve| {
            curve
                .keys
                .iter()
                .any(|key| close(key.t, time) && close(key.v, value))
        })
}

fn global_keys(graph: &AnimationGraph, keys: &[&str]) -> bool {
    keys.iter().all(|key| {
        graph.nodes.values().any(|node| {
            node.try_inner_downcast_ref::<GlobalF32InputNode>()
                .is_some_and(|node| node.key == *key)
        })
    })
}

fn verify_static_contract(
    mut commands: Commands,
    assets: Res<AssetServer>,
    gate_assets: Res<GateAssets>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    fsms: Res<Assets<StateMachine>>,
    mut probe: ResMut<RuntimeProbe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.static_verified {
        return;
    }
    let all_graphs = [
        &gate_assets.interface,
        &gate_assets.pistol,
        &gate_assets.relaxed,
        &gate_assets.ready,
        &gate_assets.aiming,
        &gate_assets.aim_in,
        &gate_assets.aim_out,
        &gate_assets.relaxed_to_ready,
        &gate_assets.ready_to_relaxed_allowed,
        &gate_assets.ready_to_relaxed_moving,
        &gate_assets.ready_to_relaxed_interrupt,
    ];
    if !all_graphs
        .into_iter()
        .all(|handle| assets.is_loaded_with_dependencies(handle))
        || !assets.is_loaded_with_dependencies(&gate_assets.fsm)
        || !assets.is_loaded_with_dependencies(&gate_assets.skeleton)
    {
        return;
    }
    let Some(pistol) = graphs.get(&gate_assets.pistol) else {
        return;
    };
    let Some(fsm) = fsms.get(&gate_assets.fsm) else {
        return;
    };
    let Some(relaxed) = graphs.get(&gate_assets.relaxed) else {
        return;
    };
    let Some(ready) = graphs.get(&gate_assets.ready) else {
        return;
    };
    let Some(aiming) = graphs.get(&gate_assets.aiming) else {
        return;
    };
    let Some(aim_in) = graphs.get(&gate_assets.aim_in) else {
        return;
    };
    let Some(aim_out) = graphs.get(&gate_assets.aim_out) else {
        return;
    };
    let Some(relaxed_to_ready) = graphs.get(&gate_assets.relaxed_to_ready) else {
        return;
    };
    let Some(ready_to_relaxed_allowed) = graphs.get(&gate_assets.ready_to_relaxed_allowed) else {
        return;
    };
    let Some(ready_to_relaxed_moving) = graphs.get(&gate_assets.ready_to_relaxed_moving) else {
        return;
    };
    let Some(ready_to_relaxed_interrupt) = graphs.get(&gate_assets.ready_to_relaxed_interrupt)
    else {
        return;
    };

    let interface_contract = pistol
        .io_spec
        .get_input_data(&GraphInputPin::passthrough("base_pose"))
        == Some(DataSpec::Pose)
        && pistol
            .io_spec
            .has_input_time(&GraphInputPin::passthrough("base_time"))
        && pistol.io_spec.get_output_data(&"pose".to_owned()) == Some(DataSpec::Pose)
        && pistol.io_spec.has_output_time()
        && ALS_LAYER_CURVE_NAMES
            .iter()
            .all(|name| pistol.io_spec.get_output_data(&name.to_string()) == Some(DataSpec::F32))
        && ALS_LAYER_SIGNAL_NAMES
            .iter()
            .all(|name| pistol.io_spec.get_output_data(&name.to_string()) == Some(DataSpec::F32));
    let linked_fsm = pistol
        .nodes
        .values()
        .find(|node| node.name == "pistol states")
        .and_then(|node| node.try_inner_downcast_ref::<FsmNode>())
        .is_some_and(|node| node.fsm == gate_assets.fsm);
    let action_layer = pistol
        .nodes
        .values()
        .find(|node| node.name == "locomotion action layer blend")
        .and_then(|node| node.try_inner_downcast_ref::<GameplayTagLayerBlendNode>());
    let action_contract = action_layer.is_some_and(|node| {
        node.tags
            == [
                "Als.LocomotionAction.Mantling",
                "Als.LocomotionAction.GettingUp",
                "Als.LocomotionAction.Rolling",
            ]
            && node.blend_times == [0.3, 0.3, 0.0, 0.3]
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
    let action_curves = pistol
        .nodes
        .values()
        .find(|node| node.name == "pistol poses curve samples")
        .and_then(|node| node.try_inner_downcast_ref::<CurveSampleSetNode>())
        .is_some_and(|node| {
            node.sample_positions == [0.1, 0.23333334, 0.1]
                && curve_key(node, "LayerArmRight", 0.0, 1.0)
                && curve_key(node, "HipsDirectionLock", 0.13333334, -1.0)
                && curve_key(node, "LayerHandRight", 0.0, 1.0)
                && curve_key(node, "HandLeftIk", 0.0, 0.0)
                && curve_key(node, "HandRightIk", 0.0, 0.0)
                && curve_key(node, "LayerHandLeft", 0.0, 0.0)
        });
    let action_positions = has_time_position(pistol, "poses frame three evaluator", 0.1)
        && has_time_position(pistol, "poses frame seven evaluator", 0.23333334)
        && has_time_position(pistol, "poses frame three rolling evaluator", 0.1);
    let action_modifiers = [
        "getting up LayerArmRight curve modify",
        "rolling LayerArmRight curve modify",
    ]
    .into_iter()
    .all(|name| {
        pistol
            .nodes
            .values()
            .find(|node| node.name == name)
            .and_then(|node| node.try_inner_downcast_ref::<CurveModifyNode>())
            .is_some_and(|node| {
                node.curve_names == ["LayerArmRight"]
                    && node.values == [3.0]
                    && close(node.alpha, 1.0)
            })
    }) && !pistol.nodes.values().any(|node| {
        node.try_inner_downcast_ref::<CurveModifyNode>()
            .is_some_and(|node| node.curve_names.iter().any(|name| name == "LayerArmLeft"))
    });
    let mut timings = fsm
        .transitions
        .values()
        .filter_map(|transition| match transition.data.kind {
            TransitionKind::Graph {
                timed: Some(value), ..
            } => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    timings.sort_by(f32::total_cmp);
    let state_machine_contract = fsm.states.len() == 3
        && fsm.transitions.len() == 6
        && timings == [0.2, 0.2, 0.2, 0.75, 0.75, 1.0]
        && ["Relaxed", "Ready", "Aiming"]
            .into_iter()
            .all(|label| fsm.states.values().any(|state| state.label == label))
        && fsm.transitions.values().all(|transition| {
            !transition.data.ignore_external_events && transition.data.reset_target_state
        });
    let state_graph_contract =
        global_keys(relaxed, &[GAIT_WALKING, STANDING, CROUCHING, ROTATION_MODE])
            && global_keys(
                ready,
                &[
                    GAIT_WALKING,
                    STANDING,
                    CROUCHING,
                    ROTATION_MODE,
                    MOVING,
                    SPRINTING,
                    IN_AIR,
                    TRANSITIONS_ALLOWED,
                ],
            )
            && global_keys(
                aiming,
                &[
                    GAIT_WALKING,
                    STANDING,
                    CROUCHING,
                    ROTATION_MODE,
                    PITCH_AMOUNT,
                ],
            )
            && has_time_position(relaxed, "poses frame zero evaluator", 0.0)
            && has_time_position(relaxed, "poses frame one evaluator", 0.033333335)
            && has_time_position(relaxed, "poses frame six evaluator", 0.2)
            && has_time_position(ready, "poses frame two evaluator", 0.06666667)
            && has_time_position(ready, "poses frame three evaluator", 0.1)
            && has_time_position(ready, "poses frame seven evaluator", 0.23333334)
            && has_time_position(aiming, "poses frame four evaluator", 0.13333334)
            && has_time_position(aiming, "poses frame five evaluator", 0.16666667)
            && has_time_position(aiming, "poses frame eight evaluator", 0.26666667)
            && additive_contract(
                relaxed,
                "relaxed secondary motion additive",
                0.75,
                "relaxed stance blend",
            )
            && additive_contract(
                ready,
                "ready secondary motion additive",
                0.75,
                "ready stance blend",
            )
            && additive_contract(
                aiming,
                "aiming secondary motion additive",
                0.5,
                "aiming stance blend",
            )
            && [relaxed, ready, aiming].into_iter().all(|graph| {
                graph.nodes.values().any(|node| {
                    node.try_inner_downcast_ref::<CurveWeightedSamplesNode>()
                        .is_some()
                })
            })
            && aiming
                .nodes
                .values()
                .filter(|node| {
                    node.try_inner_downcast_ref::<MeshSpaceAdditiveNode>()
                        .is_some()
                })
                .count()
                == 2
            && [relaxed, ready, aiming].into_iter().all(|graph| {
                graph.nodes.values().any(|node| {
                    node.try_inner_downcast_ref::<SuppressFirstUpdateNode>()
                        .is_some()
                })
            });
    let transition_contract = profile_contract(
        aim_in,
        "aim in profiled blend",
        AlphaBlendMode::Custom,
        2,
        0,
    ) && profile_contract(
        aim_out,
        "aim out profiled blend",
        AlphaBlendMode::Custom,
        2,
        0,
    ) && profile_contract(
        relaxed_to_ready,
        "relaxed to ready profiled blend",
        AlphaBlendMode::HermiteCubic,
        0,
        0,
    ) && profile_contract(
        ready_to_relaxed_allowed,
        "ready to relaxed quick feet profiled blend",
        AlphaBlendMode::Cubic,
        0,
        18,
    ) && profile_contract(
        ready_to_relaxed_moving,
        "ready to relaxed moving profiled blend",
        AlphaBlendMode::Cubic,
        0,
        0,
    ) && profile_contract(
        ready_to_relaxed_interrupt,
        "ready to relaxed interrupt profiled blend",
        AlphaBlendMode::HermiteCubic,
        0,
        0,
    );
    info!(
        "[PISTOL-ONE-HANDED-CONTRACT] state_detail=inputs:{}:{}:{} frames={}{}{}{}{}{}{}{}{} additive={}{}{} curves={} mesh={} suppress={}",
        global_keys(relaxed, &[GAIT_WALKING, STANDING, CROUCHING, ROTATION_MODE]),
        global_keys(
            ready,
            &[
                GAIT_WALKING,
                STANDING,
                CROUCHING,
                ROTATION_MODE,
                MOVING,
                SPRINTING,
                IN_AIR,
                TRANSITIONS_ALLOWED
            ]
        ),
        global_keys(
            aiming,
            &[
                GAIT_WALKING,
                STANDING,
                CROUCHING,
                ROTATION_MODE,
                PITCH_AMOUNT
            ]
        ),
        has_time_position(relaxed, "poses frame zero evaluator", 0.0),
        has_time_position(relaxed, "poses frame one evaluator", 0.033333335),
        has_time_position(relaxed, "poses frame six evaluator", 0.2),
        has_time_position(ready, "poses frame two evaluator", 0.06666667),
        has_time_position(ready, "poses frame three evaluator", 0.1),
        has_time_position(ready, "poses frame seven evaluator", 0.23333334),
        has_time_position(aiming, "poses frame four evaluator", 0.13333334),
        has_time_position(aiming, "poses frame five evaluator", 0.16666667),
        has_time_position(aiming, "poses frame eight evaluator", 0.26666667),
        additive_contract(
            relaxed,
            "relaxed secondary motion additive",
            0.75,
            "relaxed stance blend"
        ),
        additive_contract(
            ready,
            "ready secondary motion additive",
            0.75,
            "ready stance blend"
        ),
        additive_contract(
            aiming,
            "aiming secondary motion additive",
            0.5,
            "aiming stance blend"
        ),
        [relaxed, ready, aiming]
            .into_iter()
            .all(|graph| graph.nodes.values().any(|node| node
                .try_inner_downcast_ref::<CurveWeightedSamplesNode>()
                .is_some())),
        aiming
            .nodes
            .values()
            .filter(|node| node
                .try_inner_downcast_ref::<MeshSpaceAdditiveNode>()
                .is_some())
            .count()
            == 2,
        [relaxed, ready, aiming]
            .into_iter()
            .all(|graph| graph.nodes.values().any(|node| node
                .try_inner_downcast_ref::<SuppressFirstUpdateNode>()
                .is_some())),
    );
    let source_contract = [
        PISTOL_PATH,
        FSM_PATH,
        "graphs/ab_als/overlays/pistol_one_handed/state_relaxed.animgraph.ron",
        "graphs/ab_als/overlays/pistol_one_handed/state_ready.animgraph.ron",
        "graphs/ab_als/overlays/pistol_one_handed/state_aiming.animgraph.ron",
    ]
    .into_iter()
    .map(|path| std::fs::read_to_string(format!("assets/{path}")))
    .collect::<Result<Vec<_>, _>>()
    .is_ok_and(|sources| {
        sources
            .iter()
            .all(|source| !source.contains("AB_Als_Monolithic"))
            && sources[1].contains("timed: Some(0.2)")
            && sources[2].contains("000000000029")
            && sources[3].contains("FsmBuiltin(TimeElapsed)")
            && sources[3].contains("value: 3.0")
            && sources[4].contains("A_Als_PistolOneHanded_Aim_Crouch")
    });
    let numeric_contract = aim_in
        .nodes
        .values()
        .find(|node| node.name == "aim in profiled blend")
        .and_then(|node| node.try_inner_downcast_ref::<ProfiledBlendNode>())
        .is_some_and(|node| {
            close(
                transition_alpha(0.5, node.alpha_blend_mode, &node.custom_curve),
                0.8773595,
            )
        });
    let pass = interface_contract
        && linked_fsm
        && action_contract
        && action_curves
        && action_positions
        && action_modifiers
        && state_machine_contract
        && state_graph_contract
        && transition_contract
        && source_contract
        && numeric_contract;
    info!(
        "[PISTOL-ONE-HANDED-CONTRACT] interface={} linked_fsm={} action={} action_curves={} action_positions={} action_modifiers={} state_machine={} states={} transitions={} source={} numeric={}",
        interface_contract,
        linked_fsm,
        action_contract,
        action_curves,
        action_positions,
        action_modifiers,
        state_machine_contract,
        state_graph_contract,
        transition_contract,
        source_contract,
        numeric_contract
    );
    if !pass {
        warn!("[PISTOL-ONE-HANDED-CONTRACT][VERDICT] static=false -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
        return;
    }
    let root = graphs.add(build_linked_root(
        gate_assets.interface.clone(),
        gate_assets.pistol.clone(),
    ));
    let mut player = AnimationGraphPlayer::new(gate_assets.skeleton.clone()).with_graph(root);
    player.set_input_data("base_pose", DataValue::Pose(Pose::default()));
    player.set_input_data("layer_index", DataValue::F32(0.0));
    set_inputs(&mut player, 0.0, 1.0, 0.0, 1.0, 0.0);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.4));
    let player = commands
        .spawn((
            Name::new("pistol_one_handed_contract_probe"),
            Children::default(),
            Transform::IDENTITY,
            player,
        ))
        .id();
    probe.player = Some(player);
    probe.phase = ProbePhase::Mantling;
    probe.static_verified = true;
    info!("[PISTOL-ONE-HANDED-CONTRACT] linked root runtime probe initialized");
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!("[PISTOL-ONE-HANDED-CONTRACT][VERDICT] {message} -> FAIL");
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
    match probe.phase {
        ProbePhase::Mantling => {
            if !output_f32(&player, "LayerArmRight").is_some_and(|value| close(value, 1.0)) {
                fail(&mut exit, 5, "Mantling LayerArmRight source curve");
                return;
            }
            info!("[PISTOL-ONE-HANDED-CONTRACT] action=Mantling LayerArmRight=1.0");
            set_inputs(&mut player, 0.0, 1.0, 0.0, 2.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.4));
            probe.phase = ProbePhase::GettingUp;
        }
        ProbePhase::GettingUp => {
            if !output_f32(&player, "LayerArmRight").is_some_and(|value| close(value, 3.0)) {
                fail(&mut exit, 6, "GettingUp LayerArmRight curve");
                return;
            }
            info!("[PISTOL-ONE-HANDED-CONTRACT] action=GettingUp LayerArmRight=3.0");
            set_inputs(&mut player, 0.0, 1.0, 0.0, 3.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.4));
            probe.phase = ProbePhase::Rolling;
        }
        ProbePhase::Rolling => {
            if !output_f32(&player, "LayerArmRight").is_some_and(|value| close(value, 3.0)) {
                fail(&mut exit, 7, "Rolling LayerArmRight curve");
                return;
            }
            info!("[PISTOL-ONE-HANDED-CONTRACT] action=Rolling LayerArmRight=3.0");
            set_inputs(&mut player, 0.0, 1.0, 0.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.4));
            probe.phase = ProbePhase::Relaxed;
        }
        ProbePhase::Relaxed => {
            let Some(pose) = player.get_default_output_pose().cloned() else {
                return;
            };
            probe.relaxed = Some(pose);
            set_inputs(&mut player, 1.0, 1.0, 0.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.1));
            probe.phase = ProbePhase::Walking;
        }
        ProbePhase::Walking => {
            let Some(pose) = player.get_default_output_pose().cloned() else {
                return;
            };
            if !probe.relaxed.as_ref().is_some_and(|rest| rest != &pose) {
                fail(&mut exit, 8, "Relaxed walking pose did not change");
                return;
            }
            probe.walking = Some(pose);
            set_inputs(&mut player, 0.0, 0.0, 1.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.1));
            probe.phase = ProbePhase::Crouching;
        }
        ProbePhase::Crouching => {
            let Some(pose) = player.get_default_output_pose().cloned() else {
                return;
            };
            if !probe
                .walking
                .as_ref()
                .is_some_and(|walking| walking != &pose)
            {
                fail(&mut exit, 9, "Crouching pose did not change");
                return;
            }
            info!("[PISTOL-ONE-HANDED-CONTRACT] relaxed walk and crouch pose probes changed");
            set_inputs(&mut player, 0.0, 1.0, 0.0, 0.0, 1.0);
            player.queue_time_update(TimeUpdate::Delta(0.05));
            probe.frames = 0;
            probe.phase = ProbePhase::BeginAim;
        }
        ProbePhase::BeginAim => {
            player.queue_time_update(TimeUpdate::Delta(0.1));
            probe.frames = 0;
            probe.phase = ProbePhase::Aiming;
        }
        ProbePhase::Aiming => {
            if output_f32(&player, "PoseAiming").is_some_and(|value| value > 0.99) {
                info!("[PISTOL-ONE-HANDED-CONTRACT] fsm=Relaxed->Ready->Aiming PoseAiming=1.0");
                set_inputs(&mut player, 0.0, 1.0, 0.0, 0.0, 0.0);
                player.queue_time_update(TimeUpdate::Delta(0.25));
                probe.frames = 0;
                probe.phase = ProbePhase::BeginAimOut;
                return;
            }
            probe.frames += 1;
            if probe.frames > 16 {
                fail(&mut exit, 10, "FSM did not reach Aiming");
                return;
            }
            player.queue_time_update(TimeUpdate::Delta(0.1));
        }
        ProbePhase::BeginAimOut => {
            player.queue_time_update(TimeUpdate::Delta(0.25));
            probe.phase = ProbePhase::Ready;
        }
        ProbePhase::Ready => {
            if output_f32(&player, "PoseAiming").is_some_and(|value| value < 0.01) {
                info!("[PISTOL-ONE-HANDED-CONTRACT] fsm=Aiming->Ready PoseAiming=0.0");
                info!("[PISTOL-ONE-HANDED-CONTRACT][VERDICT] linked=true -> PASS");
                exit.write(AppExit::Success);
                return;
            }
            probe.frames += 1;
            if probe.frames > 16 {
                fail(&mut exit, 11, "FSM did not leave Aiming");
                return;
            }
            player.queue_time_update(TimeUpdate::Delta(0.25));
        }
        ProbePhase::Pending => {}
    }
}

fn auto_quit(time: Res<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= time.0 {
        warn!("[PISTOL-ONE-HANDED-CONTRACT][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(12).unwrap()));
    }
}
