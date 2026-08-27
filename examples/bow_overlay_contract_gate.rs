use std::{
    path::Path,
    time::{Duration, Instant},
};

use als_ext::{
    ALS_LAYER_CURVE_NAMES, ALS_LAYER_SIGNAL_NAMES, AlphaBlendMode, AlsExtPlugin, ConstantF32Node, CurveModifyNode,
    CurveSampleSetNode, CurveWeightedSamplesNode, GameplayTagBlendMode, GameplayTagLayerBlendNode,
    LinkedAnimLayerNode, ProfiledBlendNode, SuppressFirstUpdateNode,
    TimePositionNode, profile_target_weight, transition_alpha,
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
const BOW_GRAPH_PATH: &str = "graphs/ab_als/overlays/bow.animgraph.ron";
const BOW_FSM_PATH: &str = "graphs/ab_als/overlays/bow/bow_states.fsm.ron";
const BOW_OBJECT_GRAPH_PATH: &str = "graphs/overlay_objects/bow.animgraph.ron";
const CHARACTER_SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const BOW_SKELETON_PATH: &str = "skeletons/SKM_Als_Bow.skn.ron";
const GAIT_WALKING: &str = "als_overlay_gait_walking_amount";
const GAIT_RUNNING: &str = "als_overlay_gait_running_amount";
const GAIT_SPRINTING: &str = "als_overlay_gait_sprinting_amount";
const STANDING: &str = "als_overlay_standing_amount";
const CROUCHING: &str = "als_overlay_crouching_amount";
const ROTATION_MODE: &str = "als_overlay_rotation_mode_index";
const LOCOMOTION_MODE: &str = "als_overlay_locomotion_mode_index";
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
    bow: Handle<AnimationGraph>,
    fsm: Handle<StateMachine>,
    object: Handle<AnimationGraph>,
    character_skeleton: Handle<Skeleton>,
    bow_skeleton: Handle<Skeleton>,
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
    character_player: Option<Entity>,
    object_player: Option<Entity>,
    phase: ProbePhase,
    object_rest: Option<Pose>,
    static_verified: bool,
    fsm_frames: u8,
    saw_transition_left_request: bool,
}

#[derive(Default, PartialEq, Eq)]
enum ProbePhase {
    #[default]
    Pending,
    ObjectRest,
    ObjectDraw,
    Mantling,
    GettingUp,
    Rolling,
    BeginAim,
    ObserveAimTransition,
    Aim,
    BeginAimOut,
    AimOut,
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        interface: assets.load(INTERFACE_PATH),
        bow: assets.load(BOW_GRAPH_PATH),
        fsm: assets.load(BOW_FSM_PATH),
        object: assets.load(BOW_OBJECT_GRAPH_PATH),
        character_skeleton: assets.load(CHARACTER_SKELETON_PATH),
        bow_skeleton: assets.load(BOW_SKELETON_PATH),
        relaxed: assets.load("graphs/ab_als/overlays/bow/state_relaxed.animgraph.ron"),
        ready: assets.load("graphs/ab_als/overlays/bow/state_ready.animgraph.ron"),
        aiming: assets.load("graphs/ab_als/overlays/bow/state_aiming.animgraph.ron"),
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
    info!("[BOW-OVERLAY-CONTRACT] loading character layer and overlay object graphs");
}

fn close(value: f32, expected: f32) -> bool {
    (value - expected).abs() < 1e-4
}

fn graph_has_node(graph: &AnimationGraph, name: &str) -> bool {
    graph.nodes.values().any(|node| node.name == name)
}

fn output_f32(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_outputs().get(name) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
}

fn build_linked_root(
    interface: Handle<AnimationGraph>,
    implementation: Handle<AnimationGraph>,
) -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new(
        "AB_Als Bow linked overlay",
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

fn set_character_inputs(
    player: &mut AnimationGraphPlayer,
    locomotion_action: f32,
    rotation_mode: f32,
) {
    player.set_global_input_data(GAIT_WALKING, 0.0);
    player.set_global_input_data(GAIT_RUNNING, 0.0);
    player.set_global_input_data(GAIT_SPRINTING, 0.0);
    player.set_global_input_data(STANDING, 1.0);
    player.set_global_input_data(CROUCHING, 0.0);
    player.set_global_input_data(ROTATION_MODE, rotation_mode);
    player.set_global_input_data(LOCOMOTION_MODE, 0.0);
    player.set_global_input_data(LOCOMOTION_ACTION, locomotion_action);
    player.set_global_input_data(MOVING, 0.0);
    player.set_global_input_data(SPRINTING, 0.0);
    player.set_global_input_data(IN_AIR, 0.0);
    player.set_global_input_data(TRANSITIONS_ALLOWED, 0.0);
    player.set_global_input_data(PITCH_AMOUNT, 0.0);
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

fn idle_additive_contract(
    graph: &AnimationGraph,
    name: &str,
    alpha: f32,
    base_name: &str,
) -> bool {
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
                    .is_some_and(|node| node.interpolation_period.abs() < 1e-6)
        })
        && graph.nodes.values().any(|node| {
            node.name == "idle additive alpha"
                && node
                    .try_inner_downcast_ref::<ConstantF32Node>()
                    .is_some_and(|node| close(node.value, alpha))
        })
        && node_data_edge(graph, name, "pose_a", base_name, "pose")
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
    let graphs_loaded = [
        &gate_assets.interface,
        &gate_assets.bow,
        &gate_assets.object,
        &gate_assets.relaxed,
        &gate_assets.ready,
        &gate_assets.aiming,
        &gate_assets.aim_in,
        &gate_assets.aim_out,
        &gate_assets.relaxed_to_ready,
        &gate_assets.ready_to_relaxed_allowed,
        &gate_assets.ready_to_relaxed_moving,
        &gate_assets.ready_to_relaxed_interrupt,
    ]
    .into_iter()
    .all(|handle| assets.is_loaded_with_dependencies(handle));
    if !graphs_loaded
        || !assets.is_loaded_with_dependencies(&gate_assets.fsm)
        || !assets.is_loaded_with_dependencies(&gate_assets.character_skeleton)
        || !assets.is_loaded_with_dependencies(&gate_assets.bow_skeleton)
    {
        return;
    }
    let Some(bow) = graphs.get(&gate_assets.bow) else {
        return;
    };
    let Some(object) = graphs.get(&gate_assets.object) else {
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

    let pose_contract = bow
        .io_spec
        .get_input_data(&GraphInputPin::passthrough("base_pose"))
        == Some(DataSpec::Pose)
        && bow
            .io_spec
            .has_input_time(&GraphInputPin::passthrough("base_time"))
        && bow.io_spec.get_output_data(&"pose".to_owned()) == Some(DataSpec::Pose)
        && bow.io_spec.has_output_time()
        && bow.default_data.is_empty();
    let curve_contract = ALS_LAYER_CURVE_NAMES
        .iter()
        .all(|curve| bow.io_spec.get_output_data(&curve.to_string()) == Some(DataSpec::F32));
    let signal_contract = ALS_LAYER_SIGNAL_NAMES
        .iter()
        .all(|signal| bow.io_spec.get_output_data(&signal.to_string()) == Some(DataSpec::F32));
    let linked_fsm = bow
        .nodes
        .values()
        .find(|node| node.name == "bow states")
        .and_then(|node| node.try_inner_downcast_ref::<FsmNode>())
        .is_some_and(|node| node.fsm == gate_assets.fsm);
    let action_layer = bow
        .nodes
        .values()
        .find(|node| node.name == "locomotion action layer blend")
        .and_then(|node| node.try_inner_downcast_ref::<GameplayTagLayerBlendNode>());
    let action_blend = action_layer.is_some_and(|node| {
        node.tags
            == [
                "Als.LocomotionAction.Mantling",
                "Als.LocomotionAction.GettingUp",
                "Als.LocomotionAction.Rolling",
            ]
            && node.blend_times == [0.1, 0.1, 0.1, 0.1]
            && node.blend_mode == GameplayTagBlendMode::Linear
    });
    let action_poses = action_layer.is_some_and(|node| node.normalize_alpha);
    let action_curves = bow
        .nodes
        .values()
        .find(|node| node.name == "bow poses curve samples")
        .and_then(|node| node.try_inner_downcast_ref::<CurveSampleSetNode>())
        .is_some_and(|node| {
            node.sample_positions == [0.1, 0.2666667, 0.0]
                && node.curves.iter().any(|curve| curve.name == "PoseAiming")
                && node.curves.iter().any(|curve| curve.name == "HandRightIk")
        }) && action_layer.is_some_and(|node| {
            node.curve_names
                .iter()
                .map(String::as_str)
                .eq(ALS_LAYER_CURVE_NAMES.iter().copied())
                && node
                    .signal_names
                    .iter()
                    .map(String::as_str)
                    .eq(ALS_LAYER_SIGNAL_NAMES.iter().copied())
                && !node.normalize_curve_weights
        });
    let action_modifiers = [
        (
            "mantling LayerArmLeftAdditive curve modify",
            "LayerArmLeftAdditive",
            0.5,
        ),
        ("getting up LayerArmLeft curve modify", "LayerArmLeft", 3.0),
        ("rolling LayerArmLeft curve modify", "LayerArmLeft", 3.0),
    ]
    .into_iter()
    .all(|(name, curve, value)| {
        bow.nodes
            .values()
            .find(|node| node.name == name)
            .and_then(|node| node.try_inner_downcast_ref::<CurveModifyNode>())
            .is_some_and(|node| {
                node.curve_names == [curve] && node.values == [value] && close(node.alpha, 1.0)
            })
    });
    let state_contract = fsm.states.len() == 3
        && ["Relaxed", "Ready", "Aiming"]
            .into_iter()
            .all(|label| fsm.states.values().any(|state| state.label == label));
    let mut timings = fsm
        .transitions
        .values()
        .filter_map(|transition| match transition.data.kind {
            TransitionKind::Graph {
                timed: Some(timed), ..
            } => Some(timed),
            _ => None,
        })
        .collect::<Vec<_>>();
    timings.sort_by(f32::total_cmp);
    let transition_contract = fsm.transitions.len() == 6
        && timings == [0.2, 0.4, 0.5, 0.75, 0.75, 1.0]
        && fsm.transitions.values().all(|transition| {
            !transition.data.ignore_external_events && transition.data.reset_target_state
        });
    let state_graph_contract =
        idle_additive_contract(
            relaxed,
            "relaxed secondary motion additive",
            0.75,
            "relaxed stance blend",
        ) && idle_additive_contract(
            ready,
            "ready secondary motion additive",
            0.75,
            "ready stance blend",
        ) && idle_additive_contract(
            aiming,
            "aiming secondary motion additive",
            0.5,
            "aiming stance blend",
        )
            && relaxed.nodes.values().any(|node| {
                node.try_inner_downcast_ref::<CurveWeightedSamplesNode>()
                    .is_some()
            })
            && ready.nodes.values().any(|node| {
                node.try_inner_downcast_ref::<SuppressFirstUpdateNode>()
                    .is_some()
            })
            && aiming.nodes.values().any(|node| {
                node.try_inner_downcast_ref::<SuppressFirstUpdateNode>()
                    .is_some()
            });
    let transition_graph_contract = profile_contract(
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
    let object_contract = object
        .io_spec
        .get_input_data(&GraphInputPin::passthrough("pose_aiming"))
        == Some(DataSpec::F32)
        && object.io_spec.has_output_time()
        && object.io_spec.get_output_data(&"pose".to_owned()) == Some(DataSpec::Pose)
        && object
            .nodes
            .values()
            .find(|node| node.name == "bow draw evaluator")
            .and_then(|node| node.try_inner_downcast_ref::<TimePositionNode>())
            .is_some_and(|node| close(node.duration_fallback, 1.0))
        && graph_has_node(object, "bow draw clip");
    let sources = [
        BOW_GRAPH_PATH,
        BOW_OBJECT_GRAPH_PATH,
        "animations/Overlays/Bow/A_Als_Bow_Draw.anim.ron",
        BOW_SKELETON_PATH,
    ]
    .into_iter()
    .filter_map(|path| std::fs::read_to_string(Path::new("assets").join(path)).ok())
    .collect::<Vec<_>>();
    let asset_contract = sources.len() == 4
        && sources
            .iter()
            .all(|source| !source.contains("AB_Als_Monolithic"))
        && sources
            .iter()
            .any(|source| source.contains("SKM_Als_Bow.skn.ron"))
        && sources
            .iter()
            .any(|source| source.contains("A_Als_Bow_Draw.gltf"));
    let aim_in_value = aim_in
        .nodes
        .values()
        .find(|node| node.name == "aim in profiled blend")
        .and_then(|node| node.try_inner_downcast_ref::<ProfiledBlendNode>())
        .map(|node| transition_alpha(0.5, node.alpha_blend_mode, &node.custom_curve));
    let profile_weight = profile_target_weight(0.1, 5.0);
    let numeric_contract = aim_in_value.is_some_and(|value| close(value, 0.8773595))
        && close(profile_weight, 0.7352941);
    let pass = pose_contract
        && curve_contract
        && signal_contract
        && linked_fsm
        && action_blend
        && action_poses
        && action_curves
        && action_modifiers
        && state_contract
        && transition_contract
        && state_graph_contract
        && transition_graph_contract
        && object_contract
        && asset_contract
        && numeric_contract;
    info!(
        "[BOW-OVERLAY-CONTRACT] pose_contract={} curve_contract={} signal_contract={} linked_fsm={} action_blend={} action_poses={} action_curves={} action_modifiers={} state_contract={} transition_contract={} state_graph_contract={} transition_graph_contract={} object_contract={} asset_contract={} numeric_contract={} aim_in_half={:?} quick_feet_weight={}",
        pose_contract,
        curve_contract,
        signal_contract,
        linked_fsm,
        action_blend,
        action_poses,
        action_curves,
        action_modifiers,
        state_contract,
        transition_contract,
        state_graph_contract,
        transition_graph_contract,
        object_contract,
        asset_contract,
        numeric_contract,
        aim_in_value,
        profile_weight,
    );
    if !pass {
        warn!("[BOW-OVERLAY-CONTRACT][VERDICT] static=false -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
        return;
    }

    let character_root = graphs.add(build_linked_root(
        gate_assets.interface.clone(),
        gate_assets.bow.clone(),
    ));
    let mut character_player = AnimationGraphPlayer::new(gate_assets.character_skeleton.clone())
        .with_graph(character_root);
    character_player.set_input_data("base_pose", DataValue::Pose(Pose::default()));
    character_player.set_input_data("layer_index", DataValue::F32(0.0));
    set_character_inputs(&mut character_player, 0.0, 0.0);
    character_player.pause();
    character_player.queue_time_update(TimeUpdate::Delta(0.0));
    let character_player = commands
        .spawn((
            Name::new("bow_overlay_contract_character_probe"),
            Children::default(),
            Transform::IDENTITY,
            character_player,
        ))
        .id();

    let mut object_player = AnimationGraphPlayer::new(gate_assets.bow_skeleton.clone())
        .with_graph(gate_assets.object.clone());
    object_player.set_input_data("pose_aiming", DataValue::F32(0.0));
    object_player.pause();
    object_player.queue_time_update(TimeUpdate::Delta(0.0));
    let object_player = commands
        .spawn((
            Name::new("bow_overlay_contract_object_probe"),
            Children::default(),
            Transform::IDENTITY,
            object_player,
        ))
        .id();

    probe.character_player = Some(character_player);
    probe.object_player = Some(object_player);
    probe.phase = ProbePhase::ObjectRest;
    probe.static_verified = true;
    info!("[BOW-OVERLAY-CONTRACT] linked character and object runtime probes initialized");
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!("[BOW-OVERLAY-CONTRACT][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(code).unwrap()));
}

fn object_pose(players: &mut Query<&mut AnimationGraphPlayer>, entity: Entity) -> Option<Pose> {
    players
        .get_mut(entity)
        .ok()?
        .get_default_output_pose()
        .cloned()
}

fn observe_runtime(
    mut probe: ResMut<RuntimeProbe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    if !probe.static_verified {
        return;
    }
    let (Some(character_entity), Some(object_entity)) =
        (probe.character_player, probe.object_player)
    else {
        return;
    };
    for entity in [character_entity, object_entity] {
        if let Ok(player) = players.get_mut(entity) {
            if let Some(error) = player.get_error() {
                fail(&mut exit, 4, &format!("runtime error={error:?}"));
                return;
            }
        }
    }
    match probe.phase {
        ProbePhase::ObjectRest => {
            let Some(pose) = object_pose(&mut players, object_entity) else {
                return;
            };
            probe.object_rest = Some(pose);
            let Ok(mut object) = players.get_mut(object_entity) else {
                return;
            };
            object.set_input_data("pose_aiming", DataValue::F32(1.0));
            object.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::ObjectDraw;
        }
        ProbePhase::ObjectDraw => {
            let Some(pose) = object_pose(&mut players, object_entity) else {
                return;
            };
            if !probe.object_rest.as_ref().is_some_and(|rest| rest != &pose) {
                fail(&mut exit, 5, "Bow Draw explicit-time pose did not change");
                return;
            }
            info!(
                "[BOW-OVERLAY-CONTRACT] object DrawAmount=1.0 pose_changed=true bones={}",
                pose.bones.len()
            );
            let Ok(mut character) = players.get_mut(character_entity) else {
                return;
            };
            set_character_inputs(&mut character, 1.0, 0.0);
            character.queue_time_update(TimeUpdate::Delta(0.1));
            probe.phase = ProbePhase::Mantling;
        }
        ProbePhase::Mantling => {
            let Ok(mut character) = players.get_mut(character_entity) else {
                return;
            };
            if !output_f32(&character, "LayerArmLeftAdditive")
                .is_some_and(|value| close(value, 0.5))
            {
                fail(&mut exit, 6, "Mantling LayerArmLeftAdditive curve");
                return;
            }
            info!("[BOW-OVERLAY-CONTRACT] action=Mantling LayerArmLeftAdditive=0.5");
            set_character_inputs(&mut character, 2.0, 0.0);
            character.queue_time_update(TimeUpdate::Delta(0.1));
            probe.phase = ProbePhase::GettingUp;
        }
        ProbePhase::GettingUp => {
            let Ok(mut character) = players.get_mut(character_entity) else {
                return;
            };
            if !output_f32(&character, "LayerArmLeft").is_some_and(|value| close(value, 3.0)) {
                fail(&mut exit, 7, "GettingUp LayerArmLeft curve");
                return;
            }
            info!("[BOW-OVERLAY-CONTRACT] action=GettingUp LayerArmLeft=3.0");
            set_character_inputs(&mut character, 3.0, 0.0);
            character.queue_time_update(TimeUpdate::Delta(0.1));
            probe.phase = ProbePhase::Rolling;
        }
        ProbePhase::Rolling => {
            let Ok(mut character) = players.get_mut(character_entity) else {
                return;
            };
            if !output_f32(&character, "LayerArmLeft").is_some_and(|value| close(value, 3.0)) {
                fail(&mut exit, 8, "Rolling LayerArmLeft curve");
                return;
            }
            info!("[BOW-OVERLAY-CONTRACT] action=Rolling LayerArmLeft=3.0");
            set_character_inputs(&mut character, 0.0, 1.0);
            character.queue_time_update(TimeUpdate::Delta(0.01));
            probe.fsm_frames = 0;
            probe.phase = ProbePhase::BeginAim;
        }
        ProbePhase::BeginAim => {
            let Ok(mut character) = players.get_mut(character_entity) else {
                return;
            };
            character.queue_time_update(TimeUpdate::Delta(0.0));
            probe.fsm_frames = 0;
            probe.phase = ProbePhase::ObserveAimTransition;
        }
        ProbePhase::ObserveAimTransition => {
            let Ok(mut character) = players.get_mut(character_entity) else {
                return;
            };
            if output_f32(&character, "transition_left_request").is_some_and(|value| value > 0.99)
            {
                probe.saw_transition_left_request = true;
                info!("[BOW-OVERLAY-CONTRACT] transition_left_request=1.0");
            }
            if probe.saw_transition_left_request {
                character.queue_time_update(TimeUpdate::Delta(0.25));
                probe.fsm_frames = 0;
                probe.phase = ProbePhase::Aim;
                return;
            }
            probe.fsm_frames += 1;
            if probe.fsm_frames > 4 {
                fail(&mut exit, 10, "Relaxed to Ready transition left request was not observed");
                return;
            }
            character.queue_time_update(TimeUpdate::Delta(0.01));
        }
        ProbePhase::Aim => {
            let Ok(mut character) = players.get_mut(character_entity) else {
                return;
            };
            let pose_aiming = output_f32(&character, "PoseAiming");
            if pose_aiming.is_some_and(|value| value > 0.99) {
                info!(
                    "[BOW-OVERLAY-CONTRACT] fsm=Relaxed->Ready->Aiming PoseAiming={:?}",
                    pose_aiming
                );
                set_character_inputs(&mut character, 0.0, 0.0);
                character.queue_time_update(TimeUpdate::Delta(0.25));
                probe.fsm_frames = 0;
                probe.phase = ProbePhase::BeginAimOut;
                return;
            }
            probe.fsm_frames += 1;
            if probe.fsm_frames > 8 {
                fail(&mut exit, 9, "FSM did not reach Aiming PoseAiming=1.0");
                return;
            }
            character.queue_time_update(TimeUpdate::Delta(0.25));
        }
        ProbePhase::BeginAimOut => {
            let Ok(mut character) = players.get_mut(character_entity) else {
                return;
            };
            character.queue_time_update(TimeUpdate::Delta(0.25));
            probe.phase = ProbePhase::AimOut;
        }
        ProbePhase::AimOut => {
            let Ok(mut character) = players.get_mut(character_entity) else {
                return;
            };
            let pose_aiming = output_f32(&character, "PoseAiming");
            if pose_aiming.is_some_and(|value| value < 0.01) {
                info!(
                    "[BOW-OVERLAY-CONTRACT] fsm=Aiming->Ready PoseAiming={:?}",
                    pose_aiming
                );
                info!("[BOW-OVERLAY-CONTRACT][VERDICT] linked=true object_dependency=true -> PASS");
                exit.write(AppExit::Success);
                return;
            }
            probe.fsm_frames += 1;
            if probe.fsm_frames > 8 {
                fail(&mut exit, 10, "FSM did not leave Aiming");
                return;
            }
            character.queue_time_update(TimeUpdate::Delta(0.25));
        }
        ProbePhase::Pending => {}
    }
}

fn auto_quit(time: Res<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= time.0 {
        warn!("[BOW-OVERLAY-CONTRACT][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(11).unwrap()));
    }
}
