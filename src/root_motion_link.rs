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

pub fn read_local_rotation_delta(rm: Option<&RootMotionOutput>) -> Quat {
    let basis = mesh_to_entity_yaw();
    rm.map(|root_motion| (basis * root_motion.rotation_delta * basis.conjugate()).normalize())
        .unwrap_or(Quat::IDENTITY)
}

pub fn read_local_root_motion_delta(rm: Option<&RootMotionOutput>) -> (Vec3, Quat) {
    (read_local_delta(rm), read_local_rotation_delta(rm))
}

pub fn root_motion_world_transform(
    character_rotation: Quat,
    local_translation: Vec3,
    local_rotation: Quat,
) -> (Vec3, Quat) {
    (
        character_rotation * local_translation,
        (character_rotation * local_rotation).normalize(),
    )
}

pub fn yaw_from_direction(dir: Vec3) -> f32 {
    world_forward_to_yaw(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_motion_transform_keeps_lateral_translation_and_rotation() {
        let character_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let local_translation = Vec3::new(0.10, 0.0, -0.20);
        let local_rotation = Quat::from_rotation_y(10.0_f32.to_radians());
        let (world_translation, world_rotation) =
            root_motion_world_transform(character_rotation, local_translation, local_rotation);
        let expected_translation = character_rotation * local_translation;
        let expected_rotation = character_rotation * local_rotation;
        let translation_error = world_translation.distance(expected_translation);
        let rotation_alignment = world_rotation.dot(expected_rotation).abs();
        let legacy_translation = character_rotation * Vec3::new(0.0, 0.0, local_translation.z);
        let legacy_error = world_translation.distance(legacy_translation);
        let passed =
            translation_error < 1e-6 && rotation_alignment > 0.999_99 && legacy_error > 0.09;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.48][VERDICT] translation_error={translation_error:.6} rotation_alignment={rotation_alignment:.6} legacy_error={legacy_error:.6} -> {verdict}"
        );
        assert!(passed);
    }
}
