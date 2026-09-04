use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use avian3d::prelude::{Collider, RigidBody};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;

use als_core::character::{AlsCharacter, AlsCharacterBundle};
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::input::AlsInputCommand;
use als_core::motion::{AlsInputOverride, AlsViewYaw};
use als_core::params::AlsMovementParams;
use als_core::state::{OverlayState, RotationMode};

use bevy_als::character_assembly::{AlsCharacterAvatar, attach_avatar};
use bevy_als::head_layer::AlsHeadCurveState;
use bevy_als::playable_character::AlsPlayableCharacterPlugin;

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

#[derive(Resource)]
struct Deadline(Instant);

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
    let params_value = AlsMovementParams::default();
    let params = params_assets.add(params_value.clone());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_xyz(0.0, 0.92, 0.0);
    bundle.overlay_state = OverlayState::Rifle;
    bundle.rotation_mode = RotationMode::Aiming;
    let character = commands
        .spawn((
            bundle,
            AlsInputOverride(AlsInputCommand {
                aim_held: true,
                ..default()
            }),
            AlsViewYaw(0.0),
        ))
        .id();
    attach_avatar(
        &mut commands,
        &assets,
        character,
        "animated_scenes/motion_overlay.animscn.ron",
    );
}

fn graph_f32(player: &AnimationGraphPlayer, name: &str) -> Option<f32> {
    player
        .get_outputs()
        .get(name)
        .and_then(|value| value.as_f32().ok())
}

fn verify(
    deadline: Res<Deadline>,
    curve_states: Query<&AlsHeadCurveState, With<AlsCharacter>>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(curve_state) = curve_states.single() else {
        return;
    };
    let Some((_parent, instance)) = avatars.iter().next() else {
        return;
    };
    let Ok(player) = players.get(instance.player_entity()) else {
        return;
    };
    let pose_aiming = graph_f32(player, "PoseAiming");
    let view_block = graph_f32(player, "ViewBlock");
    let head_blend = player
        .get_global_input_data("view_state.head_blend_amount")
        .and_then(|value| value.as_f32().ok());
    let Some((pose_aiming, view_block, head_blend)) = pose_aiming
        .zip(view_block)
        .zip(head_blend)
        .map(|((pose_aiming, view_block), head_blend)| (pose_aiming, view_block, head_blend))
    else {
        if Instant::now() >= deadline.0 {
            error!("[BF102.06][VERDICT] graph outputs unavailable");
            exit.write(AppExit::Error(NonZeroU8::new(6).unwrap()));
        }
        return;
    };
    let expected = (1.0 - view_block.clamp(0.0, 1.0)) * (1.0 - pose_aiming.clamp(0.0, 1.0));
    let blocked = pose_aiming >= 0.99 || view_block >= 0.99;
    let matched = (curve_state.view_block_amount - view_block.clamp(0.0, 1.0)).abs() < 1e-4
        && (curve_state.pose_aiming_amount - pose_aiming.clamp(0.0, 1.0)).abs() < 1e-4
        && (head_blend - expected).abs() < 1e-4;
    if blocked && matched {
        info!(
            "[BF102.06][VERDICT] pose_aiming={pose_aiming:.3} view_block={view_block:.3} head_blend={head_blend:.3} expected={expected:.3} -> PASS"
        );
        exit.write(AppExit::Success);
    } else if Instant::now() >= deadline.0 {
        error!(
            "[BF102.06][VERDICT] pose_aiming={pose_aiming:.3} view_block={view_block:.3} head_blend={head_blend:.3} expected={expected:.3} blocked={blocked} matched={matched} -> FAIL"
        );
        exit.write(AppExit::Error(NonZeroU8::new(7).unwrap()));
    }
}
