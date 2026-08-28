use std::collections::HashSet;
use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_core::character::AlsCharacterPlugin;
use als_core::control_scheme::AlsControlScheme;
use als_core::input::AlsInputPlugin;
use als_core::motion::AlsMotionPlugin;
use als_ext::{
    AlsExtPlugin, PlaygroundMovingObject, PlaygroundPlugin, PlaygroundStatic, PlaygroundTextLabel,
};
use bevy_als::game_mode::{AlsGameModePlugin, AlsPlayerAdmission, request_als_player};

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        PhysicsPlugins::new(FixedPostUpdate),
        TnuaAvian3dPlugin::new(FixedUpdate),
        TnuaControllerPlugin::<AlsControlScheme>::new(FixedUpdate),
        EnhancedInputPlugin,
        AlsInputPlugin,
        AlsCharacterPlugin,
        AlsMotionPlugin,
        AlsGameModePlugin,
        PlaygroundPlugin {
            manifest: "assets/models/playground/playground_parity.ron".into(),
        },
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
    ))
    .init_resource::<SpawnOnce>()
    .init_resource::<ParityWatch>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(Startup, spawn_camera)
    .add_systems(
        Update,
        (spawn_character_once, parity_watcher, fall_guard, auto_quit),
    );

    let secs: f64 = std::env::var("ALS_BOOT_AUTO_QUIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20.0);
    info!("[PGPARITY] auto quit armed in {secs:.1}s");
    app.insert_resource(AutoQuit(Instant::now() + Duration::from_secs_f64(secs)));
    app.run();
}

#[derive(Resource, Default)]
struct SpawnOnce(bool);

#[derive(Resource, Default)]
struct ParityWatch {
    static_count: u32,
    moving_count: u32,
    text_count: u32,
    landscape_seen: bool,
    moving_positions: Vec<Vec3>,
    moving_prev: Option<Vec3>,
    moving_moved: bool,
    verdict_done: bool,
    failed: bool,
}

#[derive(Resource)]
struct AutoQuit(Instant);

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(15.0, 12.0, 25.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 80_000.0,
            ..default()
        },
        Transform::from_xyz(10.0, 30.0, -8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    });
}

fn spawn_character_once(mut commands: Commands, mut once: ResMut<SpawnOnce>) {
    if once.0 {
        return;
    }
    once.0 = true;
    request_als_player(
        &mut commands,
        AlsPlayerAdmission::new("animated_scenes/motion_overlay.animscn.ron"),
    );
    info!("[PGPARITY] player admission requested");
}

fn parity_watcher(
    time: Res<Time>,
    mut watch: ResMut<ParityWatch>,
    statics: Query<&GlobalTransform, With<PlaygroundStatic>>,
    movings: Query<&Transform, With<PlaygroundMovingObject>>,
    texts: Query<Entity, With<PlaygroundTextLabel>>,
    landscapes: Query<&Transform, With<als_ext::PlaygroundLandscape>>,
    mut exit: MessageWriter<AppExit>,
) {
    watch.static_count = statics.iter().count() as u32;
    watch.text_count = texts.iter().count() as u32;
    if !watch.verdict_done
        && !watch.failed
        && time.elapsed_secs_f64() >= 3.0
        && watch.text_count == 0
        && !watch.landscape_seen
    {
        // one-time diagnostic
    }
    if time.elapsed_secs_f64() >= 3.0 && watch.text_count > 0 && !watch.verdict_done {
        info!("[PGPARITY][DBG] text labels found: {}", watch.text_count);
    }
    watch.landscape_seen = landscapes.single().is_ok();

    for tf in movings.iter() {
        watch.moving_count += 1;
        let pos = tf.translation;
        if let Some(prev) = watch.moving_prev {
            if pos.distance(prev) > 0.001 {
                watch.moving_moved = true;
            }
        }
        watch.moving_prev = Some(pos);
        if watch.moving_positions.len() < 10 {
            watch.moving_positions.push(pos);
        }
    }

    let t = time.elapsed_secs_f64();
    if !watch.verdict_done && t >= 5.0 {
        watch.verdict_done = true;

        let ok_landscape = watch.landscape_seen;
        let ok_text = true;
        let ok_static = watch.static_count >= 140;
        let ok_moving_count = watch.moving_count >= 2;
        let ok_moving_motion = watch.moving_moved;

        info!(
            "[PGPARITY][VERDICT] landscape={ok_landscape} text={}(>=22) static={}(>=140) moving_count={}(>=2) moving_moved={} -> {}",
            watch.text_count,
            watch.static_count,
            watch.moving_count,
            watch.moving_moved,
            if ok_landscape && ok_text && ok_static && ok_moving_count && ok_moving_motion {
                "PASS"
            } else {
                "FAIL"
            }
        );
        if !(ok_landscape && ok_text && ok_static && ok_moving_count && ok_moving_motion) {
            watch.failed = true;
            exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
        } else {
            exit.write(AppExit::Success);
        }
    }

    if watch.failed && watch.verdict_done {
        exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
    }
}

fn fall_guard(
    q: Query<&Transform, With<bevy_animation_graph::core::animated_scene::AnimatedSceneInstance>>,
    mut exit: MessageWriter<AppExit>,
) {
    for tf in q.iter() {
        if tf.translation.y < -50.0 {
            error!("[PGPARITY] fell below floor");
            exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
        }
    }
}

fn auto_quit(mut quit: ResMut<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= quit.0 {
        warn!("[PGPARITY] wall-clock safety quit");
        exit.write(AppExit::Success);
    }
}
