use bevy::prelude::*;
use bevy::platform::collections::HashMap;
use avian3d::collision::contact_types::ContactGraph;
use avian3d::dynamics::solver::{constraint_graph::ConstraintGraph, contact::ContactTangentPart,
    solver_body::{SolverBody, SolverBodyInertia}};
use avian3d::prelude::{DefaultFriction, Friction, Substeps};

use super::ragdoll_physics_profile::RagdollActiveMode;

#[derive(Resource, Clone, Copy)]
pub(crate) struct SourceContactFrictionSettings {
    pub position_iterations: u32,
    pub position_stiffness: f32,
    pub tangent_velocity_squared: f32,
    pub tangent_geometry_squared: f32,
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
}

#[derive(Resource, Default)]
pub(crate) struct SourceContactFrictionFrame {
    bodies: HashMap<Entity, FreeBody>,
    points: Vec<SourceFrictionPoint>,
}

pub(crate) fn capture_source_friction_contacts(
    settings: Res<SourceContactFrictionSettings>,
    time: Res<Time<Substeps>>,
    ragdolls: Query<(), With<RagdollActiveMode>>,
    bodies: Query<&SolverBody>,
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
            if friction.static_coefficient <= 0.0 && friction.dynamic_coefficient <= 0.0 { continue; }
            let entities = [constraint.body1, constraint.body2];
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
                frame.points.push(SourceFrictionPoint {
                    color: color_index, constraint: constraint_index, point: point_index, bodies: entities, arms,
                    friction: SourcePositionFriction { tangents,
                        initial_delta: Vec2::new(relative.dot(tangents[0]), relative.dot(tangents[1])) * dt,
                        normal_pushout: 0.0, static_friction: friction.static_coefficient,
                        dynamic_friction: friction.dynamic_coefficient, net_pushout: Vec2::ZERO, static_ratio: 0.0 },
                    target_tangent: point.tangent_part.take(),
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
