#[derive(Resource)]
struct C523Air {
    position: Vec3,
    velocity: Vec3,
    step: f32,
}

fn c523_install(app: &mut App) {
    app.insert_resource(C523Air {
        position: Vec3::new(-13.0,4.0,-15.0),
        velocity: Vec3::new(4.0,-1.0,4.0),
        step: 0.05,
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
    actors:Query<(&Position,&Transform,&Collider,&LinearVelocity,&TnuaMotor,&AlsMovementParamsHandle,
        &TnuaController<AlsControlScheme>)>,
    gravity:Res<Gravity>,
    spatial:SpatialQuery,
    assets:Res<Assets<AlsMovementParams>>,
    candidates:Query<(Option<&CollisionLayers>,Has<Sensor>,Option<&ColliderOf>,
        Option<&bevy_als::als::character::AlsCollisionLayerOwnership>)>,
    names:Query<&Name>,
) {
    if probe.frame!=control.frame {return;}
    let actor=probe.actor.unwrap();
    let (position,transform,capsule,velocity,motor,handle,controller)=actors.get(actor).unwrap();
    assert_eq!(controller.basis.desired_motion,Vec3::ZERO);
    let params=assets.get(handle.id()).unwrap();
    let initial=velocity.0+motor.lin.boost;
    let step=time.delta_secs();
    let g=gravity.0+motor.lin.acceleration;
    let incoming=initial+g*step;
    let mut remaining=(initial+incoming)*(0.5*step);
    let filter=SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts=|entity| bevy_als::als::motion::character_blocks_collider(actor,entity,&candidates);
    let cast=|at,direction,distance| {
        let config=ShapeCastConfig {ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(distance)};
        spatial.cast_shape_predicate(capsule,at,transform.rotation,direction,&config,&filter,&accepts)
    };
    let mut remaining_time=step;
    let mut current=incoming;
    let mut previous_normal=None;
    let mut first_normal=None;
    let mut applied=Vec3::ZERO;
    let mut hits=Vec::new();
    for index in 0..3 {
        let (direction,distance)=Dir3::new_and_length(remaining).unwrap();
        let start=position.0+applied;
        let hit=cast(start,direction,distance).unwrap();
        let normal=hit.normal1.try_normalize().unwrap();
        let travel=bevy_als::als::motion::movement_sweep_travel(distance,hit.distance);
        applied+=*direction*travel;
        let stop=position.0+applied;
        remaining_time*=1.0-travel/distance;
        hits.push(serde_json::json!({"name":names.get(hit.entity).unwrap().as_str(),
            "normal":normal,"start":start,"request":remaining,"stop":stop,
            "distance":hit.distance,"travel":travel,"time_left":remaining_time}));
        if index==0 {first_normal=Some(normal);}
        if index==2 {
            let extent=capsule.shape_scaled().compute_local_aabb().half_extents();
            let radius=extent.x.min(extent.z);
            let floor=bevy_als::als::motion::find_ground_floor(&spatial,stop,extent.y,radius,&params.cmc,
                LocomotionState::Air,&filter,actor,&candidates);
            let landed=floor.is_some_and(|(center,impact,_)|
                (stop.y-center).abs()<=0.024 && impact<stop.y-extent.y+radius);
            let side=(first_normal.unwrap()+Vec3::Y).with_y(0.0).normalize_or_zero()*0.01;
            let (direction,distance)=Dir3::new_and_length(side).unwrap();
            let blocked=cast(stop,direction,distance).map(|hit|names.get(hit.entity).unwrap().as_str());
            c516_write("third-contact.json",serde_json::json!({"step":step,"initial":initial,"gravity":g,
                "hits":hits,"third_velocity":current,"landing_candidate":landed,
                "floor":floor.map(|(center,impact,entity)|(center,impact,names.get(entity).unwrap().as_str())),
                "side":side,"side_blocked":blocked}));
            break;
        }
        let budget=current*remaining_time;
        let slide=bevy_als::extensions::motion::falling_slide(budget,normal,previous_normal);
        previous_normal=Some(normal);
        current=slide/remaining_time;
        remaining=slide;
    }
}
