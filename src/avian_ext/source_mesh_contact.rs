use std::collections::HashMap;

use bevy::math::DVec3;

use super::source_contact_polygon::{SOURCE_SMALL_NUMBER, source_position};
use super::source_mesh_query::SourceMeshTriangle;

const CONTACT_SETTINGS: (f64, f32, f32, f64, usize) = include!("../als/settings/mesh_contact.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceTriangleContactKind {
    VertexPlane,
    EdgeEdge,
    PlaneVertex,
}

pub(super) struct SourceTriangleContact {
    pub first_cm: DVec3,
    pub second_cm: DVec3,
    pub normal: DVec3,
    pub phi_cm: f64,
    pub kind: SourceTriangleContactKind,
}

pub(super) fn source_triangle_normal([a, b, c]: [DVec3; 3]) -> DVec3 {
    let cross = (b - a).cross(c - a);
    let squared = cross.length_squared();
    if squared < f64::from(SOURCE_SMALL_NUMBER) {
        DVec3::ZERO
    } else {
        cross * squared.sqrt().recip()
    }
}

type MeshEdge = [usize; 2];

fn mesh_edge(first: usize, second: usize) -> MeshEdge {
    [first.min(second), first.max(second)]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MeshContactFeature {
    Face,
    Edge(MeshEdge),
    Vertex(usize),
}

struct MeshContactTriangle {
    face_index: usize,
    vertex_indices: [usize; 3],
    vertices_cm: [DVec3; 3],
    normal: DVec3,
}

impl MeshContactTriangle {
    fn new(triangle: &SourceMeshTriangle) -> Self {
        let vertices_cm = triangle.vertices_m.map(source_position);
        Self {
            face_index: triangle.face_index,
            vertex_indices: triangle.vertex_indices,
            vertices_cm,
            normal: source_triangle_normal(vertices_cm),
        }
    }

    fn contact_feature(&self, point_cm: DVec3) -> MeshContactFeature {
        let weights = source_barycentric(point_cm, self.vertices_cm);
        let tolerance = CONTACT_SETTINGS.0;
        if let Some(vertex) = weights.iter().position(|&weight| (weight - 1.0).abs() <= tolerance) {
            MeshContactFeature::Vertex(self.vertex_indices[vertex])
        } else if let Some(opposite) = weights.iter().position(|weight| weight.abs() <= tolerance) {
            MeshContactFeature::Edge(mesh_edge(self.vertex_indices[(opposite + 1) % 3], self.vertex_indices[(opposite + 2) % 3]))
        } else {
            MeshContactFeature::Face
        }
    }
}

fn source_barycentric(point: DVec3, [a, b, c]: [DVec3; 3]) -> [f64; 3] {
    let ac = a - c;
    let bc = b - c;
    let pc = point - c;
    let aa = ac.dot(ac);
    let ab = ac.dot(bc);
    let bb = bc.dot(bc);
    let pa = ac.dot(pc);
    let pb = bc.dot(pc);
    let determinant = aa * bb - ab * ab;
    let small = f64::from(SOURCE_SMALL_NUMBER);
    let (first, second) = if determinant > small {
        let inverse = determinant.recip();
        ((bb * pa - ab * pb) * inverse, (aa * pb - ab * pa) * inverse)
    } else {
        let on_ac = aa > small && pa >= 0.0 && pa <= aa;
        let on_bc = bb > small && pb >= 0.0 && pb <= bb;
        if on_ac || (aa > small && !on_bc) {
            (pa / aa, 0.0)
        } else if bb > small {
            (0.0, pb / bb)
        } else {
            (1.0, 0.0)
        }
    };
    [first, second, 1.0 - first - second]
}

struct MeshContactTopology {
    triangles: Vec<MeshContactTriangle>,
    edges: HashMap<MeshEdge, (usize, Option<usize>)>,
}

impl MeshContactTopology {
    fn new(triangles: &[SourceMeshTriangle]) -> Self {
        let triangles: Vec<_> = triangles.iter().map(MeshContactTriangle::new).collect();
        let mut edges = HashMap::new();
        for (index, triangle) in triangles.iter().enumerate() {
            let [a, b, c] = triangle.vertex_indices;
            for edge in [mesh_edge(a, b), mesh_edge(b, c), mesh_edge(c, a)] {
                edges.entry(edge).and_modify(|(_, last)| *last = Some(index)).or_insert((index, None));
            }
        }
        Self { triangles, edges }
    }

    fn other_triangle(&self, triangle: usize, edge: MeshEdge) -> Option<usize> {
        let &(first, second) = self.edges.get(&edge)?;
        if first == triangle { second } else { Some(first) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::source_contact_polygon::source_direction;
    use crate::extensions::source_mesh_query::SourceMeshTopology;

    fn triangle(face_index: usize, vertex_indices: [usize; 3], vertices_cm: [DVec3; 3]) -> SourceMeshTriangle {
        SourceMeshTriangle { face_index, vertex_indices, vertices_m: vertices_cm.map(|point| source_direction(point) * 0.01) }
    }

    #[test]
    fn source_mesh_contact_adjacency_keeps_first_and_last_collected_faces() {
        let points = [DVec3::ZERO, DVec3::X, DVec3::Y];
        let triangles = [triangle(300, [4, 3, 9], points), triangle(2, [3, 4, 20], [DVec3::X, DVec3::ZERO, DVec3::NEG_Y]),
            triangle(120, [4, 3, 30], points), triangle(121, [14, 13, 39], points)];
        let topology = MeshContactTopology::new(&triangles);
        assert_eq!(topology.triangles.iter().map(|triangle| triangle.face_index).collect::<Vec<_>>(), [300, 2, 120, 121]);
        assert_eq!(topology.other_triangle(0, mesh_edge(4, 3)), Some(2));
        assert_eq!(topology.other_triangle(1, mesh_edge(4, 3)), Some(0));
        assert_eq!(topology.other_triangle(2, mesh_edge(4, 3)), Some(0));
        assert_eq!(topology.other_triangle(3, mesh_edge(13, 14)), None);
        assert_eq!(topology.triangles[0].normal, DVec3::Z);
    }

    #[test]
    fn source_mesh_contact_feature_preserves_vertex_priority_and_degenerate_lines() {
        let indices = [7, 10, 2];
        let normal = MeshContactTriangle::new(&triangle(0, indices, [DVec3::ZERO, 100.0 * DVec3::X, 100.0 * DVec3::Y]));
        assert_eq!(normal.contact_feature(DVec3::new(0.03, 0.02, 0.0)), MeshContactFeature::Vertex(7));
        assert_eq!(normal.contact_feature(DVec3::new(50.0, -0.08, 0.0)), MeshContactFeature::Edge([7, 10]));
        assert_eq!(normal.contact_feature(DVec3::new(50.0, 0.11, 0.0)), MeshContactFeature::Face);
        let line = MeshContactTriangle::new(&triangle(0, indices, [DVec3::ZERO, 5.0 * DVec3::X, 10.0 * DVec3::X]));
        assert_eq!(line.contact_feature(2.5 * DVec3::X), MeshContactFeature::Edge([2, 7]));
        let point = MeshContactTriangle::new(&triangle(0, indices, [DVec3::ZERO; 3]));
        assert_eq!(point.contact_feature(DVec3::Z), MeshContactFeature::Vertex(7));
    }

    #[test]
    fn source_mesh_contact_features_use_cooked_stair_edge_ids() {
        let meshes: HashMap<String, SourceMeshTopology> = serde_json::from_str(include_str!(
            "../../assets/ALSExtras/Environment/Meshes/static_trimesh_topology.runtime.json"
        )).unwrap();
        let stairs = &meshes["SM_Als_Stairs_02"];
        let triangles = [252, 253, 270, 271].map(|face_index| {
            let vertex_indices = stairs.triangles[face_index];
            SourceMeshTriangle { face_index, vertex_indices,
                vertices_m: vertex_indices.map(|index| DVec3::from_array(stairs.vertices_m[index])) }
        });
        let topology = MeshContactTopology::new(&triangles);
        assert_eq!(topology.other_triangle(0, [75, 76]), Some(2));
        let report: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d104b3b/restore.native-samples.json"
        )).unwrap();
        let sample = report["samples"].as_array().unwrap().iter()
            .find(|sample| sample["input"]["label"] == "SM_Als_Stairs_02_foot_shift_0.2").unwrap();
        let shared_points = sample["points"].as_array().unwrap().iter().filter(|point| {
            let value = &point["shape_point_1_cm"];
            let position = DVec3::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap(), value["z"].as_f64().unwrap());
            point["face_index"] == 252 && topology.triangles[0].contact_feature(position) == MeshContactFeature::Edge([75, 76])
        }).count();
        assert_eq!(shared_points, 2);
    }
}
