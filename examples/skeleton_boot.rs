use std::time::{Duration, Instant};

use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::params::AlsMovementParams;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_als::playable_character::{
    AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};
use bevy_animation_graph::core::animation_clip::GraphClip;
use bevy_animation_graph::core::animation_graph::AnimationGraph;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::skeleton::Skeleton;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        AlsPlayableCharacterPlugin,
    ))
    .add_systems(Startup, setup);

    if let Ok(secs) = std::env::var("ALS_BOOT_AUTO_QUIT_SECS") {
        let secs: f64 = secs.parse().unwrap_or(5.0);
        app.insert_resource(AutoQuit(Instant::now() + Duration::from_secs_f64(secs)));
        app.add_systems(Update, auto_quit);
    }

    app.add_systems(Update, boot_status);

    app.run();
}

fn boot_status(
    clips: Res<Assets<GraphClip>>,
    skeletons: Res<Assets<Skeleton>>,
    graphs: Res<Assets<AnimationGraph>>,
    players: Query<&AnimationGraphPlayer>,
    mut printed: Local<u32>,
) {
    if *printed >= 2 {
        return;
    }
    let Some(player) = players
        .iter()
        .max_by(|a, b| a.elapsed().total_cmp(&b.elapsed()))
    else {
        return;
    };
    let t = player.elapsed();
    let hit = (*printed == 0 && t > 0.0) || (*printed == 1 && t > 2.0);
    if !hit {
        return;
    }
    info!(
        "SKELETON_BOOT_OK step={} clips={} skeletons={} graphs={} player_elapsed={:.3}s",
        *printed + 1,
        clips.len(),
        skeletons.len(),
        graphs.len(),
        t
    );
    *printed += 1;
}

#[derive(Resource)]
struct AutoQuit(Instant);

fn auto_quit(mut quit: Option<ResMut<AutoQuit>>, mut exit: MessageWriter<AppExit>) {
    let Some(aq) = quit.as_deref_mut() else {
        return;
    };
    if Instant::now() >= aq.0 {
        exit.write(AppExit::Success);
    }
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 0.95, 0.0)),
    );

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(1.6, 1.1, 2.4).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(2.0, 4.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
