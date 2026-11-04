use avian3d::dynamics::{
    integrator::VelocityIntegrationData,
    solver::{schedule::SolverSystems,solver_body::SolverBody},
};
use bevy_tnua::{TnuaController,TnuaRigidBodyTracker};
use bevy_als::als::motion::AlsActualVelocitySet;

const C518_FRAME: usize=49;

fn c518_install(app:&mut App) {
    use bevy_als::als::dynamics::grounded_dynamics_motor_system;
    app.edit_schedule(Update,|schedule| {
        let transaction={
            let systems:Vec<_>=schedule.graph().systems.iter().filter(|(_,system,_)|
                system.name().to_string().ends_with("::cmc_movement_transaction_system")).collect();
            assert_eq!(systems.len(),1);
            let sets=systems[0].1.default_system_sets();
            assert_eq!(sets.len(),1);
            sets[0]
        };
        schedule.add_systems(c518_before.after(grounded_dynamics_motor_system).before(transaction));
        schedule.add_systems(c518_control.after(transaction).before(c458_sample).before(TnuaPipelineSystems::Motors));
        schedule.add_systems(c518_motor.after(TnuaPipelineSystems::Motors));
    });
    app.add_systems(avian3d::prelude::PhysicsSchedule,c518_coefficients
        .after(SolverSystems::PreSubstep).before(SolverSystems::Substep));
    app.add_systems(PostUpdate,c518_raw.after(avian3d::prelude::PhysicsSystems::Last).before(AlsActualVelocitySet));
    app.add_systems(PostUpdate,c518_final.after(AlsActualVelocitySet));
}

fn c518_before(probe:Res<C458Probe>,actors:Query<(
    &Position,&LinearVelocity,&LocomotionState,&TnuaMotor,&AlsCmcAdapterRuntime,&AlsMovementBaseDynamicsState,
)>) {
    if probe.frame!=C518_FRAME { return; }
    let (p,v,mode,motor,runtime,base)=actors.get(probe.actor.unwrap()).unwrap();
    c516_write("vertical-before.json",serde_json::json!({
        "ordinal":C518_FRAME,"position":p.0,"velocity":v.0,"mode":mode,
        "support":base.support_velocity,"substeps":runtime.simulation_substeps,
        "motor_acceleration":motor.lin.acceleration,"motor_boost":motor.lin.boost,
        "planned_move":runtime.grounded_movement_delta
    }));
}

fn c518_control(probe:Res<C458Probe>,mut actors:Query<(
    &Transform,&LinearVelocity,&LocomotionState,&mut TnuaMotor,&AlsCmcAdapterRuntime,
    &TnuaController<AlsControlScheme>,&TnuaRigidBodyTracker,
    &TnuaSensorsEntities<AlsControlScheme>,
)>,sensors:Query<&TnuaProximitySensor>) {
    if probe.frame!=C518_FRAME { return; }
    let (t,v,mode,mut motor,runtime,controller,tracker,entities)=actors.get_mut(probe.actor.unwrap()).unwrap();
    let walk=controller.basis_access().unwrap();
    let sensor=sensors.get(entities.ground.unwrap()).unwrap().output.as_ref().unwrap();
    let spring=-walk.memory.standing_offset.y*walk.config.spring_strength;
    let original=motor.lin.acceleration.y;
    let neutralized=std::env::var_os("ALS_C518_NEUTRALIZE").is_some();
    if neutralized {
        assert_eq!(original,spring-tracker.gravity.y);
        assert_eq!(motor.lin.boost.y,0.0);
        motor.lin.acceleration.y=-tracker.gravity.y;
    }
    c516_write("vertical-after-transaction.json",serde_json::json!({
        "ordinal":C518_FRAME,"position":t.translation,"velocity":v.0,"mode":mode,
        "spring_offset":walk.memory.standing_offset.y,"strength":walk.config.spring_strength,
        "float_height":walk.config.float_height,"proximity":sensor.proximity,
        "sensor_normal":sensor.normal,"sensor_base_velocity":sensor.entity_linvel,
        "tracker_gravity":tracker.gravity,"spring":spring,"original_y":original,
        "effective_y":motor.lin.acceleration.y,"boost_y":motor.lin.boost.y,"neutralized":neutralized,
        "collision_velocity":runtime.grounded_collision_velocity,"move_start":runtime.grounded_move_start
    }));
}

fn c518_motor(probe:Res<C458Probe>,actors:Query<(&Position,&LinearVelocity,&TnuaMotor)>) {
    if probe.frame!=C518_FRAME+1 { return; }
    let (p,v,m)=actors.get(probe.actor.unwrap()).unwrap();
    c516_write("vertical-after-motor.json",serde_json::json!({"position":p.0,
        "velocity":v.0,"acceleration":m.lin.acceleration,"boost":m.lin.boost}));
}

fn c518_coefficients(
    probe:Res<C458Probe>,
    actors:Query<(&VelocityIntegrationData,&SolverBody)>,
    dt:Res<Time<avian3d::prelude::Substeps>>,
    count:Res<avian3d::prelude::SubstepCount>,
) {
    if probe.frame!=C518_FRAME+1 { return; }
    let (data,body)=actors.get(probe.actor.unwrap()).unwrap();
    c516_write("vertical-integration.json",serde_json::json!({
        "linear_increment":data.linear_increment,"linear_damping_rhs":data.linear_damping_rhs,
        "initial_solver_velocity":body.linear_velocity,"substep_seconds":dt.delta_secs(),"substeps":count.0
    }));
}

fn c518_raw(probe:Res<C458Probe>,actors:Query<(&Position,&LinearVelocity,&LocomotionState)>) {
    if probe.frame!=C518_FRAME+1 { return; }
    let (p,v,mode)=actors.get(probe.actor.unwrap()).unwrap();
    c516_write("vertical-raw.json",serde_json::json!({"position":p.0,"velocity":v.0,"mode":mode}));
}

fn c518_final(probe:Res<C458Probe>,actors:Query<(&Position,&LinearVelocity,&LocomotionState)>) {
    if probe.frame!=C518_FRAME+1 { return; }
    let (p,v,mode)=actors.get(probe.actor.unwrap()).unwrap();
    c516_write("vertical-published.json",serde_json::json!({"position":p.0,"velocity":v.0,"mode":mode}));
}
