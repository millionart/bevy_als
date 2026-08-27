use std::{
    path::Path,
    time::{Duration, Instant},
};

use als_ext::{
    AlsExtPlugin, ConstantF32Node, DirectTransitionEventNode, F32ToVec2Node,
    GlobalBoneMaskInputNode, GlobalF32InputNode, InertializationNode, InterruptibleFsmNode,
    InterruptibleFsmState, LayeredGraphNode, MeshSpaceLayeredBlendNode, MultiWayBlendNode,
    ProfiledBlendNode, ResetOnActivationNode, TimePositionNode, build_branch_bone_mask,
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

const ROOT_PATH: &str = "graphs/ab_als/stances/standing.animgraph.ron";
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
const STOP_LEFT_LEG_BRANCHES: &[&str] = &["thigh_l", "ik_foot_l", "VB foot_l"];
const STOP_RIGHT_LEG_BRANCHES: &[&str] = &["thigh_r", "ik_foot_r", "VB foot_r"];

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
    cycle: Handle<AnimationGraph>,
    outer: Handle<StateMachine>,
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
    movement_player: Option<Entity>,
    detail_player: Option<Entity>,
    phase: ProbePhase,
    outer_step: usize,
    outer_event_queued: bool,
    movement_step: usize,
    movement_event_queued: bool,
    detail_step: usize,
    detail_event_queued: bool,
    blend_wait_frames: u8,
    last_pose: Option<Pose>,
    interrupt_source_pose: Option<Pose>,
    settle_frames: u8,
    static_verified: bool,
}

#[derive(Default, PartialEq, Eq)]
enum ProbePhase {
    #[default]
    Pending,
    NestedBlendStart,
    NestedBlendActive,
    NestedBlendDone,
    NestedMovement,
    NestedDetails,
    OuterInterruptStart,
    OuterInterruptMove,
    OuterInterruptCheck,
    OuterFsm,
    Idle,
    MoveForward,
    MoveRightRun,
    Sprint,
    FirstPivot,
    StopPlant,
    StopReset,
    StopPlantLeft,
    RotateRight,
    RotateLeft,
    Turn,
    QuickStop,
    Lean,
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        root: assets.load(ROOT_PATH),
        cycle: assets.load(CYCLE_PATH),
        outer: assets.load(OUTER_PATH),
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

fn has_named_constant(graph: &AnimationGraph, name: &str, expected: f32) -> bool {
    node::<ConstantF32Node>(graph, name).is_some_and(|constant| close(constant.value, expected))
}

fn has_stop_plant_contract(
    graph: &AnimationGraph,
    mask_key: &str,
    layered_blend_name: &str,
    expected_frames: &[(&str, f32)],
) -> bool {
    let clamped_evaluators = graph
        .nodes
        .values()
        .filter_map(|node| node.try_inner_downcast_ref::<TimePositionNode>())
        .filter(|node| !node.loop_position)
        .count();
    let normalized_velocity_blend = graph.nodes.values().any(|node| {
        node.try_inner_downcast_ref::<MultiWayBlendNode>()
            .is_some_and(|blend| blend.pose_count == 4 && blend.normalize_alpha)
    });
    let mask_input = graph.nodes.values().any(|node| {
        node.try_inner_downcast_ref::<GlobalBoneMaskInputNode>()
            .is_some_and(|input| input.key == mask_key)
    });
    clamped_evaluators == 7
        && normalized_velocity_blend
        && mask_input
        && has_node::<MeshSpaceLayeredBlendNode>(graph, layered_blend_name)
        && expected_frames
            .iter()
            .all(|(name, value)| has_named_constant(graph, name, *value))
}

fn set_stop_leg_masks(player: &mut AnimationGraphPlayer, skeleton: &Skeleton) {
    player.set_global_input_data(
        "als_standing_stop_left_leg_mask",
        DataValue::BoneMask(build_branch_bone_mask(
            skeleton,
            STOP_LEFT_LEG_BRANCHES,
            1.0,
        )),
    );
    player.set_global_input_data(
        "als_standing_stop_right_leg_mask",
        DataValue::BoneMask(build_branch_bone_mask(
            skeleton,
            STOP_RIGHT_LEG_BRANCHES,
            1.0,
        )),
    );
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

fn state_uses_graph(machine: &StateMachine, label: &str, graph: &Handle<AnimationGraph>) -> bool {
    machine
        .states
        .values()
        .find(|state| state.label == label)
        .is_some_and(|state| state.graph.id() == graph.id())
}

fn has_direction_wrapper(
    graph: &AnimationGraph,
    direction_indices: [f32; 4],
    movement_blend: &Handle<AnimationGraph>,
) -> bool {
    let Some(layer_id) = graph.nodes.values().find_map(|node| {
        node.try_inner_downcast_ref::<LayeredGraphNode>()
            .filter(|node| node.graph.id() == movement_blend.id())
            .map(|_| node.id)
    }) else {
        return false;
    };
    [
        ("forward_direction_index", direction_indices[0]),
        ("backward_direction_index", direction_indices[1]),
        ("left_direction_index", direction_indices[2]),
        ("right_direction_index", direction_indices[3]),
    ]
    .into_iter()
    .all(|(input, expected)| {
        graph
            .edges_inverted
            .get(&TargetPin::NodeData(layer_id, input.to_owned()))
            .and_then(|source| match source {
                SourcePin::NodeData(node_id, pin) if pin == "value" => graph
                    .nodes
                    .get(node_id)
                    .and_then(|node| node.try_inner_downcast_ref::<ConstantF32Node>())
                    .map(|node| node.value),
                _ => None,
            })
            .is_some_and(|actual| close(actual, expected))
    })
}

fn has_movement_blend_contract(
    graph: &AnimationGraph,
    movement_direction_graph: &Handle<AnimationGraph>,
) -> bool {
    graph
        .nodes
        .values()
        .filter_map(|node| node.try_inner_downcast_ref::<LayeredGraphNode>())
        .filter(|node| node.graph.id() == movement_direction_graph.id())
        .count()
        == 4
        && graph.nodes.values().any(|node| {
            node.try_inner_downcast_ref::<MultiWayBlendNode>()
                .is_some_and(|node| node.pose_count == 4 && node.normalize_alpha)
        })
        && [
            "als_standing_velocity_forward_amount",
            "als_standing_velocity_backward_amount",
            "als_standing_velocity_left_amount",
            "als_standing_velocity_right_amount",
        ]
        .into_iter()
        .all(|key| {
            graph.nodes.values().any(|node| {
                node.try_inner_downcast_ref::<GlobalF32InputNode>()
                    .is_some_and(|node| node.key == key)
            })
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
        _ => None,
    }
}

fn outputs_match(player: &AnimationGraphPlayer, expected: &[(&str, f32)]) -> bool {
    expected
        .iter()
        .all(|(name, value)| output_f32(player, name).is_some_and(|actual| close(actual, *value)))
}

fn outer_fsm_state<'a>(
    player: &'a AnimationGraphPlayer,
    root: &AnimationGraph,
) -> Option<&'a InterruptibleFsmState> {
    let node_id = root
        .nodes
        .values()
        .find(|node| node.name == "standing states")?
        .id;
    let arena = player.get_context_arena()?;
    arena
        .get_toplevel()
        .node_states
        .get::<InterruptibleFsmState>(node_id, StateKey::Default)
        .ok()
}

fn outer_fsm_output_pose(player: &AnimationGraphPlayer, root: &AnimationGraph) -> Option<Pose> {
    let node_id = root
        .nodes
        .values()
        .find(|node| node.name == "standing states")?
        .id;
    let arena = player.get_context_arena()?;
    match arena.get_toplevel().node_caches.get_output_data(
        node_id,
        StateKey::Default,
        "pose".to_owned(),
    )? {
        DataValue::Pose(pose) => Some(pose),
        _ => None,
    }
}

fn outer_fsm_state_label(
    player: &AnimationGraphPlayer,
    root: &AnimationGraph,
    outer: &StateMachine,
) -> Option<String> {
    let state = outer_fsm_state(player, root)?;
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
        ("als_standing_sprint_acceleration_amount", 0.0),
        ("als_standing_sprint_play_rate", 1.0),
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
        ("als_standing_turn_play_rate", 1.2),
        ("als_standing_quick_stop_index", 0.0),
        ("als_standing_quick_stop_weight", 0.0),
        ("als_standing_quick_stop_play_rate", 1.75),
        ("als_standing_lean_right_amount", 0.0),
        ("als_standing_lean_forward_amount", 0.0),
        ("als_standing_lean_weight", 0.0),
    ] {
        player.set_global_input_data(key, value);
    }
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
        || !assets.is_loaded_with_dependencies(&gate_assets.cycle)
        || !assets.is_loaded_with_dependencies(&gate_assets.outer)
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
    let Some(cycle) = graphs.get(&gate_assets.cycle) else {
        return;
    };
    let Some(outer) = machines.get(&gate_assets.outer) else {
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
    let Some(movement_blend) = graphs.get(&gate_assets.movement_blend) else {
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
    let Some(skeleton) = skeletons.get(&gate_assets.skeleton) else {
        return;
    };
    let linked = has_node::<InterruptibleFsmNode>(root, "standing states")
        && node::<DirectTransitionEventNode>(root, "standing transition driver")
            .is_some_and(has_ordered_outer_transition_ids)
        && node::<DirectTransitionEventNode>(move_graph, "standing movement states driver")
            .is_some_and(has_ordered_movement_transition_ids)
        && node::<DirectTransitionEventNode>(
            movement_direction_graph,
            "standing movement details driver",
        )
        .is_some_and(has_ordered_detail_transition_ids)
        && has_node::<LayeredGraphNode>(root, "standing turn linked layer")
        && has_node::<LayeredGraphNode>(root, "standing quick stop linked layer")
        && has_node::<LayeredGraphNode>(root, "standing lean linked layer");
    let interfaces = [
        "pose",
        "standing_state",
        "movement_state",
        "movement_detail_state",
        "stop_state",
        "direction_index",
        "clip_index",
        "loop",
        "play_rate",
        "transition_duration",
        "PoseStanding",
        "PoseMoving",
        "RotationYawOffset",
        "FootLeftLock",
        "FootRightLock",
        "AllowTransitions",
        "RotationYawSpeed",
    ]
    .into_iter()
    .all(|name| root.io_spec.get_output_data(&name.to_owned()).is_some());
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
        && has_movement_blend_contract(movement_blend, &gate_assets.movement_direction_graph)
        && [
            ("Move Forward", [0.0, 3.0, 5.0, 1.0]),
            ("Move Backward", [0.0, 3.0, 4.0, 2.0]),
            ("Move Right Forward", [0.0, 3.0, 4.0, 1.0]),
            ("Move Right Backward", [0.0, 3.0, 5.0, 2.0]),
            ("Move Left Forward", [0.0, 3.0, 5.0, 2.0]),
            ("Move Left Backward", [0.0, 3.0, 4.0, 1.0]),
        ]
        .into_iter()
        .zip(gate_assets.movement_wrappers.iter())
        .all(|((label, direction_index), handle)| {
            state_uses_graph(movement, label, handle)
                && graphs.get(handle).is_some_and(|graph| {
                    has_direction_wrapper(graph, direction_index, &gate_assets.movement_blend)
                })
        });
    let cycle_contract = cycle
        .nodes
        .values()
        .filter(|node| node.try_inner_downcast_ref::<BlendSpaceNode>().is_some())
        .count()
        == 6
        && cycle
            .nodes
            .values()
            .filter(|node| node.try_inner_downcast_ref::<LoopNode>().is_some())
            .count()
            == 26
        && has_node::<F32ToVec2Node>(cycle, "walk run blendspace position");
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
        "als_standing_stop_right_leg_mask",
        "stop plant right mesh space layered blend",
        &[
            ("stop plant right forward frame", 0.7),
            ("stop plant right backward frame", 0.7),
            ("stop plant right left forward frame", 0.8),
            ("stop plant right left backward frame", 0.8),
            ("stop plant right right forward frame", 0.7666667),
            ("stop plant right right backward frame", 0.7),
        ],
    ) && has_stop_plant_contract(
        stop_left,
        "als_standing_stop_left_leg_mask",
        "stop plant left mesh space layered blend",
        &[
            ("stop plant left forward frame", 0.13333334),
            ("stop plant left backward frame", 0.13333334),
            ("stop plant left left forward frame", 0.2),
            ("stop plant left left backward frame", 0.13333334),
            ("stop plant left right forward frame", 0.23333334),
            ("stop plant left right backward frame", 0.23333334),
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
    let rotate_completion = [rotate_left, rotate_right].into_iter().all(|graph| {
        graph
            .nodes
            .values()
            .filter_map(|node| node.try_inner_downcast_ref::<TimePositionNode>())
            .any(|node| !node.loop_position && close(node.duration_fallback, 1.0))
            && graph.nodes.values().any(|node| {
                node.try_inner_downcast_ref::<GlobalF32InputNode>()
                    .is_some_and(|node| node.key == "als_standing_rotate_clip_time")
            })
            && !graph
                .nodes
                .values()
                .any(|node| node.try_inner_downcast_ref::<LoopNode>().is_some())
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
            == 4;
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
        && asset_contract;
    info!(
        "[STANDING-CONTRACT] linked={} interfaces={} state_machines={} wrappers={} cycle={} resets={} velocity_blends={} stop_plants={} transitions={} rotate_completion={} architecture={} assets={}",
        linked,
        interfaces,
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
    set_stop_leg_masks(&mut player, skeleton);
    player.pause();
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.player = Some(
        commands
            .spawn((
                Name::new("standing_contract_probe"),
                Children::default(),
                player,
            ))
            .id(),
    );
    let mut blend_player = AnimationGraphPlayer::new(gate_assets.skeleton.clone())
        .with_graph(gate_assets.movement_blend.clone());
    set_globals(&mut blend_player);
    for (key, value) in [
        ("forward_direction_index", 0.0),
        ("backward_direction_index", 3.0),
        ("left_direction_index", 5.0),
        ("right_direction_index", 1.0),
    ] {
        blend_player.set_input_data(key, DataValue::F32(value));
    }
    for key in [
        "als_standing_velocity_forward_amount",
        "als_standing_velocity_backward_amount",
        "als_standing_velocity_left_amount",
        "als_standing_velocity_right_amount",
    ] {
        blend_player.set_global_input_data(key, 0.25);
    }
    set_stop_leg_masks(&mut blend_player, skeleton);
    blend_player.pause();
    blend_player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.blend_player = Some(
        commands
            .spawn((
                Name::new("standing_blend_contract_probe"),
                Children::default(),
                blend_player,
            ))
            .id(),
    );
    let mut movement_player = AnimationGraphPlayer::new(gate_assets.skeleton.clone())
        .with_graph(gate_assets.move_graph.clone());
    set_globals(&mut movement_player);
    set_stop_leg_masks(&mut movement_player, skeleton);
    movement_player.pause();
    movement_player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.movement_player = Some(
        commands
            .spawn((
                Name::new("standing_movement_contract_probe"),
                Children::default(),
                movement_player,
            ))
            .id(),
    );
    let mut detail_player = AnimationGraphPlayer::new(gate_assets.skeleton.clone())
        .with_graph(gate_assets.movement_direction_graph.clone());
    set_globals(&mut detail_player);
    detail_player.set_input_data("direction_index", DataValue::F32(0.0));
    set_stop_leg_masks(&mut detail_player, skeleton);
    detail_player.pause();
    detail_player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.detail_player = Some(
        commands
            .spawn((
                Name::new("standing_detail_contract_probe"),
                Children::default(),
                detail_player,
            ))
            .id(),
    );
    probe.phase = ProbePhase::NestedBlendStart;
    probe.static_verified = true;
    info!("[STANDING-CONTRACT] linked runtime probe initialized");
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!("[STANDING-CONTRACT][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(code).unwrap()));
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
            probe.phase = ProbePhase::NestedMovement;
        }
        ProbePhase::NestedMovement => {
            let Some(entity) = probe.movement_player else {
                return;
            };
            let (Some(graph), Some(machine), Ok(mut player)) = (
                graphs.get(&gate_assets.move_graph),
                machines.get(&gate_assets.movement),
                players.get_mut(entity),
            ) else {
                return;
            };
            let steps = [
                (0.0, "Move Forward", "Move Backward"),
                (4.0, "Move Backward", "Move Right Backward"),
                (15.0, "Move Right Backward", "Move Right Forward"),
                (16.0, "Move Right Forward", "Move Right Backward"),
                (20.0, "Move Right Backward", "Move Right Forward"),
                (21.0, "Move Right Forward", "Move Right Backward"),
                (23.0, "Move Right Backward", "Move Right Forward"),
                (6.0, "Move Right Forward", "Move Forward"),
                (3.0, "Move Forward", "Move Left Forward"),
                (10.0, "Move Left Forward", "Move Right Backward"),
                (12.0, "Move Right Backward", "Move Left Forward"),
                (19.0, "Move Left Forward", "Move Left Backward"),
                (14.0, "Move Left Backward", "Move Left Forward"),
                (22.0, "Move Left Forward", "Move Left Backward"),
                (17.0, "Move Left Backward", "Move Left Forward"),
                (7.0, "Move Left Forward", "Move Forward"),
                (0.0, "Move Forward", "Move Backward"),
                (5.0, "Move Backward", "Move Left Backward"),
                (13.0, "Move Left Backward", "Move Right Forward"),
                (11.0, "Move Right Forward", "Move Left Backward"),
                (18.0, "Move Left Backward", "Move Left Forward"),
                (7.0, "Move Left Forward", "Move Forward"),
                (2.0, "Move Forward", "Move Right Forward"),
                (11.0, "Move Right Forward", "Move Left Backward"),
                (8.0, "Move Left Backward", "Move Backward"),
                (1.0, "Move Backward", "Move Forward"),
                (0.0, "Move Forward", "Move Backward"),
                (4.0, "Move Backward", "Move Right Backward"),
                (9.0, "Move Right Backward", "Move Backward"),
            ];
            let mut step = probe.movement_step;
            let mut event_queued = probe.movement_event_queued;
            let complete = drive_direct_fsm(
                &mut player,
                graph,
                machine,
                "standing movement states",
                "standing movement states driver",
                "als_standing_movement_transition_index",
                &steps,
                &mut step,
                &mut event_queued,
                "movement FSM",
                &mut exit,
            );
            probe.movement_step = step;
            probe.movement_event_queued = event_queued;
            if complete == Some(true) {
                probe.phase = ProbePhase::NestedDetails;
            }
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
            | ProbePhase::NestedMovement
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
    let Some(root) = graphs.get(&gate_assets.root) else {
        return;
    };
    let Some(outer) = machines.get(&gate_assets.outer) else {
        return;
    };
    if let Some(error) = player.get_error() {
        fail(&mut exit, 4, &format!("runtime error={error:?}"));
        return;
    }
    let Some(pose) = player.get_default_output_pose().cloned() else {
        return;
    };
    if probe.phase != ProbePhase::Idle
        && probe
            .last_pose
            .as_ref()
            .is_some_and(|previous| previous == &pose)
        && probe.settle_frames < 3
    {
        probe.settle_frames += 1;
        advance(&mut player);
        return;
    }
    probe.settle_frames = 0;
    match probe.phase {
        ProbePhase::OuterInterruptStart => {
            if outer_fsm_state_label(&player, root, outer).as_deref() != Some("Idle") {
                fail(&mut exit, 5, "outer interrupt initial idle state");
                return;
            }
            set_value(&mut player, "als_standing_transition_index", 0.0);
            advance_by(&mut player, 0.1);
            probe.phase = ProbePhase::OuterInterruptMove;
        }
        ProbePhase::OuterInterruptMove => {
            let expected = node::<DirectTransitionEventNode>(root, "standing transition driver")
                .and_then(|driver| driver.transitions.first())
                .copied();
            let active = outer_fsm_state(&player, root);
            if !matches!(
                active.map(|state| &state.state),
                Some(LowLevelStateId::HlTransition(TransitionId::Direct(id))) if Some(id) == expected.as_ref()
            ) || active.is_some_and(|state| !state.interrupted_sources.is_empty())
            {
                fail(&mut exit, 5, "outer interrupt initial transition state");
                return;
            }
            let Some(source_pose) = outer_fsm_output_pose(&player, root) else {
                fail(&mut exit, 5, "outer interrupt source pose missing");
                return;
            };
            probe.interrupt_source_pose = Some(source_pose);
            set_value(&mut player, "als_standing_transition_index", 3.0);
            advance_by(&mut player, 0.1);
            probe.phase = ProbePhase::OuterInterruptCheck;
        }
        ProbePhase::OuterInterruptCheck => {
            let expected_current =
                node::<DirectTransitionEventNode>(root, "standing transition driver")
                    .and_then(|driver| driver.transitions.get(3))
                    .copied();
            let expected_source =
                node::<DirectTransitionEventNode>(root, "standing transition driver")
                    .and_then(|driver| driver.transitions.first())
                    .copied();
            let active = outer_fsm_state(&player, root);
            let source_pose = active.and_then(InterruptibleFsmState::cached_source_pose);
            let chained = active.is_some_and(|state| {
                matches!(
                    &state.state,
                    LowLevelStateId::HlTransition(TransitionId::Direct(id)) if Some(id) == expected_current.as_ref()
                ) && matches!(
                    state.interrupted_sources.as_slice(),
                    [source] if matches!(
                        &source.state,
                        LowLevelStateId::HlTransition(TransitionId::Direct(id)) if Some(id) == expected_source.as_ref()
                    ) && source.state_entered_time < state.state_entered_time
                )
            }) && probe
                .interrupt_source_pose
                .as_ref()
                .is_some_and(|expected| source_pose.is_some_and(|actual| actual == expected));
            if !chained {
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
                if outer_fsm_state_label(&player, root, outer).as_deref() != Some("Idle") {
                    fail(&mut exit, 5, "outer FSM final idle state");
                    return;
                }
                probe.phase = ProbePhase::Idle;
                info!("[STANDING-CONTRACT] outer FSM direct transition topology=12/12");
                return;
            }
            let (index, source, target) = steps[probe.outer_step];
            let actual_source = outer_fsm_state_label(&player, root, outer);
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
            set_value(&mut player, "als_standing_sprint_amount", 1.0);
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
            set_value(&mut player, "als_standing_stop_state_index", 2.0);
            set_value(&mut player, "als_standing_clip_index", 5.0);
            set_value(&mut player, "als_standing_output_play_rate", 1.0);
            set_value(&mut player, "als_standing_transition_duration", 0.3);
            advance(&mut player);
            probe.phase = ProbePhase::StopPlant;
        }
        ProbePhase::StopPlant => {
            if !outputs_match(
                &player,
                &[
                    ("standing_state", 2.0),
                    ("stop_state", 2.0),
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
            advance(&mut player);
            probe.phase = ProbePhase::StopReset;
        }
        ProbePhase::StopReset => {
            if !outputs_match(&player, &[("standing_state", 0.0)]) {
                fail(&mut exit, 11, "stop reset to idle");
                return;
            }
            set_value(&mut player, "als_standing_stop_state_index", 4.0);
            set_value(&mut player, "als_standing_state_index", 2.0);
            advance(&mut player);
            probe.phase = ProbePhase::StopPlantLeft;
        }
        ProbePhase::StopPlantLeft => {
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
                fail(&mut exit, 12, "left stop plant state, clip, blend, or pose");
                return;
            }
            probe.last_pose = Some(pose);
            info!(
                "[STANDING-CONTRACT] stop=PlantLeftFoot evaluators=WalkRun frames=4/4/6/4/7/7 loop=true blend=0.3"
            );
            set_value(&mut player, "als_standing_state_index", 3.0);
            set_value(&mut player, "als_standing_clip_index", 6.0);
            set_value(&mut player, "als_standing_loop", 1.0);
            set_value(&mut player, "als_standing_rotation_play_rate", 2.0);
            set_value(&mut player, "als_standing_output_play_rate", 2.0);
            set_value(&mut player, "als_standing_transition_duration", 0.2);
            advance(&mut player);
            probe.phase = ProbePhase::RotateRight;
        }
        ProbePhase::RotateRight => {
            if !outputs_match(
                &player,
                &[
                    ("standing_state", 3.0),
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
            set_value(&mut player, "als_standing_state_index", 4.0);
            set_value(&mut player, "als_standing_clip_index", 7.0);
            advance(&mut player);
            probe.phase = ProbePhase::RotateLeft;
        }
        ProbePhase::RotateLeft => {
            if !outputs_match(
                &player,
                &[("standing_state", 4.0), ("clip_index", 7.0), ("loop", 1.0)],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(&mut exit, 13, "rotate left state, clip, loop, or pose");
                return;
            }
            probe.last_pose = Some(pose);
            info!(
                "[STANDING-CONTRACT] state=RotateLeft clip=Rotate90Left loop=true inertial_exit=0.2"
            );
            set_value(&mut player, "als_standing_state_index", 0.0);
            set_value(&mut player, "als_standing_turn_weight", 1.0);
            set_value(&mut player, "als_standing_turn_index", 3.0);
            set_value(&mut player, "als_standing_turn_play_rate", 1.2);
            set_value(&mut player, "als_standing_clip_index", 8.0);
            set_value(&mut player, "als_standing_loop", 0.0);
            set_value(&mut player, "als_standing_output_play_rate", 1.2);
            advance(&mut player);
            probe.phase = ProbePhase::Turn;
        }
        ProbePhase::Turn => {
            if !outputs_match(
                &player,
                &[("clip_index", 8.0), ("loop", 0.0), ("play_rate", 1.2)],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(&mut exit, 14, "turn clip, loop, rate, or pose");
                return;
            }
            probe.last_pose = Some(pose);
            info!("[STANDING-CONTRACT] turn=Right180 loop=false rate=1.2");
            set_value(&mut player, "als_standing_turn_weight", 0.0);
            set_value(&mut player, "als_standing_quick_stop_weight", 1.0);
            set_value(&mut player, "als_standing_quick_stop_index", 0.0);
            set_value(&mut player, "als_standing_quick_stop_play_rate", 1.75);
            set_value(&mut player, "als_standing_clip_index", 9.0);
            set_value(&mut player, "als_standing_output_play_rate", 1.75);
            advance(&mut player);
            probe.phase = ProbePhase::QuickStop;
        }
        ProbePhase::QuickStop => {
            if !outputs_match(
                &player,
                &[("clip_index", 9.0), ("loop", 0.0), ("play_rate", 1.75)],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(&mut exit, 15, "quick stop clip, loop, rate, or pose");
                return;
            }
            probe.last_pose = Some(pose);
            info!(
                "[STANDING-CONTRACT] quick_stop=Left loop=false rate=1.75 start=0.3 blend_in=0.1 blend_out=0.2"
            );
            set_value(&mut player, "als_standing_quick_stop_weight", 0.0);
            set_value(&mut player, "als_standing_lean_weight", 1.0);
            set_value(&mut player, "als_standing_lean_right_amount", 1.0);
            set_value(&mut player, "als_standing_clip_index", 10.0);
            set_value(&mut player, "als_standing_output_play_rate", 1.0);
            advance(&mut player);
            probe.phase = ProbePhase::Lean;
        }
        ProbePhase::Lean => {
            if !outputs_match(
                &player,
                &[("clip_index", 10.0), ("loop", 0.0), ("play_rate", 1.0)],
            ) || !probe
                .last_pose
                .as_ref()
                .is_some_and(|previous| previous != &pose)
            {
                fail(&mut exit, 16, "lean pose or telemetry");
                return;
            }
            info!("[STANDING-CONTRACT] lean=Right evaluator_time=0 loop=true");
            info!("[STANDING-CONTRACT][VERDICT] linked=true -> PASS");
            exit.write(AppExit::Success);
        }
        ProbePhase::NestedBlendStart
        | ProbePhase::NestedBlendActive
        | ProbePhase::NestedBlendDone
        | ProbePhase::NestedMovement
        | ProbePhase::NestedDetails
        | ProbePhase::Pending => {}
    }
}

fn auto_quit(timeout: Res<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= timeout.0 {
        exit.write(AppExit::Error(std::num::NonZeroU8::new(17).unwrap()));
    }
}
