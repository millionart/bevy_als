use crate::state::{RotationMode, Stance};

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
    let angle = angle.rem_euclid(360.0);
    if angle > 180.0 { angle - 360.0 } else { angle }
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
    actor_yaw_degrees: f32,
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
    let remaining_yaw_degrees =
        remap_counterclockwise_degrees(unwind_degrees(target_yaw_degrees - actor_yaw_degrees));
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

pub fn select_dynamic_transition(
    foot_lock_left_distance_squared: f32,
    foot_lock_right_distance_squared: f32,
    left_allowed: bool,
    right_allowed: bool,
    stance: Stance,
    config: &TransitionConfig,
) -> Option<TransitionSelection> {
    if !left_allowed && !right_allowed {
        return None;
    }
    let sequence = if !left_allowed
        || (right_allowed && foot_lock_left_distance_squared < foot_lock_right_distance_squared)
    {
        match stance {
            Stance::Crouching => TransitionSequence::CrouchingRight,
            Stance::Standing => TransitionSequence::StandingRight,
        }
    } else {
        match stance {
            Stance::Crouching => TransitionSequence::CrouchingLeft,
            Stance::Standing => TransitionSequence::StandingLeft,
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
        let threshold_squared = config.dynamic_foot_lock_distance.powi(2);
        assert!(
            select_dynamic_transition(0.0, 0.0, false, false, Stance::Standing, &config).is_none()
        );
        let selected = select_dynamic_transition(
            threshold_squared * 2.0,
            threshold_squared * 3.0,
            true,
            true,
            Stance::Standing,
            &config,
        )
        .unwrap();
        assert_eq!(selected.sequence, TransitionSequence::StandingRight);
        assert_eq!(selected.play_rate, 1.5);
        assert_eq!(selected.blend_duration, 0.2);
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
