use bevy::ecs::message::Message;
use bevy::prelude::*;

use crate::input::AlsInputCommand;
use crate::state::RotationMode;
use crate::transition::AlsStateRequest;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsLastFreeRotationMode(pub RotationMode);

#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct AimStateChanged {
    pub entity: Entity,
    pub aiming: bool,
}

pub fn aim_rotation_mode_system(
    mut events: MessageWriter<AimStateChanged>,
    mut q: Query<
        (
            Entity,
            &AlsInputCommand,
            Option<&crate::motion::AlsInputOverride>,
            &RotationMode,
            Option<&mut AlsLastFreeRotationMode>,
            &mut AlsStateRequest,
        ),
        With<crate::character::AlsCharacter>,
    >,
    mut commands: Commands,
) {
    for (entity, cmd, ovr, current, last_free, mut request) in &mut q {
        let base = if ovr.map(|o| o.0.aim_held).unwrap_or(false) {
            let mut c = *cmd;
            c.aim_held = true;
            c
        } else {
            *cmd
        };
        let aiming_now = *current == RotationMode::Aiming;
        let want_aim = base.aim_held;

        if want_aim && !aiming_now {
            let stored = last_free
                .as_deref()
                .map(|lf| lf.0)
                .unwrap_or(*current);
            let free = if stored == RotationMode::Aiming {
                default_free_mode()
            } else {
                stored
            };
            match last_free {
                Some(mut lf) => lf.0 = free,
                None => {
                    commands.entity(entity).insert(AlsLastFreeRotationMode(free));
                }
            }
            request.rotation_mode = Some(RotationMode::Aiming);
            events.write(AimStateChanged {
                entity,
                aiming: true,
            });
        } else if !want_aim && aiming_now {
            let stored = last_free
                .as_deref()
                .map(|lf| lf.0)
                .unwrap_or_else(default_free_mode);
            let free = if stored == RotationMode::Aiming {
                default_free_mode()
            } else {
                stored
            };
            request.rotation_mode = Some(free);
            events.write(AimStateChanged {
                entity,
                aiming: false,
            });
        }
    }
}

const fn default_free_mode() -> RotationMode {
    RotationMode::VelocityDirection
}

pub struct AlsAimPlugin;

impl Plugin for AlsAimPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AimStateChanged>().add_systems(
            Update,
            aim_rotation_mode_system.before(crate::transition::state_transition_system),
        );
    }
}

#[cfg(test)]
#[derive(Resource, Default)]
struct EventLog(Vec<AimStateChanged>);

#[cfg(test)]
fn collect_events(mut reader: MessageReader<AimStateChanged>, mut log: ResMut<EventLog>) {
    for ev in reader.read() {
        log.0.push(*ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::AlsCharacter;
    use crate::state::{Gait, LocomotionState, OverlayState, Stance};
    use crate::transition::AlsStateTransitionPlugin;
    use bevy::MinimalPlugins;

    fn spawn_character(app: &mut App, aim_held: bool, initial: RotationMode) -> Entity {
        let e = app
            .world_mut()
            .spawn((
                AlsCharacter,
                LocomotionState::Grounded,
                Stance::Standing,
                Gait::Walking,
                OverlayState::Default,
                initial,
                AlsStateRequest::default(),
                AlsInputCommand {
                    aim_held,
                    ..Default::default()
                },
            ))
            .id();
        e
    }

    fn app_with_character(aim_held: bool, initial: RotationMode) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsStateTransitionPlugin, AlsAimPlugin));
        app.init_resource::<EventLog>();
        app.add_systems(Update, collect_events);
        let e = spawn_character(&mut app, aim_held, initial);
        app.update();
        (app, e)
    }

    #[test]
    fn aim_input_requests_aiming_mode_and_broadcasts_once() {
        let (mut app, e) = app_with_character(true, RotationMode::VelocityDirection);
        assert_eq!(
            *app.world().get::<RotationMode>(e).unwrap(),
            RotationMode::Aiming
        );
        assert_eq!(app.world().resource::<EventLog>().0.len(), 1);

        app.update();
        assert_eq!(
            app.world().resource::<EventLog>().0.len(),
            1,
            "held aim must not rebroadcast"
        );
    }

    #[test]
    fn release_restores_last_free_mode() {
        let (mut app, e) =
            app_with_character(true, RotationMode::ViewDirection);
        assert_eq!(
            *app.world().get::<RotationMode>(e).unwrap(),
            RotationMode::Aiming
        );
        assert_eq!(
            app.world().get::<AlsLastFreeRotationMode>(e).unwrap().0,
            RotationMode::ViewDirection
        );

        app.world_mut()
            .get_mut::<AlsInputCommand>(e)
            .unwrap()
            .aim_held = false;
        app.update();

        assert_eq!(
            *app.world().get::<RotationMode>(e).unwrap(),
            RotationMode::ViewDirection
        );
        let log = &app.world().resource::<EventLog>().0;
        assert_eq!(log.len(), 2);
        assert!(!log[1].aiming);
    }

    #[test]
    fn last_free_mode_never_stores_aiming() {
        let (mut app, e) = app_with_character(false, RotationMode::ViewDirection);
        app.world_mut()
            .get_mut::<AlsInputCommand>(e)
            .unwrap()
            .aim_held = true;
        app.update();
        app.world_mut()
            .get_mut::<AlsInputCommand>(e)
            .unwrap()
            .aim_held = false;
        app.update();
        assert_eq!(
            *app.world().get::<RotationMode>(e).unwrap(),
            RotationMode::ViewDirection
        );

        let stored = app
            .world()
            .get::<AlsLastFreeRotationMode>(e)
            .copied()
            .unwrap_or(AlsLastFreeRotationMode(RotationMode::VelocityDirection));
        assert_ne!(
            stored.0,
            RotationMode::Aiming,
            "stored free mode must never be Aiming"
        );
    }
}
