use avian3d::prelude::*;
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
use als_core::input::{spawn_als_actions, AlsInputMap, AlsInputPlugin};
use als_core::motion::{AlsMotionPlugin, AlsViewYaw};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, Stance};
use als_core::transition::AlsStateTransitionPlugin;
use als_ext::{
    AlsExtPlugin, PlaygroundMovingObject, PlaygroundPlugin, PlaygroundSpawnPoint,
    PlaygroundTextLabel,
};
use bevy_als::debug_draw::AlsDebugDrawPlugin;

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
        AlsDebugDrawPlugin,
        PlaygroundPlugin {
            manifest: "assets/models/playground/playground.ron".into(),
        },
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
    ))
    .init_resource::<SpawnOnce>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(Startup, spawn_sun)
    .add_systems(
        Update,
        (
            spawn_character_once,
            camera_follow,
            graph_router,
            report_moving,
        ),
    );

    app.run();
}

#[derive(Resource, Default)]
struct SpawnOnce(bool);

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
        point.position.x,
        point.position.y,
        point.position.z
    );

    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform =
        Transform::from_translation(point.position).with_rotation(point.rotation);
    let character = commands.spawn(bundle).id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());

    let avatar = commands
        .spawn((
            AnimatedSceneHandle::new(
                assets.load("animated_scenes/motion_overlay.animscn.ron"),
            ),
            Transform::from_xyz(0.0, -0.90, 0.0),
        ))
        .id();
    commands.entity(character).add_child(avatar);

    commands.spawn((Camera3d::default(), Transform::from_xyz(4.0, 3.0, -6.0)));
}

fn camera_follow(
    mut cameras: Query<&mut Transform, (With<Camera>, Without<AlsCharacter>)>,
    characters: Query<&GlobalTransform, With<AlsCharacter>>,
) {
    let Ok(char_gtf) = characters.single() else {
        return;
    };
    let char_pos = char_gtf.translation();
    let target = char_pos + Vec3::new(0.0, 2.2, 4.5);
    for mut cam in cameras.iter_mut() {
        cam.translation = cam.translation.lerp(target, 0.1);
        cam.look_at(char_pos + Vec3::Y * 0.9, Vec3::Y);
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
    let mgi =
        bevy_als::motion_graph_link::compute_motion_graph_inputs(p, *stance, *gait, hspeed);
    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("speed", bevy_animation_graph::core::edge_data::DataValue::from(mgi.speed));
            player.set_input_data("actual_speed", bevy_animation_graph::core::edge_data::DataValue::from(mgi.actual_speed));
            player.set_input_data("play_rate", bevy_animation_graph::core::edge_data::DataValue::from(mgi.play_rate));
        }
    }
}

fn report_moving(
    time: Res<Time>,
    mut last: Local<f32>,
    q: Query<&Transform, With<PlaygroundMovingObject>>,
) {
    let t = time.elapsed_secs_f64() as f32;
    if t - *last < 3.0 {
        return;
    }
    *last = t;
    for tf in q.iter() {
        info!(
            "[MOVING] pos=({:.2},{:.2},{:.2})",
            tf.translation.x, tf.translation.y, tf.translation.z
        );
    }
}
