fn c567_install(app: &mut App) {
    app.add_systems(PostUpdate, (
        c567_before.before(PhysicsSystems::First),
        c567_after.after(AlsActualVelocitySet),
    ));
}

fn c567_before(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    actors: Query<(&Position, &LocomotionState, &AlsMovementBase), With<AlsCharacter>>,
    mut bodies: Query<(&Name, &mut Position, &mut Transform, &LinearVelocity), Without<AlsCharacter>>,
    names: Query<&Name>,
) {
    if probe.frame.saturating_sub(1) != control.frame { return; }
    let (actor, mode, base) = actors.get(probe.actor.unwrap()).unwrap();
    let base = base.entity.and_then(|entity| names.get(entity).ok()).map(Name::as_str);
    let (_, mut position, mut transform, velocity) = bodies.iter_mut()
        .find(|(name, _, _, _)| name.as_str() == "AuditWalkOffPlatform").unwrap();
    assert_eq!(velocity.0, Vec3::ZERO);
    let old_before = position.0;
    if std::env::var("ALS_C567_MOVING").unwrap() == "1" {
        position.0.z += 0.20;
        transform.translation.z += 0.20;
    }
    c516_write("late-base-before.json", serde_json::json!({
        "actor": actor.0, "mode": format!("{mode:?}"), "base": base,
        "old_before": old_before, "old_after": position.0, "old_velocity": velocity.0
    }));
}

fn c567_after(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    actors: Query<(&Position, &LocomotionState, &AlsMovementBase)>,
    names: Query<&Name>,
) {
    if probe.frame.saturating_sub(1) != control.frame { return; }
    let (actor, mode, base) = actors.get(probe.actor.unwrap()).unwrap();
    let name = base.entity.and_then(|entity| names.get(entity).ok()).map(Name::as_str);
    c516_write("late-base-after.json", serde_json::json!({
        "actor": actor.0, "mode": format!("{mode:?}"), "base": name, "base_location": base.location
    }));
}
