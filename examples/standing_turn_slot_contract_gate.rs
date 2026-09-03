use std::time::{Duration, Instant};

use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::{
    AnimationGraphPlugin,
    core::{
        animation_graph::{AnimationGraph, TimeUpdate},
        animation_graph_player::AnimationGraphPlayer,
        edge_data::DataValue,
        plugin::AnimationGraphSet,
        skeleton::Skeleton,
    },
};

use als_ext::{AlsExtPlugin, InertializationPassthroughNode, SourceSlotNode};

const STANDING_PATH: &str = "graphs/ab_als/stances/standing.animgraph.ron";
const IDLE_PATH: &str = "graphs/ab_als/stances/standing/state_idle.animgraph.ron";
const TURN_PATH: &str = "graphs/ab_als/stances/standing/turn.animgraph.ron";
const CROUCHING_PATH: &str = "graphs/ab_als/stances/crouching.animgraph.ron";
const CROUCHING_IDLE_PATH: &str = "graphs/ab_als/stances/crouching/state_idle.animgraph.ron";
const CROUCHING_TURN_PATH: &str = "graphs/ab_als/stances/crouching/turn.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";

#[derive(Clone, Copy)]
struct TurnSlotCase {
    label: &'static str,
    graph_path: &'static str,
    idle_path: &'static str,
    turn_path: &'static str,
    slot_name: &'static str,
    turn_index_node: &'static str,
    turn_active_node: &'static str,
    index_key: &'static str,
    weight_key: &'static str,
    layer_weight_key: &'static str,
    pose_key: &'static str,
    play_rate_key: &'static str,
    inertialization_epoch_key: &'static str,
    inertialization_duration_key: &'static str,
    crouching: bool,
}

const STANDING_CASE: TurnSlotCase = TurnSlotCase {
    label: "standing",
    graph_path: STANDING_PATH,
    idle_path: IDLE_PATH,
    turn_path: TURN_PATH,
    slot_name: "Slot 'TurnInPlaceStanding'\nGroup 'Grounded'",
    turn_index_node: "standing turn index",
    turn_active_node: "standing turn active",
    index_key: "als_standing_turn_index",
    weight_key: "als_standing_turn_weight",
    layer_weight_key: "als_standing_turn_layer_weight",
    pose_key: "als_standing_turn_pose",
    play_rate_key: "als_standing_turn_play_rate",
    inertialization_epoch_key: "als_standing_turn_inertialization_epoch",
    inertialization_duration_key: "als_standing_turn_inertialization_duration",
    crouching: false,
};

const CROUCHING_CASE: TurnSlotCase = TurnSlotCase {
    label: "crouching",
    graph_path: CROUCHING_PATH,
    idle_path: CROUCHING_IDLE_PATH,
    turn_path: CROUCHING_TURN_PATH,
    slot_name: "Slot 'TurnInPlaceCrouching'\nGroup 'Grounded'",
    turn_index_node: "crouching turn index",
    turn_active_node: "crouching turn active",
    index_key: "als_crouching_turn_index",
    weight_key: "als_crouching_turn_weight",
    layer_weight_key: "als_crouching_turn_layer_weight",
    pose_key: "als_crouching_turn_pose",
    play_rate_key: "als_crouching_turn_play_rate",
    inertialization_epoch_key: "als_crouching_turn_inertialization_epoch",
    inertialization_duration_key: "als_crouching_turn_inertialization_duration",
    crouching: true,
};

fn turn_slot_case() -> &'static TurnSlotCase {
    match std::env::var("ALS_TURN_SLOT_CASE").as_deref() {
        Ok("crouching") => &CROUCHING_CASE,
        Ok("standing") | Err(_) => &STANDING_CASE,
        Ok(value) => panic!("unsupported ALS_TURN_SLOT_CASE: {value}"),
    }
}

#[derive(Resource)]
struct GateAssets {
    standing: Handle<AnimationGraph>,
    idle: Handle<AnimationGraph>,
    turn: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource, Default)]
struct GateState {
    standing: Option<Entity>,
    turn: Option<Entity>,
    phase: u8,
    baseline: Option<bevy_animation_graph::core::pose::Pose>,
    mounted: bool,
    inertialized: bool,
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
        .init_resource::<GateState>()
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(15)))
        .add_systems(Startup, load_assets)
        .add_systems(Update, setup)
        .add_systems(
            PostUpdate,
            (observe, timeout).chain().after(AnimationGraphSet::Final),
        )
        .run();
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    let case = turn_slot_case();
    commands.insert_resource(GateAssets {
        standing: assets.load(case.graph_path),
        idle: assets.load(case.idle_path),
        turn: assets.load(case.turn_path),
        skeleton: assets.load(SKELETON_PATH),
    });
}

fn set_turn_globals(
    player: &mut AnimationGraphPlayer,
    case: &TurnSlotCase,
    index: f32,
    weight: f32,
    rate: f32,
) {
    player.set_global_input_data(case.index_key, index);
    player.set_global_input_data(case.weight_key, weight);
    player.set_global_input_data(case.play_rate_key, rate);
}

fn set_standing_globals(player: &mut AnimationGraphPlayer) {
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
        ("als_standing_gait_is_sprinting", 0.0),
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
        ("als_standing_turn_inertialization_epoch", 0.0),
        ("als_standing_turn_inertialization_duration", 0.2),
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

fn set_crouching_globals(player: &mut AnimationGraphPlayer) {
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
        ("als_crouching_delta_seconds", 0.0),
        ("als_crouching_rotation_play_rate", 1.15),
        ("als_crouching_rotate_clip_time", 0.0),
        ("als_crouching_turn_index", 0.0),
        ("als_crouching_turn_weight", 0.0),
        ("als_crouching_turn_layer_weight", 0.0),
        ("als_crouching_turn_play_rate", 1.2),
        ("als_crouching_turn_inertialization_epoch", 0.0),
        ("als_crouching_turn_inertialization_duration", 0.2),
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

fn set_case_globals(player: &mut AnimationGraphPlayer, case: &TurnSlotCase) {
    if case.crouching {
        set_crouching_globals(player);
    } else {
        set_standing_globals(player);
    }
}

fn setup(
    mut commands: Commands,
    graphs: Res<Assets<AnimationGraph>>,
    skeletons: Res<Assets<Skeleton>>,
    assets: Res<GateAssets>,
    mut state: ResMut<GateState>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.standing.is_some() {
        return;
    }
    let case = turn_slot_case();
    let (Some(standing_graph), Some(idle_graph), Some(turn_graph), Some(_skeleton)) = (
        graphs.get(&assets.standing),
        graphs.get(&assets.idle),
        graphs.get(&assets.turn),
        skeletons.get(&assets.skeleton),
    ) else {
        return;
    };
    let slot = idle_graph
        .nodes
        .values()
        .find(|node| node.name == case.slot_name)
        .and_then(|node| node.try_inner_downcast_ref::<SourceSlotNode>());
    let turn_inputs = [case.turn_index_node, case.turn_active_node]
        .into_iter()
        .all(|name| turn_graph.nodes.values().any(|node| node.name == name));
    let inertialization = standing_graph
        .nodes
        .values()
        .filter(|node| node.name == "Inertialization")
        .filter_map(|node| node.try_inner_downcast_ref::<InertializationPassthroughNode>())
        .any(|node| {
            node.request_epoch_key == case.inertialization_epoch_key
                && node.request_duration_key == case.inertialization_duration_key
        });
    if !slot.is_some_and(|slot| {
        slot.pose_key == case.pose_key
            && slot.weight_key == case.weight_key
            && slot.layer_weight_key == case.layer_weight_key
    }) || !turn_inputs
        || !inertialization
    {
        fail(&mut exit, 2, "source slot or turn graph contract");
        return;
    }
    let mut standing =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.standing.clone());
    set_case_globals(&mut standing, case);
    standing.pause();
    standing.queue_time_update(TimeUpdate::Delta(0.0));
    let mut turn =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.turn.clone());
    set_turn_globals(&mut turn, case, 3.0, 0.0, 1.2);
    turn.pause();
    turn.queue_time_update(TimeUpdate::Delta(0.0));
    state.standing = Some(
        commands
            .spawn((
                Name::new(format!("{}_turn_slot_root", case.label)),
                Children::default(),
                standing,
            ))
            .id(),
    );
    state.turn = Some(
        commands
            .spawn((
                Name::new(format!("{}_turn_slot_turn", case.label)),
                Children::default(),
                turn,
            ))
            .id(),
    );
    info!(
        "[TURN-SLOT] case={} source slot contract initialized",
        case.label
    );
}

fn output_pose(
    player: &AnimationGraphPlayer,
    name: &str,
) -> Option<bevy_animation_graph::core::pose::Pose> {
    match player.get_outputs().get(name) {
        Some(DataValue::Pose(pose)) => Some(pose.clone()),
        _ => None,
    }
}

fn pose_distance(
    first: &bevy_animation_graph::core::pose::Pose,
    second: &bevy_animation_graph::core::pose::Pose,
) -> f32 {
    first
        .paths
        .keys()
        .filter_map(|bone| {
            let first_bone = first.get_bone(*bone)?;
            let second_bone = second.get_bone(*bone)?;
            let translation = match (first_bone.translation, second_bone.translation) {
                (Some(left), Some(right)) => left.distance(right),
                _ => 0.0,
            };
            let rotation = match (first_bone.rotation, second_bone.rotation) {
                (Some(left), Some(right)) => 1.0 - left.dot(right).abs(),
                _ => 0.0,
            };
            let scale = match (first_bone.scale, second_bone.scale) {
                (Some(left), Some(right)) => left.distance(right),
                _ => 0.0,
            };
            Some(translation.max(rotation).max(scale))
        })
        .fold(0.0_f32, f32::max)
}

fn observe(
    mut state: ResMut<GateState>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let (Some(standing_entity), Some(turn_entity)) = (state.standing, state.turn) else {
        return;
    };
    let case = turn_slot_case();
    let Ok([mut standing, mut turn]) = players.get_many_mut([standing_entity, turn_entity]) else {
        return;
    };
    if let Some(error) = standing.get_error().or_else(|| turn.get_error()) {
        fail(&mut exit, 3, &format!("graph error={error:?}"));
        return;
    }
    match state.phase {
        0 => {
            let Some(baseline) = output_pose(&standing, "Result") else {
                standing.queue_time_update(TimeUpdate::Delta(0.0));
                return;
            };
            state.baseline = Some(baseline);
            set_turn_globals(&mut turn, case, 3.0, 1.0, 1.2);
            turn.queue_time_update(TimeUpdate::Delta(0.0));
            state.phase = 1;
        }
        1 => {
            if output_pose(&turn, "pose").is_none() {
                turn.queue_time_update(TimeUpdate::Delta(0.0));
                return;
            }
            turn.queue_time_update(TimeUpdate::Delta(1.0));
            state.phase = 2;
        }
        2 => {
            let Some(turn_pose) = output_pose(&turn, "pose") else {
                turn.queue_time_update(TimeUpdate::Delta(1.0));
                return;
            };
            standing.set_global_input_data(case.pose_key, DataValue::Pose(turn_pose));
            standing.set_global_input_data(case.weight_key, 1.0_f32);
            standing.set_global_input_data(case.layer_weight_key, 1.0_f32);
            standing.queue_time_update(TimeUpdate::Delta(0.0));
            state.phase = 3;
        }
        3 => {
            let (Some(baseline), Some(mounted)) =
                (state.baseline.as_ref(), output_pose(&standing, "Result"))
            else {
                standing.queue_time_update(TimeUpdate::Delta(0.1));
                return;
            };
            let mounted_distance = pose_distance(baseline, &mounted);
            state.mounted = mounted_distance > 1e-4;
            info!(
                "[TURN-SLOT] case={} mounted_distance={mounted_distance:.6}",
                case.label
            );
            standing.set_global_input_data(case.weight_key, 0.0_f32);
            standing.set_global_input_data(case.layer_weight_key, 0.0_f32);
            standing.set_global_input_data(case.inertialization_epoch_key, 1.0_f32);
            standing.set_global_input_data(case.inertialization_duration_key, 0.2_f32);
            standing.queue_time_update(TimeUpdate::Delta(0.0));
            state.phase = 4;
        }
        4 => {
            let (Some(baseline), Some(inertial)) =
                (state.baseline.as_ref(), output_pose(&standing, "Result"))
            else {
                standing.queue_time_update(TimeUpdate::Delta(0.0));
                return;
            };
            let inertial_distance = pose_distance(baseline, &inertial);
            state.inertialized = inertial_distance > 1e-4;
            info!(
                "[TURN-SLOT] case={} inertial_distance={inertial_distance:.6}",
                case.label
            );
            standing.queue_time_update(TimeUpdate::Delta(0.25));
            state.phase = 5;
        }
        _ => {
            let (Some(baseline), Some(result)) =
                (state.baseline.as_ref(), output_pose(&standing, "Result"))
            else {
                standing.queue_time_update(TimeUpdate::Delta(0.25));
                return;
            };
            let settled = pose_distance(baseline, &result) <= 1e-3;
            info!(
                "[TURN-SLOT] case={} settled_distance={:.6}",
                case.label,
                pose_distance(baseline, &result)
            );
            if state.mounted && state.inertialized && settled {
                warn!(
                    "[TURN-SLOT][VERDICT] case={} slot_pose_inertialized_then_settled=true -> PASS",
                    case.label
                );
                exit.write(AppExit::Success);
            } else {
                fail(
                    &mut exit,
                    4,
                    &format!(
                        "case={} slot_mounted={} inertia_active={} settled={}",
                        case.label, state.mounted, state.inertialized, settled
                    ),
                );
            }
        }
    }
}

fn timeout(state: Res<GateState>, deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        fail(
            &mut exit,
            5,
            &format!(
                "timeout phase={} mounted={} inertialized={}",
                state.phase, state.mounted, state.inertialized
            ),
        );
    }
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!(
        "[TURN-SLOT][VERDICT] case={} {message} -> FAIL",
        turn_slot_case().label
    );
    exit.write(AppExit::Error(std::num::NonZeroU8::new(code).unwrap()));
    std::process::exit(i32::from(code));
}
