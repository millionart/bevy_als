use std::time::{Duration, Instant};

use als_ext::{
    AlsExtPlugin, BranchFilter, CurveComposeMode, CurveComposeNode, CurveModifyNode,
    GlobalBoneMaskInputNode, GlobalF32InputNode, MeshSpaceDifferenceNode,
    MeshSpaceLayeredBlendNode, SlotNode, build_branch_filter_bone_mask,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    edge_data::{DataValue, bone_mask::BoneMask},
    plugin::AnimationGraphSet,
    pose::{BoneId, BonePose, Pose},
    skeleton::Skeleton,
};

const ROOT_PATH: &str = "graphs/ab_als/layering.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const LAYERING_DIR: &str = "assets/graphs/ab_als/layering";

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
    root: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbePhase {
    Initial,
    OverlayChanged,
}

#[derive(Resource)]
struct Probe {
    player: Option<Entity>,
    phase: ProbePhase,
    locomotion_time: f32,
    spine_bone: Option<BoneId>,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            player: None,
            phase: ProbePhase::Initial,
            locomotion_time: 0.0,
            spine_bone: None,
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
        root: assets.load(ROOT_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    commands.insert_resource(Deadline(Instant::now() + Duration::from_secs(30)));
    info!("[LAYERING-CONTRACT] loading linked layering assets");
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

fn fail(_exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[LAYERING-CONTRACT][VERDICT] {message} -> FAIL");
    std::process::exit(5);
}

fn node<'a, T: 'static>(graph: &'a AnimationGraph, name: &str) -> Option<&'a T> {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<T>())
}

fn graph_with_node<'a, T: 'static>(
    graphs: &'a Assets<AnimationGraph>,
    name: &str,
) -> Option<&'a AnimationGraph> {
    graphs
        .iter()
        .find_map(|(_, graph)| node::<T>(graph, name).map(|_| graph))
}

fn source_files() -> Vec<String> {
    let mut paths = vec!["assets/graphs/ab_als/layering.animgraph.ron".to_owned()];
    if let Ok(entries) = std::fs::read_dir(LAYERING_DIR) {
        paths.extend(entries.filter_map(Result::ok).filter_map(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "ron")
                .then(|| entry.path().to_string_lossy().into_owned())
        }));
    }
    paths
        .into_iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect()
}

fn set_default_globals(player: &mut AnimationGraphPlayer, graphs: &Assets<AnimationGraph>) {
    for (_, graph) in graphs.iter() {
        for node in graph.nodes.values() {
            if let Some(node) = node.try_inner_downcast_ref::<GlobalF32InputNode>() {
                player.set_global_input_data(node.key.clone(), 0.0_f32);
            }
            if let Some(node) = node.try_inner_downcast_ref::<GlobalBoneMaskInputNode>() {
                player
                    .set_global_input_data(node.key.clone(), DataValue::BoneMask(BoneMask::none()));
            }
        }
    }
    player.set_global_input_data("als_layering_standing_amount", 1.0_f32);
    player.set_global_input_data("als_layering_crouching_amount", 0.0_f32);
}

fn set_layer_inputs(
    player: &mut AnimationGraphPlayer,
    skeleton: Handle<Skeleton>,
    spine_id: BoneId,
) {
    let mut locomotion = Pose {
        timestamp: 1.25,
        skeleton: skeleton.clone(),
        ..default()
    };
    locomotion.add_bone(
        BonePose {
            translation: Some(Vec3::ZERO),
            ..default()
        },
        spine_id,
    );
    let mut overlay = Pose {
        timestamp: 8.0,
        skeleton,
        ..default()
    };
    overlay.add_bone(
        BonePose {
            translation: Some(Vec3::new(3.0, 0.0, 0.0)),
            ..default()
        },
        spine_id,
    );
    player.set_input_data("locomotion_pose", DataValue::Pose(locomotion));
    player.set_input_data("overlay_pose", DataValue::Pose(overlay));
    for curve in CURVES {
        let locomotion_value = match *curve {
            "PoseStanding" => 1.0,
            "LayerHead" => 0.25,
            "LayerHeadSlot" => 0.8,
            _ => 0.0,
        };
        let overlay_value = match *curve {
            "PoseStanding" => 0.5,
            "LayerHead" => 0.75,
            "LayerHeadSlot" => 0.4,
            _ => 0.0,
        };
        player.set_input_data(
            format!("locomotion_{curve}"),
            DataValue::F32(locomotion_value),
        );
        player.set_input_data(format!("overlay_{curve}"), DataValue::F32(overlay_value));
    }
}

fn set_layer_masks(player: &mut AnimationGraphPlayer, skeleton: &Skeleton) {
    let mask = build_branch_filter_bone_mask(
        skeleton,
        &[BranchFilter {
            bone_name: "spine_01",
            blend_depth: 3,
        }],
    );
    player.set_global_input_data("als_layering_spine_mask", DataValue::BoneMask(mask));
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

fn has_input(root: &AnimationGraph, name: &str) -> bool {
    root.io_spec
        .get_input_data(&GraphInputPin::Passthrough(name.to_owned()))
        .is_some()
}

fn verify_static(
    mut commands: Commands,
    server: Res<AssetServer>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    skeletons: Res<Assets<Skeleton>>,
    mut probe: ResMut<Probe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.player.is_some()
        || !server.is_loaded_with_dependencies(&assets.root)
        || !server.is_loaded_with_dependencies(&assets.skeleton)
    {
        return;
    }
    let Some(root) = graphs.get(&assets.root) else {
        return;
    };
    let root_contract = [
        "Locomotion Additive Local Space ArmLeft Mesh",
        "Locomotion Additive Local Space ArmRight Mesh",
        "Locomotion Additive Mesh Space Legs",
        "Locomotion Additive Mesh Space Head",
        "Legs",
        "Pelvis",
        "Spine",
        "Head",
        "Arm Left Mesh",
        "Arm Right Mesh",
        "Arm Left Local",
        "Arm Right Local",
    ]
    .into_iter()
    .all(|name| node::<als_ext::LayeredGraphNode>(root, name).is_some());
    let slots = [
        (
            "Legs",
            "als_layering_slot_legs_weight",
            "als_layering_legs_slot",
        ),
        (
            "Pelvis",
            "als_layering_slot_pelvis_weight",
            "als_layering_pelvis_slot",
        ),
        (
            "Spine",
            "als_layering_slot_spine_weight",
            "als_layering_spine_slot",
        ),
        (
            "Head",
            "als_layering_slot_head_weight",
            "als_layering_head_slot",
        ),
        (
            "ArmLeft",
            "als_layering_slot_arm_left_weight",
            "als_layering_arm_left_slot",
        ),
        (
            "ArmRight",
            "als_layering_slot_arm_right_weight",
            "als_layering_arm_right_slot",
        ),
    ]
    .into_iter()
    .all(|(slot, weight_key, layer_weight_key)| {
        graphs.iter().any(|(_, graph)| {
            graph
                .nodes
                .values()
                .filter_map(|node| node.try_inner_downcast_ref::<SlotNode>())
                .any(|node| {
                    node.slot_name == slot
                        && node.weight_key == weight_key
                        && node.layer_weight_key == layer_weight_key
                })
        })
    });
    let additive = graphs.iter().any(|(_, graph)| {
        node::<MeshSpaceDifferenceNode>(graph, "mesh dynamic additive").is_some()
    }) && graphs
        .iter()
        .filter_map(|(_, graph)| node::<MeshSpaceLayeredBlendNode>(graph, "pelvis mesh layer"))
        .count()
        == 1
        && graphs.iter().any(|(_, graph)| {
            node::<MeshSpaceLayeredBlendNode>(graph, "arm right mesh layer").is_some()
        });
    let curves = graph_with_node::<CurveModifyNode>(&graphs, "Layer slot curve reset").is_some_and(
        |node_graph| {
            node::<CurveModifyNode>(node_graph, "Layer slot curve reset").is_some_and(|node| {
                node.curve_names
                    == vec![
                        "LayerHeadSlot",
                        "LayerArmLeftSlot",
                        "LayerArmRightSlot",
                        "LayerSpineSlot",
                        "LayerPelvisSlot",
                        "LayerLegsSlot",
                    ]
                    && node.values == vec![0.0; 6]
                    && close(node.alpha, 1.0)
            }) && node::<CurveComposeNode>(node_graph, "Locomotion plus Curves slot").is_some_and(
                |node| node.mode == CurveComposeMode::Accumulate && close(node.alpha, 1.0),
            ) && node::<CurveComposeNode>(node_graph, "Layering curve override").is_some_and(
                |node| node.mode == CurveComposeMode::Override && close(node.alpha, 1.0),
            )
        },
    );
    let io = has_input(root, "locomotion_pose")
        && has_input(root, "overlay_pose")
        && root.io_spec.has_output_time()
        && root.io_spec.get_output_data(&"pose".to_owned()).is_some()
        && CURVES.iter().all(|curve| {
            has_input(root, &format!("locomotion_{curve}"))
                && has_input(root, &format!("overlay_{curve}"))
                && root.io_spec.get_output_data(&curve.to_string()).is_some()
        });
    let source = source_files();
    let architecture = !source.is_empty()
        && source.iter().all(|text| {
            !text.contains("AB_Als_Monolithic")
                && !text.contains("locomotion_overlay")
                && !text.contains("manual")
                && !text.contains("playground")
        });
    info!(
        "[LAYERING-CONTRACT] root={} slots={} additive={} curves={} io={} architecture={}",
        root_contract, slots, additive, curves, io, architecture
    );
    if !(root_contract && slots && additive && curves && io && architecture) {
        fail(&mut exit, "static contract");
        return;
    }
    let Some(skeleton) = skeletons.get(&assets.skeleton) else {
        return;
    };
    let spine_paths = skeleton
        .iter_paths()
        .filter(|path| {
            path.parts
                .last()
                .is_some_and(|name| name.as_str() == "spine_01")
        })
        .map(|path| {
            path.parts
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect::<Vec<_>>();
    info!("[LAYERING-CONTRACT] skeleton spine_paths={spine_paths:?}");
    let Some(spine_bone) = skeleton
        .iter_paths()
        .find(|path| {
            path.parts
                .last()
                .is_some_and(|name| name.as_str() == "spine_01")
        })
        .map(|path| path.id())
    else {
        fail(&mut exit, "skeleton spine path");
        return;
    };
    let mut player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.root.clone());
    set_default_globals(&mut player, &graphs);
    set_layer_masks(&mut player, skeleton);
    set_layer_inputs(&mut player, assets.skeleton.clone(), spine_bone);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.player = Some(
        commands
            .spawn((
                Name::new("layering_contract_probe"),
                Children::default(),
                player,
            ))
            .id(),
    );
    probe.spine_bone = Some(spine_bone);
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(entity) = probe.player else {
        return;
    };
    let Some(spine_bone) = probe.spine_bone else {
        return;
    };
    let Ok(mut player) = players.get_mut(entity) else {
        return;
    };
    if let Some(error) = player.get_error() {
        fail(&mut exit, &format!("runtime error={error:?}"));
        return;
    }
    let Some(pose) = output_pose(&player) else {
        return;
    };
    match probe.phase {
        ProbePhase::Initial => {
            let curves_match = output_f32(&player, "PoseStanding")
                .is_some_and(|value| close(value, 1.5))
                && output_f32(&player, "LayerHead").is_some_and(|value| close(value, 1.0))
                && output_f32(&player, "LayerHeadSlot").is_some_and(|value| close(value, 0.8));
            if !curves_match {
                fail(&mut exit, "curve composition");
                return;
            }
            probe.locomotion_time = pose.timestamp;
            player.set_input_data("overlay_LayerHead", DataValue::F32(0.1));
            player.set_input_data("overlay_LayerHeadSlot", DataValue::F32(1.0));
            player.set_global_input_data("als_layering_spine", 1.0_f32);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = ProbePhase::OverlayChanged;
            info!(
                "[LAYERING-CONTRACT] curves accumulate=1 override-slot=0.8 locomotion_time={}",
                probe.locomotion_time
            );
        }
        ProbePhase::OverlayChanged => {
            let curves_match = output_f32(&player, "LayerHead")
                .is_some_and(|value| close(value, 0.35))
                && output_f32(&player, "LayerHeadSlot").is_some_and(|value| close(value, 0.8));
            let spine_translation = pose
                .get_bone(spine_bone)
                .and_then(|bone| bone.translation)
                .unwrap_or_default();
            info!(
                "[LAYERING-CONTRACT] overlay phase layer_head={:?} layer_head_slot={:?} pose_time={} spine_x={}",
                output_f32(&player, "LayerHead"),
                output_f32(&player, "LayerHeadSlot"),
                pose.timestamp,
                spine_translation.x
            );
            if !curves_match
                || !close(pose.timestamp, probe.locomotion_time)
                || !(spine_translation.x > 0.0 && spine_translation.x < 3.0)
            {
                fail(&mut exit, "overlay contract continuity");
                return;
            }
            info!(
                "[LAYERING-CONTRACT] overlay swap keeps locomotion pose time continuous spine_weighted_x={}",
                spine_translation.x
            );
            info!("[LAYERING-CONTRACT][VERDICT] linked=true -> PASS");
            exit.write(AppExit::Success);
        }
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
