use bevy::math::DVec3;

pub(super) fn source_nearest_segments(first: [DVec3; 2], second: [DVec3; 2]) -> ([f64; 2], [DVec3; 2]) {
    let epsilon = f64::from(1.0e-4_f32);
    let epsilon_squared = epsilon * epsilon;
    let first_delta = first[1] - first[0];
    let second_delta = second[1] - second[0];
    let difference = first[0] - second[0];
    let a = first_delta.dot(first_delta);
    let b = first_delta.dot(second_delta);
    let c = first_delta.dot(difference);
    let e = second_delta.dot(second_delta);
    let f = second_delta.dot(difference);
    let (mut s, mut t) = (0.0, 0.0);
    if !(a <= epsilon_squared && b <= epsilon_squared) {
        if a <= epsilon_squared {
            t = (f / e).clamp(0.0, 1.0);
        } else if e <= epsilon_squared {
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let denominator = a * e - b * b;
            if denominator != 0.0 { s = ((b * f - c * e) / denominator).clamp(0.0, 1.0); }
            t = (b * s + f) / e;
            if t < 0.0 {
                s = (-c / a).clamp(0.0, 1.0);
                t = 0.0;
            } else if t > 1.0 {
                s = ((b - c) / a).clamp(0.0, 1.0);
                t = 1.0;
            }
        }
    }
    ([s, t], [first[0] + s * first_delta, second[0] + t * second_delta])
}

#[cfg(test)]
mod tests {
    use bevy::math::DVec3;

    use super::*;

    #[test]
    fn source_nearest_segments_keeps_crossing_parallel_and_clamped_parameters() {
        let first = [DVec3::new(-2.0, 0.0, 0.0), DVec3::new(2.0, 0.0, 0.0)];
        let crossing = [DVec3::new(0.0, -1.0, 0.0), DVec3::new(0.0, 1.0, 0.0)];
        let (parameters, points) = source_nearest_segments(first, crossing);
        assert_eq!(parameters, [0.5, 0.5]);
        assert_eq!(points, [DVec3::ZERO, DVec3::ZERO]);
        let parallel = [DVec3::new(-1.0, 1.0, 0.0), DVec3::new(1.0, 1.0, 0.0)];
        let (parameters, points) = source_nearest_segments(first, parallel);
        assert_eq!(parameters, [0.25, 0.0]);
        assert_eq!(points, [DVec3::new(-1.0, 0.0, 0.0), parallel[0]]);
        let beyond = [DVec3::new(3.0, 1.0, 0.0), DVec3::new(3.0, 2.0, 0.0)];
        let (parameters, points) = source_nearest_segments(first, beyond);
        assert_eq!(parameters, [1.0, 0.0]);
        assert_eq!(points, [first[1], beyond[0]]);
    }

    #[test]
    fn source_nearest_segments_preserves_source_degenerate_branch() {
        let first = [DVec3::new(3.0, 0.0, 0.0); 2];
        let second = [DVec3::ZERO, DVec3::new(4.0, 0.0, 0.0)];
        let (parameters, points) = source_nearest_segments(first, second);
        assert_eq!(parameters, [0.0, 0.0]);
        assert_eq!(points, [first[0], second[0]]);
        let (parameters, points) = source_nearest_segments(second, first);
        assert_eq!(parameters, [0.75, 0.0]);
        assert_eq!(points, [first[0], first[0]]);
    }

    #[test]
    fn source_nearest_segments_matches_native_when_available() {
        let Ok(path) = std::env::var("ALS_SEGMENT_NEAREST_NATIVE_REPORT") else { return; };
        let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let array = |value: &serde_json::Value| DVec3::new(value[0].as_f64().unwrap(),
            value[1].as_f64().unwrap(), value[2].as_f64().unwrap());
        let point = |value: &serde_json::Value| DVec3::new(value["x"].as_f64().unwrap(),
            value["y"].as_f64().unwrap(), value["z"].as_f64().unwrap());
        for sample in report["physics_contact_probe"]["samples"].as_array().unwrap() {
            let input = &sample["input"]["segment_nearest"];
            let native = &sample["segment_nearest"];
            let first = [array(&input["first_start_cm"]), array(&input["first_end_cm"])];
            let second = [array(&input["second_start_cm"]), array(&input["second_end_cm"])];
            let (parameters, points) = source_nearest_segments(first, second);
            for (actual, key) in parameters.into_iter().zip(["first_parameter", "second_parameter"]) {
                assert!((actual - native[key].as_f64().unwrap()).abs() < 1e-10, "{} {key}", sample["input"]["label"]);
            }
            for (actual, key) in points.into_iter().zip(["first_point_cm", "second_point_cm"]) {
                assert!(actual.distance(point(&native[key])) < 1e-10, "{} {key}", sample["input"]["label"]);
            }
        }
    }
}
