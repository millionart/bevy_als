#[derive(Resource)]
struct C523Air {
    position: Vec3,
    velocity: Vec3,
    step: f32,
    maximum: f32,
    original_maximum: Option<f32>,
}

fn c523_install(app: &mut App) {
    let bytes = std::fs::read("docs/audit/bf144/523/initial.json").unwrap();
    let input: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    app.insert_resource(C523Air {
        position: serde_json::from_value(input["position_m"].clone()).unwrap(),
        velocity: serde_json::from_value(input["velocity_mps"].clone()).unwrap(),
        step: input["delta_s"].as_f64().unwrap() as f32,
        maximum: input["air_maximum_mps2"].as_f64().unwrap() as f32,
        original_maximum: None,
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
        schedule.add_systems(c523_restore.after(transaction).before(c458_sample));
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
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    mut air: ResMut<C523Air>,
    time: Res<Time>,
    mut assets: ResMut<Assets<AlsMovementParams>>,
    actors: Query<(
        &Position,
        &LinearVelocity,
        &Transform,
        &LocomotionState,
        &AlsMovementParamsHandle,
        &Collider,
        &TnuaController<AlsControlScheme>,
        &TnuaMotor,
        &Stance,
        &RotationMode,
        &bevy_als::als::gait::AlsMaxAllowedGait,
        &bevy_als::als::motion::AlsViewYaw,
    )>,
    gravity: Res<Gravity>,
    spatial: SpatialQuery,
    candidates: Query<(
        Option<&CollisionLayers>,
        Has<Sensor>,
        Option<&ColliderOf>,
        Option<&bevy_als::als::character::AlsCollisionLayerOwnership>,
    )>,
    names: Query<&Name>,
) {
    if probe.frame != control.frame {
        return;
    }
    let actor = probe.actor.unwrap();
    let (
        position,
        velocity,
        transform,
        mode,
        handle,
        capsule,
        controller,
        motor,
        stance,
        rotation,
        gait,
        yaw,
    ) = actors.get(actor).unwrap();
    let mut params = assets.get_mut(handle.id()).unwrap();
    air.original_maximum = Some(params.acceleration.max_acceleration);
    params.acceleration.max_acceleration = air.maximum;
    let start = position.0;
    let initial = velocity.0 + motor.lin.boost;
    let maximum = bevy_als::als::dynamics::max_allowed_speed(
        &params,
        *stance,
        *rotation,
        gait.0,
        initial.with_y(0.0),
        yaw.0,
    );
    let step = time.delta_secs();
    let horizontal = bevy_als::als::dynamics::grounded_velocity_step_with_braking_friction(
        initial.with_y(0.0),
        controller.basis.desired_motion,
        maximum,
        bevy_als::als::params::GroundedDynamics {
            acceleration: air.maximum * 0.15,
            braking_deceleration: 0.0,
            ground_friction: 0.0,
        },
        params.cmc.min_analog_walk_speed,
        0.0,
        params.cmc.braking_substep_time,
        step,
    );
    let incoming = horizontal + Vec3::Y * initial.y + (gravity.0 + motor.lin.acceleration) * step;
    let request = (initial + incoming) * (0.5 * step);
    let filter = SpatialQueryFilter::from_excluded_entities([actor]);
    let accepts =
        |candidate| bevy_als::als::motion::character_blocks_collider(actor, candidate, &candidates);
    let (direction, length) = Dir3::new_and_length(request).unwrap();
    let config = ShapeCastConfig {
        ignore_origin_penetration: true,
        ..ShapeCastConfig::from_max_distance(length)
    };
    let hit = spatial
        .cast_shape_predicate(
            capsule,
            start,
            transform.rotation,
            direction,
            &config,
            &filter,
            &accepts,
        )
        .unwrap();
    let travel = bevy_als::als::motion::movement_sweep_travel(length, hit.distance);
    let remaining = step * (1.0 - travel / length);
    let normal = hit.normal1.try_normalize().unwrap();
    let slide = bevy_als::extensions::motion::falling_slide(incoming * remaining, normal, None);
    let (slide_direction, slide_length) = Dir3::new_and_length(slide).unwrap();
    let hit_center = start + *direction * travel;
    let slide_config = ShapeCastConfig {
        max_distance: slide_length,
        ..config
    };
    let second = spatial.cast_shape_predicate(
        capsule,
        hit_center,
        transform.rotation,
        slide_direction,
        &slide_config,
        &filter,
        &accepts,
    );
    let name = |entity| names.get(entity).ok().map(|name| name.as_str());
    c516_write(
        "initial-observed.json",
        serde_json::json!({
            "position":start,"velocity":velocity.0,"mode":mode,"step":step,
            "initial_with_motor":initial,"gravity":gravity.0+motor.lin.acceleration,"incoming":incoming,
            "desired":controller.basis.desired_motion,"request":request,"wall":name(hit.entity),
            "normal":normal,"hit_distance":hit.distance,"second_hit":second.map(|h|(name(h.entity),h.distance)),
            "single_hit_position":start+(*direction*travel+slide),"single_hit_velocity":slide/remaining,
        }),
    );
}

fn c523_restore(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    mut air: ResMut<C523Air>,
    actors: Query<&AlsMovementParamsHandle>,
    mut assets: ResMut<Assets<AlsMovementParams>>,
) {
    if probe.frame != control.frame {
        return;
    }
    let handle = actors.get(probe.actor.unwrap()).unwrap();
    assets
        .get_mut(handle.id())
        .unwrap()
        .acceleration
        .max_acceleration = air.original_maximum.take().unwrap();
}
