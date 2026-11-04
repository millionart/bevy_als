fn c533_install(app: &mut App) {
    app.add_systems(First, c533_start);
    app.edit_schedule(Update, |schedule| {
        let transaction = {
            let matches: Vec<_> = schedule
                .graph()
                .systems
                .iter()
                .filter(|(_, system, _)| {
                    system
                        .name()
                        .to_string()
                        .ends_with("::cmc_movement_transaction_system")
                })
                .collect();
            assert_eq!(matches.len(), 1);
            matches[0].1.default_system_sets()[0]
        };
        schedule.add_systems(c533_first_step.after(c516_before).before(transaction));
        schedule.add_systems(c533_after_transaction.after(c516_after).before(c458_sample));
    });
}

fn c533_start(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    mut actors: Query<(
        &mut Position,
        &mut Transform,
        &mut LinearVelocity,
        &LocomotionState,
        &AlsMovementBase,
    )>,
) {
    if probe.frame != control.frame {
        return;
    }
    let (mut position, mut transform, mut velocity, mode, base) =
        actors.get_mut(probe.actor.unwrap()).unwrap();
    assert_eq!(*mode, LocomotionState::Grounded);
    assert!(base.entity.is_some());
    position.0 = Vec3::new(-14.01, 2.9215, -15.0);
    transform.translation = position.0;
    velocity.0 = Vec3::new(6.0, 0.0, 0.0);
}

fn c533_floor_read(
    spatial: &SpatialQuery,
    point: Vec3,
    half: f32,
    radius: f32,
    params: &AlsMovementParams,
    actor: Entity,
    candidates: &Query<(
        Option<&CollisionLayers>,
        Has<Sensor>,
        Option<&ColliderOf>,
        Option<&AlsCollisionLayerOwnership>,
    )>,
    names: &Query<&Name>,
) -> serde_json::Value {
    let filter = SpatialQueryFilter::from_excluded_entities([actor]);
    let mode = LocomotionState::Grounded;
    let public = bevy_als::als::motion::find_ground_floor(
        spatial,
        point,
        half,
        radius,
        &params.cmc,
        mode,
        &filter,
        actor,
        candidates,
    );
    let mut variants = serde_json::Map::new();
    for skip in ["none", "perch"] {
        let mut events = Vec::new();
        let floor = c529_floor(
            spatial,
            point,
            half,
            radius,
            &params.cmc,
            mode,
            &filter,
            actor,
            candidates,
            skip,
            &mut events,
            names,
        )
        .map(|(floor, _)| floor);
        if skip == "none" {
            assert_eq!(floor, public);
        }
        variants.insert(skip.to_string(),serde_json::json!({"events":events,
            "result":floor.map(|(center,impact,entity)|serde_json::json!({"center":center,"impact":impact,"entity":c529_name(entity,names)}))}));
    }
    serde_json::json!({"origin":point,"walking_range":true,"variants":variants})
}

fn c533_first_step(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    time: Res<Time>,
    spatial: SpatialQuery,
    assets: Res<Assets<AlsMovementParams>>,
    actors: Query<(
        &Position,
        &LinearVelocity,
        &LocomotionState,
        &AlsMovementBaseDynamicsState,
        &AlsGroundedDynamics,
        &AlsCmcAdapterRuntime,
        &AlsMovementParamsHandle,
        &TnuaController<AlsControlScheme>,
        &Stance,
        &RotationMode,
        &bevy_als::als::gait::AlsMaxAllowedGait,
        &bevy_als::als::motion::AlsViewYaw,
        Option<&bevy_als::als::dynamics::AlsLandingBrakingFriction>,
    )>,
    candidates: Query<(
        Option<&CollisionLayers>,
        Has<Sensor>,
        Option<&ColliderOf>,
        Option<&AlsCollisionLayerOwnership>,
    )>,
    names: Query<&Name>,
) {
    if probe.frame != control.frame {
        return;
    }
    let actor = probe.actor.unwrap();
    let (
        position,
        physical,
        mode,
        base,
        dynamics,
        runtime,
        handle,
        controller,
        stance,
        rotation,
        gait,
        yaw,
        braking,
    ) = actors.get(actor).unwrap();
    let params = assets.get(handle.id()).unwrap();
    assert_eq!(*mode, LocomotionState::Grounded);
    assert_eq!(runtime.simulation_substeps, 2);
    assert!(runtime.grounded_movement_delta.is_some());
    let delta = time.delta_secs();
    assert!(
        delta > params.cmc.max_simulation_time_step && params.cmc.max_simulation_iterations > 1
    );
    let step = params.cmc.max_simulation_time_step.min(delta * 0.5);
    let speed = bevy_als::als::dynamics::max_allowed_speed(
        params,
        *stance,
        *rotation,
        gait.0,
        base.relative_velocity,
        yaw.0,
    );
    let factor = braking.map_or(bevy_als::als::dynamics::BRAKING_FRICTION_FACTOR, |state| {
        state.factor
    });
    let velocity = bevy_als::als::dynamics::grounded_velocity_step_with_braking_friction(
        base.relative_velocity,
        controller.basis.desired_motion,
        speed,
        dynamics.0,
        params.cmc.min_analog_walk_speed,
        factor,
        params.cmc.braking_substep_time,
        step,
    );
    let first_end = position.0 + velocity * step;
    let half = params.crouch.standing_capsule_half_height;
    let radius = params.crouch.standing_capsule_radius;
    c516_write(
        "walking-step.json",
        serde_json::json!({"position":position.0,"velocity":physical.0,
        "base_velocity":base.relative_velocity,"step_s":step,"delta_s":delta,"first_step_velocity":velocity,
        "start_floor":c533_floor_read(&spatial,position.0,half,radius,params,actor,&candidates,&names),
        "first_floor":c533_floor_read(&spatial,first_end,half,radius,params,actor,&candidates,&names)}),
    );
}

fn c533_after_transaction(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    spatial: SpatialQuery,
    assets: Res<Assets<AlsMovementParams>>,
    actors: Query<(
        &Transform,
        &LocomotionState,
        &AlsCmcAdapterRuntime,
        &AlsMovementParamsHandle,
    )>,
    candidates: Query<(
        Option<&CollisionLayers>,
        Has<Sensor>,
        Option<&ColliderOf>,
        Option<&AlsCollisionLayerOwnership>,
    )>,
    names: Query<&Name>,
) {
    if probe.frame != control.frame {
        return;
    }
    let actor = probe.actor.unwrap();
    let (transform, mode, runtime, handle) = actors.get(actor).unwrap();
    let params = assets.get(handle.id()).unwrap();
    c516_write(
        "walking-transaction.json",
        serde_json::json!({"position":transform.translation,"mode":mode,
        "collision_velocity":runtime.grounded_collision_velocity,
        "end_floor":c533_floor_read(&spatial,transform.translation,params.crouch.standing_capsule_half_height,
            params.crouch.standing_capsule_radius,params,actor,&candidates,&names)}),
    );
}
