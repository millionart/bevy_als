use bevy::math::{DQuat, DVec3};

use super::source_contact_convex::{SourceConvexContactPoint, source_convex_contact};
use super::source_contact_epa::SourceEpaSettings;
use super::source_contact_face::{SourceConvexTopology, source_capsule_convex_face, source_convex_scale};
use super::source_contact_gjk::source_capsule_convex_gjk;
use super::source_contact_polygon::{SOURCE_KINDA_SMALL_NUMBER, source_direction, source_position};

#[derive(Clone, Copy)]
pub(super) struct SourceCapsuleGeometry {
    pub first_m: DVec3,
    pub axis: DVec3,
    pub length_m: f64,
    pub radius_m: f64,
}

fn source_safe_normal(value: DVec3) -> DVec3 {
    let squared = value.length_squared();
    if squared <= f64::MIN_POSITIVE { DVec3::X } else { value / squared.sqrt() }
}

fn source_clip_capsule_line(reference: &[DVec3], normal: DVec3, mut points: [DVec3; 2], winding: f64) -> Vec<DVec3> {
    let Some(mut previous) = reference.last().copied() else { return Vec::new(); };
    let [_, _, plane_tolerance_cm, _] = include!("../als/settings/capsule_convex_contact.rs");
    for &current in reference {
        let edge_normal = winding * source_safe_normal(normal.cross(previous - current));
        previous = current;
        let first = (points[0] - current).dot(edge_normal);
        let second = (points[1] - current).dot(edge_normal);
        if first > plane_tolerance_cm && second > plane_tolerance_cm { return Vec::new(); }
        if first > plane_tolerance_cm && second < plane_tolerance_cm {
            let alpha = second / (second - first);
            points[0] = points[1] + alpha * (points[0] - points[1]);
        } else if second > plane_tolerance_cm && first < plane_tolerance_cm {
            let alpha = first / (first - second);
            points[1] = points[0] + alpha * (points[1] - points[0]);
        }
    }
    points.to_vec()
}

pub(super) fn source_capsule_convex_manifold(
    convex: &SourceConvexTopology,
    scale: DVec3,
    capsule: SourceCapsuleGeometry,
    translation: DVec3,
    rotation: DQuat,
    cull_distance: f64,
) -> Option<Vec<SourceConvexContactPoint>> {
    let scale = source_convex_scale(scale);
    let source_rotation = DQuat::from_xyzw(-rotation.x, -rotation.z, -rotation.y, rotation.w);
    let source_translation = source_position(translation);
    let first = source_translation + source_rotation * source_position(capsule.first_m);
    let axis = source_rotation * source_direction(capsule.axis);
    let length = capsule.length_m * 100.0;
    let radius = capsule.radius_m * 100.0;
    let vertices = convex.vertices_m.iter().copied().map(DVec3::from_array).collect::<Vec<_>>();
    let (epsilon, iterations) = include!("../als/settings/contact_gjk.rs");
    let (face_area_m4, initial_edge_m2, visibility_m, witness_m, max_iterations, max_flood, relative_epsilon) =
        include!("../als/settings/contact_epa.rs");
    let epa = SourceEpaSettings { face_area_m4, initial_edge_m2, visibility_m, witness_m,
        max_iterations, max_flood, relative_epsilon };
    let gjk = source_capsule_convex_gjk(&vertices, scale, source_direction(first) * 0.01,
        source_direction(axis), capsule.length_m, epsilon, iterations, epa)?;
    let phi = -gjk.penetration_m * 100.0 - radius;
    if phi > cull_distance * 100.0 { return Some(Vec::new()); }
    let Some(face) = source_capsule_convex_face(convex, scale, &gjk, source_direction(axis)) else {
        return Some(Vec::new());
    };
    let polygon = convex.faces.get(face.index)?.vertices.iter().map(|index| {
        convex.vertices_m.get(*index).copied().map(DVec3::from_array).map(|point| source_position(point * face.scale))
    }).collect::<Option<Vec<_>>>()?;
    let normal = source_direction(gjk.normal_first);
    let plane_normal = source_direction(face.face.normal);
    let plane_origin = source_position(face.face.origin);
    let winding = face.scale.x.signum() * face.scale.y.signum() * face.scale.z.signum();
    let inverse = source_rotation.inverse();
    let make_contact = |capsule_point: DVec3, convex_point: DVec3, contact_normal: DVec3, contact_phi: f64| {
        let local = inverse * (capsule_point - source_translation);
        source_convex_contact(source_direction(local) * 0.01, source_direction(convex_point) * 0.01,
            source_direction(contact_normal), contact_phi * 0.01)
    };
    let gjk_capsule = source_position(gjk.closest_second_m) - radius * normal;
    let gjk_convex = source_position(gjk.closest_first_m);
    let mut contacts = Vec::new();
    if face.cylinder {
        let capsule_normal = source_safe_normal(normal - axis.dot(normal) * axis);
        let clipped = source_clip_capsule_line(&polygon, capsule_normal, [first, first + axis * length], winding);
        let divisor = capsule_normal.dot(plane_normal);
        if !clipped.is_empty() && divisor.abs() > f64::from(SOURCE_KINDA_SMALL_NUMBER) {
            for point in clipped {
                let capsule_point = point - radius * capsule_normal;
                let distance = (plane_origin - capsule_point).dot(plane_normal) / divisor;
                let convex_point = capsule_point + distance * capsule_normal;
                let contact_phi = (capsule_point - convex_point).dot(capsule_normal);
                contacts.push(make_contact(capsule_point, convex_point, capsule_normal, contact_phi));
            }
            return Some(contacts);
        }
    } else if face.auxiliary {
        let cylinder_normal = plane_normal - axis.dot(plane_normal) * axis;
        let cylinder_normal = cylinder_normal / cylinder_normal.length();
        let start = first - radius * cylinder_normal;
        let clipped = source_clip_capsule_line(&polygon, plane_normal, [start, start + axis * length], winding);
        let [_, _, _, fraction] = include!("../als/settings/capsule_convex_contact.rs");
        let tolerance_squared = (fraction * radius).powi(2);
        for capsule_point in clipped {
            if (capsule_point - gjk_capsule).length_squared() > tolerance_squared {
                let convex_point = capsule_point - (capsule_point - plane_origin).dot(plane_normal) * plane_normal;
                let contact_phi = (capsule_point - convex_point).dot(plane_normal);
                contacts.push(make_contact(capsule_point, convex_point, plane_normal, contact_phi));
            }
        }
    }
    contacts.push(make_contact(gjk_capsule, gjk_convex, normal, phi));
    Some(contacts)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn source_capsule_line_clipping_preserves_endpoints_and_tolerance() {
        let p = |x, y, z| DVec3::new(x, y, z);
        let reference = [p(-1.0, -1.0, 0.0), p(1.0, -1.0, 0.0), p(1.0, 1.0, 0.0), p(-1.0, 1.0, 0.0)];
        let clipped = source_clip_capsule_line(&reference, DVec3::Z, [p(-2.0, 0.0, 0.0), p(2.0, 0.0, 0.0)], 1.0);
        assert_eq!(clipped, vec![p(-1.0, 0.0, 0.0), p(1.0, 0.0, 0.0)]);
        let near = [p(-1.005, 0.0, 0.0), p(0.0, 0.0, 0.0)];
        assert_eq!(source_clip_capsule_line(&reference, DVec3::Z, near, 1.0), near.to_vec());
        assert!(source_clip_capsule_line(&reference, DVec3::Z, [p(-2.0, 0.0, 0.0), p(-1.1, 0.0, 0.0)], 1.0).is_empty());
        let reflected = reference.map(|value| value * DVec3::new(-1.0, 1.0, 1.0));
        assert_eq!(source_clip_capsule_line(&reflected, DVec3::Z, [p(-2.0, 0.0, 0.0), p(2.0, 0.0, 0.0)], -1.0), clipped);
    }

    #[test]
    fn source_capsule_convex_manifold_matches_native_samples() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d130b2/manifold.native-samples.json")).unwrap();
        for sample in fixture["samples"].as_array().unwrap() {
            let input = &sample["input"];
            let geometry = &sample["geometry"];
            let first = point(&geometry["endpoint_0_cm"]) * 0.01;
            let second = point(&geometry["endpoint_1_cm"]) * 0.01;
            let length = first.distance(second);
            let capsule = SourceCapsuleGeometry { first_m: first, axis: (second - first) / length,
                length_m: length, radius_m: geometry["radius_cm"].as_f64().unwrap() * 0.01 };
            let body_rotation = rotation(&input["body_rotation_xyzw"]);
            let mesh_rotation = rotation(&input["mesh_rotation_xyzw"]);
            let local = &geometry["shape_to_body"];
            let local_rotation = DQuat::from_xyzw(-local["rotation"]["x"].as_f64().unwrap(),
                -local["rotation"]["z"].as_f64().unwrap(), -local["rotation"]["y"].as_f64().unwrap(),
                local["rotation"]["w"].as_f64().unwrap());
            let capsule_position = array(&input["body_translation_cm"]) * 0.01
                + body_rotation * (point(&local["translation"]) * 0.01);
            let mesh_position = array(&input["mesh_translation_cm"]) * 0.01;
            let relative_position = mesh_rotation.inverse() * (capsule_position - mesh_position);
            let relative_rotation = mesh_rotation.inverse() * body_rotation * local_rotation;
            let scale = array(&input["mesh_scale"]);
            let shape_index = input["mesh_shape_index"].as_u64().unwrap() as u32;
            let convex = crate::extensions::source_contact_face::source_convex_topology("SM_Als_Arch_01", shape_index).unwrap();
            let cull = input["cull_distance_cm"].as_f64().unwrap() * 0.01;
            let actual = source_capsule_convex_manifold(convex, scale, capsule, relative_position, relative_rotation, cull).unwrap();
            let expected = sample["points"].as_array().unwrap();
            assert_eq!(actual.len(), expected.len(), "{}", input["label"]);
            for (actual, expected) in actual.iter().zip(expected) {
                assert!(actual.first_m.distance(point(&expected["shape_point_0_cm"]) * 0.01) < 2e-5, "{}", input["label"]);
                assert!(actual.second_m.distance(point(&expected["shape_point_1_cm"]) * 0.01) < 2e-5, "{}", input["label"]);
                let normal = mesh_rotation.inverse() * point(&expected["world_normal"]);
                assert!(actual.normal_second.distance(normal) < 2e-5, "{}", input["label"]);
                assert!((actual.phi_m - expected["phi_cm"].as_f64().unwrap() * 0.01).abs() < 2e-5, "{}", input["label"]);
            }
        }
    }
}
