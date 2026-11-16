use bevy::math::DQuat;
use bevy_als::extensions::control_rig_hierarchy::{RigTransform, rig_slerp};

fn scalar(rotation: DQuat) -> DQuat {
    let squared = rotation.y.mul_add(rotation.y, rotation.x * rotation.x)
        + rotation.w.mul_add(rotation.w, rotation.z * rotation.z);
    if squared >= f64::from(1e-8_f32) {
        rotation * squared.sqrt().recip()
    } else {
        DQuat::IDENTITY
    }
}

fn slerp_cosine(a: DQuat, b: DQuat) -> f64 {
    (((a.x * b.x + a.z * b.z) + a.y * b.y) + a.w * b.w).abs()
}

fn main() {
    let w = 0.9999_f64 + 3e-11;
    let goal = DQuat::from_xyzw((1.0 - w * w).sqrt(), 0.0, 0.0, w);
    let rig_input = |rotation| RigTransform {
        rotation,
        ..Default::default()
    };
    let native_a = rig_input(DQuat::IDENTITY)
        .global(RigTransform::default())
        .rotation;
    let native_b = rig_input(goal).global(RigTransform::default()).rotation;
    let scalar_a = scalar(DQuat::IDENTITY);
    let scalar_b = scalar(goal);
    let old_cosine = slerp_cosine(native_a, native_b);
    let scalar_cosine = slerp_cosine(scalar_a, scalar_b);
    let old = rig_slerp(native_a, native_b, 0.25);
    let trial = rig_slerp(scalar_a, scalar_b, 0.25);
    let phase = |rotation: DQuat| 2.0 * rotation.x.atan2(rotation.w);
    println!(
        "{{\"input\":{:?},\"old_cosine\":{old_cosine:.17},\"scalar_cosine\":{scalar_cosine:.17},\"old_angle_rad\":{:.17},\"scalar_angle_rad\":{:.17},\"old_spherical\":{},\"scalar_spherical\":{}}}",
        goal.to_array(),
        phase(old),
        phase(trial),
        old_cosine < 0.9999,
        scalar_cosine < 0.9999,
    );
}
