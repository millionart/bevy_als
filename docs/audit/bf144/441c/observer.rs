#[derive(Resource)]
struct C441Probe {
    actor: Option<Entity>,
    frame: usize,
    rows: Vec<serde_json::Value>,
    progress: Option<(bool, usize, usize)>,
}
impl Default for C441Probe {
    fn default() -> Self { Self { actor: None, frame: 0, rows: Vec::new(), progress: None } }
}
fn c441_progress(value: serde_json::Value) {
    let output = std::env::var("ALS_C441_OUTPUT").expect("parameter evidence output is required");
    std::fs::write(std::path::Path::new(&output).with_extension("progress.json"), value.to_string()).unwrap();
}
fn c441_drive(
    mut commands: Commands,
    mut probe: ResMut<C441Probe>,
    ready: Res<ManualPlaygroundStartupReady>,
    admissions: Query<&AlsPlayerAdmissionComplete>,
    graphs: Res<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    mut actors: Query<(&mut bevy_als::als::gait::AlsDesiredGait, &mut bevy_als::als::transition::AlsDesiredStance, &mut bevy_als::als::motion::AlsViewYaw, &mut bevy_als::als::state::RotationMode)>,
) {
    let progress = (ready.passed, admissions.iter().count(), graphs.len());
    if probe.progress != Some(progress) { info!("[C441] startup_progress={progress:?}"); probe.progress = Some(progress); c441_progress(serde_json::json!({"startup":progress,"captured":probe.frame})); }
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
fn c441_sample(
    mut probe: ResMut<C441Probe>,
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
        &bevy_als::als::gait::AlsMaxAllowedGait,
    )>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(actor) = probe.actor else { return; };
    if probe.frame >= 240 { return; }
    let Ok((locomotion, stance, mode, view, gait, dynamics, base, cmc, handle, controller, position, physical, braking, allowed)) = actors.get(actor) else { return; };
    let Some(params) = assets.get(handle.id()) else { return; };
    let thresholds = params.gait_speed_thresholds(*stance, *mode, base.relative_velocity, view.0);
    let expected = params.grounded_dynamics(*stance, *mode, gait.0);
    let maximum = bevy_als::als::dynamics::max_allowed_speed(params, *stance, *mode, allowed.0, base.relative_velocity, view.0);
    let public_gait = bevy_als::als::gait::calculate_gait_amount_for_thresholds(base.relative_velocity.length(), thresholds);
    let horizontal_gait = bevy_als::als::gait::calculate_gait_amount_for_thresholds(base.relative_velocity.with_y(0.0).length(), thresholds);
    let row = serde_json::json!({
        "ordinal": probe.frame, "delta": time.delta_secs(), "simulation_delta": cmc.simulation_delta_seconds,
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
    if probe.frame % 30 == 0 { info!("[C441] captured_parameters={}",probe.frame); c441_progress(serde_json::json!({"captured":probe.frame})); }
    if probe.frame == 240 {
        let output = std::env::var("ALS_C441_OUTPUT").expect("parameter evidence output is required");
        let document = serde_json::json!({"gate":"CAPTURED_ORIGINAL_PLAYGROUND_240_MOVEMENT_PARAMETERS","original_scene_and_camera":true,"rows":probe.rows,"screenshots":0,"performance_acceptance":"paused"});
        std::fs::write(output, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
        exit.write(AppExit::Success);
    }
}
