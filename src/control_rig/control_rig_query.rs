use std::sync::Arc;

use avian3d::prelude::*;
use crate::extensions::RagdollSpatialQuery as SpatialQuery;
use bevy::{math::DVec3, platform::collections::HashMap, prelude::*};
use parry3d_f64::{query::{Ray, RayCast}, shape::Cuboid};

use crate::extensions::control_rig_hierarchy::{RigTransform, source_rotate_vector};

pub struct RigQueryWorld {
    pub colliders: HashMap<Entity, (Position, Rotation, Collider, CollisionLayers)>,
}

#[derive(Clone)]
pub struct RigQueryContext {
    pub world: Arc<RigQueryWorld>,
    pub to_world: RigTransform,
    pub channel: String,
    pub filter: SpatialQueryFilter,
}

impl RigQueryContext {
    pub fn trace(
        &self,
        start: DVec3,
        end: DVec3,
        channel: &str,
        complex: bool,
    ) -> Result<Option<(DVec3, DVec3)>, String> {
        if channel != self.channel || !complex {
            return Err("rig collision query mode is unsupported".to_owned());
        }
        let delta = end - start;
        if !delta.is_finite() || delta == DVec3::ZERO {
            return Ok(None);
        }
        let delta_f32 = delta.as_vec3();
        let fallback = Dir3::new(delta_f32).ok().map(|direction| (direction, delta_f32.length()));
        let origin = start.as_vec3();
        let mut nearest = 1.0;
        let mut hit = None;
        for (entity, (position, rotation, collider, layers)) in &self.world.colliders {
            if !self.filter.test(*entity, *layers) {
                continue;
            }
            let candidate = if let Some(cuboid) = collider.shape_scaled().as_cuboid() {
                let rotation = rotation.0.as_dquat();
                let local_origin = source_rotate_vector(rotation.conjugate(), start - position.0.as_dvec3());
                let local_delta = source_rotate_vector(rotation.conjugate(), delta);
                Cuboid::new(cuboid.half_extents.as_dvec3())
                    .cast_local_ray_and_get_normal(&Ray::new(local_origin, local_delta), nearest, true)
                    .map(|impact| (impact.time_of_impact, start + delta * impact.time_of_impact,
                        source_rotate_vector(rotation, impact.normal)))
            } else if let Some((direction, distance)) = fallback {
                collider.cast_ray(position.0, *rotation, origin, direction.as_vec3(), distance, true)
                    .map(|(length, normal)| (f64::from(length) / f64::from(distance),
                        (origin + direction.as_vec3() * length).as_dvec3(), normal.as_dvec3()))
            } else {
                None
            };
            let Some((fraction, point, normal)) = candidate else {
                continue;
            };
            if fraction < nearest {
                nearest = fraction;
                hit = Some((point, normal));
            }
        }
        Ok(hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::{
        control_rig_hierarchy::RigHierarchy, control_rig_ik::RigGeometry,
        control_rig_math::RigValue, control_rig_units::RigUnitContext,
    };

    #[test]
    fn rig_query_preserves_double_precision_on_source_box_ray() {
        let mut entities = World::new();
        let floor = entities.spawn_empty().id();
        let query = RigQueryContext {
            world: Arc::new(RigQueryWorld { colliders: [(
                floor,
                (Position(Vec3::new(-10.0, -0.25, -20.0)), Rotation::default(),
                    Collider::cuboid(40.0, 0.5, 20.0), CollisionLayers::ALL),
            )].into_iter().collect() }),
            to_world: RigTransform::default(), channel: "Visibility".to_owned(), filter: default(),
        };
        let start = DVec3::new(-1515.661_365_680_93 * 0.01, 49.999_994_039_535_52 * 0.01,
            -1508.488_173_045_665_7 * 0.01);
        let end = start.with_y(-80.000_005_960_464_48 * 0.01);
        let hit = query.trace(start, end, "Visibility", true).unwrap().unwrap();
        assert_eq!(hit, (DVec3::new(start.x, 0.0, start.z), DVec3::Y));
        let inside = DVec3::new(-10.0, -0.25, -20.0);
        assert_eq!(query.trace(inside, inside + DVec3::Y, "Visibility", true).unwrap(),
            Some((inside, DVec3::ZERO)));
    }

    #[test]
    fn rig_query_selects_nearest_mixed_precision_collider() {
        let mut entities = World::new();
        let floor = entities.spawn_empty().id();
        let sphere = entities.spawn_empty().id();
        let mut query = RigQueryContext {
            world: Arc::new(RigQueryWorld { colliders: [
                (floor, (Position(Vec3::new(0.0, -0.25, 0.0)), Rotation::default(),
                    Collider::cuboid(10.0, 0.5, 10.0), CollisionLayers::ALL)),
                (sphere, (Position(Vec3::Y), Rotation::default(),
                    Collider::sphere(0.5), CollisionLayers::ALL)),
            ].into_iter().collect() }),
            to_world: RigTransform::default(), channel: "Visibility".to_owned(), filter: default(),
        };
        let start = DVec3::Y * 3.0;
        let end = DVec3::NEG_Y * 3.0;
        assert_eq!(query.trace(start, end, "Visibility", true).unwrap().unwrap().0.y, 1.5);
        query.filter.excluded_entities.insert(sphere);
        assert_eq!(query.trace(start, end, "Visibility", true).unwrap().unwrap().0.y, 0.0);
    }

    #[test]
    fn control_rig_query_uses_avian_geometry_filters_and_world_transforms() {
        fn capture(
            In(filter): In<SpatialQueryFilter>,
            colliders: Query<(Entity, &Position, &Rotation, &Collider, &CollisionLayers)>,
            spatial: SpatialQuery,
        ) -> (RigQueryWorld, Option<RayHitData>) {
            (
                RigQueryWorld {
                    colliders: colliders
                        .iter()
                        .map(|(entity, position, rotation, collider, layers)| {
                            (entity, (*position, *rotation, collider.clone(), *layers))
                        })
                        .collect(),
                },
                spatial.cast_ray(Vec3::new(0.0, 3.0, 0.0), Dir3::NEG_Y, 6.0, true, &filter),
            )
        }
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            TransformPlugin,
            PhysicsPlugins::new(PostUpdate),
        ))
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 60.0),
        ));
        app.finish();
        app.cleanup();
        let owner = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(1.0, 1.0, 1.0),
                Transform::from_xyz(0.0, 1.0, 0.0),
                CollisionLayers::new(crate::als::character::AlsCollisionLayer::Visibility, LayerMask::ALL),
            ))
            .id();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(1.0, 0.2, 1.0),
            Transform::from_xyz(0.0, 2.0, 0.0),
            CollisionLayers::new(crate::als::character::AlsCollisionLayer::Default, LayerMask::ALL),
        ));
        let floor = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(20.0, 0.2, 20.0),
                Transform::from_xyz(0.0, -0.1, 0.0),
                CollisionLayers::new(crate::als::character::AlsCollisionLayer::Visibility, LayerMask::ALL),
            ))
            .id();
        for _ in 0..3 {
            app.update();
        }
        let filter = SpatialQueryFilter::from_mask(crate::als::character::AlsCollisionLayer::Visibility).with_excluded_entities([owner]);
        let (snapshot, native) = app
            .world_mut()
            .run_system_cached_with(capture, filter.clone())
            .unwrap();
        let native = native.unwrap();
        let query = RigQueryContext {
            world: Arc::new(snapshot),
            filter,
            to_world: RigTransform::default(),
            channel: "Visibility".to_owned(),
        };
        let hit = query
            .trace(
                DVec3::new(0.0, 3.0, 0.0),
                DVec3::new(0.0, -3.0, 0.0),
                "Visibility",
                true,
            )
            .unwrap()
            .unwrap();
        assert!((hit.0.y - (3.0 - native.distance) as f64).abs() < 1e-7);
        assert_eq!(hit.1, native.normal.as_dvec3());
        assert!(hit.0.y.abs() < 1e-6);
        app.world_mut()
            .get_mut::<Transform>(floor)
            .unwrap()
            .translation
            .y = 0.9;
        app.update();
        let (_, moved) = app
            .world_mut()
            .run_system_cached_with(capture, query.filter.clone())
            .unwrap();
        assert!((moved.unwrap().distance - 2.0).abs() < 1e-6);
        assert_eq!(
            query
                .trace(
                    DVec3::new(0.0, 3.0, 0.0),
                    DVec3::new(0.0, -3.0, 0.0),
                    "Visibility",
                    true
                )
                .unwrap(),
            Some(hit)
        );
        let mut include_owner = query.clone();
        include_owner.filter.excluded_entities.clear();
        assert!(
            (include_owner
                .trace(
                    DVec3::new(0.0, 3.0, 0.0),
                    DVec3::new(0.0, -3.0, 0.0),
                    "Visibility",
                    true
                )
                .unwrap()
                .unwrap()
                .0
                .y
                - 1.5)
                .abs()
                < 1e-6
        );
        assert!(
            query
                .trace(
                    DVec3::X * 100.0,
                    DVec3::X * 100.0 - DVec3::Y,
                    "Visibility",
                    true
                )
                .unwrap()
                .is_none()
        );
        assert!(
            query
                .trace(DVec3::ZERO, DVec3::ZERO, "Visibility", true)
                .unwrap()
                .is_none()
        );
        assert!(query.trace(DVec3::ZERO, DVec3::Y, "Other", true).is_err());
        assert!(
            query
                .trace(DVec3::ZERO, DVec3::Y, "Visibility", false)
                .is_err()
        );
        let mut hierarchy = RigHierarchy::new(Vec::new(), 1e-6).unwrap();
        let geometry = RigGeometry {
            near_zero: 1e-6,
            normalize_squared: 1e-12,
            minimum_length: 1e-10,
            solve_near_zero: 1e-6,
            basis: [DVec3::X, DVec3::Z, DVec3::Y],
        };
        let curves = HashMap::default();
        let mut calls = 0;
        let mut trace = |start, end, channel: &str, complex| {
            calls += 1;
            query.trace(start, end, channel, complex)
        };
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &curves,
            game_world: true,
            construction: false,
            delta_seconds: 0.02,
            geometry: &geometry,
            to_world: RigTransform {
                translation: DVec3::new(2.0, 0.2, 0.0),
                scale: DVec3::new(1.0, 0.5, 1.0),
                ..default()
            },
            line_trace: Some(&mut trace),
        };
        use RigValue::*;
        let mut values = vec![
            Vector(DVec3::Y * 100.0),
            Name("Visibility".to_owned()),
            F32(0.5),
            F32(0.8),
            F32(45.0),
            F32(0.1),
            Bool(true),
            Bool(false),
            F32(0.0),
            Vector(DVec3::ZERO),
        ];
        context
            .call("FAlsRigUnit_FootOffsetTrace::Execute", &mut values)
            .unwrap();
        assert_eq!(calls, 1);
        let F32(offset) = values[8] else {
            panic!("invalid offset")
        };
        assert!((offset + 0.45).abs() < 1e-6);
        assert_eq!(values[9], Vector(DVec3::Y * 2.0));
    }
}
