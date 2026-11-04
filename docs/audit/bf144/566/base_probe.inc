
fn c566_enable(
    probe:Res<C458Probe>,
    control:Res<C516Control>,
    actors:Query<&AlsMovementBase>,
    mut bodies:Query<(&Name,&Position,&mut LinearVelocity)>,
    names:Query<&Name>,
) {
    if probe.frame!=control.frame {return;}
    let actor=probe.actor.unwrap();
    let base=actors.get(actor).unwrap();
    let base=base.entity.and_then(|entity|names.get(entity).ok()).map(|name|name.as_str());
    let moving=std::env::var("ALS_C566_MOVING").unwrap()=="1";
    let (_,position,mut velocity)=bodies.iter_mut().find(|(name,_,_)|name.as_str()=="AuditWalkOffSecondBase").unwrap();
    velocity.0=if moving {Vec3::Z} else {Vec3::ZERO};
    c516_write("second-base.json",serde_json::json!({"old":base,"position":position.0,"velocity":velocity.0}));
}
