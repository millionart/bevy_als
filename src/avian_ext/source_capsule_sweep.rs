use avian3d::parry::shape::{Capsule, Cuboid};
use bevy::math::{DQuat, DVec3};
use bevy::prelude::{Quat, Vec3};

use crate::extensions::source_math::source_rotate_vector;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CapsuleSweepResult {
    Miss,
    InitialOverlap { depth: f32, point: Vec3, normal: Vec3 },
    DeepOverlap,
    Hit { distance: f32, distance_cm: f32, point: Vec3, normal: Vec3 },
}

#[derive(Clone, Copy, Default)]
struct Vertex {
    difference: Vec3,
    fixed: Vec3,
    moving: Vec3,
}

#[derive(Clone, Copy, Default)]
struct Simplex {
    vertices: [Vertex; 4],
    len: usize,
    weights: [f32; 4],
}

impl Simplex {
    fn reduce_to(&mut self, indices: &[usize], weights: &[f32]) {
        let old = self.vertices;
        for (new, &index) in indices.iter().enumerate() {
            self.vertices[new] = old[index];
        }
        self.len = indices.len();
        self.weights = [0.0; 4];
        self.weights[..weights.len()].copy_from_slice(weights);
    }

    fn nearest(&mut self) -> Vec3 {
        match self.len {
            1 => {
                self.weights = [1.0, 0.0, 0.0, 0.0];
                self.vertices[0].difference
            }
            2 => self.nearest_line(),
            3 => self.nearest_triangle(),
            4 => self.nearest_tetrahedron(),
            _ => unreachable!(),
        }
    }

    fn nearest_line(&mut self) -> Vec3 {
        let a = self.vertices[0].difference;
        let b = self.vertices[1].difference;
        let edge = b - a;
        let numerator = (-a).dot(edge);
        let denominator = edge.length_squared();
        if denominator <= f32::MIN_POSITIVE || numerator >= denominator {
            self.reduce_to(&[1], &[1.0]);
            b
        } else if numerator <= 0.0 {
            self.reduce_to(&[0], &[1.0]);
            a
        } else {
            let fraction = (numerator / denominator).clamp(0.0, 1.0);
            self.weights = [1.0 - fraction, fraction, 0.0, 0.0];
            edge.mul_add(Vec3::splat(fraction), a)
        }
    }

    fn nearest_triangle(&mut self) -> Vec3 {
        let a = self.vertices[0].difference;
        let b = self.vertices[1].difference;
        let c = self.vertices[2].difference;
        let ab = b - a;
        let ac = c - a;
        let plane = ab.cross(ac);
        let plane_squared = plane.length_squared();
        if (a * f32::MIN_POSITIVE).length_squared() >= plane_squared {
            self.len = 2;
            return self.nearest_line();
        }
        let d1 = ab.dot(-a);
        let d2 = ac.dot(-a);
        if d1 <= 0.0 && d2 <= 0.0 {
            self.reduce_to(&[0], &[1.0]);
            return a;
        }
        let d3 = ab.dot(-b);
        let d4 = ac.dot(-b);
        if d3 >= 0.0 && d3 >= d4 {
            self.reduce_to(&[1], &[1.0]);
            return b;
        }
        let vc = (-d3).mul_add(d2, d1 * d4);
        let ab_denominator = d1 - d3;
        if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 && ab_denominator > 0.0 {
            let fraction = d1 / ab_denominator;
            self.reduce_to(&[0, 1], &[1.0 - fraction, fraction]);
            return ab.mul_add(Vec3::splat(fraction), a);
        }
        let d5 = ab.dot(-c);
        let d6 = ac.dot(-c);
        if d6 >= 0.0 && d6 >= d5 {
            self.reduce_to(&[2], &[1.0]);
            return c;
        }
        let vb = (-d1).mul_add(d6, d5 * d2);
        let ac_denominator = d2 - d6;
        if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 && ac_denominator > 0.0 {
            let fraction = d2 / ac_denominator;
            self.reduce_to(&[0, 2], &[1.0 - fraction, fraction]);
            return ac.mul_add(Vec3::splat(fraction), a);
        }
        let va = (-d5).mul_add(d4, d3 * d6);
        let bc_denominator = (d4 - d3) + (d5 - d6);
        if va <= 0.0 && d4 >= d3 && d5 >= d6 && bc_denominator > 0.0 {
            let fraction = (d4 - d3) / bc_denominator;
            self.reduce_to(&[1, 2], &[1.0 - fraction, fraction]);
            return (c - b).mul_add(Vec3::splat(fraction), b);
        }
        let denominator = (va + (vb + vc)).recip();
        let v = vb * denominator;
        let w = vc * denominator;
        self.weights = [1.0 - v - w, v, w, 0.0];
        plane * a.dot(plane / plane_squared)
    }

    fn nearest_tetrahedron(&mut self) -> Vec3 {
        let [a, b, c, d] = self.vertices.map(|vertex| vertex.difference);
        let determinants = [-b.dot(c.cross(d)), a.dot(c.cross(d)),
            -a.dot(b.cross(d)), a.dot(b.cross(c))];
        let total = (determinants[0] + determinants[1]) + (determinants[2] + determinants[3]);
        let faces = [[1, 2, 3], [0, 2, 3], [0, 1, 3], [0, 1, 2]];
        let mut nearest = None;
        for (index, determinant) in determinants.into_iter().enumerate() {
            if determinant != 0.0 && total != 0.0 && determinant.is_sign_positive() == total.is_sign_positive() {
                continue;
            }
            let mut face = *self;
            face.reduce_to(&faces[index], &[0.0; 3]);
            let point = face.nearest_triangle();
            if nearest.as_ref().is_none_or(|(distance, _, _): &(f32, Vec3, Simplex)| point.length_squared() < *distance) {
                nearest = Some((point.length_squared(), point, face));
            }
        }
        if let Some((_, point, face)) = nearest {
            *self = face;
            point
        } else {
            self.weights = determinants.map(|value| value / total);
            Vec3::ZERO
        }
    }
}

fn axes(value: DVec3) -> DVec3 {
    DVec3::new(value.x, value.z, value.y)
}

fn box_support(direction: Vec3, extents: Vec3) -> Vec3 {
    Vec3::new(if direction.x < 0.0 { -extents.x } else { extents.x },
        if direction.y < 0.0 { -extents.y } else { extents.y },
        if direction.z < 0.0 { -extents.z } else { extents.z })
}

fn support(direction: Vec3, extents: Vec3, a: Vec3, b: Vec3, axis: Vec3) -> Vertex {
    let fixed = box_support(direction, extents);
    let moving = if (-direction).dot(axis) >= 0.0 { b } else { a };
    Vertex { difference: Vec3::ZERO, fixed, moving }
}

pub(crate) fn capsule_cuboid_sweep(
    capsule: &Capsule, origin: Vec3, rotation: Quat, cuboid: &Cuboid, center: Vec3,
    cuboid_rotation: Quat, direction: Vec3, limit: f32, source_dimensions_cm: Option<(f32, f32)>,
) -> CapsuleSweepResult {
    let box_rotation = DQuat::from_xyzw(cuboid_rotation.x as f64, cuboid_rotation.y as f64,
        cuboid_rotation.z as f64, cuboid_rotation.w as f64);
    let capsule_rotation = DQuat::from_xyzw(rotation.x as f64, rotation.y as f64,
        rotation.z as f64, rotation.w as f64);
    let box_inverse = box_rotation.inverse();
    let segment = capsule.segment.b - capsule.segment.a;
    let segment_axis = segment.normalize_or_zero();
    let segment_center = (capsule.segment.a + capsule.segment.b) * 0.5;
    let (radius, half_height) = source_dimensions_cm.unwrap_or_else(|| (
        (f64::from(capsule.radius) * 100.0) as f32,
        (f64::from(segment.length() * 0.5 + capsule.radius) * 100.0) as f32,
    ));
    let half_segment = (half_height - radius).max(1e-4);
    let start = (origin.as_dvec3() - center.as_dvec3()) * 100.0
        + capsule_rotation * (segment_center.as_dvec3() * 100.0
            - segment_axis.as_dvec3() * half_segment as f64);
    let local_start = source_rotate_vector(box_inverse, start);
    let local_axis = box_inverse * (capsule_rotation * segment_axis.as_dvec3());
    let a = axes(local_start).as_vec3();
    let b = axes(local_start + local_axis * (2.0 * half_segment) as f64).as_vec3();
    let axis = axes(local_axis).as_vec3();
    let extents = axes(cuboid.half_extents.as_dvec3() * 100.0).as_vec3();
    let direction = axes(box_inverse * direction.as_dvec3()).as_vec3();
    let mut simplex = Simplex::default();
    let initial = support(direction, extents, a, b, axis);
    simplex.vertices[0] = initial;
    let mut separation = initial.moving - initial.fixed;
    let mut time = 0.0f32;
    let mut position = Vec3::ZERO;
    let mut normal = Vec3::Z;
    let mut previous_distance = f32::MAX;
    let inflated_squared = radius.mul_add(radius, 1e-6);
    let mut close = separation.length_squared() < 1e-6;
    let mut inflated_close = close;
    for _ in 0..32 {
        if close { break; }
        separation = separation.normalize();
        let mut next = support(separation, extents, a, b, axis);
        next.difference = position - (next.fixed - next.moving);
        simplex.vertices[simplex.len] = next;
        let projected = separation.dot(next.difference);
        if projected > radius {
            let approach = separation.dot(direction);
            if approach >= 0.0 { return CapsuleSweepResult::Miss; }
            let next_time = time - (projected - radius) / approach;
            if next_time > time {
                if next_time > limit { return CapsuleSweepResult::Miss; }
                time = next_time;
                position = direction * time;
                normal = separation;
                for vertex in &mut simplex.vertices[..simplex.len] {
                    vertex.difference = position - (vertex.fixed - vertex.moving);
                }
                simplex.vertices[simplex.len].difference = position - (next.fixed - next.moving);
                simplex.len += 1;
                previous_distance = f32::MAX;
                inflated_close = false;
            }
        } else {
            simplex.len += 1;
        }
        if inflated_close && projected >= 0.0 {
            close = 1e-6 + projected >= previous_distance;
        }
        if simplex.len == 0 { return CapsuleSweepResult::Miss; }
        separation = simplex.nearest();
        if !close {
            let current = separation.length_squared();
            close = inflated_squared > current;
            let degenerate = current >= previous_distance;
            previous_distance = current;
            if close && time == 0.0 && current > 1e-6 && inflated_squared > 1e-6 && simplex.len < 4 {
                close = false;
                inflated_close = true;
            }
            if degenerate { break; }
        }
    }
    let mut moving_point = Vec3::ZERO;
    for index in 0..simplex.len {
        moving_point = simplex.vertices[index].moving.mul_add(Vec3::splat(simplex.weights[index]), moving_point);
    }
    let from_source = |value: Vec3| DVec3::new(value.x as f64, value.z as f64, value.y as f64);
    if time <= 0.0 {
        if previous_distance <= 1e-6 || previous_distance == f32::MAX {
            return CapsuleSweepResult::DeepOverlap;
        }
        normal = separation.normalize();
        let depth = (radius - previous_distance.sqrt()).max(0.0);
        let contact = normal.mul_add(Vec3::splat(depth),
            (-normal).mul_add(Vec3::splat(radius), moving_point));
        let world_point = center.as_dvec3() + box_rotation * (from_source(contact) * 0.01);
        let world_normal = (box_rotation * from_source(normal)).normalize().as_vec3();
        return CapsuleSweepResult::InitialOverlap {
            depth: depth * 0.01, point: world_point.as_vec3(), normal: world_normal,
        };
    }
    let contact = position + (-normal).mul_add(Vec3::splat(radius), moving_point);
    let world_point = center.as_dvec3() + box_rotation * (from_source(contact) * 0.01);
    let world_normal = (box_rotation * from_source(normal)).normalize().as_vec3();
    CapsuleSweepResult::Hit { distance: time * 0.01, distance_cm: time, point: world_point.as_vec3(), normal: world_normal }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn source_vector(value: &Value, scale: f64) -> Vec3 {
        Vec3::new((value[0].as_f64().unwrap() * scale) as f32,
            (value[2].as_f64().unwrap() * scale) as f32,
            (value[1].as_f64().unwrap() * scale) as f32)
    }

    fn source_point(value: &Value, scale: f64) -> Vec3 {
        Vec3::new((value["x"].as_f64().unwrap() * scale) as f32,
            (value["z"].as_f64().unwrap() * scale) as f32,
            (value["y"].as_f64().unwrap() * scale) as f32)
    }

    #[test]
    fn positive_capsule_box_sweeps_match_native_distance_normal_and_point() {
        let document: Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d125a/native-fixtures.json")).unwrap();
        let samples = document["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 14);
        for sample in samples {
            let input = &sample["input"];
            let query = &input["capsule_sweep"];
            let output = &sample["output"];
            let name = input["name"].as_str().unwrap();
            let radius = (query["radius_cm"].as_f64().unwrap() * 0.01) as f32;
            let half = (query["half_height_cm"].as_f64().unwrap() * 0.01) as f32;
            let capsule = Capsule::new_y(half - radius, radius);
            let box_shape = Cuboid::new(source_vector(&query["box_half_extents_cm"], 0.01));
            let quaternion = &input["rotation_xyzw"];
            let orientation = Quat::from_xyzw(-(quaternion[0].as_f64().unwrap() as f32),
                -(quaternion[2].as_f64().unwrap() as f32),
                -(quaternion[1].as_f64().unwrap() as f32), quaternion[3].as_f64().unwrap() as f32);
            let result = capsule_cuboid_sweep(&capsule, source_vector(&query["start_cm"], 0.01),
                Quat::IDENTITY, &box_shape, source_vector(&query["box_translation_cm"], 0.01),
                orientation, source_vector(&query["direction"], 1.0),
                query["length_cm"].as_f64().unwrap() as f32, None);
            let CapsuleSweepResult::Hit { distance, point, normal, .. } = result else {
                panic!("Native sweep {name} missed: {result:?}");
            };
            let expected_distance = (output["distance_cm"].as_f64().unwrap() * 0.01) as f32;
            let expected_point = source_point(&output["position_cm"], 0.01);
            let expected_normal = source_point(&output["normal"], 1.0);
            assert!((distance - expected_distance).abs() <= 1e-6,
                "Native sweep {name} distance differs: {distance} != {expected_distance}");
            assert!(point.abs_diff_eq(expected_point, 1e-6),
                "Native sweep {name} point differs: {point:?} != {expected_point:?}");
            assert!(normal.abs_diff_eq(expected_normal, 1e-6),
                "Native sweep {name} normal differs: {normal:?} != {expected_normal:?}");
        }
    }

    #[test]
    fn capsule_box_sweep_does_not_hit_behind_its_path() {
        let capsule = Capsule::new_y(0.6, 0.3);
        let box_shape = Cuboid::new(Vec3::ONE);
        let result = capsule_cuboid_sweep(&capsule, Vec3::new(3.0, 0.0, 0.0), Quat::IDENTITY,
            &box_shape, Vec3::ZERO, Quat::IDENTITY, Vec3::X, 500.0, None);
        assert_eq!(result, CapsuleSweepResult::Miss);
    }

    #[test]
    fn capsule_box_initial_floor_overlap_returns_source_penetration() {
        let capsule = Capsule::new_y(0.6, 0.3);
        for width in [4.0, 100.0] {
            for expected_depth in [0.01, 0.1] {
                let box_shape = Cuboid::new(Vec3::new(width * 0.5, 0.1, width * 0.5));
                let result = capsule_cuboid_sweep(&capsule, Vec3::Y * (0.9 - expected_depth),
                    Quat::IDENTITY, &box_shape, Vec3::new(0.0, -0.1, 0.0), Quat::IDENTITY,
                    Vec3::NEG_Y, 50.0, None);
                let CapsuleSweepResult::InitialOverlap { depth, point, normal } = result else {
                    panic!("Initial floor overlap was not detected: {result:?}");
                };
                assert!((depth - expected_depth).abs() <= 1e-6,
                    "Initial floor overlap depth differs: {depth} != {expected_depth}");
                assert!(point.y.abs() <= 1e-6, "Initial floor overlap point differs: {point:?}");
                assert!(normal.abs_diff_eq(Vec3::Y, 1e-6), "Initial floor overlap normal differs: {normal:?}");
            }
        }
    }
}
