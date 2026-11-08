use bevy::math::{DQuat, DVec3, Vec3};

use super::source_contact_polygon::{source_direction, source_position};
use super::source_gjk_simplex::{SourceGjkSimplex, SourceGjkVertex};

#[derive(Clone)]
pub(crate) struct SourceGjkSeparation {
    pub simplex_cm: SourceGjkSimplex,
    pub last_vertices: [usize; 2],
    pub normal_first: DVec3,
    pub normal_second: DVec3,
    pub closest_first_m: DVec3,
    pub closest_second_m: DVec3,
    pub penetration_m: f64,
}

#[derive(Clone)]
pub(crate) enum SourceGjkResult {
    Separated(SourceGjkSeparation),
    NeedsEpa { simplex_cm: SourceGjkSimplex, last_vertices: [usize; 2] },
}

pub(super) fn source_box_support(minimum: DVec3, maximum: DVec3, margin: f64, direction: DVec3) -> (DVec3, usize) {
    let mut point = DVec3::ZERO;
    let mut index = 0;
    for axis in 0..3 {
        if direction[axis] < 0.0 { point[axis] = minimum[axis] + margin; }
        else { point[axis] = maximum[axis] - margin; index |= 1 << axis; }
    }
    (point, index)
}

pub(super) fn source_convex_support(vertices: &[Vec3], scale: DVec3, direction: DVec3) -> Option<(DVec3, usize)> {
    let direction = (scale * direction).as_vec3();
    let mut maximum = f32::MIN;
    let mut selected = None;
    for (index, point) in vertices.iter().enumerate() {
        let dot = point.x * direction.x + point.y * direction.y + point.z * direction.z;
        if dot > maximum { maximum = dot; selected = Some(index); }
    }
    selected.map(|index| (vertices[index].as_dvec3() * scale, index))
}

pub(crate) fn source_box_convex_gjk(
    box_minimum: DVec3,
    box_maximum: DVec3,
    box_margin: f64,
    convex_vertices: &[DVec3],
    convex_scale: DVec3,
    relative_translation: DVec3,
    relative_rotation: DQuat,
    epsilon: f64,
    iteration_limit: usize,
    previous: Option<SourceGjkSimplex>,
) -> Option<SourceGjkResult> {
    let minimum = source_position(box_minimum);
    let maximum = source_position(box_maximum);
    let margin = f64::from((box_margin * 100.0) as f32);
    let vertices = convex_vertices.iter().copied().map(|point| source_position(point).as_vec3()).collect::<Vec<_>>();
    let scale = source_direction(convex_scale);
    let translation = source_position(relative_translation);
    let rotation = DQuat::from_xyzw(-relative_rotation.x, -relative_rotation.z, -relative_rotation.y, relative_rotation.w);
    let inverse = rotation.inverse();
    let epsilon = epsilon * 100.0;
    let mut simplex = previous.unwrap_or_default();
    let mut direction = DVec3::NEG_X;
    let mut distance = f32::MAX as f64;
    if simplex.len > 0 {
        for vertex in &mut simplex.vertices[..simplex.len] {
            vertex.difference = vertex.first - (rotation * vertex.second + translation);
        }
        let projected = simplex.project();
        let restored_distance = projected.length();
        if restored_distance > epsilon {
            direction = projected / restored_distance;
            distance = restored_distance;
        } else {
            simplex.len = 0;
        }
    }
    let mut normal = -direction;
    let mut last_vertices = [0; 2];
    for _iteration in 1..iteration_limit {
        let (first, first_index) = source_box_support(minimum, maximum, margin, -direction);
        let (second, second_index) = source_convex_support(&vertices, scale, inverse * direction)?;
        last_vertices = [first_index, second_index];
        let difference = first - (rotation * second + translation);
        simplex.vertices[simplex.len] = SourceGjkVertex { difference, first, second };
        simplex.len += 1;
        let projected = simplex.project();
        let next_distance = projected.length();
        if next_distance < epsilon { return Some(SourceGjkResult::NeedsEpa { simplex_cm: simplex, last_vertices }); }
        let degenerate = next_distance >= distance;
        direction = projected / next_distance;
        normal = -direction;
        distance = next_distance;
        if degenerate { break; }
    }
    if simplex.len == 0 { return None; }
    let mut first = DVec3::ZERO;
    let mut second = DVec3::ZERO;
    for index in 0..simplex.len {
        first += simplex.vertices[index].first * simplex.weights[index];
        second += simplex.vertices[index].second * simplex.weights[index];
    }
    first += normal * margin;
    Some(SourceGjkResult::Separated(SourceGjkSeparation {
        simplex_cm: simplex,
        last_vertices,
        normal_first: source_direction(normal),
        normal_second: source_direction(inverse * normal),
        closest_first_m: source_direction(first) * 0.01,
        closest_second_m: source_direction(second) * 0.01,
        penetration_m: (margin - distance) * 0.01,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_point(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap(), value["z"].as_f64().unwrap())
    }

    fn runtime_point(value: DVec3) -> DVec3 {
        source_direction(value) * 0.01
    }

    fn fixture(environment: &str, fallback: &str) -> (serde_json::Value, bool) {
        if let Ok(path) = std::env::var(environment) {
            let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let samples = report["physics_contact_probe"]["samples"].as_array().unwrap();
            let first = &samples[0];
            (serde_json::json!({"geometry": first["geometry"], "other_geometry": first["other_geometry"],
                "collision_margin_0_cm": first["collision_margin_0_cm"], "samples": samples}), true)
        } else {
            (serde_json::from_str(fallback).unwrap(), false)
        }
    }

    fn assert_native_result(actual: &SourceGjkSeparation, expected: &serde_json::Value) {
        assert_eq!(actual.last_vertices, [expected["vertex_0"].as_u64().unwrap() as usize,
            expected["vertex_1"].as_u64().unwrap() as usize]);
        assert!(actual.normal_first.distance(source_direction(source_point(&expected["shape_normal_0"]))) < 1e-6);
        assert!(actual.normal_second.distance(source_direction(source_point(&expected["shape_normal_1"]))) < 1e-6);
        assert!(actual.closest_first_m.distance(runtime_point(source_point(&expected["shape_point_0_cm"]))) < 1e-7);
        assert!(actual.closest_second_m.distance(runtime_point(source_point(&expected["shape_point_1_cm"]))) < 1e-7);
        assert!((actual.penetration_m - expected["penetration_cm"].as_f64().unwrap() * 0.01).abs() < 1e-7);
    }

    #[test]
    fn source_support_vertices_keep_source_tie_order() {
        let vertices = [Vec3::new(1.0, 2.0, 0.0), Vec3::new(1.0, -1.0, 0.0)];
        assert_eq!(source_convex_support(&vertices, DVec3::ONE, DVec3::X).unwrap().1, 0);
        assert_eq!(source_convex_support(&vertices, DVec3::ONE, DVec3::NEG_Y).unwrap().1, 1);
        let (point, index) = source_box_support(DVec3::splat(-2.0), DVec3::splat(2.0), 0.5, DVec3::ZERO);
        assert_eq!(point, DVec3::splat(1.5));
        assert_eq!(index, 7);
    }

    #[test]
    fn source_cold_box_convex_gjk_matches_native_bench_simplex() {
        let (fixture, direct) = fixture("ALS_SOURCE_COLD_GJK_REPORT", include_str!(
            "../../docs/audit/bf144/d100b3b3b2b/gjk.native-samples.json"));
        let geometry = &fixture["geometry"];
        let minimum = runtime_point(source_point(&geometry["minimum_cm"]));
        let maximum = runtime_point(source_point(&geometry["maximum_cm"]));
        let margin = fixture["collision_margin_0_cm"].as_f64().unwrap() * 0.01;
        let vertices = fixture["other_geometry"]["convex_core_vertices_cm"].as_array().unwrap().iter()
            .map(|value| runtime_point(source_point(value))).collect::<Vec<_>>();
        let (epsilon, iterations) = include!("../als/settings/contact_gjk.rs");
        for sample in fixture["samples"].as_array().unwrap() {
            let input = &sample["input"];
            let translation = &input["body_translation_cm"];
            let body = DVec3::new(translation[0].as_f64().unwrap(), translation[1].as_f64().unwrap(),
                translation[2].as_f64().unwrap());
            let result = source_box_convex_gjk(minimum, maximum, margin, &vertices, DVec3::ONE,
                -runtime_point(body), DQuat::IDENTITY, epsilon, iterations, None).unwrap();
            let native = sample["gjk_simplex"].as_array().unwrap();
            let simplex = match &result {
                SourceGjkResult::Separated(result) => {
                    assert_ne!(input["label"], "bench_foot_core_overlap");
                    let normal = source_direction(source_point(&sample["points"][0]["world_normal"]));
                    assert!(result.normal_first.distance(-normal) < 1e-6);
                    assert!(result.last_vertices[0] < 8 && result.last_vertices[1] < vertices.len());
                    if direct { assert_native_result(result, &sample["cold_gjk"]); }
                    &result.simplex_cm
                }
                SourceGjkResult::NeedsEpa { simplex_cm, last_vertices } => {
                    assert_eq!(input["label"], "bench_foot_core_overlap");
                    assert!(last_vertices[0] < 8 && last_vertices[1] < vertices.len());
                    simplex_cm
                }
            };
            assert_eq!(simplex.len, native.len());
            for (index, expected) in native.iter().enumerate() {
                assert!((simplex.weights[index] - expected["weight"].as_f64().unwrap()).abs() < 1e-7);
                assert!(simplex.vertices[index].first.distance(source_point(&expected["shape_point_0_cm"])) < 1e-6);
                assert!(simplex.vertices[index].second.distance(source_point(&expected["shape_point_1_cm"])) < 1e-6);
            }
        }
    }

    #[test]
    fn source_warm_gjk_matches_native_next_simplex() {
        let (fixture, direct) = fixture("ALS_SOURCE_WARM_GJK_REPORT", include_str!(
            "../../docs/audit/bf144/d100b3b3b3a/warm.native-samples.json"));
        let minimum = runtime_point(source_point(&fixture["geometry"]["minimum_cm"]));
        let maximum = runtime_point(source_point(&fixture["geometry"]["maximum_cm"]));
        let margin = fixture["collision_margin_0_cm"].as_f64().unwrap() * 0.01;
        let vertices = fixture["other_geometry"]["convex_core_vertices_cm"].as_array().unwrap().iter()
            .map(|value| runtime_point(source_point(value))).collect::<Vec<_>>();
        let (epsilon, iterations) = include!("../als/settings/contact_gjk.rs");
        let pose = |input: &serde_json::Value, translation_key: &str, rotation_key: &str| {
            let translation = &input[translation_key];
            let source = DVec3::new(translation[0].as_f64().unwrap(), translation[1].as_f64().unwrap(),
                translation[2].as_f64().unwrap());
            let rotation = &input[rotation_key];
            let quat = DQuat::from_xyzw(-rotation[0].as_f64().unwrap(), -rotation[2].as_f64().unwrap(),
                -rotation[1].as_f64().unwrap(), rotation[3].as_f64().unwrap());
            (runtime_point(source), quat)
        };
        let relative = |input: &serde_json::Value| {
            let (body, body_rotation) = pose(input, "body_translation_cm", "body_rotation_xyzw");
            let (mesh, mesh_rotation) = pose(input, "mesh_translation_cm", "mesh_rotation_xyzw");
            let inverse = body_rotation.inverse();
            (inverse * (mesh - body), inverse * mesh_rotation)
        };
        for sample in fixture["samples"].as_array().unwrap() {
            let input = &sample["input"];
            let (translation, rotation) = relative(input);
            let initial = source_box_convex_gjk(minimum, maximum, margin, &vertices, DVec3::ONE,
                translation, rotation, epsilon, iterations, None).unwrap();
            let SourceGjkResult::Separated(initial) = initial else { panic!("The source initial cores must be separated"); };
            let (translation, rotation) = relative(&input["next"]);
            let result = source_box_convex_gjk(minimum, maximum, margin, &vertices, DVec3::ONE,
                translation, rotation, epsilon, iterations, Some(initial.simplex_cm)).unwrap();
            if direct && let SourceGjkResult::Separated(value) = &result {
                assert_native_result(value, &sample["warm_gjk"]);
            }
            if !sample["restored"].as_bool().unwrap() {
                let SourceGjkResult::Separated(result) = &result else { panic!("The source next cores must be separated"); };
                let expected = sample["next_gjk_simplex"].as_array().unwrap();
                assert_eq!(result.simplex_cm.len, expected.len());
                for (index, value) in expected.iter().enumerate() {
                    assert!((result.simplex_cm.weights[index] - value["weight"].as_f64().unwrap()).abs() < 1e-7);
                    assert!(result.simplex_cm.vertices[index].first.distance(source_point(&value["shape_point_0_cm"])) < 1e-6);
                    assert!(result.simplex_cm.vertices[index].second.distance(source_point(&value["shape_point_1_cm"])) < 1e-6);
                }
            }
        }
    }

    #[test]
    fn source_gjk_discards_cached_simplex_at_zero_distance() {
        let minimum = DVec3::splat(-0.5);
        let maximum = DVec3::splat(0.5);
        let vertices = [DVec3::new(-0.1, -0.1, -0.1), DVec3::new(0.1, -0.1, -0.1),
            DVec3::new(0.0, 0.1, -0.1), DVec3::new(0.0, 0.0, 0.1)];
        let translation = DVec3::X * 2.0;
        let (epsilon, iterations) = include!("../als/settings/contact_gjk.rs");
        let cold = source_box_convex_gjk(minimum, maximum, 0.01, &vertices, DVec3::ONE, translation,
            DQuat::IDENTITY, epsilon, iterations, None).unwrap();
        let mut previous = SourceGjkSimplex { len: 1, ..Default::default() };
        previous.vertices[0].first = source_position(translation);
        let warm = source_box_convex_gjk(minimum, maximum, 0.01, &vertices, DVec3::ONE, translation,
            DQuat::IDENTITY, epsilon, iterations, Some(previous)).unwrap();
        let (SourceGjkResult::Separated(cold), SourceGjkResult::Separated(warm)) = (cold, warm)
            else { panic!("The source cores must be separated"); };
        assert_eq!(cold.last_vertices, warm.last_vertices);
        assert_eq!(cold.normal_first, warm.normal_first);
        assert_eq!(cold.penetration_m, warm.penetration_m);
    }
}
