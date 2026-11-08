use avian3d::collision::contact_types::{ContactGraph, ContactPoint};
use avian3d::prelude::{Collider, Rotation};
use bevy::prelude::*;

use super::ragdoll_physics_profile::RagdollActiveMode;

const SOURCE_CAPSULE_AXIS_NORMAL_THRESHOLD: f32 = include!("capsule_contact_axis_threshold.rs");

fn source_capsule_cylinder_contact(axis: Vec3, normal: Vec3) -> bool {
    axis.dot(normal).abs() < SOURCE_CAPSULE_AXIS_NORMAL_THRESHOLD
}

fn order_source_capsule_points(points: &mut [ContactPoint], axis: Vec3, capsule_first: bool) {
    points.sort_by(|left, right| {
        let left = if capsule_first { left.anchor1 } else { left.anchor2 };
        let right = if capsule_first { right.anchor1 } else { right.anchor2 };
        right.dot(axis).total_cmp(&left.dot(axis))
    });
}

pub(crate) fn order_source_capsule_box_contacts(
    ragdolls: Query<(), With<RagdollActiveMode>>,
    colliders: Query<(&Collider, &Rotation)>,
    mut graph: ResMut<ContactGraph>,
) {
    for pair in graph.iter_active_mut() {
        if !pair.body1.is_some_and(|body| ragdolls.contains(body))
            && !pair.body2.is_some_and(|body| ragdolls.contains(body)) { continue; }
        let (Ok((first, first_rotation)), Ok((second, second_rotation))) =
            (colliders.get(pair.collider1), colliders.get(pair.collider2)) else { continue; };
        let first_shape = first.shape_scaled();
        let second_shape = second.shape_scaled();
        let (axis, capsule_first) = if let Some(capsule) = first_shape.as_capsule().filter(|_| second_shape.as_cuboid().is_some()) {
            (first_rotation.0 * (capsule.segment.b - capsule.segment.a).normalize_or_zero(), true)
        } else if let Some(capsule) = second_shape.as_capsule().filter(|_| first_shape.as_cuboid().is_some()) {
            (second_rotation.0 * (capsule.segment.b - capsule.segment.a).normalize_or_zero(), false)
        } else { continue; };
        for manifold in &mut pair.manifolds {
            if source_capsule_cylinder_contact(axis, manifold.normal) {
                order_source_capsule_points(&mut manifold.points, axis, capsule_first);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector(value: &serde_json::Value) -> Vec3 {
        Vec3::new(value[0].as_f64().unwrap() as f32, value[1].as_f64().unwrap() as f32,
            value[2].as_f64().unwrap() as f32)
    }

    #[test]
    fn source_capsule_box_order_matches_native_head_probe() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../docs/audit/bf144/d100a/axis-evidence.json")).unwrap();
        let axis = vector(&fixture["source_endpoint_0_minus_1_runtime_direction"]);
        let normal = vector(&fixture["source_normal_runtime"]);
        let expected = fixture["source_points_body_m"].as_array().unwrap().iter().map(vector).collect::<Vec<_>>();
        assert_eq!(expected.len(), 2);
        assert!(source_capsule_cylinder_contact(axis, normal));
        for first in [false, true] {
            let mut points = expected.iter().rev().map(|value| {
                if first { ContactPoint::new(*value, Vec3::ZERO, *value, 0.0) }
                else { ContactPoint::new(Vec3::ZERO, *value, *value, 0.0) }
            }).collect::<Vec<_>>();
            order_source_capsule_points(&mut points, axis, first);
            for (point, expected) in points.iter().zip(&expected) {
                let actual = if first { point.anchor1 } else { point.anchor2 };
                assert!(actual.abs_diff_eq(*expected, 1e-6));
            }
        }
        assert!(source_capsule_cylinder_contact(Vec3::X, Vec3::new(0.009, 1.0, 0.0).normalize()));
        assert!(!source_capsule_cylinder_contact(Vec3::X, Vec3::new(0.011, 1.0, 0.0).normalize()));
    }

    #[test]
    fn source_ragdoll_capsule_box_manifold_uses_source_endpoint_order() {
        use avian3d::prelude::{AngularVelocity, ColliderDisabled, ColliderOf, ColliderTransform, Gravity,
            JointDisabled, LinearDamping, LinearVelocity, RigidBody, Sleeping, SleepingDisabled};
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
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
        let shape_local = *app.world().get::<ColliderTransform>(shape).unwrap();
        let capsule = app.world().get::<Collider>(shape).unwrap().shape_scaled().as_capsule().unwrap();
        let local_axis = shape_local.rotation.0 * (capsule.segment.b - capsule.segment.a).normalize();
        let radius = capsule.radius;
        let desired = Quat::from_rotation_arc(local_axis, Vec3::X);
        let body_position = app.world().get::<avian3d::prelude::Position>(head).unwrap().0;
        let center = body_position + desired * shape_local.translation;
        let parent = app.world().get::<ChildOf>(head).unwrap().parent();
        let parent_rotation = app.world_mut().run_system_once(move |transforms: bevy::transform::helper::TransformHelper| {
            transforms.compute_global_transform(parent).unwrap().rotation()
        }).unwrap();
        app.world_mut().get_mut::<Transform>(head).unwrap().rotation = parent_rotation.conjugate() * desired;
        app.world_mut().entity_mut(head).insert((Rotation(desired), SleepingDisabled, LinearDamping(0.0),
            LinearVelocity::ZERO, AngularVelocity::ZERO)).remove::<Sleeping>();
        let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
            Transform::from_translation(center - Vec3::Y * (radius + 0.099)))).id();
        app.update();
        let graph = app.world().resource::<ContactGraph>();
        let (_, pair) = graph.get(shape, floor).expect("A capsule-box contact was not generated");
        let first = pair.collider1 == shape;
        let manifold = pair.manifolds.iter().find(|manifold| manifold.points.len() >= 2)
            .expect("The capsule-box contact did not expose two points");
        let projections = manifold.points.iter().map(|point| {
            (if first { point.anchor1 } else { point.anchor2 }).dot(Vec3::X)
        }).collect::<Vec<_>>();
        assert!(projections.windows(2).all(|pair| pair[0] >= pair[1]));
        assert!(projections[0] - projections[projections.len() - 1] > 0.01);
    }
}
