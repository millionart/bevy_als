use avian3d::collider_tree::ColliderTrees;
use avian3d::parry::shape::Capsule;
use avian3d::prelude::{
    Collider, Position, RayHitData, Rotation, ShapeCastConfig, ShapeHitData, SimpleCollider,
    SpatialQuery, SpatialQueryFilter,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

#[derive(Component, Clone)]
pub struct ComplexCollider(pub Collider);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeCollisionMode {
    NoCollision,
    QueryOnly,
    PhysicsOnly,
    QueryAndPhysics,
    ProbeOnly,
    QueryAndProbe,
}

impl ShapeCollisionMode {
    pub(super) fn from_source(value: &str) -> Result<Self, String> {
        match value {
            "NoCollision" => Ok(Self::NoCollision),
            "QueryOnly" => Ok(Self::QueryOnly),
            "PhysicsOnly" => Ok(Self::PhysicsOnly),
            "QueryAndPhysics" => Ok(Self::QueryAndPhysics),
            "ProbeOnly" => Ok(Self::ProbeOnly),
            "QueryAndProbe" => Ok(Self::QueryAndProbe),
            _ => Err(format!("unsupported shape collision mode: {value}")),
        }
    }

    pub fn has_query(self) -> bool {
        matches!(
            self,
            Self::QueryOnly | Self::QueryAndPhysics | Self::QueryAndProbe
        )
    }
}

#[derive(SystemParam)]
pub struct SceneQuery<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    modes: Query<'w, 's, &'static ShapeCollisionMode>,
    colliders: Query<'w, 's, (&'static Position, &'static Rotation, &'static Collider)>,
    trees: Res<'w, ColliderTrees>,
}

impl SceneQuery<'_, '_> {
    pub fn collider_geometry(&self, entity: Entity) -> Option<(&Position, &Rotation, &Collider)> {
        self.colliders.get(entity).ok()
    }

    fn queryable(&self, entity: Entity) -> bool {
        self.modes.get(entity).map_or(true, |mode| mode.has_query())
    }

    pub fn cast_ray(
        &self,
        origin: Vec3,
        direction: Dir3,
        max_distance: f32,
        solid: bool,
        filter: &SpatialQueryFilter,
    ) -> Option<RayHitData> {
        self.cast_ray_predicate(origin, direction, max_distance, solid, filter, &|_| true)
    }

    pub fn cast_ray_predicate(
        &self,
        origin: Vec3,
        direction: Dir3,
        max_distance: f32,
        solid: bool,
        filter: &SpatialQueryFilter,
        predicate: &dyn Fn(Entity) -> bool,
    ) -> Option<RayHitData> {
        self.spatial
            .cast_ray_predicate(origin, direction, max_distance, solid, filter, &|entity| {
                self.queryable(entity) && predicate(entity)
            })
    }

    pub fn cast_shape(
        &self,
        shape: &Collider,
        origin: Vec3,
        rotation: Quat,
        direction: Dir3,
        config: &ShapeCastConfig,
        filter: &SpatialQueryFilter,
    ) -> Option<ShapeHitData> {
        self.cast_shape_predicate(shape, origin, rotation, direction, config, filter, &|_| {
            true
        })
    }

    pub fn cast_shape_predicate(
        &self,
        shape: &Collider,
        origin: Vec3,
        rotation: Quat,
        direction: Dir3,
        config: &ShapeCastConfig,
        filter: &SpatialQueryFilter,
        predicate: &dyn Fn(Entity) -> bool,
    ) -> Option<ShapeHitData> {
        let scaled = shape.shape_scaled();
        let Some(capsule) = scaled
            .as_capsule()
            .filter(|capsule| capsule.segment.a != capsule.segment.b)
            .filter(|_| config.target_distance == 0.0)
        else {
            return self.spatial.cast_shape_predicate(
                shape,
                origin,
                rotation,
                direction,
                config,
                filter,
                &|entity| self.queryable(entity) && predicate(entity),
            );
        };
        self.cast_shape_with_capsule_core(
            shape, capsule, origin, rotation, direction, config, filter, predicate,
        )
    }

    pub(crate) fn cast_source_sphere_predicate(
        &self,
        sphere: &Collider,
        origin: Vec3,
        direction: Dir3,
        distance: f32,
        filter: &SpatialQueryFilter,
        predicate: &dyn Fn(Entity) -> bool,
    ) -> Option<ShapeHitData> {
        let radius = sphere.shape_scaled().as_ball()?.radius;
        let core = Capsule::new(Vec3::ZERO, Vec3::ZERO, radius);
        self.cast_shape_with_capsule_core(
            sphere,
            &core,
            origin,
            Quat::IDENTITY,
            direction,
            &ShapeCastConfig::from_max_distance(distance),
            filter,
            predicate,
        )
    }

    fn cast_shape_with_capsule_core(
        &self,
        shape: &Collider,
        capsule: &Capsule,
        origin: Vec3,
        rotation: Quat,
        direction: Dir3,
        config: &ShapeCastConfig,
        filter: &SpatialQueryFilter,
        predicate: &dyn Fn(Entity) -> bool,
    ) -> Option<ShapeHitData> {
        use crate::extensions::source_capsule_sweep::{CapsuleSweepResult, capsule_cuboid_sweep};
        use avian3d::parry::{
            math::Pose3,
            query::{ShapeCastOptions, cast_shapes, contact},
        };

        let scaled = shape.shape_scaled();
        let swept_pose = Pose3::from_parts(origin, rotation);
        let bounds = shape.aabb(origin, rotation, 0.0);
        let mut best_rank = config.max_distance;
        let mut closest = None;
        for tree in self.trees.iter_trees() {
            tree.sweep_traverse_closest(
                bounds.into(),
                direction,
                best_rank.max(0.0),
                config.target_distance,
                |proxy_id| {
                    let Some(proxy) = tree.get_proxy(proxy_id) else {
                        return f32::MAX;
                    };
                    let entity = proxy.collider;
                    if !filter.test(entity, proxy.layers)
                        || !self.queryable(entity)
                        || !predicate(entity)
                    {
                        return f32::MAX;
                    }
                    let Ok((position, obstacle_rotation, collider)) = self.colliders.get(entity)
                    else {
                        return f32::MAX;
                    };
                    let fixed_pose = Pose3::from_parts(position.0, obstacle_rotation.0);
                    let fixed_shape = collider.shape_scaled();
                    let mut result = fixed_shape.as_cuboid().map(|cuboid| {
                        capsule_cuboid_sweep(
                            capsule,
                            origin,
                            rotation,
                            cuboid,
                            position.0,
                            obstacle_rotation.0,
                            *direction,
                            config.max_distance,
                        )
                    });
                    if matches!(result, Some(CapsuleSweepResult::DeepOverlap))
                        && let Ok(Some(contact)) = contact(
                            &fixed_pose,
                            fixed_shape.as_ref(),
                            &swept_pose,
                            scaled.as_ref(),
                            0.0,
                        )
                        && contact.dist < 0.0
                    {
                        result = Some(CapsuleSweepResult::InitialOverlap {
                            depth: -contact.dist,
                            point: contact.point1,
                            normal: contact.normal1,
                        });
                    }
                    let fallback = || {
                        let options = ShapeCastOptions {
                            max_time_of_impact: config.max_distance,
                            target_distance: config.target_distance,
                            stop_at_penetration: !config.ignore_origin_penetration,
                            compute_impact_geometry_on_penetration: config
                                .compute_contact_on_penetration,
                        };
                        let hit = cast_shapes(
                            &fixed_pose,
                            Vec3::ZERO,
                            fixed_shape.as_ref(),
                            &swept_pose,
                            *direction,
                            scaled.as_ref(),
                            options,
                        )
                        .ok()??;
                        Some(ShapeHitData {
                            entity,
                            distance: hit.time_of_impact,
                            point1: fixed_pose * hit.witness1,
                            point2: swept_pose * hit.witness2 + *direction * hit.time_of_impact,
                            normal1: fixed_pose.rotation * hit.normal1,
                            normal2: swept_pose.rotation * hit.normal2,
                        })
                    };
                    let hit = match result {
                        Some(CapsuleSweepResult::Miss) => None,
                        Some(CapsuleSweepResult::Hit {
                            distance,
                            point,
                            normal,
                        }) => Some(ShapeHitData {
                            entity,
                            distance,
                            point1: point,
                            point2: point,
                            normal1: normal,
                            normal2: -normal,
                        }),
                        Some(CapsuleSweepResult::InitialOverlap {
                            depth,
                            point,
                            normal,
                        }) => {
                            if config.ignore_origin_penetration && normal.dot(*direction) >= 0.0 {
                                None
                            } else {
                                Some(ShapeHitData {
                                    entity,
                                    distance: 0.0,
                                    point1: point,
                                    point2: point - normal * depth,
                                    normal1: normal,
                                    normal2: -normal,
                                })
                            }
                        }
                        Some(CapsuleSweepResult::DeepOverlap) | None => fallback(),
                    };
                    let Some(hit) = hit else { return f32::MAX };
                    let rank = if hit.distance == 0.0 {
                        -(hit.point1 - hit.point2).dot(hit.normal1).max(0.0)
                    } else {
                        hit.distance
                    };
                    if rank < best_rank {
                        best_rank = rank;
                        closest = Some(hit);
                    }
                    hit.distance
                },
            );
        }
        closest
    }

    pub fn shape_intersections(
        &self,
        shape: &Collider,
        position: Vec3,
        rotation: Quat,
        filter: &SpatialQueryFilter,
    ) -> Vec<Entity> {
        let mut hits = Vec::new();
        self.shape_intersections_callback(shape, position, rotation, filter, |entity| {
            hits.push(entity);
            true
        });
        hits
    }

    pub fn shape_intersections_callback(
        &self,
        shape: &Collider,
        position: Vec3,
        rotation: Quat,
        filter: &SpatialQueryFilter,
        mut callback: impl FnMut(Entity) -> bool,
    ) {
        self.spatial
            .shape_intersections_callback(shape, position, rotation, filter, |entity| {
                !self.queryable(entity) || callback(entity)
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::prelude::{CollisionLayers, RigidBody};

    #[test]
    fn capsule_scene_sweep_matches_native_large_floor() {
        use avian3d::prelude::PhysicsPlugins;
        use bevy::{ecs::system::RunSystemOnce, time::TimeUpdateStrategy};
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            TransformPlugin,
            PhysicsPlugins::new(PostUpdate),
        ));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            1.0 / 60.0,
        )));
        app.finish();
        app.cleanup();
        let floor = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(20.0, 0.5, 20.0),
                Position(Vec3::new(0.0, -0.25, -20.0)),
                Rotation::IDENTITY,
                CollisionLayers::new(
                    [
                        crate::als::character::AlsCollisionLayer::Default,
                        crate::als::character::AlsCollisionLayer::Visibility,
                    ],
                    avian3d::prelude::LayerMask::ALL,
                ),
            ))
            .id();
        app.update();
        app.update();
        let (hit, prediction, prediction_distance) = app
            .world_mut()
            .run_system_once(move |spatial: SceneQuery| {
                let capsule = Collider::capsule(0.3, 1.2);
                let origin = Vec3::new(-7.9832387, 1.0044739, -15.300977);
                let direction =
                    Dir3::new(Vec3::new(0.76588976, -0.64297193, -0.00017596605)).unwrap();
                let hit = spatial.cast_shape(
                    &capsule,
                    origin,
                    Quat::IDENTITY,
                    direction,
                    &ShapeCastConfig::from_max_distance(0.3182573),
                    &SpatialQueryFilter::default(),
                );
                let velocity = *direction * 4.0;
                let prediction = crate::als::air::ground_prediction_sweep(
                    &spatial,
                    Entity::PLACEHOLDER,
                    origin,
                    velocity,
                    0.9,
                    0.3,
                );
                let prediction_distance = 1.5 + (velocity.y + 2.0) / -38.0 * 18.5;
                (hit, prediction, prediction_distance)
            })
            .unwrap();
        let hit = hit.unwrap();
        assert_eq!(hit.entity, floor);
        assert!(
            (hit.distance - 0.162486).abs() <= 1e-6,
            "Native large floor distance differs: {}",
            hit.distance
        );
        assert!(hit.normal1.abs_diff_eq(Vec3::Y, 1e-6));
        assert!(prediction.ground_valid);
        assert!((prediction.hit_fraction.unwrap() * prediction_distance - 0.162486).abs() <= 1e-6);
    }

    #[test]
    fn capsule_scene_sweep_preserves_filters_and_nearest_hit() {
        use avian3d::prelude::PhysicsPlugins;
        use bevy::{ecs::system::RunSystemOnce, time::TimeUpdateStrategy};
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            TransformPlugin,
            PhysicsPlugins::new(PostUpdate),
        ));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            1.0 / 60.0,
        )));
        app.finish();
        app.cleanup();
        let hidden = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(0.2, 4.0, 4.0),
                Position(Vec3::X),
                ShapeCollisionMode::PhysicsOnly,
            ))
            .id();
        let sphere = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::sphere(0.2),
                Position(Vec3::X * 2.0),
            ))
            .id();
        let wall = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(0.2, 4.0, 4.0),
                Position(Vec3::X * 2.5),
            ))
            .id();
        app.update();
        app.update();
        let (first, filtered, only_hidden) = app
            .world_mut()
            .run_system_once(move |spatial: SceneQuery| {
                let capsule = Collider::capsule(0.3, 1.2);
                let config = ShapeCastConfig::from_max_distance(4.0);
                let first = spatial.cast_shape(
                    &capsule,
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    Dir3::X,
                    &config,
                    &SpatialQueryFilter::default(),
                );
                let filtered = spatial.cast_shape(
                    &capsule,
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    Dir3::X,
                    &config,
                    &SpatialQueryFilter::from_excluded_entities([sphere]),
                );
                let only_hidden = spatial.cast_shape_predicate(
                    &capsule,
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    Dir3::X,
                    &config,
                    &SpatialQueryFilter::default(),
                    &|entity| entity == hidden,
                );
                (first, filtered, only_hidden)
            })
            .unwrap();
        assert_eq!(first.unwrap().entity, sphere);
        let filtered = filtered.unwrap();
        assert_eq!(filtered.entity, wall);
        assert!((filtered.distance - 2.1).abs() <= 1e-5);
        assert!(only_hidden.is_none());
    }

    #[test]
    fn capsule_scene_sweep_keeps_initial_penetration_and_outward_filter() {
        use avian3d::prelude::PhysicsPlugins;
        use bevy::{ecs::system::RunSystemOnce, time::TimeUpdateStrategy};
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            TransformPlugin,
            PhysicsPlugins::new(PostUpdate),
        ));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            1.0 / 60.0,
        )));
        app.finish();
        app.cleanup();
        let floor = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(100.0, 0.2, 100.0),
                Position(Vec3::new(0.0, -0.1, 0.0)),
            ))
            .id();
        let deep_floor = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(100.0, 1.0, 100.0),
                Position(Vec3::new(200.0, -0.5, 0.0)),
            ))
            .id();
        app.update();
        app.update();
        let (hit, outward, deep) = app
            .world_mut()
            .run_system_once(move |spatial: SceneQuery| {
                let capsule = Collider::capsule(0.3, 1.2);
                let origin = Vec3::Y * 0.89;
                let config = ShapeCastConfig::from_max_distance(0.5);
                let hit = spatial.cast_shape(
                    &capsule,
                    origin,
                    Quat::IDENTITY,
                    Dir3::NEG_Y,
                    &config,
                    &SpatialQueryFilter::default(),
                );
                let outward = spatial.cast_shape(
                    &capsule,
                    origin,
                    Quat::IDENTITY,
                    Dir3::Y,
                    &ShapeCastConfig {
                        ignore_origin_penetration: true,
                        ..config
                    },
                    &SpatialQueryFilter::default(),
                );
                let deep = spatial.cast_shape(
                    &Collider::capsule(0.3, 1.08),
                    Vec3::new(200.0, -0.2, 0.0),
                    Quat::IDENTITY,
                    Dir3::NEG_Y,
                    &ShapeCastConfig::from_max_distance(0.534001),
                    &SpatialQueryFilter::default(),
                );
                (hit, outward, deep)
            })
            .unwrap();
        let hit = hit.unwrap();
        assert_eq!(hit.entity, floor);
        assert_eq!(hit.distance, 0.0);
        assert!(((hit.point1 - hit.point2).dot(hit.normal1) - 0.01).abs() <= 1e-6);
        assert!(outward.is_none());
        let deep = deep.unwrap();
        assert_eq!(deep.entity, deep_floor);
        assert_eq!(deep.distance, 0.0);
        assert!(((deep.point1 - deep.point2).dot(deep.normal1) - 1.04).abs() <= 1e-6);
    }
}
