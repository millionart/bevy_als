fn c539_control(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    spatial: SpatialQuery,
    assets: Res<Assets<AlsMovementParams>>,
    actors: Query<(
        &Position,
        &Transform,
        &LocomotionState,
        &AlsMovementParamsHandle,
    )>,
    candidates: Query<(
        Option<&CollisionLayers>,
        Has<Sensor>,
        Option<&ColliderOf>,
        Option<&AlsCollisionLayerOwnership>,
    )>,
    names: Query<&Name>,
    mut commands: Commands,
) {
    if probe.frame != control.frame + 1 {
        return;
    }
    let actor = probe.actor.unwrap();
    let (position, transform, mode, handle) = actors.get(actor).unwrap();
    let params = assets.get(handle.id()).unwrap();
    let start = position.0;
    let (center, impact, floor) = bevy_als::als::motion::find_ground_floor(
        &spatial,
        start,
        params.crouch.standing_capsule_half_height,
        params.crouch.standing_capsule_radius,
        &params.cmc,
        *mode,
        &SpatialQueryFilter::from_excluded_entities([actor]),
        actor,
        &candidates,
    )
    .unwrap();
    let source_gap = std::env::var("ALS_C539_SOURCE_GAP_M")
        .ok()
        .map(|raw| raw.parse::<f32>().unwrap());
    let mut adjusted = start;
    if let Some(gap) = source_gap {
        adjusted.y = center + gap;
        let mut next_transform = *transform;
        next_transform.translation.y += adjusted.y - start.y;
        commands
            .entity(actor)
            .insert((Position(adjusted), next_transform));
    }
    c516_write(
        "gap-control.json",
        serde_json::json!({
            "initial_position":start,"position":adjusted,"floor_center":center,"impact":impact,
            "floor":c529_name(floor,&names),"before_gap_m":start.y-center,
            "after_gap_m":adjusted.y-center,"source_gap_m":source_gap
        }),
    );
}
