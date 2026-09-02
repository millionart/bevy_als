#[derive(Debug, Clone, PartialEq)]
pub struct TurnInPlaceConfig {
    pub view_yaw_angle_threshold_degrees: f32,
    pub view_yaw_speed_threshold_deg_per_s: f32,
    pub turn_180_angle_threshold_degrees: f32,
    pub play_rate: f32,
    pub max_activation_delay_seconds: f32,
}

impl Default for TurnInPlaceConfig {
    fn default() -> Self {
        Self {
            view_yaw_angle_threshold_degrees: 45.0,
            view_yaw_speed_threshold_deg_per_s: 50.0,
            turn_180_angle_threshold_degrees: 130.0,
            play_rate: 1.2,
            max_activation_delay_seconds: 0.75,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnVariant {
    Left90,
    Right90,
    Left180,
    Right180,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnInPlaceEvent {
    pub variant: TurnVariant,
    pub animated_turn_angle_degrees: f32,
    pub play_rate: f32,
}

#[derive(Debug, Default, Clone)]
pub struct TurnInPlaceDetector {
    activation_delay: f32,
}

pub fn wrap_angle_degrees(angle: f32) -> f32 {
    let mut a = angle % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a <= -180.0 {
        a += 360.0;
    }
    a
}

fn mapped_activation_delay(angle_abs: f32, threshold: f32, max_delay: f32) -> f32 {
    let span = 180.0 - threshold;
    let t = ((angle_abs - threshold) / span.max(f32::EPSILON)).clamp(0.0, 1.0);
    max_delay * t
}

impl TurnInPlaceDetector {
    pub fn update(
        &mut self,
        view_yaw_angle: f32,
        view_yaw_speed: f32,
        dt: f32,
        allowed: bool,
        stance_crouched: bool,
        config: &TurnInPlaceConfig,
    ) -> Option<TurnInPlaceEvent> {
        let _ = stance_crouched;
        if !allowed || dt <= 0.0 {
            self.activation_delay = 0.0;
            return None;
        }

        let angle_abs = wrap_angle_degrees(view_yaw_angle).abs();
        if view_yaw_speed >= config.view_yaw_speed_threshold_deg_per_s
            || angle_abs <= config.view_yaw_angle_threshold_degrees
        {
            self.activation_delay = 0.0;
            return None;
        }

        self.activation_delay += dt;
        let required = mapped_activation_delay(
            angle_abs,
            config.view_yaw_angle_threshold_degrees,
            config.max_activation_delay_seconds,
        );
        if self.activation_delay <= required {
            return None;
        }

        let left = view_yaw_angle <= 0.0;
        let variant = if angle_abs < config.turn_180_angle_threshold_degrees {
            if left {
                TurnVariant::Left90
            } else {
                TurnVariant::Right90
            }
        } else if left {
            TurnVariant::Left180
        } else {
            TurnVariant::Right180
        };
        Some(TurnInPlaceEvent {
            variant,
            animated_turn_angle_degrees: if matches!(
                variant,
                TurnVariant::Left180 | TurnVariant::Right180
            ) {
                180.0
            } else {
                90.0
            },
            play_rate: config.play_rate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> TurnInPlaceConfig {
        TurnInPlaceConfig::default()
    }

    #[test]
    fn triggers_after_sustained_slow_rotation() {
        let mut d = TurnInPlaceDetector::default();
        let c = cfg();
        let dt = 0.1;
        let mut fired = None;
        for _ in 0..30 {
            fired = d.update(70.0, 10.0, dt, true, false, &c);
            if fired.is_some() {
                break;
            }
        }
        assert!(fired.is_some(), "should fire within 30 ticks of 70deg hold");
        assert_eq!(fired.unwrap().variant, TurnVariant::Right90);
    }

    #[test]
    fn fast_view_rotation_resets_delay() {
        let mut d = TurnInPlaceDetector::default();
        let c = cfg();
        for _ in 0..40 {
            assert!(d.update(120.0, 200.0, 0.1, true, false, &c).is_none());
        }
    }

    #[test]
    fn small_angle_never_triggers() {
        let mut d = TurnInPlaceDetector::default();
        let c = cfg();
        for _ in 0..40 {
            assert!(d.update(20.0, 5.0, 0.1, true, false, &c).is_none());
        }
    }

    #[test]
    fn not_allowed_resets() {
        let mut d = TurnInPlaceDetector::default();
        let c = cfg();
        for _ in 0..15 {
            d.update(120.0, 5.0, 0.1, false, false, &c);
        }
        assert!(d.update(120.0, 5.0, 0.1, true, false, &c).is_none());
    }

    #[test]
    fn sign_selects_direction_and_180_threshold() {
        let mut dl = TurnInPlaceDetector::default();
        let mut dr = TurnInPlaceDetector::default();
        let c = cfg();
        let mut fl = None;
        let mut fr = None;
        for _ in 0..40 {
            fl = dl.update(-150.0, 10.0, 0.05, true, false, &c);
            fr = dr.update(150.0, 10.0, 0.05, true, false, &c);
            if fl.is_some() && fr.is_some() {
                break;
            }
        }
        assert_eq!(fl.unwrap().variant, TurnVariant::Left180);
        assert_eq!(fr.unwrap().variant, TurnVariant::Right180);
    }

    #[test]
    fn wrap_angle_normalizes() {
        assert!((wrap_angle_degrees(270.0) - (-90.0)).abs() < 1e-4);
        assert!((wrap_angle_degrees(-270.0) - 90.0).abs() < 1e-4);
        assert!((wrap_angle_degrees(360.0)).abs() < 1e-4);
    }

    #[test]
    fn source_activation_delay_upper_bound_is_preserved() {
        assert!((TurnInPlaceConfig::default().max_activation_delay_seconds - 0.75).abs() < 1e-6);
    }

    #[test]
    fn source_activation_delay_increases_from_yaw_threshold_to_one_eighty() {
        let config = TurnInPlaceConfig::default();
        assert!(
            mapped_activation_delay(
                config.view_yaw_angle_threshold_degrees,
                config.view_yaw_angle_threshold_degrees,
                config.max_activation_delay_seconds,
            )
            .abs()
                < 1e-6
        );
        assert!(
            (mapped_activation_delay(
                180.0,
                config.view_yaw_angle_threshold_degrees,
                config.max_activation_delay_seconds,
            ) - config.max_activation_delay_seconds)
                .abs()
                < 1e-6
        );
        assert!(
            (mapped_activation_delay(
                90.0,
                config.view_yaw_angle_threshold_degrees,
                config.max_activation_delay_seconds,
            ) - 0.25)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn source_turn_request_remains_active_while_the_yaw_condition_persists() {
        let mut detector = TurnInPlaceDetector::default();
        let config = TurnInPlaceConfig::default();
        let mut events = 0;
        for _ in 0..4 {
            if detector
                .update(90.0, 0.0, 0.3, true, false, &config)
                .is_some()
            {
                events += 1;
            }
        }
        assert_eq!(events, 4);
    }
}
