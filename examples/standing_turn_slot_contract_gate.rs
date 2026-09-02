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
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";

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
    commands.insert_resource(GateAssets {
        standing: assets.load(STANDING_PATH),
        idle: assets.load(IDLE_PATH),
        turn: assets.load(TURN_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
}

fn set_turn_globals(player: &mut AnimationGraphPlayer, index: f32, weight: f32, rate: f32) {
    player.set_global_input_data("als_standing_turn_index", index);
    player.set_global_input_data("als_standing_turn_weight", weight);
    player.set_global_input_data("als_standing_turn_play_rate", rate);
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
        .find(|node| node.name == "Slot 'TurnInPlaceStanding'\nGroup 'Grounded'")
        .and_then(|node| node.try_inner_downcast_ref::<SourceSlotNode>());
    let turn_inputs = ["standing turn index", "standing turn active"]
        .into_iter()
        .all(|name| turn_graph.nodes.values().any(|node| node.name == name));
    let inertialization = standing_graph
        .nodes
        .values()
        .filter(|node| node.name == "Inertialization")
        .filter_map(|node| node.try_inner_downcast_ref::<InertializationPassthroughNode>())
        .any(|node| {
            node.request_epoch_key == "als_standing_turn_inertialization_epoch"
                && node.request_duration_key == "als_standing_turn_inertialization_duration"
        });
    if !slot.is_some_and(|slot| {
        slot.pose_key == "als_standing_turn_pose"
            && slot.weight_key == "als_standing_turn_weight"
            && slot.layer_weight_key == "als_standing_turn_layer_weight"
    }) || !turn_inputs
        || !inertialization
    {
        fail(&mut exit, 2, "source slot or turn graph contract");
        return;
    }
    let mut standing =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.standing.clone());
    set_standing_globals(&mut standing);
    standing.pause();
    standing.queue_time_update(TimeUpdate::Delta(0.0));
    let mut turn =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.turn.clone());
    set_turn_globals(&mut turn, 3.0, 0.0, 1.2);
    turn.pause();
    turn.queue_time_update(TimeUpdate::Delta(0.0));
    state.standing = Some(
        commands
            .spawn((
                Name::new("standing_turn_slot_root"),
                Children::default(),
                standing,
            ))
            .id(),
    );
    state.turn = Some(
        commands
            .spawn((
                Name::new("standing_turn_slot_turn"),
                Children::default(),
                turn,
            ))
            .id(),
    );
    info!("[STANDING-TURN-SLOT] source slot contract initialized");
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
            set_turn_globals(&mut turn, 3.0, 1.0, 1.2);
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
            standing.set_global_input_data("als_standing_turn_pose", DataValue::Pose(turn_pose));
            standing.set_global_input_data("als_standing_turn_weight", 1.0_f32);
            standing.set_global_input_data("als_standing_turn_layer_weight", 1.0_f32);
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
            info!("[STANDING-TURN-SLOT] mounted_distance={mounted_distance:.6}");
            standing.set_global_input_data("als_standing_turn_weight", 0.0_f32);
            standing.set_global_input_data("als_standing_turn_layer_weight", 0.0_f32);
            standing.set_global_input_data("als_standing_turn_inertialization_epoch", 1.0_f32);
            standing.set_global_input_data("als_standing_turn_inertialization_duration", 0.2_f32);
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
            info!("[STANDING-TURN-SLOT] inertial_distance={inertial_distance:.6}");
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
                "[STANDING-TURN-SLOT] settled_distance={:.6}",
                pose_distance(baseline, &result)
            );
            if state.mounted && state.inertialized && settled {
                warn!(
                    "[STANDING-TURN-SLOT][VERDICT] slot_pose_inertialized_then_settled=true -> PASS"
                );
                exit.write(AppExit::Success);
            } else {
                fail(
                    &mut exit,
                    4,
                    &format!(
                        "slot_mounted={} inertia_active={} settled={}",
                        state.mounted, state.inertialized, settled
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
    warn!("[STANDING-TURN-SLOT][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(code).unwrap()));
    std::process::exit(i32::from(code));
}
