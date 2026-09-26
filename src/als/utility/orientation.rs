use bevy::prelude::*;

pub fn yaw_to_world_forward(yaw: f32) -> Vec3 {
    (Quat::from_rotation_y(yaw) * Vec3::NEG_Z).normalize_or_zero()
}

pub fn world_forward_to_yaw(direction: Vec3) -> f32 {
    let horizontal = Vec2::new(direction.x, direction.z);
    if horizontal.length_squared() <= f32::EPSILON {
        0.0
    } else {
        f32::atan2(-direction.x, -direction.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap(yaw: f32) -> f32 {
        (yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
    }

    #[test]
    fn yaw_round_trips_the_bevy_negative_z_forward_axis() {
        for yaw in [
            -std::f32::consts::PI,
            -std::f32::consts::FRAC_PI_2,
            -std::f32::consts::FRAC_PI_4,
            0.0,
            std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
        ] {
            let forward = yaw_to_world_forward(yaw);
            let expected = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
            assert!(forward.distance(expected) < 1e-6, "yaw={yaw}");
            assert!(
                wrap(world_forward_to_yaw(forward) - yaw).abs() < 1e-6,
                "yaw={yaw}"
            );
        }
    }

    #[test]
    fn zero_direction_has_a_stable_yaw() {
        assert_eq!(world_forward_to_yaw(Vec3::ZERO), 0.0);
    }
}
