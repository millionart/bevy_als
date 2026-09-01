use std::time::{Duration, Instant};

use als_ext::{
    AlsExtPlugin, AnimationContextInput, ControlRigBoundaryNode, GlobalBoneMaskInputNode,
    GlobalBoolInputNode, GlobalF32InputNode, InertializationPassthroughNode,
    InertializedLinkedAnimLayerNode, InterruptibleFsmNode, LayeredGraphNode,
    SkeletonAwareLayeredGraphNode, player_animation_context_state,
};
use bevy::asset::LoadedFolder;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::{
    animation_clip::GraphClip,
    animation_graph::{AnimationGraph, GraphInputPin, SourcePin, TargetPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    edge_data::DataValue,
    plugin::AnimationGraphSet,
    pose::Pose,
    ragdoll::definition::BodyMode,
    skeleton::Skeleton,
};

const ROOT_PATH: &str = "graphs/ab_als/ab_als.animgraph.ron";
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

const SLOT_CURVES: &[&str] = &[
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
];

const OVERLAY_CURVES: &[&str] = &[
    "LayerArmLeft",
    "LayerArmLeftAdditive",
    "LayerArmLeftLocalSpace",
    "LayerArmRight",
    "LayerArmRightAdditive",
    "LayerArmRightLocalSpace",
    "LayerHead",
    "LayerHeadAdditive",
    "LayerLegs",
    "LayerPelvis",
    "LayerSpine",
    "LayerSpineAdditive",
    "HandLeftIk",
    "HandRightIk",
    "HipsDirectionLock",
    "LayerHandLeft",
    "LayerHandRight",
    "PoseAiming",
];

#[derive(Resource)]
struct GateAssets {
    root: Handle<AnimationGraph>,
    view: Handle<AnimationGraph>,
    grounded: Handle<AnimationGraph>,
    locomotion: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
    contracts: Vec<Handle<AnimationGraph>>,
    folder: Handle<LoadedFolder>,
    warmup: Vec<Handle<GraphClip>>,
}

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    TransitionSlot,
    PostLocomotionSlot,
    Overlay,
    Ragdoll,
    Restore,
}

#[derive(Resource)]
struct Probe {
    player: Option<Entity>,
    phase: Phase,
    base_pose: Option<Pose>,
    warmup_ticks: u8,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            player: None,
            phase: Phase::Idle,
            base_pose: None,
            warmup_ticks: 0,
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
        view: assets.load("graphs/ab_als/view.animgraph.ron"),
        grounded: assets.load("graphs/ab_als/grounded.animgraph.ron"),
        locomotion: assets.load("graphs/ab_als/locomotion.animgraph.ron"),
        skeleton: assets.load(SKELETON_PATH),
        contracts: [
            "graphs/ab_als/ali_overlay.animgraph.ron",
            "graphs/ab_als/grounded.animgraph.ron",
            "graphs/ab_als/locomotion.animgraph.ron",
            "graphs/ab_als/layering.animgraph.ron",
            "graphs/ab_als/head.animgraph.ron",
            "graphs/ab_als/ragdolling.animgraph.ron",
            "graphs/ab_als/overlays/default.animgraph.ron",
            "graphs/ab_als/overlays/masculine.animgraph.ron",
            "graphs/ab_als/overlays/feminine.animgraph.ron",
            "graphs/ab_als/overlays/injured.animgraph.ron",
            "graphs/ab_als/overlays/hands_tied.animgraph.ron",
            "graphs/ab_als/overlays/rifle.animgraph.ron",
            "graphs/ab_als/overlays/pistol_one_handed.animgraph.ron",
            "graphs/ab_als/overlays/pistol_two_handed.animgraph.ron",
            "graphs/ab_als/overlays/bow.animgraph.ron",
            "graphs/ab_als/overlays/torch.animgraph.ron",
            "graphs/ab_als/overlays/binoculars.animgraph.ron",
            "graphs/ab_als/overlays/box.animgraph.ron",
            "graphs/ab_als/overlays/barrel.animgraph.ron",
        ]
        .into_iter()
        .map(|path| assets.load(path))
        .collect(),
        folder: assets.load_folder("graphs/ab_als"),
        warmup: [
            "animations/Base/A_Als_Stand_Pose.anim.ron",
            "animations/Base/A_Als_Crouch_Pose.anim.ron",
            "animations/Overlays/Other/A_Als_Default_Poses.anim.ron",
            "animations/Base/A_Als_Idle.anim.ron",
        ]
        .into_iter()
        .map(|path| assets.load(path))
        .collect(),
    });
    commands.insert_resource(Deadline(Instant::now() + Duration::from_secs(45)));
    info!("[AB-ALS-CONTRACT] loading canonical root graph");
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

fn fail(_exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[AB-ALS-CONTRACT][VERDICT] {message} -> FAIL");
    std::process::exit(7);
}

fn node<'a, T: 'static>(graph: &'a AnimationGraph, name: &str) -> Option<&'a T> {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<T>())
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

fn context_input(
    inputs: &[AnimationContextInput],
    pose_input: &str,
    time_input: &str,
    curve_prefix: &str,
    missing_curve_value: Option<f32>,
) -> bool {
    inputs.iter().any(|input| {
        input.pose_input == pose_input
            && input.time_input == time_input
            && input.curve_prefix == curve_prefix
            && input.missing_curve_value == missing_curve_value
    })
}

fn context_f32(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    node_name: &str,
    name: &str,
) -> Option<f32> {
    let node = graph.nodes.values().find(|node| node.name == node_name)?;
    player_animation_context_state(player, node.id)?
        .curves
        .get(name)
        .copied()
        .filter(|value| value.is_finite())
}

fn root_f32(player: &AnimationGraphPlayer, graph: &AnimationGraph, name: &str) -> Option<f32> {
    context_f32(player, graph, "AB_Als_Ragdolling", name)
}

fn global_f32(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    player
        .get_global_input_data(name)
        .and_then(|value| value.as_f32().ok())
        .filter(|value| value.is_finite())
}

fn output_pose(player: &AnimationGraphPlayer) -> Option<&Pose> {
    match player.get_outputs().get("pose") {
        Some(DataValue::Pose(pose)) => Some(pose),
        _ => None,
    }
}

fn pose_is_finite(pose: &Pose) -> bool {
    pose.bones.iter().all(|bone| {
        bone.translation.is_none_or(Vec3::is_finite)
            && bone.rotation.is_none_or(Quat::is_finite)
            && bone.scale.is_none_or(Vec3::is_finite)
            && bone
                .weights
                .as_ref()
                .is_none_or(|weights| weights.iter().all(|value| value.is_finite()))
    })
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

fn scenes_use_canonical_root() -> bool {
    std::fs::read_dir("assets/animated_scenes")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "ron")
        })
        .all(|entry| {
            std::fs::read_to_string(entry.path())
                .is_ok_and(|scene| scene.contains("graphs/ab_als/ab_als.animgraph.ron"))
        })
}

fn set_global_defaults(player: &mut AnimationGraphPlayer, graphs: &Assets<AnimationGraph>) {
    for (_, graph) in graphs.iter() {
        for node in graph.nodes.values() {
            if let Some(node) = node.try_inner_downcast_ref::<GlobalF32InputNode>() {
                player.set_global_input_data(node.key.clone(), 0.0_f32);
            }
            if let Some(node) = node.try_inner_downcast_ref::<GlobalBoolInputNode>() {
                player.set_global_input_data(node.key.clone(), false);
            }
            if let Some(node) = node.try_inner_downcast_ref::<GlobalBoneMaskInputNode>() {
                player.set_global_input_data(
                    node.key.clone(),
                    DataValue::BoneMask(
                        bevy_animation_graph::core::edge_data::bone_mask::BoneMask::all(),
                    ),
                );
            }
        }
    }
    player.set_global_input_data("als_root_overlay_index", 0.0_f32);
    player.set_global_input_data("als_ragdolling_flail_play_rate", 0.5_f32);
    player.set_global_input_data("als_root_defaults_enabled", true);
}

fn verify_static(
    mut commands: Commands,
    server: Res<AssetServer>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    mut probe: ResMut<Probe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.player.is_some()
        || !server.is_loaded_with_dependencies(&assets.root)
        || !server.is_loaded_with_dependencies(&assets.view)
        || !server.is_loaded_with_dependencies(&assets.grounded)
        || !server.is_loaded_with_dependencies(&assets.locomotion)
        || !server.is_loaded_with_dependencies(&assets.skeleton)
        || !assets
            .contracts
            .iter()
            .all(|handle| server.is_loaded_with_dependencies(handle))
        || !server.is_loaded_with_dependencies(&assets.folder)
        || !assets
            .warmup
            .iter()
            .all(|handle| server.is_loaded_with_dependencies(handle))
    {
        return;
    }
    let (Some(root), Some(view), Some(grounded), Some(locomotion)) = (
        graphs.get(&assets.root),
        graphs.get(&assets.view),
        graphs.get(&assets.grounded),
        graphs.get(&assets.locomotion),
    ) else {
        return;
    };
    let linked = ["AB_Als_Layering", "AB_Als_Head", "AB_Als_Ragdolling"]
        .into_iter()
        .all(|name| {
            node::<LayeredGraphNode>(root, name).is_some_and(|node| {
                node.capture_context
                    && node.internalize_time
                    && !node.expose_context_curves
                    && (name != "AB_Als_Ragdolling"
                        || (node.hide_context_auxiliary_outputs
                            && node
                                .exposed_output_data
                                .iter()
                                .map(String::as_str)
                                .eq(std::iter::once("ragdoll_config"))))
            })
        })
        && ["AB_Als_Standing", "AB_Als_Crouching"]
            .into_iter()
            .all(|name| {
                node::<LayeredGraphNode>(root, name)
                    .is_some_and(|node| node.capture_context && node.internalize_time)
            })
        && ["AB_Als_Grounded", "AB_Als_Locomotion"]
            .into_iter()
            .all(|name| {
                node::<SkeletonAwareLayeredGraphNode>(root, name).is_some_and(|node| {
                    node.fallback_pose_key == "als_root_reference_pose"
                        && node.slot_name.is_none()
                        && node.capture_context
                        && node.internalize_time
                })
            });
    let grounded_stance_inputs = grounded
        .io_spec
        .get_input_data(&GraphInputPin::Passthrough("Standing Input".to_owned()))
        .is_some()
        && grounded
            .io_spec
            .get_input_data(&GraphInputPin::Passthrough("Crouching Input".to_owned()))
            .is_some()
        && grounded
            .io_spec
            .iter_input_times()
            .any(|pin| pin == &GraphInputPin::Passthrough("standing_time".to_owned()))
        && grounded
            .io_spec
            .iter_input_times()
            .any(|pin| pin == &GraphInputPin::Passthrough("crouching_time".to_owned()))
        && node::<InterruptibleFsmNode>(grounded, "Grounded States").is_some()
        && locomotion
            .io_spec
            .get_input_data(&GraphInputPin::Passthrough("grounded_pose".to_owned()))
            .is_some();
    let stance_contexts = node::<SkeletonAwareLayeredGraphNode>(root, "AB_Als_Grounded")
        .is_some_and(|node| {
            context_input(
                &node.context_inputs,
                "Standing Input",
                "standing_time",
                "standing_",
                Some(0.0),
            ) && context_input(
                &node.context_inputs,
                "Crouching Input",
                "crouching_time",
                "crouching_",
                Some(0.0),
            )
        });
    let root_chain =
        node_data_edge(
            root,
            "AB_Als_Grounded",
            "Standing Input",
            "AB_Als_Standing",
            "pose",
        ) && node_data_edge(
            root,
            "AB_Als_Grounded",
            "Crouching Input",
            "AB_Als_Crouching",
            "pose",
        ) && node_data_edge(
            root,
            "Slot 'Transition'",
            "Source",
            "AB_Als_Grounded",
            "pose",
        ) && node_data_edge(
            root,
            "AB_Als_Locomotion",
            "Grounded Input",
            "Slot 'Transition'",
            "pose",
        ) && node_data_edge(
            root,
            "Slot 'PostLocomotion'",
            "Source",
            "AB_Als_Locomotion",
            "pose",
        ) && node_data_edge(
            root,
            "AB_Als_Layering",
            "Locomotion Input",
            "Slot 'PostLocomotion'",
            "pose",
        ) && node_data_edge(root, "AB_Als_Head", "View Input", "AB_Als_Layering", "pose")
            && node_data_edge(root, "ControlRig CR_Als", "Source", "AB_Als_Head", "pose")
            && node_data_edge(
                root,
                "AB_Als_Ragdolling",
                "Ragdolling Input",
                "ControlRig CR_Als",
                "pose",
            )
            && node::<SkeletonAwareLayeredGraphNode>(root, "Slot 'Transition'").is_some_and(
                |node| {
                    node.capture_context
                        && node.internalize_time
                        && context_input(
                            &node.context_inputs,
                            "Source",
                            "source_time",
                            "source_",
                            Some(0.0),
                        )
                },
            )
            && node::<SkeletonAwareLayeredGraphNode>(root, "AB_Als_Locomotion").is_some_and(
                |node| {
                    node.capture_context
                        && node.internalize_time
                        && context_input(
                            &node.context_inputs,
                            "Grounded Input",
                            "grounded_time",
                            "grounded_",
                            None,
                        )
                },
            )
            && node::<SkeletonAwareLayeredGraphNode>(root, "Slot 'PostLocomotion'").is_some_and(
                |node| {
                    node.capture_context
                        && node.internalize_time
                        && context_input(
                            &node.context_inputs,
                            "Source",
                            "source_time",
                            "source_",
                            None,
                        )
                },
            )
            && node::<LayeredGraphNode>(root, "AB_Als_Layering").is_some_and(|node| {
                node.capture_context
                    && node.internalize_time
                    && context_input(
                        &node.context_inputs,
                        "Locomotion Input",
                        "locomotion_time_local_left_mesh",
                        "locomotion_",
                        Some(0.0),
                    )
            })
            && !root
                .nodes
                .values()
                .any(|node| node.name == "root zero curve");
    let overlay = node::<InertializedLinkedAnimLayerNode>(root, "ALI_Overlay - Overlay")
        .is_some_and(|node| {
            node.selector == "layer_index"
                && node.selector_global_key.as_deref() == Some("als_root_overlay_index")
                && node.implementations.len() == 13
                && node.reset_on_becoming_relevant
                && node.forward_requests_through_skipped_cached_poses
                && node.capture_context
                && !node.expose_context_curves
                && node.exposed_output_data.is_empty()
                && node.internalize_time
                && node.global_data_inputs.len() == 1
                && node.global_data_inputs[0].input == "base_pose"
                && node.global_data_inputs[0].global_key == "als_root_reference_pose"
        })
        && !root
            .nodes
            .values()
            .any(|node| node.name == "AB_Als Overlay local reference pose");
    let inertialization = node::<InertializationPassthroughNode>(root, "Inertialization")
        .is_some_and(|node| {
            node.reset_on_becoming_relevant
                && node.forward_requests_through_skipped_cached_poses
                && node.capture_context
                && node.exposed_output_data.is_empty()
                && node.internalize_time
                && node
                    .curve_names
                    .iter()
                    .map(String::as_str)
                    .eq(OVERLAY_CURVES
                        .iter()
                        .copied()
                        .chain(std::iter::once("transition_left_request")))
        });
    let overlay_chain =
        node_data_edge(
            root,
            "Inertialization",
            "Source",
            "ALI_Overlay - Overlay",
            "pose",
        ) && node_data_edge(
            root,
            "AB_Als_Layering",
            "Overlay Input",
            "Inertialization",
            "pose",
        ) && node::<LayeredGraphNode>(root, "AB_Als_Layering").is_some_and(|node| {
            context_input(
                &node.context_inputs,
                "Overlay Input",
                "overlay_time_legs",
                "overlay_",
                Some(0.0),
            )
        });
    let slots =
        node::<SkeletonAwareLayeredGraphNode>(root, "Slot 'Transition'").is_some_and(|node| {
            node.slot_name.as_deref() == Some("Transition")
                && node.slot_pose_key == "als_root_transition_slot_pose"
                && node.slot_weight_key == "als_root_transition_slot_weight"
                && node.slot_curve_prefix.as_deref() == Some("als_root_transition_slot_curve_")
                && node
                    .slot_curve_names
                    .iter()
                    .map(String::as_str)
                    .eq(SLOT_CURVES.iter().copied())
                && !node.always_update_source_pose
                && node.capture_context
                && node.internalize_time
        }) && node::<SkeletonAwareLayeredGraphNode>(root, "Slot 'PostLocomotion'").is_some_and(
            |node| {
                node.slot_name.as_deref() == Some("PostLocomotion")
                    && node.slot_pose_key == "als_root_post_locomotion_slot_pose"
                    && node.slot_weight_key == "als_root_post_locomotion_slot_weight"
                    && node.slot_curve_prefix.as_deref()
                        == Some("als_root_post_locomotion_slot_curve_")
                    && node
                        .slot_curve_names
                        .iter()
                        .map(String::as_str)
                        .eq(SLOT_CURVES.iter().copied())
                    && !node.always_update_source_pose
                    && node.capture_context
                    && node.internalize_time
            },
        );
    let view_redirect = view.nodes.is_empty()
        && view
            .io_spec
            .get_input_data(&GraphInputPin::Passthrough("view_pose".to_owned()))
            .is_some()
        && view.io_spec.get_output_data(&"pose".to_owned()).is_some()
        && node::<LayeredGraphNode>(root, "AB_Als_Head").is_some_and(|node| {
            node.internalize_time
                && context_input(
                    &node.context_inputs,
                    "View Input",
                    "view_time",
                    "view_",
                    None,
                )
        });
    let control_rig =
        node::<ControlRigBoundaryNode>(root, "ControlRig CR_Als").is_some_and(|node| {
            node.rig_class == "CR_Als"
                && node.alpha_enabled
                && node.transfer_input_pose
                && node.transfer_input_curves
                && !node.transfer_pose_in_global_space
                && node.pose_input == "Source"
                && node.capture_context
                && node.internalize_time
                && node
                    .curve_names
                    .iter()
                    .map(String::as_str)
                    .eq(CURVES.iter().copied())
        });
    let ragdoll_context = node::<LayeredGraphNode>(root, "AB_Als_Ragdolling").is_some_and(|node| {
        context_input(
            &node.context_inputs,
            "Ragdolling Input",
            "ragdolling_time",
            "ragdolling_",
            None,
        )
    });
    let io = !root.io_spec.has_output_time()
        && root.io_spec.iter_input_times().next().is_none()
        && root.io_spec.get_output_data(&"pose".to_owned()).is_some()
        && root
            .io_spec
            .get_output_data(&"ragdoll_config".to_owned())
            .is_some()
        && root.io_spec.iter_output_data().count() == 2;
    let source =
        std::fs::read_to_string("assets/graphs/ab_als/ab_als.animgraph.ron").unwrap_or_default();
    let architecture = !source.contains("AB_Als_Monolithic")
        && !source.contains("manual")
        && !source.contains("playground")
        && !source.contains("locomotion_overlay");
    let scenes = scenes_use_canonical_root();
    info!(
        "[AB-ALS-CONTRACT] linked={} grounded_stance_inputs={} stance_contexts={} root_chain={} overlay={} overlay_chain={} slots={} inertialization={} view_redirect={} control_rig={} io={} scenes={} architecture={}",
        linked,
        grounded_stance_inputs,
        stance_contexts,
        root_chain,
        overlay,
        overlay_chain,
        slots,
        inertialization,
        view_redirect,
        control_rig,
        io,
        scenes,
        architecture
    );
    if !(linked
        && grounded_stance_inputs
        && stance_contexts
        && root_chain
        && overlay
        && overlay_chain
        && slots
        && inertialization
        && view_redirect
        && control_rig
        && ragdoll_context
        && io
        && scenes
        && architecture)
    {
        fail(&mut exit, "static root contract");
        return;
    }
    let mut player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.root.clone());
    set_global_defaults(&mut player, &graphs);
    player.set_global_input_data(
        "als_root_reference_pose",
        DataValue::Pose(Pose {
            skeleton: assets.skeleton.clone(),
            ..default()
        }),
    );
    if !matches!(
        player.get_global_input_data("als_root_defaults_enabled"),
        Some(DataValue::Bool(true))
    ) {
        fail(&mut exit, "root defaults flag");
        return;
    }
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.player = Some(
        commands
            .spawn((
                Name::new("ab_als_contract_probe"),
                Children::default(),
                player,
            ))
            .id(),
    );
}

fn observe(
    mut probe: ResMut<Probe>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(entity) = probe.player else {
        return;
    };
    let Ok(mut player) = players.get_mut(entity) else {
        return;
    };
    let Some(root) = graphs.get(&assets.root) else {
        return;
    };
    if let Some(error) = player.get_error() {
        if probe.warmup_ticks < 16 {
            probe.warmup_ticks += 1;
            player.queue_time_update(TimeUpdate::Delta(0.0));
            info!(
                "[AB-ALS-CONTRACT] warmup={} deferred runtime error={error:?}",
                probe.warmup_ticks
            );
            return;
        }
        fail(&mut exit, &format!("runtime error={error:?}"));
        return;
    }
    match probe.phase {
        Phase::Idle => {
            let Some(pose) = output_pose(&player).cloned() else {
                return;
            };
            let valid = pose_is_finite(&pose)
                && config_matches(&player, false)
                && global_f32(&player, "als_root_overlay_index")
                    .is_some_and(|value| close(value, 0.0))
                && root_f32(&player, root, "action_index").is_some_and(|value| close(value, 0.0))
                && CURVES
                    .iter()
                    .all(|curve| root_f32(&player, root, curve).is_some());
            if !valid {
                fail(&mut exit, "idle root phase");
                return;
            }
            probe.base_pose = Some(pose);
            let Some(slot_pose) = probe.base_pose.clone() else {
                return;
            };
            player
                .set_global_input_data("als_root_transition_slot_pose", DataValue::Pose(slot_pose));
            player.set_global_input_data("als_root_transition_slot_weight", 0.5_f32);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::TransitionSlot;
            info!(
                "[AB-ALS-CONTRACT] idle grounded/locomotion/layering/head/ragdoll chain is active"
            );
        }
        Phase::TransitionSlot => {
            let valid = output_pose(&player).is_some_and(pose_is_finite)
                && global_f32(&player, "als_root_transition_slot_weight")
                    .is_some_and(|value| close(value, 0.5));
            if !valid {
                fail(&mut exit, "transition slot phase");
                return;
            }
            let Some(slot_pose) = output_pose(&player).cloned() else {
                return;
            };
            player.set_global_input_data("als_root_transition_slot_weight", 0.0_f32);
            player.set_global_input_data(
                "als_root_post_locomotion_slot_pose",
                DataValue::Pose(slot_pose),
            );
            player.set_global_input_data("als_root_post_locomotion_slot_weight", 0.5_f32);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::PostLocomotionSlot;
            info!("[AB-ALS-CONTRACT] Transition slot applies through the root boundary");
        }
        Phase::PostLocomotionSlot => {
            let valid = output_pose(&player).is_some_and(pose_is_finite)
                && global_f32(&player, "als_root_post_locomotion_slot_weight")
                    .is_some_and(|value| close(value, 0.5));
            if !valid {
                fail(&mut exit, "post locomotion slot phase");
                return;
            }
            player.set_global_input_data("als_root_post_locomotion_slot_weight", 0.0_f32);
            player.set_global_input_data("als_root_overlay_index", 9.0_f32);
            player.queue_time_update(TimeUpdate::Delta(0.2));
            probe.phase = Phase::Overlay;
            info!("[AB-ALS-CONTRACT] PostLocomotion slot applies through the root boundary");
        }
        Phase::Overlay => {
            let valid = output_pose(&player).is_some_and(pose_is_finite)
                && global_f32(&player, "als_root_overlay_index")
                    .is_some_and(|value| close(value, 9.0))
                && root_f32(&player, root, "PoseStanding").is_some()
                && root_f32(&player, root, "LayerHead").is_some()
                && context_f32(&player, root, "Inertialization", "transition_left_request")
                    .is_some()
                && config_matches(&player, false);
            if !valid {
                fail(&mut exit, "overlay linked layer phase");
                return;
            }
            player.set_global_input_data("als_ragdolling_action_index", 1.0_f32);
            player.set_global_input_data("als_ragdolling_dynamic", true);
            player.set_global_input_data("als_ragdolling_readback", true);
            player.set_global_input_data("als_ragdolling_flail_play_rate", 0.5_f32);
            player.queue_time_update(TimeUpdate::Delta(0.5));
            probe.phase = Phase::Ragdoll;
            info!("[AB-ALS-CONTRACT] Overlay index=Torch reached the linked layer branch");
        }
        Phase::Ragdoll => {
            let valid = output_pose(&player).is_some_and(pose_is_finite)
                && root_f32(&player, root, "action_index").is_some_and(|value| close(value, 1.0))
                && config_matches(&player, true);
            if !valid {
                fail(&mut exit, "ragdoll root phase");
                return;
            }
            let Some(snapshot) = probe.base_pose.clone() else {
                return;
            };
            player.set_global_input_data("als_ragdolling_snapshot_pose", DataValue::Pose(snapshot));
            player.set_global_input_data("als_ragdolling_action_index", 0.0_f32);
            player.set_global_input_data("als_ragdolling_dynamic", false);
            player.set_global_input_data("als_ragdolling_readback", false);
            player.queue_time_update(TimeUpdate::Delta(0.4));
            probe.phase = Phase::Restore;
            info!("[AB-ALS-CONTRACT] Ragdolling is final root branch with dynamic/readback config");
        }
        Phase::Restore => {
            let valid = root_f32(&player, root, "action_index")
                .is_some_and(|value| close(value, 0.0))
                && config_matches(&player, false)
                && global_f32(&player, "als_root_transition_slot_weight").is_some()
                && global_f32(&player, "als_root_post_locomotion_slot_weight").is_some();
            if !valid {
                fail(&mut exit, "root recovery phase");
                return;
            }
            info!("[AB-ALS-CONTRACT][VERDICT] linked=true -> PASS");
            exit.write(AppExit::Success);
        }
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
