#[derive(Resource)]
struct C523Air {
    position: Vec3,
    velocity: Vec3,
    step: f32,
}

fn c523_install(app: &mut App) {
    app.insert_resource(C523Air {
        position: Vec3::new(-13.0,4.0,-15.0),
        velocity: Vec3::new(2.0,0.0,0.0),
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
    actors:Query<(&Position,&Collider,&LinearVelocity,&LocomotionState,&TnuaController<AlsControlScheme>,
        &TnuaMotor,&AlsMovementParamsHandle,&Stance,&RotationMode,&bevy_als::als::gait::AlsMaxAllowedGait,
        &bevy_als::als::motion::AlsViewYaw)>,
    assets:Res<Assets<AlsMovementParams>>,
) {
    if probe.frame!=control.frame {return;}
    let (position,capsule,velocity,mode,controller,motor,handle,stance,rotation,gait,yaw)=actors.get(probe.actor.unwrap()).unwrap();
    let params=assets.get(handle.id()).unwrap();
    let initial=velocity.0+motor.lin.boost;
    let horizontal=initial.with_y(0.0);
    let desired=controller.basis.desired_motion;
    let step=time.delta_secs();
    let maximum=bevy_als::als::dynamics::max_allowed_speed(params,*stance,*rotation,gait.0,horizontal,yaw.0);
    let accel=bevy_als::als::motion::air_acceleration_for_horizontal_speed(params,horizontal.length())
        .min(params.acceleration.max_acceleration);
    let replay=|cap|bevy_als::als::dynamics::grounded_velocity_step_with_braking_friction(horizontal,
        desired,cap,bevy_als::als::params::GroundedDynamics {acceleration:accel,braking_deceleration:0.0,ground_friction:0.0},
        params.cmc.min_analog_walk_speed,0.0,params.cmc.braking_substep_time,step);
    c516_write("air-speed.json",serde_json::json!({"position":position.0,"mode":mode,"stance":stance,
        "half_m":capsule.shape_scaled().compute_local_aabb().half_extents().y,"gait":gait.0,
        "initial":initial,"step":step,"desired":desired,"live_cap":maximum,"acceleration":accel,
        "live_speed":replay(maximum),"crouch_cap_speed":replay(2.0)}));
}
