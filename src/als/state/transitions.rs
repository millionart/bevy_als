use crate::als::{
    foot_ik::is_relevant_animation_weight,
    state::{RotationMode, Stance},
};

pub const QUICK_STOP_BLEND_IN_SECS: f32 = 0.1;
pub const QUICK_STOP_BLEND_OUT_SECS: f32 = 0.2;
pub const QUICK_STOP_MIN_PLAY_RATE: f32 = 1.75;
pub const QUICK_STOP_MAX_PLAY_RATE: f32 = 3.0;
pub const QUICK_STOP_START_TIME_SECS: f32 = 0.3;
pub const DYNAMIC_TRANSITION_FOOT_LOCK_DISTANCE_METERS: f32 = 0.08;
pub const DYNAMIC_TRANSITION_BLEND_SECS: f32 = 0.2;
pub const DYNAMIC_TRANSITION_PLAY_RATE: f32 = 1.5;
pub const DYNAMIC_TRANSITION_FRAME_DELAY: u32 = 2;
pub const COUNTERCLOCKWISE_REMAP_THRESHOLD_DEGREES: f32 = 175.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionSequence {
    StandingLeft,
    StandingRight,
    CrouchingLeft,
    CrouchingRight,
    CrouchToStand,
    StandToCrouch,
}

impl TransitionSequence {
    pub fn asset_stem(self) -> &'static str {
        match self {
            Self::StandingLeft => "A_Als_Stand_DynamicTransition_Left",
            Self::StandingRight => "A_Als_Stand_DynamicTransition_Right",
            Self::CrouchingLeft => "A_Als_Crouch_DynamicTransition_Left",
            Self::CrouchingRight => "A_Als_Crouch_DynamicTransition_Right",
            Self::CrouchToStand => "A_Als_CrouchToStand",
            Self::StandToCrouch => "A_Als_StandToCrouch",
        }
    }

    pub fn is_dynamic(self) -> bool {
        !matches!(self, Self::CrouchToStand | Self::StandToCrouch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionSelection {
    pub sequence: TransitionSequence,
    pub play_rate: f32,
    pub blend_duration: f32,
    pub start_time: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuickStopSelection {
    pub right: bool,
    pub play_rate: f32,
    pub blend_in_duration: f32,
    pub blend_out_duration: f32,
    pub start_time: f32,
    pub remaining_yaw_degrees: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionConfig {
    pub dynamic_foot_lock_distance: f32,
    pub dynamic_play_rate: f32,
    pub blend_duration: f32,
    pub frame_delay: u32,
    pub quick_stop_blend_in_duration: f32,
    pub quick_stop_blend_out_duration: f32,
    pub quick_stop_min_play_rate: f32,
    pub quick_stop_max_play_rate: f32,
    pub quick_stop_start_time: f32,
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            dynamic_foot_lock_distance: DYNAMIC_TRANSITION_FOOT_LOCK_DISTANCE_METERS,
            dynamic_play_rate: DYNAMIC_TRANSITION_PLAY_RATE,
            blend_duration: DYNAMIC_TRANSITION_BLEND_SECS,
            frame_delay: DYNAMIC_TRANSITION_FRAME_DELAY,
            quick_stop_blend_in_duration: QUICK_STOP_BLEND_IN_SECS,
            quick_stop_blend_out_duration: QUICK_STOP_BLEND_OUT_SECS,
            quick_stop_min_play_rate: QUICK_STOP_MIN_PLAY_RATE,
            quick_stop_max_play_rate: QUICK_STOP_MAX_PLAY_RATE,
            quick_stop_start_time: QUICK_STOP_START_TIME_SECS,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct TransitionCooldown {
    frames_remaining: u32,
}

impl TransitionCooldown {
    pub fn tick(&mut self) {
        self.frames_remaining = self.frames_remaining.saturating_sub(1);
    }

    pub fn is_active(&self) -> bool {
        self.frames_remaining > 0
    }

    pub fn arm(&mut self, config: &TransitionConfig) {
        self.frames_remaining = config.frame_delay;
    }
}

pub fn unwind_degrees(angle: f32) -> f32 {
    let angle = if angle.abs() > 360.0 {
        angle % 360.0
    } else {
        angle
    };
    crate::als::head::unwind_degrees(angle)
}

pub fn remap_counterclockwise_degrees(angle: f32) -> f32 {
    if angle > COUNTERCLOCKWISE_REMAP_THRESHOLD_DEGREES {
        angle - 360.0
    } else {
        angle
    }
}

pub fn select_quick_stop(
    rotation_mode: RotationMode,
    actor_yaw_degrees: f64,
    target_yaw_degrees: f32,
    config: &TransitionConfig,
) -> QuickStopSelection {
    if rotation_mode != RotationMode::VelocityDirection {
        return QuickStopSelection {
            right: false,
            play_rate: config.quick_stop_min_play_rate,
            blend_in_duration: config.quick_stop_blend_in_duration,
            blend_out_duration: config.quick_stop_blend_out_duration,
            start_time: config.quick_stop_start_time,
            remaining_yaw_degrees: 0.0,
        };
    }
    let remaining_yaw_degrees = remap_counterclockwise_degrees(unwind_degrees(
        (f64::from(target_yaw_degrees) - actor_yaw_degrees) as f32,
    ));
    let rate_alpha = remaining_yaw_degrees.abs() / 180.0;
    QuickStopSelection {
        right: remaining_yaw_degrees > 0.0,
        play_rate: config.quick_stop_min_play_rate
            + (config.quick_stop_max_play_rate - config.quick_stop_min_play_rate) * rate_alpha,
        blend_in_duration: config.quick_stop_blend_in_duration,
        blend_out_duration: config.quick_stop_blend_out_duration,
        start_time: config.quick_stop_start_time,
        remaining_yaw_degrees,
    }
}

pub fn select_stance_transition(
    from: Stance,
    to: Stance,
    moving: bool,
) -> Option<TransitionSelection> {
    if from == to || moving {
        return None;
    }
    let sequence = match (from, to) {
        (Stance::Crouching, Stance::Standing) => TransitionSequence::CrouchToStand,
        (Stance::Standing, Stance::Crouching) => TransitionSequence::StandToCrouch,
        _ => return None,
    };
    Some(TransitionSelection {
        sequence,
        play_rate: 1.0,
        blend_duration: DYNAMIC_TRANSITION_BLEND_SECS,
        start_time: 0.0,
    })
}

pub(crate) fn foot_lock_distance_squared_cm(
    target: impl Into<bevy::math::DVec3>,
    locked: impl Into<bevy::math::DVec3>,
) -> f64 {
    let [x, y, z] = (target.into() * 100.0 - locked.into() * 100.0).to_array();
    x * x + z * z + y * y
}

pub fn select_dynamic_transition(
    distances_squared_cm: [f64; 2],
    lock_amounts: [f32; 2],
    scale_world_space: f32,
    stance: Stance,
    config: &TransitionConfig,
) -> Option<TransitionSelection> {
    let threshold_cm =
        (f64::from(config.dynamic_foot_lock_distance) * 100.0) as f32 * scale_world_space;
    let threshold_squared = f64::from(threshold_cm * threshold_cm);
    let [left_distance, right_distance] = distances_squared_cm;
    let left_allowed =
        is_relevant_animation_weight(lock_amounts[0]) && left_distance > threshold_squared;
    let right_allowed =
        is_relevant_animation_weight(lock_amounts[1]) && right_distance > threshold_squared;
    if !left_allowed && !right_allowed {
        return None;
    }
    let sequence = if !left_allowed || (right_allowed && left_distance < right_distance) {
        match stance {
            Stance::Crouching => TransitionSequence::CrouchingRight,
            _ => TransitionSequence::StandingRight,
        }
    } else {
        match stance {
            Stance::Crouching => TransitionSequence::CrouchingLeft,
            _ => TransitionSequence::StandingLeft,
        }
    };
    Some(TransitionSelection {
        sequence,
        play_rate: config.dynamic_play_rate,
        blend_duration: config.blend_duration,
        start_time: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec3;

    #[test]
    fn source_config_matches_animation_instance_settings() {
        let config = TransitionConfig::default();
        assert_eq!(config.dynamic_foot_lock_distance, 0.08);
        assert_eq!(config.dynamic_play_rate, 1.5);
        assert_eq!(config.blend_duration, 0.2);
        assert_eq!(config.frame_delay, 2);
        assert_eq!(config.quick_stop_blend_in_duration, 0.1);
        assert_eq!(config.quick_stop_blend_out_duration, 0.2);
        assert_eq!(config.quick_stop_min_play_rate, 1.75);
        assert_eq!(config.quick_stop_max_play_rate, 3.0);
        assert_eq!(config.quick_stop_start_time, 0.3);
    }

    #[test]
    fn quick_stop_matches_source_direction_and_rate_rules() {
        let config = TransitionConfig::default();
        let aiming = select_quick_stop(RotationMode::Aiming, 0.0, 150.0, &config);
        assert!(!aiming.right && aiming.play_rate == 1.75);
        let right = select_quick_stop(RotationMode::VelocityDirection, 0.0, 90.0, &config);
        assert!(right.right && (right.play_rate - 2.375).abs() < 1e-6);
        let left = select_quick_stop(RotationMode::VelocityDirection, 0.0, -90.0, &config);
        assert!(!left.right && (left.play_rate - 2.375).abs() < 1e-6);
        let counterclockwise =
            select_quick_stop(RotationMode::VelocityDirection, 0.0, 180.0, &config);
        assert!(!counterclockwise.right && (counterclockwise.play_rate - 3.0).abs() < 1e-6);
        let remapped = select_quick_stop(RotationMode::VelocityDirection, 0.0, 176.0, &config);
        assert!(!remapped.right && (remapped.play_rate - 3.0277777).abs() < 1e-5);
    }

    #[test]
    fn dynamic_transition_requires_source_threshold_and_uses_opposite_side() {
        let config = TransitionConfig::default();
        assert!(
            select_dynamic_transition([64.0, 64.0], [1.0, 1.0], 1.0, Stance::Standing, &config)
                .is_none()
        );
        let selected =
            select_dynamic_transition([128.0, 192.0], [1.0, 1.0], 1.0, Stance::Standing, &config)
                .unwrap();
        assert_eq!(selected.sequence, TransitionSequence::StandingRight);
        assert_eq!(selected.play_rate, 1.5);
        assert_eq!(selected.blend_duration, 0.2);
    }

    #[test]
    fn dynamic_transition_compares_native_distance_precision_for_both_stances() {
        use crate::als::foot_ik::{AlsDynamicTransitionState, AlsFootLockState};

        #[derive(serde::Deserialize)]
        struct Fixture {
            cases: Vec<Case>,
        }

        #[derive(serde::Deserialize)]
        struct Case {
            name: String,
            stance: String,
            scale: f32,
            amounts: [f32; 2],
            targets: [[f32; 3]; 2],
            locks: [[f32; 3]; 2],
            source_distances_squared_cm: [f64; 2],
            expected: Option<String>,
        }

        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../tests/fixtures/actions/transition_distance.json"
        ))
        .unwrap();
        let config = TransitionConfig::default();
        for case in fixture.cases {
            let stance = if case.stance == "crouching" {
                Stance::Crouching
            } else {
                Stance::Standing
            };
            let expected = match (stance, case.expected.as_deref()) {
                (_, None) => None,
                (Stance::Crouching, Some("left")) => Some(TransitionSequence::CrouchingLeft),
                (Stance::Crouching, Some("right")) => Some(TransitionSequence::CrouchingRight),
                (Stance::Standing, Some("left")) => Some(TransitionSequence::StandingLeft),
                (Stance::Standing, Some("right")) => Some(TransitionSequence::StandingRight),
                _ => panic!("invalid dynamic transition fixture"),
            };
            let mut state = AlsFootLockState {
                valid: true,
                ..Default::default()
            };
            let mut distances = [0.0; 2];
            for (index, foot) in [&mut state.left, &mut state.right].into_iter().enumerate() {
                foot.target_location = Vec3::from_array(case.targets[index]).as_dvec3();
                foot.lock_location = Vec3::from_array(case.locks[index]).as_dvec3();
                foot.lock_amount = case.amounts[index];
                distances[index] =
                    foot_lock_distance_squared_cm(foot.target_location, foot.lock_location);
            }
            assert_eq!(distances, case.source_distances_squared_cm, "{}", case.name);
            let actual = AlsDynamicTransitionState::default()
                .update(&state, true, case.scale, stance, &config);
            assert_eq!(
                actual.map(|selection| selection.sequence),
                expected,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn cooldown_blocks_exactly_two_frames() {
        let config = TransitionConfig::default();
        let mut cooldown = TransitionCooldown::default();
        cooldown.arm(&config);
        assert!(cooldown.is_active());
        cooldown.tick();
        assert!(cooldown.is_active());
        cooldown.tick();
        assert!(!cooldown.is_active());
    }
}
