use bevy::math::DVec3;

use super::source_contact_capsule::SourceCapsuleGeometry;
use super::source_contact_polygon::{source_direction, source_position};
use super::source_contact_segment::source_nearest_segments;

const NORMAL_TOLERANCE: f64 = 1.0e-5;
const NORMAL_TOLERANCE_SQUARED: f64 = NORMAL_TOLERANCE * NORMAL_TOLERANCE;
const FACE_CONTACT_SIN_ANGLE_THRESHOLD: f64 = 0.34;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceTriangleContactKind {
    VertexPlane,
    EdgeEdge,
}

pub(super) struct SourceTriangleContact {
    pub first_cm: DVec3,
    pub second_cm: DVec3,
    pub normal: DVec3,
    pub phi_cm: f64,
    pub kind: SourceTriangleContactKind,
}

#[derive(Clone, Copy, Default)]
struct SourceCapsuleTriangleEdge {
    delta: DVec3,
    edge_point: DVec3,
    segment_point: DVec3,
    edge_t: f64,
    segment_t: f64,
    distance_squared: f64,
    sign: f64,
    dot_face: f64,
}

struct SourceCapsuleTriangle {
    segment: [DVec3; 2],
    axis: DVec3,
    radius: f64,
    triangle: [DVec3; 3],
    face_normal: DVec3,
    face_distances: [f64; 2],
    edge_normals: [DVec3; 3],
    edge_distances: [[f64; 2]; 3],
    edges: [SourceCapsuleTriangleEdge; 3],
    distance_tolerance: f64,
    reject_distance: f64,
    collided: [bool; 2],
    contacts: Vec<SourceTriangleContact>,
}

impl SourceCapsuleTriangle {
    fn prefers_face_contact(&self) -> bool {
        self.axis.dot(self.face_normal).abs() < FACE_CONTACT_SIN_ANGLE_THRESHOLD
    }

    fn add_parallel_edge_contacts(&mut self, edge_index: usize) {
        let start = self.triangle[(edge_index + 2) % 3];
        let edge = self.triangle[edge_index] - start;
        let length_squared = edge.length_squared();
        if length_squared <= NORMAL_TOLERANCE_SQUARED {
            return;
        }
        let parameters = self.segment.map(|point| (point - start).dot(edge) / length_squared);
        let radius = self.radius;
        let reject_squared = self.reject_distance * self.reject_distance;
        let mut add_contact = |point: DVec3, delta: DVec3| {
            let squared = delta.length_squared();
            if squared > NORMAL_TOLERANCE_SQUARED && squared < reject_squared {
                let distance = squared.sqrt();
                let normal = delta / distance;
                let phi_cm = distance - radius;
                self.contacts.push(SourceTriangleContact {
                    first_cm: point + phi_cm * normal,
                    second_cm: point,
                    normal,
                    phi_cm,
                    kind: SourceTriangleContactKind::EdgeEdge,
                });
            }
        };
        if parameters.iter().any(|parameter| (0.0..=1.0).contains(parameter)) {
            for (point, parameter) in self.segment.into_iter().zip(parameters) {
                let projected = start + parameter * edge;
                let clamped = start + parameter.clamp(0.0, 1.0) * edge;
                add_contact(clamped, point - projected);
            }
        } else {
            let [first, second] = parameters;
            let use_first = if first < 0.0 && second < 0.0 { first > second } else { first < second };
            let index = if use_first { 0 } else { 1 };
            let clamped = start + parameters[index].clamp(0.0, 1.0) * edge;
            add_contact(clamped, self.segment[index] - clamped);
        }
    }

    fn add_parallel_or_cylinder_contacts(&mut self) -> bool {
        if self.contacts.len() == 2 {
            return true;
        }
        if (self.face_distances[0] - self.face_distances[1]).abs() <= self.distance_tolerance {
            let parallel = self.edge_distances.iter().position(|&[first, second]| {
                first >= 0.0 && second >= 0.0 && (first - second).abs() <= self.distance_tolerance
            });
            if let Some(edge_index) = parallel {
                self.add_parallel_edge_contacts(edge_index);
                return true;
            }
        }
        if !self.prefers_face_contact() {
            return false;
        }

        let mut radial = self.axis.cross(self.face_normal).cross(self.axis);
        let squared = radial.length_squared();
        if squared <= NORMAL_TOLERANCE {
            return false;
        }
        radial *= squared.sqrt().recip();
        if radial.dot(self.face_normal) > 0.0 {
            radial = -radial;
        }
        for index in 0..2 {
            if self.collided[index] || self.edge_distances.iter().any(|distance| {
                distance[index] > self.radius + self.distance_tolerance
            }) {
                continue;
            }
            let first_cm = self.segment[index] + self.radius * radial;
            let inside = (0..3).all(|edge| {
                (first_cm - self.triangle[edge]).dot(self.edge_normals[edge]) <= self.distance_tolerance
            });
            if inside {
                let phi_cm = (first_cm - self.triangle[0]).dot(self.face_normal);
                self.contacts.push(SourceTriangleContact {
                    first_cm,
                    second_cm: first_cm - phi_cm * self.face_normal,
                    normal: self.face_normal,
                    phi_cm,
                    kind: SourceTriangleContactKind::VertexPlane,
                });
            }
        }
        self.contacts.len() == 2
    }

    fn add_edge_contacts(&mut self) {
        let prefer_face = self.prefers_face_contact();
        for (index, edge) in self.edges.into_iter().enumerate() {
            if self.edge_distances[index].iter().all(|&distance| distance <= -self.distance_tolerance)
                || edge.dot_face < -NORMAL_TOLERANCE
            {
                continue;
            }
            let previous = (index + 2) % 3;
            let edge_direction = self.triangle[index] - self.triangle[previous];
            let face_contact = prefer_face
                && edge.edge_t > 0.0 && edge.edge_t < 1.0
                && edge.segment_t > 0.0 && edge.segment_t < 1.0;

            let (normal, signed_distance) = if edge.distance_squared > NORMAL_TOLERANCE_SQUARED {
                let distance = edge.distance_squared.sqrt();
                (edge.delta / distance, distance * edge.sign)
            } else {
                let mut normal = self.axis.cross(edge_direction);
                let squared = normal.length_squared();
                if squared <= NORMAL_TOLERANCE_SQUARED {
                    continue;
                }
                normal *= squared.sqrt().recip();
                if normal.dot(self.face_normal) < 0.0 {
                    normal = -normal;
                }
                if self.edge_normals[index].dot(normal) < -NORMAL_TOLERANCE {
                    continue;
                }
                (normal, 0.0)
            };

            if !face_contact {
                if self.edge_normals[index].dot(normal) < -NORMAL_TOLERANCE {
                    continue;
                }
                let vertex = if edge.edge_t == 0.0 {
                    Some(previous)
                } else if edge.edge_t == 1.0 {
                    Some(index)
                } else {
                    None
                };
                if let Some(vertex) = vertex {
                    let inside_normal = (0..3).any(|other| {
                        other != vertex
                            && (self.triangle[vertex] - self.triangle[other]).dot(normal) < -NORMAL_TOLERANCE
                    });
                    if inside_normal {
                        continue;
                    }
                }
            }

            let first_cm = edge.segment_point - self.radius * normal;
            if face_contact {
                let phi_cm = (first_cm - self.triangle[0]).dot(self.face_normal);
                self.contacts.push(SourceTriangleContact {
                    first_cm,
                    second_cm: first_cm - phi_cm * self.face_normal,
                    normal: self.face_normal,
                    phi_cm,
                    kind: SourceTriangleContactKind::VertexPlane,
                });
            } else {
                self.contacts.push(SourceTriangleContact {
                    first_cm,
                    second_cm: edge.edge_point,
                    normal,
                    phi_cm: signed_distance - self.radius,
                    kind: SourceTriangleContactKind::EdgeEdge,
                });
            }
        }
    }
}

fn source_capsule_triangle_start(
    capsule: SourceCapsuleGeometry,
    triangle_m: [DVec3; 3],
    cull_m: f64,
) -> Option<SourceCapsuleTriangle> {
    let first = source_position(capsule.first_m).as_vec3();
    let axis = source_direction(capsule.axis).as_vec3();
    let length = (capsule.length_m * 100.0) as f32;
    let segment = [first.as_dvec3(), (first + axis * length).as_dvec3()];
    let axis = axis.as_dvec3();
    let radius = f64::from((capsule.radius_m * 100.0) as f32);
    let reject_distance = radius + cull_m * 100.0;
    let distance_tolerance = f64::from(1.0e-5_f32 * length);

    let triangle = triangle_m.map(source_position);
    let face_cross = (triangle[1] - triangle[0]).cross(triangle[2] - triangle[0]);
    let face_length_squared = face_cross.length_squared();
    if face_length_squared <= NORMAL_TOLERANCE_SQUARED {
        return None;
    }
    let face_normal = face_cross * face_length_squared.sqrt().recip();
    let face_distances = segment.map(|point| (point - triangle[0]).dot(face_normal));
    if (face_distances[0] > reject_distance && face_distances[1] > reject_distance)
        || 0.5 * (face_distances[0] + face_distances[1]) < -distance_tolerance
    {
        return None;
    }

    let mut edge_normals = [DVec3::ZERO; 3];
    let mut edge_distances = [[0.0; 2]; 3];
    for index in 0..3 {
        let previous = (index + 2) % 3;
        let normal = (triangle[index] - triangle[previous]).cross(face_normal);
        let squared = normal.length_squared();
        edge_normals[index] = if squared > NORMAL_TOLERANCE_SQUARED {
            normal * squared.sqrt().recip()
        } else {
            normal
        };
        edge_distances[index] = segment.map(|point| (point - triangle[index]).dot(edge_normals[index]));
        if edge_distances[index].iter().all(|&distance| distance > reject_distance) {
            return None;
        }
    }

    let mut edges = [SourceCapsuleTriangleEdge::default(); 3];
    for index in 0..3 {
        let previous = (index + 2) % 3;
        let ([segment_t, edge_t], [segment_point, edge_point]) =
            source_nearest_segments(segment, [triangle[previous], triangle[index]]);
        let mut delta = segment_point - edge_point;
        let distance_squared = delta.length_squared();
        let sign = if delta.dot(edge_normals[index]) < -NORMAL_TOLERANCE {
            delta = -delta;
            -1.0
        } else {
            1.0
        };
        let dot_face = delta.dot(face_normal);
        let separation_cull = if dot_face < -NORMAL_TOLERANCE { radius } else { reject_distance };
        if sign > 0.0 && distance_squared > separation_cull * separation_cull {
            return None;
        }
        edges[index] = SourceCapsuleTriangleEdge {
            delta,
            edge_point,
            segment_point,
            edge_t,
            segment_t,
            distance_squared,
            sign,
            dot_face,
        };
    }

    let mut contacts = Vec::new();
    let mut collided = [false; 2];
    for index in 0..2 {
        let inside = edge_distances.iter().all(|distance| distance[index] <= distance_tolerance);
        let distance = face_distances[index];
        if inside && distance < reject_distance && distance < face_distances[1 - index] + distance_tolerance {
            contacts.push(SourceTriangleContact {
                first_cm: segment[index] - radius * face_normal,
                second_cm: segment[index] - distance * face_normal,
                normal: face_normal,
                phi_cm: distance - radius,
                kind: SourceTriangleContactKind::VertexPlane,
            });
            collided[index] = true;
        }
    }
    Some(SourceCapsuleTriangle {
        segment,
        axis,
        radius,
        triangle,
        face_normal,
        face_distances,
        edge_normals,
        edge_distances,
        edges,
        distance_tolerance,
        reject_distance,
        collided,
        contacts,
    })
}

pub(super) fn source_capsule_triangle_contacts(
    capsule: SourceCapsuleGeometry,
    triangle_m: [DVec3; 3],
    cull_m: f64,
) -> Vec<SourceTriangleContact> {
    let Some(mut state) = source_capsule_triangle_start(capsule, triangle_m, cull_m) else {
        return Vec::new();
    };
    if !state.add_parallel_or_cylinder_contacts() {
        state.add_edge_contacts();
    }
    state.contacts
}

#[cfg(test)]
mod tests {
    use bevy::math::DVec3;

    use super::*;

    #[derive(serde::Deserialize)]
    struct NativeCapsuleSample {
        label: String,
        endpoints_cm: [[f64; 3]; 2],
        radius_cm: f64,
        cull_cm: f64,
        triangles: Vec<NativeTriangle>,
    }

    #[derive(serde::Deserialize)]
    struct NativeTriangle {
        index: usize,
        vertices_cm: [[f64; 3]; 3],
        points: Vec<NativePoint>,
    }

    #[derive(serde::Deserialize)]
    struct NativePoint {
        first_cm: [f64; 3],
        second_cm: [f64; 3],
        normal: [f64; 3],
        phi_cm: f64,
        contact_type: u8,
    }

    impl NativeCapsuleSample {
        fn capsule(&self) -> SourceCapsuleGeometry {
            let [first, second] = self.endpoints_cm.map(DVec3::from_array);
            let delta = second - first;
            SourceCapsuleGeometry {
                first_m: source_direction(first) * 0.01,
                axis: source_direction(delta.normalize()),
                length_m: delta.length() * 0.01,
                radius_m: self.radius_cm * 0.01,
            }
        }
    }

    impl NativeTriangle {
        fn vertices_m(&self) -> [DVec3; 3] {
            self.vertices_cm.map(|point| source_direction(DVec3::from_array(point)) * 0.01)
        }
    }

    fn assert_native_contacts(
        actual: &[SourceTriangleContact],
        expected: &[NativePoint],
        label: (&str, usize),
        normal_tolerance: f64,
    ) {
        assert_eq!(actual.len(), expected.len(), "{label:?}");
        for (actual, expected) in actual.iter().zip(expected) {
            let contact_type = match actual.kind {
                SourceTriangleContactKind::VertexPlane => 1,
                SourceTriangleContactKind::EdgeEdge => 2,
            };
            assert_eq!(contact_type, expected.contact_type, "{label:?}");
            assert!(actual.first_cm.distance(DVec3::from_array(expected.first_cm)) < 1e-5, "{label:?}");
            assert!(actual.second_cm.distance(DVec3::from_array(expected.second_cm)) < 1e-5, "{label:?}");
            assert!(actual.normal.distance(DVec3::from_array(expected.normal)) < normal_tolerance, "{label:?}");
            assert!((actual.phi_cm - expected.phi_cm).abs() < 1e-5, "{label:?}");
        }
    }

    fn triangle() -> [DVec3; 3] {
        [DVec3::new(-2.0, 0.0, -2.0), DVec3::new(2.0, 0.0, -2.0), DVec3::new(-2.0, 0.0, 2.0)]
    }

    fn capsule(first: DVec3, axis: DVec3, length: f64) -> SourceCapsuleGeometry {
        SourceCapsuleGeometry { first_m: first, axis, length_m: length, radius_m: 0.1 }
    }

    #[test]
    fn source_capsule_triangle_endcaps_preserve_order_and_single_sided_culling() {
        let capsule = capsule(DVec3::new(-0.2, 0.08, -0.2), DVec3::X, 0.4);
        let state = source_capsule_triangle_start(capsule, triangle(), 0.03).unwrap();
        assert_eq!(state.contacts.len(), 2);
        assert_eq!(state.collided, [true, true]);
        assert_eq!(state.contacts[0].kind, SourceTriangleContactKind::VertexPlane);
        assert_eq!(state.contacts[0].first_cm, DVec3::new(-20.0, -20.0, -2.0));
        assert_eq!(state.contacts[1].first_cm, DVec3::new(20.0, -20.0, -2.0));
        assert_eq!(state.contacts[0].second_cm, DVec3::new(-20.0, -20.0, 0.0));
        assert_eq!(state.contacts[0].normal, DVec3::Z);
        assert_eq!(state.contacts[0].phi_cm, -2.0);
        let [a, b, c] = triangle();
        assert!(source_capsule_triangle_start(capsule, [a, c, b], 0.03).is_none());
    }

    #[test]
    fn source_capsule_triangle_culls_separated_and_degenerate_faces() {
        let high = capsule(DVec3::new(-0.2, 0.2, -0.2), DVec3::X, 0.4);
        assert!(source_capsule_triangle_start(high, triangle(), 0.03).is_none());
        let outside = capsule(DVec3::new(5.0, 0.08, 5.0), DVec3::X, 0.4);
        assert!(source_capsule_triangle_start(outside, triangle(), 0.03).is_none());
        let close = capsule(DVec3::new(-0.2, 0.08, -0.2), DVec3::Y, 0.1);
        assert!(source_capsule_triangle_start(close, [DVec3::ZERO; 3], 0.03).is_none());
        let state = source_capsule_triangle_start(close, triangle(), 0.03).unwrap();
        assert_eq!(state.collided, [true, false]);
        assert_eq!(state.contacts.len(), 1);
    }

    #[test]
    fn source_capsule_triangle_initial_contacts_match_native_caps_and_backfaces() {
        let samples: Vec<NativeCapsuleSample> = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b4b2a2/triangle-cases.json"
        ))
        .unwrap();
        for sample in samples {
            let endpoints = sample.endpoints_cm.map(DVec3::from_array);
            let capsule = sample.capsule();
            for face in &sample.triangles {
                let state = source_capsule_triangle_start(capsule, face.vertices_m(), sample.cull_cm * 0.01);
                let label = (sample.label.as_str(), face.index);
                if face.points.is_empty() {
                    assert!(state.is_none(), "{label:?}");
                    continue;
                }
                let state = state.unwrap_or_else(|| panic!("Source rejected {label:?}"));
                let caps = face.points.iter().take_while(|point| {
                    let support = DVec3::from_array(point.first_cm) + sample.radius_cm * DVec3::from_array(point.normal);
                    point.contact_type == 1 && endpoints.iter().any(|endpoint| endpoint.distance(support) < 1e-6)
                }).count();
                assert_native_contacts(&state.contacts, &face.points[..caps], label, 1e-9);
            }
        }
    }

    #[test]
    fn source_capsule_triangle_parallel_and_cylinder_contacts_match_native() {
        let mut checked = 0;
        for fixture in [
            include_str!("../../docs/audit/bf144/d100b4b2a2/triangle-cases.json"),
            include_str!("../../docs/audit/bf144/d100b4b2a3/parallel-cases.json"),
        ] {
            let samples: Vec<NativeCapsuleSample> = serde_json::from_str(fixture).unwrap();
            for sample in samples {
                let capsule = sample.capsule();
                for face in &sample.triangles {
                    let cylinder = face.points.len() == 2 && matches!(face.index, 1053 | 175 | 176);
                    if cylinder || sample.label.starts_with("stair_parallel_") {
                        let label = (sample.label.as_str(), face.index);
                        let mut state = source_capsule_triangle_start(capsule, face.vertices_m(), sample.cull_cm * 0.01)
                            .unwrap_or_else(|| panic!("Source rejected {label:?}"));
                        assert!(state.add_parallel_or_cylinder_contacts(), "{label:?}");
                        assert_native_contacts(&state.contacts, &face.points, label, 1e-7);
                        checked += 1;
                    }
                }
            }
        }
        assert_eq!(checked, 6);
    }

    #[test]
    fn source_capsule_triangle_cylinder_respects_face_angle() {
        for (sine, completes) in [(0.33_f64, true), (0.35, false)] {
            let axis = DVec3::new((1.0 - sine * sine).sqrt(), sine, 0.0);
            let capsule = capsule(DVec3::new(-0.2, 0.08, -0.3), axis, 0.1);
            let mut state = source_capsule_triangle_start(capsule, triangle(), 0.03).unwrap();
            assert_eq!(state.contacts.len(), 1);
            assert_eq!(state.add_parallel_or_cylinder_contacts(), completes);
            assert_eq!(state.contacts.len(), if completes { 2 } else { 1 });
        }
    }

    #[test]
    fn source_capsule_triangle_final_contacts_match_native_manifolds() {
        let mut checked = 0;
        for fixture in [
            include_str!("../../docs/audit/bf144/d100b4b2a2/triangle-cases.json"),
            include_str!("../../docs/audit/bf144/d100b4b2a3/parallel-cases.json"),
        ] {
            let samples: Vec<NativeCapsuleSample> = serde_json::from_str(fixture).unwrap();
            for sample in samples {
                let capsule = sample.capsule();
                for face in &sample.triangles {
                    let contacts = source_capsule_triangle_contacts(capsule, face.vertices_m(), sample.cull_cm * 0.01);
                    assert_native_contacts(&contacts, &face.points, (sample.label.as_str(), face.index), 1e-7);
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 15);
    }

    #[test]
    fn source_capsule_triangle_zero_distance_crossing_uses_face_normal() {
        let capsule = capsule(DVec3::new(0.0, 0.0, -2.1), DVec3::Z, 0.2);
        let contacts = source_capsule_triangle_contacts(capsule, triangle(), 0.03);
        assert_eq!(contacts.len(), 2);
        assert_eq!(contacts[0].second_cm, DVec3::new(0.0, -190.0, 0.0));
        let crossing = &contacts[1];
        assert_eq!(crossing.kind, SourceTriangleContactKind::VertexPlane);
        assert_eq!(crossing.first_cm, DVec3::new(0.0, -200.0, -10.0));
        assert_eq!(crossing.second_cm, DVec3::new(0.0, -200.0, 0.0));
        assert_eq!(crossing.normal, DVec3::Z);
    }

    #[test]
    fn source_capsule_triangle_vertex_normal_must_leave_both_incident_edges() {
        let capsule = capsule(DVec3::new(-0.04, 0.08, -0.01), DVec3::Y, 0.1);
        let triangle = [DVec3::ZERO, DVec3::X, DVec3::new(-1.0, 0.0, 1.0)];
        let contacts = source_capsule_triangle_contacts(capsule, triangle, 0.03);
        assert_eq!(contacts.len(), 1);
        let edge = &contacts[0];
        assert_eq!(edge.kind, SourceTriangleContactKind::EdgeEdge);
        assert!(edge.second_cm.distance(DVec3::new(-1.5, 1.5, 0.0)) < 1e-12);
        assert!(edge.normal.distance(DVec3::new(-2.5, -2.5, 8.0).normalize()) < 1e-12);
    }
}
