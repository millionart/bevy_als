fn c536_trace(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    spatial: SpatialQuery,
    assets: Res<Assets<AlsMovementParams>>,
    actors: Query<(&Position,&Transform,&Collider,&LocomotionState,&AlsMovementParamsHandle)>,
    candidates: Query<(Option<&CollisionLayers>,Has<Sensor>,Option<&ColliderOf>,Option<&AlsCollisionLayerOwnership>)>,
    permissions: Query<&bevy_als::als::mantle::AlsMantleTargetPermission>,
    names: Query<&Name>,
) {
    if probe.frame!=control.frame {return;}
    let actor=probe.actor.unwrap();
    let (position,transform,capsule,mode,handle)=actors.get(actor).unwrap();
    let params=assets.get(handle.id()).unwrap();
    let output=std::path::PathBuf::from(std::env::var("ALS_C458_OUTPUT").unwrap());
    let stage:serde_json::Value=serde_json::from_slice(&std::fs::read(output.with_file_name("stage-before.json")).unwrap()).unwrap();
    let delta:Vec3=serde_json::from_value(stage["planned_move"].clone()).unwrap();
    let mut cases=serde_json::Map::new();
    for skip in [false,true] {
        let mut events=Vec::new();
        let result=c536_step(&spatial,capsule,position.0,transform.rotation,params.crouch.standing_capsule_half_height,
            params.crouch.standing_capsule_radius,delta.with_y(0.0),&params.cmc,*mode,actor,&candidates,&permissions,skip,&mut events,&names);
        cases.insert(if skip {"no-floor-guard"} else {"actual"}.to_string(),serde_json::json!({
            "events":events,"result":result.map(|(advanced,remaining,next_y)|
                serde_json::json!({"advanced":advanced,"remaining":remaining,"next_y":next_y}))
        }));
    }
    c516_write("step-down-replay.json",serde_json::json!({
        "start":position.0,"requested":delta,"variants":cases
    }));
}
