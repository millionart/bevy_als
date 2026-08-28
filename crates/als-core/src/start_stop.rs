use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

use crate::input::AlsInputCommand;
use crate::motion::{AlsInputOverride, AlsViewYaw, view_relative_move_axes};

#[derive(
    Component,
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Reflect,
    serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Default)]
pub enum MoveDirection {
    #[default]
    Forward,
    ForwardLeft,
    ForwardRight,
    Left,
    Right,
    BackwardLeft,
    BackwardRight,
    Backward,
}

impl MoveDirection {
    pub const CENTERS_DEGREES: [f32; 8] = [0.0, 45.0, 90.0, 135.0, 180.0, -135.0, -90.0, -45.0];

    pub const fn from_sector(sector: usize) -> Self {
        match sector % 8 {
            0 => Self::Forward,
            1 => Self::ForwardLeft,
            2 => Self::Left,
            3 => Self::BackwardLeft,
            4 => Self::Backward,
            5 => Self::BackwardRight,
            6 => Self::Right,
            _ => Self::ForwardRight,
        }
    }
}

#[derive(
    Component,
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Reflect,
    serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Default)]
pub enum StartStopPhase {
    #[default]
    Idle,
    Started(MoveDirection),
    Stopping(MoveDirection),
}

#[derive(
    Component,
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Reflect,
    serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Default)]
pub struct StartStopState {
    pub phase: StartStopPhase,
    pub last_direction: Option<MoveDirection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect, serde::Serialize, serde::Deserialize)]
pub struct StartStopConfig {
    pub start_speed_threshold: f32,
    pub stop_speed_threshold: f32,
}

impl Default for StartStopConfig {
    fn default() -> Self {
        Self {
            start_speed_threshold: 0.1,
            stop_speed_threshold: 0.05,
        }
    }
}

const SECTOR_ANGLE: f32 = std::f32::consts::FRAC_PI_4;
const ZERO_INPUT_EPSILON: f32 = 1e-6;

pub fn relative_angle(input_dir: Vec2, view_yaw: f32) -> f32 {
    if input_dir.length() <= ZERO_INPUT_EPSILON {
        return 0.0;
    }
    let forward = Vec2::new(-view_yaw.sin(), view_yaw.cos());
    let dot = forward.dot(input_dir);
    let cross = forward.x * input_dir.y - forward.y * input_dir.x;
    cross.atan2(dot)
}

pub fn classify(input_dir: Vec2, view_yaw: f32) -> MoveDirection {
    let rel = relative_angle(input_dir, view_yaw);
    let sector = (rel / SECTOR_ANGLE).round() as i32;
    MoveDirection::from_sector(sector.rem_euclid(8) as usize)
}

pub fn update_start_stop(
    state: &mut StartStopState,
    config: &StartStopConfig,
    horizontal_speed: f32,
    direction: MoveDirection,
) -> Option<StartStopPhase> {
    let previous = state.phase;

    if horizontal_speed > config.stop_speed_threshold {
        state.last_direction = Some(direction);
    }
    let tracked = state.last_direction;

    match state.phase {
        StartStopPhase::Idle => {
            if horizontal_speed >= config.start_speed_threshold
                && let Some(dir) = tracked
            {
                state.phase = StartStopPhase::Started(dir);
            }
        }
        StartStopPhase::Started(_) => {
            if horizontal_speed <= config.stop_speed_threshold {
                if let Some(dir) = tracked {
                    state.phase = StartStopPhase::Stopping(dir);
                }
            } else if let Some(dir) = tracked {
                state.phase = StartStopPhase::Started(dir);
            }
        }
        StartStopPhase::Stopping(_) => {
            if horizontal_speed >= config.start_speed_threshold
                && let Some(dir) = tracked
            {
                state.phase = StartStopPhase::Started(dir);
            } else if horizontal_speed <= config.stop_speed_threshold {
                state.phase = StartStopPhase::Idle;
            }
        }
    }

    if state.phase == previous {
        None
    } else {
        Some(state.phase)
    }
}

pub fn start_stop_detector_system(
    time: Res<Time>,
    mut q: Query<
        (
            &LinearVelocity,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            &AlsViewYaw,
            &mut StartStopState,
        ),
        With<crate::character::AlsCharacter>,
    >,
) {
    let Ok((vel, cmd, ovr, view, mut ss)) = q.single_mut() else {
        return;
    };
    let _ = time;
    let input = ovr.map(|o| &o.0).unwrap_or(cmd);
    let hspeed = Vec2::new(vel.x, vel.z).length();
    let dir = classify(view_relative_move_axes(input.move_dir, view.0), view.0);
    update_start_stop(&mut ss, &StartStopConfig::default(), hspeed, dir);
}
pub fn predict_stop_distance(speed: f32, braking_deceleration: f32) -> f32 {
    if braking_deceleration <= 1e-4 {
        return 0.0;
    }
    speed * speed / (2.0 * braking_deceleration)
}

pub fn stop_direction_is_left(dir: MoveDirection) -> bool {
    matches!(
        dir,
        MoveDirection::Left
            | MoveDirection::ForwardLeft
            | MoveDirection::BackwardLeft
            | MoveDirection::Backward
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn world_dir(yaw: f32, rel_degrees: f32) -> Vec2 {
        let rel = rel_degrees.to_radians();
        let forward = Vec2::new(-yaw.sin(), yaw.cos());
        let (s, c) = rel.sin_cos();
        Vec2::new(c * forward.x - s * forward.y, s * forward.x + c * forward.y)
    }

    fn all_directions() -> [MoveDirection; 8] {
        [
            MoveDirection::Forward,
            MoveDirection::ForwardLeft,
            MoveDirection::Left,
            MoveDirection::BackwardLeft,
            MoveDirection::Backward,
            MoveDirection::BackwardRight,
            MoveDirection::Right,
            MoveDirection::ForwardRight,
        ]
    }

    #[test]
    fn sector_centers_map_to_expected_directions_regardless_of_yaw() {
        let expected = all_directions();
        for yaw in [0.0f32, 1.234, -2.71, 17.9] {
            for (sector, center) in MoveDirection::CENTERS_DEGREES.iter().enumerate() {
                assert_eq!(
                    classify(world_dir(yaw, *center), yaw),
                    expected[sector],
                    "yaw={yaw} rel={center}"
                );
            }
        }
    }

    #[test]
    fn sector_boundaries_bracket_expected_directions() {
        let expected = all_directions();
        let yaw = 0.37f32;
        for sector in 0..8usize {
            let boundary = 45.0 * sector as f32 + 22.5;
            let below = classify(world_dir(yaw, boundary - 1.0), yaw);
            let above = classify(world_dir(yaw, boundary + 1.0), yaw);
            assert_eq!(below, expected[sector], "just below {boundary} deg");
            assert_eq!(
                above,
                expected[(sector + 1) % 8],
                "just above {boundary} deg"
            );
        }
    }

    #[test]
    fn classification_is_periodic_in_yaw_and_wraps_negative_angles() {
        let yaw = 2.51f32;
        for rel in [
            -179.0f32, -135.0, -90.0, -45.0, 0.0, 45.0, 90.0, 135.0, 179.0,
        ] {
            assert_eq!(
                classify(world_dir(yaw, rel), yaw),
                classify(world_dir(yaw - TAU, rel), yaw - TAU),
                "rel={rel} must be invariant under yaw - tau"
            );
            assert_eq!(
                classify(world_dir(yaw, rel), yaw),
                classify(world_dir(yaw, rel - 360.0), yaw),
                "rel={rel} must wrap across the backward seam"
            );
        }
    }

    #[test]
    fn positive_relative_angle_means_left_of_view() {
        let yaw = 0.0f32;
        let left = world_dir(yaw, 90.0);
        let right = world_dir(yaw, -90.0);
        assert!(relative_angle(left, yaw) > 0.0);
        assert!(relative_angle(right, yaw) < 0.0);
        assert_eq!(classify(left, yaw), MoveDirection::Left);
        assert_eq!(classify(right, yaw), MoveDirection::Right);
    }

    #[test]
    fn zero_input_is_documented_as_forward() {
        assert_eq!(classify(Vec2::ZERO, 1.5), MoveDirection::Forward);
        assert_eq!(relative_angle(Vec2::ZERO, -3.2), 0.0);
    }

    #[test]
    fn default_config_keeps_hysteresis_gap() {
        let config = StartStopConfig::default();
        assert!(
            config.stop_speed_threshold < config.start_speed_threshold,
            "stop threshold {} must stay below start threshold {}",
            config.stop_speed_threshold,
            config.start_speed_threshold
        );
    }

    #[test]
    fn idle_starts_only_above_start_threshold() {
        let mut state = StartStopState::default();
        let config = StartStopConfig::default();

        assert_eq!(
            update_start_stop(&mut state, &config, 0.04, MoveDirection::Forward),
            None
        );
        assert_eq!(state.phase, StartStopPhase::Idle);

        assert_eq!(
            update_start_stop(&mut state, &config, 0.09, MoveDirection::Forward),
            None,
            "speed inside the hysteresis band must not start from idle"
        );

        assert_eq!(
            update_start_stop(&mut state, &config, 0.11, MoveDirection::ForwardLeft),
            Some(StartStopPhase::Started(MoveDirection::ForwardLeft))
        );
    }

    #[test]
    fn hysteresis_band_holds_started_until_stop_threshold() {
        let mut state = StartStopState {
            phase: StartStopPhase::Started(MoveDirection::Forward),
            last_direction: Some(MoveDirection::Forward),
        };
        let config = StartStopConfig::default();

        assert_eq!(
            update_start_stop(&mut state, &config, 0.09, MoveDirection::Forward),
            None
        );
        assert_eq!(
            update_start_stop(&mut state, &config, 0.06, MoveDirection::Forward),
            None
        );
        assert_eq!(
            update_start_stop(&mut state, &config, 0.09, MoveDirection::Forward),
            None
        );
        assert_eq!(state.phase, StartStopPhase::Started(MoveDirection::Forward));

        assert_eq!(
            update_start_stop(&mut state, &config, 0.05, MoveDirection::Forward),
            Some(StartStopPhase::Stopping(MoveDirection::Forward))
        );
    }

    #[test]
    fn stopping_resumes_only_above_start_threshold() {
        let mut state = StartStopState {
            phase: StartStopPhase::Stopping(MoveDirection::BackwardRight),
            last_direction: Some(MoveDirection::BackwardRight),
        };
        let config = StartStopConfig::default();

        assert_eq!(
            update_start_stop(&mut state, &config, 0.06, MoveDirection::Forward),
            None
        );
        assert_eq!(
            update_start_stop(&mut state, &config, 0.09, MoveDirection::Forward),
            None
        );
        assert_eq!(
            state.phase,
            StartStopPhase::Stopping(MoveDirection::BackwardRight),
            "direction must stay frozen while stopping"
        );

        assert_eq!(
            update_start_stop(&mut state, &config, 0.11, MoveDirection::Forward),
            Some(StartStopPhase::Started(MoveDirection::Forward))
        );
    }

    #[test]
    fn direction_tracks_input_while_started_and_freezes_while_stopping() {
        let mut state = StartStopState {
            phase: StartStopPhase::Started(MoveDirection::Forward),
            last_direction: Some(MoveDirection::Forward),
        };
        let config = StartStopConfig::default();

        assert_eq!(
            update_start_stop(&mut state, &config, 2.0, MoveDirection::Left),
            Some(StartStopPhase::Started(MoveDirection::Left)),
            "started phase follows the live movement direction"
        );

        assert_eq!(
            update_start_stop(&mut state, &config, 0.03, MoveDirection::Left),
            Some(StartStopPhase::Stopping(MoveDirection::Left))
        );

        assert_eq!(
            update_start_stop(&mut state, &config, 0.02, MoveDirection::Backward),
            Some(StartStopPhase::Idle),
            "below stop threshold the stop resolves to idle"
        );
        assert_eq!(state.phase, StartStopPhase::Idle);

        assert_eq!(
            update_start_stop(&mut state, &config, 2.0, MoveDirection::Backward),
            Some(StartStopPhase::Started(MoveDirection::Backward)),
            "resuming above the start threshold restarts from idle"
        );
    }

    #[test]
    fn rest_state_never_emits_stop_without_prior_motion() {
        let mut state = StartStopState::default();
        let config = StartStopConfig::default();

        for _ in 0..5 {
            assert_eq!(
                update_start_stop(&mut state, &config, 0.0, MoveDirection::Backward),
                None
            );
        }
        assert_eq!(state.phase, StartStopPhase::Idle);
        assert_eq!(state.last_direction, None);
    }

    #[test]
    fn speed_spike_straight_from_rest_reports_start_with_live_direction() {
        let mut state = StartStopState::default();
        let config = StartStopConfig::default();

        assert_eq!(
            update_start_stop(&mut state, &config, 3.5, MoveDirection::ForwardRight),
            Some(StartStopPhase::Started(MoveDirection::ForwardRight))
        );
        assert_eq!(state.last_direction, Some(MoveDirection::ForwardRight));
    }
}
