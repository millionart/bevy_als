use bevy_als::als::{
    dynamics::AlsMovementBase,
    motion::AlsActualVelocity,
    transition::{AlsStateRequest, Dimension, StateTransitionEvent},
};
use bevy_tnua::{basis_capabilities::TnuaBasisWithDisplacement, builtins::TnuaBuiltinWalk};

const C522_CLEAR_CONTACT: bool = false;

fn c522_install(app: &mut App) {
    app.add_systems(
        PostUpdate,
        (
            c522_before
                .after(PhysicsSystems::Last)
                .before(AlsActualVelocitySet),
            c522_after.after(AlsActualVelocitySet),
        ),
    );
}

fn c522_before(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    mut actors: Query<(
        &Position,
        &LinearVelocity,
        &LocomotionState,
        &Stance,
        &AlsStateRequest,
        &AlsMovementParamsHandle,
        &TnuaController<AlsControlScheme>,
        &mut AlsCmcAdapterRuntime,
    )>,
    assets: Res<Assets<AlsMovementParams>>,
    spatial: SpatialQuery,
    candidates: Query<(
        Option<&CollisionLayers>,
        Has<Sensor>,
        Option<&ColliderOf>,
        Option<&bevy_als::als::character::AlsCollisionLayerOwnership>,
    )>,
    collisions: Collisions,
    transforms: bevy::transform::helper::TransformHelper,
    names: Query<&Name>,
) {
    let frame = probe.frame.saturating_sub(1);
    if !(control.frame..=control.frame + 1).contains(&frame) {
        return;
    }
    let actor = probe.actor.unwrap();
    let (position, velocity, mode, stance, request, handle, controller, mut runtime) =
        actors.get_mut(actor).unwrap();
    let params = assets.get(handle.id()).unwrap();
    let scale = transforms
        .compute_global_transform(actor)
        .unwrap()
        .to_scale_rotation_translation()
        .0;
    let half = if *stance == Stance::Crouching {
        params.crouch.crouched_capsule_half_height
    } else {
        params.crouch.standing_capsule_half_height
    } * scale.y;
    let radius = params.crouch.standing_capsule_radius * scale.x.min(scale.z);
    let floor = bevy_als::als::motion::find_ground_floor(
        &spatial,
        position.0,
        half,
        radius,
        &params.cmc,
        *mode,
        &SpatialQueryFilter::from_excluded_entities([actor]),
        actor,
        &candidates,
    )
    .map(|(center, impact, entity)| {
        serde_json::json!({
            "center":center,"impact":impact,"name":names.get(entity).ok().map(|name|name.as_str()),
            "gap":position.y-center,"lower_hemisphere":position.y-half+radius,
        })
    });
    let manifolds:Vec<_>=collisions.collisions_with(actor).flat_map(|pair| {
        let other=if pair.collider1==actor {pair.collider2} else {pair.collider1};
        pair.manifolds.iter().map(move |manifold| (pair, other, manifold))
    }).map(|(pair, other, manifold)| serde_json::json!({
        "other":names.get(other).ok().map(|name|name.as_str()),
        "constraints":pair.flags.contains(ContactPairFlags::GENERATE_CONSTRAINTS),
        "normal":if pair.collider1==actor {-manifold.normal} else {manifold.normal},
        "points":manifold.points.iter().map(|point| [point.normal_speed,point.normal_impulse]).collect::<Vec<_>>(),
    })).collect();
    let contact = runtime.airborne_sweep_contact;
    if C522_CLEAR_CONTACT && frame == control.frame {
        runtime.airborne_sweep_contact = false;
    }
    let basis = controller.basis_access().unwrap();
    c516_write(
        &format!("contact-before-{frame}.json"),
        serde_json::json!({
            "ordinal":frame,"mode":mode,"position":position.0,"raw_velocity":velocity.0,
            "planned_velocity":runtime.grounded_collision_velocity,
            "requested_mode":request.locomotion_state,"has_basis_displacement":TnuaBuiltinWalk::displacement(&basis).is_some(),
            "floor":floor,"manifolds":manifolds,"generated_contact":contact,"consumed_contact":runtime.airborne_sweep_contact,
        }),
    );
}

fn c522_after(
    probe: Res<C458Probe>,
    control: Res<C516Control>,
    actors: Query<(
        &Position,
        &LinearVelocity,
        &LocomotionState,
        &AlsCmcAdapterRuntime,
        &AlsActualVelocity,
        &AlsMovementBase,
    )>,
    mut transitions: MessageReader<StateTransitionEvent>,
) {
    let frame = probe.frame.saturating_sub(1);
    let changes: Vec<_> = transitions
        .read()
        .filter(|event| {
            Some(event.entity) == probe.actor && event.dimension == Dimension::LocomotionState
        })
        .map(|event| [format!("{:?}", event.from), format!("{:?}", event.to)])
        .collect();
    if !(control.frame..=control.frame + 1).contains(&frame) {
        return;
    }
    let (position, velocity, mode, runtime, actual, base) =
        actors.get(probe.actor.unwrap()).unwrap();
    c516_write(
        &format!("contact-after-{frame}.json"),
        serde_json::json!({
            "ordinal":frame,"mode":mode,"position":position.0,"velocity":velocity.0,
            "published":actual.world_velocity,"has_base":base.entity.is_some(),
            "stored_contact":runtime.airborne_sweep_contact,"transitions":changes,
        }),
    );
}
