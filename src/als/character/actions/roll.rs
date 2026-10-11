use bevy::prelude::*;

use crate::als::input::AlsInputCommand;
use crate::als::motion::AlsViewYaw;
use crate::als::orientation::yaw_to_world_forward;
use crate::als::start_stop::{MoveDirection, classify};
use crate::als::state::{LocomotionState, Stance};

pub const ROLL_MONTAGE_SEQUENCE_LENGTH_SECS: f32 = 1.5;
pub const ROLL_PLAY_RATE: f32 = 1.3;
pub const ROLL_DURATION_SECS: f32 = ROLL_MONTAGE_SEQUENCE_LENGTH_SECS / ROLL_PLAY_RATE;
pub const ROLL_WEIGHT_IN_SECS: f32 = 0.1;
pub const ROLL_WEIGHT_OUT_SECS: f32 = 0.3;
pub const ROLL_ROTATION_HALF_LIFE_SECS: f32 = 0.1;
pub const ROLL_ON_LAND_SPEED_THRESHOLD: f32 = 7.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RollLifecycle {
    pub started: bool,
    pub ended: bool,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsRollState {
    pub active: bool,
    pub time: f32,
    pub previous_time: f32,
    pub cooldown: f32,
    pub direction_yaw: f32,
    pub last_direction: MoveDirection,
    pub stance_before_roll: Stance,
}

impl Default for AlsRollState {
    fn default() -> Self {
        Self {
            active: false,
            time: 0.0,
            previous_time: 0.0,
            cooldown: 0.0,
            direction_yaw: 0.0,
            last_direction: MoveDirection::Forward,
            stance_before_roll: Stance::Standing,
        }
    }
}

pub fn roll_weight_at(elapsed: f32) -> f32 {
    let remaining = (ROLL_DURATION_SECS - elapsed).max(0.0);
    let alpha = (elapsed / ROLL_WEIGHT_IN_SECS)
        .clamp(0.0, 1.0)
        .min((remaining / ROLL_WEIGHT_OUT_SECS).clamp(0.0, 1.0));
    alpha * alpha * (3.0 - 2.0 * alpha)
}

pub fn select_roll_direction(input_yaw: Option<f32>, basis: &AlsViewYaw) -> (MoveDirection, f32) {
    let yaw = input_yaw.unwrap_or(basis.0);
    let world = yaw_to_world_forward(yaw);
    let direction = input_yaw.map_or(MoveDirection::Forward,
        |_| classify(Vec2::new(world.x, -world.z), basis.0));
    (direction, yaw)
}

pub fn roll_displacement_dir(yaw: f32) -> Vec3 {
    yaw_to_world_forward(yaw)
}

pub fn wrap_angle(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

pub fn damper_exact_angle(
    current: f32,
    target: f32,
    delta_seconds: f32,
    half_life_seconds: f32,
) -> f32 {
    if half_life_seconds <= 0.0 {
        return target;
    }
    let raw_delta = target - current;
    let mut delta = if (-std::f32::consts::PI..=std::f32::consts::PI).contains(&raw_delta) {
        raw_delta
    } else {
        wrap_angle(raw_delta)
    };
    if delta < -crate::als::transitions::COUNTERCLOCKWISE_REMAP_THRESHOLD_DEGREES.to_radians() {
        delta += std::f32::consts::TAU;
    }
    let alpha = 1.0 - crate::als::animation::inv_exp_approx(
        std::f32::consts::LN_2 / (half_life_seconds + 1e-8) * delta_seconds.max(0.0),
    );
    wrap_angle(current + delta * alpha)
}

impl AlsRollState {
    pub fn root_motion_active(&self) -> bool {
        self.active && self.time < ROLL_DURATION_SECS - ROLL_WEIGHT_OUT_SECS
    }

    pub fn start_from_landing(
        &mut self,
        landing_speed: f32,
        velocity_yaw: Option<f32>,
        actor_yaw: f32,
        stance: Stance,
    ) -> RollLifecycle {
        if self.root_motion_active() || landing_speed < ROLL_ON_LAND_SPEED_THRESHOLD {
            return RollLifecycle::default();
        }
        self.direction_yaw = velocity_yaw.unwrap_or(actor_yaw);
        self.last_direction = MoveDirection::Forward;
        self.stance_before_roll = stance;
        self.active = true;
        self.time = 0.0;
        self.previous_time = 0.0;
        RollLifecycle {
            started: true,
            ended: false,
        }
    }

    pub fn update(
        &mut self,
        cmd: &AlsInputCommand,
        direction: (MoveDirection, f32),
        loco: &LocomotionState,
        stance: &Stance,
        dt: f32,
    ) -> RollLifecycle {
        let mut lifecycle = RollLifecycle::default();
        self.cooldown = 0.0;
        self.previous_time = self.time;
        if self.active {
            let playing = self.root_motion_active();
            self.time += dt;
            if self.time >= ROLL_DURATION_SECS {
                self.active = false;
                self.time = ROLL_DURATION_SECS;
                lifecycle.ended = true;
            }
            if playing { return lifecycle; }
        }
        if *loco == LocomotionState::Grounded && cmd.roll_pressed {
            self.last_direction = direction.0;
            self.direction_yaw = direction.1;
            self.stance_before_roll = *stance;
            self.active = true;
            self.time = 0.0;
            self.previous_time = 0.0;
            lifecycle.started = true;
        }
        lifecycle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roll_montage_weight_uses_source_hermite_cubic_blends() {
        assert!((roll_weight_at(ROLL_WEIGHT_IN_SECS * 0.25) - 0.15625).abs() < 1e-6);
        assert!(
            (roll_weight_at(ROLL_DURATION_SECS - ROLL_WEIGHT_OUT_SECS * 0.25) - 0.15625).abs()
                < 1e-6
        );
    }

    fn base_cmd() -> AlsInputCommand {
        AlsInputCommand {
            roll_pressed: true,
            ..default()
        }
    }

    #[test]
    fn idle_request_rolls_toward_actor_forward() {
        let mut state = AlsRollState::default();
        let lifecycle = state.update(
            &base_cmd(),
            select_roll_direction(None, &AlsViewYaw(std::f32::consts::FRAC_PI_2)),
            &LocomotionState::Grounded,
            &Stance::Standing,
            1.0 / 60.0,
        );
        assert!(lifecycle.started && state.active);
        assert_eq!(state.last_direction, MoveDirection::Forward);
        assert!((state.direction_yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn lateral_move_selects_strafe_roll() {
        let mut state = AlsRollState::default();
        let cmd = AlsInputCommand {
            roll_pressed: true,
            move_dir: Vec2::X,
            ..default()
        };
        state.update(
            &cmd,
            select_roll_direction(Some(-std::f32::consts::FRAC_PI_2), &AlsViewYaw(0.0)),
            &LocomotionState::Grounded,
            &Stance::Standing,
            1.0 / 60.0,
        );
        assert_eq!(state.last_direction, MoveDirection::Right);
        assert!(roll_displacement_dir(state.direction_yaw).distance(Vec3::X) < 1e-5);
    }

    #[test]
    fn source_lifecycle_has_no_fixed_cooldown_and_preserves_play_rate() {
        let mut state = AlsRollState::default();
        state.update(
            &base_cmd(),
            select_roll_direction(None, &AlsViewYaw(0.0)),
            &LocomotionState::Grounded,
            &Stance::Crouching,
            1.0 / 60.0,
        );
        assert!(state.active);
        assert_eq!(state.stance_before_roll, Stance::Crouching);
        let end = state.update(
            &AlsInputCommand::default(),
            select_roll_direction(None, &AlsViewYaw(0.0)),
            &LocomotionState::Grounded,
            &Stance::Crouching,
            ROLL_DURATION_SECS,
        );
        assert!(end.ended && !state.active);
        assert_eq!(state.cooldown, 0.0);
        assert!((ROLL_DURATION_SECS - 1.5 / 1.3).abs() < 1e-6);
    }

    #[test]
    fn montage_blend_windows_match_source_asset() {
        assert!(roll_weight_at(0.0) <= 1e-6);
        assert!((roll_weight_at(ROLL_WEIGHT_IN_SECS) - 1.0).abs() < 1e-6);
        assert!(roll_weight_at(ROLL_DURATION_SECS) <= 1e-6);
    }

    #[test]
    fn source_rotation_damper_uses_half_life() {
        let target = std::f32::consts::PI;
        let after_one_half_life = damper_exact_angle(
            0.0,
            target,
            ROLL_ROTATION_HALF_LIFE_SECS,
            ROLL_ROTATION_HALF_LIFE_SECS,
        );
        let alpha = 1.0 - crate::als::animation::inv_exp_approx(std::f32::consts::LN_2 / (ROLL_ROTATION_HALF_LIFE_SECS + 1e-8) * ROLL_ROTATION_HALF_LIFE_SECS);
        assert!((after_one_half_life.abs() - std::f32::consts::PI * alpha).abs() < 1e-5);
        assert!((damper_exact_angle(0.0, target, 0.1, 0.0) - target).abs() < 1e-6);
    }

    #[test]
    fn source_roll_damper_preserves_world_turn_across_yaw_bases() {
        let alpha = 1.0 - crate::als::animation::inv_exp_approx(std::f32::consts::LN_2 / (0.1 + 1e-8) * 0.1);
        for (target_degrees, source_delta_degrees) in [
            (176.0_f32, -184.0_f32),
            (174.0, 174.0),
            (175.0, 175.0),
            (180.0, -180.0),
            (-176.0, -176.0),
        ] {
            let bevy_target = (-target_degrees).to_radians();
            let actual = damper_exact_angle(0.0, bevy_target, 0.1, 0.1);
            let expected = wrap_angle((-source_delta_degrees).to_radians() * alpha);
            assert!(yaw_to_world_forward(actual).distance(yaw_to_world_forward(expected)) < 1e-5,
                "source target={target_degrees} actual={actual} expected={expected}");
        }
    }

    #[test]
    fn source_landing_roll_threshold_uses_preimpact_velocity() {
        for speed in [6.999, 7.0, 7.001] {
            let mut state = AlsRollState::default();
            let started = state.start_from_landing(speed, Some(-std::f32::consts::FRAC_PI_2), 0.0, Stance::Standing).started;
            assert_eq!(started, speed >= 7.0);
            if started { assert!(roll_displacement_dir(state.direction_yaw).distance(Vec3::X) < 1e-6); }
        }
    }
}
