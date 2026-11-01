#[derive(Resource)]
struct C442C1Probe {
    actor: Option<Entity>,
    frame: usize,
    rows: Vec<serde_json::Value>,
    progress: Option<(bool, usize, usize)>,
    camera_rows: Vec<serde_json::Value>,
    cold_rows: Vec<serde_json::Value>,
}
impl Default for C442C1Probe {
    fn default() -> Self { Self { actor: None, frame: 0, rows: Vec::new(), progress: None, camera_rows:Vec::new(),cold_rows:Vec::new() } }
}
fn c442c1_progress(value: serde_json::Value) {
    let output = std::env::var("ALS_C442C1_OUTPUT").expect("parameter evidence output is required");
    std::fs::write(std::path::Path::new(&output).with_extension("progress.json"), value.to_string()).unwrap();
}
fn c442c1_drive(
    mut commands: Commands,
    mut probe: ResMut<C442C1Probe>,
    ready: Res<ManualPlaygroundStartupReady>,
    admissions: Query<&AlsPlayerAdmissionComplete>,
    graphs: Res<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    mut actors: Query<(&mut bevy_als::als::gait::AlsDesiredGait, &mut bevy_als::als::transition::AlsDesiredStance, &mut bevy_als::als::motion::AlsViewYaw, &mut bevy_als::als::state::RotationMode)>,
) {
    let progress = (ready.passed, admissions.iter().count(), graphs.len());
    if probe.progress != Some(progress) { info!("[C442C1] startup_progress={progress:?}"); probe.progress = Some(progress); c442c1_progress(serde_json::json!({"startup":progress,"captured":probe.frame})); }
    if !ready.passed || probe.frame >= 240 { return; }
    let Some(admission) = admissions.iter().next() else { return; };
    let Ok((mut gait, mut stance, mut view, mut mode)) = actors.get_mut(admission.character) else { return; };
    probe.actor = Some(admission.character);
    gait.0 = bevy_als::als::state::Gait::Running;
    stance.0 = bevy_als::als::state::Stance::Standing;
    view.0 = -5.0*std::f32::consts::PI/6.0;
    *mode = bevy_als::als::state::RotationMode::ViewDirection;
    let moving = (20..60).contains(&probe.frame) || (120..160).contains(&probe.frame);
    commands.entity(admission.character).insert(bevy_als::als::motion::AlsInputOverride(bevy_als::als::input::AlsInputCommand { move_dir: if moving { Vec2::Y } else { Vec2::ZERO }, ..default() }));
}
fn c442c1_sample(
    mut probe: ResMut<C442C1Probe>,
    time: Res<Time>,
    assets: Res<Assets<AlsMovementParams>>,
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
    actor_transforms: Query<&Transform,With<bevy_als::als::character::AlsCharacter>>,
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
    let row = serde_json::json!({
        "ordinal": probe.frame,"actor_yaw":actor_transforms.get(actor).ok().map(|t|t.rotation.to_euler(EulerRot::YXZ).0), "player_view_yaw":player_view.map(|v|v.0), "consumed_yaw":used_yaw, "input_move":effective.move_dir.to_array(), "input_blocked":input_blocked, "camera_at_update":camera, "expected_direction":expected_direction.to_array(), "raw_control_direction":raw_direction.to_array(), "delta": time.delta_secs(), "simulation_delta": cmc.simulation_delta_seconds,
        "simulation_steps": cmc.simulation_substeps, "locomotion": format!("{locomotion:?}"), "stance": format!("{stance:?}"), "rotation_mode": format!("{mode:?}"),
        "view_yaw": view.0, "position": position.0.to_array(), "physical_velocity": physical.0.to_array(), "world_velocity_at_dynamics": base.world_velocity.to_array(),
        "support_velocity": base.support_velocity.to_array(), "has_base": base.entity.is_some(), "relative_velocity_consumed": base.relative_velocity.to_array(),
        "desired_motion": controller.basis.desired_motion.to_array(), "gait_amount": gait.0, "public_gait":public_gait, "horizontal_gait":horizontal_gait, "allowed_gait":format!("{:?}",allowed.0), "thresholds": [thresholds.walk,thresholds.run,thresholds.sprint],
        "dynamics": [dynamics.0.acceleration,dynamics.0.braking_deceleration,dynamics.0.ground_friction],
        "public_sample": [expected.acceleration,expected.braking_deceleration,expected.ground_friction], "max_speed": maximum,
        "planned_velocity": base.pre_collision_velocity.map(|v|v.to_array()), "planned_delta": cmc.grounded_movement_delta.map(|v|v.to_array()),
        "braking_friction_factor": braking.factor, "has_animation_root_velocity": cmc.has_animation_root_velocity,
    });
    probe.rows.push(row);
    probe.frame += 1;
    if probe.frame % 30 == 0 { info!("[C442C1] captured_parameters={}",probe.frame); c442c1_progress(serde_json::json!({"captured":probe.frame})); }
}
fn c442c1_late(
    mut probe: ResMut<C442C1Probe>,
    views: Query<(&Camera,&GlobalTransform,&bevy_als::als_camera::AlsCharacterCamera),(With<Camera3d>,With<AlsCameraViewSource>)>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(actor)=probe.actor else{return;};
    if probe.frame==0{return;}
    let camera=views.iter().find(|(cam,_,owner)|owner.character==actor && cam.is_active).map(|(_,t,_)|serde_json::json!({"yaw":t.rotation().to_euler(EulerRot::YXZ).0,"rotation":t.rotation().to_array()}));
    let ordinal=probe.frame-1;probe.camera_rows.push(serde_json::json!({"ordinal":ordinal,"camera":camera}));
    if probe.frame==240 {
        let output=std::env::var("ALS_C442C1_OUTPUT").expect("input direction evidence output is required");
        std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({"gate":"CAPTURED_REAL_INPUT_CAMERA_AND_DESIRED_DIRECTION240","rows":probe.rows,"natural_cold":probe.cold_rows,"camera_after_post_update":probe.camera_rows,"screenshots":0,"performance_acceptance":"paused"})).unwrap()).unwrap();
        exit.write(AppExit::Success);
    }
}

fn c442c1_camera(
    probe: Res<C442C1Probe>,
    mut cameras: Query<(&mut Camera,&bevy_als::als_camera::AlsCharacterCamera),(With<Camera3d>,With<AlsCameraViewSource>)>,
) {
    let Some(actor)=probe.actor else{return;};
    for (mut camera,owner) in &mut cameras {
        if owner.character==actor { camera.is_active=!(20..22).contains(&probe.frame); }
    }
}
fn c442c1_cold(
    ready: Res<ManualPlaygroundStartupReady>,
    mut probe: ResMut<C442C1Probe>,
    cameras: Query<(&Camera,Ref<GlobalTransform>,&bevy_als::als_camera::AlsCharacterCamera),(With<Camera3d>,With<AlsCameraViewSource>)>,
    actors: Query<(Entity,&bevy_als::als::motion::AlsViewYaw,Option<&bevy_als::als::view::AlsPlayerViewYaw>,
        Option<&bevy_als::als::input::AlsInputCommand>,Option<&bevy_als::als::motion::AlsInputOverride>,
        Has<bevy_als::als::motion::AlsMovementInputBlocked>,Option<&bevy_tnua::TnuaController<bevy_als::als::control_scheme::AlsControlScheme>>),With<bevy_als::als::character::AlsCharacter>>,
) {
    if ready.passed{return;}
    for (actor,view,player,command,override_command,blocked,controller) in &actors {
        let effective=command.map(|c|bevy_als::als::motion::movement_input_for_action(*override_command.map_or(c,|v|&v.0),blocked));
        probe.cold_rows.push(serde_json::json!({"entity":format!("{actor:?}"),"raw_yaw":view.0,
            "player_yaw":player.map(|v|v.0),"blocked":blocked,"input":effective.map(|v|v.move_dir.to_array()),
            "desired":controller.map(|v|v.basis.desired_motion.to_array()),
            "cameras":cameras.iter().filter(|(_,_,owner)|owner.character==actor).map(|(camera,transform,_)|serde_json::json!({"active":camera.is_active,"added":transform.is_added(),"yaw":transform.rotation().to_euler(EulerRot::YXZ).0})).collect::<Vec<_>>()}));
    }
}
