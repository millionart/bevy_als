use avian3d::collision::contact_types::{ContactGraph, ContactId, ContactManifold, ContactPoint};
use avian3d::collision::narrow_phase::NarrowPhaseConfig;
use avian3d::dynamics::solver::{PhysicsLengthUnit, constraint_graph::{ConstraintGraph, ContactConstraintHandle, COLOR_OVERFLOW_INDEX}};
use avian3d::prelude::{AngularVelocity, Collider, ColliderTransform, CollisionMargin, ComputedCenterOfMass,
    LinearVelocity, Position, RigidBody, RigidBodyDisabled, Rotation, SpeculativeMargin};
use bevy::ecs::component::Tick;
use bevy::math::{DQuat, DVec3};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use std::collections::HashMap as SourceBoundsMap;
use std::sync::LazyLock;

use super::source_contact_polygon::{SOURCE_SMALL_NUMBER, source_direction, source_position};
use super::ragdoll_physics_profile::RagdollActiveMode;
use super::source_contact_convex::SourceConvexContactPoint;
use super::source_gjk_simplex::SourceGjkSimplex;

#[derive(Resource, Clone, Copy)]
pub(crate) struct SourceRestoreSettings {
    pub contact_scale: f64,
    pub empty_position_scale: f64,
    pub position_scale: f64,
    pub empty_rotation_threshold: f64,
    pub rotation_threshold: f64,
    pub collision_tolerance_scale: f32,
    pub minimum_points: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SourceRestorePose {
    pub position_m: DVec3,
    pub rotation: DQuat,
}

#[derive(Clone, Copy)]
pub(crate) struct SourceRestorePoint {
    pub initial_first_m: DVec3,
    pub initial_second_m: DVec3,
    pub normal_second: DVec3,
    pub second_m: DVec3,
    pub phi_m: f64,
    pub disabled: bool,
}

pub(super) fn source_float_position(value: DVec3) -> DVec3 {
    source_direction(source_position(value).as_vec3().as_dvec3()) * 0.01
}

pub(crate) fn source_restore_manifold(
    points: &mut [SourceRestorePoint],
    last: [SourceRestorePose; 2],
    current: [SourceRestorePose; 2],
    collision_tolerance_m: f64,
    allowed: bool,
    settings: SourceRestoreSettings,
) -> bool {
    if !allowed { return false; }
    let empty = points.is_empty();
    let position_scale = if empty { settings.empty_position_scale } else { settings.position_scale };
    let rotation_threshold = if empty { settings.empty_rotation_threshold } else { settings.rotation_threshold };
    let shape_tolerance = position_scale * collision_tolerance_m;
    if !(shape_tolerance > 0.0 && rotation_threshold > 0.0) { return false; }
    let translation = (current[0].position_m - current[1].position_m) - (last[0].position_m - last[1].position_m);
    if !(translation.x.abs() <= shape_tolerance && translation.y.abs() <= shape_tolerance
        && translation.z.abs() <= shape_tolerance) { return false; }
    let last_rotation = last[0].rotation.inverse() * last[1].rotation;
    let current_rotation = current[0].rotation.inverse() * current[1].rotation;
    if !(current_rotation.dot(last_rotation) > rotation_threshold) { return false; }
    let contact_tolerance_squared = (settings.contact_scale * collision_tolerance_m).powi(2);
    let mut active = points.len();
    for point in points.iter_mut() {
        if point.disabled { active -= 1; continue; }
        let world = current[0].position_m + current[0].rotation * point.initial_first_m;
        let first_in_second = current[1].rotation.inverse() * (world - current[1].position_m);
        let delta = first_in_second - point.initial_second_m;
        let phi = delta.dot(point.normal_second);
        let lateral = delta - phi * point.normal_second;
        if lateral.length_squared() < contact_tolerance_squared {
            point.second_m = source_float_position(first_in_second - phi * point.normal_second);
            point.phi_m = f64::from((phi * 100.0) as f32) * 0.01;
        } else {
            point.disabled = true;
            active -= 1;
        }
    }
    active >= points.len().min(settings.minimum_points)
}

#[derive(Clone)]
pub(super) struct SourceBoxContact {
    colliders: [Entity; 2],
    bodies: [Entity; 2],
    shape_ticks: [Tick; 2],
    last: [SourceRestorePose; 2],
    points: Vec<SourceRestorePoint>,
    templates: Vec<ContactPoint>,
}

#[derive(Resource, Default)]
pub(crate) struct SourceBoxContactHistory {
    entries: HashMap<u32, SourceBoxContact>,
    pub(super) compounds: HashMap<(u32, u32, Tick), SourceBoxContact>,
    pub(super) convex_gjk: HashMap<(u32, u32, Tick), SourceConvexGjkHistory>,
    meshes: HashMap<u32, SourceMeshHistory>,
}

#[derive(Clone)]
pub(super) struct SourceConvexGjkHistory {
    pub colliders: [Entity; 2],
    pub bodies: [Entity; 2],
    pub shape_ticks: [Tick; 2],
    pub simplex: SourceGjkSimplex,
}

impl SourceConvexGjkHistory {
    pub(super) fn matches(&self, context: &SourceContactContext) -> bool {
        self.colliders == context.colliders && self.bodies == context.bodies && self.shape_ticks == context.shape_ticks
    }
}

#[derive(Component)]
pub(crate) struct SourceMeshContact(pub String);

static SOURCE_MESH_BOUNDS: LazyLock<SourceBoundsMap<String, [[f64; 3]; 2]>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../assets/ALSExtras/Environment/Meshes/static_trimesh_bounds.runtime.json"))
        .expect("Generated source mesh bounds must be valid")
});

pub(super) fn source_scaled_bounds_max_extent_cm(bounds: [DVec3; 2], scale: Vec3) -> f32 {
    let scale = scale.as_dvec3();
    ((bounds[1] * 100.0) * scale - (bounds[0] * 100.0) * scale).abs().max_element() as f32
}

pub(super) fn source_mesh_max_extent_cm(asset: &str, scale: Vec3) -> Option<f32> {
    let bounds = (*SOURCE_MESH_BOUNDS.get(asset)?).map(DVec3::from_array);
    Some(source_scaled_bounds_max_extent_cm(bounds, scale))
}

pub(super) fn source_box_shape_pose(
    position: &Position,
    rotation: &Rotation,
    local: Option<&ColliderTransform>,
    body_position: &Position,
    body_rotation: &Rotation,
) -> SourceRestorePose {
    let identity = local.is_some_and(|value| {
        let rotation = value.rotation.0;
        rotation.x.abs() <= SOURCE_SMALL_NUMBER && rotation.y.abs() <= SOURCE_SMALL_NUMBER
            && rotation.z.abs() <= SOURCE_SMALL_NUMBER && (rotation.w.abs() - 1.0).abs() <= SOURCE_SMALL_NUMBER
    });
    if identity {
        SourceRestorePose { position_m: body_position.0.as_dvec3(), rotation: body_rotation.0.as_dquat() }
    } else {
        SourceRestorePose { position_m: position.0.as_dvec3(), rotation: rotation.0.as_dquat() }
    }
}

fn contact_normal_speed(linear: [Vec3; 2], angular: [Vec3; 2], anchors: [Vec3; 2], normal: Vec3) -> f32 {
    (linear[1] - linear[0] + angular[1].cross(anchors[1]) - angular[0].cross(anchors[0])).dot(normal)
}

#[derive(Clone, Copy)]
pub(super) struct SourceContactContext {
    pub colliders: [Entity; 2],
    pub bodies: [Entity; 2],
    pub shape_ticks: [Tick; 2],
    pub frames: [SourceRestorePose; 2],
    pub tolerance: f64,
    pub skin: f32,
    pub box_first: bool,
    pub surface_normal: Vec3,
    pub centers: [Vec3; 2],
    pub linear: [Vec3; 2],
    pub angular: [Vec3; 2],
}

pub(super) fn capture_source_contact(manifold: &ContactManifold, context: &SourceContactContext) -> SourceBoxContact {
    let SourceContactContext { colliders, bodies, shape_ticks, frames, skin, box_first, surface_normal, .. } = *context;
    let normal = manifold.normal.as_dvec3() * if box_first { -1.0 } else { 1.0 };
    let normal = frames[1].rotation.inverse() * normal;
    let normal = source_direction(source_direction(normal).as_vec3().as_dvec3());
    let points = manifold.points.iter().map(|point| {
        let offset = surface_normal.as_dvec3() * (f64::from(point.penetration - skin) * 0.5);
        let first = point.point.as_dvec3() + offset;
        let second = point.point.as_dvec3() - offset;
        let (first, second) = if box_first { (first, second) } else { (second, first) };
        let first = source_float_position(frames[0].rotation.inverse() * (first - frames[0].position_m));
        let second = source_float_position(frames[1].rotation.inverse() * (second - frames[1].position_m));
        SourceRestorePoint { initial_first_m: first, initial_second_m: second, normal_second: normal,
            second_m: second, phi_m: -f64::from(point.penetration - skin), disabled: false }
    }).collect();
    SourceBoxContact { colliders, bodies, shape_ticks, last: frames, points, templates: manifold.points.clone() }
}

pub(super) fn source_generated_manifold(
    template: &ContactManifold,
    points: &[SourceConvexContactPoint],
    context: &SourceContactContext,
) -> ContactManifold {
    let mut manifold = template.clone();
    let normal = context.frames[1].rotation * points[0].normal_second;
    manifold.normal = (normal * if context.box_first { -1.0 } else { 1.0 }).as_vec3();
    let normal = manifold.normal;
    manifold.points = points.iter().enumerate().map(|(index, point)| {
        let first = context.frames[0].position_m + context.frames[0].rotation * point.first_m;
        let second = context.frames[1].position_m + context.frames[1].rotation * point.second_m;
        let midpoint = ((first + second) * 0.5).as_vec3();
        let mut result = template.points.get(index).copied().unwrap_or_else(||
            ContactPoint::new(Vec3::ZERO, Vec3::ZERO, midpoint, 0.0));
        result.point = midpoint;
        result.anchor1 = midpoint - context.centers[0];
        result.anchor2 = midpoint - context.centers[1];
        result.penetration = (-point.phi_m) as f32 + context.skin;
        result.normal_speed = contact_normal_speed(context.linear, context.angular, [result.anchor1, result.anchor2], normal);
        result
    }).collect();
    manifold
}

pub(super) fn capture_source_exact_contact(
    manifold: &ContactManifold,
    points: &[SourceConvexContactPoint],
    context: &SourceContactContext,
) -> SourceBoxContact {
    let points = points.iter().map(|point| SourceRestorePoint { initial_first_m: point.first_m,
        initial_second_m: point.second_m, normal_second: point.normal_second, second_m: point.second_m,
        phi_m: point.phi_m, disabled: false }).collect();
    SourceBoxContact { colliders: context.colliders, bodies: context.bodies, shape_ticks: context.shape_ticks,
        last: context.frames, points, templates: manifold.points.clone() }
}

pub(super) fn restore_source_contact(
    manifold: &mut ContactManifold,
    previous: Option<&SourceBoxContact>,
    context: &SourceContactContext,
    settings: SourceRestoreSettings,
) -> Option<SourceBoxContact> {
    let prior = previous.filter(|saved| saved.colliders == context.colliders
        && saved.bodies == context.bodies && saved.shape_ticks == context.shape_ticks)?;
    let mut saved = prior.clone();
    if !source_restore_manifold(&mut saved.points, saved.last, context.frames, context.tolerance, true, settings) {
        return None;
    }
    apply_source_restored_contact(manifold, &mut saved, context);
    Some(saved)
}

fn apply_source_restored_contact(manifold: &mut ContactManifold, saved: &mut SourceBoxContact, context: &SourceContactContext) {
    let normal = context.frames[1].rotation * saved.points[0].normal_second;
    manifold.normal = (normal * if context.box_first { -1.0 } else { 1.0 }).as_vec3();
    let normal = manifold.normal;
    let mut points = Vec::new();
    for (index, point) in saved.points.iter().enumerate().filter(|(_, point)| !point.disabled) {
        let mut result = manifold.points.get(index).copied().unwrap_or(saved.templates[index]);
        let first = context.frames[0].position_m + context.frames[0].rotation * point.initial_first_m;
        let second = context.frames[1].position_m + context.frames[1].rotation * point.second_m;
        let midpoint = ((first + second) * 0.5).as_vec3();
        result.point = midpoint;
        result.anchor1 = midpoint - context.centers[0];
        result.anchor2 = midpoint - context.centers[1];
        result.penetration = (-point.phi_m) as f32 + context.skin;
        result.normal_speed = contact_normal_speed(context.linear, context.angular, [result.anchor1, result.anchor2], normal);
        saved.templates[index] = result;
        points.push(result);
    }
    manifold.points = points;
}

fn source_restore_contact_groups(
    groups: &mut [SourceBoxContact],
    context: &SourceContactContext,
    settings: SourceRestoreSettings,
) -> bool {
    let Some(first) = groups.first() else { return false; };
    if groups.iter().any(|group| group.colliders != context.colliders || group.bodies != context.bodies
        || group.shape_ticks != context.shape_ticks || group.last != first.last) { return false; }
    let mut points = groups.iter().flat_map(|group| group.points.iter().copied()).collect::<Vec<_>>();
    if !source_restore_manifold(&mut points, first.last, context.frames, context.tolerance, true, settings) {
        return false;
    }
    for (destination, point) in groups.iter_mut().flat_map(|group| group.points.iter_mut()).zip(points) {
        *destination = point;
    }
    true
}

#[derive(Clone)]
struct SourceMeshContacts {
    manifolds: Vec<ContactManifold>,
    contacts: Vec<SourceBoxContact>,
}

#[derive(Clone)]
struct SourceMeshHistory {
    source_tick: Tick,
    contacts: SourceMeshContacts,
}

fn capture_source_mesh_contacts(manifolds: &[ContactManifold], context: &SourceContactContext) -> SourceMeshContacts {
    let contacts = manifolds.iter().map(|manifold| {
        let mut context = *context;
        context.surface_normal = manifold.normal;
        capture_source_contact(manifold, &context)
    }).collect();
    SourceMeshContacts { manifolds: manifolds.to_vec(), contacts }
}

fn restore_source_mesh_contacts(
    saved: &mut SourceMeshContacts,
    context: &SourceContactContext,
    settings: SourceRestoreSettings,
) -> Option<Vec<ContactManifold>> {
    let mut contacts = saved.contacts.clone();
    if !source_restore_contact_groups(&mut contacts, context, settings) { return None; }
    let mut manifolds = Vec::new();
    for (template, contact) in saved.manifolds.iter().zip(&mut contacts) {
        if contact.points.iter().all(|point| point.disabled) { continue; }
        let mut manifold = template.clone();
        manifold.points = contact.templates.clone();
        apply_source_restored_contact(&mut manifold, contact, context);
        manifolds.push(manifold);
    }
    saved.contacts = contacts;
    Some(manifolds)
}

pub(crate) fn restore_source_box_contacts(
    settings: Option<Res<SourceRestoreSettings>>,
    config: Res<NarrowPhaseConfig>,
    length: Res<PhysicsLengthUnit>,
    time: Res<Time>,
    ragdolls: Query<(), With<RagdollActiveMode>>,
    colliders: Query<(Ref<Collider>, &Position, &Rotation, Option<&CollisionMargin>, Option<&ColliderTransform>, Option<&SpeculativeMargin>)>,
    bodies: Query<(&RigidBody, &Position, &Rotation, &ComputedCenterOfMass, Option<&CollisionMargin>,
        &LinearVelocity, &AngularVelocity, Option<&SpeculativeMargin>), Without<RigidBodyDisabled>>,
    mut history: ResMut<SourceBoxContactHistory>,
    mut graph: ResMut<ContactGraph>,
) {
    let Some(settings) = settings else { history.entries.clear(); return; };
    let mut next = HashMap::default();
    let mut updates = Vec::new();
    for pair in graph.iter_active_touching() {
        if !pair.generates_constraints() || pair.manifolds.len() != 1 { continue; }
        let Some(body1) = pair.body1 else { continue; };
        let Some(body2) = pair.body2 else { continue; };
        let box_first = if ragdolls.contains(body1) { true } else if ragdolls.contains(body2) { false } else { continue; };
        let (box_entity, other_entity, box_body, other_body) = if box_first {
            (pair.collider1, pair.collider2, body1, body2)
        } else { (pair.collider2, pair.collider1, body2, body1) };
        let (Ok(box_shape), Ok(other_shape), Ok(box_data), Ok(other_data)) =
            (colliders.get(box_entity), colliders.get(other_entity), bodies.get(box_body), bodies.get(other_body)) else { continue; };
        if !box_data.0.is_dynamic() || !other_data.0.is_static()
            || box_shape.0.shape_scaled().as_cuboid().is_none() || other_shape.0.shape_scaled().as_cuboid().is_none() { continue; }
        let frames = [source_box_shape_pose(box_shape.1, box_shape.2, box_shape.4, box_data.1, box_data.2),
            source_box_shape_pose(other_shape.1, other_shape.2, other_shape.4, other_data.1, other_data.2)];
        let ticks = [box_shape.0.last_changed(), other_shape.0.last_changed()];
        let identity = ([box_entity, other_entity], [box_body, other_body]);
        let first_bounds = box_shape.0.shape_scaled().compute_local_aabb();
        let second_bounds = other_shape.0.shape_scaled().compute_local_aabb();
        let first_size_cm = (f64::from((first_bounds.maxs - first_bounds.mins).abs().max_element()) * 100.0) as f32;
        let second_size_cm = (f64::from((second_bounds.maxs - second_bounds.mins).abs().max_element()) * 100.0) as f32;
        let tolerance = f64::from(settings.collision_tolerance_scale * first_size_cm.min(second_size_cm)) * 0.01;
        let skin = box_shape.3.or(box_data.4).map_or(0.0, |value| value.0)
            + other_shape.3.or(other_data.4).map_or(0.0, |value| value.0);
        let ordered_bodies = if box_first { [box_data, other_data] } else { [other_data, box_data] };
        let margins = if box_first { [box_shape.5, other_shape.5] } else { [other_shape.5, box_shape.5] };
        let linear = [0, 1].map(|index| {
            let body = ordered_bodies[index];
            let margin = margins[index].or(body.7).map_or(length.0 * config.default_speculative_margin, |value| value.0);
            if margin < f32::MAX { body.5.0.clamp_length_max(margin / time.delta_secs()) } else { body.5.0 }
        });
        let mut manifold = pair.manifolds[0].clone();
        let context = SourceContactContext { colliders: identity.0, bodies: identity.1, shape_ticks: ticks,
            frames, tolerance, skin, box_first, surface_normal: manifold.normal,
            centers: ordered_bodies.map(|body| body.1.0 + body.2.0 * body.3.0),
            linear, angular: ordered_bodies.map(|body| body.6.0) };
        let saved = if let Some(saved) = restore_source_contact(&mut manifold, history.entries.get(&pair.contact_id.0),
            &context, *settings) {
            updates.push((pair.contact_id, manifold));
            saved
        } else { capture_source_contact(&manifold, &context) };
        next.insert(pair.contact_id.0, saved);
    }
    for pair in graph.iter_sleeping_touching() {
        if let Some(saved) = history.entries.get(&pair.contact_id.0)
            && graph.get_by_id(pair.contact_id).is_some_and(|(edge, _)| edge.is_sleeping()) {
            next.insert(pair.contact_id.0, saved.clone());
        }
    }
    for (id, manifold) in updates {
        if let Some((_, pair)) = graph.get_mut_by_id(id) { pair.manifolds[0] = manifold; }
    }
    history.entries = next;
}

pub(super) fn source_solver_slots(handles: &[ContactConstraintHandle]) -> Vec<usize> {
    let mut slots = (0..handles.len()).collect::<Vec<_>>();
    slots.sort_by_key(|index| {
        let handle = handles[*index];
        let color = handle.color_index as usize;
        (if color == COLOR_OVERFLOW_INDEX { 0 } else { color + 1 }, handle.local_index)
    });
    slots
}

pub(super) fn write_source_solver_groups(
    graph: &mut ContactGraph,
    constraints: &mut ConstraintGraph,
    id: ContactId,
    manifolds: Vec<ContactManifold>,
) {
    let Some((edge, pair)) = graph.get_by_id(id) else { return; };
    let (Some(body1), Some(body2)) = (pair.body1, pair.body2) else { return; };
    let count = edge.constraint_handles.len();
    for _ in manifolds.len()..count {
        constraints.pop_manifold(&mut graph.edges, id, body1, body2);
    }
    if let Some((edge, pair)) = graph.get_mut_by_id(id) {
        pair.manifolds = manifolds;
        while edge.constraint_handles.len() < pair.manifolds.len() {
            constraints.push_manifold(edge, pair);
        }
        let slots = source_solver_slots(&edge.constraint_handles);
        let ordered = pair.manifolds.clone();
        for (slot, manifold) in slots.into_iter().zip(ordered) { pair.manifolds[slot] = manifold; }
    }
}

pub(crate) fn restore_source_mesh_manifolds(
    settings: Option<Res<SourceRestoreSettings>>,
    config: Res<NarrowPhaseConfig>,
    length: Res<PhysicsLengthUnit>,
    time: Res<Time>,
    ragdolls: Query<(), With<RagdollActiveMode>>,
    sources: Query<Ref<SourceMeshContact>>,
    colliders: Query<(Ref<Collider>, &Position, &Rotation, Option<&CollisionMargin>, Option<&ColliderTransform>, Option<&SpeculativeMargin>)>,
    bodies: Query<(&RigidBody, &Position, &Rotation, &ComputedCenterOfMass, Option<&CollisionMargin>,
        &LinearVelocity, &AngularVelocity, Option<&SpeculativeMargin>), Without<RigidBodyDisabled>>,
    mut history: ResMut<SourceBoxContactHistory>,
    mut graph: ResMut<ContactGraph>,
    mut constraints: ResMut<ConstraintGraph>,
) {
    let Some(settings) = settings else { history.meshes.clear(); return; };
    let mut next = HashMap::default();
    let mut updates = Vec::new();
    for pair in graph.iter_active_touching() {
        if !pair.generates_constraints() || pair.manifolds.is_empty() { continue; }
        let Some((edge, _)) = graph.get_by_id(pair.contact_id) else { continue; };
        if edge.constraint_handles.len() != pair.manifolds.len() { continue; }
        let (mesh_first, source) = if let Ok(source) = sources.get(pair.collider1) { (true, source) }
            else if let Ok(source) = sources.get(pair.collider2) { (false, source) } else { continue; };
        let (Some(body1), Some(body2)) = (pair.body1, pair.body2) else { continue; };
        let (box_entity, mesh_entity, box_body, mesh_body) = if mesh_first {
            (pair.collider2, pair.collider1, body2, body1)
        } else { (pair.collider1, pair.collider2, body1, body2) };
        if !ragdolls.contains(box_body) { continue; }
        let (Ok(box_shape), Ok(mesh_shape), Ok(box_data), Ok(mesh_data)) =
            (colliders.get(box_entity), colliders.get(mesh_entity), bodies.get(box_body), bodies.get(mesh_body)) else { continue; };
        if !box_data.0.is_dynamic() || !mesh_data.0.is_static()
            || box_shape.0.shape_scaled().as_cuboid().is_none() || mesh_shape.0.shape_scaled().as_trimesh().is_none() { continue; }
        let Some(mesh_size_cm) = source_mesh_max_extent_cm(&source.0, mesh_shape.0.scale()) else { continue; };
        let frames = [source_box_shape_pose(box_shape.1, box_shape.2, box_shape.4, box_data.1, box_data.2),
            SourceRestorePose { position_m: mesh_shape.1.0.as_dvec3(), rotation: mesh_shape.2.0.as_dquat() }];
        let bounds = box_shape.0.shape_scaled().compute_local_aabb();
        let box_size_cm = (f64::from((bounds.maxs - bounds.mins).abs().max_element()) * 100.0) as f32;
        let skin = box_shape.3.or(box_data.4).map_or(0.0, |value| value.0)
            + mesh_shape.3.or(mesh_data.4).map_or(0.0, |value| value.0);
        let ordered_bodies = if mesh_first { [mesh_data, box_data] } else { [box_data, mesh_data] };
        let margins = if mesh_first { [mesh_shape.5, box_shape.5] } else { [box_shape.5, mesh_shape.5] };
        let linear = [0, 1].map(|index| {
            let body = ordered_bodies[index];
            let margin = margins[index].or(body.7).map_or(length.0 * config.default_speculative_margin, |value| value.0);
            if margin < f32::MAX { body.5.0.clamp_length_max(margin / time.delta_secs()) } else { body.5.0 }
        });
        let context = SourceContactContext { colliders: [box_entity, mesh_entity], bodies: [box_body, mesh_body],
            shape_ticks: [box_shape.0.last_changed(), mesh_shape.0.last_changed()], frames,
            tolerance: f64::from(settings.collision_tolerance_scale * box_size_cm.min(mesh_size_cm)) * 0.01,
            skin, box_first: !mesh_first, surface_normal: Vec3::ZERO,
            centers: ordered_bodies.map(|body| body.1.0 + body.2.0 * body.3.0),
            linear, angular: ordered_bodies.map(|body| body.6.0) };
        let source_tick = source.last_changed();
        let prior = history.meshes.get(&pair.contact_id.0).filter(|saved| saved.source_tick == source_tick);
        let mut saved = prior.cloned().unwrap_or_else(|| SourceMeshHistory { source_tick, contacts:
            capture_source_mesh_contacts(&source_solver_slots(&edge.constraint_handles).into_iter()
                .map(|index| pair.manifolds[index].clone()).collect::<Vec<_>>(), &context) });
        let restored = prior.and_then(|_| restore_source_mesh_contacts(&mut saved.contacts, &context, *settings));
        if let Some(mut restored) = restored {
            let current = &pair.manifolds[0];
            for manifold in &mut restored {
                manifold.friction = current.friction;
                manifold.restitution = current.restitution;
                manifold.tangent_velocity = current.tangent_velocity;
            }
            updates.push((pair.contact_id, restored));
        } else if prior.is_some() {
            saved.contacts = capture_source_mesh_contacts(&source_solver_slots(&edge.constraint_handles).into_iter()
                .map(|index| pair.manifolds[index].clone()).collect::<Vec<_>>(), &context);
        }
        next.insert(pair.contact_id.0, saved);
    }
    for (&id, saved) in &history.meshes {
        if graph.get_by_id(ContactId(id)).is_some_and(|(edge, pair)| edge.is_sleeping() && pair.is_touching()) {
            next.insert(id, saved.clone());
        }
    }
    for (id, manifolds) in updates {
        write_source_solver_groups(&mut graph, &mut constraints, id, manifolds);
    }
    history.meshes = next;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_mesh_extents_match_native_meshes_and_scales() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d104b3a/bounds.native-samples.json")).unwrap();
        for sample in fixture["samples"].as_array().unwrap() {
            let scale = Vec3::new(sample["scale"][0].as_f64().unwrap() as f32,
                sample["scale"][1].as_f64().unwrap() as f32, sample["scale"][2].as_f64().unwrap() as f32);
            let actual = source_mesh_max_extent_cm(sample["mesh"].as_str().unwrap(), scale).unwrap();
            assert_eq!(actual, sample["native_max_extent_cm"].as_f64().unwrap() as f32, "{}", sample["mesh"]);
        }
        assert!(source_mesh_max_extent_cm("missing", Vec3::ONE).is_none());
    }

    fn settings() -> SourceRestoreSettings {
        let ([contact_scale, empty_position_scale, position_scale, empty_rotation_threshold, rotation_threshold],
            collision_tolerance_scale, minimum_points) = include!("../als/settings/contact_restore.rs");
        SourceRestoreSettings { contact_scale, empty_position_scale, position_scale, empty_rotation_threshold,
            rotation_threshold, collision_tolerance_scale, minimum_points }
    }

    fn object_vector(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value["x"].as_f64().unwrap(), value["z"].as_f64().unwrap(), value["y"].as_f64().unwrap())
    }

    fn array_vector(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value[0].as_f64().unwrap(), value[2].as_f64().unwrap(), value[1].as_f64().unwrap())
    }

    fn array_rotation(value: &serde_json::Value) -> DQuat {
        DQuat::from_xyzw(-value[0].as_f64().unwrap(), -value[2].as_f64().unwrap(),
            -value[1].as_f64().unwrap(), value[3].as_f64().unwrap())
    }

    fn object_rotation(value: &serde_json::Value) -> DQuat {
        DQuat::from_xyzw(-value["x"].as_f64().unwrap(), -value["z"].as_f64().unwrap(),
            -value["y"].as_f64().unwrap(), value["w"].as_f64().unwrap())
    }

    fn pose(input: &serde_json::Value, translation: &str, rotation: &str, local: Option<&serde_json::Value>) -> SourceRestorePose {
        let position = array_vector(&input[translation]) * 0.01;
        let orientation = array_rotation(&input[rotation]);
        if let Some(local) = local {
            SourceRestorePose { position_m: position + orientation * (object_vector(&local["translation"]) * 0.01),
                rotation: orientation * object_rotation(&local["rotation"]) }
        } else {
            SourceRestorePose { position_m: position, rotation: orientation }
        }
    }

    #[test]
    fn source_manifold_restore_matches_native_two_frame_contacts() {
        let fixtures = [include_str!("../../docs/audit/bf144/d104a/restore.native-samples.json"),
            include_str!("../../docs/audit/bf144/d104b2b/restore.native-samples.json"),
            include_str!("../../docs/audit/bf144/d104b3b/restore.native-samples.json")];
        let settings = settings();
        for fixture in fixtures {
            let fixture: serde_json::Value = serde_json::from_str(fixture).unwrap();
            for sample in fixture["samples"].as_array().unwrap() {
                let input = &sample["input"];
                let next = &input["next"];
                let local = &sample["geometry"]["shape_to_body"];
                let other = sample.get("other_geometry");
                let (translation_key, rotation_key) = if other.is_some() {
                    ("mesh_translation_cm", "mesh_rotation_xyzw")
                } else { ("box_translation_cm", "box_rotation_xyzw") };
                let other_local = other.map(|value| &value["shape_to_body"]);
                let last = [pose(input, "body_translation_cm", "body_rotation_xyzw", Some(local)),
                    pose(input, translation_key, rotation_key, other_local)];
                let current = [pose(next, "body_translation_cm", "body_rotation_xyzw", Some(local)),
                    pose(next, translation_key, rotation_key, other_local)];
                let mut points = sample["points"].as_array().unwrap().iter().map(|point| {
                    let first = object_vector(&point["shape_point_0_cm"]) * 0.01;
                    let second = object_vector(&point["shape_point_1_cm"]) * 0.01;
                    let local_normal = last[1].rotation.inverse() * object_vector(&point["world_normal"]);
                    let local_normal = source_direction(source_direction(local_normal).as_vec3().as_dvec3());
                    SourceRestorePoint { initial_first_m: first, initial_second_m: second, normal_second: local_normal,
                        second_m: second, phi_m: point["phi_cm"].as_f64().unwrap() * 0.01, disabled: false }
                }).collect::<Vec<_>>();
                let tolerance = if sample["geometry"]["kind"] == "box" {
                    let shape = &sample["geometry"];
                    let first_size = (object_vector(&shape["maximum_cm"]) - object_vector(&shape["minimum_cm"]))
                        .abs().max_element() as f32;
                    let second_size = if let Some(other) = other {
                        (object_vector(&other["bounds_max_cm"]) - object_vector(&other["bounds_min_cm"]))
                            .abs().max_element() as f32
                    } else { (array_vector(&input["box_half_extents_cm"]) * 2.0).abs().max_element() as f32 };
                    f64::from(settings.collision_tolerance_scale * first_size.min(second_size)) * 0.01
                } else { 1.0 };
                let restored = source_restore_manifold(&mut points, last, current, tolerance,
                    sample["restore_allowed"].as_bool().unwrap(), settings);
                assert_eq!(restored, sample["restored"].as_bool().unwrap(), "{}", input["label"]);
                if restored {
                    let expected = sample["next_points"].as_array().unwrap();
                    assert_eq!(points.iter().filter(|point| !point.disabled).count(), expected.len());
                    for (point, expected) in points.iter().filter(|point| !point.disabled).zip(expected) {
                        assert!(point.second_m.distance(object_vector(&expected["shape_point_1_cm"]) * 0.01) < 2e-6,
                            "{}", input["label"]);
                        assert!((point.phi_m - expected["phi_cm"].as_f64().unwrap() * 0.01).abs() < 2e-6,
                            "{}", input["label"]);
                    }
                }
            }
        }
    }

    #[test]
    fn source_manifold_restore_checks_component_rotation_and_active_count() {
        let settings = settings();
        let pose = SourceRestorePose { position_m: DVec3::ZERO, rotation: DQuat::IDENTITY };
        let point = SourceRestorePoint { initial_first_m: DVec3::ZERO, initial_second_m: DVec3::ZERO,
            normal_second: DVec3::Y, second_m: DVec3::ZERO, phi_m: 0.0, disabled: false };
        let mut points = [point];
        let limit = settings.position_scale * 0.1;
        let translated = SourceRestorePose { position_m: DVec3::new(limit, 0.0, limit), ..pose };
        assert!(source_restore_manifold(&mut points, [pose; 2], [translated, pose], 0.1, true, settings));
        let beyond = SourceRestorePose { position_m: DVec3::X * (limit * 1.01), ..pose };
        assert!(!source_restore_manifold(&mut [point], [pose; 2], [beyond, pose], 0.1, true, settings));
        let angle = settings.rotation_threshold.acos() * 2.0;
        let rotated = SourceRestorePose { rotation: DQuat::from_rotation_y(angle * 1.1), ..pose };
        assert!(!source_restore_manifold(&mut [point], [pose; 2], [rotated, pose], 0.1, true, settings));
        let mut enough = [point; 5];
        enough[0].disabled = true;
        assert!(source_restore_manifold(&mut enough, [pose; 2], [pose; 2], 0.1, true, settings));
        enough[1].disabled = true;
        assert!(!source_restore_manifold(&mut enough, [pose; 2], [pose; 2], 0.1, true, settings));
        assert!(source_restore_manifold(&mut [], [pose; 2], [pose; 2], 0.1, true, settings));
    }

    #[test]
    fn source_box_shape_origin_matches_native_center_encoding() {
        let body = Position(Vec3::new(2.0, 3.0, 4.0));
        let body_rotation = Rotation(Quat::from_rotation_x(0.4));
        let position = Position(Vec3::new(2.5, 3.0, 4.0));
        let rotation = Rotation(Quat::IDENTITY);
        let mut local = ColliderTransform { translation: Vec3::X * 0.5, ..Default::default() };
        let frame = source_box_shape_pose(&position, &rotation, Some(&local), &body, &body_rotation);
        assert_eq!(frame.position_m, body.0.as_dvec3());
        assert_eq!(frame.rotation, body_rotation.0.as_dquat());
        local.rotation = Rotation(Quat::from_rotation_z(SOURCE_SMALL_NUMBER));
        assert_eq!(source_box_shape_pose(&position, &rotation, Some(&local), &body, &body_rotation).rotation,
            body_rotation.0.as_dquat());
        local.rotation = Rotation(Quat::from_rotation_z(0.2));
        let frame = source_box_shape_pose(&position, &rotation, Some(&local), &body, &body_rotation);
        assert_eq!(frame.position_m, position.0.as_dvec3());
        assert_eq!(frame.rotation, rotation.0.as_dquat());
        local.rotation = Rotation(-Quat::IDENTITY);
        assert_eq!(source_box_shape_pose(&position, &rotation, Some(&local), &body, &body_rotation).position_m,
            body.0.as_dvec3());
    }

    #[test]
    fn restored_contact_recomputes_normal_speed_at_the_old_surface() {
        let speed = contact_normal_speed([Vec3::ZERO, Vec3::Y], [Vec3::Z * 2.0, Vec3::ZERO],
            [Vec3::X * 0.5, Vec3::ZERO], Vec3::Y);
        assert_eq!(speed, 0.0);
        let moved = contact_normal_speed([Vec3::ZERO, Vec3::Y], [Vec3::Z * 2.0, Vec3::ZERO],
            [Vec3::X, Vec3::ZERO], Vec3::Y);
        assert_eq!(moved, -1.0);
    }

    #[test]
    fn source_contact_history_rejects_replaced_shapes_and_bodies() {
        let entity = |index| Entity::from_raw_u32(index).unwrap();
        let pose = SourceRestorePose { position_m: DVec3::ZERO, rotation: DQuat::IDENTITY };
        let context = SourceContactContext { colliders: [entity(1), entity(2)], bodies: [entity(3), entity(4)],
            shape_ticks: [Tick::new(1), Tick::new(2)], frames: [pose; 2], tolerance: 0.01, skin: 0.0,
            box_first: true, surface_normal: Vec3::Y, centers: [Vec3::ZERO; 2],
            linear: [Vec3::ZERO; 2], angular: [Vec3::ZERO; 2] };
        let manifold = ContactManifold::new([ContactPoint::new(Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 0.0)], Vec3::Y);
        let saved = capture_source_contact(&manifold, &context);
        assert!(restore_source_contact(&mut manifold.clone(), Some(&saved), &context, settings()).is_some());
        let mut changed = context;
        changed.colliders[0] = entity(5);
        assert!(restore_source_contact(&mut manifold.clone(), Some(&saved), &changed, settings()).is_none());
        changed = context;
        changed.bodies[1] = entity(6);
        assert!(restore_source_contact(&mut manifold.clone(), Some(&saved), &changed, settings()).is_none());
        changed = context;
        changed.shape_ticks[0] = Tick::new(3);
        assert!(restore_source_contact(&mut manifold.clone(), Some(&saved), &changed, settings()).is_none());
    }

    #[test]
    fn source_generated_contacts_keep_both_native_surfaces_for_restore() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3b3c2a/classification.native-samples.json")).unwrap();
        let sample = &fixture["samples"][1];
        let position = array_vector(&sample["input"]["body_translation_cm"]) * 0.01;
        let box_pose = SourceRestorePose { position_m: position, rotation: DQuat::IDENTITY };
        let mesh_pose = SourceRestorePose { position_m: DVec3::ZERO, rotation: DQuat::IDENTITY };
        let entity = |index| Entity::from_raw_u32(index).unwrap();
        let points = sample["points"].as_array().unwrap().iter().map(|point| SourceConvexContactPoint {
            first_m: object_vector(&point["shape_point_0_cm"]) * 0.01,
            second_m: object_vector(&point["shape_point_1_cm"]) * 0.01,
            normal_second: object_vector(&point["world_normal"]), phi_m: point["phi_cm"].as_f64().unwrap() * 0.01,
        }).collect::<Vec<_>>();
        for box_first in [false, true] {
            let mut context = SourceContactContext { colliders: [entity(1), entity(2)], bodies: [entity(3), entity(4)],
                shape_ticks: [Tick::new(1), Tick::new(2)], frames: [box_pose, mesh_pose], tolerance: 0.028,
                skin: 0.0, box_first, surface_normal: Vec3::ZERO,
                centers: if box_first { [position.as_vec3(), Vec3::ZERO] } else { [Vec3::ZERO, position.as_vec3()] },
                linear: [Vec3::ZERO; 2], angular: [Vec3::ZERO; 2] };
            let template = ContactManifold::new([ContactPoint::new(Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 0.0)], Vec3::Y);
            let mut manifold = source_generated_manifold(&template, &points, &context);
            for (actual, native) in manifold.points.iter().zip(&points) {
                let midpoint = ((position + native.first_m + native.second_m) * 0.5).as_vec3();
                assert!(actual.point.abs_diff_eq(midpoint, 1e-6));
                assert!((actual.penetration + native.phi_m as f32).abs() < 1e-6);
            }
            let cached = capture_source_exact_contact(&manifold, &points, &context);
            context.frames[0].position_m.x += 0.002;
            let cached = restore_source_contact(&mut manifold, Some(&cached), &context, settings()).unwrap();
            for (saved, native) in cached.points.iter().zip(&points) {
                assert_eq!(saved.initial_first_m, native.first_m);
                assert_eq!(saved.initial_second_m, native.second_m);
            }
        }
    }

    #[test]
    fn source_mesh_restores_points_using_one_cross_group_threshold() {
        let entity = |index| Entity::from_raw_u32(index).unwrap();
        let pose = SourceRestorePose { position_m: DVec3::ZERO, rotation: DQuat::IDENTITY };
        let context = SourceContactContext { colliders: [entity(1), entity(2)], bodies: [entity(3), entity(4)],
            shape_ticks: [Tick::new(1), Tick::new(2)], frames: [pose; 2], tolerance: 0.01, skin: 0.0,
            box_first: true, surface_normal: Vec3::Y, centers: [Vec3::ZERO; 2],
            linear: [Vec3::ZERO; 2], angular: [Vec3::ZERO; 2] };
        let point = ContactPoint::new(Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 0.0);
        let first = ContactManifold::new([point; 4], Vec3::Y);
        let second = ContactManifold::new([point; 4], Vec3::X);
        let mut second_context = context;
        second_context.surface_normal = Vec3::X;
        let mut groups = [capture_source_contact(&first, &context), capture_source_contact(&second, &second_context)];
        groups[0].points[0].initial_first_m = DVec3::X * 0.02;
        assert!(restore_source_contact(&mut first.clone(), Some(&groups[0]), &context, settings()).is_none());
        assert!(source_restore_contact_groups(&mut groups, &context, settings()));
        assert_eq!(groups.iter().flat_map(|group| &group.points).filter(|point| !point.disabled).count(), 7);
        for point in &mut groups[1].points { point.disabled = true; }
        assert!(!source_restore_contact_groups(&mut groups, &context, settings()));
        let mut replaced = context;
        replaced.shape_ticks[0] = Tick::new(3);
        assert!(!source_restore_contact_groups(&mut groups, &replaced, settings()));
    }

    #[test]
    fn source_mesh_group_writeback_keeps_initial_point_indices() {
        let entity = |index| Entity::from_raw_u32(index).unwrap();
        let pose = SourceRestorePose { position_m: DVec3::ZERO, rotation: DQuat::IDENTITY };
        let context = SourceContactContext { colliders: [entity(1), entity(2)], bodies: [entity(3), entity(4)],
            shape_ticks: [Tick::new(1), Tick::new(2)], frames: [pose; 2], tolerance: 0.01, skin: 0.0,
            box_first: true, surface_normal: Vec3::Y, centers: [Vec3::ZERO; 2],
            linear: [Vec3::ZERO; 2], angular: [Vec3::ZERO; 2] };
        let point = ContactPoint::new(Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 0.0);
        let manifolds = [ContactManifold::new([point; 4], Vec3::Y), ContactManifold::new([point; 4], Vec3::X)];
        let mut saved = capture_source_mesh_contacts(&manifolds, &context);
        saved.contacts[0].points[0].initial_first_m = DVec3::X * 0.02;
        for _ in 0..2 {
            let restored = restore_source_mesh_contacts(&mut saved, &context, settings()).unwrap();
            assert_eq!(restored.iter().map(|manifold| manifold.points.len()).collect::<Vec<_>>(), vec![3, 4]);
            assert_eq!(restored[0].normal, Vec3::Y);
            assert_eq!(restored[1].normal, Vec3::X);
            assert_eq!(saved.contacts[0].templates.len(), 4);
        }
        for point in &mut saved.contacts[0].points { point.disabled = true; }
        let restored = restore_source_mesh_contacts(&mut saved, &context, settings()).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].normal, Vec3::X);
        saved.contacts[1].points[0].disabled = true;
        assert!(restore_source_mesh_contacts(&mut saved, &context, settings()).is_none());
        assert_eq!(saved.contacts[1].points.iter().filter(|point| !point.disabled).count(), 3);
    }

    #[test]
    fn source_mesh_slots_follow_actual_solver_order() {
        let handles = [
            ContactConstraintHandle { color_index: 21, local_index: 0 },
            ContactConstraintHandle { color_index: COLOR_OVERFLOW_INDEX as u8, local_index: 2 },
            ContactConstraintHandle { color_index: 3, local_index: 4 },
            ContactConstraintHandle { color_index: 3, local_index: 1 },
        ];
        assert_eq!(source_solver_slots(&handles), vec![1, 3, 2, 0]);
    }

    #[test]
    fn source_mesh_restore_rebuilds_missing_solver_slots() {
        use avian3d::collision::narrow_phase::NarrowPhaseSystems;
        use avian3d::prelude::{Gravity, LockedAxes, PhysicsPlugins, PhysicsSchedule, SleepingDisabled, TimeToSleep};
        use bevy::time::TimeUpdateStrategy;
        use bevy_animation_graph::core::ragdoll::definition::BodyMode;

        #[derive(Resource, Default)]
        struct TrimOnce(bool);

        fn trim_one(mut trim: ResMut<TrimOnce>, mut graph: ResMut<ContactGraph>, mut constraints: ResMut<ConstraintGraph>) {
            if !trim.0 { return; }
            let target = graph.iter_active_touching().find_map(|pair| {
                let (edge, _) = graph.get_by_id(pair.contact_id)?;
                (edge.constraint_handles.len() > 1).then_some((pair.contact_id, pair.body1?, pair.body2?))
            });
            if let Some((id, body1, body2)) = target {
                constraints.pop_manifold(&mut graph.edges, id, body1, body2);
                let (_, pair) = graph.get_mut_by_id(id).unwrap();
                pair.manifolds.pop();
                trim.0 = false;
            }
        }

        for source_body in [false, true] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::new(PostUpdate)))
                .insert_resource(Gravity(Vec3::ZERO))
                .insert_resource(settings())
                .init_resource::<SourceBoxContactHistory>()
                .init_resource::<TrimOnce>()
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
                .add_systems(PhysicsSchedule, trim_one.in_set(NarrowPhaseSystems::Last).before(restore_source_mesh_manifolds))
                .add_systems(PhysicsSchedule, restore_source_mesh_manifolds.in_set(NarrowPhaseSystems::Last));
            app.finish();
            app.cleanup();
            let mesh = Collider::trimesh(vec![Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, -1.0),
                Vec3::new(1.0, 0.0, 1.0), Vec3::new(-1.0, 0.0, 1.0)], vec![[0, 2, 1], [0, 3, 2]]);
            let ground = app.world_mut().spawn((RigidBody::Static, mesh, SourceMeshContact("SM_Als_Cart".into()), Transform::IDENTITY)).id();
            let foot = app.world_mut().spawn((RigidBody::Dynamic, LockedAxes::ALL_LOCKED, SleepingDisabled,
                Collider::cuboid(0.2, 0.2, 0.2), Transform::from_xyz(0.0, 0.05, 0.0))).id();
            if source_body { app.world_mut().entity_mut(foot).insert(RagdollActiveMode(BodyMode::Dynamic)); }
            app.update();
            app.update();
            let (edge, pair) = app.world().resource::<ContactGraph>().get(ground, foot).unwrap();
            let original = pair.manifolds.len();
            assert!(original > 1);
            assert_eq!(edge.constraint_handles.len(), original);
            app.world_mut().resource_mut::<TrimOnce>().0 = true;
            app.world_mut().get_mut::<Position>(foot).unwrap().0.x += 0.002;
            app.world_mut().get_mut::<Transform>(foot).unwrap().translation.x += 0.002;
            app.update();
            assert!(!app.world().resource::<TrimOnce>().0);
            let graph = app.world().resource::<ContactGraph>();
            let (edge, pair) = graph.get(ground, foot).unwrap();
            assert_eq!(pair.manifolds.len(), if source_body { original } else { original - 1 });
            assert_eq!(edge.constraint_handles.len(), pair.manifolds.len());
            let constraints = app.world().resource::<ConstraintGraph>();
            for (index, handle) in edge.constraint_handles.iter().enumerate() {
                let stored = &constraints.colors[handle.color_index as usize].manifold_handles[handle.local_index];
                assert_eq!(stored.contact_id, pair.contact_id);
                assert_eq!(stored.manifold_index, index);
            }
            assert_eq!(app.world().resource::<SourceBoxContactHistory>().meshes.len(), usize::from(source_body));
            if source_body {
                let old_tick = app.world().resource::<SourceBoxContactHistory>().meshes[&pair.contact_id.0].source_tick;
                app.world_mut().entity_mut(ground).insert(SourceMeshContact("SM_Als_Cart".into()));
                app.update();
                let graph = app.world().resource::<ContactGraph>();
                let (_, pair) = graph.get(ground, foot).unwrap();
                let id = pair.contact_id;
                let remaining = pair.manifolds[0].clone();
                let new_tick = app.world().resource::<SourceBoxContactHistory>().meshes[&id.0].source_tick;
                assert_ne!(old_tick, new_tick);
                let initial = app.world().resource::<SourceBoxContactHistory>().meshes[&id.0]
                    .contacts.contacts[0].points[0].initial_first_m;
                app.world_mut().entity_mut(foot).remove::<SleepingDisabled>();
                app.world_mut().insert_resource(TimeToSleep(0.05));
                let slept = (0..30).any(|_| {
                    app.update();
                    app.world().resource::<ContactGraph>().get(ground, foot).is_some_and(|(edge, _)| edge.is_sleeping())
                });
                assert!(slept, "The source mesh contact did not sleep within 30 physics frames");
                app.update();
                assert_eq!(app.world().resource::<SourceBoxContactHistory>().meshes[&id.0]
                    .contacts.contacts[0].points[0].initial_first_m, initial);
                app.world_mut().get_mut::<Position>(foot).unwrap().0.x += 0.001;
                app.world_mut().get_mut::<Transform>(foot).unwrap().translation.x += 0.001;
                let woke = (0..5).any(|_| {
                    app.update();
                    app.world().resource::<ContactGraph>().get(ground, foot).is_some_and(|(edge, _)| !edge.is_sleeping())
                });
                assert!(woke, "The source mesh contact did not wake within five physics frames");
                assert_eq!(app.world().resource::<SourceBoxContactHistory>().meshes[&id.0]
                    .contacts.contacts[0].points[0].initial_first_m, initial);
                app.world_mut().resource_scope(|world, mut graph: Mut<ContactGraph>| {
                    let mut constraints = world.resource_mut::<ConstraintGraph>();
                    write_source_solver_groups(&mut graph, &mut constraints, id, vec![remaining]);
                });
                let (edge, pair) = app.world().resource::<ContactGraph>().get(ground, foot).unwrap();
                assert_eq!(edge.constraint_handles.len(), 1);
                assert_eq!(pair.manifolds.len(), 1);
            }
        }
    }

    #[test]
    fn source_box_contact_restores_actual_surface_across_small_motion() {
        use avian3d::collision::narrow_phase::NarrowPhaseSystems;
        use avian3d::prelude::{Gravity, LockedAxes, PhysicsPlugins, PhysicsSchedule, SleepingDisabled};
        use bevy::time::TimeUpdateStrategy;
        use bevy_animation_graph::core::ragdoll::definition::BodyMode;
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d104a/restore.native-samples.json")).unwrap();
        let sample = fixture["samples"].as_array().unwrap().iter().find(|value| {
            value["input"]["label"] == "foot_l_tilt_45_width_5.0_shift_0.2"
        }).unwrap();
        let input = &sample["input"];
        let geometry = &sample["geometry"];
        let source_frame = pose(input, "body_translation_cm", "body_rotation_xyzw", Some(&geometry["shape_to_body"]));
        let minimum = object_vector(&geometry["minimum_cm"]) * 0.01;
        let maximum = object_vector(&geometry["maximum_cm"]) * 0.01;
        let center = source_frame.position_m + source_frame.rotation * ((minimum + maximum) * 0.5);
        let size = (maximum - minimum).as_vec3();
        let floor = pose(input, "box_translation_cm", "box_rotation_xyzw", None);
        let floor_size = (array_vector(&input["box_half_extents_cm"]) * 0.02).as_vec3();
        let surfaces = |world: &World, body: Entity, floor: Entity| {
            let (_, pair) = world.resource::<ContactGraph>().get(body, floor).unwrap();
            let manifold = &pair.manifolds[0];
            let direction = manifold.normal * if pair.collider1 == body { 1.0 } else { -1.0 };
            manifold.points.iter().map(|point| point.point + direction * point.penetration * 0.5).collect::<Vec<_>>()
        };
        for (source_body, delta) in [(true, 0.002_f32), (true, -0.002), (true, 0.006), (false, 0.002)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::new(PostUpdate)))
                .insert_resource(Gravity(Vec3::ZERO))
                .insert_resource(settings())
                .init_resource::<SourceBoxContactHistory>()
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
                .add_systems(PhysicsSchedule, restore_source_box_contacts.in_set(NarrowPhaseSystems::Last));
            app.finish();
            app.cleanup();
            let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(floor_size.x, floor_size.y, floor_size.z),
                Transform::from_translation(floor.position_m.as_vec3()).with_rotation(floor.rotation.as_quat()))).id();
            let body = app.world_mut().spawn((RigidBody::Dynamic, LockedAxes::ALL_LOCKED, SleepingDisabled,
                Collider::cuboid(size.x, size.y, size.z),
                Transform::from_translation(center.as_vec3()).with_rotation(source_frame.rotation.as_quat()))).id();
            if source_body { app.world_mut().entity_mut(body).insert(RagdollActiveMode(BodyMode::Dynamic)); }
            app.update();
            app.update();
            let before = surfaces(app.world(), body, floor);
            assert!(!before.is_empty());
            app.world_mut().get_mut::<Position>(body).unwrap().0.x += delta;
            app.world_mut().get_mut::<Transform>(body).unwrap().translation.x += delta;
            app.update();
            let after = surfaces(app.world(), body, floor);
            let retained = before.len() == after.len() && before.iter().zip(&after)
                .all(|(before, after)| (*after - *before - Vec3::X * delta).length() < 2e-5);
            assert_eq!(retained, source_body && delta.abs() < 0.005, "source={source_body} delta={delta}");
            if source_body && delta == 0.006 {
                app.world_mut().get_mut::<Position>(body).unwrap().0.x += 0.002;
                app.world_mut().get_mut::<Transform>(body).unwrap().translation.x += 0.002;
                app.update();
                let recaptured = surfaces(app.world(), body, floor);
                assert_eq!(after.len(), recaptured.len());
                assert!(after.iter().zip(&recaptured)
                    .all(|(before, after)| (*after - *before - Vec3::X * 0.002).length() < 2e-5));
            }
        }
    }
}
