use als_ext::{apply_inertial_residual, capture_inertial_residual, inertial_residual_weight};
use bevy::prelude::*;
use bevy_animation_graph::core::pose::{BoneId, BonePose, Pose};

const EPSILON: f32 = 1e-4;

fn main() {
    let bone = BoneId::default();
    let source = pose_with_rotation(bone, Quat::IDENTITY);
    let target = pose_with_rotation(bone, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
    let source_after_transition =
        pose_with_rotation(bone, Quat::from_rotation_y(std::f32::consts::PI));
    let residual = capture_inertial_residual(&source, &target);
    let start = apply_inertial_residual(&target, &residual, 1.0);
    let midpoint =
        apply_inertial_residual(&target, &residual, inertial_residual_weight(0.5, 0.2, 0.05));
    let end = apply_inertial_residual(&target, &residual, 0.0);
    let source_rotation = rotation(&source);
    let target_rotation = rotation(&target);
    let after_rotation = rotation(&source_after_transition);
    let start_rotation = rotation(&start);
    let midpoint_rotation = rotation(&midpoint);
    let end_rotation = rotation(&end);
    let start_matches_source = quat_close(start_rotation, source_rotation);
    let end_matches_target = quat_close(end_rotation, target_rotation);
    let captured_residual_is_stable = midpoint_rotation.dot(after_rotation).abs() < 0.999;
    let passed = start_matches_source && end_matches_target && captured_residual_is_stable;
    let verdict = if passed { "PASS" } else { "FAIL" };
    println!(
        "[BF102.24][VERDICT] start_yaw={:.3} midpoint_yaw={:.3} end_yaw={:.3} source_after_yaw={:.3} start_matches_source={} end_matches_target={} captured_residual_is_stable={} -> {verdict}",
        yaw_degrees(start_rotation),
        yaw_degrees(midpoint_rotation),
        yaw_degrees(end_rotation),
        yaw_degrees(after_rotation),
        start_matches_source,
        end_matches_target,
        captured_residual_is_stable,
    );
    if !passed {
        std::process::exit(24);
    }
}

fn pose_with_rotation(bone: BoneId, rotation: Quat) -> Pose {
    let mut pose = Pose::default();
    pose.add_bone(
        BonePose::from_transform(Transform::from_rotation(rotation)),
        bone,
    );
    pose
}

fn rotation(pose: &Pose) -> Quat {
    pose.bones[0].rotation.unwrap()
}

fn quat_close(left: Quat, right: Quat) -> bool {
    left.dot(right).abs() >= 1.0 - EPSILON
}

fn yaw_degrees(rotation: Quat) -> f32 {
    rotation.to_euler(EulerRot::YXZ).0.to_degrees()
}
