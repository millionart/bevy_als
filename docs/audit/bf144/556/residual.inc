#[derive(Resource)]
struct C523Air {
    position: Vec3,
    velocity: Vec3,
    step: f32,
}

fn c523_install(app: &mut App) {
    let vx=std::env::var("ALS_C556_VX").unwrap().parse::<f32>().unwrap();
    let vz=std::env::var("ALS_C556_VZ").unwrap().parse::<f32>().unwrap();
    app.insert_resource(C523Air {
        position: Vec3::new(-13.0,4.0,-15.0),
        velocity: Vec3::new(vx,-0.2,vz),
        step: 0.06,
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
    actors:Query<(&Position,&LinearVelocity,&LocomotionState,&TnuaController<AlsControlScheme>,&TnuaMotor)>,
    gravity:Res<Gravity>,
) {
    if probe.frame!=control.frame {return;}
    let (position,velocity,mode,controller,motor)=actors.get(probe.actor.unwrap()).unwrap();
    c516_write("air-start.json",serde_json::json!({"position":position.0,"velocity":velocity.0,
        "mode":mode,"step":time.delta_secs(),"desired":controller.basis.desired_motion,
        "boost":motor.lin.boost,"effective_gravity":gravity.0+motor.lin.acceleration}));
}
