use std::f32::consts::PI;
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

use als_core::character::{AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin, AlsCollisionLayer};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::foot_ik::{
    ANKLE_TO_SOLE, AlsDynamicTransitionState, AlsFootBones, AlsFootIkRuntime, AlsFootLockSettings,
    AlsFootPinHold, AlsFootPinRequest, AlsFootProbe, extend_foot_pin_hold,
};
use als_core::input::{AlsInputCommand, AlsInputMap, AlsInputPlugin, spawn_als_actions};
use als_core::motion::{AlsInputOverride, AlsMotionPlugin, AlsViewYaw};
use als_core::params::AlsMovementParams;
use als_core::start_stop::{StartStopConfig, StartStopState, classify, update_start_stop};
use als_core::state::{Gait, LocomotionState, Stance};
use als_core::transition::AlsStateTransitionPlugin;
use als_core::turn_in_place::{TurnInPlaceConfig, TurnInPlaceDetector, wrap_angle_degrees};
use als_ext::AlsExtPlugin;

use bevy_als::foot_ik_apply::FootIkApplyPlugin;
use bevy_als::motion_graph_link::{compute_motion_graph_inputs, write_motion_graph_pins};

const SNAPSHOT_INTERVAL: f32 = 0.25;
const LOOK_UNITS_PER_RAD: f32 = 400.0;
const TURN_RATE_DEG_PER_S: f32 = 100.0;
const SPIN_WINDOW_SECS: f64 = 1.8;
const SPIN_DELAY_SECS: f64 = 0.8;
const YAW_SPEED_ENVELOPE_DECAY: f32 = 250.0;

const RAMP_ANGLE_DEG: f32 = 12.0;
const RAMP_LENGTH: f32 = 8.0;
const RAMP_THICKNESS: f32 = 0.35;
const RAMP_WIDTH: f32 = 4.0;
const RAMP_START_Z: f32 = 2.0;

const PHASE_DURATIONS: [f64; 6] = [1.5, 3.5, 1.5, 3.5, 4.5, 1.5];
const LAST_PHASE: usize = 5;
const TOTAL_SECS: f64 = 16.0;
const VERDICT_AT_SECS: f64 = 15.2;

const MOVE_EPSILON: f32 = 0.15;
const PENETRATION_TOL: f32 = 0.06;
const STAND_FLOAT_TOL: f32 = 0.12;
const ROOT_DRIFT_TOL: f32 = 0.05;
const LOCK_SHIFT_TOL: f32 = 0.03;
const PELVIS_DROP_MIN: f32 = -0.30;

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
        als_core::aim::AlsAimPlugin,
        AlsStateTransitionPlugin,
        AlsCharacterPlugin,
        AlsMotionPlugin,
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
        als_core::foot_ik::AlsFootProbePlugin,
        FootIkApplyPlugin,
    ))
    .init_resource::<SpawnOnce>()
    .init_resource::<DemoDriver>()
    .init_resource::<GateVerdict>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(
        Update,
        (
            spawn_character_once,
            auto_demo,
            als_detectors_system,
            graph_input_router_system,
            camera_follow,
            fall_guard,
            metrics_logger,
            auto_quit,
        ),
    );

    let secs: f64 = std::env::var("ALS_BOOT_AUTO_QUIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30.0);
    info!("[M7GATE] auto quit armed in {secs:.1}s");
    app.insert_resource(AutoQuit(Instant::now() + Duration::from_secs_f64(secs)));
    app.run();
}

#[derive(Resource, Default)]
struct SpawnOnce(bool);

#[derive(Resource, Default)]
struct DemoDriver {
    phase: usize,
    done: bool,
}

#[derive(Resource, Default)]
struct GateVerdict {
    failed: bool,
}

#[derive(Resource)]
struct AutoQuit(Instant);

#[derive(Component, Default)]
struct TurnDetector(TurnInPlaceDetector);

#[derive(Default)]
struct PinWatch {
    ready_logged: bool,
    pen_worst: f32,
    stand_bad: bool,
    stop_pins_seen: bool,
    turn_engaged: bool,
    root_origin: Option<Vec3>,
    max_root_drift: f32,
    lock_offsets: [Option<f32>; 2],
    max_lock_shift: f32,
    verdict_done: bool,
}

fn phase_start(index: usize) -> f64 {
    PHASE_DURATIONS[..index].iter().sum()
}

fn phase_at(t: f64) -> usize {
    let mut acc = 0.0;
    for (i, d) in PHASE_DURATIONS.iter().enumerate() {
        acc += d;
        if t < acc {
            return i;
        }
    }
    LAST_PHASE
}

fn phase_desc(phase: usize) -> &'static str {
    match phase {
        0 => "idle flat",
        1 => "run up ramp",
        2 => "stand on slope",
        3 => "walk down ramp",
        4 => "stop + spin + pins",
        _ => "settle idle",
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let ground_mat = materials.add(Color::srgb(0.24, 0.27, 0.24));
    let ramp_mat = materials.add(Color::srgb(0.45, 0.38, 0.30));
    let plateau_mat = materials.add(Color::srgb(0.30, 0.36, 0.42));

    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 0.3, 40.0),
        CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL),
        Transform::from_xyz(0.0, -0.15, 0.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(40.0, 0.3, 40.0)))),
        MeshMaterial3d(ground_mat),
    ));

    let rad = RAMP_ANGLE_DEG.to_radians();
    let half_len = RAMP_LENGTH * 0.5;
    let half_thick = RAMP_THICKNESS * 0.5;
    let rise = half_len * rad.sin();
    let run = half_len * rad.cos();
    let center_z = RAMP_START_Z + run;
    let center_y = rise - half_thick * rad.cos();
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(RAMP_WIDTH, RAMP_THICKNESS, RAMP_LENGTH),
        CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL),
        Transform::from_xyz(0.0, center_y, center_z).with_rotation(Quat::from_rotation_x(-rad)),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(
            RAMP_WIDTH,
            RAMP_THICKNESS,
            RAMP_LENGTH,
        )))),
        MeshMaterial3d(ramp_mat),
    ));

    let plateau_top = 2.0 * rise;
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(RAMP_WIDTH, 0.3, 8.0),
        CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL),
        Transform::from_xyz(0.0, plateau_top - 0.15, RAMP_START_Z + 2.0 * run + 4.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(RAMP_WIDTH, 0.3, 8.0)))),
        MeshMaterial3d(plateau_mat),
    ));
    info!(
        "[M7GATE] scene built (flat + {RAMP_ANGLE_DEG} deg ramp, seams flush at y=0 and y={plateau_top:.3})"
    );
}

fn spawn_character_once(
    mut commands: Commands,
    mut once: ResMut<SpawnOnce>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    assets: Res<AssetServer>,
) {
    if once.0 {
        return;
    }
    once.0 = true;
    setup_scene(commands.reborrow(), meshes, materials);

    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_xyz(0.0, 0.95, 0.0).with_rotation(Quat::from_rotation_y(PI));
    let scripted = std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok();

    let character = commands
        .spawn(bundle)
        .insert((
            AlsViewYaw(PI),
            StartStopState::default(),
            TurnDetector::default(),
            AlsFootPinHold::default(),
        ))
        .id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());

    let avatar = bevy_als::character_assembly::attach_avatar(
        &mut commands,
        &assets,
        character,
        "animated_scenes/motion_overlay.animscn.ron",
    );

    if scripted {
        commands
            .entity(character)
            .insert(AlsInputOverride::default());
        info!("[M7GATE] scripted demo armed");
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
    verdict: Res<GateVerdict>,
    mut q: Query<(&mut AlsInputOverride, &mut AlsFootPinHold), With<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_err() || driver.done {
        return;
    }
    let Ok((mut over, mut pin_hold)) = q.single_mut() else {
        return;
    };
    let t = time.elapsed_secs_f64();
    if t >= TOTAL_SECS {
        driver.done = true;
        if verdict.failed {
            error!("[M7GATE] demo complete with FAILED verdict at t={t:.2}");
        } else {
            info!("[M7GATE] demo complete at t={t:.2}");
            exit.write(AppExit::Success);
        }
        return;
    }
    let idx = phase_at(t);
    let entered = idx != driver.phase;
    if entered {
        driver.phase = idx;
        info!(
            "[M7GATE][PHASE {}] {} (t={t:.2}, start={:.2})",
            idx,
            phase_desc(idx),
            phase_start(idx)
        );
    }
    apply_phase_motion(idx, t, time.delta_secs_f64() as f32, &mut over);
    if idx == 4 {
        pin_hold.until_secs = time.elapsed_secs_f64() + 0.25;
    }
}

fn apply_phase_motion(phase: usize, t: f64, dt: f32, over: &mut AlsInputOverride) {
    let mut move_dir = Vec2::ZERO;
    let mut look = Vec2::ZERO;
    let local = t - phase_start(phase);

    match phase {
        1 => move_dir = Vec2::NEG_Y,
        3 => move_dir = Vec2::Y,
        4 => {
            if local >= SPIN_DELAY_SECS && local < SPIN_DELAY_SECS + SPIN_WINDOW_SECS {
                look.x = -(TURN_RATE_DEG_PER_S.to_radians() * dt * LOOK_UNITS_PER_RAD);
            }
        }
        _ => {}
    }

    over.0.move_dir = move_dir;
    over.0.look_delta = look;
    over.0.aim_held = false;
    over.0.jump_pressed = false;
    over.0.jump_held = false;
}

#[allow(clippy::type_complexity)]
fn als_detectors_system(
    time: Res<Time>,
    mut prev_view: Local<Option<(f64, f32)>>,
    mut yaw_env: Local<f32>,
    mut q: Query<
        (
            &LinearVelocity,
            &Transform,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            &AlsViewYaw,
            &LocomotionState,
            &mut StartStopState,
            Option<&mut AlsFootPinHold>,
            &mut TurnDetector,
        ),
        With<AlsCharacter>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((vel, tf, cmd, ovr, view, loco, mut ss, hold, mut turn)) = q.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let input = ovr.map(|o| &o.0).unwrap_or(cmd);
    let hspeed = Vec2::new(vel.x, vel.z).length();

    let dir = classify(
        als_core::motion::view_relative_move_axes(input.move_dir, view.0),
        view.0,
    );
    if let Some(phase) = update_start_stop(&mut ss, &StartStopConfig::default(), hspeed, dir) {
        info!("[M7GATE][STARTSTOP] phase -> {phase:?}");
    }

    let view_deg = view.0.to_degrees();
    let fwd = tf.forward();
    let body_deg = f32::atan2(fwd.x, -fwd.z).to_degrees();
    let sample_dt = match *prev_view {
        Some((pt, _)) => (time.elapsed_secs_f64() - pt) as f32,
        None => 0.0,
    };
    let inst_yaw_speed = match *prev_view {
        Some((_, pv)) if sample_dt > 1e-4 => wrap_angle_degrees(view_deg - pv) / sample_dt,
        _ => 0.0,
    };
    *yaw_env = (*yaw_env - YAW_SPEED_ENVELOPE_DECAY * sample_dt)
        .max(0.0)
        .max(inst_yaw_speed.abs());
    *prev_view = Some((time.elapsed_secs_f64(), view_deg));

    let rel = wrap_angle_degrees(view_deg - body_deg);
    let allowed = *loco == LocomotionState::Grounded && hspeed < MOVE_EPSILON;
    if let Some(ev) = turn.0.update(
        rel,
        *yaw_env,
        dt,
        allowed,
        false,
        &TurnInPlaceConfig::default(),
    ) {
        info!(
            "[M7GATE][TURN] variant={:?} rel={rel:.1}deg play_rate={:.2} -> extending foot pin hold",
            ev.variant, ev.play_rate
        );
        match hold {
            Some(mut h) => extend_foot_pin_hold(&mut h, time.elapsed_secs_f64()),
            None => {
                error!("[M7GATE] AlsFootPinHold missing on character");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(5).unwrap()));
            }
        }
    }
}

fn graph_input_router_system(
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
        error!(
            "[M7GATE] fell below floor y={:.2} -- aborting",
            char_t.translation.y
        );
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
    }
}

#[allow(clippy::type_complexity)]
fn metrics_logger(
    time: Res<Time>,
    mut last: Local<f32>,
    mut watch: Local<PinWatch>,
    mut q: Query<
        (
            &Transform,
            &LinearVelocity,
            &LocomotionState,
            &AlsFootBones,
            &AlsFootProbe,
            &AlsFootIkRuntime,
            &AlsFootLockSettings,
            &AlsDynamicTransitionState,
            Option<&AlsFootPinRequest>,
        ),
        With<AlsCharacter>,
    >,
    bone_globals: Query<&GlobalTransform, Without<AlsCharacter>>,
    instances: Query<&AnimatedSceneInstance>,
    players: Query<&AnimationGraphPlayer>,
    mut verdict: ResMut<GateVerdict>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs_f64();

    for instance in instances.iter() {
        if let Ok(player) = players.get(instance.player_entity())
            && let Some(err) = player.get_error()
        {
            error!("[M7GATE] graph error at t={t:.2}: {err:?}");
            verdict.failed = true;
            exit.write(AppExit::Error(std::num::NonZeroU8::new(4).unwrap()));
            return;
        }
    }

    if (t as f32) - *last < SNAPSHOT_INTERVAL {
        return;
    }
    *last = t as f32;

    let Ok((char_tf, vel, loco, bones, probe, runtime, lock_settings, dynamic, pins)) =
        q.single_mut()
    else {
        return;
    };
    if !watch.ready_logged {
        watch.ready_logged = true;
        info!(
            "[BCHAR18F][SOURCE] smooth={:.2} lock={} thigh={:.1} foot={:.1} release={:.1}/{:.1} dynamic=({:.2},{:.2},{:.1},{})",
            lock_settings.moving_smooth_speed_threshold,
            lock_settings.allow_foot_lock,
            lock_settings.thigh_angle_limit_degrees,
            lock_settings.foot_angle_limit_degrees,
            lock_settings.moving_decrease_per_second,
            lock_settings.not_grounded_decrease_per_second,
            als_core::transitions::DYNAMIC_TRANSITION_FOOT_LOCK_DISTANCE_METERS,
            als_core::transitions::DYNAMIC_TRANSITION_BLEND_SECS,
            als_core::transitions::DYNAMIC_TRANSITION_PLAY_RATE,
            als_core::transitions::DYNAMIC_TRANSITION_FRAME_DELAY,
        );
    }

    let phase = phase_at(t);
    let hspeed = Vec2::new(vel.x, vel.z).length();
    let mut line = format!(
        "[BCHAR18F] t={t:.2} phase={} loco={loco:?} v={hspeed:.2} pelvis_drop={:+.3} dynamic={:?} d2=({:.4},{:.4}) delay={}",
        phase_desc(phase),
        runtime.pelvis_drop,
        dynamic.selection,
        dynamic.left_distance_squared,
        dynamic.right_distance_squared,
        dynamic.frames_remaining,
    );

    let mut pens = [f32::NAN; 2];
    for side in 0..2usize {
        let side_name = if side == 0 { "L" } else { "R" };
        let hit = if side == 0 { probe.left } else { probe.right };
        let chain = bones.chain(side);
        let ankle_y = match bone_globals.get(chain.foot) {
            Ok(g) => g.translation().y,
            Err(_) => continue,
        };
        let ground_y = hit.map(|h| h.point.y).unwrap_or(f32::NAN);
        let pen = if hit.is_some() {
            (ankle_y - ANKLE_TO_SOLE) - ground_y
        } else {
            f32::NAN
        };
        pens[side] = pen;
        if pen.is_finite() && pen < watch.pen_worst {
            watch.pen_worst = pen;
        }

        let (offset, pin_w) = runtime.side(side);
        let pinned_now = pins
            .map(|p| if side == 0 { p.left } else { p.right })
            .unwrap_or(false);

        if phase == 4 && pinned_now && pin_w > 0.5 {
            if hspeed >= MOVE_EPSILON {
                watch.stop_pins_seen = true;
            } else {
                watch.turn_engaged = true;
                let root_xz = Vec3::new(char_tf.translation.x, 0.0, char_tf.translation.z);
                match watch.root_origin {
                    None => watch.root_origin = Some(root_xz),
                    Some(o) => {
                        let drift = root_xz.distance(o);
                        if drift > watch.max_root_drift {
                            watch.max_root_drift = drift;
                        }
                    }
                }
                let slot = &mut watch.lock_offsets[side];
                match *slot {
                    None => *slot = Some(offset),
                    Some(lock) => {
                        let shift = (offset - lock).abs();
                        if shift > watch.max_lock_shift {
                            watch.max_lock_shift = shift;
                        }
                    }
                }
            }
        }

        line.push_str(&format!(
            " {side_name}[ankle_y={ankle_y:.3} ground={ground_y:.3} pen={pen:+.3} off={offset:+.3} pin={pin_w:.2}{}]",
            if pinned_now { "*" } else { "" },
        ));
    }

    if phase == 2 && hspeed < MOVE_EPSILON {
        let both = probe.left.is_some() && probe.right.is_some();
        let pens_ok = pens
            .iter()
            .all(|p| !p.is_finite() || (*p >= -PENETRATION_TOL && *p <= STAND_FLOAT_TOL));
        let drop_ok = runtime.pelvis_drop <= 0.001 && runtime.pelvis_drop >= PELVIS_DROP_MIN;
        if !(both && pens_ok && drop_ok) {
            watch.stand_bad = true;
        }
    }

    info!("{line}");

    if t >= VERDICT_AT_SECS && !watch.verdict_done {
        watch.verdict_done = true;
        let pen_ok = watch.pen_worst >= -PENETRATION_TOL;
        let stand_ok = !watch.stand_bad;
        let turn_ok = watch.turn_engaged
            && watch.max_root_drift <= ROOT_DRIFT_TOL
            && watch.max_lock_shift <= LOCK_SHIFT_TOL;
        info!(
            "[M7GATE][VERDICT] min_penetration={:+.3} (tol {:-.3}) -> {}",
            watch.pen_worst,
            -PENETRATION_TOL,
            if pen_ok { "PASS" } else { "FAIL" }
        );
        info!(
            "[M7GATE][VERDICT] slope_stand feet+pens+pelvis_drop -> {}",
            if stand_ok { "PASS" } else { "FAIL" }
        );
        info!(
            "[M7GATE][VERDICT] turn_pins engaged={} root_drift={:.3}/{:.3} lock_shift={:.3}/{:.3} stop_pins_seen={} -> {}",
            watch.turn_engaged,
            watch.max_root_drift,
            ROOT_DRIFT_TOL,
            watch.max_lock_shift,
            LOCK_SHIFT_TOL,
            watch.stop_pins_seen,
            if turn_ok { "PASS" } else { "FAIL" }
        );
        if !(pen_ok && stand_ok && turn_ok) {
            verdict.failed = true;
            exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
        }
    }
}

fn auto_quit(quit: ResMut<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= quit.0 {
        warn!("[M7GATE] wall-clock safety quit");
        exit.write(AppExit::Success);
    }
}
