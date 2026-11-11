use std::collections::HashMap;

use bevy::math::DVec3;

use super::source_contact_polygon::{SOURCE_SMALL_NUMBER, source_position};
use super::source_mesh_query::SourceMeshTriangle;

const CONTACT_SETTINGS: (f64, f32, f32, f64, usize) = include!("../als/settings/mesh_contact.rs");
const BARYCENTRIC_TOLERANCE: f64 = CONTACT_SETTINGS.0;
const FACE_NORMAL_DOT_THRESHOLD: f32 = CONTACT_SETTINGS.1;
const EDGE_NORMAL_DOT_REJECT_TOLERANCE: f32 = CONTACT_SETTINGS.2;
const BACKFACE_CULL_TOLERANCE: f64 = CONTACT_SETTINGS.3;
const VERTEX_NORMAL_TOLERANCE_SQUARED: f64 = include!("../als/settings/mesh_contact_vertex.rs");

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
        let tolerance = BARYCENTRIC_TOLERANCE;
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

    fn is_shared_edge(&self, edge: MeshEdge) -> bool {
        self.edges.get(&edge).is_some_and(|(_, second)| second.is_some())
    }
}

struct MeshContactCandidate {
    point: SourceTriangleContact,
    face_index: usize,
    triangle: usize,
    feature: MeshContactFeature,
    normal_dot: f32,
    enabled: bool,
}

impl MeshContactCandidate {
    fn set_normal(&mut self, normal: DVec3) {
        self.point.normal = normal;
        self.point.first_cm = self.point.second_cm + self.point.phi_cm * normal;
    }
}

struct EdgeContactOwner {
    triangle: usize,
    points: [Option<usize>; 2],
    normal_dot: f32,
    face_contact: bool,
}

impl EdgeContactOwner {
    fn new(triangle: usize) -> Self {
        Self { triangle, points: [None; 2], normal_dot: 0.0, face_contact: false }
    }
}

struct VertexContactOwner {
    point: usize,
    face_contact: bool,
}

struct MeshContactAccumulator {
    topology: MeshContactTopology,
    contacts: Vec<MeshContactCandidate>,
    edges: HashMap<MeshEdge, EdgeContactOwner>,
    vertices: HashMap<usize, VertexContactOwner>,
    face_edges: Vec<usize>,
    capacity: usize,
}

impl MeshContactAccumulator {
    fn new(triangles: &[SourceMeshTriangle], source_capacity: usize) -> Self {
        let capacity = source_capacity.min(triangles.len().saturating_mul(4));
        Self {
            topology: MeshContactTopology::new(triangles),
            contacts: Vec::with_capacity(capacity),
            edges: HashMap::new(),
            vertices: HashMap::new(),
            face_edges: vec![0; triangles.len()],
            capacity,
        }
    }

    fn add_triangle_contacts(&mut self, triangle: usize, points: Vec<SourceTriangleContact>) {
        let geometry = &self.topology.triangles[triangle];
        for point in points {
            let index = self.contacts.len();
            if index == self.capacity {
                break;
            }
            let feature = geometry.contact_feature(point.second_cm);
            let normal_dot = point.normal.dot(geometry.normal) as f32;
            let face_contact = normal_dot > FACE_NORMAL_DOT_THRESHOLD;
            let mut candidate = MeshContactCandidate {
                point, face_index: geometry.face_index, triangle, feature, normal_dot, enabled: true,
            };
            match feature {
                MeshContactFeature::Edge(edge) => {
                    let owner = self.edges.entry(edge).or_insert_with(|| EdgeContactOwner::new(triangle));
                    if owner.triangle != triangle {
                        if owner.face_contact || (!face_contact && normal_dot < owner.normal_dot) {
                            continue;
                        }
                        for previous in owner.points.into_iter().flatten() {
                            self.contacts[previous].enabled = false;
                        }
                        *owner = EdgeContactOwner::new(triangle);
                    }
                    let slot = if owner.points[0].is_some() { 1 } else { 0 };
                    owner.points[slot] = Some(index);
                    owner.normal_dot = owner.normal_dot.max(normal_dot);
                    owner.face_contact |= face_contact;
                    if face_contact {
                        self.face_edges[triangle] += 1;
                        if let Some(other) = self.topology.other_triangle(triangle, edge) {
                            self.face_edges[other] += 1;
                        }
                    }
                }
                MeshContactFeature::Vertex(vertex) => {
                    if let Some(owner) = self.vertices.get_mut(&vertex) {
                        let existing = &mut self.contacts[owner.point];
                        if !owner.face_contact && normal_dot > existing.normal_dot {
                            candidate.enabled = existing.enabled;
                            *existing = candidate;
                            owner.face_contact = face_contact;
                        }
                        continue;
                    }
                    self.vertices.insert(vertex, VertexContactOwner { point: index, face_contact });
                }
                MeshContactFeature::Face => {}
            }
            self.contacts.push(candidate);
        }
    }

    fn face_contact_count(&self, triangle: usize) -> usize {
        let vertices = &self.topology.triangles[triangle].vertex_indices;
        self.face_edges[triangle] + vertices.iter().filter(|vertex| {
            self.vertices.get(*vertex).is_some_and(|owner| owner.face_contact)
        }).count()
    }

    fn correct_contact_normals(&mut self) {
        for contact in &mut self.contacts {
            if !contact.enabled {
                continue;
            }
            if f64::from(contact.normal_dot) < -BACKFACE_CULL_TOLERANCE {
                contact.enabled = false;
                continue;
            }
            if contact.normal_dot < FACE_NORMAL_DOT_THRESHOLD {
                match contact.feature {
                    MeshContactFeature::Edge(edge) => correct_mesh_edge_normal(&self.topology, contact, edge),
                    MeshContactFeature::Vertex(vertex) => correct_mesh_vertex_normal(&self.topology, contact, vertex),
                    MeshContactFeature::Face => {}
                }
            }
        }
    }
}

fn correct_mesh_edge_normal(topology: &MeshContactTopology, contact: &mut MeshContactCandidate, edge: MeshEdge) {
    let triangle = contact.triangle;
    let normal = topology.triangles[triangle].normal;
    let Some(other) = topology.other_triangle(triangle, edge) else {
        contact.set_normal(normal);
        return;
    };

    let other_normal = topology.triangles[other].normal;
    let minimum_dot = f64::from(other_normal.dot(normal) as f32);
    let contact_dot = f64::from(contact.normal_dot);
    if contact_dot >= minimum_dot {
        return;
    }
    if minimum_dot - contact_dot > f64::from(EDGE_NORMAL_DOT_REJECT_TOLERANCE) {
        contact.enabled = false;
        return;
    }

    let other_dot = contact.point.normal.dot(other_normal);
    let (corrected, corrected_triangle, dot) = if contact_dot >= other_dot {
        (normal, triangle, contact_dot)
    } else {
        (other_normal, other, other_dot)
    };
    let oriented = if dot > -f64::from(SOURCE_SMALL_NUMBER) { corrected } else { -corrected };
    contact.set_normal(oriented);
    contact.triangle = corrected_triangle;
}

fn correct_mesh_vertex_normal(topology: &MeshContactTopology, contact: &mut MeshContactCandidate, vertex: usize) {
    let original = &topology.triangles[contact.triangle];
    let Some(local) = original.vertex_indices.iter().position(|&id| id == vertex) else {
        return;
    };
    let position = original.vertices_cm[local];
    for (index, triangle) in topology.triangles.iter().enumerate() {
        let Some(local) = triangle.vertex_indices.iter().position(|&id| id == vertex) else {
            continue;
        };
        let next = (local + 1) % 3;
        let previous = (local + 2) % 3;
        if !topology.is_shared_edge(mesh_edge(vertex, triangle.vertex_indices[next]))
            || !topology.is_shared_edge(mesh_edge(vertex, triangle.vertex_indices[previous]))
        {
            contact.set_normal(original.normal);
            return;
        }

        let edges = [triangle.vertices_cm[next] - position, position - triangle.vertices_cm[previous]];
        let in_prism = edges.into_iter().all(|edge| {
            let sign = contact.point.normal.dot(triangle.normal.cross(edge));
            sign * sign.abs() >= -VERTEX_NORMAL_TOLERANCE_SQUARED * edge.length_squared()
        });
        if in_prism {
            contact.set_normal(triangle.normal);
            contact.triangle = index;
            return;
        }
    }
}

fn generate_mesh_contacts(
    triangles: &[SourceMeshTriangle],
    source_capacity: usize,
    mut generate: impl FnMut(&SourceMeshTriangle) -> Vec<SourceTriangleContact>,
) -> MeshContactAccumulator {
    let mut contacts = MeshContactAccumulator::new(triangles, source_capacity);
    let mut visited = vec![false; triangles.len()];
    for (index, triangle) in triangles.iter().enumerate() {
        if contacts.face_contact_count(index) == 0 {
            contacts.add_triangle_contacts(index, generate(triangle));
            visited[index] = true;
        }
    }
    for (index, triangle) in triangles.iter().enumerate() {
        if !visited[index] && contacts.face_contact_count(index) < 3 {
            contacts.add_triangle_contacts(index, generate(triangle));
        }
    }
    contacts
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

    fn adjacent_triangles() -> [SourceMeshTriangle; 2] {
        [triangle(30, [0, 1, 2], [DVec3::ZERO, 10.0 * DVec3::X, 10.0 * DVec3::Y]),
            triangle(40, [1, 0, 3], [10.0 * DVec3::X, DVec3::ZERO, -10.0 * DVec3::Y])]
    }

    fn contact(second_cm: DVec3, normal: DVec3) -> SourceTriangleContact {
        SourceTriangleContact {
            first_cm: second_cm - normal,
            second_cm,
            normal,
            phi_cm: -1.0,
            kind: SourceTriangleContactKind::EdgeEdge,
        }
    }

    #[test]
    fn source_mesh_edge_owner_replaces_old_slots_and_keeps_first_face_owner() {
        let triangles = adjacent_triangles();
        let mut manifold = MeshContactAccumulator::new(&triangles, 8);
        let aligned = DVec3::new(0.6, 0.0, 0.8);
        manifold.add_triangle_contacts(0, vec![contact(2.5 * DVec3::X, aligned), contact(7.5 * DVec3::X, aligned)]);
        manifold.add_triangle_contacts(1, vec![contact(5.0 * DVec3::X, DVec3::new(0.8, 0.0, 0.6))]);
        assert_eq!(manifold.contacts.len(), 2);
        assert!(manifold.contacts.iter().all(|point| point.enabled));

        manifold.add_triangle_contacts(1, vec![contact(5.0 * DVec3::X, DVec3::Z)]);
        manifold.add_triangle_contacts(0, vec![contact(5.0 * DVec3::X, DVec3::Z)]);
        assert_eq!(manifold.contacts.len(), 3);
        assert!(manifold.contacts[..2].iter().all(|point| !point.enabled));
        assert!(manifold.contacts[2].enabled);
        assert_eq!(manifold.contacts[2].face_index, 40);
        assert_eq!(manifold.contacts[2].feature, MeshContactFeature::Edge([0, 1]));
        assert_eq!(manifold.face_edges, [1, 1]);
    }

    #[test]
    fn source_mesh_vertex_replacement_preserves_slot_and_stops_at_capacity() {
        let triangles = adjacent_triangles();
        let mut manifold = MeshContactAccumulator::new(&triangles, 8);
        let vertex = DVec3::ZERO;
        let inside = DVec3::new(2.0, 2.0, 0.0);
        let tilted = DVec3::new(0.6, 0.0, 0.8);
        manifold.add_triangle_contacts(0, vec![contact(vertex, tilted), contact(inside, DVec3::Z)]);
        manifold.add_triangle_contacts(1, vec![contact(vertex, DVec3::Z)]);
        assert_eq!(manifold.contacts.len(), 2);
        assert_eq!(manifold.contacts[0].face_index, 40);
        assert_eq!(manifold.contacts[0].triangle, 1);
        assert_eq!(manifold.contacts[1].point.second_cm, inside);

        let mut full = MeshContactAccumulator::new(&triangles, 1);
        full.add_triangle_contacts(0, vec![contact(vertex, tilted)]);
        full.add_triangle_contacts(1, vec![contact(vertex, DVec3::Z)]);
        assert_eq!(full.contacts.len(), 1);
        assert_eq!(full.contacts[0].face_index, 30);
        assert_eq!(full.contacts[0].point.normal, tilted);
    }

    #[test]
    fn source_mesh_two_pass_defers_neighbors_and_skips_three_face_contacts() {
        let [first, adjacent] = adjacent_triangles();
        let isolated = triangle(50, [4, 5, 6], [DVec3::ZERO, 10.0 * DVec3::X, 10.0 * DVec3::Y]);
        let triangles = [first, adjacent, isolated];
        for (vertex_contact, expected) in [(false, vec![30, 50, 40]), (true, vec![30, 50])] {
            let mut visits = Vec::new();
            let manifold = generate_mesh_contacts(&triangles, 12, |triangle| {
                visits.push(triangle.face_index);
                if triangle.face_index != 30 {
                    return Vec::new();
                }
                let mut points = vec![contact(2.5 * DVec3::X, DVec3::Z), contact(7.5 * DVec3::X, DVec3::Z)];
                if vertex_contact {
                    points.push(contact(DVec3::ZERO, DVec3::Z));
                }
                points
            });
            assert_eq!(visits, expected);
            assert_eq!(manifold.face_contact_count(1), if vertex_contact { 3 } else { 2 });
        }
    }

    #[test]
    fn source_mesh_second_pass_checks_contacts_added_by_earlier_neighbors() {
        let [first, adjacent] = adjacent_triangles();
        let last = triangle(50, [1, 3, 4], [10.0 * DVec3::X, -10.0 * DVec3::Y, DVec3::new(10.0, -10.0, 0.0)]);
        let mut visits = Vec::new();
        let manifold = generate_mesh_contacts(&[first, adjacent, last], 12, |triangle| {
            visits.push(triangle.face_index);
            match triangle.face_index {
                30 => vec![contact(10.0 * DVec3::X, DVec3::Z)],
                40 => vec![contact(DVec3::new(7.5, -2.5, 0.0), DVec3::Z), contact(DVec3::new(2.5, -7.5, 0.0), DVec3::Z)],
                _ => Vec::new(),
            }
        });
        assert_eq!(visits, [30, 40]);
        assert_eq!(manifold.face_contact_count(2), 3);
    }

    #[test]
    fn source_mesh_pruning_rejects_backfaces_and_outside_coplanar_normals() {
        let triangles = adjacent_triangles();
        let mut manifold = MeshContactAccumulator::new(&triangles, 8);
        let tilted = DVec3::new(0.6, 0.0, 0.8);
        manifold.add_triangle_contacts(0, vec![
            contact(2.5 * DVec3::X, DVec3::X),
            contact(7.5 * DVec3::X, tilted),
            contact(DVec3::new(2.0, 2.0, 0.0), DVec3::NEG_Z),
            contact(5.0 * DVec3::Y, tilted),
        ]);
        manifold.correct_contact_normals();
        assert_eq!(manifold.contacts.iter().map(|point| point.enabled).collect::<Vec<_>>(), [false, true, false, true]);
        for index in [1, 3] {
            let point = &manifold.contacts[index].point;
            assert_eq!(point.normal, DVec3::Z);
            assert_eq!(point.first_cm, point.second_cm - DVec3::Z);
            assert_eq!(point.phi_cm, -1.0);
        }
    }

    #[test]
    fn source_mesh_shared_edge_can_use_neighbor_normal_without_changing_face_identity() {
        let [first, _] = adjacent_triangles();
        let neighbor = triangle(40, [1, 0, 3], [10.0 * DVec3::X, DVec3::ZERO, DVec3::new(0.0, -8.0, 6.0)]);
        let mut manifold = MeshContactAccumulator::new(&[first, neighbor], 8);
        let second = 5.0 * DVec3::X;
        manifold.add_triangle_contacts(0, vec![contact(second, DVec3::new(0.0, 0.8, 0.6))]);
        manifold.correct_contact_normals();
        let corrected = &manifold.contacts[0];
        let expected = DVec3::new(0.0, 0.6, 0.8);
        assert!(corrected.enabled);
        assert_eq!(corrected.triangle, 1);
        assert_eq!(corrected.face_index, 30);
        assert_eq!(corrected.point.kind, SourceTriangleContactKind::EdgeEdge);
        assert!(corrected.point.normal.distance(expected) < 1e-12);
        assert!(corrected.point.first_cm.distance(second - expected) < 1e-12);
        assert_eq!(corrected.point.phi_cm, -1.0);
    }

    #[test]
    fn source_mesh_edge_normal_matches_native_stair_contact() {
        let case: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b4b3d/edge-normal-case.json"
        )).unwrap();
        let vector = |value: &serde_json::Value| DVec3::new(value[0].as_f64().unwrap(),
            value[1].as_f64().unwrap(), value[2].as_f64().unwrap());
        let triangles = case["triangles"].as_array().unwrap().iter().map(|face| {
            let ids = std::array::from_fn(|index| face["vertex_indices"][index].as_u64().unwrap() as usize);
            let points = std::array::from_fn(|index| vector(&face["vertices_cm"][index]));
            triangle(face["face_index"].as_u64().unwrap() as usize, ids, points)
        }).collect::<Vec<_>>();
        let index = triangles.iter().position(|triangle| triangle.face_index == 271).unwrap();
        let raw = &case["raw"];
        let mut manifold = MeshContactAccumulator::new(&triangles, 8);
        assert_eq!(raw["contact_type"], 2);
        manifold.add_triangle_contacts(index, vec![SourceTriangleContact {
            first_cm: vector(&raw["first_cm"]), second_cm: vector(&raw["second_cm"]),
            normal: vector(&raw["normal"]), phi_cm: raw["phi_cm"].as_f64().unwrap(),
            kind: SourceTriangleContactKind::EdgeEdge,
        }]);
        assert_eq!(manifold.contacts[0].feature, MeshContactFeature::Edge([76, 78]));
        manifold.correct_contact_normals();
        let fixed = &manifold.contacts[0];
        let expected = &case["expected"];
        assert!(fixed.enabled);
        assert_eq!(fixed.face_index as u64, expected["face_index"].as_u64().unwrap());
        assert!(fixed.point.normal.distance(vector(&expected["normal"])) < 1e-9);
        assert!(fixed.point.first_cm.distance(vector(&expected["first_cm"])) < 1e-5);
        assert!((fixed.point.phi_cm - expected["phi_cm"].as_f64().unwrap()).abs() < 1e-5);
    }

    #[test]
    fn source_mesh_boundary_vertex_uses_original_face_normal() {
        let [first, _] = adjacent_triangles();
        let tilted = triangle(40, [1, 0, 3], [10.0 * DVec3::X, DVec3::ZERO, DVec3::new(0.0, -8.0, 6.0)]);
        let mut manifold = MeshContactAccumulator::new(&[first, tilted], 8);
        manifold.add_triangle_contacts(1, vec![contact(DVec3::ZERO, DVec3::Z)]);
        assert_eq!(manifold.contacts[0].feature, MeshContactFeature::Vertex(0));
        manifold.correct_contact_normals();
        let fixed = &manifold.contacts[0];
        let original = DVec3::new(0.0, 0.6, 0.8);
        assert_eq!((fixed.triangle, fixed.face_index), (1, 40));
        assert!(fixed.point.normal.distance(original) < 1e-12);
        assert!(fixed.point.first_cm.distance(-original) < 1e-12);
        assert_eq!(fixed.point.phi_cm, -1.0);
    }

    #[test]
    fn source_mesh_closed_vertex_fan_uses_first_matching_prism() {
        let [a, b, c, d, e] = [DVec3::ZERO, 10.0 * DVec3::X, 10.0 * DVec3::Y,
            DVec3::new(-10.0, 0.0, 6.0), -10.0 * DVec3::Y];
        let triangles = [triangle(10, [0, 1, 2], [a, b, c]), triangle(20, [0, 2, 3], [a, c, d]),
            triangle(30, [0, 3, 4], [a, d, e]), triangle(40, [0, 4, 1], [a, e, b])];
        let mut manifold = MeshContactAccumulator::new(&triangles, 16);
        manifold.add_triangle_contacts(0, vec![contact(a, DVec3::new(-0.6, 0.4, 0.48_f64.sqrt()))]);
        manifold.correct_contact_normals();
        let fixed = &manifold.contacts[0];
        let expected = DVec3::new(0.6, 0.0, 1.0).normalize();
        assert!(fixed.enabled);
        assert_eq!((fixed.triangle, fixed.face_index), (1, 10));
        assert!(fixed.point.normal.distance(expected) < 1e-12);
        assert!(fixed.point.first_cm.distance(-expected) < 1e-12);
        assert_eq!(fixed.point.phi_cm, -1.0);
    }
}
