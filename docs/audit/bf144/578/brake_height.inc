#[derive(Resource, Default)]
struct C578Height {
    start: Vec3,
    replay: Vec3,
}

fn c578_install(app: &mut App) {
    use bevy_als::als::dynamics::grounded_dynamics_motor_system;
    app.init_resource::<C578Height>();
    app.edit_schedule(Update, |schedule| {
        let transaction = {
            let matches: Vec<_> = schedule.graph().systems.iter().filter(|(_, system, _)|
                system.name().to_string().ends_with("::cmc_movement_transaction_system")).collect();
            assert_eq!(matches.len(), 1);
            matches[0].1.default_system_sets()[0]
        };
        schedule.add_systems((
            c578_before.after(grounded_dynamics_motor_system).before(transaction),
            c578_after.after(transaction).before(TnuaPipelineSystems::Motors).before(c458_sample),
        ));
    });
    app.add_systems(PostUpdate, c578_physics.after(PhysicsSystems::Last).before(AlsActualVelocitySet));
}

fn c578_before(
    probe: Res<C458Probe>,
    mut state: ResMut<C578Height>,
    spatial: SpatialQuery,
    assets: Res<Assets<AlsMovementParams>>,
    actors: Query<(&Position, &Collider, &LocomotionState, &AlsGroundedDynamics, &AlsMovementBaseDynamicsState,
        &AlsCmcAdapterRuntime, &AlsMovementParamsHandle, &TnuaController<AlsControlScheme>, &Stance, &RotationMode,
        &bevy_als::als::gait::AlsMaxAllowedGait, &bevy_als::als::motion::AlsViewYaw,
        &bevy_als::als::dynamics::AlsLandingBrakingFriction)>,
    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>,
        Option<&bevy_als::als::character::AlsCollisionLayerOwnership>)>,
    geometry: Query<(&Name, &Position, &Rotation, &Collider)>,
) {
    use bevy_als::als::{dynamics, motion, character};
    if probe.frame != 45 { return; }
    let actor = probe.actor.unwrap();
    let (position, capsule, mode, ground, base, runtime, handle, controller, stance, rotation, gait, yaw, braking) =
        actors.get(actor).unwrap();
    assert_eq!(*mode, LocomotionState::Grounded);
    assert_eq!(runtime.simulation_substeps, 3);
    assert_eq!(controller.basis.desired_motion, Vec3::ZERO);
    let params = assets.get(handle.id()).unwrap();
    let cmc = &params.cmc;
    let max_speed = dynamics::max_allowed_speed(params, *stance, *rotation, gait.0, base.relative_velocity, yaw.0);
    let filter = SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts = |other| motion::character_blocks_collider(actor, other, &candidates);
    let half = params.crouch.standing_capsule_half_height;
    let radius = params.crouch.standing_capsule_radius;
    let shrink = (half - radius) * 0.10000002384185791;
    let floor_range = 0.45_f32 + (0.024_f32 + 1.0000000000000002e-06);
    let short_capsule = character::als_character_capsule(half - shrink, radius);
    let mut at = position.0;
    let mut velocity = base.relative_velocity;
    let mut remaining = runtime.simulation_delta_seconds;
    let mut steps = Vec::new();
    for index in 1..=3 {
        let step = if remaining > cmc.max_simulation_time_step {
            cmc.max_simulation_time_step.min(remaining * 0.5)
        } else { remaining };
        remaining -= step;
        let plan = dynamics::grounded_velocity_step_with_braking_friction(
            velocity, Vec3::ZERO, max_speed, ground.0, cmc.min_analog_walk_speed,
            braking.factor, cmc.braking_substep_time, step);
        let delta = plan * step;
        let Ok((direction, distance)) = Dir3::new_and_length(delta) else {
            assert_eq!(plan, Vec3::ZERO);
            steps.push(serde_json::json!({"index": index, "step": step, "empty":true, "position":at}));
            velocity = plan;
            continue;
        };
        let forward = ShapeCastConfig { ignore_origin_penetration: true,
            ..ShapeCastConfig::from_max_distance(distance) };
        assert!(spatial.cast_shape_predicate(capsule, at, Quat::IDENTITY, direction, &forward, &filter, &accepts).is_none());
        let destination = at + delta;
        let raw_limit = 0.45_f32 + 0.024_f32 * 2.0;
        let raw = spatial.cast_shape_predicate(capsule, destination, Quat::IDENTITY, Dir3::NEG_Y,
            &ShapeCastConfig::from_max_distance(raw_limit), &filter, &accepts).unwrap();
        assert!(raw.normal1.y > 0.9999 && raw.distance > 0.0);
        let (name, box_center, box_rotation, box_shape) = geometry.get(raw.entity).unwrap();
        assert_eq!(name.as_str(), "AuditLandingDeck");
        assert!(box_shape.shape_scaled().as_cuboid().is_some());
        let clearance = character::ALS_CHARACTER_FLOOR_CLEARANCE + character::ALS_CHARACTER_TNUA_FLOAT_EPSILON;
        let proposed = destination.with_y((destination.y - raw.distance + clearance).max(at.y));
        let (floor_center, _, floor) = motion::find_ground_floor(&spatial, proposed, half, radius, cmc,
            *mode, &filter, actor, &candidates).unwrap();
        let adjusted = if (0.019..=0.024).contains(&(proposed.y - floor_center)) { proposed } else {
            proposed.with_y(floor_center + (0.019 + 0.024) * 0.5)
        };
        let short_limit = floor_range + shrink;
        let short = spatial.cast_shape_predicate(&short_capsule, proposed, Quat::IDENTITY, Dir3::NEG_Y,
            &ShapeCastConfig::from_max_distance(short_limit), &filter, &accepts).unwrap();
        assert_eq!(floor, raw.entity);
        assert_eq!(short.entity, raw.entity);
        assert!(short.normal1.y > 0.9999 && short.distance > 0.0);
        assert_eq!(proposed.y - (short.distance - shrink), floor_center);
        if let Ok((correction, length)) = Dir3::new_and_length(adjusted - proposed) {
            let config = ShapeCastConfig { ignore_origin_penetration: true,
                ..ShapeCastConfig::from_max_distance(length) };
            assert!(spatial.cast_shape_predicate(capsule, proposed, Quat::IDENTITY, correction,
                &config, &filter, &accepts).is_none());
        }
        velocity = (adjusted - at).with_y(0.0) / step;
        steps.push(serde_json::json!({
            "index": index, "step": step, "empty":false, "start": at,
            "plan":plan, "box_center":box_center.0, "box_rotation":box_rotation.0,
            "box_extents":box_shape.shape_scaled().compute_local_aabb().half_extents(),
            "full_start":destination, "full_hit":raw.distance, "full_limit":raw_limit,
            "full_capsule":capsule.shape_scaled().compute_local_aabb().half_extents(),
            "short_start":proposed, "short_hit":short.distance, "short_limit":short_limit,
            "short_capsule":short_capsule.shape_scaled().compute_local_aabb().half_extents(),
            "short_shrink":shrink, "floor_center":floor_center, "adjusted":adjusted
        }));
        at = adjusted;
    }
    state.start = position.0;
    state.replay = at;
    c516_write("brake-replay.json", serde_json::json!({
        "start": state.start, "velocity": base.relative_velocity, "factor": braking.factor,
        "steps": steps, "final": at, "final_velocity": velocity
    }));
}

fn c578_after(
    probe: Res<C458Probe>,
    state: Res<C578Height>,
    mut commands: Commands,
    mut actors: Query<(&Position, &mut Transform, &LinearVelocity, &LocomotionState,
        &AlsCmcAdapterRuntime, &TnuaMotor, &bevy_als::als::character::AlsEffectiveGravity)>,
) {
    if probe.frame != 45 { return; }
    let actor = probe.actor.unwrap();
    let (position, mut transform, physical, mode, runtime, motor, gravity) = actors.get_mut(actor).unwrap();
    let candidate = position.0;
    assert_eq!(candidate, state.replay);
    assert_eq!(candidate, transform.translation);
    let label = std::env::var("ALS_C578_MODE").unwrap();
    let submitted = if label == "band" {
        transform.translation.y = state.start.y;
        let corrected = candidate.with_y(state.start.y);
        commands.entity(actor).insert(Position(corrected));
        corrected
    } else { candidate };
    c516_write("brake-submitted.json", serde_json::json!({
        "mode": mode, "candidate":candidate, "submitted":submitted,
        "velocity":runtime.grounded_collision_velocity, "physical_y":physical.y,
        "motor_boost_y":motor.lin.boost.y, "motor_acceleration_y":motor.lin.acceleration.y, "gravity_y":gravity.value.y
    }));
}

fn c578_physics(probe: Res<C458Probe>, actors: Query<(&Position, &LinearVelocity)>) {
    if probe.frame.saturating_sub(1) != 45 { return; }
    let (position, velocity) = actors.get(probe.actor.unwrap()).unwrap();
    c516_write("brake-physics.json", serde_json::json!({"position":position.0,"velocity":velocity.0}));
}
