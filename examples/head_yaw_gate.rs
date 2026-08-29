use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use avian3d::prelude::{Collider, RigidBody};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;

use als_core::character::{AlsCharacter, AlsCharacterBundle};
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::motion::AlsViewYaw;
use als_core::orientation::world_forward_to_yaw;
use als_core::params::AlsMovementParams;
use bevy_als::character_assembly::{AlsCharacterAvatar, attach_avatar};
use bevy_als::head_layer::AlsHeadState;
use bevy_als::playable_character::AlsPlayableCharacterPlugin;

#[derive(Resource)]
struct Deadline(Instant);

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            AlsPlayableCharacterPlugin,
        ))
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(12)))
        .add_systems(Startup, setup)
        .add_systems(Update, verify)
        .run();
}

fn setup(
    mut commands: Commands,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    assets: Res<AssetServer>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 0.2, 40.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_xyz(0.0, 0.92, 0.0)
        .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
    let character = commands
        .spawn((bundle, AlsViewYaw(std::f32::consts::FRAC_PI_2)))
        .id();
    attach_avatar(
        &mut commands,
        &assets,
        character,
        "animated_scenes/motion_overlay.animscn.ron",
    );
}

fn verify(
    deadline: Res<Deadline>,
    characters: Query<(&Transform, &AlsViewYaw, &AlsHeadState), With<AlsCharacter>>,
    avatars: Query<&AnimatedSceneInstance, With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((transform, view_yaw, head)) = characters.single() else {
        return;
    };
    let Some(player) = avatars
        .iter()
        .find_map(|instance| players.get(instance.player_entity()).ok())
    else {
        return;
    };
    let Some(graph_amount) = player
        .get_global_input_data("als_head_yaw_amount")
        .and_then(|value| value.as_f32().ok())
    else {
        if Instant::now() >= deadline.0 {
            error!("[BF102.10][VERDICT] head output unavailable -> FAIL");
            exit.write(AppExit::Error(NonZeroU8::new(10).unwrap()));
        }
        return;
    };
    let body_yaw = world_forward_to_yaw(transform.rotation * Vec3::NEG_Z);
    let aligned = (body_yaw - view_yaw.0).abs() < 1e-4
        && head.yaw_angle.abs() < 1e-3
        && (head.yaw_amount - 0.5).abs() < 1e-4
        && (graph_amount - 0.5).abs() < 1e-4;
    if aligned {
        info!(
            "[BF102.10] body_yaw={:.3} view_yaw={:.3} head_yaw={:.3} amount={graph_amount:.3}",
            body_yaw, view_yaw.0, head.yaw_angle,
        );
        info!("[BF102.10][VERDICT] aligned_body_and_view_keep_head_neutral -> PASS");
        exit.write(AppExit::Success);
    } else if Instant::now() >= deadline.0 {
        error!(
            "[BF102.10][VERDICT] body_yaw={:.3} view_yaw={:.3} head_yaw={:.3} amount={graph_amount:.3} -> FAIL",
            body_yaw, view_yaw.0, head.yaw_angle,
        );
        exit.write(AppExit::Error(NonZeroU8::new(11).unwrap()));
    }
}
