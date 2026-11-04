
fn c552_stop(
    probe:Res<C458Probe>,
    control:Res<C516Control>,
    mut actors:Query<(
        &mut Position,&mut Transform,&mut AlsCmcAdapterRuntime,&mut AlsMovementBaseDynamicsState,
        &TnuaMotor,&LinearVelocity,
    )>,
) {
    if probe.frame!=control.frame {return;}
    let (mut position,mut transform,mut runtime,mut base,motor,raw)=actors.get_mut(probe.actor.unwrap()).unwrap();
    let output=std::path::PathBuf::from(std::env::var("ALS_C458_OUTPUT").unwrap());
    let substeps:serde_json::Value=serde_json::from_slice(&std::fs::read(output.with_file_name("stall-substeps.json")).unwrap()).unwrap();
    let one=&substeps["rows"][0];
    let two=&substeps["rows"][1];
    let vector=|value:&serde_json::Value| -> Vec3 {serde_json::from_value(value.clone()).unwrap()};
    let step_one_end=vector(&one["end"]);
    assert_eq!(vector(&one["start"]),step_one_end);
    assert_eq!(position.0,transform.translation);
    assert_eq!(position.0,vector(&two["end"]));
    let step_one_input=vector(&one["input_velocity"]);
    let original_position=position.0;
    let original_velocity=runtime.grounded_collision_velocity;
    let original_pre=base.pre_collision_velocity;
    let enabled=std::env::var_os("ALS_C552_STOP").is_some();
    if enabled {
        position.0=step_one_end;
        transform.translation=step_one_end;
        runtime.grounded_collision_velocity=Some(Vec3::ZERO);
        base.pre_collision_velocity=Some(step_one_input);
    }
    c516_write("stop-handoff.json",serde_json::json!({
        "enabled":enabled,"original_position":original_position,"selected_position":position.0,
        "original_feedback":original_velocity,"selected_feedback":runtime.grounded_collision_velocity,
        "original_pre_collision":original_pre,"selected_pre_collision":base.pre_collision_velocity,
        "motor_boost":motor.lin.boost,"motor_acceleration":motor.lin.acceleration,"raw_velocity":raw.0,
    }));
}
