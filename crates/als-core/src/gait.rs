use crate::params::AlsMovementParams;
use crate::state::{Gait, Stance};
use crate::stride_curves::{stride_curve_run, stride_curve_walk};

pub const STRIDE_BLEND_RANGE_MAX: f32 = 1.0;
pub const PLAY_RATE_MAX: f32 = 3.0;
pub const CROUCH_PLAY_RATE_MAX: f32 = 2.0;
pub const GAIT_CROSSFADE_SECS: f32 = 0.25;
pub const SPEED_EMA_HALF_LIFE_SECS: f32 = 0.08;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsPlayRate {
    pub play_rate: f32,
    pub stride_blend: f32,
}

pub fn stride_blend_amount(running_amount: f32) -> f32 {
    stride_blend_amount_for_speed(running_amount, 0.0)
}

pub fn stride_blend_amount_for_speed(running_amount: f32, speed_ms: f32) -> f32 {
    // ALS: StrideBlendAmount = Curve(Speed) lerped by RunningAmount, with
    // Speed in cm/s (walk curve tmax=150, run curve tmax=350). This crate
    // uses m/s throughout, so convert before sampling.
    let speed_cm = speed_ms * 100.0;
    let w = stride_curve_walk(speed_cm);
    let r = stride_curve_run(speed_cm);
    let blend = running_amount.clamp(0.0, 1.0);
    w * (1.0 - blend) + r * blend
}

#[allow(dead_code)]
fn unweighted_gait_amount(gait: Gait) -> f32 {
    match gait {
        Gait::Walking => 0.0,
        Gait::Running => 0.5,
        Gait::Sprinting => 1.0,
    }
}

pub fn animated_gait_speed(params: &AlsMovementParams, stance: Stance, gait: Gait) -> f32 {
    let s = params.stance_speeds(stance);
    match gait {
        Gait::Walking => s.animated_walk_speed,
        Gait::Running => s.animated_run_speed,
        Gait::Sprinting => s.animated_sprint_speed,
    }
}

fn effective_gait(stance: Stance, gait: Gait) -> Gait {
    match stance {
        Stance::Standing => gait,
        Stance::Crouching => Gait::Walking,
    }
}

pub fn als_play_rate(
    params: &AlsMovementParams,
    stance: Stance,
    gait: Gait,
    speed_ema: f32,
    stride_blend: f32,
) -> AlsPlayRate {
    if stride_blend < 1e-4 {
        return AlsPlayRate {
            play_rate: 1.0,
            stride_blend,
        };
    }
    if stance == Stance::Crouching {
        let animated = params.crouch.animated_crouch_speed;
        if animated < 1e-4 {
            return AlsPlayRate {
                play_rate: 1.0,
                stride_blend,
            };
        }
        let rate = (speed_ema / animated / stride_blend).min(CROUCH_PLAY_RATE_MAX);
        AlsPlayRate {
            play_rate: rate.max(0.0),
            stride_blend,
        }
    } else if effective_gait(stance, gait) == Gait::Walking {
        let animated = params.speeds.standing.animated_walk_speed;
        if animated < 1e-4 {
            return AlsPlayRate {
                play_rate: 1.0,
                stride_blend,
            };
        }
        let rate = (speed_ema / animated / stride_blend).min(PLAY_RATE_MAX);
        AlsPlayRate {
            play_rate: rate.max(0.0),
            stride_blend,
        }
    } else {
        let animated = animated_gait_speed(params, stance, gait);
        if animated < 1e-4 {
            return AlsPlayRate {
                play_rate: 1.0,
                stride_blend,
            };
        }
        let rate = (speed_ema / animated / stride_blend).min(PLAY_RATE_MAX);
        AlsPlayRate {
            play_rate: rate.max(0.0),
            stride_blend,
        }
    }
}

pub fn ema_toward(current: f32, target: f32, dt: f32, half_life: f32) -> f32 {
    if half_life <= 1e-4 {
        return target;
    }
    let alpha = 1.0 - 0.5f32.powf(dt / half_life);
    current + (target - current) * alpha
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaitWeights {
    pub move_w: f32,
    pub run_w: f32,
    pub sprint_w: f32,
}

impl Default for GaitWeights {
    fn default() -> Self {
        Self {
            move_w: 0.0,
            run_w: 0.0,
            sprint_w: 0.0,
        }
    }
}

impl GaitWeights {
    pub fn targets(gait: Gait) -> Self {
        match gait {
            Gait::Walking => Self {
                move_w: 1.0,
                run_w: 0.0,
                sprint_w: 0.0,
            },
            Gait::Running => Self {
                move_w: 1.0,
                run_w: 1.0,
                sprint_w: 0.0,
            },
            Gait::Sprinting => Self {
                move_w: 1.0,
                run_w: 1.0,
                sprint_w: 1.0,
            },
        }
    }

    pub fn advance(&self, gait: Gait, moving: bool, dt: f32) -> Self {
        let t = if moving {
            Self::targets(gait)
        } else {
            Self::default()
        };
        let step = |cur: f32, tgt: f32| -> f32 {
            let d = GAIT_CROSSFADE_SECS.max(1e-4);
            if cur < tgt {
                (cur + dt / d).min(tgt)
            } else {
                (cur - dt / d).max(tgt)
            }
        };
        Self {
            move_w: step(self.move_w, t.move_w),
            run_w: step(self.run_w, t.run_w),
            sprint_w: step(self.sprint_w, t.sprint_w),
        }
    }

    pub fn settled_target(&self, gait: Gait, moving: bool) -> Self {
        if moving {
            Self::targets(gait)
        } else {
            Self::default()
        }
    }

    pub fn running_amount(&self) -> f32 {
        self.run_w.min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_play_rate_matches_als_curve() {
        let p = AlsMovementParams::default();
        // walk at its full animated speed: walk curve saturates at 1.0
        let stride = stride_blend_amount_for_speed(0.0, 1.5);
        assert!((stride - 1.0).abs() < 1e-3);
        let pr = als_play_rate(&p, Stance::Standing, Gait::Walking, 1.5, stride);
        assert!((pr.play_rate - 1.5 / 1.5 / stride).abs() < 1e-4);
        assert!((pr.stride_blend - stride).abs() < 1e-6);
    }

    #[test]
    fn standing_walk_uses_standing_animated_speed() {
        let mut p = AlsMovementParams::default();
        p.speeds.standing.animated_walk_speed = 2.0;
        p.crouch.animated_crouch_speed = 0.5;
        let result = als_play_rate(&p, Stance::Standing, Gait::Walking, 1.0, 1.0);
        assert!((result.play_rate - 0.5).abs() < 1e-6);
    }

    #[test]
    fn stride_curves_start_at_als_minimum() {
        // ALS curves start at 0.2 at zero speed (both Walk and Run)
        let s0 = stride_blend_amount(0.0);
        assert!((s0 - 0.2).abs() < 1e-4);
        let s1 = stride_blend_amount(1.0);
        assert!((s1 - 0.2).abs() < 1e-4);
    }

    #[test]
    fn stride_curve_mid_speed_between_bounds() {
        // mid-band sample must sit strictly between the 0.2 floor and 1.0 cap
        let s = stride_blend_amount_for_speed(0.5, 0.75);
        assert!(s > 0.2 && s < 1.0);
    }

    #[test]
    fn run_play_rate_uses_run_stride_and_animated_speed() {
        let p = AlsMovementParams::default();
        // run at half animated speed: run curve at 175/350 = mid table
        let stride = stride_blend_amount_for_speed(1.0, 3.5);
        let pr = als_play_rate(&p, Stance::Standing, Gait::Running, 3.5, stride);
        assert!((pr.play_rate - 3.5 / 3.5 / stride).abs() < 1e-4);
        assert!((pr.stride_blend - stride).abs() < 1e-6);
        let pr_sprint = als_play_rate(&p, Stance::Standing, Gait::Sprinting, 6.0, stride);
        assert!((pr_sprint.play_rate - 6.0 / 6.0 / stride).abs() < 1e-4);
    }

    #[test]
    fn sprint_play_rate_clamps_at_three() {
        let p = AlsMovementParams::default();
        let stride = stride_blend_amount_for_speed(1.0, 6.0);
        let pr = als_play_rate(&p, Stance::Standing, Gait::Sprinting, 6.5, stride);
        assert!((pr.play_rate - 6.5 / 6.0 / stride).abs() < 1e-4);
        let pr_extreme = als_play_rate(&p, Stance::Standing, Gait::Sprinting, 60.0, stride);
        assert!((pr_extreme.play_rate - PLAY_RATE_MAX).abs() < 1e-5);
    }

    #[test]
    fn crouch_play_rate_clamps_at_two() {
        let p = AlsMovementParams::default();
        let stride = stride_blend_amount_for_speed(0.0, 1.5);
        let pr = als_play_rate(&p, Stance::Crouching, Gait::Walking, 1.5, stride);
        let expected = 1.5 / 1.5 / stride;
        assert!((pr.play_rate - expected).abs() < 1e-4);
        let pr_over = als_play_rate(&p, Stance::Crouching, Gait::Sprinting, 90.0, stride);
        assert!((pr_over.play_rate - CROUCH_PLAY_RATE_MAX).abs() < 1e-5);
    }

    #[test]
    fn near_idle_speed_gives_near_zero_rate() {
        let p = AlsMovementParams::default();
        let pr = als_play_rate(
            &p,
            Stance::Standing,
            Gait::Running,
            0.05,
            stride_blend_amount(1.0),
        );
        assert!(pr.play_rate < 0.2);
    }

    #[test]
    fn ema_converges_exponentially() {
        let v = ema_toward(0.0, 10.0, 0.08, 0.08);
        assert!((v - 5.0).abs() < 1e-4);
        let v2 = ema_toward(v, 10.0, 0.08, 0.08);
        assert!((v2 - 7.5).abs() < 1e-3);
    }

    #[test]
    fn weights_advance_discretely_per_gait() {
        let w = GaitWeights::default().advance(Gait::Walking, true, 0.25);
        assert_eq!(w, GaitWeights::targets(Gait::Walking));
        let w2 = w.advance(Gait::Running, true, 0.125);
        assert!((w2.move_w - 1.0).abs() < 1e-6);
        assert!((w2.run_w - 0.5).abs() < 1e-6);
        assert_eq!(w2.sprint_w, 0.0);
    }

    #[test]
    fn weights_fall_back_to_idle_when_stopped() {
        let start = GaitWeights::targets(Gait::Sprinting);
        let mid = start.advance(Gait::Sprinting, false, 0.125);
        assert!((mid.move_w - 0.5).abs() < 1e-6);
        assert!((mid.run_w - 0.5).abs() < 1e-6);
        assert!((mid.sprint_w - 0.5).abs() < 1e-6);
    }
}
