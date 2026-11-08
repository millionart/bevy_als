use bevy::math::{DQuat, DVec3};

use super::source_contact_polygon::{source_direction, source_position};

#[derive(Clone, Copy)]
pub(crate) struct SourceRestoreSettings {
    pub contact_scale: f64,
    pub empty_position_scale: f64,
    pub position_scale: f64,
    pub empty_rotation_threshold: f64,
    pub rotation_threshold: f64,
    pub collision_tolerance_scale: f32,
    pub minimum_points: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct SourceRestorePose {
    pub position_m: DVec3,
    pub rotation: DQuat,
}

#[derive(Clone, Copy)]
pub(crate) struct SourceRestorePoint {
    pub initial_first_m: DVec3,
    pub initial_second_m: DVec3,
    pub normal_second: DVec3,
    pub second_m: DVec3,
    pub phi_m: f64,
    pub disabled: bool,
}

fn source_float_position(value: DVec3) -> DVec3 {
    source_direction(source_position(value).as_vec3().as_dvec3()) * 0.01
}

pub(crate) fn source_restore_manifold(
    points: &mut [SourceRestorePoint],
    last: [SourceRestorePose; 2],
    current: [SourceRestorePose; 2],
    collision_tolerance_m: f64,
    allowed: bool,
    settings: SourceRestoreSettings,
) -> bool {
    if !allowed { return false; }
    let empty = points.is_empty();
    let position_scale = if empty { settings.empty_position_scale } else { settings.position_scale };
    let rotation_threshold = if empty { settings.empty_rotation_threshold } else { settings.rotation_threshold };
    let shape_tolerance = position_scale * collision_tolerance_m;
    if !(shape_tolerance > 0.0 && rotation_threshold > 0.0) { return false; }
    let translation = (current[0].position_m - current[1].position_m) - (last[0].position_m - last[1].position_m);
    if !(translation.x.abs() <= shape_tolerance && translation.y.abs() <= shape_tolerance
        && translation.z.abs() <= shape_tolerance) { return false; }
    let last_rotation = last[0].rotation.inverse() * last[1].rotation;
    let current_rotation = current[0].rotation.inverse() * current[1].rotation;
    if !(current_rotation.dot(last_rotation) > rotation_threshold) { return false; }
    let contact_tolerance_squared = (settings.contact_scale * collision_tolerance_m).powi(2);
    let mut active = points.len();
    for point in points.iter_mut() {
        if point.disabled { active -= 1; continue; }
        let world = current[0].position_m + current[0].rotation * point.initial_first_m;
        let first_in_second = current[1].rotation.inverse() * (world - current[1].position_m);
        let delta = first_in_second - point.initial_second_m;
        let phi = delta.dot(point.normal_second);
        let lateral = delta - phi * point.normal_second;
        if lateral.length_squared() < contact_tolerance_squared {
            point.second_m = source_float_position(first_in_second - phi * point.normal_second);
            point.phi_m = f64::from((phi * 100.0) as f32) * 0.01;
        } else {
            point.disabled = true;
            active -= 1;
        }
    }
    active >= points.len().min(settings.minimum_points)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> SourceRestoreSettings {
        let ([contact_scale, empty_position_scale, position_scale, empty_rotation_threshold, rotation_threshold],
            collision_tolerance_scale, minimum_points) = include!("../als/settings/contact_restore.rs");
        SourceRestoreSettings { contact_scale, empty_position_scale, position_scale, empty_rotation_threshold,
            rotation_threshold, collision_tolerance_scale, minimum_points }
    }

    fn object_vector(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value["x"].as_f64().unwrap(), value["z"].as_f64().unwrap(), value["y"].as_f64().unwrap())
    }

    fn array_vector(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value[0].as_f64().unwrap(), value[2].as_f64().unwrap(), value[1].as_f64().unwrap())
    }

    fn array_rotation(value: &serde_json::Value) -> DQuat {
        DQuat::from_xyzw(-value[0].as_f64().unwrap(), -value[2].as_f64().unwrap(),
            -value[1].as_f64().unwrap(), value[3].as_f64().unwrap())
    }

    fn object_rotation(value: &serde_json::Value) -> DQuat {
        DQuat::from_xyzw(-value["x"].as_f64().unwrap(), -value["z"].as_f64().unwrap(),
            -value["y"].as_f64().unwrap(), value["w"].as_f64().unwrap())
    }

    fn pose(input: &serde_json::Value, translation: &str, rotation: &str, local: Option<&serde_json::Value>) -> SourceRestorePose {
        let position = array_vector(&input[translation]) * 0.01;
        let orientation = array_rotation(&input[rotation]);
        if let Some(local) = local {
            SourceRestorePose { position_m: position + orientation * (object_vector(&local["translation"]) * 0.01),
                rotation: orientation * object_rotation(&local["rotation"]) }
        } else {
            SourceRestorePose { position_m: position, rotation: orientation }
        }
    }

    #[test]
    fn source_manifold_restore_matches_native_two_frame_contacts() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d104a/restore.native-samples.json")).unwrap();
        let settings = settings();
        for sample in fixture["samples"].as_array().unwrap() {
            let input = &sample["input"];
            let next = &input["next"];
            let local = &sample["geometry"]["shape_to_body"];
            let other = sample.get("other_geometry");
            let (translation_key, rotation_key) = if other.is_some() {
                ("mesh_translation_cm", "mesh_rotation_xyzw")
            } else { ("box_translation_cm", "box_rotation_xyzw") };
            let other_local = other.map(|value| &value["shape_to_body"]);
            let last = [pose(input, "body_translation_cm", "body_rotation_xyzw", Some(local)),
                pose(input, translation_key, rotation_key, other_local)];
            let current = [pose(next, "body_translation_cm", "body_rotation_xyzw", Some(local)),
                pose(next, translation_key, rotation_key, other_local)];
            let mut points = sample["points"].as_array().unwrap().iter().map(|point| {
                let first = object_vector(&point["shape_point_0_cm"]) * 0.01;
                let second = object_vector(&point["shape_point_1_cm"]) * 0.01;
                let local_normal = last[1].rotation.inverse() * object_vector(&point["world_normal"]);
                let local_normal = source_direction(source_direction(local_normal).as_vec3().as_dvec3());
                SourceRestorePoint { initial_first_m: first, initial_second_m: second, normal_second: local_normal,
                    second_m: second, phi_m: point["phi_cm"].as_f64().unwrap() * 0.01, disabled: false }
            }).collect::<Vec<_>>();
            let tolerance = if sample["geometry"]["kind"] == "box" {
                let shape = &sample["geometry"];
                let first_size = (object_vector(&shape["maximum_cm"]) - object_vector(&shape["minimum_cm"]))
                    .abs().max_element() as f32;
                let second_size = if let Some(other) = other {
                    (object_vector(&other["bounds_max_cm"]) - object_vector(&other["bounds_min_cm"]))
                        .abs().max_element() as f32
                } else { (array_vector(&input["box_half_extents_cm"]) * 2.0).abs().max_element() as f32 };
                f64::from(settings.collision_tolerance_scale * first_size.min(second_size)) * 0.01
            } else { 1.0 };
            let restored = source_restore_manifold(&mut points, last, current, tolerance,
                sample["restore_allowed"].as_bool().unwrap(), settings);
            assert_eq!(restored, sample["restored"].as_bool().unwrap(), "{}", input["label"]);
            if restored {
                let expected = sample["next_points"].as_array().unwrap();
                assert_eq!(points.iter().filter(|point| !point.disabled).count(), expected.len());
                for (point, expected) in points.iter().filter(|point| !point.disabled).zip(expected) {
                    assert!(point.second_m.distance(object_vector(&expected["shape_point_1_cm"]) * 0.01) < 2e-6,
                        "{}", input["label"]);
                    assert!((point.phi_m - expected["phi_cm"].as_f64().unwrap() * 0.01).abs() < 2e-6,
                        "{}", input["label"]);
                }
            }
        }
    }

    #[test]
    fn source_manifold_restore_checks_component_rotation_and_active_count() {
        let settings = settings();
        let pose = SourceRestorePose { position_m: DVec3::ZERO, rotation: DQuat::IDENTITY };
        let point = SourceRestorePoint { initial_first_m: DVec3::ZERO, initial_second_m: DVec3::ZERO,
            normal_second: DVec3::Y, second_m: DVec3::ZERO, phi_m: 0.0, disabled: false };
        let mut points = [point];
        let limit = settings.position_scale * 0.1;
        let translated = SourceRestorePose { position_m: DVec3::new(limit, 0.0, limit), ..pose };
        assert!(source_restore_manifold(&mut points, [pose; 2], [translated, pose], 0.1, true, settings));
        let beyond = SourceRestorePose { position_m: DVec3::X * (limit * 1.01), ..pose };
        assert!(!source_restore_manifold(&mut [point], [pose; 2], [beyond, pose], 0.1, true, settings));
        let angle = settings.rotation_threshold.acos() * 2.0;
        let rotated = SourceRestorePose { rotation: DQuat::from_rotation_y(angle * 1.1), ..pose };
        assert!(!source_restore_manifold(&mut [point], [pose; 2], [rotated, pose], 0.1, true, settings));
        let mut enough = [point; 5];
        enough[0].disabled = true;
        assert!(source_restore_manifold(&mut enough, [pose; 2], [pose; 2], 0.1, true, settings));
        enough[1].disabled = true;
        assert!(!source_restore_manifold(&mut enough, [pose; 2], [pose; 2], 0.1, true, settings));
        assert!(source_restore_manifold(&mut [], [pose; 2], [pose; 2], 0.1, true, settings));
    }
}
