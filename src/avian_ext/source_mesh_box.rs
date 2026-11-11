use bevy::math::DVec3;

use super::source_contact_polygon::{SOURCE_BOX_FACE_VERTICES, SOURCE_SMALL_NUMBER, clip_plane, reduce_source_points_along, source_position};
use super::source_mesh_contact::{SourceTriangleContact, SourceTriangleContactKind, source_triangle_normal};

const SAT_DATA: ([[usize; 4]; 12], f32, f64) = include!("source_mesh_box_sat_data.rs");
const BOX_NORMALS: [DVec3; 6] = [DVec3::NEG_X, DVec3::NEG_Y, DVec3::NEG_Z, DVec3::X, DVec3::Y, DVec3::Z];

#[derive(Clone, Copy)]
struct SourceTriangleBox {
    center: DVec3,
    half_extents: DVec3,
}

impl SourceTriangleBox {
    fn from_bounds_cm([minimum, maximum]: [DVec3; 2]) -> Self {
        Self {
            center: 0.5 * (minimum + maximum),
            half_extents: 0.5 * (maximum - minimum),
        }
    }

    fn vertex(self, index: usize) -> DVec3 {
        let sign = |bit: usize| if index & bit == 0 { -1.0 } else { 1.0 };
        self.center + DVec3::new(sign(1), sign(2), sign(4)) * self.half_extents
    }

    fn plane_point(self, index: usize) -> DVec3 {
        self.center + BOX_NORMALS[index] * self.half_extents
    }
}

struct SourceBoxTriangleSat {
    kind: SourceTriangleContactKind,
    normal: DVec3,
    clip_axis: DVec3,
    clip_box_to_triangle: bool,
    box_plane: usize,
    triangle_normal: DVec3,
    triangle_center: DVec3,
}

fn most_opposing_box_plane(normal: DVec3) -> usize {
    let absolute = normal.abs();
    let axis = if absolute.x > absolute.y && absolute.x > absolute.z {
        0
    } else if absolute.y > absolute.z {
        1
    } else {
        2
    };
    if normal[axis] < 0.0 { axis + 3 } else { axis }
}

fn source_box_triangle_sat(
    shape: SourceTriangleBox,
    triangle: [DVec3; 3],
    cull_cm: f64,
) -> Option<SourceBoxTriangleSat> {
    let triangle_normal = source_triangle_normal(triangle);
    let triangle_center = (triangle[0] + triangle[1] + triangle[2]) / 3.0;
    let vertices: [DVec3; 8] = std::array::from_fn(|index| shape.vertex(index));
    let vertex_distances = vertices.map(|vertex| (vertex - triangle_center).dot(triangle_normal));
    let triangle_separation = vertex_distances.into_iter().fold(f64::MAX, f64::min);
    if triangle_separation > cull_cm || vertex_distances.into_iter().fold(f64::MIN, f64::max) < 0.0 {
        return None;
    }

    let mut box_separation = f64::MIN;
    let mut box_plane = 0;
    for (index, normal) in BOX_NORMALS.into_iter().enumerate() {
        let point = shape.plane_point(index);
        let separation = triangle.iter().map(|vertex| (*vertex - point).dot(normal)).fold(f64::MAX, f64::min);
        if separation > cull_cm {
            return None;
        }
        if separation > box_separation {
            box_separation = separation;
            box_plane = index;
        }
    }

    let box_normal = BOX_NORMALS[box_plane];
    let box_point = shape.plane_point(box_plane);
    let [distance0, distance1, distance2] = triangle.map(|point| (point - box_point).dot(box_normal));
    let triangle_edge_distances = [distance2.min(distance0), distance0.min(distance1), distance1.min(distance2)];
    let mut edge_separation = f64::MIN;
    let mut edge_normal = DVec3::ZERO;
    let minkowski_tolerance = f64::from(SAT_DATA.1);
    for [first, second, plane_a, plane_b] in SAT_DATA.0 {
        if vertex_distances[first] > cull_cm && vertex_distances[second] > cull_cm {
            continue;
        }
        let box_edge = vertices[second] - vertices[first];
        let cba = (-triangle_normal).dot(box_edge);
        for (index, &triangle_edge_distance) in triangle_edge_distances.iter().enumerate() {
            if triangle_edge_distance > cull_cm {
                continue;
            }
            let start = triangle[index];
            let triangle_edge = triangle[(index + 1) % 3] - start;
            let adc = BOX_NORMALS[plane_a].dot(triangle_edge);
            let bdc = BOX_NORMALS[plane_b].dot(triangle_edge);
            if adc * bdc >= -minkowski_tolerance || cba * bdc <= minkowski_tolerance {
                continue;
            }

            let mut axis = box_edge.cross(triangle_edge);
            let length_squared = axis.length_squared();
            if (start - triangle_center).dot(axis) < 0.0 {
                axis = -axis;
            }
            let separation = (vertices[first] - start).dot(axis);
            let signed_square = separation * separation.abs();
            if signed_square > cull_cm * cull_cm * length_squared {
                return None;
            }
            if signed_square > edge_separation * edge_separation.abs() * length_squared {
                let inverse_length = length_squared.sqrt().recip();
                edge_separation = separation * inverse_length;
                edge_normal = axis * inverse_length;
            }
        }
    }

    let orient = |axis: DVec3| if axis.dot(shape.center - triangle_center) < 0.0 { -axis } else { axis };
    let biased = triangle_separation + SAT_DATA.2;
    let (kind, normal, clip_axis, clip_box_to_triangle, box_plane) =
        if biased > box_separation && biased > edge_separation {
            (
                SourceTriangleContactKind::VertexPlane,
                orient(triangle_normal),
                triangle_normal,
                true,
                most_opposing_box_plane(triangle_normal),
            )
        } else if box_separation > edge_separation {
            (SourceTriangleContactKind::PlaneVertex, orient(box_normal), box_normal, false, box_plane)
        } else if edge_separation != f64::MIN {
            let normal = orient(edge_normal);
            let box_plane = most_opposing_box_plane(normal);
            let triangle_dot = triangle_normal.dot(normal);
            let box_dot = BOX_NORMALS[box_plane].dot(normal);
            let clip_box_to_triangle = triangle_dot.abs() > box_dot.abs();
            let clip_dot = if clip_box_to_triangle { triangle_dot } else { box_dot };
            let clip_axis = if clip_dot < 0.0 { -normal } else { normal };
            (SourceTriangleContactKind::EdgeEdge, normal, clip_axis, clip_box_to_triangle, box_plane)
        } else {
            return None;
        };
    Some(SourceBoxTriangleSat {
        kind, normal, clip_axis, clip_box_to_triangle, box_plane, triangle_normal, triangle_center,
    })
}

pub(super) fn source_box_triangle_contacts(
    bounds_m: [DVec3; 2],
    triangle_m: [DVec3; 3],
    cull_m: f64,
) -> Vec<SourceTriangleContact> {
    let shape = SourceTriangleBox::from_bounds_cm(bounds_m.map(source_position));
    let triangle = triangle_m.map(source_position);
    let cull_cm = cull_m * 100.0;
    let Some(sat) = source_box_triangle_sat(shape, triangle, cull_cm) else {
        return Vec::new();
    };

    let face = SOURCE_BOX_FACE_VERTICES[sat.box_plane].map(|index| shape.vertex(index));
    let (reference, mut points): (&[DVec3], _) = if sat.clip_box_to_triangle {
        (&triangle, face.to_vec())
    } else {
        (&face, triangle.to_vec())
    };
    let mut previous = reference[reference.len() - 1];
    for &current in reference {
        if points.len() <= 1 {
            break;
        }
        let plane_normal = sat.clip_axis.cross(previous - current);
        if plane_normal.length_squared() > f64::from(SOURCE_SMALL_NUMBER) {
            points = clip_plane(&points, plane_normal, current.dot(plane_normal));
        }
        previous = current;
    }
    reduce_source_points_along(&mut points, sat.normal);

    let box_point = shape.plane_point(sat.box_plane);
    let (projection_normal, divisor) = if sat.kind == SourceTriangleContactKind::EdgeEdge {
        let normal = if sat.clip_box_to_triangle { sat.triangle_normal } else { BOX_NORMALS[sat.box_plane] };
        let divisor = sat.normal.dot(normal);
        debug_assert!(divisor.abs() > f64::from(SOURCE_SMALL_NUMBER), "Invalid source clipping projection");
        (normal, divisor)
    } else {
        (sat.normal, 1.0)
    };

    points.into_iter().filter_map(|point| {
        let (first_cm, second_cm, phi_cm) = if sat.clip_box_to_triangle {
            let phi_cm = (point - sat.triangle_center).dot(projection_normal) / divisor;
            (point, point - phi_cm * sat.normal, phi_cm)
        } else {
            let phi_cm = (box_point - point).dot(projection_normal) / divisor;
            (point + phi_cm * sat.normal, point, phi_cm)
        };
        (phi_cm < cull_cm).then_some(SourceTriangleContact { first_cm, second_cm, normal: sat.normal, phi_cm, kind: sat.kind })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::source_contact_polygon::source_direction;

    fn runtime_point(point_cm: DVec3) -> DVec3 {
        source_direction(point_cm) * 0.01
    }

    #[derive(serde::Deserialize)]
    struct NativeSat {
        bounds_cm: [[f64; 3]; 2],
        cull_cm: f64,
        faces: Vec<NativeFace>,
    }

    #[derive(serde::Deserialize)]
    struct NativeFace {
        index: usize,
        vertices_cm: [[f64; 3]; 3],
        contact_type: u8,
        normal: [f64; 3],
    }

    #[test]
    fn source_box_triangle_sat_matches_cart_face_bias_and_edge_eligibility() {
        let fixture: NativeSat = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b4b2b1/cart-sat-cases.json"
        )).unwrap();
        let shape = SourceTriangleBox::from_bounds_cm(fixture.bounds_cm.map(DVec3::from_array));
        assert_eq!(fixture.faces.len(), 2);
        for face in fixture.faces {
            let sat = source_box_triangle_sat(shape, face.vertices_cm.map(DVec3::from_array), fixture.cull_cm)
                .unwrap_or_else(|| panic!("Source rejected Cart face {}", face.index));
            let kind = match face.contact_type {
                1 => SourceTriangleContactKind::VertexPlane,
                3 => SourceTriangleContactKind::PlaneVertex,
                kind => panic!("Unexpected native Cart contact type {kind}"),
            };
            assert_eq!(sat.kind, kind, "Cart face {}", face.index);
            assert!(sat.normal.distance(DVec3::from_array(face.normal)) < 4e-7, "Cart face {}", face.index);
            assert_eq!(sat.box_plane, 2);
            assert_eq!(sat.clip_box_to_triangle, face.index == 988);
            let expected_axis = if face.index == 988 { sat.triangle_normal } else { DVec3::NEG_Z };
            assert_eq!(sat.clip_axis, expected_axis);
        }
    }

    #[test]
    fn source_box_triangle_sat_culls_backfaces_and_separation() {
        let shape = SourceTriangleBox::from_bounds_cm([DVec3::splat(-1.0), DVec3::ONE]);
        let [a, b, c] = [DVec3::new(-10.0, -10.0, -1.5), DVec3::new(10.0, -10.0, -1.5), DVec3::new(-10.0, 10.0, -1.5)];
        assert_eq!(source_box_triangle_sat(shape, [a, b, c], 3.0).unwrap().normal, DVec3::Z);
        assert!(source_box_triangle_sat(shape, [a, c, b], 3.0).is_none());
        assert!(source_box_triangle_sat(shape, [a, b, c].map(|point| point - 3.0 * DVec3::Z), 3.0).is_none());
    }

    #[test]
    fn source_box_sat_edges_keep_winding_and_axis_tie_order() {
        let shape = SourceTriangleBox::from_bounds_cm([DVec3::splat(-1.0), DVec3::ONE]);
        for [first, second, a, b] in SAT_DATA.0 {
            assert!(BOX_NORMALS[a].cross(BOX_NORMALS[b]).dot(shape.vertex(second) - shape.vertex(first)) > 0.0);
        }
        assert_eq!(most_opposing_box_plane(DVec3::ONE), 2);
        assert_eq!(most_opposing_box_plane(-DVec3::ONE), 5);
    }

    #[test]
    fn source_box_triangle_clipping_keeps_four_point_order_and_strict_cull() {
        let bounds = [DVec3::splat(-0.01), DVec3::splat(0.01)];
        let face = |height| [DVec3::new(-10.0, -10.0, height), DVec3::new(10.0, -10.0, height),
            DVec3::new(0.0, 10.0, height)].map(runtime_point);
        let contacts = source_box_triangle_contacts(bounds, face(-0.8), 0.03);
        let points = contacts.iter().map(|point| point.first_cm).collect::<Vec<_>>();
        assert_eq!(points, [DVec3::new(-1.0, -1.0, -1.0), DVec3::new(-1.0, 1.0, -1.0),
            DVec3::new(1.0, 1.0, -1.0), DVec3::new(1.0, -1.0, -1.0)]);
        assert!(contacts.iter().all(|point| (point.second_cm.z + 0.8).abs() < 1e-12));
        assert_eq!(source_box_triangle_contacts(bounds, face(-3.999), 0.03).len(), 4);
        assert!(source_box_triangle_contacts(bounds, face(-4.0), 0.03).is_empty());
    }

    #[test]
    fn source_box_triangle_clipping_reduces_six_vertices_to_four() {
        let bounds = [DVec3::splat(-0.01), DVec3::splat(0.01)];
        let triangle = [DVec3::new(-2.0, -0.5, -0.8), DVec3::new(2.0, -0.5, -0.8),
            DVec3::new(0.0, 2.0, -0.8)].map(runtime_point);
        let contacts = source_box_triangle_contacts(bounds, triangle, 0.03);
        assert_eq!(contacts.len(), 4);
        for (index, point) in contacts.iter().enumerate() {
            assert_eq!(point.kind, SourceTriangleContactKind::VertexPlane);
            assert!(point.first_cm.x.abs() <= 1.0 + 1e-12);
            assert!(point.first_cm.y >= -0.5 - 1e-12 && point.first_cm.y <= 1.0 + 1e-12);
            assert!((point.phi_cm + 0.2).abs() < 1e-12);
            assert!(contacts[..index].iter().all(|other| other.first_cm.distance(point.first_cm) > 1e-9));
        }
    }

    #[test]
    fn source_box_triangle_contacts_match_native_when_available() {
        let Ok(path) = std::env::var("ALS_BOX_TRIANGLE_NATIVE_REPORT") else { return; };
        let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let point = |value: &serde_json::Value| DVec3::new(value["x"].as_f64().unwrap(),
            value["y"].as_f64().unwrap(), value["z"].as_f64().unwrap());
        let samples = report["physics_contact_probe"]["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 4);
        for sample in samples {
            let geometry = &sample["geometry"];
            assert_eq!(geometry["kind"], "box");
            let bounds = [point(&geometry["minimum_cm"]), point(&geometry["maximum_cm"])].map(runtime_point);
            let input = &sample["input"];
            let cull_m = input["cull_distance_cm"].as_f64().unwrap() * 0.01;
            let groups = sample["triangle_contacts"].as_array().unwrap();
            let indices = input["triangle_indices"].as_array().unwrap();
            assert_eq!(groups.len(), indices.len());
            for (face, index) in groups.iter().zip(indices) {
                assert_eq!(&face["triangle_index"], index);
                let vertices = face["vertices_cm"].as_array().unwrap();
                assert_eq!(vertices.len(), 3);
                let triangle = std::array::from_fn(|index| runtime_point(point(&vertices[index])));
                let contacts = source_box_triangle_contacts(bounds, triangle, cull_m);
                let expected = face["points"].as_array().unwrap();
                let label = (&input["label"], index);
                if input["label"] == "cart_box_backfaces" {
                    assert!(expected.is_empty(), "{label:?}");
                } else if input["label"] == "cart_box_production" && matches!(index.as_u64(), Some(988 | 989)) {
                    assert!(!expected.is_empty(), "{label:?}");
                }
                assert_eq!(contacts.len(), expected.len(), "{label:?}");
                for (actual, expected) in contacts.iter().zip(expected) {
                    let kind = match actual.kind {
                        SourceTriangleContactKind::VertexPlane => 1,
                        SourceTriangleContactKind::EdgeEdge => 2,
                        SourceTriangleContactKind::PlaneVertex => 3,
                    };
                    assert_eq!(expected["contact_type"].as_i64(), Some(kind), "{label:?}");
                    assert!(actual.first_cm.distance(point(&expected["shape_point_0_cm"])) < 1e-7, "{label:?}");
                    assert!(actual.second_cm.distance(point(&expected["shape_point_1_cm"])) < 1e-7, "{label:?}");
                    assert!(actual.normal.distance(point(&expected["world_normal"])) < 1e-8, "{label:?}");
                    assert!((actual.phi_cm - expected["phi_cm"].as_f64().unwrap()).abs() < 1e-7, "{label:?}");
                }
            }
        }
    }
}
