use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::pose::Pose;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_core::character::{AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::input::{spawn_als_actions, AlsInputMap, AlsInputPlugin};
use als_core::motion::{AlsInputOverride, AlsMotionPlugin};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, RotationMode, Stance};
use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin};
use als_ext::AlsExtPlugin;

use bevy_als::anim_probe::AnimProbePlugin;
use bevy_als::character_assembly::AvatarGroundCompensationPlugin;
use bevy_als::direction_layer::DirectionLayerPlugin;
use bevy_als::motion_graph_link::{
    compute_motion_graph_inputs, write_motion_graph_pins,
};
use bevy_als::stance_layer::{AlsStanceBlend, StanceLayerPlugin};
use als_core::motion::AlsViewYaw;
use als_core::movement_direction::{AlsMovementDirectionState, AlsMovementDirection};

const CROUCH_AT_SECS: f64 = 1.5;
const STRAFE_AT_SECS: f64 = 9.6;
const WALK_AT_SECS: f64 = 3.0;
const STOP_WALK_AT_SECS: f64 = 5.0;
const STAND_AT_SECS: f64 = 6.5;
const BACKWARD_AT_SECS: f64 = 7.5;
const STOP_BACKWARD_AT_SECS: f64 = 9.0;
const TOTAL_SECS: f64 = 11.5;

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
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
        StanceLayerPlugin,
        DirectionLayerPlugin,
        AnimProbePlugin,
        AvatarGroundCompensationPlugin,
    ))
    .add_plugins(bevy_als::gait_layer::GaitLayerPlugin)
    .init_resource::<SpawnOnce>()
    .init_resource::<GateVerdict>()
    .init_resource::<ShotPlan>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(
        Update,
        (
            spawn_character_once,
            scripted_driver,
            direction_probe_driver,
            direction_probe_comparator,
            motion_pin_router,
            camera_follow,
            metrics_logger,
            screenshot_runner,
            auto_quit,
        ),
    );

    info!("[AP1GATE] stance gate armed, total {TOTAL_SECS}s");
    app.insert_resource(AutoQuit(Instant::now() + Duration::from_secs_f64(TOTAL_SECS)));
    app.run();
}

#[derive(Resource, Default)]
struct SpawnOnce(bool);

#[derive(Resource)]
struct AutoQuit(Instant);

#[derive(Resource, Default)]
struct ShotPlan {
    standing_taken: bool,
    crouch_taken: bool,
    backward_taken: bool,
}

#[derive(Resource, Default)]
struct GateVerdict {
    crouch_weight_seen: bool,
    stand_weight_returned: bool,
    stance_flipped_crouch: bool,
    stance_flipped_stand: bool,
    walk_while_crouched_seen: bool,
    direction_pin_responsive: bool,
    feet_grounded_crouch: bool,
    backward_direction_seen: bool,
    backward_facing_matches_view: bool,
    strafe_left_seen: bool,
    failed: bool,
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
        Collider::cuboid(40.0, 0.3, 40.0),
        Transform::from_xyz(0.0, -0.15, 0.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(40.0, 0.3, 40.0)))),
        MeshMaterial3d(ground_mat),
    ));

    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_xyz(0.0, 0.95, 0.0);

    let character = commands.spawn(bundle).id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());
    commands.entity(character).insert(AlsStateRequest::default());
    bevy_als::character_assembly::attach_avatar(
        &mut commands,
        &assets,
        character,
        "animated_scenes/motion_overlay.animscn.ron",
    );
    commands.entity(character).insert(AlsInputOverride::default());

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(2.6, 1.6, -2.6).looking_at(Vec3::new(0.0, 0.9, 0.0), Vec3::Y),
    ));

    let avatar_a = commands
        .spawn((
            ProbeDirectionA,
            AnimatedSceneHandle::new(assets.load("animated_scenes/probe_locomotion.animscn.ron")),
            Transform::from_xyz(3.0, 0.92, 3.0),
        ))
        .id();
    let avatar_b = commands
        .spawn((
            ProbeDirectionB,
            AnimatedSceneHandle::new(assets.load("animated_scenes/probe_locomotion.animscn.ron")),
            Transform::from_xyz(-3.0, 0.92, 3.0),
        ))
        .id();
    info!("[AP1GATE] direction probe avatars spawned a={avatar_a} b={avatar_b}");
    info!("[AP1GATE] character spawned");
}

#[derive(Component)]
struct ProbeDirectionA;

#[derive(Component)]
struct ProbeDirectionB;

fn direction_probe_driver(
    mut q: Query<
        (
            Entity,
            &AnimatedSceneInstance,
            Option<&ProbeDirectionA>,
            Option<&ProbeDirectionB>,
        ),
        Or<(With<ProbeDirectionA>, With<ProbeDirectionB>)>,
    >,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (entity, instance, a, b) in &q {
        let Ok(mut player) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        let dir = if a.is_some() {
            0.0f32
        } else if b.is_some() {
            180.0f32
        } else {
            continue;
        };
        player.set_input_data("speed", DataValue::from(1.75f32));
        player.set_input_data("play_rate", DataValue::from(1.0f32));
        player.set_input_data("stance_weight", DataValue::from(0.0f32));
        player.set_input_data("direction_angle", DataValue::from(dir));
        player.set_input_data("gait_move_weight", DataValue::from(1.0f32));
        player.set_input_data("gait_run_weight", DataValue::from(0.0f32));
        player.set_input_data("gait_sprint_weight", DataValue::from(0.0f32));
        let _ = entity;
    }
}

fn pose_delta(a: &Pose, b: &Pose) -> (f32, usize) {
    let mut max_d = 0.0f32;
    let mut n = 0usize;
    for (bone_id, idx_a) in &a.paths {
        if let Some(idx_b) = b.paths.get(bone_id) {
            if let (Some(ta), Some(tb)) = (&a.bones[*idx_a].translation, &b.bones[*idx_b].translation)
            {
                let d = ta.distance(*tb);
                if d > max_d {
                    max_d = d;
                }
                n += 1;
            }
        }
    }
    (max_d, n)
}

fn direction_probe_comparator(
    time: Res<Time>,
    mut last_log: Local<f32>,
    q: Query<
        (
            Entity,
            &AnimatedSceneInstance,
            Option<&ProbeDirectionA>,
            Option<&ProbeDirectionB>,
        ),
        Or<(With<ProbeDirectionA>, With<ProbeDirectionB>)>,
    >,
    players: Query<&AnimationGraphPlayer>,
    mut verdict: ResMut<GateVerdict>,
) {
    let t = time.elapsed_secs();
    let mut pose_a: Option<(Entity, Pose)> = None;
    let mut pose_b: Option<(Entity, Pose)> = None;
    for (entity, instance, a, _b) in &q {
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        if let Some(p) = player.get_default_output_pose() {
            let p = p.clone();
            if a.is_some() {
                pose_a = Some((entity, p));
            } else {
                pose_b = Some((entity, p));
            }
        }
    }
    if let (Some((ea, pa)), Some((eb, pb))) = (pose_a, pose_b) {
        let (max_d, n) = pose_delta(&pa, &pb);
        if t - *last_log > 0.5 {
            *last_log = t;
            info!(
                "[AP1GATE][PROBE_AB] t={t:.2} a={ea} b={eb} max_bone_delta={max_d:.4} bones={n} ts_a={:.3} ts_b={:.3}",
                pa.timestamp, pb.timestamp
            );
        }
        if t > 1.0 && max_d > 0.05 {
            verdict.direction_pin_responsive = true;
        }
    }
}

fn scripted_driver(
    time: Res<Time>,
    mut latches: Local<(bool, bool, bool, bool, bool)>,
    mut q: Query<
        (
            &mut AlsStateRequest,
            &mut AlsInputOverride,
            &Stance,
        ),
        With<AlsCharacter>,
    >,
) {
    let t = time.elapsed_secs_f64();
    let Ok((mut request, mut ovr, _stance)) = q.single_mut() else {
        return;
    };
    if !latches.0 && t >= CROUCH_AT_SECS {
        request.stance = Some(Stance::Crouching);
        latches.0 = true;
        info!("[AP1GATE] request crouching");
    }
    if !latches.1 && t >= STAND_AT_SECS {
        request.stance = Some(Stance::Standing);
        latches.1 = true;
        info!("[AP1GATE] request standing");
    }
    if !latches.2 && t >= BACKWARD_AT_SECS {
        request.rotation_mode = Some(RotationMode::ViewDirection);
        latches.2 = true;
        info!("[AP1GATE] request view-direction mode for backward phase");
    }
    if !latches.3 && t >= STOP_BACKWARD_AT_SECS {
        request.rotation_mode = Some(RotationMode::VelocityDirection);
        latches.3 = true;
        info!("[AP1GATE] restore velocity-direction mode");
    }
    if !latches.4 && t >= STRAFE_AT_SECS {
        request.rotation_mode = Some(RotationMode::ViewDirection);
        latches.4 = true;
        info!("[AP1GATE] request view-direction mode for strafe phase");
    }
    let walking = t >= WALK_AT_SECS && t < STOP_WALK_AT_SECS;
    let backward = t >= BACKWARD_AT_SECS && t < STOP_BACKWARD_AT_SECS;
    let strafe = t >= STRAFE_AT_SECS;
    ovr.0.move_dir = if walking {
        Vec2::new(0.0, 1.0)
    } else if backward {
        Vec2::new(0.0, -1.0)
    } else if strafe {
        Vec2::new(-1.0, 0.0)
    } else {
        Vec2::ZERO
    };
}

fn motion_pin_router(
    characters: Query<(&Stance, &Gait, &LinearVelocity), With<AlsCharacter>>,
    avatars: Query<(&ChildOf, Option<&AlsStanceBlend>, &AnimatedSceneInstance)>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (parent, blend, instance) in &avatars {
        let Ok((stance, gait, vel)) = characters.get(parent.parent()) else {
            continue;
        };
        let Ok(mut player) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        let inputs = compute_motion_graph_inputs(
            &AlsMovementParams::default(),
            *stance,
            *gait,
            Vec2::new(vel.x, vel.z).length(),
        );
        write_motion_graph_pins(&mut player, &inputs);
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
        cam.translation = char_t.translation + Vec3::new(2.6, 1.6, -2.6);
        cam.look_at(char_t.translation + Vec3::Y * 0.8, Vec3::Y);
    }
}

fn metrics_logger(
    time: Res<Time>,
    mut last_log: Local<f64>,
    mut verdict: ResMut<GateVerdict>,
    characters: Query<
        (
            &Stance,
            &LinearVelocity,
            &Transform,
            &AlsViewYaw,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<
        (
            &AlsStanceBlend,
            Option<&AlsMovementDirectionState>,
            &GlobalTransform,
        ),
        Without<AlsCharacter>,
    >,
) {
    let t = time.elapsed_secs_f64();
    let Ok((stance, vel, tf, view_yaw)) = characters.single() else {
        return;
    };
    let Ok((blend, dir_state, avatar_gt)) = avatars.single() else {
        return;
    };
    let weight = blend.weight;
    let mesh_y = avatar_gt.translation().y;
    let hspeed = Vec2::new(vel.x, vel.z).length();
    let facing_yaw = f32::atan2(tf.forward().x, -tf.forward().z).to_degrees();
    let vel_yaw = f32::atan2(vel.x, -vel.z).to_degrees();
    let dir_angle = dir_state.map(|s| s.angle).unwrap_or(0.0);
    let dir_name = dir_state
        .map(|s| format!("{:?}", s.direction))
        .unwrap_or_else(|| "None".into());
    if t - *last_log > 0.25 {
        *last_log = t;
        info!(
            "[AP1GATE] t={t:.2} stance={stance:?} w={weight:.3} hspd={hspeed:.2} face={facing_yaw:.0} velyaw={vel_yaw:.0} view={:.0} dir_angle={dir_angle:.0} dir={dir_name} mesh_y={mesh_y:.3}",
            view_yaw.0,
        );
    }
    if matches!(stance, Stance::Crouching) {
        verdict.stance_flipped_crouch = true;
    }
    if verdict.stance_flipped_crouch && matches!(stance, Stance::Standing) && t > STAND_AT_SECS {
        verdict.stance_flipped_stand = true;
    }
    if t > WALK_AT_SECS && t < STOP_WALK_AT_SECS && matches!(stance, Stance::Crouching) {
        if hspeed > 0.8 {
            verdict.walk_while_crouched_seen = true;
        }
        if weight >= 0.95 {
            verdict.crouch_weight_seen = true;
        }
        if (mesh_y - bevy_als::character_assembly::AVATAR_MESH_ROOT_WORLD_Y).abs() < 0.05 {
            verdict.feet_grounded_crouch = true;
        }
    }
    if t > STAND_AT_SECS + 1.5 && weight <= 0.05 {
        verdict.stand_weight_returned = true;
    }
    if t > BACKWARD_AT_SECS + 0.4 && t < STOP_BACKWARD_AT_SECS && hspeed > 0.8 {
        if dir_angle.abs() >= 150.0 {
            verdict.backward_direction_seen = true;
        }
        let err = (facing_yaw - view_yaw.0).abs();
        let err = err.min(360.0 - err);
        if err < 15.0 {
            verdict.backward_facing_matches_view = true;
        }
    }
    if t > STRAFE_AT_SECS + 0.4 && hspeed > 0.8 {
        let err = (dir_angle - (-90.0)).abs();
        let err = err.min(360.0 - err);
        if err < 30.0 {
            verdict.strafe_left_seen = true;
        }
    }
}

#[derive(Component)]
struct ThrowawayShotCamera;

fn screenshot_runner(
    time: Res<Time>,
    mut plan: ResMut<ShotPlan>,
    mut commands: Commands,
    cameras: Query<&Transform, (With<Camera3d>, Without<ThrowawayShotCamera>)>,
) {
    let t = time.elapsed_secs_f64();
    if plan.standing_taken && plan.crouch_taken && plan.backward_taken {
        return;
    }
    let due = if !plan.standing_taken && t >= 1.0 {
        Some(std::path::PathBuf::from("docs/ap1-shots/standing.png"))
    } else if !plan.crouch_taken && t >= 4.5 {
        Some(std::path::PathBuf::from("docs/ap1-shots/crouching.png"))
    } else if !plan.backward_taken && t >= 8.4 {
        Some(std::path::PathBuf::from("docs/ap1-shots/backward.png"))
    } else {
        None
    };
    let Some(path) = due else {
        return;
    };
    let Ok(view) = cameras.single() else {
        return;
    };
    if t < 2.0 {
        plan.standing_taken = true;
    } else if t < 6.0 {
        plan.crouch_taken = true;
    } else {
        plan.backward_taken = true;
    }
    commands
        .spawn((
            ThrowawayShotCamera,
            Camera3d::default(),
            *view,
            Screenshot::primary_window(),
        ))
        .observe(save_to_disk(path));
}

fn auto_quit(timer: Res<AutoQuit>, verdict: Res<GateVerdict>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() < timer.0 {
        return;
    }
    let pass = verdict.crouch_weight_seen
        && verdict.stand_weight_returned
        && verdict.stance_flipped_crouch
        && verdict.stance_flipped_stand
        && verdict.walk_while_crouched_seen
        && verdict.backward_direction_seen
        && verdict.backward_facing_matches_view
        && verdict.direction_pin_responsive
        && verdict.feet_grounded_crouch
        && verdict.strafe_left_seen
        && !verdict.failed;
    info!(
        "[AP1GATE] VERDICT {} crouch_w={} stand_back={} flip_down={} flip_up={} crouch_walk={} bwd_dir={} bwd_face_ok={} pin_resp={} feet_gnd={} strafe_l={}",
        if pass { "PASS" } else { "FAIL" },
        verdict.crouch_weight_seen,
        verdict.stand_weight_returned,
        verdict.stance_flipped_crouch,
        verdict.stance_flipped_stand,
        verdict.walk_while_crouched_seen,
        verdict.backward_direction_seen,
        verdict.backward_facing_matches_view,
        verdict.direction_pin_responsive,
        verdict.feet_grounded_crouch,
        verdict.strafe_left_seen,
    );
    if pass {
        exit.write(AppExit::Success);
    } else {
        exit.write(AppExit::Error(std::num::NonZeroU8::new(1).unwrap()));
    }
}
