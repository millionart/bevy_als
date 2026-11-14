use std::num::NonZeroU8;

use bevy::prelude::*;

use bevy_als::als::aim::{AlsAimPlugin, AlsLastFreeRotationMode};
use bevy_als::als::character::AlsCharacter;
use bevy_als::als::input::AlsInputCommand;
use bevy_als::als::input_request::{
    AlsCameraLeftShoulder, AlsFirstPersonView, AlsInputRequestPlugin,
};
use bevy_als::als::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use bevy_als::als::transition::{
    AlsStateRequest, AlsStateTransitionPlugin, state_transition_system,
};

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            AlsStateTransitionPlugin,
            AlsInputRequestPlugin,
            AlsAimPlugin,
        ))
        .add_systems(Startup, spawn_character)
        .add_systems(Update, verify_view_modes.after(state_transition_system))
        .run();
}

fn spawn_character(mut commands: Commands) {
    commands.spawn((
        AlsCharacter,
        LocomotionState::Grounded,
        Stance::Standing,
        Gait::Running,
        OverlayState::Default,
        RotationMode::VelocityDirection,
        AlsStateRequest::default(),
        AlsInputCommand {
            rotation_mode_pressed: true,
            ..default()
        },
    ));
}

fn verify_view_modes(
    mut phase: Local<u8>,
    mut characters: Query<
        (
            &mut AlsInputCommand,
            &RotationMode,
            Option<&AlsLastFreeRotationMode>,
            Has<AlsFirstPersonView>,
            Has<AlsCameraLeftShoulder>,
        ),
        With<AlsCharacter>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((mut command, rotation, last_free, first_person, left_shoulder)) =
        characters.single_mut()
    else {
        return;
    };
    let failed = |message: String, exit: &mut MessageWriter<AppExit>| {
        eprintln!("[BCHAR6][VERDICT] {message} -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(12).unwrap()));
    };
    match *phase {
        0 => {
            if *rotation != RotationMode::ViewDirection {
                failed(format!("phase=free rotation={rotation:?}"), &mut exit);
                return;
            }
            command.rotation_mode_pressed = false;
            command.aim_held = true;
            *phase = 1;
        }
        1 => {
            if *rotation != RotationMode::Aiming
                || last_free.map(|value| value.0) != Some(RotationMode::ViewDirection)
            {
                failed(
                    format!("phase=aim rotation={rotation:?} last_free={last_free:?}"),
                    &mut exit,
                );
                return;
            }
            command.rotation_mode_pressed = true;
            *phase = 2;
        }
        2 => {
            if *rotation != RotationMode::Aiming
                || last_free.map(|value| value.0) != Some(RotationMode::VelocityDirection)
            {
                failed(
                    format!("phase=aim_toggle rotation={rotation:?} last_free={last_free:?}"),
                    &mut exit,
                );
                return;
            }
            command.rotation_mode_pressed = false;
            command.aim_held = false;
            *phase = 3;
        }
        3 => {
            if *rotation != RotationMode::VelocityDirection {
                failed(
                    format!("phase=aim_release rotation={rotation:?}"),
                    &mut exit,
                );
                return;
            }
            command.view_mode_pressed = true;
            command.switch_shoulder_pressed = true;
            *phase = 4;
        }
        4 => {
            if !first_person || !left_shoulder {
                return;
            }
            command.view_mode_pressed = false;
            command.switch_shoulder_pressed = false;
            *phase = 5;
        }
        5 => {
            command.view_mode_pressed = true;
            command.switch_shoulder_pressed = true;
            *phase = 6;
        }
        _ => {
            if first_person || left_shoulder {
                return;
            }
            println!(
                "[BCHAR6][VERDICT] free=ViewDirection aim=Aiming aim_toggle_last_free=VelocityDirection aim_release=VelocityDirection first_person_toggle=true shoulder_toggle=true -> PASS"
            );
            exit.write(AppExit::Success);
        }
    }
}
