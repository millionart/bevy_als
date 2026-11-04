use bevy::prelude::*;
use bevy::math::{DQuat, DVec3};

fn velocity_local_f64(world_velocity: Vec3, character_rotation: Quat) -> Vec3 {
    (character_rotation.as_dquat().inverse() * world_velocity.as_dvec3()).as_vec3()
}

pub fn velocity_blend_direction(world_velocity: Vec3, character_rotation: Quat) -> Vec3 {
    let local = velocity_local_f64(world_velocity, character_rotation);
    if local.length_squared() > 1e-12 {
        local / local.abs().element_sum()
    } else {
        Vec3::ZERO
    }
}

pub fn acceleration_amount(
    acceleration: Vec3,
    velocity: Vec3,
    rotation: Quat,
    maximum_acceleration: f32,
    maximum_braking_deceleration: f32,
) -> Vec2 {
    let world = DVec3::new(acceleration.x as f64, acceleration.z as f64, acceleration.y as f64) * 100.0;
    let world_velocity = DVec3::new(velocity.x as f64, velocity.z as f64, velocity.y as f64) * 100.0;
    let maximum = if world.dot(world_velocity) >= 0.0 {
        maximum_acceleration
    } else {
        maximum_braking_deceleration
    };
    if maximum <= 1e-6 {
        return Vec2::ZERO;
    }
    let half = std::f64::consts::FRAC_1_SQRT_2;
    let source_rotation = rotation.as_dquat() * DQuat::from_xyzw(0.0, half, 0.0, half);
    let inverse = DVec3::new(source_rotation.x, source_rotation.z, source_rotation.y);
    let twice_cross = 2.0 * inverse.cross(world);
    let local = (world + source_rotation.w * twice_cross + inverse.cross(twice_cross)).as_vec3();
    let amount = local / (maximum * 100.0);
    let length_squared = amount.length_squared();
    let amount = if length_squared > 1.0 { amount * length_squared.sqrt().recip() } else { amount };
    Vec2::new(amount.x, amount.y)
}

pub fn inv_exp_approx(value: f32) -> f32 {
    1.0 / (1.0
        + 1.00746054 * value
        + 0.45053901 * value * value
        + 0.25724632 * value * value * value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_local_rotation_preserves_double_precision() {
        let velocity = Vec3::new(2.0000267, 0.0, -0.00008583068);
        let rotation = Quat::from_xyzw(0.0, -0.4792092, 0.0, 0.87770075);
        let local = velocity_local_f64(velocity, rotation);
        assert_eq!(local, Vec3::new(1.08137655, 0.0, -1.68247807));
    }

    #[test]
    fn source_local_acceleration_amount_matches_native_lean_controls() {
        let acceleration = Vec3::new(20.000265, 0.0, -0.0008583067);
        let velocity = Vec3::new(1.333351, 0.0, -0.00008583068);
        let rotation = Quat::from_xyzw(0.0, -0.43686104, 0.0, 0.8995291);
        let amount = acceleration_amount(acceleration, velocity, rotation, 20.0, 15.0);
        let source = Vec2::new(0.785964787, 0.618271291);
        assert!(amount.abs_diff_eq(source, 1e-7), "Native local lean differs: {amount:?} != {source:?}");
        let vertical = Vec3::new(0.0, 3.0, -4.0);
        assert!(acceleration_amount(vertical, vertical, Quat::IDENTITY, 1.0, 1.0)
            .abs_diff_eq(Vec2::new(0.8, 0.0), 1e-6));
        assert_eq!(acceleration_amount(Vec3::X, Vec3::X, Quat::IDENTITY, 0.0, 0.0), Vec2::ZERO);
    }

    #[test]
    fn source_lean_uses_double_precision_acceleration_velocity_dot() {
        let acceleration = Vec3::new(0.00051497726, 0.0, 6.0);
        let velocity = Vec3::new(0.6666755, 0.0, -0.000057220455);
        let rotation = Quat::from_xyzw(0.0, -0.40180978, 0.0, 0.9157232);
        assert_eq!(acceleration.dot(velocity), 0.0);
        let amount = acceleration_amount(acceleration, velocity, rotation, 20.0, 15.0);
        let native = Vec2::new(-0.27081388, 0.29438046);
        assert!(amount.abs_diff_eq(native, 1e-7), "Native braking lean differs: {amount:?} != {native:?}");
        let positive = acceleration_amount(Vec3::X, Vec3::X, Quat::IDENTITY, 20.0, 15.0);
        let braking = acceleration_amount(-Vec3::X, Vec3::X, Quat::IDENTITY, 20.0, 15.0);
        assert!((positive.y - 0.05).abs() < 1e-6);
        assert!((braking.y + 1.0 / 15.0).abs() < 1e-6);
    }
}
