use avian3d::collision::collider::{AabbContext, AnyCollider, ColliderAabb, ContactManifoldContext, SimpleCollider};
use avian3d::collision::contact_types::ContactManifold;
use avian3d::collision::narrow_phase::{NarrowPhasePlugin, NarrowPhaseSystems};
use avian3d::dynamics::rigid_body::mass_properties::bevy_heavy::{ComputeMassProperties3d, MassProperties3d};
use avian3d::prelude::{Collider, PhysicsPlugins, PhysicsSchedule, Rotation};
use bevy::prelude::*;

use super::ragdoll_physics_profile::RagdollCollisionHooks;

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
    type Context = ();

    fn aabb_with_context(&self, position: Vec3, rotation: impl Into<Rotation>, _: AabbContext<Self::Context>) -> ColliderAabb {
        self.0.aabb(position, rotation)
    }

    fn contact_manifolds_with_context(
        &self, other: &Self, position1: Vec3, rotation1: impl Into<Rotation>,
        position2: Vec3, rotation2: impl Into<Rotation>, prediction_distance: f32,
        manifolds: &mut Vec<ContactManifold>, _: ContactManifoldContext<Self::Context>,
    ) {
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
    use bevy::time::TimeUpdateStrategy;

    #[test]
    fn source_narrow_collider_delegates_target_geometry_and_mass() {
        let first = Collider::cuboid(0.5, 0.8, 0.3);
        let second = Collider::capsule(0.15, 0.6);
        let wrapped_first = SourceNarrowCollider(first.clone());
        let wrapped_second = SourceNarrowCollider(second.clone());
        assert_eq!(wrapped_first.mass_properties(1.7), first.mass_properties(1.7));
        for x in [0.0, 0.36, 1.0] {
            let mut original = Vec::new();
            let mut wrapped = Vec::new();
            first.contact_manifolds(&second, Vec3::ZERO, Quat::IDENTITY, Vec3::X * x,
                Quat::from_rotation_z(0.3), 0.05, &mut original);
            wrapped_first.contact_manifolds(&wrapped_second, Vec3::ZERO, Quat::IDENTITY, Vec3::X * x,
                Quat::from_rotation_z(0.3), 0.05, &mut wrapped);
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
