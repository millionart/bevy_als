use std::time::{Duration, Instant};

use als_ext::{
    AlsExtPlugin, BlendSpace1DNode, GlobalF32InputNode, LayeredGraphNode, MeshSpaceAdditiveNode,
    ReferenceMeshDifferenceNode, TimePositionNode,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::core::{
    animation_clip::GraphClip,
    animation_graph::{AnimationGraph, GraphInputPin, SourcePin, TargetPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    animation_node::AnimationNode,
    edge_data::{DataSpec, DataValue},
    plugin::AnimationGraphSet,
    pose::Pose,
    skeleton::Skeleton,
};
use bevy_animation_graph::{AnimationGraphPlugin, builtin_nodes::clip_node::ClipNode};

const HEAD_PATH: &str = "graphs/ab_als/head.animgraph.ron";
const VIEW_PATH: &str = "graphs/ab_als/view.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const STAND_PATH: &str = "animations/Base/A_Als_Stand_Pose.anim.ron";

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
    head: Handle<AnimationGraph>,
    view: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
    stand: Handle<GraphClip>,
}

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Stand,
    View,
    HeadBase,
    HeadCenter,
    HeadOffAxis,
}

#[derive(Resource)]
struct Probe {
    stand: Option<Entity>,
    view: Option<Entity>,
    head: Option<Entity>,
    phase: Phase,
    base_pose: Option<Pose>,
    input_timestamp: Option<f32>,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            stand: None,
            view: None,
            head: None,
            phase: Phase::Stand,
            base_pose: None,
            input_timestamp: None,
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
    .add_systems(Startup, load_assets)
    .add_systems(Update, (verify_static, timeout))
    .add_systems(PostUpdate, observe.after(AnimationGraphSet::Final));
    app.run();
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        head: assets.load(HEAD_PATH),
        view: assets.load(VIEW_PATH),
        skeleton: assets.load(SKELETON_PATH),
        stand: assets.load(STAND_PATH),
    });
    commands.insert_resource(Deadline(Instant::now() + Duration::from_secs(30)));
    info!("[HEAD-CONTRACT] loading Head and View redirect assets");
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

fn fail(_exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[HEAD-CONTRACT][VERDICT] {message} -> FAIL");
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

fn set_globals(player: &mut AnimationGraphPlayer, graphs: &Assets<AnimationGraph>) {
    for (_, graph) in graphs.iter() {
        for node in graph.nodes.values() {
            if let Some(node) = node.try_inner_downcast_ref::<GlobalF32InputNode>() {
                player.set_global_input_data(node.key.clone(), 0.0_f32);
            }
        }
    }
    player.set_global_input_data("head_state.pitch_angle", 0.0_f32);
    player.set_global_input_data("head_state.yaw_amount", 0.5_f32);
    player.set_global_input_data("view_state.head_blend_amount", 0.0_f32);
}

fn set_curve_inputs(player: &mut AnimationGraphPlayer) {
    for curve in CURVES {
        let value = match *curve {
            "ViewBlock" => 0.25,
            "PoseAiming" => 0.5,
            _ => 0.0,
        };
        player.set_input_data(format!("view_{curve}"), DataValue::F32(value));
    }
}

fn output_f32(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_outputs().get(name) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
}

fn output_pose(player: &AnimationGraphPlayer) -> Option<&Pose> {
    match player.get_outputs().get("pose") {
        Some(DataValue::Pose(pose)) => Some(pose),
        _ => None,
    }
}

fn pose_scales_are_unit(pose: &Pose) -> bool {
    pose.bones
        .iter()
        .filter_map(|bone| bone.scale)
        .all(|scale| {
            scale.is_finite()
                && scale.x.abs() >= 0.95
                && scale.y.abs() >= 0.95
                && scale.z.abs() >= 0.95
                && scale.x.abs() <= 1.05
                && scale.y.abs() <= 1.05
                && scale.z.abs() <= 1.05
        })
}

fn max_translation_delta(base: &Pose, result: &Pose) -> f32 {
    result
        .paths
        .iter()
        .filter_map(|(bone, index)| {
            let result = result.bones.get(*index)?.translation?;
            let base = base.get_bone(*bone)?.translation?;
            Some((result - base).length())
        })
        .fold(0.0_f32, f32::max)
}

fn max_rotation_delta(base: &Pose, result: &Pose) -> f32 {
    result
        .paths
        .iter()
        .filter_map(|(bone, index)| {
            let result = result.bones.get(*index)?.rotation?;
            let base = base.get_bone(*bone)?.rotation?;
            Some(1.0 - result.dot(base).abs())
        })
        .fold(0.0_f32, f32::max)
}

fn max_translation_length(pose: &Pose) -> f32 {
    pose.bones
        .iter()
        .filter_map(|bone| bone.translation)
        .map(Vec3::length)
        .fold(0.0_f32, f32::max)
}

fn wrap_head_graph(head: &AnimationGraph, head_handle: Handle<AnimationGraph>) -> AnimationGraph {
    let mut graph = AnimationGraph::new();
    let node = AnimationNode::new("AB_Als_Head nested", LayeredGraphNode::new(head_handle));
    let node_id = node.id;
    graph.add_node(node);
    for (input, spec) in head.io_spec.iter_input_data() {
        let GraphInputPin::Passthrough(pin) = input else {
            continue;
        };
        graph.add_input_data(input.clone(), spec.clone());
        graph.add_edge(
            SourcePin::InputData(input.clone()),
            TargetPin::NodeData(node_id, pin.clone()),
        );
    }
    for input in head.io_spec.iter_input_times() {
        let GraphInputPin::Passthrough(pin) = input else {
            continue;
        };
        graph.add_input_time(input.clone());
        graph.add_edge(
            SourcePin::InputTime(input.clone()),
            TargetPin::NodeTime(node_id, pin.clone()),
        );
    }
    for (output, spec) in head.io_spec.iter_output_data() {
        graph.add_output_data(output.clone(), spec.clone());
        graph.add_edge(
            SourcePin::NodeData(node_id, output.clone()),
            TargetPin::OutputData(output.clone()),
        );
    }
    if head.io_spec.has_output_time() {
        graph.add_output_time();
        graph.add_edge(SourcePin::NodeTime(node_id), TargetPin::OutputTime);
    }
    graph
}

fn verify_static(
    mut commands: Commands,
    server: Res<AssetServer>,
    assets: Res<GateAssets>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut probe: ResMut<Probe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.stand.is_some()
        || probe.view.is_some()
        || !server.is_loaded_with_dependencies(&assets.head)
        || !server.is_loaded_with_dependencies(&assets.view)
        || !server.is_loaded_with_dependencies(&assets.skeleton)
        || !server.is_loaded_with_dependencies(&assets.stand)
    {
        return;
    }
    let (Some(head), Some(view)) = (graphs.get(&assets.head), graphs.get(&assets.view)) else {
        return;
    };
    let root = node::<MeshSpaceAdditiveNode>(head, "head mesh space additive").is_some()
        && node::<BlendSpace1DNode>(head, "BS_Als_Look evaluator").is_some_and(|node| {
            node.points.len() == 3
                && node.points[0].id == "down"
                && close(node.points[0].position, -90.0)
                && node.points[1].id == "forward"
                && close(node.points[1].position, 0.0)
                && node.points[2].id == "up"
                && close(node.points[2].position, 90.0)
        })
        && node::<GlobalF32InputNode>(head, "HeadState PitchAngle")
            .is_some_and(|node| node.key == "head_state.pitch_angle")
        && node::<GlobalF32InputNode>(head, "HeadState YawAmount")
            .is_some_and(|node| node.key == "head_state.yaw_amount")
        && node::<GlobalF32InputNode>(head, "ViewState HeadBlendAmount")
            .is_some_and(|node| node.key == "view_state.head_blend_amount");
    let evaluators = [
        "look down evaluator",
        "look forward evaluator",
        "look up evaluator",
    ]
    .into_iter()
    .all(|name| {
        node::<TimePositionNode>(head, name)
            .is_some_and(|node| !node.loop_position && close(node.duration_fallback, 1.0))
    });
    let reference_deltas = head
        .nodes
        .values()
        .filter(|node| {
            node.try_inner_downcast_ref::<ReferenceMeshDifferenceNode>()
                .is_some()
        })
        .count()
        == 3;
    let io = has_input(head, "view_pose")
        && head.io_spec.has_output_time()
        && head.io_spec.get_output_data(&"pose".to_owned()).is_some()
        && CURVES.iter().all(|curve| {
            has_input(head, &format!("view_{curve}"))
                && head.io_spec.get_output_data(&curve.to_string()).is_some()
        })
        && ["head_pitch_angle", "head_yaw_amount", "head_blend_amount"]
            .into_iter()
            .all(|name| head.io_spec.get_output_data(&name.to_owned()).is_some());
    let redirect_input = view.nodes.is_empty()
        && has_input(view, "view_pose")
        && view.io_spec.get_output_data(&"pose".to_owned()).is_some();
    let source =
        std::fs::read_to_string("assets/graphs/ab_als/head.animgraph.ron").unwrap_or_default();
    let architecture = !source.contains("AB_Als_Monolithic")
        && !source.contains("manual")
        && !source.contains("playground");
    info!(
        "[HEAD-CONTRACT] root={} evaluators={} reference_deltas={} io={} redirect_input={} architecture={}",
        root, evaluators, reference_deltas, io, redirect_input, architecture
    );
    if !(root && evaluators && reference_deltas && io && redirect_input && architecture) {
        fail(&mut exit, "static contract");
        return;
    }
    let input = Pose {
        timestamp: 1.75,
        skeleton: assets.skeleton.clone(),
        ..default()
    };
    let head_wrapper_graph = wrap_head_graph(head, assets.head.clone());
    let head_wrapper = graphs.add(head_wrapper_graph);
    let mut stand_graph = AnimationGraph::new();
    let stand_node = AnimationNode::new(
        "stand pose source",
        ClipNode::new(assets.stand.clone(), None, None),
    );
    let stand_node_id = stand_node.id;
    stand_graph.add_node(stand_node);
    stand_graph.add_output_data("pose".into(), DataSpec::Pose);
    stand_graph.add_output_time();
    stand_graph.add_edge(
        SourcePin::NodeData(stand_node_id, ClipNode::OUT_POSE.into()),
        TargetPin::OutputData("pose".into()),
    );
    stand_graph.add_edge(SourcePin::NodeTime(stand_node_id), TargetPin::OutputTime);
    let stand_graph = graphs.add(stand_graph);
    let mut stand_player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(stand_graph);
    stand_player.pause();
    stand_player.queue_time_update(TimeUpdate::Delta(0.0));
    let stand_entity = commands
        .spawn((
            Name::new("head_contract_stand_probe"),
            Children::default(),
            stand_player,
        ))
        .id();
    let mut view_player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.view.clone());
    view_player.set_input_data("view_pose", DataValue::Pose(input.clone()));
    view_player.pause();
    view_player.queue_time_update(TimeUpdate::Delta(0.0));
    let view_entity = commands
        .spawn((
            Name::new("head_contract_view_probe"),
            Children::default(),
            view_player,
        ))
        .id();
    let mut head_player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(head_wrapper);
    set_globals(&mut head_player, &graphs);
    set_curve_inputs(&mut head_player);
    head_player.set_input_data("view_pose", DataValue::Pose(input));
    head_player.pause();
    head_player.queue_time_update(TimeUpdate::Delta(0.0));
    let head_entity = commands
        .spawn((
            Name::new("head_contract_head_probe"),
            Children::default(),
            head_player,
        ))
        .id();
    probe.stand = Some(stand_entity);
    probe.view = Some(view_entity);
    probe.head = Some(head_entity);
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let (Some(stand_entity), Some(view_entity), Some(head_entity)) =
        (probe.stand, probe.view, probe.head)
    else {
        return;
    };
    match probe.phase {
        Phase::Stand => {
            let stand_pose = {
                let Ok(stand) = players.get_mut(stand_entity) else {
                    return;
                };
                if let Some(error) = stand.get_error() {
                    fail(&mut exit, &format!("stand runtime error={error:?}"));
                    return;
                }
                let Some(pose) = output_pose(&stand).cloned() else {
                    return;
                };
                pose
            };
            if stand_pose.bones.len() < 70 {
                fail(&mut exit, "stand source pose is incomplete");
                return;
            }
            let Ok(mut view) = players.get_mut(view_entity) else {
                return;
            };
            view.set_input_data("view_pose", DataValue::Pose(stand_pose.clone()));
            view.queue_time_update(TimeUpdate::Delta(0.0));
            probe.input_timestamp = Some(stand_pose.timestamp);
            probe.phase = Phase::View;
            info!(
                "[HEAD-CONTRACT] stand source bones={} max_translation_length={:.6} fed View redirect",
                stand_pose.bones.len(),
                max_translation_length(&stand_pose),
            );
        }
        Phase::View => {
            let Ok(view) = players.get_mut(view_entity) else {
                return;
            };
            if let Some(error) = view.get_error() {
                fail(&mut exit, &format!("view runtime error={error:?}"));
                return;
            }
            let Some(view_pose) = output_pose(&view).cloned() else {
                return;
            };
            let Ok(mut head) = players.get_mut(head_entity) else {
                return;
            };
            head.set_input_data("view_pose", DataValue::Pose(view_pose));
            head.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::HeadBase;
            info!("[HEAD-CONTRACT] View redirect output fed Head View Input");
        }
        Phase::HeadBase => {
            let Ok(mut head) = players.get_mut(head_entity) else {
                return;
            };
            if let Some(error) = head.get_error() {
                fail(&mut exit, &format!("head runtime error={error:?}"));
                return;
            }
            let Some(pose) = output_pose(&head).cloned() else {
                return;
            };
            let values = output_f32(&head, "head_pitch_angle")
                .is_some_and(|value| close(value, 0.0))
                && output_f32(&head, "head_yaw_amount").is_some_and(|value| close(value, 0.5))
                && output_f32(&head, "head_blend_amount").is_some_and(|value| close(value, 0.0))
                && output_f32(&head, "ViewBlock").is_some_and(|value| close(value, 0.25));
            let time_preserved = probe
                .input_timestamp
                .is_some_and(|timestamp| close(pose.timestamp, timestamp));
            if !values || !time_preserved || pose.bones.len() < 70 {
                info!(
                    "[HEAD-CONTRACT] base values={values} time_preserved={time_preserved} timestamp={} bones={}",
                    pose.timestamp,
                    pose.bones.len()
                );
                fail(&mut exit, "base runtime probe");
                return;
            }
            probe.base_pose = Some(pose);
            head.set_global_input_data("head_state.pitch_angle", 0.0_f32);
            head.set_global_input_data("head_state.yaw_amount", 0.5_f32);
            head.set_global_input_data("view_state.head_blend_amount", 1.0_f32);
            head.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::HeadCenter;
            info!("[HEAD-CONTRACT] base blend=0 pose/time/curve contract preserved");
        }
        Phase::HeadCenter => {
            let Ok(mut head) = players.get_mut(head_entity) else {
                return;
            };
            let Some(pose) = output_pose(&head).cloned() else {
                return;
            };
            let Some(base) = probe.base_pose.as_ref() else {
                return;
            };
            let values = output_f32(&head, "head_pitch_angle")
                .is_some_and(|value| close(value, 0.0))
                && output_f32(&head, "head_yaw_amount").is_some_and(|value| close(value, 0.5))
                && output_f32(&head, "head_blend_amount").is_some_and(|value| close(value, 1.0));
            let max_translation_delta = max_translation_delta(base, &pose);
            let max_rotation_delta = max_rotation_delta(base, &pose);
            let time_preserved = probe
                .input_timestamp
                .is_some_and(|timestamp| close(pose.timestamp, timestamp));
            info!(
                "[HEAD-CONTRACT] center values={values} time_preserved={time_preserved} scales={} unchanged={} max_translation_delta={max_translation_delta:.6} max_rotation_delta={max_rotation_delta:.6}",
                pose_scales_are_unit(&pose),
                pose == *base,
            );
            if !values
                || !time_preserved
                || !pose_scales_are_unit(&pose)
                || max_translation_delta > 0.1
                || max_rotation_delta > 1e-5
            {
                fail(&mut exit, "center additive runtime probe");
                return;
            }
            head.set_global_input_data("head_state.pitch_angle", 90.0_f32);
            head.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::HeadOffAxis;
            info!("[HEAD-CONTRACT] pitch=0 yaw_amount=.5 blend=1 preserves reference rotation");
        }
        Phase::HeadOffAxis => {
            let Ok(head) = players.get_mut(head_entity) else {
                return;
            };
            let Some(pose) = output_pose(&head).cloned() else {
                return;
            };
            let Some(base) = probe.base_pose.as_ref() else {
                return;
            };
            let values = output_f32(&head, "head_pitch_angle")
                .is_some_and(|value| close(value, 90.0))
                && output_f32(&head, "head_yaw_amount").is_some_and(|value| close(value, 0.5))
                && output_f32(&head, "head_blend_amount").is_some_and(|value| close(value, 1.0));
            let max_translation_delta = max_translation_delta(base, &pose);
            let max_rotation_delta = max_rotation_delta(base, &pose);
            let time_preserved = probe
                .input_timestamp
                .is_some_and(|timestamp| close(pose.timestamp, timestamp));
            if !values
                || pose == *base
                || !time_preserved
                || !pose_scales_are_unit(&pose)
                || max_translation_delta > 0.1
                || max_rotation_delta <= 1e-5
            {
                fail(&mut exit, "off-axis additive runtime probe");
                return;
            }
            info!(
                "[HEAD-CONTRACT] pitch=90 yaw_amount=.5 blend=1 max_translation_delta={max_translation_delta:.6} max_rotation_delta={max_rotation_delta:.6}"
            );
            info!("[HEAD-CONTRACT][VERDICT] linked=true -> PASS");
            exit.write(AppExit::Success);
        }
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
