use als_core::character::AlsCharacter;
use als_core::state::LocomotionState;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::plugin::AnimationGraphSet;

use crate::character_assembly::AlsCharacterAvatar;
use crate::locomotion_layer::AlsLocomotionLayerSet;
use crate::stop_link::AlsTransitionPlayback;
use crate::turn_layer::AlsTurnMontageStack;

#[derive(Component, Clone, Copy)]
struct SourceLocomotionLifecycleState {
    locomotion: LocomotionState,
}

pub struct LocomotionLifecyclePlugin;

impl Plugin for LocomotionLifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            stop_transition_and_turn_on_locomotion_exit
                .after(AlsLocomotionLayerSet)
                .before(AnimationGraphSet::PrePhysics),
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
            Option<&mut AlsTransitionPlayback>,
        ),
        With<AlsCharacter>,
    >,
    mut avatars: Query<(&ChildOf, &mut AlsTurnMontageStack), With<AlsCharacterAvatar>>,
) {
    let mut exited = Vec::new();
    for (character, locomotion, previous, transition) in &mut characters {
        let prior = previous.as_ref().map_or(*locomotion, |state| state.locomotion);
        if source_grounded_exit_stops_transition_and_turn(prior, *locomotion) {
            if let Some(mut transition) = transition {
                transition.request_stop();
            }
            exited.push(character);
        }
        if let Some(mut previous) = previous {
            previous.locomotion = *locomotion;
        } else {
            commands.entity(character).insert(SourceLocomotionLifecycleState {
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
