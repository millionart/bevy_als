#[derive(Resource, Default)]
struct C574Height(f32);

fn c574_install(app: &mut App) {
    use bevy_als::als::dynamics::grounded_dynamics_motor_system;
    app.init_resource::<C574Height>();
    app.edit_schedule(Update, |schedule| {
        let transaction = {
            let matches: Vec<_> = schedule.graph().systems.iter().filter(|(_, system, _)|
                system.name().to_string().ends_with("::cmc_movement_transaction_system")).collect();
            assert_eq!(matches.len(), 1);
            matches[0].1.default_system_sets()[0]
        };
        schedule.add_systems((
            c574_before.after(grounded_dynamics_motor_system).before(transaction),
            c574_transaction.after(transaction).before(TnuaPipelineSystems::Motors).before(c458_sample),
        ));
    });
    app.add_systems(PostUpdate, (
        c574_physics.after(PhysicsSystems::Last).before(AlsActualVelocitySet),
        c574_final.after(AlsActualVelocitySet),
    ));
}

fn c574_before(
    probe: Res<C458Probe>,
    mut start: ResMut<C574Height>,
    actors: Query<(&Position, &LinearVelocity, &LocomotionState, &AlsCmcAdapterRuntime, &AlsMovementBaseDynamicsState)>,
) {
    let frame = probe.frame;
    if !(52..=53).contains(&frame) { return; }
    let (position, velocity, mode, runtime, base) = actors.get(probe.actor.unwrap()).unwrap();
    start.0 = position.y;
    c516_write(&format!("height-before-{frame}.json"), serde_json::json!({
        "position": position.0, "velocity": velocity.0, "mode": mode, "support": base.support_velocity,
        "substeps": runtime.simulation_substeps, "delta": runtime.simulation_delta_seconds
    }));
}

fn c574_transaction(
    probe: Res<C458Probe>,
    start: Res<C574Height>,
    mut commands: Commands,
    spatial: SpatialQuery,
    assets: Res<Assets<AlsMovementParams>>,
    mut actors: Query<(&Position, &mut Transform, &LinearVelocity, &LocomotionState,
        &AlsCmcAdapterRuntime, &AlsMovementParamsHandle, &TnuaMotor, &bevy_als::als::character::AlsEffectiveGravity)>,
    candidates: Query<(Option<&CollisionLayers>, Has<Sensor>, Option<&ColliderOf>, Option<&bevy_als::als::character::AlsCollisionLayerOwnership>)>,
    names: Query<&Name>,
) {
    let frame = probe.frame;
    if !(52..=53).contains(&frame) { return; }
    let actor = probe.actor.unwrap();
    let (position, mut transform, velocity, mode, runtime, handle, motor, gravity) =
        actors.get_mut(actor).unwrap();
    assert_eq!(*mode, LocomotionState::Grounded);
    let candidate = transform.translation;
    assert_eq!(position.0, candidate);
    let params = assets.get(handle.id()).unwrap();
    let filter = SpatialQueryFilter::from_excluded_entities([actor]);
    let at = candidate.with_y(start.0);
    let (center, impact, floor) = bevy_als::als::motion::find_ground_floor(
        &spatial, at, params.crouch.standing_capsule_half_height, params.crouch.standing_capsule_radius,
        &params.cmc, *mode, &filter, actor, &candidates).unwrap();
    let floor = names.get(floor).unwrap().as_str();
    let gap = at.y - center;
    let within_band = (0.019..=0.024).contains(&gap);
    let control = std::env::var("ALS_C574_MODE").unwrap() == "band";
    let submitted = if control {
        assert!(within_band && floor == "AuditLandingDeck");
        transform.translation.y = start.0;
        commands.entity(actor).insert(Position(at));
        at
    } else {
        position.0
    };
    c516_write(&format!("height-transaction-{frame}.json"), serde_json::json!({
        "candidate": candidate, "submitted": submitted, "control": control,
        "floor": floor, "floor_center": center, "floor_impact": impact, "preserved_gap": gap,
        "within_band": within_band, "motor_boost_y": motor.lin.boost.y,
        "motor_acceleration_y": motor.lin.acceleration.y, "gravity_y": gravity.value.y, "body_velocity_y": velocity.y,
        "collision_velocity": runtime.grounded_collision_velocity
    }));
}

fn c574_physics(probe: Res<C458Probe>, actors: Query<(&Position, &LinearVelocity)>) {
    let frame = probe.frame.saturating_sub(1);
    if !(52..=53).contains(&frame) { return; }
    let (position, velocity) = actors.get(probe.actor.unwrap()).unwrap();
    c516_write(&format!("height-physics-{frame}.json"), serde_json::json!({
        "position": position.0, "velocity": velocity.0
    }));
}

fn c574_final(probe: Res<C458Probe>, actors: Query<(&Position, &LinearVelocity, &LocomotionState)>) {
    let frame = probe.frame.saturating_sub(1);
    if !(52..=53).contains(&frame) { return; }
    let (position, velocity, mode) = actors.get(probe.actor.unwrap()).unwrap();
    c516_write(&format!("height-final-{frame}.json"), serde_json::json!({
        "position": position.0, "velocity": velocity.0, "mode": mode
    }));
}
