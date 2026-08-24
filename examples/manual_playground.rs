use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_camera::{AlsCameraPlugin, AlsCameraRig, AlsCameraTarget};
use als_core::air::AlsAirState;
use als_core::character::{AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::foot_ik::AlsFootProbePlugin;
use als_core::input::{spawn_als_actions, AlsInputCommand, AlsInputMap, AlsInputPlugin};
use als_core::input_request::AlsInputRequestPlugin;
use als_core::aim::AlsAimPlugin;
use als_core::motion::{AlsMotionPlugin, AlsViewYaw};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, Stance};
use als_core::transition::AlsStateTransitionPlugin;
use als_core::view::{clamp_pitch, AlsViewPitch};
use als_ext::{AlsExtPlugin, PlaygroundBoxCollider, PlaygroundPlugin, PlaygroundSpawnPoint};
use bevy_als::debug_draw::AlsDebugDrawPlugin;
use bevy_als::foot_ik_apply::FootIkApplyPlugin;
use bevy_als::mantle_drive::AlsMantleDrivePlugin;
use bevy_als::motion_graph_link::{
    compute_motion_graph_inputs, write_air_pins, write_motion_graph_pins,
};
use bevy_als::overlay_layer::OverlayLayerPlugin;
use bevy_als::ragdoll_link::AlsRagdollLinkPlugin;
use bevy_als::roll_drive::AlsRollDrivePlugin;
use bevy_als::show_debug_hud::AlsShowDebugHudPlugin;
use bevy_als::stop_link::AlsStopLinkPlugin;
use bevy_als::view_layer::ViewLayerPlugin;

const LOOK_RADIANS_PER_COUNT: f32 = 0.0025;

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
        AlsInputRequestPlugin,
        AlsAimPlugin,
        AlsStateTransitionPlugin,
        AlsCharacterPlugin,
        AlsMotionPlugin,
        als_core::air::AlsAirPlugin,
    ));
    app.add_plugins((
        AlsFootProbePlugin,
        FootIkApplyPlugin,
        OverlayLayerPlugin,
        ViewLayerPlugin,
        AlsRollDrivePlugin,
        AlsStopLinkPlugin,
        AlsMantleDrivePlugin,
        AlsRagdollLinkPlugin,
        AlsDebugDrawPlugin,
        AlsShowDebugHudPlugin,
        PlaygroundPlugin {
            manifest: "assets/models/playground/playground.ron".into(),
        },
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
        AlsCameraPlugin,
    ))
    .init_resource::<SpawnOnce>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(Startup, (spawn_sun, arm_auto_quit))
    .add_systems(
        Update,
        (
            spawn_character_once,
            playground_physics_bridge,
            graph_router,
            look_pitch_integrator,
            auto_quit,
        ),
    );

    app.run();
}

#[derive(Resource, Default)]
struct SpawnOnce(bool);

#[derive(Resource)]
struct AutoQuit(Instant);

fn arm_auto_quit(mut commands: Commands) {
    let Ok(secs) = std::env::var("ALS_BOOT_AUTO_QUIT_SECS") else {
        return;
    };
    let Ok(secs) = secs.parse::<f64>() else {
        return;
    };
    commands.insert_resource(AutoQuit(Instant::now() + Duration::from_secs_f64(secs)));
    info!("[MANUAL] auto quit in {secs}s");
}

fn auto_quit(timer: Option<Res<AutoQuit>>, mut exit: MessageWriter<AppExit>) {
    let Some(t) = timer else {
        return;
    };
    if Instant::now() >= t.0 {
        info!("[MANUAL] auto quit reached");
        exit.write(AppExit::Success);
    }
}

fn spawn_sun(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 30_000.0,
            ..default()
        },
        Transform::from_xyz(5.0, 20.0, -3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn spawn_character_once(
    mut commands: Commands,
    mut once: ResMut<SpawnOnce>,
    spawn_point: Option<Res<PlaygroundSpawnPoint>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
        "[MANUAL] character at ({:.2},{:.2},{:.2})",
        point.position.x, point.position.y, point.position.z
    );

    let ground_mat = materials.add(Color::srgb(0.35, 0.45, 0.28));
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(120.0, 40.0, 120.0),
        Transform::from_xyz(0.0, -20.1, 0.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(120.0, 40.0, 120.0)))),
        MeshMaterial3d(ground_mat),
    ));

    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform =
        Transform::from_translation(point.position).with_rotation(point.rotation);
    let spawn_fwd = bundle.transform.forward();
    let character = commands
        .spawn(bundle)
        .insert((AlsViewYaw(f32::atan2(spawn_fwd.x, -spawn_fwd.z)), AlsViewPitch(0.0), AlsCameraTarget))
        .id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());

    let avatar = commands
        .spawn((
            AnimatedSceneHandle::new(
                assets.load("animated_scenes/als_ragdoll.animscn.ron"),
            ),
            Transform::from_xyz(0.0, -0.90, 0.0),
        ))
        .id();
    commands.entity(character).add_child(avatar);

    let cam_pos = point.position + Vec3::new(0.0, 1.8, 5.0);
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(cam_pos).looking_at(point.position + Vec3::Y, Vec3::Y),
        AlsCameraRig::default(),
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

#[allow(clippy::type_complexity)]
fn graph_router(
    q: Query<
        (
            &Stance,
            &Gait,
            &LinearVelocity,
            &als_core::character::AlsMovementParamsHandle,
            Option<&AlsAirState>,
        ),
        With<AlsCharacter>,
    >,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
    params: Res<Assets<AlsMovementParams>>,
) {
    let Ok((stance, gait, vel, handle, air)) = q.single() else {
        return;
    };
    let Some(p) = params.get(handle.id()) else {
        return;
    };
    let hspeed = Vec2::new(vel.x, vel.z).length();
    let mgi = compute_motion_graph_inputs(p, *stance, *gait, hspeed);
    let air_state = air.copied().unwrap_or(AlsAirState::default());
    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            write_motion_graph_pins(&mut player, &mgi);
            write_air_pins(&mut player, &air_state);
        }
    }
}

fn look_pitch_integrator(
    mut q: Query<(&AlsInputCommand, &mut AlsViewPitch), With<AlsCharacter>>,
) {
    for (cmd, mut pitch) in &mut q {
        pitch.0 = clamp_pitch(pitch.0 - cmd.look_delta.y * LOOK_RADIANS_PER_COUNT);
    }
}
