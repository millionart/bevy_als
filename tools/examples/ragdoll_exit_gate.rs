use bevy::prelude::Vec3;
use bevy_als::als::state::LocomotionState;
use bevy_als::ragdoll_link::ragdoll_exit_route;

fn main() {
    let velocity = Vec3::new(2.5, -4.0, -1.5);
    let (air_state, air_velocity) = ragdoll_exit_route(false, velocity);
    let (ground_state, ground_velocity) = ragdoll_exit_route(true, velocity);
    let pass = air_state == LocomotionState::Air && air_velocity == velocity
        && ground_state == LocomotionState::Grounded && ground_velocity == Vec3::ZERO;
    println!("[BF102.50][GATE] air_state={air_state:?} air_velocity={air_velocity:?} ground_state={ground_state:?} ground_velocity={ground_velocity:?} -> {}",
        if pass { "PASS" } else { "FAIL" });
    assert!(pass, "Ragdoll exit routing differs from source");
}
