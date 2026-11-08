use bevy::math::DVec3;

use super::source_contact_polygon::{SOURCE_SMALL_NUMBER, source_direction, source_position};

pub(crate) const SOURCE_BOX_VERTEX_PLANES: [[usize; 3]; 8] = include!("source_box_vertex_planes.rs");

#[derive(Clone, Copy)]
pub(crate) struct SourceContactFace {
    pub normal: DVec3,
    pub origin: DVec3,
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
    use crate::extensions::source_contact_polygon::source_box_face;

    fn point(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value["x"].as_f64().unwrap(), value["z"].as_f64().unwrap(), value["y"].as_f64().unwrap())
    }

    fn array(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value[0].as_f64().unwrap(), value[1].as_f64().unwrap(), value[2].as_f64().unwrap())
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
