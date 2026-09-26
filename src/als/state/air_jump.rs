use crate::als::params::AlsMovementParams;
use crate::als::state::Gait;

pub const JUMP_WALK_RUN_SPEED_SPLIT: f32 = 2.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpStartVariant {
    WalkLeft,
    WalkRight,
    RunLeft,
    RunRight,
}

impl JumpStartVariant {
    pub fn clip_path(self) -> &'static str {
        match self {
            Self::WalkLeft => "animations/Air/Jump/A_Als_Jump_Walk_Left.anim.ron",
            Self::WalkRight => "animations/Air/Jump/A_Als_Jump_Walk_Right.anim.ron",
            Self::RunLeft => "animations/Air/Jump/A_Als_Jump_Run_Left.anim.ron",
            Self::RunRight => "animations/Air/Jump/A_Als_Jump_Run_Right.anim.ron",
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

pub fn jump_play_rate(speed_cm_per_s: f32) -> f32 {
    lerp(1.2, 1.5, speed_cm_per_s / 600.0)
}

pub fn select_jump_start_variant(
    _params: &AlsMovementParams,
    _gait: Gait,
    horizontal_speed: f32,
    left_foot_planted: bool,
) -> JumpStartVariant {
    let is_run = horizontal_speed >= JUMP_WALK_RUN_SPEED_SPLIT;
    if is_run {
        if left_foot_planted {
            JumpStartVariant::RunRight
        } else {
            JumpStartVariant::RunLeft
        }
    } else if left_foot_planted {
        JumpStartVariant::WalkRight
    } else {
        JumpStartVariant::WalkLeft
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_play_rate_matches_als_ramp() {
        assert!((jump_play_rate(0.0) - 1.2).abs() < 1e-5);
        assert!((jump_play_rate(300.0) - 1.35).abs() < 1e-5);
        assert!((jump_play_rate(600.0) - 1.5).abs() < 1e-5);
        assert!((jump_play_rate(1200.0) - 1.5).abs() < 1e-5);
    }

    #[test]
    fn variant_splits_on_speed_threshold() {
        let p = AlsMovementParams::default();
        let v_walk = select_jump_start_variant(&p, Gait::Walking, 1.0, false);
        assert_eq!(v_walk, JumpStartVariant::WalkLeft);
        let v_run = select_jump_start_variant(&p, Gait::Running, 3.75, false);
        assert_eq!(v_run, JumpStartVariant::RunLeft);
        let v_run_r = select_jump_start_variant(&p, Gait::Running, 3.75, true);
        assert_eq!(v_run_r, JumpStartVariant::RunRight);
    }

    #[test]
    fn sprint_always_selects_run_variant() {
        let p = AlsMovementParams::default();
        let v = select_jump_start_variant(&p, Gait::Sprinting, 6.5, false);
        assert_eq!(v, JumpStartVariant::RunLeft);
    }

    #[test]
    fn foot_alternation_swaps_side() {
        let p = AlsMovementParams::default();
        let a = select_jump_start_variant(&p, Gait::Running, 3.0, false);
        let b = select_jump_start_variant(&p, Gait::Running, 3.0, true);
        assert_ne!(a, b);
    }
}
