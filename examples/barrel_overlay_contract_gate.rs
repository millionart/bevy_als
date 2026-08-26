use std::{
    path::Path,
    time::{Duration, Instant},
};

use als_ext::{
    ALS_LAYER_CURVE_NAMES, AlsExtPlugin, ConstantF32Node, CurveBlendNode, CurveModifyNode,
    CurveSampleSetNode, GameplayTagBlendNode, LinkedAnimLayerNode, MultiWayBlendNode,
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
const GRAPH_PATH: &str = "graphs/ab_als/overlays/barrel.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const GAIT_WALKING: &str = "als_overlay_gait_walking_amount";
const STANDING: &str = "als_overlay_standing_amount";
const CROUCHING: &str = "als_overlay_crouching_amount";
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
    .add_systems(Startup, load_graph)
    .add_systems(Update, (verify_static_contract, auto_quit))
    .add_systems(PostUpdate, observe_runtime.after(AnimationGraphSet::Final));
    app.run();
}

#[derive(Resource)]
struct GateAssets {
    interface: Handle<AnimationGraph>,
    graph: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource)]
struct AutoQuit(Instant);

#[derive(Resource, Default)]
struct RuntimeProbe {
    player: Option<Entity>,
    phase: ProbePhase,
    grounded_idle: Option<Pose>,
    grounded_walk: Option<Pose>,
    crouching: Option<Pose>,
    static_verified: bool,
}

#[derive(Default, PartialEq, Eq)]
enum ProbePhase {
    #[default]
    Pending,
    GroundedIdle,
    GroundedWalk,
    Crouching,
    MixedStance,
    GettingUp,
    ReturnHalf,
    ReturnDefault,
    Rolling,
}

fn load_graph(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        interface: assets.load(INTERFACE_PATH),
        graph: assets.load(GRAPH_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    commands.insert_resource(AutoQuit(Instant::now() + Duration::from_secs(20)));
    info!("[BARREL-OVERLAY-CONTRACT] loading graph=AB_Als_Barrel");
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

fn close(value: f32, expected: f32) -> bool {
    (value - expected).abs() < 1e-4
}

fn all_source_curve_values_match(
    player: &AnimationGraphPlayer,
    hand_right: f32,
    arm_right: f32,
) -> bool {
    let exact = [
        ("LayerArmLeftAdditive", 0.0),
        ("LayerArmLeftLocalSpace", 1.0),
        ("LayerArmRight", 1.0),
        ("LayerArmRightLocalSpace", 1.0),
        ("LayerHead", 1.0),
        ("LayerHeadAdditive", 0.25),
        ("LayerLegs", 1.0),
        ("LayerPelvis", 1.0),
        ("LayerSpine", 0.75),
        ("LayerSpineAdditive", 1.0),
        ("LayerHandLeft", 1.0),
        ("HandLeftIk", 0.0),
        ("HipsDirectionLock", 0.0),
        ("LayerHandRight", 0.0),
        ("PoseAiming", 0.0),
    ];
    exact.into_iter().all(|(name, expected)| {
        output_f32(player, name).is_some_and(|value| close(value, expected))
    }) && output_f32(player, "HandRightIk").is_some_and(|value| close(value, hand_right))
        && output_f32(player, "LayerArmRightAdditive").is_some_and(|value| close(value, arm_right))
}

fn set_overlay_inputs(
    player: &mut AnimationGraphPlayer,
    gait_walking: f32,
    standing: f32,
    crouching: f32,
    locomotion_action: f32,
) {
    player.set_global_input_data(GAIT_WALKING, gait_walking);
    player.set_global_input_data(STANDING, standing);
    player.set_global_input_data(CROUCHING, crouching);
    player.set_global_input_data(LOCOMOTION_ACTION, locomotion_action);
}

fn build_linked_root(
    interface: Handle<AnimationGraph>,
    barrel: Handle<AnimationGraph>,
) -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new(
        "AB_Als Barrel linked overlay",
        LinkedAnimLayerNode {
            interface,
            implementations: vec![barrel],
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
    graph.add_output_time();
    graph.add_input_data_edge(GraphInputPin::passthrough("base_pose"), id, "base_pose");
    graph.add_input_time_edge(GraphInputPin::passthrough("base_time"), id, "base_time");
    graph.add_input_data_edge(GraphInputPin::passthrough("layer_index"), id, "layer_index");
    graph.add_output_data_edge(id, "pose", "pose");
    for curve in ALS_LAYER_CURVE_NAMES {
        graph.add_output_data_edge(id, curve, curve);
    }
    graph.add_output_pose_edge(id);
    graph
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
    if !assets.is_loaded_with_dependencies(&gate_assets.interface)
        || !assets.is_loaded_with_dependencies(&gate_assets.graph)
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
        && graph.io_spec.has_output_time()
        && graph.default_data.is_empty();
    let curve_contract = ALS_LAYER_CURVE_NAMES
        .iter()
        .all(|curve| graph.io_spec.get_output_data(&curve.to_string()) == Some(DataSpec::F32));
    let evaluator_count = [
        "barrel poses frame zero evaluator",
        "barrel poses frame one evaluator",
        "barrel poses frame two evaluator",
    ]
    .into_iter()
    .filter(|name| graph_has_node(graph, name))
    .count()
        == 3;
    let normalized_stance = graph
        .nodes
        .values()
        .find(|node| node.name == "grounded stance multi way blend")
        .and_then(|node| node.try_inner_downcast_ref::<MultiWayBlendNode>())
        .is_some_and(|node| node.pose_count == 2 && node.normalize_alpha);
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
        .is_some_and(|node| close(node.value, 0.25));
    let idle_loop = graph
        .nodes
        .values()
        .find(|node| node.name == "secondary motion idle loop")
        .and_then(|node| node.try_inner_downcast_ref::<LoopNode>())
        .is_some_and(|node| node.interpolation_period.abs() < 1e-6);
    let sample_set = graph
        .nodes
        .values()
        .find(|node| node.name == "barrel pose curves")
        .and_then(|node| node.try_inner_downcast_ref::<CurveSampleSetNode>())
        .is_some_and(|node| {
            node.sample_positions.len() == 3
                && close(node.sample_positions[0], 0.0)
                && close(node.sample_positions[1], 0.033333335)
                && close(node.sample_positions[2], 0.06666667)
                && ALS_LAYER_CURVE_NAMES
                    .iter()
                    .all(|curve| node.curves.iter().any(|candidate| candidate.name == *curve))
                && node
                    .curves
                    .iter()
                    .find(|curve| curve.name == "HandRightIk")
                    .is_some_and(|curve| curve.keys.len() == 3 && close(curve.keys[2].v, 1.0000001))
                && node
                    .curves
                    .iter()
                    .find(|curve| curve.name == "LayerArmRightAdditive")
                    .is_some_and(|curve| {
                        curve.keys.len() == 3
                            && close(curve.keys[0].v, 0.75)
                            && curve.keys[2].v.abs() < 1e-5
                    })
        });
    let curve_blends = [
        ("barrel gait curve blend", 2, false),
        ("barrel stance curve blend", 2, true),
        ("locomotion action curve blend", 4, false),
    ]
    .into_iter()
    .all(|(name, input_count, normalize_weights)| {
        graph
            .nodes
            .values()
            .find(|node| node.name == name)
            .and_then(|node| node.try_inner_downcast_ref::<CurveBlendNode>())
            .is_some_and(|node| {
                node.input_count == input_count
                    && node.normalize_weights == normalize_weights
                    && node.curve_names.len() == ALS_LAYER_CURVE_NAMES.len()
            })
    });
    let curve_modifiers = [
        "getting up LayerArmLeft curve modify",
        "rolling LayerArmLeft curve modify",
    ]
    .into_iter()
    .all(|name| {
        graph
            .nodes
            .values()
            .find(|node| node.name == name)
            .and_then(|node| node.try_inner_downcast_ref::<CurveModifyNode>())
            .is_some_and(|node| {
                node.curve_names == ["LayerArmLeft"]
                    && node.values == [3.0]
                    && close(node.alpha, 1.0)
            })
    });
    let tag_blend = graph
        .nodes
        .values()
        .find(|node| node.name == "locomotion action tag blend")
        .and_then(|node| node.try_inner_downcast_ref::<GameplayTagBlendNode>())
        .is_some_and(|node| {
            node.tags
                == [
                    "Als.LocomotionAction.Mantling",
                    "Als.LocomotionAction.GettingUp",
                    "Als.LocomotionAction.Rolling",
                ]
                && node.blend_times.len() == 4
                && close(node.blend_times[0], 0.3)
                && close(node.blend_times[1], 0.1)
                && close(node.blend_times[2], 0.0)
                && close(node.blend_times[3], 0.1)
        });
    let source = std::fs::read_to_string(Path::new("assets").join(GRAPH_PATH)).unwrap_or_default();
    let asset_paths = source.matches("A_Als_Barrel_Poses.anim.ron").count() == 3
        && source.contains("animations/Base/A_Als_Idle.anim.ron");
    let pass = pose_contract
        && curve_contract
        && evaluator_count
        && normalized_stance
        && additive
        && additive_alpha
        && idle_loop
        && sample_set
        && curve_blends
        && curve_modifiers
        && tag_blend
        && asset_paths;
    info!(
        "[BARREL-OVERLAY-CONTRACT] pose_contract={} curve_contract={} evaluators={} normalized_stance={} additive={} additive_alpha={} idle_loop={} sample_set={} curve_blends={} curve_modifiers={} tag_blend={} asset_paths={}",
        pose_contract,
        curve_contract,
        evaluator_count,
        normalized_stance,
        additive,
        additive_alpha,
        idle_loop,
        sample_set,
        curve_blends,
        curve_modifiers,
        tag_blend,
        asset_paths,
    );
    if !pass {
        info!("[BARREL-OVERLAY-CONTRACT][VERDICT] static=false -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
        return;
    }

    let root = graphs.add(build_linked_root(
        gate_assets.interface.clone(),
        gate_assets.graph.clone(),
    ));
    let mut player = AnimationGraphPlayer::new(gate_assets.skeleton.clone()).with_graph(root);
    player.set_input_data("base_pose", DataValue::Pose(Pose::default()));
    player.set_input_data("layer_index", DataValue::F32(0.0));
    set_overlay_inputs(&mut player, 0.0, 1.0, 0.0, 0.0);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.player = Some(
        commands
            .spawn((
                Name::new("barrel_overlay_contract_probe"),
                Children::default(),
                Transform::IDENTITY,
                player,
            ))
            .id(),
    );
    probe.phase = ProbePhase::GroundedIdle;
    probe.static_verified = true;
    info!("[BARREL-OVERLAY-CONTRACT] linked runtime probe initialized");
}

fn verify_runtime_values(
    player: &AnimationGraphPlayer,
    hand_right: f32,
    arm_right: f32,
    arm_left: f32,
) -> bool {
    all_source_curve_values_match(player, hand_right, arm_right)
        && output_f32(player, "LayerArmLeft").is_some_and(|value| close(value, arm_left))
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
        warn!("[BARREL-OVERLAY-CONTRACT][VERDICT] runtime error={error:?} -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(4).unwrap()));
        return;
    }
    let Some(pose) = player.get_default_output_pose().cloned() else {
        return;
    };
    match probe.phase {
        ProbePhase::GroundedIdle => {
            if !verify_runtime_values(&player, 0.0, 0.75, 1.0) {
                warn!("[BARREL-OVERLAY-CONTRACT][VERDICT] grounded idle curves -> FAIL");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(5).unwrap()));
                return;
            }
            info!(
                "[BARREL-OVERLAY-CONTRACT] runtime state=grounded_idle bones={} HandRightIk=0.0 LayerArmRightAdditive=0.75 LayerArmLeft=1.0",
                pose.bones.len()
            );
            probe.grounded_idle = Some(pose);
            set_overlay_inputs(&mut player, 1.0, 1.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::GroundedWalk;
        }
        ProbePhase::GroundedWalk => {
            if !verify_runtime_values(&player, 0.0, 0.75, 1.0)
                || !probe
                    .grounded_idle
                    .as_ref()
                    .is_some_and(|idle| idle != &pose)
            {
                warn!("[BARREL-OVERLAY-CONTRACT][VERDICT] grounded walk output -> FAIL");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
                return;
            }
            info!("[BARREL-OVERLAY-CONTRACT] runtime state=grounded_walk pose_changed=true");
            probe.grounded_walk = Some(pose);
            set_overlay_inputs(&mut player, 0.0, 0.0, 1.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::Crouching;
        }
        ProbePhase::Crouching => {
            if !verify_runtime_values(&player, 1.0, 0.0, 1.0)
                || !probe
                    .grounded_walk
                    .as_ref()
                    .is_some_and(|walk| walk != &pose)
            {
                warn!("[BARREL-OVERLAY-CONTRACT][VERDICT] crouching output -> FAIL");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));
                return;
            }
            info!(
                "[BARREL-OVERLAY-CONTRACT] runtime state=crouching HandRightIk=1.0 LayerArmRightAdditive=0.0"
            );
            probe.crouching = Some(pose);
            set_overlay_inputs(&mut player, 0.0, 0.25, 0.25, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::MixedStance;
        }
        ProbePhase::MixedStance => {
            if !verify_runtime_values(&player, 0.5, 0.375, 1.0) {
                warn!("[BARREL-OVERLAY-CONTRACT][VERDICT] normalized stance curves -> FAIL");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(8).unwrap()));
                return;
            }
            info!(
                "[BARREL-OVERLAY-CONTRACT] runtime state=mixed_stance HandRightIk=0.5 LayerArmRightAdditive=0.375"
            );
            set_overlay_inputs(&mut player, 0.0, 1.0, 0.0, 2.0);
            player.queue_time_update(TimeUpdate::Delta(0.01));
            probe.phase = ProbePhase::GettingUp;
        }
        ProbePhase::GettingUp => {
            if !verify_runtime_values(&player, 0.0, 0.75, 3.0) {
                warn!(
                    "[BARREL-OVERLAY-CONTRACT][VERDICT] getting up curve modify HandRightIk={:?} LayerArmRightAdditive={:?} LayerArmLeft={:?} -> FAIL",
                    output_f32(&player, "HandRightIk"),
                    output_f32(&player, "LayerArmRightAdditive"),
                    output_f32(&player, "LayerArmLeft"),
                );
                exit.write(AppExit::Error(std::num::NonZeroU8::new(9).unwrap()));
                return;
            }
            info!("[BARREL-OVERLAY-CONTRACT] runtime action=GettingUp LayerArmLeft=3.0");
            set_overlay_inputs(&mut player, 0.0, 1.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.15));
            probe.phase = ProbePhase::ReturnHalf;
        }
        ProbePhase::ReturnHalf => {
            if !verify_runtime_values(&player, 0.0, 0.75, 2.0) {
                warn!("[BARREL-OVERLAY-CONTRACT][VERDICT] default return half blend -> FAIL");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(10).unwrap()));
                return;
            }
            info!("[BARREL-OVERLAY-CONTRACT] runtime action=Default elapsed=0.15 LayerArmLeft=2.0");
            player.queue_time_update(TimeUpdate::Delta(0.15));
            probe.phase = ProbePhase::ReturnDefault;
        }
        ProbePhase::ReturnDefault => {
            if !verify_runtime_values(&player, 0.0, 0.75, 1.0) {
                warn!("[BARREL-OVERLAY-CONTRACT][VERDICT] default return completion -> FAIL");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(11).unwrap()));
                return;
            }
            info!("[BARREL-OVERLAY-CONTRACT] runtime action=Default elapsed=0.30 LayerArmLeft=1.0");
            set_overlay_inputs(&mut player, 0.0, 1.0, 0.0, 3.0);
            player.queue_time_update(TimeUpdate::Delta(0.1));
            probe.phase = ProbePhase::Rolling;
        }
        ProbePhase::Rolling => {
            if !verify_runtime_values(&player, 0.0, 0.75, 3.0) {
                warn!("[BARREL-OVERLAY-CONTRACT][VERDICT] rolling curve modify -> FAIL");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(12).unwrap()));
                return;
            }
            info!("[BARREL-OVERLAY-CONTRACT] runtime action=Rolling LayerArmLeft=3.0");
            info!("[BARREL-OVERLAY-CONTRACT][VERDICT] graph=AB_Als_Barrel linked=true -> PASS");
            exit.write(AppExit::Success);
        }
        ProbePhase::Pending => {}
    }
}

fn auto_quit(time: Res<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= time.0 {
        warn!("[BARREL-OVERLAY-CONTRACT][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(13).unwrap()));
    }
}
