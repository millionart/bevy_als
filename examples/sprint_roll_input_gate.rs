use std::num::NonZeroU8;

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_enhanced_input::prelude::*;

use als_core::character::AlsCharacter;
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::input::{
    AlsInputCommand, AlsInputPlugin, ROLL_TAP_SECS, RollAction, SPRINT_HOLD_SECS, SprintAction,
    als_input_bridge_system,
};
use als_core::input_request::AlsInputRequestPlugin;
use als_core::params::AlsMovementParams;
use als_core::state::Gait;
use als_core::transition::{AlsStateTransitionPlugin, state_transition_system};
use bevy_als::character_assembly::{AlsCharacterBlueprint, spawn_als_character};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            AnimationGraphPlugin::default(),
            EnhancedInputPlugin,
            AlsInputPlugin,
            AlsInputRequestPlugin,
            AlsStateTransitionPlugin,
        ))
        .init_asset::<StandardMaterial>()
        .init_asset::<AlsMovementParams>()
        .init_asset::<AlsControlSchemeConfig>()
        .init_resource::<GateState>()
        .add_systems(Startup, spawn_character)
        .add_systems(
            PreUpdate,
            inject_phase
                .after(EnhancedInputSystems::Apply)
                .before(als_input_bridge_system),
        )
        .add_systems(Update, verify_phase.after(state_transition_system))
        .run();
}

#[derive(Resource, Default)]
struct GateState {
    phase: u8,
    pending: bool,
    press_roll: bool,
    tap_roll: bool,
    prehold_sprint: bool,
    held_sprint: bool,
    released_sprint: bool,
}

fn spawn_character(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    spawn_als_character(
        &mut commands,
        &assets,
        &mut materials,
        &mut params_assets,
        &mut config_assets,
        AlsCharacterBlueprint::new(
            Transform::IDENTITY,
            "animated_scenes/motion_overlay.animscn.ron",
        ),
    );
}

fn inject_phase(
    mut state: ResMut<GateState>,
    characters: Query<Entity, With<AlsCharacter>>,
    mut roll_actions: Query<
        (&mut TriggerState, &mut ActionEvents),
        (With<Action<RollAction>>, Without<Action<SprintAction>>),
    >,
    mut sprint_actions: Query<
        (&mut TriggerState, &mut ActionEvents),
        (With<Action<SprintAction>>, Without<Action<RollAction>>),
    >,
) {
    if state.pending || state.phase >= 5 || characters.single().is_err() {
        return;
    }
    let Ok((mut roll_state, mut roll_events)) = roll_actions.single_mut() else {
        return;
    };
    let Ok((mut sprint_state, mut sprint_events)) = sprint_actions.single_mut() else {
        return;
    };
    match state.phase {
        0 => {
            *roll_state = TriggerState::Ongoing;
            *roll_events = ActionEvents::START | ActionEvents::ONGOING;
            *sprint_state = TriggerState::None;
            *sprint_events = ActionEvents::empty();
        }
        1 => {
            *roll_state = TriggerState::Fired;
            *roll_events = ActionEvents::FIRE;
            *sprint_state = TriggerState::None;
            *sprint_events = ActionEvents::empty();
        }
        2 => {
            *roll_state = TriggerState::None;
            *roll_events = ActionEvents::COMPLETE;
            *sprint_state = TriggerState::Ongoing;
            *sprint_events = ActionEvents::START | ActionEvents::ONGOING;
        }
        3 => {
            *roll_state = TriggerState::None;
            *roll_events = ActionEvents::empty();
            *sprint_state = TriggerState::Fired;
            *sprint_events = ActionEvents::FIRE;
        }
        _ => {
            *roll_state = TriggerState::None;
            *roll_events = ActionEvents::empty();
            *sprint_state = TriggerState::None;
            *sprint_events = ActionEvents::COMPLETE;
        }
    }
    state.pending = true;
}

fn verify_phase(
    mut state: ResMut<GateState>,
    characters: Query<(&AlsInputCommand, &Gait), With<AlsCharacter>>,
    tap: Query<&Tap, With<Action<RollAction>>>,
    hold: Query<&Hold, With<Action<SprintAction>>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !state.pending {
        return;
    }
    let Ok((command, gait)) = characters.single() else {
        return;
    };
    let Ok(tap) = tap.single() else {
        return;
    };
    let Ok(hold) = hold.single() else {
        return;
    };
    let pass = match state.phase {
        0 => !command.roll_pressed && !command.sprint_held,
        1 => command.roll_pressed && !command.sprint_held,
        2 => !command.roll_pressed && !command.sprint_held,
        3 => !command.roll_pressed && command.sprint_held && *gait == Gait::Sprinting,
        _ => !command.roll_pressed && !command.sprint_held && *gait == Gait::Running,
    };
    if !pass {
        error!(
            "[BCHAR4][VERDICT] phase={} roll={} sprint={} gait={gait:?} -> FAIL",
            state.phase, command.roll_pressed, command.sprint_held
        );
        exit.write(AppExit::Error(NonZeroU8::new(7).unwrap()));
        return;
    }
    match state.phase {
        0 => state.press_roll = command.roll_pressed,
        1 => state.tap_roll = command.roll_pressed,
        2 => state.prehold_sprint = command.sprint_held,
        3 => state.held_sprint = command.sprint_held,
        _ => state.released_sprint = !command.sprint_held && *gait == Gait::Running,
    }
    state.phase += 1;
    state.pending = false;
    if state.phase == 5 {
        let tap_secs = tap.timer().duration().as_secs_f32();
        let hold_secs = hold.timer().duration().as_secs_f32();
        let complete = !state.press_roll
            && state.tap_roll
            && !state.prehold_sprint
            && state.held_sprint
            && state.released_sprint
            && (tap_secs - ROLL_TAP_SECS).abs() < 1e-6
            && (hold_secs - SPRINT_HOLD_SECS).abs() < 1e-6;
        if complete {
            info!(
                "[BCHAR4][VERDICT] press_roll={} tap_roll={} prehold_sprint={} held_sprint={} released_sprint={} tap_secs={tap_secs:.3} hold_secs={hold_secs:.3} -> PASS",
                state.press_roll,
                state.tap_roll,
                state.prehold_sprint,
                state.held_sprint,
                state.released_sprint,
            );
            exit.write(AppExit::Success);
        } else {
            error!(
                "[BCHAR4][VERDICT] press_roll={} tap_roll={} prehold_sprint={} held_sprint={} released_sprint={} tap_secs={tap_secs:.3} hold_secs={hold_secs:.3} -> FAIL",
                state.press_roll,
                state.tap_roll,
                state.prehold_sprint,
                state.held_sprint,
                state.released_sprint,
            );
            exit.write(AppExit::Error(NonZeroU8::new(8).unwrap()));
        }
    }
}
