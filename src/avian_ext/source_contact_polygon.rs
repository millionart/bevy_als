use bevy::math::{DQuat, DVec3};

const SOURCE_POLYGON_DATA: ([[usize; 4]; 6], f32, f32, usize) = include!("source_contact_polygon_data.rs");
pub(super) const SOURCE_SMALL_NUMBER: f32 = SOURCE_POLYGON_DATA.1;

pub(super) fn source_position(value: DVec3) -> DVec3 {
    DVec3::new(value.x, value.z, value.y) * 100.0
}

fn runtime_position(value: DVec3) -> DVec3 {
    DVec3::new(value.x, value.z, value.y) * 0.01
}

pub(super) fn source_direction(value: DVec3) -> DVec3 {
    DVec3::new(value.x, value.z, value.y)
}

pub(crate) fn source_box_face(minimum: DVec3, maximum: DVec3, face: usize) -> (Vec<DVec3>, DVec3) {
    let minimum = source_position(minimum);
    let maximum = source_position(maximum);
    let vertices = SOURCE_POLYGON_DATA.0[face].into_iter().map(|index| {
        runtime_position(DVec3::new(
            if index & 1 == 0 { minimum.x } else { maximum.x },
            if index & 2 == 0 { minimum.y } else { maximum.y },
            if index & 4 == 0 { minimum.z } else { maximum.z },
        ))
    }).collect();
    let axis = face % 3;
    let mut normal = DVec3::ZERO;
    normal[axis] = if face < 3 { -1.0 } else { 1.0 };
    (vertices, source_direction(normal))
}

fn clip_plane(points: &[DVec3], normal: DVec3, distance: f64) -> Vec<DVec3> {
    let mut output = Vec::with_capacity(points.len());
    let mut current = *points.last().expect("A source clipping polygon must not be empty");
    let mut current_dot = current.dot(normal);
    let limit = distance + distance * f64::from(SOURCE_POLYGON_DATA.1);
    for &point in points {
        let previous = current;
        let previous_dot = current_dot;
        current = point;
        current_dot = current.dot(normal);
        let intersection = || {
            let denominator = current_dot - previous_dot;
            if denominator.abs() < f64::from(SOURCE_POLYGON_DATA.1) { previous }
            else { previous + (current - previous) * ((distance - previous_dot) / denominator) }
        };
        if current_dot <= limit {
            if previous_dot > limit {
                output.push(intersection());
                if output.len() >= SOURCE_POLYGON_DATA.3 { break; }
            }
            output.push(current);
        } else if previous_dot < limit {
            output.push(intersection());
        }
        if output.len() >= SOURCE_POLYGON_DATA.3 { break; }
    }
    output
}

fn source_rotation_to_z(normal: DVec3) -> DQuat {
    let cosine = normal.dot(DVec3::Z).clamp(-1.0, 1.0);
    let mut axis = normal.cross(DVec3::Z);
    let magnitude = axis.length();
    if magnitude == 0.0 { return DQuat::from_xyzw(normal.x, normal.y, normal.z, 0.0); }
    let squared = 0.5 * (1.0 + cosine);
    axis *= (1.0 - squared).sqrt() / magnitude;
    DQuat::from_xyzw(axis.x, axis.y, axis.z, squared.sqrt())
}

fn reduce_source_points(points: &mut Vec<DVec3>) {
    if points.len() <= 4 { return; }
    let mut deepest = 0;
    let mut depth = f32::MAX as f64;
    for (index, point) in points.iter().enumerate() {
        if point.z < depth { deepest = index; depth = point.z; }
    }
    points.swap(0, deepest);
    let mut farthest = 1;
    let mut distance = -1.0;
    for index in 1..points.len() {
        let delta = points[index] - points[0];
        let candidate = delta.x * delta.x + delta.y * delta.y;
        if candidate > distance { farthest = index; distance = candidate; }
    }
    points.swap(1, farthest);
    let mut largest = 2;
    let mut area = 0.0_f64;
    let edge = points[1] - points[0];
    for index in 2..points.len() {
        let candidate = edge.cross(points[index] - points[0]).z;
        if candidate.abs() > area.abs() { largest = index; area = candidate; }
    }
    points.swap(2, largest);
    if area < 0.0 { points.swap(0, 1); }
    let mut largest = 3;
    let mut area = 0.0;
    for index in 3..points.len() {
        for edge in 0..3 {
            let candidate = (points[index] - points[edge]).cross(points[(edge + 1) % 3] - points[edge]).z;
            if candidate > area { largest = index; area = candidate; }
        }
    }
    points.swap(3, largest);
    points.truncate(4);
}

pub(crate) fn source_clip_convex_face(
    reference: &[DVec3],
    normal: DVec3,
    incident: &[DVec3],
    winding: f64,
) -> Vec<DVec3> {
    if reference.is_empty() || incident.is_empty() { return Vec::new(); }
    let reference = reference.iter().copied().map(source_position).collect::<Vec<_>>();
    let normal = source_direction(normal);
    let mut points = incident.iter().take(SOURCE_POLYGON_DATA.3).copied().map(source_position).collect::<Vec<_>>();
    let mut previous = *reference.last().unwrap();
    for current in reference {
        if points.len() <= 1 { break; }
        let mut edge_normal = winding * normal.cross(previous - current);
        let squared = edge_normal.length_squared();
        if squared < f64::from(SOURCE_POLYGON_DATA.2) { edge_normal = DVec3::X; }
        else { edge_normal /= squared.sqrt(); }
        points = clip_plane(&points, edge_normal, current.dot(edge_normal));
        previous = current;
    }
    if points.len() == 4 { points.swap(1, 2); }
    if points.len() > 4 {
        let rotation = source_rotation_to_z(normal);
        for point in &mut points { *point = rotation * *point; }
        reduce_source_points(&mut points);
        let inverse = rotation.conjugate();
        for point in &mut points { *point = inverse * *point; }
    }
    points.into_iter().map(runtime_position).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_xyz(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap(), value["z"].as_f64().unwrap())
    }

    fn source_array(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value[0].as_f64().unwrap(), value[1].as_f64().unwrap(), value[2].as_f64().unwrap())
    }

    #[test]
    fn source_clipping_preserves_four_point_and_reduction_order() {
        let reference = [DVec3::new(-1.0, 0.0, -1.0), DVec3::new(1.0, 0.0, -1.0),
            DVec3::new(1.0, 0.0, 1.0), DVec3::new(-1.0, 0.0, 1.0)];
        let incident = [DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.5, 0.0, 0.0),
            DVec3::new(0.5, 0.0, 0.5), DVec3::new(0.0, 0.0, 0.5)];
        assert_eq!(source_clip_convex_face(&reference, DVec3::Y, &incident, 1.0),
            vec![incident[0], incident[2], incident[1], incident[3]]);
        let mut points = vec![DVec3::new(0.0, 0.0, 1.0), DVec3::new(3.0, 0.0, 0.0),
            DVec3::new(3.0, 2.0, 0.0), DVec3::new(0.0, 2.0, 0.0), DVec3::new(1.0, 1.0, -1.0)];
        reduce_source_points(&mut points);
        assert_eq!(points, vec![DVec3::new(1.0, 1.0, -1.0), DVec3::new(3.0, 0.0, 0.0),
            DVec3::new(3.0, 2.0, 0.0), DVec3::new(0.0, 2.0, 0.0)]);
        assert_eq!(source_rotation_to_z(DVec3::NEG_Z) * DVec3::NEG_Z, DVec3::NEG_Z);
    }

    #[test]
    fn source_clipping_keeps_intersections_and_near_parallel_endpoint() {
        let triangle = [DVec3::ZERO, DVec3::new(2.0, 0.0, 0.0), DVec3::new(0.0, 2.0, 0.0)];
        assert_eq!(clip_plane(&triangle, DVec3::X, 1.0), vec![DVec3::ZERO, DVec3::X,
            DVec3::new(1.0, 1.0, 0.0), DVec3::new(0.0, 2.0, 0.0)]);
        let inside = DVec3::new(-1e-10, 0.0, 0.0);
        let outside = DVec3::new(1e-10, 0.0, 0.0);
        assert_eq!(clip_plane(&[inside, outside], DVec3::X, 0.0), vec![outside, inside, inside]);
    }

    #[test]
    fn source_bench_contact_faces_match_native_order() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3b2/bench.native-samples.json")).unwrap();
        let samples = fixture["samples"].as_array().unwrap();
        for (sample, box_index, convex_index, convex_is_reference) in [
            (&samples[0], 3, 20, true), (&samples[1], 2, 10, false),
        ] {
            let geometry = &sample["geometry"];
            let convex = &sample["other_geometry"];
            let body = source_array(&sample["input"]["body_translation_cm"]);
            let minimum = source_xyz(&geometry["minimum_cm"]);
            let maximum = source_xyz(&geometry["maximum_cm"]);
            let (box_face, box_normal) = source_box_face(runtime_position(minimum), runtime_position(maximum), box_index);
            let body = runtime_position(body);
            let plane = &convex["convex_core_planes"][convex_index];
            let vertices = convex["convex_core_vertices_cm"].as_array().unwrap();
            let convex_face = plane["vertex_indices"].as_array().unwrap().iter().map(|index| {
                runtime_position(source_xyz(&vertices[index.as_u64().unwrap() as usize]))
            }).collect::<Vec<_>>();
            let convex_normal = source_direction(source_xyz(&plane["normal"]));
            let box_in_convex = box_face.iter().map(|point| *point + body).collect::<Vec<_>>();
            let convex_in_box = convex_face.iter().map(|point| *point - body).collect::<Vec<_>>();
            let (reference, normal, incident, point_key, offset) = if convex_is_reference {
                (&convex_face, convex_normal, &box_in_convex, "shape_point_0_cm", body)
            } else {
                (&box_face, box_normal, &convex_in_box, "shape_point_1_cm", -body)
            };
            let actual = source_clip_convex_face(reference, normal, incident, 1.0);
            let expected = sample["points"].as_array().unwrap().iter()
                .map(|point| runtime_position(source_xyz(&point[point_key])) + offset).collect::<Vec<_>>();
            assert_eq!(actual.len(), expected.len());
            for (actual, expected) in actual.iter().zip(expected) {
                assert!(actual.distance(expected) < 0.00002, "Source contact point order differs: {actual:?} {expected:?}");
            }
        }
    }
}
