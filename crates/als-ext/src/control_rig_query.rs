use std::sync::Arc;

use avian3d::{collider_tree::ColliderTrees, prelude::*};
use bevy::{math::DVec3, platform::collections::HashMap, prelude::*};

use crate::control_rig_hierarchy::RigTransform;

pub struct RigQueryWorld {
    pub trees: ColliderTrees,
    pub colliders: HashMap<Entity, (Position, Rotation, Collider)>,
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
        let delta = (end - start).as_vec3();
        let Ok(direction) = Dir3::new(delta) else {
            return Ok(None);
        };
        let origin = start.as_vec3();
        let mut distance = delta.length();
        let mut hit = None;
        for tree in self.world.trees.iter_trees() {
            tree.ray_traverse_closest(Ray3d::new(origin, direction), distance, |id| {
                let Some(proxy) = tree.get_proxy(id) else {
                    return f32::MAX;
                };
                if !self.filter.test(proxy.collider, proxy.layers) {
                    return f32::MAX;
                }
                let Some((position, rotation, collider)) =
                    self.world.colliders.get(&proxy.collider)
                else {
                    return f32::MAX;
                };
                let Some((length, normal)) = collider.cast_ray(
                    position.0,
                    *rotation,
                    origin,
                    direction.as_vec3(),
                    distance,
                    true,
                ) else {
                    return f32::MAX;
                };
                if length < distance {
                    distance = length;
                    hit = Some((
                        (origin + direction.as_vec3() * length).as_dvec3(),
                        normal.as_dvec3(),
                    ));
                }
                length
            });
        }
        Ok(hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        control_rig_hierarchy::RigHierarchy, control_rig_ik::RigGeometry,
        control_rig_math::RigValue, control_rig_units::RigUnitContext,
    };

    #[test]
    fn control_rig_query_uses_avian_geometry_filters_and_world_transforms() {
        fn capture(
            In(filter): In<SpatialQueryFilter>,
            trees: Res<ColliderTrees>,
            colliders: Query<(Entity, &Position, &Rotation, &Collider)>,
            spatial: SpatialQuery,
        ) -> (RigQueryWorld, Option<RayHitData>) {
            (
                RigQueryWorld {
                    trees: trees.clone(),
                    colliders: colliders
                        .iter()
                        .map(|(entity, position, rotation, collider)| {
                            (entity, (*position, *rotation, collider.clone()))
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
                CollisionLayers::from_bits(1, u32::MAX),
            ))
            .id();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(1.0, 0.2, 1.0),
            Transform::from_xyz(0.0, 2.0, 0.0),
            CollisionLayers::from_bits(2, u32::MAX),
        ));
        let floor = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(20.0, 0.2, 20.0),
                Transform::from_xyz(0.0, -0.1, 0.0),
                CollisionLayers::from_bits(1, u32::MAX),
            ))
            .id();
        for _ in 0..3 {
            app.update();
        }
        let filter = SpatialQueryFilter::from_mask(1u32).with_excluded_entities([owner]);
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
