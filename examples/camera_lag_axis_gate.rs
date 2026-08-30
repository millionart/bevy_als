use als_camera::{damp_ue_location_lag, ue_location_lag_to_bevy_camera_space};
use bevy::prelude::*;

fn main() {
    let ue_half_lives = Vec3::new(0.2, 0.4, 0.8);
    let bevy_half_lives = ue_location_lag_to_bevy_camera_space(ue_half_lives);
    let response = damp_ue_location_lag(Vec3::ZERO, Vec3::ONE, 0.2, ue_half_lives);
    let expected_half_lives = Vec3::new(0.4, 0.8, 0.2);
    let expected_response = Vec3::new(0.292_893_23, 0.159_103_59, 0.5);
    let pass = bevy_half_lives.distance(expected_half_lives) <= 1e-6
        && response.distance(expected_response) <= 1e-6;
    println!(
        "[BF102.52][GATE] bevy_half_lives=({:.3},{:.3},{:.3}) response=({:.6},{:.6},{:.6}) -> {}",
        bevy_half_lives.x,
        bevy_half_lives.y,
        bevy_half_lives.z,
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
