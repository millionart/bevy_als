use std::num::NonZeroU8;

use bevy::prelude::*;

use bevy_als::als::aim::AlsAimPlugin;
use bevy_als::als::character::AlsCharacter;
use bevy_als::als::input::AlsInputCommand;
use bevy_als::als::input_request::AlsInputRequestPlugin;
use bevy_als::als::motion::{JumpChain, jump_chain};
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
        RotationMode::Aiming,
        bevy_als::als::aim::AlsLastFreeRotationMode(RotationMode::VelocityDirection),
        bevy_als::als::state::AlsNotifyLocomotionAction::None,
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
        (
            &mut AlsInputCommand,
            &mut LocomotionState,
            &RotationMode,
            &mut bevy_als::als::state::AlsNotifyLocomotionAction,
            &AlsStateRequest,
        ),
        With<AlsCharacter>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((mut command, mut locomotion, rotation, mut action, request)) = characters.single_mut()
    else {
        return;
    };
    let chain_pass = jump_chain(true, Stance::Crouching, true) == JumpChain::ExitRagdoll
        && jump_chain(false, Stance::Standing, true) == JumpChain::Mantle
        && jump_chain(false, Stance::Crouching, false) == JumpChain::StandUp
        && jump_chain(false, Stance::Standing, false) == JumpChain::Jump;
    match state.phase {
        0 => {
            if *action == bevy_als::als::state::AlsNotifyLocomotionAction::Ragdolling
                || *locomotion != LocomotionState::Grounded
                || *rotation != RotationMode::Aiming
            {
                eprintln!(
                    "[BCHAR5][VERDICT] phase=enter locomotion={locomotion:?} rotation={rotation:?} -> FAIL"
                );
                exit.write(AppExit::Error(NonZeroU8::new(9).unwrap()));
                return;
            }
            command.ragdoll_pressed = false;
            command.aim_held = false;
            *action = bevy_als::als::state::AlsNotifyLocomotionAction::Ragdolling;
            *locomotion = LocomotionState::None;
            state.phase = 1;
        }
        1 => {
            if *action != bevy_als::als::state::AlsNotifyLocomotionAction::Ragdolling
                || *locomotion != LocomotionState::None
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
            let pass = request.ragdoll_exit
                && *locomotion == LocomotionState::None
                && *action == bevy_als::als::state::AlsNotifyLocomotionAction::Ragdolling
                && *rotation == RotationMode::VelocityDirection
                && chain_pass;
            if pass {
                println!(
                    "[BCHAR5][VERDICT] missing_bodies_rejected=true aim_release=VelocityDirection exit_ragdoll_requested=true jump_chain=ExitRagdoll>Mantle>StandUp>Jump -> PASS"
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
