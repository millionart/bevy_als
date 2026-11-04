fn c571_install(app: &mut App) {
    use bevy_als::als::dynamics::grounded_dynamics_motor_system;
    app.add_systems(Update, c571_before.after(TnuaPipelineSystems::Logic).before(grounded_dynamics_motor_system));
    app.add_systems(Last, (c571_clock.after(c523_delta), c571_after));
}

fn c571_clock(probe: Res<C458Probe>, mut clock: ResMut<TimeUpdateStrategy>) {
    if (41..=75).contains(&probe.frame) {
        *clock = TimeUpdateStrategy::ManualDuration(Duration::from_micros(31_250));
    }
}

fn c571_before(
    probe: Res<C458Probe>,
    time: Res<Time>,
    actors: Query<(&LocomotionState, &bevy_als::als::dynamics::AlsLandingBrakingFriction,
        &bevy_als::actual_motion::AlsActualMotion, &LinearVelocity)>,
) {
    let frame = probe.frame;
    if !(39..=75).contains(&frame) { return; }
    let (mode, braking, motion, velocity) = actors.get(probe.actor.unwrap()).unwrap();
    c516_write(&format!("timer-before-{frame}.json"), serde_json::json!({
        "frame": frame, "delta": time.delta_secs(), "mode": mode,
        "factor": braking.factor, "retained_input": motion.has_input, "velocity": velocity.0
    }));
}

fn c571_after(
    probe: Res<C458Probe>,
    time: Res<Time>,
    actors: Query<(&LocomotionState, &bevy_als::als::dynamics::AlsLandingBrakingFriction,
        &bevy_als::actual_motion::AlsActualMotion, &LinearVelocity)>,
) {
    let frame = probe.frame.saturating_sub(1);
    if !(39..=75).contains(&frame) { return; }
    let (mode, braking, motion, velocity) = actors.get(probe.actor.unwrap()).unwrap();
    c516_write(&format!("timer-after-{frame}.json"), serde_json::json!({
        "frame": frame, "delta": time.delta_secs(), "mode": mode,
        "factor": braking.factor, "remaining_seconds": braking.remaining_seconds,
        "input": motion.has_input, "velocity": velocity.0
    }));
}
