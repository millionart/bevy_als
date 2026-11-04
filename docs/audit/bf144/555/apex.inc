#[derive(Resource)]
struct C523Air {
    position: Vec3,
    velocity: Vec3,
    step: f32,
}

fn c523_install(app: &mut App) {
    let bytes = std::fs::read("docs/audit/bf144/555/initial.json").unwrap();
    let input: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    app.insert_resource(C523Air {
        position: serde_json::from_value(input["position_m"].clone()).unwrap(),
        velocity: serde_json::from_value(input["velocity_mps"].clone()).unwrap(),
        step: input["delta_s"].as_f64().unwrap() as f32,
    });
    app.add_systems(First, c523_begin);
    app.add_systems(Last, c523_delta.after(c516_delta));
    app.edit_schedule(Update, |schedule| {
        let transaction = {
            let systems: Vec<_> = schedule
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
            assert_eq!(systems.len(), 1);
            systems[0].1.default_system_sets()[0]
        };
        schedule.add_systems(c523_before.after(c516_before).before(transaction));
    });
}

fn c523_begin(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    air: Res<C523Air>,
    mut actors: Query<(
        &mut Position,
        &mut Transform,
        &mut LinearVelocity,
        &mut LocomotionState,
    )>,
) {
    if probe.frame != control.frame {
        return;
    }
    let (mut position, mut transform, mut velocity, mut mode) =
        actors.get_mut(probe.actor.unwrap()).unwrap();
    position.0 = air.position;
    transform.translation = air.position;
    velocity.0 = air.velocity;
    *mode = LocomotionState::Air;
}

fn c523_delta(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    air: Res<C523Air>,
    mut clock: ResMut<TimeUpdateStrategy>,
) {
    if probe.frame == control.frame {
        *clock = TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(air.step));
    }
}

fn c523_before(
    probe:Res<C458Probe>,
    control:Res<C516Control>,
    time:Res<Time>,
    actors:Query<(&Position,&LinearVelocity,&Transform,&LocomotionState,&Collider,
        &TnuaController<AlsControlScheme>,&TnuaMotor)>,
    gravity:Res<Gravity>,
    spatial:SpatialQuery,
    candidates:Query<(Option<&CollisionLayers>,Has<Sensor>,Option<&ColliderOf>,Option<&bevy_als::als::character::AlsCollisionLayerOwnership>)>,
    names:Query<&Name>,
) {
    if probe.frame!=control.frame {return;}
    let actor=probe.actor.unwrap();
    let (position,velocity,transform,mode,capsule,controller,motor)=actors.get(actor).unwrap();
    let initial=velocity.0+motor.lin.boost;
    let effective_gravity=gravity.0+motor.lin.acceleration;
    let step=time.delta_secs();
    let end_velocity=initial+effective_gravity*step;
    let full=(initial+end_velocity)*(0.5*step);
    let apex_time=-initial.y/effective_gravity.y;
    let ascent=Vec3::Y*(initial.y*apex_time*0.5);
    let filter=SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts=|entity|bevy_als::als::motion::character_blocks_collider(actor,entity,&candidates);
    let cast=|delta| {
        let (direction,distance)=Dir3::new_and_length(delta).unwrap();
        let config=ShapeCastConfig {ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(distance)};
        spatial.cast_shape_predicate(capsule,position.0,transform.rotation,direction,&config,&filter,&accepts)
            .map(|hit|serde_json::json!({"name":names.get(hit.entity).unwrap().as_str(),
                "distance":hit.distance,"normal":hit.normal1,"point":hit.point1}))
    };
    c516_write("apex-queries.json",serde_json::json!({"position":position.0,"mode":mode,"step":step,
        "raw_velocity":velocity.0,"initial":initial,"gravity":effective_gravity,"desired":controller.basis.desired_motion,
        "full":full,"full_hit":cast(full),"apex_time":apex_time,"ascent":ascent,"ascent_hit":cast(ascent)}));
}
