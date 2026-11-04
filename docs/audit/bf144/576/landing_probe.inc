fn c576_install(app: &mut App) {
    use bevy_als::als::dynamics::grounded_dynamics_motor_system;
    app.edit_schedule(Update, |schedule| {
        let transaction = {
            let matches: Vec<_> = schedule.graph().systems.iter().filter(|(_, system, _)|
                system.name().to_string().ends_with("::cmc_movement_transaction_system")).collect();
            assert_eq!(matches.len(), 1);
            matches[0].1.default_system_sets()[0]
        };
        schedule.add_systems((
            c576_before.after(grounded_dynamics_motor_system).before(transaction),
            c576_after.after(transaction).before(c458_sample),
        ));
    });
}

fn c576_before(
    probe: Res<C458Probe>,
    time: Res<Time>,
    spatial: SpatialQuery,
    assets: Res<Assets<AlsMovementParams>>,
    actors: Query<(&Position, &Transform, &LinearVelocity, &LocomotionState, &Collider,
        &TnuaController<AlsControlScheme>, &TnuaMotor, &bevy_als::als::character::AlsEffectiveGravity,
        &AlsMovementParamsHandle, &Stance, &RotationMode,
        &bevy_als::als::gait::AlsMaxAllowedGait, &bevy_als::als::motion::AlsViewYaw,
        &bevy_als::actual_motion::AlsActualMotion)>,
    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>,
        Option<&bevy_als::als::character::AlsCollisionLayerOwnership>)>,
    names: Query<&Name>,
) {
    use bevy_als::als::{dynamics, motion, params::GroundedDynamics};
    if probe.frame != 58 { return; }
    let actor = probe.actor.unwrap();
    let (pos, transform, physical, mode, capsule, controller, motor, gravity, handle,
        stance, rotation, gait, yaw, saved) = actors.get(actor).unwrap();
    assert_eq!(*mode, LocomotionState::Air);
    let params = assets.get(handle.id()).unwrap();
    let cmc = &params.cmc;
    let desired = controller.basis.desired_motion;
    let mut velocity = physical.0 + motor.lin.boost;
    let mut position = pos.0;
    let initial_velocity = velocity;
    let acceleration = motion::air_acceleration_for_horizontal_speed(params, velocity.with_y(0.0).length())
        .min(params.acceleration.max_acceleration);
    let air = GroundedDynamics { acceleration, braking_deceleration: 0.0, ground_friction: 0.0 };
    let max_speed = dynamics::max_allowed_speed(params, *stance, *rotation, gait.0, velocity.with_y(0.0), yaw.0);
    let gravity = gravity.value + motor.lin.acceleration;
    let mut remaining = time.delta_secs();
    let filter = SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts = |entity| motion::character_blocks_collider(actor, entity, &candidates);
    let mut air_steps = Vec::new();
    for index in 1..=3 {
        let step = if remaining > cmc.max_simulation_time_step {
            cmc.max_simulation_time_step.min(remaining * 0.5)
        } else { remaining };
        let next = dynamics::grounded_velocity_step_with_braking_friction(
            velocity.with_y(0.0), desired, max_speed, air, cmc.min_analog_walk_speed,
            0.0, cmc.braking_substep_time, step) + Vec3::Y * velocity.y + gravity * step;
        assert!(next.y.abs() < 10.0);
        let movement = (velocity + next) * (0.5 * step);
        let (direction, distance) = Dir3::new_and_length(movement).unwrap();
        let config = ShapeCastConfig { ignore_origin_penetration: true,
            ..ShapeCastConfig::from_max_distance(distance) };
        let hit = spatial.cast_shape_predicate(capsule, position, transform.rotation, direction, &config, &filter, &accepts);
        air_steps.push(serde_json::json!({"index": index,"step": step, "position": position,
            "incoming": velocity, "new_velocity": next, "requested": movement,
            "hit_distance": hit.as_ref().map(|hit| hit.distance)}));
        if index < 3 {
            assert!(hit.is_none());
            position += movement;
            velocity = next;
            remaining -= step;
            continue;
        }
        let hit = hit.unwrap();
        let travel = motion::movement_sweep_travel(distance, hit.distance);
        let impact = position + *direction * travel;
        let extent = capsule.shape_scaled().compute_local_aabb().half_extents();
        let radius = extent.x.min(extent.z);
        let (floor_y, impact_y, entity) = motion::find_ground_floor(
            &spatial, impact, extent.y, radius, cmc, LocomotionState::Air,
            &filter, actor, &candidates).unwrap();
        assert!((impact.y - floor_y).abs() <= 0.024);
        assert!(impact_y < impact.y - extent.y + radius);
        assert_eq!(entity, hit.entity);
        let walk_time = remaining - step + step * (1.0 - travel / distance);
        assert!(walk_time < cmc.max_simulation_time_step);
        let walk_entry = next.with_y(0.0);
        let thresholds = params.gait_speed_thresholds(*stance, *rotation, walk_entry, yaw.0);
        let gait_amount = bevy_als::als::gait::calculate_gait_amount_for_thresholds(walk_entry.length(), thresholds);
        let ground = params.grounded_dynamics(*stance, *rotation, gait_amount);
        let walk_max = dynamics::max_allowed_speed(params, *stance, *rotation, gait.0, walk_entry, yaw.0);
        let friction = if saved.has_input { 0.5 } else { 3.0 };
        let planned = dynamics::grounded_velocity_step_with_braking_friction(
            walk_entry, desired, walk_max, ground, cmc.min_analog_walk_speed, friction,
            cmc.braking_substep_time, walk_time);
        let cached = GroundedDynamics { acceleration: params.acceleration.max_acceleration, ..ground };
        let source_time: serde_json::Value = serde_json::from_slice(
            &std::fs::read("docs/audit/bf144/576/native-time.json").unwrap()).unwrap();
        let label = std::env::var("ALS_C575_MODE").unwrap();
        let source_time = source_time[&label].as_f64().unwrap() as f32;
        let cached_plan = |delta| dynamics::grounded_velocity_step_with_braking_friction(
            walk_entry, desired, walk_max, cached, cmc.min_analog_walk_speed,
            friction, cmc.braking_substep_time, delta);
        let request = planned * walk_time;
        let start_y = if (0.019..=0.024).contains(&(impact.y - floor_y)) { impact.y }
            else { floor_y + (0.019 + 0.024) * 0.5 };
        let start = impact.with_y(start_y);
        let (forward, range) = Dir3::new_and_length(request).unwrap();
        let forward_config = ShapeCastConfig { max_distance: range, ..config };
        assert!(spatial.cast_shape_predicate(capsule, start, Quat::IDENTITY, forward, &forward_config, &filter, &accepts).is_none());
        let end = start + request;
        let applied = (end - start).with_y(0.0);
        c516_write("landing-replay.json", serde_json::json!({
            "position": pos.0, "initial_velocity": initial_velocity, "gravity": gravity,
            "air_steps": air_steps, "impact": impact, "hit_distance": hit.distance,
            "hit_entity": names.get(entity).ok().map(Name::as_str), "floor_impact": impact_y,
            "hit_normal": hit.normal1, "walk_time": walk_time, "walk_entry": walk_entry,
            "ground_friction": ground.ground_friction, "walk_max": walk_max,
            "fresh_acceleration": ground.acceleration, "cached_acceleration": cached.acceleration,
            "source_time": source_time, "cached_plan": cached_plan(walk_time),
            "cached_plan_source_time": cached_plan(source_time),
            "planned_velocity": planned, "requested_walk": request, "walk_start": start,
            "walk_end": end, "applied_walk": applied, "replayed_velocity": applied / walk_time
        }));
    }
}

fn c576_after(
    probe: Res<C458Probe>,
    actors: Query<(&Position, &LocomotionState, &AlsCmcAdapterRuntime)>,
) {
    if probe.frame != 58 { return; }
    let (position, mode, runtime) = actors.get(probe.actor.unwrap()).unwrap();
    c516_write("landing-published.json", serde_json::json!({
        "position": position.0, "mode": mode, "steps": runtime.simulation_substeps,
        "velocity": runtime.grounded_collision_velocity, "contact": runtime.airborne_sweep_contact
    }));
}
