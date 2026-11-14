use bevy::prelude::*;
use bevy_als::extensions::{
    ArmatureDirection, ArmaturePoint, armature_direction_from_world, armature_point_from_world,
    bone_local_point_from_world, parent_point_from_world, world_direction_from_armature,
    world_point_from_armature,
};

fn frame(translation: Vec3, rotation: Quat) -> GlobalTransform {
    GlobalTransform::from(Transform::from_translation(translation).with_rotation(rotation))
}

fn main() {
    let armature_a = frame(
        Vec3::new(4.0, 1.5, -3.0),
        Quat::from_euler(EulerRot::YXZ, 0.7, -0.2, 0.15),
    );
    let armature_b = frame(
        Vec3::new(-2.0, 0.3, 5.0),
        Quat::from_euler(EulerRot::YXZ, -0.9, 0.35, -0.1),
    );
    let local_target = ArmaturePoint(Vec3::new(0.16, -0.87, 0.31));
    let local_normal = ArmatureDirection(Vec3::new(0.0, 0.93, 0.37).normalize());
    let world_a = world_point_from_armature(&armature_a, local_target);
    let world_b = world_point_from_armature(&armature_b, local_target);
    let rebuilt_a = armature_point_from_world(&armature_a, world_a);
    let rebuilt_b = armature_point_from_world(&armature_b, world_b);
    let normal_a = armature_direction_from_world(
        &armature_a,
        world_direction_from_armature(&armature_a, local_normal),
    );
    let normal_b = armature_direction_from_world(
        &armature_b,
        world_direction_from_armature(&armature_b, local_normal),
    );
    let parent_a = frame(Vec3::new(1.0, -0.5, 2.0), Quat::from_rotation_y(0.4));
    let parent_b = frame(Vec3::new(-3.0, 0.8, -1.0), Quat::from_rotation_y(-0.6));
    let parent_local_a = parent_point_from_world(&parent_a, world_a);
    let bone_local_b = bone_local_point_from_world(&parent_b, world_a);
    let rotation_error = rebuilt_a
        .0
        .distance(local_target.0)
        .max(rebuilt_b.0.distance(local_target.0));
    let normal_error = (1.0 - normal_a.0.dot(local_normal.0))
        .abs()
        .max((1.0 - normal_b.0.dot(local_normal.0)).abs());
    let isolation_distance = parent_local_a.0.distance(bone_local_b.0);
    let passed = rotation_error < 1e-5 && normal_error < 1e-5 && isolation_distance > 0.1;
    let verdict = if passed { "PASS" } else { "FAIL" };

    println!(
        "[BF106.02][VERDICT] rotation_error={rotation_error:.8} normal_error={normal_error:.8} isolation_distance={isolation_distance:.6} -> {verdict}"
    );
    assert!(passed);
}
