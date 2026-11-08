use bevy::prelude::*;

pub(crate) fn save_source_contact_anchor(
    current: [Vec3; 2],
    anchors: [Vec3; 2],
    ratio: f32,
    point_count: usize,
    ratio_tolerance: f32,
) -> Option<[Vec3; 2]> {
    if ratio >= 1.0 - ratio_tolerance {
        Some(anchors)
    } else if ratio < ratio_tolerance {
        (point_count < 8).then_some(current)
    } else {
        Some([current[0].lerp(anchors[0], ratio), current[1].lerp(anchors[1], ratio)])
    }
}

pub(crate) fn find_source_contact_anchor(
    saved: &[[Vec3; 2]],
    current: [Vec3; 2],
    point_index: usize,
    quadratic: [bool; 2],
    exact_distance: f32,
    near_distance: f32,
) -> Option<usize> {
    let exact_squared = exact_distance * exact_distance;
    let mut best_squared = near_distance * near_distance;
    let mut best = None;
    for offset in 0..saved.len() {
        let index = (point_index + offset) % saved.len();
        let distances = [current[0].distance_squared(saved[index][0]), current[1].distance_squared(saved[index][1])];
        let distance = match quadratic {
            [true, false] => distances[0],
            [false, true] => distances[1],
            _ => distances[0].min(distances[1]),
        };
        if distance < exact_squared { return Some(index); }
        if distance < best_squared {
            best_squared = distance;
            best = Some(index);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector(value: &serde_json::Value) -> Vec3 {
        Vec3::new(value[0].as_f64().unwrap() as f32, value[1].as_f64().unwrap() as f32,
            value[2].as_f64().unwrap() as f32)
    }

    fn pair(value: &serde_json::Value) -> [Vec3; 2] { [vector(&value[0]), vector(&value[1])] }

    #[test]
    fn source_anchor_history_matches_native_probe() {
        let settings: [f32; 3] = include!("../als/settings/contact_history.rs");
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../docs/audit/bf144/d96a/history.runtime.json")).unwrap();
        for sample in fixture["samples"].as_array().unwrap() {
            let current = sample["current"].as_array().unwrap();
            let offsets = pair(&sample["anchor_offsets"]);
            let ratios = sample["ratios"].as_array().unwrap();
            let saved = current.iter().zip(ratios).filter_map(|(value, ratio)| {
                let points = pair(value);
                let anchors = [points[0] + offsets[0], points[1] + offsets[1]];
                save_source_contact_anchor(points, anchors, ratio.as_f64().unwrap() as f32, current.len(), settings[2])
            }).collect::<Vec<_>>();
            let expected = sample["saved"].as_array().unwrap();
            assert_eq!(saved.len(), expected.len(), "{}", sample["label"]);
            for (actual, expected) in saved.iter().zip(expected) {
                let expected = pair(expected);
                for index in 0..2 { assert!(actual[index].abs_diff_eq(expected[index], 2e-6), "{}", sample["label"]); }
            }
            let quadratic = [sample["quadratic"][0].as_bool().unwrap(), sample["quadratic"][1].as_bool().unwrap()];
            for (index, value) in sample["next_points"].as_array().unwrap().iter().enumerate() {
                let found = find_source_contact_anchor(&saved, pair(value), index, quadratic, settings[0], settings[1]);
                assert_eq!(found.is_some(), sample["next_matches"][index].as_bool().unwrap(),
                    "{} point={index}", sample["label"]);
                if let Some(found) = found {
                    let expected = pair(&sample["next_anchors"][index]);
                    for side in 0..2 { assert!(saved[found][side].abs_diff_eq(expected[side], 2e-6), "{}", sample["label"]); }
                }
            }
        }
    }

    #[test]
    fn source_anchor_matching_observes_strict_distance_and_quadratic_side() {
        let current = [Vec3::ZERO; 2];
        let saved = [[Vec3::X * 0.002, Vec3::ZERO], [Vec3::X * 0.001, Vec3::ZERO]];
        assert_eq!(find_source_contact_anchor(&saved, current, 0, [true, false], 0.002, 0.01), Some(1));
        let exact = [[Vec3::X * 0.001, Vec3::ZERO], [Vec3::X * 0.0015, Vec3::ZERO]];
        assert_eq!(find_source_contact_anchor(&exact, current, 1, [true, false], 0.002, 0.01), Some(1));
        let far = [[Vec3::X * 0.01, Vec3::ZERO]];
        assert_eq!(find_source_contact_anchor(&far, current, 0, [true, false], 0.002, 0.01), None);
        assert_eq!(find_source_contact_anchor(&far, current, 0, [false, false], 0.002, 0.01), Some(0));
        let anchors = [Vec3::X, Vec3::Y];
        assert_eq!(save_source_contact_anchor(current, anchors, 1.0, 8, 0.0001), Some(anchors));
        assert_eq!(save_source_contact_anchor(current, anchors, 0.5, 8, 0.0001), Some([Vec3::X * 0.5, Vec3::Y * 0.5]));
        assert_eq!(save_source_contact_anchor(current, anchors, 0.0, 7, 0.0001), Some(current));
        assert_eq!(save_source_contact_anchor(current, anchors, 0.0, 8, 0.0001), None);
    }
}
