use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

use bevy_als::extensions::{
    AlsExtPlugin, CurveModifyNode, DirectTransitionEventNode, GlobalBoneMaskInputNode,
    GlobalF32InputNode, InertializationNode, InterruptibleFsmNode, InterruptibleFsmState,
    LayeredGraphNode, ProfiledBlendNode, TimePositionNode,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    context::node_states::StateKey,
    edge_data::{DataValue, bone_mask::BoneMask},
    plugin::AnimationGraphSet,
    state_machine::{
        high_level::{StateMachine, TransitionId, TransitionKind},
        low_level::LowLevelStateId,
    },
};

const ROOT_PATH: &str = "Validation/Graphs/ab_als/grounded.animgraph.ron";
const FSM_PATH: &str = "Validation/Graphs/ab_als/grounded/grounded_states.fsm.ron";
const SKELETON_PATH: &str = "ALS/Character/SKM_Als.skn.ron";
const GROUNDED_GRAPH_DIR: &str = "assets/graphs/ab_als/grounded";

#[derive(Resource)]
struct GateAssets {
    root: Handle<AnimationGraph>,
    roll: Handle<AnimationGraph>,
    fsm: Handle<StateMachine>,
    skeleton: Handle<bevy_animation_graph::core::skeleton::Skeleton>,
}

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

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource, Default)]
struct Probe {
    static_verified: bool,
    curves_verified: bool,
    roll_player: Option<Entity>,
    roll_verified: bool,
    cases: Vec<CaseRuntime>,
    seen: BTreeSet<usize>,
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
        roll: assets.load("Validation/Graphs/ab_als/grounded/state_roll.animgraph.ron"),
        fsm: assets.load(FSM_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
    let seconds = std::env::var("GROUNDED_GATE_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(45);
    commands.insert_resource(Deadline(Instant::now() + Duration::from_secs(seconds)));
    info!("[GROUNDED-CONTRACT] loading linked grounded layer assets");
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!("[GROUNDED-CONTRACT][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(code).unwrap()));
}

fn node<'a, T: 'static>(graph: &'a AnimationGraph, name: &str) -> Option<&'a T> {
    graph
        .nodes
        .values()
        .find(|node| node.name == name)
        .and_then(|node| node.try_inner_downcast_ref::<T>())
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
    let mut files = vec!["assets/Validation/Graphs/ab_als/grounded.animgraph.ron".to_owned()];
    if let Ok(entries) = std::fs::read_dir(GROUNDED_GRAPH_DIR) {
        files.extend(entries.filter_map(Result::ok).filter_map(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "ron")
                .then(|| entry.path().to_string_lossy().into_owned())
        }));
    }
    files
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
    for (key, value) in [
        ("als_standing_transition_index", -1.0),
        ("als_standing_movement_transition_index", -1.0),
        ("als_standing_movement_detail_transition_index", -1.0),
        ("als_crouching_transition_index", -1.0),
        ("als_crouching_movement_transition_index", -1.0),
        ("als_standing_curve_pose_standing", 1.0),
        ("als_standing_curve_pose_moving", 0.0),
        ("als_standing_curve_foot_left_lock", 1.0),
        ("als_standing_curve_foot_right_lock", 1.0),
        ("als_standing_curve_allow_transitions", 1.0),
        ("als_crouching_curve_pose_crouching", 1.0),
        ("als_crouching_curve_pose_moving", 0.0),
        ("als_crouching_curve_foot_left_lock", 1.0),
        ("als_crouching_curve_foot_right_lock", 1.0),
        ("als_crouching_curve_allow_transitions", 1.0),
        ("als_grounded_transition_index", -1.0),
        ("als_grounded_entry_index", 3.0),
        ("als_grounded_stance_change_alpha", 0.5),
        ("als_grounded_curve_PoseStanding", 0.5),
        ("als_grounded_curve_PoseCrouching", 0.5),
        ("als_grounded_curve_PoseMoving", 0.0),
        ("als_grounded_curve_FootLeftLock", 1.0),
        ("als_grounded_curve_FootRightLock", 1.0),
        ("als_grounded_curve_PoseGroundedInput", 0.0),
        ("als_grounded_curve_FootLeftIkInput", 0.0),
        ("als_grounded_curve_FootRightIkInput", 0.0),
    ] {
        player.set_global_input_data(key, value);
    }
}

fn spawn_case(
    commands: &mut Commands,
    assets: &GateAssets,
    graphs: &Assets<AnimationGraph>,
    suffix: &str,
    steps: &[Step],
) -> CaseRuntime {
    let mut player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.root.clone());
    set_default_globals(&mut player, graphs);
    player.queue_time_update(TimeUpdate::Delta(0.0));
    let entity = commands
        .spawn((
            Name::new(format!("grounded_case_{suffix}")),
            Children::default(),
            player,
        ))
        .id();
    CaseRuntime {
        player: entity,
        steps: steps.to_vec(),
        cursor: 0,
        pending: false,
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
        || !server.is_loaded_with_dependencies(&assets.roll)
    {
        return;
    }
    let (Some(root), Some(fsm)) = (graphs.get(&assets.root), machines.get(&assets.fsm)) else {
        return;
    };
    let root_contract = node::<InterruptibleFsmNode>(root, "Grounded States").is_some()
        && node::<DirectTransitionEventNode>(root, "grounded transition driver")
            .is_some_and(|node| node.transitions.len() == 20)
        && node::<LayeredGraphNode>(root, "Standing Input linked graph").is_some()
        && node::<LayeredGraphNode>(root, "Crouching Input linked graph").is_some();
    let state_contract = labels(fsm)
        == vec![
            "Crouching",
            "Crouching To Standing",
            "Entry Conduit",
            "Roll",
            "Standing",
            "Standing To Crouching",
        ]
        && state_positions_match(
            fsm,
            &[
                ("Entry Conduit", (176.0, 0.0)),
                ("Standing", (416.0, -128.0)),
                ("Crouching", (416.0, 128.0)),
                ("Standing To Crouching", (416.0, 256.0)),
                ("Crouching To Standing", (416.0, -256.0)),
                ("Roll", (416.0, 0.0)),
            ],
        )
        && fsm.transitions.len() == 20
        && transition_count(fsm, "Entry Conduit", "Roll", Some(0.2)) == 1
        && transition_count(fsm, "Entry Conduit", "Crouching", Some(0.2)) == 1
        && transition_count(fsm, "Entry Conduit", "Standing", Some(0.2)) == 2
        && transition_count(fsm, "Standing", "Crouching", Some(0.5)) == 1
        && transition_count(fsm, "Standing", "Standing To Crouching", Some(0.3)) == 1
        && transition_count(fsm, "Crouching", "Standing", Some(0.5)) == 1
        && transition_count(fsm, "Crouching", "Crouching To Standing", Some(0.3)) == 1
        && transition_count(fsm, "Standing To Crouching", "Crouching", None) == 1
        && transition_count(fsm, "Standing To Crouching", "Standing", Some(0.5)) == 1
        && transition_count(fsm, "Standing To Crouching", "Crouching", Some(0.5)) == 1
        && transition_count(
            fsm,
            "Standing To Crouching",
            "Crouching To Standing",
            Some(0.3),
        ) == 1
        && transition_count(
            fsm,
            "Standing To Crouching",
            "Standing To Crouching",
            Some(0.3),
        ) == 1
        && transition_count(fsm, "Crouching To Standing", "Standing", None) == 1
        && transition_count(fsm, "Crouching To Standing", "Standing", Some(0.5)) == 1
        && transition_count(fsm, "Crouching To Standing", "Crouching", Some(0.5)) == 1
        && transition_count(
            fsm,
            "Crouching To Standing",
            "Crouching To Standing",
            Some(0.3),
        ) == 1
        && transition_count(
            fsm,
            "Crouching To Standing",
            "Standing To Crouching",
            Some(0.3),
        ) == 1
        && transition_count(fsm, "Roll", "Crouching", Some(0.6)) == 1
        && transition_count(fsm, "Roll", "Standing", Some(0.6)) == 1;
    let curve_contract =
        node::<CurveModifyNode>(root, "grounded root curve modify").is_some_and(|node| {
            node.curve_names == ["PoseGrounded", "FootLeftIk", "FootRightIk"]
                && node.values == [1.0, 1.0, 1.0]
                && close(node.alpha, 1.0)
        });
    let inertial_contract = root
        .nodes
        .values()
        .filter_map(|node| node.try_inner_downcast_ref::<LayeredGraphNode>())
        .count()
        >= 2
        && graphs.iter().any(|(_, graph)| {
            node::<InertializationNode>(graph, "grounded inertial pose")
                .is_some_and(|node| close(node.transition_duration, 0.3))
        });
    let evaluator_contract = graphs.iter().any(|(_, graph)| {
        node::<TimePositionNode>(graph, "roll evaluator")
            .is_some_and(|node| close(node.duration_fallback, 1.5) && !node.loop_position)
    }) && graphs.iter().any(|(_, graph)| {
        node::<ProfiledBlendNode>(graph, "grounded stance change custom curve").is_some()
    });
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
        "grounded_state",
        "transition_index",
        "entry_index",
        "clip_index",
        "loop",
        "play_rate",
        "transition_duration",
    ]
    .into_iter()
    .all(|name| root.io_spec.get_output_data(&name.to_owned()).is_some());
    let source = source_files();
    let architecture = !source.is_empty()
        && source.iter().all(|text| {
            !text.contains("AB_Als_Monolithic")
                && !text.contains("locomotion_overlay")
                && !text.contains("manual")
                && !text.contains("playground")
        });
    info!(
        "[GROUNDED-CONTRACT] root={} states={} curves={} inertial={} evaluator={} outputs={} architecture={}",
        root_contract,
        state_contract,
        curve_contract,
        inertial_contract,
        evaluator_contract,
        outputs,
        architecture
    );
    if !(root_contract
        && state_contract
        && curve_contract
        && inertial_contract
        && evaluator_contract
        && outputs
        && architecture)
    {
        fail(&mut exit, 3, "static contract");
        return;
    }
    let cases = [
        vec![
            Step {
                index: 0,
                source: "Entry Conduit",
                target: "Roll",
            },
            Step {
                index: 19,
                source: "Roll",
                target: "Standing",
            },
            Step {
                index: 4,
                source: "Standing",
                target: "Crouching",
            },
        ],
        vec![
            Step {
                index: 0,
                source: "Entry Conduit",
                target: "Roll",
            },
            Step {
                index: 18,
                source: "Roll",
                target: "Crouching",
            },
            Step {
                index: 6,
                source: "Crouching",
                target: "Standing",
            },
        ],
        vec![
            Step {
                index: 1,
                source: "Entry Conduit",
                target: "Crouching",
            },
            Step {
                index: 7,
                source: "Crouching",
                target: "Crouching To Standing",
            },
            Step {
                index: 13,
                source: "Crouching To Standing",
                target: "Standing",
            },
        ],
        vec![
            Step {
                index: 1,
                source: "Entry Conduit",
                target: "Crouching",
            },
            Step {
                index: 7,
                source: "Crouching",
                target: "Crouching To Standing",
            },
            Step {
                index: 14,
                source: "Crouching To Standing",
                target: "Standing",
            },
            Step {
                index: 5,
                source: "Standing",
                target: "Standing To Crouching",
            },
            Step {
                index: 8,
                source: "Standing To Crouching",
                target: "Crouching",
            },
        ],
        vec![
            Step {
                index: 2,
                source: "Entry Conduit",
                target: "Standing",
            },
            Step {
                index: 5,
                source: "Standing",
                target: "Standing To Crouching",
            },
            Step {
                index: 9,
                source: "Standing To Crouching",
                target: "Standing",
            },
        ],
        vec![
            Step {
                index: 3,
                source: "Entry Conduit",
                target: "Standing",
            },
            Step {
                index: 5,
                source: "Standing",
                target: "Standing To Crouching",
            },
            Step {
                index: 10,
                source: "Standing To Crouching",
                target: "Crouching",
            },
            Step {
                index: 7,
                source: "Crouching",
                target: "Crouching To Standing",
            },
            Step {
                index: 15,
                source: "Crouching To Standing",
                target: "Crouching",
            },
        ],
        vec![
            Step {
                index: 2,
                source: "Entry Conduit",
                target: "Standing",
            },
            Step {
                index: 5,
                source: "Standing",
                target: "Standing To Crouching",
            },
            Step {
                index: 11,
                source: "Standing To Crouching",
                target: "Crouching To Standing",
            },
            Step {
                index: 15,
                source: "Crouching To Standing",
                target: "Crouching",
            },
        ],
        vec![
            Step {
                index: 1,
                source: "Entry Conduit",
                target: "Crouching",
            },
            Step {
                index: 7,
                source: "Crouching",
                target: "Crouching To Standing",
            },
            Step {
                index: 17,
                source: "Crouching To Standing",
                target: "Standing To Crouching",
            },
            Step {
                index: 10,
                source: "Standing To Crouching",
                target: "Crouching",
            },
        ],
    ];
    probe.cases = cases
        .iter()
        .enumerate()
        .map(|(index, steps)| {
            spawn_case(&mut commands, &assets, &graphs, &index.to_string(), steps)
        })
        .collect();
    let mut roll_player =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.roll.clone());
    set_default_globals(&mut roll_player, &graphs);
    roll_player.set_global_input_data("als_root_defaults_enabled", true);
    roll_player.queue_time_update(TimeUpdate::Delta(0.0));
    probe.roll_player = Some(
        commands
            .spawn((
                Name::new("grounded_roll_endpoint_probe"),
                Children::default(),
                roll_player,
            ))
            .id(),
    );
    probe.static_verified = true;
    info!("[GROUNDED-CONTRACT] linked runtime probes initialized");
}

fn fsm_state<'a>(
    player: &'a AnimationGraphPlayer,
    graph: &AnimationGraph,
) -> Option<&'a InterruptibleFsmState> {
    let id = graph
        .nodes
        .values()
        .find(|node| node.name == "Grounded States")?
        .id;
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
) -> Option<String> {
    match &fsm_state(player, graph)?.state {
        LowLevelStateId::HlState(id) => machine.states.get(id).map(|state| state.label.clone()),
        _ => None,
    }
}

fn output(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    match player.get_outputs().get(name) {
        Some(DataValue::F32(value)) => Some(*value),
        _ => None,
    }
}

fn advance(player: &mut AnimationGraphPlayer, time: f32) {
    player.queue_time_update(TimeUpdate::Absolute(time));
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
    let (Some(root), Some(machine)) = (graphs.get(&assets.root), machines.get(&assets.fsm)) else {
        return;
    };
    if !probe.roll_verified {
        let Some(roll_entity) = probe.roll_player else {
            return;
        };
        let Ok(roll_player) = players.get(roll_entity) else {
            return;
        };
        if let Some(error) = roll_player.get_error() {
            fail(
                &mut exit,
                4,
                &format!("roll evaluator runtime error={error:?}"),
            );
            return;
        }
        let Some(DataValue::Pose(pose)) = roll_player.get_outputs().get("pose") else {
            return;
        };
        if !close(pose.timestamp, 1.5) {
            fail(&mut exit, 4, "roll evaluator endpoint clamp");
            return;
        }
        probe.roll_verified = true;
        info!("[GROUNDED-CONTRACT] roll evaluator explicit frame=45 endpoint=1.5 clamp=true");
    }
    if !probe.curves_verified {
        let Some(case) = probe.cases.first() else {
            return;
        };
        let Ok(player) = players.get(case.player) else {
            return;
        };
        let curves = [
            output(&player, "PoseGrounded").is_some_and(|value| close(value, 1.0)),
            output(&player, "FootLeftIk").is_some_and(|value| close(value, 1.0)),
            output(&player, "FootRightIk").is_some_and(|value| close(value, 1.0)),
            output(&player, "PoseStanding").is_some_and(|value| close(value, 1.0)),
        ]
        .into_iter()
        .all(|value| value);
        if !curves {
            fail(&mut exit, 4, "root curve runtime contract");
            return;
        }
        probe.curves_verified = true;
        info!("[GROUNDED-CONTRACT] root curves=PoseGrounded/FootLeftIk/FootRightIk values=1");
    }
    let mut newly_seen = Vec::new();
    for case in &mut probe.cases {
        let Ok(mut player) = players.get_mut(case.player) else {
            continue;
        };
        if let Some(error) = player.get_error() {
            fail(&mut exit, 4, &format!("runtime error={error:?}"));
            return;
        }
        let Some(state) = fsm_state(&player, root) else {
            continue;
        };
        if case.pending {
            let step = case.steps[case.cursor];
            let expected = node::<DirectTransitionEventNode>(root, "grounded transition driver")
                .and_then(|driver| driver.transitions.get(step.index))
                .copied();
            let active = matches!(&state.state, LowLevelStateId::HlTransition(TransitionId::Direct(id)) if Some(id) == expected.as_ref());
            let immediate = fsm_label(&player, root, machine).as_deref() == Some(step.target);
            if !active && !immediate {
                fail(&mut exit, 4, "direct transition did not activate");
                return;
            }
            player.set_global_input_data("als_grounded_transition_index", -1.0_f32);
            advance(&mut player, case.cursor as f32 * 10.0 + 2.0);
            newly_seen.push(step.index);
            case.pending = false;
            case.cursor += 1;
            continue;
        }
        if matches!(state.state, LowLevelStateId::HlTransition(_)) {
            advance(&mut player, case.cursor as f32 * 10.0 + 3.0);
            continue;
        }
        if case.cursor >= case.steps.len() {
            continue;
        }
        let step = case.steps[case.cursor];
        if fsm_label(&player, root, machine).as_deref() != Some(step.source) {
            fail(&mut exit, 4, "direct transition source state");
            return;
        }
        player.set_global_input_data("als_grounded_transition_index", step.index as f32);
        advance(&mut player, case.cursor as f32 * 10.0 + 1.0);
        case.pending = true;
        info!(
            "[GROUNDED-CONTRACT] direct={} {}->{}",
            step.index, step.source, step.target
        );
    }
    probe.seen.extend(newly_seen);
    if !probe.roll_verified
        || probe
            .cases
            .iter()
            .any(|case| case.cursor < case.steps.len() || case.pending)
    {
        return;
    }
    let expected: BTreeSet<_> = [
        0_usize, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 17, 18, 19,
    ]
    .into_iter()
    .collect();
    if probe.seen != expected {
        fail(&mut exit, 4, "runtime transition coverage");
        return;
    }
    info!(
        "[GROUNDED-CONTRACT] source executable direct topology=18/18 self_edges=2 source_rejected"
    );
    info!("[GROUNDED-CONTRACT][VERDICT] linked=true -> PASS");
    exit.write(AppExit::Success);
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(&mut exit, 5, "timeout");
    }
}
