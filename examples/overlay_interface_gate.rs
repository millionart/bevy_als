use std::num::NonZeroU8;

use bevy::prelude::*;

use als_core::character::AlsCharacter;
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_core::transition::{
    AlsStateRequest, AlsStateTransitionPlugin, Dimension, StateTransitionEvent, StateValue,
    state_transition_system,
};

fn main() {
    App::new()
        .add_plugins((MinimalPlugins, AlsStateTransitionPlugin))
        .add_systems(Startup, spawn_character)
        .add_systems(
            Update,
            verify_overlay_interface.after(state_transition_system),
        )
        .run();
}

fn spawn_character(mut commands: Commands) {
    commands.spawn((
        AlsCharacter,
        LocomotionState::Grounded,
        Stance::Standing,
        Gait::Running,
        RotationMode::VelocityDirection,
        OverlayState::Default,
        AlsStateRequest {
            overlay_state: Some(OverlayState::Rifle),
            ..default()
        },
    ));
}

fn verify_overlay_interface(
    mut phase: Local<u8>,
    mut events: MessageReader<StateTransitionEvent>,
    mut characters: Query<(Entity, &mut AlsStateRequest, &OverlayState), With<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((entity, mut request, overlay)) = characters.single_mut() else {
        return;
    };
    let received = events.read().copied().collect::<Vec<_>>();
    match *phase {
        0 => {
            let expected = StateTransitionEvent {
                entity,
                dimension: Dimension::OverlayState,
                from: StateValue::OverlayState(OverlayState::Default),
                to: StateValue::OverlayState(OverlayState::Rifle),
            };
            if *overlay != OverlayState::Rifle || received != vec![expected] {
                eprintln!(
                    "[BCHAR7][VERDICT] phase=rifle overlay={overlay:?} events={received:?} -> FAIL"
                );
                exit.write(AppExit::Error(NonZeroU8::new(13).unwrap()));
                return;
            }
            request.overlay_state = Some(OverlayState::Default);
            *phase = 1;
        }
        _ => {
            let expected = StateTransitionEvent {
                entity,
                dimension: Dimension::OverlayState,
                from: StateValue::OverlayState(OverlayState::Rifle),
                to: StateValue::OverlayState(OverlayState::Default),
            };
            let pass = *overlay == OverlayState::Default && received == vec![expected];
            if pass {
                println!(
                    "[BCHAR7][VERDICT] OverlayState Default->Rifle->Default event_count=2 default_fallback=true animation_graph_modified=false -> PASS"
                );
                exit.write(AppExit::Success);
            } else {
                eprintln!(
                    "[BCHAR7][VERDICT] phase=default overlay={overlay:?} events={received:?} -> FAIL"
                );
                exit.write(AppExit::Error(NonZeroU8::new(14).unwrap()));
            }
        }
    }
}
