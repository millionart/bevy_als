use std::{
    path::Path,
    time::{Duration, Instant},
};

use als_ext::{
    ALS_LAYER_CURVE_NAMES, AlsExtPlugin, ConstantF32Node, CurveBlendNode, CurveModifyNode,
    CurveSampleSetNode, GameplayTagBlendMode, GameplayTagBlendNode, GlobalF32InputNode,
    LinkedAnimLayerNode, MeshSpaceAdditiveNode, MultiWayBlendNode,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::builtin_nodes::{
    blend_node::{BlendMode, BlendNode},
    loop_node::LoopNode,
    replicate_time::ReplicateTimeNode,
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
const GRAPH_PATH: &str = "graphs/ab_als/overlays/binoculars.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const GAIT_WALKING: &str = "als_overlay_gait_walking_amount";
const GAIT_SPRINTING: &str = "als_overlay_gait_sprinting_amount";
const STANDING: &str = "als_overlay_standing_amount";
const CROUCHING: &str = "als_overlay_crouching_amount";
const PITCH: &str = "als_overlay_pitch_amount";
const ROTATION_MODE: &str = "als_overlay_rotation_mode_index";
const LOCOMOTION_ACTION: &str = "als_overlay_locomotion_action_index";
const TEST_PITCH: f32 = 0.5;

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
    default_idle: Option<Pose>,
    default_walk: Option<Pose>,
    default_sprint: Option<Pose>,
    aim_pose: Option<Pose>,
    static_verified: bool,
}

#[derive(Default, PartialEq, Eq)]
enum ProbePhase {
    #[default]
    Pending,
    DefaultIdle,
    DefaultWalk,
    DefaultSprint,
    Crouching,
    AimingPartial,
    AimingFull,
    MantlingHalf,
    GettingUp,
    RollingHalf,
    RollingFull,
}

fn load_graph(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        interface: assets.load(INTERFACE_PATH),
        graph: assets.load(GRAPH_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    commands.insert_resource(AutoQuit(Instant::now() + Duration::from_secs(25)));
    info!("[BINOCULARS-OVERLAY-CONTRACT] loading graph=AB_Als_Binoculars");
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

fn curve_value(player: &AnimationGraphPlayer, name: &str, expected: f32) -> bool {
    output_f32(player, name).is_some_and(|value| close(value, expected))
}

fn all_default_idle_curves(player: &AnimationGraphPlayer) -> bool {
    [
        ("LayerArmLeft", 1.0),
        ("LayerArmLeftAdditive", 0.0),
        ("LayerArmLeftLocalSpace", 1.0),
        ("LayerArmRight", 1.0),
        ("LayerArmRightAdditive", 0.0),
        ("LayerArmRightLocalSpace", 1.0),
        ("LayerHead", 1.0),
        ("LayerHeadAdditive", 1.0),
        ("LayerLegs", 1.0),
        ("LayerPelvis", 1.0),
        ("LayerSpine", 1.0),
        ("LayerSpineAdditive", 1.0),
        ("HandLeftIk", 1.0),
        ("HandRightIk", 0.0),
        ("HipsDirectionLock", 0.0),
        ("LayerHandLeft", 0.0),
        ("LayerHandRight", 1.0),
        ("PoseAiming", 0.0),
    ]
    .into_iter()
    .all(|(name, expected)| curve_value(player, name, expected))
}

fn set_overlay_inputs(
    player: &mut AnimationGraphPlayer,
    gait_walking: f32,
    gait_sprinting: f32,
    standing: f32,
    crouching: f32,
    pitch: f32,
    rotation_mode: f32,
    locomotion_action: f32,
) {
    player.set_global_input_data(GAIT_WALKING, gait_walking);
    player.set_global_input_data(GAIT_SPRINTING, gait_sprinting);
    player.set_global_input_data(STANDING, standing);
    player.set_global_input_data(CROUCHING, crouching);
    player.set_global_input_data(PITCH, pitch);
    player.set_global_input_data(ROTATION_MODE, rotation_mode);
    player.set_global_input_data(LOCOMOTION_ACTION, locomotion_action);
}

fn build_linked_root(
    interface: Handle<AnimationGraph>,
    binoculars: Handle<AnimationGraph>,
) -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new(
        "AB_Als Binoculars linked overlay",
        LinkedAnimLayerNode {
            interface,
            implementations: vec![binoculars],
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

fn tag_contract(
    graph: &AnimationGraph,
    name: &str,
    tags: &[&str],
    blend_times: &[f32],
    blend_mode: GameplayTagBlendMode,
) -> bool {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<GameplayTagBlendNode>())
        .is_some_and(|node| {
            node.tags
                .iter()
                .map(String::as_str)
                .eq(tags.iter().copied())
                && node.blend_times.len() == blend_times.len()
                && node
                    .blend_times
                    .iter()
                    .zip(blend_times)
                    .all(|(actual, expected)| close(*actual, *expected))
                && node.blend_mode == blend_mode
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
        "default frame zero evaluator",
        "default walking evaluator",
        "default sprinting evaluator",
        "default crouching evaluator",
        "aim standing walk base evaluator",
        "aim standing run base evaluator",
        "aim crouching base evaluator",
        "mantling evaluator",
        "getting up evaluator",
        "rolling evaluator",
        "aim mesh additive evaluator",
        "aim crouching mesh additive evaluator",
    ]
    .into_iter()
    .filter(|name| graph_has_node(graph, name))
    .count()
        == 12;
    let multi_way = [
        ("default stance multi way blend", 2),
        ("aim stance multi way blend", 2),
        ("rotation mode pose multi way blend", 2),
        ("locomotion action pose multi way blend", 4),
    ]
    .into_iter()
    .all(|(name, pose_count)| {
        graph
            .nodes
            .values()
            .find(|node| node.name == name)
            .and_then(|node| node.try_inner_downcast_ref::<MultiWayBlendNode>())
            .is_some_and(|node| node.pose_count == pose_count && node.normalize_alpha)
    });
    let ordinary_blends = [
        "default sprinting blend",
        "default gait blend",
        "aim standing gait blend",
    ]
    .into_iter()
    .all(|name| {
        graph
            .nodes
            .values()
            .find(|node| node.name == name)
            .and_then(|node| node.try_inner_downcast_ref::<BlendNode>())
            .is_some_and(|node| matches!(node.mode, BlendMode::LinearInterpolate))
    });
    let mesh_additives = [
        "aim standing mesh space additive",
        "aim crouching mesh space additive",
    ]
    .into_iter()
    .all(|name| {
        graph
            .nodes
            .values()
            .find(|node| node.name == name)
            .and_then(|node| node.try_inner_downcast_ref::<MeshSpaceAdditiveNode>())
            .is_some()
    }) && graph
        .nodes
        .values()
        .find(|node| node.name == "mesh space additive alpha")
        .and_then(|node| node.try_inner_downcast_ref::<ConstantF32Node>())
        .is_some_and(|node| close(node.value, 1.0));
    let local_additive = graph
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
            .is_some_and(|node| close(node.value, 0.5));
    let idle_loop = graph
        .nodes
        .values()
        .find(|node| node.name == "secondary motion idle loop")
        .and_then(|node| node.try_inner_downcast_ref::<LoopNode>())
        .is_some_and(|node| node.interpolation_period.abs() < 1e-6);
    let tag_blends = tag_contract(
        graph,
        "rotation mode tag blend",
        &["Als.RotationMode.Aiming"],
        &[0.75, 0.2],
        GameplayTagBlendMode::HermiteCubic,
    ) && tag_contract(
        graph,
        "locomotion action tag blend",
        &[
            "Als.LocomotionAction.Mantling",
            "Als.LocomotionAction.GettingUp",
            "Als.LocomotionAction.Rolling",
        ],
        &[0.5, 0.2, 0.0, 0.2],
        GameplayTagBlendMode::Linear,
    );
    let time_distribution = graph
        .nodes
        .values()
        .find(|node| node.name == "overlay tag time distributor")
        .and_then(|node| node.try_inner_downcast_ref::<ReplicateTimeNode>())
        .is_some_and(|node| node.extra_count == 2)
        && ["rotation mode tag blend", "locomotion action tag blend"]
            .into_iter()
            .all(|name| {
                graph
                    .nodes
                    .values()
                    .find(|node| node.name == name)
                    .and_then(|node| node.try_inner_downcast_ref::<GameplayTagBlendNode>())
                    .is_some_and(|node| !node.forward_time)
            });
    let curve_samples = graph
        .nodes
        .values()
        .find(|node| node.name == "binoculars poses curve samples")
        .and_then(|node| node.try_inner_downcast_ref::<CurveSampleSetNode>())
        .is_some_and(|node| {
            node.sample_positions
                == [
                    0.0, 0.041667, 0.058333, 0.13333334, 0.06666667, 0.1, 0.16666667,
                ]
                && ALS_LAYER_CURVE_NAMES
                    .iter()
                    .all(|curve| node.curves.iter().any(|candidate| candidate.name == *curve))
                && node
                    .curves
                    .iter()
                    .find(|curve| curve.name == "HandRightIk")
                    .is_some_and(|curve| curve.keys.len() == 1 && curve.keys[0].v.abs() < 1e-6)
                && node
                    .curves
                    .iter()
                    .find(|curve| curve.name == "PoseAiming")
                    .is_some_and(|curve| curve.keys.len() == 6 && close(curve.keys[2].v, 1.0))
        });
    let curve_blends = [
        ("default sprinting curve blend", 2, false),
        ("default gait curve blend", 2, false),
        ("default stance curve blend", 2, true),
        ("aim standing gait curve blend", 2, false),
        ("aim stance curve blend", 2, true),
        ("rotation mode curve blend", 2, false),
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
    let curve_modifiers = graph
        .nodes
        .values()
        .find(|node| node.name == "getting up LayerArmRight curve modify")
        .and_then(|node| node.try_inner_downcast_ref::<CurveModifyNode>())
        .is_some_and(|node| {
            node.curve_names == ["LayerArmRight"] && node.values == [3.0] && close(node.alpha, 1.0)
        })
        && graph
            .nodes
            .values()
            .find(|node| node.name == "rolling arm curves modify")
            .and_then(|node| node.try_inner_downcast_ref::<CurveModifyNode>())
            .is_some_and(|node| {
                node.curve_names == ["LayerArmLeft", "LayerArmRight"]
                    && node.values == [3.0, 3.0]
                    && close(node.alpha, 1.0)
            });
    let global_inputs = [
        GAIT_WALKING,
        GAIT_SPRINTING,
        STANDING,
        CROUCHING,
        PITCH,
        ROTATION_MODE,
        LOCOMOTION_ACTION,
    ]
    .into_iter()
    .all(|key| {
        graph.nodes.values().any(|node| {
            node.try_inner_downcast_ref::<GlobalF32InputNode>()
                .is_some_and(|candidate| candidate.key == key)
        })
    });
    let source = std::fs::read_to_string(Path::new("assets").join(GRAPH_PATH)).unwrap_or_default();
    let asset_paths = source.matches("A_Als_Binoculars_Poses.anim.ron").count() == 10
        && source.contains("A_Als_Binoculars_Aim.anim.ron")
        && source.contains("A_Als_Binoculars_Aim_Crouch.anim.ron")
        && source.contains("animations/Base/A_Als_Idle.anim.ron")
        && Path::new("assets")
            .join("animations/Overlays/Binoculars/A_Als_Binoculars_Poses_NoCurves.anim.ron")
            .is_file()
        && !source.contains("AB_Als_Monolithic");
    let pass = pose_contract
        && curve_contract
        && evaluator_count
        && multi_way
        && ordinary_blends
        && mesh_additives
        && local_additive
        && idle_loop
        && tag_blends
        && time_distribution
        && curve_samples
        && curve_blends
        && curve_modifiers
        && global_inputs
        && asset_paths;
    info!(
        "[BINOCULARS-OVERLAY-CONTRACT] pose_contract={} curve_contract={} evaluators={} multi_way={} ordinary_blends={} mesh_additives={} local_additive={} idle_loop={} tag_blends={} time_distribution={} curve_samples={} curve_blends={} curve_modifiers={} global_inputs={} asset_paths={}",
        pose_contract,
        curve_contract,
        evaluator_count,
        multi_way,
        ordinary_blends,
        mesh_additives,
        local_additive,
        idle_loop,
        tag_blends,
        time_distribution,
        curve_samples,
        curve_blends,
        curve_modifiers,
        global_inputs,
        asset_paths,
    );
    if !pass {
        warn!("[BINOCULARS-OVERLAY-CONTRACT][VERDICT] static=false -> FAIL");
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
    set_overlay_inputs(&mut player, 0.0, 0.0, 1.0, 0.0, TEST_PITCH, 0.0, 0.0);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.player = Some(
        commands
            .spawn((
                Name::new("binoculars_overlay_contract_probe"),
                Children::default(),
                Transform::IDENTITY,
                player,
            ))
            .id(),
    );
    probe.phase = ProbePhase::DefaultIdle;
    probe.static_verified = true;
    info!("[BINOCULARS-OVERLAY-CONTRACT] linked runtime probe initialized");
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, reason: &str) {
    warn!("[BINOCULARS-OVERLAY-CONTRACT][VERDICT] {reason} -> FAIL");
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
        warn!("[BINOCULARS-OVERLAY-CONTRACT][VERDICT] runtime error={error:?} -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(4).unwrap()));
        return;
    }
    let Some(pose) = player.get_default_output_pose().cloned() else {
        return;
    };
    match probe.phase {
        ProbePhase::DefaultIdle => {
            if !all_default_idle_curves(&player) || pose.bones.is_empty() {
                fail(&mut exit, 5, "default idle curves");
                return;
            }
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT] runtime state=default_idle bones={} LayerArmLeft=1.0 LayerArmRight=1.0 PoseAiming=0.0",
                pose.bones.len()
            );
            probe.default_idle = Some(pose);
            set_overlay_inputs(&mut player, 1.0, 0.0, 1.0, 0.0, TEST_PITCH, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::DefaultWalk;
        }
        ProbePhase::DefaultWalk => {
            if !curve_value(&player, "LayerArmLeft", 1.0)
                || !probe
                    .default_idle
                    .as_ref()
                    .is_some_and(|idle| idle != &pose)
            {
                fail(&mut exit, 6, "default walk output");
                return;
            }
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT] runtime state=default_walk pose_changed=true LayerArmLeft=1.0"
            );
            probe.default_walk = Some(pose);
            set_overlay_inputs(&mut player, 1.0, 1.0, 1.0, 0.0, TEST_PITCH, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::DefaultSprint;
        }
        ProbePhase::DefaultSprint => {
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT] probe state=default_sprint LayerArmLeft={:?} LayerArmRight={:?} pose_changed={}",
                output_f32(&player, "LayerArmLeft"),
                output_f32(&player, "LayerArmRight"),
                probe
                    .default_walk
                    .as_ref()
                    .is_some_and(|walk| walk != &pose),
            );
            if !curve_value(&player, "LayerArmLeft", 0.0)
                || !curve_value(&player, "LayerArmRight", 0.75)
            {
                fail(&mut exit, 7, "default sprint output");
                return;
            }
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT] runtime state=default_sprint LayerArmLeft=0.0 LayerArmRight=0.75"
            );
            probe.default_sprint = Some(pose);
            set_overlay_inputs(&mut player, 0.0, 0.0, 0.0, 1.0, TEST_PITCH, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::Crouching;
        }
        ProbePhase::Crouching => {
            if !curve_value(&player, "LayerArmLeftLocalSpace", 1.0)
                || !curve_value(&player, "PoseAiming", 0.0)
                || !probe
                    .default_sprint
                    .as_ref()
                    .is_some_and(|sprint| sprint != &pose)
            {
                fail(&mut exit, 8, "crouching output");
                return;
            }
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT] runtime state=crouching LayerArmLeftLocalSpace=1.0 PoseAiming=0.0"
            );
            set_overlay_inputs(&mut player, 0.0, 0.0, 1.0, 0.0, TEST_PITCH, 1.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.05));
            probe.phase = ProbePhase::AimingPartial;
        }
        ProbePhase::AimingPartial => {
            if !curve_value(&player, "PoseAiming", 0.15625) {
                fail(&mut exit, 9, "aiming hermite partial blend");
                return;
            }
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT] runtime state=aiming elapsed=0.05 PoseAiming=0.15625"
            );
            player.queue_time_update(TimeUpdate::Delta(0.15));
            probe.phase = ProbePhase::AimingFull;
        }
        ProbePhase::AimingFull => {
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT] probe state=aiming_full PoseAiming={:?} mesh_pose_changed={}",
                output_f32(&player, "PoseAiming"),
                probe
                    .default_idle
                    .as_ref()
                    .is_some_and(|idle| idle != &pose),
            );
            if !curve_value(&player, "PoseAiming", 1.0)
                || !probe
                    .default_idle
                    .as_ref()
                    .is_some_and(|idle| idle != &pose)
            {
                fail(&mut exit, 10, "aiming mesh space output");
                return;
            }
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT] runtime state=aiming elapsed=0.20 PoseAiming=1.0 mesh_pose_changed=true"
            );
            probe.aim_pose = Some(pose);
            set_overlay_inputs(&mut player, 0.0, 0.0, 1.0, 0.0, TEST_PITCH, 1.0, 1.0);
            player.queue_time_update(TimeUpdate::Delta(0.1));
            probe.phase = ProbePhase::MantlingHalf;
        }
        ProbePhase::MantlingHalf => {
            if !curve_value(&player, "PoseAiming", 0.5)
                || !probe.aim_pose.as_ref().is_some_and(|aim| aim != &pose)
            {
                fail(&mut exit, 11, "mantling linear half blend");
                return;
            }
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT] runtime action=Mantling elapsed=0.10 PoseAiming=0.5"
            );
            set_overlay_inputs(&mut player, 0.0, 0.0, 1.0, 0.0, TEST_PITCH, 1.0, 2.0);
            player.queue_time_update(TimeUpdate::Delta(0.01));
            probe.phase = ProbePhase::GettingUp;
        }
        ProbePhase::GettingUp => {
            if !curve_value(&player, "LayerArmRight", 3.0)
                || !curve_value(&player, "LayerArmLeft", 1.0)
                || !curve_value(&player, "PoseAiming", 0.0)
            {
                fail(&mut exit, 12, "getting up curve modify");
                return;
            }
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT] runtime action=GettingUp LayerArmLeft=1.0 LayerArmRight=3.0"
            );
            set_overlay_inputs(&mut player, 0.0, 0.0, 1.0, 0.0, TEST_PITCH, 1.0, 3.0);
            player.queue_time_update(TimeUpdate::Delta(0.1));
            probe.phase = ProbePhase::RollingHalf;
        }
        ProbePhase::RollingHalf => {
            if !curve_value(&player, "LayerArmLeft", 2.0)
                || !curve_value(&player, "LayerArmRight", 3.0)
            {
                fail(&mut exit, 13, "rolling linear half blend");
                return;
            }
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT] runtime action=Rolling elapsed=0.10 LayerArmLeft=2.0 LayerArmRight=3.0"
            );
            player.queue_time_update(TimeUpdate::Delta(0.1));
            probe.phase = ProbePhase::RollingFull;
        }
        ProbePhase::RollingFull => {
            if !curve_value(&player, "LayerArmLeft", 3.0)
                || !curve_value(&player, "LayerArmRight", 3.0)
            {
                fail(&mut exit, 14, "rolling completion");
                return;
            }
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT] runtime action=Rolling elapsed=0.20 LayerArmLeft=3.0 LayerArmRight=3.0"
            );
            info!(
                "[BINOCULARS-OVERLAY-CONTRACT][VERDICT] graph=AB_Als_Binoculars linked=true -> PASS"
            );
            exit.write(AppExit::Success);
        }
        ProbePhase::Pending => {}
    }
}

fn auto_quit(time: Res<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= time.0 {
        warn!("[BINOCULARS-OVERLAY-CONTRACT][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(15).unwrap()));
    }
}
