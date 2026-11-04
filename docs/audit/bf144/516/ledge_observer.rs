use bevy_als::als::{
    character::{AlsCmcAdapterRuntime, AlsMovementParamsHandle},
    dynamics::{AlsGroundedDynamics,AlsMovementBaseDynamicsState},
};
use bevy_tnua::TnuaPipelineSystems;

#[derive(Resource)]
struct C516Control {
    frame: usize,
    mode: String,
    previous_acceleration: Option<f32>,
    restore: Option<f32>,
}

fn c516_write(name:&str,value:serde_json::Value) {
    let output=std::path::PathBuf::from(std::env::var("ALS_C458_OUTPUT").unwrap()).with_file_name(name);
    std::fs::write(output,serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

fn c516_setup(
    mut commands: Commands,
    ready: Res<ManualPlaygroundStartupReady>,
    admissions: Query<&AlsPlayerAdmissionComplete>,
    mut actors: Query<(&mut Transform,&mut Position)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut done: Local<bool>,
) {
    if *done || !ready.passed {
        return;
    }
    let Some(admission)=admissions.iter().next() else {return;};
    let Ok((mut transform,mut pos))=actors.get_mut(admission.character) else {return;};
    let half_width_cm=std::env::var("ALS_C516_HALF_WIDTH_CM").unwrap().parse::<u32>().unwrap();
    assert!(half_width_cm>0);
    let width=half_width_cm as f32*0.02;
    let origin=pos.0;
    assert!((transform.translation-origin).length()<0.01);
    commands.spawn((
        Name::new("AuditWalkOffPlatform"),
        Transform::from_xyz(origin.x,1.9,origin.z),
        RigidBody::Static,
        Collider::cuboid(width,0.2,3.0),
        CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
        Mesh3d(meshes.add(Cuboid::new(width,0.2,3.0))),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
    ));
    pos.0.y+=2.0;
    transform.translation=pos.0;
    c516_write("fixture.json",serde_json::json!({"origin":origin,"half_width_cm":half_width_cm,"floor_y":2.0}));
    *done=true;
}

fn c516_install(app:&mut App) {
    use bevy_als::als::dynamics::grounded_dynamics_motor_system;
    let frame=std::env::var("ALS_C516_FRAME").unwrap().parse().unwrap();
    let mode=std::env::var("ALS_C516_CONTROL").unwrap();
    assert!(matches!(mode.as_str(),"none"|"air-cached"|"walking-cached"));
    app.insert_resource(C516Control {frame,mode,previous_acceleration:None,restore:None});
    app.add_systems(Last,c516_delta);
    app.add_systems(Update,c516_cache.after(TnuaPipelineSystems::Logic).before(grounded_dynamics_motor_system));
    app.edit_schedule(Update,|schedule| {
        let transaction={
            let matches:Vec<_>=schedule.graph().systems.iter().filter(|(_,system,_)|
                system.name().to_string().ends_with("::cmc_movement_transaction_system")).collect();
            assert_eq!(matches.len(),1);
            let sets=matches[0].1.default_system_sets();
            assert_eq!(sets.len(),1);
            sets[0]
        };
        schedule.add_systems(c516_before.after(grounded_dynamics_motor_system).before(transaction));
        schedule.add_systems(c516_after.after(transaction).before(c458_sample).before(TnuaPipelineSystems::Motors));
    });
}

fn c516_delta(probe:Res<C458Probe>,control:Res<C516Control>,mut time:ResMut<TimeUpdateStrategy>) {
    if probe.frame==control.frame {
        *time=TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(0.06));
    } else if probe.frame==control.frame+1 {
        *time=TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0/30.0));
    }
}

fn c516_cache(probe:Res<C458Probe>,mut control:ResMut<C516Control>,dynamics:Query<&AlsGroundedDynamics>) {
    if probe.frame==control.frame {
        control.previous_acceleration=Some(dynamics.get(probe.actor.unwrap()).unwrap().0.acceleration);
    }
}

fn c516_before(probe:Res<C458Probe>,mut control:ResMut<C516Control>,mut assets:ResMut<Assets<AlsMovementParams>>,
    mut actors:Query<(
        &Position,&LocomotionState,&mut AlsGroundedDynamics,&AlsMovementBaseDynamicsState,
        &AlsCmcAdapterRuntime,&AlsMovementParamsHandle,
)>) {
    if probe.frame!=control.frame {
        return;
    }
    let (p,mode,mut dynamics,base,runtime,handle)=actors.get_mut(probe.actor.unwrap()).unwrap();
    let fresh=dynamics.0.acceleration;
    let generic=assets.get(&handle.0).unwrap().acceleration.max_acceleration;
    let cached=control.previous_acceleration.unwrap();
    match control.mode.as_str() {
        "walking-cached" => {
            assert_eq!(*mode,LocomotionState::Grounded);
            assert_eq!(runtime.simulation_substeps,2);
            dynamics.0.acceleration=cached;
            control.restore=Some(fresh);
        }
        "air-cached" => {
            assert_eq!(*mode,LocomotionState::Grounded);
            assert_eq!(runtime.simulation_substeps,2);
            assert!(cached<generic);
            assets.get_mut(&handle.0).unwrap().acceleration.max_acceleration=cached;
            control.restore=Some(generic);
        }
        _ => {}
    }
    c516_write("stage-before.json",serde_json::json!({"frame":control.frame,"position":p.0,"mode":mode,
        "cached_world_velocity":base.world_velocity,"relative":base.relative_velocity,"cached_acceleration":cached,
        "support":base.support_velocity,"fresh_acceleration":fresh,
        "walking_acceleration":dynamics.0.acceleration,
        "air_maximum":assets.get(&handle.0).unwrap().acceleration.max_acceleration,
        "planned_velocity":base.pre_collision_velocity,"planned_move":runtime.grounded_movement_delta,
        "substeps":runtime.simulation_substeps,"delta":runtime.simulation_delta_seconds}));
}

fn c516_after(probe:Res<C458Probe>,control:Res<C516Control>,mut assets:ResMut<Assets<AlsMovementParams>>,
    mut actors:Query<(
        &Transform,&LocomotionState,&AlsMovementBaseDynamicsState,&AlsCmcAdapterRuntime,
        &mut AlsGroundedDynamics,&AlsMovementParamsHandle,
)>) {
    if probe.frame!=control.frame {
        return;
    }
    let (transform,mode,base,runtime,mut dynamics,handle)=actors.get_mut(probe.actor.unwrap()).unwrap();
    match control.mode.as_str() {
        "walking-cached" => dynamics.0.acceleration=control.restore.unwrap(),
        "air-cached" => assets.get_mut(&handle.0).unwrap().acceleration.max_acceleration=control.restore.unwrap(),
        _ => {}
    }
    c516_write("stage-after.json",serde_json::json!({"frame":control.frame,"position":transform.translation,
        "mode":mode,"walking_last_input":base.pre_collision_velocity,
        "collision_velocity":runtime.grounded_collision_velocity,"published_grounded_acceleration":dynamics.0.acceleration,
        "substeps":runtime.simulation_substeps,"delta":runtime.simulation_delta_seconds}));
}
