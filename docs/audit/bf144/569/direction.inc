fn c569_direction(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    actors: Query<(
        &Position, &LinearVelocity, &LocomotionState, &RotationMode,
        &bevy_als::actual_motion::AlsActualMotion,
        &bevy_als::als::motion::AlsRotationResponseState,
    )>,
) {
    if probe.frame.saturating_sub(1) != control.frame { return; }
    let (position, velocity, mode, rotation, motion, response) =
        actors.get(probe.actor.unwrap()).unwrap();
    c516_write("direction.json", serde_json::json!({
        "position": position.0, "velocity": velocity.0, "mode": mode, "rotation_mode": rotation,
        "motion_velocity": motion.world_velocity, "motion_velocity_yaw": motion.velocity_yaw_degrees,
        "target_yaw": response.target_yaw, "response_velocity_yaw": response.velocity_yaw,
        "horizontal_speed": motion.horizontal_speed
    }));
}
