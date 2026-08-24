use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::animated_scene::{AnimatedSceneHandle, AnimatedSceneInstance};
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_core::character::{AlsCharacterBundle, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::input::{spawn_als_actions, AlsInputMap, AlsInputPlugin};
use als_core::motion::{AlsMotionPlugin, AlsViewYaw};
use als_core::params::AlsMovementParams;
use als_core::transition::AlsStateTransitionPlugin;
use als_ext::AlsExtPlugin;

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
        AlsDebugDrawPlugin,
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
    ))
    .init_resource::<SpawnOnce>()
    .add_systems(Startup, spawn_ground)
    .add_systems(Update, (spawn_character_once, auto_quit));

    info!("[DDSMOKE] running for 4s");
    let _ = Duration::from_secs(4);
    app.insert_resource(Deadline(Instant::now() + Duration::from_secs(6)));
    app.run();
}

#[derive(Resource, Default)]
struct SpawnOnce(bool);

#[derive(Resource)]
struct Deadline(Instant);

fn spawn_ground(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 0.3, 40.0),
        Transform::from_xyz(0.0, -0.15, 0.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(40.0, 0.3, 40.0)))),
        MeshMaterial3d(materials.add(Color::srgb(0.24, 0.27, 0.24))),
    ));
}

fn spawn_character_once(
    mut commands: Commands,
    mut once: ResMut<SpawnOnce>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    assets: Res<AssetServer>,
) {
    if once.0 {
        return;
    }
    once.0 = true;
    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let bundle = AlsCharacterBundle::new(params, config);
    let character = commands.spawn(bundle).id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());

    let avatar = bevy_als::character_assembly::attach_avatar(&mut commands, &assets, character, "animated_scenes/motion_overlay.animscn.ron");
}

fn auto_quit(deadline: Res<Deadline>, time: Res<Time>, mut exit: MessageWriter<AppExit>) {
    if time.elapsed_secs_f64() >= 4.0 || Instant::now() >= deadline.0 {
        info!("[DDSMOKE] clean exit");
        exit.write(AppExit::Success);
    }
}
