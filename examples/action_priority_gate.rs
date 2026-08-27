use std::num::NonZeroU8;

use bevy::prelude::*;

use als_core::aim::AlsAimPlugin;
use als_core::character::AlsCharacter;
use als_core::input::AlsInputCommand;
use als_core::input_request::AlsInputRequestPlugin;
use als_core::motion::{JumpChain, jump_chain};
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin, state_transition_system};

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            AlsStateTransitionPlugin,
            AlsInputRequestPlugin,
            AlsAimPlugin,
        ))
        .add_systems(Startup, spawn_character)
        .add_systems(Update, verify_priority.after(state_transition_system))
        .run();
}

#[derive(Resource, Default)]
struct GateState {
    phase: u8,
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
            ragdoll_pressed: true,
            aim_held: true,
            ..default()
        },
    ));
}

fn verify_priority(
    mut state: Local<GateState>,
    mut characters: Query<
        (&mut AlsInputCommand, &LocomotionState, &RotationMode),
        With<AlsCharacter>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((mut command, locomotion, rotation)) = characters.single_mut() else {
        return;
    };
    let chain_pass = jump_chain(LocomotionState::Ragdolling, Stance::Crouching, true)
        == JumpChain::ExitRagdoll
        && jump_chain(LocomotionState::Grounded, Stance::Standing, true) == JumpChain::Mantle
        && jump_chain(LocomotionState::Grounded, Stance::Crouching, false) == JumpChain::StandUp
        && jump_chain(LocomotionState::Air, Stance::Standing, false) == JumpChain::Jump;
    match state.phase {
        0 => {
            if *locomotion != LocomotionState::Ragdolling || *rotation != RotationMode::Aiming {
                eprintln!(
                    "[BCHAR5][VERDICT] phase=enter locomotion={locomotion:?} rotation={rotation:?} -> FAIL"
                );
                exit.write(AppExit::Error(NonZeroU8::new(9).unwrap()));
                return;
            }
            command.ragdoll_pressed = false;
            command.aim_held = false;
            state.phase = 1;
        }
        1 => {
            if *locomotion != LocomotionState::Ragdolling
                || *rotation != RotationMode::VelocityDirection
            {
                eprintln!(
                    "[BCHAR5][VERDICT] phase=aim_release locomotion={locomotion:?} rotation={rotation:?} -> FAIL"
                );
                exit.write(AppExit::Error(NonZeroU8::new(10).unwrap()));
                return;
            }
            command.ragdoll_pressed = true;
            state.phase = 2;
        }
        _ => {
            let pass = *locomotion == LocomotionState::Grounded
                && *rotation == RotationMode::VelocityDirection
                && chain_pass;
            if pass {
                println!(
                    "[BCHAR5][VERDICT] enter_ragdoll=true aim_release=VelocityDirection exit_ragdoll=Grounded jump_chain=ExitRagdoll>Mantle>StandUp>Jump -> PASS"
                );
                exit.write(AppExit::Success);
            } else {
                eprintln!(
                    "[BCHAR5][VERDICT] exit locomotion={locomotion:?} rotation={rotation:?} chain={chain_pass} -> FAIL"
                );
                exit.write(AppExit::Error(NonZeroU8::new(11).unwrap()));
            }
        }
    }
}
