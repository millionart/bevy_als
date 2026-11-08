use bevy::math::DVec3;
use std::collections::HashMap;
use std::sync::LazyLock;

use super::source_contact_gjk::SourceGjkResult;
use super::source_contact_polygon::{SOURCE_KINDA_SMALL_NUMBER, SOURCE_SMALL_NUMBER, source_box_face, source_direction, source_position};

pub(crate) const SOURCE_BOX_VERTEX_PLANES: [[usize; 3]; 8] = include!("source_box_vertex_planes.rs");

#[derive(serde::Deserialize)]
pub(super) struct SourceConvexPlane {
    pub normal: [f64; 3],
    pub origin_m: [f64; 3],
    pub vertices: Vec<usize>,
}

#[derive(serde::Deserialize)]
pub(super) struct SourceVertexPlanes {
    pub count: usize,
    pub planes: Vec<usize>,
}

#[derive(serde::Deserialize)]
pub(super) struct SourceConvexTopology {
    pub vertices_m: Vec<[f64; 3]>,
    pub faces: Vec<SourceConvexPlane>,
    pub vertex_planes: Vec<SourceVertexPlanes>,
    pub margin_m: f64,
    pub winding: f64,
    #[serde(skip)]
    pub bounds_m: [DVec3; 2],
}

static SOURCE_CONVEX_TOPOLOGY: LazyLock<HashMap<String, Vec<SourceConvexTopology>>> = LazyLock::new(|| {
    let mut shapes: HashMap<String, Vec<SourceConvexTopology>> = serde_json::from_str(include_str!(
        "../../assets/ALSExtras/Environment/Meshes/static_convex_topology.runtime.json"
    )).expect("Generated source convex topology must be valid");
    for hull in shapes.values_mut().flatten() {
        hull.bounds_m = hull.vertices_m.iter().copied().map(DVec3::from_array).fold(
            [DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)],
            |[minimum, maximum], vertex| [minimum.min(vertex), maximum.max(vertex)],
        );
    }
    shapes
});

pub(super) fn source_convex_hulls(asset: &str) -> Option<&'static [SourceConvexTopology]> {
    SOURCE_CONVEX_TOPOLOGY.get(asset).map(Vec::as_slice)
}

pub(super) fn source_convex_topology(asset: &str, index: u32) -> Option<&'static SourceConvexTopology> {
    source_convex_hulls(asset)?.get(index as usize)
}

#[derive(Clone, Copy)]
pub(crate) struct SourceContactFace {
    pub normal: DVec3,
    pub origin: DVec3,
}

pub(super) struct SourceBoxConvexFaces {
    pub box_index: usize,
    pub convex_index: usize,
    pub reference_box: bool,
    pub plane_contact: bool,
    pub box_face: SourceContactFace,
    pub convex_face: SourceContactFace,
    pub convex_scale: DVec3,
}

pub(super) fn source_convex_scale(scale: DVec3) -> DVec3 {
    let minimum = f64::from(SOURCE_KINDA_SMALL_NUMBER);
    DVec3::from_array(scale.to_array().map(|value| {
        f64::from((if value.abs() < minimum { minimum } else { value }) as f32)
    }))
}

pub(super) fn source_box_convex_faces(
    minimum: DVec3,
    maximum: DVec3,
    convex: &SourceConvexTopology,
    scale: DVec3,
    gjk: &SourceGjkResult,
    box_margin: f64,
    face_search: f64,
    plane_epsilon: f32,
    reference_bias: f32,
) -> Option<SourceBoxConvexFaces> {
    let scale = source_convex_scale(scale);
    let box_faces = (0..6).map(|index| {
        let (vertices, normal) = source_box_face(minimum, maximum, index);
        SourceContactFace { origin: vertices[0], normal }
    }).collect::<Vec<_>>();
    let convex_faces = convex.faces.iter().map(|face| SourceContactFace {
        origin: DVec3::from_array(face.origin_m), normal: DVec3::from_array(face.normal),
    }).collect::<Vec<_>>();
    let box_planes = SOURCE_BOX_VERTEX_PLANES.get(gjk.last_vertices[0])?;
    let convex_planes = convex.vertex_planes.get(gjk.last_vertices[1])?;
    let direction1 = -gjk.normal_first;
    let direction2 = -gjk.normal_second;
    let box_index = source_select_contact_plane(&box_faces, box_planes, box_planes.len(),
        gjk.closest_first_m, direction1, box_margin, face_search, None, true)?;
    let convex_index = source_select_contact_plane(&convex_faces, &convex_planes.planes, convex_planes.count,
        gjk.closest_second_m, -direction2, 0.0, face_search, (scale != DVec3::ONE).then_some(scale), false)?;
    let box_face = box_faces[box_index];
    let mut convex_face = convex_faces[convex_index];
    if scale != DVec3::ONE {
        convex_face.origin *= scale;
        let normal = convex_face.normal / scale;
        if normal.length_squared() > f64::from(SOURCE_SMALL_NUMBER) { convex_face.normal = normal.normalize(); }
    }
    let box_alignment = (-direction1).dot(box_face.normal).abs();
    let convex_alignment = direction2.dot(convex_face.normal).abs();
    Some(SourceBoxConvexFaces {
        box_index, convex_index, reference_box: !(convex_alignment + f64::from(reference_bias) > box_alignment),
        plane_contact: (box_alignment - 1.0).abs() <= f64::from(plane_epsilon)
            || (convex_alignment - 1.0).abs() <= f64::from(plane_epsilon),
        box_face, convex_face, convex_scale: scale,
    })
}

pub(super) struct SourceCapsuleConvexFace {
    pub index: usize,
    pub face: SourceContactFace,
    pub scale: DVec3,
    pub cylinder: bool,
    pub plane_contact: bool,
    pub auxiliary: bool,
}

fn source_capsule_contact_flags(axis: DVec3, gjk_normal: DVec3, plane_normal: DVec3) -> (bool, bool, bool) {
    let [axis_threshold, auxiliary_threshold, _, _] = include!("../als/settings/capsule_convex_contact.rs");
    let [plane_epsilon, _] = include!("../als/settings/contact_face_classification.rs");
    let cylinder = axis.dot(gjk_normal).abs() < axis_threshold;
    let plane_contact = (plane_normal.dot(gjk_normal) - 1.0).abs() <= f64::from(plane_epsilon);
    let auxiliary = plane_contact && axis.dot(plane_normal).abs() < auxiliary_threshold;
    (cylinder, plane_contact, auxiliary)
}

pub(super) fn source_capsule_convex_face(
    convex: &SourceConvexTopology,
    scale: DVec3,
    gjk: &SourceGjkResult,
    axis: DVec3,
) -> Option<SourceCapsuleConvexFace> {
    let scale = source_convex_scale(scale);
    let faces = convex.faces.iter().map(|face| SourceContactFace {
        origin: DVec3::from_array(face.origin_m), normal: DVec3::from_array(face.normal),
    }).collect::<Vec<_>>();
    let planes = convex.vertex_planes.get(gjk.last_vertices[0])?;
    let index = source_select_contact_plane(&faces, &planes.planes, planes.count, gjk.closest_first_m,
        -gjk.normal_first, 0.0, include!("../als/settings/contact_face_search.rs"),
        (scale != DVec3::ONE).then_some(scale), false)?;
    let mut face = faces[index];
    if scale != DVec3::ONE {
        face.origin *= scale;
        let normal = face.normal / scale;
        if normal.length_squared() > f64::from(SOURCE_SMALL_NUMBER) { face.normal = normal.normalize(); }
    }
    let (cylinder, plane_contact, auxiliary) = source_capsule_contact_flags(axis, gjk.normal_first, face.normal);
    Some(SourceCapsuleConvexFace { index, face, scale, cylinder, plane_contact, auxiliary })
}

pub(crate) fn source_select_contact_plane(
    faces: &[SourceContactFace],
    vertex_planes: &[usize],
    vertex_plane_count: usize,
    position: DVec3,
    direction: DVec3,
    margin: f64,
    minimum_distance: f64,
    scale: Option<DVec3>,
    is_box: bool,
) -> Option<usize> {
    if faces.is_empty() { return None; }
    let position = source_position(position);
    let direction = source_direction(direction);
    let margin = margin * 100.0;
    let minimum_distance = minimum_distance * 100.0;
    let maximum = margin.max(minimum_distance);
    let scale = scale.map(source_direction);
    let mut best = None;
    let mut best_dot = 1.0;
    let mut consider = |index: usize| {
        let Some(face) = faces.get(index) else { return; };
        let normal = source_direction(face.normal);
        let origin = source_position(face.origin);
        let (distance, dot, limit) = if let Some(scale) = scale {
            let scaled = normal.as_vec3() / scale.as_vec3();
            let squared = scaled.length_squared();
            if squared <= SOURCE_SMALL_NUMBER { return; }
            let scaled = scaled * squared.sqrt().recip();
            let distance = (position.as_vec3() - origin.as_vec3() * scale.as_vec3()).dot(scaled);
            let dot = direction.as_vec3().dot(scaled);
            (f64::from(distance), f64::from(dot), f64::from((margin as f32).max(minimum_distance as f32)))
        } else {
            ((position - origin).dot(normal), direction.dot(normal), maximum)
        };
        if distance.abs() <= limit && dot <= -f64::from(SOURCE_SMALL_NUMBER) && dot < best_dot {
            best = Some(index);
            best_dot = dot;
        }
    };
    if vertex_plane_count > 3 {
        for index in 0..faces.len() { consider(index); }
    } else {
        for &index in vertex_planes { consider(index); }
    }
    if best.is_some() { return best; }
    if is_box {
        let absolute = direction.abs();
        let axis = if absolute.x > absolute.y && absolute.x > absolute.z { 0 }
            else if absolute.y > absolute.z { 1 } else { 2 };
        return Some(axis + if direction[axis] < 0.0 { 3 } else { 0 });
    }
    let mut selected = None;
    let mut smallest = f64::MAX;
    for (index, face) in faces.iter().enumerate() {
        let mut normal = source_direction(face.normal);
        if let Some(scale) = scale {
            normal /= scale;
            let squared = normal.length_squared();
            normal = if squared > f64::from(SOURCE_SMALL_NUMBER) { normal / squared.sqrt() } else { DVec3::ZERO };
        }
        let dot = normal.dot(direction);
        if dot < smallest { selected = Some(index); smallest = dot; }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value["x"].as_f64().unwrap(), value["z"].as_f64().unwrap(), value["y"].as_f64().unwrap())
    }

    fn array(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value[0].as_f64().unwrap(), value[1].as_f64().unwrap(), value[2].as_f64().unwrap())
    }

    #[test]
    fn source_capsule_convex_faces_match_native_cylinder_and_endcap() {
        use crate::extensions::source_contact_epa::SourceEpaSettings;
        use crate::extensions::source_contact_gjk::source_capsule_convex_gjk;
        let sample: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3b3e/endcap.native-sample.json")).unwrap();
        let body = &sample["input"]["body_translation_cm"];
        let body = DVec3::new(body[0].as_f64().unwrap(), body[2].as_f64().unwrap(), body[1].as_f64().unwrap()) * 0.01;
        let first = body + point(&sample["geometry"]["endpoint_0_cm"]) * 0.01;
        let second = body + point(&sample["geometry"]["endpoint_1_cm"]) * 0.01;
        let length = first.distance(second);
        let axis = (second - first) / length;
        let (epsilon, iterations) = include!("../als/settings/contact_gjk.rs");
        let (face_area_m4, initial_edge_m2, visibility_m, witness_m, max_iterations, max_flood, relative_epsilon) =
            include!("../als/settings/contact_epa.rs");
        let epa = SourceEpaSettings { face_area_m4, initial_edge_m2, visibility_m, witness_m,
            max_iterations, max_flood, relative_epsilon };
        for (group_index, shape_index, cylinder) in [(0, 0, true), (1, 2, false)] {
            let convex = source_convex_topology("SM_Als_Arch_01", shape_index).unwrap();
            let vertices = convex.vertices_m.iter().copied().map(DVec3::from_array).collect::<Vec<_>>();
            let gjk = source_capsule_convex_gjk(&vertices, DVec3::ONE, first, axis, length, epsilon, iterations, epa).unwrap();
            let selected = source_capsule_convex_face(convex, DVec3::ONE, &gjk, axis).unwrap();
            assert_eq!(selected.cylinder, cylinder);
            if !cylinder {
                assert!(selected.plane_contact && selected.auxiliary);
                let normal = point(&sample["aggregate_contacts"][group_index]["points"][0]["world_normal"]);
                assert!(selected.face.normal.distance(normal) < 2e-5);
            }
        }
    }

    #[test]
    fn source_capsule_contact_flags_keep_strict_source_boundaries() {
        let axis = |dot: f64| DVec3::new((1.0 - dot * dot).sqrt(), dot, 0.0);
        let normal = DVec3::Y;
        assert!(source_capsule_contact_flags(axis(0.009), normal, normal).0);
        assert!(!source_capsule_contact_flags(axis(0.011), normal, normal).0);
        assert!(source_capsule_contact_flags(axis(0.706), normal, normal).2);
        assert!(!source_capsule_contact_flags(axis(0.708), normal, normal).2);
        assert!(!source_capsule_contact_flags(axis(0.5), normal, -normal).1);
    }

    #[test]
    fn source_runtime_convex_topology_matches_native_bench() {
        let sample: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d143/bench-foot.native-sample.json")).unwrap();
        let native = &sample["other_geometry"];
        let topology = source_convex_topology("SM_Als_Bench", 2).unwrap();
        let vertices = native["convex_core_vertices_cm"].as_array().unwrap();
        let planes = native["convex_core_planes"].as_array().unwrap();
        assert_eq!(topology.vertices_m.len(), vertices.len());
        assert_eq!(topology.faces.len(), planes.len());
        for (actual, expected) in topology.vertices_m.iter().zip(vertices) {
            assert!(DVec3::from_array(*actual).distance(point(expected) * 0.01) < 1e-12);
        }
        for (actual, expected) in topology.faces.iter().zip(planes) {
            assert!(DVec3::from_array(actual.normal).distance(point(&expected["normal"])) < 1e-12);
            assert!(DVec3::from_array(actual.origin_m).distance(point(&expected["origin_cm"]) * 0.01) < 1e-12);
            assert_eq!(actual.vertices, expected["vertex_indices"].as_array().unwrap().iter()
                .map(|index| index.as_u64().unwrap() as usize).collect::<Vec<_>>());
        }
        assert!((topology.margin_m - native["margin_cm"].as_f64().unwrap() * 0.01).abs() < 1e-12);
        assert_eq!(topology.winding, -1.0);
        assert_eq!(SOURCE_CONVEX_TOPOLOGY.values().map(Vec::len).sum::<usize>(), 15);
        for hull in SOURCE_CONVEX_TOPOLOGY.values().flatten() {
            assert_eq!(hull.vertices_m.len(), hull.vertex_planes.len());
            assert!(hull.vertex_planes.iter().all(|adjacency| adjacency.planes.len() <= 3
                && adjacency.planes.iter().all(|index| *index < hull.faces.len()) && adjacency.count >= adjacency.planes.len()));
        }
        assert!(source_convex_topology("missing", 0).is_none());
        assert!(source_convex_topology("SM_Als_Bench", 99).is_none());
    }

    #[test]
    fn source_face_selection_preserves_candidate_order_and_box_ties() {
        let face = SourceContactFace { normal: DVec3::NEG_X, origin: DVec3::ZERO };
        let faces = [face, face, SourceContactFace { normal: DVec3::NEG_Y, origin: DVec3::ZERO }];
        assert_eq!(source_select_contact_plane(&faces, &[1, 0], 2, DVec3::ZERO,
            DVec3::X, 0.0, 0.01, None, false), Some(1));
        assert_eq!(source_select_contact_plane(&faces, &[1, 0, 2], 4, DVec3::ZERO,
            DVec3::X, 0.0, 0.01, None, false), Some(0));
        assert_eq!(source_select_contact_plane(&faces, &[1, 0], 2, DVec3::ZERO,
            DVec3::X, 0.0, 0.01, Some(DVec3::splat(2.0)), false), Some(1));
        assert_eq!(source_select_contact_plane(&faces, &[0], 1, DVec3::ZERO,
            DVec3::NEG_X, 0.0, 0.01, Some(DVec3::new(-1.0, 1.0, 1.0)), false), Some(0));
        let far = [SourceContactFace { normal: DVec3::NEG_X, origin: DVec3::splat(10.0) }; 6];
        assert_eq!(source_select_contact_plane(&far, &[0], 1, DVec3::ZERO,
            DVec3::new(1.0, 0.0, 1.0), 0.0, 0.01, None, true), Some(1));
    }

    #[test]
    fn source_box_convex_face_classification_respects_bias_and_scale() {
        use crate::extensions::source_gjk_simplex::SourceGjkSimplex;
        let hull = SourceConvexTopology { vertices_m: vec![[-1.0, 0.0, 0.0]],
            faces: vec![SourceConvexPlane { normal: [-1.0, 0.0, 0.0], origin_m: [-1.0, 0.0, 0.0], vertices: vec![0] }],
            vertex_planes: vec![SourceVertexPlanes { count: 1, planes: vec![0] }],
            margin_m: 0.0, winding: -1.0, bounds_m: [DVec3::NEG_X; 2] };
        let mut gjk = SourceGjkResult { simplex_cm: SourceGjkSimplex::default(), last_vertices: [7, 0],
            normal_first: DVec3::X, normal_second: DVec3::X, closest_first_m: DVec3::X,
            closest_second_m: DVec3::NEG_X, penetration_m: 0.0, max_support_delta_m: 0.0, epa_status: None };
        let [epsilon, bias] = include!("../als/settings/contact_face_classification.rs");
        let select = |gjk: &SourceGjkResult, scale| source_box_convex_faces(DVec3::splat(-1.0), DVec3::ONE,
            &hull, scale, gjk, 0.0, 0.01, epsilon, bias).unwrap();
        let faces = select(&gjk, DVec3::ONE);
        assert_eq!((faces.box_index, faces.convex_index), (3, 0));
        assert!(!faces.reference_box && faces.plane_contact);
        gjk.normal_second = (DVec3::X + DVec3::Y).normalize();
        assert!(select(&gjk, DVec3::ONE).reference_box);
        gjk.normal_first = gjk.normal_second;
        assert!(!select(&gjk, DVec3::ONE).plane_contact);
        gjk.normal_first = DVec3::X;
        gjk.normal_second = DVec3::NEG_X;
        gjk.closest_second_m = DVec3::X;
        let reflected = select(&gjk, DVec3::new(-1.0, 1.0, 1.0));
        assert_eq!(reflected.convex_face.normal, DVec3::X);
        assert_eq!(reflected.convex_face.origin, DVec3::X);
        let minimum = f64::from(SOURCE_KINDA_SMALL_NUMBER);
        assert_eq!(source_convex_scale(DVec3::new(0.0, -0.00001, -1.0)), DVec3::new(minimum, minimum, -1.0));
    }

    #[test]
    fn source_box_convex_faces_match_native_bench_candidates() {
        use crate::extensions::source_gjk_simplex::SourceGjkSimplex;
        let sample: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d143/bench-foot.native-sample.json")).unwrap();
        let simplex = sample["gjk_simplex"].as_array().unwrap();
        let weighted = |key: &str| simplex.iter().fold(DVec3::ZERO, |sum, value| {
            sum + point(&value[key]) * value["weight"].as_f64().unwrap()
        }) * 0.01;
        let normal = point(&sample["points"][0]["world_normal"]);
        let margin = sample["collision_margin_0_cm"].as_f64().unwrap() * 0.01;
        let gjk = SourceGjkResult { simplex_cm: SourceGjkSimplex::default(), last_vertices: [3, 10],
            normal_first: -normal, normal_second: -normal, closest_first_m: weighted("shape_point_0_cm") - normal * margin,
            closest_second_m: weighted("shape_point_1_cm"), penetration_m: 0.0, max_support_delta_m: 0.0, epa_status: None };
        let minimum = point(&sample["geometry"]["minimum_cm"]) * 0.01;
        let maximum = point(&sample["geometry"]["maximum_cm"]) * 0.01;
        let [epsilon, bias] = include!("../als/settings/contact_face_classification.rs");
        let faces = source_box_convex_faces(minimum, maximum, source_convex_topology("SM_Als_Bench", 2).unwrap(),
            DVec3::ONE, &gjk, margin, include!("../als/settings/contact_face_search.rs"), epsilon, bias).unwrap();
        assert_eq!((faces.box_index, faces.convex_index), (2, 10));
        assert!(faces.reference_box && faces.plane_contact);
    }

    #[test]
    fn source_box_convex_faces_match_direct_native_gjk_when_available() {
        use crate::extensions::source_gjk_simplex::SourceGjkSimplex;
        let Ok(path) = std::env::var("ALS_SOURCE_FACE_CLASSIFICATION_REPORT")
            .or_else(|_| std::env::var("ALS_SOURCE_COLD_GJK_REPORT")) else { return; };
        let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let [epsilon, bias] = include!("../als/settings/contact_face_classification.rs");
        for sample in report["physics_contact_probe"]["samples"].as_array().unwrap() {
            let contacts = sample["points"].as_array().unwrap();
            if contacts.is_empty() { continue; }
            let raw = &sample["cold_gjk"];
            let gjk = SourceGjkResult { simplex_cm: SourceGjkSimplex::default(),
                last_vertices: [raw["vertex_0"].as_u64().unwrap() as usize, raw["vertex_1"].as_u64().unwrap() as usize],
                normal_first: point(&raw["shape_normal_0"]), normal_second: point(&raw["shape_normal_1"]),
                closest_first_m: point(&raw["shape_point_0_cm"]) * 0.01,
                closest_second_m: point(&raw["shape_point_1_cm"]) * 0.01,
                penetration_m: raw["penetration_cm"].as_f64().unwrap() * 0.01, max_support_delta_m: 0.0, epa_status: None };
            let minimum = point(&sample["geometry"]["minimum_cm"]) * 0.01;
            let maximum = point(&sample["geometry"]["maximum_cm"]) * 0.01;
            let scale = point(&sample["other_geometry"]["convex_core_scale"]);
            let margin = sample["collision_margin_0_cm"].as_f64().unwrap() * 0.01;
            let name = sample["input"]["mesh_asset_path"].as_str().unwrap().rsplit('/').next().unwrap();
            let index = sample["input"]["mesh_shape_index"].as_u64().unwrap() as u32;
            let faces = source_box_convex_faces(minimum, maximum, source_convex_topology(name, index).unwrap(),
                scale, &gjk, margin, include!("../als/settings/contact_face_search.rs"), epsilon, bias).unwrap();
            let contact_type = contacts[0]["contact_type"].as_i64().unwrap();
            assert_eq!(faces.plane_contact, contact_type != 2, "{}", sample["input"]["label"]);
            if faces.plane_contact {
                assert_eq!(faces.reference_box, contact_type == 3, "{}", sample["input"]["label"]);
                let (face, key) = if faces.reference_box { (faces.box_face, "shape_point_0_cm") }
                    else { (faces.convex_face, "shape_point_1_cm") };
                for contact in contacts {
                    assert!(((point(&contact[key]) * 0.01 - face.origin).dot(face.normal)).abs() < 2e-5,
                        "{}", sample["input"]["label"]);
                }
            }
        }
    }

    #[test]
    fn source_bench_simplex_selects_native_box_and_convex_faces() {
        let sample: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d143/bench-foot.native-sample.json")).unwrap();
        let topology: serde_json::Value = serde_json::from_str(include_str!(
            "../../assets/ALSExtras/Environment/Meshes/static_convex_topology.runtime.json")).unwrap();
        let shape = &topology["SM_Als_Bench"][2];
        let native = &sample["other_geometry"]["convex_core_vertices_cm"];
        let simplex = sample["gjk_simplex"].as_array().unwrap();
        assert_eq!(native[10], simplex[0]["shape_point_1_cm"]);
        let weighted = |key: &str| simplex.iter().fold(DVec3::ZERO, |sum, value| {
            sum + point(&value[key]) * value["weight"].as_f64().unwrap()
        }) * 0.01;
        let normal = point(&sample["points"][0]["world_normal"]);
        let margin = sample["collision_margin_0_cm"].as_f64().unwrap() * 0.01;
        let lower = point(&sample["geometry"]["minimum_cm"]) * 0.01;
        let upper = point(&sample["geometry"]["maximum_cm"]) * 0.01;
        let box_faces = (0..6).map(|index| {
            let (vertices, normal) = source_box_face(lower, upper, index);
            SourceContactFace { origin: vertices[0], normal }
        }).collect::<Vec<_>>();
        let minimum = f64::from(include!("../als/settings/contact_face_search.rs"));
        assert_eq!(source_select_contact_plane(&box_faces, &SOURCE_BOX_VERTEX_PLANES[3], 3,
            weighted("shape_point_0_cm") - normal * margin, normal, margin, minimum, None, true), Some(2));
        let convex_faces = shape["faces"].as_array().unwrap().iter().map(|face| SourceContactFace {
            origin: array(&face["origin_m"]), normal: array(&face["normal"]),
        }).collect::<Vec<_>>();
        let adjacency = &shape["vertex_planes"][10];
        let indices = adjacency["planes"].as_array().unwrap().iter()
            .map(|value| value.as_u64().unwrap() as usize).collect::<Vec<_>>();
        assert_eq!(source_select_contact_plane(&convex_faces, &indices,
            adjacency["count"].as_u64().unwrap() as usize, weighted("shape_point_1_cm"),
            -normal, 0.0, minimum, None, false), Some(10));
    }
}
