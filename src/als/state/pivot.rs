use bevy::prelude::*;

const DIRECTION_EPSILON: f32 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq, Reflect, serde::Serialize, serde::Deserialize)]
pub struct PivotConfig {
    pub reversal_angle_degrees: f32,
    pub min_speed: f32,
    pub cooldown_seconds: f32,
}

impl Default for PivotConfig {
    fn default() -> Self {
        Self {
            reversal_angle_degrees: 100.0,
            min_speed: 2.0,
            cooldown_seconds: 0.3,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, serde::Serialize, serde::Deserialize,
)]
pub enum PivotSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect, serde::Serialize, serde::Deserialize)]
pub struct PivotEvent {
    pub velocity_dir: Vec2,
    pub desired_dir: Vec2,
    pub angle_radians: f32,
    pub speed: f32,
    pub side: PivotSide,
}

#[derive(
    Component, Debug, Default, Clone, Copy, PartialEq, Reflect, serde::Serialize, serde::Deserialize,
)]
#[reflect(Default)]
pub struct PivotDetector {
    pub config: PivotConfig,
    pub cooldown_remaining: f32,
}

fn is_usable(vector: Vec2) -> bool {
    vector.is_finite()
}

impl PivotDetector {
    pub const fn new(config: PivotConfig) -> Self {
        Self {
            config,
            cooldown_remaining: 0.0,
        }
    }

    pub fn update(&mut self, velocity: Vec2, desired_dir: Vec2, dt: f32) -> Option<PivotEvent> {
        self.cooldown_remaining = (self.cooldown_remaining - dt.max(0.0)).max(0.0);

        if !is_usable(velocity) || !is_usable(desired_dir) {
            return None;
        }

        let speed = velocity.length();
        if speed < self.config.min_speed {
            return None;
        }

        let desired_length = desired_dir.length();
        if desired_length <= DIRECTION_EPSILON {
            return None;
        }

        let velocity_dir = velocity / speed;
        let desired_unit = desired_dir / desired_length;
        let dot = velocity_dir.dot(desired_unit);
        let cross = velocity_dir.x * desired_unit.y - velocity_dir.y * desired_unit.x;
        let angle = cross.atan2(dot);

        if angle.abs() <= self.config.reversal_angle_degrees.to_radians() {
            return None;
        }

        if self.cooldown_remaining > 0.0 {
            return None;
        }

        self.cooldown_remaining = self.config.cooldown_seconds;

        Some(PivotEvent {
            velocity_dir,
            desired_dir: desired_unit,
            angle_radians: angle,
            speed,
            side: if cross >= 0.0 {
                PivotSide::Left
            } else {
                PivotSide::Right
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn detector() -> PivotDetector {
        PivotDetector::new(PivotConfig::default())
    }

    #[test]
    fn default_config_matches_als_reference_values() {
        let config = PivotConfig::default();
        assert_eq!(config.reversal_angle_degrees, 100.0);
        assert_eq!(
            config.min_speed, 2.0,
            "ALS PivotActivationSpeedThreshold is 200 cm/s"
        );
        assert!(config.cooldown_seconds > 0.0);
    }

    #[test]
    fn full_reversal_above_min_speed_triggers() {
        let mut detector = detector();
        let event = detector
            .update(Vec2::new(4.0, 0.0), Vec2::new(-2.0, 0.0), DT)
            .expect("a hard reversal must emit a pivot event");
        assert!((event.angle_radians - std::f32::consts::PI).abs() < 1e-4);
        assert!((event.speed - 4.0).abs() < 1e-6);
        assert!((event.velocity_dir - Vec2::X).length() < 1e-6);
        assert!((event.desired_dir + Vec2::X).length() < 1e-6);
    }

    #[test]
    fn reversal_side_follows_cross_product_sign() {
        let mut left_detector = detector();
        let left_event = left_detector
            .update(Vec2::new(4.0, 0.0), Vec2::new(-1.0, 1.0), DT)
            .expect("back-left reversal must emit");
        assert_eq!(left_event.side, PivotSide::Left);

        let mut right_detector = detector();
        let right_event = right_detector
            .update(Vec2::new(4.0, 0.0), Vec2::new(-1.0, -1.0), DT)
            .expect("back-right reversal must emit");
        assert_eq!(right_event.side, PivotSide::Right);
    }

    #[test]
    fn sustained_moderate_angle_never_triggers() {
        let mut detector = detector();
        for _ in 0..120 {
            let event = detector.update(Vec2::new(4.0, 0.0), Vec2::new(0.0, 4.0), DT);
            assert!(
                event.is_none(),
                "a steady 90 degree drift must not count as a pivot"
            );
        }
    }

    #[test]
    fn gradual_turn_with_small_lead_angle_never_triggers() {
        let mut detector = detector();
        let step = 15.0f32.to_radians();
        let mut heading = 0.0f32;
        for _ in 0..24 {
            let desired_angle = heading + step;
            let velocity = Vec2::new(heading.cos(), heading.sin()) * 4.0;
            let desired = Vec2::new(desired_angle.cos(), desired_angle.sin());
            let event = detector.update(velocity, desired, DT);
            assert!(
                event.is_none(),
                "per-frame lead angle {} rad stays far below the threshold",
                step
            );
            heading = desired_angle;
        }
    }

    #[test]
    fn cooldown_suppresses_immediate_repeat_and_recovers_after_expiry() {
        let mut detector = detector();
        assert!(
            detector
                .update(Vec2::new(4.0, 0.0), Vec2::new(-1.0, 0.0), DT)
                .is_some()
        );

        let config = detector.config;
        let mut elapsed = 0.0f32;
        while elapsed + DT < config.cooldown_seconds {
            elapsed += DT;
            assert!(
                detector
                    .update(Vec2::new(4.0, 0.0), Vec2::new(-1.0, 0.0), DT)
                    .is_none(),
                "repeats inside the {} s cooldown window are suppressed",
                config.cooldown_seconds
            );
        }

        assert!(
            detector
                .update(
                    Vec2::new(4.0, 0.0),
                    Vec2::new(-1.0, 0.0),
                    config.cooldown_seconds
                )
                .is_some(),
            "a new reversal must fire once the cooldown fully elapses"
        );
    }

    #[test]
    fn low_speed_below_threshold_never_triggers() {
        let mut detector = detector();
        assert!(
            detector
                .update(Vec2::new(1.5, 0.0), Vec2::new(-1.0, 0.0), DT)
                .is_none(),
            "speed 1.5 m/s sits below the 2.0 m/s activation floor"
        );
        assert!(detector.cooldown_remaining == 0.0);
    }

    #[test]
    fn speed_equal_to_threshold_triggers() {
        let mut detector = detector();
        assert!(
            detector
                .update(Vec2::new(2.0, 0.0), Vec2::new(-1.0, 0.0), DT)
                .is_some(),
            "the activation floor itself is inclusive"
        );
    }

    #[test]
    fn zero_or_non_finite_inputs_are_ignored() {
        let mut detector = detector();

        assert!(
            detector
                .update(Vec2::new(4.0, 0.0), Vec2::ZERO, DT)
                .is_none()
        );
        assert!(
            detector
                .update(Vec2::ZERO, Vec2::new(-1.0, 0.0), DT)
                .is_none()
        );
        assert!(
            detector
                .update(Vec2::new(f32::NAN, 0.0), Vec2::new(-1.0, 0.0), DT)
                .is_none()
        );
        assert!(
            detector
                .update(Vec2::new(f32::INFINITY, 0.0), Vec2::new(-1.0, 0.0), DT)
                .is_none()
        );
        assert_eq!(detector.cooldown_remaining, 0.0);
    }

    #[test]
    fn negative_dt_does_not_advance_cooldown_below_zero() {
        let mut detector = detector();
        assert!(
            detector
                .update(Vec2::new(4.0, 0.0), Vec2::new(-1.0, 0.0), DT)
                .is_some()
        );
        assert!(
            detector
                .update(Vec2::new(4.0, 0.0), Vec2::new(-1.0, 0.0), -1.0)
                .is_none()
        );
        assert!((detector.cooldown_remaining - detector.config.cooldown_seconds).abs() < 1e-6);
    }
}
