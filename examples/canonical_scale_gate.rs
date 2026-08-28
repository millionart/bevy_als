use std::time::{Duration, Instant};

use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::params::AlsMovementParams;
use bevy::animation::AnimationTargetId;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

use bevy_als::control_rig_apply::AlsControlRigPropagationSet;
use bevy_als::playable_character::{
    AlsCharacterAvatar, AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};

const MIN_ALLOWED_SCALE: f32 = 0.95;
const MAX_ALLOWED_SCALE: f32 = 1.05;

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource, Default)]
struct Probe {
    character: Option<Entity>,
    complete: bool,
    attempts: u32,
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
        AlsPlayableCharacterPlugin,
    ))
    .init_resource::<Probe>()
    .insert_resource(Deadline(Instant::now() + Duration::from_secs(12)))
    .add_systems(Startup, setup)
    .add_systems(Update, timeout)
    .add_systems(PostUpdate, observe.after(AlsControlRigPropagationSet));
    app.run();
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    mut probe: ResMut<Probe>,
) {
    let character = spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 0.95, 0.0)),
    );
    probe.character = Some(character);
    info!("[CANONICAL-SCALE] canonical AB_Als character spawned");
}

fn observe(
    mut probe: ResMut<Probe>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    targets: Query<&GlobalTransform, With<AnimationTargetId>>,
    mut exit: MessageWriter<AppExit>,
) {
    if probe.complete {
        return;
    }
    let Some(character) = probe.character else {
        return;
    };
    let Some((_, instance)) = avatars
        .iter()
        .find(|(parent, _)| parent.parent() == character)
    else {
        return;
    };
    let Ok(player) = players.get(instance.player_entity()) else {
        return;
    };
    probe.attempts += 1;
    let target_count = targets.iter().count();
    let Some(DataValue::Pose(pose)) = player.get_outputs().get("pose") else {
        if probe.attempts % 120 == 0 {
            info!(
                "[CANONICAL-SCALE] waiting pose targets={} error={:?}",
                target_count,
                player.get_error(),
            );
        }
        return;
    };
    if target_count < 50 {
        if probe.attempts % 120 == 0 {
            info!("[CANONICAL-SCALE] waiting targets={target_count}");
        }
        return;
    }
    let target_scales = targets
        .iter()
        .map(|transform| transform.to_scale_rotation_translation().0.abs())
        .collect::<Vec<_>>();
    let min_global_scale = target_scales
        .iter()
        .map(|scale| scale.min_element())
        .fold(f32::INFINITY, f32::min);
    let max_global_scale = target_scales
        .iter()
        .map(|scale| scale.max_element())
        .fold(0.0_f32, f32::max);
    let pose_scale_entries = pose
        .bones
        .iter()
        .filter(|bone| bone.scale.is_some())
        .count();
    let min_pose_scale = pose
        .bones
        .iter()
        .filter_map(|bone| bone.scale)
        .map(|scale| scale.abs().min_element())
        .fold(f32::INFINITY, f32::min);
    let max_pose_scale = pose
        .bones
        .iter()
        .filter_map(|bone| bone.scale)
        .map(|scale| scale.abs().max_element())
        .fold(0.0_f32, f32::max);
    let pass = min_global_scale >= MIN_ALLOWED_SCALE
        && max_global_scale <= MAX_ALLOWED_SCALE
        && (pose_scale_entries == 0
            || (min_pose_scale >= MIN_ALLOWED_SCALE && max_pose_scale <= MAX_ALLOWED_SCALE));
    info!(
        "[CANONICAL-SCALE] targets={} pose_scale_entries={} pose_scale=[{min_pose_scale:.6},{max_pose_scale:.6}] global_scale=[{min_global_scale:.6},{max_global_scale:.6}] allowed=[{MIN_ALLOWED_SCALE:.6},{MAX_ALLOWED_SCALE:.6}]",
        target_count, pose_scale_entries,
    );
    info!(
        "[CANONICAL-SCALE][VERDICT] canonical={} -> {}",
        pass,
        if pass { "PASS" } else { "FAIL" }
    );
    probe.complete = true;
    if pass {
        exit.write(AppExit::Success);
    } else {
        exit.write(AppExit::Error(std::num::NonZeroU8::new(10).unwrap()));
    }
}

fn timeout(probe: Res<Probe>, deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if !probe.complete && Instant::now() >= deadline.0 {
        warn!("[CANONICAL-SCALE][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(11).unwrap()));
    }
}
