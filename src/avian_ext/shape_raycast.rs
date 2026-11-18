use avian3d::parry::{
    partitioning::TraversalAction,
    shape::{Shape, TypedShape},
};
use bevy::{math::DVec3, prelude::Vec3};
use bevy_control_rig::math::source_rotate_vector;
use parry3d_f64::{
    bounding_volume::Aabb,
    query::{
        Ray, RayCast, RayIntersection,
        details::local_ray_intersection_with_support_map_with_params, gjk::VoronoiSimplex,
    },
    shape::{Ball, Capsule, Cuboid, Cylinder, SupportMap, Triangle},
};

pub(crate) fn nearer_hit(fraction: f64, nearest: f64, already_hit: bool) -> bool {
    fraction < nearest || (!already_hit && fraction == nearest)
}

pub(crate) fn source_mesh_has_scale(scale: Vec3) -> bool {
    (scale.as_dvec3() - DVec3::ONE).abs().max_element() >= f64::from(1e-4_f32)
}

fn trace_trimesh(
    mesh: &avian3d::parry::shape::TriMesh,
    ray: &Ray,
    limit: f64,
) -> Option<RayIntersection> {
    let mut nearest = limit;
    let mut hit = None;
    mesh.bvh().traverse(|node| {
        let bounds = node.aabb();
        if Aabb::new(bounds.mins.as_dvec3(), bounds.maxs.as_dvec3())
            .cast_local_ray(ray, nearest, true)
            .is_none()
        {
            return TraversalAction::Prune;
        }
        if let Some(index) = node.leaf_data() {
            let triangle = mesh.triangle(index);
            let triangle = Triangle::new(
                triangle.a.as_dvec3(),
                triangle.b.as_dvec3(),
                triangle.c.as_dvec3(),
            );
            if let Some(impact) = triangle.cast_local_ray_and_get_normal(ray, nearest, true)
                && nearer_hit(impact.time_of_impact, nearest, hit.is_some())
            {
                nearest = impact.time_of_impact;
                hit = Some(impact);
            }
        }
        TraversalAction::Continue
    });
    hit
}

struct PreciseConvex<'a>(&'a [Vec3]);

impl SupportMap for PreciseConvex<'_> {
    fn local_support_point(&self, direction: DVec3) -> DVec3 {
        let mut best = self.0[0].as_dvec3();
        let mut best_dot = best.dot(direction);
        for point in &self.0[1..] {
            let point = point.as_dvec3();
            let dot = point.dot(direction);
            if dot > best_dot {
                best = point;
                best_dot = dot;
            }
        }
        best
    }
}

pub(crate) fn trace_precise_shape(
    shape: &dyn Shape,
    unscaled: &dyn Shape,
    scale: Vec3,
    ray: &Ray,
    limit: f64,
) -> Result<Option<RayIntersection>, ()> {
    let hit = match shape.as_typed_shape() {
        TypedShape::Cuboid(shape) => Cuboid::new(shape.half_extents.as_dvec3())
            .cast_local_ray_and_get_normal(ray, limit, true),
        TypedShape::TriMesh(shape) => trace_trimesh(shape, ray, limit),
        TypedShape::Ball(shape) => {
            Ball::new(f64::from(shape.radius)).cast_local_ray_and_get_normal(ray, limit, true)
        }
        TypedShape::Capsule(shape) => Capsule::new(
            shape.segment.a.as_dvec3(),
            shape.segment.b.as_dvec3(),
            f64::from(shape.radius),
        )
        .cast_local_ray_and_get_normal(ray, limit, true),
        TypedShape::Cylinder(shape) => {
            Cylinder::new(f64::from(shape.half_height), f64::from(shape.radius))
                .cast_local_ray_and_get_normal(ray, limit, true)
        }
        TypedShape::ConvexPolyhedron(shape) => local_ray_intersection_with_support_map_with_params(
            &PreciseConvex(shape.points()),
            &mut VoronoiSimplex::new(),
            ray,
            limit,
            true,
        ),
        TypedShape::Compound(shape) => {
            let TypedShape::Compound(unscaled) = unscaled.as_typed_shape() else {
                return Err(());
            };
            let mut nearest = limit;
            let mut result = None;
            for ((pose, child), (_, unscaled_child)) in shape.shapes().iter().zip(unscaled.shapes())
            {
                let rotation = pose.rotation.as_dquat();
                let local = Ray::new(
                    source_rotate_vector(
                        rotation.conjugate(),
                        ray.origin - pose.translation.as_dvec3(),
                    ),
                    source_rotate_vector(rotation.conjugate(), ray.dir),
                );
                if let Some(mut impact) =
                    trace_precise_shape(&**child, &**unscaled_child, scale, &local, nearest)?
                    && nearer_hit(impact.time_of_impact, nearest, result.is_some())
                {
                    nearest = impact.time_of_impact;
                    impact.normal = source_rotate_vector(rotation, impact.normal);
                    result = Some(impact);
                }
            }
            result
        }
        _ => return Err(()),
    };
    let inclusive = match unscaled.as_typed_shape() {
        TypedShape::TriMesh(_) => !source_mesh_has_scale(scale),
        TypedShape::ConvexPolyhedron(_) => scale == Vec3::ONE,
        _ => true,
    };
    Ok(hit.filter(|impact| {
        impact.time_of_impact < limit || (inclusive && impact.time_of_impact == limit)
    }))
}
