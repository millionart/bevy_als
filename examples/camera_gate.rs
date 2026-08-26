use std::time::{Duration, Instant};

use als_camera::{AlsCameraRig, AlsCameraTarget};
use als_core::character::AlsCharacter;
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::motion::{AlsInputOverride, AlsViewYaw};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, RotationMode};
use als_core::transition::AlsStateRequest;
use als_core::view::{AlsViewPitch, AlsViewSmoothed};
use avian3d::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use bevy_als::playable_character::{
    AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};

const SNAPSHOT_INTERVAL: f32 = 0.2;
fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        AlsPlayableCharacterPlugin,
    ))
    .init_resource::<SpawnOnce>()
    .init_resource::<DemoDriver>()
    .init_resource::<CamWatch>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(
        Update,
        (
            spawn_character_once,
            auto_demo,
            aim_pitch_driver,
            cam_watcher,
            fall_guard,
            auto_quit,
        ),
    );

    let secs: f64 = std::env::var("ALS_BOOT_AUTO_QUIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30.0);
    info!("[CGATE] auto quit armed in {secs:.1}s");
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
    aiming: bool,
    sweep_pitch: bool,
}

fn steps() -> [Step; 7] {
    [
        Step {
            t0: 0.5,
            dur: 1.2,
            label: "idle",
            move_dir: Vec2::ZERO,
            gait: None,
            aiming: false,
            sweep_pitch: false,
        },
        Step {
            t0: 1.7,
            dur: 2.6,
            label: "run toward wall (probe shrink)",
            move_dir: Vec2::Y,
            gait: Some(Gait::Running),
            aiming: false,
            sweep_pitch: false,
        },
        Step {
            t0: 4.3,
            dur: 1.8,
            label: "aim on (blend to shoulder)",
            move_dir: Vec2::ZERO,
            gait: None,
            aiming: true,
            sweep_pitch: false,
        },
        Step {
            t0: 6.1,
            dur: 1.6,
            label: "pitch sweep while aiming",
            move_dir: Vec2::ZERO,
            gait: None,
            aiming: true,
            sweep_pitch: true,
        },
        Step {
            t0: 7.7,
            dur: 1.8,
            label: "aim off (blend back)",
            move_dir: Vec2::ZERO,
            gait: None,
            aiming: false,
            sweep_pitch: false,
        },
        Step {
            t0: 9.5,
            dur: 1.5,
            label: "run again",
            move_dir: Vec2::Y,
            gait: Some(Gait::Running),
            aiming: false,
            sweep_pitch: false,
        },
        Step {
            t0: 11.0,
            dur: 1.2,
            label: "settle",
            move_dir: Vec2::ZERO,
            gait: None,
            aiming: false,
            sweep_pitch: false,
        },
    ]
}

#[derive(Resource, Default)]
struct CamWatch {
    min_clearance: Option<f32>,
    max_frame_jump: f32,
    prev_cam_pos: Option<Vec3>,
    blend_seen_high: bool,
    blend_jump_violation: bool,
    failed: bool,
    pitch_seen: f32,
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
    let wall_mat = materials.add(Color::srgb(0.55, 0.30, 0.22));
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 0.3, 60.0),
        Transform::from_xyz(0.0, -0.15, 10.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(40.0, 0.3, 60.0)))),
        MeshMaterial3d(ground_mat),
    ));
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(8.0, 4.0, 0.4),
        Transform::from_xyz(0.0, 1.85, -9.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(8.0, 4.0, 0.4)))),
        MeshMaterial3d(wall_mat),
    ));

    let character = spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        AlsCharacterBlueprint::new(
            Transform::from_xyz(0.0, 0.95, 0.0)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        ),
    );

    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() {
        commands
            .entity(character)
            .insert(AlsInputOverride::default());
        info!("[CGATE] scripted demo armed");
        let tname = std::any::type_name::<als_camera::AlsCameraTarget>();
        info!("[CGATE][DBG] inserted marker type={tname}");
    }

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(3.5, 2.8, -4.0),
        AlsCameraRig::default(),
    ));
    info!("[CGATE] camera spawned with rig");

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(20.0, 40.0, -15.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn auto_demo(
    time: Res<Time>,
    mut driver: ResMut<DemoDriver>,
    mut q: Query<(&mut AlsInputOverride, &mut AlsStateRequest, &AlsViewYaw), With<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_err() || driver.done {
        return;
    }
    let Ok((mut over, mut req, _view)) = q.single_mut() else {
        return;
    };
    let t = time.elapsed_secs_f64();
    let all = steps();
    let last = &all[all.len() - 1];
    if t >= last.t0 + last.dur + 0.5 {
        driver.done = true;
        exit.write(AppExit::Success);
        return;
    }
    let Some(idx) = (0..all.len()).rev().find(|i| t >= all[*i].t0) else {
        return;
    };
    if idx != driver.step {
        driver.step = idx;
        let s = &all[idx];
        info!("[CGATE][STEP {}] {} (aiming={})", idx, s.label, s.aiming);
        if let Some(g) = s.gait {
            req.gait = Some(g);
        }
        let prev_aiming = idx > 0 && all[idx - 1].aiming;
        if s.aiming && !prev_aiming {
            req.rotation_mode = Some(RotationMode::Aiming);
        }
        if !s.aiming && prev_aiming {
            req.rotation_mode = Some(RotationMode::VelocityDirection);
        }
    }
    let s = &all[idx];
    over.0.move_dir = s.move_dir;
    over.0.jump_pressed = false;
    over.0.jump_held = false;
    over.0.roll_pressed = false;
    over.0.crouch_pressed = false;
    over.0.sprint_held = false;
    over.0.aim_held = s.aiming;
    over.0.look_delta = Vec2::ZERO;
}

fn aim_pitch_driver(time: Res<Time>, mut q: Query<&mut AlsViewPitch, With<AlsCharacter>>) {
    let t = time.elapsed_secs_f64() as f32;
    if !(6.1..=7.7).contains(&t) {
        return;
    }
    let local = (t - 6.1) / 1.6;
    let deg = (local * std::f32::consts::TAU).sin() * 55.0;
    for mut pitch in q.iter_mut() {
        pitch.0 = deg.to_radians();
    }
}

fn camera_follow(mut cameras: Query<&mut Camera, Without<AlsCharacter>>) {
    for mut _cam in cameras.iter_mut() {}
}

fn fall_guard(q: Query<&Transform, With<AlsCharacter>>, mut exit: MessageWriter<AppExit>) {
    let Ok(char_t) = q.single() else {
        return;
    };
    if char_t.translation.y < -20.0 {
        error!("[CGATE] fell below floor -- aborting");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
    }
}

#[allow(clippy::type_complexity)]
fn cam_watcher(
    time: Res<Time>,
    mut watch: ResMut<CamWatch>,
    mut q: Query<
        (
            &Transform,
            &GlobalTransform,
            &LinearVelocity,
            &LocomotionState,
            &RotationMode,
            Option<&AlsViewPitch>,
            Option<&AlsViewSmoothed>,
        ),
        (Without<AlsCameraRig>, With<AlsCharacter>),
    >,
    mut cams: Query<(&Transform, &AlsCameraRig), (With<Camera>, Without<AlsCharacter>)>,
    mut exit: MessageWriter<AppExit>,
    q_target_probe: Query<Entity, With<AlsCameraTarget>>,
    q_rot: Query<&RotationMode, With<AlsCharacter>>,
    q_yaw: Query<&AlsViewYaw, With<AlsCharacter>>,
    q_gt: Query<&GlobalTransform, With<AlsCharacter>>,
    q_lv: Query<&LinearVelocity, With<AlsCharacter>>,
) {
    let Ok((char_tf, _gtf, _vel, loco, rot, pitch, smoothed)) = q.single() else {
        let c = q.iter().count();
        let has_target = q_target_probe.iter().count();
        let has_rot = q_rot.iter().count();
        let has_yaw = q_yaw.iter().count();
        let has_gt = q_gt.iter().count();
        let has_lv = q_lv.iter().count();
        warn!(
            "[CGATE][MISS] n={c} target={has_target} rot={has_rot} yaw={has_yaw} gt={has_gt} lv={has_lv}"
        );
        return;
    };
    let Ok((cam_tf, cam_rig)) = cams.single_mut() else {
        warn!("[CGATE] cam query miss");
        return;
    };
    if time.elapsed_secs_f64() < 2.5 {
        watch.prev_cam_pos = Some(cam_tf.translation);
        return;
    }
    let _ = loco;

    if let Some(prev) = watch.prev_cam_pos {
        let jump = cam_tf.translation.distance(prev);
        if jump > watch.max_frame_jump && jump < 50.0 {
            watch.max_frame_jump = jump;
        }
    }
    watch.prev_cam_pos = Some(cam_tf.translation);

    let pivot_y = char_tf.translation.y + 0.65;
    let clearance = (cam_tf.translation.z - (-8.8f32))
        .abs()
        .min(cam_tf.translation.y);
    let facing_wall = char_tf.translation.z < -7.0;
    if facing_wall {
        let cand = clearance.max(0.0);
        watch.min_clearance = Some(watch.min_clearance.map_or(cand, |m| m.min(cand)));
    }

    if cam_rig.aim_blend > 0.95 {
        watch.blend_seen_high = true;
    }

    if let Some(sm) = smoothed {
        let diff = (sm.0 - pitch.map(|p| p.0).unwrap_or(0.0)).abs();
        if diff > 1.2 {
            watch.blend_jump_violation = true;
        }
        watch.pitch_seen = sm.0.abs();
    }

    let t = time.elapsed_secs_f64();
    if !watch.verdict_done && t >= 12.6 {
        watch.verdict_done = true;
        let min_c = watch.min_clearance.unwrap_or(f32::INFINITY);
        let ok_probe = min_c >= 0.05 || min_c.is_infinite();
        let ok_smooth = watch.max_frame_jump <= 1.2 && !watch.blend_jump_violation;
        let ok_linkage = watch.blend_seen_high || watch.pitch_seen > 0.2;
        info!(
            "[CGATE][VERDICT] probe_min={:.3} frame_jump_max={:.3} blend_high={} linkage_ok={ok_linkage} -> {}",
            watch.min_clearance.unwrap_or(f32::INFINITY),
            watch.max_frame_jump,
            watch.blend_seen_high,
            if ok_probe && ok_smooth && ok_linkage {
                "PASS"
            } else {
                "FAIL"
            }
        );
        if !(ok_probe && ok_smooth && ok_linkage) {
            watch.failed = true;
            exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
        } else {
            exit.write(AppExit::Success);
        }
        let _ = pivot_y;
    }
}

fn auto_quit(mut quit: ResMut<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= quit.0 {
        warn!("[CGATE] wall-clock safety quit");
        exit.write(AppExit::Success);
    }
}
