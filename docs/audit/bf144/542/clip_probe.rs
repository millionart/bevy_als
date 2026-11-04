
fn c542_clip(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    time: Res<Time>,
    spatial: SpatialQuery,
    mut actors: Query<(&Transform, &LinearVelocity, &Collider, &mut TnuaMotor)>,
    candidates: Query<(
        Option<&CollisionLayers>,
        Has<Sensor>,
        Option<&ColliderOf>,
        Option<&AlsCollisionLayerOwnership>,
    )>,
    names: Query<&Name>,
) {
    if probe.frame != control.frame {
        return;
    }
    let actor = probe.actor.unwrap();
    let (transform, velocity, capsule, mut motor) = actors.get_mut(actor).unwrap();
    let dt = time.delta_secs();
    let start = transform.translation;
    let original_boost = motor.lin.boost;
    let remaining = (velocity.0 + original_boost).with_y(0.0) * dt;
    let (direction, distance) = Dir3::new_and_length(remaining).unwrap();
    let filter = SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts = |candidate| {
        bevy_als::als::motion::character_blocks_collider(actor, candidate, &candidates)
    };
    let config = ShapeCastConfig {
        ignore_origin_penetration: true,
        ..ShapeCastConfig::from_max_distance(distance)
    };
    let hit = spatial.cast_shape_predicate(
        capsule, start, Quat::IDENTITY, direction, &config, &filter, &accepts,
    );
    let enabled = std::env::var_os("ALS_C542_CLIP").is_some();
    let clipped = hit.as_ref().map(|hit| {
        *direction * bevy_als::als::motion::movement_sweep_travel(distance, hit.distance)
    });
    if enabled {
        let requested = clipped.unwrap() / dt;
        motor.lin.boost.x = requested.x - velocity.0.x;
        motor.lin.boost.z = requested.z - velocity.0.z;
    }
    c516_write(
        "horizontal-slide.json",
        serde_json::json!({
            "enabled":enabled,"dt":dt,"start":start,"velocity":velocity.0,
            "remaining":remaining,"clipped":clipped,"original_boost":original_boost,
            "selected_boost":motor.lin.boost,"acceleration":motor.lin.acceleration,
            "hit":hit.as_ref().map(|hit|c529_hit(hit,&names)),
        }),
    );
}
