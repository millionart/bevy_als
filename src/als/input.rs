use std::collections::VecDeque;

use avian3d::prelude::PhysicsTime;
use bevy::prelude::*;

use crate::als::{character::AlsCharacter, motion::AlsInputOverride};

/// Marks an on-foot input context for ALS diagnostics.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OnFoot;

/// Per-frame input written by the host in `PreUpdate`.
///
/// Use the movement axes for right and forward input respectively. Refresh held
/// values every frame and set each `*_pressed` field for a single update.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Reflect)]
#[reflect(Default)]
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
}

impl AlsInputCommand {
    /// Clears the entire command before writing a new frame.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[doc(hidden)]
#[derive(Component, Default)]
pub struct AlsStepInput {
    pending: VecDeque<AlsInputCommand>,
    pressed: AlsInputCommand,
}

impl AlsStepInput {
    pub(crate) fn command(
        command: &AlsInputCommand,
        override_command: Option<&AlsInputOverride>,
        step: Option<&Self>,
    ) -> AlsInputCommand {
        let mut command = *override_command.map_or(command, |value| &value.0);
        if let Some(step) = step {
            let pressed = step.pressed;
            command.jump_pressed = pressed.jump_pressed;
            command.crouch_pressed = pressed.crouch_pressed;
            command.roll_pressed = pressed.roll_pressed;
            command.walk_pressed = pressed.walk_pressed;
            command.ragdoll_pressed = pressed.ragdoll_pressed;
            command.rotation_mode_pressed = pressed.rotation_mode_pressed;
            command.view_mode_pressed = pressed.view_mode_pressed;
            command.switch_shoulder_pressed = pressed.switch_shoulder_pressed;
        }
        command
    }
}

pub(crate) fn buffer_input(
    time: Res<Time<Virtual>>,
    mut characters: Query<(
        &AlsInputCommand,
        Option<&AlsInputOverride>,
        &mut AlsStepInput,
    )>,
) {
    if time.is_paused() {
        return;
    }
    for (command, override_command, mut step) in &mut characters {
        let command = AlsStepInput::command(command, override_command, None);
        if command.jump_pressed
            || command.crouch_pressed
            || command.roll_pressed
            || command.walk_pressed
            || command.ragdoll_pressed
            || command.rotation_mode_pressed
            || command.view_mode_pressed
            || command.switch_shoulder_pressed
        {
            step.pending.push_back(command);
        }
    }
}

pub(crate) fn begin_input_step(
    time: Res<Time>,
    physics: Res<Time<avian3d::prelude::Physics>>,
    mut characters: Query<&mut AlsStepInput, With<AlsCharacter>>,
) {
    for mut step in &mut characters {
        step.pressed = if time.delta().is_zero() || physics.is_paused() {
            AlsInputCommand::default()
        } else {
            step.pending.pop_front().unwrap_or_default()
        };
    }
}
