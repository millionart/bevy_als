use bevy::prelude::*;

use crate::input::AlsInputCommand;
use crate::motion::{AlsViewYaw, view_relative_move_axes, view_relative_move_direction};
use crate::start_stop::{MoveDirection, classify};
use crate::state::{LocomotionState, Stance};

pub const ROLL_DURATION_SECS: f32 = 1.5;
pub const ROLL_DISTANCE_METERS: f32 = 3.0;
pub const ROLL_WEIGHT_IN_SECS: f32 = 0.12;
pub const ROLL_WEIGHT_OUT_SECS: f32 = 0.18;
pub const ROLL_COOLDOWN_SECS: f32 = 0.35;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsRollState {
    pub active: bool,
    pub time: f32,
    pub cooldown: f32,
    pub direction_yaw: f32,
    pub last_direction: MoveDirection,
}

impl Default for AlsRollState {
    fn default() -> Self {
        Self {
            active: false,
            time: 0.0,
            cooldown: 0.0,
            direction_yaw: 0.0,
            last_direction: MoveDirection::Forward,
        }
    }
}

pub fn roll_speed_at(time_norm: f32) -> f32 {
    let t = time_norm.clamp(0.0, 1.0);
    let bell = (std::f32::consts::PI * t).sin().powi(2);
    ROLL_DISTANCE_METERS * bell / (ROLL_DURATION_SECS * 0.5)
}

pub fn roll_weight_at(elapsed: f32) -> f32 {
    let tin = ROLL_WEIGHT_IN_SECS.max(1e-4);
    let tout = ROLL_WEIGHT_OUT_SECS.max(1e-4);
    let remaining = (ROLL_DURATION_SECS - elapsed).max(0.0);
    let w_in = (elapsed / tin).clamp(0.0, 1.0);
    let w_out = (remaining / tout).clamp(0.0, 1.0);
    w_in.min(w_out)
}

pub fn select_roll_direction(cmd: &AlsInputCommand, view_yaw: &AlsViewYaw) -> (MoveDirection, f32) {
    let input = if cmd.move_dir.length_squared() > 1e-6 {
        cmd.move_dir
    } else {
        Vec2::Y
    };
    let dir = classify(view_relative_move_axes(input, view_yaw.0), view_yaw.0);
    let world = view_relative_move_direction(input, view_yaw.0);
    (dir, f32::atan2(world.x, -world.z))
}

impl AlsRollState {
    pub fn update(
        &mut self,
        cmd: &AlsInputCommand,
        view_yaw: &AlsViewYaw,
        loco: &LocomotionState,
        stance: &Stance,
        dt: f32,
    ) {
        self.cooldown = (self.cooldown - dt).max(0.0);
        if self.active {
            self.time += dt;
            if self.time >= ROLL_DURATION_SECS {
                self.active = false;
                self.time = 0.0;
                self.cooldown = ROLL_COOLDOWN_SECS;
            }
            return;
        }
        let can_roll = *loco == LocomotionState::Grounded
            && *stance == Stance::Standing
            && self.cooldown <= 0.0;
        if can_roll && cmd.roll_pressed {
            let (dir, yaw) = select_roll_direction(cmd, view_yaw);
            self.last_direction = dir;
            self.direction_yaw = yaw;
            self.active = true;
            self.time = 0.0;
        }
    }
}

pub fn roll_displacement_dir(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, -yaw.cos())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_cmd() -> AlsInputCommand {
        let mut c = AlsInputCommand::default();
        c.roll_pressed = true;
        c
    }

    #[test]
    fn idle_request_rolls_toward_view_forward() {
        let mut s = AlsRollState::default();
        let cmd = base_cmd();
        let view = AlsViewYaw(std::f32::consts::FRAC_PI_2);
        s.update(
            &cmd,
            &view,
            &LocomotionState::Grounded,
            &Stance::Standing,
            1.0 / 60.0,
        );
        assert!(s.active);
        assert_eq!(s.last_direction, MoveDirection::Forward);
        assert!((s.direction_yaw + std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn lateral_move_selects_strafe_roll() {
        let mut s = AlsRollState::default();
        let mut cmd = base_cmd();
        cmd.move_dir = Vec2::new(1.0, 0.0);
        let view = AlsViewYaw(0.0);
        s.update(
            &cmd,
            &view,
            &LocomotionState::Grounded,
            &Stance::Standing,
            1.0 / 60.0,
        );
        assert_eq!(s.last_direction, MoveDirection::Right);
        assert!((s.direction_yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        let dir_vec = roll_displacement_dir(s.direction_yaw);
        assert!(dir_vec.distance(Vec3::X) < 1e-5, "right roll displaces +X");
    }

    #[test]
    fn local_forward_roll_tracks_a_rotated_camera() {
        let mut s = AlsRollState::default();
        let cmd = base_cmd();
        let view = AlsViewYaw(std::f32::consts::FRAC_PI_2);
        s.update(
            &cmd,
            &view,
            &LocomotionState::Grounded,
            &Stance::Standing,
            1.0 / 60.0,
        );
        assert_eq!(s.last_direction, MoveDirection::Forward);
        assert!((s.direction_yaw + std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert!(roll_displacement_dir(s.direction_yaw).distance(Vec3::NEG_X) < 1e-5);
    }

    #[test]
    fn cannot_roll_midair_or_crouched_or_in_cooldown() {
        let mut s = AlsRollState::default();
        let cmd = base_cmd();
        s.update(
            &cmd,
            &AlsViewYaw(0.0),
            &LocomotionState::Air,
            &Stance::Standing,
            1.0 / 60.0,
        );
        assert!(!s.active);
        s.update(
            &cmd,
            &AlsViewYaw(0.0),
            &LocomotionState::Grounded,
            &Stance::Crouching,
            1.0 / 60.0,
        );
        assert!(!s.active);

        s.update(
            &cmd,
            &AlsViewYaw(0.0),
            &LocomotionState::Grounded,
            &Stance::Standing,
            1.0 / 60.0,
        );
        assert!(s.active);
        let mut iters = 0;
        while s.active {
            s.update(
                &cmd,
                &AlsViewYaw(0.0),
                &LocomotionState::Grounded,
                &Stance::Standing,
                1.0 / 60.0,
            );
            iters += 1;
            assert!(iters < 200, "roll never ended");
        }
        assert!(
            (85..=95).contains(&iters),
            "roll ends near duration, iters={iters}"
        );
        s.update(
            &cmd,
            &AlsViewYaw(0.0),
            &LocomotionState::Grounded,
            &Stance::Standing,
            1.0 / 60.0,
        );
        assert!(!s.active, "cooldown blocks immediate re-roll");
    }

    #[test]
    fn speed_curve_integrates_to_roll_distance() {
        let steps = 240;
        let dt = ROLL_DURATION_SECS / steps as f32;
        let mut dist = 0.0;
        for i in 0..steps {
            let t = (i as f32 + 0.5) * dt / ROLL_DURATION_SECS;
            dist += roll_speed_at(t) * dt;
        }
        assert!(
            (dist - ROLL_DISTANCE_METERS).abs() < 0.02,
            "displacement integral {dist} != {}",
            ROLL_DISTANCE_METERS
        );
        assert!((roll_speed_at(0.5) - roll_speed_at(0.5)).abs() < 1e-6);
        let mid = roll_speed_at(0.5);
        let quarter = roll_speed_at(0.25);
        assert!(mid > quarter, "bell peaks at midpoint");
        assert!(roll_speed_at(-0.5) < 1e-6 && roll_speed_at(1.5) < 1e-6);
    }

    #[test]
    fn weight_envelope_ramps_in_and_out() {
        assert!(roll_weight_at(0.0) <= 0.01);
        assert!((roll_weight_at(ROLL_WEIGHT_IN_SECS) - 1.0).abs() < 1e-4);
        assert!((roll_weight_at(ROLL_DURATION_SECS * 0.5) - 1.0).abs() < 1e-4);
        assert!(roll_weight_at(ROLL_DURATION_SECS) <= 0.01);
    }

    #[test]
    fn direction_vector_matches_yaw_convention() {
        let fwd = roll_displacement_dir(0.0);
        assert!(fwd.distance(Vec3::NEG_Z) < 1e-5);
        let right_of_north = roll_displacement_dir(std::f32::consts::FRAC_PI_2);
        assert!(
            right_of_north.distance(Vec3::X) < 1e-5,
            "yaw+90 deg faces +X"
        );
    }
}
