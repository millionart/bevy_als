use crate::state::Stance;

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
pub struct TransitionConfig {
    pub dynamic_play_rate: f32,
    pub blend_duration: f32,
    pub frame_delay: u32,
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            dynamic_play_rate: 1.0,
            blend_duration: 0.2,
            frame_delay: 2,
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
        blend_duration: 0.2,
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
    let sequence = if !left_allowed || (right_allowed && foot_lock_left_distance_squared < foot_lock_right_distance_squared)
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
    fn stance_change_selects_sequence() {
        let s = select_stance_transition(Stance::Crouching, Stance::Standing, false).unwrap();
        assert_eq!(s.sequence, TransitionSequence::CrouchToStand);
        let s = select_stance_transition(Stance::Standing, Stance::Crouching, false).unwrap();
        assert_eq!(s.sequence, TransitionSequence::StandToCrouch);
    }

    #[test]
    fn moving_blocks_idle_only_transition() {
        assert!(select_stance_transition(Stance::Crouching, Stance::Standing, true).is_none());
    }

    #[test]
    fn same_stance_is_none() {
        assert!(select_stance_transition(Stance::Standing, Stance::Standing, false).is_none());
    }

    #[test]
    fn foot_lock_distance_picks_opposite_side() {
        let cfg = TransitionConfig::default();
        let left_locked = select_dynamic_transition(1.0, 100.0, true, true, Stance::Standing, &cfg).unwrap();
        assert_eq!(left_locked.sequence, TransitionSequence::StandingRight);
        let right_locked = select_dynamic_transition(100.0, 1.0, true, true, Stance::Standing, &cfg).unwrap();
        assert_eq!(right_locked.sequence, TransitionSequence::StandingLeft);
    }

    #[test]
    fn equal_foot_lock_defaults_left() {
        let cfg = TransitionConfig::default();
        let sel = select_dynamic_transition(50.0, 50.0, true, true, Stance::Standing, &cfg).unwrap();
        assert_eq!(sel.sequence, TransitionSequence::StandingLeft);
    }

    #[test]
    fn disallowed_side_forces_other() {
        let cfg = TransitionConfig::default();
        let sel = select_dynamic_transition(1.0, 100.0, false, true, Stance::Crouching, &cfg).unwrap();
        assert_eq!(sel.sequence, TransitionSequence::CrouchingRight);
        let sel = select_dynamic_transition(100.0, 1.0, true, false, Stance::Crouching, &cfg).unwrap();
        assert_eq!(sel.sequence, TransitionSequence::CrouchingLeft);
    }

    #[test]
    fn cooldown_frames() {
        let cfg = TransitionConfig::default();
        let mut cd = TransitionCooldown::default();
        assert!(!cd.is_active());
        cd.arm(&cfg);
        assert!(cd.is_active());
        cd.tick();
        assert!(cd.is_active());
        cd.tick();
        assert!(!cd.is_active());
    }
}
