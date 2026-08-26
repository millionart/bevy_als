use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::ragdoll::definition::BodyLabel;

use als_core::character::AlsCharacter;
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::motion::AlsInputOverride;
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState};
use als_core::transition::AlsStateRequest;

use bevy_als::playable_character::{
    AlsCharacterAvatar, AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};

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
    .init_resource::<RagdollWatch>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(
        Update,
        (
            spawn_character_once,
            auto_demo,
            ragdoll_watcher,
            camera_follow,
            fall_guard,
            auto_quit,
        ),
    );

    let secs: f64 = std::env::var("ALS_BOOT_AUTO_QUIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30.0);
    info!("[RGATE] auto quit armed in {secs:.1}s");
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
struct RagdollWatch {
    ragdoll_entered: bool,
    pelvis_peak_y: f32,
    pelvis_rest_y: f32,
    settle_seen: bool,
    recovered: bool,
    recovery_y: f32,
    nan_bodies: u32,
    failed: bool,
    verdict_done: bool,
}

struct Step {
    t0: f64,
    dur: f64,
    label: &'static str,
    move_dir: Vec2,
    gait: Option<Gait>,
    trigger_ragdoll: bool,
    trigger_recover: bool,
}

fn steps() -> [Step; 5] {
    [
        Step {
            t0: 0.5,
            dur: 1.0,
            label: "idle",
            move_dir: Vec2::ZERO,
            gait: None,
            trigger_ragdoll: false,
            trigger_recover: false,
        },
        Step {
            t0: 1.5,
            dur: 1.6,
            label: "sprint",
            move_dir: Vec2::NEG_Y,
            gait: Some(Gait::Sprinting),
            trigger_ragdoll: false,
            trigger_recover: false,
        },
        Step {
            t0: 3.1,
            dur: 3.2,
            label: "ragdoll (mid-run)",
            move_dir: Vec2::ZERO,
            gait: None,
            trigger_ragdoll: true,
            trigger_recover: false,
        },
        Step {
            t0: 6.3,
            dur: 2.4,
            label: "recover to grounded idle",
            move_dir: Vec2::ZERO,
            gait: None,
            trigger_ragdoll: false,
            trigger_recover: true,
        },
        Step {
            t0: 8.7,
            dur: 1.3,
            label: "post-recovery idle",
            move_dir: Vec2::ZERO,
            gait: None,
            trigger_ragdoll: false,
            trigger_recover: false,
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
    let ground_mat = materials.add(Color::srgb(0.24, 0.27, 0.24));
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(60.0, 0.3, 60.0),
        Transform::from_xyz(0.0, -0.15, 0.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(60.0, 0.3, 60.0)))),
        MeshMaterial3d(ground_mat),
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
        info!("[RGATE] scripted demo armed");
    }

    commands.spawn((Camera3d::default(), Transform::from_xyz(4.0, 3.0, -5.0)));
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
    if t >= last.t0 + last.dur + 0.4 {
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
        info!("[RGATE][STEP {}] {}", idx, s.label);
        if s.trigger_ragdoll {
            req.locomotion_state = Some(LocomotionState::Ragdolling);
            info!("[RGATE] request Ragdolling mid-run");
        }
        if s.trigger_recover {
            req.locomotion_state = Some(LocomotionState::Grounded);
            info!("[RGATE] request Grounded recovery");
        }
        if let Some(g) = s.gait {
            req.gait = Some(g);
        }
    } else {
        let s = &all[idx];
        if s.trigger_ragdoll {
            req.locomotion_state = Some(LocomotionState::Ragdolling);
        }
    }
    over.0.move_dir = all[idx].move_dir;
    over.0.jump_pressed = false;
    over.0.jump_held = false;
    over.0.roll_pressed = false;
    over.0.crouch_pressed = false;
    over.0.sprint_held = matches!(all[idx].gait, Some(Gait::Sprinting));
    over.0.aim_held = false;
    over.0.look_delta = Vec2::ZERO;
}

#[allow(clippy::type_complexity)]
fn ragdoll_watcher(
    time: Res<Time>,
    mut watch: ResMut<RagdollWatch>,
    mut q: Query<
        (
            &Transform,
            &LinearVelocity,
            &LocomotionState,
            &mut AlsStateRequest,
        ),
        With<AlsCharacter>,
    >,
    instances: Query<&AnimatedSceneInstance, With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    labels: Query<&BodyLabel>,
    positions: Query<(&Position, &LinearVelocity)>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((tf, vel, loco, mut req)) = q.single_mut() else {
        return;
    };

    if *loco == LocomotionState::Ragdolling && !watch.ragdoll_entered {
        watch.ragdoll_entered = true;
        watch.pelvis_rest_y = 0.93;
        info!("[RGATE][PHASE] ragdoll active -- physics takes over");
        req.locomotion_state = None;
    }

    if watch.ragdoll_entered && !watch.settle_seen {
        for instance in instances.iter() {
            if let Ok(player) = players.get(instance.player_entity())
                && let Some(spawned) = player.get_spawned_ragdoll()
            {
                let mut lowest = f32::INFINITY;
                let mut peak = 0.0f32;
                for body_entity in spawned.bodies.values() {
                    if let Ok((pos, bvel)) = positions.get(*body_entity) {
                        let y = pos.y;
                        if !y.is_finite() || pos.x.is_nan() {
                            watch.nan_bodies += 1;
                        }
                        lowest = lowest.min(y);
                        peak = peak.max(bvel.length());
                    }
                }
                warn!(
                    "[RGATE][PROBE] char_y={:.2} lowest={:.2} max_bvel={:.2}",
                    tf.translation.y, lowest, peak
                );
                if lowest < 0.55 && peak < 0.35 {
                    watch.settle_seen = true;
                    info!(
                        "[RGATE][SETTLE] bodies at rest (lowest={lowest:.2}) after falling from peak pelvis {:.2}",
                        watch.pelvis_peak_y
                    );
                }
            }
        }
    }

    if watch.settle_seen
        && !watch.recovered
        && *loco == LocomotionState::Grounded
        && tf.translation.y > 0.7
        && tf.translation.y < 1.1
    {
        watch.recovered = true;
        watch.recovery_y = tf.translation.y;
        info!(
            "[RGATE][RECOVERED] back on feet y={:.2} v={:.2}",
            tf.translation.y,
            Vec2::new(vel.x, vel.z).length()
        );
    }

    if watch.recovered && !watch.verdict_done {
        let elapsed_since = time.elapsed_secs_f64() % 3600.0;
        let _ = elapsed_since;
        let ok_settle = watch.settle_seen;
        let ok_recover = watch.recovered;
        let ok_no_nan = watch.nan_bodies == 0;
        info!(
            "[RGATE][VERDICT] settle={ok_settle} recover={ok_recover} nan_bodies={} -> {}",
            watch.nan_bodies,
            if ok_settle && ok_recover && ok_no_nan {
                "PASS"
            } else {
                "FAIL"
            }
        );
        watch.verdict_done = true;
        if !(ok_settle && ok_recover && ok_no_nan) {
            watch.failed = true;
            exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
        } else {
            exit.write(AppExit::Success);
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
        error!("[RGATE] fell below floor -- aborting");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
    }
}

fn auto_quit(mut quit: ResMut<AutoQuit>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= quit.0 {
        warn!("[RGATE] wall-clock safety quit");
        exit.write(AppExit::Success);
    }
}
