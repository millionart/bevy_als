use bevy_als::als::{
    character::{AlsCmcAdapterRuntime, AlsMovementParamsHandle},
    dynamics::{AlsGroundedDynamics, AlsLandingBrakingFriction, AlsMovementBaseDynamicsState,
        grounded_velocity_step_with_braking_friction, max_allowed_speed, BRAKING_FRICTION_FACTOR},
    gait::AlsMaxAllowedGait,
    motion::AlsViewYaw,
    state::{RotationMode, Stance},
};
use bevy_tnua::{TnuaController, TnuaPipelineSystems};

const C514_ORDINAL: usize = 26;

#[derive(Resource, Default)]
struct C514Previous(Option<(f32, Option<Vec3>, Vec3)>);

fn c514_install(app: &mut App) {
    use bevy_als::als::dynamics::grounded_dynamics_motor_system;
    app.init_resource::<C514Previous>();
    app.add_systems(Update,c514_capture.after(TnuaPipelineSystems::Logic).before(grounded_dynamics_motor_system));
    app.edit_schedule(Update, |schedule| {
        let transaction = {
            let systems: Vec<_> = schedule.graph().systems.iter().filter(|(_,system,_)| {
                system.name().to_string().ends_with("::cmc_movement_transaction_system")
            }).collect();
            assert_eq!(systems.len(), 1);
            let sets = systems[0].1.default_system_sets();
            assert_eq!(sets.len(), 1);
            sets[0]
        };
        schedule.add_systems(c514_apply.after(grounded_dynamics_motor_system)
            .before(transaction).before(c458_sample).before(TnuaPipelineSystems::Motors));
    });
}

fn c514_capture(
    probe: Res<C458Probe>,
    dynamics: Query<(&AlsGroundedDynamics, &AlsCmcAdapterRuntime, &Position)>,
    mut previous: ResMut<C514Previous>,
) {
    if probe.frame == C514_ORDINAL {
        let (dynamics, runtime, position) = dynamics.get(probe.actor.unwrap()).unwrap();
        previous.0 = Some((dynamics.0.acceleration, runtime.grounded_move_start, position.0));
    }
}

fn c514_apply(
    probe: Res<C458Probe>,
    previous: Res<C514Previous>,
    time: Res<Time>,
    assets: Res<Assets<AlsMovementParams>>,
    mut actors: Query<(
        &AlsMovementParamsHandle, &LocomotionState, &Stance, &RotationMode,
        &AlsViewYaw, &AlsMaxAllowedGait, &AlsGroundedDynamics,
        &TnuaController<AlsControlScheme>, Option<&AlsLandingBrakingFriction>,
        &mut AlsMovementBaseDynamicsState, &mut AlsCmcAdapterRuntime, &mut TnuaMotor, &Position,
    )>,
) {
    if probe.frame != C514_ORDINAL {
        return;
    }
    let (handle, mode, stance, rotation, view, gait, fresh, controller, braking,
        mut base, mut runtime, mut motor, position) = actors.get_mut(probe.actor.unwrap()).unwrap();
    assert_eq!(*mode, LocomotionState::Grounded);
    assert_eq!(runtime.simulation_substeps, 1);
    let (old, previous_move_start, initial_position) = previous.0.unwrap();
    assert_eq!(runtime.grounded_move_start, previous_move_start);
    assert_eq!(position.0, initial_position);
    assert_ne!(previous_move_start, Some(initial_position));
    assert_eq!(runtime.simulation_delta_seconds, time.delta_secs());
    assert_eq!(base.support_velocity, Vec3::ZERO);
    assert!(!runtime.min_analog_speed_applied && !runtime.has_animation_root_velocity);
    let params = assets.get(&handle.0).unwrap();
    let delta = runtime.simulation_delta_seconds;
    let current = base.relative_velocity;
    let speed = max_allowed_speed(params,*stance,*rotation,gait.0,current,view.0);
    let desired = controller.basis.desired_motion;
    let factor = braking.map_or(BRAKING_FRICTION_FACTOR, |value| value.factor);
    let simulate = |dynamics| grounded_velocity_step_with_braking_friction(
        current,desired,speed,dynamics,params.cmc.min_analog_walk_speed,factor,
        params.cmc.braking_substep_time,delta);
    let baseline = simulate(fresh.0);
    assert_eq!(base.pre_collision_velocity,Some(baseline));
    assert_eq!(runtime.grounded_movement_delta,Some(baseline*delta));
    assert_eq!(old, 20.0);
    assert!(old > fresh.0.acceleration);
    let mut cached = fresh.0;
    cached.acceleration = old;
    let plan = simulate(cached);
    let apply = std::env::var_os("ALS_C514_CACHED_ACCELERATION").is_some();
    if apply {
        base.pre_collision_velocity = Some(plan);
        base.motor_boost = plan-current;
        runtime.grounded_movement_delta = Some(plan*delta);
        motor.lin.boost.x = plan.x-base.world_velocity.x;
        motor.lin.boost.z = plan.z-base.world_velocity.z;
    }
    let output = std::path::PathBuf::from(std::env::var("ALS_C458_OUTPUT").unwrap()).with_file_name("stage.json");
    std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({
        "ordinal":C514_ORDINAL,"before_collision_transaction":true,"applied":apply,
        "previous_acceleration":old,"fresh":[fresh.0.acceleration,fresh.0.braking_deceleration,fresh.0.ground_friction],
        "initial_position":initial_position.to_array(),"prior_move_start":previous_move_start.map(|v|v.to_array()),
        "initial":current.to_array(),"desired":desired.to_array(),"speed":speed,"delta":delta,
        "baseline_plan":baseline.to_array(),"cached_plan":plan.to_array(),
        "recorded_plan":base.pre_collision_velocity.unwrap().to_array(),
        "recorded_movement":runtime.grounded_movement_delta.unwrap().to_array()
    })).unwrap()).unwrap();
}
