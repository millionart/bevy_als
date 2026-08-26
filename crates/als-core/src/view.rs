use bevy::prelude::*;

pub const PITCH_LIMIT_DEGREES: f32 = 85.0;
pub const LOOK_HALF_LIFE_SECONDS: f32 = 0.08;

#[derive(Component, Debug, Default, Clone, Copy, Reflect)]
#[reflect(Default)]
pub struct AlsViewPitch(pub f32);

#[derive(Component, Debug, Default, Clone, Copy, Reflect)]
#[reflect(Default)]
pub struct AlsViewSmoothed(pub f32);

pub fn clamp_pitch(pitch_radians: f32) -> f32 {
    let limit = PITCH_LIMIT_DEGREES.to_radians();
    pitch_radians.clamp(-limit, limit)
}

pub fn damp_view(current: f32, target: f32, half_life_seconds: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return current;
    }
    let lambda = std::f32::consts::LN_2.max(f32::EPSILON) / half_life_seconds.max(1e-4);
    current + (target - current) * (1.0 - (-lambda * dt).exp())
}

pub fn view_pitch_norm(pitch_radians: f32) -> f32 {
    clamp_pitch(pitch_radians) / PITCH_LIMIT_DEGREES.to_radians()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_clamps_to_limit() {
        assert_eq!(
            clamp_pitch(200.0_f32.to_radians()),
            PITCH_LIMIT_DEGREES.to_radians()
        );
        assert_eq!(
            clamp_pitch(-200.0_f32.to_radians()),
            -PITCH_LIMIT_DEGREES.to_radians()
        );
        assert!((clamp_pitch(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn norm_maps_limits_to_unit_range() {
        assert!((view_pitch_norm(PITCH_LIMIT_DEGREES.to_radians()) - 1.0).abs() < 1e-6);
        assert!((view_pitch_norm(-PITCH_LIMIT_DEGREES.to_radians()) + 1.0).abs() < 1e-6);
        assert!(view_pitch_norm(0.0).abs() < 1e-6);
        let over = view_pitch_norm(400.0_f32.to_radians());
        assert!((over - 1.0).abs() < 1e-6);
    }

    #[test]
    fn damping_converges_toward_target_and_never_overshoots() {
        let mut cur = 0.0_f32;
        for _ in 0..600 {
            cur = damp_view(cur, 1.0, LOOK_HALF_LIFE_SECONDS, 1.0 / 60.0);
            assert!(cur <= 1.0 + 1e-6 && cur >= 0.0 - 1e-6);
        }
        assert!((cur - 1.0).abs() < 1e-3);
    }

    #[test]
    fn zero_dt_keeps_current() {
        assert_eq!(damp_view(0.25, 1.0, LOOK_HALF_LIFE_SECONDS, 0.0), 0.25);
    }

    #[test]
    fn half_life_halves_distance() {
        let after = damp_view(1.0, 0.0, 0.5, 0.5);
        assert!((after - 0.5).abs() < 1e-3);
    }
}
