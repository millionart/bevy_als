use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_enhanced_input::prelude::Press as EIPress;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct MoveAction;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct LookAction;

#[derive(InputAction)]
#[action_output(bool)]
pub struct JumpAction;

#[derive(InputAction)]
#[action_output(bool)]
pub struct CrouchAction;

#[derive(InputAction)]
#[action_output(bool)]
pub struct SprintAction;

#[derive(InputAction)]
#[action_output(bool)]
pub struct AimAction;

#[derive(InputAction)]
#[action_output(bool)]
pub struct RollAction;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OnFoot;

#[derive(
    Component, Debug, Default, Clone, Copy, PartialEq, Reflect,
)]
#[reflect(Default)]
pub struct AlsInputCommand {
    pub move_dir: Vec2,
    pub look_delta: Vec2,
    pub jump_held: bool,
    pub jump_pressed: bool,
    pub crouch_pressed: bool,
    pub roll_pressed: bool,
    pub sprint_held: bool,
    pub aim_held: bool,
}

impl AlsInputCommand {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone)]
pub struct AlsInputMap {
    pub move_north: Binding,
    pub move_south: Binding,
    pub move_west: Binding,
    pub move_east: Binding,
    pub move_stick: [Binding; 2],
    pub look_mouse: Binding,
    pub look_stick: [Binding; 2],
    pub jump: Vec<Binding>,
    pub crouch: Vec<Binding>,
    pub sprint: Vec<Binding>,
    pub aim: Vec<Binding>,
    pub roll: Vec<Binding>,
}

impl Default for AlsInputMap {
    fn default() -> Self {
        Self {
            move_north: KeyCode::KeyW.into(),
            move_south: KeyCode::KeyS.into(),
            move_west: KeyCode::KeyA.into(),
            move_east: KeyCode::KeyD.into(),
            move_stick: [
                GamepadAxis::LeftStickX.into(),
                GamepadAxis::LeftStickY.into(),
            ],
            look_mouse: Binding::mouse_motion(),
            look_stick: [
                GamepadAxis::RightStickX.into(),
                GamepadAxis::RightStickY.into(),
            ],
            jump: vec![KeyCode::Space.into(), GamepadButton::South.into()],
            crouch: vec![
                KeyCode::ControlLeft.into(),
                GamepadButton::RightTrigger.into(),
            ],
            sprint: vec![KeyCode::ShiftLeft.into(), GamepadButton::LeftThumb.into()],
            aim: vec![
                MouseButton::Right.into(),
                GamepadButton::LeftTrigger2.into(),
            ],
            roll: vec![KeyCode::AltLeft.into(), GamepadButton::East.into()],
        }
    }
}

pub fn spawn_als_actions(cmds: &mut Commands, player: Entity, map: &AlsInputMap) {
    cmds.entity(player).insert((
        OnFoot,
        AlsInputCommand::default(),
        actions!(OnFoot[
            (
                Action::<MoveAction>::new(),
                Bindings::spawn((
                    Bidirectional::new(map.move_east, map.move_west),
                    Bidirectional::new(map.move_north, map.move_south)
                        .with(SwizzleAxis::YXZ),
                    Axial::new(map.move_stick[0], map.move_stick[1])
                        .with(DeadZone::default()),
                )),
            ),
            (
                Action::<LookAction>::new(),
                Bindings::spawn((
                    vec![map.look_mouse],
                    Axial::new(map.look_stick[0], map.look_stick[1])
                        .with(DeadZone::default()),
                )),
            ),
            (
                Action::<JumpAction>::new(),
                Bindings::spawn(map.jump.clone()),
            ),
            (
                Action::<CrouchAction>::new(),
                EIPress::default(),
                Bindings::spawn(map.crouch.clone()),
            ),
            (
                Action::<SprintAction>::new(),
                Bindings::spawn(map.sprint.clone()),
            ),
            (
                Action::<AimAction>::new(),
                Bindings::spawn(map.aim.clone()),
            ),
            (
                Action::<RollAction>::new(),
                EIPress::default(),
                Bindings::spawn(map.roll.clone()),
            ),
        ]),
    ));
}

#[allow(clippy::too_many_arguments)]
pub fn als_input_bridge_system(
    mut receivers: Query<(&mut AlsInputCommand, &Actions<OnFoot>)>,
    moves: Query<&Action<MoveAction>>,
    looks: Query<&Action<LookAction>>,
    jumps: Query<(&TriggerState, &ActionEvents), With<Action<JumpAction>>>,
    sprints: Query<&TriggerState, With<Action<SprintAction>>>,
    aims: Query<&TriggerState, With<Action<AimAction>>>,
    crouches: Query<&ActionEvents, With<Action<CrouchAction>>>,
    rolls: Query<&ActionEvents, With<Action<RollAction>>>,
) {
    for (mut cmd, actions) in &mut receivers {
        cmd.reset();

        if let Some(mv) = moves.iter_many(actions).next() {
            cmd.move_dir = **mv;
        }

        if let Some(lk) = looks.iter_many(actions).next() {
            cmd.look_delta = **lk;
        }

        if let Some((state, events)) = jumps.iter_many(actions).next() {
            cmd.jump_held = *state == TriggerState::Fired;
            cmd.jump_pressed = events.contains(ActionEvents::START);
        }

        if let Some(state) = sprints.iter_many(actions).next() {
            cmd.sprint_held = *state == TriggerState::Fired;
        }

        if let Some(state) = aims.iter_many(actions).next() {
            cmd.aim_held = *state == TriggerState::Fired;
        }

        if let Some(events) = crouches.iter_many(actions).next() {
            cmd.crouch_pressed = events.contains(ActionEvents::START);
        }

        if let Some(events) = rolls.iter_many(actions).next() {
            cmd.roll_pressed = events.contains(ActionEvents::START);
        }
    }
}

pub struct AlsInputPlugin;

impl Plugin for AlsInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_input_context::<OnFoot>().add_systems(
            PreUpdate,
            als_input_bridge_system.after(EnhancedInputSystems::Apply),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{ecs::system::RunSystemOnce, MinimalPlugins};

    #[test]
    fn default_command_is_zeroed() {
        let cmd = AlsInputCommand::default();
        assert_eq!(cmd.move_dir, Vec2::ZERO);
        assert_eq!(cmd.look_delta, Vec2::ZERO);
        assert!(!cmd.jump_held);
        assert!(!cmd.jump_pressed);
        assert!(!cmd.crouch_pressed);
        assert!(!cmd.roll_pressed);
        assert!(!cmd.sprint_held);
        assert!(!cmd.aim_held);
    }

    #[test]
    fn reset_clears_continuous_and_edge_fields() {
        let mut cmd = AlsInputCommand {
            move_dir: Vec2::ONE,
            look_delta: Vec2::NEG_ONE,
            jump_held: true,
            jump_pressed: true,
            crouch_pressed: true,
            roll_pressed: true,
            sprint_held: true,
            aim_held: true,
        };
        cmd.reset();
        assert_eq!(
            cmd,
            AlsInputCommand::default(),
            "reset should produce the default value"
        );
    }

    #[test]
    fn bridge_tolerates_missing_actions_and_entities() {
        let mut world = World::new();

        world.run_system_once(als_input_bridge_system).unwrap();

        let player = world
            .spawn((
                AlsInputCommand {
                    move_dir: Vec2::ONE,
                    jump_pressed: true,
                    sprint_held: true,
                    ..Default::default()
                },
                Actions::<OnFoot>::default(),
            ))
            .id();
        world.run_system_once(als_input_bridge_system).unwrap();

        let cmd = world.get::<AlsInputCommand>(player).unwrap();
        assert_eq!(*cmd, AlsInputCommand::default());
    }

    #[test]
    fn spawn_creates_seven_actions_with_bindings() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(EnhancedInputPlugin)
            .add_plugins(AlsInputPlugin);
        app.finish();

        let player = app.world_mut().spawn_empty().id();
        let mut cmds = app.world_mut().commands();
        spawn_als_actions(&mut cmds, player, &AlsInputMap::default());
        app.update();

        let world = app.world_mut();
        assert_eq!(world.get::<Actions<OnFoot>>(player).unwrap().len(), 7);

        for action in world.get::<Actions<OnFoot>>(player).unwrap().iter() {
            let bindings = world.get::<Bindings>(action).unwrap();
            assert!(!bindings.is_empty(), "action {action:?} should have bindings");
        }

        let mut cmds = app.world_mut().commands();
        spawn_als_actions(&mut cmds, player, &AlsInputMap::default());
        app.update();
    }

    #[test]
    fn full_chain_from_action_mock_to_command() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(EnhancedInputPlugin)
            .add_plugins(AlsInputPlugin);
        app.finish();

        let player = app.world_mut().spawn_empty().id();
        let mut cmds = app.world_mut().commands();
        spawn_als_actions(&mut cmds, player, &AlsInputMap::default());
        app.update();

        app.world_mut()
            .commands()
            .entity(player)
            .mock_once::<OnFoot, JumpAction>(TriggerState::Fired, true)
            .mock_once::<OnFoot, CrouchAction>(TriggerState::Fired, true)
            .mock_once::<OnFoot, RollAction>(TriggerState::Fired, true)
            .mock::<OnFoot, SprintAction>(
                TriggerState::Fired,
                true,
                MockSpan::Updates(50),
            )
            .mock::<OnFoot, AimAction>(
                TriggerState::Fired,
                true,
                MockSpan::Updates(50),
            )
            .mock::<OnFoot, MoveAction>(
                TriggerState::Fired,
                Vec2::new(1.0, 0.5),
                MockSpan::Updates(50),
            );

        app.update();
        let cmd = *app.world().get::<AlsInputCommand>(player).unwrap();
        assert!(cmd.jump_pressed && cmd.jump_held, "jump pressed and held");
        assert!(cmd.crouch_pressed, "crouch toggle pressed");
        assert!(cmd.roll_pressed, "roll pressed");
        assert!(cmd.sprint_held, "sprint held");
        assert!(cmd.aim_held, "aim held");
        assert!(
            (cmd.move_dir - Vec2::new(1.0, 0.5)).length() < 1e-5,
            "move_dir should equal mocked value, got {:?}",
            cmd.move_dir
        );

        app.update();
        let cmd = *app.world().get::<AlsInputCommand>(player).unwrap();
        assert!(!cmd.jump_pressed);
        assert!(!cmd.jump_held);
        assert!(!cmd.crouch_pressed);
        assert!(!cmd.roll_pressed);
        assert!(cmd.sprint_held);
        assert!(cmd.aim_held);
        assert!(
            (cmd.move_dir - Vec2::new(1.0, 0.5)).length() < 1e-5,
            "move_dir should persist within hold span"
        );

        let actions = app
            .world()
            .get::<Actions<OnFoot>>(player)
            .unwrap()
            .iter()
            .collect::<Vec<_>>();
        for action in actions {
            if let Some(mut mock) = app.world_mut().get_mut::<ActionMock>(action) {
                mock.enabled = false;
            }
        }
        app.update();
        let cmd = *app.world().get::<AlsInputCommand>(player).unwrap();
        assert_eq!(cmd, AlsInputCommand::default());
    }
}
