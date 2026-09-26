use bevy_als::als_camera::damp_location_lag;
use bevy::prelude::*;

fn main() {
    let half_lives = Vec3::new(0.4, 0.8, 0.2);
    let response = damp_location_lag(Vec3::ZERO, Vec3::ONE, 0.2, half_lives);
    let expected_response = Vec3::new(0.2927782, 0.15927327, 0.50011265);
    let pass = response.distance(expected_response) <= 1e-6;
    println!(
        "[BF102.52][GATE] bevy_half_lives=({:.3},{:.3},{:.3}) response=({:.6},{:.6},{:.6}) -> {}",
        half_lives.x,
        half_lives.y,
        half_lives.z,
        response.x,
        response.y,
        response.z,
        if pass { "PASS" } else { "FAIL" },
    );
    assert!(
        pass,
        "camera lag axis conversion did not preserve source response"
    );
}
