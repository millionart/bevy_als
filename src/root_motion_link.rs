use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::systems::RootMotionOutput;

use als_core::orientation::world_forward_to_yaw;

use crate::character_assembly::AlsCharacterAvatar;

pub fn mesh_to_entity_yaw() -> Quat {
    Quat::from_rotation_y(std::f32::consts::PI)
}

pub fn ensure_root_motion_output(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<AlsCharacterAvatar>,
            With<AnimatedSceneInstance>,
            Without<RootMotionOutput>,
        ),
    >,
) {
    for entity in &q {
        commands.entity(entity).insert(RootMotionOutput::default());
    }
}

pub fn read_local_delta(rm: Option<&RootMotionOutput>) -> Vec3 {
    rm.map(|r| mesh_to_entity_yaw() * r.translation_delta)
        .unwrap_or(Vec3::ZERO)
}

pub fn clamp_drive_speed(velocity: Vec3, max_speed: f32) -> Vec3 {
    if velocity.length() > max_speed {
        velocity.normalize_or_zero() * max_speed
    } else {
        velocity
    }
}

pub fn yaw_from_direction(dir: Vec3) -> f32 {
    world_forward_to_yaw(dir)
}
