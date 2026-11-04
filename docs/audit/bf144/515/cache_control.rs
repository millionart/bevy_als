use bevy_als::als::{
    character::{AlsCmcAdapterRuntime, AlsMovementParamsHandle},
    dynamics::{AlsGroundedDynamics, AlsLandingBrakingFriction, AlsMovementBaseDynamicsState,
        grounded_velocity_step_with_braking_friction, max_allowed_speed, BRAKING_FRICTION_FACTOR},
    gait::AlsMaxAllowedGait,
    motion::AlsViewYaw,
    state::{RotationMode, Stance},
};
use bevy_tnua::{TnuaController, TnuaPipelineSystems};

const C515_ORDINAL: usize = 26;

#[derive(Resource, Default)]
struct C515Cache {
    previous: Option<(f32, Vec3)>,
    fresh: Option<f32>,
}

fn c515_install(app: &mut App) {
    use bevy_als::als::dynamics::grounded_dynamics_motor_system;
    app.init_resource::<C515Cache>();
    app.add_systems(Last,c515_delta);
    app.add_systems(Update,c515_capture.after(TnuaPipelineSystems::Logic).before(grounded_dynamics_motor_system));
    app.edit_schedule(Update, |schedule| {
        let transaction = {
            let systems: Vec<_> = schedule.graph().systems.iter().filter(|(_,system,_)| {
                system.name().to_string().ends_with("::cmc_movement_transaction_system")
            }).collect();
            assert_eq!(systems.len(),1);
            let sets=systems[0].1.default_system_sets();
            assert_eq!(sets.len(),1);
            sets[0]
        };
        schedule.add_systems(c515_before.after(grounded_dynamics_motor_system).before(transaction));
        schedule.add_systems(c515_after.after(transaction).before(c458_sample).before(TnuaPipelineSystems::Motors));
    });
}

fn c515_delta(probe: Res<C458Probe>, mut time: ResMut<TimeUpdateStrategy>) {
    if probe.frame==C515_ORDINAL {
        *time=TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(0.06));
    } else if probe.frame==C515_ORDINAL+1 {
        *time=TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0/30.0));
    }
}

fn c515_capture(
    probe: Res<C458Probe>,
    mut cache: ResMut<C515Cache>,
    actors: Query<(&AlsGroundedDynamics,&Position)>,
) {
    if probe.frame==C515_ORDINAL {
        let (dynamics,pos)=actors.get(probe.actor.unwrap()).unwrap();
        cache.previous=Some((dynamics.0.acceleration,pos.0));
    }
}

fn c515_before(
    probe: Res<C458Probe>,
    mut cache: ResMut<C515Cache>,
    time: Res<Time>,
    assets: Res<Assets<AlsMovementParams>>,
    mut actors: Query<(
        &AlsMovementParamsHandle,&LocomotionState,&Stance,&RotationMode,&AlsViewYaw,
        &AlsMaxAllowedGait,&TnuaController<AlsControlScheme>,Option<&AlsLandingBrakingFriction>,
        &mut AlsGroundedDynamics,&mut AlsMovementBaseDynamicsState,&mut AlsCmcAdapterRuntime,&mut TnuaMotor,&Position,
    )>,
) {
    if probe.frame!=C515_ORDINAL {
        return;
    }
    let (handle,mode,stance,rotation,view,gait,controller,braking,mut dynamics,
        mut base,mut runtime,mut motor,pos)=actors.get_mut(probe.actor.unwrap()).unwrap();
    let (old,initial_position)=cache.previous.unwrap();
    assert_eq!(pos.0,initial_position);
    assert_eq!(*mode,LocomotionState::Grounded);
    assert_eq!(runtime.simulation_substeps,2);
    assert_eq!(runtime.simulation_delta_seconds,time.delta_secs());
    assert_eq!(base.support_velocity,Vec3::ZERO);
    assert!(!runtime.min_analog_speed_applied && !runtime.has_animation_root_velocity);
    let params=assets.get(&handle.0).unwrap();
    let delta=runtime.simulation_delta_seconds;
    assert_eq!(delta,0.06);
    let step=delta*0.5;
    assert!(delta>params.cmc.max_simulation_time_step && step<=params.cmc.max_simulation_time_step);
    let current=base.relative_velocity;
    let speed=max_allowed_speed(params,*stance,*rotation,gait.0,current,view.0);
    let desired=controller.basis.desired_motion;
    let factor=braking.map_or(BRAKING_FRICTION_FACTOR,|value|value.factor);
    let simulate=|dynamics| {
        let advance=|velocity| grounded_velocity_step_with_braking_friction(
            velocity,desired,speed,dynamics,params.cmc.min_analog_walk_speed,
            factor,params.cmc.braking_substep_time,step);
        let first=advance(current);
        let second=advance(first);
        (first,second,first*step+second*step)
    };
    let fresh=dynamics.0;
    let baseline=simulate(fresh);
    assert_eq!(base.pre_collision_velocity,Some(baseline.1));
    assert_eq!(runtime.grounded_movement_delta,Some(baseline.2));
    assert_eq!(old,20.0);
    assert!(fresh.acceleration<old);
    cache.fresh=Some(fresh.acceleration);
    let mut cached=fresh;
    cached.acceleration=old;
    let replacement=simulate(cached);
    let control=std::env::var("ALS_C515_CONTROL").ok();
    match control.as_deref() {
        Some("plan-only") => {
            base.pre_collision_velocity=Some(replacement.1);
            base.motor_boost=replacement.1-current;
            runtime.grounded_movement_delta=Some(replacement.2);
            motor.lin.boost.x=replacement.1.x-base.world_velocity.x;
            motor.lin.boost.z=replacement.1.z-base.world_velocity.z;
        }
        Some("cached") => dynamics.0.acceleration=old,
        None => {}
        _ => panic!("Unknown two-step acceleration control"),
    }
    c515_write("stage-before.json",serde_json::json!({
        "ordinal":C515_ORDINAL,"control":control,"position":initial_position,
        "velocity":current,"desired":desired,"delta":delta,"steps":[step,step],
        "cached_acceleration":old,"fresh":[fresh.acceleration,fresh.braking_deceleration,fresh.ground_friction],
        "baseline_plan":[baseline.0,baseline.1,baseline.2],"cached_plan":[replacement.0,replacement.1,replacement.2],
        "transaction_acceleration":dynamics.0.acceleration,"preplan":base.pre_collision_velocity,
        "preplan_displacement":runtime.grounded_movement_delta
    }));
}

fn c515_after(
    probe: Res<C458Probe>,
    cache: Res<C515Cache>,
    mut actors: Query<(&mut AlsGroundedDynamics,&AlsMovementBaseDynamicsState,&AlsCmcAdapterRuntime)>,
) {
    if probe.frame!=C515_ORDINAL {
        return;
    }
    let (mut dynamics,base,runtime)=actors.get_mut(probe.actor.unwrap()).unwrap();
    assert_eq!(runtime.grounded_movement_delta,None);
    assert_eq!(runtime.grounded_move_start,None);
    let collision=runtime.grounded_collision_velocity.unwrap();
    let consumed=dynamics.0.acceleration;
    dynamics.0.acceleration=cache.fresh.unwrap();
    c515_write("stage-after.json",serde_json::json!({
        "ordinal":C515_ORDINAL,"transaction_acceleration":consumed,"published_acceleration":dynamics.0.acceleration,
        "collision_velocity":collision,"last_input":base.pre_collision_velocity.unwrap(),
        "substeps":runtime.simulation_substeps
    }));
}

fn c515_write(name: &str, value: serde_json::Value) {
    let output=std::path::PathBuf::from(std::env::var("ALS_C458_OUTPUT").unwrap()).with_file_name(name);
    std::fs::write(output,serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}
