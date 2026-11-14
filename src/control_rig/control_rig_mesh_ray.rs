use bevy::{math::DVec3, prelude::Vec3};
use serde::Deserialize;

type Bounds = [[f32; 3]; 2];
const SMALL_NUMBER: f64 = 1e-8_f32 as f64;

#[derive(Deserialize)]
pub struct CookedMeshRay {
    vertices_cm: Vec<[f32; 3]>,
    triangles: Vec<[usize; 3]>,
    face_bounds_cm: Vec<Bounds>,
    nodes: Vec<[Option<MeshChild>; 2]>,
}

#[derive(Deserialize)]
struct MeshChild {
    index: usize,
    face_count: usize,
    bounds_cm: Bounds,
}

pub(crate) struct MeshRayImpact {
    pub(crate) time_cm: f64,
    pub(crate) point_cm: DVec3,
    pub(crate) normal: DVec3,
    #[cfg(test)]
    pub(crate) face: usize,
}

struct BoundsRay {
    start: Vec3,
    direction: Vec3,
    inverse_direction: Vec3,
}

impl BoundsRay {
    fn new(start: DVec3, direction: DVec3) -> Self {
        let direction = direction.as_vec3();
        Self {
            start: start.as_vec3(),
            direction,
            inverse_direction: direction.recip(),
        }
    }

    fn intersects(&self, bounds: &Bounds, length: f32) -> bool {
        let [minimum, maximum] = bounds;
        let mut near = 0.0_f32;
        let mut far = length;
        for axis in 0..3 {
            let start = self.start[axis];
            if self.direction[axis].abs() < 1e-8 {
                if start < minimum[axis] || start > maximum[axis] {
                    return false;
                }
                continue;
            }
            let inverse = self.inverse_direction[axis];
            let first = (minimum[axis] - start) * inverse;
            let second = (maximum[axis] - start) * inverse;
            near = near.max(first.min(second));
            far = far.min(first.max(second));
        }
        near <= far
    }
}

fn dot(a: DVec3, b: DVec3) -> f64 {
    a.x * b.x + a.z * b.z + a.y * b.y
}

fn cross(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(
        a.z * b.y - a.y * b.z,
        a.x * b.z - a.z * b.x,
        a.y * b.x - a.x * b.y,
    )
}

pub(crate) fn mesh_vector_length(vector: DVec3) -> f64 {
    dot(vector, vector).sqrt()
}

pub(crate) fn mesh_safe_normal(vector: DVec3) -> DVec3 {
    let squared = dot(vector, vector);
    if squared == 1.0 {
        vector
    } else if squared < SMALL_NUMBER {
        DVec3::ZERO
    } else {
        vector * squared.sqrt().recip()
    }
}

fn mesh_scaled_normal(normal: DVec3, inverse_scale: Vec3) -> DVec3 {
    let normal = normal.as_vec3() * inverse_scale;
    let squared = normal.x * normal.x + normal.z * normal.z + normal.y * normal.y;
    if squared == 1.0 {
        normal.as_dvec3()
    } else if squared < f32::MIN_POSITIVE {
        DVec3::ZERO
    } else {
        (normal * squared.sqrt().recip()).as_dvec3()
    }
}

fn triangle_impact(
    [a, b, c]: [DVec3; 3],
    start: DVec3,
    direction: DVec3,
    length: f32,
    cull_back_faces: bool,
) -> Option<(f64, DVec3)> {
    let ab = b - a;
    let ac = c - a;
    let normal = cross(ab, ac);
    let inverse_direction = -direction;
    let denominator = dot(inverse_direction, normal);
    if (cull_back_faces && dot(direction, normal) > 0.0) || denominator.abs() < SMALL_NUMBER {
        return None;
    }
    let inverse = denominator.recip();
    let offset = start - a;
    let time = dot(offset, normal) * inverse;
    if time < 0.0 || time > f64::from(length) {
        return None;
    }
    let relative = cross(inverse_direction, offset);
    let u = dot(ac, relative) * inverse;
    if u < -SMALL_NUMBER || u > 1.0 + SMALL_NUMBER {
        return None;
    }
    let v = -dot(ab, relative) * inverse;
    if v < -SMALL_NUMBER || u + v > 1.0 + SMALL_NUMBER {
        return None;
    }
    Some((time, mesh_safe_normal(normal) * denominator.signum()))
}

impl CookedMeshRay {
    fn trace_core(
        &self,
        start: DVec3,
        direction: DVec3,
        length: f64,
        cull_back_faces: bool,
    ) -> Option<MeshRayImpact> {
        let ray = BoundsRay::new(start, direction);
        let mut current_length = length as f32;
        let mut hit = None;
        let mut nodes = Vec::with_capacity(16);
        nodes.push(0);
        while let Some(index) = nodes.pop() {
            for child in self.nodes[index].iter().flatten() {
                if !ray.intersects(&child.bounds_cm, current_length) {
                    continue;
                }
                if child.face_count == 0 {
                    nodes.push(child.index);
                    continue;
                }
                for face in child.index..child.index + child.face_count {
                    if !ray.intersects(&self.face_bounds_cm[face], current_length) {
                        continue;
                    }
                    let triangle = self.triangles[face]
                        .map(|index| Vec3::from_array(self.vertices_cm[index]).as_dvec3());
                    if let Some((time, normal)) =
                        triangle_impact(triangle, start, direction, current_length, cull_back_faces)
                    {
                        current_length = time as f32;
                        hit = Some(MeshRayImpact {
                            time_cm: time,
                            point_cm: start + direction * time,
                            normal,
                            #[cfg(test)]
                            face,
                        });
                    }
                }
            }
        }
        hit.filter(|impact| impact.time_cm <= length)
    }

    pub(crate) fn trace(
        &self,
        start: DVec3,
        direction: DVec3,
        length: f64,
        scale: Option<Vec3>,
        cull_back_faces: bool,
    ) -> Option<MeshRayImpact> {
        let Some(scale) = scale else {
            return self.trace_core(start, direction, length, cull_back_faces);
        };
        let inverse_scale = scale.recip();
        let core_start = (start.as_vec3() * inverse_scale).as_dvec3();
        let core_direction = (direction.as_vec3() * inverse_scale).as_dvec3();
        let length_scale = mesh_vector_length(core_direction);
        if !length_scale.is_finite() || length_scale <= f64::MIN_POSITIVE {
            return None;
        }
        let inverse_length_scale = length_scale.recip();
        let mut hit = self.trace_core(
            core_start,
            core_direction * inverse_length_scale,
            length * length_scale,
            cull_back_faces,
        )?;
        hit.time_cm *= inverse_length_scale;
        if hit.time_cm >= length {
            return None;
        }
        hit.point_cm = (hit.point_cm.as_vec3() * scale).as_dvec3();
        hit.normal = mesh_scaled_normal(hit.normal, inverse_scale);
        Some(hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::prelude::{Collider, CollisionLayers, Position, Rotation};
    use bevy::{platform::collections::HashMap, prelude::*};
    use std::sync::Arc;

    use crate::extensions::{
        control_rig_hierarchy::{RigHierarchy, RigTransform},
        control_rig_ik::RigGeometry,
        control_rig_math::RigValue,
        control_rig_query::{RigCookedMeshRay, RigQueryContext, RigQueryWorld},
        control_rig_units::RigUnitContext,
    };

    #[derive(Deserialize)]
    struct Cases {
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        fixture: String,
        case: String,
        mesh: String,
        start_cm: [f64; 3],
        end_cm: [f64; 3],
        translation_cm: [f64; 3],
        scale: [f32; 3],
        scaled: bool,
        rig_center_m: Option<f64>,
        hit: Option<Hit>,
    }

    #[derive(Deserialize)]
    struct Hit {
        face: usize,
        position_cm: [f64; 3],
        normal: [f64; 3],
    }

    #[test]
    fn cooked_mesh_faces_and_foot_offsets_match_native_queries() {
        let meshes: HashMap<String, CookedMeshRay> = serde_json::from_str(include_str!(
            "../../assets/ALSExtras/Environment/Meshes/playground_mesh_ray_data.runtime.json"
        ))
        .unwrap();
        let meshes: HashMap<_, _> = meshes
            .into_iter()
            .map(|(name, mesh)| (name, Arc::new(mesh)))
            .collect();
        let cases: Cases = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d162b/cooked-ray-kernel.replay.json"
        ))
        .unwrap();
        let entity = World::new().spawn_empty().id();
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
        for case in cases.cases {
            let mesh = meshes[&case.mesh].clone();
            let scale = Vec3::from_array(case.scale);
            let start = DVec3::from_array(case.start_cm);
            let end = DVec3::from_array(case.end_cm);
            let position = DVec3::from_array(case.translation_cm);
            let delta = end - start;
            let length = f64::from(mesh_vector_length(delta) as f32);
            let local = mesh.trace(
                start - position,
                delta * length.recip(),
                length,
                case.scaled.then_some(scale),
                true,
            );
            assert_eq!(
                local.map(|impact| impact.face),
                case.hit.as_ref().map(|hit| hit.face),
                "Wrong cooked face for {}/{}",
                case.fixture,
                case.case
            );

            let mut collider = Collider::trimesh(
                mesh.vertices_cm
                    .iter()
                    .map(|vertex| Vec3::from_array(*vertex) * 0.01)
                    .collect(),
                mesh.triangles
                    .iter()
                    .map(|face| face.map(|vertex| vertex as u32))
                    .collect(),
            );
            collider.set_scale(scale, 10);
            let query = RigQueryContext {
                world: Arc::new(RigQueryWorld {
                    colliders: [(
                        entity,
                        (
                            Position((position / 100.0).as_vec3()),
                            Rotation::default(),
                            collider,
                            CollisionLayers::ALL,
                        ),
                    )]
                    .into_iter()
                    .collect(),
                    mesh_ray_data: [(
                        entity,
                        RigCookedMeshRay {
                            geometry: mesh,
                            cull_back_faces: true,
                        },
                    )]
                    .into_iter()
                    .collect(),
                }),
                to_world: RigTransform::default(),
                channel: "Visibility".to_owned(),
                filter: default(),
            };
            let actual = query
                .trace(start / 100.0, end / 100.0, "Visibility", true)
                .unwrap();
            assert_eq!(
                actual.is_some(),
                case.hit.is_some(),
                "Wrong mesh hit for {}/{}",
                case.fixture,
                case.case
            );
            let Some(expected) = case.hit else {
                continue;
            };
            let point = DVec3::from_array(expected.position_cm) / 100.0;
            let normal = DVec3::from_array(expected.normal);
            let (actual_point, actual_normal) = actual.unwrap();
            assert!(
                actual_point.distance(point) < 1e-7 && actual_normal.distance(normal) < 1e-10,
                "Wrong cooked hit for {}/{}: actual=({actual_point:?}, {actual_normal:?}), expected=({point:?}, {normal:?})",
                case.fixture,
                case.case
            );

            let Some(center) = case.rig_center_m else {
                continue;
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
                to_world: RigTransform {
                    translation: DVec3::Y * center,
                    ..default()
                },
                line_trace: Some(&mut trace),
            };
            use RigValue::*;
            let mut inputs = vec![
                Vector((start / 100.0).with_y(0.0)),
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
            context
                .call("FAlsRigUnit_FootOffsetTrace::Execute", &mut inputs)
                .unwrap();
            let F32(offset) = inputs[8] else {
                panic!("Missing rig mesh foot offset");
            };
            let Vector(rig_normal) = inputs[9] else {
                panic!("Missing rig mesh foot normal");
            };
            let slope = normal.y as f32;
            let expected_offset =
                (point.y - center + f64::from(0.134656_f32 / slope - 0.134656_f32)) as f32;
            assert!(
                (offset - expected_offset).abs() < 1e-6 && rig_normal.distance(normal) < 1e-10,
                "Wrong rig mesh foot input for {}",
                case.case
            );
        }
    }
}
