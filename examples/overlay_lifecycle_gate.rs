use std::num::NonZeroU8;

use bevy::asset::AssetPlugin;
use bevy::prelude::*;

use als_core::character::AlsCharacter;
use als_core::mantle::{AlsMantleState, MantleKind};
use als_core::state::{LocomotionState, OverlayState};
use bevy_als::character_assembly::AlsCharacterAssembly;
use bevy_als::overlay_object::{AlsOverlayObject, OverlayObjectPlugin, RIGHT_GUN_BONE};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            OverlayObjectPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, verify_overlay_lifecycle)
        .run();
}

fn setup(mut commands: Commands) {
    let avatar = commands
        .spawn((Name::new("avatar"), Transform::IDENTITY))
        .id();
    let right = commands
        .spawn((Name::new(RIGHT_GUN_BONE), Transform::IDENTITY))
        .id();
    commands.entity(avatar).add_child(right);
    commands.spawn((
        AlsCharacter,
        OverlayState::Rifle,
        LocomotionState::Grounded,
        AlsMantleState::default(),
        AlsCharacterAssembly {
            avatar,
            camera: None,
        },
    ));
}

fn verify_overlay_lifecycle(
    mut phase: Local<u8>,
    mut frames: Local<u32>,
    mut characters: Query<(&mut LocomotionState, &mut AlsMantleState), With<AlsCharacter>>,
    props: Query<&AlsOverlayObject>,
    mut exit: MessageWriter<AppExit>,
) {
    *frames += 1;
    let Ok((mut locomotion, mut mantle)) = characters.single_mut() else {
        return;
    };
    let prop_count = props.iter().count();
    if *frames > 240 {
        eprintln!(
            "[BCHAR9][VERDICT] timeout phase={} prop_count={prop_count} -> FAIL",
            *phase
        );
        exit.write(AppExit::Error(NonZeroU8::new(16).unwrap()));
        return;
    }
    match *phase {
        0 if prop_count == 1 => {
            mantle.active = true;
            mantle.kind = MantleKind::High;
            *phase = 1;
        }
        1 if prop_count == 0 => {
            mantle.kind = MantleKind::LowLeft;
            *phase = 2;
        }
        2 if prop_count == 1 => {
            *locomotion = LocomotionState::Ragdolling;
            *phase = 3;
        }
        3 if prop_count == 0 => {
            *locomotion = LocomotionState::Grounded;
            mantle.active = false;
            *phase = 4;
        }
        4 if prop_count == 1 => {
            println!(
                "[BCHAR9][VERDICT] high_mantle_clears=true low_mantle_keeps=true ragdoll_clears=true grounded_restores=true -> PASS"
            );
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
