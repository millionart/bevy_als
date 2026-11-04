#[derive(Resource)]
struct C523Air {
    position: Vec3,
    velocity: Vec3,
    step: f32,
}

fn c523_install(app: &mut App) {
    app.insert_resource(C523Air {
        position: Vec3::new(-13.0,4.0,-15.0),
        velocity: Vec3::new(4.0,-2.0,2.0),
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
        &TnuaController<AlsControlScheme>,&Stance,&RotationMode,&bevy_als::als::gait::AlsMaxAllowedGait,
        &bevy_als::als::motion::AlsViewYaw)>,
    gravity:Res<Gravity>,
    spatial:SpatialQuery,
    assets:Res<Assets<AlsMovementParams>>,
    candidates:Query<(Option<&CollisionLayers>,Has<Sensor>,Option<&ColliderOf>,
        Option<&bevy_als::als::character::AlsCollisionLayerOwnership>)>,
    names:Query<&Name>,
) {
    if probe.frame!=control.frame {return;}
    let actor=probe.actor.unwrap();
    let (position,transform,capsule,velocity,motor,handle,controller,stance,mode,gait,yaw)=actors.get(actor).unwrap();
    let params=assets.get(handle.id()).unwrap();
    let initial=velocity.0+motor.lin.boost;
    let step=time.delta_secs();
    let g=gravity.0+motor.lin.acceleration;
    let maximum=bevy_als::als::dynamics::max_allowed_speed(params,*stance,*mode,gait.0,initial.with_y(0.0),yaw.0);
    let acceleration=bevy_als::als::motion::air_acceleration_for_horizontal_speed(params,initial.with_y(0.0).length())
        .min(params.acceleration.max_acceleration);
    let desired=controller.basis.desired_motion;
    let horizontal=bevy_als::als::dynamics::grounded_velocity_step_with_braking_friction(initial.with_y(0.0),
        desired,maximum,bevy_als::als::params::GroundedDynamics {acceleration,braking_deceleration:0.0,ground_friction:0.0},
        params.cmc.min_analog_walk_speed,0.0,params.cmc.braking_substep_time,step);
    let no_input=initial+g*step;
    let incoming=horizontal+Vec3::Y*initial.y+g*step;
    let first_request=(initial+incoming)*(0.5*step);
    let filter=SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts=|entity| bevy_als::als::motion::character_blocks_collider(actor,entity,&candidates);
    let mut variants=serde_json::Map::new();
    for (label,drop_second_input) in [("actual",false),("without-second-input",true)] {
        let mut remaining=first_request;
        let mut remaining_time=step;
        let mut current=incoming;
        let mut previous_normal=None;
        let mut applied=Vec3::ZERO;
        let mut hits=Vec::new();
        for _ in 0..3 {
            let Ok((direction,distance))=Dir3::new_and_length(remaining) else {break;};
            let config=ShapeCastConfig {ignore_origin_penetration:true,..ShapeCastConfig::from_max_distance(distance)};
            let start=position.0+applied;
            let Some(hit)=spatial.cast_shape_predicate(capsule,start,transform.rotation,direction,&config,&filter,&accepts)
                else {applied+=remaining;break;};
            let travel=bevy_als::als::motion::movement_sweep_travel(distance,hit.distance);
            applied+=*direction*travel;
            let normal=hit.normal1.try_normalize().unwrap();
            remaining_time*=1.0-travel/distance;
            let budget=if drop_second_input && hits.len()==1 && normal.y > 0.001 {
                bevy_als::extensions::motion::falling_slide(no_input,previous_normal.unwrap(),None)*remaining_time
            } else {current*remaining_time};
            let slide=bevy_als::extensions::motion::falling_slide(budget,normal,previous_normal);
            hits.push(serde_json::json!({"name":names.get(hit.entity).unwrap().as_str(),
                "normal":normal,"start":start,"request":remaining,"stop":position.0+applied,
                "travel":travel,"time_left":remaining_time,"slide":slide}));
            previous_normal=Some(normal);
            if remaining_time>f32::EPSILON {current=slide/remaining_time;}
            if slide.dot(budget)<=0.0 {break;}
            remaining=slide;
        }
        variants.insert(label.to_owned(),serde_json::json!({"hits":hits,"end":position.0+applied,"velocity":current}));
    }
    c516_write("corner-replay.json",serde_json::json!({"step":step,"position":position.0,"initial":initial,
        "gravity":g,"desired":desired,"incoming":incoming,"no_input":no_input,"variants":variants}));
}
