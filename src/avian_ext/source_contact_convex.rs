use bevy::math::{DQuat, DVec3};

use super::source_contact_face::{SourceBoxConvexFaces, SourceConvexTopology};
use super::source_contact_gjk::SourceGjkResult;
use super::source_contact_polygon::{source_box_face, source_clip_convex_face, source_direction};
use super::source_contact_restore::source_float_position;

#[derive(Clone, Copy)]
pub(super) struct SourceConvexContactPoint {
    pub first_m: DVec3,
    pub second_m: DVec3,
    pub normal_second: DVec3,
    pub phi_m: f64,
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
        SourceConvexContactPoint {
            first_m: source_float_position(first), second_m: source_float_position(second),
            normal_second: source_direction(source_direction(normal_second).as_vec3().as_dvec3()),
            phi_m: f64::from((phi * 100.0) as f32) * 0.01,
        }
    }).collect();
    Some(contacts)
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
                closest_second_m: DVec3::ZERO, penetration_m: 0.0, epa_status: None };
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
            closest_second_m: DVec3::ZERO, penetration_m: 0.0, epa_status: None };
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
}
