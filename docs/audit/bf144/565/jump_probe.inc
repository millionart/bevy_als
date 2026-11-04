
fn c523_install(app:&mut App) {
    app.add_systems(First,c565_switch);
    app.edit_schedule(Update,|schedule| {
        let transaction={
            let matches:Vec<_>=schedule.graph().systems.iter().filter(|(_,system,_)|
                system.name().to_string().ends_with("::cmc_movement_transaction_system")).collect();
            assert_eq!(matches.len(),1);
            matches[0].1.default_system_sets()[0]
        };
        schedule.add_systems(c565_motor.after(c516_before).before(transaction));
    });
}

fn c565_switch(
    probe:Res<C458Probe>,
    control:Res<C516Control>,
    actors:Query<(&AlsMovementBase,&AlsMovementBaseDynamicsState,&LocomotionState)>,
    mut bodies:Query<(&Name,&mut Position,&mut Transform,&mut LinearVelocity)>,
    names:Query<&Name>,
) {
    if probe.frame!=control.frame {return;}
    let (base,dynamics,mode)=actors.get(probe.actor.unwrap()).unwrap();
    let base_name=base.entity.and_then(|entity|names.get(entity).ok()).map(|name|name.as_str());
    let dynamics_name=dynamics.entity.and_then(|entity|names.get(entity).ok()).map(|name|name.as_str());
    let switched=std::env::var("ALS_C565_SWITCH").unwrap()=="1";
    let mut platforms=serde_json::Map::new();
    for (name,mut position,mut transform,mut velocity) in &mut bodies {
        match name.as_str() {
            "AuditWalkOffPlatform"=>{velocity.0=Vec3::X;}
            "AuditJumpBaseNew"=>{
                if switched {position.0.y=1.905; transform.translation.y=1.905;}
                velocity.0=Vec3::NEG_X;
            }
            _=>continue,
        }
        platforms.insert(name.to_string(),serde_json::json!({"position":position.0,"velocity":velocity.0}));
    }
    assert_eq!(platforms.len(),2);
    c516_write("jump-base-before.json",serde_json::json!({"switched":switched,"mode":mode,"base":base_name,
        "dynamics_base":dynamics_name,"previous_support":dynamics.support_velocity,"platforms":platforms}));
}

fn c565_motor(
    probe:Res<C458Probe>,
    control:Res<C516Control>,
    actors:Query<(&Position,&LinearVelocity,&LocomotionState,&TnuaMotor,&TnuaController<AlsControlScheme>,
        &bevy_tnua::TnuaSensorsEntities<AlsControlScheme>)>,
    sensors:Query<&bevy_tnua::TnuaProximitySensor>,
    names:Query<&Name>,
) {
    if probe.frame!=control.frame {return;}
    let (position,velocity,mode,motor,controller,sensor_entities)=actors.get(probe.actor.unwrap()).unwrap();
    let sensor=sensors.get(sensor_entities.ground.unwrap()).unwrap();
    let name=|entity|names.get(entity).ok().map(|name|name.as_str());
    c516_write("jump-motor.json",serde_json::json!({"position":position.0,"mode":mode,
        "velocity":velocity.0,"boost":motor.lin.boost,"acceleration":motor.lin.acceleration,
        "memory":controller.basis_memory.standing_on_entity().and_then(name),
        "sensor":sensor.output.as_ref().map(|hit|serde_json::json!({"entity":name(hit.entity),
            "velocity":hit.entity_linvel,"proximity":hit.proximity}))}));
}
