#[derive(Resource)]
struct C443CProbe {
    actor: Option<Entity>,
    frame: usize,
    rows: Vec<serde_json::Value>,
    progress: Option<(bool, usize, usize)>,
    camera_rows: Vec<serde_json::Value>,
    motor_rows: Vec<serde_json::Value>,
    raw_physics_rows: Vec<serde_json::Value>,
    coefficient_rows: Vec<serde_json::Value>,
}
impl Default for C443CProbe {
    fn default() -> Self { Self { actor: None, frame: 0, rows: Vec::new(), progress: None, camera_rows:Vec::new(),motor_rows:Vec::new(),raw_physics_rows:Vec::new(),coefficient_rows:Vec::new() } }
}
fn c443c_progress(value: serde_json::Value) {
    let output = std::env::var("ALS_C443C_OUTPUT").expect("parameter evidence output is required");
    std::fs::write(std::path::Path::new(&output).with_extension("progress.json"), value.to_string()).unwrap();
}
fn c443c_drive(
    mut commands: Commands,
    mut probe: ResMut<C443CProbe>,
    ready: Res<ManualPlaygroundStartupReady>,
    admissions: Query<&AlsPlayerAdmissionComplete>,
    graphs: Res<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    mut actors: Query<(&mut bevy_als::als::gait::AlsDesiredGait, &mut bevy_als::als::transition::AlsDesiredStance, &mut bevy_als::als::motion::AlsViewYaw, &mut bevy_als::als::state::RotationMode)>,
) {
    let progress = (ready.passed, admissions.iter().count(), graphs.len());
    if probe.progress != Some(progress) { info!("[C443C] startup_progress={progress:?}"); probe.progress = Some(progress); c443c_progress(serde_json::json!({"startup":progress,"captured":probe.frame})); }
    if !ready.passed || probe.frame >= 240 { return; }
    let Some(admission) = admissions.iter().next() else { return; };
    let Ok((mut gait, mut stance, mut view, mut mode)) = actors.get_mut(admission.character) else { return; };
    probe.actor = Some(admission.character);
    gait.0 = bevy_als::als::state::Gait::Running;
    stance.0 = bevy_als::als::state::Stance::Standing;
    view.0 = -std::f32::consts::FRAC_PI_2;
    *mode = bevy_als::als::state::RotationMode::ViewDirection;
    let moving = (20..60).contains(&probe.frame) || (120..160).contains(&probe.frame);
    commands.entity(admission.character).insert(bevy_als::als::motion::AlsInputOverride(bevy_als::als::input::AlsInputCommand { move_dir: if moving { Vec2::Y } else { Vec2::ZERO }, ..default() }));
}
fn c443c_sample(
    mut probe: ResMut<C443CProbe>,
    time: Res<Time>,
    assets: Res<Assets<AlsMovementParams>>,
    spatial: SpatialQuery,
    shapes: Query<(&Collider,&GlobalTransform)>,
    candidates: Query<(Option<&CollisionLayers>,Has<Sensor>,Option<&ColliderOf>,Option<&bevy_als::als::character::AlsCollisionLayerOwnership>)>,
    actors: Query<(
        &bevy_als::als::state::LocomotionState,
        &bevy_als::als::state::Stance,
        &bevy_als::als::state::RotationMode,
        &bevy_als::als::motion::AlsViewYaw,
        &bevy_als::als::gait::AlsGaitAmount,
        &bevy_als::als::dynamics::AlsGroundedDynamics,
        &bevy_als::als::dynamics::AlsMovementBaseDynamicsState,
        &bevy_als::als::character::AlsCmcAdapterRuntime,
        &bevy_als::als::character::AlsMovementParamsHandle,
        &bevy_tnua::TnuaController<bevy_als::als::control_scheme::AlsControlScheme>,
        &Position,
        &LinearVelocity,
        &bevy_als::als::dynamics::AlsLandingBrakingFriction,
        (&bevy_als::als::gait::AlsMaxAllowedGait, Option<&bevy_als::als::view::AlsPlayerViewYaw>, Option<&bevy_als::als::motion::AlsInputOverride>, &bevy_als::als::input::AlsInputCommand, Has<bevy_als::als::motion::AlsMovementInputBlocked>),
    )>,
    views: Query<(Entity,&Camera,Ref<GlobalTransform>,&bevy_als::als_camera::AlsCharacterCamera),(With<Camera3d>,With<AlsCameraViewSource>)>,
) {
    let Some(actor) = probe.actor else { return; };
    if probe.frame >= 240 { return; }
    let Ok((locomotion, stance, mode, view, gait, dynamics, base, cmc, handle, controller, position, physical, braking, (allowed, player_view, override_command, command, input_blocked))) = actors.get(actor) else { return; };
    let Some(params) = assets.get(handle.id()) else { return; };
    let thresholds = params.gait_speed_thresholds(*stance, *mode, base.relative_velocity, view.0);
    let expected = params.grounded_dynamics(*stance, *mode, gait.0);
    let maximum = bevy_als::als::dynamics::max_allowed_speed(params, *stance, *mode, allowed.0, base.relative_velocity, view.0);
    let public_gait = bevy_als::als::gait::calculate_gait_amount_for_thresholds(base.relative_velocity.length(), thresholds);
    let horizontal_gait = bevy_als::als::gait::calculate_gait_amount_for_thresholds(base.relative_velocity.with_y(0.0).length(), thresholds);
    let effective = bevy_als::als::motion::movement_input_for_action(*override_command.map_or(command, |v|&v.0), input_blocked);
    let used_yaw = player_view.map_or(view.0, |v|v.0);
    let expected_direction = bevy_als::als::motion::view_relative_move_direction(effective.move_dir, used_yaw)*effective.move_dir.length().min(1.0);
    let raw_direction = bevy_als::als::motion::view_relative_move_direction(effective.move_dir, view.0)*effective.move_dir.length().min(1.0);
    let camera = views.iter().find(|(_,cam,t,owner)|owner.character==actor && cam.is_active && !t.is_added()).map(|(id,_,t,_)|serde_json::json!({"entity":format!("{id:?}"),"yaw":t.rotation().to_euler(EulerRot::YXZ).0,"rotation":t.rotation().to_array()}));
    let (capsule,body_world)=shapes.get(actor).expect("Observed capsule is required");
    let radius=params.crouch.standing_capsule_radius*body_world.to_scale_rotation_translation().0.x.min(body_world.to_scale_rotation_translation().0.z);
    let delta=cmc.grounded_movement_delta.unwrap_or(Vec3::ZERO).with_y(0.0);
    let filter=SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts=|candidate|bevy_als::als::motion::character_blocks_collider(actor,candidate,&candidates);
    let capture=|h:ShapeHitData|serde_json::json!({"entity":format!("{:?}",h.entity),"distance":h.distance,"point1":h.point1.to_array(),"point2":h.point2.to_array(),"normal1":h.normal1.to_array(),"normal2":h.normal2.to_array()});
    let mut floor_probe=serde_json::json!({"eligible":false,"delta":delta.to_array(),"scale":body_world.to_scale_rotation_translation().0.to_array(),"radius":radius});
    if let Ok((direction,distance))=Dir3::new_and_length(delta) {
        let forward=spatial.cast_shape_predicate(capsule,position.0,Quat::IDENTITY,direction,
            &ShapeCastConfig {ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(distance)},&filter,&accepts);
        floor_probe["forward"]=forward.map_or(serde_json::Value::Null,capture);
        if forward.is_none() {
            let destination=position.0+delta;
            let hit=spatial.cast_shape_predicate(capsule,destination,Quat::IDENTITY,Dir3::NEG_Y,
                &ShapeCastConfig::from_max_distance(0.45+0.024*2.0),&filter,&accepts);
            floor_probe["eligible"]=serde_json::json!(true);
            floor_probe["destination"]=serde_json::json!(destination.to_array());
            floor_probe["floor"]=hit.map_or(serde_json::Value::Null,capture);
            if let Some(hit)=hit {
                let reduced=(radius-0.0015).max(0.001501);
                floor_probe["edge_valid"]=serde_json::json!(Vec2::new(hit.point1.x-destination.x,hit.point1.z-destination.z).length_squared()<reduced*reduced);
                floor_probe["predicted_y"]=serde_json::json!(destination.y-hit.distance+(bevy_als::als::character::ALS_CHARACTER_FLOOR_CLEARANCE+bevy_als::als::character::ALS_CHARACTER_TNUA_FLOAT_EPSILON));
            }
        }
    }
    let row = serde_json::json!({
        "ordinal": probe.frame,"floor_probe":floor_probe, "player_view_yaw":player_view.map(|v|v.0), "consumed_yaw":used_yaw, "input_move":effective.move_dir.to_array(), "input_blocked":input_blocked, "camera_at_update":camera, "expected_direction":expected_direction.to_array(), "raw_control_direction":raw_direction.to_array(), "delta": time.delta_secs(), "simulation_delta": cmc.simulation_delta_seconds,
        "simulation_steps": cmc.simulation_substeps, "locomotion": format!("{locomotion:?}"), "stance": format!("{stance:?}"), "rotation_mode": format!("{mode:?}"),
        "view_yaw": view.0, "position": position.0.to_array(), "physical_velocity": physical.0.to_array(), "world_velocity_at_dynamics": base.world_velocity.to_array(),
        "support_velocity": base.support_velocity.to_array(), "has_base": base.entity.is_some(), "relative_velocity_consumed": base.relative_velocity.to_array(),
        "desired_motion": controller.basis.desired_motion.to_array(), "gait_amount": gait.0, "public_gait":public_gait, "horizontal_gait":horizontal_gait, "allowed_gait":format!("{:?}",allowed.0), "thresholds": [thresholds.walk,thresholds.run,thresholds.sprint],
        "dynamics": [dynamics.0.acceleration,dynamics.0.braking_deceleration,dynamics.0.ground_friction],
        "public_sample": [expected.acceleration,expected.braking_deceleration,expected.ground_friction], "max_speed": maximum,
        "planned_velocity": base.pre_collision_velocity.map(|v|v.to_array()), "planned_delta": cmc.grounded_movement_delta.map(|v|v.to_array()),"grounded_move_start":cmc.grounded_move_start.map(|v|v.to_array()),
        "braking_friction_factor": braking.factor, "has_animation_root_velocity": cmc.has_animation_root_velocity,
    });
    probe.rows.push(row);
    probe.frame += 1;
    if probe.frame % 30 == 0 { info!("[C443C] captured_parameters={}",probe.frame); c443c_progress(serde_json::json!({"captured":probe.frame})); }
}
fn c443c_late(
    mut probe: ResMut<C443CProbe>,
    bodies: Query<(&Position,&LinearVelocity,&avian3d::prelude::ComputedCenterOfMass),With<bevy_als::als::character::AlsCharacter>>,
    frame_time: Res<Time<Virtual>>,
    physics: Res<Time<avian3d::prelude::Physics>>,
    substep_time: Res<Time<avian3d::prelude::Substeps>>,
    substeps: Res<avian3d::prelude::SubstepCount>,
    gravity: Res<avian3d::prelude::Gravity>,
    views: Query<(&Camera,&GlobalTransform,&bevy_als::als_camera::AlsCharacterCamera),(With<Camera3d>,With<AlsCameraViewSource>)>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(actor)=probe.actor else{return;};
    if probe.frame==0{return;}
    let camera=views.iter().find(|(cam,_,owner)|owner.character==actor && cam.is_active).map(|(_,t,_)|serde_json::json!({"yaw":t.rotation().to_euler(EulerRot::YXZ).0,"rotation":t.rotation().to_array()}));
    let ordinal=probe.frame-1;
    let Ok((position,velocity,com))=bodies.get(actor) else{panic!("Observed body is missing");};
    probe.camera_rows.push(serde_json::json!({"ordinal":ordinal,"camera":camera,"position":position.0.to_array(),"velocity":velocity.0.to_array(),"physics_delta":physics.delta_secs(),"physics_delta_ns":physics.delta().as_nanos() as u64,"frame_delta_ns":frame_time.delta().as_nanos() as u64,"substep_delta_ns":substep_time.delta().as_nanos() as u64,"center_of_mass":com.0.to_array(),"substep_delta":substep_time.delta_secs(),"substeps":substeps.0,"gravity":gravity.0.to_array()}));
    if probe.frame==240 {
        let output=std::env::var("ALS_C443C_OUTPUT").expect("input direction evidence output is required");
        std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({"gate":"CAPTURED_REAL_INPUT_CAMERA_AND_DESIRED_DIRECTION240","rows":probe.rows,"after_motors":probe.motor_rows,"before_floor_sync":probe.raw_physics_rows,"integration_coefficients":probe.coefficient_rows,"camera_after_post_update":probe.camera_rows,"screenshots":0,"performance_acceptance":"paused"})).unwrap()).unwrap();
        exit.write(AppExit::Success);
    }
}

fn c443c_motor(
    mut probe: ResMut<C443CProbe>,
    bodies: Query<(&Position,&LinearVelocity,&bevy_tnua::TnuaMotor,Option<&bevy_tnua::TnuaGravity>,Option<&GravityScale>),With<bevy_als::als::character::AlsCharacter>>,
) {
    let Some(actor)=probe.actor else{return;};
    if probe.frame==0{return;}
    let Ok((position,velocity,motor,gravity,gravity_scale))=bodies.get(actor) else{panic!("Observed motor body is missing");};
    let ordinal=probe.frame-1;
    probe.motor_rows.push(serde_json::json!({"ordinal":ordinal,"motor_acceleration":motor.lin.acceleration.to_array(),"tnua_gravity":gravity.map_or(Vec3::ZERO,|g|g.0).to_array(),"gravity_scale":gravity_scale.map_or(1.0,|g|g.0),"custom_gravity_present":gravity.is_some(),"gravity_scale_present":gravity_scale.is_some(),"position":position.0.to_array(),"velocity":velocity.0.to_array()}));
}

fn c443c_coefficients(
    mut probe: ResMut<C443CProbe>,
    bodies: Query<(&avian3d::dynamics::integrator::VelocityIntegrationData,&avian3d::dynamics::solver::solver_body::SolverBody,Ref<Collider>)>,
) {
    let Some(actor)=probe.actor else{return;};
    if probe.frame==0{return;}
    let (integration,body,collider)=bodies.get(actor).expect("Observed solver body is required");
    let ordinal=probe.frame-1;
    probe.coefficient_rows.push(serde_json::json!({"ordinal":ordinal,"linear_increment":integration.linear_increment.to_array(),
        "linear_damping_rhs":integration.linear_damping_rhs,"initial_velocity":body.linear_velocity.to_array(),"initial_delta":body.delta_position.to_array(),"collider_changed":collider.is_changed()}));
}

fn c443c_raw_physics(
    mut probe: ResMut<C443CProbe>,
    spatial: SpatialQuery,
    transforms: bevy::transform::helper::TransformHelper,
    assets: Res<Assets<AlsMovementParams>>,
    bodies: Query<(&Position,&LinearVelocity,&Collider,&GlobalTransform,&bevy_als::als::character::AlsMovementParamsHandle,&Transform,Has<ChildOf>)>,
    candidates: Query<(Option<&CollisionLayers>,Has<Sensor>,Option<&ColliderOf>,Option<&bevy_als::als::character::AlsCollisionLayerOwnership>)>,
) {
    let Some(actor)=probe.actor else{return;};
    if probe.frame==0{return;}
    let (position,velocity,capsule,world,handle,local,parented)=bodies.get(actor).expect("Observed raw physics body is required");
    let params=assets.get(handle.id()).expect("Observed movement parameters are required");
    let cached_scale=world.to_scale_rotation_translation().0;
    let current_world=transforms.compute_global_transform(actor).expect("Current body transform is required");
    let scale=current_world.to_scale_rotation_translation().0;
    let half=params.crouch.standing_capsule_half_height*scale.y;
    let radius=params.crouch.standing_capsule_radius*scale.x.min(scale.z);
    let filter=SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts=|candidate|bevy_als::als::motion::character_blocks_collider(actor,candidate,&candidates);
    let floor=bevy_als::als::motion::find_ground_floor(&spatial,position.0,half,radius,&params.cmc,
        bevy_als::als::state::LocomotionState::Grounded,&filter,actor,&candidates);
    let mut correction=serde_json::Value::Null;
    if let Some((center,impact,entity))=floor {
        let distance=position.y-center;
        let next=if (0.019..=0.024).contains(&distance){position.y}else{center+(0.019+0.024)*0.5};
        let displacement=Vec3::Y*(next-position.y);
        let (direction,length)=Dir3::new_and_length(displacement).unwrap_or((Dir3::NEG_Y,0.0));
        let hit=spatial.cast_shape_predicate(capsule,position.0,current_world.rotation(),direction,
            &ShapeCastConfig{ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(length)},&filter,&accepts);
        correction=serde_json::json!({"center":center,"impact_y":impact,"entity":format!("{entity:?}"),"distance":distance,"target_y":next,"scale":scale.to_array(),"cached_scale":cached_scale.to_array(),"half":half,
            "adjustment_hit":hit.map(|h|serde_json::json!({"distance":h.distance,"normal":h.normal1.to_array(),"point":h.point1.to_array()}))});
    }
    let source_half=params.crouch.standing_capsule_half_height*local.scale.y;
    let source_radius=params.crouch.standing_capsule_radius*local.scale.x.min(local.scale.z);
    let source_floor=bevy_als::als::motion::find_ground_floor(&spatial,position.0,source_half,source_radius,&params.cmc,
        bevy_als::als::state::LocomotionState::Grounded,&filter,actor,&candidates);
    let source_floor=source_floor.map(|(center,impact,entity)| {
        let distance=position.y-center;
        let target=if (0.019..=0.024).contains(&distance){position.y}else{center+(0.019+0.024)*0.5};
        let (direction,length)=Dir3::new_and_length(Vec3::Y*(target-position.y)).unwrap_or((Dir3::NEG_Y,0.0));
        let hit=spatial.cast_shape_predicate(capsule,position.0,current_world.rotation(),direction,
            &ShapeCastConfig{ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(length)},&filter,&accepts);
        serde_json::json!({"center":center,"impact_y":impact,"distance":distance,"entity":format!("{entity:?}"),"target_y":target,"unblocked_final_y":(position.0+*direction*length).y,"adjustment_blocked":hit.is_some()})
    });
    let scale_inputs=serde_json::json!({"parented":parented,"local_scale":local.scale.to_array(),"local_rotation":local.rotation.to_array(),"local_translation":local.translation.to_array(),
        "actual_scale":scale.to_array(),"cached_scale":cached_scale.to_array(),"collider_scale":capsule.scale().to_array(),
        "base_half":params.crouch.standing_capsule_half_height,"base_radius":params.crouch.standing_capsule_radius,
        "actual_half":half,"actual_radius":radius,"source_scale_half":source_half,"source_scale_radius":source_radius,"source_scale_floor":source_floor});
    let ordinal=probe.frame-1;
    probe.raw_physics_rows.push(serde_json::json!({"ordinal":ordinal,"position":position.0.to_array(),"velocity":velocity.0.to_array(),"floor":correction,"scale_inputs":scale_inputs}));
}
