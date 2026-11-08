use avian3d::collision::contact_types::{ContactGraph, ContactManifold, ContactPoint};
use avian3d::collision::narrow_phase::NarrowPhaseConfig;
use avian3d::dynamics::solver::{constraint_graph::{ContactConstraintHandle, COLOR_OVERFLOW_INDEX}, PhysicsLengthUnit};
use avian3d::parry::{math::Pose, query::{ContactManifold as ParryManifold, DefaultQueryDispatcher, PersistentQueryDispatcher}};
use avian3d::prelude::{AngularVelocity, Collider, CollisionMargin, ComputedCenterOfMass, LinearVelocity,
    Position, RigidBody, RigidBodyDisabled, Rotation, SpeculativeMargin};
use bevy::prelude::*;

use super::ragdoll_physics_profile::RagdollActiveMode;

const SOURCE_CAPSULE_AXIS_NORMAL_THRESHOLD: f32 = include!("capsule_contact_axis_threshold.rs");

#[derive(Component)]
pub(crate) struct SourceCompoundOrder;

fn unique_subshape_ids(candidates: &[Vec<(usize, [u32; 2])>], raw_count: usize) -> Option<Vec<[u32; 2]>> {
    let mut earliest = Vec::with_capacity(candidates.len());
    let mut start = 0;
    for choices in candidates {
        let index = choices.iter().find(|(index, _)| *index >= start)?.0;
        earliest.push(index);
        start = index + 1;
    }
    let mut latest = vec![0; candidates.len()];
    let mut end = raw_count;
    for (index, choices) in candidates.iter().enumerate().rev() {
        let chosen = choices.iter().rev().find(|(raw, _)| *raw < end)?.0;
        latest[index] = chosen;
        end = chosen;
    }
    candidates.iter().enumerate().map(|(index, choices)| {
        let lower = if index == 0 { 0 } else { earliest[index - 1] + 1 };
        let upper = latest.get(index + 1).copied().unwrap_or(raw_count);
        let mut possible = choices.iter().filter(|(raw, _)| *raw >= lower && *raw < upper).map(|(_, ids)| *ids);
        let first = possible.next()?;
        possible.all(|ids| ids == first).then_some(first)
    }).collect()
}

pub(crate) fn contact_subshape_ids(
    first: &Collider,
    first_position: Position,
    first_rotation: Rotation,
    second: &Collider,
    second_position: Position,
    second_rotation: Rotation,
    prediction_distance: f32,
    manifolds: &[ContactManifold],
) -> Option<Vec<[u32; 2]>> {
    let first_pose = Pose::from_parts(first_position.0, first_rotation.0);
    let second_pose = Pose::from_parts(second_position.0, second_rotation.0);
    let mut raw = Vec::<ParryManifold<(), ()>>::new();
    DefaultQueryDispatcher.contact_manifolds(
        &first_pose.inv_mul(&second_pose), first.shape_scaled().0.as_ref(),
        second.shape_scaled().0.as_ref(), prediction_distance, &mut raw, &mut None,
    ).ok()?;
    let candidates = manifolds.iter().map(|manifold| {
        raw.iter().enumerate().filter_map(|(index, candidate)| {
            let subpose = candidate.subshape_pos1.unwrap_or_default();
            let normal = first_rotation.0 * (subpose.rotation * candidate.local_n1).normalize();
            if manifold.points.is_empty() || manifold.normal != normal { return None; }
            let matches = manifold.points.iter().all(|point| candidate.contacts().iter().any(|raw_point| {
                let anchor = first_rotation.0 * subpose.transform_point(raw_point.local_p1)
                    + normal * raw_point.dist * 0.5;
                point.point == first_position.0 + anchor
                    && point.feature_id1 == raw_point.fid1.into()
                    && point.feature_id2 == raw_point.fid2.into()
            }));
            matches.then_some((index, [candidate.subshape1, candidate.subshape2]))
        }).collect::<Vec<_>>()
    }).collect::<Vec<_>>();
    unique_subshape_ids(&candidates, raw.len())
}

fn order_compound_groups(
    manifolds: &mut [ContactManifold],
    ids: &[[u32; 2]],
    handles: &[ContactConstraintHandle],
    source_side: usize,
) {
    if manifolds.len() < 2 || manifolds.len() != ids.len() || manifolds.len() != handles.len() { return; }
    let mut slots = (0..manifolds.len()).collect::<Vec<_>>();
    slots.sort_by_key(|index| {
        let handle = handles[*index];
        let color = handle.color_index as usize;
        (if color == COLOR_OVERFLOW_INDEX { 0 } else { color + 1 }, handle.local_index)
    });
    let mut source = (0..manifolds.len()).collect::<Vec<_>>();
    source.sort_by_key(|index| ids[*index][source_side]);
    let ordered = source.into_iter().map(|index| manifolds[index].clone()).collect::<Vec<_>>();
    for (slot, manifold) in slots.into_iter().zip(ordered) { manifolds[slot] = manifold; }
}

pub(crate) fn order_source_compound_groups(
    time: Res<Time>,
    config: Res<NarrowPhaseConfig>,
    length: Res<PhysicsLengthUnit>,
    ragdolls: Query<(), With<RagdollActiveMode>>,
    sources: Query<(), With<SourceCompoundOrder>>,
    colliders: Query<(&Collider, &Position, &Rotation, Option<&CollisionMargin>, Option<&SpeculativeMargin>)>,
    bodies: Query<(&RigidBody, &Position, &Rotation, &ComputedCenterOfMass, &LinearVelocity, &AngularVelocity,
        Option<&CollisionMargin>, Option<&SpeculativeMargin>), Without<RigidBodyDisabled>>,
    mut graph: ResMut<ContactGraph>,
) {
    let delta = time.delta_secs();
    let mut updates = Vec::new();
    for pair in graph.iter_active() {
        if !pair.body1.is_some_and(|body| ragdolls.contains(body))
            && !pair.body2.is_some_and(|body| ragdolls.contains(body)) { continue; }
        let source_side = if sources.contains(pair.collider1) { 0 }
            else if sources.contains(pair.collider2) { 1 } else { continue; };
        let (Ok(first), Ok(second)) = (colliders.get(pair.collider1), colliders.get(pair.collider2)) else { continue; };
        let Some((edge, _)) = graph.get_by_id(pair.contact_id) else { continue; };
        if pair.manifolds.len() < 2 || edge.constraint_handles.len() != pair.manifolds.len() { continue; }
        let state = |body: Option<Entity>, margin: Option<&CollisionMargin>, speculative: Option<&SpeculativeMargin>| {
            let body = body.and_then(|entity| bodies.get(entity).ok());
            let mut velocity = body.as_ref().map_or(Vec3::ZERO, |data| data.4.0);
            let margin = margin.or_else(|| body.as_ref().and_then(|data| data.6)).map_or(0.0, |value| value.0);
            let speculative = speculative.or_else(|| body.as_ref().and_then(|data| data.7))
                .map_or(config.default_speculative_margin * length.0, |value| value.0);
            if speculative < f32::MAX { velocity = velocity.clamp_length_max(speculative * delta.recip()); }
            (velocity, margin)
        };
        let (velocity1, margin1) = state(pair.body1, first.3, first.4);
        let (velocity2, margin2) = state(pair.body2, second.3, second.4);
        let prediction = (delta * (velocity2 - velocity1).length()).max(config.contact_tolerance * length.0)
            + (margin1 + margin2);
        let Some(ids) = contact_subshape_ids(first.0, *first.1, *first.2, second.0, *second.1, *second.2,
            prediction, &pair.manifolds) else { continue; };
        let mut manifolds = pair.manifolds.clone();
        order_compound_groups(&mut manifolds, &ids, &edge.constraint_handles, source_side);
        updates.push((pair.contact_id, manifolds));
    }
    for (id, manifolds) in updates {
        if let Some((_, pair)) = graph.get_mut_by_id(id) { pair.manifolds = manifolds; }
    }
}

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

    #[test]
    fn source_compound_groups_follow_actual_solver_slot_order() {
        let make = |value: f32| ContactManifold::new(
            [ContactPoint::new(Vec3::ZERO, Vec3::ZERO, Vec3::splat(value), 0.0)], Vec3::Y,
        );
        let mut manifolds = vec![make(2.0), make(0.0), make(1.0)];
        let ids = [[0, 2], [0, 0], [0, 1]];
        let handles = [
            ContactConstraintHandle { color_index: 21, local_index: 1 },
            ContactConstraintHandle { color_index: COLOR_OVERFLOW_INDEX as u8, local_index: 0 },
            ContactConstraintHandle { color_index: 3, local_index: 4 },
        ];
        order_compound_groups(&mut manifolds, &ids, &handles, 1);
        assert_eq!(manifolds.iter().map(|value| value.points[0].point.x).collect::<Vec<_>>(), vec![2.0, 0.0, 1.0]);
        let reversed = [handles[1], handles[0], handles[2]];
        order_compound_groups(&mut manifolds, &ids, &reversed, 1);
        assert_eq!(manifolds.iter().map(|value| value.points[0].point.x).collect::<Vec<_>>(), vec![0.0, 2.0, 1.0]);
        let unchanged = manifolds.clone();
        order_compound_groups(&mut manifolds, &ids, &handles[..2], 1);
        assert_eq!(manifolds, unchanged);
    }

    #[test]
    fn source_arch_foot_groups_reach_the_solver_in_native_order() {
        use avian3d::collision::narrow_phase::NarrowPhaseSystems;
        use avian3d::prelude::{Gravity, LockedAxes, PhysicsPlugins, PhysicsSchedule, SleepingDisabled};
        use bevy::time::TimeUpdateStrategy;
        use bevy_animation_graph::core::ragdoll::definition::BodyMode;
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../docs/audit/bf144/d100b3a/arch-foot.native-sample.json")).unwrap();
        let definitions: std::collections::HashMap<String, Vec<Vec<[f32; 3]>>> = serde_json::from_str(
            include_str!("../../assets/ALSExtras/Environment/Meshes/static_convex_collision.runtime.json")).unwrap();
        let input = &fixture["input"];
        let name = input["mesh_asset_path"].as_str().unwrap().rsplit('/').next().unwrap();
        let expected = fixture["aggregate_contacts"].as_array().unwrap().iter()
            .map(|value| value["mesh_shape_index"].as_u64().unwrap() as u32).collect::<Vec<_>>();
        let source_vector = |value: &serde_json::Value| Vec3::new(value["x"].as_f64().unwrap() as f32,
            value["y"].as_f64().unwrap() as f32, value["z"].as_f64().unwrap() as f32);
        let lower = source_vector(&fixture["geometry"]["minimum_cm"]);
        let upper = source_vector(&fixture["geometry"]["maximum_cm"]);
        let center_source = vector(&input["body_translation_cm"]) + (lower + upper) * 0.5;
        let size_source = upper - lower;
        let center = Vec3::new(center_source.x, center_source.z, center_source.y) * 0.01;
        let size = Vec3::new(size_source.x, size_source.z, size_source.y) * 0.01;
        for source_order in [false, true] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::new(PostUpdate)))
                .insert_resource(Gravity(Vec3::ZERO))
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
                .add_systems(PhysicsSchedule, order_source_compound_groups.in_set(NarrowPhaseSystems::Last));
            app.finish();
            app.cleanup();
            let compound = Collider::compound(definitions[name].iter().map(|hull| (
                Vec3::ZERO, Quat::IDENTITY,
                Collider::convex_hull(hull.iter().copied().map(Vec3::from_array).collect()).unwrap(),
            )).collect());
            let floor = app.world_mut().spawn((RigidBody::Static, compound, Transform::IDENTITY)).id();
            if source_order { app.world_mut().entity_mut(floor).insert(SourceCompoundOrder); }
            let foot = app.world_mut().spawn((RigidBody::Dynamic, LockedAxes::ALL_LOCKED, SleepingDisabled,
                Collider::cuboid(size.x, size.y, size.z), Transform::from_translation(center),
                RagdollActiveMode(BodyMode::Dynamic))).id();
            app.update();
            app.update();
            let graph = app.world().resource::<ContactGraph>();
            let (edge, pair) = graph.get(floor, foot).expect("The source foot did not contact the source arch");
            assert_eq!(pair.manifolds.len(), expected.len());
            let first = (app.world().get::<Collider>(pair.collider1).unwrap(),
                *app.world().get::<Position>(pair.collider1).unwrap(), *app.world().get::<Rotation>(pair.collider1).unwrap());
            let second = (app.world().get::<Collider>(pair.collider2).unwrap(),
                *app.world().get::<Position>(pair.collider2).unwrap(), *app.world().get::<Rotation>(pair.collider2).unwrap());
            let source_side = usize::from(pair.collider2 == floor);
            let mut slots = edge.constraint_handles.iter().enumerate().map(|(index, handle)| {
                let ids = contact_subshape_ids(first.0, first.1, first.2, second.0, second.1, second.2,
                    0.005, &pair.manifolds[index..index + 1]).unwrap();
                let color = handle.color_index as usize;
                ((if color == COLOR_OVERFLOW_INDEX { 0 } else { color + 1 }, handle.local_index), ids[0][source_side])
            }).collect::<Vec<_>>();
            slots.sort_by_key(|value| value.0);
            let order = slots.iter().map(|value| value.1).collect::<Vec<_>>();
            if source_order { assert_eq!(order, expected); }
            else {
                let mut sorted = order;
                sorted.sort_unstable();
                assert_eq!(sorted, expected);
            }
        }
    }

    #[test]
    fn subshape_identity_requires_an_unambiguous_ordered_match() {
        let a = [1, 0];
        let b = [2, 0];
        assert_eq!(unique_subshape_ids(&[vec![(0, a), (1, b)], vec![(1, b)]], 2), Some(vec![a, b]));
        assert_eq!(unique_subshape_ids(&[vec![(0, a), (1, b)]], 2), None);
        assert_eq!(unique_subshape_ids(&[vec![(0, a), (1, a)]], 2), Some(vec![a]));
        assert_eq!(unique_subshape_ids(&[vec![(1, a)], vec![(0, b)]], 2), None);
        assert_eq!(unique_subshape_ids(&[], 0), Some(Vec::new()));
    }

    fn assert_composite_subshape_identity(composite: Collider) {
        use avian3d::collision::collider::contact_query::contact_manifolds;
        let other = Collider::cuboid(2.0, 0.2, 2.0);
        let position = Position(Vec3::ZERO);
        let other_position = Position(Vec3::Y * 0.09);
        let rotation = Rotation(Quat::IDENTITY);
        for flipped in [false, true] {
            let (first, pos1, second, pos2, side) = if flipped {
                (&other, other_position, &composite, position, 1)
            } else {
                (&composite, position, &other, other_position, 0)
            };
            let mut manifolds = Vec::new();
            contact_manifolds(first, pos1, rotation, second, pos2, rotation, 0.05, &mut manifolds);
            let ids = contact_subshape_ids(first, pos1, rotation, second, pos2, rotation, 0.05, &manifolds).unwrap();
            let mut indices = ids.iter().map(|ids| ids[side]).collect::<Vec<_>>();
            indices.sort_unstable();
            assert_eq!(indices, vec![0, 1]);
            let retained = vec![manifolds[1].clone()];
            let retained_ids = contact_subshape_ids(first, pos1, rotation, second, pos2, rotation, 0.05, &retained).unwrap();
            assert_eq!(retained_ids, vec![ids[1]]);
            let mut pruned = manifolds.clone();
            for manifold in &mut pruned { manifold.points.truncate(1); }
            assert_eq!(contact_subshape_ids(first, pos1, rotation, second, pos2, rotation, 0.05, &pruned), Some(ids));
            pruned[0].points[0].point += Vec3::Y;
            assert_eq!(contact_subshape_ids(first, pos1, rotation, second, pos2, rotation, 0.05, &pruned), None);
        }
    }

    #[test]
    fn contact_subshape_identity_matches_compound_and_mesh_after_pruning() {
        let compound = Collider::compound(vec![
            (Vec3::new(-0.55, -0.5, 0.0), Quat::IDENTITY, Collider::cuboid(0.8, 1.0, 1.0)),
            (Vec3::new(0.55, -0.5, 0.0), Quat::IDENTITY, Collider::cuboid(0.8, 1.0, 1.0)),
        ]);
        assert_composite_subshape_identity(compound);
        let vertices = vec![
            Vec3::new(-1.0, 0.0, -1.0), Vec3::new(-0.1, 0.0, -1.0), Vec3::new(-0.5, 0.0, 1.0),
            Vec3::new(0.1, 0.0, -1.0), Vec3::new(1.0, 0.0, -1.0), Vec3::new(0.5, 0.0, 1.0),
        ];
        let mesh = Collider::try_trimesh(vertices, vec![[0, 2, 1], [3, 5, 4]]).unwrap();
        assert_composite_subshape_identity(mesh);
    }

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
