use bevy_als::als::character::AlsCollisionLayerOwnership;
use bevy_als::als::mantle::AlsMantleTargetPermission;

fn c560_read(name:&str) -> serde_json::Value {
    let root=std::path::PathBuf::from(std::env::var("ALS_C458_OUTPUT").unwrap());
    serde_json::from_slice(&std::fs::read(root.with_file_name(name)).unwrap()).unwrap()
}

fn c560_vec(value:&serde_json::Value) -> Vec3 {
    serde_json::from_value(value.clone()).unwrap()
}

fn c560_replay(
    probe:Res<C458Probe>,
    control:Res<C516Control>,
    spatial:SpatialQuery,
    actors:Query<(&Transform,&Collider,&AlsMovementParamsHandle,&Stance,&RotationMode,&bevy_als::actual_motion::AlsActualMotion,
        &bevy_als::als::gait::AlsMaxAllowedGait,&bevy_als::als::motion::AlsViewYaw)>,
    assets:Res<Assets<AlsMovementParams>>,
    candidates:Query<(Option<&CollisionLayers>,Has<Sensor>,Option<&ColliderOf>,Option<&AlsCollisionLayerOwnership>)>,
    permissions:Query<&AlsMantleTargetPermission>,
) {
    if probe.frame!=control.frame {return;}
    let actor=probe.actor.unwrap();
    let (transform,capsule,handle,stance,rotation,motion,gait,yaw)=actors.get(actor).unwrap();
    assert!(!motion.has_input && motion.world_velocity.y > -bevy_als::als::roll::ROLL_ON_LAND_SPEED_THRESHOLD);
    let params=assets.get(handle.id()).unwrap();
    let trace=c560_read("third-contact.json");
    let third=&trace["hits"][2];
    assert!(trace["landing_candidate"].as_bool().unwrap());
    assert!(third["travel"].as_f64().unwrap()>0.0);
    let dt=trace["step"].as_f64().unwrap() as f32;
    let remaining=third["time_left"].as_f64().unwrap() as f32;
    assert!(remaining<params.cmc.max_simulation_time_step && params.cmc.max_simulation_iterations>1);
    let incoming=(c560_vec(&trace["initial"])+c560_vec(&trace["gravity"])*dt).with_y(0.0);
    let deflected=c560_vec(&trace["third_velocity"]).with_y(0.0);
    let mut start=*transform;
    start.translation=c560_vec(&third["stop"]);
    let extent=capsule.shape_scaled().compute_local_aabb().half_extents();
    let half=extent.y;
    let radius=extent.x.min(extent.z);
    assert!(c560_adjust_walking_floor_height(&spatial,capsule,&mut start,half,radius,&params.cmc,actor,&candidates));
    let mut variants=serde_json::Map::new();
    for (label, velocity) in [("incoming",incoming),("deflected",deflected)] {
        let thresholds=params.gait_speed_thresholds(*stance,*rotation,velocity,yaw.0);
        let gait_amount=bevy_als::als::gait::calculate_gait_amount_for_thresholds(velocity.length(),thresholds);
        let dynamics=params.grounded_dynamics(*stance,*rotation,gait_amount);
        let speed=bevy_als::als::dynamics::max_allowed_speed(params,*stance,*rotation,gait.0,velocity,yaw.0);
        let step_velocity=bevy_als::als::dynamics::grounded_velocity_step_with_braking_friction(
            velocity,Vec3::ZERO,speed,dynamics,params.cmc.min_analog_walk_speed,
            bevy_als::als::dynamics::LANDING_BRAKING_FRICTION_WITHOUT_INPUT,params.cmc.braking_substep_time,remaining);
        let request=step_velocity*remaining;
        let mut end=start;
        let mut feedback=step_velocity;
        if let Some((advanced,remainder,next_y))=c560_grounded_movement_transaction(&spatial,capsule,start.translation,
            start.rotation,half,radius,request,&params.cmc,LocomotionState::Grounded,actor,&candidates,&permissions) {
            end.translation=advanced+remainder.unwrap_or(request);
            if let Some(y)=next_y && y>start.translation.y {end.translation.y=y;}
            assert!(c560_adjust_walking_floor_height(&spatial,capsule,&mut end,half,radius,&params.cmc,actor,&candidates));
            feedback=(end.translation-start.translation).with_y(0.0)/remaining;
        }
        variants.insert(label.to_owned(),serde_json::json!({"walking_input":velocity,
            "requested_velocity":step_velocity,"end":end.translation,"feedback":feedback}));
    }
    c516_write("walking-handoff-replay.json",serde_json::json!({"landing":start.translation,
        "remaining":remaining,"variants":variants}));
}

fn c560_handoff(
    probe:Res<C458Probe>,
    control:Res<C516Control>,
    mut actors:Query<(&mut Position,&mut Transform,&mut AlsCmcAdapterRuntime,&LocomotionState,&TnuaMotor)>,
) {
    if probe.frame!=control.frame {return;}
    let (mut position,mut transform,mut runtime,mode,motor)=actors.get_mut(probe.actor.unwrap()).unwrap();
    assert_eq!(*mode,LocomotionState::Grounded);
    let replay=c560_read("walking-handoff-replay.json");
    let original=&replay["variants"]["incoming"];
    let alternate=&replay["variants"]["deflected"];
    let original_position=position.0;
    let original_feedback=runtime.grounded_collision_velocity.unwrap();
    assert_eq!(position.0,transform.translation);
    assert_eq!(original_position,c560_vec(&original["end"]));
    assert_eq!(original_feedback,c560_vec(&original["feedback"]));
    let enabled=std::env::var_os("ALS_C560_CORRECT").is_some();
    if enabled {
        position.0=c560_vec(&alternate["end"]);
        transform.translation=position.0;
        runtime.grounded_collision_velocity=Some(c560_vec(&alternate["feedback"]));
    }
    c516_write("handoff-control.json",serde_json::json!({"enabled":enabled,"original_position":original_position,
        "original_feedback":original_feedback,"selected_position":position.0,
        "selected_feedback":runtime.grounded_collision_velocity,"motor_boost":motor.lin.boost,
        "motor_acceleration":motor.lin.acceleration}));
}
