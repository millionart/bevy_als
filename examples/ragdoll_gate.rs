use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::ragdoll::definition::BodyLabel;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::{TnuaControllerPlugin, TnuaToggle};
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_core::character::{AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::input::{AlsInputMap, AlsInputPlugin, spawn_als_actions};
use als_core::motion::{AlsInputOverride, AlsMotionPlugin, AlsViewYaw};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, Stance};
use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin, state_transition_system};
use als_ext::{
    AlsExtPlugin, RagdollCollisionHooks, RagdollCollisionMember, RagdollPhysicsProfileApplied,
};

use bevy_als::motion_graph_link::{compute_motion_graph_inputs, write_motion_graph_pins};
use bevy_als::pa_als::PaAlsPhysicsPlugin;
use bevy_als::ragdoll_link::{
    AlsRagdollGroundFollow, AlsRagdollJointDrive, AlsRagdollLinkPlugin, ragdoll_exit_route_system,
};
use bevy_als::stop_link::AlsStopLinkPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        PhysicsPlugins::new(FixedPostUpdate).with_collision_hooks::<RagdollCollisionHooks>(),
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
        PaAlsPhysicsPlugin,
        AlsRagdollLinkPlugin,
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
            auto_demo.before(ragdoll_exit_route_system),
            ragdoll_watcher.after(state_transition_system),
            graph_router,
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
    kinematic_logged: bool,
    kinematic_valid: bool,
    profile_valid: bool,
    profile_logged: bool,
    profile_probe_logged: bool,
    initial_physics_captured: bool,
    initial_layers: Option<CollisionLayers>,
    initial_body_mode: Option<RigidBody>,
    initial_tnua_toggle: Option<TnuaToggle>,
    ragdoll_entered: bool,
    handoff_valid: bool,
    follow_valid: bool,
    handoff_logged: bool,
    max_follow_error: f32,
    settle_seen: bool,
    recovered: bool,
    recovery_physics_valid: bool,
    ground_exit_velocity: Option<f32>,
    joint_drive_startup: Option<(f32, f32)>,
    joint_drive_full: bool,
    joint_drive_expected_targets: u32,
    joint_drive_peak_targets: u32,
    joint_drive_peak_speed_amount: f32,
    joint_drive_peak_stiffness: f32,
    recovery_probe_count: u8,
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
            label: "recover through source route",
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
        Collider::cuboid(200.0, 0.3, 200.0),
        Transform::from_xyz(0.0, -0.15, 0.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(200.0, 0.3, 200.0)))),
        MeshMaterial3d(ground_mat),
    ));

    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_xyz(0.0, 0.95, 0.0)
        .with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
    let character = commands.spawn(bundle).id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());

    let _avatar = bevy_als::character_assembly::attach_avatar(
        &mut commands,
        &assets,
        character,
        "animated_scenes/als_ragdoll.animscn.ron",
    );

    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() {
        commands.entity(character).insert((
            AlsInputOverride::default(),
            AlsViewYaw(std::f32::consts::PI),
        ));
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
            req.ragdoll_exit = true;
            info!("[RGATE] request ragdoll recovery");
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
    mut watch: ResMut<RagdollWatch>,
    mut q: Query<
        (
            &Transform,
            &LinearVelocity,
            &LocomotionState,
            &mut AlsStateRequest,
            Option<&CollisionLayers>,
            &RigidBody,
            Option<&TnuaToggle>,
            Option<&AlsRagdollGroundFollow>,
            Option<&AlsRagdollJointDrive>,
        ),
        With<AlsCharacter>,
    >,
    instances: Query<&AnimatedSceneInstance>,
    players: Query<&AnimationGraphPlayer>,
    labels: Query<&BodyLabel>,
    profiles: Query<&RagdollPhysicsProfileApplied>,
    collision_members: Query<&RagdollCollisionMember>,
    joints: Query<&SphericalJoint>,
    body_transforms: Query<(&Position, &Rotation)>,
    positions: Query<(&Position, &LinearVelocity)>,
    modes: Query<&RigidBody>,
    layers: Query<&CollisionLayers>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((
        tf,
        vel,
        loco,
        mut req,
        character_layers,
        character_mode,
        tnua_toggle,
        ground_follow,
        joint_drive,
    )) = q.single_mut()
    else {
        return;
    };

    if *loco == LocomotionState::Ragdolling && !watch.ragdoll_entered {
        watch.ragdoll_entered = true;
        info!("[RGATE][PHASE] ragdoll active -- physics takes over");
        req.locomotion_state = None;
    }

    if !watch.kinematic_logged && !watch.ragdoll_entered {
        for instance in instances.iter() {
            if let Ok(player) = players.get(instance.player_entity())
                && let Some(spawned) = player.get_spawned_ragdoll()
            {
                let mut lowest = f32::INFINITY;
                let mut highest = f32::NEG_INFINITY;
                let mut kinematic = 0usize;
                let mut dynamic = 0usize;
                let mut pelvis_y = None;
                for body_entity in spawned.bodies.values() {
                    if let Ok((pos, _)) = positions.get(*body_entity) {
                        lowest = lowest.min(pos.y);
                        highest = highest.max(pos.y);
                        if labels
                            .get(*body_entity)
                            .is_ok_and(|label| label.0 == "pelvis")
                        {
                            pelvis_y = Some(pos.y);
                        }
                    }
                    match modes.get(*body_entity) {
                        Ok(RigidBody::Kinematic) => kinematic += 1,
                        Ok(RigidBody::Dynamic) => dynamic += 1,
                        _ => {}
                    }
                }
                let profile_counts_valid =
                    profiles.get(instance.player_entity()).is_ok_and(|profile| {
                        profile.body_count == 21
                            && profile.collider_count == 22
                            && profile.joint_count == 20
                            && profile.collision_disabled_pair_count == 49
                    }) && spawned.colliders.values().all(|collider| {
                        collision_members.get(*collider).is_ok_and(|member| {
                            member.group == spawned.root && member.disabled_pairs.len() == 49
                        })
                    });
                let anchor_error = spawned_joint_anchor_error(spawned, &joints, &body_transforms)
                    .unwrap_or(f32::INFINITY);
                let profile_valid = profile_counts_valid && anchor_error <= 1e-4;
                if !watch.profile_probe_logged
                    && let Ok(profile) = profiles.get(instance.player_entity())
                {
                    info!(
                        "[RGATE][PROFILE-PROBE] bodies={} colliders={} joints={} collision_disabled_pairs={} applied_anchor_error={:.6} current_anchor_error={anchor_error:.6}",
                        profile.body_count,
                        profile.collider_count,
                        profile.joint_count,
                        profile.collision_disabled_pair_count,
                        profile.max_anchor_error_m
                    );
                    watch.profile_probe_logged = true;
                }
                if !watch.profile_logged && profile_valid {
                    info!(
                        "[RGATE][PROFILE] bodies=21 colliders=22 joints=20 collision_disabled_pairs=49 max_anchor_error_m={anchor_error:.6} -> PASS"
                    );
                    watch.profile_logged = true;
                }
                watch.profile_valid |= profile_valid;
                if spawned.bodies.len() == 21 && profile_valid {
                    watch.initial_physics_captured = true;
                    watch.initial_layers = character_layers.copied();
                    watch.initial_body_mode = Some(*character_mode);
                    watch.initial_tnua_toggle = tnua_toggle.copied();
                    watch.kinematic_valid = kinematic == 21
                        && dynamic == 0
                        && lowest.is_finite()
                        && highest.is_finite()
                        && lowest > -0.05
                        && highest < 2.1
                        && pelvis_y.is_some_and(|y| (0.7..1.2).contains(&y));
                    info!(
                        "[RGATE][KINEMATIC] bodies={} kinematic={} dynamic={} lowest={lowest:.3} highest={highest:.3} pelvis={:.3} valid={} profile_valid={} anchor_error={anchor_error:.6} capsule_layers={:?} capsule_body={:?} capsule_tnua={:?}",
                        spawned.bodies.len(),
                        kinematic,
                        dynamic,
                        pelvis_y.unwrap_or(f32::NAN),
                        watch.kinematic_valid,
                        profile_valid,
                        watch.initial_layers,
                        watch.initial_body_mode,
                        watch.initial_tnua_toggle
                    );
                    watch.kinematic_logged = true;
                }
            }
        }
    }

    if watch.ragdoll_entered {
        for instance in instances.iter() {
            if let Ok(player) = players.get(instance.player_entity())
                && let Some(spawned) = player.get_spawned_ragdoll()
            {
                let mut dynamic = 0usize;
                let mut root_kinematic = false;
                for body in spawned.bodies.values() {
                    let is_root = labels.get(*body).is_ok_and(|label| label.0 == "root");
                    if is_root {
                        root_kinematic = modes
                            .get(*body)
                            .is_ok_and(|mode| *mode == RigidBody::Kinematic);
                    } else if modes
                        .get(*body)
                        .is_ok_and(|mode| *mode == RigidBody::Dynamic)
                    {
                        dynamic += 1;
                    }
                }
                let bodies_dynamic = dynamic == 20 && root_kinematic;
                let colliders_active = spawned.colliders.values().all(|collider| {
                    layers
                        .get(*collider)
                        .is_ok_and(|layer| *layer != CollisionLayers::NONE)
                });
                let capsule_disabled = character_layers
                    .is_some_and(|layer| *layer == CollisionLayers::NONE)
                    && *character_mode == RigidBody::Kinematic
                    && tnua_toggle.is_some_and(|toggle| *toggle == TnuaToggle::Disabled);
                watch.handoff_valid |=
                    bodies_dynamic && colliders_active && capsule_disabled && watch.profile_valid;
                watch.joint_drive_expected_targets = watch.joint_drive_expected_targets.max(19);
                if let Some(follow) = ground_follow {
                    let error = tf.translation.distance(follow.target);
                    watch.max_follow_error = watch.max_follow_error.max(error);
                    watch.follow_valid |= follow.grounded && error <= 0.002;
                    if !watch.handoff_logged && watch.handoff_valid && watch.follow_valid {
                        info!(
                            "[RGATE][HANDOFF] dynamic_bodies={dynamic} root_kinematic={root_kinematic} colliders_active={colliders_active} capsule_disabled={capsule_disabled} profile_valid={} grounded={} target_y={:.3} error={error:.6} -> PASS",
                            watch.profile_valid, follow.grounded, follow.target.y
                        );
                        watch.handoff_logged = true;
                    }
                }
            }
        }
    }

    if *loco == LocomotionState::Ragdolling {
        if let Some(drive) = joint_drive {
            if watch.joint_drive_startup.is_none() {
                watch.joint_drive_startup = Some((drive.speed_amount, drive.angular_stiffness));
                info!(
                    "[RGATE][DRIVE] startup_speed_amount={:.3} startup_stiffness={:.1}",
                    drive.speed_amount, drive.angular_stiffness
                );
            }
            watch.joint_drive_peak_targets =
                watch.joint_drive_peak_targets.max(drive.target_joint_count);
            watch.joint_drive_peak_speed_amount =
                watch.joint_drive_peak_speed_amount.max(drive.speed_amount);
            watch.joint_drive_peak_stiffness = watch
                .joint_drive_peak_stiffness
                .max(drive.angular_stiffness);
            if !watch.joint_drive_full
                && drive.angular_stiffness >= 49.999
                && drive.angular_damping >= 0.999
                && drive.target_joint_count >= watch.joint_drive_expected_targets
            {
                watch.joint_drive_full = true;
                info!(
                    "[RGATE][DRIVE] full_speed_amount={:.3} stiffness={:.1} targets={} driven={}",
                    drive.speed_amount,
                    drive.angular_stiffness,
                    drive.target_joint_count,
                    drive.driven_joint_count
                );
            }
        }
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
                    info!("[RGATE][SETTLE] bodies at rest (lowest={lowest:.2})");
                }
            }
        }
    }

    let recovery_physics_restored = watch.initial_physics_captured
        && watch.initial_layers == character_layers.copied()
        && watch.initial_body_mode == Some(*character_mode)
        && watch.initial_tnua_toggle == tnua_toggle.copied();

    if watch.settle_seen
        && !watch.recovered
        && *loco == LocomotionState::Grounded
        && tf.translation.y > 0.7
        && tf.translation.y < 1.1
        && recovery_physics_restored
        && vel.0.length() <= 0.1
    {
        watch.recovered = true;
        watch.recovery_physics_valid = true;
        watch.ground_exit_velocity = Some(vel.0.length());
        info!(
            "[RGATE][RECOVERED] back on feet y={:.2} v={:.2} physics_restored={}",
            tf.translation.y,
            Vec2::new(vel.x, vel.z).length(),
            watch.recovery_physics_valid
        );
    }

    if watch.ragdoll_entered
        && *loco == LocomotionState::Grounded
        && !watch.recovered
        && watch.recovery_probe_count < 3
    {
        info!(
            "[RGATE][RECOVERY_PROBE] y={:.3} body={:?} layers={:?} tnua={:?} restored={recovery_physics_restored}",
            tf.translation.y, character_mode, character_layers, tnua_toggle
        );
        watch.recovery_probe_count += 1;
    }

    if watch.recovered && !watch.verdict_done {
        let ok_kinematic = watch.kinematic_valid;
        let ok_profile = watch.profile_valid;
        let ok_handoff = watch.handoff_valid;
        let ok_follow = watch.follow_valid;
        let ok_settle = watch.settle_seen;
        let ok_recover = watch.recovered;
        let ok_recovery_physics = watch.recovery_physics_valid;
        let ok_ground_exit_velocity = watch
            .ground_exit_velocity
            .is_some_and(|velocity| velocity <= 0.1);
        let ok_joint_drive = watch.joint_drive_startup.is_some()
            && watch.joint_drive_full
            && watch.joint_drive_expected_targets > 0
            && watch.joint_drive_peak_targets >= watch.joint_drive_expected_targets;
        let ok_no_nan = watch.nan_bodies == 0;
        info!(
            "[RGATE][VERDICT] kinematic={ok_kinematic} profile={ok_profile} handoff={ok_handoff} follow={ok_follow} settle={ok_settle} recover={ok_recover} recovery_physics={ok_recovery_physics} ground_exit_velocity={:?} drive_startup={:?} drive_full={} drive_targets={}/{} drive_peak=({:.3},{:.1}) nan_bodies={} max_follow_error={:.6} -> {}",
            watch.ground_exit_velocity,
            watch.joint_drive_startup,
            watch.joint_drive_full,
            watch.joint_drive_peak_targets,
            watch.joint_drive_expected_targets,
            watch.joint_drive_peak_speed_amount,
            watch.joint_drive_peak_stiffness,
            watch.nan_bodies,
            watch.max_follow_error,
            if ok_kinematic
                && ok_profile
                && ok_handoff
                && ok_follow
                && ok_settle
                && ok_recover
                && ok_recovery_physics
                && ok_ground_exit_velocity
                && ok_joint_drive
                && ok_no_nan
            {
                "PASS"
            } else {
                "FAIL"
            }
        );
        watch.verdict_done = true;
        if !(ok_kinematic
            && ok_profile
            && ok_handoff
            && ok_follow
            && ok_settle
            && ok_recover
            && ok_recovery_physics
            && ok_ground_exit_velocity
            && ok_joint_drive
            && ok_no_nan)
        {
            watch.failed = true;
            exit.write(AppExit::Error(std::num::NonZeroU8::new(6).unwrap()));
        } else {
            exit.write(AppExit::Success);
        }
    }
}

fn spawned_joint_anchor_error(
    spawned: &bevy_animation_graph::core::ragdoll::spawning::SpawnedRagdoll,
    joints: &Query<&SphericalJoint>,
    body_transforms: &Query<(&Position, &Rotation)>,
) -> Option<f32> {
    let mut maximum = 0.0_f32;
    for joint_entity in spawned.joints.values() {
        let joint = joints.get(*joint_entity).ok()?;
        let (frame_1, frame_2) = (joint.local_frame1()?, joint.local_frame2()?);
        let [(position_1, rotation_1), (position_2, rotation_2)] =
            body_transforms.get_many([joint.body1, joint.body2]).ok()?;
        let anchor_1 = position_1.0 + rotation_1.0 * Vec3::from(frame_1.translation);
        let anchor_2 = position_2.0 + rotation_2.0 * Vec3::from(frame_2.translation);
        maximum = maximum.max(anchor_1.distance(anchor_2));
    }
    Some(maximum)
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
        error!("[RGATE] fell below floor -- aborting");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
    }
}

fn auto_quit(quit: Res<AutoQuit>, watch: Res<RagdollWatch>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= quit.0 {
        if watch.verdict_done {
            exit.write(AppExit::Success);
        } else {
            error!("[RGATE] wall-clock safety quit before verdict");
            exit.write(AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));
        }
    }
}
