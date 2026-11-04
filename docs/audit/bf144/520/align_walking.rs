#[derive(Resource,Default)]
struct C520Restore(Option<(Vec3,f32,Option<f32>)>);

fn c520_install(app:&mut App) {
    app.init_resource::<C520Restore>();
    app.edit_schedule(Update,|schedule| {
        let transaction={
            let systems:Vec<_>=schedule.graph().systems.iter().filter(|(_,system,_)|
                system.name().to_string().ends_with("::cmc_movement_transaction_system")).collect();
            assert_eq!(systems.len(),1);
            let sets=systems[0].1.default_system_sets();
            assert_eq!(sets.len(),1);
            sets[0]
        };
        schedule.add_systems(c520_before.after(c516_before).before(transaction));
        schedule.add_systems(c520_restore.after(transaction).before(c516_after).before(c518_control));
    });
}

fn c520_before(
    probe:Res<C458Probe>,
    fixture:Res<C516Control>,
    mut restore:ResMut<C520Restore>,
    mut assets:ResMut<Assets<AlsMovementParams>>,
    mut actors:Query<(&mut Position,&mut Transform,&mut AlsMovementBaseDynamicsState,&mut AlsGroundedDynamics,&AlsMovementParamsHandle)>,
) {
    if probe.frame!=fixture.frame {return;}
    let Ok(label)=std::env::var("ALS_C520_ALIGN") else {return;};
    let dir=std::path::PathBuf::from(std::env::var("ALS_C458_OUTPUT").unwrap());
    let placement:serde_json::Value=serde_json::from_slice(&std::fs::read(dir.with_file_name("fixture.json")).unwrap()).unwrap();
    let input:serde_json::Value=serde_json::from_slice(
        &std::fs::read("docs/audit/bf144/520/alignment-input.json").unwrap()).unwrap();
    let (mut position,mut transform,mut base,mut dynamics,handle)=actors.get_mut(probe.actor.unwrap()).unwrap();
    let original=position.0;
    let origin=placement["origin"][0].as_f64().unwrap() as f32;
    position.0.x=origin+input["relative_x"].as_f64().unwrap() as f32;
    transform.translation.x=position.0.x;
    let maximum=assets.get(&handle.0).unwrap().acceleration.max_acceleration;
    let source_acceleration=input["cached_acceleration"].as_f64().unwrap() as f32;
    if matches!(label.as_str(),"wall-162-aligned-walking"|"wall-162-aligned-air") {
        let air=label=="wall-162-aligned-air";
        restore.0=Some((base.relative_velocity,dynamics.0.acceleration,air.then_some(maximum)));
        base.relative_velocity.x=input["velocity_x"].as_f64().unwrap() as f32;
        dynamics.0.acceleration=source_acceleration;
        if air {
            assets.get_mut(&handle.0).unwrap().acceleration.max_acceleration=source_acceleration;
        }
    }
    c516_write("alignment.json",serde_json::json!({"original_position":original,"aligned_position":position.0,
         "walking_velocity":base.relative_velocity,"walking_acceleration":dynamics.0.acceleration,
        "air_maximum":assets.get(&handle.0).unwrap().acceleration.max_acceleration}));
}

fn c520_restore(
    probe:Res<C458Probe>,
    mut restore:ResMut<C520Restore>,
    mut assets:ResMut<Assets<AlsMovementParams>>,
    mut actors:Query<(&mut AlsMovementBaseDynamicsState,&mut AlsGroundedDynamics,&AlsMovementParamsHandle)>,
) {
    let Some((velocity,acceleration,air))=restore.0.take() else {return;};
    let (mut base,mut dynamics,handle)=actors.get_mut(probe.actor.unwrap()).unwrap();
    base.relative_velocity=velocity;
    dynamics.0.acceleration=acceleration;
    if let Some(maximum)=air {
        assets.get_mut(&handle.0).unwrap().acceleration.max_acceleration=maximum;
    }
}
