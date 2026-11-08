use bevy::math::{DVec3, Vec3};

use super::source_contact_polygon::source_position;

#[derive(serde::Deserialize)]
pub(super) struct SourceMeshBvh {
    pub face_bounds_m: Vec<[[f64; 3]; 2]>,
    pub nodes: Vec<[Option<SourceMeshBvhChild>; 2]>,
}

#[derive(serde::Deserialize)]
pub(super) struct SourceMeshBvhChild {
    pub index: usize,
    pub face_count: usize,
    pub min_m: [f64; 3],
    pub max_m: [f64; 3],
}

fn source_mesh_bounds_intersect(bounds_m: [[f64; 3]; 2], query_cm: [Vec3; 2]) -> bool {
    let minimum = source_position(DVec3::from_array(bounds_m[0])).as_vec3();
    let maximum = source_position(DVec3::from_array(bounds_m[1])).as_vec3();
    !minimum.cmpgt(query_cm[1]).any() && !query_cm[0].cmpgt(maximum).any()
}

pub(super) fn source_mesh_bvh_faces(bvh: &SourceMeshBvh, query_cm: [DVec3; 2]) -> Option<Vec<usize>> {
    if bvh.nodes.is_empty() { return Some(Vec::new()); }
    let query = query_cm.map(|point| point.as_vec3());
    let mut stack = vec![0];
    let mut visited = vec![false; bvh.nodes.len()];
    let mut faces = Vec::new();
    while let Some(index) = stack.pop() {
        let seen = visited.get_mut(index)?;
        if *seen { return None; }
        *seen = true;
        for child in bvh.nodes.get(index)?.iter().flatten() {
            if !source_mesh_bounds_intersect([child.min_m, child.max_m], query) { continue; }
            if child.face_count == 0 {
                stack.push(child.index);
            } else {
                let end = child.index.checked_add(child.face_count)?;
                for face in child.index..end {
                    if source_mesh_bounds_intersect(*bvh.face_bounds_m.get(face)?, query) { faces.push(face); }
                }
            }
        }
    }
    Some(faces)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> SourceMeshBvh {
        let child = |index, count, min, max| serde_json::json!({"index": index, "face_count": count,
            "min_m": [min, 0.0, 0.0], "max_m": [max, 1.0, 1.0]});
        serde_json::from_value(serde_json::json!({
            "nodes": [[child(1, 0, 0.0, 0.5), child(2, 1, 0.5, 1.0)],
                      [child(0, 1, 0.0, 0.25), child(1, 1, 0.25, 0.5)]],
            "face_bounds_m": [[[0.0, 0.0, 0.0], [0.25, 1.0, 1.0]],
                              [[0.25, 0.0, 0.0], [0.5, 1.0, 1.0]],
                              [[0.5, 0.0, 0.0], [1.0, 1.0, 1.0]]]
        })).unwrap()
    }

    #[test]
    fn source_mesh_bvh_preserves_stack_order_and_float_boundaries() {
        let bvh = fixture();
        let query = |min_x, max_x| [DVec3::new(min_x, 0.0, 0.0), DVec3::new(max_x, 100.0, 100.0)];
        assert_eq!(source_mesh_bvh_faces(&bvh, query(0.0, 100.0)), Some(vec![2, 0, 1]));
        assert_eq!(source_mesh_bvh_faces(&bvh, query(0.0, 24.0)), Some(vec![0]));
        assert_eq!(source_mesh_bvh_faces(&bvh, query(0.0, 25.0 - 1e-7)), Some(vec![0, 1]));
        assert_eq!(source_mesh_bvh_faces(&bvh, query(26.0, 49.0)), Some(vec![1]));
        assert_eq!(source_mesh_bvh_faces(&bvh, query(101.0, 102.0)), Some(Vec::new()));
    }

    #[test]
    fn source_mesh_bvh_filters_nodes_and_rejects_corrupt_references() {
        let mut bvh = fixture();
        bvh.nodes[0][0].as_mut().unwrap().max_m[0] = 0.2;
        let query = [DVec3::new(30.0, 0.0, 0.0), DVec3::new(40.0, 100.0, 100.0)];
        assert_eq!(source_mesh_bvh_faces(&bvh, query), Some(Vec::new()));
        let mut cyclic = fixture();
        cyclic.nodes[0][0].as_mut().unwrap().index = 0;
        assert!(source_mesh_bvh_faces(&cyclic, [DVec3::ZERO, DVec3::splat(100.0)]).is_none());
        let mut missing = fixture();
        missing.face_bounds_m.truncate(2);
        assert!(source_mesh_bvh_faces(&missing, [DVec3::ZERO, DVec3::splat(100.0)]).is_none());
        let empty: SourceMeshBvh = serde_json::from_value(serde_json::json!({"nodes": [], "face_bounds_m": []})).unwrap();
        assert_eq!(source_mesh_bvh_faces(&empty, [DVec3::ZERO, DVec3::splat(100.0)]), Some(Vec::new()));
    }

    #[test]
    fn source_mesh_bvh_matches_native_queries_when_available() {
        let Ok(report_path) = std::env::var("ALS_MESH_BVH_NATIVE_REPORT") else { return; };
        let data_path = std::env::var("ALS_MESH_BVH_TARGET_DATA").expect("Source mesh BVH target data is required");
        let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(report_path).unwrap()).unwrap();
        let meshes: std::collections::HashMap<String, SourceMeshBvh> =
            serde_json::from_str(&std::fs::read_to_string(data_path).unwrap()).unwrap();
        for sample in report["physics_contact_probe"]["samples"].as_array().unwrap() {
            let input = &sample["input"];
            let name = input["mesh_asset_path"].as_str().unwrap().rsplit('/').next().unwrap();
            let bounds = &input["mesh_bvh_query_bounds_cm"];
            let point = |value: &serde_json::Value| DVec3::new(value[0].as_f64().unwrap(),
                value[1].as_f64().unwrap(), value[2].as_f64().unwrap());
            let query = [point(&bounds["min"]), point(&bounds["max"])];
            let expected = sample["other_geometry"]["trimesh_core_bvh_query_order"].as_array().unwrap().iter()
                .map(|face| face.as_u64().unwrap() as usize).collect::<Vec<_>>();
            assert_eq!(source_mesh_bvh_faces(&meshes[name], query), Some(expected), "{}", input["label"]);
        }
    }
}
