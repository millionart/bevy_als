use avian3d::prelude::{Collider, CollisionLayers, LayerMask, RigidBody};
use bevy::prelude::*;
use bevy_als::{
    AlsCharacterBlueprint, AlsCollisionLayer, AlsControlSchemeConfig, AlsMovementParams, AlsPlugin,
    spawn_als_character,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, AlsPlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut params: ResMut<Assets<AlsMovementParams>>,
    mut configs: ResMut<Assets<AlsControlSchemeConfig>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(24.0, 0.2, 24.0),
        CollisionLayers::new(
            [AlsCollisionLayer::Default, AlsCollisionLayer::Visibility],
            LayerMask::ALL,
        ),
        Mesh3d(meshes.add(Cuboid::new(24.0, 0.2, 24.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.24, 0.32, 0.39))),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(3.0, 8.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    spawn_als_character(
        &mut commands,
        &assets,
        &mut params,
        &mut configs,
        AlsCharacterBlueprint {
            spawn_camera: true,
            ..AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 1.5, 0.0))
        },
    );
}
