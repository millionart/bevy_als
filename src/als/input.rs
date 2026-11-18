use bevy::prelude::*;

/// Marks an on-foot input context for ALS diagnostics.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OnFoot;

/// Per-frame input written by the host before ALS systems run in `Update`.
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
