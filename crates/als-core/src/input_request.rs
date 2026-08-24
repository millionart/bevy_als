use bevy::prelude::*;

use crate::aim::AlsLastFreeRotationMode;
use crate::character::AlsCharacter;
use crate::input::AlsInputCommand;
use crate::motion::AlsInputOverride;
use crate::state::{Gait, LocomotionState, RotationMode, Stance};
use crate::transition::{state_transition_system, AlsStateRequest};

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsFirstPersonView;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsCameraLeftShoulder;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsSprintPrev(pub bool);

pub fn input_state_request_system(
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &Stance,
            &Gait,
            &LocomotionState,
            &RotationMode,
            Option<&mut AlsLastFreeRotationMode>,
            Has<AlsFirstPersonView>,
            Has<AlsCameraLeftShoulder>,
            Option<&mut AlsSprintPrev>,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            &mut AlsStateRequest,
        ),
        With<AlsCharacter>,
    >,
) {
    for (
        entity,
        stance,
        gait,
        loco,
        rotation_mode,
        mut last_free,
        first_person,
        left_shoulder,
        mut sprint_prev,
        cmd,
        ovr,
        mut request,
    ) in &mut q
    {
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

        let sprint_now = effective.sprint_held;
        let sprint_was = sprint_prev.as_deref().map(|s| s.0).unwrap_or(false);
        if sprint_now && !sprint_was {
            request.gait = Some(Gait::Sprinting);
        } else if !sprint_now && sprint_was {
            request.gait = Some(Gait::Running);
        }
        match sprint_prev {
            Some(mut sp) => sp.0 = sprint_now,
            None => {
                commands.entity(entity).insert(AlsSprintPrev(sprint_now));
            }
        }

        if effective.walk_pressed {
            let base = match gait {
                Gait::Walking => Gait::Running,
                _ => Gait::Walking,
            };
            if *loco == LocomotionState::Grounded || request.gait.is_some() {
                request.gait = Some(base);
            }
        }

        if effective.ragdoll_pressed {
            let target = match loco {
                LocomotionState::Ragdolling => LocomotionState::Grounded,
                _ => LocomotionState::Ragdolling,
            };
            request.locomotion_state = Some(target);
        }

        if effective.rotation_mode_pressed {
            let stored = last_free
                .as_deref()
                .map(|lf| lf.0)
                .unwrap_or(*rotation_mode);
            let free = if stored == RotationMode::ViewDirection {
                RotationMode::VelocityDirection
            } else {
                RotationMode::ViewDirection
            };
            match last_free.as_deref_mut() {
                Some(lf) => lf.0 = free,
                None => {
                    commands.entity(entity).insert(AlsLastFreeRotationMode(free));
                }
            }
            if *rotation_mode != RotationMode::Aiming {
                request.rotation_mode = Some(free);
            }
        }

        if effective.view_mode_pressed {
            if first_person {
                commands.entity(entity).remove::<AlsFirstPersonView>();
            } else {
                commands.entity(entity).insert(AlsFirstPersonView);
            }
        }

        if effective.switch_shoulder_pressed {
            if left_shoulder {
                commands.entity(entity).remove::<AlsCameraLeftShoulder>();
            } else {
                commands.entity(entity).insert(AlsCameraLeftShoulder);
            }
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
            "release edge overwrites queued sprint with running; same-value accept clears the slot same frame"
        );

        *app.world_mut().get_mut::<LocomotionState>(e).unwrap() = LocomotionState::Grounded;
        app.update();
        assert_eq!(*app.world().get::<Gait>(e).unwrap(), Gait::Running);
        assert_eq!(app.world().get::<AlsStateRequest>(e).unwrap().gait, None);
    }

    #[test]
    fn walk_toggle_switches_between_walking_and_running() {
        let mut app = app_with_plugins();
        let e = spawn_character(
            &mut app,
            Stance::Standing,
            Gait::Running,
            AlsInputCommand {
                walk_pressed: true,
                ..Default::default()
            },
        );

        app.update();
        assert_eq!(*app.world().get::<Gait>(e).unwrap(), Gait::Walking);

        app.world_mut().get_mut::<AlsInputCommand>(e).unwrap().walk_pressed = false;
        app.update();
        assert_eq!(*app.world().get::<Gait>(e).unwrap(), Gait::Walking);

        app.world_mut().get_mut::<AlsInputCommand>(e).unwrap().walk_pressed = true;
        app.update();
        assert_eq!(*app.world().get::<Gait>(e).unwrap(), Gait::Running);
    }

    #[test]
    fn ragdoll_toggle_requests_enter_and_exit() {
        let mut app = app_with_plugins();
        let e = spawn_character(
            &mut app,
            Stance::Standing,
            Gait::Running,
            AlsInputCommand {
                ragdoll_pressed: true,
                ..Default::default()
            },
        );

        app.update();
        assert_eq!(
            *app.world().get::<LocomotionState>(e).unwrap(),
            LocomotionState::Ragdolling
        );

        app.world_mut().get_mut::<AlsInputCommand>(e).unwrap().ragdoll_pressed = false;
        app.update();

        app.world_mut().get_mut::<AlsInputCommand>(e).unwrap().ragdoll_pressed = true;
        app.update();
        assert_eq!(
            *app.world().get::<LocomotionState>(e).unwrap(),
            LocomotionState::Grounded,
            "second press while ragdolled requests recovery"
        );
    }

    #[test]
    fn rotation_mode_toggle_flips_free_mode_and_updates_last_free() {
        let mut app = app_with_plugins();
        let e = spawn_character(
            &mut app,
            Stance::Standing,
            Gait::Running,
            AlsInputCommand {
                rotation_mode_pressed: true,
                ..Default::default()
            },
        );

        app.update();
        assert_eq!(
            *app.world().get::<RotationMode>(e).unwrap(),
            RotationMode::ViewDirection
        );
        assert_eq!(
            app.world().get::<AlsLastFreeRotationMode>(e).unwrap().0,
            RotationMode::ViewDirection
        );

        app.world_mut().get_mut::<AlsInputCommand>(e).unwrap().rotation_mode_pressed = false;
        app.update();
        app.world_mut().get_mut::<AlsInputCommand>(e).unwrap().rotation_mode_pressed = true;
        app.update();
        assert_eq!(
            *app.world().get::<RotationMode>(e).unwrap(),
            RotationMode::VelocityDirection
        );
    }

    #[test]
    fn rotation_mode_toggle_while_aiming_only_updates_last_free() {
        let mut app = app_with_plugins();
        let e = spawn_character(
            &mut app,
            Stance::Standing,
            Gait::Running,
            AlsInputCommand {
                rotation_mode_pressed: true,
                ..Default::default()
            },
        );
        *app.world_mut().get_mut::<RotationMode>(e).unwrap() = RotationMode::Aiming;

        app.update();
        assert_eq!(
            *app.world().get::<RotationMode>(e).unwrap(),
            RotationMode::Aiming,
            "aiming rotation must not be interrupted by the free-mode toggle"
        );
        assert_eq!(
            app.world().get::<AlsLastFreeRotationMode>(e).unwrap().0,
            RotationMode::ViewDirection,
            "stored free mode still flips for later aim release"
        );
    }

    #[test]
    fn view_mode_and_shoulder_toggles_flip_components() {
        let mut app = app_with_plugins();
        let e = spawn_character(
            &mut app,
            Stance::Standing,
            Gait::Running,
            AlsInputCommand {
                view_mode_pressed: true,
                switch_shoulder_pressed: true,
                ..Default::default()
            },
        );

        app.update();
        assert!(app.world().get::<AlsFirstPersonView>(e).is_some());
        assert!(app.world().get::<AlsCameraLeftShoulder>(e).is_some());

        for field in ["view_mode_pressed", "switch_shoulder_pressed"] {
            let mut cmd = app.world_mut().get_mut::<AlsInputCommand>(e).unwrap();
            if field == "view_mode_pressed" {
                cmd.view_mode_pressed = false;
            } else {
                cmd.switch_shoulder_pressed = false;
            }
        }
        app.update();

        app.world_mut().get_mut::<AlsInputCommand>(e).unwrap().view_mode_pressed = true;
        app.world_mut().get_mut::<AlsInputCommand>(e).unwrap().switch_shoulder_pressed = true;
        app.update();
        assert!(app.world().get::<AlsFirstPersonView>(e).is_none());
        assert!(app.world().get::<AlsCameraLeftShoulder>(e).is_none());
    }
}
