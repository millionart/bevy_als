use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

use als_ext::{
    AlsExtPlugin, ConstantF32Node, DirectTransitionEventNode, GlobalBoneMaskInputNode,
    GlobalF32InputNode, InertializationNode, InterruptibleFsmNode, InterruptibleFsmState,
    LayeredGraphNode, ProfiledBlendNode, ResetOnActivationNode, TimePositionNode,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::builtin_nodes::{
    clip_node::ClipNode, loop_node::LoopNode, speed_node::SpeedNode,
};
use bevy_animation_graph::core::{
    animation_graph::AnimationGraph,
    animation_graph_player::AnimationGraphPlayer,
    context::node_states::StateKey,
    edge_data::{DataValue, bone_mask::BoneMask},
    plugin::AnimationGraphSet,
    state_machine::{
        high_level::{StateMachine, TransitionId, TransitionKind},
        low_level::LowLevelStateId,
    },
};

const ROOT_PATH: &str = "graphs/ab_als/locomotion.animgraph.ron";
const FSM_PATH: &str = "graphs/ab_als/locomotion/locomotion_states.fsm.ron";
const JUMP_PATH: &str = "graphs/ab_als/locomotion/state_jump.animgraph.ron";
const JUMP_FSM_PATH: &str = "graphs/ab_als/locomotion/jump_states.fsm.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";
const LOCOMOTION_GRAPH_DIR: &str = "assets/graphs/ab_als/locomotion";

#[derive(Resource)]
struct GateAssets {
    root: Handle<AnimationGraph>,
    fsm: Handle<StateMachine>,
    jump: Handle<AnimationGraph>,
    jump_fsm: Handle<StateMachine>,
    skeleton: Handle<bevy_animation_graph::core::skeleton::Skeleton>,
}

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Clone, Copy)]
struct Step {
    index: usize,
    source: &'static str,
    target: &'static str,
}

struct CaseRuntime {
    player: Entity,
    steps: Vec<Step>,
    cursor: usize,
    pending: bool,
}

#[derive(Resource, Default)]
struct Probe {
    outer_cases: Vec<CaseRuntime>,
    jump_cases: Vec<CaseRuntime>,
    static_verified: bool,
    curves_verified: bool,
    outer_seen: BTreeSet<usize>,
    jump_seen: BTreeSet<usize>,
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
        fsm: assets.load(FSM_PATH),
        jump: assets.load(JUMP_PATH),
        jump_fsm: assets.load(JUMP_FSM_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    commands.insert_resource(Deadline(Instant::now() + Duration::from_secs(30)));
    info!("[LOCOMOTION-CONTRACT] loading linked locomotion layer assets");
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

fn fail(exit: &mut MessageWriter<AppExit>, message: &str) {
    warn!("[LOCOMOTION-CONTRACT][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(4).unwrap()));
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

fn contains_loop(graph: &AnimationGraph) -> bool {
    graph
        .nodes
        .values()
        .any(|node| node.try_inner_downcast_ref::<LoopNode>().is_some())
}

fn constant_is(graph: &AnimationGraph, name: &str, expected: f32) -> bool {
    node::<ConstantF32Node>(graph, name).is_some_and(|node| close(node.value, expected))
}

fn labels(machine: &StateMachine) -> Vec<&str> {
    let mut labels: Vec<_> = machine
        .states
        .values()
        .map(|state| state.label.as_str())
        .collect();
    labels.sort_unstable();
    labels
}

fn transition_count(
    machine: &StateMachine,
    source: &str,
    target: &str,
    duration: Option<f32>,
) -> usize {
    let source = machine
        .states
        .values()
        .find(|state| state.label == source)
        .map(|state| state.id);
    let target = machine
        .states
        .values()
        .find(|state| state.label == target)
        .map(|state| state.id);
    match (source, target) {
        (Some(source), Some(target)) => machine
            .transitions
            .values()
            .filter(|transition| {
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
            .count(),
        _ => 0,
    }
}

fn source_files() -> Vec<String> {
    let mut paths = vec!["assets/graphs/ab_als/locomotion.animgraph.ron".to_owned()];
    if let Ok(entries) = std::fs::read_dir(LOCOMOTION_GRAPH_DIR) {
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

fn spawn_case(
    commands: &mut Commands,
    graph: Handle<AnimationGraph>,
    skeleton: Handle<bevy_animation_graph::core::skeleton::Skeleton>,
    graphs: &Assets<AnimationGraph>,
    name: String,
    steps: &[Step],
) -> CaseRuntime {
    let mut player = AnimationGraphPlayer::new(skeleton).with_graph(graph);
    set_default_globals(&mut player, graphs);
    player.queue_time_update(bevy_animation_graph::core::animation_graph::TimeUpdate::Delta(0.0));
    let entity = commands
        .spawn((Name::new(name), Children::default(), player))
        .id();
    CaseRuntime {
        player: entity,
        steps: steps.to_vec(),
        cursor: 0,
        pending: false,
    }
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
        _ => None,
    }
}

fn advance(player: &mut AnimationGraphPlayer, time: f32) {
    player
        .queue_time_update(bevy_animation_graph::core::animation_graph::TimeUpdate::Absolute(time));
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
    for (key, value) in [
        ("als_standing_transition_index", -1.0),
        ("als_standing_movement_transition_index", -1.0),
        ("als_standing_movement_detail_transition_index", -1.0),
        ("als_crouching_transition_index", -1.0),
        ("als_crouching_movement_transition_index", -1.0),
        ("als_grounded_transition_index", -1.0),
        ("als_locomotion_transition_index", -1.0),
        ("als_locomotion_jump_transition_index", -1.0),
        ("als_standing_curve_pose_standing", 1.0),
        ("als_standing_curve_foot_left_lock", 1.0),
        ("als_standing_curve_foot_right_lock", 1.0),
        ("als_standing_curve_allow_transitions", 1.0),
        ("als_crouching_curve_pose_crouching", 1.0),
        ("als_crouching_curve_foot_left_lock", 1.0),
        ("als_crouching_curve_foot_right_lock", 1.0),
        ("als_crouching_curve_allow_transitions", 1.0),
        ("als_grounded_curve_PoseGroundedInput", 0.0),
        ("als_grounded_curve_FootLeftIkInput", 0.0),
        ("als_grounded_curve_FootRightIkInput", 0.0),
    ] {
        player.set_global_input_data(key, value);
    }
}

fn verify_static(
    mut commands: Commands,
    server: Res<AssetServer>,
    assets: Res<GateAssets>,
    graphs: Res<Assets<AnimationGraph>>,
    machines: Res<Assets<StateMachine>>,
    mut probe: ResMut<Probe>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.static_verified
        || !server.is_loaded_with_dependencies(&assets.root)
        || !server.is_loaded_with_dependencies(&assets.jump)
    {
        return;
    }
    let (Some(root), Some(fsm), Some(jump), Some(jump_fsm)) = (
        graphs.get(&assets.root),
        machines.get(&assets.fsm),
        graphs.get(&assets.jump),
        machines.get(&assets.jump_fsm),
    ) else {
        return;
    };
    let root_contract = node::<LayeredGraphNode>(root, "Grounded Input linked graph").is_some()
        && node::<InterruptibleFsmNode>(root, "Locomotion States").is_some()
        && node::<DirectTransitionEventNode>(root, "locomotion transition driver")
            .is_some_and(|node| node.transitions.len() == 16);
    let state_contract = labels(fsm)
        == vec![
            "Fall",
            "Grounded",
            "Jump",
            "Land",
            "Land Conduct",
            "Land Movement",
        ]
        && fsm.transitions.len() == 16
        && transition_count(fsm, "Grounded", "Jump", Some(0.1)) == 1
        && transition_count(fsm, "Grounded", "Fall", Some(0.6)) == 1
        && transition_count(fsm, "Fall", "Land Conduct", Some(0.2)) == 1
        && transition_count(fsm, "Fall", "Grounded", Some(0.2)) == 1
        && transition_count(fsm, "Jump", "Land Conduct", Some(0.2)) == 1
        && transition_count(fsm, "Jump", "Grounded", Some(0.2)) == 1
        && transition_count(fsm, "Land", "Grounded", Some(0.2)) == 1
        && transition_count(fsm, "Land", "Grounded", Some(0.8)) == 1
        && transition_count(fsm, "Land", "Land Movement", Some(0.2)) == 1
        && transition_count(fsm, "Land", "Jump", Some(0.1)) == 1
        && transition_count(fsm, "Land", "Fall", Some(0.6)) == 1
        && transition_count(fsm, "Land Movement", "Grounded", Some(0.3)) == 1
        && transition_count(fsm, "Land Movement", "Jump", Some(0.1)) == 1
        && transition_count(fsm, "Land Movement", "Fall", Some(0.6)) == 1
        && transition_count(fsm, "Land Conduct", "Land Movement", Some(0.1)) == 1
        && transition_count(fsm, "Land Conduct", "Land", Some(0.1)) == 1;
    let jump_contract = node::<InterruptibleFsmNode>(jump, "Jump States").is_some()
        && node::<DirectTransitionEventNode>(jump, "jump transition driver")
            .is_some_and(|node| node.transitions.len() == 5)
        && labels(jump_fsm) == vec!["Entry Conduit", "Flail", "Left Foot", "Loop", "Right Foot"]
        && jump_fsm.transitions.len() == 5
        && transition_count(jump_fsm, "Entry Conduit", "Right Foot", Some(0.2)) == 1
        && transition_count(jump_fsm, "Entry Conduit", "Left Foot", Some(0.2)) == 1
        && transition_count(jump_fsm, "Left Foot", "Loop", Some(0.2)) == 1
        && transition_count(jump_fsm, "Right Foot", "Loop", Some(0.2)) == 1
        && transition_count(jump_fsm, "Loop", "Flail", Some(1.0)) == 1;
    let outputs = [
        "pose",
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
    ]
    .into_iter()
    .all(|name| root.io_spec.get_output_data(&name.to_owned()).is_some());
    let loop_contract =
        [
            (
                "jump walk left clip",
                "jump walk left reset",
                "jump run left reset",
                "jump walk left rate",
                "jump run left rate",
            ),
            (
                "jump walk right clip",
                "jump walk right reset",
                "jump run right reset",
                "jump walk right rate",
                "jump run right rate",
            ),
        ]
        .into_iter()
        .all(
            |(clip, first_reset, second_reset, first_rate, second_rate)| {
                graph_with_node::<ClipNode>(&graphs, clip).is_some_and(|graph| {
                    !contains_loop(graph)
                        && node::<ResetOnActivationNode>(graph, first_reset).is_some()
                        && node::<ResetOnActivationNode>(graph, second_reset).is_some()
                        && node::<SpeedNode>(graph, first_rate).is_some()
                        && node::<SpeedNode>(graph, second_rate).is_some()
                        && node::<GlobalF32InputNode>(graph, "jump source rate")
                            .is_some_and(|node| node.key == "als_locomotion_jump_play_rate")
                })
            },
        ) && graph_with_node::<ClipNode>(&graphs, "jump loop clip").is_some_and(|graph| {
            node::<LoopNode>(graph, "jump loop").is_some()
                && node::<SpeedNode>(graph, "jump loop rate").is_some()
                && node::<GlobalF32InputNode>(graph, "jump source rate")
                    .is_some_and(|node| node.key == "als_locomotion_jump_play_rate")
        }) && graph_with_node::<ClipNode>(&graphs, "jump flail clip").is_some_and(|graph| {
            node::<LoopNode>(graph, "jump flail loop").is_some()
                && node::<SpeedNode>(graph, "jump flail rate").is_some()
                && constant_is(graph, "jump flail source rate", 1.2)
        }) && graph_with_node::<ClipNode>(&graphs, "fall clip").is_some_and(|graph| {
            node::<ClipNode>(graph, "fall fast clip").is_some()
                && node::<ClipNode>(graph, "fall flail clip").is_some()
                && node::<LoopNode>(graph, "fall loop").is_some()
                && node::<LoopNode>(graph, "fall fast loop").is_some()
                && node::<LoopNode>(graph, "fall flail loop").is_some()
                && node::<SpeedNode>(graph, "fall flail rate").is_some()
                && constant_is(graph, "fall flail source rate", 1.0)
        }) && graph_with_node::<ClipNode>(&graphs, "land light clip").is_some_and(|graph| {
            !contains_loop(graph)
                && node::<ClipNode>(graph, "land heavy clip").is_some()
                && node::<ResetOnActivationNode>(graph, "land light reset").is_some()
                && node::<ResetOnActivationNode>(graph, "land heavy reset").is_some()
        }) && graph_with_node::<ClipNode>(&graphs, "land movement light additive clip")
            .is_some_and(|graph| {
                !contains_loop(graph)
                    && node::<ClipNode>(graph, "land movement heavy additive clip").is_some()
                    && node::<ResetOnActivationNode>(graph, "land movement light reset").is_some()
                    && node::<ResetOnActivationNode>(graph, "land movement heavy reset").is_some()
                    && node::<SpeedNode>(graph, "land movement light rate").is_some()
                    && node::<SpeedNode>(graph, "land movement heavy rate").is_some()
                    && constant_is(graph, "land movement light source rate", 1.75)
                    && constant_is(graph, "land movement heavy source rate", 1.5)
            })
            && graph_with_node::<ClipNode>(&graphs, "air lean center clip").is_some_and(|graph| {
                [
                    "air lean center loop",
                    "air lean forward loop",
                    "air lean backward loop",
                    "air lean left loop",
                    "air lean right loop",
                ]
                .into_iter()
                .all(|name| node::<LoopNode>(graph, name).is_some())
            });
    let curve_contract = graph_with_node::<GlobalF32InputNode>(&graphs, "PoseStanding")
        .is_some_and(|graph| {
            [
                ("PoseStanding", "als_locomotion_curve_PoseStanding"),
                ("PoseCrouching", "als_locomotion_curve_PoseCrouching"),
                ("PoseMoving", "als_locomotion_curve_PoseMoving"),
                (
                    "RotationYawOffset",
                    "als_locomotion_curve_RotationYawOffset",
                ),
                ("FootLeftLock", "als_locomotion_curve_FootLeftLock"),
                ("FootRightLock", "als_locomotion_curve_FootRightLock"),
                ("AllowTransitions", "als_locomotion_curve_AllowTransitions"),
                ("RotationYawSpeed", "als_locomotion_curve_RotationYawSpeed"),
                ("PoseGrounded", "als_locomotion_curve_PoseGrounded"),
                ("FootLeftIk", "als_locomotion_curve_FootLeftIk"),
                ("FootRightIk", "als_locomotion_curve_FootRightIk"),
                ("PoseInAir", "als_locomotion_curve_PoseInAir"),
            ]
            .into_iter()
            .all(|(name, key)| {
                node::<GlobalF32InputNode>(graph, name).is_some_and(|node| node.key == key)
            })
        });
    let node_contract = graphs
        .iter()
        .any(|(_, graph)| node::<InertializationNode>(graph, "locomotion inertial pose").is_some())
        && graphs.iter().any(|(_, graph)| {
            node::<ProfiledBlendNode>(graph, "locomotion quick feet transition").is_some()
        })
        && graphs
            .iter()
            .any(|(_, graph)| node::<ResetOnActivationNode>(graph, "land light reset").is_some())
        && graphs.iter().any(|(_, graph)| {
            node::<TimePositionNode>(graph, "fall land light evaluator")
                .is_some_and(|node| node.loop_position && close(node.duration_fallback, 1.1666666))
        })
        && graphs.iter().any(|(_, graph)| {
            node::<TimePositionNode>(graph, "jump land heavy evaluator")
                .is_some_and(|node| node.loop_position && close(node.duration_fallback, 1.1666666))
        });
    let source = source_files();
    let architecture = !source.is_empty()
        && source.iter().all(|text| {
            !text.contains("AB_Als_Monolithic")
                && !text.contains("locomotion_overlay")
                && !text.contains("graphs/air.animgraph.ron")
                && !text.contains("graphs/actions.animgraph.ron")
                && !text.contains("manual")
                && !text.contains("playground")
        });
    info!(
        "[LOCOMOTION-CONTRACT] root={} states={} jump={} loops={} curves={} nodes={} outputs={} architecture={}",
        root_contract,
        state_contract,
        jump_contract,
        loop_contract,
        curve_contract,
        node_contract,
        outputs,
        architecture
    );
    if !(root_contract
        && state_contract
        && jump_contract
        && loop_contract
        && curve_contract
        && node_contract
        && outputs
        && architecture)
    {
        fail(&mut exit, "static contract");
        return;
    }
    let outer_cases = [
        vec![
            Step {
                index: 0,
                source: "Grounded",
                target: "Jump",
            },
            Step {
                index: 4,
                source: "Jump",
                target: "Land Conduct",
            },
            Step {
                index: 14,
                source: "Land Conduct",
                target: "Land Movement",
            },
            Step {
                index: 11,
                source: "Land Movement",
                target: "Grounded",
            },
        ],
        vec![
            Step {
                index: 1,
                source: "Grounded",
                target: "Fall",
            },
            Step {
                index: 2,
                source: "Fall",
                target: "Land Conduct",
            },
            Step {
                index: 15,
                source: "Land Conduct",
                target: "Land",
            },
            Step {
                index: 6,
                source: "Land",
                target: "Grounded",
            },
        ],
        vec![
            Step {
                index: 1,
                source: "Grounded",
                target: "Fall",
            },
            Step {
                index: 3,
                source: "Fall",
                target: "Grounded",
            },
        ],
        vec![
            Step {
                index: 0,
                source: "Grounded",
                target: "Jump",
            },
            Step {
                index: 5,
                source: "Jump",
                target: "Grounded",
            },
        ],
        vec![
            Step {
                index: 1,
                source: "Grounded",
                target: "Fall",
            },
            Step {
                index: 2,
                source: "Fall",
                target: "Land Conduct",
            },
            Step {
                index: 15,
                source: "Land Conduct",
                target: "Land",
            },
            Step {
                index: 7,
                source: "Land",
                target: "Grounded",
            },
        ],
        vec![
            Step {
                index: 1,
                source: "Grounded",
                target: "Fall",
            },
            Step {
                index: 2,
                source: "Fall",
                target: "Land Conduct",
            },
            Step {
                index: 15,
                source: "Land Conduct",
                target: "Land",
            },
            Step {
                index: 8,
                source: "Land",
                target: "Land Movement",
            },
            Step {
                index: 12,
                source: "Land Movement",
                target: "Jump",
            },
        ],
        vec![
            Step {
                index: 1,
                source: "Grounded",
                target: "Fall",
            },
            Step {
                index: 2,
                source: "Fall",
                target: "Land Conduct",
            },
            Step {
                index: 15,
                source: "Land Conduct",
                target: "Land",
            },
            Step {
                index: 9,
                source: "Land",
                target: "Jump",
            },
        ],
        vec![
            Step {
                index: 1,
                source: "Grounded",
                target: "Fall",
            },
            Step {
                index: 2,
                source: "Fall",
                target: "Land Conduct",
            },
            Step {
                index: 15,
                source: "Land Conduct",
                target: "Land",
            },
            Step {
                index: 10,
                source: "Land",
                target: "Fall",
            },
        ],
        vec![
            Step {
                index: 1,
                source: "Grounded",
                target: "Fall",
            },
            Step {
                index: 2,
                source: "Fall",
                target: "Land Conduct",
            },
            Step {
                index: 14,
                source: "Land Conduct",
                target: "Land Movement",
            },
            Step {
                index: 13,
                source: "Land Movement",
                target: "Fall",
            },
        ],
    ];
    probe.outer_cases = outer_cases
        .iter()
        .enumerate()
        .map(|(index, steps)| {
            spawn_case(
                &mut commands,
                assets.root.clone(),
                assets.skeleton.clone(),
                &graphs,
                format!("locomotion_outer_case_{index}"),
                steps,
            )
        })
        .collect();
    let jump_cases = [
        vec![
            Step {
                index: 0,
                source: "Entry Conduit",
                target: "Right Foot",
            },
            Step {
                index: 3,
                source: "Right Foot",
                target: "Loop",
            },
        ],
        vec![
            Step {
                index: 1,
                source: "Entry Conduit",
                target: "Left Foot",
            },
            Step {
                index: 2,
                source: "Left Foot",
                target: "Loop",
            },
        ],
    ];
    probe.jump_cases = jump_cases
        .iter()
        .enumerate()
        .map(|(index, steps)| {
            spawn_case(
                &mut commands,
                assets.jump.clone(),
                assets.skeleton.clone(),
                &graphs,
                format!("locomotion_jump_case_{index}"),
                steps,
            )
        })
        .collect();
    probe.static_verified = true;
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
    let (Some(root), Some(fsm), Some(jump), Some(jump_fsm)) = (
        graphs.get(&assets.root),
        machines.get(&assets.fsm),
        graphs.get(&assets.jump),
        machines.get(&assets.jump_fsm),
    ) else {
        return;
    };
    if !probe.curves_verified {
        let Some(case) = probe.outer_cases.first() else {
            return;
        };
        let Ok(player) = players.get_mut(case.player) else {
            return;
        };
        if let Some(error) = player.get_error() {
            fail(&mut exit, &format!("runtime error={error:?}"));
            return;
        }
        let values = ["PoseGrounded", "FootLeftIk", "FootRightIk"]
            .into_iter()
            .all(|name| {
                matches!(player.get_outputs().get(name), Some(DataValue::F32(value)) if close(*value, 1.0))
            });
        if !values {
            return;
        }
        probe.curves_verified = true;
        info!("[LOCOMOTION-CONTRACT] grounded input pose/time/curves propagated");
    }
    let mut outer_seen = Vec::new();
    for case in &mut probe.outer_cases {
        if !drive_case(
            case,
            &mut players,
            root,
            fsm,
            "Locomotion States",
            "locomotion transition driver",
            "als_locomotion_transition_index",
            &mut outer_seen,
            &mut exit,
            "outer",
        ) {
            return;
        }
    }
    probe.outer_seen.extend(outer_seen);
    let mut jump_seen = Vec::new();
    for case in &mut probe.jump_cases {
        if !drive_case(
            case,
            &mut players,
            jump,
            jump_fsm,
            "Jump States",
            "jump transition driver",
            "als_locomotion_jump_transition_index",
            &mut jump_seen,
            &mut exit,
            "jump",
        ) {
            return;
        }
    }
    probe.jump_seen.extend(jump_seen);
    let outer_expected: BTreeSet<_> = (0_usize..16).collect();
    let jump_expected: BTreeSet<_> = (0_usize..4).collect();
    let done = probe
        .outer_cases
        .iter()
        .all(|case| case.cursor >= case.steps.len() && !case.pending)
        && probe
            .jump_cases
            .iter()
            .all(|case| case.cursor >= case.steps.len() && !case.pending);
    if !done {
        return;
    }
    if probe.outer_seen != outer_expected || probe.jump_seen != jump_expected {
        fail(&mut exit, "runtime topology coverage");
        return;
    }
    info!("[LOCOMOTION-CONTRACT] outer direct topology=16/16");
    info!("[LOCOMOTION-CONTRACT] jump executable topology=4/4 loop_to_flail=source_unreachable");
    info!("[LOCOMOTION-CONTRACT][VERDICT] linked=true -> PASS");
    exit.write(AppExit::Success);
}

#[allow(clippy::too_many_arguments)]
fn drive_case(
    case: &mut CaseRuntime,
    players: &mut Query<&mut AnimationGraphPlayer>,
    graph: &AnimationGraph,
    machine: &StateMachine,
    fsm_name: &str,
    driver_name: &str,
    input_key: &str,
    seen: &mut Vec<usize>,
    exit: &mut MessageWriter<AppExit>,
    name: &str,
) -> bool {
    let Ok(mut player) = players.get_mut(case.player) else {
        return true;
    };
    if let Some(error) = player.get_error() {
        fail(exit, &format!("{name} runtime error={error:?}"));
        return false;
    }
    let Some(state) = fsm_state(&player, graph, fsm_name) else {
        return true;
    };
    if case.pending {
        let step = case.steps[case.cursor];
        let expected = node::<DirectTransitionEventNode>(graph, driver_name)
            .and_then(|driver| driver.transitions.get(step.index))
            .copied();
        let active = matches!(&state.state, LowLevelStateId::HlTransition(TransitionId::Direct(id)) if Some(id) == expected.as_ref());
        let immediate =
            fsm_label(&player, graph, machine, fsm_name).as_deref() == Some(step.target);
        if !active && !immediate {
            fail(exit, &format!("{name} direct transition did not activate"));
            return false;
        }
        player.set_global_input_data(input_key, -1.0_f32);
        advance(&mut player, case.cursor as f32 * 10.0 + 2.0);
        seen.push(step.index);
        case.pending = false;
        case.cursor += 1;
        return true;
    }
    if matches!(state.state, LowLevelStateId::HlTransition(_)) {
        advance(&mut player, case.cursor as f32 * 10.0 + 3.0);
        return true;
    }
    if case.cursor >= case.steps.len() {
        return true;
    }
    let step = case.steps[case.cursor];
    if fsm_label(&player, graph, machine, fsm_name).as_deref() != Some(step.source) {
        fail(exit, &format!("{name} direct transition source state"));
        return false;
    }
    player.set_global_input_data(input_key, step.index as f32);
    advance(&mut player, case.cursor as f32 * 10.0 + 1.0);
    case.pending = true;
    info!(
        "[LOCOMOTION-CONTRACT] {name} direct={} {}->{}",
        step.index, step.source, step.target
    );
    true
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, "timeout");
    }
}
