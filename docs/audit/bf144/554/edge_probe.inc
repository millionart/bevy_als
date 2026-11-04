
fn c554_before(
    probe:Res<C458Probe>,
    spatial:SpatialQuery,
    actors:Query<(&Position,&Transform,&Collider,&LinearVelocity,&TnuaMotor,&AlsCmcAdapterRuntime,
        &AlsMovementBase,&AlsMovementParamsHandle,&TnuaController<AlsControlScheme>)>,
    assets:Res<Assets<AlsMovementParams>>,
    candidates:Query<(Option<&CollisionLayers>,Has<Sensor>,Option<&ColliderOf>,Option<&AlsCollisionLayerOwnership>)>,
    permissions:Query<&bevy_als::als::mantle::AlsMantleTargetPermission>,
    names:Query<&Name>,
) {
    let frame=probe.frame;
    if !matches!(frame,47|48) {return;}
    let actor=probe.actor.unwrap();
    let (position,transform,capsule,velocity,motor,runtime,base,handle,controller)=actors.get(actor).unwrap();
    let params=assets.get(handle.id()).unwrap();
    let planned=runtime.grounded_movement_delta.unwrap().with_y(0.0);
    let basis=controller.basis_access().unwrap();
    let mut events=Vec::new();
    let result=c536_step(&spatial,capsule,position.0,transform.rotation,params.crouch.standing_capsule_half_height,
        params.crouch.standing_capsule_radius,planned,&params.cmc,LocomotionState::Grounded,actor,
        &candidates,&permissions,false,&mut events,&names);
    c516_write(&format!("edge-before-{frame}.json"),serde_json::json!({
        "ordinal":frame,"position":position.0,"velocity":velocity.0,
        "boost":motor.lin.boost,"acceleration":motor.lin.acceleration,
        "base":base.entity.and_then(|entity|c529_name(entity,&names)),
        "sensor_support":controller.basis_memory.standing_on_entity().and_then(|entity|c529_name(entity,&names)),
        "displacement":TnuaBuiltinWalk::displacement(&basis),
        "spring_strength":basis.config.spring_strength,"float_height":basis.config.float_height,
        "planned":planned,"events":events,"next_y":result.and_then(|(_,_,y)|y),
    }));
}

fn c554_after(
    probe:Res<C458Probe>,
    actors:Query<(&Transform,&LinearVelocity,&TnuaMotor)>,
) {
    let frame=probe.frame;
    if !matches!(frame,47|48) {return;}
    let (transform,velocity,motor)=actors.get(probe.actor.unwrap()).unwrap();
    c516_write(&format!("edge-after-{frame}.json"),serde_json::json!({
        "ordinal":frame,"position":transform.translation,"velocity":velocity.0,
        "boost":motor.lin.boost,"acceleration":motor.lin.acceleration,
    }));
}
