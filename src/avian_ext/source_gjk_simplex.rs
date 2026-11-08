use bevy::math::DVec3;

#[derive(Clone, Copy, Default)]
pub(crate) struct SourceGjkVertex {
    pub difference: DVec3,
    pub first: DVec3,
    pub second: DVec3,
}

#[derive(Clone, Default)]
pub(crate) struct SourceGjkSimplex {
    pub vertices: [SourceGjkVertex; 4],
    pub weights: [f64; 4],
    pub len: usize,
}

#[derive(Clone, Copy)]
struct Projection {
    indices: [usize; 4],
    len: usize,
    weights: [f64; 4],
    point: DVec3,
}

fn vertex(index: usize, point: DVec3) -> Projection {
    let mut weights = [0.0; 4];
    weights[index] = 1.0;
    Projection { indices: [index, 0, 0, 0], len: 1, weights, point }
}

fn line(points: &[DVec3; 4], indices: [usize; 2]) -> Projection {
    let [first, second] = indices;
    let a = points[first];
    let b = points[second];
    let edge = b - a;
    let dot = (-a).dot(edge);
    if dot <= 0.0 { return vertex(first, a); }
    let squared = edge.length_squared();
    if squared <= dot || squared <= f64::MIN_POSITIVE { return vertex(second, b); }
    let ratio = (dot / squared).clamp(0.0, 1.0);
    let mut weights = [0.0; 4];
    weights[first] = 1.0 - ratio;
    weights[second] = ratio;
    Projection { indices: [first, second, 0, 0], len: 2, weights, point: ratio * edge + a }
}

fn sign_match(first: f64, second: f64) -> bool {
    (first > 0.0 && second > 0.0) || (first < 0.0 && second < 0.0)
}

fn triangle(points: &[DVec3; 4], indices: [usize; 3]) -> Projection {
    let [first, second, third] = indices;
    let a = points[first];
    let b = points[second];
    let c = points[third];
    let normal = (b - a).cross(c - a);
    let squared = normal.length_squared();
    if (a * f64::MIN_POSITIVE).length_squared() >= squared { return line(points, [first, second]); }
    let projected = normal * a.dot(normal / squared);
    let mut determinant = 0.0;
    let mut best_u = 0;
    let mut best_v = 0;
    let mut largest = 0.0;
    let mut u = 1;
    let mut v = 2;
    for axis in 0..3 {
        let candidate = b[u] * c[v] - c[u] * b[v] + c[u] * a[v] - a[u] * c[v]
            + a[u] * b[v] - b[u] * a[v];
        if axis == 0 || candidate.abs() > largest {
            determinant = candidate;
            largest = candidate.abs();
            best_u = u;
            best_v = v;
        }
        u = v;
        v = axis;
    }
    let pa = a - projected;
    let pb = b - projected;
    let pc = c - projected;
    let cofactors = [pb[best_u] * pc[best_v] - pc[best_u] * pb[best_v],
        -pa[best_u] * pc[best_v] + pc[best_u] * pa[best_v],
        pa[best_u] * pb[best_v] - pb[best_u] * pa[best_v]];
    let edges = [[second, third], [first, third], [first, second]];
    let mut closest = None;
    let mut minimum = 0.0;
    for index in 0..3 {
        if !sign_match(determinant, cofactors[index]) {
            let candidate = line(points, edges[index]);
            let distance = candidate.point.length_squared();
            if closest.is_none() || distance < minimum { closest = Some(candidate); minimum = distance; }
        }
    }
    if let Some(closest) = closest { return closest; }
    let inverse = determinant.recip();
    let mut weights = [0.0; 4];
    weights[first] = cofactors[0] * inverse;
    weights[second] = cofactors[1] * inverse;
    weights[third] = cofactors[2] * inverse;
    Projection { indices: [first, second, third, 0], len: 3, weights, point: projected }
}

fn tetrahedron(points: &[DVec3; 4]) -> Projection {
    let [a, b, c, d] = *points;
    let cofactors = [-b.dot(c.cross(d)), a.dot(c.cross(d)), -a.dot(b.cross(d)), a.dot(b.cross(c))];
    let determinant = (cofactors[0] + cofactors[1]) + (cofactors[2] + cofactors[3]);
    let faces = [[1, 2, 3], [0, 2, 3], [0, 1, 3], [0, 1, 2]];
    let mut closest = None;
    let mut minimum = 0.0;
    for index in 0..4 {
        if !sign_match(determinant, cofactors[index]) {
            let candidate = triangle(points, faces[index]);
            let distance = candidate.point.length_squared();
            if closest.is_none() || distance < minimum { closest = Some(candidate); minimum = distance; }
        }
    }
    if let Some(closest) = closest { return closest; }
    Projection { indices: [0, 1, 2, 3], len: 4,
        weights: cofactors.map(|value| value / determinant), point: DVec3::ZERO }
}

impl SourceGjkSimplex {
    pub(crate) fn project(&mut self) -> DVec3 {
        let points = self.vertices.map(|value| value.difference);
        let projection = match self.len {
            1 => vertex(0, points[0]),
            2 => line(&points, [0, 1]),
            3 => triangle(&points, [0, 1, 2]),
            4 => tetrahedron(&points),
            _ => unreachable!(),
        };
        let vertices = self.vertices;
        self.len = projection.len;
        self.weights = [0.0; 4];
        for index in 0..projection.len {
            let source = projection.indices[index];
            self.vertices[index] = vertices[source];
            self.weights[index] = projection.weights[source];
        }
        projection.point
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector(value: &serde_json::Value) -> DVec3 {
        DVec3::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap(), value["z"].as_f64().unwrap())
    }

    #[test]
    fn source_simplex_projection_matches_native_bench_samples() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d100b3b3b2a/simplex.native-samples.json")).unwrap();
        for sample in fixture["samples"].as_array().unwrap() {
            let expected = sample["gjk_simplex"].as_array().unwrap();
            let mut simplex = SourceGjkSimplex { len: expected.len(), ..Default::default() };
            let mut expected_point = DVec3::ZERO;
            for (index, item) in expected.iter().enumerate() {
                let difference = vector(&item["world_delta_cm"]);
                simplex.vertices[index] = SourceGjkVertex { difference,
                    first: vector(&item["shape_point_0_cm"]), second: vector(&item["shape_point_1_cm"]) };
                expected_point += difference * item["weight"].as_f64().unwrap();
            }
            let point = simplex.project();
            assert_eq!(simplex.len, expected.len());
            assert!(point.distance(expected_point) < 1e-6);
            for (index, item) in expected.iter().enumerate() {
                assert!((simplex.weights[index] - item["weight"].as_f64().unwrap()).abs() < 1e-7);
                assert_eq!(simplex.vertices[index].first, vector(&item["shape_point_0_cm"]));
                assert_eq!(simplex.vertices[index].second, vector(&item["shape_point_1_cm"]));
            }
        }
    }

    #[test]
    fn source_simplex_projection_handles_degenerate_and_interior_shapes() {
        let mut simplex = SourceGjkSimplex { len: 2, ..Default::default() };
        simplex.vertices[0].difference = DVec3::NEG_X;
        simplex.vertices[1].difference = DVec3::X;
        assert_eq!(simplex.project(), DVec3::ZERO);
        assert_eq!(&simplex.weights[..2], &[0.5, 0.5]);
        simplex.len = 3;
        simplex.vertices[2].difference = DVec3::X * 2.0;
        assert_eq!(simplex.project(), DVec3::ZERO);
        assert_eq!(simplex.len, 2);
        simplex.len = 4;
        for (vertex, difference) in simplex.vertices.iter_mut().zip([
            DVec3::X, DVec3::Y, DVec3::Z, DVec3::splat(-1.0),
        ]) { vertex.difference = difference; }
        assert_eq!(simplex.project(), DVec3::ZERO);
        assert_eq!(simplex.len, 4);
        assert_eq!(simplex.weights, [0.25; 4]);
    }
}
