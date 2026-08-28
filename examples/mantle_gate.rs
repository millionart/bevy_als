use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_core::character::{AlsCharacter, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::input::AlsInputPlugin;
use als_core::mantle::{
    AlsMantleState, MANTLE_GROUNDED_TRACE, MANTLE_IN_AIR_TRACE, MANTLE_SLOPE_ANGLE_COS, MantleKind,
};
use als_core::motion::{AlsCharacterDebug, AlsInputOverride, AlsMotionPlugin};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, Stance};
use als_core::transition::AlsStateTransitionPlugin;
use als_ext::AlsExtPlugin;
use als_ext::overlay_materials::OverlayMaterialsPlugin;

use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use bevy_als::mantle_drive::AlsMantleDrivePlugin;
use bevy_als::motion_graph_link::{compute_motion_graph_inputs, write_motion_graph_pins};

#[derive(Resource, Default)]
struct MantleShot {
    path: Option<String>,
    edge_at: Option<Instant>,
    taken: bool,
    prev_active: bool,
}

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
        AlsMantleDrivePlugin,
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
        OverlayMaterialsPlugin,
    ))
    .init_resource::<SpawnOnce>()
    .init_resource::<DemoDriver>()
    .init_resource::<MantleWatch>()
    .init_resource::<MantleShot>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(
        Update,
        (
            spawn_character_once,
            auto_demo,
            mantle_watcher,
            graph_router,
            camera_follow,
            fall_guard,
            snapshot_logger,
            mantle_shot_system,
            auto_quit,
        ),
    );

    let secs: f64 = std::env::var("ALS_BOOT_AUTO_QUIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20.0);
    info!("[MGATE] auto quit armed in {secs:.1}s");
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

#[derive(Resource, Default)]
struct MantleWatch {
    prev_active: bool,
    count: u32,
    kinds: [Option<MantleKind>; 2],
    targets: [f32; 2],
    landings: [Option<f32>; 2],
    grounded_seen: [bool; 2],
    settle_timer: f32,
    pending_slot: Option<usize>,
    failed: bool,
}

struct Step {
    t0: f64,
    dur: f64,
    label: &'static str,
    gait: Option<Gait>,
}

fn steps() -> [Step; 5] {
    [
        Step {
            t0: 0.5,
            dur: 1.0,
            label: "idle",
            gait: None,
        },
        Step {
            t0: 1.5,
            dur: 2.2,
            label: "run to low ledge",
            gait: Some(Gait::Running),
        },
        Step {
            t0: 3.7,
            dur: 1.8,
            label: "cross low ledge top",
            gait: Some(Gait::Running),
        },
        Step {
            t0: 5.5,
            dur: 2.4,
            label: "run to high ledge",
            gait: Some(Gait::Running),
        },
        Step {
            t0: 7.9,
            dur: 3.8,
            label: "cross high ledge top",
            gait: Some(Gait::Running),
        },
    ]
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
    info!(
        "[BCHAR18M][SOURCE] grounded=({:.2},{:.2},{:.2},{:.2},{:.2}) in_air=({:.2},{:.2},{:.2},{:.2},{:.2}) slope_cos={MANTLE_SLOPE_ANGLE_COS:.6}",
        MANTLE_GROUNDED_TRACE.minimum_ledge_height,
        MANTLE_GROUNDED_TRACE.maximum_ledge_height,
        MANTLE_GROUNDED_TRACE.reach_distance,
        MANTLE_GROUNDED_TRACE.target_location_offset,
        MANTLE_GROUNDED_TRACE.start_location_offset,
        MANTLE_IN_AIR_TRACE.minimum_ledge_height,
        MANTLE_IN_AIR_TRACE.maximum_ledge_height,
        MANTLE_IN_AIR_TRACE.reach_distance,
        MANTLE_IN_AIR_TRACE.target_location_offset,
        MANTLE_IN_AIR_TRACE.start_location_offset,
    );
    let ground_mat = materials.add(Color::srgb(0.24, 0.27, 0.24));
    let box_mat = materials.add(Color::srgb(0.45, 0.38, 0.30));
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(60.0, 0.3, 80.0),
        Transform::from_xyz(0.0, -0.15, 10.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(60.0, 0.3, 80.0)))),
        MeshMaterial3d(ground_mat),
    ));
    let low_top = 0.8f32;
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(2.4, low_top, 1.2),
        Transform::from_xyz(0.0, low_top / 2.0, -4.5),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(2.4, low_top, 1.2)))),
        MeshMaterial3d(box_mat.clone()),
    ));
    let high_top = 1.35f32;
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(2.4, high_top, 1.2),
        Transform::from_xyz(0.0, high_top / 2.0, 2.5),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(2.4, high_top, 1.2)))),
        MeshMaterial3d(box_mat),
    ));

    let character = bevy_als::character_assembly::spawn_als_character(
        &mut commands,
        &assets,
        &mut materials,
        &mut params_assets,
        &mut config_assets,
        bevy_als::character_assembly::AlsCharacterBlueprint::new(
            Transform::from_xyz(0.0, 0.95, -12.0)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            "animated_scenes/motion_overlay.animscn.ron",
        ),
    );
    commands
        .entity(character)
        .insert(AlsCharacterDebug::default());

    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() {
        commands
            .entity(character)
            .insert(AlsInputOverride::default());
        info!("[MGATE] scripted demo armed");
    }

    commands.spawn((Camera3d::default(), Transform::from_xyz(4.0, 3.0, -14.0)));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(20.0, 40.0, -15.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn auto_demo(
    time: Res<Time>,
    mut driver: ResMut<DemoDriver>,
    mut q: Query<&mut AlsInputOverride, With<AlsCharacter>>,
    chars: Query<(&Transform, &AlsMantleState), With<AlsCharacter>>,
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
    if t >= all[all.len() - 1].t0 + all[all.len() - 1].dur + 1.4 {
        driver.done = true;
        exit.write(AppExit::Success);
        return;
    }
    let Some(idx) = (0..all.len()).rev().find(|i| t >= all[*i].t0) else {
        return;
    };
    if idx != driver.step {
        driver.step = idx;
        info!("[MGATE][STEP {}] {}", idx, all[idx].label);
        if let Some(g) = all[idx].gait {
            info!("[MGATE] gait request {g:?} handled by transition system via step config");
        }
    }
    let (pulse_zone, mantle_free) = chars
        .single()
        .map(|(tf, m)| {
            let z = tf.translation.z;
            let zone = (z > -6.15 && z < -4.9) || (z > 0.85 && z < 2.1);
            (zone, !m.active && m.cooldown <= 0.0)
        })
        .unwrap_or((false, false));
    over.0.move_dir = Vec2::Y;
    over.0.jump_pressed = pulse_zone && mantle_free;
    over.0.jump_held = false;
    over.0.roll_pressed = false;
    over.0.crouch_pressed = false;
    over.0.sprint_held = false;
    over.0.aim_held = false;
    over.0.look_delta = Vec2::ZERO;
}

fn mantle_shot_system(
    mut shot: ResMut<MantleShot>,
    m: Query<&AlsMantleState, With<AlsCharacter>>,
    cams: Query<Entity, With<Camera3d>>,
    mut commands: Commands,
) {
    if shot.path.is_none() {
        shot.path = std::env::var("ALS_SHOT_PATH").ok();
        if shot.path.is_some() {
            info!("[MGATE][SHOT] instrumentation armed");
        }
    }
    let Some(path) = shot.path.clone() else {
        return;
    };
    let Ok(m) = m.single() else {
        return;
    };
    if !shot.prev_active && m.active {
        shot.edge_at = Some(Instant::now());
        info!("[MGATE][SHOT] mantle begin detected, capture scheduled +0.45s");
    }
    shot.prev_active = m.active;

    if !shot.taken
        && let Some(edge) = shot.edge_at
        && Instant::now() >= edge + Duration::from_millis(450)
        && let Ok(cam) = cams.single()
    {
        commands.entity(cam).insert(Screenshot::primary_window());
        commands.entity(cam).observe(save_to_disk(path.clone()));
        info!("[MGATE][SHOT] captured mid-mantle -> {path}");
        shot.taken = true;
    }
}

fn mantle_watcher(
    time: Res<Time>,
    mut watch: ResMut<MantleWatch>,
    q: Query<
        (
            &Transform,
            &LinearVelocity,
            &LocomotionState,
            &AlsMantleState,
            Option<&AlsCharacterDebug>,
        ),
        With<AlsCharacter>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    let _ = time;
    let Ok((tf, vel, loco, m, debug)) = q.single() else {
        return;
    };
    let slot_now = watch.count.clamp(0, 1) as usize;
    watch.kinds[slot_now] = Some(m.kind);

    if !watch.prev_active && m.active && watch.count < 2 {
        let slot = watch.count.min(1) as usize;
        watch.targets[slot] = m.target.y;
        let trace: Option<als_core::mantle::MantleProbeTrace> =
            debug.and_then(|value| value.0.mantle_probe);
        info!(
            "[BCHAR18M][BEGIN {}] kind={:?} target_y={:.2} duration={:.6} start={:.3} rate={:.2} clip={:.3} warp=({:.3},{:.3}) trace=({:?},{:?},{:?},{:?})",
            slot + 1,
            m.kind,
            m.target.y,
            m.duration,
            m.start_offset,
            m.rate_scale,
            m.clip_time(),
            m.location_warp_amount(),
            m.rotation_warp_amount(),
            trace.and_then(|trace| trace.forward_start),
            trace.and_then(|trace| trace.forward_end),
            trace.and_then(|trace| trace.downward_start),
            trace.and_then(|trace| trace.downward_end),
        );
    }
    if let Some(slot) = watch.pending_slot {
        watch.settle_timer -= time.delta_secs();
        if loco == &LocomotionState::Grounded
            && (tf.translation.y - watch.targets[slot]).abs() <= 0.30
        {
            watch.grounded_seen[slot] = true;
        }
        if watch.settle_timer <= 0.0 || watch.grounded_seen[slot] {
            watch.landings[slot] = Some(tf.translation.y);
            info!(
                "[MGATE][SETTLE {}] y={:.2} loco={loco:?} grounded_ok={}",
                slot + 1,
                tf.translation.y,
                watch.grounded_seen[slot]
            );
            watch.pending_slot = None;
            watch.count += 1;
        }
        watch.prev_active = m.active;
        return;
    }
    if watch.prev_active && !m.active && watch.count < 2 {
        let slot = watch.count.min(1) as usize;
        info!("[MGATE][DONE {}] raw_y={:.2}", slot + 1, tf.translation.y);
        watch.pending_slot = Some(slot);
        watch.settle_timer = 1.0;
    }
    watch.prev_active = m.active;

    if watch.count == 2 && !watch.failed {
        watch.failed = false;
        for i in 0..2usize {
            match watch.landings[i] {
                None => {
                    error!("[MGATE][VERDICT] mantle {i} never completed");
                    watch.failed = true;
                }
                Some(y) => {
                    let on_top = y >= watch.targets[i] - 0.30;
                    if !(on_top && watch.grounded_seen[i]) {
                        error!(
                            "[MGATE][VERDICT] mantle {i} landing insufficient settle={:.2} target={:.2} grounded={}",
                            y, watch.targets[i], watch.grounded_seen[i]
                        );
                        watch.failed = true;
                    }
                }
            }
        }
        if !watch.failed {
            info!(
                "[MGATE][VERDICT] both mantles landed on ledge top y={:.2}/{:.2} -> PASS",
                watch.landings[0].unwrap_or(f32::NAN),
                watch.landings[1].unwrap_or(f32::NAN)
            );
        } else {
            exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
        }
        watch.count += 10;
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
        cam.translation = char_t.translation + Vec3::new(3.5, 2.8, -4.0);
        cam.look_at(char_t.translation + Vec3::Y * 0.9, Vec3::Y);
    }
}

fn fall_guard(q: Query<&Transform, With<AlsCharacter>>, mut exit: MessageWriter<AppExit>) {
    let Ok(char_t) = q.single() else {
        return;
    };
    if char_t.translation.y < -20.0 {
        error!("[MGATE] fell below floor -- aborting");
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
            &AlsMantleState,
        ),
        With<AlsCharacter>,
    >,
) {
    let t = time.elapsed_secs_f64();
    if (t as f32) - *last < 0.15 {
        return;
    }
    *last = t as f32;
    let Ok((tf, vel, loco, m)) = q.single() else {
        return;
    };
    info!(
        "[MGATE] t={t:.2} loco={loco:?} v={:.2} y={:.2} z={:.2} mantling={} mt={:.2}",
        Vec2::new(vel.x, vel.z).length(),
        tf.translation.y,
        tf.translation.z,
        m.active,
        m.time,
    );
}

fn auto_quit(mut quit: ResMut<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= quit.0 {
        warn!("[MGATE] wall-clock safety quit");
        exit.write(AppExit::Success);
    }
}
