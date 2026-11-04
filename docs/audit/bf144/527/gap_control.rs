fn c527_nudge(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    mut actors: Query<(&mut Position, &mut Transform, &LocomotionState, &AlsCmcAdapterRuntime)>,
) {
    if probe.frame != control.frame + 1 {
        return;
    }
    let Ok(raw) = std::env::var("ALS_C527_SHIFT_M") else {
        return;
    };
    let shift: f32 = raw.parse().unwrap();
    assert!(shift.is_finite() && shift.abs() < 0.01);
    let (mut position, mut transform, mode, runtime) = actors.get_mut(probe.actor.unwrap()).unwrap();
    let original = position.0;
    assert_eq!(*mode, LocomotionState::Air);
    assert!(runtime.airborne_sweep_contact);
    position.0.y += shift;
    transform.translation.y += shift;
    c516_write("gap-control.json", serde_json::json!({
        "ordinal":control.frame,"shift_m":shift,"original_position":original,"shifted_position":position.0,
        "planned_velocity":runtime.grounded_collision_velocity,"generated_contact":runtime.airborne_sweep_contact,
    }));
}
