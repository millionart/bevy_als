use bevy::math::DVec3;

use super::source_gjk_simplex::{SourceGjkSimplex, SourceGjkVertex};

#[derive(Clone, Copy)]
pub(crate) struct SourceEpaSettings {
    pub face_area_m4: f64,
    pub initial_edge_m2: f64,
    pub visibility_m: f64,
    pub witness_m: f64,
    pub max_iterations: usize,
    pub max_flood: usize,
    pub relative_epsilon: f64,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SourceEpaVertex {
    pub first_cm: DVec3,
    pub second_cm: DVec3,
}

impl SourceEpaVertex {
    fn difference(self) -> DVec3 {
        self.first_cm - self.second_cm
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SourceEpaFace {
    pub indices: [usize; 3],
    pub normal: DVec3,
    pub distance_cm: f64,
    pub adjacent_faces: [usize; 3],
    pub adjacent_edges: [usize; 3],
    pub obsolete: bool,
}

impl SourceEpaFace {
    pub(crate) fn new(
        vertices: &[SourceEpaVertex],
        indices: [usize; 3],
        adjacent_faces: [usize; 3],
        adjacent_edges: [usize; 3],
        settings: SourceEpaSettings,
    ) -> Option<Self> {
        let [a, b, c] = indices.map(|index| vertices[index].difference());
        let normal = (b - a).cross(c - a);
        let squared = normal.length_squared();
        if squared < settings.face_area_m4 * 100.0_f64.powi(4) { return None; }
        let normal = normal * squared.sqrt().recip();
        Some(Self { indices, normal, distance_cm: normal.dot(a), adjacent_faces, adjacent_edges, obsolete: false })
    }

    pub(crate) fn distance_to_plane(self, point_cm: DVec3) -> f64 {
        self.normal.dot(point_cm) - self.distance_cm
    }

    pub(crate) fn origin_projected_inside(self, vertices: &[SourceEpaVertex], epsilon_cm2: f64) -> bool {
        let projected = self.distance_cm * self.normal;
        let [a, b, c] = self.indices.map(|index| vertices[index].difference() - projected);
        let ac = a.cross(c).dot(self.normal);
        let cb = c.cross(b).dot(self.normal);
        if (ac < -epsilon_cm2 && cb > epsilon_cm2) || (ac > epsilon_cm2 && cb < -epsilon_cm2) { return false; }
        let ba = b.cross(a).dot(self.normal);
        !((ac < -epsilon_cm2 && ba > epsilon_cm2) || (ac > epsilon_cm2 && ba < -epsilon_cm2))
    }
}

pub(crate) fn flip_source_epa_face(faces: &mut [SourceEpaFace], index: usize) {
    let current = faces[index];
    for (old, new) in [(1, 2), (2, 1)] {
        let neighbor = current.adjacent_faces[old];
        let edge = current.adjacent_edges[old];
        debug_assert_eq!(faces[neighbor].adjacent_edges[edge], old);
        faces[neighbor].adjacent_edges[edge] = new;
    }
    let face = &mut faces[index];
    face.indices.swap(0, 1);
    face.adjacent_faces.swap(1, 2);
    face.adjacent_edges.swap(1, 2);
    face.normal = -face.normal;
    face.distance_cm = -face.distance_cm;
}

fn add_source_epa_farther_point<A, B>(
    vertices: &mut Vec<SourceEpaVertex>,
    direction: DVec3,
    first_support: &mut A,
    second_support: &mut B,
    last_vertices: &mut [usize; 2],
) where
    A: FnMut(DVec3) -> (DVec3, usize),
    B: FnMut(DVec3) -> (DVec3, usize),
{
    let (first0, _) = first_support(direction);
    let (first1, first_index1) = first_support(-direction);
    last_vertices[0] = first_index1;
    let (second0, _) = second_support(-direction);
    let (second1, second_index1) = second_support(direction);
    last_vertices[1] = second_index1;
    let distance0 = (first0 - second0).dot(direction);
    let distance1 = (first1 - second1).dot(-direction);
    let (first_cm, second_cm) = if distance1 >= distance0 { (first1, second1) } else { (first0, second0) };
    vertices.push(SourceEpaVertex { first_cm, second_cm });
}

fn source_epa_tetrahedron(vertices: &[SourceEpaVertex], settings: SourceEpaSettings) -> Option<Vec<SourceEpaFace>> {
    let layouts = [([1, 2, 3], [3, 1, 2], [1, 1, 1]), ([0, 3, 2], [2, 0, 3], [2, 1, 0]),
        ([0, 1, 3], [3, 0, 1], [2, 2, 0]), ([0, 2, 1], [1, 0, 2], [2, 0, 0])];
    layouts.into_iter().map(|(indices, adjacent_faces, adjacent_edges)| {
        SourceEpaFace::new(vertices, indices, adjacent_faces, adjacent_edges, settings)
    }).collect()
}

pub(crate) fn initialize_source_epa<A, B>(
    vertices: &mut Vec<SourceEpaVertex>,
    settings: SourceEpaSettings,
    mut first_support: A,
    mut second_support: B,
    last_vertices: &mut [usize; 2],
) -> Result<Vec<SourceEpaFace>, DVec3>
where
    A: FnMut(DVec3) -> (DVec3, usize),
    B: FnMut(DVec3) -> (DVec3, usize),
{
    let initial = vertices.len();
    if initial == 1 {
        add_source_epa_farther_point(vertices, DVec3::Z, &mut first_support, &mut second_support, last_vertices);
    }
    let mut faces = match initial {
        1 | 2 => {
            let direction = vertices[1].difference() - vertices[0].difference();
            if direction.length_squared() <= settings.initial_edge_m2 * 100.0_f64.powi(2) { return Err(DVec3::Z); }
            let absolute = direction.abs();
            let axis = if absolute.x <= absolute.y && absolute.x <= absolute.z { DVec3::X }
                else if absolute.y <= absolute.z { DVec3::Y } else { DVec3::Z };
            let orthogonal = direction.cross(axis);
            let second_orthogonal = orthogonal.cross(direction);
            add_source_epa_farther_point(vertices, orthogonal, &mut first_support, &mut second_support, last_vertices);
            add_source_epa_farther_point(vertices, second_orthogonal, &mut first_support, &mut second_support, last_vertices);
            source_epa_tetrahedron(vertices, settings).ok_or_else(|| orthogonal.normalize())?
        }
        3 => {
            let base = SourceEpaFace::new(vertices, [0, 2, 1], [1, 0, 2], [2, 0, 0], settings).ok_or(DVec3::Z)?;
            add_source_epa_farther_point(vertices, base.normal, &mut first_support, &mut second_support, last_vertices);
            source_epa_tetrahedron(vertices, settings).ok_or(base.normal)?
        }
        4 => source_epa_tetrahedron(vertices, settings).ok_or(DVec3::Z)?,
        _ => return Err(DVec3::Z),
    };
    let mut maximum = 0.0_f64;
    for face in &faces {
        if face.distance_cm.abs() > maximum.abs() { maximum = face.distance_cm; }
    }
    if maximum < 0.0 {
        for index in 0..faces.len() { flip_source_epa_face(&mut faces, index); }
    }
    Ok(faces)
}

#[derive(Clone, Copy)]
pub(crate) struct SourceEpaWitness {
    pub penetration_cm: f64,
    pub normal: DVec3,
    pub first_cm: DVec3,
    pub second_cm: DVec3,
}

pub(crate) fn source_epa_witness(
    vertices: &[SourceEpaVertex],
    face: SourceEpaFace,
    settings: SourceEpaSettings,
) -> SourceEpaWitness {
    let mut simplex = SourceGjkSimplex { len: 3, ..Default::default() };
    for (index, vertex) in face.indices.into_iter().enumerate() {
        let vertex = vertices[vertex];
        simplex.vertices[index] = SourceGjkVertex {
            difference: vertex.difference(), first: vertex.first_cm, second: vertex.second_cm,
        };
    }
    let mut normal = simplex.project();
    let mut penetration = normal.length();
    if penetration < settings.witness_m * 100.0 {
        normal = face.normal;
        if face.distance_cm < 0.0 { penetration = -penetration; }
    } else {
        normal /= penetration;
        if face.distance_cm < 0.0 { normal = -normal; penetration = -penetration; }
    }
    let mut first = DVec3::ZERO;
    let mut second = DVec3::ZERO;
    for index in 0..simplex.len {
        first += simplex.vertices[index].first * simplex.weights[index];
        second += simplex.vertices[index].second * simplex.weights[index];
    }
    SourceEpaWitness { penetration_cm: penetration, normal, first_cm: first, second_cm: second }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourceEpaStatus {
    Ok,
    MaxIterations,
    BadInitialSimplex,
    Degenerate,
}

pub(crate) struct SourceEpaResult {
    pub status: SourceEpaStatus,
    pub witness: SourceEpaWitness,
}

fn source_epa_visibility_border(
    faces: &mut [SourceEpaFace],
    start: usize,
    point_cm: DVec3,
    settings: SourceEpaSettings,
) -> Vec<(usize, usize)> {
    let face = faces[start];
    let mut stack = (0..3).map(|edge| (face.adjacent_faces[edge], face.adjacent_edges[edge])).collect::<Vec<_>>();
    let mut border = Vec::new();
    for _ in 0..settings.max_flood {
        let Some((index, edge)) = stack.pop() else { break; };
        let face = &mut faces[index];
        if face.obsolete { continue; }
        if face.distance_to_plane(point_cm) <= settings.visibility_m * 100.0 {
            border.push((index, edge));
        } else {
            face.obsolete = true;
            for next in [(edge + 1) % 3, (edge + 2) % 3] {
                stack.push((face.adjacent_faces[next], face.adjacent_edges[next]));
            }
        }
    }
    border
}

pub(crate) fn run_source_epa<A, B>(
    vertices: &mut Vec<SourceEpaVertex>,
    settings: SourceEpaSettings,
    mut first_support: A,
    mut second_support: B,
    last_vertices: &mut [usize; 2],
) -> SourceEpaResult
where
    A: FnMut(DVec3) -> (DVec3, usize),
    B: FnMut(DVec3) -> (DVec3, usize),
{
    let mut faces = match initialize_source_epa(vertices, settings, &mut first_support, &mut second_support, last_vertices) {
        Ok(faces) => faces,
        Err(normal) => return SourceEpaResult { status: SourceEpaStatus::BadInitialSimplex,
            witness: SourceEpaWitness { penetration_cm: 0.0, normal, first_cm: DVec3::ZERO, second_cm: DVec3::ZERO } },
    };
    let mut queue = (0..faces.len()).filter(|&index| faces[index].distance_cm <= 0.0
        || faces[index].origin_projected_inside(vertices, 0.0)).collect::<Vec<_>>();
    let mut last = queue.last().map_or(faces[0], |&index| faces[index]);
    let mut upper = f64::MAX;
    let mut status = SourceEpaStatus::MaxIterations;
    let mut dirty = true;
    for _ in 0..settings.max_iterations {
        if queue.is_empty() { break; }
        if dirty {
            queue.sort_by(|left, right| faces[*right].distance_cm.partial_cmp(&faces[*left].distance_cm)
                .unwrap_or(std::cmp::Ordering::Equal));
            dirty = false;
        }
        let index = queue.pop().unwrap();
        let face = faces[index];
        if face.obsolete { continue; }
        if face.distance_cm > upper { status = SourceEpaStatus::Ok; break; }
        let (first_cm, first_index) = first_support(face.normal);
        last_vertices[0] = first_index;
        let (second_cm, second_index) = second_support(-face.normal);
        last_vertices[1] = second_index;
        let point = first_cm - second_cm;
        let support_distance = face.normal.dot(point);
        if support_distance < upper { upper = support_distance; }
        let lower = face.distance_cm;
        if upper <= (1.0 + settings.relative_epsilon) * lower.abs() {
            status = SourceEpaStatus::Ok;
            last = face;
            break;
        }
        if upper < lower { status = SourceEpaStatus::Ok; break; }
        last = face;
        let vertex_index = vertices.len();
        vertices.push(SourceEpaVertex { first_cm, second_cm });
        faces[index].obsolete = true;
        let border = source_epa_visibility_border(&mut faces, index, point, settings);
        if border.len() < 3 { status = SourceEpaStatus::Degenerate; break; }
        let first_new = faces.len();
        let mut terminated = false;
        for (offset, &(border_index, edge)) in border.iter().enumerate() {
            let current = first_new + offset;
            let next = if offset + 1 < border.len() { current + 1 } else { first_new };
            let previous = if offset > 0 { current - 1 } else { first_new + border.len() - 1 };
            let old = faces[border_index];
            let indices = [old.indices[(edge + 1) % 3], old.indices[edge], vertex_index];
            let new_face = SourceEpaFace::new(vertices, indices, [border_index, previous, next], [edge, 2, 1], settings);
            faces[border_index].adjacent_faces[edge] = current;
            faces[border_index].adjacent_edges[edge] = 0;
            let Some(new_face) = new_face else { status = SourceEpaStatus::Degenerate; terminated = true; break; };
            faces.push(new_face);
            if new_face.distance_cm <= upper && (new_face.distance_cm <= 0.0
                || new_face.origin_projected_inside(vertices, 0.0)) {
                queue.push(current);
                dirty = true;
            }
        }
        if terminated { break; }
    }
    SourceEpaResult { status, witness: source_epa_witness(vertices, last, settings) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> SourceEpaSettings {
        let (face_area_m4, initial_edge_m2, visibility_m, witness_m, max_iterations, max_flood, relative_epsilon) =
            include!("../als/settings/contact_epa.rs");
        SourceEpaSettings { face_area_m4, initial_edge_m2, visibility_m, witness_m, max_iterations, max_flood, relative_epsilon }
    }

    fn vertex(point: DVec3) -> SourceEpaVertex {
        SourceEpaVertex { first_cm: point, second_cm: DVec3::ZERO }
    }

    #[test]
    fn source_epa_face_and_witness_preserve_signed_geometry() {
        let vertices = [vertex(DVec3::X), vertex(DVec3::Y), vertex(DVec3::Z)];
        let face = SourceEpaFace::new(&vertices, [0, 1, 2], [0; 3], [0; 3], settings()).unwrap();
        let expected_normal = DVec3::ONE / 3.0_f64.sqrt();
        assert!(face.normal.distance(expected_normal) < 1e-12);
        assert!((face.distance_cm - 1.0 / 3.0_f64.sqrt()).abs() < 1e-12);
        assert!(face.origin_projected_inside(&vertices, 0.0));
        let witness = source_epa_witness(&vertices, face, settings());
        assert!(witness.normal.distance(expected_normal) < 1e-12);
        assert!((witness.penetration_cm - 1.0 / 3.0_f64.sqrt()).abs() < 1e-12);
        assert!(witness.first_cm.distance(DVec3::splat(1.0 / 3.0)) < 1e-12);
        assert_eq!(witness.second_cm, DVec3::ZERO);
        let reversed = SourceEpaFace::new(&vertices, [0, 2, 1], [0; 3], [0; 3], settings()).unwrap();
        let witness = source_epa_witness(&vertices, reversed, settings());
        assert!(witness.normal.distance(-expected_normal) < 1e-12);
        assert!(witness.penetration_cm < 0.0);
        let degenerate = [vertex(DVec3::ZERO), vertex(DVec3::X), vertex(DVec3::X * 2.0)];
        assert!(SourceEpaFace::new(&degenerate, [0, 1, 2], [0; 3], [0; 3], settings()).is_none());
        let touching = [vertex(DVec3::NEG_X), vertex(DVec3::X), vertex(DVec3::Y)];
        let touching_face = SourceEpaFace::new(&touching, [0, 1, 2], [0; 3], [0; 3], settings()).unwrap();
        let witness = source_epa_witness(&touching, touching_face, settings());
        assert_eq!(witness.penetration_cm, 0.0);
        assert_eq!(witness.normal, DVec3::Z);
        let outside = [vertex(DVec3::new(3.0, 0.0, 1.0)), vertex(DVec3::new(4.0, 0.0, 1.0)),
            vertex(DVec3::new(3.0, 1.0, 1.0))];
        let outside_face = SourceEpaFace::new(&outside, [0, 1, 2], [0; 3], [0; 3], settings()).unwrap();
        assert!(!outside_face.origin_projected_inside(&outside, 0.0));
    }

    #[test]
    fn source_epa_native_tetrahedron_faces_and_adjacency_survive_flip() {
        let sample: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3b3b3b1/epa.native-simplex.json")).unwrap();
        let evidence: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3b3b3b1/face-evidence.json")).unwrap();
        let vertices = sample["gjk_simplex"].as_array().unwrap().iter().map(|value| {
            let point = &value["world_delta_cm"];
            vertex(DVec3::new(point["x"].as_f64().unwrap(), point["y"].as_f64().unwrap(), point["z"].as_f64().unwrap()))
        }).collect::<Vec<_>>();
        let layouts = [([1, 2, 3], [3, 1, 2], [1, 1, 1]), ([0, 3, 2], [2, 0, 3], [2, 1, 0]),
            ([0, 1, 3], [3, 0, 1], [2, 2, 0]), ([0, 2, 1], [1, 0, 2], [2, 0, 0])];
        let mut faces = layouts.into_iter().map(|(indices, adjacent_faces, adjacent_edges)| {
            SourceEpaFace::new(&vertices, indices, adjacent_faces, adjacent_edges, settings()).unwrap()
        }).collect::<Vec<_>>();
        for (face, expected) in faces.iter().zip(evidence["initial_faces"].as_array().unwrap()) {
            let normal = &expected["normal"];
            let normal = DVec3::new(normal[0].as_f64().unwrap(), normal[1].as_f64().unwrap(), normal[2].as_f64().unwrap());
            assert!(face.normal.distance(normal) < 1e-10);
            assert!((face.distance_cm - expected["distance_cm"].as_f64().unwrap()).abs() < 1e-10);
        }
        for index in 0..faces.len() { flip_source_epa_face(&mut faces, index); }
        let mut native_vertices = vertices.clone();
        let mut last = [99, 99];
        let initialized = initialize_source_epa(&mut native_vertices, settings(),
            |_| panic!("A four-point simplex must not request support"),
            |_| panic!("A four-point simplex must not request support"), &mut last).unwrap();
        assert_eq!(last, [99, 99]);
        for (expected, actual) in faces.iter().zip(&initialized) {
            assert_eq!(expected.indices, actual.indices);
            assert_eq!(expected.normal, actual.normal);
            assert_eq!(expected.distance_cm, actual.distance_cm);
        }
        for (index, face) in faces.iter().enumerate() {
            for edge in 0..3 {
                let neighbor = faces[face.adjacent_faces[edge]];
                let opposite = face.adjacent_edges[edge];
                assert_eq!(neighbor.adjacent_faces[opposite], index);
                assert_eq!(neighbor.adjacent_edges[opposite], edge);
            }
        }
    }

    #[test]
    fn source_epa_initialization_preserves_support_call_order() {
        let shape = [DVec3::new(2.0, 0.0, 0.0), DVec3::new(-1.0, 1.0, 0.0),
            DVec3::new(-1.0, -1.0, 1.0), DVec3::new(-1.0, -1.0, -1.0)];
        let support = |direction: DVec3| {
            let mut index = 0;
            let mut best = f64::MIN;
            for (candidate, point) in shape.iter().enumerate() {
                let dot = point.dot(direction);
                if dot > best { best = dot; index = candidate; }
            }
            (shape[index], index)
        };
        let cube = |direction: DVec3| {
            let mut point = DVec3::splat(-1.0);
            let mut index = 0;
            for axis in 0..3 {
                if direction[axis] >= 0.0 { point[axis] = 1.0; index |= 1 << axis; }
            }
            (point, index)
        };
        let mut single = vec![vertex(DVec3::ZERO)];
        let mut single_last = [99, 99];
        assert_eq!(initialize_source_epa(&mut single, settings(), cube,
            |_| (DVec3::ZERO, 9), &mut single_last).unwrap().len(), 4);
        assert_eq!(single.len(), 4);
        assert_eq!(single_last, [2, 9]);
        for initial in [2, 3, 4] {
            let input = [shape[3], shape[0], shape[2], shape[1]];
            let mut vertices = input[..initial].iter().copied().map(vertex).collect::<Vec<_>>();
            let mut last = [99, 99];
            let faces = initialize_source_epa(&mut vertices, settings(), support,
                |_| (DVec3::ZERO, 9), &mut last).unwrap();
            assert_eq!(vertices.len(), 4);
            assert_eq!(faces.len(), 4);
            if initial == 2 {
                assert_eq!(vertices[3].first_cm, shape[1]);
                assert_eq!(last, [2, 9]);
            }
            if initial == 4 { assert_eq!(last, [99, 99]); }
            for (index, face) in faces.iter().enumerate() {
                for edge in 0..3 {
                    let neighbor = faces[face.adjacent_faces[edge]];
                    let opposite = face.adjacent_edges[edge];
                    assert_eq!(neighbor.adjacent_faces[opposite], index);
                    assert_eq!(neighbor.adjacent_edges[opposite], edge);
                }
            }
        }
        let mut short = vec![vertex(DVec3::ZERO), vertex(DVec3::X * 0.009)];
        let mut last = [99, 99];
        let error = initialize_source_epa(&mut short, settings(), support,
            |_| (DVec3::ZERO, 9), &mut last).err().unwrap();
        assert_eq!(error, DVec3::Z);
        assert_eq!(last, [99, 99]);
    }

    #[test]
    fn source_epa_expands_a_cube_to_its_nearest_surface() {
        let support = |direction: DVec3| {
            let mut point = DVec3::splat(-1.0);
            let mut index = 0;
            for axis in 0..3 {
                if direction[axis] >= 0.0 { point[axis] = 1.0; index |= 1 << axis; }
            }
            (point, index)
        };
        let mut vertices = [DVec3::ONE, DVec3::new(-1.0, -1.0, 1.0),
            DVec3::new(-1.0, 1.0, -1.0), DVec3::new(1.0, -1.0, -1.0)]
            .into_iter().map(vertex).collect::<Vec<_>>();
        let mut last = [99, 99];
        let result = run_source_epa(&mut vertices, settings(), support, |_| (DVec3::ZERO, 9), &mut last);
        assert_eq!(result.status, SourceEpaStatus::Ok);
        assert!((result.witness.penetration_cm - 1.0).abs() < 1e-10);
        assert!((result.witness.normal.length() - 1.0).abs() < 1e-10);
        assert!(result.witness.first_cm.distance(result.witness.normal) < 1e-10);
        assert_eq!(result.witness.second_cm, DVec3::ZERO);
        assert_eq!(last[1], 9);
    }

    #[test]
    fn source_epa_preserves_limit_and_degenerate_status() {
        let support = |direction: DVec3| {
            let point = DVec3::new(if direction.x < 0.0 { -1.0 } else { 1.0 },
                if direction.y < 0.0 { -1.0 } else { 1.0 }, if direction.z < 0.0 { -1.0 } else { 1.0 });
            (point, 0)
        };
        let initial = [DVec3::ONE, DVec3::new(-1.0, -1.0, 1.0),
            DVec3::new(-1.0, 1.0, -1.0), DVec3::new(1.0, -1.0, -1.0)];
        let mut limited = settings();
        limited.max_iterations = 0;
        let mut vertices = initial.into_iter().map(vertex).collect::<Vec<_>>();
        let result = run_source_epa(&mut vertices, limited, support, |_| (DVec3::ZERO, 0), &mut [0, 0]);
        assert_eq!(result.status, SourceEpaStatus::MaxIterations);
        let mut stalled = settings();
        stalled.max_flood = 0;
        let mut vertices = initial.into_iter().map(vertex).collect::<Vec<_>>();
        let result = run_source_epa(&mut vertices, stalled, support, |_| (DVec3::ZERO, 0), &mut [0, 0]);
        assert_eq!(result.status, SourceEpaStatus::Degenerate);
    }

    #[test]
    fn source_epa_core_overlap_reaches_native_bench_normal() {
        use crate::extensions::source_contact_gjk::{source_box_support, source_convex_support};
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3b3b2b/gjk.native-samples.json")).unwrap();
        let sample = fixture["samples"].as_array().unwrap().iter().find(|value| {
            value["input"]["label"] == "bench_foot_core_overlap"
        }).unwrap();
        let vector = |value: &serde_json::Value| DVec3::new(value["x"].as_f64().unwrap(),
            value["y"].as_f64().unwrap(), value["z"].as_f64().unwrap());
        let body = &sample["input"]["body_translation_cm"];
        let relative = -DVec3::new(body[0].as_f64().unwrap(), body[1].as_f64().unwrap(), body[2].as_f64().unwrap());
        let minimum = vector(&fixture["geometry"]["minimum_cm"]);
        let maximum = vector(&fixture["geometry"]["maximum_cm"]);
        let margin = fixture["collision_margin_0_cm"].as_f64().unwrap();
        let convex = fixture["other_geometry"]["convex_core_vertices_cm"].as_array().unwrap().iter()
            .map(|value| vector(value).as_vec3()).collect::<Vec<_>>();
        let mut vertices = sample["gjk_simplex"].as_array().unwrap().iter().map(|value| SourceEpaVertex {
            first_cm: vector(&value["shape_point_0_cm"]), second_cm: vector(&value["shape_point_1_cm"]) + relative,
        }).collect::<Vec<_>>();
        let mut last = [0, 0];
        let result = run_source_epa(&mut vertices, settings(),
            |direction| source_box_support(minimum, maximum, margin, direction),
            |direction| {
                let (point, index) = source_convex_support(&convex, DVec3::ONE, direction).unwrap();
                (point + relative, index)
            }, &mut last);
        assert!(matches!(result.status, SourceEpaStatus::Ok | SourceEpaStatus::MaxIterations));
        let native_normal = vector(&sample["points"][0]["world_normal"]);
        assert!(result.witness.normal.distance(-native_normal) < 1e-5);
        assert!(last[0] < 8 && last[1] < convex.len());
    }
}
