fn c568_install(app: &mut App) {
    app.edit_schedule(Update, |schedule| {
        let transaction = {
            let matches: Vec<_> = schedule.graph().systems.iter().filter(|(_, system, _)|
                system.name().to_string().ends_with("::cmc_movement_transaction_system")).collect();
            assert_eq!(matches.len(), 1);
            matches[0].1.default_system_sets()[0]
        };
        schedule.add_systems(c568_forward.after(c533_first_step).before(transaction));
    });
}

fn c568_forward(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    spatial: SpatialQuery,
    actors: Query<(&Position, &Collider, &LocomotionState, &AlsMovementBase)>,
    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&AlsCollisionLayerOwnership>)>,
    names: Query<&Name>,
) {
    if probe.frame != control.frame { return; }
    let actor = probe.actor.unwrap();
    let (position, capsule, mode, base) = actors.get(actor).unwrap();
    let filter = SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts = |candidate| bevy_als::als::motion::character_blocks_collider(actor, candidate, &candidates);
    let mut overlaps = spatial.shape_intersections(capsule, position.0, Quat::IDENTITY, &filter)
        .into_iter().filter(|entity| accepts(*entity))
        .map(|entity| c529_name(entity, &names)).collect::<Vec<_>>();
    overlaps.sort();
    let cast = |ignore| spatial.cast_shape_predicate(
        capsule, position.0, Quat::IDENTITY, Dir3::X, &ShapeCastConfig {
            ignore_origin_penetration: ignore,
            ..ShapeCastConfig::from_max_distance(0.3125)
        }, &filter, &accepts
    ).map(|hit| serde_json::json!({
        "entity": c529_name(hit.entity,&names), "distance": hit.distance, "normal": hit.normal1
    }));
    c516_write("forward.json", serde_json::json!({
        "actor": position.0, "mode": mode, "base": base.entity.map(|entity| c529_name(entity,&names)),
        "overlaps": overlaps, "raw": cast(false), "used": cast(true)
    }));
}
