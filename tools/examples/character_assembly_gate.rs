use std::num::NonZeroU8;

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;

use bevy_als::als_camera::{AlsCameraRig, AlsCameraTarget};
use bevy_als::als::character::{AlsCharacter, AlsCharacterPlugin};
use bevy_als::als::control_scheme::AlsControlSchemeConfig;
use bevy_als::als::params::AlsMovementParams;
use bevy_als::als::view::AlsViewPitch;
use bevy_als::character_assembly::{
    AlsCharacterAssembly, AlsCharacterAvatar, AlsCharacterBlueprint, AlsCharacterCamera,
    AlsPlayerCharacter, spawn_als_character_with_materials,
};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            AnimationGraphPlugin::default(),
            AlsCharacterPlugin,
        ))
        .init_asset::<StandardMaterial>()
        .add_systems(Startup, spawn_character)
        .add_systems(Update, verify_character_assembly)
        .run();
}

fn spawn_character(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    spawn_als_character_with_materials(
        &mut commands,
        &assets,
        &mut materials,
        &mut params_assets,
        &mut config_assets,
        AlsCharacterBlueprint {
            spawn_camera: true,
            ..AlsCharacterBlueprint::new(Transform::from_rotation(Quat::from_rotation_y(0.75)))
        },
    );
}

fn verify_character_assembly(
    characters: Query<
        (
            Entity,
            &AlsCharacterAssembly,
            &bevy_als::als::motion::AlsViewYaw,
            &AlsViewPitch,
            Has<AlsPlayerCharacter>,
            Has<AlsCameraTarget>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<&AlsCharacterAvatar>,
    cameras: Query<(&AlsCharacterCamera, &AlsCameraRig), With<Camera3d>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((character, assembly, yaw, pitch, input_owner, camera_target)) = characters.single()
    else {
        return;
    };
    let avatar_owner = avatars
        .get(assembly.avatar)
        .ok()
        .map(|owner| owner.character);
    let Some(camera) = assembly.camera else {
        error!("[BCHAR1][VERDICT] missing camera relationship -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(2).unwrap()));
        return;
    };
    let camera_owner = cameras.get(camera).ok().map(|(owner, _)| owner.character);
    let camera_rig = cameras.get(camera).is_ok();
    let pass = input_owner
        && camera_target
        && avatar_owner == Some(character)
        && camera_owner == Some(character)
        && camera_rig
        && yaw.0.is_finite()
        && pitch.0.is_finite();
    if pass {
        info!(
            "[BCHAR1][VERDICT] input_owner={input_owner} camera_target={camera_target} avatar_owner={avatar_owner:?} camera_owner={camera_owner:?} camera_rig={camera_rig} yaw={:.6} pitch={:.6} -> PASS",
            yaw.0, pitch.0
        );
        exit.write(AppExit::Success);
    } else {
        error!(
            "[BCHAR1][VERDICT] input_owner={input_owner} camera_target={camera_target} avatar_owner={avatar_owner:?} camera_owner={camera_owner:?} camera_rig={camera_rig} yaw={:.6} pitch={:.6} -> FAIL",
            yaw.0, pitch.0
        );
        exit.write(AppExit::Error(NonZeroU8::new(3).unwrap()));
    }
}
