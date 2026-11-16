use std::sync::Arc;

use avian3d::prelude::*;
use bevy::{math::DVec3, platform::collections::HashMap, prelude::*};
use parry3d_f64::query::Ray;

use crate::extensions::{
    control_rig_hierarchy::RigTransform,
    control_rig_math::source_rotate_vector,
    cooked_mesh_raycast::{CookedMeshRayData, mesh_safe_normal, mesh_vector_length},
    shape_raycast::{nearer_hit, source_mesh_has_scale, trace_precise_shape},
};

#[derive(Default)]
pub struct RigQueryWorld {
    pub colliders: HashMap<Entity, (Position, Rotation, Collider, CollisionLayers)>,
    pub mesh_ray_data: HashMap<Entity, CookedMeshRayData>,
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
        self.trace_in_units(start, end, channel, complex, 1.0)
    }

    pub(crate) fn trace_in_units(
        &self,
        start: DVec3,
        end: DVec3,
        channel: &str,
        complex: bool,
        meters_per_unit: f64,
    ) -> Result<Option<(DVec3, DVec3)>, String> {
        if channel != self.channel || !complex {
            return Err("rig collision query mode is unsupported".to_owned());
        }
        let delta = end - start;
        if !delta.is_finite() || delta == DVec3::ZERO {
            return Ok(None);
        }
        let delta_meters = delta * meters_per_unit;
        let start_meters = start * meters_per_unit;
        let delta_f32 = delta_meters.as_vec3();
        let fallback = Dir3::new(delta_f32)
            .ok()
            .map(|direction| (direction, delta_f32.length()));
        let origin = start_meters.as_vec3();
        let centimeters_per_unit = meters_per_unit * 100.0;
        let start_cm = start * centimeters_per_unit;
        let delta_cm = end * centimeters_per_unit - start_cm;
        let length_cm = f64::from(mesh_vector_length(delta_cm) as f32);
        let mut nearest = 1.0;
        let mut hit = None;
        for (entity, (position, rotation, collider, layers)) in &self.world.colliders {
            if !self.filter.test(*entity, *layers) {
                continue;
            }
            let rotation_f64 = rotation.0.as_dquat();
            let candidate = if let Some(mesh) = self.world.mesh_ray_data.get(entity) {
                if !length_cm.is_finite() || length_cm <= f64::from(1e-4_f32) {
                    continue;
                }
                let position_cm = position.0.as_dvec3() * 100.0;
                let start = source_rotate_vector(rotation_f64.conjugate(), start_cm - position_cm);
                let direction =
                    source_rotate_vector(rotation_f64.conjugate(), delta_cm * length_cm.recip());
                let scale = collider.scale();
                let scale = source_mesh_has_scale(scale).then_some(scale);
                mesh.geometry
                    .trace(
                        start,
                        direction,
                        length_cm * nearest,
                        scale,
                        mesh.cull_back_faces,
                    )
                    .map(|impact| {
                        let point = (position_cm
                            + source_rotate_vector(rotation_f64, impact.point_cm))
                            * (0.01 / meters_per_unit);
                        let normal =
                            mesh_safe_normal(source_rotate_vector(rotation_f64, impact.normal));
                        (impact.time_cm / length_cm, point, normal)
                    })
            } else {
                let local = Ray::new(
                    source_rotate_vector(
                        rotation_f64.conjugate(),
                        start_meters - position.0.as_dvec3(),
                    ),
                    source_rotate_vector(rotation_f64.conjugate(), delta_meters),
                );
                match trace_precise_shape(
                    &**collider.shape_scaled(),
                    &**collider.shape(),
                    collider.scale(),
                    &local,
                    nearest,
                ) {
                    Ok(impact) => impact.map(|impact| {
                        (
                            impact.time_of_impact,
                            start + delta * impact.time_of_impact,
                            source_rotate_vector(rotation_f64, impact.normal),
                        )
                    }),
                    Err(()) => fallback.and_then(|(direction, distance)| {
                        collider
                            .cast_ray(
                                position.0,
                                *rotation,
                                origin,
                                direction.as_vec3(),
                                distance,
                                true,
                            )
                            .map(|(length, normal)| {
                                (
                                    f64::from(length) / f64::from(distance),
                                    (origin + direction.as_vec3() * length).as_dvec3()
                                        / meters_per_unit,
                                    normal.as_dvec3(),
                                )
                            })
                    }),
                }
            };
            let Some((fraction, point, normal)) = candidate else {
                continue;
            };
            if nearer_hit(fraction, nearest, hit.is_some()) {
                nearest = fraction;
                hit = Some(if fraction == 0.0 {
                    (start, (-delta).normalize_or_zero())
                } else {
                    (point, normal)
                });
            }
        }
        Ok(hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::SceneQuery;
    use crate::extensions::{
        control_rig_hierarchy::RigHierarchy,
        control_rig_ik::RigGeometry,
        control_rig_math::RigValue,
        control_rig_units::{RigUnitContext, call_unit},
    };

    #[test]
    fn rig_query_preserves_double_precision_on_source_box_ray() {
        let mut entities = World::new();
        let floor = entities.spawn_empty().id();
        let query = RigQueryContext {
            world: Arc::new(RigQueryWorld {
                colliders: [(
                    floor,
                    (
                        Position(Vec3::new(-10.0, -0.25, -20.0)),
                        Rotation::default(),
                        Collider::cuboid(40.0, 0.5, 20.0),
                        CollisionLayers::ALL,
                    ),
                )]
                .into_iter()
                .collect(),
                ..default()
            }),
            to_world: RigTransform::default(),
            channel: "Visibility".to_owned(),
            filter: default(),
        };
        let start = DVec3::new(
            -1515.661_365_680_93 * 0.01,
            49.999_994_039_535_52 * 0.01,
            -1508.488_173_045_665_7 * 0.01,
        );
        let end = start.with_y(-80.000_005_960_464_48 * 0.01);
        let hit = query
            .trace(start, end, "Visibility", true)
            .unwrap()
            .unwrap();
        assert_eq!(hit, (DVec3::new(start.x, 0.0, start.z), DVec3::Y));
        let inside = DVec3::new(-10.0, -0.25, -20.0);
        assert_eq!(
            query
                .trace(inside, inside + DVec3::Y, "Visibility", true)
                .unwrap(),
            Some((inside, DVec3::NEG_Y))
        );
        let direction = DVec3::new(3.0, -4.0, 0.0);
        let (point, normal) = query
            .trace(inside, inside + direction, "Visibility", true)
            .unwrap()
            .unwrap();
        assert_eq!(point, inside);
        assert!(normal.abs_diff_eq(DVec3::new(-0.6, 0.8, 0.0), 1e-12));
    }

    #[test]
    fn first_endpoint_hit_respects_the_source_shape_wrapper() {
        let mesh = Collider::trimesh(
            vec![
                Vec3::new(-2.0, 0.0, -2.0),
                Vec3::new(-2.0, 0.0, 2.0),
                Vec3::new(2.0, 0.0, -2.0),
            ],
            vec![[0, 1, 2]],
        );
        let compound = Collider::compound(vec![(
            Vec3::ZERO,
            Quat::IDENTITY,
            Collider::cuboid(2.0, 1.0, 2.0),
        )]);
        let mut entities = World::new();
        let entity = entities.spawn_empty().id();
        for (mut collider, scale, center, endpoint) in [
            (Collider::cuboid(2.0, 1.0, 2.0), 2.0, Vec3::NEG_Y, true),
            (mesh.clone(), 1.0, Vec3::ZERO, true),
            (mesh.clone(), 1.00005, Vec3::ZERO, true),
            (mesh, 2.0, Vec3::ZERO, false),
            (compound, 2.0, Vec3::NEG_Y, true),
        ] {
            collider.set_scale(Vec3::splat(scale), 10);
            let query = RigQueryContext {
                world: Arc::new(RigQueryWorld {
                    colliders: [(
                        entity,
                        (
                            Position(center),
                            Rotation::default(),
                            collider,
                            CollisionLayers::ALL,
                        ),
                    )]
                    .into_iter()
                    .collect(),
                    ..default()
                }),
                to_world: RigTransform::default(),
                channel: "Visibility".to_owned(),
                filter: default(),
            };
            let start = DVec3::new(-0.5, 1.0, -0.5);
            for (end_y, expected) in [(0.0001, false), (0.0, endpoint), (-0.0001, true)] {
                let result = query
                    .trace(start, start.with_y(end_y), "Visibility", true)
                    .unwrap();
                assert_eq!(
                    result.is_some(),
                    expected,
                    "Wrong ray boundary for scale {scale} and end {end_y}"
                );
                if let Some((point, normal)) = result {
                    assert!(point.y.abs() < 1e-12);
                    assert_eq!(normal, DVec3::Y);
                }
            }
        }
        assert!(nearer_hit(1.0, 1.0, false));
        assert!(!nearer_hit(1.0, 1.0, true));
        assert!(nearer_hit(0.5, 1.0, true));
    }

    #[test]
    fn exact_box_endpoint_reaches_foot_offset_trace() {
        let mut entities = World::new();
        let floor = entities.spawn_empty().id();
        let to_world = RigTransform {
            translation: DVec3::Y * (f64::from(0.8_f32) * 1.25 - 1.0),
            scale: DVec3::splat(1.25),
            ..default()
        };
        let query = RigQueryContext {
            world: Arc::new(RigQueryWorld {
                colliders: [(
                    floor,
                    (
                        Position(Vec3::NEG_Y * 1.25),
                        Rotation::default(),
                        Collider::cuboid(2.0, 0.5, 2.0),
                        CollisionLayers::ALL,
                    ),
                )]
                .into_iter()
                .collect(),
                ..default()
            }),
            to_world,
            channel: "Visibility".to_owned(),
            filter: default(),
        };
        let mut trace = |start, end: DVec3, channel: &str, complex| {
            assert_eq!(end.y, -1.0);
            query.trace(start, end, channel, complex)
        };
        let mut hierarchy = RigHierarchy::new(Vec::new(), 1e-6).unwrap();
        let geometry = RigGeometry {
            meters_per_unit: 1.0,
            near_zero: 1e-6,
            normalize_squared: 1e-12,
            minimum_length: 1e-10,
            solve_near_zero: 1e-6,
            basis: [DVec3::X, DVec3::Z, DVec3::Y],
        };
        let curves = HashMap::default();
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &curves,
            game_world: true,
            construction: false,
            delta_seconds: 1.0 / 60.0,
            geometry: &geometry,
            to_world,
            line_trace: Some(&mut trace),
        };
        use RigValue::*;
        let mut values = vec![
            Vector(DVec3::ZERO),
            Name("Visibility".to_owned()),
            F32(0.5),
            F32(0.8),
            F32(45.0),
            F32(0.134656),
            Bool(true),
            Bool(false),
            F32(0.0),
            Vector(DVec3::ZERO),
        ];
        call_unit(
            &mut context,
            "FAlsRigUnit_FootOffsetTrace::Execute",
            &mut values,
        )
        .unwrap();
        let F32(offset) = values[8] else {
            panic!("Missing foot offset");
        };
        assert!(
            (offset + 0.766336).abs() < 1e-6,
            "Endpoint foot offset was {offset}"
        );
    }

    #[test]
    fn initial_box_overlap_reaches_the_rig_foot_offset() {
        let mut entities = World::new();
        let floor = entities.spawn_empty().id();
        let mut hierarchy = RigHierarchy::new(Vec::new(), 1e-6).unwrap();
        let geometry = RigGeometry {
            meters_per_unit: 1.0,
            near_zero: 1e-6,
            normalize_squared: 1e-12,
            minimum_length: 1e-10,
            solve_near_zero: 1e-6,
            basis: [DVec3::X, DVec3::Z, DVec3::Y],
        };
        let curves = HashMap::default();
        for (half_height, expected) in [(0.75, 0.5), (0.5, 0.5), (0.4, 0.4)] {
            let query = RigQueryContext {
                world: Arc::new(RigQueryWorld {
                    colliders: [(
                        floor,
                        (
                            Position::default(),
                            Rotation::default(),
                            Collider::cuboid(2.0, 2.0 * half_height, 2.0),
                            CollisionLayers::ALL,
                        ),
                    )]
                    .into_iter()
                    .collect(),
                    ..default()
                }),
                to_world: RigTransform::default(),
                channel: "Visibility".to_owned(),
                filter: default(),
            };
            let mut trace =
                |start, end, channel: &str, complex| query.trace(start, end, channel, complex);
            let mut context = RigUnitContext {
                hierarchy: &mut hierarchy,
                curves: &curves,
                game_world: true,
                construction: false,
                delta_seconds: 1.0 / 60.0,
                geometry: &geometry,
                to_world: RigTransform::default(),
                line_trace: Some(&mut trace),
            };
            use RigValue::*;
            let mut values = vec![
                Vector(DVec3::ZERO),
                Name("Visibility".to_owned()),
                F32(0.5),
                F32(0.8),
                F32(45.0),
                F32(0.134656),
                Bool(true),
                Bool(false),
                F32(0.0),
                Vector(DVec3::ZERO),
            ];
            call_unit(
                &mut context,
                "FAlsRigUnit_FootOffsetTrace::Execute",
                &mut values,
            )
            .unwrap();
            let F32(offset) = values[8] else {
                panic!("Missing foot offset");
            };
            assert!(
                (offset - expected).abs() < 1e-6,
                "Foot offset {offset} at half height {half_height}"
            );
            assert_eq!(values[9], Vector(DVec3::Y));
        }
    }

    #[test]
    fn rig_query_selects_nearest_mixed_precision_collider() {
        let mut entities = World::new();
        let floor = entities.spawn_empty().id();
        let sphere = entities.spawn_empty().id();
        let mut query = RigQueryContext {
            world: Arc::new(RigQueryWorld {
                colliders: [
                    (
                        floor,
                        (
                            Position(Vec3::new(0.0, -0.25, 0.0)),
                            Rotation::default(),
                            Collider::cuboid(10.0, 0.5, 10.0),
                            CollisionLayers::ALL,
                        ),
                    ),
                    (
                        sphere,
                        (
                            Position(Vec3::Y),
                            Rotation::default(),
                            Collider::sphere(0.5),
                            CollisionLayers::ALL,
                        ),
                    ),
                ]
                .into_iter()
                .collect(),
                ..default()
            }),
            to_world: RigTransform::default(),
            channel: "Visibility".to_owned(),
            filter: default(),
        };
        let start = DVec3::Y * 3.0;
        let end = DVec3::NEG_Y * 3.0;
        assert_eq!(
            query
                .trace(start, end, "Visibility", true)
                .unwrap()
                .unwrap()
                .0
                .y,
            1.5
        );
        query.filter.excluded_entities.insert(sphere);
        assert_eq!(
            query
                .trace(start, end, "Visibility", true)
                .unwrap()
                .unwrap()
                .0
                .y,
            0.0
        );
    }

    #[test]
    fn rig_query_traverses_mesh_bvh_with_double_precision_ray() {
        let mut entities = World::new();
        let mesh = entities.spawn_empty().id();
        let vertices = vec![
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new(-1.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, -1.0),
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, -1.0),
        ];
        let query = RigQueryContext {
            world: Arc::new(RigQueryWorld {
                colliders: [(
                    mesh,
                    (
                        Position::default(),
                        Rotation::default(),
                        Collider::trimesh(vertices, vec![[0, 1, 2], [3, 4, 5]]),
                        CollisionLayers::ALL,
                    ),
                )]
                .into_iter()
                .collect(),
                ..default()
            }),
            to_world: RigTransform::default(),
            channel: "Visibility".to_owned(),
            filter: default(),
        };
        let start = DVec3::new(0.123_456_789_012 * 0.01, 2.0, -0.234_567_891_234 * 0.01);
        let hit = query
            .trace(start, start.with_y(-2.0), "Visibility", true)
            .unwrap()
            .unwrap();
        assert_eq!(hit, (DVec3::new(start.x, 0.0, start.z), DVec3::Y));
        assert_eq!(
            query
                .trace(
                    DVec3::new(3.0, 2.0, 0.0),
                    DVec3::new(3.0, -2.0, 0.0),
                    "Visibility",
                    true
                )
                .unwrap(),
            None
        );
        let single = RigQueryContext {
            world: Arc::new(RigQueryWorld {
                colliders: [(
                    mesh,
                    (
                        Position::default(),
                        Rotation::default(),
                        Collider::trimesh(
                            vec![
                                Vec3::new(-1.0, 0.0, -1.0),
                                Vec3::new(-1.0, 0.0, 1.0),
                                Vec3::new(1.0, 0.0, -1.0),
                            ],
                            vec![[0, 1, 2]],
                        ),
                        CollisionLayers::ALL,
                    ),
                )]
                .into_iter()
                .collect(),
                ..default()
            }),
            ..query
        };
        assert_eq!(
            single
                .trace(start, start.with_y(-2.0), "Visibility", true)
                .unwrap(),
            Some(hit)
        );
    }

    #[test]
    fn rig_query_keeps_double_ray_coordinates_for_curved_shapes() {
        let mut entities = World::new();
        let entity = entities.spawn_empty().id();
        let start = DVec3::new(0.123_456_789_012, 2.0, -0.234_567_891_234);
        for collider in [
            Collider::sphere(0.7),
            Collider::capsule(0.4, 0.8),
            Collider::cylinder(0.7, 1.0),
        ] {
            let query = RigQueryContext {
                world: Arc::new(RigQueryWorld {
                    colliders: [(
                        entity,
                        (
                            Position::default(),
                            Rotation::default(),
                            collider,
                            CollisionLayers::ALL,
                        ),
                    )]
                    .into_iter()
                    .collect(),
                    ..default()
                }),
                to_world: RigTransform::default(),
                channel: "Visibility".to_owned(),
                filter: default(),
            };
            let (hit, _) = query
                .trace(start, start.with_y(-2.0), "Visibility", true)
                .unwrap()
                .unwrap();
            assert_eq!(hit.x, start.x);
            assert_eq!(hit.z, start.z);
        }
    }

    #[test]
    fn rig_query_handles_compound_convex_with_double_ray() {
        let mut entities = World::new();
        let entity = entities.spawn_empty().id();
        let points = [-0.5, 0.5]
            .into_iter()
            .flat_map(|x| {
                [-0.5, 0.5]
                    .into_iter()
                    .flat_map(move |y| [-0.5, 0.5].into_iter().map(move |z| Vec3::new(x, y, z)))
            })
            .collect::<Vec<_>>();
        let collider = Collider::compound(vec![
            (
                Vec3::Y * 0.5,
                Quat::IDENTITY,
                Collider::convex_hull(&points).unwrap(),
            ),
            (Vec3::NEG_Y, Quat::IDENTITY, Collider::sphere(0.25)),
        ]);
        let query = RigQueryContext {
            world: Arc::new(RigQueryWorld {
                colliders: [(
                    entity,
                    (
                        Position::default(),
                        Rotation::default(),
                        collider,
                        CollisionLayers::ALL,
                    ),
                )]
                .into_iter()
                .collect(),
                ..default()
            }),
            to_world: RigTransform::default(),
            channel: "Visibility".to_owned(),
            filter: default(),
        };
        let start = DVec3::new(0.123_456_789_012, 2.0, -0.234_567_891_234);
        let (hit, _) = query
            .trace(start, start.with_y(-2.0), "Visibility", true)
            .unwrap()
            .unwrap();
        assert_eq!((hit.x, hit.z), (start.x, start.z));
        assert!((hit.y - 1.0).abs() < 1e-5);
    }

    #[test]
    fn rig_query_keeps_fallback_for_unhandled_shape() {
        let mut entities = World::new();
        let entity = entities.spawn_empty().id();
        let query = RigQueryContext {
            world: Arc::new(RigQueryWorld {
                colliders: [(
                    entity,
                    (
                        Position::default(),
                        Rotation::default(),
                        Collider::cone(0.7, 1.0),
                        CollisionLayers::ALL,
                    ),
                )]
                .into_iter()
                .collect(),
                ..default()
            }),
            to_world: RigTransform::default(),
            channel: "Visibility".to_owned(),
            filter: default(),
        };
        let start = DVec3::new(0.123_456_789_012, 2.0, -0.234_567_891_234);
        let (hit, _) = query
            .trace(start, start.with_y(-2.0), "Visibility", true)
            .unwrap()
            .unwrap();
        assert_eq!(hit.x, f64::from(start.x as f32));
    }

    #[test]
    fn control_rig_query_uses_avian_geometry_filters_and_world_transforms() {
        fn capture(
            In(filter): In<SpatialQueryFilter>,
            colliders: Query<(Entity, &Position, &Rotation, &Collider, &CollisionLayers)>,
            spatial: SceneQuery,
        ) -> (RigQueryWorld, Option<RayHitData>) {
            (
                RigQueryWorld {
                    colliders: colliders
                        .iter()
                        .map(|(entity, position, rotation, collider, layers)| {
                            (entity, (*position, *rotation, collider.clone(), *layers))
                        })
                        .collect(),
                    ..default()
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
                CollisionLayers::new(
                    crate::als::character::AlsCollisionLayer::Visibility,
                    LayerMask::ALL,
                ),
            ))
            .id();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(1.0, 0.2, 1.0),
            Transform::from_xyz(0.0, 2.0, 0.0),
            CollisionLayers::new(
                crate::als::character::AlsCollisionLayer::Default,
                LayerMask::ALL,
            ),
        ));
        let floor = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(20.0, 0.2, 20.0),
                Transform::from_xyz(0.0, -0.1, 0.0),
                CollisionLayers::new(
                    crate::als::character::AlsCollisionLayer::Visibility,
                    LayerMask::ALL,
                ),
            ))
            .id();
        for _ in 0..3 {
            app.update();
        }
        let filter =
            SpatialQueryFilter::from_mask(crate::als::character::AlsCollisionLayer::Visibility)
                .with_excluded_entities([owner]);
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
            meters_per_unit: 1.0,
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
        call_unit(
            &mut context,
            "FAlsRigUnit_FootOffsetTrace::Execute",
            &mut values,
        )
        .unwrap();
        assert_eq!(calls, 1);
        let F32(offset) = values[8] else {
            panic!("invalid offset")
        };
        assert!((offset + 0.45).abs() < 1e-6);
        assert_eq!(values[9], Vector(DVec3::Y * 2.0));
    }
}
