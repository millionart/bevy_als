use als_ext::ik::{
    StableTwoBonePole, blend_ik_rotation, child_world_rotation_after_parent_rotation,
    solve_two_bone_ik,
};
use bevy::prelude::*;

fn main() {
    let left_hip = Vec3::new(-0.2, 0.0, 0.0);
    let left_knee = Vec3::new(-0.2, -0.4, 0.16);
    let left_ankle = Vec3::new(-0.2, -0.8, 0.0);
    let right_hip = Vec3::new(0.2, 0.0, 0.0);
    let right_knee = Vec3::new(0.2, -0.4, -0.16);
    let right_ankle = Vec3::new(0.2, -0.8, 0.0);
    let mut left_pole = StableTwoBonePole::default();
    let mut right_pole = StableTwoBonePole::default();
    let left_direction = left_pole.resolve(
        Quat::IDENTITY,
        left_hip,
        left_knee,
        left_ankle,
        left_ankle + Vec3::new(0.04, 0.0, 0.02),
        Vec3::Z,
    );
    let right_direction = right_pole.resolve(
        Quat::IDENTITY,
        right_hip,
        right_knee,
        right_ankle,
        right_ankle + Vec3::new(-0.04, 0.0, 0.02),
        Vec3::Z,
    );
    let solve = solve_two_bone_ik(
        left_hip,
        left_knee,
        left_ankle,
        left_ankle + Vec3::new(0.04, 0.0, 0.02),
        left_knee.distance(left_hip),
        left_ankle.distance(left_knee),
        left_direction,
    )
    .unwrap();
    let child_axis = Vec3::NEG_Y;
    let target_axis = (left_ankle + Vec3::new(0.04, 0.0, 0.02) - solve.knee_position).normalize();
    let child_rotation = child_world_rotation_after_parent_rotation(
        solve.hip_rotation_world,
        Quat::IDENTITY,
        child_axis,
        target_axis,
    )
    .unwrap();
    let blended = blend_ik_rotation(Quat::IDENTITY, Quat::from_rotation_y(0.8), 0.5);
    let mirror_dot = left_direction.dot(right_direction);
    let child_alignment = (child_rotation * child_axis).dot(target_axis);
    let blend_angle = blended.angle_between(Quat::IDENTITY);
    let passed =
        mirror_dot < -0.8 && child_alignment > 0.999_99 && blend_angle > 0.1 && blend_angle < 0.7;
    let verdict = if passed { "PASS" } else { "FAIL" };

    println!(
        "[BF106.03][VERDICT] mirror_dot={mirror_dot:.6} child_alignment={child_alignment:.6} blend_angle={blend_angle:.6} -> {verdict}"
    );
    assert!(passed);
}
