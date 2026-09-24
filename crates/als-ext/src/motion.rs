use bevy::prelude::{Quat, Vec2, Vec3};

pub fn falling_slide(delta: Vec3, normal: Vec3) -> Vec3 {
    let slide = delta - normal * delta.dot(normal);
    if slide.y <= 0.0 || slide.y - delta.y <= 1e-6 {
        return slide;
    }
    let limited = slide * (delta.y / slide.y).max(0.0);
    let remainder = Vec3::new(slide.x - limited.x, 0.0, slide.z - limited.z);
    let horizontal_normal = Vec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
    limited + remainder - horizontal_normal * remainder.dot(horizontal_normal)
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HorizontalVelocityBlend {
    pub forward: f32,
    pub backward: f32,
    pub left: f32,
    pub right: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HorizontalMotion {
    pub world_velocity: Vec3,
    pub local_velocity: Vec2,
    pub horizontal_speed: f32,
    pub signed_direction_radians: f32,
    pub velocity_blend: HorizontalVelocityBlend,
}

pub fn horizontal_motion_from_world_velocity(
    world_velocity: Vec3,
    character_rotation: Quat,
) -> HorizontalMotion {
    let world_velocity = Vec3::new(world_velocity.x, 0.0, world_velocity.z);
    let forward = character_rotation * Vec3::NEG_Z;
    let right = character_rotation * Vec3::X;
    let local_velocity = Vec2::new(world_velocity.dot(forward), world_velocity.dot(right));
    let horizontal_speed = local_velocity.length();
    if horizontal_speed <= f32::EPSILON {
        return HorizontalMotion {
            world_velocity,
            local_velocity,
            ..HorizontalMotion::default()
        };
    }
    let unit = local_velocity / horizontal_speed;
    let normalized = unit / (unit.x.abs() + unit.y.abs()).max(f32::EPSILON);
    HorizontalMotion {
        world_velocity,
        local_velocity,
        horizontal_speed,
        signed_direction_radians: local_velocity.y.atan2(local_velocity.x),
        velocity_blend: HorizontalVelocityBlend {
            forward: normalized.x.max(0.0),
            backward: (-normalized.x).max(0.0),
            left: (-normalized.y).max(0.0),
            right: normalized.y.max(0.0),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falling_slide_limits_boost_without_erasing_jump_or_parallel_motion() {
        let normal = Vec3::new(-3.0_f32.sqrt() * 0.5, 0.5, 0.0);
        for (delta, expected) in [
            (Vec3::new(0.1, -0.01, 0.04), Vec3::new(0.0, 0.0, 0.04)),
            (Vec3::new(0.1, 0.01, 0.04), Vec3::new(0.005773503, 0.01, 0.04)),
            (Vec3::new(0.01, -0.1, 0.04), Vec3::new(-0.04080127, -0.070669875, 0.04)),
        ] {
            let result = falling_slide(delta, normal);
            assert!(result.distance(expected) < 1e-6, "{delta:?}: {result:?} != {expected:?}");
        }
        assert!(falling_slide(Vec3::new(0.1, -0.01, 0.04), Vec3::NEG_X)
            .distance(Vec3::new(0.0, -0.01, 0.04)) < 1e-6);
    }

    #[test]
    fn horizontal_motion_preserves_local_forward_after_yaw_rotation() {
        let motion = horizontal_motion_from_world_velocity(
            Vec3::NEG_X,
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        );
        assert!(motion.local_velocity.distance(Vec2::X) < 1e-6);
        assert!(motion.signed_direction_radians.abs() < 1e-6);
        assert_eq!(
            motion.velocity_blend,
            HorizontalVelocityBlend {
                forward: 1.0,
                ..HorizontalVelocityBlend::default()
            }
        );
    }

    #[test]
    fn horizontal_motion_uses_manhattan_normalized_four_way_weights() {
        let motion =
            horizontal_motion_from_world_velocity(Vec3::new(1.0, 2.0, -1.0), Quat::IDENTITY);
        assert!((motion.horizontal_speed - 2.0_f32.sqrt()).abs() < 1e-6);
        assert!((motion.signed_direction_radians - std::f32::consts::FRAC_PI_4).abs() < 1e-6);
        assert!((motion.velocity_blend.forward - 0.5).abs() < 1e-6);
        assert!((motion.velocity_blend.right - 0.5).abs() < 1e-6);
        assert_eq!(motion.velocity_blend.backward, 0.0);
        assert_eq!(motion.velocity_blend.left, 0.0);
    }

    #[test]
    fn horizontal_motion_zeroes_vertical_and_stationary_velocity() {
        let motion = horizontal_motion_from_world_velocity(Vec3::Y * 3.0, Quat::IDENTITY);
        assert_eq!(motion.world_velocity, Vec3::ZERO);
        assert_eq!(motion.local_velocity, Vec2::ZERO);
        assert_eq!(motion.horizontal_speed, 0.0);
        assert_eq!(motion.velocity_blend, HorizontalVelocityBlend::default());
    }
}
