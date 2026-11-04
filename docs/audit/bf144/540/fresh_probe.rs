fn c540_probe(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    spatial: SpatialQuery,
    assets: Res<Assets<AlsMovementParams>>,
    actors: Query<(&Position, &LocomotionState, &AlsMovementParamsHandle)>,
    candidates: Query<(
        Option<&CollisionLayers>,
        Has<Sensor>,
        Option<&ColliderOf>,
        Option<&AlsCollisionLayerOwnership>,
    )>,
    names: Query<&Name>,
    geometry: Query<(&Name, &Position, &Collider)>,
) {
    if probe.frame != control.frame + 1 {
        return;
    }
    let actor = probe.actor.unwrap();
    let (position, mode, handle) = actors.get(actor).unwrap();
    let params = assets.get(handle.id()).unwrap();
    let half = params.crouch.standing_capsule_half_height;
    let radius = params.crouch.standing_capsule_radius;
    let filter = SpatialQueryFilter::from_excluded_entities([actor]);
    let mut events = Vec::new();
    let selected = c529_floor(
        &spatial,
        position.0,
        half,
        radius,
        &params.cmc,
        *mode,
        &filter,
        actor,
        &candidates,
        "none",
        &mut events,
        &names,
    )
    .map(|(floor, _)| floor);
    let live = bevy_als::als::motion::find_ground_floor(
        &spatial,
        position.0,
        half,
        radius,
        &params.cmc,
        *mode,
        &filter,
        actor,
        &candidates,
    );
    assert_eq!(selected, live);
    let (_, center, collider) = geometry
        .iter()
        .find(|(name, _, _)| name.as_str() == "AuditWalkOffPlatform")
        .unwrap();
    c516_write(
        "fresh-floor.json",
        serde_json::json!({
            "origin":position.0,"mode":mode,"half_m":half,"radius_m":radius,
            "floor":selected.map(|(height,impact,entity)|serde_json::json!({"center":height,"impact":impact,"entity":c529_name(entity,&names)})),
            "box_center":center.0,"box_half_extents":collider.shape_scaled().compute_local_aabb().half_extents(),
            "events":events
        }),
    );
}
