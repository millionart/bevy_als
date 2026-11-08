use avian3d::dynamics::solver::{constraint_graph::ConstraintGraph, solver_body::SolverBody};
use bevy::prelude::*;

use super::ragdoll_physics_profile::RagdollActiveMode;

#[derive(Resource, Default)]
pub(crate) struct SourceContactSeparationRestore(Vec<(usize, usize, usize, f32)>);

pub(crate) fn refresh_source_contact_normal_speed(
    ragdolls: Query<(), With<RagdollActiveMode>>,
    bodies: Query<&SolverBody>,
    mut graph: ResMut<ConstraintGraph>,
) {
    for constraint in graph.colors.iter_mut().flat_map(|color| &mut color.contact_constraints) {
        if !ragdolls.contains(constraint.body1) && !ragdolls.contains(constraint.body2) { continue; }
        let first = bodies.get(constraint.body1).unwrap_or(&SolverBody::DUMMY);
        let second = bodies.get(constraint.body2).unwrap_or(&SolverBody::DUMMY);
        for point in &mut constraint.points {
            point.normal_speed = (second.velocity_at_point(point.anchor2) - first.velocity_at_point(point.anchor1))
                .dot(constraint.normal);
        }
    }
}

pub(crate) fn suppress_source_velocity_separation(
    ragdolls: Query<(), With<RagdollActiveMode>>,
    bodies: Query<&SolverBody>,
    mut graph: ResMut<ConstraintGraph>,
    mut restore: ResMut<SourceContactSeparationRestore>,
) {
    restore.0.clear();
    for (color_index, color) in graph.colors.iter_mut().enumerate() {
        for (constraint_index, constraint) in color.contact_constraints.iter_mut().enumerate() {
            if !ragdolls.contains(constraint.body1) && !ragdolls.contains(constraint.body2) { continue; }
            let first = bodies.get(constraint.body1).unwrap_or(&SolverBody::DUMMY);
            let second = bodies.get(constraint.body2).unwrap_or(&SolverBody::DUMMY);
            for (point_index, point) in constraint.points.iter_mut().enumerate() {
                let initial = point.initial_separation + (point.anchor2 - point.anchor1).dot(constraint.normal);
                if initial > 0.0 { continue; }
                let delta = second.delta_position - first.delta_position
                    + second.delta_rotation * point.anchor2 - first.delta_rotation * point.anchor1;
                let projected = delta.dot(constraint.normal);
                if projected + point.initial_separation > 0.0 {
                    restore.0.push((color_index, constraint_index, point_index, point.initial_separation));
                    point.initial_separation = -projected;
                }
            }
        }
    }
}

pub(crate) fn restore_source_velocity_separation(
    mut graph: ResMut<ConstraintGraph>,
    mut restore: ResMut<SourceContactSeparationRestore>,
) {
    for (color, constraint, point, original) in restore.0.drain(..) {
        graph.colors[color].contact_constraints[constraint].points[point].initial_separation = original;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::collision::contact_types::ContactId;
    use avian3d::dynamics::solver::{contact::{ContactConstraint, ContactConstraintPoint, ContactNormalPart},
        softness_parameters::SoftnessCoefficients};
    use bevy::ecs::system::RunSystemOnce;
    use bevy_animation_graph::core::ragdoll::definition::BodyMode;

    fn constraint(first: Entity, second: Entity, penetration: f32) -> ContactConstraint {
        ContactConstraint { body1: first, body2: second, relative_dominance: 0,
            friction: 0.0, restitution: 0.0, tangent_velocity: Vec3::ZERO,
            normal: Vec3::Y, tangent1: Vec3::X,
            points: vec![ContactConstraintPoint {
                normal_part: ContactNormalPart { impulse: 1.0, total_impulse: 1.0, effective_mass: 1.0,
                    softness: SoftnessCoefficients { bias: 1.0, mass_scale: 1.0, impulse_scale: 0.0 } },
                tangent_part: None, anchor1: Vec3::NEG_Y * 0.1, anchor2: Vec3::ZERO,
                normal_speed: 0.0, initial_separation: -0.1 - penetration,
            }], contact_id: ContactId(0), manifold_index: 0 }
    }

    #[test]
    fn source_contact_velocity_ignores_new_gap_and_restores_original_geometry() {
        let mut world = World::new();
        world.insert_resource(ConstraintGraph::default());
        world.insert_resource(SourceContactSeparationRestore::default());
        let solver = SolverBody { delta_position: Vec3::NEG_Y * 0.02, ..default() };
        let ragdoll = world.spawn((solver.clone(), RagdollActiveMode(BodyMode::Dynamic))).id();
        let ordinary = world.spawn(solver).id();
        let floor = world.spawn_empty().id();
        let cases = [constraint(ragdoll, floor, 0.01), constraint(ragdoll, floor, -0.01),
            constraint(ordinary, floor, 0.01), constraint(ragdoll, floor, 0.04)];
        let original = cases.iter().map(|item| item.points[0].initial_separation).collect::<Vec<_>>();
        world.resource_mut::<ConstraintGraph>().colors[0].contact_constraints.extend(cases);
        world.run_system_once(suppress_source_velocity_separation).unwrap();
        let graph = world.resource::<ConstraintGraph>();
        let contacts = &graph.colors[0].contact_constraints;
        assert!((contacts[0].points[0].initial_separation + 0.12).abs() < 1e-6);
        let mut normal = contacts[0].points[0].normal_part.clone();
        let separation = 0.12 + contacts[0].points[0].initial_separation;
        let impulse = normal.solve_impulse(separation, Vec3::NEG_Y * 0.2, Vec3::Y, false, 10.0, 0.1);
        assert!((impulse - 0.2).abs() < 1e-6);
        for index in 1..4 { assert_eq!(contacts[index].points[0].initial_separation, original[index]); }
        world.run_system_once(restore_source_velocity_separation).unwrap();
        let graph = world.resource::<ConstraintGraph>();
        for (contact, expected) in graph.colors[0].contact_constraints.iter().zip(original) {
            assert_eq!(contact.points[0].initial_separation, expected);
        }
    }

    #[test]
    fn source_restitution_reads_integrated_linear_and_angular_velocity() {
        let mut world = World::new();
        world.insert_resource(ConstraintGraph::default());
        let solver = SolverBody { linear_velocity: Vec3::Y * 2.1635, angular_velocity: Vec3::Z,
            ..default() };
        let ragdoll = world.spawn((solver.clone(), RagdollActiveMode(BodyMode::Dynamic))).id();
        let ordinary = world.spawn(solver).id();
        let floor = world.spawn_empty().id();
        let mut active = constraint(ragdoll, floor, 0.01);
        active.points[0].anchor1 = Vec3::X * 0.1;
        active.points[0].normal_speed = -2.0;
        let mut control = constraint(ordinary, floor, 0.01);
        control.points[0].normal_speed = -2.0;
        world.resource_mut::<ConstraintGraph>().colors[0].contact_constraints.extend([active, control]);
        world.run_system_once(refresh_source_contact_normal_speed).unwrap();
        let graph = world.resource::<ConstraintGraph>();
        let contacts = &graph.colors[0].contact_constraints;
        assert!((contacts[0].points[0].normal_speed + 2.2635).abs() < 1e-5);
        assert_eq!(contacts[1].points[0].normal_speed, -2.0);
    }

    #[test]
    fn source_world_restitution_budget_reaches_native_head_two_point_solver() {
        use avian3d::dynamics::solver::{SolverConfig, solver_body::SolverBodyInertia};
        use avian3d::math::SymmetricMatrix;
        use avian3d::prelude::LockedAxes;
        let (mut app, _, _) = crate::roll_drive::tests::loaded_roll_character();
        app.update();
        let iterations = app.world().resource::<SolverConfig>().restitution_iterations;
        assert_eq!(iterations, 2);
        let profile = crate::pa_als::parse_pa_als_profile();
        let native = profile.bodies.iter().find(|body| body.label == "head").unwrap().mass_properties.unwrap();
        let inverse_mass = native.mass_kg.recip();
        let inverse_inertia = native.inertia_tensor_kg_m2.inverse();
        let inertia = SolverBodyInertia::new(inverse_mass, SymmetricMatrix::from_mat3_unchecked(inverse_inertia),
            LockedAxes::default(), 0, true);
        let mut world = World::new();
        let head = world.spawn_empty().id(); let floor = world.spawn_empty().id();
        let mut contact = constraint(head, floor, 0.01);
        contact.restitution = 0.3;
        let arms = [Vec3::new(-0.1, 0.0, 0.0), Vec3::new(0.07, 0.0, 0.02)];
        contact.points.push(contact.points[0].clone());
        for (point, arm) in contact.points.iter_mut().zip(arms) {
            point.anchor1 = arm;
            point.normal_speed = -2.0;
            point.normal_part.impulse = 10.0;
            point.normal_part.total_impulse = 1.0;
            let cross = arm.cross(Vec3::Y);
            point.normal_part.effective_mass = (inverse_mass + cross.dot(inverse_inertia * cross)).recip();
        }
        let mut once = contact.clone();
        let mut once_head = SolverBody::default(); let mut once_floor = SolverBody::DUMMY;
        once.apply_restitution(&mut once_head, &mut once_floor, &inertia, &SolverBodyInertia::DUMMY, 0.1);
        let mut actual = SolverBody::default(); let mut fixed = SolverBody::DUMMY;
        for _ in 0..iterations { contact.apply_restitution(&mut actual, &mut fixed, &inertia, &SolverBodyInertia::DUMMY, 0.1); }
        let mut velocity = Vec3::ZERO; let mut angular = Vec3::ZERO;
        for _ in 0..2 {
            for arm in arms {
                let cross = arm.cross(Vec3::Y);
                let effective_mass = (inverse_mass + cross.dot(inverse_inertia * cross)).recip();
                let normal_speed = -(velocity + angular.cross(arm)).dot(Vec3::Y);
                let impulse = -effective_mass * (normal_speed - 0.6);
                velocity -= Vec3::Y * (inverse_mass * impulse);
                angular -= inverse_inertia * cross * impulse;
            }
        }
        assert!(actual.linear_velocity.abs_diff_eq(velocity, 1e-5));
        assert!(actual.angular_velocity.abs_diff_eq(angular, 1e-5));
        assert!((actual.angular_velocity - once_head.angular_velocity).length() > 1e-4);
        let mut single = once.clone();
        single.points.truncate(1);
        single.points[0].normal_part.impulse = 10.0;
        let mut single_head = SolverBody::default(); let mut single_floor = SolverBody::DUMMY;
        single.apply_restitution(&mut single_head, &mut single_floor, &inertia, &SolverBodyInertia::DUMMY, 0.1);
        let first_pass = single_head.clone();
        single.apply_restitution(&mut single_head, &mut single_floor, &inertia, &SolverBodyInertia::DUMMY, 0.1);
        assert!(single_head.linear_velocity.abs_diff_eq(first_pass.linear_velocity, 1e-6));
        assert!(single_head.angular_velocity.abs_diff_eq(first_pass.angular_velocity, 1e-6));
        let mut no_bounce = once;
        no_bounce.restitution = 0.0;
        let mut still = SolverBody::default(); let mut static_body = SolverBody::DUMMY;
        no_bounce.apply_restitution(&mut still, &mut static_body, &inertia, &SolverBodyInertia::DUMMY, 0.1);
        assert_eq!(still.linear_velocity, Vec3::ZERO);
    }
}
