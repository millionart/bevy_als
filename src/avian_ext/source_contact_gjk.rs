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
pub(crate) enum SourceColdGjk {
    Separated(SourceGjkSeparation),
    NeedsEpa { simplex_cm: SourceGjkSimplex, last_vertices: [usize; 2] },
}

fn source_box_support(minimum: DVec3, maximum: DVec3, margin: f64, direction: DVec3) -> (DVec3, usize) {
    let mut point = DVec3::ZERO;
    let mut index = 0;
    for axis in 0..3 {
        if direction[axis] < 0.0 { point[axis] = minimum[axis] + margin; }
        else { point[axis] = maximum[axis] - margin; index |= 1 << axis; }
    }
    (point, index)
}

fn source_convex_support(vertices: &[Vec3], scale: DVec3, direction: DVec3) -> Option<(DVec3, usize)> {
    let direction = (scale * direction).as_vec3();
    let mut maximum = f32::MIN;
    let mut selected = None;
    for (index, point) in vertices.iter().enumerate() {
        let dot = point.x * direction.x + point.y * direction.y + point.z * direction.z;
        if dot > maximum { maximum = dot; selected = Some(index); }
    }
    selected.map(|index| (vertices[index].as_dvec3() * scale, index))
}

pub(crate) fn source_box_convex_cold_gjk(
    box_minimum: DVec3,
    box_maximum: DVec3,
    box_margin: f64,
    convex_vertices: &[DVec3],
    convex_scale: DVec3,
    relative_translation: DVec3,
    relative_rotation: DQuat,
    epsilon: f64,
    iteration_limit: usize,
) -> Option<SourceColdGjk> {
    let minimum = source_position(box_minimum);
    let maximum = source_position(box_maximum);
    let margin = f64::from((box_margin * 100.0) as f32);
    let vertices = convex_vertices.iter().copied().map(|point| source_position(point).as_vec3()).collect::<Vec<_>>();
    let scale = source_direction(convex_scale);
    let translation = source_position(relative_translation);
    let rotation = DQuat::from_xyzw(-relative_rotation.x, -relative_rotation.z, -relative_rotation.y, relative_rotation.w);
    let inverse = rotation.inverse();
    let epsilon = epsilon * 100.0;
    let mut simplex = SourceGjkSimplex::default();
    let mut direction = DVec3::NEG_X;
    let mut normal = -direction;
    let mut distance = f32::MAX as f64;
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
        if next_distance < epsilon { return Some(SourceColdGjk::NeedsEpa { simplex_cm: simplex, last_vertices }); }
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
    Some(SourceColdGjk::Separated(SourceGjkSeparation {
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
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3b3b2b/gjk.native-samples.json")).unwrap();
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
            let result = source_box_convex_cold_gjk(minimum, maximum, margin, &vertices, DVec3::ONE,
                -runtime_point(body), DQuat::IDENTITY, epsilon, iterations).unwrap();
            let native = sample["gjk_simplex"].as_array().unwrap();
            let simplex = match &result {
                SourceColdGjk::Separated(result) => {
                    assert_ne!(input["label"], "bench_foot_core_overlap");
                    let normal = source_direction(source_point(&sample["points"][0]["world_normal"]));
                    assert!(result.normal_first.distance(-normal) < 1e-6);
                    assert!(result.last_vertices[0] < 8 && result.last_vertices[1] < vertices.len());
                    &result.simplex_cm
                }
                SourceColdGjk::NeedsEpa { simplex_cm, last_vertices } => {
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
}
