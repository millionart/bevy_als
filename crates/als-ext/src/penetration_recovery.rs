use bevy::prelude::*;

pub fn sum_penetration_adjustments(adjustments: impl IntoIterator<Item = Vec3>) -> Vec3 {
    adjustments.into_iter().fold(Vec3::ZERO, |sum, adjustment| {
        if adjustment.is_finite() {
            sum + adjustment
        } else {
            sum
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_all_finite_penetration_adjustments() {
        let sum = sum_penetration_adjustments([
            Vec3::new(0.2, 0.0, 0.0),
            Vec3::new(0.0, 0.3, 0.0),
            Vec3::new(f32::NAN, 0.0, 0.0),
        ]);
        let passed = sum.distance(Vec3::new(0.2, 0.3, 0.0)) <= 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.56][VERDICT] adjustment=({:.3},{:.3},{:.3}) -> {verdict}",
            sum.x, sum.y, sum.z,
        );
        assert!(passed);
    }
}
