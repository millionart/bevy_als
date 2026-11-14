use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::systems::RootMotionOutput;

use crate::als::orientation::world_forward_to_yaw;

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

pub fn root_motion_world_transform(
    actor_world: GlobalTransform,
    mesh_world: GlobalTransform,
    local_translation: Vec3,
    local_rotation: Quat,
) -> (Vec3, Quat) {
    let actor_in_mesh = mesh_world
        .affine()
        .inverse()
        .transform_point3(actor_world.translation());
    let moved_actor = mesh_world
        .affine()
        .transform_point3(local_rotation * actor_in_mesh + local_translation);
    let mesh_rotation = mesh_world.rotation();
    let world_delta_rotation = mesh_rotation * local_rotation * mesh_rotation.conjugate();
    (
        moved_actor - actor_world.translation(),
        (world_delta_rotation * actor_world.rotation()).normalize(),
    )
}

pub fn yaw_from_direction(dir: Vec3) -> f32 {
    world_forward_to_yaw(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_world_root_motion_preserves_scale_pivot_and_rotation_basis() {
        for scale in [0.5, 1.0, 2.0] {
            for mesh_yaw in [std::f32::consts::PI, 0.4, -0.7] {
                let actor = GlobalTransform::from(
                    Transform::from_xyz(3.0, 2.0, -1.0)
                        .with_rotation(Quat::from_rotation_y(0.8))
                        .with_scale(Vec3::splat(scale)),
                );
                let relative = Transform::from_xyz(0.1, -0.92, 0.2)
                    .with_rotation(Quat::from_rotation_y(mesh_yaw));
                let mesh = actor.mul_transform(relative);
                let delta = Transform::from_translation(Vec3::new(0.1, 0.0, -0.2))
                    .with_rotation(Quat::from_rotation_y(0.2));
                let expected = mesh.affine()
                    * delta.compute_affine()
                    * mesh.affine().inverse()
                    * actor.affine();
                let (translation, rotation) =
                    root_motion_world_transform(actor, mesh, delta.translation, delta.rotation);
                assert!(
                    translation.distance(Vec3::from(expected.translation) - actor.translation())
                        < 0.00001
                );
                let expected_rotation = mesh.rotation()
                    * delta.rotation
                    * mesh.rotation().conjugate()
                    * actor.rotation();
                assert!(rotation.dot(expected_rotation).abs() > 0.99999);
            }
        }
    }

    #[test]
    fn root_motion_transform_keeps_lateral_translation_and_rotation() {
        let character_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let local_translation = Vec3::new(0.10, 0.0, -0.20);
        let local_rotation = Quat::from_rotation_y(10.0_f32.to_radians());
        let (world_translation, world_rotation) = root_motion_world_transform(
            GlobalTransform::from(Transform::from_rotation(character_rotation)),
            GlobalTransform::from(Transform::from_rotation(character_rotation)),
            local_translation,
            local_rotation,
        );
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
