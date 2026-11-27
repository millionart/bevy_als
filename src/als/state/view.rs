use bevy::prelude::*;

use crate::als::character::AlsCharacter;
use crate::als::input::AlsInputCommand;
use crate::als::motion::{AlsInputOverride, AlsViewYaw};

pub const PITCH_LIMIT_DEGREES: f32 = 89.900002;
pub const LOOK_HALF_LIFE_SECONDS: f32 = 0.08;
pub const LOOK_MOUSE_RADIANS_PER_COUNT: f32 = std::f32::consts::PI / 180.0;
pub const LOOK_UP_MOUSE_SENSITIVITY: f32 = 1.0;
pub const LOOK_RIGHT_MOUSE_SENSITIVITY: f32 = 1.0;
pub const LOOK_UP_RATE_DEGREES_PER_SECOND: f32 = 90.0;
pub const LOOK_RIGHT_RATE_DEGREES_PER_SECOND: f32 = 240.0;

#[derive(Component, Debug, Default, Clone, Copy, Reflect)]
#[reflect(Default)]
pub struct AlsViewPitch(pub f32);

#[derive(Component, Debug, Default, Clone, Copy, Reflect)]
#[reflect(Default)]
pub struct AlsViewSmoothed(pub f32);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsPlayerViewYaw(pub f32);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AlsPlayerViewReadiness {
    pub source: Entity,
    pub has_published: bool,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsViewInputSet;

pub fn look_delta_radians(command: &AlsInputCommand, dt: f32) -> Vec2 {
    Vec2::new(
        command.look_delta.x * LOOK_RIGHT_MOUSE_SENSITIVITY * LOOK_MOUSE_RADIANS_PER_COUNT
            + command.look_rate.x * LOOK_RIGHT_RATE_DEGREES_PER_SECOND.to_radians() * dt,
        command.look_delta.y * LOOK_UP_MOUSE_SENSITIVITY * LOOK_MOUSE_RADIANS_PER_COUNT
            + command.look_rate.y * LOOK_UP_RATE_DEGREES_PER_SECOND.to_radians() * dt,
    )
}

pub fn apply_view_input_system(
    time: Res<Time<Real>>,
    virtual_time: Res<Time<Virtual>>,
    mut characters: Query<
        (
            &mut AlsViewYaw,
            &mut AlsViewPitch,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        (
            With<AlsCharacter>,
            Without<crate::als::character::AlsBirthAnimationPending>,
        ),
    >,
) {
    if virtual_time.is_paused() {
        return;
    }
    let dilation = virtual_time.relative_speed();
    let delta_seconds = if dilation == 0.0 {
        time.delta_secs()
    } else {
        virtual_time.delta_secs() / dilation
    };
    for (mut yaw, mut pitch, command, override_command) in &mut characters {
        let effective = override_command
            .map(|override_command| &override_command.0)
            .unwrap_or(command);
        let delta = look_delta_radians(effective, delta_seconds);
        yaw.0 -= delta.x;
        pitch.0 = clamp_pitch(pitch.0 - delta.y);
    }
}

pub struct AlsViewInputPlugin;

impl Plugin for AlsViewInputPlugin {
    fn build(&self, app: &mut App) {
        let schedule = crate::AlsPhysicsSchedule::frame_input(app);
        app.configure_sets(
            schedule,
            AlsViewInputSet.in_set(crate::playable_character::AlsFrameInputSet),
        )
        .add_systems(schedule, apply_view_input_system.in_set(AlsViewInputSet));
    }
}

pub fn clamp_pitch(pitch_radians: f32) -> f32 {
    let limit = PITCH_LIMIT_DEGREES.to_radians();
    let mut pitch = pitch_radians % std::f32::consts::TAU;
    if pitch > std::f32::consts::PI {
        pitch -= std::f32::consts::TAU;
    } else if pitch <= -std::f32::consts::PI {
        pitch += std::f32::consts::TAU;
    }
    pitch.clamp(-limit, limit)
}

pub fn damp_view(current: f32, target: f32, half_life_seconds: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return current;
    }
    let lambda = std::f32::consts::LN_2.max(f32::EPSILON) / half_life_seconds.max(1e-4);
    current + (target - current) * (1.0 - (-lambda * dt).exp())
}

pub fn view_pitch_norm(pitch_radians: f32) -> f32 {
    clamp_pitch(pitch_radians) / PITCH_LIMIT_DEGREES.to_radians()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_clamps_to_limit() {
        assert_eq!(clamp_pitch(200.0_f32.to_radians()), -89.9_f32.to_radians());
        assert_eq!(clamp_pitch(-200.0_f32.to_radians()), 89.9_f32.to_radians());
        assert!((clamp_pitch(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn norm_maps_limits_to_unit_range() {
        assert!((view_pitch_norm(PITCH_LIMIT_DEGREES.to_radians()) - 1.0).abs() < 1e-6);
        assert!((view_pitch_norm(-PITCH_LIMIT_DEGREES.to_radians()) + 1.0).abs() < 1e-6);
        assert!(view_pitch_norm(0.0).abs() < 1e-6);
        let over = view_pitch_norm(100.0_f32.to_radians());
        assert!((over - 1.0).abs() < 1e-6);
    }

    #[test]
    fn damping_converges_toward_target_and_never_overshoots() {
        let mut cur = 0.0_f32;
        for _ in 0..600 {
            cur = damp_view(cur, 1.0, LOOK_HALF_LIFE_SECONDS, 1.0 / 60.0);
            assert!(cur <= 1.0 + 1e-6 && cur >= 0.0 - 1e-6);
        }
        assert!((cur - 1.0).abs() < 1e-3);
    }

    #[test]
    fn zero_dt_keeps_current() {
        assert_eq!(damp_view(0.25, 1.0, LOOK_HALF_LIFE_SECONDS, 0.0), 0.25);
    }

    #[test]
    fn half_life_halves_distance() {
        let after = damp_view(1.0, 0.0, 0.5, 0.5);
        assert!((after - 0.5).abs() < 1e-3);
    }

    #[test]
    fn mouse_and_rate_look_use_separate_source_units() {
        let command = AlsInputCommand {
            look_delta: Vec2::new(4.0, -2.0),
            look_rate: Vec2::new(0.5, -0.25),
            ..default()
        };
        let delta = look_delta_radians(&command, 0.5);
        let expected_x = (4.0_f32 + 0.5 * 240.0 * 0.5).to_radians();
        let expected_y = (-2.0_f32 - 0.25 * 90.0 * 0.5).to_radians();
        assert!((delta.x - expected_x).abs() < 1e-6);
        assert!((delta.y - expected_y).abs() < 1e-6);
    }
}
