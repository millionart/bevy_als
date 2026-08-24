use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_core::character::{AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::input::{spawn_als_actions, AlsInputMap, AlsInputPlugin};
use als_core::motion::{AlsInputOverride, AlsMotionPlugin, AlsViewYaw};
use als_core::params::AlsMovementParams;
use als_core::roll::{roll_displacement_dir, AlsRollState};

const ROLL_CLIP_DISTANCE: f32 = 2.75;
use als_core::state::{Gait, LocomotionState, Stance};
use als_core::transition::AlsStateTransitionPlugin;
use als_ext::AlsExtPlugin;

use bevy_als::motion_graph_link::{compute_motion_graph_inputs, write_motion_graph_pins};
use bevy_als::roll_drive::AlsRollDrivePlugin;

const SNAPSHOT_INTERVAL: f32 = 0.1;

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
        AlsStateTransitionPlugin,
        AlsCharacterPlugin,
        AlsMotionPlugin,
        AlsRollDrivePlugin,
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
    ))
    .init_resource::<SpawnOnce>()
    .init_resource::<DemoDriver>()
    .init_resource::<RollWatch>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(
        Update,
        (
            spawn_character_once,
            auto_demo,
            roll_watcher,
            graph_router,
            camera_follow,
            fall_guard,
            snapshot_logger,
            auto_quit,
        ),
    );

    let secs: f64 = std::env::var("ALS_BOOT_AUTO_QUIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20.0);
    info!("[ROLLGATE] auto quit armed in {secs:.1}s");
    app.insert_resource(AutoQuit(Instant::now() + Duration::from_secs_f64(secs)));
    app.run();
}

#[derive(Resource, Default)]
struct SpawnOnce(bool);

#[derive(Resource, Default)]
struct DemoDriver {
    step: usize,
    done: bool,
}

struct Step {
    t0: f64,
    dur: f64,
    move_dir: Vec2,
    roll_pulse: bool,
    label: &'static str,
}

fn steps() -> [Step; 6] {
    [
        Step { t0: 0.5, dur: 1.0, move_dir: Vec2::ZERO, roll_pulse: false, label: "idle" },
        Step { t0: 1.5, dur: 1.2, move_dir: Vec2::NEG_Y, roll_pulse: false, label: "run up" },
        Step { t0: 2.7, dur: 1.6, move_dir: Vec2::NEG_Y, roll_pulse: true, label: "forward roll" },
        Step { t0: 4.3, dur: 1.2, move_dir: Vec2::ZERO, roll_pulse: false, label: "settle" },
        Step { t0: 5.5, dur: 1.6, move_dir: Vec2::X, roll_pulse: true, label: "right roll" },
        Step { t0: 7.1, dur: 1.4, move_dir: Vec2::ZERO, roll_pulse: false, label: "final settle" },
    ]
}

#[derive(Resource, Default)]
struct RollWatch {
    active_prev: bool,
    start_xz: Option<Vec2>,
    expect_dir_yaw: f32,
    roll_index: usize,
    peak_v: f32,
    results: [Option<(f32, f32, f32)>; 2],
    failed: bool,
}

#[derive(Resource)]
struct AutoQuit(Instant);

fn spawn_character_once(
    mut commands: Commands,
    mut once: ResMut<SpawnOnce>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    assets: Res<AssetServer>,
) {
    if once.0 {
        return;
    }
    once.0 = true;
    let ground_mat = materials.add(Color::srgb(0.24, 0.27, 0.24));
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(60.0, 0.3, 60.0),
        Transform::from_xyz(0.0, -0.15, 0.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(60.0, 0.3, 60.0)))),
        MeshMaterial3d(ground_mat),
    ));

    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform =
        Transform::from_xyz(0.0, 0.95, 0.0).with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
    let character = commands.spawn(bundle).id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());

    let avatar = commands
        .spawn((
            AnimatedSceneHandle::new(assets.load("animated_scenes/motion_overlay.animscn.ron")),
            Transform::from_xyz(0.0, -0.90, 0.0),
        ))
        .id();
    commands.entity(character).add_child(avatar);

    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() {
        commands.entity(character).insert((
            AlsInputOverride::default(),
            AlsViewYaw(std::f32::consts::PI),
        ));
        info!("[ROLLGATE] scripted demo armed");
    }

    commands.spawn((Camera3d::default(), Transform::from_xyz(3.0, 2.5, -3.0)));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(20.0, 40.0, -15.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn auto_demo(
    time: Res<Time>,
    mut driver: ResMut<DemoDriver>,
    mut q: Query<&mut AlsInputOverride, With<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_err() || driver.done {
        return;
    }
    let Ok(mut over) = q.single_mut() else {
        return;
    };
    let t = time.elapsed_secs_f64();

    let all = steps();
    let Some(idx) = (0..all.len()).rev().find(|i| t >= all[*i].t0) else {
        over.0.reset();
        return;
    };
    if idx != driver.step {
        driver.step = idx;
        info!("[ROLLGATE][STEP {}] {}", idx, all[idx].label);
    }
    if t >= all[all.len() - 1].t0 + all[all.len() - 1].dur + 0.5 {
        driver.done = true;
        exit.write(AppExit::Success);
        return;
    }
    let s = &all[idx];
    let local = (t - s.t0) as f32;
    over.0.move_dir = s.move_dir;
    over.0.roll_pressed = s.roll_pulse && local < 0.05;
    over.0.jump_pressed = false;
    over.0.jump_held = false;
    over.0.crouch_pressed = false;
    over.0.sprint_held = false;
    over.0.aim_held = false;
    over.0.look_delta = Vec2::ZERO;
}

#[allow(clippy::type_complexity)]
fn roll_watcher(
    mut watch: ResMut<RollWatch>,
    q: Query<
        (
            &Transform,
            &LinearVelocity,
            &Stance,
            &LocomotionState,
            &AlsRollState,
        ),
        With<AlsCharacter>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((tf, vel, _stance, _loco, roll)) = q.single() else {
        return;
    };
    let xz = Vec2::new(tf.translation.x, tf.translation.z);

    if !watch.active_prev && roll.active {
        watch.start_xz = Some(xz);
        watch.expect_dir_yaw = roll.direction_yaw;
        watch.peak_v = 0.0;
        info!(
            "[ROLLGATE][ROLL {}] started dir_yaw={:.2} expected_vec={:?}",
            watch.roll_index + 1,
            roll.direction_yaw,
            roll_displacement_dir(roll.direction_yaw)
        );
    }
    if roll.active {
        watch.peak_v = watch.peak_v.max(Vec2::new(vel.x, vel.z).length());
    }
    if watch.active_prev && !roll.active {
        let Some(start) = watch.start_xz.take() else {
            return;
        };
        let dist = xz.distance(start);
        let dir_vec = roll_displacement_dir(watch.expect_dir_yaw);
        let moved = xz - start;
        let moved_len = moved.length();
        let dir2 = Vec2::new(dir_vec.x, dir_vec.z);
        let align = if moved_len > 0.05 {
            moved.normalize().dot(dir2)
        } else {
            0.0
        };
        let peak_v = watch.peak_v;
        info!(
            "[ROLLGATE][RESULT {}] dist={dist:.2} (clip root {ROLL_CLIP_DISTANCE}) align={align:+.3} peak_v={peak_v:.2}",
            watch.roll_index + 1
        );
        let ok_dist = (dist - ROLL_CLIP_DISTANCE).abs() <= 0.6;
        let ok_align = align >= 0.95;
        if !(ok_dist && ok_align) {
            watch.failed = true;
            error!(
                "[ROLLGATE][FAIL {}] dist_ok={ok_dist} align_ok={ok_align}",
                watch.roll_index + 1
            );
        } else if watch.roll_index < 2 {
            let idx = watch.roll_index;
            watch.results[idx] = Some((dist, align, peak_v));
        }
        watch.roll_index += 1;
    }
    watch.active_prev = roll.active;
    if watch.failed {
        exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
    }
}

fn graph_router(
    q: Query<
        (
            &Stance,
            &Gait,
            &LinearVelocity,
            &als_core::character::AlsMovementParamsHandle,
        ),
        With<AlsCharacter>,
    >,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
    params: Res<Assets<AlsMovementParams>>,
) {
    use als_core::state::Gait;
    let Ok((stance, gait, vel, handle)) = q.single() else {
        return;
    };
    let Some(p) = params.get(handle.id()) else {
        return;
    };
    let hspeed = Vec2::new(vel.x, vel.z).length();
    let mgi = compute_motion_graph_inputs(p, *stance, *gait, hspeed);
    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            write_motion_graph_pins(&mut player, &mgi);
        }
    }
}

fn camera_follow(
    mut cameras: Query<&mut Transform, (With<Camera3d>, Without<AlsCharacter>)>,
    characters: Query<&Transform, (With<AlsCharacter>, Without<Camera3d>)>,
) {
    let Ok(char_t) = characters.single() else {
        return;
    };
    for mut cam in cameras.iter_mut() {
        cam.translation = char_t.translation + Vec3::new(3.0, 2.2, -3.0);
        cam.look_at(char_t.translation + Vec3::Y * 0.9, Vec3::Y);
    }
}

fn fall_guard(q: Query<&Transform, With<AlsCharacter>>, mut exit: MessageWriter<AppExit>) {
    let Ok(char_t) = q.single() else {
        return;
    };
    if char_t.translation.y < -20.0 {
        error!("[ROLLGATE] fell below floor -- aborting");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
    }
}

fn snapshot_logger(
    time: Res<Time>,
    mut last: Local<f32>,
    q: Query<
        (
            &Transform,
            &LinearVelocity,
            &Stance,
            &LocomotionState,
            &AlsRollState,
        ),
        With<AlsCharacter>,
    >,
    mut watch: ResMut<RollWatch>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs_f64();
    if (t as f32) - *last < SNAPSHOT_INTERVAL {
        return;
    }
    *last = t as f32;
    let Ok((tf, vel, stance, loco, roll)) = q.single() else {
        return;
    };
    info!(
        "[ROLLGATE] t={t:.2} loco={loco:?} stance={stance:?} v={:.2} pos=({:.2},{:.2},{:.2}) rolling={} rt={:.2} cd={:.2}",
        Vec2::new(vel.x, vel.z).length(),
        tf.translation.x,
        tf.translation.y,
        tf.translation.z,
        roll.active,
        roll.time,
        roll.cooldown,
    );
}

fn auto_quit(mut quit: ResMut<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= quit.0 {
        warn!("[ROLLGATE] wall-clock safety quit");
        exit.write(AppExit::Success);
    }
}
