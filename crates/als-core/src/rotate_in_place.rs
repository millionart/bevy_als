#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotateInPlaceConfig {
    pub view_yaw_angle_threshold_degrees: f32,
    pub first_person_view_yaw_angle_threshold_degrees: f32,
    pub play_rate_range: (f32, f32),
    pub reference_yaw_speed_range: (f32, f32),
}

impl Default for RotateInPlaceConfig {
    fn default() -> Self {
        Self {
            view_yaw_angle_threshold_degrees: 50.0,
            first_person_view_yaw_angle_threshold_degrees: 65.0,
            play_rate_range: (1.15, 3.0),
            reference_yaw_speed_range: (180.0, 460.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotateSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotateInPlaceState {
    pub side: Option<RotateSide>,
    pub play_rate: f32,
}

impl Default for RotateInPlaceState {
    fn default() -> Self {
        Self {
            side: None,
            play_rate: 1.0,
        }
    }
}

fn map_range(value: f32, in_range: (f32, f32), out_range: (f32, f32)) -> f32 {
    let span = (in_range.1 - in_range.0).max(f32::EPSILON);
    let t = ((value - in_range.0) / span).clamp(0.0, 1.0);
    out_range.0 + (out_range.1 - out_range.0) * t
}

pub fn update_rotate_in_place(
    state: &mut RotateInPlaceState,
    moving: bool,
    allowed: bool,
    view_yaw_angle_degrees: f32,
    view_yaw_speed_deg_per_s: f32,
    config: &RotateInPlaceConfig,
    pending_update: bool,
    dt: f32,
) {
    let side = if moving || !allowed {
        None
    } else if view_yaw_angle_degrees < -config.view_yaw_angle_threshold_degrees {
        Some(RotateSide::Left)
    } else if view_yaw_angle_degrees > config.view_yaw_angle_threshold_degrees {
        Some(RotateSide::Right)
    } else {
        None
    };

    let target = if side.is_some() {
        map_range(
            view_yaw_speed_deg_per_s,
            config.reference_yaw_speed_range,
            config.play_rate_range,
        )
    } else {
        config.play_rate_range.0
    };
    state.side = side;
    state.play_rate = if pending_update {
        target
    } else {
        damp_exact(state.play_rate, target, dt)
    };
}

fn damp_exact(current: f32, target: f32, dt: f32) -> f32 {
    let alpha = 1.0 - 2.0_f32.powf(-dt.max(0.0) / 0.15);
    current + (target - current) * alpha
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_clears_state() {
        let mut s = RotateInPlaceState {
            side: Some(RotateSide::Left),
            play_rate: 2.0,
        };
        update_rotate_in_place(
            &mut s,
            true,
            true,
            -80.0,
            300.0,
            &Default::default(),
            false,
            0.15,
        );
        assert_eq!(s.side, None);
        assert!((s.play_rate - 1.575).abs() < 1e-6);
        assert_eq!(RotateInPlaceState::default().play_rate, 1.0);
    }

    #[test]
    fn not_allowed_clears_state() {
        let mut s = RotateInPlaceState {
            side: Some(RotateSide::Right),
            play_rate: 2.0,
        };
        update_rotate_in_place(
            &mut s,
            false,
            false,
            80.0,
            300.0,
            &Default::default(),
            false,
            0.05,
        );
        assert_eq!(s.side, None);
    }

    #[test]
    fn left_threshold_selects_left() {
        let mut s = RotateInPlaceState::default();
        update_rotate_in_place(
            &mut s,
            false,
            true,
            -80.0,
            300.0,
            &Default::default(),
            false,
            0.05,
        );
        assert_eq!(s.side, Some(RotateSide::Left));
    }

    #[test]
    fn right_threshold_selects_right() {
        let mut s = RotateInPlaceState::default();
        update_rotate_in_place(
            &mut s,
            false,
            true,
            80.0,
            300.0,
            &Default::default(),
            false,
            0.05,
        );
        assert_eq!(s.side, Some(RotateSide::Right));
    }

    #[test]
    fn within_threshold_is_idle() {
        let mut s = RotateInPlaceState::default();
        update_rotate_in_place(
            &mut s,
            false,
            true,
            30.0,
            300.0,
            &Default::default(),
            false,
            0.05,
        );
        assert_eq!(s.side, None);
    }

    #[test]
    fn play_rate_scales_with_view_speed() {
        let cfg = RotateInPlaceConfig::default();
        let mut slow = RotateInPlaceState::default();
        let mut fast = RotateInPlaceState::default();
        for _ in 0..40 {
            update_rotate_in_place(&mut slow, false, true, -80.0, 190.0, &cfg, false, 0.1);
            update_rotate_in_place(&mut fast, false, true, -80.0, 450.0, &cfg, false, 0.1);
        }
        assert!(slow.play_rate < fast.play_rate);
        assert!((fast.play_rate - 2.93).abs() < 0.05);
        assert!(fast.play_rate < cfg.play_rate_range.1);
    }

    #[test]
    fn idle_damps_to_min_play_rate() {
        let cfg = RotateInPlaceConfig::default();
        let mut s = RotateInPlaceState {
            side: None,
            play_rate: 3.0,
        };
        for _ in 0..120 {
            update_rotate_in_place(&mut s, false, true, 10.0, 0.0, &cfg, false, 0.1);
        }
        assert!((s.play_rate - 1.15).abs() < 1e-3);
    }

    #[test]
    fn source_damping_uses_fifteen_hundredths_half_life() {
        assert!((damp_exact(0.0, 1.0, 0.15) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn source_first_person_threshold_is_sixty_five_degrees() {
        assert_eq!(
            RotateInPlaceConfig::default().first_person_view_yaw_angle_threshold_degrees,
            65.0
        );
    }
}
