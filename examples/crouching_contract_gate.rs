use std::time::{Duration, Instant};

use bevy_als::extensions::{
    AlsExtPlugin, DirectTransitionEventNode, InterruptibleFsmNode, InterruptibleFsmState,
    LayeredGraphNode, SourceCachedPoseSaveNode,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    context::node_states::StateKey,
    edge_data::DataValue,
    plugin::AnimationGraphSet,
    pose::Pose,
    skeleton::Skeleton,
    state_machine::{high_level::StateMachine, low_level::LowLevelStateId},
};

const ROOT_PATH: &str = "graphs/ab_als/stances/crouching_runtime_entry.animgraph.ron";
const SOURCE_PATH: &str = "graphs/ab_als/stances/crouching.animgraph.ron";
const MOVEMENT_RUNTIME_PATH: &str =
    "graphs/ab_als/stances/crouching/movement_states_runtime.animgraph.ron";
const OUTER_RUNTIME_PATH: &str =
    "graphs/ab_als/stances/crouching/crouching_states_runtime.animgraph.ron";
const MOVEMENT_FSM_PATH: &str = "graphs/ab_als/stances/crouching/movement_states.fsm.ron";
const OUTER_FSM_PATH: &str = "graphs/ab_als/stances/crouching/crouching_states.fsm.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const SOURCE_MARKER_ROTATION_FLOOR: f32 = 0.05;
const CACHED_POSE_OUTPUT: &str = "__als_source_cached_pose";
const CACHE_KEYS: [(&str, &str); 7] = [
    ("Save cached pose 'Movement'", "ab_als_crouching:movement"),
    (
        "Save cached pose 'Move Forward'",
        "ab_als_crouching:move_forward",
    ),
    (
        "Save cached pose 'Move Backward'",
        "ab_als_crouching:move_backward",
    ),
    (
        "Save cached pose 'Move Right Forward'",
        "ab_als_crouching:move_right_forward",
    ),
    (
        "Save cached pose 'Move Right Backward'",
        "ab_als_crouching:move_right_backward",
    ),
    (
        "Save cached pose 'Move Left Forward'",
        "ab_als_crouching:move_left_forward",
    ),
    (
        "Save cached pose 'Move Left Backward'",
        "ab_als_crouching:move_left_backward",
    ),
];

#[derive(Resource)]
struct GateAssets {
    root: Handle<AnimationGraph>,
    source: Handle<AnimationGraph>,
    movement_runtime: Handle<AnimationGraph>,
    outer_runtime: Handle<AnimationGraph>,
    movement_fsm: Handle<StateMachine>,
    outer_fsm: Handle<StateMachine>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource, Default)]
struct Probe {
    entity: Option<Entity>,
    phase: Phase,
    move_steps: u8,
    baseline: Option<ChainPoses>,
}

#[derive(Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Pending,
    VerifyNoEvent,
    EnterMove,
    SampleMarkerMotion,
}

#[derive(Clone)]
struct ChainPoses {
    sequence_player: Pose,
    cache_producer: Pose,
    cache_consumer: Pose,
    movement_fsm: Pose,
    outer_fsm: Pose,
    parent_graph: Pose,
    root: Pose,
}

#[derive(Resource)]
struct Deadline(Instant);

fn main() {
    App::new()
        .add_plugins((
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
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(30)))
        .add_systems(Startup, load_assets)
        .add_systems(Update, (initialize, timeout))
        .add_systems(PostUpdate, observe.after(AnimationGraphSet::Final))
        .run();
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GateAssets {
        root: assets.load(ROOT_PATH),
        source: assets.load(SOURCE_PATH),
        movement_runtime: assets.load(MOVEMENT_RUNTIME_PATH),
        outer_runtime: assets.load(OUTER_RUNTIME_PATH),
        movement_fsm: assets.load(MOVEMENT_FSM_PATH),
        outer_fsm: assets.load(OUTER_FSM_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!("[CROUCHING-CONTRACT][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(code).unwrap()));
    std::process::exit(i32::from(code));
}

fn layered<'a>(graph: &'a AnimationGraph, name: &str) -> Option<&'a LayeredGraphNode> {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<LayeredGraphNode>())
}

fn cached_key<'a>(graph: &'a AnimationGraph, name: &str) -> Option<&'a str> {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<SourceCachedPoseSaveNode>())
        .map(|node| node.cache_key.as_str())
}

fn has_fsm(graph: &AnimationGraph, name: &str, fsm: &Handle<StateMachine>) -> bool {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<InterruptibleFsmNode>())
        .is_some_and(|node| node.fsm.id() == fsm.id() && node.state_pose_output == "Result")
}

fn has_no_event_default(graph: &AnimationGraph, name: &str, key: &str) -> bool {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<DirectTransitionEventNode>())
        .is_some_and(|node| node.key == key && node.default_value == -1.0)
}

fn static_contract(
    assets: &GateAssets,
    root: &AnimationGraph,
    source: &AnimationGraph,
    movement_runtime: &AnimationGraph,
    outer_runtime: &AnimationGraph,
) -> bool {
    let root_contract = layered(root, "crouching source graph runtime entry").is_some_and(|node| {
        node.graph.id() == assets.source.id()
            && node.internalize_time
            && node.output_aliases.len() == 1
            && node.output_aliases[0].child_output == "Result"
            && node.output_aliases[0].outer_output == "pose"
    });
    let cache_contract = CACHE_KEYS
        .iter()
        .all(|(name, key)| cached_key(source, name) == Some(*key));
    let movement_contract = layered(source, "Movement States\nState Machine").is_some_and(|node| {
        let expected = [
            ("forward_pose", "Save cached pose 'Move Forward'"),
            ("backward_pose", "Save cached pose 'Move Backward'"),
            (
                "right_forward_pose",
                "Save cached pose 'Move Right Forward'",
            ),
            (
                "right_backward_pose",
                "Save cached pose 'Move Right Backward'",
            ),
            ("left_forward_pose", "Save cached pose 'Move Left Forward'"),
            (
                "left_backward_pose",
                "Save cached pose 'Move Left Backward'",
            ),
        ];
        node.graph.id() == assets.movement_runtime.id()
            && node.internalize_time
            && node.cached_pose_inputs.len() == expected.len()
            && expected.iter().all(|(input, cache)| {
                node.cached_pose_inputs
                    .iter()
                    .any(|value| value.input == *input && value.cache_node == *cache)
            })
    });
    let outer_contract = layered(source, "Crouching States\nState Machine").is_some_and(|node| {
        node.graph.id() == assets.outer_runtime.id()
            && node.internalize_time
            && node.cached_pose_inputs.is_empty()
            && node.prewarm_cached_pose_nodes == ["Save cached pose 'Movement'"]
    });
    let fsm_contract = has_fsm(
        movement_runtime,
        "crouching movement states runtime",
        &assets.movement_fsm,
    ) && has_fsm(outer_runtime, "crouching states runtime", &assets.outer_fsm)
        && has_no_event_default(
            movement_runtime,
            "crouching movement transition driver",
            "als_crouching_movement_transition_index",
        )
        && has_no_event_default(
            outer_runtime,
            "crouching transition driver",
            "als_crouching_transition_index",
        );
    root_contract && cache_contract && movement_contract && outer_contract && fsm_contract
}

fn set_globals(player: &mut AnimationGraphPlayer) {
    for (key, value) in [
        ("als_crouching_state_index", 0.0),
        ("als_crouching_movement_state_index", 0.0),
        ("als_crouching_movement_transition_index", -1.0),
        ("als_crouching_hips_direction_lock_amount", 0.0),
        ("als_crouching_direction_index", 0.0),
        ("als_crouching_clip_index", 0.0),
        ("als_crouching_loop", 1.0),
        ("als_crouching_output_play_rate", 1.0),
        ("als_crouching_transition_duration", 0.2),
        ("als_crouching_curve_pose_crouching", 1.0),
        ("als_crouching_curve_pose_moving", 0.0),
        ("als_crouching_curve_rotation_yaw_offset", 0.0),
        ("als_crouching_curve_foot_left_lock", 1.0),
        ("als_crouching_curve_foot_right_lock", 1.0),
        ("als_crouching_curve_allow_transitions", 1.0),
        ("als_crouching_curve_rotation_yaw_speed", 0.0),
        ("als_crouching_velocity_forward_amount", 1.0),
        ("als_crouching_velocity_backward_amount", 0.0),
        ("als_crouching_velocity_left_amount", 0.0),
        ("als_crouching_velocity_right_amount", 0.0),
        ("als_crouching_stride_amount", 1.0),
        ("als_crouching_play_rate", 1.0),
        ("als_crouching_source_pose_frame", 0.0),
        ("als_crouching_movement_marker_phase", 0.0),
        ("als_movement_marker_phase", 0.0),
        ("als_crouching_delta_seconds", 0.1),
        ("als_crouching_rotation_play_rate", 1.0),
        ("als_crouching_rotate_clip_time", 0.0),
        ("als_crouching_turn_index", 0.0),
        ("als_crouching_turn_weight", 0.0),
        ("als_crouching_turn_play_rate", 1.2),
        ("als_crouching_lean_right_amount", 0.0),
        ("als_crouching_lean_forward_amount", 0.0),
    ] {
        player.set_global_input_data(key, value);
    }
    player.set_global_input_data("als_crouching_rotate_left_loop", false);
    player.set_global_input_data("als_crouching_rotate_right_loop", false);
    player.set_global_input_data(
        "als_crouching_stop_left_leg_mask",
        DataValue::BoneMask(default()),
    );
    player.set_global_input_data(
        "als_crouching_stop_right_leg_mask",
        DataValue::BoneMask(default()),
    );
    player.set_global_input_data(
        "als_crouching_stop_legs_mask",
        DataValue::BoneMask(default()),
    );
}

fn initialize(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    mut probe: ResMut<Probe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.entity.is_some()
        || !asset_server.is_loaded_with_dependencies(&assets.root)
        || !asset_server.is_loaded_with_dependencies(&assets.source)
        || !asset_server.is_loaded_with_dependencies(&assets.movement_runtime)
        || !asset_server.is_loaded_with_dependencies(&assets.outer_runtime)
        || !asset_server.is_loaded_with_dependencies(&assets.skeleton)
    {
        return;
    }
    let (Some(root), Some(source), Some(movement_runtime), Some(outer_runtime)) = (
        graphs.get(&assets.root),
        graphs.get(&assets.source),
        graphs.get(&assets.movement_runtime),
        graphs.get(&assets.outer_runtime),
    ) else {
        return;
    };
    if !static_contract(&assets, root, source, movement_runtime, outer_runtime) {
        fail(&mut exit, 3, "current source cache and FSM contract");
        return;
    }
    let mut player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.root.clone());
    set_globals(&mut player);
    player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.entity = Some(
        commands
            .spawn((
                Name::new("crouching_contract_gate"),
                Children::default(),
                player,
            ))
            .id(),
    );
    probe.phase = Phase::VerifyNoEvent;
    info!("[CROUCHING-CONTRACT] static source cache/FSM contract -> PASS");
}

fn node_pose(
    player: &AnimationGraphPlayer,
    handle: &Handle<AnimationGraph>,
    graph: &AnimationGraph,
    node_name: &str,
    output: &str,
) -> Option<Pose> {
    let node = graph.nodes.values().find(|node| node.name == node_name)?;
    let arena = player.get_context_arena()?;
    arena.iter_context_ids().find_map(|context_id| {
        let context = arena.get_context(context_id)?;
        (context.get_graph_id() == handle.id()).then(|| {
            context
                .node_caches
                .get_output_data(node.id, StateKey::Default, output.to_owned())
                .and_then(|value| value.into_pose().ok())
        })?
    })
}

fn outer_state_label(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    graph_handle: &Handle<AnimationGraph>,
    state_machine: &StateMachine,
) -> Option<String> {
    let node = graph
        .nodes
        .values()
        .find(|node| node.name == "crouching states runtime")?;
    let arena = player.get_context_arena()?;
    arena.iter_context_ids().find_map(|context_id| {
        let context = arena.get_context(context_id)?;
        if context.get_graph_id() != graph_handle.id() {
            return None;
        }
        let state = context
            .node_states
            .get::<InterruptibleFsmState>(node.id, StateKey::Default)
            .ok()?;
        match &state.state {
            LowLevelStateId::HlState(id) => state_machine
                .states
                .get(id)
                .map(|state| state.label.clone()),
            LowLevelStateId::HlTransition(_) => None,
        }
    })
}

fn chain_snapshot(
    player: &AnimationGraphPlayer,
    assets: &GateAssets,
    graphs: &Assets<AnimationGraph>,
) -> Option<ChainPoses> {
    let source = graphs.get(&assets.source)?;
    let movement_runtime = graphs.get(&assets.movement_runtime)?;
    let outer_runtime = graphs.get(&assets.outer_runtime)?;
    Some(ChainPoses {
        sequence_player: node_pose(
            player,
            &assets.source,
            source,
            "A_Als_Crouch_Walk_Forward\nSequence Player\nSync group Movement",
            "Pose",
        )?,
        cache_producer: node_pose(
            player,
            &assets.source,
            source,
            "Save cached pose 'Move Forward'",
            CACHED_POSE_OUTPUT,
        )?,
        cache_consumer: node_pose(
            player,
            &assets.source,
            source,
            "Movement States\nState Machine",
            "Pose",
        )?,
        movement_fsm: node_pose(
            player,
            &assets.movement_runtime,
            movement_runtime,
            "crouching movement states runtime",
            "pose",
        )?,
        outer_fsm: node_pose(
            player,
            &assets.outer_runtime,
            outer_runtime,
            "crouching states runtime",
            "pose",
        )?,
        parent_graph: node_pose(
            player,
            &assets.source,
            source,
            "Crouching States\nState Machine",
            "Pose",
        )?,
        root: player.get_default_output_pose()?.clone(),
    })
}

fn max_pose_rotation_delta(previous: &Pose, current: &Pose) -> f32 {
    previous
        .bones
        .iter()
        .zip(&current.bones)
        .filter_map(|(left, right)| left.rotation.zip(right.rotation))
        .map(|(left, right)| 2.0 * left.dot(right).abs().clamp(0.0, 1.0).acos())
        .fold(0.0_f32, f32::max)
}

fn chain_deltas(previous: &ChainPoses, current: &ChainPoses) -> [(&'static str, f32); 7] {
    [
        (
            "sequence_player",
            max_pose_rotation_delta(&previous.sequence_player, &current.sequence_player),
        ),
        (
            "cache_producer",
            max_pose_rotation_delta(&previous.cache_producer, &current.cache_producer),
        ),
        (
            "cache_consumer",
            max_pose_rotation_delta(&previous.cache_consumer, &current.cache_consumer),
        ),
        (
            "movement_fsm",
            max_pose_rotation_delta(&previous.movement_fsm, &current.movement_fsm),
        ),
        (
            "outer_fsm",
            max_pose_rotation_delta(&previous.outer_fsm, &current.outer_fsm),
        ),
        (
            "parent_graph",
            max_pose_rotation_delta(&previous.parent_graph, &current.parent_graph),
        ),
        (
            "root",
            max_pose_rotation_delta(&previous.root, &current.root),
        ),
    ]
}

fn observe(
    mut probe: ResMut<Probe>,
    mut players: Query<&mut AnimationGraphPlayer>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    state_machines: Res<Assets<StateMachine>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(entity) = probe.entity else {
        return;
    };
    let Ok(mut player) = players.get_mut(entity) else {
        return;
    };
    if let Some(error) = player.get_error() {
        fail(&mut exit, 4, &format!("graph error={error:?}"));
        return;
    }
    match probe.phase {
        Phase::Pending => {}
        Phase::VerifyNoEvent => {
            let (Some(outer_runtime), Some(outer_fsm)) = (
                graphs.get(&assets.outer_runtime),
                state_machines.get(&assets.outer_fsm),
            ) else {
                return;
            };
            let Some(label) =
                outer_state_label(&player, outer_runtime, &assets.outer_runtime, outer_fsm)
            else {
                return;
            };
            if label != "Idle" {
                fail(
                    &mut exit,
                    4,
                    &format!("missing transition driver changed source entry state to {label}"),
                );
                return;
            }
            player.set_global_input_data("als_crouching_transition_index", 8.0);
            player.set_global_input_data("als_crouching_state_index", 1.0);
            player.set_global_input_data("als_crouching_movement_state_index", 0.0);
            player.set_global_input_data("als_crouching_direction_index", 0.0);
            player.set_global_input_data("als_crouching_curve_pose_moving", 1.0);
            player.queue_time_update(TimeUpdate::Delta(0.2));
            probe.move_steps = 1;
            probe.phase = Phase::EnterMove;
        }
        Phase::EnterMove => {
            player.set_global_input_data("als_crouching_transition_index", -1.0);
            player.queue_time_update(TimeUpdate::Delta(0.2));
            probe.move_steps += 1;
            if probe.move_steps < 4 {
                return;
            }
            let Some(snapshot) = chain_snapshot(&player, &assets, &graphs) else {
                fail(&mut exit, 5, "source chain baseline is unavailable");
                return;
            };
            probe.baseline = Some(snapshot);
            player.set_global_input_data("als_crouching_movement_marker_phase", 0.5);
            player.queue_time_update(TimeUpdate::Delta(0.0));
            probe.phase = Phase::SampleMarkerMotion;
        }
        Phase::SampleMarkerMotion => {
            let Some(current) = chain_snapshot(&player, &assets, &graphs) else {
                fail(&mut exit, 6, "source chain marker sample is unavailable");
                return;
            };
            let Some(previous) = probe.baseline.as_ref() else {
                fail(&mut exit, 6, "source chain baseline is missing");
                return;
            };
            let deltas = chain_deltas(previous, &current);
            let failures = deltas
                .iter()
                .filter(|(_, value)| *value < SOURCE_MARKER_ROTATION_FLOOR)
                .map(|(name, value)| format!("{name}={value:.6}"))
                .collect::<Vec<_>>();
            if !failures.is_empty() {
                fail(
                    &mut exit,
                    7,
                    &format!(
                        "source marker motion did not cross every cache/FSM/root boundary: {}",
                        failures.join(", ")
                    ),
                );
                return;
            }
            let values = deltas
                .iter()
                .map(|(name, value)| format!("{name}={value:.6}"))
                .collect::<Vec<_>>();
            info!(
                "[CROUCHING-CONTRACT][VERDICT] sequence/cache/movement-fsm/outer-fsm/root {} -> PASS",
                values.join(", ")
            );
            exit.write(AppExit::Success);
        }
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, 10, "timeout");
    }
}
