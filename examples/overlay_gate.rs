use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::asset::{AssetEvent, Assets};
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::skeleton::Skeleton;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_core::character::{AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::debug_hud::AlsStateHudPlugin;
use als_core::input::{AlsInputCommand, AlsInputMap, AlsInputPlugin, spawn_als_actions};
use als_core::motion::{AlsInputOverride, AlsMotionPlugin, AlsViewYaw};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin};
use als_core::view::{AlsViewPitch, AlsViewSmoothed};
use als_ext::{AlsExtPlugin, PlaygroundBoxCollider, PlaygroundPlugin, PlaygroundSpawnPoint};

use bevy_als::motion_graph_link::{compute_motion_graph_inputs, write_motion_graph_pins};
use bevy_als::overlay_layer::OverlayLayerPlugin;
use bevy_als::view_layer::ViewLayerPlugin;

const SNAPSHOT_INTERVAL: f32 = 0.25;
const MOVE_SPEED_EPSILON: f32 = 0.15;
const PITCH_SCALE: f32 = 0.0022;

const PHASE_DURATIONS: [f64; 6] = [1.5, 3.5, 4.0, 4.0, 2.0, 3.0];
const LAST_PHASE: usize = 5;
const TOTAL_SECS: f64 = 18.0;

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
        AlsStateHudPlugin,
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
    ));
    app.add_plugins((
        OverlayLayerPlugin,
        ViewLayerPlugin,
        PlaygroundPlugin {
            manifest: "assets/models/playground/playground.ron".into(),
        },
    ));
    app.init_resource::<SpawnOnce>();
    app.init_resource::<DemoDriver>();
    app.init_resource::<SkeletonReady>();
    app.insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    });
    app.add_systems(
        Update,
        (
            spawn_character_once,
            playground_physics_bridge,
            auto_demo,
            overlay_mirror,
            pitch_integrator,
            mask_pin_writer,
            aim_event_logger,
            graph_input_router_system,
            camera_follow,
            fall_guard,
            snapshot_logger,
            safety_quit,
        ),
    );

    if let Ok(secs) = std::env::var("ALS_BOOT_AUTO_QUIT_SECS") {
        let secs: f64 = secs.parse().unwrap_or(30.0);
        info!("[M6GATE] auto quit armed in {secs:.1}s");
        app.insert_resource(AutoQuit(Some(
            Instant::now() + Duration::from_secs_f64(secs),
        )));
        app.add_systems(Update, auto_quit);
    }

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
struct SkeletonReady {
    handle: Option<Handle<Skeleton>>,
    sent: bool,
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
        0 => "idle default overlay",
        1 => "walk baseline",
        2 => "walk + torch overlay blend-in",
        3 => "aim quadrants sweep",
        4 => "aim release (torch held)",
        _ => "overlay blend-out idle",
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
        "[M6GATE] spawning character at ({:.2},{:.2},{:.2})",
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
        .insert(AlsViewYaw(f32::atan2(spawn_fwd.x, -spawn_fwd.z)))
        .id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());

    let avatar = commands
        .spawn((
            AnimatedSceneHandle::new(assets.load("animated_scenes/motion_overlay.animscn.ron")),
            OverlayState::Default,
            AlsViewPitch(0.0),
        ))
        .id();
    commands.entity(character).add_child(avatar);

    if scripted {
        commands
            .entity(character)
            .insert(AlsInputOverride::default());
        info!("[M6GATE] scripted demo armed");
    }

    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(point.position + Vec3::new(4.0, 3.0, 5.0)),
    ));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(40.0, 80.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
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
    mut q: Query<(&Transform, &mut AlsInputOverride, &mut AlsStateRequest), With<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_err() || driver.done {
        return;
    }
    let Ok((_char_t, mut over, mut req)) = q.single_mut() else {
        return;
    };
    let t = time.elapsed_secs_f64();
    if t >= TOTAL_SECS {
        driver.done = true;
        info!("[M6GATE] demo complete at t={t:.2}");
        exit.write(AppExit::Success);
        return;
    }
    let idx = phase_at(t);
    let entered = idx != driver.phase;
    if entered {
        driver.phase = idx;
        info!(
            "[M6GATE][PHASE {}] {} (t={t:.2}, start={:.2})",
            idx,
            phase_desc(idx),
            phase_start(idx)
        );
        match idx {
            1 => req.gait = Some(Gait::Running),
            2 => {
                req.overlay_state = Some(OverlayState::Torch);
                info!("[M6GATE] request overlay -> Torch (while walking)");
            }
            5 => {
                req.overlay_state = Some(OverlayState::Default);
                info!("[M6GATE] request overlay -> Default");
            }
            _ => {}
        }
    }
    apply_phase_motion(idx, t, time.delta_secs_f64() as f32, &mut over);
}

fn apply_phase_motion(phase: usize, t: f64, dt: f32, over: &mut AlsInputOverride) {
    let local = t - phase_start(phase);
    let mut move_dir = Vec2::ZERO;
    let mut look = Vec2::ZERO;

    match phase {
        1 | 2 | 3 => {
            move_dir = Vec2::NEG_Y;
            look.y = pitch_sway_units(local, dt, phase);
        }
        _ => {}
    }

    if phase == 3 {
        over.0.move_dir = move_dir;
        over.0.aim_held = true;
        over.0.jump_pressed = false;
        over.0.jump_held = false;
        let sweep_local = (local - 0.4).max(0.0) as f32;
        if sweep_local < SWEEP_TOTAL_SECS {
            look.x = -(YAW_SWEEP_DEG_PER_S.to_radians() * dt * LOOK_UNITS_PER_RAD);
        }
        over.0.look_delta = look;
        return;
    }

    over.0.move_dir = move_dir;
    over.0.look_delta = look;
    over.0.aim_held = false;
    over.0.jump_pressed = false;
    over.0.jump_held = false;
}

fn pitch_sway_units(local: f64, dt: f32, phase: usize) -> f32 {
    let rate_deg_s = if (phase == 1 && local > 1.0) || (phase == 2 && local <= 1.5) {
        -PITCH_SWAY_DEG_PER_S
    } else if phase == 1 || phase == 2 || phase == 3 {
        PITCH_SWAY_DEG_PER_S
    } else {
        0.0
    };
    -rate_deg_s.to_radians() * dt / PITCH_SCALE
}

const YAW_SWEEP_DEG_PER_S: f32 = 100.0;
const SWEEP_TOTAL_SECS: f32 = 2.8;
const PITCH_SWAY_DEG_PER_S: f32 = 20.0;
const LOOK_UNITS_PER_RAD: f32 = 400.0;

fn quadrant_label(yaw_deg: f32) -> &'static str {
    let wrapped = ((yaw_deg + 180.0).rem_euclid(360.0)) / 90.0;
    const NAMES: [&str; 4] = ["SW", "NW", "NE", "SE"];
    NAMES[(wrapped as usize) % 4]
}

fn overlay_mirror(
    characters: Query<&OverlayState, (With<AlsCharacter>, Changed<OverlayState>)>,
    mut avatars: Query<&mut OverlayState, Without<AlsCharacter>>,
) {
    let Some(char_overlay) = characters.iter().next() else {
        return;
    };
    for mut av in avatars.iter_mut() {
        if *av != *char_overlay {
            *av = *char_overlay;
        }
    }
}

fn pitch_integrator(
    characters: Query<(&AlsInputCommand, Option<&AlsInputOverride>), With<AlsCharacter>>,
    children_q: Query<&Children, With<AlsCharacter>>,
    mut avatar_pitch: Query<&mut AlsViewPitch, Without<AlsCharacter>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let Ok((cmd, ovr)) = characters.single() else {
        return;
    };
    let effective = ovr.map(|o| &o.0).unwrap_or(cmd);
    let dy = -effective.look_delta.y * PITCH_SCALE;
    let Ok(children) = children_q.single() else {
        return;
    };
    for child in children.iter() {
        if let Ok(mut pitch) = avatar_pitch.get_mut(child) {
            pitch.0 = (pitch.0 + dy).clamp(-85.0f32.to_radians(), 85.0f32.to_radians());
        }
    }
}

fn track_skeleton(
    assets: Res<AssetServer>,
    skeletons: Res<Assets<Skeleton>>,
    mut ev: MessageReader<AssetEvent<Skeleton>>,
    mut ready: ResMut<SkeletonReady>,
) {
    if ready.handle.is_none() {
        ready.handle = Some(assets.load("skeletons/SKM_Als.skn.ron"));
    }
    let Some(handle) = ready.handle.clone() else {
        return;
    };
    for e in ev.read() {
        if let AssetEvent::LoadedWithDependencies { id } = e {
            if *id == handle.id() && skeletons.contains(&handle) && !ready.sent {
                info!("[M6GATE] skeleton loaded; masks armed");
                ready.sent = true;
            }
        }
    }
}

fn mask_pin_writer(
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("diff_factor", DataValue::from(1.0_f32));
        }
    }
}

fn aim_event_logger(mut ev: MessageReader<als_core::aim::AimStateChanged>) {
    for event in ev.read() {
        info!(
            "[M6GATE][AIM] entity={:?} aiming={} (broadcast to camera hook)",
            event.entity, event.aiming
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

#[allow(clippy::type_complexity)]
fn snapshot_logger(
    time: Res<Time>,
    mut last: Local<f32>,
    q_char: Query<
        (
            &Transform,
            &LinearVelocity,
            &LocomotionState,
            &Stance,
            &Gait,
            &RotationMode,
            &AlsViewYaw,
            &OverlayState,
            &als_core::character::AlsMovementParamsHandle,
        ),
        With<AlsCharacter>,
    >,
    q_avatar: Query<
        (
            &OverlayState,
            &AlsViewSmoothed,
            Option<&als_core::overlay_blend::OverlayBlendState>,
        ),
        Without<AlsCharacter>,
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
    let Ok((tf, vel, loco, stance, gait, rot, view, char_ov, handle)) = q_char.single() else {
        return;
    };
    let Some(p) = params.get(handle.id()) else {
        return;
    };
    let hspeed = Vec2::new(vel.x, vel.z).length();
    let mgi = compute_motion_graph_inputs(p, *stance, *gait, hspeed);
    let fwd = tf.forward();
    let body_deg = f32::atan2(fwd.x, -fwd.z).to_degrees();
    let view_deg = view.0.to_degrees();

    let (av_ov, smooth_pitch, blend_w) = q_avatar
        .single()
        .map(|(ov, s, b)| {
            (
                *ov,
                s.0.to_degrees(),
                b.map(|b| b.last_weight).unwrap_or(0.0),
            )
        })
        .unwrap_or((*char_ov, f32::NAN, 0.0));

    let mut elapsed_s = "-".to_string();
    for instance in instances.iter() {
        if let Ok(player) = players.get(instance.player_entity()) {
            elapsed_s = format!("{:.3}", player.elapsed());
            if let Some(err) = player.get_error() {
                error!("[M6GATE] graph error at t={t:.2}: {err:?}");
                exit.write(AppExit::Error(std::num::NonZeroU8::new(4).unwrap()));
            }
        }
    }

    info!(
        "[M6GATE] t={t:.2} phase={} loco={loco:?} stance={stance:?} gait={gait:?} rot={rot:?} char_ov={char_ov:?} avatar_ov={av_ov:?} w={blend_w:.3} speed_pin={:.2} actual={:.2} rate={:.2} pos=({:.2},{:.2},{:.2}) body={body_deg:.0} view={view_deg:.0} quad={} pitch_s={smooth_pitch:.1}deg elapsed={elapsed_s}",
        phase_desc(phase_at(t)),
        mgi.speed,
        mgi.actual_speed,
        mgi.play_rate,
        tf.translation.x,
        tf.translation.y,
        tf.translation.z,
        quadrant_label(view_deg),
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
            "[M6GATE] fell below floor y={:.2} -- aborting",
            char_t.translation.y
        );
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
    }
}

fn auto_quit(mut quit: Option<ResMut<AutoQuit>>, mut exit: MessageWriter<AppExit>) {
    let Some(aq) = quit.as_deref_mut() else {
        return;
    };
    let Some(deadline) = aq.0 else {
        return;
    };
    if Instant::now() >= deadline {
        warn!("[M6GATE] safety quit");
        exit.write(AppExit::Success);
    }
}

fn safety_quit(time: Res<Time>, mut exit: MessageWriter<AppExit>) {
    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() && time.elapsed_secs_f64() >= 45.0 {
        warn!("[M6GATE] hard safety quit");
        exit.write(AppExit::Success);
    }
}
