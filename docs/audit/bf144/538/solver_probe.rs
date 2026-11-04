use avian3d::dynamics::solver::schedule::SubstepSolverSystems;

#[derive(Resource, Default)]
struct C538Trace {
    saved: Option<(Entity, CollisionLayers)>,
    steps: Vec<serde_json::Value>,
}

fn c538_install(app: &mut App) {
    app.init_resource::<C538Trace>();
    app.add_systems(
        Update,
        c538_control
            .after(c533_after_transaction)
            .before(TnuaPipelineSystems::Motors),
    );
    app.add_systems(
        PhysicsSchedule,
        c538_prepare
            .after(SolverSystems::PreSubstep)
            .before(SolverSystems::Substep),
    );
    app.edit_schedule(SubstepSchedule, |schedule| {
        let dependency = |suffix: &str| {
            let matches: Vec<_> = schedule
                .graph()
                .systems
                .iter()
                .filter(|(_, system, _)| system.name().to_string().ends_with(suffix))
                .collect();
            assert_eq!(matches.len(), 1);
            matches[0].1.default_system_sets()[0]
        };
        let joint = dependency("::solve_source_joint_velocity_iterations");
        let profile = dependency("::solve_profile_dof");
        schedule.add_systems(
            c538_substep
                .after(SubstepSolverSystems::Damping)
                .after(joint)
                .after(profile),
        );
    });
    app.add_systems(
        PhysicsSchedule,
        c538_impulses
            .after(SolverSystems::StoreContactImpulses)
            .before(PhysicsStepSystems::Finalize),
    );
    app.add_systems(PostUpdate, c538_restore.after(c522_after));
}

fn c538_control(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    mut trace: ResMut<C538Trace>,
    actor: Query<(&Transform, &LinearVelocity, &TnuaMotor)>,
    geometry: Query<(Entity, &Name, &CollisionLayers)>,
    mut commands: Commands,
) {
    if probe.frame != control.frame {
        return;
    }
    let (transform, velocity, motor) = actor.get(probe.actor.unwrap()).unwrap();
    let mask = std::env::var_os("ALS_C538_MASK_FIN").is_some();
    if mask {
        let (entity, _, layers) = geometry
            .iter()
            .find(|(_, name, _)| name.as_str() == "AuditStepFin")
            .unwrap();
        trace.saved = Some((entity, *layers));
        commands.entity(entity).insert(CollisionLayers::NONE);
    }
    c516_write(
        "solver-control.json",
        serde_json::json!({
            "position":transform.translation,"velocity":velocity.0,"boost":motor.lin.boost,
            "acceleration":motor.lin.acceleration,"masked_fin":mask,
        }),
    );
}

fn c538_prepare(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    mut trace: ResMut<C538Trace>,
    bodies: Query<&SolverBody>,
    contacts: Res<ContactGraph>,
    names: Query<&Name>,
) {
    if probe.frame != control.frame + 1 {
        return;
    }
    let actor = probe.actor.unwrap();
    let body = bodies.get(actor).unwrap();
    trace.steps.clear();
    c516_write(
        "solver-entry.json",
        serde_json::json!({
            "velocity":body.linear_velocity,"position_delta":body.delta_position,
            "contacts":c538_contacts(actor,&contacts,&names),
        }),
    );
}

fn c538_contacts(
    actor: Entity,
    contacts: &ContactGraph,
    names: &Query<&Name>,
) -> Vec<serde_json::Value> {
    contacts.contact_pairs_with(actor).filter_map(|pair| {
        let other=if pair.collider1==actor {pair.collider2} else {pair.collider1};
        let name=names.get(other).ok()?;
        Some(serde_json::json!({"entity":name.as_str(),
            "constraints":pair.flags.contains(ContactPairFlags::GENERATE_CONSTRAINTS),
            "manifolds":pair.manifolds.iter().map(|manifold|serde_json::json!({
                "normal":if pair.collider1==actor {-manifold.normal} else {manifold.normal},
                "impulses":manifold.points.iter().map(|point| point.normal_impulse).collect::<Vec<_>>(),
            })).collect::<Vec<_>>()
        }))
    }).collect()
}

fn c538_substep(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    mut trace: ResMut<C538Trace>,
    bodies: Query<&SolverBody>,
) {
    if probe.frame != control.frame + 1 {
        return;
    }
    let body = bodies.get(probe.actor.unwrap()).unwrap();
    trace.steps.push(
        serde_json::json!({"velocity":body.linear_velocity,"position_delta":body.delta_position}),
    );
}

fn c538_impulses(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    trace: Res<C538Trace>,
    bodies: Query<(&Position, &LinearVelocity)>,
    contacts: Res<ContactGraph>,
    names: Query<&Name>,
) {
    if probe.frame != control.frame + 1 {
        return;
    }
    let actor = probe.actor.unwrap();
    let (position, velocity) = bodies.get(actor).unwrap();
    c516_write(
        "solver-exit.json",
        serde_json::json!({
            "position":position.0,"velocity":velocity.0,"steps":trace.steps,
            "contacts":c538_contacts(actor,&contacts,&names),
        }),
    );
}

fn c538_restore(mut trace: ResMut<C538Trace>, mut commands: Commands) {
    if let Some((entity, original)) = trace.saved.take() {
        commands.entity(entity).insert(original);
    }
}
