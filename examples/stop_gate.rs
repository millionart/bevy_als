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
use als_core::input::{AlsInputCommand, AlsInputMap, AlsInputPlugin, spawn_als_actions};
use als_core::motion::{AlsInputOverride, AlsMotionPlugin, AlsViewYaw};
use als_core::params::AlsMovementParams;
use als_core::start_stop::{StartStopPhase, StartStopState};
use als_core::state::{Gait, LocomotionState, Stance};
use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin};
use als_ext::AlsExtPlugin;

use bevy_als::motion_graph_link::{compute_motion_graph_inputs, write_motion_graph_pins};
use bevy_als::stop_link::{AlsStopLinkPlugin, AlsStopMatch};

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
        AlsStopLinkPlugin,
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
    ))
    .init_resource::<SpawnOnce>()
    .init_resource::<DemoDriver>()
    .init_resource::<StopWatch>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(
        Update,
        (
            spawn_character_once,
            auto_demo,
            stop_watcher,
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
    info!("[STOPGATE] auto quit armed in {secs:.1}s");
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
    gait: Option<Gait>,
    label: &'static str,
}

fn steps() -> [Step; 4] {
    [
        Step {
            t0: 0.5,
            dur: 1.0,
            move_dir: Vec2::ZERO,
            gait: None,
            label: "idle",
        },
        Step {
            t0: 1.5,
            dur: 1.6,
            move_dir: Vec2::NEG_Y,
            gait: Some(Gait::Running),
            label: "sprint up",
        },
        Step {
            t0: 3.1,
            dur: 2.5,
            move_dir: Vec2::ZERO,
            gait: None,
            label: "hard stop",
        },
        Step {
            t0: 5.6,
            dur: 1.4,
            move_dir: Vec2::ZERO,
            gait: None,
            label: "settle",
        },
    ]
}

#[derive(Resource, Default)]
struct StopWatch {
    prev_active: bool,
    start_xz: Vec2,
    predicted_total: f32,
    measured_slide: f32,
    remaining_at_end: f32,
    observed_stop: bool,
    peak_weight: f32,
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
    bundle.transform = Transform::from_xyz(0.0, 0.95, 0.0)
        .with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
    let character = commands.spawn(bundle).id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());

    let avatar = bevy_als::character_assembly::attach_avatar(
        &mut commands,
        &assets,
        character,
        "animated_scenes/motion_overlay.animscn.ron",
    );

    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() {
        commands.entity(character).insert((
            AlsInputOverride::default(),
            AlsViewYaw(std::f32::consts::PI),
            StartStopState::default(),
        ));
        info!("[STOPGATE] scripted demo armed");
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
    mut q: Query<(&mut AlsInputOverride, &mut AlsStateRequest), With<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_err() || driver.done {
        return;
    }
    let Ok((mut over, mut req)) = q.single_mut() else {
        return;
    };
    let t = time.elapsed_secs_f64();
    let all = steps();
    if t >= all[all.len() - 1].t0 + all[all.len() - 1].dur + 0.4 {
        driver.done = true;
        exit.write(AppExit::Success);
        return;
    }
    let Some(idx) = (0..all.len()).rev().find(|i| t >= all[*i].t0) else {
        return;
    };
    if idx != driver.step {
        driver.step = idx;
        info!("[STOPGATE][STEP {}] {}", idx, all[idx].label);
        if let Some(g) = all[idx].gait {
            req.gait = Some(g);
        }
    }
    over.0.move_dir = all[idx].move_dir;
    over.0.roll_pressed = false;
    over.0.jump_pressed = false;
    over.0.jump_held = false;
    over.0.crouch_pressed = false;
    over.0.sprint_held = false;
    over.0.aim_held = false;
    over.0.look_delta = Vec2::ZERO;
}

fn stop_watcher(
    mut watch: ResMut<StopWatch>,
    q: Query<(&Transform, &LinearVelocity, &AlsStopMatch), With<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((tf, _vel, m)) = q.single() else {
        return;
    };
    watch.peak_weight = watch.peak_weight.max(m.weight);

    if !watch.prev_active && m.active {
        watch.start_xz = Vec2::new(tf.translation.x, tf.translation.z);
        watch.predicted_total = m.total;
        info!(
            "[BCHAR18S][PREDICT] total={:.3} right={} rate={:.2} start={:.2} blend={:.2}/{:.2} yaw={:.1}",
            m.total,
            m.right_weight > 0.5,
            m.quick_stop_play_rate,
            m.quick_stop_start_time,
            m.quick_stop_blend_in_duration,
            m.quick_stop_blend_out_duration,
            m.quick_stop_remaining_yaw_degrees,
        );
    }
    if watch.prev_active && !m.active {
        let xz = Vec2::new(tf.translation.x, tf.translation.z);
        watch.measured_slide = xz.distance(watch.start_xz);
        watch.remaining_at_end = m.remaining;
        watch.observed_stop = true;
        let err = if watch.predicted_total > 1e-3 {
            (watch.measured_slide - watch.predicted_total).abs() / watch.predicted_total
        } else {
            f32::INFINITY
        };
        info!(
            "[STOPGATE][RESULT] predicted={:.3} measured_slide={:.3} rel_err={err:.1}% remaining_end={:.3} peak_w={:.2}",
            watch.predicted_total, watch.measured_slide, watch.remaining_at_end, watch.peak_weight
        );
        let ok_closed = watch.remaining_at_end <= 0.08;
        let ok_weight = watch.peak_weight >= 0.95;
        let ok_slide = (0.05..=1.5).contains(&watch.measured_slide);
        let source_contract = (1.75..=3.0).contains(&m.quick_stop_play_rate)
            && (m.quick_stop_start_time - 0.3).abs() <= 1e-6
            && (m.quick_stop_blend_in_duration - 0.1).abs() <= 1e-6
            && (m.quick_stop_blend_out_duration - 0.2).abs() <= 1e-6;
        info!(
            "[BCHAR18S][VERDICT] plant_on_stop={ok_closed} weight_ok={ok_weight} slide_in_band={ok_slide} source_contract={source_contract} -> {}",
            if ok_closed && ok_weight && ok_slide && source_contract {
                "PASS"
            } else {
                "FAIL"
            }
        );
        if !(ok_closed && ok_weight && ok_slide && source_contract) {
            watch.failed = true;
            exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
        }
    }
    watch.prev_active = m.active;
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
        error!("[STOPGATE] fell below floor -- aborting");
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
            &LocomotionState,
            &StartStopState,
            &AlsStopMatch,
        ),
        With<AlsCharacter>,
    >,
) {
    let t = time.elapsed_secs_f64();
    if (t as f32) - *last < 0.1 {
        return;
    }
    *last = t as f32;
    let Ok((tf, vel, loco, ss, m)) = q.single() else {
        return;
    };
    info!(
        "[STOPGATE] t={t:.2} loco={loco:?} phase= v={:.2} pos_z={:.2} sw={:.2} rem={:.3} tot={:.3}",
        Vec2::new(vel.x, vel.z).length(),
        tf.translation.z,
        m.weight,
        m.remaining,
        m.total,
    );
}

fn auto_quit(mut quit: ResMut<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= quit.0 {
        warn!("[STOPGATE] wall-clock safety quit");
        exit.write(AppExit::Success);
    }
}
