use bevy_als::als::state::{Stance,RotationMode};
fn c521_trace(
    probe:Res<C458Probe>,
    state:Res<C516Control>,
    spatial:SpatialQuery,
    actors:Query<(
        &Collider,&Transform,&AlsMovementBaseDynamicsState,&AlsCmcAdapterRuntime,
        &TnuaController<AlsControlScheme>,&TnuaRigidBodyTracker,&AlsMovementParamsHandle,
        &Stance,&RotationMode,&bevy_als::als::gait::AlsMaxAllowedGait,&bevy_als::als::motion::AlsViewYaw,
    )>,
    assets:Res<Assets<AlsMovementParams>>,
    candidates:Query<(Option<&CollisionLayers>,Has<Sensor>,Option<&ColliderOf>,Option<&bevy_als::als::character::AlsCollisionLayerOwnership>)>,
    names:Query<&Name>,
) {
    use bevy_als::als::motion::{character_blocks_collider,movement_sweep_travel};
    if probe.frame!=state.frame || std::env::var("ALS_C520_ALIGN").ok().as_deref()!=Some("wall-162-aligned-air") {
        return;
    }
    let actor=probe.actor.unwrap();
    let (capsule,transform,base,runtime,controller,tracker,handle,stance,rotation,gait,yaw)=actors.get(actor).unwrap();
    let path=std::path::PathBuf::from(std::env::var("ALS_C458_OUTPUT").unwrap());
    let read=|name|serde_json::from_slice::<serde_json::Value>(&std::fs::read(path.with_file_name(name)).unwrap()).unwrap();
    let alignment=read("alignment.json");
    let stage=read("stage-before.json");
    let start:Vec3=serde_json::from_value(alignment["aligned_position"].clone()).unwrap();
    let step=stage["delta"].as_f64().unwrap() as f32 * 0.5;
    let walk_velocity=base.pre_collision_velocity.unwrap();
    let request=walk_velocity*step;
    let accepts=|candidate|character_blocks_collider(actor,candidate,&candidates);
    let filter=SpatialQueryFilter::from_excluded_entities([actor]);
    let (direction,distance)=Dir3::new_and_length(request).unwrap();
    let config=ShapeCastConfig {ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(distance)};
    let hit=spatial.cast_shape_predicate(capsule,start,Quat::IDENTITY,direction,&config,&filter,&accepts).unwrap();
    assert_eq!(names.get(hit.entity).unwrap().as_str(),"AuditWalkOffBarrier");
    let advanced=*direction*movement_sweep_travel(distance,hit.distance);
    let tail=request-advanced;
    let normal=hit.normal1.with_y(0.0).normalize_or_zero();
    let departure=(start+advanced)+(tail-normal*tail.dot(normal));
    let actual=departure-start;
    let air_time=step+step*(1.0-(actual.with_y(0.0).length()/(walk_velocity.length()*step)).min(1.0));
    let params=assets.get(&handle.0).unwrap();
    let speed=bevy_als::als::dynamics::max_allowed_speed(params,*stance,*rotation,gait.0,base.relative_velocity,yaw.0);
    let maximum=alignment["air_maximum"].as_f64().unwrap() as f32;
    let horizontal=bevy_als::als::dynamics::grounded_velocity_step_with_braking_friction(
        walk_velocity.with_y(0.0),controller.basis.desired_motion,speed,
        bevy_als::als::params::GroundedDynamics {acceleration:maximum*0.15,braking_deceleration:0.0,ground_friction:0.0},
        params.cmc.min_analog_walk_speed,0.0,params.cmc.braking_substep_time,air_time);
    let velocity=horizontal+Vec3::Y*walk_velocity.y+tracker.gravity*air_time;
    let movement=(walk_velocity+velocity)*(0.5*air_time);
    let mut velocity=velocity;
    let mut remaining=movement;
    let mut applied=Vec3::ZERO;
    let mut time=air_time;
    let mut previous=None;
    let mut hits=Vec::new();
    for _ in 0..3 {
        let Ok((direction,distance))=Dir3::new_and_length(remaining) else {break;};
        let config=ShapeCastConfig {ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(distance)};
        let origin=departure+applied;
        let Some(hit)=spatial.cast_shape_predicate(capsule,origin,transform.rotation,direction,&config,&filter,&accepts)
            else {applied+=remaining;break;};
        let travel=movement_sweep_travel(distance,hit.distance);
        applied+=*direction*travel;
        let normal=hit.normal1.try_normalize().unwrap();
        time*=1.0-travel/distance;
        let incoming=velocity;
        let budget=velocity*time;
        let slide=bevy_als::extensions::motion::falling_slide(budget,normal,previous);
        previous=Some(normal);
        if time>f32::EPSILON {velocity=slide/time;}
        hits.push(serde_json::json!({"origin":origin,"movement":remaining,"distance":distance,
            "hit_distance":hit.distance,"travel":travel,"normal":normal,"point":hit.point1,
            "remaining_time":time,"incoming":incoming,"outgoing":velocity,"slide":slide,
            "name":names.get(hit.entity).ok().map(|name|name.as_str())}));
        if slide.dot(budget)<=0.0 {break;}
        remaining=slide;
    }
    c516_write("wall-trace.json",serde_json::json!({
        "ground_start":start,"ground_request":request,"ground_hit":hit.distance,"ground_normal":hit.normal1,
        "departure":departure,"air_time":air_time,"capsule_half_extents":capsule.shape_scaled().compute_local_aabb().half_extents(),
        "air_movement":movement,"hits":hits,"final_position":departure+applied,"final_velocity":velocity,
        "actual_position":transform.translation,"actual_velocity":runtime.grounded_collision_velocity.unwrap()
    }));
}
