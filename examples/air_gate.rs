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

use als_core::air::{AirPhase, AlsAirPlugin, AlsAirState, LAND_HEAVY_IMPACT_VY};
use als_core::character::{AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::flail::AlsFlailState;
use als_core::input::{AlsInputMap, AlsInputPlugin, spawn_als_actions};
use als_core::motion::{AlsInputOverride, AlsMotionPlugin};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, Stance};
use als_core::transition::AlsStateTransitionPlugin;
use als_ext::AlsExtPlugin;

use bevy_als::motion_graph_link::{
    compute_motion_graph_inputs, write_air_pins, write_motion_graph_pins,
};

const PHASE_DURATIONS: [f64; 6] = [1.0, 2.0, 3.0, 2.0, 3.5, 1.5];
const LAST_PHASE: usize = 5;
const TOTAL_SECS: f64 = 13.0;
const SNAPSHOT_INTERVAL: f32 = 0.25;

const TELEPORT_HEIGHT: f32 = 20.0;

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
        AlsAirPlugin,
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
    ))
    .add_plugins(bevy_als::air_jump_layer::AirJumpLayerPlugin)
    .init_resource::<SpawnOnce>()
    .init_resource::<DemoDriver>()
    .init_resource::<GateVerdict>()
    .init_resource::<TeleportRequest>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(
        Update,
        (
            spawn_character_once,
            auto_demo,
            apply_teleport,
            air_pin_router,
            camera_follow,
            fall_guard,
            metrics_logger,
            auto_quit,
        ),
    );

    let secs: f64 = std::env::var("ALS_BOOT_AUTO_QUIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25.0);
    info!("[M8GATE] auto quit armed in {secs:.1}s");
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
    light_seen: bool,
    heavy_seen: bool,
    peak_fall_fast: f32,
    airborne_seen: bool,
    jump_variant_pins_seen: bool,
    jump_run_selected_at_speed: bool,
    flail_seen: bool,
    flail_pin_seen: bool,
    peak_flail: f32,
    failed: bool,
}

#[derive(Resource, Default)]
struct TeleportRequest(pub bool);

#[derive(Resource)]
struct AutoQuit(Instant);

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
        1 => "run forward",
        2 => "jump (expect light land)",
        3 => "recover + run",
        4 => "teleport high fall (expect heavy)",
        _ => "settle idle",
    }
}

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
        Transform::from_xyz(0.0, -0.15, 30.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(60.0, 0.3, 120.0)))),
        MeshMaterial3d(ground_mat),
    ));

    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_xyz(0.0, 0.95, 0.0)
        .with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
    let scripted = std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok();

    let character = commands.spawn(bundle).id();
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
        info!("[M8GATE] scripted demo armed");
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
    mut teleport: ResMut<TeleportRequest>,
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
    if t >= TOTAL_SECS {
        driver.done = true;
        if verdict.failed
            || !verdict.light_seen
            || !verdict.heavy_seen
            || !verdict.jump_variant_pins_seen
            || !verdict.jump_run_selected_at_speed
            || !verdict.flail_seen
            || !verdict.flail_pin_seen
        {
            error!(
                "[M8GATE][VERDICT] overall FAIL light={} heavy={} peak_ff={:.2} peak_flail={:.2} var_pins={} run_sel={} flail={} flail_pin={}",
                verdict.light_seen,
                verdict.heavy_seen,
                verdict.peak_fall_fast,
                verdict.peak_flail,
                verdict.jump_variant_pins_seen,
                verdict.jump_run_selected_at_speed,
                verdict.flail_seen,
                verdict.flail_pin_seen,
            );
            exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
        } else {
            info!(
                "[M8GATE][VERDICT] overall PASS light=true heavy=true var_pins=true run_sel=true flail=true flail_pin=true"
            );
            exit.write(AppExit::Success);
        }
        return;
    }
    let idx = phase_at(t);
    let entered = idx != driver.phase;
    if entered {
        driver.phase = idx;
        info!(
            "[M8GATE][PHASE {}] {} (t={t:.2}, start={:.2})",
            idx,
            phase_desc(idx),
            phase_start(idx)
        );
        if idx == 4 {
            teleport.0 = true;
        }
    }
    let local = t - phase_start(idx);
    let jump_hold = idx == 2 && entered;
    let jump_keep = idx == 2 && local < 0.08;
    if jump_hold {
        over.0.jump_pressed = true;
    } else {
        over.0.jump_pressed = false;
    }
    over.0.jump_held = jump_keep || jump_hold;
    over.0.move_dir = match idx {
        1 | 3 => Vec2::NEG_Y,
        2 => Vec2::NEG_Y,
        4 => Vec2::NEG_Y * (local.min(1.0) as f32),
        _ => Vec2::ZERO,
    };
    over.0.look_delta = Vec2::ZERO;
    over.0.aim_held = false;
}

fn apply_teleport(
    mut teleport: ResMut<TeleportRequest>,
    mut q: Query<&mut Transform, With<AlsCharacter>>,
) {
    if !teleport.0 {
        return;
    }
    teleport.0 = false;
    if let Ok(mut tf) = q.single_mut() {
        tf.translation.y += TELEPORT_HEIGHT;
        let expected_impact = (2.0 * 9.81 * TELEPORT_HEIGHT).sqrt();
        info!(
            "[M8GATE] teleported to y={:.2}, expected impact ~{expected_impact:.1} m/s (threshold {LAND_HEAVY_IMPACT_VY})",
            tf.translation.y
        );
    }
}

#[allow(clippy::type_complexity)]
fn air_pin_router(
    q: Query<
        (
            &Stance,
            &Gait,
            &LinearVelocity,
            &als_core::character::AlsMovementParamsHandle,
            &AlsAirState,
            &AlsFlailState,
        ),
        With<AlsCharacter>,
    >,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
    params: Res<Assets<AlsMovementParams>>,
) {
    let Ok((stance, gait, vel, handle, air, flail)) = q.single() else {
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
            write_air_pins(&mut player, air, flail);
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
        error!("[M8GATE] fell below floor -- aborting");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
    }
}

#[allow(clippy::type_complexity)]
fn metrics_logger(
    time: Res<Time>,
    mut last: Local<f32>,
    mut verdict: ResMut<GateVerdict>,
    mut q: Query<
        (
            Entity,
            &Transform,
            &LinearVelocity,
            &LocomotionState,
            &Gait,
            &AlsAirState,
            &AlsFlailState,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<&bevy_animation_graph::core::animated_scene::AnimatedSceneInstance>,
    players: Query<&AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs_f64();
    if (t as f32) - *last < SNAPSHOT_INTERVAL {
        return;
    }
    *last = t as f32;

    let Ok((_char_entity, tf, vel, loco, gait, air, flail)) = q.single_mut() else {
        return;
    };
    let instance = avatars
        .iter()
        .find(|i| i.player_entity() != Entity::PLACEHOLDER && players.contains(i.player_entity()));

    if matches!(air.phase, AirPhase::Airborne) {
        verdict.airborne_seen = true;
    }
    verdict.peak_fall_fast = verdict.peak_fall_fast.max(air.fall_fast_weight);
    verdict.peak_flail = verdict.peak_flail.max(flail.weight);
    if flail.weight > 0.5 {
        verdict.flail_seen = true;
    }
    if matches!(air.phase, AirPhase::Landing) && !air.landing_heavy && air.land_weight > 0.5 {
        verdict.light_seen = true;
        info!(
            "[M8GATE][LAND] light variant active w={:.2}",
            air.land_weight
        );
    }
    if matches!(air.phase, AirPhase::Landing) && air.landing_heavy && air.land_weight > 0.5 {
        verdict.heavy_seen = true;
        info!(
            "[M8GATE][LAND] heavy variant active w={:.2}/{:.2}",
            air.land_weight, air.land_heavy_weight
        );
    }
    let pin_check = instance.and_then(|i| players.get(i.player_entity()).ok());
    if let Some(player) = pin_check {
        let run_w = match player.get_input_data("jump_run_weight".into()) {
            Some(bevy_animation_graph::core::edge_data::DataValue::F32(v)) => v,
            _ => f32::NAN,
        };
        let left_w = match player.get_input_data("jump_left_weight".into()) {
            Some(bevy_animation_graph::core::edge_data::DataValue::F32(v)) => v,
            _ => f32::NAN,
        };
        let flail_w = match player.get_input_data("flail_weight".into()) {
            Some(bevy_animation_graph::core::edge_data::DataValue::F32(v)) => v,
            _ => f32::NAN,
        };
        if flail.weight > 0.5 && (flail_w - flail.weight).abs() < 1e-3 {
            verdict.flail_pin_seen = true;
            info!(
                "[M8GATE][FLAIL] active state_w={:.2} pin_w={flail_w:.2}",
                flail.weight
            );
        }
        if !run_w.is_nan() && !left_w.is_nan() && matches!(air.phase, AirPhase::Airborne) {
            verdict.jump_variant_pins_seen = true;
            let hspeed = Vec2::new(vel.x, vel.z).length();
            if hspeed > als_core::air_jump::JUMP_WALK_RUN_SPEED_SPLIT && run_w > 0.5 {
                verdict.jump_run_selected_at_speed = true;
                info!(
                    "[M8GATE][JUMP] run start selected at hspd={hspeed:.2} run_w={run_w:.1} left_w={left_w:.1}"
                );
            } else if hspeed <= als_core::air_jump::JUMP_WALK_RUN_SPEED_SPLIT {
                verdict.jump_run_selected_at_speed =
                    verdict.jump_run_selected_at_speed || (run_w < 0.5);
            }
        }
    }

    info!(
        "[M8GATE] t={t:.2} phase={} loco={loco:?} gait={gait:?} vy={:+.2} peak_dn={:.2} pos=({:.2},{:.2},{:.2}) air_w={:.2} ff_w={:.2} flail_w={:.2} land_w={:.2} heavy_w={:.2} impact={:.2} airphase={:?} heavy={}",
        phase_desc(phase_at(t)),
        vel.y,
        air.down_vy_smooth,
        tf.translation.x,
        tf.translation.y,
        tf.translation.z,
        air.air_weight,
        air.fall_fast_weight,
        flail.weight,
        air.land_weight,
        air.land_heavy_weight,
        air.last_impact_vy,
        air.phase,
        air.landing_heavy,
    );
}

fn auto_quit(mut quit: ResMut<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= quit.0 {
        warn!("[M8GATE] wall-clock safety quit");
        exit.write(AppExit::Success);
    }
}
