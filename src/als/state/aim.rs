use bevy::ecs::message::Message;
use bevy::prelude::*;

use crate::als::gait::AlsMaxAllowedGait;
use crate::als::input::AlsInputCommand;
use crate::als::state::{Gait, LocomotionState, RotationMode, Stance};
use crate::als::transition::{Dimension, StateTransitionEvent, StateValue};

pub const SPRINT_HAS_PRIORITY_OVER_AIMING: bool = true;
pub const ROTATE_TO_VELOCITY_WHEN_SPRINTING: bool = true;
pub const ALLOW_AIMING_IN_AIR: bool = true;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsLastFreeRotationMode(pub RotationMode);

#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct AimStateChanged {
    pub entity: Entity,
    pub aiming: bool,
}

pub fn aim_rotation_mode_system(
    time: Res<Time<Virtual>>,
    mut events: MessageWriter<AimStateChanged>,
    mut transitions: MessageWriter<StateTransitionEvent>,
    mut q: Query<
        (
            Entity,
            &AlsInputCommand,
            Option<&crate::als::motion::AlsInputOverride>,
            &AlsMaxAllowedGait,
            &Stance,
            &LocomotionState,
            Has<crate::als::input_request::AlsFirstPersonView>,
            &mut RotationMode,
            Option<&mut AlsLastFreeRotationMode>,
        ),
        (
            With<crate::als::character::AlsCharacter>,
            Without<crate::als::character::AlsBirthAnimationPending>,
        ),
    >,
    mut commands: Commands,
) {
    if time.is_paused() {
        return;
    }
    for (entity, cmd, ovr, max_allowed, _, loco, first_person, mut current, last_free) in &mut q {
        let free = last_free.as_deref().map(|lf| lf.0).unwrap_or_else(|| {
            if *current == RotationMode::Aiming {
                RotationMode::ViewDirection
            } else {
                *current
            }
        });
        let want_aim =
            crate::als::motion::effective_input(cmd, ovr).aim_held || free == RotationMode::Aiming;
        if last_free.is_none() && *current != RotationMode::Aiming {
            commands
                .entity(entity)
                .insert(AlsLastFreeRotationMode(free));
        }

        let sprinting = max_allowed.0 == Gait::Sprinting;
        let desired = if *loco == LocomotionState::Air {
            if want_aim && ALLOW_AIMING_IN_AIR {
                RotationMode::Aiming
            } else if first_person {
                RotationMode::ViewDirection
            } else {
                free
            }
        } else if first_person {
            if want_aim && (!sprinting || !SPRINT_HAS_PRIORITY_OVER_AIMING) {
                RotationMode::Aiming
            } else {
                RotationMode::ViewDirection
            }
        } else if sprinting {
            if want_aim && !SPRINT_HAS_PRIORITY_OVER_AIMING {
                RotationMode::Aiming
            } else if ROTATE_TO_VELOCITY_WHEN_SPRINTING {
                RotationMode::VelocityDirection
            } else {
                free
            }
        } else if want_aim {
            RotationMode::Aiming
        } else {
            free
        };

        if *current != desired {
            let previous = *current;
            *current = desired;
            transitions.write(StateTransitionEvent {
                entity,
                dimension: Dimension::RotationMode,
                from: StateValue::RotationMode(previous),
                to: StateValue::RotationMode(desired),
            });
            events.write(AimStateChanged {
                entity,
                aiming: desired == RotationMode::Aiming,
            });
        }
    }
}

pub struct AlsAimPlugin;

impl Plugin for AlsAimPlugin {
    fn build(&self, app: &mut App) {
        let outer = crate::AlsPhysicsSchedule::outer(app);
        app.add_message::<AimStateChanged>()
            .add_message::<StateTransitionEvent>()
            .add_systems(
                outer,
                aim_rotation_mode_system
                    .after(crate::als::gait::refresh_gait_system)
                    .before(crate::als::motion::AlsRotationResponseSet)
                    .before(crate::gait_layer::AlsGaitLayerSet),
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
    use crate::als::character::AlsCharacter;
    use crate::als::motion::AlsInputOverride;
    use crate::als::state::{Gait, LocomotionState, OverlayState, Stance};
    use crate::als::transition::AlsStateRequest;
    use crate::als::transition::AlsStateTransitionPlugin;
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
        app.add_systems(PostUpdate, collect_events.after(aim_rotation_mode_system));
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
        let (mut app, e) = app_with_character(true, RotationMode::ViewDirection);
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
    fn aim_button_preserves_the_saved_rotation_intent() {
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
        assert_eq!(
            stored.0,
            RotationMode::ViewDirection,
            "Aim button changes must preserve the saved rotation intent"
        );
    }

    #[test]
    fn desired_aiming_intent_survives_button_release_and_source_mode_branches() {
        let cases = [
            (
                LocomotionState::Grounded,
                Stance::Standing,
                false,
                Gait::Running,
                RotationMode::Aiming,
            ),
            (
                LocomotionState::Grounded,
                Stance::Crouching,
                false,
                Gait::Running,
                RotationMode::Aiming,
            ),
            (
                LocomotionState::Air,
                Stance::Standing,
                false,
                Gait::Running,
                RotationMode::Aiming,
            ),
            (
                LocomotionState::Air,
                Stance::Crouching,
                false,
                Gait::Running,
                RotationMode::Aiming,
            ),
            (
                LocomotionState::Grounded,
                Stance::Standing,
                true,
                Gait::Running,
                RotationMode::Aiming,
            ),
            (
                LocomotionState::Air,
                Stance::Standing,
                true,
                Gait::Running,
                RotationMode::Aiming,
            ),
            (
                LocomotionState::Grounded,
                Stance::Standing,
                false,
                Gait::Sprinting,
                RotationMode::VelocityDirection,
            ),
            (
                LocomotionState::Grounded,
                Stance::Standing,
                true,
                Gait::Sprinting,
                RotationMode::ViewDirection,
            ),
            (
                LocomotionState::Grounded,
                Stance::Crouching,
                false,
                Gait::Sprinting,
                RotationMode::VelocityDirection,
            ),
            (
                LocomotionState::Air,
                Stance::Standing,
                false,
                Gait::Sprinting,
                RotationMode::Aiming,
            ),
            (
                LocomotionState::Custom,
                Stance::Standing,
                false,
                Gait::Sprinting,
                RotationMode::VelocityDirection,
            ),
            (
                LocomotionState::Custom,
                Stance::Crouching,
                true,
                Gait::Sprinting,
                RotationMode::ViewDirection,
            ),
            (
                LocomotionState::None,
                Stance::Standing,
                false,
                Gait::Sprinting,
                RotationMode::VelocityDirection,
            ),
            (
                LocomotionState::None,
                Stance::Crouching,
                true,
                Gait::Sprinting,
                RotationMode::ViewDirection,
            ),
            (
                LocomotionState::Custom,
                Stance::Standing,
                false,
                Gait::Running,
                RotationMode::Aiming,
            ),
        ];
        for (locomotion, stance, first_person, gait, expected) in cases {
            let (mut app, entity) = app_with_character(false, RotationMode::ViewDirection);
            app.world_mut().entity_mut(entity).insert((
                locomotion,
                stance,
                AlsLastFreeRotationMode(RotationMode::Aiming),
                AlsMaxAllowedGait(gait),
            ));
            if first_person {
                app.world_mut()
                    .entity_mut(entity)
                    .insert(crate::als::input_request::AlsFirstPersonView);
            }
            for aim_held in [false, true, false] {
                app.world_mut()
                    .get_mut::<AlsInputCommand>(entity)
                    .unwrap()
                    .aim_held = aim_held;
                app.update();
                assert_eq!(
                    *app.world().get::<RotationMode>(entity).unwrap(),
                    expected,
                    "locomotion={locomotion:?} stance={stance:?} first_person={first_person} gait={gait:?} button={aim_held}"
                );
                assert_eq!(
                    app.world()
                        .get::<AlsLastFreeRotationMode>(entity)
                        .unwrap()
                        .0,
                    RotationMode::Aiming
                );
            }
        }
    }

    #[test]
    fn retained_sprint_priority_survives_action_modes_and_aim_changes() {
        for mode in [LocomotionState::Custom, LocomotionState::None] {
            for stance in [Stance::Standing, Stance::Crouching] {
                for first_person in [false, true] {
                    let (mut app, entity) = app_with_character(false, RotationMode::ViewDirection);
                    app.world_mut().entity_mut(entity).insert((
                        mode,
                        stance,
                        AlsLastFreeRotationMode(RotationMode::ViewDirection),
                        AlsMaxAllowedGait(Gait::Sprinting),
                    ));
                    if first_person {
                        app.world_mut()
                            .entity_mut(entity)
                            .insert(crate::als::input_request::AlsFirstPersonView);
                    }
                    for aim_held in [false, true, false] {
                        app.world_mut()
                            .get_mut::<AlsInputCommand>(entity)
                            .unwrap()
                            .aim_held = aim_held;
                        app.update();
                        let expected = if first_person {
                            RotationMode::ViewDirection
                        } else {
                            RotationMode::VelocityDirection
                        };
                        assert_eq!(
                            *app.world().get::<RotationMode>(entity).unwrap(),
                            expected,
                            "mode={mode:?} stance={stance:?} first_person={first_person} aim={aim_held}"
                        );
                        assert_eq!(
                            app.world().get::<AlsMaxAllowedGait>(entity).unwrap().0,
                            Gait::Sprinting
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn retained_sprint_rotation_reaches_the_loaded_animation_graph() {
        use crate::als::mantle::{AlsMantleState, MantleKind};
        use crate::extensions::SourceAnimationInputs;
        use avian3d::prelude::Position;
        use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;

        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let origin = app.world().get::<Position>(character).unwrap().0;
        let mut mantle = AlsMantleState::default();
        mantle.begin(
            MantleKind::High,
            origin,
            origin + Vec3::new(0.0, 1.5, -0.8),
            1.5,
            0.0,
            0.0,
        );
        app.world_mut().entity_mut(character).insert((
            mantle,
            LocomotionState::Custom,
            AlsMaxAllowedGait(Gait::Sprinting),
            AlsLastFreeRotationMode(RotationMode::ViewDirection),
            AlsInputCommand {
                aim_held: true,
                ..default()
            },
        ));
        app.update();
        assert!(app.world().get::<AlsMantleState>(character).unwrap().active);
        assert_eq!(
            *app.world().get::<RotationMode>(character).unwrap(),
            RotationMode::VelocityDirection
        );
        let player = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let tag = app
            .world()
            .get::<SourceAnimationInputs>(player)
            .unwrap()
            .gameplay_tag_input("rotation_mode")
            .unwrap();
        assert_eq!(tag.as_str(), "Als.RotationMode.VelocityDirection");
    }

    #[test]
    fn actual_aiming_without_saved_intent_keeps_source_default_intent() {
        let (mut app, entity) = app_with_character(false, RotationMode::Aiming);
        assert_eq!(
            *app.world().get::<RotationMode>(entity).unwrap(),
            RotationMode::ViewDirection
        );
        app.update();
        assert_eq!(
            app.world()
                .get::<AlsLastFreeRotationMode>(entity)
                .unwrap()
                .0,
            RotationMode::ViewDirection
        );
    }

    #[test]
    fn sprint_with_aim_uses_velocity_direction_then_restores_aiming() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsStateTransitionPlugin, AlsAimPlugin));
        let e = spawn_character(&mut app, true, RotationMode::ViewDirection);
        app.world_mut().entity_mut(e).insert(AlsInputCommand {
            aim_held: true,
            sprint_held: true,
            move_dir: Vec2::Y,
            ..Default::default()
        });
        app.world_mut()
            .entity_mut(e)
            .insert(AlsMaxAllowedGait(Gait::Sprinting));
        app.update();
        assert_eq!(
            *app.world().get::<RotationMode>(e).unwrap(),
            RotationMode::VelocityDirection
        );

        app.world_mut()
            .get_mut::<AlsInputCommand>(e)
            .unwrap()
            .sprint_held = false;
        app.world_mut()
            .entity_mut(e)
            .insert(AlsMaxAllowedGait(Gait::Running));
        app.update();
        assert_eq!(
            *app.world().get::<RotationMode>(e).unwrap(),
            RotationMode::Aiming
        );
    }

    #[test]
    fn input_override_selects_the_complete_aim_command() {
        let cases = [
            (false, None, false),
            (true, None, true),
            (false, Some(true), true),
            (true, Some(false), false),
        ];
        for (input_aim, override_aim, expected_aim) in cases {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AlsStateTransitionPlugin, AlsAimPlugin));
            let entity = spawn_character(&mut app, input_aim, RotationMode::ViewDirection);
            if let Some(aim_held) = override_aim {
                app.world_mut()
                    .entity_mut(entity)
                    .insert(AlsInputOverride(AlsInputCommand {
                        aim_held,
                        ..Default::default()
                    }));
            }
            app.update();
            assert_eq!(
                *app.world().get::<RotationMode>(entity).unwrap() == RotationMode::Aiming,
                expected_aim,
                "input_aim={input_aim} override_aim={override_aim:?}"
            );
        }
    }
}
