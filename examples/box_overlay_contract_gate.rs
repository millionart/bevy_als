use std::{
    path::Path,
    time::{Duration, Instant},
};

use als_ext::{
    ALS_LAYER_CURVE_NAMES, ALS_LAYER_SIGNAL_NAMES, AlsExtPlugin, ConstantF32Node, CurveModifyNode,
    CurveSampleSetNode, CurveWeightedSamplesNode, GameplayTagBlendMode, GameplayTagLayerBlendNode,
    GlobalSyncGroupSampleNode, LinkedAnimLayerNode, MultiWayBlendNode, TimePositionNode,
    set_gameplay_tag_input,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::builtin_nodes::blend_node::{BlendMode, BlendNode};
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    animation_node::AnimationNode,
    edge_data::{DataSpec, DataValue},
    plugin::AnimationGraphSet,
    pose::Pose,
    skeleton::Skeleton,
};
use bevy_gameplay_tag::GameplayTag;

const INTERFACE_PATH: &str = "graphs/ab_als/ali_overlay.animgraph.ron";
const BOX_GRAPH_PATH: &str = "graphs/ab_als/overlays/box.animgraph.ron";
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
    mantling_pose: Option<Pose>,
    getting_up_pose: Option<Pose>,
    static_verified: bool,
}

#[derive(Default, PartialEq, Eq)]
enum ProbePhase {
    #[default]
    Pending,
    Default,
    Walking,
    Crouching,
    Mantling,
    GettingUp,
    Rolling,
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        interface: assets.load(INTERFACE_PATH),
        implementation: assets.load(BOX_GRAPH_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    commands.insert_resource(AutoQuit(Instant::now() + Duration::from_secs(20)));
    info!("[BOX-OVERLAY-CONTRACT] loading linked layer assets");
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
        "AB_Als Box linked overlay",
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
    locomotion_action: f32,
) {
    player.set_global_input_data(GAIT_WALKING, gait_walking);
    player.set_global_input_data(STANDING, standing);
    player.set_global_input_data(CROUCHING, crouching);
    player.set_global_input_data(LOCOMOTION_ACTION, locomotion_action);
    player.set_global_input_data("als_secondary_motion_phase", 0.0_f32);
}

fn locomotion_action_tag(index: f32) -> GameplayTag {
    GameplayTag::new(match index as i32 {
        1 => "Als.LocomotionAction.Mantling",
        2 => "Als.LocomotionAction.GettingUp",
        3 => "Als.LocomotionAction.Rolling",
        _ => "",
    })
}

fn modify_contract(graph: &AnimationGraph, name: &str, curves: &[&str], values: &[f32]) -> bool {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<CurveModifyNode>())
        .is_some_and(|node| {
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
    let Some(box_graph) = graphs.get(&gate_assets.implementation) else {
        return;
    };
    let pose_contract = box_graph
        .io_spec
        .get_input_data(&GraphInputPin::passthrough("base_pose"))
        == Some(DataSpec::Pose)
        && box_graph
            .io_spec
            .has_input_time(&GraphInputPin::passthrough("base_time"))
        && box_graph.io_spec.get_output_data(&"pose".to_owned()) == Some(DataSpec::Pose)
        && box_graph.io_spec.has_output_time()
        && box_graph.default_data.is_empty();
    let curve_contract = ALS_LAYER_CURVE_NAMES
        .iter()
        .all(|curve| box_graph.io_spec.get_output_data(&curve.to_string()) == Some(DataSpec::F32));
    let signal_contract = ALS_LAYER_SIGNAL_NAMES.iter().all(|signal| {
        box_graph.io_spec.get_output_data(&signal.to_string()) == Some(DataSpec::F32)
    });
    let evaluators = [
        "box poses frame zero evaluator",
        "box poses frame one evaluator",
        "box poses frame two evaluator",
        "mantling box poses frame zero evaluator",
        "getting up box poses frame two evaluator",
        "rolling box poses frame zero evaluator",
    ]
    .into_iter()
    .all(|name| {
        box_graph
            .nodes
            .values()
            .find(|node| node.name == name)
            .and_then(|node| node.try_inner_downcast_ref::<TimePositionNode>())
            .is_some_and(|node| close(node.duration_fallback, 0.1))
    });
    let grounded = graph_has_node::<BlendNode>(box_graph, "grounded gait blend")
        && graph_has_node::<MultiWayBlendNode>(box_graph, "grounded stance multi way blend")
        && box_graph
            .nodes
            .values()
            .find(|node| node.name == "secondary motion additive")
            .and_then(|node| node.try_inner_downcast_ref::<BlendNode>())
            .is_some_and(|node| matches!(node.mode, BlendMode::Additive))
        && box_graph
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
            })
        && box_graph
            .nodes
            .values()
            .find(|node| node.name == "secondary motion additive alpha")
            .and_then(|node| node.try_inner_downcast_ref::<ConstantF32Node>())
            .is_some_and(|node| close(node.value, 0.75));
    let sample_set = box_graph
        .nodes
        .values()
        .find(|node| node.name == "box poses curve samples")
        .and_then(|node| node.try_inner_downcast_ref::<CurveSampleSetNode>())
        .is_some_and(|node| {
            node.sample_positions == [0.0, 0.033333335, 0.06666667]
                && node.curves.len() == ALS_LAYER_CURVE_NAMES.len()
                && node.curves.iter().any(|curve| curve.name == "HandLeftIk")
                && node.curves.iter().any(|curve| curve.name == "PoseAiming")
        });
    let weighted_curves = box_graph
        .nodes
        .values()
        .find(|node| node.name == "box weighted pose curves")
        .and_then(|node| node.try_inner_downcast_ref::<CurveWeightedSamplesNode>())
        .is_some_and(|node| {
            node.sample_positions == [0.0, 0.033333335, 0.06666667]
                && node.normalize_weights
                && node.curves.len() == ALS_LAYER_CURVE_NAMES.len()
        });
    let action_layer = box_graph
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
        box_graph,
        "mantling head and spine curve modify",
        &["LayerHeadAdditive", "LayerSpineAdditive"],
        &[0.5, 0.5],
    ) && modify_contract(
        box_graph,
        "getting up arms curve modify",
        &["LayerArmLeft", "LayerArmRight"],
        &[3.0, 3.0],
    ) && modify_contract(
        box_graph,
        "rolling arms curve modify",
        &["LayerArmLeft", "LayerArmRight"],
        &[3.0, 3.0],
    );
    let source =
        std::fs::read_to_string(Path::new("assets").join(BOX_GRAPH_PATH)).unwrap_or_default();
    let asset_contract = source.contains("A_Als_Box_Poses.anim.ron")
        && source.contains("A_Als_Idle.anim.ron")
        && !source.contains("AB_Als_Monolithic");
    let pass = pose_contract
        && curve_contract
        && signal_contract
        && evaluators
        && grounded
        && sample_set
        && weighted_curves
        && action_blend
        && modifiers
        && asset_contract;
    info!(
        "[BOX-OVERLAY-CONTRACT] pose_contract={} curve_contract={} signal_contract={} evaluators={} grounded={} sample_set={} weighted_curves={} action_blend={} modifiers={} asset_contract={}",
        pose_contract,
        curve_contract,
        signal_contract,
        evaluators,
        grounded,
        sample_set,
        weighted_curves,
        action_blend,
        modifiers,
        asset_contract,
    );
    if !pass {
        warn!("[BOX-OVERLAY-CONTRACT][VERDICT] static=false -> FAIL");
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
    set_inputs(&mut player, 0.0, 1.0, 0.0, 0.0);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    let entity = commands
        .spawn((
            Name::new("box_overlay_contract_probe"),
            Children::default(),
            Transform::IDENTITY,
            player,
        ))
        .id();
    set_gameplay_tag_input(entity, "locomotion_action", locomotion_action_tag(0.0));
    probe.player = Some(entity);
    probe.phase = ProbePhase::Default;
    probe.static_verified = true;
    info!("[BOX-OVERLAY-CONTRACT] linked runtime probe initialized");
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!("[BOX-OVERLAY-CONTRACT][VERDICT] {message} -> FAIL");
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
        ProbePhase::Default => {
            let expected = [
                ("LayerArmLeft", 1.0),
                ("LayerArmLeftAdditive", 0.0),
                ("LayerArmRight", 1.0),
                ("LayerArmRightAdditive", 0.0),
                ("LayerHeadAdditive", 0.5),
                ("LayerSpineAdditive", 0.75),
                ("HandLeftIk", 1.0),
                ("HandRightIk", 0.0),
                ("HipsDirectionLock", 0.0),
                ("LayerHandLeft", 0.0),
                ("LayerHandRight", 0.0),
                ("PoseAiming", 0.0),
                ("transition_left_request", 0.0),
            ];
            if !outputs_match(&player, &expected) {
                fail(&mut exit, 5, "Default source curve values");
                return;
            }
            probe.default_pose = Some(pose.clone());
            info!(
                "[BOX-OVERLAY-CONTRACT] default gait=0 standing=1 bones={} HandLeftIk=1 LayerHeadAdditive=0.5 LayerSpineAdditive=0.75",
                pose.bones.len()
            );
            set_inputs(&mut player, 1.0, 1.0, 0.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::Walking;
        }
        ProbePhase::Walking => {
            if !probe
                .default_pose
                .as_ref()
                .is_some_and(|default| default != &pose)
            {
                fail(&mut exit, 6, "Walking frame one pose did not change");
                return;
            }
            probe.walking_pose = Some(pose.clone());
            info!(
                "[BOX-OVERLAY-CONTRACT] gait=1 pose_changed=true bones={}",
                pose.bones.len()
            );
            set_inputs(&mut player, 0.0, 0.0, 1.0, 0.0);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::Crouching;
        }
        ProbePhase::Crouching => {
            if !probe
                .walking_pose
                .as_ref()
                .is_some_and(|walking| walking != &pose)
            {
                fail(&mut exit, 7, "Crouching frame two pose did not change");
                return;
            }
            probe.crouching_pose = Some(pose.clone());
            info!(
                "[BOX-OVERLAY-CONTRACT] crouching=1 pose_changed=true bones={}",
                pose.bones.len()
            );
            set_inputs(&mut player, 0.0, 1.0, 0.0, 1.0);
            set_gameplay_tag_input(entity, "locomotion_action", locomotion_action_tag(1.0));
            player.queue_time_update(TimeUpdate::Delta(0.2));
            probe.phase = ProbePhase::Mantling;
        }
        ProbePhase::Mantling => {
            if !outputs_match(
                &player,
                &[("LayerHeadAdditive", 0.5), ("LayerSpineAdditive", 0.5)],
            ) || !probe
                .crouching_pose
                .as_ref()
                .is_some_and(|crouching| crouching != &pose)
            {
                fail(&mut exit, 8, "Mantling pose or curves");
                return;
            }
            probe.mantling_pose = Some(pose.clone());
            info!(
                "[BOX-OVERLAY-CONTRACT] action=Mantling LayerHeadAdditive=0.5 LayerSpineAdditive=0.5 pose_changed=true"
            );
            set_inputs(&mut player, 0.0, 1.0, 0.0, 2.0);
            set_gameplay_tag_input(entity, "locomotion_action", locomotion_action_tag(2.0));
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::GettingUp;
        }
        ProbePhase::GettingUp => {
            if !outputs_match(&player, &[("LayerArmLeft", 3.0), ("LayerArmRight", 3.0)])
                || !probe
                    .mantling_pose
                    .as_ref()
                    .is_some_and(|mantling| mantling != &pose)
            {
                fail(&mut exit, 9, "GettingUp pose or curves");
                return;
            }
            probe.getting_up_pose = Some(pose.clone());
            info!(
                "[BOX-OVERLAY-CONTRACT] action=GettingUp LayerArmLeft=3 LayerArmRight=3 pose_changed=true"
            );
            set_inputs(&mut player, 0.0, 1.0, 0.0, 3.0);
            set_gameplay_tag_input(entity, "locomotion_action", locomotion_action_tag(3.0));
            player.queue_time_update(TimeUpdate::Delta(0.2));
            probe.phase = ProbePhase::Rolling;
        }
        ProbePhase::Rolling => {
            if !outputs_match(&player, &[("LayerArmLeft", 3.0), ("LayerArmRight", 3.0)])
                || !probe
                    .getting_up_pose
                    .as_ref()
                    .is_some_and(|getting_up| getting_up != &pose)
            {
                fail(&mut exit, 10, "Rolling pose or curves");
                return;
            }
            info!(
                "[BOX-OVERLAY-CONTRACT] action=Rolling LayerArmLeft=3 LayerArmRight=3 pose_changed=true"
            );
            info!("[BOX-OVERLAY-CONTRACT][VERDICT] linked=true -> PASS");
            exit.write(AppExit::Success);
        }
        ProbePhase::Pending => {}
    }
}

fn auto_quit(time: Res<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= time.0 {
        warn!("[BOX-OVERLAY-CONTRACT][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(11).unwrap()));
    }
}
