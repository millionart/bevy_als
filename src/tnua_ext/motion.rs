use bevy::prelude::Vec3;

pub fn falling_slide(delta: Vec3, normal: Vec3, previous_normal: Option<Vec3>) -> Vec3 {
    let slide = match previous_normal {
        Some(previous) if previous.dot(normal) <= 0.0 => {
            let crease = normal.cross(previous).normalize_or_zero();
            crease * delta.dot(crease)
        }
        _ => delta - normal * delta.dot(normal),
    };
    let slide = limit_slope_boost(slide, delta, normal);
    if let Some(previous) = previous_normal
        && (previous.dot(normal) - 1.0).abs() < 1e-4
        && slide.dot(delta) > 0.0
    {
        return limit_slope_boost(slide + normal * 0.0001, delta, normal);
    }
    slide
}

fn limit_slope_boost(slide: Vec3, delta: Vec3, normal: Vec3) -> Vec3 {
    if slide.y <= 0.0 || slide.y - delta.y <= 1e-6 {
        return slide;
    }
    let limited = slide * (delta.y / slide.y).max(0.0);
    let remainder = Vec3::new(slide.x - limited.x, 0.0, slide.z - limited.z);
    let horizontal_normal = Vec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
    limited + remainder - horizontal_normal * remainder.dot(horizontal_normal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Quat;

    #[test]
    fn falling_slide_limits_boost_without_erasing_jump_or_parallel_motion() {
        let normal = Vec3::new(-3.0_f32.sqrt() * 0.5, 0.5, 0.0);
        for (delta, expected) in [
            (Vec3::new(0.1, -0.01, 0.04), Vec3::new(0.0, 0.0, 0.04)),
            (
                Vec3::new(0.1, 0.01, 0.04),
                Vec3::new(0.005773503, 0.01, 0.04),
            ),
            (
                Vec3::new(0.01, -0.1, 0.04),
                Vec3::new(-0.04080127, -0.070669875, 0.04),
            ),
        ] {
            let result = falling_slide(delta, normal, None);
            assert!(
                result.distance(expected) < 1e-6,
                "{delta:?}: {result:?} != {expected:?}"
            );
        }
        assert!(
            falling_slide(Vec3::new(0.1, -0.01, 0.04), Vec3::NEG_X, None)
                .distance(Vec3::new(0.0, -0.01, 0.04))
                < 1e-6
        );
    }

    #[test]
    fn falling_two_wall_slide_uses_the_crease_and_limits_upward_deflection() {
        let delta = Vec3::new(0.0, -0.03, 0.065);
        let normal = Vec3::new(0.5, 0.0, -3.0_f32.sqrt() * 0.5);
        assert!(falling_slide(delta, normal, None).dot(Vec3::NEG_X) < -0.02);
        assert!(
            falling_slide(delta, normal, Some(Vec3::NEG_X)).distance(Vec3::new(0.0, -0.03, 0.0))
                < 1e-6
        );
        let obtuse_normal = Vec3::new(-normal.x, 0.0, normal.z);
        assert!(
            falling_slide(delta, obtuse_normal, Some(Vec3::NEG_X)).distance(Vec3::new(
                -0.028145825,
                -0.03,
                0.01625
            )) < 1e-6
        );
        let slope = Vec3::new(-3.0_f32.sqrt() * 0.5, 0.5, 0.0);
        let other_slope = Vec3::new(0.5, 0.5, -0.5_f32.sqrt());
        let result = falling_slide(Vec3::Z * 0.04, other_slope, Some(slope));
        assert!(
            result.is_finite() && result.y.abs() < 1e-6,
            "slope crease created upward deflection: {result:?}"
        );
    }

    #[test]
    fn falling_two_wall_slide_nudges_only_near_parallel_forward_slides() {
        let delta = Vec3::new(0.0, -0.01, 0.1);
        for (degrees, expected_nudge) in [(0.5_f32, 0.0001), (1.25, 0.0)] {
            let normal = Quat::from_rotation_y(degrees.to_radians()) * Vec3::NEG_X;
            let slide = falling_slide(delta, normal, Some(Vec3::NEG_X));
            assert!(
                (slide.dot(normal) - expected_nudge).abs() < 1e-7,
                "{degrees}: {slide:?}"
            );
            assert!(falling_slide(delta, normal, None).dot(normal).abs() < 1e-7);
        }
        assert_eq!(
            falling_slide(Vec3::X, Vec3::NEG_X, Some(Vec3::NEG_X)),
            Vec3::ZERO
        );
        let slope = Vec3::new(-3.0_f32.sqrt() * 0.5, 0.5, 0.0);
        let slide = falling_slide(Vec3::Z * 0.1, slope, Some(slope));
        assert!(slide.y.abs() < 1e-6);
    }
}
