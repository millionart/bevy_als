    let capsule=capsules.get(actor).unwrap();
    let filter=SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts=|entity| bevy_als::als::motion::character_blocks_collider(actor,entity,&candidates);
    let mut substep_position=position.0;
    let mut incoming=base.relative_velocity;
    let mut rows=Vec::new();
    for _ in 0..2 {
        let step_velocity=bevy_als::als::dynamics::grounded_velocity_step_with_braking_friction(
            incoming,controller.basis.desired_motion,speed,dynamics.0,params.cmc.min_analog_walk_speed,
            factor,params.cmc.braking_substep_time,step);
        let request=step_velocity*step;
        let (direction,distance)=Dir3::new_and_length(request).unwrap();
        let config=ShapeCastConfig {ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(distance)};
        let hit=spatial.cast_shape_predicate(capsule,substep_position,Quat::IDENTITY,direction,&config,&filter,&accepts).unwrap();
        assert_eq!(c529_name(hit.entity,&names),Some("AuditStallWall"));
        assert!(!permissions.get(hit.entity).unwrap().allows_step_up);
        let travel=bevy_als::als::motion::movement_sweep_travel(distance,hit.distance);
        let advance=*direction*travel;
        let normal=hit.normal1.with_y(0.0).normalize_or_zero();
        let tail=request-advance;
        let slide=tail-normal*tail.dot(normal);
        let end=(substep_position+advance)+slide;
        let (floor,_,support)=bevy_als::als::motion::find_ground_floor(
            &spatial,end,half,radius,&params.cmc,LocomotionState::Grounded,&filter,actor,&candidates).unwrap();
        assert!((0.019..=0.024).contains(&(end.y-floor)));
        let applied=end-substep_position;
        rows.push(serde_json::json!({"start":substep_position,"input_velocity":step_velocity,
            "request":request,"hit":c529_hit(&hit,&names),"travel":travel,"slide":slide,
            "floor":c529_name(support,&names),"floor_gap_m":end.y-floor,"end":end,"applied":applied}));
        substep_position=end;
        incoming=applied.with_y(0.0)/step;
    }
    c516_write("stall-substeps.json",serde_json::json!({"step_s":step,"rows":rows}));
