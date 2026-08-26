use bevy::prelude::*;

use crate::character::AlsCharacter;
use crate::input::AlsInputCommand;
use crate::state::{LocomotionState, Stance};
use avian3d::prelude::{LinearVelocity, SpatialQuery, SpatialQueryFilter};

pub const MANTLE_HIGH_MIN_HEIGHT: f32 = 1.25;
pub const MANTLE_MAX_LEDGE_HEIGHT: f32 = 1.6;
pub const MANTLE_MIN_LEDGE_HEIGHT: f32 = 0.45;
pub const MANTLE_PROBE_REACH: f32 = 0.7;
pub const MANTLE_PROBE_SHIFT: f32 = 0.45;
pub const MANTLE_TARGET_INSET: f32 = 0.45;
pub const MANTLE_HIGH_DURATION: f32 = 2.667;
pub const MANTLE_LOW_DURATION: f32 = 1.5;
pub const MANTLE_SLOPE_ANGLE_COS: f32 = 0.819;
pub const MANTLE_TARGET_SPEED_THRESHOLD: f32 = 0.1;
pub const MANTLE_HEADROOM_CLEARANCE: f32 = 1.85;
const WEIGHT_IN_SECS: f32 = 0.15;
const WEIGHT_OUT_SECS: f32 = 0.25;

pub const MANTLE_WARP_SECS: f32 = 0.3;
pub const MANTLE_SETTLE_SECS: f32 = 0.35;
pub const MANTLE_SCALE_MIN: f32 = 0.35;
pub const MANTLE_SCALE_MAX: f32 = 1.6;
pub const MANTLE_START_OFFSET_REF_HEIGHT: f32 = 0.5;
pub const MANTLE_START_OFFSET_MAX_SECS: f32 = 0.5;
pub const HIGH_CLIP_TRAVEL: Vec3 = Vec3::new(0.0, 2.0, -0.65);
pub const LOW_CLIP_TRAVEL: Vec3 = Vec3::new(0.0, 1.0, -0.65);

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

    pub fn clip_travel(self) -> Vec3 {
        match self {
            MantleKind::High => HIGH_CLIP_TRAVEL,
            MantleKind::LowLeft | MantleKind::LowRight => LOW_CLIP_TRAVEL,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LedgeProbe {
    pub face_distance: f32,
    pub top_height: f32,
    pub lateral_sign: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MantleIntent {
    pub kind: MantleKind,
    pub target: Vec3,
    pub top_height: f32,
    pub yaw_target: f32,
}

pub fn yaw_from_direction(dir: Vec3) -> f32 {
    f32::atan2(dir.x, -dir.z)
}

#[allow(clippy::too_many_arguments)]
pub fn detect_mantle(
    spatial: &SpatialQuery,
    excluded: Entity,
    origin: Vec3,
    foot_y: f32,
    fwd: Vec3,
    velocities: &Query<&LinearVelocity, Without<AlsCharacter>>,
) -> Option<MantleIntent> {
    let filter = SpatialQueryFilter::from_excluded_entities([excluded]);
    let Ok(dir) = Dir3::new(fwd) else {
        return None;
    };
    let mut face: Option<(f32, Vec3, f32, Entity)> = None;
    for h in [0.35f32, 0.65, 0.95] {
        let o = origin + fwd * MANTLE_PROBE_SHIFT + Vec3::Y * (h - (origin.y - foot_y));
        if let Some(hit) = spatial.cast_ray(o, dir, MANTLE_PROBE_REACH, true, &filter) {
            let best = face.map(|(d, _, _, _)| d).unwrap_or(f32::INFINITY);
            if hit.distance < best {
                let top_o = o + fwd * hit.distance + Vec3::Y * 1.2;
                let top_hit = spatial.cast_ray(top_o, Dir3::NEG_Y, 1.8, true, &filter);
                let (top_y, _top_normal_y) = top_hit
                    .map(|t| (top_o.y - t.distance, t.normal.y))
                    .unwrap_or((f32::NEG_INFINITY, 0.0));
                face = Some((
                    hit.distance,
                    hit.normal,
                    top_y,
                    top_hit.map(|t| t.entity).expect("top hit"),
                ));
            }
        }
    }
    let (face_distance, normal, top_y, ledge_entity) = face?;

    let top_normal = spatial
        .cast_ray(
            origin + fwd * (MANTLE_PROBE_SHIFT + face_distance) + Vec3::Y * 1.4,
            Dir3::NEG_Y,
            2.0,
            true,
            &filter,
        )
        .map(|t| t.normal.y)
        .unwrap_or(0.0);
    if top_normal < MANTLE_SLOPE_ANGLE_COS {
        return None;
    }

    let target_speed = velocities.get(ledge_entity).ok().map(|v| v.0.length());
    if target_speed.is_some_and(|s| s > MANTLE_TARGET_SPEED_THRESHOLD) {
        return None;
    }
    let right_of = Vec3::Y.cross(fwd).normalize_or_zero();
    let ledge = LedgeProbe {
        face_distance,
        top_height: top_y - foot_y,
        lateral_sign: normal.dot(right_of),
    };
    let kind = classify_mantle(&ledge)?;

    let face_plane_dist = MANTLE_PROBE_SHIFT + face_distance;
    let target = Vec3::new(
        origin.x + fwd.x * (face_plane_dist + MANTLE_TARGET_INSET),
        top_y + 0.92,
        origin.z + fwd.z * (face_plane_dist + MANTLE_TARGET_INSET),
    );
    let wall_dir = Vec3::new(-normal.x, 0.0, -normal.z).normalize_or_zero();
    let yaw_target = yaw_from_direction(if wall_dir.length_squared() > 1e-4 {
        wall_dir
    } else {
        fwd
    });

    let headroom_o = Vec3::new(target.x, top_y + 0.30, target.z);
    if spatial
        .cast_ray(
            headroom_o,
            Dir3::Y,
            MANTLE_HEADROOM_CLEARANCE,
            true,
            &filter,
        )
        .is_some()
    {
        return None;
    }
    let start_clearance = spatial.cast_ray(origin + Vec3::Y * 1.2, Dir3::Y, 0.9, true, &filter);
    if start_clearance.is_some() {
        return None;
    }

    Some(MantleIntent {
        kind,
        target,
        top_height: ledge.top_height,
        yaw_target,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MantleProbeTrace {
    pub forward_starts: [Vec3; 3],
    pub forward_ends: [Option<Vec3>; 3],
    pub downward_origin: Option<Vec3>,
    pub downward_hit: Option<Vec3>,
}

const PROBE_ROW_HEIGHTS: [f32; 3] = [0.35, 0.65, 0.95];

pub fn trace_mantle_probes(
    spatial: &SpatialQuery,
    excluded: Entity,
    origin: Vec3,
    foot_y: f32,
    fwd: Vec3,
) -> Option<MantleProbeTrace> {
    let filter = SpatialQueryFilter::from_excluded_entities([excluded]);
    let Ok(dir) = Dir3::new(fwd) else {
        return None;
    };
    let mut forward_starts = [Vec3::ZERO; 3];
    let mut forward_ends = [None; 3];
    let mut best: Option<(usize, f32)> = None;
    for (i, h) in PROBE_ROW_HEIGHTS.into_iter().enumerate() {
        let o = origin + fwd * MANTLE_PROBE_SHIFT + Vec3::Y * (h - (origin.y - foot_y));
        forward_starts[i] = o;
        let hit = spatial.cast_ray(o, dir, MANTLE_PROBE_REACH, true, &filter);
        forward_ends[i] = hit.map(|t| o + fwd * t.distance);
        if let Some(t) = hit {
            let better = best.is_none_or(|(_, d)| t.distance < d);
            if better {
                best = Some((i, t.distance));
            }
        }
    }
    let (downward_origin, downward_hit) = best
        .map(|(i, distance)| {
            let top_o = forward_starts[i] + fwd * distance + Vec3::Y * 1.2;
            let down = spatial
                .cast_ray(top_o, Dir3::NEG_Y, 1.8, true, &filter)
                .map(|t| top_o - Vec3::Y * t.distance);
            (Some(top_o), down)
        })
        .unwrap_or((None, None));
    Some(MantleProbeTrace {
        forward_starts,
        forward_ends,
        downward_origin,
        downward_hit,
    })
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

pub fn start_offset_for_height(top_height: f32) -> f32 {
    let norm = ((top_height - MANTLE_START_OFFSET_REF_HEIGHT)
        / (MANTLE_HIGH_MIN_HEIGHT - MANTLE_START_OFFSET_REF_HEIGHT))
        .clamp(0.0, 1.0);
    MANTLE_START_OFFSET_MAX_SECS * (1.0 - norm)
}

pub fn warp_envelope(clip_time: f32) -> f32 {
    smoothstep((clip_time / MANTLE_WARP_SECS).clamp(0.0, 1.0))
}

pub fn fit_translation_scale(required_length: f32, travel_length: f32) -> f32 {
    if travel_length < 1e-4 {
        return 1.0;
    }
    (required_length / travel_length).clamp(MANTLE_SCALE_MIN, MANTLE_SCALE_MAX)
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
    pub start_offset: f32,
    pub yaw_start: f32,
    pub yaw_target: f32,
    pub scale: f32,
    pub q_total: Vec3,
    pub root_travel: Vec3,
    pub prev_pos: Vec3,
    pub settle: f32,
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
            start_offset: 0.0,
            yaw_start: 0.0,
            yaw_target: 0.0,
            scale: 1.0,
            q_total: Vec3::ZERO,
            root_travel: Vec3::ZERO,
            prev_pos: Vec3::ZERO,
            settle: 0.0,
        }
    }
}

impl AlsMantleState {
    pub fn begin(
        &mut self,
        kind: MantleKind,
        start: Vec3,
        target: Vec3,
        top_height: f32,
        yaw_start: f32,
        yaw_target: f32,
    ) {
        let duration = kind.duration();
        let start_offset = start_offset_for_height(top_height);
        let remaining_frac = ((duration - start_offset) / duration).max(1e-3);
        let q_total = kind.clip_travel() * remaining_frac;
        let scale = fit_translation_scale((target - start).length(), q_total.length());
        self.active = true;
        self.kind = kind;
        self.duration = duration;
        self.time = 0.0;
        self.start = start;
        self.target = target;
        self.start_offset = start_offset;
        self.yaw_start = yaw_start;
        self.yaw_target = yaw_target;
        self.scale = scale;
        self.q_total = q_total;
        self.root_travel = Vec3::ZERO;
        self.prev_pos = start;
    }

    pub fn clip_time(&self) -> f32 {
        (self.time + self.start_offset).min(self.duration)
    }

    pub fn action_end(&self) -> f32 {
        self.duration - self.start_offset
    }

    fn reset_motion(&mut self) {
        self.root_travel = Vec3::ZERO;
        self.prev_pos = Vec3::ZERO;
    }

    pub fn tick(
        &mut self,
        cmd: &AlsInputCommand,
        loco: &LocomotionState,
        stance: &Stance,
        dt: f32,
    ) {
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
            self.reset_motion();
            return;
        }
        self.time += dt;
        if self.time >= self.action_end() {
            self.active = false;
            self.time = 0.0;
            self.cooldown = 0.3;
            self.settle = MANTLE_SETTLE_SECS;
            self.reset_motion();
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
        assert_eq!(
            classify_mantle(&probe(1.3, 0.5, 1.0)),
            Some(MantleKind::High)
        );
        assert_eq!(
            classify_mantle(&probe(1.25, 0.5, 1.0)),
            Some(MantleKind::High),
            "high threshold inclusive at 125cm"
        );
        assert_eq!(
            classify_mantle(&probe(1.24, 0.5, 0.4)),
            Some(MantleKind::LowRight),
            "below threshold falls to low band"
        );
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
    fn slope_cos_threshold_matches_35_deg() {
        let cos35 = (35.0f32.to_radians()).cos();
        assert!((MANTLE_SLOPE_ANGLE_COS - cos35).abs() < 1e-3);
        assert!(MANTLE_TARGET_SPEED_THRESHOLD > 0.0);
        assert!(MANTLE_HEADROOM_CLEARANCE > 1.7);
    }

    #[test]
    fn start_offset_skips_deeper_for_lower_ledges() {
        assert!((start_offset_for_height(0.45) - 0.5).abs() < 1e-5);
        assert!(start_offset_for_height(MANTLE_HIGH_MIN_HEIGHT).abs() < 1e-5);
        assert!(start_offset_for_height(MANTLE_MAX_LEDGE_HEIGHT).abs() < 1e-5);
        let mid = start_offset_for_height(0.85);
        assert!(mid > 0.2 && mid < 0.3, "mid ledge offset {mid}");
    }

    #[test]
    fn warp_envelope_saturates_within_window() {
        assert!(warp_envelope(0.0) <= 1e-5);
        assert!(warp_envelope(0.15) > 0.4 && warp_envelope(0.15) < 0.7);
        assert!((warp_envelope(0.3) - 1.0).abs() < 1e-5);
        assert!((warp_envelope(2.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn translation_scale_fits_and_clamps() {
        let s = fit_translation_scale(1.25, 2.2);
        assert!((s - 1.25 / 2.2).abs() < 1e-5);
        assert!((fit_translation_scale(9.0, 1.0) - MANTLE_SCALE_MAX).abs() < 1e-5);
        assert!((fit_translation_scale(0.05, 1.0) - MANTLE_SCALE_MIN).abs() < 1e-5);
        assert!((fit_translation_scale(1.0, 0.0) - 1.0).abs() < 1e-5);
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
        s.begin(MantleKind::LowLeft, Vec3::ZERO, Vec3::ONE, 0.8, 0.0, 0.0);
        assert!(s.active);
        let cmd = AlsInputCommand::default();
        let end = s.action_end();
        let iters = (end / (1.0 / 60.0)) as usize + 2;
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
        assert!(s.clip_time() <= MANTLE_LOW_DURATION);
    }
}
