use std::time::{Duration, Instant};

use bevy::asset::{AssetEvent, Assets};
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::skeleton::Skeleton;

use als_core::overlay_blend::OverlayBlendState;
use als_core::state::OverlayState;
use als_ext::{build_bone_mask, TORCH_RIGHT_HAND_BONES};

use bevy_als::overlay_layer::OverlayLayerPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        AnimationGraphPlugin::default(),
        als_ext::AlsExtPlugin,
        OverlayLayerPlugin,
    ))
    .init_resource::<SkeletonReady>()
    .init_resource::<OverlayToggle>()
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (track_skeleton, apply_mask_pin, overlay_toggle, log_status),
    );

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
    let avatar = commands
        .spawn(AnimatedSceneHandle::new(
            assets.load("animated_scenes/motion_overlay.animscn.ron"),
        ))
        .insert(OverlayState::Default)
        .id();
    info!("[M6SMOKE] avatar={avatar:?}");

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

fn apply_mask_pin(
    skeletons: Res<Assets<Skeleton>>,
    mut ready: ResMut<SkeletonReady>,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let Some(handle) = ready.handle.clone() else {
        return;
    };
    let Some(sk) = skeletons.get(&handle) else {
        return;
    };
    let mask = build_bone_mask(sk, TORCH_RIGHT_HAND_BONES, 1.0);
    if !ready.mask_sent {
        info!("[M6SMOKE] mask bones={}", mask.get_weights().len());
        ready.mask_sent = true;
    }
    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("speed", DataValue::from(0.0_f32));
            player.set_input_data("actual_speed", DataValue::from(0.0_f32));
            player.set_input_data("play_rate", DataValue::from(1.0_f32));
            player.set_input_data("diff_factor", DataValue::from(1.0_f32));
            player.set_input_data("upper_body_mask", DataValue::BoneMask(mask.clone()));
        }
    }
}

#[derive(Resource)]
struct OverlayToggle {
    next_switch: f64,
    on: bool,
}

impl Default for OverlayToggle {
    fn default() -> Self {
        Self {
            next_switch: 2.0,
            on: false,
        }
    }
}

fn overlay_toggle(time: Res<Time>, mut q: Query<&mut OverlayState>, mut toggle: ResMut<OverlayToggle>) {
    let t = time.elapsed_secs_f64();
    if t >= toggle.next_switch && t < 9.5 {
        toggle.on = !toggle.on;
        toggle.next_switch = if toggle.on { t + 3.5 } else { t + 2.5 };
        let target = if toggle.on {
            OverlayState::Torch
        } else {
            OverlayState::Default
        };
        info!("[M6SMOKE][TOGGLE] overlay -> {target:?} at t={t:.2}");
        for mut overlay in &mut q {
            *overlay = target;
        }
    }
}

#[derive(Default)]
struct LogState {
    last: f32,
}

fn log_status(
    time: Res<Time>,
    mut q: Query<(&OverlayState, Option<&OverlayBlendState>)>,
    instances: Query<&AnimatedSceneInstance>,
    players: Query<&AnimationGraphPlayer>,
    mut state: Local<LogState>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs_f64() as f32;
    if now - state.last < 0.25 {
        return;
    }
    state.last = now;
    let Ok((overlay, blend)) = q.single_mut() else {
        return;
    };
    let blend_info = blend
        .map(|b| format!("{:?}@{:.2}", b.phase, b.phase_time))
        .unwrap_or("inserting".to_string());
    for instance in instances.iter() {
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        match player.get_error() {
            Some(err) => {
                error!("[M6SMOKE] t={now:.2} graph_error={err:?}");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(2).unwrap()));
            }
            None => info!(
                "[M6SMOKE] t={now:.2} overlay={overlay:?} blend={blend_info}",
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
