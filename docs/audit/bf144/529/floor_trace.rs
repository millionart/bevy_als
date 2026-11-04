use bevy_als::als::character::AlsCollisionLayerOwnership;

fn c529_name<'a>(entity: Entity, names: &'a Query<&Name>) -> Option<&'a str> {
    names.get(entity).ok().map(|name| name.as_str())
}

fn c529_hit(hit: &ShapeHitData, names: &Query<&Name>) -> serde_json::Value {
    serde_json::json!({"entity":c529_name(hit.entity,names),"normal":hit.normal1,
        "point":hit.point1,"point2":hit.point2,"distance_m":hit.distance})
}

fn c529_trace(
    spatial: &SpatialQuery,
    origin: Vec3,
    half: f32,
    radius: f32,
    cmc: &bevy_als::als::params::CmcAdapterParams,
    mode: LocomotionState,
    actor: Entity,
    candidates: &Query<(
        Option<&CollisionLayers>,
        Has<Sensor>,
        Option<&ColliderOf>,
        Option<&AlsCollisionLayerOwnership>,
    )>,
    names: &Query<&Name>,
) {
    let output = std::path::PathBuf::from(std::env::var("ALS_C458_OUTPUT").unwrap());
    let read =
        |path| serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap();
    let movement = read(output.with_file_name("initial-observed.json"));
    let read_vec =
        |value: &serde_json::Value| -> Vec3 { serde_json::from_value(value.clone()).unwrap() };
    let start = read_vec(&movement["position"]);
    let (direction, distance) = Dir3::new_and_length(read_vec(&movement["request"])).unwrap();
    let hit_distance = movement["hit_distance"].as_f64().unwrap() as f32;
    let target_stop =
        start + *direction * bevy_als::als::motion::movement_sweep_travel(distance, hit_distance);
    let native = read(std::path::PathBuf::from(
        "docs/audit/bf144/529/native-origins.json",
    ));
    let label = std::env::var("ALS_C529_LABEL").unwrap();
    let native_stop = read_vec(&native[label]);
    let filter = SpatialQueryFilter::from_excluded_entities([actor]);
    let display = |(center, impact, entity)| {
        serde_json::json!({
        "center":center,"impact":impact,"entity":c529_name(entity,names)})
    };
    let mut phases = serde_json::Map::new();
    for (phase, point) in [
        ("post", origin),
        ("target-stop", target_stop),
        ("native-stop", native_stop),
    ] {
        let published = bevy_als::als::motion::find_ground_floor(
            spatial, point, half, radius, cmc, mode, &filter, actor, candidates,
        );
        let mut variants = serde_json::Map::new();
        for skip in ["none", "short", "center", "perch"] {
            let mut events = Vec::new();
            let result = c529_floor(
                spatial,
                point,
                half,
                radius,
                cmc,
                mode,
                &filter,
                actor,
                candidates,
                skip,
                &mut events,
                names,
            )
            .map(|(floor, _)| floor);
            if skip == "none" {
                assert_eq!(result, published);
            }
            variants.insert(
                skip.to_string(),
                serde_json::json!({"events":events,"result":result.map(display)}),
            );
        }
        phases.insert(
            phase.to_string(),
            serde_json::json!({
            "origin":point,"public":published.map(display),"variants":variants}),
        );
    }
    c516_write(
        "floor-provenance.json",
        serde_json::json!({
            "phases":phases,"half_m":half,"radius_m":radius,"mode":mode,"perch_threshold_m":cmc.perch_radius_threshold,
        }),
    );
}
