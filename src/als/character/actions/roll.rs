use bevy::prelude::*;

use crate::als::input::AlsInputCommand;
use crate::als::motion::AlsViewYaw;
use crate::als::orientation::yaw_to_world_forward;
use crate::als::start_stop::{MoveDirection, classify};
use crate::als::state::{LocomotionState, Stance};
use crate::extensions::profile_blend::{
    AlphaBlendMode, MontageBlend, advance_forward_montage_position,
};

pub const ROLL_MONTAGE_SEQUENCE_LENGTH_SECS: f32 = 1.5;
pub const ROLL_PLAY_RATE: f32 = 1.3;
pub const ROLL_DURATION_SECS: f32 = ROLL_MONTAGE_SEQUENCE_LENGTH_SECS / ROLL_PLAY_RATE;
pub const ROLL_WEIGHT_IN_SECS: f32 = 0.1;
pub const ROLL_WEIGHT_OUT_SECS: f32 = 0.3;
pub const ROLL_ROTATION_HALF_LIFE_SECS: f32 = 0.1;
pub const ROLL_ON_LAND_SPEED_THRESHOLD: f32 = 7.0;
const ROLL_BRANCHING_POINTS: &[f32] = &include!("../../settings/roll_montage_clock.rs");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RollLifecycle {
    pub started: bool,
    pub ended: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RollMontagePlayback {
    position: f32,
    previous_position: f32,
    previous_weight: f32,
    blend: MontageBlend,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsRollState {
    pub active: bool,
    pub time: f32,
    pub previous_time: f32,
    pub natural_blend_start: Option<f32>,
    pub montage: Option<RollMontagePlayback>,
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
            natural_blend_start: None,
            montage: None,
            direction_yaw: 0.0,
            last_direction: MoveDirection::Forward,
            stance_before_roll: Stance::Standing,
        }
    }
}

pub fn select_roll_direction(input_yaw: Option<f32>, basis: &AlsViewYaw) -> (MoveDirection, f32) {
    let yaw = input_yaw.unwrap_or(basis.0);
    let world = yaw_to_world_forward(yaw);
    let direction = input_yaw.map_or(MoveDirection::Forward, |_| {
        classify(Vec2::new(world.x, -world.z), basis.0)
    });
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
    let alpha = 1.0
        - crate::als::animation::inv_exp_approx(
            std::f32::consts::LN_2 / (half_life_seconds + 1e-8) * delta_seconds.max(0.0),
        );
    wrap_angle(current + delta * alpha)
}

impl AlsRollState {
    pub fn weight(&self) -> f32 {
        self.montage.map_or_else(
            || self.elapsed_weight(self.time),
            |montage| montage.blend.weight(),
        )
    }

    fn elapsed_weight(&self, time: f32) -> f32 {
        let alpha = if let Some(start) = self.natural_blend_start {
            let duration = ROLL_DURATION_SECS - start;
            if duration > 0.0 {
                (ROLL_DURATION_SECS - time) / duration
            } else {
                0.0
            }
        } else {
            time / ROLL_WEIGHT_IN_SECS
        }
        .clamp(0.0, 1.0);
        alpha * alpha * (3.0 - 2.0 * alpha)
    }

    pub(crate) fn previous_weight(&self) -> f32 {
        self.montage.map_or_else(
            || self.elapsed_weight(self.previous_time),
            |montage| montage.previous_weight,
        )
    }

    pub(crate) fn clip_time(&self) -> f32 {
        self.montage.map_or_else(
            || (self.time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS),
            |montage| montage.position,
        )
    }

    pub(crate) fn previous_clip_time(&self) -> f32 {
        self.montage.map_or_else(
            || (self.previous_time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS),
            |montage| montage.previous_position,
        )
    }

    pub(crate) fn remaining_play_time(&self) -> f32 {
        (ROLL_MONTAGE_SEQUENCE_LENGTH_SECS - self.clip_time()) / ROLL_PLAY_RATE
    }

    fn begin_playback(&mut self) {
        self.active = true;
        self.time = 0.0;
        self.previous_time = 0.0;
        self.natural_blend_start = None;
        self.montage = None;
    }

    fn playback_at_elapsed(&self) -> RollMontagePlayback {
        let (mut blend, elapsed) = if let Some(start) = self.natural_blend_start {
            (
                MontageBlend::new(
                    1.0,
                    0.0,
                    ROLL_DURATION_SECS - start,
                    AlphaBlendMode::HermiteCubic,
                ),
                (self.time - start).max(0.0),
            )
        } else {
            (
                MontageBlend::new(0.0, 1.0, ROLL_WEIGHT_IN_SECS, AlphaBlendMode::HermiteCubic),
                self.time,
            )
        };
        let weight = blend.advance(elapsed);
        let position = self.clip_time();
        RollMontagePlayback {
            position,
            previous_position: position,
            previous_weight: weight,
            blend,
        }
    }

    pub(crate) fn advance_playback(&mut self, delta: f32, natural_blend: bool) -> bool {
        let mut montage = self.montage.unwrap_or_else(|| self.playback_at_elapsed());
        self.previous_time = self.time;
        self.time += delta;
        montage.previous_position = montage.position;
        montage.previous_weight = montage.blend.weight();
        if natural_blend {
            montage.blend.advance(delta);
        }
        let blend = &mut montage.blend;
        let start = &mut self.natural_blend_start;
        let elapsed = self.time;
        advance_roll_clip(&mut montage.position, delta, |position| {
            let remaining = (ROLL_MONTAGE_SEQUENCE_LENGTH_SECS - position) / ROLL_PLAY_RATE;
            if natural_blend && start.is_none() && remaining <= ROLL_WEIGHT_OUT_SECS {
                *start = Some(elapsed);
                *blend =
                    MontageBlend::new(blend.weight(), 0.0, remaining, AlphaBlendMode::HermiteCubic);
            }
        });
        let ended = natural_blend && start.is_some() && montage.blend.weight() <= 0.0;
        if ended {
            self.active = false;
            self.time = self.time.min(ROLL_DURATION_SECS);
        }
        self.montage = Some(montage);
        ended
    }

    pub fn root_motion_active(&self) -> bool {
        self.active && self.time < ROLL_DURATION_SECS - ROLL_WEIGHT_OUT_SECS
    }

    pub(crate) fn root_motion_active_at_frame_start(&self) -> bool {
        self.active && self.previous_time < ROLL_DURATION_SECS - ROLL_WEIGHT_OUT_SECS
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
        self.begin_playback();
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
        self.previous_time = self.time;
        if self.active {
            let playing = self.root_motion_active();
            lifecycle.ended = self.advance_playback(dt, true);
            if playing {
                return lifecycle;
            }
        }
        if *loco == LocomotionState::Grounded && cmd.roll_pressed {
            self.last_direction = direction.0;
            self.direction_yaw = direction.1;
            self.stance_before_roll = *stance;
            self.begin_playback();
            lifecycle.started = true;
        }
        lifecycle
    }
}

pub(crate) fn advance_roll_clip(position: &mut f32, delta: f32, after_step: impl FnMut(f32)) {
    advance_forward_montage_position(
        position,
        delta,
        ROLL_PLAY_RATE,
        ROLL_MONTAGE_SEQUENCE_LENGTH_SECS,
        ROLL_BRANCHING_POINTS,
        after_step,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roll_uses_native_substeps_and_accumulated_blend_weights() {
        let native: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../docs/audit/bf144/d118b/roll.native.json"
        ))
        .unwrap();
        let samples = native["montage_clock_probe"]["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 14);
        for sample in samples {
            let name = sample["name"].as_str().unwrap();
            let mut state = AlsRollState::default();
            assert!(
                state
                    .update(
                        &base_cmd(),
                        (MoveDirection::Forward, 0.0),
                        &LocomotionState::Grounded,
                        &Stance::Standing,
                        0.0
                    )
                    .started
            );
            for (index, frame) in sample["frames"].as_array().unwrap().iter().enumerate() {
                if state.active {
                    state.advance_playback(frame["delta_seconds"].as_f64().unwrap() as f32, true);
                }
                assert_eq!(
                    state.weight(),
                    frame["weight"].as_f64().unwrap() as f32,
                    "{name}: weight at {index}"
                );
                if frame["playing"].as_bool().unwrap() {
                    assert_eq!(
                        state.clip_time(),
                        frame["position"].as_f64().unwrap() as f32,
                        "{name}: clip position at {index}"
                    );
                }
            }
        }
    }

    #[test]
    fn roll_montage_weight_uses_source_hermite_cubic_blends() {
        let mut state = AlsRollState {
            time: ROLL_WEIGHT_IN_SECS * 0.25,
            ..default()
        };
        assert!((state.weight() - 0.15625).abs() < 1e-6);
        state.natural_blend_start = Some(ROLL_DURATION_SECS - ROLL_WEIGHT_OUT_SECS);
        state.time = ROLL_DURATION_SECS - ROLL_WEIGHT_OUT_SECS * 0.25;
        assert!((state.weight() - 0.15625).abs() < 1e-6);
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
        assert!(
            state
                .update(
                    &base_cmd(),
                    (MoveDirection::Forward, 0.0),
                    &LocomotionState::Grounded,
                    &Stance::Crouching,
                    0.0
                )
                .started
        );
        assert!((ROLL_DURATION_SECS - 1.5 / 1.3).abs() < 1e-6);
    }

    #[test]
    fn montage_blend_windows_match_source_asset() {
        let mut state = AlsRollState::default();
        assert!(state.weight() <= 1e-6);
        state.time = ROLL_WEIGHT_IN_SECS;
        assert!((state.weight() - 1.0).abs() < 1e-6);
        state.time = ROLL_DURATION_SECS;
        state.natural_blend_start = Some(ROLL_DURATION_SECS);
        assert!(state.weight() <= 1e-6);
    }

    #[test]
    fn natural_roll_stop_preserves_weight_and_uses_the_remaining_budget() {
        for crossing_delta in [0.03, 0.07, 0.17] {
            let mut state = AlsRollState {
                active: true,
                time: ROLL_DURATION_SECS - ROLL_WEIGHT_OUT_SECS - 0.02,
                ..default()
            };
            let input = AlsInputCommand::default();
            state.update(
                &input,
                (MoveDirection::Forward, 0.0),
                &LocomotionState::Grounded,
                &Stance::Standing,
                crossing_delta,
            );
            assert_eq!(
                state.weight(),
                1.0,
                "Natural Stop consumed weight in its trigger frame"
            );
            let remaining = ROLL_DURATION_SECS - state.time;
            for (delta, weight) in [
                (remaining * 0.25, 0.84375),
                (remaining * 0.25, 0.5),
                (remaining * 0.5, 0.0),
            ] {
                state.update(
                    &input,
                    (MoveDirection::Forward, 0.0),
                    &LocomotionState::Grounded,
                    &Stance::Standing,
                    delta,
                );
                assert!(
                    (state.weight() - weight).abs() < 1e-5,
                    "Natural Roll lost the source remaining-time budget"
                );
            }
            assert!(!state.active);
            assert!(
                state
                    .update(
                        &base_cmd(),
                        (MoveDirection::Forward, 0.0),
                        &LocomotionState::Grounded,
                        &Stance::Standing,
                        0.0
                    )
                    .started
            );
            assert_eq!(state.weight(), 0.0);
        }
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
        let alpha = 1.0
            - crate::als::animation::inv_exp_approx(
                std::f32::consts::LN_2 / (ROLL_ROTATION_HALF_LIFE_SECS + 1e-8)
                    * ROLL_ROTATION_HALF_LIFE_SECS,
            );
        assert!((after_one_half_life.abs() - std::f32::consts::PI * alpha).abs() < 1e-5);
        assert!((damper_exact_angle(0.0, target, 0.1, 0.0) - target).abs() < 1e-6);
    }

    #[test]
    fn source_roll_damper_preserves_world_turn_across_yaw_bases() {
        let alpha = 1.0
            - crate::als::animation::inv_exp_approx(std::f32::consts::LN_2 / (0.1 + 1e-8) * 0.1);
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
            assert!(
                yaw_to_world_forward(actual).distance(yaw_to_world_forward(expected)) < 1e-5,
                "source target={target_degrees} actual={actual} expected={expected}"
            );
        }
    }

    #[test]
    fn source_landing_roll_threshold_uses_preimpact_velocity() {
        for speed in [6.999, 7.0, 7.001] {
            let mut state = AlsRollState::default();
            let started = state
                .start_from_landing(
                    speed,
                    Some(-std::f32::consts::FRAC_PI_2),
                    0.0,
                    Stance::Standing,
                )
                .started;
            assert_eq!(started, speed >= 7.0);
            if started {
                assert!(roll_displacement_dir(state.direction_yaw).distance(Vec3::X) < 1e-6);
            }
        }
    }
}
