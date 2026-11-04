fn c533_start(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    mut actors: Query<(&mut Position,&mut Transform,&mut LinearVelocity,&mut Collider,&LocomotionState,&AlsMovementBase)>,
) {
    if probe.frame != control.frame { return; }
    let (mut position,mut transform,mut velocity,mut collider,mode,base)=actors.get_mut(probe.actor.unwrap()).unwrap();
    assert_eq!(*mode,LocomotionState::Grounded);
    assert!(base.entity.is_some());
    position.0=Vec3::new(-13.99,2.271,-15.0);
    transform.translation=position.0;
    transform.scale=Vec3::splat(0.3);
    collider.set_scale(transform.scale,16);
    velocity.0=Vec3::ZERO;
    c516_write("scaled-start.json",serde_json::json!({
        "position":position.0,"scale":transform.scale,"half_extents":collider.shape_scaled().compute_local_aabb().half_extents()
    }));
}
