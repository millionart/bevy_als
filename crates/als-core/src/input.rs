use bevy::prelude::*;
use bevy_enhanced_input::prelude::Press as EIPress;
use bevy_enhanced_input::prelude::*;

pub const SPRINT_HOLD_SECS: f32 = 0.1;
pub const ROLL_TAP_SECS: f32 = 0.3;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct MoveAction;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct LookAction;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct LookMouseAction;

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

#[derive(InputAction)]
#[action_output(bool)]
pub struct WalkAction;

#[derive(InputAction)]
#[action_output(bool)]
pub struct RagdollAction;

#[derive(InputAction)]
#[action_output(bool)]
pub struct RotationModeAction;

#[derive(InputAction)]
#[action_output(bool)]
pub struct ViewModeAction;

#[derive(InputAction)]
#[action_output(bool)]
pub struct SwitchShoulderAction;

#[derive(InputAction)]
#[action_output(bool)]
pub struct OverlayMenuAction;

#[derive(InputAction)]
#[action_output(f32)]
pub struct NextOverlayModeAction;

#[derive(InputAction)]
#[action_output(f32)]
pub struct PreviousOverlayModeAction;

#[derive(InputAction)]
#[action_output(bool)]
pub struct SlomoAction;

#[derive(InputAction)]
#[action_output(bool)]
pub struct ToggleUiAction;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OnFoot;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsSprintInputLatch(pub bool);

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Reflect)]
#[reflect(Default)]
#[require(AlsSprintInputLatch)]
pub struct AlsInputCommand {
    pub move_dir: Vec2,
    pub look_delta: Vec2,
    pub look_rate: Vec2,
    pub jump_held: bool,
    pub jump_pressed: bool,
    pub crouch_pressed: bool,
    pub roll_pressed: bool,
    pub sprint_held: bool,
    pub aim_held: bool,
    pub walk_pressed: bool,
    pub ragdoll_pressed: bool,
    pub rotation_mode_pressed: bool,
    pub view_mode_pressed: bool,
    pub switch_shoulder_pressed: bool,
    pub overlay_menu_held: bool,
    pub next_overlay_mode_pressed: bool,
    pub previous_overlay_mode_pressed: bool,
    pub slomo_pressed: bool,
    pub toggle_ui_pressed: bool,
}

impl AlsInputCommand {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub fn roll_pressed_from_events(events: ActionEvents) -> bool {
    events.contains(ActionEvents::FIRE)
}

pub fn sprint_held_from(trigger: TriggerState, raw_held: bool, latched: &mut bool) -> bool {
    if trigger == TriggerState::Fired {
        *latched = true;
    }
    if !raw_held {
        *latched = false;
    }
    *latched
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
    pub look_left: Binding,
    pub look_right: Binding,
    pub look_up: Binding,
    pub look_down: Binding,
    pub jump: Vec<Binding>,
    pub crouch: Vec<Binding>,
    pub sprint: Vec<Binding>,
    pub aim: Vec<Binding>,
    pub roll: Vec<Binding>,
    pub walk: Vec<Binding>,
    pub ragdoll: Vec<Binding>,
    pub rotation_mode: Vec<Binding>,
    pub view_mode: Vec<Binding>,
    pub switch_shoulder: Vec<Binding>,
    pub overlay_menu: Vec<Binding>,
    pub slomo: Vec<Binding>,
    pub toggle_ui: Vec<Binding>,
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
            look_left: KeyCode::ArrowLeft.into(),
            look_right: KeyCode::ArrowRight.into(),
            look_up: KeyCode::ArrowUp.into(),
            look_down: KeyCode::ArrowDown.into(),
            jump: vec![KeyCode::Space.into(), GamepadButton::South.into()],
            crouch: vec![KeyCode::ControlLeft.into(), GamepadButton::LeftThumb.into()],
            sprint: vec![KeyCode::ShiftLeft.into(), GamepadButton::East.into()],
            aim: vec![MouseButton::Right.into(), GamepadButton::LeftTrigger.into()],
            roll: vec![KeyCode::ShiftLeft.into(), GamepadButton::East.into()],
            walk: vec![KeyCode::AltLeft.into(), GamepadButton::RightTrigger.into()],
            ragdoll: vec![KeyCode::KeyX.into(), GamepadButton::West.into()],
            rotation_mode: vec![KeyCode::Digit1.into(), GamepadButton::DPadLeft.into()],
            view_mode: vec![KeyCode::Digit2.into(), GamepadButton::DPadRight.into()],
            switch_shoulder: vec![KeyCode::KeyC.into(), GamepadButton::RightThumb.into()],
            overlay_menu: vec![KeyCode::KeyQ.into(), GamepadButton::North.into()],
            slomo: vec![KeyCode::KeyZ.into(), GamepadButton::Select.into()],
            toggle_ui: vec![KeyCode::Tab.into(), GamepadButton::Start.into()],
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
                Action::<LookMouseAction>::new(),
                Bindings::spawn(vec![map.look_mouse]),
            ),
            (
                Action::<LookAction>::new(),
                Bindings::spawn((
                    Bidirectional::new(map.look_right, map.look_left),
                    Bidirectional::new(map.look_down, map.look_up).with(SwizzleAxis::YXZ),
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
                Hold::new(SPRINT_HOLD_SECS).one_shot(true),
                Bindings::spawn(map.sprint.clone()),
            ),
            (
                Action::<AimAction>::new(),
                Bindings::spawn(map.aim.clone()),
            ),
            (
                Action::<RollAction>::new(),
                Tap::new(ROLL_TAP_SECS),
                Bindings::spawn(map.roll.clone()),
            ),
            (
                Action::<WalkAction>::new(),
                EIPress::default(),
                Bindings::spawn(map.walk.clone()),
            ),
            (
                Action::<RagdollAction>::new(),
                EIPress::default(),
                Bindings::spawn(map.ragdoll.clone()),
            ),
            (
                Action::<RotationModeAction>::new(),
                EIPress::default(),
                Bindings::spawn(map.rotation_mode.clone()),
            ),
            (
                Action::<ViewModeAction>::new(),
                EIPress::default(),
                Bindings::spawn(map.view_mode.clone()),
            ),
            (
                Action::<SwitchShoulderAction>::new(),
                EIPress::default(),
                Bindings::spawn(map.switch_shoulder.clone()),
            ),
            (
                Action::<OverlayMenuAction>::new(),
                Down::default(),
                Bindings::spawn(map.overlay_menu.clone()),
            ),
            (
                Action::<NextOverlayModeAction>::new(),
                Down::default(),
                Clamp::pos(),
                bindings![
                    GamepadButton::DPadUp,
                    (Binding::mouse_wheel(), SwizzleAxis::YXZ),
                ],
            ),
            (
                Action::<PreviousOverlayModeAction>::new(),
                Down::default(),
                Clamp::pos(),
                bindings![
                    GamepadButton::DPadDown,
                    (Binding::mouse_wheel(), SwizzleAxis::YXZ, Negate::x()),
                ],
            ),
            (
                Action::<SlomoAction>::new(),
                EIPress::default(),
                Bindings::spawn(map.slomo.clone()),
            ),
            (
                Action::<ToggleUiAction>::new(),
                EIPress::default(),
                Bindings::spawn(map.toggle_ui.clone()),
            ),
        ]),
    ));
}

#[allow(clippy::too_many_arguments)]
pub fn als_input_bridge_system(
    mut receivers: Query<(
        &mut AlsInputCommand,
        &Actions<OnFoot>,
        &mut AlsSprintInputLatch,
    )>,
    keys: Res<ButtonInput<KeyCode>>,
    moves: Query<&Action<MoveAction>>,
    mouse_looks: Query<&Action<LookMouseAction>>,
    looks: Query<&Action<LookAction>>,
    jumps: Query<(&TriggerState, &ActionEvents), With<Action<JumpAction>>>,
    sprints: Query<(&TriggerState, &Action<SprintAction>), With<Action<SprintAction>>>,
    aims: Query<&TriggerState, With<Action<AimAction>>>,
    crouches: Query<&ActionEvents, With<Action<CrouchAction>>>,
    rolls: Query<&ActionEvents, With<Action<RollAction>>>,
    walks: Query<&ActionEvents, With<Action<WalkAction>>>,
    ragdolls: Query<&ActionEvents, With<Action<RagdollAction>>>,
    rotation_modes: Query<&ActionEvents, With<Action<RotationModeAction>>>,
    view_modes: Query<&ActionEvents, With<Action<ViewModeAction>>>,
    switch_shoulders: Query<&ActionEvents, With<Action<SwitchShoulderAction>>>,
    mut ui_actions: (
        Query<(&TriggerState, &Action<OverlayMenuAction>)>,
        Query<&TriggerState, With<Action<NextOverlayModeAction>>>,
        Query<&TriggerState, With<Action<PreviousOverlayModeAction>>>,
        Query<&ActionEvents, With<Action<SlomoAction>>>,
        Query<&ActionEvents, With<Action<ToggleUiAction>>>,
    ),
) {
    for (mut cmd, actions, mut sprint_latch) in &mut receivers {
        cmd.reset();

        if let Some(mv) = moves.iter_many(actions).next() {
            cmd.move_dir = **mv;
        }

        if let Some(mouse) = mouse_looks.iter_many(actions).next() {
            cmd.look_delta = **mouse;
        }

        if let Some(lk) = looks.iter_many(actions).next() {
            cmd.look_rate = **lk;
        }

        if let Some((state, events)) = jumps.iter_many(actions).next() {
            cmd.jump_held = *state == TriggerState::Fired;
            cmd.jump_pressed = events.contains(ActionEvents::START);
        }

        if let Some((state, action)) = sprints.iter_many(actions).next() {
            let raw_held = **action;
            cmd.sprint_held = sprint_held_from(*state, raw_held, &mut sprint_latch.0);
        }

        if let Some(state) = aims.iter_many(actions).next() {
            cmd.aim_held = *state == TriggerState::Fired;
        }

        if let Some(events) = crouches.iter_many(actions).next() {
            cmd.crouch_pressed = events.contains(ActionEvents::START);
        }

        if let Some(events) = rolls.iter_many(actions).next() {
            cmd.roll_pressed = roll_pressed_from_events(*events);
        }

        if let Some(events) = walks.iter_many(actions).next() {
            cmd.walk_pressed = events.contains(ActionEvents::START);
        }

        if let Some(events) = ragdolls.iter_many(actions).next() {
            cmd.ragdoll_pressed = events.contains(ActionEvents::START);
        }

        if let Some(events) = rotation_modes.iter_many(actions).next() {
            cmd.rotation_mode_pressed = events.contains(ActionEvents::START);
        }

        if let Some(events) = view_modes.iter_many(actions).next() {
            cmd.view_mode_pressed = events.contains(ActionEvents::START);
        }

        if let Some(events) = switch_shoulders.iter_many(actions).next() {
            cmd.switch_shoulder_pressed = events.contains(ActionEvents::START);
        }

        let (overlay_menus, next_overlays, previous_overlays, slomos, toggle_uis) = &mut ui_actions;

        if let Some((state, _)) = overlay_menus.iter_many(actions).next() {
            cmd.overlay_menu_held = *state == TriggerState::Fired;
        }

        if let Some(state) = next_overlays.iter_many(actions).next() {
            cmd.next_overlay_mode_pressed = *state == TriggerState::Fired;
        }

        if let Some(state) = previous_overlays.iter_many(actions).next() {
            cmd.previous_overlay_mode_pressed = *state == TriggerState::Fired;
        }

        if let Some(events) = slomos.iter_many(actions).next() {
            cmd.slomo_pressed = events.contains(ActionEvents::START);
        }

        if let Some(events) = toggle_uis.iter_many(actions).next() {
            cmd.toggle_ui_pressed = events.contains(ActionEvents::START);
        }

        let shift_held = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        if shift_held {
            cmd.rotation_mode_pressed = false;
            cmd.view_mode_pressed = false;
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
    use bevy::{MinimalPlugins, ecs::system::RunSystemOnce};

    #[test]
    fn default_command_is_zeroed() {
        let cmd = AlsInputCommand::default();
        assert_eq!(cmd.move_dir, Vec2::ZERO);
        assert_eq!(cmd.look_delta, Vec2::ZERO);
        assert_eq!(cmd.look_rate, Vec2::ZERO);
        assert!(!cmd.jump_held);
        assert!(!cmd.jump_pressed);
        assert!(!cmd.crouch_pressed);
        assert!(!cmd.roll_pressed);
        assert!(!cmd.sprint_held);
        assert!(!cmd.aim_held);
        assert!(!cmd.walk_pressed);
        assert!(!cmd.ragdoll_pressed);
        assert!(!cmd.rotation_mode_pressed);
        assert!(!cmd.view_mode_pressed);
        assert!(!cmd.switch_shoulder_pressed);
        assert!(!cmd.overlay_menu_held);
        assert!(!cmd.next_overlay_mode_pressed);
        assert!(!cmd.previous_overlay_mode_pressed);
        assert!(!cmd.slomo_pressed);
        assert!(!cmd.toggle_ui_pressed);
    }

    #[test]
    fn reset_clears_continuous_and_edge_fields() {
        let mut cmd = AlsInputCommand {
            move_dir: Vec2::ONE,
            look_delta: Vec2::NEG_ONE,
            look_rate: Vec2::ONE,
            jump_held: true,
            jump_pressed: true,
            crouch_pressed: true,
            roll_pressed: true,
            sprint_held: true,
            aim_held: true,
            walk_pressed: true,
            ragdoll_pressed: true,
            rotation_mode_pressed: true,
            view_mode_pressed: true,
            switch_shoulder_pressed: true,
            overlay_menu_held: true,
            next_overlay_mode_pressed: true,
            previous_overlay_mode_pressed: true,
            slomo_pressed: true,
            toggle_ui_pressed: true,
        };
        cmd.reset();
        assert_eq!(
            cmd,
            AlsInputCommand::default(),
            "reset should produce the default value"
        );
    }

    #[test]
    fn roll_requires_the_tap_fire_event_not_the_initial_press_event() {
        let press = ActionEvents::START | ActionEvents::ONGOING;
        let release = ActionEvents::FIRE;
        assert!(!roll_pressed_from_events(press));
        assert!(roll_pressed_from_events(release));
    }

    #[test]
    fn sprint_latch_persists_after_one_shot_hold_and_clears_on_release() {
        let mut latched = false;
        assert!(!sprint_held_from(TriggerState::Ongoing, true, &mut latched));
        assert!(sprint_held_from(TriggerState::Fired, true, &mut latched));
        assert!(sprint_held_from(TriggerState::None, true, &mut latched));
        assert!(!sprint_held_from(TriggerState::None, false, &mut latched));
    }

    #[test]
    fn bridge_tolerates_missing_actions_and_entities() {
        let mut world = World::new();
        world.insert_resource(ButtonInput::<KeyCode>::default());

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
    fn spawn_creates_eighteen_actions_with_bindings() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(EnhancedInputPlugin)
            .add_plugins(AlsInputPlugin)
            .insert_resource(ButtonInput::<KeyCode>::default());
        app.finish();

        let player = app.world_mut().spawn_empty().id();
        let mut cmds = app.world_mut().commands();
        spawn_als_actions(&mut cmds, player, &AlsInputMap::default());
        app.update();

        let world = app.world_mut();
        assert_eq!(world.get::<Actions<OnFoot>>(player).unwrap().len(), 18);

        for action in world.get::<Actions<OnFoot>>(player).unwrap().iter() {
            let bindings = world.get::<Bindings>(action).unwrap();
            assert!(
                !bindings.is_empty(),
                "action {action:?} should have bindings"
            );
        }

        let (sprint_one_shot, sprint_secs) = {
            let sprint = world
                .query_filtered::<&Hold, With<Action<SprintAction>>>()
                .single(world)
                .unwrap();
            (sprint.one_shot, sprint.timer().duration().as_secs_f32())
        };
        let roll_secs = {
            let roll = world
                .query_filtered::<&Tap, With<Action<RollAction>>>()
                .single(world)
                .unwrap();
            roll.timer().duration().as_secs_f32()
        };
        let next_actuation = {
            let next_overlay = world
                .query_filtered::<&Down, With<Action<NextOverlayModeAction>>>()
                .single(world)
                .unwrap();
            next_overlay.actuation
        };
        let previous_actuation = {
            let previous_overlay = world
                .query_filtered::<&Down, With<Action<PreviousOverlayModeAction>>>()
                .single(world)
                .unwrap();
            previous_overlay.actuation
        };
        assert!(sprint_one_shot);
        assert!((sprint_secs - SPRINT_HOLD_SECS).abs() < 1e-6);
        assert!((roll_secs - ROLL_TAP_SECS).abs() < 1e-6);
        assert!((next_actuation - 0.5).abs() < f32::EPSILON);
        assert!((previous_actuation - 0.5).abs() < f32::EPSILON);

        let mut cmds = app.world_mut().commands();
        spawn_als_actions(&mut cmds, player, &AlsInputMap::default());
        app.update();
    }

    #[test]
    fn default_input_map_matches_the_als_context() {
        let map = AlsInputMap::default();
        assert_eq!(map.move_north, KeyCode::KeyW.into());
        assert_eq!(map.move_south, KeyCode::KeyS.into());
        assert_eq!(map.move_west, KeyCode::KeyA.into());
        assert_eq!(map.move_east, KeyCode::KeyD.into());
        assert_eq!(
            map.move_stick,
            [
                GamepadAxis::LeftStickX.into(),
                GamepadAxis::LeftStickY.into()
            ]
        );
        assert_eq!(map.look_mouse, Binding::mouse_motion());
        assert_eq!(
            map.look_stick,
            [
                GamepadAxis::RightStickX.into(),
                GamepadAxis::RightStickY.into()
            ]
        );
        assert_eq!(map.look_left, KeyCode::ArrowLeft.into());
        assert_eq!(map.look_right, KeyCode::ArrowRight.into());
        assert_eq!(map.look_up, KeyCode::ArrowUp.into());
        assert_eq!(map.look_down, KeyCode::ArrowDown.into());
        assert_eq!(
            map.jump,
            vec![KeyCode::Space.into(), GamepadButton::South.into()]
        );
        assert_eq!(
            map.crouch,
            vec![KeyCode::ControlLeft.into(), GamepadButton::LeftThumb.into()]
        );
        assert_eq!(
            map.sprint,
            vec![KeyCode::ShiftLeft.into(), GamepadButton::East.into()]
        );
        assert_eq!(
            map.aim,
            vec![MouseButton::Right.into(), GamepadButton::LeftTrigger.into()]
        );
        assert_eq!(
            map.roll,
            vec![KeyCode::ShiftLeft.into(), GamepadButton::East.into()]
        );
        assert_eq!(
            map.walk,
            vec![KeyCode::AltLeft.into(), GamepadButton::RightTrigger.into()]
        );
        assert_eq!(
            map.ragdoll,
            vec![KeyCode::KeyX.into(), GamepadButton::West.into()]
        );
        assert_eq!(
            map.rotation_mode,
            vec![KeyCode::Digit1.into(), GamepadButton::DPadLeft.into()]
        );
        assert_eq!(
            map.view_mode,
            vec![KeyCode::Digit2.into(), GamepadButton::DPadRight.into()]
        );
        assert_eq!(
            map.switch_shoulder,
            vec![KeyCode::KeyC.into(), GamepadButton::RightThumb.into()]
        );
        assert_eq!(
            map.overlay_menu,
            vec![KeyCode::KeyQ.into(), GamepadButton::North.into()]
        );
        assert_eq!(
            map.slomo,
            vec![KeyCode::KeyZ.into(), GamepadButton::Select.into()]
        );
        assert_eq!(
            map.toggle_ui,
            vec![KeyCode::Tab.into(), GamepadButton::Start.into()]
        );
    }

    #[test]
    fn shift_suppresses_digit_key_actions() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(EnhancedInputPlugin)
            .add_plugins(AlsInputPlugin)
            .insert_resource(ButtonInput::<KeyCode>::default());
        app.finish();

        let player = app.world_mut().spawn_empty().id();
        let mut cmds = app.world_mut().commands();
        spawn_als_actions(&mut cmds, player, &AlsInputMap::default());
        app.update();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        app.world_mut()
            .commands()
            .entity(player)
            .mock_once::<OnFoot, RotationModeAction>(TriggerState::Fired, true);
        app.update();
        let cmd = *app.world().get::<AlsInputCommand>(player).unwrap();
        assert!(
            !cmd.rotation_mode_pressed,
            "shift must suppress digit-key gameplay actions"
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ShiftLeft);
        app.update();
        app.world_mut()
            .commands()
            .entity(player)
            .mock_once::<OnFoot, RotationModeAction>(TriggerState::Fired, true);
        app.update();
        let cmd = *app.world().get::<AlsInputCommand>(player).unwrap();
        assert!(cmd.rotation_mode_pressed, "bare digit key must actuate");
    }

    #[test]
    fn movement_keys_remain_live_while_holding_and_changing_direction() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::input::InputPlugin,
            EnhancedInputPlugin,
            AlsInputPlugin,
        ));
        app.finish();
        let player = app.world_mut().spawn_empty().id();
        spawn_als_actions(
            &mut app.world_mut().commands(),
            player,
            &AlsInputMap::default(),
        );
        app.update();
        for (frame, (pressed, expected)) in [
            (vec![KeyCode::KeyW], Vec2::Y),
            (vec![KeyCode::KeyW], Vec2::Y),
            (vec![KeyCode::KeyW, KeyCode::KeyD], Vec2::ONE),
            (vec![KeyCode::KeyD], Vec2::X),
            (vec![KeyCode::KeyS], Vec2::NEG_Y),
            (vec![KeyCode::KeyA], Vec2::NEG_X),
            (vec![], Vec2::ZERO),
        ]
        .into_iter()
        .enumerate()
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release_all();
            for key in pressed {
                keys.press(key);
            }
            app.update();
            let actual = app.world().get::<AlsInputCommand>(player).unwrap().move_dir;
            assert_eq!(actual, expected, "movement input frame={frame}");
        }
    }

    #[test]
    fn full_chain_from_action_mock_to_command() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(EnhancedInputPlugin)
            .add_plugins(AlsInputPlugin)
            .insert_resource(ButtonInput::<KeyCode>::default());
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
            .mock::<OnFoot, SprintAction>(TriggerState::Fired, true, MockSpan::Updates(50))
            .mock::<OnFoot, AimAction>(TriggerState::Fired, true, MockSpan::Updates(50))
            .mock::<OnFoot, MoveAction>(
                TriggerState::Fired,
                Vec2::new(1.0, 0.5),
                MockSpan::Updates(50),
            )
            .mock::<OnFoot, LookMouseAction>(
                TriggerState::Fired,
                Vec2::new(4.0, -3.0),
                MockSpan::Updates(50),
            )
            .mock::<OnFoot, LookAction>(
                TriggerState::Fired,
                Vec2::new(0.5, -0.25),
                MockSpan::Updates(50),
            )
            .mock::<OnFoot, OverlayMenuAction>(TriggerState::Fired, true, MockSpan::Updates(50))
            .mock_once::<OnFoot, NextOverlayModeAction>(TriggerState::Fired, 1.0)
            .mock_once::<OnFoot, PreviousOverlayModeAction>(TriggerState::Fired, 1.0)
            .mock_once::<OnFoot, SlomoAction>(TriggerState::Fired, true)
            .mock_once::<OnFoot, ToggleUiAction>(TriggerState::Fired, true);

        app.update();
        let cmd = *app.world().get::<AlsInputCommand>(player).unwrap();
        assert!(cmd.jump_pressed && cmd.jump_held, "jump pressed and held");
        assert!(cmd.crouch_pressed, "crouch toggle pressed");
        assert!(cmd.roll_pressed, "roll pressed");
        assert!(cmd.sprint_held, "sprint held");
        assert!(cmd.aim_held, "aim held");
        assert!(cmd.overlay_menu_held, "overlay menu held");
        assert!(cmd.next_overlay_mode_pressed, "next overlay pressed");
        assert!(
            cmd.previous_overlay_mode_pressed,
            "previous overlay pressed"
        );
        assert!(cmd.slomo_pressed, "slomo pressed");
        assert!(cmd.toggle_ui_pressed, "toggle ui pressed");
        assert_eq!(cmd.look_delta, Vec2::new(4.0, -3.0));
        assert_eq!(cmd.look_rate, Vec2::new(0.5, -0.25));
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
        assert!(cmd.overlay_menu_held, "overlay menu still held in span");
        assert!(!cmd.next_overlay_mode_pressed);
        assert!(!cmd.previous_overlay_mode_pressed);
        assert!(!cmd.slomo_pressed);
        assert!(!cmd.toggle_ui_pressed);
        assert_eq!(cmd.look_delta, Vec2::new(4.0, -3.0));
        assert_eq!(cmd.look_rate, Vec2::new(0.5, -0.25));
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
