use crate::als::character::AlsCharacter;
use crate::als::state::LocomotionState;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;

use crate::character_assembly::AlsCharacterAvatar;
use crate::locomotion_layer::AlsLocomotionLayerSet;
use crate::stop_link::AlsTransitionMontageStack;
use crate::turn_layer::AlsTurnMontageStack;

#[derive(Component, Clone, Copy)]
struct SourceLocomotionLifecycleState {
    locomotion: LocomotionState,
}

pub struct LocomotionLifecyclePlugin;

impl Plugin for LocomotionLifecyclePlugin {
    fn build(&self, app: &mut App) {
        let outer = crate::AlsPhysicsSchedule::outer(app);
        app.add_systems(
            outer,
            stop_transition_and_turn_on_locomotion_exit
                .after(AlsLocomotionLayerSet)
                .before(
                    crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate,
                ),
        );
    }
}

fn source_grounded_exit_stops_transition_and_turn(
    previous: LocomotionState,
    current: LocomotionState,
) -> bool {
    previous == LocomotionState::Grounded && current == LocomotionState::Air
}

fn stop_transition_and_turn_on_locomotion_exit(
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            &LocomotionState,
            Option<&mut SourceLocomotionLifecycleState>,
            Option<&mut AlsTransitionMontageStack>,
        ),
        With<AlsCharacter>,
    >,
    mut avatars: Query<(&ChildOf, &mut AlsTurnMontageStack), With<AlsCharacterAvatar>>,
) {
    let mut exited = Vec::new();
    for (character, locomotion, previous, transition) in &mut characters {
        let prior = previous
            .as_ref()
            .map_or(*locomotion, |state| state.locomotion);
        if source_grounded_exit_stops_transition_and_turn(prior, *locomotion) {
            if let Some(mut transition) = transition {
                transition.request_stop();
            }
            exited.push(character);
        }
        if let Some(mut previous) = previous {
            previous.locomotion = *locomotion;
        } else {
            commands
                .entity(character)
                .insert(SourceLocomotionLifecycleState {
                    locomotion: *locomotion,
                });
        }
    }
    for character in exited {
        for (parent, mut stack) in &mut avatars {
            if parent.parent() == character {
                stack.request_stop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_grounded_exit_stops_only_when_entering_air() {
        assert!(source_grounded_exit_stops_transition_and_turn(
            LocomotionState::Grounded,
            LocomotionState::Air,
        ));
        assert!(!source_grounded_exit_stops_transition_and_turn(
            LocomotionState::Air,
            LocomotionState::Grounded,
        ));
    }
}
