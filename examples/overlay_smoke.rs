use std::time::{Duration, Instant};

use bevy::asset::{AssetEvent, Assets};
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::skeleton::Skeleton;

use als_ext::{build_bone_mask, TORCH_RIGHT_HAND_BONES};

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        AnimationGraphPlugin::default(),
        als_ext::AlsExtPlugin,
    ))
    .init_resource::<SkeletonReady>()
    .add_systems(Startup, setup)
    .add_systems(Update, (track_skeleton, drive_overlay, log_status));

    if let Ok(secs) = std::env::var("ALS_BOOT_AUTO_QUIT_SECS") {
        let secs: f64 = secs.parse().unwrap_or(6.0);
        app.insert_resource(AutoQuit(Instant::now() + Duration::from_secs_f64(secs)));
        app.add_systems(Update, auto_quit);
    }

    app.run();
}

#[derive(Resource, Default)]
struct SkeletonReady {
    handle: Option<Handle<Skeleton>>,
    mask_sent: bool,
}

#[derive(Resource)]
struct AutoQuit(Instant);

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(AnimatedSceneHandle::new(
        assets.load("animated_scenes/motion_overlay.animscn.ron"),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(1.6, 1.1, 2.4).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(2.0, 4.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn track_skeleton(
    assets: Res<AssetServer>,
    skeletons: Res<Assets<Skeleton>>,
    mut ev: MessageReader<AssetEvent<Skeleton>>,
    mut ready: ResMut<SkeletonReady>,
) {
    if ready.handle.is_none() {
        let h: Handle<Skeleton> = assets.load("skeletons/SKM_Als.skn.ron");
        ready.handle = Some(h);
    }
    let Some(handle) = ready.handle.as_ref() else {
        return;
    };
    for e in ev.read() {
        if let AssetEvent::LoadedWithDependencies { id } = e {
            if *id == handle.id() && skeletons.contains(handle) {
                info!("[M6SMOKE] skeleton loaded; mask armed");
            }
        }
    }
}

fn overlay_weight(t: f64) -> f32 {
    match t {
        x if x < 2.0 => 0.0,
        x if x < 3.5 => ((x - 2.0) / 1.5) as f32,
        _ => 1.0,
    }
}

fn drive_overlay(
    time: Res<Time>,
    skeletons: Res<Assets<Skeleton>>,
    mut ready: ResMut<SkeletonReady>,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let Some(handle) = ready.handle.clone() else {
        return;
    };
    if !ready.mask_sent && !skeletons.contains(&handle) {
        return;
    }
    let t = time.elapsed_secs_f64();
    let weight = overlay_weight(t);
    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("speed", DataValue::from(0.0_f32));
            player.set_input_data("actual_speed", DataValue::from(0.0_f32));
            player.set_input_data("play_rate", DataValue::from(1.0_f32));
            player.set_input_data("diff_factor", DataValue::from(1.0_f32));
            player.set_input_data("overlay_weight", DataValue::from(weight));
            if let Some(sk) = skeletons.get(&handle) {
                let mask = build_bone_mask(sk, TORCH_RIGHT_HAND_BONES, 1.0);
                if !ready.mask_sent {
                    info!("[M6SMOKE] mask bones={}", mask.get_weights().len());
                    ready.mask_sent = true;
                }
                player.set_input_data("upper_body_mask", DataValue::BoneMask(mask));
            }
        }
    }
}

#[derive(Default)]
struct LogState {
    last: f32,
}

fn log_status(
    time: Res<Time>,
    instances: Query<&AnimatedSceneInstance>,
    players: Query<&AnimationGraphPlayer>,
    mut state: Local<LogState>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs_f64() as f32;
    if now - state.last < 0.5 {
        return;
    }
    state.last = now;
    for instance in instances.iter() {
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let has_pose = matches!(
            player.get_outputs().get("pose"),
            Some(DataValue::Pose(_))
        );
        let spec_ok = true;
        let _ = (has_pose, spec_ok);
        match player.get_error() {
            Some(err) => {
                error!("[M6SMOKE] t={now:.2} graph_error={err:?}");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(2).unwrap()));
            }
            None => info!(
                "[M6SMOKE] t={now:.2} weight={:.3} pose={} elapsed={:.3}",
                overlay_weight(time.elapsed_secs_f64()),
                has_pose,
                player.elapsed(),
            ),
        }
    }
}

fn auto_quit(mut quit: Option<ResMut<AutoQuit>>, mut exit: MessageWriter<AppExit>) {
    let Some(aq) = quit.as_deref_mut() else {
        return;
    };
    if Instant::now() >= aq.0 {
        info!("[M6SMOKE] auto-quit");
        exit.write(AppExit::Success);
    }
}
