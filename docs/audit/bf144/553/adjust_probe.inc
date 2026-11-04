
fn c553_adjust(
    probe:Res<C458Probe>,
    control:Res<C516Control>,
    actors:Query<(&Position,&Transform,&Collider,&LocomotionState,&AlsMovementParamsHandle,Has<ChildOf>)>,
    spatial:SpatialQuery,
    mut commands:Commands,
    assets:Res<Assets<AlsMovementParams>>,
    candidates:Query<(Option<&CollisionLayers>,Has<Sensor>,Option<&ColliderOf>,Option<&AlsCollisionLayerOwnership>)>,
    names:Query<&Name>,
) {
    if probe.frame!=control.frame+1 {return;}
    let actor=probe.actor.unwrap();
    let (position,transform,capsule,mode,handle,parented)=actors.get(actor).unwrap();
    assert_eq!(*mode,LocomotionState::Grounded);
    assert!(!parented);
    let start=position.0;
    let params=assets.get(handle.id()).unwrap();
    let filter=SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts=|entity| bevy_als::als::motion::character_blocks_collider(actor,entity,&candidates);
    let floor=bevy_als::als::motion::find_ground_floor(&spatial,start,params.crouch.standing_capsule_half_height,
        params.crouch.standing_capsule_radius,&params.cmc,*mode,&filter,actor,&candidates);
    let enabled=std::env::var_os("ALS_C553_ADJUST").is_some();
    let request=floor.map(|(center,_,_)| center+0.0215-start.y);
    let sweep=request.and_then(|dy| {
        let (direction,distance)=Dir3::new_and_length(Vec3::Y*dy).ok()?;
        let hit=spatial.cast_shape_predicate(capsule,start,transform.rotation,direction,
            &ShapeCastConfig {ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(distance)},&filter,&accepts);
        let travel=hit.as_ref().map_or(distance,|hit|bevy_als::als::motion::movement_sweep_travel(distance,hit.distance));
        Some((start+*direction*travel,hit))
    });
    let selected = if enabled {
        assert!(request.unwrap() < -0.45);
        let (end,hit)=sweep.as_ref().unwrap();
        assert!(hit.is_none());
        let mut moved=*transform;
        moved.translation=*end;
        commands.entity(actor).insert((Position(*end),moved));
        *end
    } else {start};
    c516_write("floor-adjustment.json",serde_json::json!({
        "enabled":enabled,"start":start,"selected":selected,"request_y":request,
        "floor":floor.map(|(center,impact,entity)|serde_json::json!({"center":center,"impact":impact,"name":c529_name(entity,&names)})),
        "end":sweep.as_ref().map(|(end,_)|end),
        "hit":sweep.as_ref().and_then(|(_,hit)|hit.as_ref().map(|hit|c529_hit(hit,&names))),
    }));
}
