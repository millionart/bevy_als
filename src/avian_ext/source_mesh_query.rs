use bevy::math::{DQuat, DVec3};

use super::source_contact_polygon::{source_direction, source_position};
use super::source_mesh_bvh::{source_mesh_bvh_faces, SourceMeshBvh};

#[derive(serde::Deserialize)]
pub(super) struct SourceMeshTopology {
    pub vertices_m: Vec<[f64; 3]>,
    pub triangles: Vec<[usize; 3]>,
}

pub(super) struct SourceMeshQuery {
    pub object_bounds_cm: [DVec3; 2],
    pub mesh_bounds_cm: [DVec3; 2],
    translation_cm: DVec3,
    rotation: DQuat,
    scale: DVec3,
}

impl SourceMeshQuery {
    pub(super) fn inverse_contact_no_scale(&self, point_cm: DVec3, normal: DVec3) -> (DVec3, DVec3) {
        let inverse = self.rotation.inverse();
        (inverse * (point_cm - self.translation_cm), inverse * normal)
    }
}

pub(super) struct SourceMeshTriangle {
    pub face_index: usize,
    pub vertex_indices: [usize; 3],
    pub vertices_m: [DVec3; 3],
}

pub(super) fn source_mesh_query(
    bounds_m: [DVec3; 2], translation_m: DVec3, rotation: DQuat, scale: DVec3, cull_m: f64,
) -> SourceMeshQuery {
    let cull = DVec3::splat(cull_m * 100.0);
    let object_bounds_cm = [source_position(bounds_m[0]) - cull, source_position(bounds_m[1]) + cull];
    let translation_cm = source_position(translation_m);
    let rotation = DQuat::from_xyzw(-rotation.x, -rotation.z, -rotation.y, rotation.w);
    let scale = source_direction(scale);
    let inverse_rotation = rotation.inverse();
    let inverse_scale = scale.recip();
    let extent = (object_bounds_cm[1] - object_bounds_cm[0]) * 0.5;
    let mut transformed_extent = DVec3::ZERO;
    for point in [extent, DVec3::new(extent.x, extent.y, -extent.z),
        DVec3::new(extent.x, -extent.y, extent.z), DVec3::new(extent.x, -extent.y, -extent.z)] {
        transformed_extent = transformed_extent.max((inverse_scale * (inverse_rotation * point)).abs());
    }
    let center = (object_bounds_cm[0] + object_bounds_cm[1]) * 0.5;
    let transformed_center = inverse_scale * (inverse_rotation * (center - translation_cm));
    SourceMeshQuery { object_bounds_cm,
        mesh_bounds_cm: [transformed_center - transformed_extent, transformed_center + transformed_extent],
        translation_cm, rotation, scale }
}

pub(super) fn source_mesh_collect(
    topology: &SourceMeshTopology, bvh: &SourceMeshBvh, query: &SourceMeshQuery,
) -> Option<Vec<SourceMeshTriangle>> {
    let mut output = Vec::new();
    for face_index in source_mesh_bvh_faces(bvh, query.mesh_bounds_cm)? {
        let mut vertex_indices = *topology.triangles.get(face_index)?;
        if query.scale.x * query.scale.y * query.scale.z < 0.0 { vertex_indices.swap(1, 2); }
        let [Some(a), Some(b), Some(c)] = vertex_indices.map(|index| topology.vertices_m.get(index).copied()) else {
            return None;
        };
        let vertices = [a, b, c].map(|point| {
            let source_vertex = source_position(DVec3::from_array(point)).as_vec3().as_dvec3();
            query.translation_cm + query.rotation * (query.scale * source_vertex)
        });
        let minimum = vertices[0].min(vertices[1]).min(vertices[2]);
        let maximum = vertices[0].max(vertices[1]).max(vertices[2]);
        if !maximum.cmplt(query.object_bounds_cm[0]).any() && !minimum.cmpgt(query.object_bounds_cm[1]).any() {
            output.push(SourceMeshTriangle { face_index, vertex_indices,
                vertices_m: vertices.map(|point| source_direction(point) * 0.01) });
        }
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use bevy::math::{DQuat, DVec3};

    use super::*;
    use crate::extensions::source_mesh_bvh::SourceMeshBvh;

    fn fixture() -> (SourceMeshTopology, SourceMeshBvh) {
        let topology = serde_json::from_value(serde_json::json!({
            "vertices_m": [[0.1, 0.1, 0.1], [0.2, 0.1, 0.1], [0.1, 0.2, 0.1],
                           [0.8, 0.1, 0.1], [0.9, 0.1, 0.1], [0.8, 0.2, 0.1],
                           [0.3, 0.1, 0.1], [0.4, 0.1, 0.1], [0.3, 0.2, 0.1]],
            "triangles": [[0, 1, 2], [3, 4, 5], [6, 7, 8]]
        })).unwrap();
        let child = |index, count| serde_json::json!({"index": index, "face_count": count,
            "min_m": [0.0, 0.0, 0.0], "max_m": [1.0, 1.0, 1.0]});
        let bvh = serde_json::from_value(serde_json::json!({
            "nodes": [[child(1, 0), child(2, 1)], [child(0, 1), child(1, 1)]],
            "face_bounds_m": [[[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]],
                              [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]],
                              [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]]
        })).unwrap();
        (topology, bvh)
    }

    #[test]
    fn query_bounds_match_all_inverse_transformed_corners() {
        let bounds = [DVec3::new(-0.3, -0.2, 0.1), DVec3::new(0.4, 0.5, 0.8)];
        let translation = DVec3::new(0.7, -0.6, 0.25);
        let rotation = DQuat::from_rotation_z(0.71) * DQuat::from_rotation_x(-0.24);
        let scale = DVec3::new(-2.0, 0.5, 1.5);
        let cull = 0.03;
        let query = source_mesh_query(bounds, translation, rotation, scale, cull);
        let minimum = source_position(bounds[0]) - DVec3::splat(cull * 100.0);
        let maximum = source_position(bounds[1]) + DVec3::splat(cull * 100.0);
        let source_rotation = DQuat::from_xyzw(-rotation.x, -rotation.z, -rotation.y, rotation.w);
        let mut expected = [DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)];
        for i in 0..8 {
            let corner = DVec3::new(if i & 1 == 0 { minimum.x } else { maximum.x },
                if i & 2 == 0 { minimum.y } else { maximum.y },
                if i & 4 == 0 { minimum.z } else { maximum.z });
            let local = source_direction(scale).recip() * (source_rotation.inverse() * (corner - source_position(translation)));
            expected = [expected[0].min(local), expected[1].max(local)];
        }
        assert!(query.mesh_bounds_cm[0].distance(expected[0]) < 1e-10);
        assert!(query.mesh_bounds_cm[1].distance(expected[1]) < 1e-10);
        assert_eq!(query.object_bounds_cm, [minimum, maximum]);
    }

    #[test]
    fn collect_filters_transformed_triangles_and_preserves_bvh_order() {
        let (topology, bvh) = fixture();
        let query = source_mesh_query([DVec3::ZERO, DVec3::splat(0.3)], DVec3::ZERO,
            DQuat::IDENTITY, DVec3::ONE, 0.0);
        let triangles = source_mesh_collect(&topology, &bvh, &query).unwrap();
        assert_eq!(triangles.iter().map(|triangle| triangle.face_index).collect::<Vec<_>>(), vec![2, 0]);
        assert_eq!(triangles[0].vertex_indices, [6, 7, 8]);
        assert_eq!(triangles[1].vertex_indices, [0, 1, 2]);
        let query = source_mesh_query([DVec3::new(-0.3, 0.0, 0.0), DVec3::new(0.0, 0.3, 0.3)],
            DVec3::ZERO, DQuat::IDENTITY, DVec3::new(-1.0, 1.0, 1.0), 0.0);
        let triangles = source_mesh_collect(&topology, &bvh, &query).unwrap();
        assert_eq!(triangles.iter().map(|triangle| triangle.face_index).collect::<Vec<_>>(), vec![2, 0]);
        assert_eq!(triangles[0].vertex_indices, [6, 8, 7]);
        assert_eq!(triangles[1].vertex_indices, [0, 2, 1]);
        assert_eq!(triangles[1].vertices_m[1], DVec3::new(-0.1, 0.2, 0.1));
    }

    #[test]
    fn collect_rejects_missing_topology() {
        let (mut topology, bvh) = fixture();
        topology.vertices_m.clear();
        let query = source_mesh_query([DVec3::ZERO, DVec3::ONE], DVec3::ZERO,
            DQuat::IDENTITY, DVec3::ONE, 0.0);
        assert!(source_mesh_collect(&topology, &bvh, &query).is_none());
    }

    #[test]
    fn query_matches_native_bounds_and_collected_triangles_when_available() {
        let Ok(report_path) = std::env::var("ALS_MESH_QUERY_NATIVE_REPORT") else { return; };
        let bvh_path = std::env::var("ALS_MESH_BVH_TARGET_DATA").expect("Source mesh BVH target data is required");
        let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(report_path).unwrap()).unwrap();
        let bvh: std::collections::HashMap<String, SourceMeshBvh> =
            serde_json::from_str(&std::fs::read_to_string(bvh_path).unwrap()).unwrap();
        let topology: std::collections::HashMap<String, SourceMeshTopology> = serde_json::from_str(include_str!(
            "../../assets/ALSExtras/Environment/Meshes/static_trimesh_topology.runtime.json")).unwrap();
        let point = |value: &serde_json::Value| DVec3::new(value["x"].as_f64().unwrap(),
            value["y"].as_f64().unwrap(), value["z"].as_f64().unwrap());
        let mut count = 0;
        for sample in report["physics_contact_probe"]["samples"].as_array().unwrap() {
            let input = &sample["input"];
            let native = &sample["mesh_triangle_query"];
            let transform = &native["mesh_to_convex"];
            let q = &transform["rotation"];
            let rotation = DQuat::from_xyzw(-q["x"].as_f64().unwrap(), -q["z"].as_f64().unwrap(),
                -q["y"].as_f64().unwrap(), q["w"].as_f64().unwrap());
            let bounds = [source_direction(point(&native["convex_min_cm"])) * 0.01,
                source_direction(point(&native["convex_max_cm"])) * 0.01];
            let query = source_mesh_query(bounds, source_direction(point(&transform["translation"])) * 0.01,
                rotation, source_direction(point(&transform["scale"])), input["cull_distance_cm"].as_f64().unwrap() * 0.01);
            for (actual, key) in query.object_bounds_cm.into_iter().zip(["object_min_cm", "object_max_cm"]) {
                assert!(actual.distance(point(&native[key])) < 1e-7, "{} {key}", input["label"]);
            }
            for (actual, key) in query.mesh_bounds_cm.into_iter().zip(["mesh_min_cm", "mesh_max_cm"]) {
                assert!(actual.distance(point(&native[key])) < 1e-7, "{} {key}", input["label"]);
            }
            let name = input["mesh_asset_path"].as_str().unwrap().rsplit('/').next().unwrap();
            let actual = source_mesh_collect(&topology[name], &bvh[name], &query).unwrap();
            let expected = native["triangles"].as_array().unwrap();
            assert_eq!(actual.len(), expected.len(), "{}", input["label"]);
            count += actual.len();
            for (triangle, expected) in actual.iter().zip(expected) {
                assert_eq!(triangle.face_index, expected["face_index"].as_u64().unwrap() as usize, "{}", input["label"]);
                for (index, vertex) in triangle.vertex_indices.iter().zip(expected["vertex_indices"].as_array().unwrap()) {
                    assert_eq!(*index, vertex.as_u64().unwrap() as usize, "{}", input["label"]);
                }
                for (vertex, expected) in triangle.vertices_m.iter().zip(expected["vertices_cm"].as_array().unwrap()) {
                    assert!(vertex.distance(source_direction(point(expected)) * 0.01) < 1e-8, "{}", input["label"]);
                }
            }
        }
        assert!(count > 0, "Native mesh query must include triangles");
    }
}
