fn c577_query(
    spatial: &SpatialQuery,
    geometry: &Query<(&Collider, &Position, &Rotation)>,
    capsule: &Collider,
    origin: Vec3,
    orientation: Quat,
    direction: Dir3,
    distance: f32,
    hit: ShapeHitData,
    filter: &SpatialQueryFilter,
    step: f32,
) {
    use avian3d::collision::collider::contact_query::time_of_impact;
    let (floor, center, rotation) = geometry.get(hit.entity).unwrap();
    assert!(floor.shape_scaled().as_cuboid().is_some());
    let extent = floor.shape_scaled().compute_local_aabb().half_extents();
    let capsule_extent = capsule.shape_scaled().compute_local_aabb().half_extents();
    assert!(capsule.shape_scaled().as_capsule().is_some());
    let config = ShapeCastConfig { ignore_origin_penetration: true,
        ..ShapeCastConfig::from_max_distance(distance) };
    let cast = |from: Vec3, dir: Dir3| {
        let h = spatial.cast_shape_predicate(capsule, from, orientation, dir, &config, filter,
            &|candidate| candidate == hit.entity).unwrap();
        serde_json::json!({"start":from, "direction":*dir, "distance":h.distance, "normal":h.normal1,
            "point_on_floor":h.point1, "point_on_capsule":h.point2})
    };
    let mirrored = Dir3::new_unchecked(Vec3::new(direction.x, direction.y, -direction.z));
    let source: serde_json::Value = serde_json::from_slice(
        &std::fs::read("docs/audit/bf144/577/native-start-height.json").unwrap()).unwrap();
    let label = std::env::var("ALS_C575_MODE").unwrap();
    let source_height = source[&label].as_f64().unwrap() as f32;
    let direct = |box_center: Vec3, start: Vec3| {
        time_of_impact(floor, Position(box_center), *rotation, LinearVelocity::ZERO,
            capsule, Position(start), Rotation(orientation), LinearVelocity(*direction), distance)
            .unwrap().unwrap().time_of_impact
    };
    c516_write("landing-geometry.json", serde_json::json!({
        "floor_center":center.0, "floor_rotation":rotation.0, "floor_half_extents":extent,
        "capsule_extents":capsule_extent, "orientation":orientation,
        "max_distance":distance, "step":step, "actual_distance":hit.distance,
        "actual":cast(origin,direction),
        "direction_control":cast(origin,mirrored),
        "height_control":cast(origin.with_y(source_height),direction),
        "direct_world":direct(center.0,origin), "direct_relative":direct(Vec3::ZERO,origin-center.0)
    }));
}
