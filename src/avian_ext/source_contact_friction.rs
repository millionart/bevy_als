use bevy::prelude::*;
use bevy::platform::collections::HashMap;
use bevy::ecs::component::Tick;
use avian3d::collision::contact_types::ContactGraph;
use avian3d::dynamics::solver::{constraint_graph::ConstraintGraph, contact::ContactTangentPart,
    solver_body::{SolverBody, SolverBodyInertia}, PhysicsLengthUnit, SolverConfig};
use avian3d::prelude::{Collider, ComputedCenterOfMass, DefaultFriction, Friction, Position, Rotation, Substeps};

use super::ragdoll_physics_profile::RagdollActiveMode;
use super::source_contact_history::{SourceShapeFrame, find_source_contact_anchor,
    save_source_contact_anchor, source_velocity_friction_ratio};

#[derive(Resource, Clone, Copy)]
pub(crate) struct SourceContactFrictionSettings {
    pub position_iterations: u32,
    pub position_stiffness: f32,
    pub tangent_velocity_squared: f32,
    pub tangent_geometry_squared: f32,
    pub history_exact_distance: f32,
    pub history_near_distance: f32,
    pub history_ratio_tolerance: f32,
}

#[derive(Clone, Copy)]
struct FreeBody {
    delta_position: Vec3,
    linear_velocity: Vec3,
    angular_velocity: Vec3,
    dynamic: bool,
}

impl Default for FreeBody {
    fn default() -> Self {
        Self { delta_position: Vec3::ZERO, linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO, dynamic: false }
    }
}

struct SourceFrictionPoint {
    color: usize,
    constraint: usize,
    point: usize,
    bodies: [Entity; 2],
    arms: [Vec3; 2],
    friction: SourcePositionFriction,
    target_tangent: Option<ContactTangentPart>,
    history: Option<SourceHistoryPoint>,
}

struct SourceHistoryPoint {
    contact: u32,
    colliders: [Entity; 2],
    bodies: [Entity; 2],
    shape_ticks: [Tick; 2],
    point_index: usize,
    point_count: usize,
    current: [Vec3; 2],
    anchors: [Vec3; 2],
}

struct SavedHistory {
    colliders: [Entity; 2],
    bodies: [Entity; 2],
    shape_ticks: [Tick; 2],
    anchors: Vec<[Vec3; 2]>,
}

impl SavedHistory {
    fn oriented(&self, colliders: [Entity; 2], bodies: [Entity; 2], ticks: [Tick; 2]) -> Option<Vec<[Vec3; 2]>> {
        if self.colliders == colliders && self.bodies == bodies && self.shape_ticks == ticks {
            Some(self.anchors.clone())
        } else if self.colliders == [colliders[1], colliders[0]]
            && self.bodies == [bodies[1], bodies[0]] && self.shape_ticks == [ticks[1], ticks[0]] {
            Some(self.anchors.iter().map(|anchors| [anchors[1], anchors[0]]).collect())
        } else {
            None
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct SourceContactFrictionFrame {
    bodies: HashMap<Entity, FreeBody>,
    points: Vec<SourceFrictionPoint>,
    history: HashMap<u32, SavedHistory>,
}

pub(crate) fn capture_source_friction_contacts(
    settings: Res<SourceContactFrictionSettings>,
    time: Res<Time<Substeps>>,
    ragdolls: Query<(), With<RagdollActiveMode>>,
    bodies: Query<&SolverBody>,
    body_poses: Query<(&Position, &Rotation, &ComputedCenterOfMass)>,
    colliders: Query<(Ref<Collider>, &Position, &Rotation)>,
    materials: Query<&Friction>,
    default_friction: Res<DefaultFriction>,
    contact_graph: Res<ContactGraph>,
    mut graph: ResMut<ConstraintGraph>,
    mut frame: ResMut<SourceContactFrictionFrame>,
) {
    frame.bodies.clear();
    frame.points.clear();
    if settings.position_iterations == 0 || settings.position_stiffness <= 0.0 { return; }
    let dt = time.delta_secs();
    for (color_index, color) in graph.colors.iter_mut().enumerate() {
        for (constraint_index, constraint) in color.contact_constraints.iter_mut().enumerate() {
            if !ragdolls.contains(constraint.body1) && !ragdolls.contains(constraint.body2) { continue; }
            let Some((_, pair)) = contact_graph.get_by_id(constraint.contact_id) else { continue; };
            let material = |collider, body| materials.get(collider).or_else(|_| materials.get(body))
                .map(|value| *value).unwrap_or(default_friction.0);
            let friction = material(pair.collider1, constraint.body1).combine(material(pair.collider2, constraint.body2));
            let entities = [constraint.body1, constraint.body2];
            let collider_entities = [pair.collider1, pair.collider2];
            let shape_data = |collider, body| {
                let (shape, position, rotation) = colliders.get(collider).ok()?;
                let (body_position, body_rotation, center) = body_poses.get(body).ok()?;
                let quadratic = shape.shape_scaled().as_ball().is_some() || shape.shape_scaled().as_capsule().is_some();
                Some((SourceShapeFrame { position: position.0, rotation: rotation.0 },
                    body_position.0 + body_rotation.0 * center.0, quadratic, shape.last_changed()))
            };
            let shape_data = match (shape_data(collider_entities[0], entities[0]), shape_data(collider_entities[1], entities[1])) {
                (Some(first), Some(second)) => Some(([first.0, second.0], [first.1, second.1],
                    [first.2, second.2], [first.3, second.3])),
                _ => None,
            };
            let saved = frame.history.get(&constraint.contact_id.0).map_or_else(Vec::new, |history| {
                let Some((_, _, _, ticks)) = shape_data else { return Vec::new(); };
                history.oriented(collider_entities, entities, ticks).unwrap_or_default()
            });
            let point_offset = pair.manifolds.iter().take(constraint.manifold_index).map(|manifold| manifold.points.len()).sum::<usize>();
            let point_count = pair.manifolds.iter().map(|manifold| manifold.points.len()).sum::<usize>();
            let mut free = [FreeBody::default(); 2];
            for index in 0..2 {
                if let Ok(body) = bodies.get(entities[index]) {
                    free[index] = FreeBody { delta_position: body.delta_position,
                        linear_velocity: body.linear_velocity, angular_velocity: body.angular_velocity,
                        dynamic: !body.flags.is_kinematic() };
                    frame.bodies.entry(entities[index]).or_insert(free[index]);
                }
            }
            for (point_index, point) in constraint.points.iter_mut().enumerate() {
                let arms = [point.anchor1, point.anchor2];
                let relative = free[0].linear_velocity + free[0].angular_velocity.cross(arms[0])
                    - free[1].linear_velocity - free[1].angular_velocity.cross(arms[1]);
                let tangential = relative - constraint.normal * relative.dot(constraint.normal);
                let tangent = if tangential.length_squared() > settings.tangent_velocity_squared {
                    tangential.normalize()
                } else {
                    let first = Vec3::Z.cross(constraint.normal);
                    if first.length_squared() > settings.tangent_geometry_squared { first.normalize() }
                    else { Vec3::X.cross(constraint.normal).normalize() }
                };
                let tangents = [tangent, constraint.normal.cross(tangent)];
                let mut history = None;
                let friction_delta = if let Some((frames, centers, quadratic, shape_ticks)) = shape_data {
                    let current = [frames[0].local_point(centers[0] + arms[0]), frames[1].local_point(centers[1] + arms[1])];
                    let point_index = point_offset + point_index;
                    let matched = find_source_contact_anchor(&saved, current, point_index, quadratic,
                        settings.history_exact_distance, settings.history_near_distance);
                    let anchors = matched.map_or(current, |index| saved[index]);
                    history = Some(SourceHistoryPoint { contact: constraint.contact_id.0, colliders: collider_entities,
                        bodies: entities, shape_ticks, point_index, point_count, current, anchors });
                    if matched.is_some() {
                        let predicted = [0, 1].map(|index| frames[index].predicted(centers[index],
                            free[index].linear_velocity, free[index].angular_velocity, dt, free[index].dynamic));
                        predicted[0].world_point(anchors[0]) - predicted[1].world_point(anchors[1])
                    } else { relative * dt }
                } else { relative * dt };
                frame.points.push(SourceFrictionPoint {
                    color: color_index, constraint: constraint_index, point: point_index, bodies: entities, arms,
                    friction: SourcePositionFriction { tangents,
                        initial_delta: Vec2::new(friction_delta.dot(tangents[0]), friction_delta.dot(tangents[1])),
                        normal_pushout: 0.0, static_friction: friction.static_coefficient,
                        dynamic_friction: friction.dynamic_coefficient, net_pushout: Vec2::ZERO, static_ratio: 0.0 },
                    target_tangent: point.tangent_part.take(),
                    history,
                });
            }
        }
    }
}

#[derive(Clone, Copy)]
struct PositionState {
    initial: SourcePositionBody,
    current: SourcePositionBody,
}

pub(crate) fn solve_source_position_contacts(
    settings: Res<SourceContactFrictionSettings>,
    time: Res<Time<Substeps>>,
    mut frame: ResMut<SourceContactFrictionFrame>,
    mut graph: ResMut<ConstraintGraph>,
    mut bodies: Query<(&mut SolverBody, &SolverBodyInertia)>,
) {
    if frame.points.is_empty() { return; }
    let dt = time.delta_secs();
    let mut states = HashMap::<Entity, PositionState>::default();
    for (&entity, free) in &frame.bodies {
        let Ok((body, inertia)) = bodies.get_mut(entity) else { continue; };
        let state = SourcePositionBody {
            inverse_mass: inertia.effective_inv_mass().x,
            inverse_inertia: Mat3::from(inertia.effective_inv_angular_inertia()),
            arm: Vec3::ZERO,
            dynamic: free.dynamic,
            delta_position: body.delta_position - free.delta_position - free.linear_velocity * dt,
            delta_rotation: (body.angular_velocity - free.angular_velocity) * dt,
        };
        states.insert(entity, PositionState { initial: state, current: state });
    }
    for point in &mut frame.points {
        let constraint = &graph.colors[point.color].contact_constraints[point.constraint];
        point.friction.normal_pushout = constraint.points[point.point].normal_part.impulse * dt;
    }
    for _ in 0..settings.position_iterations {
        for point in &mut frame.points {
            let constraint = &graph.colors[point.color].contact_constraints[point.constraint];
            let absent = SourcePositionBody { inverse_mass: 0.0, inverse_inertia: Mat3::ZERO,
                arm: Vec3::ZERO, dynamic: false, delta_position: Vec3::ZERO, delta_rotation: Vec3::ZERO };
            let mut pair = [states.get(&point.bodies[0]).map_or(absent, |state| state.current),
                states.get(&point.bodies[1]).map_or(absent, |state| state.current)];
            pair[0].arm = point.arms[0];
            pair[1].arm = point.arms[1];
            if constraint.relative_dominance > 0 { pair[0].dynamic = false; }
            if constraint.relative_dominance < 0 { pair[1].dynamic = false; }
            solve_source_position_friction(&mut pair, &mut point.friction, settings.position_stiffness);
            for index in 0..2 {
                if let Some(state) = states.get_mut(&point.bodies[index]) {
                    state.current.delta_position = pair[index].delta_position;
                    state.current.delta_rotation = pair[index].delta_rotation;
                }
            }
        }
    }
    for (entity, state) in states {
        let Ok((mut body, _)) = bodies.get_mut(entity) else { continue; };
        let position = state.current.delta_position - state.initial.delta_position;
        let angular = state.current.delta_rotation - state.initial.delta_rotation;
        if position == Vec3::ZERO && angular == Vec3::ZERO { continue; }
        body.delta_position += position;
        body.linear_velocity += position / dt;
        body.angular_velocity += angular / dt;
        let half = angular * 0.5;
        body.delta_rotation.0 = (Quat::from_xyzw(half.x, half.y, half.z, 1.0) * body.delta_rotation.0).normalize();
    }
    for point in &mut frame.points {
        let constraint = &mut graph.colors[point.color].contact_constraints[point.constraint];
        let tangents = constraint.tangent_directions();
        let position_impulse = point.friction.tangents[0] * point.friction.net_pushout.x
            + point.friction.tangents[1] * point.friction.net_pushout.y;
        if let Some(mut target) = point.target_tangent.take() {
            target.impulse = -Vec2::new(position_impulse.dot(tangents[0]), position_impulse.dot(tangents[1])) / dt;
            constraint.points[point.point].tangent_part = Some(target);
        }
    }
}

pub(crate) fn update_source_friction_history_ratios(
    time: Res<Time<Substeps>>,
    solver: Res<SolverConfig>,
    length_unit: Res<PhysicsLengthUnit>,
    bodies: Query<(&SolverBody, &SolverBodyInertia)>,
    graph: Res<ConstraintGraph>,
    mut frame: ResMut<SourceContactFrictionFrame>,
) {
    let mass_tolerance: f32 = include!("contact_friction_tolerances.rs")[0];
    let squared_tolerance: f32 = include!("velocity_friction_squared_tolerance.rs");
    let threshold = (solver.restitution_threshold * length_unit.0).next_down();
    let dt = time.delta_secs();
    let dummy_body = SolverBody::DUMMY;
    let dummy_inertia = SolverBodyInertia::DUMMY;
    let SourceContactFrictionFrame { bodies: free_bodies, points, .. } = &mut *frame;
    for point in points {
        let constraint = &graph.colors[point.color].contact_constraints[point.constraint];
        let normal_position_impulse = constraint.points[point.point].normal_part.impulse;
        if normal_position_impulse <= 0.0 || point.friction.dynamic_friction <= 0.0 { continue; }
        let first = bodies.get(point.bodies[0]).unwrap_or((&dummy_body, &dummy_inertia));
        let second = bodies.get(point.bodies[1]).unwrap_or((&dummy_body, &dummy_inertia));
        let inertias = [if constraint.relative_dominance > 0 { &dummy_inertia } else { first.1 },
            if constraint.relative_dominance < 0 { &dummy_inertia } else { second.1 }];
        let velocity = [first.0.velocity_at_point(point.arms[0]), second.0.velocity_at_point(point.arms[1])];
        let free = [free_bodies.get(&point.bodies[0]).copied().unwrap_or_default(),
            free_bodies.get(&point.bodies[1]).copied().unwrap_or_default()];
        let free_velocity = [free[0].linear_velocity + free[0].angular_velocity.cross(point.arms[0]),
            free[1].linear_velocity + free[1].angular_velocity.cross(point.arms[1])];
        let incoming = (free_velocity[1] - free_velocity[0]).dot(constraint.normal);
        let bounce = if incoming < -threshold { -constraint.restitution * incoming } else { 0.0 };
        let normal_error = (velocity[1] - velocity[0]).dot(constraint.normal) - bounce;
        let normal_mass = constraint.points[point.point].normal_part.effective_mass;
        let normal_velocity_impulse = (-normal_mass * normal_error).max(-normal_position_impulse);
        let normal_total = normal_position_impulse + normal_velocity_impulse;
        let mut tangent_mass = Vec2::ZERO;
        for axis in 0..2 {
            let tangent = point.friction.tangents[axis];
            let mut inverse = 0.0;
            for index in 0..2 {
                if !free[index].dynamic { continue; }
                let cross = point.arms[index].cross(tangent);
                inverse += inertias[index].effective_inv_mass().x
                    + cross.dot(inertias[index].effective_inv_angular_inertia() * cross);
            }
            if inverse > mass_tolerance { tangent_mass[axis] = inverse.recip(); }
        }
        let relative = velocity[0] - velocity[1];
        let tangent_velocity = Vec2::new(relative.dot(point.friction.tangents[0]), relative.dot(point.friction.tangents[1]));
        point.friction.static_ratio = source_velocity_friction_ratio(point.friction.static_ratio,
            point.friction.net_pushout / dt, tangent_velocity, tangent_mass, normal_total,
            point.friction.dynamic_friction, squared_tolerance);
    }
}

pub(crate) fn save_source_contact_history(
    settings: Res<SourceContactFrictionSettings>,
    contact_graph: Res<ContactGraph>,
    mut frame: ResMut<SourceContactFrictionFrame>,
) {
    let mut next = HashMap::<u32, ([Entity; 2], [Entity; 2], [Tick; 2], Vec<Option<[Vec3; 2]>>)>::default();
    for point in &frame.points {
        let Some(history) = &point.history else { continue; };
        let entry = next.entry(history.contact).or_insert_with(||
            (history.colliders, history.bodies, history.shape_ticks, vec![None; history.point_count]));
        entry.3[history.point_index] = save_source_contact_anchor(history.current, history.anchors,
            point.friction.static_ratio, history.point_count, settings.history_ratio_tolerance);
    }
    frame.history.retain(|id, history| {
        contact_graph.get_by_id(avian3d::collision::contact_types::ContactId(*id)).is_some_and(|(_, pair)| {
            (history.colliders == [pair.collider1, pair.collider2]
                && [Some(history.bodies[0]), Some(history.bodies[1])] == [pair.body1, pair.body2])
                || (history.colliders == [pair.collider2, pair.collider1]
                    && [Some(history.bodies[1]), Some(history.bodies[0])] == [pair.body1, pair.body2])
        })
    });
    for (contact, (colliders, bodies, shape_ticks, anchors)) in next {
        frame.history.insert(contact, SavedHistory { colliders, bodies, shape_ticks, anchors: anchors.into_iter().flatten().collect() });
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourcePositionBody {
    pub inverse_mass: f32,
    pub inverse_inertia: Mat3,
    pub arm: Vec3,
    pub dynamic: bool,
    pub delta_position: Vec3,
    pub delta_rotation: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourcePositionFriction {
    pub tangents: [Vec3; 2],
    pub initial_delta: Vec2,
    pub normal_pushout: f32,
    pub static_friction: f32,
    pub dynamic_friction: f32,
    pub net_pushout: Vec2,
    pub static_ratio: f32,
}

pub(crate) fn solve_source_position_friction(
    bodies: &mut [SourcePositionBody; 2],
    contact: &mut SourcePositionFriction,
    stiffness: f32,
) {
    let [mass_tolerance, impulse_tolerance]: [f32; 2] = include!("contact_friction_tolerances.rs");
    let normal_pushout = contact.normal_pushout.max(0.0);
    if stiffness <= 0.0 || (normal_pushout == 0.0 && contact.net_pushout == Vec2::ZERO) { return; }
    let mut delta = Vec2::ZERO;
    for axis in 0..2 {
        let tangent = contact.tangents[axis];
        let mut error = contact.initial_delta[axis];
        let mut inverse_mass = 0.0;
        for (index, body) in bodies.iter().enumerate() {
            if !body.dynamic { continue; }
            let cross = body.arm.cross(tangent);
            let sign = if index == 0 { 1.0 } else { -1.0 };
            error += sign * (body.delta_position.dot(tangent) + body.delta_rotation.dot(cross));
            inverse_mass += body.inverse_mass + cross.dot(body.inverse_inertia * cross);
        }
        let effective_mass = if inverse_mass > mass_tolerance { inverse_mass.recip() } else { 0.0 };
        delta[axis] = -stiffness * effective_mass * error;
    }
    let old = contact.net_pushout;
    let mut next = old + delta;
    contact.static_ratio = 1.0;
    if normal_pushout < impulse_tolerance {
        next = Vec2::ZERO;
        contact.static_ratio = 0.0;
    } else if next.length_squared() > (contact.static_friction * normal_pushout).powi(2) {
        contact.static_ratio = contact.dynamic_friction * normal_pushout / next.length();
        next *= contact.static_ratio;
    }
    contact.net_pushout = next;
    let applied = next - old;
    let impulse = contact.tangents[0] * applied.x + contact.tangents[1] * applied.y;
    for (index, body) in bodies.iter_mut().enumerate() {
        if !body.dynamic { continue; }
        let sign = if index == 0 { 1.0 } else { -1.0 };
        body.delta_position += sign * body.inverse_mass * impulse;
        body.delta_rotation += sign * (body.inverse_inertia * body.arm.cross(impulse));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_contact_history_tracks_shape_and_body_identity() {
        let mut world = World::new();
        let a = world.spawn_empty().id(); let b = world.spawn_empty().id();
        let first = world.spawn_empty().id(); let second = world.spawn_empty().id();
        let replacement = world.spawn_empty().id();
        let saved = SavedHistory { colliders: [a, b], bodies: [first, second],
            shape_ticks: [Tick::new(1), Tick::new(2)], anchors: vec![[Vec3::X, Vec3::Y]] };
        assert_eq!(saved.oriented([a, b], [first, second], [Tick::new(1), Tick::new(2)]), Some(vec![[Vec3::X, Vec3::Y]]));
        assert_eq!(saved.oriented([b, a], [second, first], [Tick::new(2), Tick::new(1)]), Some(vec![[Vec3::Y, Vec3::X]]));
        assert!(saved.oriented([a, b], [replacement, second], [Tick::new(1), Tick::new(2)]).is_none());
        assert!(saved.oriented([a, b], [first, second], [Tick::new(3), Tick::new(2)]).is_none());
    }

    #[test]
    fn source_contact_history_loaded_ragdoll_restores_tangent_anchor() {
        use avian3d::prelude::{AngularDamping, AngularVelocity, ColliderDisabled, ColliderOf, ComputedCenterOfMass,
            Gravity, JointDisabled, LinearDamping, LinearVelocity, RigidBody, Sleeping, SleepingDisabled};
        use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
        use bevy::time::TimeUpdateStrategy;
        for dt in [1.0_f32 / 60.0, 1.0 / 30.0] {
            for offset in [0.0_f32, 0.001, -0.001, 0.005, 0.02] {
                let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
                let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
                app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
                for _ in 0..12 { app.update(); }
                let spawned = app.world().get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap();
                let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
                let joints = spawned.joints.values().copied().collect::<Vec<_>>();
                let head = bodies.iter().find(|(label, _)| label == "head").unwrap().1;
                let shape = colliders.iter().copied().find(|entity| app.world().get::<ColliderOf>(*entity)
                    .is_some_and(|owner| owner.body == head)).unwrap();
                for entity in colliders { if entity != shape { app.world_mut().entity_mut(entity).insert(ColliderDisabled); } }
                for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
                app.world_mut().insert_resource(Gravity(Vec3::ZERO));
                app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(dt)));
                let local_center = app.world().get::<ComputedCenterOfMass>(head).unwrap().0;
                let position = app.world().get::<Position>(head).unwrap().0;
                let rotation = app.world().get::<Rotation>(head).unwrap().0;
                let center = position + rotation * local_center;
                app.world_mut().entity_mut(shape).insert((Collider::sphere(0.1), Transform::from_translation(local_center),
                    avian3d::prelude::Restitution::new(0.0), Friction::new(0.7)));
                app.world_mut().entity_mut(head).insert((SleepingDisabled, LinearDamping(0.0), AngularDamping(0.0),
                    LinearVelocity::ZERO, AngularVelocity::ZERO)).remove::<Sleeping>();
                let surface = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                    Transform::from_translation(center - Vec3::Y * 0.199),
                    avian3d::prelude::Restitution::new(0.0), Friction::new(0.7))).id();
                app.update();
                let contact = app.world().resource::<ContactGraph>().get(surface, shape).unwrap().1.contact_id.0;
                let shape_rotation = app.world().get::<Rotation>(shape).unwrap().0;
                let mut frame = app.world_mut().resource_mut::<SourceContactFrictionFrame>();
                let saved = frame.history.get_mut(&contact).unwrap();
                let side = saved.colliders.iter().position(|entity| *entity == shape).unwrap();
                assert!(!saved.anchors.is_empty());
                for anchors in &mut saved.anchors { anchors[side] += shape_rotation.inverse() * (Vec3::X * offset); }
                drop(frame);
                app.world_mut().entity_mut(head).insert((LinearVelocity(Vec3::NEG_Y), AngularVelocity::ZERO));
                let before = app.world().get::<Position>(head).unwrap().0;
                app.update();
                let actual = app.world().get::<Position>(head).unwrap().x - before.x;
                let inertia = app.world().get::<SolverBodyInertia>(head).unwrap();
                let inverse_mass = inertia.effective_inv_mass().x;
                let inverse_inertia = Mat3::from(inertia.effective_inv_angular_inertia());
                let arms = [Vec3::NEG_Y.cross(Vec3::X) * 0.1, Vec3::NEG_Y.cross(Vec3::Z) * 0.1];
                let kx = inverse_mass + arms[0].dot(inverse_inertia * arms[0]);
                let kz = inverse_mass + arms[1].dot(inverse_inertia * arms[1]);
                let coupling = arms[0].dot(inverse_inertia * arms[1]);
                let mut impulse = Vec2::ZERO;
                if offset.abs() < 0.01 {
                    for _ in 0..4 {
                        let error = Vec2::new(offset + kx * impulse.x + coupling * impulse.y,
                            coupling * impulse.x + kz * impulse.y);
                        impulse += Vec2::new(-0.5 * error.x / kx, -0.5 * error.y / kz);
                    }
                }
                let expected = inverse_mass * impulse.x;
                assert!((actual - expected).abs() < 0.0002,
                    "dt={dt} offset={offset} actual={actual} source={expected}");
                app.world_mut().despawn(surface);
                app.update();
                assert!(!app.world().resource::<SourceContactFrictionFrame>().history.contains_key(&contact));
            }
        }
    }

    fn body(inverse_mass: f32, inverse_inertia: Mat3, arm: Vec3, dynamic: bool) -> SourcePositionBody {
        SourcePositionBody { inverse_mass, inverse_inertia, arm, dynamic,
            delta_position: Vec3::ZERO, delta_rotation: Vec3::ZERO }
    }

    fn point(delta: Vec2, normal_pushout: f32) -> SourcePositionFriction {
        SourcePositionFriction { tangents: [Vec3::X, Vec3::Z], initial_delta: delta,
            normal_pushout, static_friction: 0.7, dynamic_friction: 0.7,
            net_pushout: Vec2::ZERO, static_ratio: 0.0 }
    }

    #[test]
    fn source_position_friction_matches_native_head_single_axis() {
        let profile = crate::pa_als::parse_pa_als_profile();
        let native = profile.bodies.iter().find(|body| body.label == "head").unwrap().mass_properties.unwrap();
        let inverse_mass = native.mass_kg.recip();
        let inverse_inertia = native.inertia_tensor_kg_m2.inverse();
        let arm = Vec3::NEG_Y * 0.2;
        let angle = 0.5 * (2.0 * inverse_inertia.z_axis.x).atan2(inverse_inertia.x_axis.x - inverse_inertia.z_axis.z);
        let tangent = Vec3::new(angle.cos(), 0.0, angle.sin());
        let mut bodies = [body(inverse_mass, inverse_inertia, arm, true), body(0.0, Mat3::ZERO, Vec3::ZERO, false)];
        let mut contact = point(Vec2::new(1.0 / 60.0, 0.0), 1.0);
        contact.tangents = [tangent, Vec3::Y.cross(tangent)];
        for _ in 0..4 { solve_source_position_friction(&mut bodies, &mut contact, 0.5); }
        let cross = arm.cross(tangent);
        let inverse_effective_mass = inverse_mass + cross.dot(inverse_inertia * cross);
        let expected = -(15.0 / 16.0) * (1.0 / 60.0) / inverse_effective_mass;
        assert!((contact.net_pushout.x - expected).abs() < 1e-6);
        assert!(bodies[0].delta_position.abs_diff_eq(tangent * (inverse_mass * expected), 1e-6));
        assert!(bodies[0].delta_rotation.abs_diff_eq(inverse_inertia * cross * expected, 1e-6));
        assert_eq!(bodies[1].delta_position, Vec3::ZERO);
    }

    #[test]
    fn source_position_friction_clips_and_unwinds_coulomb_cone() {
        let mut bodies = [body(1.0, Mat3::IDENTITY, Vec3::NEG_Y, true), body(0.0, Mat3::ZERO, Vec3::ZERO, false)];
        let mut contact = point(Vec2::new(0.1, 0.0), 0.01);
        for _ in 0..4 { solve_source_position_friction(&mut bodies, &mut contact, 0.5); }
        assert!((contact.net_pushout.length() - 0.007).abs() < 1e-7);
        assert!(contact.static_ratio < 1.0);
        contact.normal_pushout = 0.0;
        solve_source_position_friction(&mut bodies, &mut contact, 0.5);
        assert_eq!(contact.net_pushout, Vec2::ZERO);
        assert!(bodies[0].delta_position.abs_diff_eq(Vec3::ZERO, 1e-7));
        assert!(bodies[0].delta_rotation.abs_diff_eq(Vec3::ZERO, 1e-7));
        assert_eq!(contact.static_ratio, 0.0);
    }

    #[test]
    fn source_position_friction_handles_two_bodies_and_independent_axes() {
        let mut bodies = [body(0.5, Mat3::ZERO, Vec3::ZERO, true), body(0.25, Mat3::ZERO, Vec3::ZERO, true)];
        let mut contact = point(Vec2::new(0.04, -0.02), 1.0);
        for _ in 0..4 { solve_source_position_friction(&mut bodies, &mut contact, 0.5); }
        assert!(contact.net_pushout.abs_diff_eq(Vec2::new(-0.05, 0.025), 1e-6));
        assert!(bodies[0].delta_position.abs_diff_eq(Vec3::new(-0.025, 0.0, 0.0125), 1e-6));
        assert!(bodies[1].delta_position.abs_diff_eq(Vec3::new(0.0125, 0.0, -0.00625), 1e-6));
        let mut immovable = [body(1e-8, Mat3::ZERO, Vec3::ZERO, true), body(0.0, Mat3::ZERO, Vec3::ZERO, false)];
        let mut zero = point(Vec2::ONE, 1.0);
        solve_source_position_friction(&mut immovable, &mut zero, 0.5);
        assert_eq!(zero.net_pushout, Vec2::ZERO);
        let mut frictionless = point(Vec2::ONE, 1.0);
        frictionless.static_friction = 0.0;
        frictionless.dynamic_friction = 0.0;
        solve_source_position_friction(&mut bodies, &mut frictionless, 0.5);
        assert_eq!(frictionless.net_pushout, Vec2::ZERO);
    }
}
