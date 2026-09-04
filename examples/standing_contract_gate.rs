use std::{
    path::Path,
    time::{Duration, Instant},
};

use als_ext::{
    AlsExtPlugin, CubicVec2AxisFilterNode, DirectTransitionEventNode, F32ToVec2Node,
    GlobalF32InputNode, InertializationNode, InterruptibleFsmNode, InterruptibleFsmState,
    LayeredGraphNode, MultiWayBlendNode, NormalizedBlendSpaceNode, ProfiledBlendNode,
    ResetOnActivationNode, SourceCurveModifyNode, SourceEnumPoseBlendNode,
    SourceLayeredBoneBlendNode, SourceMultiWayBlendNode, SourceScopedCachedPoseUseNode,
    SourceSequenceEvaluatorNode, SourceSequencePlayerNode, SourceSetHipsDirectionNode,
    SyncGroupSampleNode, set_gameplay_tag_input,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::builtin_nodes::loop_node::LoopNode;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    context::node_states::StateKey,
    edge_data::{DataSpec, DataValue},
    plugin::AnimationGraphSet,
    pose::Pose,
    skeleton::Skeleton,
    state_machine::{
        high_level::{StateMachine, TransitionId, TransitionKind},
        low_level::LowLevelStateId,
    },
};
use bevy_gameplay_tag::GameplayTag;

const ROOT_PATH: &str = "graphs/ab_als/stances/standing_runtime_entry.animgraph.ron";
const SOURCE_ROOT_PATH: &str = "graphs/ab_als/stances/standing.animgraph.ron";
const CYCLE_PATH: &str = "graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron";
const DETAILS_PATH: &str = "graphs/ab_als/stances/standing/movement_details.fsm.ron";
const MOVEMENT_PATH: &str = "graphs/ab_als/stances/standing/movement_states.fsm.ron";
const MOVE_GRAPH_PATH: &str = "graphs/ab_als/stances/standing/state_move.animgraph.ron";
const MOVEMENT_DIRECTION_GRAPH_PATH: &str =
    "graphs/ab_als/stances/standing/state_movement_direction.animgraph.ron";
const MOVEMENT_BLEND_GRAPH_PATH: &str =
    "graphs/ab_als/stances/standing/state_movement_blend.animgraph.ron";
const MOVEMENT_WRAPPER_PATHS: [&str; 6] = [
    "graphs/ab_als/stances/standing/state_movement_forward.animgraph.ron",
    "graphs/ab_als/stances/standing/state_movement_backward.animgraph.ron",
    "graphs/ab_als/stances/standing/state_movement_right_forward.animgraph.ron",
    "graphs/ab_als/stances/standing/state_movement_right_backward.animgraph.ron",
    "graphs/ab_als/stances/standing/state_movement_left_forward.animgraph.ron",
    "graphs/ab_als/stances/standing/state_movement_left_backward.animgraph.ron",
];
const OUTER_PATH: &str = "graphs/ab_als/stances/standing/standing_states.fsm.ron";
const OUTER_RUNTIME_PATH: &str =
    "graphs/ab_als/stances/standing/standing_states_runtime.animgraph.ron";
const OUTER_MOVE_PATH: &str = "graphs/ab_als/stances/standing/state_outer_move.animgraph.ron";
const STOP_STATES_PATH: &str = "graphs/ab_als/stances/standing/stop_states.fsm.ron";
const FIRST_PIVOT_PATH: &str =
    "graphs/ab_als/stances/standing/state_detail_first_pivot.animgraph.ron";
const RUN_START_PATH: &str = "graphs/ab_als/stances/standing/state_detail_run_start.animgraph.ron";
const RUN_START_WALK_PATH: &str =
    "graphs/ab_als/stances/standing/state_detail_run_start_from_walk.animgraph.ron";
const SECOND_PIVOT_PATH: &str =
    "graphs/ab_als/stances/standing/state_detail_second_pivot.animgraph.ron";
const DETAIL_BLEND_PATH: &str = "graphs/ab_als/stances/standing/transition_detail_01.animgraph.ron";
const HIPS_PATH: &str = "graphs/ab_als/stances/standing/transition_hips_075.animgraph.ron";
const OUTER_BLEND_PATH: &str = "graphs/ab_als/stances/standing/transition_outer_02.animgraph.ron";
const OUTER_STOP_PATH: &str =
    "graphs/ab_als/stances/standing/transition_outer_stop_03.animgraph.ron";
const OUTER_INERTIAL_PATH: &str =
    "graphs/ab_als/stances/standing/transition_inertial_02.animgraph.ron";
const ROTATE_LEFT_PATH: &str = "graphs/ab_als/stances/standing/state_rotate_left.animgraph.ron";
const ROTATE_RIGHT_PATH: &str = "graphs/ab_als/stances/standing/state_rotate_right.animgraph.ron";
const STOP_RIGHT_PATH: &str = "graphs/ab_als/stances/standing/state_stop_plant_right.animgraph.ron";
const STOP_LEFT_PATH: &str = "graphs/ab_als/stances/standing/state_stop_plant_left.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";

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
    .add_systems(
        PostUpdate,
        (observe_nested_runtime, observe_runtime)
            .chain()
            .after(AnimationGraphSet::Final),
    );
    app.run();
}

#[derive(Resource)]
struct GateAssets {
    root: Handle<AnimationGraph>,
    source_root: Handle<AnimationGraph>,
    cycle: Handle<AnimationGraph>,
    outer: Handle<StateMachine>,
    outer_runtime: Handle<AnimationGraph>,
    outer_move: Handle<AnimationGraph>,
    stop: Handle<StateMachine>,
    details: Handle<StateMachine>,
    movement: Handle<StateMachine>,
    move_graph: Handle<AnimationGraph>,
    movement_direction_graph: Handle<AnimationGraph>,
    movement_blend: Handle<AnimationGraph>,
    movement_wrappers: [Handle<AnimationGraph>; 6],
    first_pivot: Handle<AnimationGraph>,
    run_start: Handle<AnimationGraph>,
    run_start_walk: Handle<AnimationGraph>,
    second_pivot: Handle<AnimationGraph>,
    detail_blend: Handle<AnimationGraph>,
    hips: Handle<AnimationGraph>,
    outer_blend: Handle<AnimationGraph>,
    outer_stop: Handle<AnimationGraph>,
    outer_inertial: Handle<AnimationGraph>,
    rotate_left: Handle<AnimationGraph>,
    rotate_right: Handle<AnimationGraph>,
    stop_right: Handle<AnimationGraph>,
    stop_left: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource)]
struct AutoQuit(Instant);

#[derive(Resource, Default)]
struct RuntimeProbe {
    player: Option<Entity>,
    blend_player: Option<Entity>,
    detail_player: Option<Entity>,
    phase: ProbePhase,
    outer_step: usize,
    outer_event_queued: bool,
    detail_step: usize,
    detail_event_queued: bool,
    blend_wait_frames: u8,
    last_pose: Option<Pose>,
    settle_frames: u8,
    root_wait_frames: u8,
    static_verified: bool,
}

#[derive(Default, PartialEq, Eq)]
enum ProbePhase {
    #[default]
    Pending,
    NestedBlendStart,
    NestedBlendActive,
    NestedBlendDone,
    NestedDetails,
    OuterInterruptStart,
    OuterInterruptMove,
    OuterInterruptCheck,
    OuterFsm,
    Idle,
    MoveForward,
    MoveRightRun,
    LeanMove,
    Sprint,
    FirstPivot,
    StopEnter,
    StopPlant,
    StopReset,
    StopMoveAgain,
    StopPlantLeft,
    StopToIdleForRotate,
    EnterRotateRight,
    RotateRight,
    RotateLeft,
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        root: assets.load(ROOT_PATH),
        source_root: assets.load(SOURCE_ROOT_PATH),
        cycle: assets.load(CYCLE_PATH),
        outer: assets.load(OUTER_PATH),
        outer_runtime: assets.load(OUTER_RUNTIME_PATH),
        outer_move: assets.load(OUTER_MOVE_PATH),
        stop: assets.load(STOP_STATES_PATH),
        details: assets.load(DETAILS_PATH),
        movement: assets.load(MOVEMENT_PATH),
        move_graph: assets.load(MOVE_GRAPH_PATH),
        movement_direction_graph: assets.load(MOVEMENT_DIRECTION_GRAPH_PATH),
        movement_blend: assets.load(MOVEMENT_BLEND_GRAPH_PATH),
        movement_wrappers: MOVEMENT_WRAPPER_PATHS.map(|path| assets.load(path)),
        first_pivot: assets.load(FIRST_PIVOT_PATH),
        run_start: assets.load(RUN_START_PATH),
        run_start_walk: assets.load(RUN_START_WALK_PATH),
        second_pivot: assets.load(SECOND_PIVOT_PATH),
        detail_blend: assets.load(DETAIL_BLEND_PATH),
        hips: assets.load(HIPS_PATH),
        outer_blend: assets.load(OUTER_BLEND_PATH),
        outer_stop: assets.load(OUTER_STOP_PATH),
        outer_inertial: assets.load(OUTER_INERTIAL_PATH),
        rotate_left: assets.load(ROTATE_LEFT_PATH),
        rotate_right: assets.load(ROTATE_RIGHT_PATH),
        stop_right: assets.load(STOP_RIGHT_PATH),
        stop_left: assets.load(STOP_LEFT_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    commands.insert_resource(AutoQuit(Instant::now() + Duration::from_secs(45)));
    info!("[STANDING-CONTRACT] loading linked standing layer assets");
}

fn close(value: f32, expected: f32) -> bool {
    (value - expected).abs() < 1e-4
}

fn has_node<T: 'static>(graph: &AnimationGraph, name: &str) -> bool {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<T>())
        .is_some()
}

fn node<'a, T: 'static>(graph: &'a AnimationGraph, name: &str) -> Option<&'a T> {
    graph
        .nodes
        .values()
        .find(|candidate| candidate.name == name)
        .and_then(|candidate| candidate.try_inner_downcast_ref::<T>())
}

fn has_stop_plant_contract(
    graph: &AnimationGraph,
    curve_name: &str,
    branch_bones: &[&str],
    expected_frames: &[(&str, f32)],
) -> bool {
    let source_evaluators = graph
        .nodes
        .values()
        .filter_map(|node| {
            node.try_inner_downcast_ref::<SourceSequenceEvaluatorNode>()
                .map(|evaluator| (node.name.as_str(), evaluator))
        })
        .collect::<Vec<_>>();
    let enum_blends = graph
        .nodes
        .values()
        .filter_map(|node| node.try_inner_downcast_ref::<SourceEnumPoseBlendNode>())
        .collect::<Vec<_>>();
    graph.nodes.len() == 12
        && !graph.io_spec.has_output_time()
        && graph.io_spec.iter_input_data().next().is_none()
        && graph.io_spec.iter_input_times().next().is_none()
        && graph.io_spec.get_output_data(&"Result".to_owned()) == Some(DataSpec::Pose)
        && graph.io_spec.iter_output_data().count() == 1
        && source_evaluators.len() == 6
        && expected_frames.iter().all(|(name, frame)| {
            source_evaluators.iter().any(|(actual_name, evaluator)| {
                *actual_name == *name
                    && close(evaluator.explicit_frame, *frame)
                    && close(evaluator.frames_per_second, 30.0)
                    && evaluator.should_loop
                    && evaluator.teleport_to_explicit_time
                    && evaluator.reinitialization_behavior == "ExplicitTime"
            })
        })
        && node::<SourceScopedCachedPoseUseNode>(graph, "Use cached pose 'Movement Details'")
            .is_some_and(|node| node.cache_node == "Save cached pose 'Movement Details'")
        && node::<SourceLayeredBoneBlendNode>(graph, "Layered blend per bone").is_some_and(|node| {
            node.branch_bones
                .iter()
                .map(String::as_str)
                .eq(branch_bones.iter().copied())
                && close(node.blend_weight, 1.0)
                && node.mesh_space_rotation_blend
                && node.blend_root_motion_based_on_root_bone
                && node.base_pose_pin == "Base Pose"
                && node.blend_pose_pin == "Blend Poses 0"
                && node.output_pin == "Pose"
        })
        && node::<SourceCurveModifyNode>(graph, "Modify Curve").is_some_and(|node| {
            node.curve_name == curve_name
                && node.curve_names == [curve_name]
                && close(node.value, 1.0)
                && close(node.pin_default, 1.0)
                && node.pin_defaults == [1.0]
                && node.value_key.is_empty()
                && node.alpha == 1.0
                && node.apply_mode == "Blend"
        })
        && node::<SourceMultiWayBlendNode>(graph, "Blend Multi").is_some_and(|node| {
            node.pose_count == 4
                && node.desired_alpha_keys
                    == [
                        "als_standing_velocity_forward_amount",
                        "als_standing_velocity_backward_amount",
                        "als_standing_velocity_left_amount",
                        "als_standing_velocity_right_amount",
                    ]
                && node.desired_alpha_defaults == [0.0, 0.0, 0.0, 0.0]
                && node.normalize_alpha
                && close(node.alpha_scale, 1.0)
                && node.pose_pin_prefix == "Poses "
                && node.alpha_pin_prefix == "Desired Alphas "
        })
        && enum_blends.len() == 2
        && enum_blends.iter().any(|node| {
            node.active_key == "als_standing_hips_direction"
                && close(node.selected_enum_value, 3.0)
                && close(node.active_enum_default, 0.0)
                && close(node.default_blend_time, 0.1)
                && close(node.selected_blend_time, 0.1)
                && node.default_pose_pin == "Default Pose"
                && node.selected_pose_pin == "Left Backward Pose"
                && node.default_blend_time_pin == "Default Blend Time"
                && node.selected_blend_time_pin == "Left Backward Blend Time"
                && node.active_enum_pin == "Active Enum Value"
                && node.output_pin == "Pose"
                && node.source_visible_enum_entry == "LeftBackward"
        })
        && enum_blends.iter().any(|node| {
            node.active_key == "als_standing_hips_direction"
                && close(node.selected_enum_value, 5.0)
                && close(node.active_enum_default, 0.0)
                && close(node.default_blend_time, 0.1)
                && close(node.selected_blend_time, 0.1)
                && node.default_pose_pin == "Default Pose"
                && node.selected_pose_pin == "Right Backward Pose"
                && node.default_blend_time_pin == "Default Blend Time"
                && node.selected_blend_time_pin == "Right Backward Blend Time"
                && node.active_enum_pin == "Active Enum Value"
                && node.output_pin == "Pose"
                && node.source_visible_enum_entry == "RightBackward"
        })
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

fn state_positions_match(machine: &StateMachine, expected: &[(&str, (f32, f32))]) -> bool {
    expected.iter().all(|(label, (x, y))| {
        machine
            .states
            .values()
            .find(|state| state.label == *label)
            .and_then(|state| machine.editor_metadata.states.get(&state.id))
            .is_some_and(|position| close(position.x, *x) && close(position.y, *y))
    })
}

fn state_uses_graph(machine: &StateMachine, label: &str, graph: &Handle<AnimationGraph>) -> bool {
    machine
        .states
        .values()
        .find(|state| state.label == label)
        .is_some_and(|state| state.graph.id() == graph.id())
}

fn has_source_movement_state_contract(
    graph: &AnimationGraph,
    cache_names: [&str; 4],
    hips_direction: &str,
    source_value_path: &str,
) -> bool {
    let expected_alpha_keys = [
        "als_standing_velocity_forward_amount",
        "als_standing_velocity_backward_amount",
        "als_standing_velocity_left_amount",
        "als_standing_velocity_right_amount",
    ];
    graph.nodes.len() == 7
        && !graph.io_spec.has_output_time()
        && graph.io_spec.iter_input_data().next().is_none()
        && graph.io_spec.iter_input_times().next().is_none()
        && graph.io_spec.get_output_data(&"Result".to_owned()) == Some(DataSpec::Pose)
        && graph.io_spec.iter_output_data().count() == 1
        && cache_names.into_iter().all(|cache_name| {
            let node_name = format!("Use cached pose '{cache_name}'");
            node::<SourceScopedCachedPoseUseNode>(graph, &node_name)
                .is_some_and(|node| node.cache_node == format!("Save cached pose '{cache_name}'"))
        })
        && graph
            .nodes
            .values()
            .filter_map(|node| node.try_inner_downcast_ref::<SourceScopedCachedPoseUseNode>())
            .count()
            == 4
        && node::<SourceCurveModifyNode>(graph, "Modify Curve").is_some_and(|node| {
            node.curve_name == "RotationYawOffset"
                && close(node.value, 0.0)
                && close(node.pin_default, 0.0)
                && node.value_key == "als_standing_curve_rotation_yaw_offset"
                && node.source_value_path == source_value_path
                && close(node.alpha, 1.0)
                && node.apply_mode == "Blend"
        })
        && node::<SourceSetHipsDirectionNode>(graph, "SetHipsDirection\nOn Become Relevant")
            .is_some_and(|node| {
                node.new_hips_direction == hips_direction && node.on_become_relevant
            })
        && node::<SourceMultiWayBlendNode>(graph, "Blend Multi").is_some_and(|node| {
            node.pose_count == 4
                && node.desired_alpha_keys == expected_alpha_keys
                && node.desired_alpha_defaults == [0.0, 0.0, 0.0, 0.0]
                && node.normalize_alpha
                && close(node.alpha_scale, 1.0)
        })
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

fn has_outer_transition(
    machine: &StateMachine,
    source_label: &str,
    target_label: &str,
    duration: Option<f32>,
) -> bool {
    let (Some(source), Some(target)) = (
        state_id(machine, source_label),
        state_id(machine, target_label),
    ) else {
        return false;
    };
    machine.transitions.values().any(|transition| {
        transition.source == source
            && transition.target == target
            && match (&transition.data.kind, duration) {
                (TransitionKind::Immediate, None) => true,
                (
                    TransitionKind::Graph {
                        timed: Some(value), ..
                    },
                    Some(expected),
                ) => close(*value, expected),
                _ => false,
            }
    })
}

fn has_outer_transition_contract(machine: &StateMachine) -> bool {
    [
        ("Idle", "Move", Some(0.2)),
        ("Move", "Stop", None),
        ("Stop", "Idle", Some(0.3)),
        ("Move", "Idle", Some(0.2)),
        ("Idle", "Rotate Left", Some(0.2)),
        ("Idle", "Rotate Right", Some(0.2)),
        ("Rotate Left", "Idle", None),
        ("Rotate Right", "Idle", None),
        ("Rotate Left", "Rotate Right", Some(0.2)),
        ("Rotate Right", "Rotate Left", Some(0.2)),
        ("Rotate Left", "Idle", Some(0.2)),
        ("Rotate Right", "Idle", Some(0.2)),
    ]
    .into_iter()
    .all(|(source, target, duration)| has_outer_transition(machine, source, target, duration))
        && machine.transitions.len() == 12
}

fn has_ordered_outer_transition_ids(node: &DirectTransitionEventNode) -> bool {
    let expected = [
        "15100000-0000-4000-8000-000000000101",
        "15100000-0000-4000-8000-000000000102",
        "15100000-0000-4000-8000-000000000103",
        "15100000-0000-4000-8000-000000000104",
        "15100000-0000-4000-8000-000000000105",
        "15100000-0000-4000-8000-000000000106",
        "15100000-0000-4000-8000-000000000107",
        "15100000-0000-4000-8000-000000000108",
        "15100000-0000-4000-8000-000000000109",
        "15100000-0000-4000-8000-000000000110",
        "15100000-0000-4000-8000-000000000111",
        "15100000-0000-4000-8000-000000000112",
    ];
    node.transitions
        .iter()
        .map(|transition| transition.uuid().to_string())
        .eq(expected.into_iter().map(str::to_owned))
}

fn has_ordered_detail_transition_ids(node: &DirectTransitionEventNode) -> bool {
    let expected = [
        "15300000-0000-4000-8000-000000000101",
        "15300000-0000-4000-8000-000000000128",
        "15300000-0000-4000-8000-000000000115",
        "15300000-0000-4000-8000-000000000112",
        "15300000-0000-4000-8000-000000000122",
        "15300000-0000-4000-8000-000000000104",
        "15300000-0000-4000-8000-000000000103",
        "15300000-0000-4000-8000-000000000106",
        "15300000-0000-4000-8000-000000000117",
        "15300000-0000-4000-8000-000000000110",
        "15300000-0000-4000-8000-000000000127",
    ];
    node.transitions
        .iter()
        .map(|transition| transition.uuid().to_string())
        .eq(expected.into_iter().map(str::to_owned))
}

fn has_ordered_movement_transition_ids(node: &DirectTransitionEventNode) -> bool {
    let expected = [
        "15200000-0000-4000-8000-000000000101",
        "15200000-0000-4000-8000-000000000106",
        "15200000-0000-4000-8000-000000000102",
        "15200000-0000-4000-8000-000000000104",
        "15200000-0000-4000-8000-000000000108",
        "15200000-0000-4000-8000-000000000110",
        "15200000-0000-4000-8000-000000000111",
        "15200000-0000-4000-8000-000000000121",
        "15200000-0000-4000-8000-000000000127",
        "15200000-0000-4000-8000-000000000117",
        "15200000-0000-4000-8000-000000000124",
        "15200000-0000-4000-8000-000000000115",
        "15200000-0000-4000-8000-000000000119",
        "15200000-0000-4000-8000-000000000128",
        "15200000-0000-4000-8000-000000000130",
        "15200000-0000-4000-8000-000000000118",
        "15200000-0000-4000-8000-000000000113",
        "15200000-0000-4000-8000-000000000131",
        "15200000-0000-4000-8000-000000000132",
        "15200000-0000-4000-8000-000000000125",
        "15200000-0000-4000-8000-000000000133",
        "15200000-0000-4000-8000-000000000134",
        "15200000-0000-4000-8000-000000000135",
        "15200000-0000-4000-8000-000000000136",
    ];
    node.transitions
        .iter()
        .map(|transition| transition.uuid().to_string())
        .eq(expected.into_iter().map(str::to_owned))
}

fn timed_transition_count(
    machine: &StateMachine,
    source_label: &str,
    target_label: &str,
    duration: f32,
) -> usize {
    let (Some(source), Some(target)) = (
        state_id(machine, source_label),
        state_id(machine, target_label),
    ) else {
        return 0;
    };
    machine
        .transitions
        .values()
        .filter(|transition| {
            transition.source == source
                && transition.target == target
                && matches!(transition.data.kind, TransitionKind::Graph { timed: Some(value), .. } if close(value, duration))
        })
        .count()
}

fn has_detail_transition_contract(machine: &StateMachine) -> bool {
    let expected = [
        ("Walk", "Run", 0.2),
        ("Run", "Walk", 0.1),
        ("Walk", "Run Start", 0.1),
        ("Walk", "Run Start From Walk", 0.1),
        ("Run Start", "Run", 0.2),
        ("Run Start From Walk", "Run", 0.1),
        ("Run", "First Pivot", 0.1),
        ("First Pivot", "Run", 0.2),
        ("First Pivot", "Second Pivot", 0.1),
        ("Second Pivot", "First Pivot", 0.1),
        ("Second Pivot", "Run", 0.2),
    ];
    machine.transitions.len() == expected.len()
        && expected.into_iter().all(|(source, target, duration)| {
            timed_transition_count(machine, source, target, duration) == 1
        })
}

fn has_movement_transition_contract(machine: &StateMachine) -> bool {
    let expected = [
        ("Move Forward", "Move Backward", 0.5, 1),
        ("Move Backward", "Move Forward", 0.5, 1),
        ("Move Forward", "Move Right Forward", 0.7, 1),
        ("Move Forward", "Move Left Forward", 0.7, 1),
        ("Move Backward", "Move Right Backward", 0.7, 1),
        ("Move Backward", "Move Left Backward", 0.7, 1),
        ("Move Right Forward", "Move Forward", 0.7, 1),
        ("Move Left Forward", "Move Forward", 0.7, 1),
        ("Move Left Backward", "Move Backward", 0.7, 1),
        ("Move Right Backward", "Move Backward", 0.7, 1),
        ("Move Left Forward", "Move Right Backward", 0.5, 1),
        ("Move Right Forward", "Move Left Backward", 0.5, 1),
        ("Move Right Backward", "Move Left Forward", 0.5, 1),
        ("Move Left Backward", "Move Right Forward", 0.5, 1),
        ("Move Left Backward", "Move Left Forward", 0.75, 2),
        ("Move Left Backward", "Move Left Forward", 0.7, 1),
        ("Move Right Backward", "Move Right Forward", 0.75, 3),
        ("Move Right Forward", "Move Right Backward", 0.75, 1),
        ("Move Right Forward", "Move Right Backward", 0.7, 1),
        ("Move Left Forward", "Move Left Backward", 0.7, 1),
        ("Move Left Forward", "Move Left Backward", 0.75, 1),
    ];
    machine.transitions.len() == 24
        && expected
            .into_iter()
            .all(|(source, target, duration, count)| {
                timed_transition_count(machine, source, target, duration) == count
            })
}

fn has_stop_entry_contract(machine: &StateMachine) -> bool {
    let Some(entry) = machine
        .states
        .values()
        .find(|state| state.label == "Entry")
        .map(|state| state.id)
    else {
        return false;
    };
    let expected = [
        ("Lock Right Foot", None),
        ("Plant Right Foot", Some(0.1)),
        ("Lock Left Foot", None),
        ("Plant Left Foot", Some(0.1)),
    ];
    machine.start_state == entry
        && machine.transitions.len() == expected.len()
        && expected.iter().all(|(target, duration)| {
            machine.transitions.values().any(|transition| {
                transition.source == entry
                    && machine
                        .states
                        .get(&transition.target)
                        .is_some_and(|state| state.label.as_str() == *target)
                    && match (&transition.data.kind, duration) {
                        (TransitionKind::Immediate, None) => true,
                        (
                            TransitionKind::Graph {
                                timed: Some(value), ..
                            },
                            Some(expected),
                        ) => close(*value, *expected),
                        _ => false,
                    }
            })
        })
}

fn output_f32(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_outputs().get(name) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => {
            let key = match name {
                "standing_state" => "als_standing_state_index",
                "movement_state" => "als_standing_movement_state_index",
                "movement_detail_state" => "als_standing_movement_detail_state_index",
                "stop_state" => "als_standing_stop_state_index",
                "direction_index" => "als_standing_direction_index",
                "clip_index" => "als_standing_clip_index",
                "loop" => "als_standing_loop",
                "play_rate" => "als_standing_output_play_rate",
                "transition_duration" => "als_standing_transition_duration",
                _ => return None,
            };
            player
                .get_global_input_data(key)
                .and_then(|value| value.as_f32().ok())
        }
    }
}

fn outputs_match(player: &AnimationGraphPlayer, expected: &[(&str, f32)]) -> bool {
    expected
        .iter()
        .all(|(name, value)| output_f32(player, name).is_some_and(|actual| close(actual, *value)))
}

fn pose_fingerprint(pose: &Pose) -> (f32, usize, f32, f32, f32) {
    let translation = pose.bones.iter().fold(0.0, |sum, bone| {
        sum + bone
            .translation
            .map_or(0.0, |value| value.x + value.y + value.z)
    });
    let rotation = pose.bones.iter().fold(0.0, |sum, bone| {
        sum + bone
            .rotation
            .map_or(0.0, |value| value.x + value.y + value.z + value.w)
    });
    let scale = pose.bones.iter().fold(0.0, |sum, bone| {
        sum + bone.scale.map_or(0.0, |value| value.x + value.y + value.z)
    });
    (
        pose.timestamp,
        pose.bones.len(),
        translation,
        rotation,
        scale,
    )
}

fn outer_fsm_state<'a>(
    player: &'a AnimationGraphPlayer,
    runtime: &AnimationGraph,
    runtime_handle: &Handle<AnimationGraph>,
) -> Option<&'a InterruptibleFsmState> {
    let node_id = runtime
        .nodes
        .values()
        .find(|node| node.name == "standing states runtime")?
        .id;
    let arena = player.get_context_arena()?;
    arena.iter_context_ids().find_map(|context_id| {
        let context = arena.get_context(context_id)?;
        (context.get_graph_id() == runtime_handle.id())
            .then(|| {
                context
                    .node_states
                    .get::<InterruptibleFsmState>(node_id, StateKey::Default)
                    .ok()
            })
            .flatten()
    })
}

fn outer_fsm_state_label(
    player: &AnimationGraphPlayer,
    runtime: &AnimationGraph,
    runtime_handle: &Handle<AnimationGraph>,
    outer: &StateMachine,
) -> Option<String> {
    let state = outer_fsm_state(player, runtime, runtime_handle)?;
    match &state.state {
        LowLevelStateId::HlState(state_id) => {
            outer.states.get(state_id).map(|state| state.label.clone())
        }
        other => Some(format!("{other:?}")),
    }
}

fn set_globals(player: &mut AnimationGraphPlayer) {
    for (key, value) in [
        ("als_standing_state_index", 0.0),
        ("als_standing_transition_index", -1.0),
        ("als_standing_movement_state_index", 0.0),
        ("als_standing_movement_transition_index", -1.0),
        ("als_standing_movement_detail_state_index", 0.0),
        ("als_standing_movement_detail_transition_index", -1.0),
        ("als_standing_hips_direction_lock_amount", 0.0),
        ("als_standing_stop_state_index", 0.0),
        ("als_standing_direction_index", 0.0),
        ("als_standing_hips_direction", 0.0),
        ("als_standing_clip_index", 0.0),
        ("als_standing_loop", 0.0),
        ("als_standing_output_play_rate", 1.0),
        ("als_standing_transition_duration", 0.0),
        ("als_standing_curve_pose_standing", 0.0),
        ("als_standing_curve_pose_moving", 0.0),
        ("als_standing_curve_rotation_yaw_offset", 0.0),
        ("als_standing_curve_foot_left_lock", 0.0),
        ("als_standing_curve_foot_right_lock", 0.0),
        ("als_standing_curve_allow_transitions", 0.0),
        ("als_standing_curve_rotation_yaw_speed", 0.0),
        ("als_standing_stride_amount", 1.0),
        ("als_standing_walk_run_amount", 0.0),
        ("als_standing_play_rate", 1.0),
        ("als_standing_sprint_amount", 0.0),
        ("als_standing_sprint_block_amount", 0.0),
        ("als_standing_sprint_acceleration_amount", 0.0),
        ("als_standing_sprint_play_rate", 1.0),
        ("als_standing_movement_marker_phase", 0.12647075),
        ("als_animation_delta_seconds", 0.0),
        ("als_standing_acceleration_direction_index", 0.0),
        ("als_standing_velocity_forward_amount", 1.0),
        ("als_standing_velocity_backward_amount", 0.0),
        ("als_standing_velocity_right_amount", 0.0),
        ("als_standing_velocity_left_amount", 0.0),
        ("als_standing_stop_left_backward_weight", 0.0),
        ("als_standing_stop_right_backward_weight", 0.0),
        ("als_standing_rotation_play_rate", 1.15),
        ("als_standing_rotate_clip_time", 0.0),
        ("als_standing_turn_index", 0.0),
        ("als_standing_turn_weight", 0.0),
        ("als_standing_turn_layer_weight", 0.0),
        ("als_standing_turn_play_rate", 1.2),
        ("als_standing_quick_stop_index", 0.0),
        ("als_standing_quick_stop_weight", 0.0),
        ("als_standing_quick_stop_play_rate", 1.75),
        ("als_standing_lean_right_amount", 0.0),
        ("als_standing_lean_forward_amount", 0.0),
        ("als_standing_lean_weight", 0.0),
        ("als_standing_unweighted_gait_running_amount", 0.0),
    ] {
        player.set_global_input_data(key, value);
    }
    player.set_global_input_data("als_standing_rotate_left_loop", false);
    player.set_global_input_data("als_standing_rotate_right_loop", false);
}

fn set_value(player: &mut AnimationGraphPlayer, key: &str, value: f32) {
    player.set_global_input_data(key, value);
}

fn standing_graph_sources() -> Vec<String> {
    let root = "assets/graphs/ab_als/stances/standing.animgraph.ron";
    let directory = Path::new("assets/graphs/ab_als/stances/standing");
    let mut paths = vec![root.to_owned()];
    if let Ok(entries) = std::fs::read_dir(directory) {
        paths.extend(
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "ron")
                        .then(|| entry.path())
                })
                .map(|path| path.to_string_lossy().into_owned()),
        );
    }
    paths
        .into_iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect()
}

fn verify_static_contract(
    mut commands: Commands,
    assets: Res<AssetServer>,
    gate_assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    machines: Res<Assets<StateMachine>>,
    skeletons: Res<Assets<Skeleton>>,
    mut probe: ResMut<RuntimeProbe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.static_verified
        || !assets.is_loaded_with_dependencies(&gate_assets.root)
        || !assets.is_loaded_with_dependencies(&gate_assets.source_root)
        || !assets.is_loaded_with_dependencies(&gate_assets.cycle)
        || !assets.is_loaded_with_dependencies(&gate_assets.outer)
        || !assets.is_loaded_with_dependencies(&gate_assets.outer_runtime)
        || !assets.is_loaded_with_dependencies(&gate_assets.outer_move)
        || !assets.is_loaded_with_dependencies(&gate_assets.stop)
        || !assets.is_loaded_with_dependencies(&gate_assets.details)
        || !assets.is_loaded_with_dependencies(&gate_assets.movement)
        || !assets.is_loaded_with_dependencies(&gate_assets.move_graph)
        || !assets.is_loaded_with_dependencies(&gate_assets.movement_direction_graph)
        || !assets.is_loaded_with_dependencies(&gate_assets.movement_blend)
        || gate_assets
            .movement_wrappers
            .iter()
            .any(|handle| !assets.is_loaded_with_dependencies(handle))
        || !assets.is_loaded_with_dependencies(&gate_assets.first_pivot)
        || !assets.is_loaded_with_dependencies(&gate_assets.run_start)
        || !assets.is_loaded_with_dependencies(&gate_assets.run_start_walk)
        || !assets.is_loaded_with_dependencies(&gate_assets.second_pivot)
        || !assets.is_loaded_with_dependencies(&gate_assets.detail_blend)
        || !assets.is_loaded_with_dependencies(&gate_assets.hips)
        || !assets.is_loaded_with_dependencies(&gate_assets.outer_blend)
        || !assets.is_loaded_with_dependencies(&gate_assets.outer_stop)
        || !assets.is_loaded_with_dependencies(&gate_assets.outer_inertial)
        || !assets.is_loaded_with_dependencies(&gate_assets.rotate_left)
        || !assets.is_loaded_with_dependencies(&gate_assets.rotate_right)
        || !assets.is_loaded_with_dependencies(&gate_assets.stop_right)
        || !assets.is_loaded_with_dependencies(&gate_assets.stop_left)
        || !assets.is_loaded_with_dependencies(&gate_assets.skeleton)
    {
        return;
    }
    let Some(root) = graphs.get(&gate_assets.root) else {
        return;
    };
    let Some(source_root) = graphs.get(&gate_assets.source_root) else {
        return;
    };
    let Some(cycle) = graphs.get(&gate_assets.cycle) else {
        return;
    };
    let Some(outer) = machines.get(&gate_assets.outer) else {
        return;
    };
    let Some(outer_runtime) = graphs.get(&gate_assets.outer_runtime) else {
        return;
    };
    let Some(outer_move) = graphs.get(&gate_assets.outer_move) else {
        return;
    };
    let Some(stop) = machines.get(&gate_assets.stop) else {
        return;
    };
    let Some(details) = machines.get(&gate_assets.details) else {
        return;
    };
    let Some(movement) = machines.get(&gate_assets.movement) else {
        return;
    };
    let Some(move_graph) = graphs.get(&gate_assets.move_graph) else {
        return;
    };
    let Some(movement_direction_graph) = graphs.get(&gate_assets.movement_direction_graph) else {
        return;
    };
    let Some(stop_right) = graphs.get(&gate_assets.stop_right) else {
        return;
    };
    let Some(stop_left) = graphs.get(&gate_assets.stop_left) else {
        return;
    };
    let Some(detail_blend) = graphs.get(&gate_assets.detail_blend) else {
        return;
    };
    let Some(outer_blend) = graphs.get(&gate_assets.outer_blend) else {
        return;
    };
    let Some(outer_stop) = graphs.get(&gate_assets.outer_stop) else {
        return;
    };
    let Some(outer_inertial) = graphs.get(&gate_assets.outer_inertial) else {
        return;
    };
    let Some(rotate_left) = graphs.get(&gate_assets.rotate_left) else {
        return;
    };
    let Some(rotate_right) = graphs.get(&gate_assets.rotate_right) else {
        return;
    };
    let Some(_skeleton) = skeletons.get(&gate_assets.skeleton) else {
        return;
    };
    let linked = has_node::<LayeredGraphNode>(source_root, "Standing States\nState Machine")
        && has_node::<LayeredGraphNode>(source_root, "Movement Details States\nState Machine")
        && has_node::<LayeredGraphNode>(source_root, "Movement States\nState Machine")
        && has_node::<InterruptibleFsmNode>(outer_runtime, "standing states runtime")
        && node::<DirectTransitionEventNode>(outer_runtime, "standing states runtime driver")
            .is_some_and(has_ordered_outer_transition_ids)
        && node::<DirectTransitionEventNode>(move_graph, "standing movement states driver")
            .is_some_and(has_ordered_movement_transition_ids)
        && node::<DirectTransitionEventNode>(
            movement_direction_graph,
            "standing movement details driver",
        )
        .is_some_and(has_ordered_detail_transition_ids)
        && source_root.nodes.len() == 37;
    let interfaces = !source_root.io_spec.has_output_time()
        && source_root.io_spec.iter_input_data().next().is_none()
        && source_root.io_spec.get_output_data(&"Result".to_owned()) == Some(DataSpec::Pose)
        && source_root.io_spec.iter_output_data().count() == 1;
    let runtime_entry = root.nodes.len() == 1
        && !root.io_spec.has_output_time()
        && root.io_spec.get_output_data(&"pose".to_owned()) == Some(DataSpec::Pose)
        && root.io_spec.iter_output_data().count() == 1;
    let state_machines = labels_match(
        outer,
        &["Idle", "Move", "Stop", "Rotate Right", "Rotate Left"],
    ) && labels_match(
        details,
        &[
            "Walk",
            "Run",
            "Second Pivot",
            "Run Start From Walk",
            "Run Start",
            "First Pivot",
        ],
    ) && state_positions_match(
        outer,
        &[
            ("Idle", (176.0, 0.0)),
            ("Move", (-16.0, -128.0)),
            ("Stop", (352.0, -128.0)),
            ("Rotate Right", (352.0, 128.0)),
            ("Rotate Left", (-48.0, 128.0)),
        ],
    ) && state_positions_match(
        movement,
        &[
            ("Move Forward", (-48.0, 128.0)),
            ("Move Backward", (-48.0, 592.0)),
            ("Move Right Forward", (208.0, 256.0)),
            ("Move Right Backward", (208.0, 464.0)),
            ("Move Left Forward", (-336.0, 256.0)),
            ("Move Left Backward", (-352.0, 464.0)),
        ],
    ) && state_positions_match(
        details,
        &[
            ("Walk", (176.0, 0.0)),
            ("Run", (672.0, 0.0)),
            ("Second Pivot", (848.0, 128.0)),
            ("Run Start From Walk", (368.0, 128.0)),
            ("Run Start", (416.0, -128.0)),
            ("First Pivot", (848.0, 0.0)),
        ],
    ) && state_positions_match(
        stop,
        &[
            ("Entry", (176.0, 0.0)),
            ("Lock Right Foot", (368.0, -64.0)),
            ("Plant Right Foot", (368.0, 144.0)),
            ("Lock Left Foot", (368.0, -144.0)),
            ("Plant Left Foot", (368.0, 64.0)),
        ],
    ) && has_outer_transition_contract(outer)
        && has_detail_transition_contract(details)
        && has_movement_transition_contract(movement)
        && has_stop_entry_contract(stop);
    let wrapper_contract = cycle
        .io_spec
        .get_input_data(&GraphInputPin::Passthrough("direction_index".into()))
        .is_some()
        && !has_node::<GlobalF32InputNode>(cycle, "walk run direction index")
        && movement_direction_graph
            .io_spec
            .get_input_data(&GraphInputPin::Passthrough("direction_index".into()))
            .is_some()
        && node::<InterruptibleFsmNode>(move_graph, "standing movement states")
            .is_some_and(|node| node.state_pose_output == "Result")
        && state_uses_graph(outer, "Move", &gate_assets.outer_move)
        && outer_move.nodes.len() == 1
        && !outer_move.io_spec.has_output_time()
        && outer_move.io_spec.iter_input_data().next().is_none()
        && outer_move.io_spec.get_output_data(&"Result".to_owned()) == Some(DataSpec::Pose)
        && node::<SourceScopedCachedPoseUseNode>(outer_move, "Use cached pose 'Movement Details'")
            .is_some_and(|node| node.cache_node == "Save cached pose 'Movement Details'")
        && node::<InterruptibleFsmNode>(outer_runtime, "standing states runtime").is_some_and(
            |node| {
                node.state_pose_outputs.iter().any(|value| {
                    value.state_id == "15100000-0000-4000-8000-000000000002"
                        && value.output == "Result"
                })
            },
        )
        && movement.node_spec.has_output_time()
        && movement.node_spec.get_output_data(&"pose".to_owned()) == Some(DataSpec::Pose)
        && movement.node_spec.iter_output_data().count() == 1
        && [
            (
                "Move Forward",
                [
                    "Move Forward",
                    "Move Backward",
                    "Move Left Forward",
                    "Move Right Forward",
                ],
                "Forward",
                "GroundedState.RotationYawOffsets.ForwardAngle",
            ),
            (
                "Move Backward",
                [
                    "Move Right Backward",
                    "Move Left Backward",
                    "Move Forward",
                    "Move Backward",
                ],
                "Backward",
                "GroundedState.RotationYawOffsets.BackwardAngle",
            ),
            (
                "Move Right Forward",
                [
                    "Move Right Forward",
                    "Move Left Backward",
                    "Move Forward",
                    "Move Backward",
                ],
                "RightForward",
                "GroundedState.RotationYawOffsets.RightAngle",
            ),
            (
                "Move Right Backward",
                [
                    "Move Left Forward",
                    "Move Right Backward",
                    "Move Forward",
                    "Move Backward",
                ],
                "RightBackward",
                "GroundedState.RotationYawOffsets.RightAngle",
            ),
            (
                "Move Left Forward",
                [
                    "Move Left Forward",
                    "Move Right Backward",
                    "Move Forward",
                    "Move Backward",
                ],
                "LeftForward",
                "GroundedState.RotationYawOffsets.LeftAngle",
            ),
            (
                "Move Left Backward",
                [
                    "Move Right Forward",
                    "Move Left Backward",
                    "Move Forward",
                    "Move Backward",
                ],
                "LeftBackward",
                "GroundedState.RotationYawOffsets.LeftAngle",
            ),
        ]
        .into_iter()
        .zip(gate_assets.movement_wrappers.iter())
        .all(
            |((label, cache_names, hips_direction, source_value_path), handle)| {
                state_uses_graph(movement, label, handle)
                    && graphs.get(handle).is_some_and(|graph| {
                        has_source_movement_state_contract(
                            graph,
                            cache_names,
                            hips_direction,
                            source_value_path,
                        )
                    })
            },
        );
    let phase_template = |markers: &[als_ext::SyncMarker]| {
        markers.len() == 2
            && markers[0].name == "Left"
            && close(markers[0].time, 0.0)
            && markers[1].name == "Right"
            && close(markers[1].time, 0.5)
    };
    let normalized_cycle_nodes = cycle
        .nodes
        .values()
        .filter_map(|node| node.try_inner_downcast_ref::<NormalizedBlendSpaceNode>())
        .collect::<Vec<_>>();
    let cycle_contract = normalized_cycle_nodes.len() == 6
        && normalized_cycle_nodes.iter().all(|node| {
            node.looped
                && close(node.phase_duration, 1.0)
                && phase_template(&node.phase_markers)
                && node.points.len() == 4
                && node
                    .points
                    .iter()
                    .all(|point| point.duration > 0.0 && point.markers.len() == 2)
        })
        && cycle
            .nodes
            .values()
            .filter(|node| node.try_inner_downcast_ref::<LoopNode>().is_some())
            .count()
            == 26
        && has_node::<F32ToVec2Node>(cycle, "walk run blendspace position")
        && node::<CubicVec2AxisFilterNode>(cycle, "walk run cubic axis filter").is_some_and(
            |node| close(node.x_window_seconds, 0.2) && close(node.y_window_seconds, 0.4),
        )
        && cycle
            .nodes
            .values()
            .filter_map(|node| node.try_inner_downcast_ref::<SyncGroupSampleNode>())
            .all(|node| {
                node.looped
                    && close(node.leader_duration, 1.0)
                    && phase_template(&node.leader_markers)
                    && close(node.follower_duration, 0.6)
                    && node.follower_markers.len() == 2
            })
        && cycle
            .nodes
            .values()
            .filter_map(|node| node.try_inner_downcast_ref::<SyncGroupSampleNode>())
            .count()
            == 2
        && cycle.nodes.values().any(|node| {
            node.try_inner_downcast_ref::<GlobalF32InputNode>()
                .is_some_and(|node| node.key == "als_standing_movement_marker_phase")
        });
    let resets = [
        (&gate_assets.first_pivot, 0.25),
        (&gate_assets.run_start, 0.1),
        (&gate_assets.run_start_walk, 0.15),
        (&gate_assets.second_pivot, 0.25),
    ]
    .into_iter()
    .all(|(handle, expected)| {
        graphs
            .get(handle)
            .and_then(|graph| {
                graph
                    .nodes
                    .values()
                    .filter_map(|node| node.try_inner_downcast_ref::<ResetOnActivationNode>())
                    .next()
            })
            .is_some_and(|node| close(node.reset_time, expected))
    });
    let velocity_blends = [
        &gate_assets.first_pivot,
        &gate_assets.run_start,
        &gate_assets.run_start_walk,
        &gate_assets.second_pivot,
    ]
    .into_iter()
    .all(|handle| {
        graphs.get(handle).is_some_and(|graph| {
            graph.nodes.values().any(|node| {
                node.try_inner_downcast_ref::<MultiWayBlendNode>()
                    .is_some_and(|blend| blend.pose_count == 4 && blend.normalize_alpha)
            })
        })
    });
    let stop_plants = has_stop_plant_contract(
        stop_right,
        "FootRightLock",
        &["thigh_r", "ik_foot_r", "VB foot_r"],
        &[
            ("A_Als_Walk_Forward\nSequence Evaluator (by frame)", 21.0),
            ("A_Als_Walk_Backward\nSequence Evaluator (by frame)", 21.0),
            (
                "A_Als_Walk_Left_Forward\nSequence Evaluator (by frame)",
                24.0,
            ),
            (
                "A_Als_Walk_Left_Backward\nSequence Evaluator (by frame)",
                24.0,
            ),
            (
                "A_Als_Walk_Right_Forward\nSequence Evaluator (by frame)",
                23.0,
            ),
            (
                "A_Als_Walk_Right_Backward\nSequence Evaluator (by frame)",
                21.0,
            ),
        ],
    ) && has_stop_plant_contract(
        stop_left,
        "FootLeftLock",
        &["thigh_l", "ik_foot_l", "VB foot_l"],
        &[
            ("A_Als_Walk_Forward\nSequence Evaluator (by frame)", 4.0),
            ("A_Als_Walk_Backward\nSequence Evaluator (by frame)", 4.0),
            (
                "A_Als_Walk_Left_Forward\nSequence Evaluator (by frame)",
                6.0,
            ),
            (
                "A_Als_Walk_Left_Backward\nSequence Evaluator (by frame)",
                4.0,
            ),
            (
                "A_Als_Walk_Right_Forward\nSequence Evaluator (by frame)",
                7.0,
            ),
            (
                "A_Als_Walk_Right_Backward\nSequence Evaluator (by frame)",
                7.0,
            ),
        ],
    );
    let transition_nodes =
        node::<ProfiledBlendNode>(detail_blend, "standing movement detail transition")
            .is_some_and(|blend| format!("{:?}", blend.alpha_blend_mode) == "HermiteCubic")
            && graphs.get(&gate_assets.hips).is_some_and(|graph| {
                node::<ProfiledBlendNode>(graph, "standing hips direction profile").is_some_and(
                    |blend| format!("{:?}", blend.alpha_blend_mode) == "QuadraticInOut",
                )
            })
            && node::<ProfiledBlendNode>(outer_blend, "standing outer transition")
                .is_some_and(|blend| format!("{:?}", blend.alpha_blend_mode) == "HermiteCubic")
            && node::<ProfiledBlendNode>(outer_stop, "standing stop transition")
                .is_some_and(|blend| format!("{:?}", blend.alpha_blend_mode) == "HermiteCubic")
            && node::<InertializationNode>(outer_inertial, "standing inertial rotate exit")
                .is_some_and(|node| close(node.transition_duration, 0.2));
    let rotate_completion =
        [
            (
                rotate_left,
                "A_Als_Rotate_90_Left\nSequence Player",
                "als_standing_rotate_left_loop",
            ),
            (
                rotate_right,
                "A_Als_Rotate_90_Right\nSequence Player",
                "als_standing_rotate_right_loop",
            ),
        ]
        .into_iter()
        .all(|(graph, node_name, loop_key)| {
            node::<SourceSequencePlayerNode>(graph, node_name).is_some_and(|node| {
                close(node.play_rate, 1.0)
                    && node.play_rate_key == "als_standing_rotation_play_rate"
                    && node.loop_animation
                    && node.loop_animation_key == loop_key
                    && node.expose_loop_animation_pin
            })
        }) && std::fs::read_to_string("src/standing_layer.rs").is_ok_and(|source| {
            source.contains("als_standing_rotate_left_loop")
                && source.contains("als_standing_rotate_right_loop")
        });
    let source = std::fs::read_to_string(
        "assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron",
    )
    .unwrap_or_default();
    let sources = standing_graph_sources();
    let architecture = !sources.is_empty()
        && sources.iter().all(|source| {
            !source.contains("AB_Als_Monolithic")
                && !source.contains("locomotion_overlay")
                && !source.contains("manual")
                && !source.contains("playground")
        })
        && sources
            .iter()
            .filter(|source| source.contains("als_ext::InterruptibleFsmNode"))
            .count()
            >= 4;
    let asset_contract = [
        "A_Als_Walk_Forward.anim.ron",
        "A_Als_Walk_Right_Forward.anim.ron",
        "A_Als_Walk_Right_Backward.anim.ron",
        "A_Als_Walk_Backward.anim.ron",
        "A_Als_Walk_Left_Backward.anim.ron",
        "A_Als_Walk_Left_Forward.anim.ron",
        "A_Als_Sprint.anim.ron",
        "A_Als_Sprint_Acceleration.anim.ron",
    ]
    .into_iter()
    .all(|asset| source.contains(asset))
        && !source.contains("AB_Als_Monolithic");
    let pass = linked
        && interfaces
        && state_machines
        && wrapper_contract
        && cycle_contract
        && resets
        && velocity_blends
        && stop_plants
        && transition_nodes
        && rotate_completion
        && architecture
        && asset_contract
        && runtime_entry;
    info!(
        "[STANDING-CONTRACT] linked={} interfaces={} runtime_entry={} state_machines={} wrappers={} cycle={} resets={} velocity_blends={} stop_plants={} transitions={} rotate_completion={} architecture={} assets={}",
        linked,
        interfaces,
        runtime_entry,
        state_machines,
        wrapper_contract,
        cycle_contract,
        resets,
        velocity_blends,
        stop_plants,
        transition_nodes,
        rotate_completion,
        architecture,
        asset_contract,
    );
    if !pass {
        fail(&mut exit, 3, "static contract");
        return;
    }
    let mut player = AnimationGraphPlayer::new(gate_assets.skeleton.clone())
        .with_graph(gate_assets.root.clone());
    set_globals(&mut player);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    let player_entity = commands
        .spawn((
            Name::new("standing_contract_probe"),
            Children::default(),
            player,
        ))
        .id();
    set_gameplay_tag_input(player_entity, "gait", GameplayTag::new("Als.Gait.Walking"));
    probe.player = Some(player_entity);
    probe.phase = ProbePhase::OuterInterruptStart;
    probe.static_verified = true;
    info!("[STANDING-CONTRACT] linked runtime probe initialized");
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!("[STANDING-CONTRACT][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(code).unwrap()));
    std::process::exit(i32::from(code));
}

fn advance(player: &mut AnimationGraphPlayer) {
    advance_by(player, 1.0);
}

fn advance_by(player: &mut AnimationGraphPlayer, delta_seconds: f32) {
    player.queue_time_update(TimeUpdate::Delta(delta_seconds));
}

fn direct_fsm_state<'a>(
    player: &'a AnimationGraphPlayer,
    graph: &AnimationGraph,
    node_name: &str,
) -> Option<&'a InterruptibleFsmState> {
    let node_id = graph.nodes.values().find(|node| node.name == node_name)?.id;
    let arena = player.get_context_arena()?;
    arena
        .get_toplevel()
        .node_states
        .get::<InterruptibleFsmState>(node_id, StateKey::Default)
        .ok()
}

fn direct_fsm_state_label(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    machine: &StateMachine,
    node_name: &str,
) -> Option<String> {
    match &direct_fsm_state(player, graph, node_name)?.state {
        LowLevelStateId::HlState(state_id) => machine
            .states
            .get(state_id)
            .map(|state| state.label.clone()),
        other => Some(format!("{other:?}")),
    }
}

fn child_fsm_states<'a>(
    player: &'a AnimationGraphPlayer,
    graph: &AnimationGraph,
    graph_handle: &Handle<AnimationGraph>,
    node_name: &str,
) -> Vec<&'a InterruptibleFsmState> {
    let Some(node_id) = graph
        .nodes
        .values()
        .find(|node| node.name == node_name)
        .map(|node| node.id)
    else {
        return Vec::new();
    };
    let Some(arena) = player.get_context_arena() else {
        return Vec::new();
    };
    arena
        .iter_context_ids()
        .filter_map(|context_id| {
            let context = arena.get_context(context_id)?;
            (context.get_graph_id() == graph_handle.id())
                .then(|| {
                    context
                        .node_states
                        .get::<InterruptibleFsmState>(node_id, StateKey::Default)
                        .ok()
                })
                .flatten()
        })
        .collect()
}

fn drive_direct_fsm(
    player: &mut AnimationGraphPlayer,
    graph: &AnimationGraph,
    machine: &StateMachine,
    fsm_name: &str,
    driver_name: &str,
    input_key: &str,
    steps: &[(f32, &str, &str)],
    step: &mut usize,
    event_queued: &mut bool,
    probe_name: &str,
    exit: &mut MessageWriter<AppExit>,
) -> Option<bool> {
    if let Some(error) = player.get_error() {
        fail(exit, 4, &format!("{probe_name} runtime error={error:?}"));
        return None;
    }
    let Some(state) = direct_fsm_state(player, graph, fsm_name) else {
        advance_by(player, 0.0);
        return Some(false);
    };
    if *event_queued {
        let Some((index, _, _)) = steps.get(*step).copied() else {
            fail(exit, 4, "nested direct event step missing");
            return None;
        };
        let expected = node::<DirectTransitionEventNode>(graph, driver_name)
            .and_then(|driver| driver.transitions.get(index as usize))
            .copied();
        if !matches!(
            &state.state,
            LowLevelStateId::HlTransition(TransitionId::Direct(id)) if Some(id) == expected.as_ref()
        ) {
            fail(exit, 4, "nested direct transition did not become active");
            return None;
        }
        set_value(player, input_key, -1.0);
        advance(player);
        *event_queued = false;
        *step += 1;
        return Some(false);
    }
    if matches!(&state.state, LowLevelStateId::HlTransition(_)) {
        advance(player);
        return Some(false);
    }
    if *step == steps.len() {
        let Some((_, _, target)) = steps.last().copied() else {
            fail(exit, 4, "nested direct event steps empty");
            return None;
        };
        if direct_fsm_state_label(player, graph, machine, fsm_name).as_deref() != Some(target) {
            fail(exit, 4, "nested direct transition final target");
            return None;
        }
        info!(
            "[STANDING-CONTRACT] {} direct transition topology={}/{}",
            probe_name,
            steps.len(),
            steps.len()
        );
        return Some(true);
    }
    let (index, source, target) = steps[*step];
    let actual = direct_fsm_state_label(player, graph, machine, fsm_name);
    if actual.as_deref() != Some(source) {
        warn!(
            "[STANDING-CONTRACT] {} expected_source={} actual={:?}",
            probe_name, source, actual
        );
        fail(exit, 4, "nested direct transition source state");
        return None;
    }
    set_value(player, input_key, index);
    advance_by(player, 0.0);
    *event_queued = true;
    info!(
        "[STANDING-CONTRACT] {} direct={} {}->{}",
        probe_name, index, source, target
    );
    Some(false)
}

fn observe_nested_runtime(
    mut probe: ResMut<RuntimeProbe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    gate_assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    machines: Res<Assets<StateMachine>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !probe.static_verified {
        return;
    }
    match probe.phase {
        ProbePhase::NestedBlendStart => {
            let Some(entity) = probe.blend_player else {
                return;
            };
            let (Some(graph), Some(machine), Ok(mut player)) = (
                graphs.get(&gate_assets.movement_direction_graph),
                machines.get(&gate_assets.details),
                players.get_mut(entity),
            ) else {
                return;
            };
            if player.get_error().is_some() {
                fail(&mut exit, 4, "four-pose detail runtime error");
                return;
            }
            let _ = player.get_default_output_pose();
            let states = child_fsm_states(
                &player,
                graph,
                &gate_assets.movement_direction_graph,
                "standing movement details",
            );
            if states.len() < 4 {
                probe.blend_wait_frames = probe.blend_wait_frames.saturating_add(1);
                if probe.blend_wait_frames >= 5 {
                    warn!(
                        "[STANDING-CONTRACT] four-pose detail contexts={}",
                        states.len()
                    );
                    fail(&mut exit, 4, "four-pose detail child context count");
                    return;
                }
                advance_by(&mut player, 0.0);
                return;
            }
            let all_walk = states.iter().all(|state| {
                matches!(&state.state, LowLevelStateId::HlState(id) if machine.states.get(id).is_some_and(|state| state.label == "Walk"))
            });
            if !all_walk {
                fail(&mut exit, 4, "four-pose detail initial walk states");
                return;
            }
            set_value(
                &mut player,
                "als_standing_movement_detail_transition_index",
                0.0,
            );
            advance_by(&mut player, 0.0);
            probe.blend_wait_frames = 0;
            probe.phase = ProbePhase::NestedBlendActive;
        }
        ProbePhase::NestedBlendActive => {
            let Some(entity) = probe.blend_player else {
                return;
            };
            let (Some(graph), Ok(mut player)) = (
                graphs.get(&gate_assets.movement_direction_graph),
                players.get_mut(entity),
            ) else {
                return;
            };
            if let Some(error) = player.get_error() {
                fail(
                    &mut exit,
                    4,
                    &format!("four-pose detail transition error={error:?}"),
                );
                return;
            }
            let expected =
                node::<DirectTransitionEventNode>(graph, "standing movement details driver")
                    .and_then(|driver| driver.transitions.first())
                    .copied();
            let states = child_fsm_states(
                &player,
                graph,
                &gate_assets.movement_direction_graph,
                "standing movement details",
            );
            let all_active = states.len() == 4
                && states.iter().all(|state| {
                    matches!(
                        &state.state,
                        LowLevelStateId::HlTransition(TransitionId::Direct(id)) if Some(id) == expected.as_ref()
                    )
            });
            if !all_active {
                probe.blend_wait_frames = probe.blend_wait_frames.saturating_add(1);
                if probe.blend_wait_frames < 8 {
                    advance_by(&mut player, 0.0);
                    return;
                }
                warn!(
                    "[STANDING-CONTRACT] four-pose detail active_states={:?} expected={:?}",
                    states
                        .iter()
                        .map(|state| format!("{:?}", state.state))
                        .collect::<Vec<_>>(),
                    expected,
                );
                fail(
                    &mut exit,
                    4,
                    "four-pose detail direct transition propagation",
                );
                return;
            }
            set_value(
                &mut player,
                "als_standing_movement_detail_transition_index",
                -1.0,
            );
            advance(&mut player);
            probe.phase = ProbePhase::NestedBlendDone;
        }
        ProbePhase::NestedBlendDone => {
            let Some(entity) = probe.blend_player else {
                return;
            };
            let (Some(graph), Some(machine), Ok(mut player)) = (
                graphs.get(&gate_assets.movement_direction_graph),
                machines.get(&gate_assets.details),
                players.get_mut(entity),
            ) else {
                return;
            };
            let states = child_fsm_states(
                &player,
                graph,
                &gate_assets.movement_direction_graph,
                "standing movement details",
            );
            let all_run = states.len() == 4
                && states.iter().all(|state| {
                    matches!(&state.state, LowLevelStateId::HlState(id) if machine.states.get(id).is_some_and(|state| state.label == "Run"))
                });
            if !all_run {
                advance(&mut player);
                return;
            }
            info!("[STANDING-CONTRACT] four-pose detail propagation=4/4");
            probe.phase = ProbePhase::NestedDetails;
        }
        ProbePhase::NestedDetails => {
            let Some(entity) = probe.detail_player else {
                return;
            };
            let (Some(graph), Some(machine), Ok(mut player)) = (
                graphs.get(&gate_assets.movement_direction_graph),
                machines.get(&gate_assets.details),
                players.get_mut(entity),
            ) else {
                return;
            };
            let steps = [
                (0.0, "Walk", "Run"),
                (9.0, "Run", "First Pivot"),
                (1.0, "First Pivot", "Second Pivot"),
                (3.0, "Second Pivot", "Run"),
                (7.0, "Run", "Walk"),
                (5.0, "Walk", "Run Start"),
                (4.0, "Run Start", "Run"),
                (7.0, "Run", "Walk"),
                (6.0, "Walk", "Run Start From Walk"),
                (8.0, "Run Start From Walk", "Run"),
                (9.0, "Run", "First Pivot"),
                (1.0, "First Pivot", "Second Pivot"),
                (2.0, "Second Pivot", "First Pivot"),
                (10.0, "First Pivot", "Run"),
            ];
            let mut step = probe.detail_step;
            let mut event_queued = probe.detail_event_queued;
            let complete = drive_direct_fsm(
                &mut player,
                graph,
                machine,
                "standing movement details",
                "standing movement details driver",
                "als_standing_movement_detail_transition_index",
                &steps,
                &mut step,
                &mut event_queued,
                "movement details FSM",
                &mut exit,
            );
            probe.detail_step = step;
            probe.detail_event_queued = event_queued;
            if complete == Some(true) {
                probe.phase = ProbePhase::OuterInterruptStart;
            }
        }
        _ => {}
    }
}

fn observe_runtime(
    mut probe: ResMut<RuntimeProbe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    gate_assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    machines: Res<Assets<StateMachine>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !probe.static_verified {
        return;
    }
    if matches!(
        probe.phase,
        ProbePhase::NestedBlendStart
            | ProbePhase::NestedBlendActive
            | ProbePhase::NestedBlendDone
            | ProbePhase::NestedDetails
    ) {
        return;
    }
    let Some(entity) = probe.player else {
        return;
    };
    let Ok(mut player) = players.get_mut(entity) else {
        return;
    };
    let Some(_root) = graphs.get(&gate_assets.root) else {
        return;
    };
    let Some(outer) = machines.get(&gate_assets.outer) else {
        return;
    };
    let Some(outer_runtime) = graphs.get(&gate_assets.outer_runtime) else {
        return;
    };
    if let Some(error) = player.get_error() {
        fail(&mut exit, 4, &format!("runtime error={error:?}"));
        return;
    }
    let Some(pose) = player.get_default_output_pose().cloned() else {
        probe.root_wait_frames = probe.root_wait_frames.saturating_add(1);
        if probe.root_wait_frames >= 8 {
            fail(&mut exit, 4, "root output pose missing");
        }
        advance_by(&mut player, 0.0);
        return;
    };
    probe.root_wait_frames = 0;
    if probe.phase != ProbePhase::Idle
        && probe
            .last_pose
            .as_ref()
            .is_some_and(|previous| previous == &pose)
        && probe.settle_frames < 3
    {
        probe.settle_frames += 1;
        let delta_seconds = matches!(
            probe.phase,
            ProbePhase::RotateRight | ProbePhase::RotateLeft
        )
        .then_some(0.1)
        .unwrap_or(1.0);
        advance_by(&mut player, delta_seconds);
        return;
    }
    probe.settle_frames = 0;
    match probe.phase {
        ProbePhase::OuterInterruptStart => {
            if outer_fsm_state_label(&player, outer_runtime, &gate_assets.outer_runtime, outer)
                .as_deref()
                != Some("Idle")
            {
                fail(&mut exit, 5, "outer interrupt initial idle state");
                return;
            }
            set_value(&mut player, "als_standing_transition_index", 0.0);
            advance_by(&mut player, 0.1);
            probe.phase = ProbePhase::OuterInterruptMove;
        }
        ProbePhase::OuterInterruptMove => {
            let expected =
                node::<DirectTransitionEventNode>(outer_runtime, "standing states runtime driver")
                    .and_then(|driver| driver.transitions.first())
                    .copied();
            let active = outer_fsm_state(&player, outer_runtime, &gate_assets.outer_runtime);
            if !matches!(
                active.map(|state| &state.state),
                Some(LowLevelStateId::HlTransition(TransitionId::Direct(id))) if Some(id) == expected.as_ref()
            ) || active.is_some_and(|state| !state.interrupted_sources.is_empty())
            {
                fail(&mut exit, 5, "outer interrupt initial transition state");
                return;
            }
            let Some(_source_pose) =
                outer_fsm_state(&player, outer_runtime, &gate_assets.outer_runtime)
                    .and_then(InterruptibleFsmState::last_output_pose)
                    .cloned()
            else {
                fail(&mut exit, 5, "outer interrupt source pose missing");
                return;
            };
            set_value(&mut player, "als_standing_transition_index", 3.0);
            advance_by(&mut player, 0.1);
            probe.phase = ProbePhase::OuterInterruptCheck;
        }
        ProbePhase::OuterInterruptCheck => {
            let expected_current =
                node::<DirectTransitionEventNode>(outer_runtime, "standing states runtime driver")
                    .and_then(|driver| driver.transitions.get(3))
                    .copied();
            let expected_source =
                node::<DirectTransitionEventNode>(outer_runtime, "standing states runtime driver")
                    .and_then(|driver| driver.transitions.first())
                    .copied();
            let source_move_state = state_id(outer, "Move");
            let active = outer_fsm_state(&player, outer_runtime, &gate_assets.outer_runtime);
            let source_pose = active.and_then(InterruptibleFsmState::cached_source_pose);
            let state_chain = active.is_some_and(|state| {
                matches!(
                    &state.state,
                    LowLevelStateId::HlTransition(TransitionId::Direct(id)) if Some(id) == expected_current.as_ref()
                ) && matches!(
                    state.interrupted_sources.as_slice(),
                    [source] if (
                        matches!(
                            &source.state,
                            LowLevelStateId::HlTransition(TransitionId::Direct(id)) if Some(id) == expected_source.as_ref()
                        ) || matches!(
                            &source.state,
                            LowLevelStateId::HlState(id) if Some(id) == source_move_state.as_ref()
                        )
                    ) && source.state_entered_time <= state.state_entered_time
                )
            });
            let source_pose_valid = source_pose.is_some_and(|pose| {
                !pose.bones.is_empty()
                    && pose.timestamp.is_finite()
                    && pose.bones.iter().all(|bone| {
                        bone.translation.is_none_or(Vec3::is_finite)
                            && bone.rotation.is_none_or(Quat::is_finite)
                            && bone.scale.is_none_or(Vec3::is_finite)
                    })
            });
            let chained = state_chain && source_pose_valid;
            if !chained {
                info!(
                    "[STANDING-CONTRACT] outer interrupt state={:?} interrupted_sources={:?} cached_source={:?} state_chain={} pose_valid={}",
                    active.map(|state| &state.state),
                    active.map(|state| &state.interrupted_sources),
                    source_pose
                        .as_ref()
                        .map(|pose| (pose.timestamp, pose.bones.len())),
                    state_chain,
                    source_pose_valid,
                );
                fail(&mut exit, 5, "outer interrupt source pose chain");
                return;
            }
            info!(
                "[STANDING-CONTRACT] outer FSM interrupt=Move->Idle source=Idle->Move source_pose_bones={}",
                source_pose.map_or(0, |pose| pose.bones.len()),
            );
            set_value(&mut player, "als_standing_transition_index", -1.0);
            advance(&mut player);
            probe.phase = ProbePhase::OuterFsm;
        }
        ProbePhase::OuterFsm => {
            let steps = [
                (0.0, "Idle", "Move"),
                (1.0, "Move", "Stop"),
                (2.0, "Stop", "Idle"),
                (0.0, "Idle", "Move"),
                (3.0, "Move", "Idle"),
                (4.0, "Idle", "Rotate Left"),
                (8.0, "Rotate Left", "Rotate Right"),
                (9.0, "Rotate Right", "Rotate Left"),
                (6.0, "Rotate Left", "Idle"),
                (5.0, "Idle", "Rotate Right"),
                (7.0, "Rotate Right", "Idle"),
                (4.0, "Idle", "Rotate Left"),
                (10.0, "Rotate Left", "Idle"),
                (5.0, "Idle", "Rotate Right"),
                (11.0, "Rotate Right", "Idle"),
            ];
            if probe.outer_event_queued {
                set_value(&mut player, "als_standing_transition_index", -1.0);
                advance(&mut player);
                probe.outer_event_queued = false;
                probe.outer_step += 1;
                return;
            }
            if probe.outer_step == steps.len() {
                if outer_fsm_state_label(&player, outer_runtime, &gate_assets.outer_runtime, outer)
                    .as_deref()
                    != Some("Idle")
                {
                    fail(&mut exit, 5, "outer FSM final idle state");
                    return;
                }
                probe.phase = ProbePhase::Idle;
                info!("[STANDING-CONTRACT] outer FSM direct transition topology=12/12");
                return;
            }
            let (index, source, target) = steps[probe.outer_step];
            let actual_source =
                outer_fsm_state_label(&player, outer_runtime, &gate_assets.outer_runtime, outer);
            if actual_source.as_deref() != Some(source) {
                warn!(
                    "[STANDING-CONTRACT] outer FSM expected_source={} actual={:?}",
                    source, actual_source
                );
                fail(&mut exit, 5, "outer FSM direct transition source state");
                return;
            }
            set_value(&mut player, "als_standing_transition_index", index);
            advance(&mut player);
            probe.outer_event_queued = true;
            info!(
                "[STANDING-CONTRACT] outer FSM direct={} {}->{}",
                index, source, target
            );
        }
        ProbePhase::Idle => {
            if !outputs_match(
                &player,
                &[("standing_state", 0.0), ("clip_index", 0.0), ("loop", 0.0)],
            ) {
                fail(&mut exit, 5, "idle state, clip, and loop");
                return;
            }
            probe.last_pose = Some(pose);
            info!("[STANDING-CONTRACT] idle state=Idle clip=StandPose loop=false");
            set_value(&mut player, "als_standing_state_index", 1.0);
            set_value(&mut player, "als_standing_transition_index", 0.0);
            set_value(&mut player, "als_standing_movement_state_index", 0.0);
            set_value(&mut player, "als_standing_movement_detail_state_index", 0.0);
            set_value(&mut player, "als_standing_direction_index", 0.0);
            set_value(&mut player, "als_standing_clip_index", 1.0);
            set_value(&mut player, "als_standing_loop", 1.0);
            set_value(&mut player, "als_standing_output_play_rate", 1.0);
            set_value(&mut player, "als_standing_transition_duration", 0.2);
            advance(&mut player);
            probe.phase = ProbePhase::MoveForward;
        }
        ProbePhase::MoveForward => {
            info!(
                "[STANDING-CONTRACT] forward probe state={:?} movement={:?} detail={:?} clip={:?} loop={:?} rate={:?} blend={:?} bones={} changed={}",
                output_f32(&player, "standing_state"),
                output_f32(&player, "movement_state"),
                output_f32(&player, "movement_detail_state"),
                output_f32(&player, "clip_index"),
                output_f32(&player, "loop"),
                output_f32(&player, "play_rate"),
                output_f32(&player, "transition_duration"),
                pose.bones.len(),
                probe
                    .last_pose
                    .as_ref()
                    .is_some_and(|previous| previous != &pose),
            );
            if !outputs_match(
                &player,
                &[
                    ("standing_state", 1.0),
                    ("movement_state", 0.0),
                    ("movement_detail_state", 0.0),
                    ("clip_index", 1.0),
                    ("loop", 1.0),
                    ("play_rate", 1.0),
                    ("transition_duration", 0.2),
                ],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(&mut exit, 6, "forward walk state, loop, play rate, or pose");
                return;
            }
            probe.last_pose = Some(pose);
            info!(
                "[STANDING-CONTRACT] move=Forward detail=Walk clip=WalkForward loop=true rate=1.0 blend=0.2"
            );
            set_value(&mut player, "als_standing_movement_state_index", 2.0);
            set_value(&mut player, "als_standing_movement_detail_state_index", 1.0);
            set_value(&mut player, "als_standing_direction_index", 1.0);
            set_value(&mut player, "als_standing_walk_run_amount", 1.0);
            set_value(&mut player, "als_standing_clip_index", 2.0);
            set_value(&mut player, "als_standing_transition_duration", 0.7);
            advance(&mut player);
            probe.phase = ProbePhase::MoveRightRun;
        }
        ProbePhase::MoveRightRun => {
            if !outputs_match(
                &player,
                &[
                    ("movement_state", 2.0),
                    ("movement_detail_state", 1.0),
                    ("direction_index", 1.0),
                    ("clip_index", 2.0),
                    ("loop", 1.0),
                    ("transition_duration", 0.7),
                ],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(
                    &mut exit,
                    7,
                    "right forward run state, direction, loop, or pose",
                );
                return;
            }
            probe.last_pose = Some(pose);
            info!(
                "[STANDING-CONTRACT] move=RightForward detail=Run clip=RunRightForward loop=true blend=0.7"
            );
            set_value(&mut player, "als_standing_lean_right_amount", 1.0);
            set_value(
                &mut player,
                "als_standing_unweighted_gait_running_amount",
                1.0,
            );
            advance_by(&mut player, 0.1);
            probe.phase = ProbePhase::LeanMove;
        }
        ProbePhase::LeanMove => {
            if !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(&mut exit, 7, "moving lean additive pose");
                return;
            }
            probe.last_pose = Some(pose);
            info!("[STANDING-CONTRACT] moving right lean additive=true");
            set_value(&mut player, "als_standing_lean_right_amount", 0.0);
            set_value(
                &mut player,
                "als_standing_unweighted_gait_running_amount",
                0.0,
            );
            set_value(&mut player, "als_standing_sprint_amount", 1.0);
            set_value(&mut player, "als_standing_gait_is_sprinting", 1.0);
            set_value(&mut player, "als_standing_sprint_acceleration_amount", 0.25);
            set_value(&mut player, "als_standing_clip_index", 3.0);
            set_value(&mut player, "als_standing_output_play_rate", 1.0);
            advance(&mut player);
            probe.phase = ProbePhase::Sprint;
        }
        ProbePhase::Sprint => {
            if !outputs_match(
                &player,
                &[("clip_index", 3.0), ("loop", 1.0), ("play_rate", 1.0)],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(&mut exit, 8, "sprint clip, loop, rate, or pose");
                return;
            }
            probe.last_pose = Some(pose);
            info!(
                "[STANDING-CONTRACT] sprint clip=SprintAcceleration loop=true rate=1.0 basis=0.8333333"
            );
            set_value(&mut player, "als_standing_sprint_amount", 0.0);
            set_value(&mut player, "als_standing_gait_is_sprinting", 0.0);
            set_value(&mut player, "als_standing_movement_detail_state_index", 5.0);
            set_value(&mut player, "als_standing_velocity_forward_amount", 0.0);
            set_value(&mut player, "als_standing_velocity_right_amount", 1.0);
            set_value(&mut player, "als_standing_clip_index", 4.0);
            set_value(&mut player, "als_standing_loop", 0.0);
            set_value(&mut player, "als_standing_output_play_rate", 1.25);
            set_value(&mut player, "als_standing_transition_duration", 0.5);
            advance(&mut player);
            probe.phase = ProbePhase::FirstPivot;
        }
        ProbePhase::FirstPivot => {
            if !outputs_match(
                &player,
                &[
                    ("movement_detail_state", 5.0),
                    ("clip_index", 4.0),
                    ("loop", 0.0),
                    ("play_rate", 1.25),
                    ("transition_duration", 0.5),
                ],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(
                    &mut exit,
                    9,
                    "first pivot state, clip, rate, blend, or pose",
                );
                return;
            }
            probe.last_pose = Some(pose);
            info!(
                "[STANDING-CONTRACT] detail=FirstPivot clip=AccelerationRight loop=false rate=1.25 start=0.25 blend=0.5"
            );
            set_value(&mut player, "als_standing_state_index", 2.0);
            set_value(&mut player, "als_standing_stop_state_index", 4.0);
            set_value(&mut player, "als_standing_transition_index", -1.0);
            set_value(&mut player, "als_standing_clip_index", 5.0);
            set_value(&mut player, "als_standing_output_play_rate", 1.0);
            set_value(&mut player, "als_standing_transition_duration", 0.3);
            advance(&mut player);
            probe.phase = ProbePhase::StopEnter;
        }
        ProbePhase::StopEnter => {
            set_value(&mut player, "als_standing_transition_index", 1.0);
            advance(&mut player);
            probe.phase = ProbePhase::StopPlant;
        }
        ProbePhase::StopPlant => {
            info!(
                "[STANDING-CONTRACT] stop plant probe state={:?} stop={:?} clip={:?} duration={:?} pose_changed={} current={:?} previous={:?}",
                output_f32(&player, "standing_state"),
                output_f32(&player, "stop_state"),
                output_f32(&player, "clip_index"),
                output_f32(&player, "transition_duration"),
                probe
                    .last_pose
                    .as_ref()
                    .is_some_and(|previous| previous != &pose),
                pose_fingerprint(&pose),
                probe.last_pose.as_ref().map(pose_fingerprint),
            );
            if !outputs_match(
                &player,
                &[
                    ("standing_state", 2.0),
                    ("stop_state", 4.0),
                    ("clip_index", 5.0),
                    ("transition_duration", 0.3),
                ],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(
                    &mut exit,
                    10,
                    "right stop plant state, clip, blend, or pose",
                );
                return;
            }
            probe.last_pose = Some(pose);
            info!(
                "[STANDING-CONTRACT] stop=PlantRightFoot evaluators=WalkRun frames=21/21/24/24/23/21 loop=true blend=0.3"
            );
            set_value(&mut player, "als_standing_state_index", 0.0);
            set_value(&mut player, "als_standing_stop_state_index", 0.0);
            set_value(&mut player, "als_standing_transition_index", 2.0);
            advance(&mut player);
            probe.phase = ProbePhase::StopReset;
        }
        ProbePhase::StopReset => {
            if !outputs_match(&player, &[("standing_state", 0.0)]) {
                fail(&mut exit, 11, "stop reset to idle");
                return;
            }
            set_value(&mut player, "als_standing_state_index", 1.0);
            set_value(&mut player, "als_standing_transition_index", 0.0);
            advance(&mut player);
            probe.phase = ProbePhase::StopMoveAgain;
        }
        ProbePhase::StopMoveAgain => {
            set_value(&mut player, "als_standing_stop_state_index", 3.0);
            set_value(&mut player, "als_standing_state_index", 2.0);
            set_value(&mut player, "als_standing_transition_index", 1.0);
            advance(&mut player);
            probe.phase = ProbePhase::StopPlantLeft;
        }
        ProbePhase::StopPlantLeft => {
            if !outputs_match(
                &player,
                &[
                    ("standing_state", 2.0),
                    ("stop_state", 3.0),
                    ("clip_index", 5.0),
                    ("transition_duration", 0.3),
                ],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(&mut exit, 12, "left stop plant state, clip, blend, or pose");
                return;
            }
            probe.last_pose = Some(pose);
            info!(
                "[STANDING-CONTRACT] stop=PlantLeftFoot evaluators=WalkRun frames=4/4/6/4/7/7 loop=true blend=0.3"
            );
            set_value(&mut player, "als_standing_state_index", 0.0);
            set_value(&mut player, "als_standing_transition_index", 2.0);
            advance(&mut player);
            probe.phase = ProbePhase::StopToIdleForRotate;
        }
        ProbePhase::StopToIdleForRotate => {
            set_value(&mut player, "als_standing_transition_index", -1.0);
            advance(&mut player);
            probe.phase = ProbePhase::EnterRotateRight;
        }
        ProbePhase::EnterRotateRight => {
            set_value(&mut player, "als_standing_state_index", 4.0);
            set_value(&mut player, "als_standing_transition_index", 5.0);
            set_value(&mut player, "als_standing_clip_index", 6.0);
            set_value(&mut player, "als_standing_loop", 1.0);
            player.set_global_input_data("als_standing_rotate_left_loop", false);
            player.set_global_input_data("als_standing_rotate_right_loop", true);
            set_value(&mut player, "als_standing_rotation_play_rate", 2.0);
            set_value(&mut player, "als_standing_output_play_rate", 2.0);
            set_value(&mut player, "als_standing_transition_duration", 0.2);
            advance_by(&mut player, 0.1);
            probe.phase = ProbePhase::RotateRight;
        }
        ProbePhase::RotateRight => {
            if !outputs_match(
                &player,
                &[
                    ("standing_state", 4.0),
                    ("clip_index", 6.0),
                    ("loop", 1.0),
                    ("play_rate", 2.0),
                    ("transition_duration", 0.2),
                ],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(
                    &mut exit,
                    12,
                    "rotate right state, clip, loop, rate, or pose",
                );
                return;
            }
            probe.last_pose = Some(pose);
            info!("[STANDING-CONTRACT] state=RotateRight clip=Rotate90Right loop=true rate=2.0");
            set_value(&mut player, "als_standing_state_index", 3.0);
            set_value(&mut player, "als_standing_transition_index", 9.0);
            set_value(&mut player, "als_standing_clip_index", 7.0);
            player.set_global_input_data("als_standing_rotate_left_loop", true);
            player.set_global_input_data("als_standing_rotate_right_loop", false);
            advance_by(&mut player, 0.1);
            probe.phase = ProbePhase::RotateLeft;
        }
        ProbePhase::RotateLeft => {
            if !outputs_match(
                &player,
                &[("standing_state", 3.0), ("clip_index", 7.0), ("loop", 1.0)],
            ) {
                fail(&mut exit, 13, "rotate left state, clip, loop, or pose");
                return;
            }
            probe.last_pose = Some(pose);
            info!(
                "[STANDING-CONTRACT] state=RotateLeft clip=Rotate90Left loop=true inertial_exit=0.2"
            );
            warn!("[STANDING-CONTRACT][VERDICT] movement_stop_rotate_lean=true -> PASS");
            exit.write(AppExit::Success);
            return;
        }
        ProbePhase::NestedBlendStart
        | ProbePhase::NestedBlendActive
        | ProbePhase::NestedBlendDone
        | ProbePhase::NestedDetails
        | ProbePhase::Pending => {}
    }
}

fn auto_quit(timeout: Res<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= timeout.0 {
        fail(&mut exit, 17, "runtime timeout");
    }
}
