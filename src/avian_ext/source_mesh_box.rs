use bevy::math::DVec3;

use super::source_contact_polygon::SOURCE_SMALL_NUMBER;
use super::source_mesh_capsule::SourceTriangleContactKind;

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
    let cross = (triangle[1] - triangle[0]).cross(triangle[2] - triangle[0]);
    let squared = cross.length_squared();
    let triangle_normal = if squared < f64::from(SOURCE_SMALL_NUMBER) {
        DVec3::ZERO
    } else {
        cross * squared.sqrt().recip()
    };
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
