fn c446g_physics_motor(
    mut probe:ResMut<C446GProbe>,
    frame:Res<bevy::diagnostic::FrameCount>,
    admissions:Query<&AlsPlayerAdmissionComplete>,
    actors:Query<(&Position,&LinearVelocity,&bevy_tnua::TnuaMotor,Option<&bevy_tnua::TnuaGravity>,Option<&GravityScale>,
        &bevy_tnua::TnuaController<bevy_als::als::control_scheme::AlsControlScheme>,&bevy_als::als::character::AlsCmcAdapterRuntime,
        Option<&bevy_tnua::TnuaRigidBodyTracker>,Option<&bevy_tnua::TnuaSensorsEntities<bevy_als::als::control_scheme::AlsControlScheme>>,Has<Sleeping>,Has<RigidBodyDisabled>,Option<&bevy_tnua::TnuaToggle>,&RigidBody)>,
    sensors:Query<&bevy_tnua::TnuaProximitySensor>,
) {
    let Some(actor)=probe.actor.or_else(||admissions.iter().next().map(|v|v.character))else{return;};
    let Ok((position,velocity,motor,gravity,scale,controller,cmc,tracker,entities,sleeping,disabled,toggle,rigid_body))=actors.get(actor)else{return;};
    let walk=controller.basis_access().ok().map(|v|serde_json::json!({"float_height":v.config.float_height,"spring_strength":v.config.spring_strength,"spring_dampening":v.config.spring_dampening,"standing_offset":v.memory.standing_offset.to_array(),"standing_on":v.memory.standing_on_entity().map(|e|format!("{e:?}"))}));
    let ground=entities.and_then(|v|v.ground).and_then(|e|sensors.get(e).ok()).map(|s|serde_json::json!({"cast_origin":s.cast_origin.to_array(),"range":s.cast_range,"output":s.output.as_ref().map(|v|serde_json::json!({"distance":v.proximity,"normal":v.normal.to_array(),"entity_linvel":v.entity_linvel.to_array(),"entity":format!("{:?}",v.entity)}))}));
    probe.physics_motor_rows.push(serde_json::json!({"app_frame":frame.0,"position":position.0.to_array(),"velocity":velocity.0.to_array(),"motor_acceleration":motor.lin.acceleration.to_array(),"motor_boost":motor.lin.boost.to_array(),"custom_gravity":gravity.map(|g|g.0.to_array()),"gravity_scale":scale.map(|g|g.0),"tracker":tracker.map(|v|serde_json::json!({"position":v.translation.to_array(),"velocity":v.velocity.to_array(),"gravity":v.gravity.to_array()})),"ground":ground,"walk":walk,"sleeping":sleeping,"disabled":disabled,"toggle":toggle.map(|v|format!("{v:?}")),"rigid_body":format!("{rigid_body:?}"),"planned_delta":cmc.grounded_movement_delta.map(|v|v.to_array()),"move_start":cmc.grounded_move_start.map(|v|v.to_array())}));
}

fn c446g_physics_coefficients(
    mut probe:ResMut<C446GProbe>,
    frame:Res<bevy::diagnostic::FrameCount>,
    admissions:Query<&AlsPlayerAdmissionComplete>,
    actors:Query<(Option<&avian3d::dynamics::integrator::VelocityIntegrationData>,Option<&avian3d::dynamics::solver::solver_body::SolverBody>,Ref<Collider>,Has<Sleeping>,Has<RigidBodyDisabled>,Has<avian3d::prelude::CustomPositionIntegration>)>,
    frame_time:Res<Time<Virtual>>,
    physics:Res<Time<avian3d::prelude::Physics>>,
    substep:Res<Time<avian3d::prelude::Substeps>>,
    count:Res<avian3d::prelude::SubstepCount>,
    gravity:Res<Gravity>,
) {
    let Some(actor)=probe.actor.or_else(||admissions.iter().next().map(|v|v.character))else{return;};
    let Ok((integration,body,collider,sleeping,disabled,custom_position))=actors.get(actor)else{return;};
    probe.physics_coefficient_rows.push(serde_json::json!({"app_frame":frame.0,"integration":integration.map(|v|serde_json::json!({"linear_increment":v.linear_increment.to_array(),"linear_damping_rhs":v.linear_damping_rhs})),"body":body.map(|v|serde_json::json!({"initial_velocity":v.linear_velocity.to_array(),"initial_delta":v.delta_position.to_array()})),"collider_changed":collider.is_changed(),"sleeping":sleeping,"disabled":disabled,"custom_position":custom_position,"frame_delta_ns":frame_time.delta().as_nanos()as u64,"physics_delta_ns":physics.delta().as_nanos()as u64,"substep_delta_ns":substep.delta().as_nanos()as u64,"substeps":count.0,"gravity":gravity.0.to_array()}));
}

fn c446g_physics_raw(
    mut probe:ResMut<C446GProbe>,
    frame:Res<bevy::diagnostic::FrameCount>,
    admissions:Query<&AlsPlayerAdmissionComplete>,
    actors:Query<(&Position,&LinearVelocity,Has<Sleeping>,Has<RigidBodyDisabled>)>,
) {
    let Some(actor)=probe.actor.or_else(||admissions.iter().next().map(|v|v.character))else{return;};
    let Ok((position,velocity,sleeping,disabled))=actors.get(actor)else{return;};
    probe.physics_raw_rows.push(serde_json::json!({"app_frame":frame.0,"position":position.0.to_array(),"velocity":velocity.0.to_array(),"sleeping":sleeping,"disabled":disabled}));
}
