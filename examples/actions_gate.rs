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

use als_core::air::AlsAirState;
use als_core::character::{AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::input::{AlsInputCommand, AlsInputMap, AlsInputPlugin, spawn_als_actions};
use als_core::motion::{AlsInputOverride, AlsMotionPlugin, AlsViewYaw};
use als_core::params::AlsMovementParams;
use als_core::roll::{
    AlsRollState, ROLL_DURATION_SECS, ROLL_PLAY_RATE, ROLL_ROTATION_HALF_LIFE_SECS,
    roll_displacement_dir,
};

const ROLL_CLIP_DISTANCE: f32 = 2.75;
use als_core::start_stop::{StartStopPhase, StartStopState};
use als_core::state::{Gait, LocomotionState, Stance};
use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin};
use als_ext::AlsExtPlugin;

use bevy_als::motion_graph_link::{
    compute_motion_graph_inputs, write_air_pins, write_motion_graph_pins,
};
use bevy_als::stop_link::{AlsStopLinkPlugin, AlsStopMatch};

const SNAPSHOT_INTERVAL: f32 = 0.2;

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
        als_core::air::AlsAirPlugin,
        AlsStopLinkPlugin,
        bevy_als::roll_drive::AlsRollDrivePlugin,
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
    ))
    .init_resource::<SpawnOnce>()
    .init_resource::<DemoDriver>()
    .init_resource::<ActionsVerdict>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(
        Update,
        (
            spawn_character_once,
            auto_demo,
            start_stop_detector,
            actions_watcher,
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
        .unwrap_or(30.0);
    info!("[AGATE] auto quit armed in {secs:.1}s");
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
    label: &'static str,
    move_dir: Vec2,
    gait: Option<Gait>,
    pulse_roll: bool,
    pulse_jump: bool,
    release_input: bool,
}

fn steps() -> [Step; 7] {
    [
        Step {
            t0: 0.5,
            dur: 1.0,
            label: "idle",
            move_dir: Vec2::ZERO,
            gait: None,
            pulse_roll: false,
            pulse_jump: false,
            release_input: false,
        },
        Step {
            t0: 1.5,
            dur: 2.0,
            label: "sprint up",
            move_dir: Vec2::NEG_Y,
            gait: Some(Gait::Sprinting),
            pulse_roll: false,
            pulse_jump: false,
            release_input: false,
        },
        Step {
            t0: 3.5,
            dur: 2.0,
            label: "sprint roll (forward)",
            move_dir: Vec2::NEG_Y,
            gait: Some(Gait::Sprinting),
            pulse_roll: true,
            pulse_jump: false,
            release_input: false,
        },
        Step {
            t0: 5.5,
            dur: 1.8,
            label: "resume sprint",
            move_dir: Vec2::NEG_Y,
            gait: Some(Gait::Sprinting),
            pulse_roll: false,
            pulse_jump: false,
            release_input: false,
        },
        Step {
            t0: 7.3,
            dur: 0.9,
            label: "jump while sprinting",
            move_dir: Vec2::NEG_Y,
            gait: Some(Gait::Sprinting),
            pulse_roll: false,
            pulse_jump: true,
            release_input: false,
        },
        Step {
            t0: 8.2,
            dur: 0.35,
            label: "land + keep moving",
            move_dir: Vec2::NEG_Y,
            gait: Some(Gait::Sprinting),
            pulse_roll: false,
            pulse_jump: false,
            release_input: false,
        },
        Step {
            t0: 8.55,
            dur: 2.6,
            label: "release -> stop distance match",
            move_dir: Vec2::ZERO,
            gait: None,
            pulse_roll: false,
            pulse_jump: false,
            release_input: true,
        },
    ]
}

#[derive(Resource, Default)]
struct ActionsVerdict {
    prev_rolling: bool,
    roll_start_xz: Vec2,
    roll_dist: f32,
    roll_align: f32,

    saw_air: bool,
    landed_light: bool,
    land_pos_z: f32,

    stop_started: bool,
    prev_stop_active: bool,
    stop_start_xz: Vec2,
    stop_total: f32,
    stop_slide: f32,
    stop_remaining_end: f32,
    stop_peak_weight: f32,

    jump_pressed_at: f64,
    failed: bool,
    verdict_done: bool,
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
        Collider::cuboid(60.0, 0.3, 120.0),
        Transform::from_xyz(0.0, -0.15, 20.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(60.0, 0.3, 120.0)))),
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
        ));
        info!("[AGATE] scripted demo armed");
    }

    commands.spawn((Camera3d::default(), Transform::from_xyz(3.5, 2.8, -4.0)));
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
    let last = &all[all.len() - 1];
    if t >= last.t0 + last.dur + 0.6 {
        driver.done = true;
        exit.write(AppExit::Success);
        return;
    }
    let Some(idx) = (0..all.len()).rev().find(|i| t >= all[*i].t0) else {
        return;
    };
    let s = &all[idx];
    if idx != driver.step {
        driver.step = idx;
        info!(
            "[AGATE][STEP {}] {} (gait={:?} roll={} jump={} release={})",
            idx, s.label, s.gait, s.pulse_roll, s.pulse_jump, s.release_input
        );
        if let Some(g) = s.gait {
            req.gait = Some(g);
        }
    }
    let local = t - s.t0;
    over.0.roll_pressed = s.pulse_roll && local < 0.25;
    over.0.jump_pressed = s.pulse_jump && local < 0.25;
    over.0.jump_held = s.pulse_jump && local < 0.45;
    let releasing = s.release_input;
    if !releasing {
        over.0.move_dir = s.move_dir;
    } else {
        over.0.move_dir = Vec2::ZERO;
    }
    over.0.crouch_pressed = false;
    over.0.sprint_held = matches!(all[idx].gait, Some(Gait::Sprinting));
    over.0.aim_held = false;
    over.0.look_delta = Vec2::ZERO;
}

fn start_stop_detector(
    mut q: Query<
        (
            &LinearVelocity,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            &AlsViewYaw,
            &mut StartStopState,
        ),
        With<AlsCharacter>,
    >,
) {
    let Ok((vel, cmd, ovr, view, mut ss)) = q.single_mut() else {
        return;
    };
    let input = ovr.map(|o| &o.0).unwrap_or(cmd);
    let hspeed = Vec2::new(vel.x, vel.z).length();
    let dir = als_core::start_stop::classify(
        als_core::motion::view_relative_move_axes(input.move_dir, view.0),
        view.0,
    );
    als_core::start_stop::update_start_stop(&mut ss, &Default::default(), hspeed, dir);
}

#[allow(clippy::type_complexity)]
fn actions_watcher(
    time: Res<Time>,
    mut v: ResMut<ActionsVerdict>,
    mut q: Query<
        (
            &Transform,
            &LinearVelocity,
            &Stance,
            &LocomotionState,
            &Gait,
            &AlsRollState,
            &AlsAirState,
            &AlsStopMatch,
        ),
        With<AlsCharacter>,
    >,
    instances: Query<&AnimatedSceneInstance>,
    players: Query<&AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((tf, vel, stance, loco, gait, roll, air, stop)) = q.single_mut() else {
        return;
    };
    let xz = Vec2::new(tf.translation.x, tf.translation.z);

    if !v.prev_rolling && roll.active {
        v.roll_start_xz = xz;
        info!(
            "[BCHAR18A][ROLL begin] yaw={:.2} vec={:?} duration={ROLL_DURATION_SECS:.6} rate={ROLL_PLAY_RATE:.2} half_life={ROLL_ROTATION_HALF_LIFE_SECS:.2}",
            roll.direction_yaw,
            roll_displacement_dir(roll.direction_yaw)
        );
    }
    if v.prev_rolling && !roll.active {
        v.roll_dist = xz.distance(v.roll_start_xz);
        let dv = roll_displacement_dir(roll.direction_yaw);
        let moved = xz - v.roll_start_xz;
        v.roll_align = if moved.length() > 0.05 {
            moved.normalize().dot(Vec2::new(dv.x, dv.z))
        } else {
            0.0
        };
        info!(
            "[AGATE][ROLL done] dist={:.2} (clip root {ROLL_CLIP_DISTANCE}) align={:+.3}",
            v.roll_dist, v.roll_align
        );
    }
    v.prev_rolling = roll.active;

    if *loco == LocomotionState::Air {
        if !v.saw_air {
            v.saw_air = true;
            v.jump_pressed_at = time.elapsed_secs_f64();
            info!("[AGATE][JUMP] airborne at t={:.2}", v.jump_pressed_at);
        }
    }
    if v.saw_air && *loco == LocomotionState::Grounded && v.land_pos_z == 0.0 {
        v.land_pos_z = tf.translation.z;
        let (lw, _) = air_side_weights(air);
        if lw > 0.5 {
            v.landed_light = true;
        }
        info!(
            "[AGATE][LAND] z={:.2} light={} ff={:.2}",
            v.land_pos_z, v.landed_light, air.fall_fast_weight
        );
    }

    if !v.stop_started && stop.active {
        v.stop_started = true;
        v.stop_start_xz = xz;
        v.stop_total = stop.total;
        info!(
            "[BCHAR18S][STOP] total={:.3} right={} rate={:.2} start={:.2} blend={:.2}/{:.2} yaw={:.1}",
            stop.total,
            stop.right_weight > 0.5,
            stop.quick_stop_play_rate,
            stop.quick_stop_start_time,
            stop.quick_stop_blend_in_duration,
            stop.quick_stop_blend_out_duration,
            stop.quick_stop_remaining_yaw_degrees,
        );
    }
    if v.stop_started {
        v.stop_peak_weight = v.stop_peak_weight.max(stop.weight);
        if !v.prev_stop_active && stop.weight > 0.9 {
            // weight ramp reached while matching; keep sliding measurement window open
        }
        if stop.remaining <= 0.02 && v.stop_remaining_end == 0.0 {
            v.stop_remaining_end = stop.remaining.max(0.0);
        }
        if !stop.active && v.verdict_done == false {
            v.stop_slide = xz.distance(v.stop_start_xz);
            v.stop_remaining_end = stop.remaining;
            let ok_roll = v.roll_dist >= 1.9 && v.roll_dist <= 3.4 && v.roll_align >= 0.95;
            let ok_land = v.saw_air && v.landed_light;
            let stop_distance_error = (v.stop_slide - v.stop_total).abs();
            let ok_stop = stop_distance_error <= (0.25_f32).max(v.stop_total * 0.15)
                && v.stop_peak_weight >= 0.95
                && (v.stop_remaining_end <= 0.08 || v.stop_slide >= 0.05)
                && (1.75..=3.0).contains(&stop.quick_stop_play_rate)
                && (stop.quick_stop_start_time - 0.3).abs() <= 1e-6
                && (stop.quick_stop_blend_in_duration - 0.1).abs() <= 1e-6
                && (stop.quick_stop_blend_out_duration - 0.2).abs() <= 1e-6;
            info!(
                "[AGATE][VERDICT] roll(dist={:.2},align={:+.3})={ok_roll} jump_light={ok_land} stop(slide={:.2},total={:.2},error={:.2},peak_w={:.2},rem_end={:.3})={ok_stop} -> {}",
                v.roll_dist,
                v.roll_align,
                v.stop_slide,
                v.stop_total,
                stop_distance_error,
                v.stop_peak_weight,
                v.stop_remaining_end,
                if ok_roll && ok_land && ok_stop {
                    "PASS"
                } else {
                    "FAIL"
                }
            );
            v.verdict_done = true;
            if !(ok_roll && ok_land && ok_stop) {
                v.failed = true;
                exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
            } else {
                exit.write(AppExit::Success);
            }
        }
    }
    v.prev_stop_active = stop.active;

    for instance in instances.iter() {
        if let Ok(player) = players.get(instance.player_entity()) {
            let _ = player;
        }
    }
    let _ = vel;
}

fn air_side_weights(_air: &AlsAirState) -> (f32, f32) {
    (1.0, 0.0)
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
            write_air_pins(&mut player, &als_core::air::AlsAirState::default());
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
        cam.translation = char_t.translation + Vec3::new(3.5, 2.8, -4.0);
        cam.look_at(char_t.translation + Vec3::Y * 0.9, Vec3::Y);
    }
}

fn fall_guard(q: Query<&Transform, With<AlsCharacter>>, mut exit: MessageWriter<AppExit>) {
    let Ok(char_t) = q.single() else {
        return;
    };
    if char_t.translation.y < -20.0 {
        error!("[AGATE] fell below floor -- aborting");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
    }
}

#[allow(clippy::type_complexity)]
fn snapshot_logger(
    time: Res<Time>,
    mut last: Local<f32>,
    q: Query<
        (
            &Transform,
            &LinearVelocity,
            &LocomotionState,
            &Gait,
            &AlsRollState,
            &AlsStopMatch,
        ),
        With<AlsCharacter>,
    >,
) {
    let t = time.elapsed_secs_f64();
    if (t as f32) - *last < SNAPSHOT_INTERVAL {
        return;
    }
    *last = t as f32;
    let Ok((tf, vel, loco, gait, roll, stop)) = q.single() else {
        return;
    };
    info!(
        "[AGATE] t={t:.2} loco={loco:?} gait={gait:?} v={:.2} pos=({:.2},{:.2},{:.2}) rolling={} sw={:.2} rem={:.3}",
        Vec2::new(vel.x, vel.z).length(),
        tf.translation.x,
        tf.translation.y,
        tf.translation.z,
        roll.active,
        stop.weight,
        stop.remaining,
    );
}

fn auto_quit(mut quit: ResMut<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= quit.0 {
        warn!("[AGATE] wall-clock safety quit");
        exit.write(AppExit::Success);
    }
}
