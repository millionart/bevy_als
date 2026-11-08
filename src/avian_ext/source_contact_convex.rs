use bevy::math::{DQuat, DVec3, Vec3};

use super::source_contact_epa::SourceEpaSettings;
use super::source_contact_face::{SourceBoxConvexFaces, SourceConvexTopology, source_box_convex_faces, source_convex_scale};
use super::source_contact_gjk::{SourceGjkResult, source_box_convex_gjk, source_gjk_exceeds_cull};
use super::source_contact_polygon::{source_box_face, source_clip_convex_face, source_direction, source_position};
use super::source_contact_restore::source_float_position;
use super::source_gjk_simplex::SourceGjkSimplex;

#[derive(Clone, Copy)]
pub(super) struct SourceConvexContactPoint {
    pub first_m: DVec3,
    pub second_m: DVec3,
    pub normal_second: DVec3,
    pub phi_m: f64,
}

pub(super) struct SourceBoxConvexManifold {
    pub gjk: SourceGjkResult,
    pub contacts: Vec<SourceConvexContactPoint>,
}

pub(super) fn source_box_convex_manifold(
    minimum: DVec3,
    maximum: DVec3,
    convex: &SourceConvexTopology,
    scale: DVec3,
    convex_to_box_translation: DVec3,
    convex_to_box_rotation: DQuat,
    box_margin: f64,
    cull_distance: f64,
    previous: Option<SourceGjkSimplex>,
) -> Option<SourceBoxConvexManifold> {
    let scale = source_convex_scale(scale);
    let vertices = convex.vertices_m.iter().copied().map(DVec3::from_array).collect::<Vec<_>>();
    let (epsilon, iterations) = include!("../als/settings/contact_gjk.rs");
    let (face_area_m4, initial_edge_m2, visibility_m, witness_m, max_iterations, max_flood, relative_epsilon) =
        include!("../als/settings/contact_epa.rs");
    let epa = SourceEpaSettings { face_area_m4, initial_edge_m2, visibility_m, witness_m, max_iterations, max_flood, relative_epsilon };
    let gjk = source_box_convex_gjk(minimum, maximum, box_margin, &vertices, scale, convex_to_box_translation,
        convex_to_box_rotation, epsilon, iterations, previous, epa)?;
    let mut contacts = Vec::new();
    if !source_gjk_exceeds_cull(&gjk, cull_distance) {
        let [plane_epsilon, reference_bias] = include!("../als/settings/contact_face_classification.rs");
        let face_search = include!("../als/settings/contact_face_search.rs");
        if let Some(faces) = source_box_convex_faces(minimum, maximum, convex, scale, &gjk, box_margin,
            face_search, plane_epsilon, reference_bias) {
            if faces.plane_contact {
                contacts = source_box_convex_face_contacts(minimum, maximum, convex, &faces,
                    convex_to_box_translation, convex_to_box_rotation, &gjk).unwrap_or_default();
            } else if let Some(contact) = source_box_convex_edge_contact(minimum, maximum, convex, &faces,
                convex_to_box_translation, convex_to_box_rotation, &gjk, box_margin, cull_distance,
                include!("../als/settings/contact_edge_zero_cull.rs")) {
                contacts.push(contact);
            }
        }
    }
    Some(SourceBoxConvexManifold { gjk, contacts })
}

fn source_convex_contact(first: DVec3, second: DVec3, normal: DVec3, phi: f64) -> SourceConvexContactPoint {
    SourceConvexContactPoint { first_m: source_float_position(first), second_m: source_float_position(second),
        normal_second: source_direction(source_direction(normal).as_vec3().as_dvec3()),
        phi_m: f64::from((phi * 100.0) as f32) * 0.01 }
}

fn source_box_closest_edge(vertices: &[DVec3], position: DVec3) -> DVec3 {
    let position = source_position(position);
    let mut previous = source_position(*vertices.last().expect("Source box face must not be empty"));
    let mut closest = DVec3::ZERO;
    let mut distance = f32::MAX as f64;
    for vertex in vertices {
        let current = source_position(*vertex);
        let edge = current - previous;
        let ratio = -(previous - position).dot(edge) / edge.length_squared();
        let ratio = if ratio < 0.0 { 0.0 } else if ratio < 1.0 { ratio } else { 1.0 };
        let candidate = previous + ratio * edge;
        let squared = (candidate - position).length_squared();
        if squared < distance { closest = candidate; distance = squared; }
        previous = current;
    }
    source_direction(closest) * 0.01
}

fn source_convex_closest_edge(vertices: &[DVec3], position: DVec3, scale: DVec3) -> DVec3 {
    let scale = source_direction(scale).as_vec3();
    let position = (source_position(position) * scale.recip().as_dvec3()).as_vec3();
    let mut previous = source_position(*vertices.last().expect("Source convex face must not be empty")).as_vec3();
    let mut closest = Vec3::ZERO;
    let mut distance = f32::MAX;
    let dot = |a: Vec3, b: Vec3| a.x * b.x + a.y * b.y + a.z * b.z;
    for vertex in vertices {
        let current = source_position(*vertex).as_vec3();
        let edge = current - previous;
        let ratio = -dot(previous - position, edge) / dot(edge, edge);
        let ratio = if ratio < 0.0 { 0.0 } else if ratio < 1.0 { ratio } else { 1.0 };
        let candidate = previous + ratio * edge;
        let squared = dot(candidate - position, candidate - position);
        if squared < distance { closest = candidate; distance = squared; }
        previous = current;
    }
    source_direction(closest.as_dvec3() * scale.as_dvec3()) * 0.01
}

pub(super) fn source_box_convex_face_contacts(
    minimum: DVec3,
    maximum: DVec3,
    convex: &SourceConvexTopology,
    faces: &SourceBoxConvexFaces,
    convex_to_box_translation: DVec3,
    convex_to_box_rotation: DQuat,
    gjk: &SourceGjkResult,
) -> Option<Vec<SourceConvexContactPoint>> {
    if !faces.plane_contact { return None; }
    let (box_vertices, _) = source_box_face(minimum, maximum, faces.box_index);
    let convex_vertices = convex.faces.get(faces.convex_index)?.vertices.iter().map(|index| {
        convex.vertices_m.get(*index).copied().map(DVec3::from_array).map(|point| point * faces.convex_scale)
    }).collect::<Option<Vec<_>>>()?;
    let inverse = convex_to_box_rotation.inverse();
    let (reference, normal, origin, incident, winding) = if faces.reference_box {
        let incident = convex_vertices.iter().map(|point| convex_to_box_rotation * *point + convex_to_box_translation).collect::<Vec<_>>();
        (box_vertices, faces.box_face.normal, faces.box_face.origin, incident, 1.0)
    } else {
        let incident = box_vertices.iter().map(|point| inverse * (*point - convex_to_box_translation)).collect::<Vec<_>>();
        let scale = faces.convex_scale;
        (convex_vertices, faces.convex_face.normal, faces.convex_face.origin, incident,
            scale.x.signum() * scale.y.signum() * scale.z.signum())
    };
    let normal_second = -gjk.normal_second;
    let separation = if faces.reference_box { -gjk.normal_first } else { -normal_second };
    let contacts = source_clip_convex_face(&reference, normal, &incident, winding).into_iter().map(|vertex| {
        let projected = vertex - (vertex - origin).dot(normal) * normal;
        let other = if faces.reference_box { inverse * (vertex - convex_to_box_translation) }
            else { convex_to_box_rotation * vertex + convex_to_box_translation };
        let (first, second) = if faces.reference_box { (projected, other) } else { (other, projected) };
        let phi = (projected - vertex).dot(separation);
        source_convex_contact(first, second, normal_second, phi)
    }).collect();
    Some(contacts)
}

pub(super) fn source_box_convex_edge_contact(
    minimum: DVec3,
    maximum: DVec3,
    convex: &SourceConvexTopology,
    faces: &SourceBoxConvexFaces,
    convex_to_box_translation: DVec3,
    convex_to_box_rotation: DQuat,
    gjk: &SourceGjkResult,
    box_margin: f64,
    cull_distance: f64,
    force_zero_cull: bool,
) -> Option<SourceConvexContactPoint> {
    if faces.plane_contact || (force_zero_cull && gjk.penetration_m < 0.0) { return None; }
    let normal_second = -gjk.normal_second;
    if box_margin <= 0.0 {
        return Some(source_convex_contact(gjk.closest_first_m, gjk.closest_second_m, normal_second, -gjk.penetration_m));
    }
    let (box_vertices, _) = source_box_face(minimum, maximum, faces.box_index);
    let convex_vertices = convex.faces.get(faces.convex_index)?.vertices.iter().map(|index| {
        convex.vertices_m.get(*index).copied().map(DVec3::from_array)
    }).collect::<Option<Vec<_>>>()?;
    let first = source_box_closest_edge(&box_vertices, gjk.closest_first_m);
    let second = source_convex_closest_edge(&convex_vertices, gjk.closest_second_m, faces.convex_scale);
    let first_in_second = convex_to_box_rotation.inverse() * (first - convex_to_box_translation);
    let phi = (first_in_second - second).dot(normal_second);
    if phi > cull_distance { return None; }
    let first = convex_to_box_rotation * (second + phi * normal_second) + convex_to_box_translation;
    Some(source_convex_contact(first, second, normal_second, phi))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::source_contact_face::{SourceContactFace, SourceConvexPlane, SourceVertexPlanes, source_convex_topology};
    use crate::extensions::source_gjk_simplex::SourceGjkSimplex;

    fn point(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value["x"].as_f64().unwrap(), value["z"].as_f64().unwrap(), value["y"].as_f64().unwrap())
    }

    fn array(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value[0].as_f64().unwrap(), value[2].as_f64().unwrap(), value[1].as_f64().unwrap())
    }

    fn rotation(value: &serde_json::Value) -> DQuat {
        DQuat::from_xyzw(-value[0].as_f64().unwrap(), -value[2].as_f64().unwrap(),
            -value[1].as_f64().unwrap(), value[3].as_f64().unwrap())
    }

    fn local_rotation(value: &serde_json::Value) -> DQuat {
        DQuat::from_xyzw(-value["x"].as_f64().unwrap(), -value["z"].as_f64().unwrap(),
            -value["y"].as_f64().unwrap(), value["w"].as_f64().unwrap())
    }

    fn relative_pose(sample: &serde_json::Value, frame: &serde_json::Value) -> (DVec3, DQuat, DQuat) {
        let body_rotation = rotation(&frame["body_rotation_xyzw"]);
        let mesh_rotation = rotation(&frame["mesh_rotation_xyzw"]);
        let box_local = &sample["geometry"]["shape_to_body"];
        let mesh_local = &sample["other_geometry"]["shape_to_body"];
        let box_position = array(&frame["body_translation_cm"]) * 0.01
            + body_rotation * (point(&box_local["translation"]) * 0.01);
        let mesh_position = array(&frame["mesh_translation_cm"]) * 0.01
            + mesh_rotation * (point(&mesh_local["translation"]) * 0.01);
        let box_rotation = body_rotation * local_rotation(&box_local["rotation"]);
        let mesh_rotation = mesh_rotation * local_rotation(&mesh_local["rotation"]);
        (box_rotation.inverse() * (mesh_position - box_position), box_rotation.inverse() * mesh_rotation, mesh_rotation)
    }

    fn assert_native_contacts(actual: &[SourceConvexContactPoint], expected: &serde_json::Value, mesh_rotation: DQuat) {
        let expected = expected.as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert!(actual.first_m.distance(point(&expected["shape_point_0_cm"]) * 0.01) < 2e-5);
            assert!(actual.second_m.distance(point(&expected["shape_point_1_cm"]) * 0.01) < 2e-5);
            assert!(actual.normal_second.distance(mesh_rotation.inverse() * point(&expected["world_normal"])) < 2e-5);
            assert!((actual.phi_m - expected["phi_cm"].as_f64().unwrap() * 0.01).abs() < 2e-5);
        }
    }

    #[test]
    fn source_complete_convex_manifold_matches_native_faces_edges_and_cull() {
        let fixtures = [include_str!("../../docs/audit/bf144/d100b3b3c2a/classification.native-samples.json"),
            include_str!("../../docs/audit/bf144/d100b3b3c4a/cull.native-samples.json")];
        for fixture in fixtures {
            let fixture: serde_json::Value = serde_json::from_str(fixture).unwrap();
            for sample in fixture["samples"].as_array().unwrap() {
                let input = &sample["input"];
                let name = input["mesh_asset_path"].as_str().unwrap().rsplit('/').next().unwrap();
                let index = input["mesh_shape_index"].as_u64().unwrap() as u32;
                let hull = source_convex_topology(name, index).unwrap();
                let minimum = point(&sample["geometry"]["minimum_cm"]) * 0.01;
                let maximum = point(&sample["geometry"]["maximum_cm"]) * 0.01;
                let margin = sample["collision_margin_0_cm"].as_f64().unwrap() * 0.01;
                let (translation, orientation, mesh_rotation) = relative_pose(sample, input);
                let manifold = source_box_convex_manifold(minimum, maximum, hull, array(&input["mesh_scale"]), translation,
                    orientation, margin, input["cull_distance_cm"].as_f64().unwrap() * 0.01, None).unwrap();
                assert_native_contacts(&manifold.contacts, &sample["points"], mesh_rotation);
            }
        }
    }

    #[test]
    fn source_complete_convex_manifold_preserves_native_warm_rebuild() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3b3c4b/warm.native-samples.json")).unwrap();
        for sample in fixture["samples"].as_array().unwrap() {
            assert_eq!(sample["restored"], false);
            let input = &sample["input"];
            let hull = source_convex_topology("SM_Als_Bench", 2).unwrap();
            let minimum = point(&sample["geometry"]["minimum_cm"]) * 0.01;
            let maximum = point(&sample["geometry"]["maximum_cm"]) * 0.01;
            let margin = sample["collision_margin_0_cm"].as_f64().unwrap() * 0.01;
            let cull = input["cull_distance_cm"].as_f64().unwrap() * 0.01;
            let (translation, orientation, _) = relative_pose(sample, input);
            let initial = source_box_convex_manifold(minimum, maximum, hull, array(&input["mesh_scale"]), translation,
                orientation, margin, cull, None).unwrap();
            let (translation, orientation, mesh_rotation) = relative_pose(sample, &input["next"]);
            let next = source_box_convex_manifold(minimum, maximum, hull, array(&input["mesh_scale"]), translation,
                orientation, margin, cull, Some(initial.gjk.simplex_cm)).unwrap();
            assert_native_contacts(&next.contacts, &sample["next_points"], mesh_rotation);
        }
    }

    #[test]
    fn source_face_projection_matches_native_bench_contact_order() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3b3c2a/classification.native-samples.json")).unwrap();
        for (sample, box_index, convex_index, reference_box) in [
            (&fixture["samples"][0], 3, 20, false), (&fixture["samples"][1], 2, 10, true),
        ] {
            let minimum = point(&sample["geometry"]["minimum_cm"]) * 0.01;
            let maximum = point(&sample["geometry"]["maximum_cm"]) * 0.01;
            let hull_index = sample["input"]["mesh_shape_index"].as_u64().unwrap() as u32;
            let hull = source_convex_topology("SM_Als_Bench", hull_index).unwrap();
            let (vertices, normal) = source_box_face(minimum, maximum, box_index);
            let plane = &hull.faces[convex_index];
            let faces = SourceBoxConvexFaces { box_index, convex_index, reference_box, plane_contact: true,
                box_face: SourceContactFace { normal, origin: vertices[0] },
                convex_face: SourceContactFace { normal: DVec3::from_array(plane.normal), origin: DVec3::from_array(plane.origin_m) },
                convex_scale: DVec3::ONE };
            let source_normal = point(&sample["points"][0]["world_normal"]);
            let gjk = SourceGjkResult { simplex_cm: SourceGjkSimplex::default(), last_vertices: [0, 0],
                normal_first: -source_normal, normal_second: -source_normal, closest_first_m: DVec3::ZERO,
                closest_second_m: DVec3::ZERO, penetration_m: 0.0, max_support_delta_m: 0.0, epa_status: None };
            let translation = -array(&sample["input"]["body_translation_cm"]) * 0.01;
            let actual = source_box_convex_face_contacts(minimum, maximum, hull, &faces, translation, DQuat::IDENTITY, &gjk).unwrap();
            let expected = sample["points"].as_array().unwrap();
            assert_eq!(actual.len(), expected.len());
            for (actual, expected) in actual.iter().zip(expected) {
                assert!(actual.first_m.distance(point(&expected["shape_point_0_cm"]) * 0.01) < 2e-5);
                assert!(actual.second_m.distance(point(&expected["shape_point_1_cm"]) * 0.01) < 2e-5);
                assert!((actual.phi_m - expected["phi_cm"].as_f64().unwrap() * 0.01).abs() < 2e-5);
                assert!(actual.normal_second.distance(point(&expected["world_normal"])) < 2e-5);
            }
        }
    }

    #[test]
    fn source_face_projection_preserves_reflected_winding_and_rotation() {
        let minimum = DVec3::splat(-1.0);
        let maximum = DVec3::ONE;
        let (convex_vertices, convex_normal) = source_box_face(minimum, maximum, 2);
        let (box_vertices, box_normal) = source_box_face(minimum, maximum, 5);
        let hull = SourceConvexTopology { vertices_m: convex_vertices.iter().map(|point| point.to_array()).collect(),
            faces: vec![SourceConvexPlane { normal: convex_normal.to_array(), origin_m: convex_vertices[0].to_array(), vertices: vec![0, 1, 2, 3] }],
            vertex_planes: vec![SourceVertexPlanes { count: 1, planes: vec![0] }], margin_m: 0.0,
            winding: -1.0, bounds_m: [minimum, maximum] };
        let gjk = SourceGjkResult { simplex_cm: SourceGjkSimplex::default(), last_vertices: [0, 0],
            normal_first: DVec3::Y, normal_second: DVec3::Y, closest_first_m: DVec3::ZERO,
            closest_second_m: DVec3::ZERO, penetration_m: 0.0, max_support_delta_m: 0.0, epa_status: None };
        let scale = DVec3::new(-1.0, 1.0, 1.0);
        for reference_box in [false, true] {
            let mut faces = SourceBoxConvexFaces { box_index: 5, convex_index: 0, reference_box, plane_contact: true,
                box_face: SourceContactFace { normal: box_normal, origin: box_vertices[0] },
                convex_face: SourceContactFace { normal: convex_normal, origin: convex_vertices[0] * scale }, convex_scale: scale };
            for rotation in [DQuat::IDENTITY, DQuat::from_rotation_y(std::f64::consts::PI)] {
                let contacts = source_box_convex_face_contacts(minimum, maximum, &hull, &faces, DVec3::Y * 2.0, rotation, &gjk).unwrap();
                assert_eq!(contacts.len(), 4);
                for contact in contacts {
                    assert!((contact.first_m.y - 1.0).abs() < 1e-7);
                    assert!((contact.second_m.y + 1.0).abs() < 1e-7);
                    assert!(contact.phi_m.abs() < 1e-7);
                    assert_eq!(contact.normal_second, DVec3::NEG_Y);
                }
            }
            faces.plane_contact = false;
            assert!(source_box_convex_face_contacts(minimum, maximum, &hull, &faces, DVec3::Y * 2.0, DQuat::IDENTITY, &gjk).is_none());
        }
    }

    #[test]
    fn source_closest_edges_keep_source_ties_and_unscaled_distance() {
        let square = [DVec3::ZERO, DVec3::X * 0.01, (DVec3::X + DVec3::Y) * 0.01, DVec3::Y * 0.01];
        let center = (DVec3::X + DVec3::Y) * 0.005;
        assert!(source_box_closest_edge(&square, center).distance(DVec3::Y * 0.005) < 1e-12);
        assert!(source_convex_closest_edge(&square, center, DVec3::ONE).distance(DVec3::Y * 0.005) < 1e-8);
        let point = DVec3::new(0.08, 0.004, 0.0);
        let closest = source_convex_closest_edge(&square, point, DVec3::new(10.0, 1.0, 1.0));
        assert!(closest.distance(DVec3::new(0.1, 0.004, 0.0)) < 1e-8);
        assert!(source_box_closest_edge(&[DVec3::ONE], DVec3::ZERO).distance(DVec3::ONE) < 1e-12);
        assert!(source_convex_closest_edge(&[DVec3::ONE], DVec3::ZERO, DVec3::ONE).distance(DVec3::ONE) < 1e-8);
    }

    #[test]
    fn source_edge_projection_matches_native_arch_with_edge_witnesses() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3b3c2a/classification.native-samples.json")).unwrap();
        let sample = &fixture["samples"][2];
        let expected = &sample["points"][0];
        let minimum = point(&sample["geometry"]["minimum_cm"]) * 0.01;
        let maximum = point(&sample["geometry"]["maximum_cm"]) * 0.01;
        let hull = source_convex_topology("SM_Als_Arch_02", 2).unwrap();
        let (box_vertices, box_normal) = source_box_face(minimum, maximum, 2);
        let plane = &hull.faces[21];
        let faces = SourceBoxConvexFaces { box_index: 2, convex_index: 21, reference_box: false, plane_contact: false,
            box_face: SourceContactFace { normal: box_normal, origin: box_vertices[0] },
            convex_face: SourceContactFace { normal: DVec3::from_array(plane.normal), origin: DVec3::from_array(plane.origin_m) },
            convex_scale: DVec3::ONE };
        let normal = point(&expected["world_normal"]);
        let mut gjk = SourceGjkResult { simplex_cm: SourceGjkSimplex::default(), last_vertices: [0, 0],
            normal_first: -normal, normal_second: -normal, closest_first_m: point(&expected["shape_point_0_cm"]) * 0.01,
            closest_second_m: point(&expected["shape_point_1_cm"]) * 0.01,
            penetration_m: -expected["phi_cm"].as_f64().unwrap() * 0.01, max_support_delta_m: 0.0, epa_status: None };
        let translation = -array(&sample["input"]["body_translation_cm"]) * 0.01;
        let margin = sample["collision_margin_0_cm"].as_f64().unwrap() * 0.01;
        let contact = source_box_convex_edge_contact(minimum, maximum, hull, &faces, translation,
            DQuat::IDENTITY, &gjk, margin, 0.03, false).unwrap();
        assert!(contact.first_m.distance(point(&expected["shape_point_0_cm"]) * 0.01) < 2e-6);
        assert!(contact.second_m.distance(point(&expected["shape_point_1_cm"]) * 0.01) < 2e-6);
        assert!((contact.phi_m - expected["phi_cm"].as_f64().unwrap() * 0.01).abs() < 2e-6);
        assert!(source_box_convex_edge_contact(minimum, maximum, hull, &faces, translation,
            DQuat::IDENTITY, &gjk, margin, -0.04, false).is_none());
        let no_margin = source_box_convex_edge_contact(minimum, maximum, hull, &faces, translation,
            DQuat::IDENTITY, &gjk, 0.0, 0.03, false).unwrap();
        assert_eq!(no_margin.first_m, source_float_position(gjk.closest_first_m));
        gjk.penetration_m = -0.001;
        assert!(source_box_convex_edge_contact(minimum, maximum, hull, &faces, translation,
            DQuat::IDENTITY, &gjk, 0.0, 0.03, true).is_none());
    }

    #[test]
    fn source_edge_projection_matches_direct_native_gjk_when_available() {
        use crate::extensions::source_contact_face::source_box_convex_faces;
        let Ok(path) = std::env::var("ALS_SOURCE_FACE_CLASSIFICATION_REPORT") else { return; };
        let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let sample = report["physics_contact_probe"]["samples"].as_array().unwrap().iter()
            .find(|value| value["points"].as_array().is_some_and(|points| points.first()
                .is_some_and(|point| point["contact_type"] == 2))).unwrap();
        let raw = &sample["cold_gjk"];
        let gjk = SourceGjkResult { simplex_cm: SourceGjkSimplex::default(),
            last_vertices: [raw["vertex_0"].as_u64().unwrap() as usize, raw["vertex_1"].as_u64().unwrap() as usize],
            normal_first: point(&raw["shape_normal_0"]), normal_second: point(&raw["shape_normal_1"]),
            closest_first_m: point(&raw["shape_point_0_cm"]) * 0.01,
            closest_second_m: point(&raw["shape_point_1_cm"]) * 0.01,
            penetration_m: raw["penetration_cm"].as_f64().unwrap() * 0.01, max_support_delta_m: 0.0, epa_status: None };
        let minimum = point(&sample["geometry"]["minimum_cm"]) * 0.01;
        let maximum = point(&sample["geometry"]["maximum_cm"]) * 0.01;
        let margin = sample["collision_margin_0_cm"].as_f64().unwrap() * 0.01;
        let name = sample["input"]["mesh_asset_path"].as_str().unwrap().rsplit('/').next().unwrap();
        let index = sample["input"]["mesh_shape_index"].as_u64().unwrap() as u32;
        let hull = source_convex_topology(name, index).unwrap();
        let [epsilon, bias] = include!("../als/settings/contact_face_classification.rs");
        let faces = source_box_convex_faces(minimum, maximum, hull, point(&sample["other_geometry"]["convex_core_scale"]),
            &gjk, margin, include!("../als/settings/contact_face_search.rs"), epsilon, bias).unwrap();
        assert!(!faces.plane_contact);
        let translation = (array(&sample["input"]["mesh_translation_cm"]) - array(&sample["input"]["body_translation_cm"])) * 0.01;
        let cull = sample["input"]["cull_distance_cm"].as_f64().unwrap() * 0.01;
        let actual = source_box_convex_edge_contact(minimum, maximum, hull, &faces, translation, DQuat::IDENTITY,
            &gjk, margin, cull, include!("../als/settings/contact_edge_zero_cull.rs")).unwrap();
        let expected = &sample["points"][0];
        assert!(actual.first_m.distance(point(&expected["shape_point_0_cm"]) * 0.01) < 2e-5);
        assert!(actual.second_m.distance(point(&expected["shape_point_1_cm"]) * 0.01) < 2e-5);
        assert!((actual.phi_m - expected["phi_cm"].as_f64().unwrap() * 0.01).abs() < 2e-5);
    }
}
