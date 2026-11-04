#[derive(Resource, Default)]
struct C573Acceleration {
    before_refresh: f32,
    refreshed: f32,
}

fn c573_install(app: &mut App) {
    use bevy_als::als::dynamics::grounded_dynamics_motor_system;
    app.init_resource::<C573Acceleration>();
    app.edit_schedule(Update, |schedule| {
        let transaction = {
            let matches: Vec<_> = schedule.graph().systems.iter().filter(|(_, system, _)|
                system.name().to_string().ends_with("::cmc_movement_transaction_system")).collect();
            assert_eq!(matches.len(), 1);
            matches[0].1.default_system_sets()[0]
        };
        schedule.add_systems((
            c573_cache.after(TnuaPipelineSystems::Logic).before(grounded_dynamics_motor_system),
            c573_use.after(grounded_dynamics_motor_system).before(transaction),
            c573_restore.after(transaction).before(c458_sample),
        ));
    });
}

fn c573_cache(
    probe: Res<C458Probe>,
    mut state: ResMut<C573Acceleration>,
    actors: Query<&AlsGroundedDynamics>,
) {
    if probe.frame == 54 {
        state.before_refresh = actors.get(probe.actor.unwrap()).unwrap().0.acceleration;
    }
}

fn c573_use(
    probe: Res<C458Probe>,
    mut state: ResMut<C573Acceleration>,
    mut actors: Query<(&mut AlsGroundedDynamics, &LocomotionState, &Position, &LinearVelocity,
        &AlsCmcAdapterRuntime, &AlsMovementBaseDynamicsState)>,
) {
    if probe.frame != 54 { return; }
    let (mut dynamics, mode, position, velocity, runtime, base) =
        actors.get_mut(probe.actor.unwrap()).unwrap();
    state.refreshed = dynamics.0.acceleration;
    let control = std::env::var("ALS_C573_MODE").unwrap() == "cached";
    if control {
        dynamics.0.acceleration = state.before_refresh;
    }
    c516_write("mixed-before.json", serde_json::json!({
        "mode": mode, "position": position.0, "velocity": velocity.0,
        "base_support": base.support_velocity, "cached_acceleration": state.before_refresh,
        "fresh_acceleration": state.refreshed, "walking_acceleration": dynamics.0.acceleration,
        "delta": runtime.simulation_delta_seconds, "planned_substeps": runtime.simulation_substeps
    }));
}

fn c573_restore(
    probe: Res<C458Probe>,
    state: Res<C573Acceleration>,
    mut actors: Query<(&mut AlsGroundedDynamics, &Transform, &LocomotionState, &AlsCmcAdapterRuntime)>,
) {
    if probe.frame != 54 { return; }
    let (mut dynamics, transform, mode, runtime) = actors.get_mut(probe.actor.unwrap()).unwrap();
    dynamics.0.acceleration = state.refreshed;
    c516_write("mixed-after.json", serde_json::json!({
        "mode": mode, "position": transform.translation, "velocity": runtime.grounded_collision_velocity,
        "restored_acceleration": dynamics.0.acceleration
    }));
}
