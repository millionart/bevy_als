use std::num::NonZeroU8;

use bevy::prelude::*;

use bevy_als::als::motion::view_relative_move_direction;

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_systems(Startup, verify_view_relative_movement)
        .run();
}

fn verify_view_relative_movement(mut exit: MessageWriter<AppExit>) {
    let samples = [
        (0.0, Vec2::Y, Vec3::NEG_Z),
        (0.0, Vec2::NEG_Y, Vec3::Z),
        (0.0, Vec2::X, Vec3::X),
        (0.0, Vec2::NEG_X, Vec3::NEG_X),
        (std::f32::consts::FRAC_PI_2, Vec2::Y, Vec3::NEG_X),
        (std::f32::consts::FRAC_PI_2, Vec2::X, Vec3::NEG_Z),
        (std::f32::consts::PI, Vec2::Y, Vec3::Z),
        (-std::f32::consts::FRAC_PI_2, Vec2::Y, Vec3::X),
    ];
    let mut max_error = 0.0_f32;
    let mut report = Vec::new();
    for (yaw, input, expected) in samples {
        let actual = view_relative_move_direction(input, yaw);
        max_error = max_error.max(actual.distance(expected));
        report.push(format!("yaw={yaw:.3} input={input:?} world={actual:?}"));
    }
    if max_error <= 1e-5 {
        println!(
            "[BCHAR3][VERDICT] max_error={max_error:.6} samples={} -> PASS",
            report.join(" | ")
        );
        exit.write(AppExit::Success);
    } else {
        eprintln!(
            "[BCHAR3][VERDICT] max_error={max_error:.6} samples={} -> FAIL",
            report.join(" | ")
        );
        exit.write(AppExit::Error(NonZeroU8::new(6).unwrap()));
    }
}
