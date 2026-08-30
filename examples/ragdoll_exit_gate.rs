use als_core::character::AlsCharacter;
use als_core::motion::AlsRagdollAirExit;
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin, state_transition_system};
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use bevy_als::ragdoll_link::{AlsRagdollGroundFollow, ragdoll_exit_route_system};

fn main() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AlsStateTransitionPlugin))
        .add_systems(
            Update,
            ragdoll_exit_route_system.before(state_transition_system),
        );
    let airborne_velocity = Vec3::new(2.5, -4.0, -1.5);
    let airborne = spawn_exit_case(&mut app, false, airborne_velocity);
    let grounded = spawn_exit_case(&mut app, true, airborne_velocity);
    app.update();
    let air_state = *app.world().get::<LocomotionState>(airborne).unwrap();
    let air_velocity = app.world().get::<LinearVelocity>(airborne).unwrap().0;
    let air_hold = app.world().get::<AlsRagdollAirExit>(airborne).is_some();
    let ground_state = *app.world().get::<LocomotionState>(grounded).unwrap();
    let ground_velocity = app.world().get::<LinearVelocity>(grounded).unwrap().0;
    let pass = air_state == LocomotionState::Air
        && air_velocity.distance(airborne_velocity) <= 1e-6
        && air_hold
        && ground_state == LocomotionState::Grounded
        && ground_velocity.length_squared() <= 1e-6;
    println!(
        "[BF102.50][GATE] air_state={air_state:?} air_hold={air_hold} air_velocity=({:.3},{:.3},{:.3}) ground_state={ground_state:?} ground_velocity={:.3} -> {}",
        air_velocity.x,
        air_velocity.y,
        air_velocity.z,
        ground_velocity.length(),
        if pass { "PASS" } else { "FAIL" },
    );
    assert!(
        pass,
        "ragdoll exit route did not preserve airborne velocity"
    );
}

fn spawn_exit_case(app: &mut App, grounded: bool, velocity: Vec3) -> Entity {
    app.world_mut()
        .spawn((
            AlsCharacter,
            LocomotionState::Ragdolling,
            Stance::Standing,
            Gait::Running,
            RotationMode::VelocityDirection,
            OverlayState::Default,
            AlsStateRequest {
                locomotion_state: Some(LocomotionState::Grounded),
                ragdoll_exit: true,
                ..default()
            },
            LinearVelocity(Vec3::ONE),
            AlsRagdollGroundFollow {
                pelvis: Vec3::ZERO,
                target: Vec3::new(0.0, 3.0, 0.0),
                velocity,
                grounded,
            },
        ))
        .id()
}
