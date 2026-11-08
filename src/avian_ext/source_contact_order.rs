use avian3d::collision::contact_types::{ContactGraph, ContactId, ContactManifold, ContactPoint};
use avian3d::collision::narrow_phase::NarrowPhaseConfig;
use avian3d::dynamics::solver::{constraint_graph::{ConstraintGraph, ContactConstraintHandle, COLOR_OVERFLOW_INDEX}, PhysicsLengthUnit};
use avian3d::parry::{math::Pose, query::{ContactManifold as ParryManifold, DefaultQueryDispatcher, PersistentQueryDispatcher}};
use avian3d::prelude::{AngularVelocity, Collider, ColliderOf, ColliderTransform, CollisionMargin, ComputedCenterOfMass, LinearVelocity,
    Position, RigidBody, RigidBodyDisabled, Rotation, SpeculativeMargin};
#[cfg(test)]
use bevy::math::DVec3;
use bevy::platform::collections::HashMap as EntityMap;
use bevy::prelude::*;
#[cfg(test)]
use std::collections::HashMap;

use super::ragdoll_physics_profile::RagdollActiveMode;
use super::source_contact_convex::{SourceConvexLifecycleSettings, source_body_local_max_extent,
    source_box_convex_manifold, source_convex_cull_distance, source_convex_hull_candidate};
use super::source_contact_face::{source_convex_hulls, source_convex_scale, source_convex_topology};
use super::source_contact_margin::{SourceConvexMarginSettings, source_box_convex_core_normal, source_box_margin, source_polygon_margins};
use super::source_contact_restore::{SourceBoxContactHistory, SourceContactContext, SourceConvexGjkHistory, SourceRestorePose,
    SourceRestoreSettings, capture_source_contact, capture_source_exact_contact, restore_source_contact,
    source_box_shape_pose, source_generated_manifold, source_scaled_bounds_max_extent_cm, write_source_solver_groups};

const SOURCE_CAPSULE_AXIS_NORMAL_THRESHOLD: f32 = include!("capsule_contact_axis_threshold.rs");

#[derive(Component)]
pub(crate) struct SourceCompoundOrder(pub String);

pub(super) fn source_convex_max_extent_cm(asset: &str, index: u32, scale: Vec3) -> Option<f32> {
    Some(source_scaled_bounds_max_extent_cm(source_convex_topology(asset, index)?.bounds_m, scale))
}

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
    margin_settings: Option<Res<SourceConvexMarginSettings>>,
    lifecycle_settings: Option<Res<SourceConvexLifecycleSettings>>,
    restore_settings: Option<Res<SourceRestoreSettings>>,
    ragdolls: Query<(), With<RagdollActiveMode>>,
    sources: Query<Ref<SourceCompoundOrder>>,
    colliders: Query<(&Collider, &Position, &Rotation, Option<&CollisionMargin>, Option<&SpeculativeMargin>)>,
    collider_details: Query<(Ref<Collider>, Option<&ColliderTransform>)>,
    ownership: Query<(Entity, &Collider, Option<&ColliderOf>, Option<&ColliderTransform>)>,
    bodies: Query<(&RigidBody, &Position, &Rotation, &ComputedCenterOfMass, &LinearVelocity, &AngularVelocity,
        Option<&CollisionMargin>, Option<&SpeculativeMargin>), Without<RigidBodyDisabled>>,
    mut contact_history: Option<ResMut<SourceBoxContactHistory>>,
    mut graph: ResMut<ContactGraph>,
    mut constraints: ResMut<ConstraintGraph>,
) {
    let delta = time.delta_secs();
    let mut updates = Vec::new();
    let mut source_updates = Vec::new();
    let mut next_compounds = EntityMap::default();
    let mut next_gjk = EntityMap::default();
    for pair in graph.iter_active() {
        if !pair.body1.is_some_and(|body| ragdolls.contains(body))
            && !pair.body2.is_some_and(|body| ragdolls.contains(body)) { continue; }
        let (source_side, source) = if let Ok(source) = sources.get(pair.collider1) { (0, source) }
            else if let Ok(source) = sources.get(pair.collider2) { (1, source) } else { continue; };
        let (Ok(first), Ok(second)) = (colliders.get(pair.collider1), colliders.get(pair.collider2)) else { continue; };
        let Some((edge, _)) = graph.get_by_id(pair.contact_id) else { continue; };
        if pair.manifolds.is_empty() || edge.constraint_handles.len() != pair.manifolds.len() { continue; }
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
        let (box_shape, box_body, compound_shape, compound_body) = if source_side == 0 {
            (second, pair.body2, first, pair.body1)
        } else {
            (first, pair.body1, second, pair.body2)
        };
        let box_pair = box_body.zip(compound_body).and_then(|(box_body, compound_body)| {
            let (Ok(box_data), Ok(compound_data)) = (bodies.get(box_body), bodies.get(compound_body)) else { return None; };
            (ragdolls.contains(box_body) && box_data.0.is_dynamic() && compound_data.0.is_static()
                && box_shape.0.shape_scaled().as_cuboid().is_some())
                .then_some((box_body, compound_body, box_data, compound_data))
        });
        let core_settings = if let (Some(settings), Some(_), Some(shape)) =
            (margin_settings.as_deref(), box_pair, box_shape.0.shape_scaled().as_cuboid())
            && source_box_margin(shape.half_extents, *settings) > 0.0 { Some(settings) } else { None };
        let restore_base = if let (Some(settings), Some(history), Some((box_body, compound_body, box_data, compound_data))) =
            (restore_settings.as_deref(), contact_history.as_deref(), box_pair)
            && pair.generates_constraints()
        {
            let (box_entity, compound_entity) = if source_side == 0 {
                (pair.collider2, pair.collider1)
            } else { (pair.collider1, pair.collider2) };
            if let (Ok((box_collider, box_local)), Ok((compound_collider, _))) =
                (collider_details.get(box_entity), collider_details.get(compound_entity)) {
                let frames = [source_box_shape_pose(box_shape.1, box_shape.2, box_local, box_data.1, box_data.2),
                    SourceRestorePose { position_m: compound_shape.1.0.as_dvec3(), rotation: compound_shape.2.0.as_dquat() }];
                let bounds = box_shape.0.shape_scaled().compute_local_aabb();
                let box_size_cm = (f64::from((bounds.maxs - bounds.mins).abs().max_element()) * 100.0) as f32;
                let ordered_bodies = if source_side == 0 { [compound_data, box_data] } else { [box_data, compound_data] };
                let context = SourceContactContext {
                    colliders: [box_entity, compound_entity], bodies: [box_body, compound_body],
                    shape_ticks: [box_collider.last_changed(), compound_collider.last_changed()], frames,
                    tolerance: 0.0, skin: margin1 + margin2, box_first: source_side == 1, surface_normal: Vec3::ZERO,
                    centers: ordered_bodies.map(|body| body.1.0 + body.2.0 * body.3.0),
                    linear: [velocity1, velocity2], angular: ordered_bodies.map(|body| body.5.0),
                };
                Some((settings, history, context, box_size_cm))
            } else { None }
        } else { None };
        let box_pose = Pose::from_parts(box_shape.1.0, box_shape.2.0);
        let compound_pose = Pose::from_parts(compound_shape.1.0, compound_shape.2.0);
        let generation = if let (Some(lifecycle), Some(margins), Some((_, _, context, _)), Some((_, _, box_data, _))) =
            (lifecycle_settings.as_deref(), margin_settings.as_deref(), restore_base, box_pair) {
            source_body_local_max_extent(context.bodies[0], ownership.iter())
                .map(|extent| (lifecycle, margins, source_convex_cull_distance(extent, [box_data.4.0, Vec3::ZERO], delta, *lifecycle)))
        } else { None };
        if let (Some((lifecycle, margin_settings, cull)), Some((settings, history, base_context, box_size_cm)),
            Some(hulls), Some(box_cuboid)) = (generation, restore_base, source_convex_hulls(&source.0),
                box_shape.0.shape_scaled().as_cuboid()) {
            let inverse = base_context.frames[0].rotation.inverse();
            let center = inverse * (box_shape.1.0.as_dvec3() - base_context.frames[0].position_m);
            let half = box_cuboid.half_extents.as_dvec3();
            let box_bounds = [center - half, center + half];
            let translation = inverse * (base_context.frames[1].position_m - base_context.frames[0].position_m);
            let rotation = inverse * base_context.frames[1].rotation;
            let margin = source_box_margin(box_cuboid.half_extents, *margin_settings);
            let [margin, _] = source_polygon_margins(margin, true, 0.0, false, margin_settings.minimum);
            let scale = source_convex_scale(compound_shape.0.scale().as_dvec3());
            let mut template = pair.manifolds[0].clone();
            template.points.clear();
            let mut ordered = Vec::new();
            for (index, hull) in hulls.iter().enumerate() {
                let key = (pair.contact_id.0, index as u32, source.last_changed());
                let mut context = base_context;
                let size_cm = source_scaled_bounds_max_extent_cm(hull.bounds_m, scale.as_vec3());
                context.tolerance = f64::from(settings.collision_tolerance_scale * box_size_cm.min(size_cm)) * 0.01;
                let prior = history.compounds.get(&key).filter(|saved| saved.matches(&context));
                if !source_convex_hull_candidate(box_bounds, hull, scale, context.frames, cull,
                    prior.is_some(), include!("../als/settings/contact_bounds_checks.rs")) { continue; }
                let mut manifold = template.clone();
                if let Some(saved) = restore_source_contact(&mut manifold, prior, &context, *settings) {
                    if let Some(gjk) = history.convex_gjk.get(&key).filter(|saved| saved.matches(&context)) {
                        next_gjk.insert(key, gjk.clone());
                    }
                    next_compounds.insert(key, saved);
                    ordered.push(manifold);
                    continue;
                }
                let previous = history.convex_gjk.get(&key).filter(|saved| saved.matches(&context))
                    .and_then(|saved| lifecycle.warm_start.then(|| saved.simplex.clone()));
                if let Some(result) = source_box_convex_manifold(box_bounds[0], box_bounds[1], hull, scale,
                    translation, rotation, f64::from(margin), cull, previous) {
                    next_gjk.insert(key, SourceConvexGjkHistory { colliders: context.colliders, bodies: context.bodies,
                        shape_ticks: context.shape_ticks, simplex: result.gjk.simplex_cm });
                    if result.contacts.iter().any(|point| point.phi_m <= cull) {
                        manifold = source_generated_manifold(&manifold, &result.contacts, &context);
                        next_compounds.insert(key, capture_source_exact_contact(&manifold, &result.contacts, &context));
                        ordered.push(manifold);
                    }
                }
            }
            source_updates.push((pair.contact_id, ordered));
            continue;
        }
        let Some(ids) = contact_subshape_ids(first.0, *first.1, *first.2, second.0, *second.1, *second.2,
            prediction, &pair.manifolds) else { continue; };
        let mut manifolds = pair.manifolds.clone();
        for (manifold, shape_ids) in manifolds.iter_mut().zip(&ids) {
            let shape = shape_ids[source_side];
            let mut entry = None;
            if let Some((settings, history, mut context, box_size_cm)) = restore_base
                && let Some(size_cm) = source_convex_max_extent_cm(&source.0, shape, compound_shape.0.scale()) {
                let key = (pair.contact_id.0, shape, source.last_changed());
                context.tolerance = f64::from(settings.collision_tolerance_scale * box_size_cm.min(size_cm)) * 0.01;
                context.surface_normal = manifold.normal;
                let saved = restore_source_contact(manifold, history.compounds.get(&key), &context, *settings);
                entry = Some((key, saved, context));
            }
            let restored = entry.as_ref().is_some_and(|(_, saved, _)| saved.is_some());
            if !restored && let Some(settings) = core_settings
                && let Some(normal) = source_box_convex_core_normal(box_shape.0, box_pose, compound_shape.0,
                    compound_pose, shape, *settings) {
                manifold.normal = if source_side == 0 { -normal } else { normal };
            }
            if let Some((key, saved, context)) = entry {
                next_compounds.insert(key, saved.unwrap_or_else(|| capture_source_contact(manifold, &context)));
            }
        }
        order_compound_groups(&mut manifolds, &ids, &edge.constraint_handles, source_side);
        updates.push((pair.contact_id, manifolds));
    }
    for (id, manifolds) in updates {
        if let Some((_, pair)) = graph.get_mut_by_id(id) { pair.manifolds = manifolds; }
    }
    for (id, manifolds) in source_updates {
        write_source_solver_groups(&mut graph, &mut constraints, id, manifolds);
    }
    if let Some(history) = contact_history.as_deref_mut() {
        if restore_settings.is_some() {
            for (&key, saved) in &history.compounds {
                if graph.get_by_id(ContactId(key.0)).is_some_and(|(edge, pair)| edge.is_sleeping() && pair.is_touching()) {
                    next_compounds.insert(key, saved.clone());
                }
            }
        }
        history.compounds = next_compounds;
        if lifecycle_settings.is_some() {
            for (&key, saved) in &history.convex_gjk {
                let same_pair = graph.get_by_id(ContactId(key.0)).is_some_and(|(_, pair)| {
                    (pair.collider1 == saved.colliders[0] && pair.collider2 == saved.colliders[1]
                        && pair.body1 == Some(saved.bodies[0]) && pair.body2 == Some(saved.bodies[1]))
                        || (pair.collider2 == saved.colliders[0] && pair.collider1 == saved.colliders[1]
                            && pair.body2 == Some(saved.bodies[0]) && pair.body1 == Some(saved.bodies[1]))
                });
                if same_pair && sources.get(saved.colliders[1]).is_ok_and(|source| source.last_changed() == key.2)
                    && let (Ok((first, _)), Ok((second, _))) =
                        (collider_details.get(saved.colliders[0]), collider_details.get(saved.colliders[1]))
                    && [first.last_changed(), second.last_changed()] == saved.shape_ticks {
                    next_gjk.entry(key).or_insert_with(|| saved.clone());
                }
            }
        }
        history.convex_gjk = next_gjk;
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
    fn source_convex_extents_match_native_hulls_and_scales() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d104b2a/bounds.native-samples.json")).unwrap();
        for sample in fixture["samples"].as_array().unwrap() {
            let array = |value: &serde_json::Value| DVec3::new(
                value[0].as_f64().unwrap(), value[1].as_f64().unwrap(), value[2].as_f64().unwrap());
            let scale = array(&sample["scale"]).as_vec3();
            let expected = ((array(&sample["bounds_m"][1]) - array(&sample["bounds_m"][0]))
                .abs().max_element() * 100.0) as f32;
            let actual = source_convex_max_extent_cm(sample["mesh"].as_str().unwrap(),
                sample["shape_index"].as_u64().unwrap() as u32, scale).unwrap();
            assert_eq!(actual, expected, "{}", sample["mesh"]);
        }
        assert!(source_convex_max_extent_cm("missing", 0, Vec3::ONE).is_none());
        assert!(source_convex_max_extent_cm("SM_Als_Bench", 99, Vec3::ONE).is_none());
    }

    #[test]
    fn source_arch_compound_restores_each_hull_before_solver_ordering() {
        use avian3d::collision::narrow_phase::NarrowPhaseSystems;
        use avian3d::prelude::{Gravity, LockedAxes, PhysicsPlugins, PhysicsSchedule, SleepingDisabled};
        use bevy::time::TimeUpdateStrategy;
        use bevy_animation_graph::core::ragdoll::definition::BodyMode;
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3a/arch-foot.native-sample.json")).unwrap();
        let definitions: HashMap<String, Vec<Vec<[f32; 3]>>> = serde_json::from_str(include_str!(
            "../../assets/ALSExtras/Environment/Meshes/static_convex_collision.runtime.json")).unwrap();
        let input = &fixture["input"];
        let to_vec = |value: &serde_json::Value| Vec3::new(value["x"].as_f64().unwrap() as f32,
            value["z"].as_f64().unwrap() as f32, value["y"].as_f64().unwrap() as f32);
        let lower = to_vec(&fixture["geometry"]["minimum_cm"]);
        let upper = to_vec(&fixture["geometry"]["maximum_cm"]);
        let translation = &input["body_translation_cm"];
        let origin = Vec3::new(translation[0].as_f64().unwrap() as f32, translation[2].as_f64().unwrap() as f32,
            translation[1].as_f64().unwrap() as f32);
        let size = (upper - lower) * 0.01;
        let center = (origin + (lower + upper) * 0.5) * 0.01;
        let expected = fixture["aggregate_contacts"].as_array().unwrap().iter()
            .map(|value| value["mesh_shape_index"].as_u64().unwrap() as u32).collect::<Vec<_>>();
        for enabled in [false, true] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::new(PostUpdate)))
                .insert_resource(Gravity(Vec3::ZERO))
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
                .init_resource::<SourceBoxContactHistory>()
                .add_systems(PhysicsSchedule, order_source_compound_groups.in_set(NarrowPhaseSystems::Last));
            if enabled {
                let ([contact_scale, empty_position_scale, position_scale, empty_rotation_threshold, rotation_threshold],
                    collision_tolerance_scale, minimum_points) = include!("../als/settings/contact_restore.rs");
                app.insert_resource(SourceRestoreSettings { contact_scale, empty_position_scale, position_scale,
                    empty_rotation_threshold, rotation_threshold, collision_tolerance_scale, minimum_points });
            }
            let [fraction, maximum, minimum] = include!("../als/settings/convex_margin.rs");
            app.insert_resource(SourceConvexMarginSettings { fraction, maximum, minimum });
            app.finish();
            app.cleanup();
            let compound = Collider::compound(definitions["SM_Als_Arch_01"].iter().map(|hull| (
                Vec3::ZERO, Quat::IDENTITY,
                Collider::convex_hull(hull.iter().copied().map(Vec3::from_array).collect()).unwrap(),
            )).collect());
            let ground = app.world_mut().spawn((RigidBody::Static, compound,
                SourceCompoundOrder("SM_Als_Arch_01".into()), Transform::IDENTITY)).id();
            let foot = app.world_mut().spawn((RigidBody::Dynamic, LockedAxes::ALL_LOCKED, SleepingDisabled,
                Collider::cuboid(size.x, size.y, size.z), Transform::from_translation(center),
                RagdollActiveMode(BodyMode::Dynamic))).id();
            app.update();
            app.update();
            let surfaces = |world: &World| {
                let (edge, pair) = world.resource::<ContactGraph>().get(ground, foot).unwrap();
                let mut slots = (0..pair.manifolds.len()).collect::<Vec<_>>();
                slots.sort_by_key(|index| {
                    let handle = edge.constraint_handles[*index];
                    let color = handle.color_index as usize;
                    (if color == COLOR_OVERFLOW_INDEX { 0 } else { color + 1 }, handle.local_index)
                });
                slots.into_iter().map(|index| {
                    let manifold = &pair.manifolds[index];
                    let normal = manifold.normal * if pair.collider1 == foot { 1.0 } else { -1.0 };
                    manifold.points.iter().map(|point| point.point + normal * point.penetration * 0.5).collect::<Vec<_>>()
                }).collect::<Vec<_>>()
            };
            let before = surfaces(app.world());
            assert_eq!(before.len(), expected.len());
            if enabled {
                let mut ids = app.world().resource::<SourceBoxContactHistory>().compounds.keys()
                    .map(|key| key.1).collect::<Vec<_>>();
                ids.sort_unstable();
                assert_eq!(ids, expected);
            }
            app.world_mut().get_mut::<Position>(foot).unwrap().0.x += 0.002;
            app.world_mut().get_mut::<Transform>(foot).unwrap().translation.x += 0.002;
            app.update();
            let after = surfaces(app.world());
            let retained = before.len() == after.len() && before.iter().zip(&after).all(|(before, after)| {
                before.len() == after.len() && before.iter().zip(after)
                    .all(|(before, after)| (*after - *before - Vec3::X * 0.002).length() < 2e-5)
            });
            assert_eq!(retained, enabled);
            if enabled {
                let old_tick = app.world().resource::<SourceBoxContactHistory>().compounds.keys().next().unwrap().2;
                app.world_mut().entity_mut(ground).insert(SourceCompoundOrder("SM_Als_Arch_01".into()));
                app.world_mut().get_mut::<Position>(foot).unwrap().0.x += 0.002;
                app.world_mut().get_mut::<Transform>(foot).unwrap().translation.x += 0.002;
                app.update();
                let new_tick = app.world().resource::<SourceBoxContactHistory>().compounds.keys().next().unwrap().2;
                assert_ne!(old_tick, new_tick);
                let replaced = surfaces(app.world());
                assert!(after.iter().zip(&replaced).any(|(before, after)| before.len() != after.len()
                    || before.iter().zip(after).any(|(before, after)| (*after - *before - Vec3::X * 0.002).length() >= 2e-5)));
            }
        }
    }

    fn check_source_convex_aggregate(fixtures: &[&str]) {
        use avian3d::collision::narrow_phase::NarrowPhaseSystems;
        use avian3d::prelude::{Gravity, LockedAxes, PhysicsPlugins, PhysicsSchedule, SleepingDisabled};
        use bevy::time::TimeUpdateStrategy;
        use bevy_animation_graph::core::ragdoll::definition::BodyMode;
        use crate::extensions::source_contact_convex::SourceConvexLifecycleSettings;
        let definitions: HashMap<String, Vec<Vec<[f32; 3]>>> = serde_json::from_str(include_str!(
            "../../assets/ALSExtras/Environment/Meshes/static_convex_collision.runtime.json")).unwrap();
        let vector = |value: &serde_json::Value| Vec3::new(value["x"].as_f64().unwrap() as f32,
            value["z"].as_f64().unwrap() as f32, value["y"].as_f64().unwrap() as f32);
        for fixture in fixtures {
            let fixture: serde_json::Value = serde_json::from_str(fixture).unwrap();
            let name = fixture["input"]["mesh_asset_path"].as_str().unwrap().rsplit('/').next().unwrap();
            let lower = vector(&fixture["geometry"]["minimum_cm"]);
            let upper = vector(&fixture["geometry"]["maximum_cm"]);
            let body = &fixture["input"]["body_translation_cm"];
            let body = Vec3::new(body[0].as_f64().unwrap() as f32, body[2].as_f64().unwrap() as f32,
                body[1].as_f64().unwrap() as f32);
            let size = (upper - lower) * 0.01;
            let center = (body + (lower + upper) * 0.5) * 0.01;
            let mut app = App::new();
            let [fraction, maximum, minimum] = include!("../als/settings/convex_margin.rs");
            let ([cull_cm, inverse_size_cm, minimum_scale, velocity_multiplier, max_velocity_expansion_cm], warm_start) = include!("../als/settings/contact_lifecycle.rs");
            let ([contact_scale, empty_position_scale, position_scale, empty_rotation_threshold, rotation_threshold],
                collision_tolerance_scale, minimum_points) = include!("../als/settings/contact_restore.rs");
            app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::new(PostUpdate)))
                .insert_resource(Gravity(Vec3::ZERO))
                .insert_resource(SourceConvexMarginSettings { fraction, maximum, minimum })
                .insert_resource(SourceConvexLifecycleSettings { cull_cm, inverse_size_cm, minimum_scale, velocity_multiplier, max_velocity_expansion_cm, warm_start })
                .insert_resource(SourceRestoreSettings { contact_scale, empty_position_scale, position_scale,
                    empty_rotation_threshold, rotation_threshold, collision_tolerance_scale, minimum_points })
                .init_resource::<SourceBoxContactHistory>()
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
                .add_systems(PhysicsSchedule, order_source_compound_groups.in_set(NarrowPhaseSystems::Last));
            app.finish();
            app.cleanup();
            let compound = Collider::compound(definitions[name].iter().map(|hull| (
                Vec3::ZERO, Quat::IDENTITY,
                Collider::convex_hull(hull.iter().copied().map(Vec3::from_array).collect()).unwrap(),
            )).collect());
            let ground = app.world_mut().spawn((RigidBody::Static, compound,
                SourceCompoundOrder(name.into()), Transform::IDENTITY)).id();
            let foot = app.world_mut().spawn((RigidBody::Dynamic, LockedAxes::ALL_LOCKED, SleepingDisabled,
                Collider::cuboid(size.x, size.y, size.z), Transform::from_translation(center),
                RagdollActiveMode(BodyMode::Dynamic))).id();
            app.update();
            app.update();
            let native = fixture["aggregate_contacts"].as_array().unwrap();
            let pair = app.world().resource::<ContactGraph>().get(ground, foot);
            if native.is_empty() && pair.is_none() { continue; }
            let (edge, pair) = pair.expect("The source active contact pair was not discovered");
            assert_eq!(pair.manifolds.len(), native.len());
            let history = app.world().resource::<SourceBoxContactHistory>();
            let mut shapes = history.compounds.keys().filter(|key| key.0 == pair.contact_id.0)
                .map(|key| key.1).collect::<Vec<_>>();
            shapes.sort_unstable();
            assert_eq!(shapes, native.iter().map(|value| value["mesh_shape_index"].as_u64().unwrap() as u32).collect::<Vec<_>>());
            if name == "SM_Als_Bench" && !native.iter().any(|value| value["mesh_shape_index"] == 2) {
                assert!(!history.convex_gjk.keys().any(|key| key.0 == pair.contact_id.0 && key.1 == 2));
            }
            let mut slots = (0..edge.constraint_handles.len()).collect::<Vec<_>>();
            slots.sort_by_key(|index| {
                let handle = edge.constraint_handles[*index];
                let color = handle.color_index as usize;
                (if color == COLOR_OVERFLOW_INDEX { 0 } else { color + 1 }, handle.local_index)
            });
            for (slot, source) in slots.into_iter().zip(native) {
                let manifold = &pair.manifolds[slot];
                let normal = manifold.normal * if pair.collider1 == foot { -1.0 } else { 1.0 };
                let expected = source["points"].as_array().unwrap();
                assert_eq!(manifold.points.len(), expected.len());
                assert!(normal.abs_diff_eq(vector(&expected[0]["world_normal"]), 2e-5));
                for (actual, expected) in manifold.points.iter().zip(expected) {
                    let midpoint = (vector(&expected["world_point_0_cm"]) + vector(&expected["world_point_1_cm"])) * 0.005;
                    assert!(actual.point.abs_diff_eq(midpoint, 2e-5));
                }
            }
        }
    }

    #[test]
    fn source_bench_positive_gap_hull_reaches_solver_in_native_order() {
        check_source_convex_aggregate(&[include_str!("../../docs/audit/bf144/d100b3b3c4c1/aggregate.native-sample.json"),
            include_str!("../../docs/audit/bf144/d100b3b3c4c3b/aggregate-outside.native-sample.json")]);
    }

    #[test]
    fn source_bench_isolated_positive_gap_activates_contact() {
        check_source_convex_aggregate(&[include_str!("../../docs/audit/bf144/d100b3b3c4c3b/isolated-inside.native-sample.json"),
            include_str!("../../docs/audit/bf144/d100b3b3c4c3b/isolated-outside.native-sample.json")]);
    }

    #[test]
    fn source_barrel_positive_gap_reaches_solver() {
        check_source_convex_aggregate(&[include_str!("../../docs/audit/bf144/d100b3b3c4c3c1/barrel-inside.native-sample.json"),
            include_str!("../../docs/audit/bf144/d100b3b3c4c3c1/barrel-outside.native-sample.json")]);
    }

    #[test]
    fn source_arch_edge_manifold_and_simplex_reach_actual_solver() {
        use avian3d::collision::narrow_phase::NarrowPhaseSystems;
        use avian3d::prelude::{Gravity, LockedAxes, PhysicsPlugins, PhysicsSchedule, SleepingDisabled};
        use bevy::time::TimeUpdateStrategy;
        use bevy_animation_graph::core::ragdoll::definition::BodyMode;
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3b3c2a/classification.native-samples.json")).unwrap();
        let sample = &fixture["samples"][2];
        let definitions: HashMap<String, Vec<Vec<[f32; 3]>>> = serde_json::from_str(include_str!(
            "../../assets/ALSExtras/Environment/Meshes/static_convex_collision.runtime.json")).unwrap();
        let vector = |value: &serde_json::Value| Vec3::new(value["x"].as_f64().unwrap() as f32,
            value["z"].as_f64().unwrap() as f32, value["y"].as_f64().unwrap() as f32);
        let lower = vector(&sample["geometry"]["minimum_cm"]);
        let upper = vector(&sample["geometry"]["maximum_cm"]);
        let body = &sample["input"]["body_translation_cm"];
        let body = Vec3::new(body[0].as_f64().unwrap() as f32, body[2].as_f64().unwrap() as f32,
            body[1].as_f64().unwrap() as f32);
        let size = (upper - lower) * 0.01;
        let center = (body + (lower + upper) * 0.5) * 0.01;
        for source_body in [false, true] {
            let mut app = App::new();
            let [fraction, maximum, minimum] = include!("../als/settings/convex_margin.rs");
            let ([cull_cm, inverse_size_cm, minimum_scale, velocity_multiplier, max_velocity_expansion_cm], warm_start) = include!("../als/settings/contact_lifecycle.rs");
            let ([contact_scale, empty_position_scale, position_scale, empty_rotation_threshold, rotation_threshold],
                collision_tolerance_scale, minimum_points) = include!("../als/settings/contact_restore.rs");
            app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::new(PostUpdate)))
                .insert_resource(Gravity(Vec3::ZERO))
                .insert_resource(SourceConvexMarginSettings { fraction, maximum, minimum })
                .insert_resource(SourceConvexLifecycleSettings { cull_cm, inverse_size_cm, minimum_scale, velocity_multiplier, max_velocity_expansion_cm, warm_start })
                .insert_resource(SourceRestoreSettings { contact_scale, empty_position_scale, position_scale,
                    empty_rotation_threshold, rotation_threshold, collision_tolerance_scale, minimum_points })
                .init_resource::<SourceBoxContactHistory>()
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
                .add_systems(PhysicsSchedule, order_source_compound_groups.in_set(NarrowPhaseSystems::Last));
            app.finish();
            app.cleanup();
            let compound = Collider::compound(definitions["SM_Als_Arch_02"].iter().map(|hull| (
                Vec3::ZERO, Quat::IDENTITY,
                Collider::convex_hull(hull.iter().copied().map(Vec3::from_array).collect()).unwrap(),
            )).collect());
            let ground = app.world_mut().spawn((RigidBody::Static, compound,
                SourceCompoundOrder("SM_Als_Arch_02".into()), Transform::IDENTITY)).id();
            let foot = app.world_mut().spawn((RigidBody::Dynamic, LockedAxes::ALL_LOCKED, SleepingDisabled,
                Collider::cuboid(size.x, size.y, size.z), Transform::from_translation(center))).id();
            if source_body { app.world_mut().entity_mut(foot).insert(RagdollActiveMode(BodyMode::Dynamic)); }
            app.update();
            app.update();
            let graph = app.world().resource::<ContactGraph>();
            let (edge, pair) = graph.get(ground, foot).unwrap();
            let history = app.world().resource::<SourceBoxContactHistory>();
            if source_body {
                let mut shapes = history.compounds.keys().filter(|key| key.0 == pair.contact_id.0)
                    .map(|key| key.1).collect::<Vec<_>>();
                shapes.sort_unstable();
                let position = shapes.iter().position(|index| *index == 2).unwrap();
                let slot = super::super::source_contact_restore::source_solver_slots(&edge.constraint_handles)[position];
                let manifold = &pair.manifolds[slot];
                let native = &sample["points"][0];
                assert_eq!(manifold.points.len(), 1);
                let normal = manifold.normal * if pair.collider1 == foot { -1.0 } else { 1.0 };
                assert!(normal.abs_diff_eq(vector(&native["world_normal"]), 2e-5));
                let midpoint = (vector(&native["world_point_0_cm"]) + vector(&native["world_point_1_cm"])) * 0.005;
                assert!(manifold.points[0].point.abs_diff_eq(midpoint, 2e-5));
                let contact_id = pair.contact_id.0;
                let previous = history.convex_gjk.iter().find(|(key, _)| key.0 == contact_id && key.1 == 2)
                    .unwrap().1.simplex.clone();
                app.update();
                let current = &app.world().resource::<SourceBoxContactHistory>().convex_gjk.iter()
                    .find(|(key, _)| key.0 == contact_id && key.1 == 2).unwrap().1.simplex;
                assert_eq!(current.len, previous.len);
                assert_eq!(current.weights, previous.weights);
            } else {
                assert!(history.compounds.is_empty() && history.convex_gjk.is_empty());
                let first = (app.world().get::<Collider>(pair.collider1).unwrap(),
                    *app.world().get::<Position>(pair.collider1).unwrap(), *app.world().get::<Rotation>(pair.collider1).unwrap());
                let second = (app.world().get::<Collider>(pair.collider2).unwrap(),
                    *app.world().get::<Position>(pair.collider2).unwrap(), *app.world().get::<Rotation>(pair.collider2).unwrap());
                let source_side = usize::from(pair.collider2 == ground);
                let ids = contact_subshape_ids(first.0, first.1, first.2, second.0, second.1, second.2,
                    0.005, &pair.manifolds).unwrap();
                let index = ids.iter().position(|value| value[source_side] == 2).unwrap();
                assert!(pair.manifolds[index].points.len() > 1);
            }
        }
    }

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
        use avian3d::collision::collider::contact_query::contact_manifolds;
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
        for (source_order, source_core) in [(false, false), (true, false), (true, true)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::new(PostUpdate)))
                .insert_resource(Gravity(Vec3::ZERO))
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
                .add_systems(PhysicsSchedule, order_source_compound_groups.in_set(NarrowPhaseSystems::Last));
            let [fraction, maximum, minimum] = include!("../als/settings/convex_margin.rs");
            let settings = SourceConvexMarginSettings { fraction, maximum, minimum };
            if source_core { app.insert_resource(settings); }
            app.finish();
            app.cleanup();
            let compound = Collider::compound(definitions[name].iter().map(|hull| (
                Vec3::ZERO, Quat::IDENTITY,
                Collider::convex_hull(hull.iter().copied().map(Vec3::from_array).collect()).unwrap(),
            )).collect());
            let floor = app.world_mut().spawn((RigidBody::Static, compound, Transform::IDENTITY)).id();
            if source_order { app.world_mut().entity_mut(floor).insert(SourceCompoundOrder("SM_Als_Arch_01".into())); }
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
            let mut original = Vec::new();
            contact_manifolds(first.0, first.1, first.2, second.0, second.1, second.2, 0.005, &mut original);
            let original_ids = contact_subshape_ids(first.0, first.1, first.2, second.0, second.1, second.2,
                0.005, &original).unwrap();
            let mut slots = edge.constraint_handles.iter().enumerate().map(|(index, handle)| {
                let manifold = &pair.manifolds[index];
                let candidates = original.iter().enumerate().filter(|(_, raw)| manifold.points.iter().all(|point| {
                    raw.points.iter().any(|candidate| candidate.point == point.point
                        && candidate.feature_id1 == point.feature_id1 && candidate.feature_id2 == point.feature_id2)
                })).map(|(index, _)| original_ids[index][source_side]).collect::<Vec<_>>();
                assert_eq!(candidates.len(), 1);
                if source_core {
                    let (box_shape, compound_shape) = if source_side == 0 { (second, first) } else { (first, second) };
                    let normal = source_box_convex_core_normal(box_shape.0, Pose::from_parts(box_shape.1.0, box_shape.2.0),
                        compound_shape.0, Pose::from_parts(compound_shape.1.0, compound_shape.2.0), candidates[0], settings).unwrap();
                    let expected = if source_side == 0 { -normal } else { normal };
                    assert!(manifold.normal.abs_diff_eq(expected, 1e-6));
                }
                let color = handle.color_index as usize;
                ((if color == COLOR_OVERFLOW_INDEX { 0 } else { color + 1 }, handle.local_index), candidates[0])
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
