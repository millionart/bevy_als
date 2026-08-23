pub const BLEND_IN_DURATION: f32 = 1.0;
pub const BLEND_OUT_DURATION: f32 = 1.0;

use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayBlendPhase {
    Idle,
    BlendingIn,
    Held,
    BlendingOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct OverlayBlendState {
    pub phase: OverlayBlendPhase,
    pub phase_time: f32,
    pub start_weight: f32,
    pub last_weight: f32,
}

impl Default for OverlayBlendState {
    fn default() -> Self {
        Self {
            phase: OverlayBlendPhase::Idle,
            phase_time: 0.0,
            start_weight: 0.0,
            last_weight: 0.0,
        }
    }
}

impl OverlayBlendState {
    pub fn update(
        &mut self,
        target_on: bool,
        dt: f32,
        aim_in_sample: f32,
        aim_out_sample: f32,
    ) -> f32 {
        let w = self.update_inner(target_on, dt, aim_in_sample, aim_out_sample);
        self.last_weight = w;
        w
    }

    fn update_inner(
        &mut self,
        target_on: bool,
        dt: f32,
        aim_in_sample: f32,
        aim_out_sample: f32,
    ) -> f32 {
        let dt = dt.max(0.0);
        match self.phase {
            OverlayBlendPhase::Idle => {
                if target_on {
                    self.phase = OverlayBlendPhase::BlendingIn;
                    self.phase_time = 0.0;
                    self.start_weight = 0.0;
                    return aim_in_sample.min(1.0).max(0.0);
                }
                0.0
            }
            OverlayBlendPhase::BlendingIn => {
                self.phase_time += dt;
                if !target_on {
                    self.start_weight =
                        self.start_weight + (1.0 - self.start_weight) * aim_in_sample;
                    self.phase = OverlayBlendPhase::BlendingOut;
                    self.phase_time = 0.0;
                    return self.start_weight * (1.0 - aim_out_sample.min(1.0).max(0.0));
                }
                if self.phase_time + 1e-4 >= BLEND_IN_DURATION {
                    self.phase = OverlayBlendPhase::Held;
                    self.phase_time = 0.0;
                    return 1.0;
                }
                let s = aim_in_sample.min(1.0).max(0.0);
                self.start_weight + (1.0 - self.start_weight) * s
            }
            OverlayBlendPhase::Held => {
                if !target_on {
                    self.phase = OverlayBlendPhase::BlendingOut;
                    self.phase_time = 0.0;
                    self.start_weight = 1.0;
                    return 1.0 - aim_out_sample.min(1.0).max(0.0);
                }
                1.0
            }
            OverlayBlendPhase::BlendingOut => {
                self.phase_time += dt;
                if target_on {
                    self.start_weight =
                        self.start_weight * (1.0 - aim_out_sample.min(1.0).max(0.0));
                    self.phase = OverlayBlendPhase::BlendingIn;
                    self.phase_time = 0.0;
                    return self.start_weight + (1.0 - self.start_weight) * aim_in_sample;
                }
                if self.phase_time + 1e-4 >= BLEND_OUT_DURATION {
                    self.phase = OverlayBlendPhase::Idle;
                    self.phase_time = 0.0;
                    self.start_weight = 0.0;
                    return 0.0;
                }
                let s = aim_out_sample.min(1.0).max(0.0);
                self.start_weight * (1.0 - s)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn idle_stays_at_zero_without_target() {
        let mut s = OverlayBlendState::default();
        let mut last = 0.0;
        for _ in 0..120 {
            last = s.update(false, DT, 1.0, 1.0);
        }
        assert_eq!(s.phase, OverlayBlendPhase::Idle);
        assert_eq!(last, 0.0);
    }

    #[test]
    fn target_starts_blending_in_and_reaches_held_after_one_second() {
        let mut s = OverlayBlendState::default();
        let mut last = s.update(true, DT, 1.0 / 60.0, 0.0);
        assert_eq!(s.phase, OverlayBlendPhase::BlendingIn);
        for i in 2..=60u32 {
            last = s.update(true, DT, i as f32 / 60.0, 0.0);
        }
        assert_eq!(s.phase, OverlayBlendPhase::BlendingIn);
        last = s.update(true, DT, 1.0, 0.0);
        assert_eq!(s.phase, OverlayBlendPhase::Held);
        assert_eq!(last, 1.0);
    }

    #[test]
    fn blending_in_follows_curve_sample_monotonically() {
        let mut s = OverlayBlendState::default();
        s.update(true, DT, 0.0, 0.0);
        let mut prev = 0.0;
        for i in 1..=30u32 {
            let w = s.update(true, DT, i as f32 / 60.0, 0.0);
            assert!(w > prev, "weight must rise along the in-curve");
            prev = w;
        }
    }

    #[test]
    fn release_mid_ramp_blends_out_continuously_from_live_weight() {
        let mut s = OverlayBlendState::default();
        s.update(true, DT, 0.0, 0.0);
        for i in 1..18u32 {
            s.update(true, DT, i as f32 / 60.0, 0.0);
        }
        let t = s.phase_time;
        assert!(t > 0.0 && t < 1.0);
        let first_out = s.update(false, 0.0, t, 0.0);
        assert_eq!(s.phase, OverlayBlendPhase::BlendingOut);
        assert!(
            (first_out - t).abs() < 1e-4,
            "out must continue from the live in-weight (identity curve)"
        );
        assert_eq!(s.start_weight, t);
    }

    #[test]
    fn full_cycle_returns_to_idle_zero() {
        let mut s = OverlayBlendState::default();
        s.update(true, DT, 0.0, 0.0);
        for i in 1..=61u32 {
            s.update(true, DT, i as f32 / 60.0, 0.0);
        }
        assert_eq!(s.phase, OverlayBlendPhase::Held);
        s.update(false, DT, 0.0, 0.0);
        for i in 1..=61u32 {
            s.update(false, DT, 0.0, i as f32 / 60.0);
        }
        assert_eq!(s.phase, OverlayBlendPhase::Idle);
        assert_eq!(s.update(false, DT, 1.0, 1.0), 0.0);
    }

    #[test]
    fn retrigger_during_out_resumes_from_live_weight() {
        let mut s = OverlayBlendState::default();
        s.update(true, DT, 0.0, 0.0);
        for i in 1..=61u32 {
            s.update(true, DT, i as f32 / 60.0, 0.0);
        }
        s.update(false, DT, 0.0, 0.0);
        for i in 1..31u32 {
            s.update(false, DT, 0.0, i as f32 / 60.0);
        }
        let t = s.phase_time;
        assert!(t > 0.0 && t < 1.0);
        let expected_start = s.start_weight * (1.0 - t);
        let resumed = s.update(true, 0.0, 0.0, t);
        assert_eq!(s.phase, OverlayBlendPhase::BlendingIn);
        assert!((resumed - expected_start).abs() < 1e-4);
        assert_eq!(s.start_weight, expected_start);
    }

    #[test]
    fn negative_dt_does_not_reverse_timers() {
        let mut s = OverlayBlendState::default();
        s.update(true, DT, 0.0, 0.0);
        let t0 = s.phase_time;
        s.update(true, -1.0, 0.5, 0.5);
        assert!(s.phase_time >= t0);
        assert_eq!(s.phase, OverlayBlendPhase::BlendingIn);
    }
}
