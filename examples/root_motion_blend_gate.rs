use als_ext::blend_normalized_root_motion;
use bevy::prelude::{Quat, Vec3};
use bevy_animation_graph::core::pose::RootMotionDelta;

fn main() {
    let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let delta = RootMotionDelta {
        translation: Vec3::new(2.0, 0.0, 0.0),
        rotation,
    };
    let blended =
        blend_normalized_root_motion(&[(Some(delta.clone()), 0.5), (Some(delta), 0.5)]).unwrap();
    let alignment = blended.rotation.dot(rotation).abs();
    let translation_error = blended.translation.distance(Vec3::new(2.0, 0.0, 0.0));
    let passed = alignment > 0.99999 && translation_error < 1e-6;
    let verdict = if passed { "PASS" } else { "FAIL" };
    println!(
        "[BF102.29][VERDICT] identical_rotation_alignment={alignment:.6} translation_error={translation_error:.6} -> {verdict}"
    );
    if !passed {
        std::process::exit(29);
    }
}
