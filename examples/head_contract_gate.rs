use std::time::{Duration, Instant};

use als_ext::{
    AlsExtPlugin, BlendSpace1DNode, GlobalF32InputNode, MeshSpaceAdditiveNode, TimePositionNode,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    edge_data::DataValue,
    plugin::AnimationGraphSet,
    pose::Pose,
    skeleton::Skeleton,
};

const HEAD_PATH: &str = "graphs/ab_als/head.animgraph.ron";
const VIEW_PATH: &str = "graphs/ab_als/view.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";

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
}

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    View,
    HeadBase,
    HeadAdditive,
}

#[derive(Resource)]
struct Probe {
    view: Option<Entity>,
    head: Option<Entity>,
    phase: Phase,
    base_pose: Option<Pose>,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            view: None,
            head: None,
            phase: Phase::View,
            base_pose: None,
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
    player.set_global_input_data("als_head_pitch_angle", 0.0_f32);
    player.set_global_input_data("als_head_yaw_amount", 0.5_f32);
    player.set_global_input_data("als_head_blend_amount", 0.0_f32);
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

fn verify_static(
    mut commands: Commands,
    server: Res<AssetServer>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    mut probe: ResMut<Probe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.view.is_some()
        || !server.is_loaded_with_dependencies(&assets.head)
        || !server.is_loaded_with_dependencies(&assets.view)
        || !server.is_loaded_with_dependencies(&assets.skeleton)
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
            .is_some_and(|node| node.key == "als_head_pitch_angle")
        && node::<GlobalF32InputNode>(head, "HeadState YawAmount")
            .is_some_and(|node| node.key == "als_head_yaw_amount")
        && node::<GlobalF32InputNode>(head, "ViewState HeadBlendAmount")
            .is_some_and(|node| node.key == "als_head_blend_amount");
    let evaluators = [
        "look down evaluator",
        "look forward evaluator",
        "look up evaluator",
    ]
    .into_iter()
    .all(|name| {
        node::<TimePositionNode>(head, name)
            .is_some_and(|node| node.loop_position && close(node.duration_fallback, 1.0))
    });
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
        "[HEAD-CONTRACT] root={} evaluators={} io={} redirect_input={} architecture={}",
        root, evaluators, io, redirect_input, architecture
    );
    if !(root && evaluators && io && redirect_input && architecture) {
        fail(&mut exit, "static contract");
        return;
    }
    let input = Pose {
        timestamp: 1.75,
        skeleton: assets.skeleton.clone(),
        ..default()
    };
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
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.head.clone());
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
    probe.view = Some(view_entity);
    probe.head = Some(head_entity);
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let (Some(view_entity), Some(head_entity)) = (probe.view, probe.head) else {
        return;
    };
    match probe.phase {
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
            if !values || !close(pose.timestamp, 1.75) {
                fail(&mut exit, "base runtime probe");
                return;
            }
            probe.base_pose = Some(pose);
            head.set_global_input_data("als_head_pitch_angle", 90.0_f32);
            head.set_global_input_data("als_head_yaw_amount", 0.5_f32);
            head.set_global_input_data("als_head_blend_amount", 1.0_f32);
            head.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::HeadAdditive;
            info!("[HEAD-CONTRACT] base blend=0 pose/time/curve contract preserved");
        }
        Phase::HeadAdditive => {
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
            if !values || pose == *base || !close(pose.timestamp, 1.75) {
                fail(&mut exit, "additive runtime probe");
                return;
            }
            info!(
                "[HEAD-CONTRACT] pitch=90 yaw_amount=.5 blend=1 changes the mesh-space additive pose"
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
