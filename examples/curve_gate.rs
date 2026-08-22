use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::AnimationGraphPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        AnimationGraphPlugin::default(),
        als_ext::AlsExtPlugin,
    ))
    .init_resource::<SampleLog>()
    .add_systems(Startup, setup)
    .add_systems(Update, (drive_curve, apply_curve_to_cube));

    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() {
        app.add_systems(Update, auto_quit);
    }

    app.run();
}

#[derive(Resource, Default)]
struct SampleLog {
    last_logged_t: f32,
}

#[derive(Component)]
struct CurveCube;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    commands.spawn(AnimatedSceneHandle::new(
        assets.load("animated_scenes/curve_gate.animscn.ron"),
    ));

    commands.spawn((
        CurveCube,
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(0.5, 0.5, 0.5)))),
        MeshMaterial3d(materials.add(Color::srgb(0.9, 0.4, 0.2))),
        Transform::from_xyz(-1.2, 0.25, 0.0),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(2.6, 1.6, 3.8).looking_at(Vec3::new(-0.6, 0.6, 0.0), Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(3.0, 5.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn drive_curve(
    time: Res<Time>,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let Ok(instance) = instances.single() else {
        return;
    };
    let Ok(mut player) = players.get_mut(instance.player_entity()) else {
        return;
    };
    player.set_input_data("drive_time", DataValue::from(time.elapsed_secs_f64().min(10.0) as f32));
}

fn apply_curve_to_cube(
    time: Res<Time>,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut cubes: Query<&mut Transform, With<CurveCube>>,
    mut log: ResMut<SampleLog>,
) {
    let Ok(instance) = instances.single() else {
        return;
    };
    let Ok(player) = players.get_mut(instance.player_entity()) else {
        return;
    };
    let Some(value) = player.get_outputs().get("curve_value") else {
        return;
    };
    let Ok(value) = value.as_f32() else {
        return;
    };
    for mut transform in cubes.iter_mut() {
        transform.scale = Vec3::splat(0.2 + value * 1.8);
        transform.translation.y = 0.25 + value * 0.8;
    }

    let t = time.elapsed_secs_f64() as f32;
    if t - log.last_logged_t >= 0.5 {
        log.last_logged_t = t;
        info!("[CURVE_GATE] drive_time={t:.2} curve_value={value:.4}");
    }
}

fn auto_quit(time: Res<Time>, mut exit: MessageWriter<AppExit>) {
    if time.elapsed_secs_f64() >= 4.5 {
        info!("[CURVE_GATE] auto-quit");
        exit.write(AppExit::Success);
    }
}
