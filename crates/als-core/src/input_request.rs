use bevy::prelude::*;

use crate::character::AlsCharacter;
use crate::input::AlsInputCommand;
use crate::motion::AlsInputOverride;
use crate::state::{Gait, Stance};
use crate::transition::{state_transition_system, AlsStateRequest};

pub fn input_state_request_system(
    mut q: Query<
        (
            &Stance,
            &Gait,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            &mut AlsStateRequest,
        ),
        With<AlsCharacter>,
    >,
) {
    for (stance, gait, cmd, ovr, mut request) in &mut q {
        let effective = ovr.map(|o| &o.0).unwrap_or(cmd);

        if effective.crouch_pressed {
            let target = match stance {
                Stance::Standing => Stance::Crouching,
                Stance::Crouching => Stance::Standing,
            };
            if request.stance != Some(target) {
                request.stance = Some(target);
            }
        }

        let target_gait = if effective.sprint_held {
            Gait::Sprinting
        } else {
            Gait::Running
        };
        if *gait != target_gait || request.gait.is_some() {
            request.gait = Some(target_gait);
        }
    }
}

pub struct AlsInputRequestPlugin;

impl Plugin for AlsInputRequestPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            input_state_request_system.before(state_transition_system),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{LocomotionState, OverlayState, RotationMode};
    use crate::transition::{Dimension, StateTransitionEvent, StateValue};
    use crate::transition::AlsStateTransitionPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::MinimalPlugins;

    type AppBuilder = App;

    fn spawn_character(
        app: &mut App,
        stance: Stance,
        gait: Gait,
        cmd: AlsInputCommand,
    ) -> Entity {
        app.world_mut()
            .spawn((
                AlsCharacter,
                LocomotionState::Grounded,
                stance,
                gait,
                RotationMode::VelocityDirection,
                OverlayState::Default,
                AlsStateRequest::default(),
                cmd,
            ))
            .id()
    }

    fn app_with_plugins() -> AppBuilder {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsStateTransitionPlugin, AlsInputRequestPlugin));
        app
    }

    #[test]
    fn crouch_edge_toggles_stance_once_per_press() {
        let mut app = app_with_plugins();
        let e = spawn_character(
            &mut app,
            Stance::Standing,
            Gait::Walking,
            AlsInputCommand {
                crouch_pressed: true,
                ..Default::default()
            },
        );

        app.update();
        assert_eq!(*app.world().get::<Stance>(e).unwrap(), Stance::Crouching);
        assert_eq!(app.world().get::<AlsStateRequest>(e).unwrap().stance, None);

        app.world_mut()
            .get_mut::<AlsInputCommand>(e)
            .unwrap()
            .crouch_pressed = false;
        app.update();
        assert_eq!(
            *app.world().get::<Stance>(e).unwrap(),
            Stance::Crouching,
            "released edge must not toggle again"
        );
    }

    #[test]
    fn second_crouch_press_returns_to_standing() {
        let mut app = app_with_plugins();
        let e = spawn_character(
            &mut app,
            Stance::Standing,
            Gait::Walking,
            AlsInputCommand::default(),
        );

        for press in [true, false] {
            app.world_mut().get_mut::<AlsInputCommand>(e).unwrap().crouch_pressed = press;
            app.update();
        }

        assert_eq!(*app.world().get::<Stance>(e).unwrap(), Stance::Crouching);

        for press in [true, false] {
            app.world_mut().get_mut::<AlsInputCommand>(e).unwrap().crouch_pressed = press;
            app.update();
        }
        assert_eq!(*app.world().get::<Stance>(e).unwrap(), Stance::Standing);
    }

    #[test]
    fn sprint_hold_requests_sprinting_and_release_restores_running() {
        let mut app = app_with_plugins();
        let e = spawn_character(
            &mut app,
            Stance::Standing,
            Gait::Running,
            AlsInputCommand {
                sprint_held: true,
                ..Default::default()
            },
        );

        app.update();
        assert_eq!(*app.world().get::<Gait>(e).unwrap(), Gait::Sprinting);
        assert_eq!(app.world().get::<AlsStateRequest>(e).unwrap().gait, None);

        app.world_mut().get_mut::<AlsInputCommand>(e).unwrap().sprint_held = false;
        app.update();
        assert_eq!(*app.world().get::<Gait>(e).unwrap(), Gait::Running);
        assert_eq!(app.world().get::<AlsStateRequest>(e).unwrap().gait, None);
    }

    #[test]
    fn sprint_request_is_idempotent_while_held() {
        let mut app = app_with_plugins();
        let e = spawn_character(
            &mut app,
            Stance::Standing,
            Gait::Running,
            AlsInputCommand {
                sprint_held: true,
                ..Default::default()
            },
        );

        app.update();
        app.update();

        let events = app
            .world_mut()
            .run_system_once(|mut r: MessageReader<StateTransitionEvent>| {
                r.read()
                    .filter(|ev| ev.dimension == Dimension::Gait)
                    .copied()
                    .collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(events.len(), 1, "held sprint must not re-emit gait transitions");
        assert_eq!(
            events[0],
            StateTransitionEvent {
                entity: e,
                dimension: Dimension::Gait,
                from: StateValue::Gait(Gait::Running),
                to: StateValue::Gait(Gait::Sprinting),
            }
        );
    }

    #[test]
    fn override_takes_precedence_over_real_input() {
        let mut app = app_with_plugins();
        let e = spawn_character(
            &mut app,
            Stance::Standing,
            Gait::Walking,
            AlsInputCommand {
                crouch_pressed: false,
                sprint_held: false,
                ..Default::default()
            },
        );
        app.world_mut().entity_mut(e).insert(AlsInputOverride(AlsInputCommand {
            crouch_pressed: true,
            sprint_held: true,
            ..Default::default()
        }));

        app.update();
        assert_eq!(*app.world().get::<Stance>(e).unwrap(), Stance::Crouching);
        assert_eq!(*app.world().get::<Gait>(e).unwrap(), Gait::Sprinting);
    }

    #[test]
    fn airborne_sprint_release_clears_queued_sprint() {
        let mut app = app_with_plugins();
        let e = spawn_character(
            &mut app,
            Stance::Standing,
            Gait::Running,
            AlsInputCommand {
                sprint_held: true,
                ..Default::default()
            },
        );
        *app.world_mut().get_mut::<LocomotionState>(e).unwrap() = LocomotionState::Air;

        app.update();
        assert_eq!(
            app.world().get::<AlsStateRequest>(e).unwrap().gait,
            Some(Gait::Sprinting),
            "sprint in air queues like ALS desired-gait persistence"
        );

        app.world_mut().get_mut::<AlsInputCommand>(e).unwrap().sprint_held = false;
        app.update();
        assert_eq!(
            app.world().get::<AlsStateRequest>(e).unwrap().gait,
            None,
            "release while airborne must replace queued sprint with running; same-value request is idempotent-accept and clears the slot"
        );

        *app.world_mut().get_mut::<LocomotionState>(e).unwrap() = LocomotionState::Grounded;
        app.update();
        assert_eq!(*app.world().get::<Gait>(e).unwrap(), Gait::Running);
        assert_eq!(app.world().get::<AlsStateRequest>(e).unwrap().gait, None);
    }
}
