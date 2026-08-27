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
use als_core::debug_hud::AlsStateHudPlugin;
use als_core::input::{AlsInputCommand, AlsInputMap, AlsInputPlugin, spawn_als_actions};
use als_core::motion::{
    AlsInputOverride, AlsMotionPlugin, AlsViewYaw, effective_gait, gait_max_speed,
};
use als_core::params::AlsMovementParams;
use als_core::pivot::{PivotConfig, PivotDetector};
use als_core::start_stop::{StartStopConfig, StartStopState, classify, update_start_stop};
use als_core::state::{Gait, LocomotionState, RotationMode, Stance};
use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin};
use als_core::turn_in_place::{TurnInPlaceConfig, TurnInPlaceDetector, wrap_angle_degrees};
use als_ext::{AlsExtPlugin, PlaygroundBoxCollider, PlaygroundPlugin, PlaygroundSpawnPoint};

use bevy_als::motion_graph_link::{compute_motion_graph_inputs, write_motion_graph_pins};

#[derive(Component, Default)]
struct TurnInPlaceComp(TurnInPlaceDetector);

const SNAPSHOT_INTERVAL: f32 = 0.5;
const LOOK_UNITS_PER_RAD: f32 = 400.0;
const TURN_RATE_DEG_PER_S: f32 = 100.0;
const TURN_TARGET_DEGREES: f32 = 120.0;
const MOVE_SPEED_EPSILON: f32 = 0.15;
const YAW_SPEED_ENVELOPE_DECAY: f32 = 250.0;
const REALIGN_RATE_DEG_PER_S: f32 = 120.0;
const BODY_VIEW_REALIGN_SECS: f64 = 1.125;
const STAND_RECENTER_SECS: f64 = 2.0;

const PHASE_DURATIONS: [f64; 7] = [2.0, 4.0, 3.0, 3.0, 3.5, 3.5, 3.0];
const LAST_PHASE: usize = 6;
const TOTAL_SECS: f64 = 22.0;

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
        AlsStateHudPlugin,
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
        PlaygroundPlugin {
            manifest: "assets/models/playground/playground.ron".into(),
        },
    ))
    .init_resource::<SpawnOnce>()
    .init_resource::<DemoDriver>()
    .add_systems(Update, (camera_follow, fall_guard, safety_quit))
    .add_systems(
        Update,
        (
            spawn_character_once,
            playground_physics_bridge,
            auto_demo,
            als_detectors_system,
            graph_input_router_system,
            snapshot_logger,
        ),
    );

    app.insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    });

    app.run();
}

#[derive(Resource, Default)]
struct SpawnOnce(bool);

#[derive(Resource, Default)]
struct DemoDriver {
    phase: usize,
    done: bool,
}

#[derive(Resource)]
struct AutoQuit(Option<Instant>);

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
        0 => "idle",
        1 => "start run forward",
        2 => "sprint full speed",
        3 => "hard stop",
        4 => "turn in place",
        5 => "crouch walk",
        _ => "stand up",
    }
}

fn spawn_character_once(
    mut commands: Commands,
    mut once: ResMut<SpawnOnce>,
    spawn_point: Option<Res<PlaygroundSpawnPoint>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    assets: Res<AssetServer>,
) {
    if once.0 || spawn_point.is_none() {
        return;
    }
    once.0 = true;
    let point = *spawn_point.unwrap();
    info!(
        "[M5GATE] spawning character at ({:.2},{:.2},{:.2})",
        point.position.x, point.position.y, point.position.z
    );

    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_translation(point.position).with_rotation(point.rotation);
    let spawn_fwd = bundle.transform.forward();
    let scripted = std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok();
    let character = commands
        .spawn(bundle)
        .insert((
            AlsViewYaw(f32::atan2(spawn_fwd.x, -spawn_fwd.z)),
            StartStopState::default(),
            PivotDetector::new(PivotConfig::default()),
            TurnInPlaceComp::default(),
        ))
        .id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());

    if scripted {
        commands
            .entity(character)
            .insert(AlsInputOverride::default());
        info!("[M5GATE] scripted demo armed");
    }

    let avatar = commands
        .spawn(AnimatedSceneHandle::new(
            assets.load("animated_scenes/motion_graph.animscn.ron"),
        ))
        .id();
    commands.entity(character).add_child(avatar);

    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(point.position + Vec3::new(4.0, 3.5, 6.0)),
    ));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(40.0, 80.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    if scripted {
        info!("[M5GATE] auto quit armed");
        commands.insert_resource(AutoQuit(Some(
            Instant::now()
                + Duration::from_secs_f64(TOTAL_SECS + 35.0)
                + Duration::from_secs_f64(10.0),
        )));
    }
}

fn playground_physics_bridge(
    mut commands: Commands,
    bodies: Query<(Entity, &PlaygroundBoxCollider), Added<PlaygroundBoxCollider>>,
) {
    for (entity, collider) in &bodies {
        commands.entity(entity).insert((
            RigidBody::Static,
            Collider::cuboid(
                collider.half_extents.x * 2.0,
                collider.half_extents.y * 2.0,
                collider.half_extents.z * 2.0,
            ),
        ));
    }
}

fn auto_demo(
    time: Res<Time>,
    mut driver: ResMut<DemoDriver>,
    mut q: Query<
        (
            Entity,
            &Transform,
            &mut AlsInputOverride,
            &mut AlsStateRequest,
        ),
        With<AlsCharacter>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_err() || driver.done {
        return;
    }
    let Ok((entity, char_t, mut over, mut req)) = q.single_mut() else {
        return;
    };
    let t = time.elapsed_secs_f64();
    if t >= TOTAL_SECS {
        driver.done = true;
        info!("[M5GATE] demo complete at t={t:.2}");
        exit.write(AppExit::Success);
        return;
    }
    let idx = phase_at(t);
    let entered = idx != driver.phase;
    if entered {
        driver.phase = idx;
        info!(
            "[M5GATE][PHASE {}] {} (t={t:.2}, start={:.2})",
            idx,
            phase_desc(idx),
            phase_start(idx)
        );
    }
    apply_phase(
        idx,
        t,
        time.delta_secs_f64() as f32,
        &mut over,
        &mut req,
        entered,
    );
    let _ = entity;
    let _ = char_t;
}

fn apply_phase(
    phase: usize,
    t: f64,
    dt: f32,
    over: &mut AlsInputOverride,
    req: &mut AlsStateRequest,
    entered: bool,
) {
    let mut move_dir = Vec2::ZERO;
    let mut look_rate_deg_s = 0.0f32;

    match phase {
        1 => {
            move_dir = Vec2::NEG_Y;
            if entered {
                req.gait = Some(Gait::Running);
            }
        }
        2 => {
            move_dir = Vec2::NEG_Y;
            if entered {
                req.gait = Some(Gait::Sprinting);
            }
        }
        3 => {
            if t - phase_start(3) < BODY_VIEW_REALIGN_SECS {
                look_rate_deg_s = -REALIGN_RATE_DEG_PER_S;
            }
        }
        4 => {
            let local = t - phase_start(4);
            let spin_window = TURN_TARGET_DEGREES as f64 / TURN_RATE_DEG_PER_S as f64;
            if local >= 0.5 && local < 0.5 + spin_window {
                look_rate_deg_s = TURN_RATE_DEG_PER_S;
            }
        }
        5 => {
            move_dir = Vec2::NEG_Y;
            if entered {
                req.stance = Some(Stance::Crouching);
                req.gait = Some(Gait::Walking);
            }
        }
        6 => {
            if entered {
                req.stance = Some(Stance::Standing);
                req.gait = Some(Gait::Walking);
            }
            if t - phase_start(6) < STAND_RECENTER_SECS {
                look_rate_deg_s = -(TURN_TARGET_DEGREES / STAND_RECENTER_SECS as f32);
            }
        }
        _ => {}
    }

    over.0.move_dir = move_dir;
    over.0.look_delta = Vec2::new(look_rate_deg_s.to_radians() * dt * LOOK_UNITS_PER_RAD, 0.0);
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
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            &AlsViewYaw,
            &Transform,
            &LocomotionState,
            &mut StartStopState,
            &mut PivotDetector,
            &mut TurnInPlaceComp,
        ),
        With<AlsCharacter>,
    >,
) {
    let Ok((vel, cmd, ovr, view, tf, loco, mut ss, mut pivot, mut turn)) = q.single_mut() else {
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
        info!("[M5GATE][STARTSTOP] phase -> {:?}", phase);
    }

    let vel_plane = Vec2::new(vel.x, -vel.z);
    if let Some(ev) = pivot.update(vel_plane, input.move_dir, dt) {
        info!(
            "[M5GATE][PIVOT] side={:?} angle={:.1}deg speed={:.2}",
            ev.side,
            ev.angle_radians.to_degrees(),
            ev.speed
        );
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
    let allowed = *loco == LocomotionState::Grounded && hspeed < MOVE_SPEED_EPSILON;
    if let Some(ev) = turn.0.update(
        rel,
        *yaw_env,
        dt,
        allowed,
        false,
        &TurnInPlaceConfig::default(),
    ) {
        info!(
            "[M5GATE][TURN] variant={:?} rel_angle={:.1}deg play_rate={:.2}",
            ev.variant, rel, ev.play_rate
        );
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

#[allow(clippy::too_many_arguments)]
fn snapshot_logger(
    time: Res<Time>,
    mut last: Local<f32>,
    q: Query<
        (
            &Transform,
            &LinearVelocity,
            &LocomotionState,
            &Stance,
            &Gait,
            &RotationMode,
            &AlsViewYaw,
            &StartStopState,
            &als_core::character::AlsMovementParamsHandle,
        ),
        With<AlsCharacter>,
    >,
    instances: Query<&AnimatedSceneInstance>,
    players: Query<&AnimationGraphPlayer>,
    params: Res<Assets<AlsMovementParams>>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs_f64();
    if (t as f32) - *last < SNAPSHOT_INTERVAL {
        return;
    }
    *last = t as f32;
    let Ok((tf, vel, loco, stance, gait, rot, view, ss, handle)) = q.single() else {
        return;
    };
    let Some(p) = params.get(handle.id()) else {
        return;
    };
    let hspeed = Vec2::new(vel.x, vel.z).length();
    let eff = effective_gait(*stance, *gait);
    let target = gait_max_speed(p, stance, &eff);
    let mgi = compute_motion_graph_inputs(p, *stance, *gait, hspeed);
    let fwd = tf.forward();
    let body_deg = f32::atan2(fwd.x, -fwd.z).to_degrees();
    let view_deg = view.0.to_degrees();
    let rel = wrap_angle_degrees(view_deg - body_deg);

    let mut player_elapsed = "-".to_string();
    let mut graph_err: Option<String> = None;
    for instance in instances.iter() {
        if let Ok(player) = players.get(instance.player_entity()) {
            player_elapsed = format!("{:.3}", player.elapsed());
            graph_err = player.get_error().map(|e| format!("{e:?}"));
        }
    }
    if let Some(err) = graph_err {
        error!("[M5GATE] graph error at t={t:.2}: {err}");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(4).unwrap()));
    }

    info!(
        "[M5GATE] t={t:.2} phase={} loco={loco:?} stance={stance:?} gait={gait:?} eff={eff:?} rot={rot:?} speed_pin={:.2} actual={:.2} target={target:.2} rate={:.2} pos=({:.2},{:.2},{:.2}) body={body_deg:.0} view={view_deg:.0} rel={rel:.0} ss={ss:?} player_elapsed={player_elapsed}",
        phase_desc(phase_at(t)),
        mgi.speed,
        mgi.actual_speed,
        mgi.play_rate,
        tf.translation.x,
        tf.translation.y,
        tf.translation.z,
    );
}

fn camera_follow(
    mut cameras: Query<&mut Transform, (With<Camera3d>, Without<AlsCharacter>)>,
    characters: Query<&Transform, (With<AlsCharacter>, Without<Camera3d>)>,
) {
    let Ok(char_t) = characters.single() else {
        return;
    };
    for mut cam in cameras.iter_mut() {
        cam.translation = char_t.translation + Vec3::new(2.5, 2.2, 4.0);
        cam.look_at(char_t.translation + Vec3::Y * 0.9, Vec3::Y);
    }
}

fn fall_guard(q: Query<&Transform, With<AlsCharacter>>, mut exit: MessageWriter<AppExit>) {
    let Ok(char_t) = q.single() else {
        return;
    };
    if char_t.translation.y < -20.0 {
        error!(
            "[M5GATE] fell below floor y={:.2} -- aborting",
            char_t.translation.y
        );
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
    }
}

fn safety_quit(quit: Option<Res<AutoQuit>>, mut exit: MessageWriter<AppExit>) {
    let Some(aq) = quit else {
        return;
    };
    let Some(deadline) = aq.0 else {
        return;
    };
    if Instant::now() >= deadline {
        warn!("[M5GATE] safety quit");
        exit.write(AppExit::Success);
    }
}
