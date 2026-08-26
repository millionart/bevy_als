use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;

use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::params::AlsMovementParams;

use bevy_als::debug_draw::AlsDebugDrawPlugin;
use bevy_als::playable_character::{
    AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        AlsPlayableCharacterPlugin,
        AlsDebugDrawPlugin,
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
    let _character = spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 0.95, 0.0)),
    );
}

fn auto_quit(deadline: Res<Deadline>, time: Res<Time>, mut exit: MessageWriter<AppExit>) {
    if time.elapsed_secs_f64() >= 4.0 || Instant::now() >= deadline.0 {
        info!("[DDSMOKE] clean exit");
        exit.write(AppExit::Success);
    }
}
