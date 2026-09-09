use bevy::prelude::*;

pub fn velocity_blend_direction(world_velocity: Vec3, character_rotation: Quat) -> Vec3 {
    let local = character_rotation.inverse() * world_velocity;
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
    let maximum = if acceleration.dot(velocity) >= 0.0 {
        maximum_acceleration
    } else {
        maximum_braking_deceleration
    };
    if maximum <= 1e-6 {
        return Vec2::ZERO;
    }
    let amount = (acceleration / maximum).clamp_length_max(1.0);
    Vec2::new(
        amount.dot(rotation * Vec3::NEG_Z),
        amount.dot(rotation * Vec3::X),
    )
}

pub fn inv_exp_approx(value: f32) -> f32 {
    1.0 / (1.0
        + 1.00746054 * value
        + 0.45053901 * value * value
        + 0.25724632 * value * value * value)
}
