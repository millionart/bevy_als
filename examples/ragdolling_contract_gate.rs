use std::time::{Duration, Instant};

use als_ext::{
    AlsExtPlugin, GameplayTagBlendMode, GlobalBoolInputNode, GlobalF32InputNode,
    GlobalPoseInputNode, RagdollConfigNode, RagdollingTagBlendNode,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::builtin_nodes::{
    clip_node::ClipNode, loop_node::LoopNode, speed_node::SpeedNode,
};
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    edge_data::DataValue,
    plugin::AnimationGraphSet,
    pose::Pose,
    ragdoll::definition::BodyMode,
    skeleton::Skeleton,
};
use serde_json::Value;

const RAGDOLLING_PATH: &str = "graphs/ab_als/ragdolling.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const CURVE_VALUE: f32 = 0.75;

const CURVES: &[&str] = &[
    "PoseStanding",
    "PoseCrouching",
    "PoseMoving",
    "RotationYawOffset",
    "FootLeftLock",
    "FootRightLock",
    "AllowTransitions",
    "RotationYawSpeed",
    "PoseGrounded",
    "FootLeftIk",
    "FootRightIk",
    "PoseInAir",
    "LayerHead",
    "LayerHeadAdditive",
    "LayerHeadSlot",
    "LayerArmLeft",
    "LayerArmLeftAdditive",
    "LayerArmLeftLocalSpace",
    "LayerArmLeftSlot",
    "LayerArmRight",
    "LayerArmRightAdditive",
    "LayerArmRightLocalSpace",
    "LayerArmRightSlot",
    "LayerHandLeft",
    "LayerHandRight",
    "LayerSpine",
    "LayerSpineAdditive",
    "LayerSpineSlot",
    "LayerPelvis",
    "LayerPelvisSlot",
    "LayerLegs",
    "LayerLegsSlot",
    "HandLeftIk",
    "HandRightIk",
    "HipsDirectionLock",
    "PoseAiming",
    "ViewBlock",
];

#[derive(Resource)]
struct GateAssets {
    graph: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Initial,
    Enter,
    Stable,
    Recover,
    Recovered,
}

#[derive(Resource)]
struct Probe {
    entity: Option<Entity>,
    phase: Phase,
    base_pose: Option<Pose>,
    snapshot_pose: Option<Pose>,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            entity: None,
            phase: Phase::Initial,
            base_pose: None,
            snapshot_pose: None,
        }
    }
}

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
    .init_resource::<Probe>()
    .add_systems(Startup, (verify_native_source, load_assets).chain())
    .add_systems(Update, (verify_static, timeout))
    .add_systems(PostUpdate, observe.after(AnimationGraphSet::Final));
    app.run();
}

fn verify_native_source() {
    let Ok(path) = std::env::var("RAGDOLLING_SOURCE_PATH") else {
        return;
    };
    let source: Value = serde_json::from_str(
        &std::fs::read_to_string(&path).expect("native Ragdolling source readable"),
    )
    .expect("native Ragdolling source parses");
    let graph = source["graphs"]
        .as_array()
        .and_then(|graphs| graphs.iter().find(|graph| graph["name"] == "AnimGraph"))
        .expect("native Ragdolling AnimGraph exists");
    let nodes = graph["nodes"].as_array().expect("native Ragdolling nodes");
    assert_eq!(nodes.len(), 6, "native Ragdolling authored node count");
    let root = native_node(nodes, "AnimGraphNode_Root", 0, 0);
    let input = native_node(nodes, "AnimGraphNode_LinkedInputPose", -528, 16);
    let outer = native_node(nodes, "AlsAnimGraphNode_GameplayTagsBlend", -336, 64);
    let inner = native_node(nodes, "AlsAnimGraphNode_GameplayTagsBlend", -672, 128);
    let flail = native_node(nodes, "AnimGraphNode_SequencePlayer", -1056, 192);
    let snapshot = native_node(nodes, "AnimGraphNode_PoseSnapshot", -1024, 64);
    let outer_text = native_property(outer, "Node");
    let inner_text = native_property(inner, "Node");
    assert!(outer_text.contains("Als.LocomotionAction.Ragdolling"));
    assert!(inner_text.contains("Als.LocomotionAction.Ragdolling"));
    assert_eq!(native_pin_default(outer, "BlendTime_0"), "0.400000");
    assert_eq!(native_pin_default(outer, "BlendTime_1"), "0.500000");
    assert_eq!(native_pin_default(inner, "BlendTime_0"), "0.000000");
    assert_eq!(native_pin_default(inner, "BlendTime_1"), "0.0");
    assert!(native_property(flail, "Node").contains("A_Als_Flail.A_Als_Flail"));
    assert!(native_property(flail, "Node").contains("PlayRate=1.000000"));
    assert!(native_property(flail, "Node").contains("bLoopAnimation=True"));
    assert!(native_property(snapshot, "Node").contains("Mode=SnapshotPin"));
    assert!(native_link(outer, "BlendPose_0", input));
    assert!(native_link(outer, "BlendPose_1", inner));
    assert!(native_link(inner, "BlendPose_0", snapshot));
    assert!(native_link(inner, "BlendPose_1", flail));
    assert!(native_link(root, "Result", outer));
    let target = std::fs::read_to_string("assets/graphs/ab_als/ragdolling.animgraph.ron")
        .expect("target Ragdolling graph readable");
    for position in [
        "\"1a220000-0000-4000-8000-000000000001\": (-1056.000, 192.000)",
        "\"1a220000-0000-4000-8000-000000000002\": (-864.000, 192.000)",
        "\"1a220000-0000-4000-8000-000000000003\": (-672.000, 192.000)",
        "\"1a220000-0000-4000-8000-000000000004\": (-1056.000, 288.000)",
        "\"1a220000-0000-4000-8000-000000000005\": (-528.000, 0.000)",
        "\"1a220000-0000-4000-8000-000000000006\": (-1024.000, 64.000)",
        "\"1a220000-0000-4000-8000-000000000007\": (-336.000, 64.000)",
        "\"1a220000-0000-4000-8000-000000000008\": (-336.000, 192.000)",
        "\"1a220000-0000-4000-8000-000000000009\": (-336.000, 288.000)",
        "\"1a220000-0000-4000-8000-000000000010\": (-336.000, 384.000)",
        "input_position: (-528.0, 16.0)",
        "output_position: (0.0, 0.0)",
    ] {
        assert!(
            target.contains(position),
            "target source layout position {position}"
        );
    }
    info!(
        "[RAGDOLLING-SOURCE] authored_nodes=6 pins=18 outer=(0.4,0.5) inner=(0,0) flail=looping snapshot=pin positions=10 -> PASS"
    );
}

fn native_node<'a>(nodes: &'a [Value], class: &str, x: i64, y: i64) -> &'a Value {
    nodes
        .iter()
        .find(|node| {
            node["class_path"]
                .as_str()
                .is_some_and(|path| path.contains(class))
                && node["position_x"].as_i64() == Some(x)
                && node["position_y"].as_i64() == Some(y)
        })
        .expect("native Ragdolling node")
}

fn native_property<'a>(node: &'a Value, name: &str) -> &'a str {
    node["properties"]
        .as_array()
        .and_then(|properties| {
            properties
                .iter()
                .find(|property| property["name"] == name)
                .and_then(|property| property["exported_text"].as_str())
        })
        .expect("native Ragdolling property")
}

fn native_pin_default<'a>(node: &'a Value, name: &str) -> &'a str {
    node["pins"]
        .as_array()
        .and_then(|pins| {
            pins.iter()
                .find(|pin| pin["name"] == name)
                .and_then(|pin| pin["default_value"].as_str())
        })
        .expect("native Ragdolling pin")
}

fn native_link(node: &Value, pin_name: &str, expected_node: &Value) -> bool {
    let Some(expected_guid) = expected_node["node_guid"].as_str() else {
        return false;
    };
    node["pins"]
        .as_array()
        .and_then(|pins| pins.iter().find(|pin| pin["name"] == pin_name))
        .and_then(|pin| pin["linked_to"].as_array())
        .is_some_and(|links| links.iter().any(|link| link["node_guid"] == expected_guid))
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        graph: assets.load(RAGDOLLING_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    commands.insert_resource(Deadline(Instant::now() + Duration::from_secs(30)));
    info!("[RAGDOLLING-CONTRACT] loading linked ragdolling graph");
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

fn fail(_exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[RAGDOLLING-CONTRACT][VERDICT] {message} -> FAIL");
    std::process::exit(7);
}

fn node<'a, T: 'static>(graph: &'a AnimationGraph, name: &str) -> Option<&'a T> {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<T>())
}

fn has_input(graph: &AnimationGraph, name: &str) -> bool {
    graph
        .io_spec
        .get_input_data(&GraphInputPin::Passthrough(name.to_owned()))
        .is_some()
}

fn output_f32(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_outputs().get(name) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
}

fn output_bool(player: &AnimationGraphPlayer, name: &str) -> Option<bool> {
    match player.get_outputs().get(name) {
        Some(DataValue::Bool(value)) => Some(*value),
        _ => None,
    }
}

fn output_pose(player: &AnimationGraphPlayer) -> Option<&Pose> {
    match player.get_outputs().get("pose") {
        Some(DataValue::Pose(pose)) => Some(pose),
        _ => None,
    }
}

fn config_matches(player: &AnimationGraphPlayer, dynamic: bool) -> bool {
    matches!(
        player.get_outputs().get("ragdoll_config"),
        Some(DataValue::RagdollConfig(config))
            if if dynamic {
                config.default_mode == Some(BodyMode::Dynamic)
                    && config.default_readback == Some(true)
            } else {
                config.default_mode == Some(BodyMode::Kinematic)
                    && config.default_readback == Some(false)
            }
    )
}

fn set_curve_inputs(player: &mut AnimationGraphPlayer) {
    for curve in CURVES {
        player.set_input_data(format!("ragdolling_{curve}"), DataValue::F32(CURVE_VALUE));
    }
}

fn set_runtime_inputs(
    player: &mut AnimationGraphPlayer,
    action_index: f32,
    flail_play_rate: f32,
    dynamic: bool,
) {
    player.set_global_input_data("als_ragdolling_action_index", action_index);
    player.set_global_input_data("als_ragdolling_flail_play_rate", flail_play_rate);
    player.set_global_input_data("als_ragdolling_dynamic", dynamic);
    player.set_global_input_data("als_ragdolling_readback", dynamic);
}

fn verify_static(
    mut commands: Commands,
    server: Res<AssetServer>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    mut probe: ResMut<Probe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.entity.is_some()
        || !server.is_loaded_with_dependencies(&assets.graph)
        || !server.is_loaded_with_dependencies(&assets.skeleton)
    {
        return;
    }
    let Some(graph) = graphs.get(&assets.graph) else {
        return;
    };
    let outer =
        node::<RagdollingTagBlendNode>(graph, "Ragdolling nested tag blend").is_some_and(|node| {
            node.tag == "Als.LocomotionAction.Ragdolling"
                && node.outer_blend_times == [0.4, 0.5]
                && node.blend_mode == GameplayTagBlendMode::Linear
                && node
                    .curve_names
                    .iter()
                    .map(String::as_str)
                    .eq(CURVES.iter().copied())
        });
    let inner =
        node::<RagdollingTagBlendNode>(graph, "Ragdolling nested tag blend").is_some_and(|node| {
            node.tag == "Als.LocomotionAction.Ragdolling"
                && node.inner_blend_times == [0.0, 0.0]
                && node.blend_mode == GameplayTagBlendMode::Linear
        });
    let player = node::<ClipNode>(graph, "ragdoll flail clip").is_some()
        && node::<LoopNode>(graph, "ragdoll flail loop")
            .is_some_and(|node| close(node.interpolation_period, 0.0))
        && node::<SpeedNode>(graph, "ragdoll flail speed").is_some()
        && node::<GlobalF32InputNode>(graph, "RagdollingState FlailPlayRate")
            .is_some_and(|node| node.key == "als_ragdolling_flail_play_rate")
        && node::<GlobalPoseInputNode>(graph, "FinalRagdollPose snapshot")
            .is_some_and(|node| node.key == "als_ragdolling_snapshot_pose");
    let config = node::<GlobalBoolInputNode>(graph, "ragdoll dynamic")
        .is_some_and(|node| node.key == "als_ragdolling_dynamic")
        && node::<GlobalBoolInputNode>(graph, "ragdoll readback")
            .is_some_and(|node| node.key == "als_ragdolling_readback")
        && node::<RagdollConfigNode>(graph, "ragdoll config").is_some();
    let io = has_input(graph, "ragdolling_pose")
        && graph.io_spec.has_output_time()
        && CURVES.iter().all(|curve| {
            has_input(graph, &format!("ragdolling_{curve}"))
                && graph.io_spec.get_output_data(&curve.to_string()).is_some()
        })
        && [
            "action_index",
            "outer_input_weight",
            "outer_ragdoll_weight",
            "inner_snapshot_weight",
            "inner_flail_weight",
            "flail_play_rate",
            "ragdoll_dynamic",
            "ragdoll_readback",
            "ragdoll_config",
        ]
        .into_iter()
        .all(|name| graph.io_spec.get_output_data(&name.to_owned()).is_some());
    let source = std::fs::read_to_string("assets/graphs/ab_als/ragdolling.animgraph.ron")
        .unwrap_or_default();
    let architecture = !source.contains("AB_Als_Monolithic")
        && !source.contains("manual")
        && !source.contains("playground")
        && !source.contains("locomotion_overlay");
    info!(
        "[RAGDOLLING-CONTRACT] outer={} inner={} player={} config={} io={} architecture={}",
        outer, inner, player, config, io, architecture
    );
    if !(outer && inner && player && config && io && architecture) {
        fail(&mut exit, "static contract");
        return;
    }
    let base_pose = Pose {
        timestamp: 1.75,
        skeleton: assets.skeleton.clone(),
        ..default()
    };
    let snapshot_pose = Pose {
        timestamp: 4.0,
        skeleton: assets.skeleton.clone(),
        ..default()
    };
    let mut player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.graph.clone());
    player.set_input_data("ragdolling_pose", DataValue::Pose(base_pose.clone()));
    player.set_global_input_data(
        "als_ragdolling_snapshot_pose",
        DataValue::Pose(snapshot_pose),
    );
    set_curve_inputs(&mut player);
    set_runtime_inputs(&mut player, 0.0, 1.0, false);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.base_pose = Some(base_pose);
    probe.entity = Some(
        commands
            .spawn((
                Name::new("ragdolling_contract_probe"),
                Children::default(),
                player,
            ))
            .id(),
    );
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(entity) = probe.entity else {
        return;
    };
    let Ok(mut player) = players.get_mut(entity) else {
        return;
    };
    if let Some(error) = player.get_error() {
        fail(&mut exit, &format!("runtime error={error:?}"));
        return;
    }
    match probe.phase {
        Phase::Initial => {
            let Some(base) = probe.base_pose.as_ref() else {
                return;
            };
            let valid = output_pose(&player).is_some_and(|pose| pose == base)
                && output_f32(&player, "outer_input_weight").is_some_and(|value| close(value, 1.0))
                && output_f32(&player, "outer_ragdoll_weight")
                    .is_some_and(|value| close(value, 0.0))
                && output_f32(&player, "PoseStanding")
                    .is_some_and(|value| close(value, CURVE_VALUE))
                && output_bool(&player, "ragdoll_dynamic") == Some(false)
                && output_bool(&player, "ragdoll_readback") == Some(false)
                && config_matches(&player, false);
            if !valid {
                fail(&mut exit, "initial input phase");
                return;
            }
            set_runtime_inputs(&mut player, 1.0, 0.5, true);
            player.queue_time_update(TimeUpdate::Delta(0.25));
            probe.phase = Phase::Enter;
        }
        Phase::Enter => {
            let valid = output_f32(&player, "action_index").is_some_and(|value| close(value, 1.0))
                && output_f32(&player, "outer_input_weight").is_some_and(|value| close(value, 0.5))
                && output_f32(&player, "outer_ragdoll_weight")
                    .is_some_and(|value| close(value, 0.5))
                && output_f32(&player, "inner_flail_weight").is_some_and(|value| close(value, 1.0))
                && output_f32(&player, "flail_play_rate").is_some_and(|value| close(value, 0.5))
                && output_bool(&player, "ragdoll_dynamic") == Some(true)
                && output_bool(&player, "ragdoll_readback") == Some(true)
                && config_matches(&player, true);
            if !valid {
                fail(&mut exit, "enter runtime phase");
                return;
            }
            info!(
                "[RAGDOLLING-CONTRACT] enter action=Ragdolling outer=.5/.5 inner_flail=1 config=dynamic/readback"
            );
            player.queue_time_update(TimeUpdate::Delta(0.25));
            probe.phase = Phase::Stable;
        }
        Phase::Stable => {
            let Some(stable_pose) = output_pose(&player).cloned() else {
                return;
            };
            let valid = output_f32(&player, "outer_input_weight")
                .is_some_and(|value| close(value, 0.0))
                && output_f32(&player, "outer_ragdoll_weight")
                    .is_some_and(|value| close(value, 1.0))
                && output_f32(&player, "inner_flail_weight").is_some_and(|value| close(value, 1.0))
                && output_f32(&player, "PoseStanding").is_some_and(|value| close(value, 0.0))
                && config_matches(&player, true);
            if !valid {
                fail(&mut exit, "stable runtime phase");
                return;
            }
            let mut snapshot = stable_pose;
            snapshot.timestamp = 4.0;
            player.set_global_input_data(
                "als_ragdolling_snapshot_pose",
                DataValue::Pose(snapshot.clone()),
            );
            player.set_global_input_data("als_ragdolling_action_index", 0.0_f32);
            player.set_global_input_data("als_ragdolling_dynamic", false);
            player.set_global_input_data("als_ragdolling_readback", false);
            player.queue_time_update(TimeUpdate::Delta(0.2));
            probe.snapshot_pose = Some(snapshot);
            probe.phase = Phase::Recover;
            info!(
                "[RAGDOLLING-CONTRACT] stable flail loop=1 rate=.5 curves=0 config=dynamic/readback"
            );
        }
        Phase::Recover => {
            let valid = output_f32(&player, "outer_input_weight")
                .is_some_and(|value| close(value, 0.5))
                && output_f32(&player, "outer_ragdoll_weight")
                    .is_some_and(|value| close(value, 0.5))
                && output_f32(&player, "inner_snapshot_weight")
                    .is_some_and(|value| close(value, 1.0))
                && output_f32(&player, "inner_flail_weight").is_some_and(|value| close(value, 0.0))
                && output_f32(&player, "PoseStanding")
                    .is_some_and(|value| close(value, CURVE_VALUE * 0.5))
                && output_bool(&player, "ragdoll_dynamic") == Some(false)
                && output_bool(&player, "ragdoll_readback") == Some(false)
                && config_matches(&player, false);
            if !valid {
                fail(&mut exit, "recovery runtime phase");
                return;
            }
            player.queue_time_update(TimeUpdate::Delta(0.2));
            probe.phase = Phase::Recovered;
            info!(
                "[RAGDOLLING-CONTRACT] recovery snapshot=1 outer=.5/.5 curve=.375 config=kinematic/no-readback"
            );
        }
        Phase::Recovered => {
            let Some(base) = probe.base_pose.as_ref() else {
                return;
            };
            let valid = output_pose(&player).is_some_and(|pose| pose == base)
                && output_f32(&player, "outer_input_weight").is_some_and(|value| close(value, 1.0))
                && output_f32(&player, "outer_ragdoll_weight")
                    .is_some_and(|value| close(value, 0.0))
                && output_f32(&player, "PoseStanding")
                    .is_some_and(|value| close(value, CURVE_VALUE));
            if !valid {
                fail(&mut exit, "recovery completion");
                return;
            }
            info!("[RAGDOLLING-CONTRACT][VERDICT] linked=true -> PASS");
            exit.write(AppExit::Success);
        }
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
