use avian3d::collision::collider::{AabbContext, AnyCollider, ColliderAabb, ContactManifoldContext, SimpleCollider};
use avian3d::collision::contact_types::{ContactManifold, ContactPoint};
use avian3d::collision::narrow_phase::{NarrowPhaseConfig, NarrowPhasePlugin, NarrowPhaseSystems};
use avian3d::dynamics::rigid_body::mass_properties::bevy_heavy::{ComputeMassProperties3d, MassProperties3d};
use avian3d::dynamics::solver::PhysicsLengthUnit;
use avian3d::prelude::{ActiveCollisionHooks, AngularVelocity, Collider, ColliderOf, ColliderTransform, CollisionMargin,
    ComputedCenterOfMass, LinearVelocity, PhysicsPlugins, PhysicsSchedule, Position, RigidBody, RigidBodyDisabled,
    Rotation, Sensor, SpeculativeMargin};
use bevy::ecs::{component::Tick, system::SystemParam};
use bevy::prelude::*;

use super::ragdoll_physics_profile::{RagdollActiveMode, RagdollCollisionHooks};
use super::source_contact_capsule::{SourceCapsuleGeometry, source_capsule_body_geometry};
use super::source_contact_convex::{SourceConvexLifecycleSettings, source_body_local_max_extent, source_convex_cull_distance};
use super::source_contact_face::{SourceConvexTopology, source_convex_hulls};
use super::source_contact_margin::SourceConvexMarginSettings;
use super::source_contact_order::{SourceCompoundOrder, SourceCompoundSettings};
use super::source_contact_restore::{SourceBoxContactHistory, SourceContactContext, SourceRestorePose, SourceRestoreSettings,
    source_box_shape_pose};

#[derive(SystemParam)]
pub(crate) struct SourceCompoundInputs<'w, 's> {
    time: Option<Res<'w, Time>>,
    config: Option<Res<'w, NarrowPhaseConfig>>,
    length: Option<Res<'w, PhysicsLengthUnit>>,
    margins: Option<Res<'w, SourceConvexMarginSettings>>,
    lifecycle: Option<Res<'w, SourceConvexLifecycleSettings>>,
    restore: Option<Res<'w, SourceRestoreSettings>>,
    pub(super) history: Option<Res<'w, SourceBoxContactHistory>>,
    sources: Query<'w, 's, Ref<'static, SourceCompoundOrder>>,
    colliders: Query<'w, 's, (Ref<'static, Collider>, &'static Position, &'static Rotation,
        Option<&'static CollisionMargin>, Option<&'static SpeculativeMargin>, Option<&'static ColliderTransform>,
        Option<&'static ColliderOf>, Has<Sensor>, Option<&'static ActiveCollisionHooks>)>,
    ownership: Query<'w, 's, (Entity, &'static Collider, Option<&'static ColliderOf>, Option<&'static ColliderTransform>)>,
    bodies: Query<'w, 's, (&'static RigidBody, &'static Position, &'static Rotation, &'static ComputedCenterOfMass,
        &'static LinearVelocity, &'static AngularVelocity, Option<&'static CollisionMargin>,
        Option<&'static SpeculativeMargin>, Has<RagdollActiveMode>), Without<RigidBodyDisabled>>,
}

pub(super) struct SourceCompoundInput {
    pub source_tick: Tick,
    pub shape: SourceCompoundShape,
    pub compound: Collider,
    pub hulls: &'static [SourceConvexTopology],
    pub context: SourceContactContext,
    pub settings: SourceCompoundSettings,
}

pub(super) enum SourceCompoundShape {
    Box { collider: Collider, position: Position },
    Capsule(SourceCapsuleGeometry),
}

impl SourceCompoundInputs<'_, '_> {
    pub(super) fn input(&self, first_entity: Entity, second_entity: Entity) -> Option<SourceCompoundInput> {
        let (margins, lifecycle, restore) = (*self.margins.as_deref()?, *self.lifecycle.as_deref()?, *self.restore.as_deref()?);
        let (time, config, length) = (self.time.as_deref()?, self.config.as_deref()?, self.length.as_deref()?);
        self.history.as_deref()?;
        let first = self.colliders.get(first_entity).ok()?;
        let second = self.colliders.get(second_entity).ok()?;
        if first.7 || second.7 || ![first.8, second.8].into_iter().flatten()
            .any(|hooks| hooks.contains(ActiveCollisionHooks::MODIFY_CONTACTS)) { return None; }
        let first_body = first.6.map_or(first_entity, |owner| owner.body);
        let second_body = second.6.map_or(second_entity, |owner| owner.body);
        let first_data = self.bodies.get(first_body).ok()?;
        let second_data = self.bodies.get(second_body).ok()?;
        let (source_side, source) = if let Ok(source) = self.sources.get(first_entity) { (0, source) }
            else if let Ok(source) = self.sources.get(second_entity) { (1, source) } else { return None; };
        let hulls = source_convex_hulls(&source.0)?;
        let delta = time.delta_secs();
        let state = |velocity: &LinearVelocity, collider_margin: Option<&CollisionMargin>, body_margin: Option<&CollisionMargin>,
            collider_speculative: Option<&SpeculativeMargin>, body_speculative: Option<&SpeculativeMargin>| {
            let mut velocity = velocity.0;
            let margin = collider_margin.or(body_margin).map_or(0.0, |value| value.0);
            let speculative = collider_speculative.or(body_speculative)
                .map_or(config.default_speculative_margin * length.0, |value| value.0);
            if speculative < f32::MAX { velocity = velocity.clamp_length_max(speculative * delta.recip()); }
            (velocity, margin)
        };
        let (velocity1, margin1) = state(first_data.4, first.3, first_data.6, first.4, first_data.7);
        let (velocity2, margin2) = state(second_data.4, second.3, second_data.6, second.4, second_data.7);
        let (box_entity, box_body, box_shape, box_data, compound_entity, compound_body, compound_shape, compound_data) =
            if source_side == 0 { (second_entity, second_body, second, second_data, first_entity, first_body, first, first_data) }
            else { (first_entity, first_body, first, first_data, second_entity, second_body, second, second_data) };
        if !box_data.0.is_dynamic() || !box_data.8 || !compound_data.0.is_static() { return None; }
        let capsule = source_capsule_body_geometry(&box_shape.0, box_shape.5);
        if capsule.is_none() && box_shape.0.shape_scaled().as_cuboid().is_none() { return None; }
        let frame = if capsule.is_some() {
            SourceRestorePose { position_m: box_data.1.0.as_dvec3(), rotation: box_data.2.0.as_dquat() }
        } else { source_box_shape_pose(box_shape.1, box_shape.2, box_shape.5, box_data.1, box_data.2) };
        let frames = [frame, SourceRestorePose { position_m: compound_shape.1.0.as_dvec3(),
            rotation: compound_shape.2.0.as_dquat() }];
        let bounds = box_shape.0.shape_scaled().compute_local_aabb();
        let box_size_cm = (f64::from((bounds.maxs - bounds.mins).abs().max_element()) * 100.0) as f32;
        let centers = if source_side == 0 {
            [compound_data.1.0 + compound_data.2.0 * compound_data.3.0, box_data.1.0 + box_data.2.0 * box_data.3.0]
        } else {
            [box_data.1.0 + box_data.2.0 * box_data.3.0, compound_data.1.0 + compound_data.2.0 * compound_data.3.0]
        };
        let angular = if source_side == 0 { [compound_data.5.0, box_data.5.0] }
            else { [box_data.5.0, compound_data.5.0] };
        let context = SourceContactContext { colliders: [box_entity, compound_entity], bodies: [box_body, compound_body],
            shape_ticks: [box_shape.0.last_changed(), compound_shape.0.last_changed()], frames, tolerance: 0.0,
            skin: margin1 + margin2, box_first: source_side == 1, surface_normal: Vec3::ZERO,
            centers, linear: [velocity1, velocity2], angular };
        let extent = source_body_local_max_extent(box_body, self.ownership.iter())?;
        let cull = source_convex_cull_distance(extent, [box_data.4.0, Vec3::ZERO], delta, lifecycle);
        let shape = if let Some(capsule) = capsule { SourceCompoundShape::Capsule(capsule) }
            else { SourceCompoundShape::Box { collider: (*box_shape.0).clone(), position: *box_shape.1 } };
        Some(SourceCompoundInput { source_tick: source.last_changed(), shape,
            compound: (*compound_shape.0).clone(), hulls, context,
            settings: SourceCompoundSettings { restore, lifecycle, margins, cull, box_size_cm } })
    }
}

#[derive(Component, Clone, Default)]
pub(crate) struct SourceNarrowCollider(Collider);

impl ComputeMassProperties3d for SourceNarrowCollider {
    fn mass(&self, density: f32) -> f32 { self.0.mass(density) }
    fn unit_principal_angular_inertia(&self) -> Vec3 { self.0.unit_principal_angular_inertia() }
    fn principal_angular_inertia(&self, mass: f32) -> Vec3 { self.0.principal_angular_inertia(mass) }
    fn local_inertial_frame(&self) -> Quat { self.0.local_inertial_frame() }
    fn center_of_mass(&self) -> Vec3 { self.0.center_of_mass() }
    fn mass_properties(&self, density: f32) -> MassProperties3d { self.0.mass_properties(density) }
}

impl AnyCollider for SourceNarrowCollider {
    type Context = SourceCompoundInputs<'static, 'static>;

    fn aabb_with_context(&self, position: Vec3, rotation: impl Into<Rotation>, _: AabbContext<Self::Context>) -> ColliderAabb {
        self.0.aabb(position, rotation)
    }

    fn contact_manifolds_with_context(
        &self, other: &Self, position1: Vec3, rotation1: impl Into<Rotation>,
        position2: Vec3, rotation2: impl Into<Rotation>, prediction_distance: f32,
        manifolds: &mut Vec<ContactManifold>, context: ContactManifoldContext<Self::Context>,
    ) {
        if context.input(context.entity1, context.entity2).is_some() {
            let midpoint = (position1 + position2) * 0.5;
            let point = ContactPoint::new(midpoint - position1, midpoint - position2, midpoint, f32::MIN_POSITIVE);
            manifolds.clear();
            manifolds.push(ContactManifold::new([point], Vec3::Y));
            return;
        }
        self.0.contact_manifolds(&other.0, position1, rotation1, position2, rotation2,
            prediction_distance, manifolds);
    }
}

fn sync_source_narrow_colliders(mut colliders: Query<(&Collider, &mut SourceNarrowCollider), Changed<Collider>>) {
    for (collider, mut narrow) in &mut colliders { narrow.0 = collider.clone(); }
}

pub(crate) fn add_source_physics(app: &mut App) {
    app.register_required_components::<Collider, SourceNarrowCollider>();
    app.add_plugins(PhysicsPlugins::new(PostUpdate).with_collision_hooks::<RagdollCollisionHooks>()
        .build().disable::<NarrowPhasePlugin<Collider, RagdollCollisionHooks>>()
        .add(NarrowPhasePlugin::<SourceNarrowCollider, RagdollCollisionHooks>::default()));
    app.add_systems(PhysicsSchedule, sync_source_narrow_colliders.in_set(NarrowPhaseSystems::First));
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::collision::contact_types::ContactGraph;
    use avian3d::prelude::{Gravity, LockedAxes, RigidBody, SleepingDisabled};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::time::TimeUpdateStrategy;

    #[test]
    fn source_narrow_collider_delegates_target_geometry_and_mass() {
        let first = Collider::cuboid(0.5, 0.8, 0.3);
        let second = Collider::capsule(0.15, 0.6);
        let wrapped_first = SourceNarrowCollider(first.clone());
        let wrapped_second = SourceNarrowCollider(second.clone());
        assert_eq!(wrapped_first.mass_properties(1.7), first.mass_properties(1.7));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).init_resource::<NarrowPhaseConfig>().init_resource::<PhysicsLengthUnit>();
        for x in [0.0, 0.36, 1.0] {
            let mut original = Vec::new();
            first.contact_manifolds(&second, Vec3::ZERO, Quat::IDENTITY, Vec3::X * x,
                Quat::from_rotation_z(0.3), 0.05, &mut original);
            let wrapped_first = wrapped_first.clone();
            let wrapped_second = wrapped_second.clone();
            let wrapped = app.world_mut().run_system_once(move |inputs: SourceCompoundInputs| {
                let mut result = Vec::new();
                let context = ContactManifoldContext::<SourceCompoundInputs>::new(Entity::PLACEHOLDER, Entity::PLACEHOLDER, &inputs);
                wrapped_first.contact_manifolds_with_context(&wrapped_second, Vec3::ZERO, Quat::IDENTITY,
                    Vec3::X * x, Quat::from_rotation_z(0.3), 0.05, &mut result, context);
                result
            }).unwrap();
            assert_eq!(wrapped, original);
        }
    }

    #[test]
    fn source_narrow_phase_tracks_collider_replacement_and_removal() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin));
        add_source_physics(&mut app);
        app.insert_resource(Gravity(Vec3::ZERO))
            .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        let ground = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(1.0, 1.0, 1.0), Transform::IDENTITY)).id();
        let body = app.world_mut().spawn((RigidBody::Dynamic, LockedAxes::ALL_LOCKED, SleepingDisabled,
            Collider::sphere(0.1), Transform::from_xyz(0.75, 0.0, 0.0))).id();
        app.update();
        app.update();
        assert!(!app.world().resource::<ContactGraph>().get(ground, body).is_some_and(|(_, pair)| pair.is_touching()));
        app.world_mut().entity_mut(body).insert(Collider::sphere(0.4));
        app.update();
        assert!(app.world().resource::<ContactGraph>().get(ground, body).is_some_and(|(_, pair)| pair.is_touching()));
        app.world_mut().entity_mut(body).remove::<Collider>();
        app.update();
        assert!(app.world().resource::<ContactGraph>().get(ground, body).is_none());
    }
}
