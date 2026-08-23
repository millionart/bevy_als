use bevy::prelude::*;

use crate::input::AlsInputCommand;
use crate::state::{LocomotionState, Stance};

pub const MANTLE_HIGH_MIN_HEIGHT: f32 = 1.0;
pub const MANTLE_MAX_LEDGE_HEIGHT: f32 = 1.6;
pub const MANTLE_MIN_LEDGE_HEIGHT: f32 = 0.45;
pub const MANTLE_PROBE_REACH: f32 = 0.7;
pub const MANTLE_HIGH_DURATION: f32 = 2.667;
pub const MANTLE_LOW_DURATION: f32 = 1.5;
const WEIGHT_IN_SECS: f32 = 0.15;
const WEIGHT_OUT_SECS: f32 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MantleKind {
    #[default]
    High,
    LowLeft,
    LowRight,
}

impl MantleKind {
    pub fn duration(self) -> f32 {
        match self {
            MantleKind::High => MANTLE_HIGH_DURATION,
            MantleKind::LowLeft | MantleKind::LowRight => MANTLE_LOW_DURATION,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LedgeProbe {
    pub face_distance: f32,
    pub top_height: f32,
    pub lateral_sign: f32,
}

pub fn classify_mantle(probe: &LedgeProbe) -> Option<MantleKind> {
    if probe.top_height < MANTLE_MIN_LEDGE_HEIGHT || probe.top_height > MANTLE_MAX_LEDGE_HEIGHT {
        return None;
    }
    if probe.face_distance > MANTLE_PROBE_REACH {
        return None;
    }
    if probe.top_height >= MANTLE_HIGH_MIN_HEIGHT {
        Some(MantleKind::High)
    } else if probe.lateral_sign >= 0.0 {
        Some(MantleKind::LowRight)
    } else {
        Some(MantleKind::LowLeft)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MantleMotion {
    pub forward_frac: f32,
    pub up_frac: f32,
}

pub fn mantle_motion_at(time_norm: f32) -> MantleMotion {
    let t = time_norm.clamp(0.0, 1.0);
    let up = match t {
        x if x < 0.5 => smoothstep(x / 0.5),
        _ => 1.0 - 0.25 * smoothstep((t - 0.5) / 0.5),
    };
    let fwd = match t {
        x if x < 0.35 => 0.0,
        x if x < 0.85 => smoothstep((x - 0.35) / 0.5),
        _ => 1.0,
    };
    MantleMotion {
        forward_frac: fwd,
        up_frac: up.clamp(0.0, 1.0),
    }
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

pub fn mantle_weight_at(elapsed: f32, duration: f32) -> f32 {
    let remaining = (duration - elapsed).max(0.0);
    let w_in = if WEIGHT_IN_SECS <= 0.0 {
        1.0
    } else {
        (elapsed / WEIGHT_IN_SECS).clamp(0.0, 1.0)
    };
    let w_out = if WEIGHT_OUT_SECS <= 0.0 {
        1.0
    } else {
        (remaining / WEIGHT_OUT_SECS).clamp(0.0, 1.0)
    };
    w_in.min(w_out)
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsMantleState {
    pub active: bool,
    pub kind: MantleKind,
    pub time: f32,
    pub duration: f32,
    pub start: Vec3,
    pub target: Vec3,
    pub cooldown: f32,
}

impl Default for AlsMantleState {
    fn default() -> Self {
        Self {
            active: false,
            kind: MantleKind::High,
            time: 0.0,
            duration: MANTLE_LOW_DURATION,
            start: Vec3::ZERO,
            target: Vec3::ZERO,
            cooldown: 0.0,
        }
    }
}

impl AlsMantleState {
    pub fn begin(&mut self, kind: MantleKind, start: Vec3, target: Vec3) {
        self.active = true;
        self.kind = kind;
        self.duration = kind.duration();
        self.time = 0.0;
        self.start = start;
        self.target = target;
    }

    pub fn tick(&mut self, cmd: &AlsInputCommand, loco: &LocomotionState, stance: &Stance, dt: f32) {
        self.cooldown = (self.cooldown - dt).max(0.0);
        if !self.active {
            return;
        }
        let abort = *loco != LocomotionState::Air && *loco != LocomotionState::Grounded
            || *stance == Stance::Crouching
            || cmd.roll_pressed;
        if abort {
            self.active = false;
            self.time = 0.0;
            self.cooldown = 0.4;
            return;
        }
        self.time += dt;
        if self.time >= self.duration {
            self.active = false;
            self.time = 0.0;
            self.cooldown = 0.3;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(h: f32, dist: f32, lateral: f32) -> LedgeProbe {
        LedgeProbe {
            face_distance: dist,
            top_height: h,
            lateral_sign: lateral,
        }
    }

    #[test]
    fn height_bands_select_high_low_or_reject() {
        assert_eq!(classify_mantle(&probe(1.3, 0.5, 1.0)), Some(MantleKind::High));
        assert_eq!(
            classify_mantle(&probe(0.8, 0.5, 0.4)),
            Some(MantleKind::LowRight)
        );
        assert_eq!(
            classify_mantle(&probe(0.8, 0.5, -0.4)),
            Some(MantleKind::LowLeft)
        );
        assert_eq!(classify_mantle(&probe(0.2, 0.5, 1.0)), None);
        assert_eq!(classify_mantle(&probe(1.9, 0.5, 1.0)), None);
        assert_eq!(classify_mantle(&probe(0.9, 1.5, 1.0)), None);
    }

    #[test]
    fn motion_profile_rises_first_then_moves_forward() {
        let early = mantle_motion_at(0.25);
        assert!(early.up_frac > 0.4 && early.forward_frac < 0.05);
        let mid = mantle_motion_at(0.6);
        assert!(mid.up_frac > 0.85 && mid.forward_frac > 0.3);
        let end = mantle_motion_at(1.0);
        assert!((end.up_frac - 0.75).abs() < 1e-3);
        assert!((end.forward_frac - 1.0).abs() < 1e-3);
    }

    #[test]
    fn weight_envelope_fades_before_clip_end() {
        assert!(mantle_weight_at(0.0, 1.5) <= 0.01);
        assert!((mantle_weight_at(0.5, 1.5) - 1.0).abs() < 1e-4);
        assert!(mantle_weight_at(1.5, 1.5) <= 0.01);
    }

    #[test]
    fn state_times_out_and_sets_cooldown() {
        let mut s = AlsMantleState::default();
        s.begin(MantleKind::LowLeft, Vec3::ZERO, Vec3::ONE);
        assert!(s.active);
        let cmd = AlsInputCommand::default();
        let iters = (MANTLE_LOW_DURATION / (1.0 / 60.0)) as usize + 2;
        for _ in 0..iters {
            s.tick(
                &cmd,
                &LocomotionState::Grounded,
                &Stance::Standing,
                1.0 / 60.0,
            );
        }
        assert!(!s.active);
        assert!(s.cooldown > 0.0);
    }
}
