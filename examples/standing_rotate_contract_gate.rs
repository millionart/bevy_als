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

use bevy_als::extensions::AlsExtPlugin;

const LEFT_PATH: &str = "graphs/ab_als/stances/standing/state_rotate_left.animgraph.ron";
const RIGHT_PATH: &str = "graphs/ab_als/stances/standing/state_rotate_right.animgraph.ron";
const SKELETON_PATH: &str = "skeletons/SKM_Als.skn.ron";

#[derive(Resource)]
struct GateAssets {
    left: Handle<AnimationGraph>,
    right: Handle<AnimationGraph>,
    skeleton: Handle<Skeleton>,
}

#[derive(Resource, Default)]
struct GateState {
    left: Option<Entity>,
    right: Option<Entity>,
    samples: u8,
    different: bool,
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
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(12)))
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
        left: assets.load(LEFT_PATH),
        right: assets.load(RIGHT_PATH),
        skeleton: assets.load(SKELETON_PATH),
    });
}

fn configure(player: &mut AnimationGraphPlayer, left: bool, right: bool) {
    player.set_global_input_data("als_standing_rotation_play_rate", 2.0);
    player.set_global_input_data("als_standing_rotate_left_loop", left);
    player.set_global_input_data("als_standing_rotate_right_loop", right);
    player.queue_time_update(TimeUpdate::Delta(0.1));
}

fn setup(
    mut commands: Commands,
    graphs: Res<Assets<AnimationGraph>>,
    skeletons: Res<Assets<Skeleton>>,
    assets: Res<GateAssets>,
    mut state: ResMut<GateState>,
) {
    if state.left.is_some()
        || graphs.get(&assets.left).is_none()
        || graphs.get(&assets.right).is_none()
    {
        return;
    }
    if skeletons.get(&assets.skeleton).is_none() {
        return;
    }
    let mut left =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.left.clone());
    configure(&mut left, true, false);
    left.pause();
    let mut right =
        AnimationGraphPlayer::new(assets.skeleton.clone()).with_graph(assets.right.clone());
    configure(&mut right, false, true);
    right.pause();
    state.left = Some(
        commands
            .spawn((
                Name::new("standing_rotate_left_contract"),
                Children::default(),
                left,
            ))
            .id(),
    );
    state.right = Some(
        commands
            .spawn((
                Name::new("standing_rotate_right_contract"),
                Children::default(),
                right,
            ))
            .id(),
    );
    info!("[STANDING-ROTATE-CONTRACT] direct left/right graph probes initialized");
}

fn observe(
    mut state: ResMut<GateState>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let (Some(left_entity), Some(right_entity)) = (state.left, state.right) else {
        return;
    };
    let Ok([mut left, mut right]) = players.get_many_mut([left_entity, right_entity]) else {
        return;
    };
    if let Some(error) = left.get_error().or_else(|| right.get_error()) {
        fail(&mut exit, 3, &format!("graph error={error:?}"));
        return;
    }
    let (Some(left_pose), Some(right_pose)) =
        (output_result_pose(&left), output_result_pose(&right))
    else {
        configure(&mut left, true, false);
        configure(&mut right, false, true);
        return;
    };
    state.samples = state.samples.saturating_add(1);
    state.different |= left_pose != right_pose;
    if state.samples >= 4 {
        if state.different {
            info!(
                "[STANDING-ROTATE-CONTRACT] samples={} left_right_pose_difference=true -> PASS",
                state.samples
            );
            exit.write(AppExit::Success);
        } else {
            fail(&mut exit, 4, "left and right source poses never diverged");
        }
        return;
    }
    configure(&mut left, true, false);
    configure(&mut right, false, true);
}

fn output_result_pose(
    player: &AnimationGraphPlayer,
) -> Option<bevy_animation_graph::core::pose::Pose> {
    match player.get_outputs().get("Result") {
        Some(DataValue::Pose(pose)) => Some(pose.clone()),
        _ => None,
    }
}

fn timeout(state: Res<GateState>, deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() < deadline.0 {
        return;
    }
    if state.left.is_none() {
        fail(&mut exit, 5, "graph assets did not initialize");
    } else {
        fail(&mut exit, 5, "pose samples did not complete");
    }
}

fn fail(exit: &mut MessageWriter<AppExit>, code: u8, message: &str) {
    warn!("[STANDING-ROTATE-CONTRACT][VERDICT] {message} -> FAIL");
    exit.write(AppExit::Error(std::num::NonZeroU8::new(code).unwrap()));
    std::process::exit(i32::from(code));
}
