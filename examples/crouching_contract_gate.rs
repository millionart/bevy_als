use std::{
    path::Path,
    time::{Duration, Instant},
};

use als_ext::{
    AlsExtPlugin, ConstantF32Node, DirectTransitionEventNode, F32ToVec2Node,
    GlobalBoneMaskInputNode, GlobalF32InputNode, InertializationNode, InterruptibleFsmNode,
    InterruptibleFsmState, LayeredGraphNode, MeshSpaceLayeredBlendNode, MultiWayBlendNode,
    ProfiledBlendNode, ResetOnActivationNode, SyncGroupSampleNode, TimePositionNode,
    build_branch_bone_mask,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::builtin_nodes::{blend_space_node::BlendSpaceNode, loop_node::LoopNode};
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, SourcePin, TargetPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    context::node_states::StateKey,
    edge_data::DataValue,
    plugin::AnimationGraphSet,
    pose::Pose,
    skeleton::Skeleton,
    state_machine::{
        high_level::{StateMachine, TransitionId, TransitionKind},
        low_level::LowLevelStateId,
    },
};

const ROOT_PATH: &str = "graphs/ab_als/stances/crouching.animgraph.ron";
const OUTER_PATH: &str = "graphs/ab_als/stances/crouching/crouching_states.fsm.ron";
const MOVEMENT_PATH: &str = "graphs/ab_als/stances/crouching/movement_states.fsm.ron";
const MOVE_PATH: &str = "graphs/ab_als/stances/crouching/state_move.animgraph.ron";
const BLEND_PATH: &str = "graphs/ab_als/stances/crouching/state_movement_blend.animgraph.ron";
const DIRECTION_PATH: &str =
    "graphs/ab_als/stances/crouching/state_movement_direction.animgraph.ron";
const STOP_PATH: &str = "graphs/ab_als/stances/crouching/state_stop.animgraph.ron";
const IDLE_PATH: &str = "graphs/ab_als/stances/crouching/state_idle.animgraph.ron";
const ROTATE_LEFT_PATH: &str = "graphs/ab_als/stances/crouching/state_rotate_left.animgraph.ron";
const ROTATE_RIGHT_PATH: &str = "graphs/ab_als/stances/crouching/state_rotate_right.animgraph.ron";
const TURN_PATH: &str = "graphs/ab_als/stances/crouching/turn.animgraph.ron";
const LEAN_PATH: &str = "graphs/ab_als/stances/crouching/lean.animgraph.ron";
const OUTER_BLEND_PATH: &str = "graphs/ab_als/stances/crouching/transition_outer_02.animgraph.ron";
const OUTER_ROTATE_PATH: &str = "graphs/ab_als/stances/crouching/transition_outer_04.animgraph.ron";
const STOP_BLEND_PATH: &str = "graphs/ab_als/stances/crouching/transition_stop_01.animgraph.ron";
const QUICK_FEET_PATH: &str =
    "graphs/ab_als/stances/crouching/transition_quickfeet_05.animgraph.ron";
const INERTIAL_PATH: &str = "graphs/ab_als/stances/crouching/transition_inertial_02.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const WRAPPER_PATHS: [&str; 6] = [
    "graphs/ab_als/stances/crouching/state_movement_forward.animgraph.ron",
    "graphs/ab_als/stances/crouching/state_movement_backward.animgraph.ron",
    "graphs/ab_als/stances/crouching/state_movement_right_forward.animgraph.ron",
    "graphs/ab_als/stances/crouching/state_movement_right_backward.animgraph.ron",
    "graphs/ab_als/stances/crouching/state_movement_left_forward.animgraph.ron",
    "graphs/ab_als/stances/crouching/state_movement_left_backward.animgraph.ron",
];

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

#[derive(Resource)]
struct GateAssets {
    root: Handle<AnimationGraph>,
    outer: Handle<StateMachine>,
    movement: Handle<StateMachine>,
    move_graph: Handle<AnimationGraph>,
    blend: Handle<AnimationGraph>,
    direction: Handle<AnimationGraph>,
    stop: Handle<AnimationGraph>,
    idle: Handle<AnimationGraph>,
    rotate_left: Handle<AnimationGraph>,
    rotate_right: Handle<AnimationGraph>,
    turn: Handle<AnimationGraph>,
    lean: Handle<AnimationGraph>,
    outer_blend: Handle<AnimationGraph>,
    outer_rotate: Handle<AnimationGraph>,
    stop_blend: Handle<AnimationGraph>,
    quick_feet: Handle<AnimationGraph>,
    inertial: Handle<AnimationGraph>,
    wrappers: [Handle<AnimationGraph>; 6],
    skeleton: Handle<Skeleton>,
}

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource, Default)]
struct Probe {
    static_verified: bool,
    outer_player: Option<Entity>,
    movement_player: Option<Entity>,
    runtime_player: Option<Entity>,
    outer_step: usize,
    outer_pending: bool,
    movement_step: usize,
    movement_pending: bool,
    phase: RuntimePhase,
    last_pose: Option<Pose>,
}

#[derive(Default, PartialEq, Eq)]
enum RuntimePhase {
    #[default]
    Pending,
    Outer,
    Movement,
    Idle,
    Move,
    Rotate,
    Turn,
    Transition,
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        root: assets.load(ROOT_PATH),
        outer: assets.load(OUTER_PATH),
        movement: assets.load(MOVEMENT_PATH),
        move_graph: assets.load(MOVE_PATH),
        blend: assets.load(BLEND_PATH),
        direction: assets.load(DIRECTION_PATH),
        stop: assets.load(STOP_PATH),
        idle: assets.load(IDLE_PATH),
        rotate_left: assets.load(ROTATE_LEFT_PATH),
        rotate_right: assets.load(ROTATE_RIGHT_PATH),
        turn: assets.load(TURN_PATH),
        lean: assets.load(LEAN_PATH),
        outer_blend: assets.load(OUTER_BLEND_PATH),
        outer_rotate: assets.load(OUTER_ROTATE_PATH),
        stop_blend: assets.load(STOP_BLEND_PATH),
        quick_feet: assets.load(QUICK_FEET_PATH),
        inertial: assets.load(INERTIAL_PATH),
        wrappers: WRAPPER_PATHS.map(|path| assets.load(path)),
        skeleton: assets.load(SKELETON_PATH),
    });
    let timeout_secs = std::env::var("CROUCH_GATE_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(45);
    commands.insert_resource(Deadline(Instant::now() + Duration::from_secs(timeout_secs)));
    info!("[CROUCHING-CONTRACT] loading linked crouching layer assets");
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!("[CROUCHING-CONTRACT][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(code).unwrap()));
}

fn node<'a, T: 'static>(graph: &'a AnimationGraph, name: &str) -> Option<&'a T> {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<T>())
}

fn state_id(
    machine: &StateMachine,
    label: &str,
) -> Option<bevy_animation_graph::core::state_machine::high_level::StateId> {
    machine
        .states
        .values()
        .find(|state| state.label == label)
        .map(|state| state.id)
}

fn labels_match(machine: &StateMachine, expected: &[&str]) -> bool {
    let mut actual: Vec<_> = machine
        .states
        .values()
        .map(|state| state.label.as_str())
        .collect();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    actual == expected
}

fn has_transition(
    machine: &StateMachine,
    source: &str,
    target: &str,
    duration: Option<f32>,
) -> bool {
    let (Some(source), Some(target)) = (state_id(machine, source), state_id(machine, target))
    else {
        return false;
    };
    machine.transitions.values().any(|transition| {
        transition.source == source
            && transition.target == target
            && match (&transition.data.kind, duration) {
                (TransitionKind::Immediate, None) => true,
                (
                    TransitionKind::Graph {
                        timed: Some(actual),
                        ..
                    },
                    Some(expected),
                ) => close(*actual, expected),
                _ => false,
            }
    })
}

fn source_files() -> Vec<String> {
    let directory = Path::new("assets/graphs/ab_als/stances/crouching");
    let mut paths = vec!["assets/graphs/ab_als/stances/crouching.animgraph.ron".to_owned()];
    if let Ok(entries) = std::fs::read_dir(directory) {
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

fn set_globals(player: &mut AnimationGraphPlayer) {
    for (key, value) in [
        ("als_crouching_state_index", 0.0),
        ("als_crouching_transition_index", -1.0),
        ("als_crouching_movement_state_index", 0.0),
        ("als_crouching_movement_transition_index", -1.0),
        ("als_crouching_hips_direction_lock_amount", 0.0),
        ("als_crouching_direction_index", 0.0),
        ("als_crouching_clip_index", 0.0),
        ("als_crouching_loop", 1.0),
        ("als_crouching_output_play_rate", 1.0),
        ("als_crouching_transition_duration", 0.0),
        ("als_crouching_curve_pose_crouching", 1.0),
        ("als_crouching_curve_pose_moving", 0.0),
        ("als_crouching_curve_rotation_yaw_offset", 0.0),
        ("als_crouching_curve_foot_left_lock", 1.0),
        ("als_crouching_curve_foot_right_lock", 1.0),
        ("als_crouching_curve_allow_transitions", 1.0),
        ("als_crouching_curve_rotation_yaw_speed", 0.0),
        ("als_crouching_velocity_forward_amount", 1.0),
        ("als_crouching_velocity_backward_amount", 0.0),
        ("als_crouching_velocity_right_amount", 0.0),
        ("als_crouching_velocity_left_amount", 0.0),
        ("als_crouching_stride_amount", 1.0),
        ("als_crouching_play_rate", 1.0),
        ("als_crouching_movement_marker_phase", 0.0),
        ("als_crouching_delta_seconds", 0.1),
        ("als_crouching_rotate_clip_time", 0.0),
        ("als_crouching_turn_index", 0.0),
        ("als_crouching_turn_weight", 0.0),
        ("als_crouching_turn_play_rate", 1.2),
        ("als_crouching_lean_right_amount", 0.0),
        ("als_crouching_lean_forward_amount", 0.0),
        ("als_crouching_transition_kind", 0.0),
        ("als_crouching_transition_slot_index", 0.0),
        ("als_crouching_transition_slot_weight", 0.0),
        ("als_crouching_transition_slot_play_rate", 1.0),
        ("als_crouching_transition_slot_start_time", 0.0),
    ] {
        player.set_global_input_data(key, value);
    }
}

fn set_masks(player: &mut AnimationGraphPlayer, skeleton: &Skeleton) {
    player.set_global_input_data(
        "als_crouching_stop_left_leg_mask",
        DataValue::BoneMask(build_branch_bone_mask(
            skeleton,
            &["thigh_l", "ik_foot_l", "VB foot_l"],
            1.0,
        )),
    );
    player.set_global_input_data(
        "als_crouching_stop_right_leg_mask",
        DataValue::BoneMask(build_branch_bone_mask(
            skeleton,
            &["thigh_r", "ik_foot_r", "VB foot_r"],
            1.0,
        )),
    );
    player.set_global_input_data(
        "als_crouching_stop_legs_mask",
        DataValue::BoneMask(build_branch_bone_mask(
            skeleton,
            &[
                "thigh_l",
                "ik_foot_l",
                "VB foot_l",
                "thigh_r",
                "ik_foot_r",
                "VB foot_r",
            ],
            1.0,
        )),
    );
}

fn verify_static(
    mut commands: Commands,
    server: Res<AssetServer>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    machines: Res<Assets<StateMachine>>,
    skeletons: Res<Assets<Skeleton>>,
    mut probe: ResMut<Probe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.static_verified {
        return;
    }
    let loaded = [
        server.is_loaded_with_dependencies(&assets.root),
        server.is_loaded_with_dependencies(&assets.outer),
        server.is_loaded_with_dependencies(&assets.movement),
        server.is_loaded_with_dependencies(&assets.move_graph),
        server.is_loaded_with_dependencies(&assets.blend),
        server.is_loaded_with_dependencies(&assets.direction),
        server.is_loaded_with_dependencies(&assets.stop),
        server.is_loaded_with_dependencies(&assets.idle),
        server.is_loaded_with_dependencies(&assets.rotate_left),
        server.is_loaded_with_dependencies(&assets.rotate_right),
        server.is_loaded_with_dependencies(&assets.turn),
        server.is_loaded_with_dependencies(&assets.lean),
        server.is_loaded_with_dependencies(&assets.outer_blend),
        server.is_loaded_with_dependencies(&assets.outer_rotate),
        server.is_loaded_with_dependencies(&assets.stop_blend),
        server.is_loaded_with_dependencies(&assets.quick_feet),
        server.is_loaded_with_dependencies(&assets.inertial),
        server.is_loaded_with_dependencies(&assets.skeleton),
    ]
    .into_iter()
    .all(|value| value)
        && assets
            .wrappers
            .iter()
            .all(|handle| server.is_loaded_with_dependencies(handle));
    if !loaded {
        return;
    }
    let (
        Some(root),
        Some(outer),
        Some(movement),
        Some(move_graph),
        Some(blend),
        Some(direction),
        Some(stop),
        Some(idle),
        Some(rotate_left),
        Some(rotate_right),
        Some(turn),
        Some(lean),
        Some(outer_blend),
        Some(outer_rotate),
        Some(stop_blend),
        Some(quick_feet),
        Some(inertial),
        Some(skeleton),
    ) = (
        graphs.get(&assets.root),
        machines.get(&assets.outer),
        machines.get(&assets.movement),
        graphs.get(&assets.move_graph),
        graphs.get(&assets.blend),
        graphs.get(&assets.direction),
        graphs.get(&assets.stop),
        graphs.get(&assets.idle),
        graphs.get(&assets.rotate_left),
        graphs.get(&assets.rotate_right),
        graphs.get(&assets.turn),
        graphs.get(&assets.lean),
        graphs.get(&assets.outer_blend),
        graphs.get(&assets.outer_rotate),
        graphs.get(&assets.stop_blend),
        graphs.get(&assets.quick_feet),
        graphs.get(&assets.inertial),
        skeletons.get(&assets.skeleton),
    )
    else {
        return;
    };
    let root_contract = node::<InterruptibleFsmNode>(root, "crouching states").is_some()
        && node::<DirectTransitionEventNode>(root, "crouching transition driver")
            .is_some_and(|node| node.transitions.len() == 12)
        && node::<LayeredGraphNode>(root, "crouching movement cache").is_some()
        && node::<LayeredGraphNode>(root, "crouching turn slot layer").is_some();
    let time_contract = outer.node_spec.has_input_time(&"movement_time".into())
        && stop
            .io_spec
            .has_input_time(&GraphInputPin::Passthrough("movement_time".into()))
        && root
            .nodes
            .values()
            .any(|node| node.name == "crouching movement clock loop")
        && root
            .nodes
            .values()
            .find(|node| node.name == "crouching states")
            .is_some_and(|node| {
                root.edges_inverted
                    .contains_key(&TargetPin::NodeTime(node.id, "movement_time".into()))
            });
    let outputs = [
        "pose",
        "crouching_state",
        "movement_state",
        "direction_index",
        "clip_index",
        "loop",
        "play_rate",
        "transition_duration",
        "PoseCrouching",
        "PoseMoving",
        "RotationYawOffset",
        "FootLeftLock",
        "FootRightLock",
        "AllowTransitions",
        "RotationYawSpeed",
        "transition_kind",
        "transition_index",
        "transition_weight",
        "transition_play_rate",
        "transition_start_time",
    ]
    .into_iter()
    .all(|name| root.io_spec.get_output_data(&name.to_owned()).is_some());
    let outer_contract = labels_match(
        outer,
        &["Idle", "Move", "Rotate Left", "Rotate Right", "Stop"],
    ) && outer.transitions.len() == 12
        && [
            ("Idle", "Move", Some(0.2)),
            ("Idle", "Rotate Right", Some(0.2)),
            ("Idle", "Rotate Left", Some(0.2)),
            ("Move", "Stop", Some(0.1)),
            ("Move", "Idle", Some(0.2)),
            ("Rotate Left", "Idle", Some(0.4)),
            ("Rotate Left", "Idle", None),
            ("Rotate Left", "Rotate Right", Some(0.2)),
            ("Rotate Right", "Idle", Some(0.4)),
            ("Rotate Right", "Idle", None),
            ("Rotate Right", "Rotate Left", Some(0.2)),
            ("Stop", "Idle", Some(0.5)),
        ]
        .into_iter()
        .all(|(source, target, duration)| has_transition(outer, source, target, duration));
    let movement_contract = labels_match(movement, &["Move Forward", "Move Backward", "Move Right Forward", "Move Right Backward", "Move Left Forward", "Move Left Backward"])
        && movement.transitions.len() == 24
        && movement.transitions.values().all(|transition| matches!(transition.data.kind, TransitionKind::Graph { timed: Some(value), .. } if close(value, 0.7)));
    let direction_contract = direction
        .nodes
        .values()
        .filter(|node| node.try_inner_downcast_ref::<LoopNode>().is_some())
        .count()
        == 6
        && direction
            .nodes
            .values()
            .filter(|node| node.try_inner_downcast_ref::<TimePositionNode>().is_some())
            .count()
            == 0
        && direction
            .nodes
            .values()
            .filter_map(|node| node.try_inner_downcast_ref::<SyncGroupSampleNode>())
            .all(|node| {
                node.looped
                    && close(node.leader_duration, 1.0)
                    && close(node.follower_duration, 1.133333)
                    && node.leader_markers.len() == 2
                    && node.follower_markers.len() == 2
            })
        && direction
            .nodes
            .values()
            .filter_map(|node| node.try_inner_downcast_ref::<SyncGroupSampleNode>())
            .count()
            == 6
        && node::<GlobalF32InputNode>(direction, "crouching movement marker phase")
            .is_some_and(|node| node.key == "als_crouching_movement_marker_phase");
    let blend_contract = blend
        .nodes
        .values()
        .filter_map(|node| node.try_inner_downcast_ref::<LayeredGraphNode>())
        .count()
        == 4
        && blend.nodes.values().any(|node| {
            node.try_inner_downcast_ref::<MultiWayBlendNode>()
                .is_some_and(|node| node.pose_count == 4 && node.normalize_alpha)
        });
    let wrapper_contract = [
        [0.0, 3.0, 5.0, 1.0],
        [0.0, 3.0, 4.0, 2.0],
        [0.0, 3.0, 4.0, 1.0],
        [0.0, 3.0, 5.0, 2.0],
        [0.0, 3.0, 5.0, 2.0],
        [0.0, 3.0, 4.0, 1.0],
    ]
    .into_iter()
    .zip(assets.wrappers.iter())
    .all(|(expected, handle)| {
        graphs.get(handle).is_some_and(|graph| {
            let Some(layer) = graph.nodes.values().find_map(|node| {
                node.try_inner_downcast_ref::<LayeredGraphNode>()
                    .filter(|node| node.graph.id() == assets.blend.id())
                    .map(|_| node.id)
            }) else {
                return false;
            };
            [
                "forward_direction_index",
                "backward_direction_index",
                "left_direction_index",
                "right_direction_index",
            ]
            .into_iter()
            .zip(expected)
            .all(|(pin, expected)| {
                graph
                    .edges_inverted
                    .get(&TargetPin::NodeData(layer, pin.into()))
                    .and_then(|source| match source {
                        SourcePin::NodeData(node, value) if value == "value" => graph
                            .nodes
                            .get(node)
                            .and_then(|node| node.try_inner_downcast_ref::<ConstantF32Node>())
                            .map(|node| node.value),
                        _ => None,
                    })
                    .is_some_and(|actual| close(actual, expected))
            })
        })
    });
    let move_contract = node::<InterruptibleFsmNode>(move_graph, "crouching movement states")
        .is_some()
        && node::<DirectTransitionEventNode>(move_graph, "crouching movement transition driver")
            .is_some_and(|node| node.transitions.len() == 24)
        && move_graph.nodes.values().any(|node| {
            node.try_inner_downcast_ref::<TimePositionNode>()
                .is_some_and(|node| !node.loop_position)
        })
        && move_graph.nodes.values().any(|node| {
            node.try_inner_downcast_ref::<ResetOnActivationNode>()
                .is_none()
        })
        && move_graph
            .nodes
            .values()
            .filter_map(|node| node.try_inner_downcast_ref::<als_ext::FInterpNode>())
            .any(|node| close(node.increasing_speed, 10.0) && close(node.decreasing_speed, 10.0));
    let stop_contract = stop
        .nodes
        .values()
        .filter_map(|node| node.try_inner_downcast_ref::<MeshSpaceLayeredBlendNode>())
        .count()
        == 1
        && stop.nodes.values().any(|node| {
            node.try_inner_downcast_ref::<GlobalBoneMaskInputNode>()
                .is_some_and(|node| node.key == "als_crouching_stop_legs_mask")
        });
    let idle_contract = idle
        .nodes
        .values()
        .filter_map(|node| node.try_inner_downcast_ref::<TimePositionNode>())
        .any(|node| !node.loop_position && close(node.duration_fallback, 0.033333335));
    let rotate_contract = [rotate_left, rotate_right].into_iter().all(|graph| {
        graph
            .nodes
            .values()
            .filter_map(|node| node.try_inner_downcast_ref::<TimePositionNode>())
            .any(|node| !node.loop_position && close(node.duration_fallback, 1.0))
    });
    let turn_contract = turn.nodes.values().any(|node| {
        node.try_inner_downcast_ref::<ResetOnActivationNode>()
            .is_some_and(|node| close(node.reset_time, 0.0))
    }) && turn.nodes.values().any(|node| {
        node.try_inner_downcast_ref::<GlobalF32InputNode>()
            .is_some_and(|node| node.key == "als_crouching_turn_weight")
    });
    let lean_contract = lean
        .nodes
        .values()
        .filter_map(|node| node.try_inner_downcast_ref::<BlendSpaceNode>())
        .count()
        == 1
        && lean
            .nodes
            .values()
            .any(|node| node.try_inner_downcast_ref::<F32ToVec2Node>().is_some());
    let transition_contract = [outer_blend, outer_rotate, stop_blend, quick_feet]
        .into_iter()
        .all(|graph| {
            node::<ProfiledBlendNode>(
                graph,
                &graph
                    .nodes
                    .values()
                    .find(|node| node.name.contains("transition"))
                    .map(|node| node.name.as_str())
                    .unwrap_or_default(),
            )
            .is_some()
        })
        && node::<InertializationNode>(inertial, "crouching inertial transition")
            .is_some_and(|node| close(node.transition_duration, 0.2));
    let source = source_files();
    let architecture = !source.is_empty()
        && source.iter().all(|text| {
            !text.contains("AB_Als_Monolithic")
                && !text.contains("locomotion_overlay")
                && !text.contains("manual")
                && !text.contains("playground")
        })
        && source
            .iter()
            .filter(|text| text.contains("als_ext::InterruptibleFsmNode"))
            .count()
            == 2;
    let pass = root_contract
        && time_contract
        && outputs
        && outer_contract
        && movement_contract
        && direction_contract
        && blend_contract
        && wrapper_contract
        && move_contract
        && stop_contract
        && idle_contract
        && rotate_contract
        && turn_contract
        && lean_contract
        && transition_contract
        && architecture;
    info!(
        "[CROUCHING-CONTRACT] root={} time={} outputs={} outer={} movement={} direction={} blend={} wrappers={} move={} stop={} idle={} rotate={} turn={} lean={} transitions={} architecture={}",
        root_contract,
        time_contract,
        outputs,
        outer_contract,
        movement_contract,
        direction_contract,
        blend_contract,
        wrapper_contract,
        move_contract,
        stop_contract,
        idle_contract,
        rotate_contract,
        turn_contract,
        lean_contract,
        transition_contract,
        architecture
    );
    if !pass {
        fail(&mut exit, 3, "static contract");
        return;
    }
    let mut outer_player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.root.clone());
    set_globals(&mut outer_player);
    set_masks(&mut outer_player, skeleton);
    outer_player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.outer_player = Some(
        commands
            .spawn((
                Name::new("crouching_outer_probe"),
                Children::default(),
                outer_player,
            ))
            .id(),
    );
    let mut movement_player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.move_graph.clone());
    set_globals(&mut movement_player);
    set_masks(&mut movement_player, skeleton);
    movement_player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.movement_player = Some(
        commands
            .spawn((
                Name::new("crouching_movement_probe"),
                Children::default(),
                movement_player,
            ))
            .id(),
    );
    let mut runtime_player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.root.clone());
    set_globals(&mut runtime_player);
    set_masks(&mut runtime_player, skeleton);
    runtime_player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.runtime_player = Some(
        commands
            .spawn((
                Name::new("crouching_runtime_probe"),
                Children::default(),
                runtime_player,
            ))
            .id(),
    );
    probe.static_verified = true;
    probe.phase = RuntimePhase::Outer;
    info!("[CROUCHING-CONTRACT] linked runtime probe initialized");
}

fn advance(player: &mut AnimationGraphPlayer, delta: f32) {
    player.queue_time_update(TimeUpdate::Delta(delta));
}

fn fsm_state<'a>(
    player: &'a AnimationGraphPlayer,
    graph: &AnimationGraph,
    name: &str,
) -> Option<&'a InterruptibleFsmState> {
    let id = graph.nodes.values().find(|node| node.name == name)?.id;
    player
        .get_context_arena()?
        .get_toplevel()
        .node_states
        .get::<InterruptibleFsmState>(id, StateKey::Default)
        .ok()
}

fn fsm_label(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    machine: &StateMachine,
    name: &str,
) -> Option<String> {
    match &fsm_state(player, graph, name)?.state {
        LowLevelStateId::HlState(id) => machine.states.get(id).map(|state| state.label.clone()),
        other => Some(format!("{other:?}")),
    }
}

fn drive_fsm(
    player: &mut AnimationGraphPlayer,
    graph: &AnimationGraph,
    machine: &StateMachine,
    fsm_name: &str,
    driver_name: &str,
    input_key: &str,
    steps: &[(f32, &str, &str)],
    step: &mut usize,
    pending: &mut bool,
    name: &str,
    exit: &mut MessageWriter<AppExit>,
) -> Option<bool> {
    if let Some(error) = player.get_error() {
        fail(exit, 4, &format!("{name} runtime error={error:?}"));
        return None;
    }
    let Some(state) = fsm_state(player, graph, fsm_name) else {
        fail(exit, 4, &format!("{name} state unavailable"));
        return None;
    };
    if *pending {
        let (index, _, target) = steps[*step];
        let expected = node::<DirectTransitionEventNode>(graph, driver_name)
            .and_then(|driver| driver.transitions.get(index as usize))
            .copied();
        let active = matches!(&state.state, LowLevelStateId::HlTransition(TransitionId::Direct(id)) if Some(id) == expected.as_ref());
        let immediate_target =
            fsm_label(player, graph, machine, fsm_name).as_deref() == Some(target);
        if !active && !immediate_target {
            fail(exit, 4, "direct transition did not activate");
            return None;
        }
        player.set_global_input_data(input_key, -1.0);
        player.seek(*step as f32 * 10.0 + 2.0);
        *pending = false;
        *step += 1;
        return Some(false);
    }
    if matches!(&state.state, LowLevelStateId::HlTransition(_)) {
        advance(player, 1.0);
        return Some(false);
    }
    if *step == steps.len() {
        info!(
            "[CROUCHING-CONTRACT] {name} direct transition topology={}/{}",
            steps.len(),
            steps.len()
        );
        return Some(true);
    }
    let (index, source, target) = steps[*step];
    if fsm_label(player, graph, machine, fsm_name).as_deref() != Some(source) {
        fail(exit, 4, "direct transition source state");
        return None;
    }
    player.set_global_input_data(input_key, index);
    player.seek(*step as f32 * 10.0 + 1.0);
    *pending = true;
    info!(
        "[CROUCHING-CONTRACT] {name} direct={} {source}->{target}",
        index
    );
    Some(false)
}

fn output(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_outputs().get(name) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    machines: Res<Assets<StateMachine>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !probe.static_verified {
        return;
    }
    let (Some(root), Some(outer), Some(move_graph), Some(movement)) = (
        graphs.get(&assets.root),
        machines.get(&assets.outer),
        graphs.get(&assets.move_graph),
        machines.get(&assets.movement),
    ) else {
        return;
    };
    match probe.phase {
        RuntimePhase::Outer => {
            let Some(entity) = probe.outer_player else {
                return;
            };
            let Ok(mut player) = players.get_mut(entity) else {
                return;
            };
            let steps = [
                (0.0, "Idle", "Move"),
                (3.0, "Move", "Stop"),
                (11.0, "Stop", "Idle"),
                (1.0, "Idle", "Rotate Right"),
                (8.0, "Rotate Right", "Idle"),
                (2.0, "Idle", "Rotate Left"),
                (5.0, "Rotate Left", "Idle"),
                (0.0, "Idle", "Move"),
                (4.0, "Move", "Idle"),
                (2.0, "Idle", "Rotate Left"),
                (7.0, "Rotate Left", "Rotate Right"),
                (10.0, "Rotate Right", "Rotate Left"),
                (6.0, "Rotate Left", "Idle"),
                (1.0, "Idle", "Rotate Right"),
                (9.0, "Rotate Right", "Idle"),
            ];
            let mut step = probe.outer_step;
            let mut pending = probe.outer_pending;
            let done = drive_fsm(
                &mut player,
                root,
                outer,
                "crouching states",
                "crouching transition driver",
                "als_crouching_transition_index",
                &steps,
                &mut step,
                &mut pending,
                "outer FSM",
                &mut exit,
            );
            probe.outer_step = step;
            probe.outer_pending = pending;
            if done == Some(true) {
                info!("[CROUCHING-CONTRACT] outer FSM source topology=12/12");
                probe.phase = RuntimePhase::Movement;
            }
        }
        RuntimePhase::Movement => {
            let Some(entity) = probe.movement_player else {
                return;
            };
            let Ok(mut player) = players.get_mut(entity) else {
                return;
            };
            let steps = [
                (0.0, "Move Forward", "Move Right Forward"),
                (6.0, "Move Right Forward", "Move Left Backward"),
                (19.0, "Move Left Backward", "Move Right Forward"),
                (7.0, "Move Right Forward", "Move Right Backward"),
                (10.0, "Move Right Backward", "Move Left Forward"),
                (15.0, "Move Left Forward", "Move Right Backward"),
                (11.0, "Move Right Backward", "Move Right Forward"),
                (8.0, "Move Right Forward", "Move Forward"),
                (1.0, "Move Forward", "Move Left Forward"),
                (16.0, "Move Left Forward", "Move Left Backward"),
                (20.0, "Move Left Backward", "Move Left Forward"),
                (17.0, "Move Left Forward", "Move Forward"),
                (2.0, "Move Forward", "Move Backward"),
                (3.0, "Move Backward", "Move Right Backward"),
                (12.0, "Move Right Backward", "Move Right Forward"),
                (9.0, "Move Right Forward", "Move Right Backward"),
                (13.0, "Move Right Backward", "Move Backward"),
                (4.0, "Move Backward", "Move Left Backward"),
                (21.0, "Move Left Backward", "Move Left Forward"),
                (18.0, "Move Left Forward", "Move Left Backward"),
                (22.0, "Move Left Backward", "Move Backward"),
                (5.0, "Move Backward", "Move Forward"),
                (0.0, "Move Forward", "Move Right Forward"),
                (7.0, "Move Right Forward", "Move Right Backward"),
                (14.0, "Move Right Backward", "Move Right Forward"),
                (6.0, "Move Right Forward", "Move Left Backward"),
                (23.0, "Move Left Backward", "Move Left Forward"),
            ];
            let mut step = probe.movement_step;
            let mut pending = probe.movement_pending;
            let done = drive_fsm(
                &mut player,
                move_graph,
                movement,
                "crouching movement states",
                "crouching movement transition driver",
                "als_crouching_movement_transition_index",
                &steps,
                &mut step,
                &mut pending,
                "movement FSM",
                &mut exit,
            );
            probe.movement_step = step;
            probe.movement_pending = pending;
            if done == Some(true) {
                info!("[CROUCHING-CONTRACT] movement FSM source topology=24/24");
                probe.phase = RuntimePhase::Idle;
            }
        }
        RuntimePhase::Idle => {
            let Some(entity) = probe.runtime_player else {
                return;
            };
            let Ok(mut player) = players.get_mut(entity) else {
                return;
            };
            let Some(pose) = player.get_default_output_pose().cloned() else {
                return;
            };
            let pass = close(output(&player, "crouching_state").unwrap_or(-1.0), 0.0)
                && close(output(&player, "PoseCrouching").unwrap_or(-1.0), 1.0)
                && close(output(&player, "FootLeftLock").unwrap_or(-1.0), 1.0);
            if !pass {
                fail(&mut exit, 5, "idle runtime probe");
                return;
            }
            info!(
                "[CROUCHING-CONTRACT] idle=PoseCrouch frame=0 evaluator_loop=true curves=FootLock/AllowTransitions"
            );
            probe.last_pose = Some(pose);
            player.set_global_input_data("als_crouching_transition_index", 0.0);
            player.set_global_input_data("als_crouching_state_index", 1.0);
            player.set_global_input_data("als_crouching_movement_state_index", 0.0);
            player.set_global_input_data("als_crouching_direction_index", 0.0);
            player.set_global_input_data("als_crouching_clip_index", 1.0);
            player.set_global_input_data("als_crouching_loop", 1.0);
            player.set_global_input_data("als_crouching_output_play_rate", 1.0);
            player.set_global_input_data("als_crouching_transition_duration", 0.2);
            player.set_global_input_data("als_crouching_curve_pose_moving", 1.0);
            advance(&mut player, 1.0);
            probe.phase = RuntimePhase::Move;
        }
        RuntimePhase::Move => {
            let Some(entity) = probe.runtime_player else {
                return;
            };
            let Ok(mut player) = players.get_mut(entity) else {
                return;
            };
            let Some(pose) = player.get_default_output_pose().cloned() else {
                return;
            };
            let changed = probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose);
            let pass = close(output(&player, "crouching_state").unwrap_or(-1.0), 1.0)
                && close(output(&player, "movement_state").unwrap_or(-1.0), 0.0)
                && close(output(&player, "clip_index").unwrap_or(-1.0), 1.0)
                && close(output(&player, "loop").unwrap_or(-1.0), 1.0)
                && close(output(&player, "play_rate").unwrap_or(-1.0), 1.0)
                && changed;
            if !pass {
                fail(&mut exit, 6, "six-way forward runtime probe");
                return;
            }
            info!(
                "[CROUCHING-CONTRACT] move=Forward clip=CrouchWalkForward loop=true rate=1.0 multiway=normalized"
            );
            probe.last_pose = Some(pose);
            player.set_global_input_data("als_crouching_transition_index", 1.0);
            player.set_global_input_data("als_crouching_state_index", 3.0);
            player.set_global_input_data("als_crouching_clip_index", 8.0);
            player.set_global_input_data("als_crouching_output_play_rate", 2.0);
            player.set_global_input_data("als_crouching_transition_duration", 0.2);
            player.set_global_input_data("als_crouching_rotate_clip_time", 0.5);
            advance(&mut player, 1.0);
            probe.phase = RuntimePhase::Rotate;
        }
        RuntimePhase::Rotate => {
            let Some(entity) = probe.runtime_player else {
                return;
            };
            let Ok(mut player) = players.get_mut(entity) else {
                return;
            };
            let Some(pose) = player.get_default_output_pose().cloned() else {
                return;
            };
            let changed = probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose);
            let pass = close(output(&player, "crouching_state").unwrap_or(-1.0), 3.0)
                && close(output(&player, "clip_index").unwrap_or(-1.0), 8.0)
                && close(output(&player, "loop").unwrap_or(-1.0), 1.0)
                && close(output(&player, "play_rate").unwrap_or(-1.0), 2.0)
                && changed;
            if !pass {
                fail(&mut exit, 7, "rotate runtime probe");
                return;
            }
            info!(
                "[CROUCHING-CONTRACT] rotate=Right clip=CrouchRotate90Right loop=true rate=2.0 curve=RotationYawSpeed"
            );
            probe.last_pose = Some(pose);
            player.set_global_input_data("als_crouching_transition_index", -1.0);
            player.set_global_input_data("als_crouching_state_index", 0.0);
            player.set_global_input_data("als_crouching_turn_weight", 1.0);
            player.set_global_input_data("als_crouching_turn_index", 3.0);
            player.set_global_input_data("als_crouching_turn_play_rate", 1.2);
            player.set_global_input_data("als_crouching_clip_index", 10.0);
            player.set_global_input_data("als_crouching_loop", 0.0);
            player.set_global_input_data("als_crouching_output_play_rate", 1.2);
            advance(&mut player, 1.0);
            probe.phase = RuntimePhase::Turn;
        }
        RuntimePhase::Turn => {
            let Some(entity) = probe.runtime_player else {
                return;
            };
            let Ok(mut player) = players.get_mut(entity) else {
                return;
            };
            let Some(pose) = player.get_default_output_pose().cloned() else {
                return;
            };
            let changed = probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose);
            let pass = close(output(&player, "clip_index").unwrap_or(-1.0), 10.0)
                && close(output(&player, "loop").unwrap_or(-1.0), 0.0)
                && close(output(&player, "play_rate").unwrap_or(-1.0), 1.2)
                && changed;
            if !pass {
                fail(&mut exit, 8, "crouch turn runtime probe");
                return;
            }
            info!(
                "[CROUCHING-CONTRACT] turn=Right180 clip=CrouchTurn180Right loop=false rate=1.2 slot=TurnInPlaceCrouching"
            );
            probe.last_pose = Some(pose);
            player.set_global_input_data("als_crouching_turn_weight", 0.0);
            player.set_global_input_data("als_crouching_transition_kind", 3.0);
            player.set_global_input_data("als_crouching_transition_slot_index", 1.0);
            player.set_global_input_data("als_crouching_transition_slot_weight", 1.0);
            player.set_global_input_data("als_crouching_transition_slot_play_rate", 1.5);
            player.set_global_input_data("als_crouching_transition_slot_start_time", 0.0);
            advance(&mut player, 1.0);
            probe.phase = RuntimePhase::Transition;
        }
        RuntimePhase::Transition => {
            let Some(entity) = probe.runtime_player else {
                return;
            };
            let Ok(player) = players.get_mut(entity) else {
                return;
            };
            let pass = close(output(&player, "transition_kind").unwrap_or(-1.0), 3.0)
                && close(output(&player, "transition_index").unwrap_or(-1.0), 1.0)
                && close(output(&player, "transition_weight").unwrap_or(-1.0), 1.0)
                && close(output(&player, "transition_play_rate").unwrap_or(-1.0), 1.5);
            if !pass {
                fail(&mut exit, 9, "transition slot signal runtime probe");
                return;
            }
            info!(
                "[CROUCHING-CONTRACT] dynamic_transition=Right clip=CrouchDynamicTransitionRight loop=false blend=0.2 rate=1.5 start=0"
            );
            info!("[CROUCHING-CONTRACT][VERDICT] linked=true -> PASS");
            exit.write(AppExit::Success);
        }
        RuntimePhase::Pending => {}
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        exit.write(AppExit::Error(std::num::NonZeroU8::new(10).unwrap()));
    }
}
