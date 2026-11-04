fn c570_install(app: &mut App) {
    app.add_systems(Update, c570_before.after(c523_before).before(c458_sample));
    app.add_systems(PostUpdate, c570_after.after(AlsActualVelocitySet));
}

fn c570_before(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    bodies: Query<(&Name, &RigidBody, &Position, &LinearVelocity, &avian3d::prelude::ComputedMass)>,
) {
    if probe.frame != control.frame { return; }
    let (_, kind, position, velocity, mass) =
        bodies.iter().find(|(name, _, _, _, _)| name.as_str() == "AuditLandingDeck").unwrap();
    c516_write("deck-before.json", serde_json::json!({
        "position": position.0, "velocity": velocity.0, "body": format!("{kind:?}"),
        "mass": mass.value()
    }));
}

fn c570_after(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    bodies: Query<(Entity, &Name, &Position, &LinearVelocity, &avian3d::prelude::ComputedMass)>,
    actors: Query<(&Position, &AlsMovementBase)>,
    collisions: Collisions,
    names: Query<&Name>,
) {
    let frame = probe.frame.saturating_sub(1);
    if !(control.frame..=control.frame+1).contains(&frame) { return; }
    let (entity, _, position, velocity, mass) =
        bodies.iter().find(|(_, name, _, _, _)| name.as_str() == "AuditLandingDeck").unwrap();
    let actor = probe.actor.unwrap();
    let (actor_position, base) = actors.get(actor).unwrap();
    let contacts: Vec<_> = collisions.collisions_with(entity)
        .filter(|pair| pair.collider1 == actor || pair.collider2 == actor)
        .flat_map(|pair| pair.manifolds.iter().flat_map(|manifold| manifold.points.iter()))
        .map(|point| point.normal_impulse).collect();
    c516_write(&format!("deck-after-{frame}.json"), serde_json::json!({
        "position": position.0, "velocity": velocity.0, "mass": mass.value(),
        "actor_position": actor_position.0, "actor_base": base.entity.and_then(|base| names.get(base).ok()).map(Name::as_str),
        "contact_impulses": contacts
    }));
}
