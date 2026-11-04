use bevy::{math::{DQuat, DVec3}, prelude::*};
use bevy_als::als::animation::acceleration_amount;
use serde_json::{Value, json};
use std::path::Path;

fn array<const N: usize>(row: &Value, key: &str) -> [f64; N] {
    std::array::from_fn(|i| row[key][i].as_f64().unwrap())
}

fn scalar(row: &Value, key: &str) -> f32 {
    row[key].as_f64().unwrap() as f32
}

fn source_local(a: DVec3, rotation: DQuat) -> Vec3 {
    let inverse = -rotation.xyz();
    let t = 2.0 * inverse.cross(a);
    (a + rotation.w * t + inverse.cross(t)).as_vec3()
}

fn source_amount(a: DVec3, v: DVec3, q: DQuat, acceleration: f32, braking: f32) -> (Vec2, bool) {
    let maximum = if a.dot(v) >= 0.0 { acceleration } else { braking };
    if maximum <= 1e-4 {
        return (Vec2::ZERO, false);
    }
    let local = source_local(a, q) * maximum.recip();
    let length = local.length_squared();
    let clamped = length > 1.0;
    let result = if clamped { local * length.sqrt().recip() } else { local };
    (result.truncate(), clamped)
}

fn project(world: Vec3, rotation: Quat) -> Vec3 {
    Vec3::new(world.dot(rotation * Vec3::NEG_Z), world.dot(rotation * Vec3::X), world.dot(rotation * Vec3::Y))
}

fn source_axes(vector: DVec3) -> DVec3 {
    DVec3::new(vector.x, vector.z, vector.y)
}

fn source_rotation(rotation: Quat) -> DQuat {
    let half = std::f64::consts::FRAC_1_SQRT_2;
    let q = rotation.as_dquat() * DQuat::from_xyzw(0.0, half, 0.0, half);
    DQuat::from_xyzw(-q.x, -q.z, -q.y, q.w)
}

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/510");
    let input: Value = serde_json::from_slice(&std::fs::read(root.join("acceleration-input.json")).unwrap()).unwrap();
    let mut native_clamps = 0;
    let mut native_brakes = 0;
    for row in input["native"].as_array().unwrap() {
        let a = DVec3::from_array(array(row, "acceleration"));
        let v = DVec3::from_array(array(row, "velocity"));
        let q = DQuat::from_array(array(row, "rotation"));
        let (result, capped) = source_amount(a, v, q, scalar(row,"max_acceleration"), scalar(row,"max_braking"));
        assert_eq!(result.to_array(), array::<2>(row,"goal").map(|x| x as f32), "Native frame {}", row["frame"]);
        native_clamps += usize::from(capped);
        native_brakes += usize::from(a.dot(v) < 0.0);
    }
    println!("Verified source position-independent kernel over observed native inputs");
    let mut cases = Vec::new();
    let mut edges = Vec::new();
    for row in input["target"].as_array().unwrap() {
        let a = Vec3::from_array(array::<3>(row,"acceleration").map(|x|x as f32));
        let v = Vec3::from_array(array::<3>(row,"velocity").map(|x|x as f32));
        let q = Quat::from_array(array::<4>(row,"rotation").map(|x|x as f32));
        let accel = scalar(row,"max_acceleration");
        let brake = scalar(row,"max_braking");
        let dot = a.dot(v);
        let wide_dot = a.as_dvec3().dot(v.as_dvec3());
        let max = if dot >= 0.0 { accel } else { brake };
        let expected = acceleration_amount(a, v, q, accel, brake);
        assert_eq!(expected.to_array(), array::<2>(row,"goal").map(|x|x as f32), "Target frame {}",row["frame"]);
        assert_eq!(expected.x, scalar(row,"standing_acceleration"));
        let scaled = a / max;
        let current = project(scaled.clamp_length_max(1.0), q).truncate();
        assert_eq!(current, expected);
        let clamp_local = project(scaled, q).clamp_length_max(1.0).truncate();
        let project_first = (project(a, q) / max).clamp_length_max(1.0).truncate();
        let reciprocal_world = project((a * max.recip()).clamp_length_max(1.0), q).truncate();
        let wide_source = source_amount(source_axes(a.as_dvec3()) * 100.0, source_axes(v.as_dvec3()) * 100.0,
                source_rotation(q), accel * 100.0, brake * 100.0).0;
        cases.push(json!({"frame":row["frame"],"dot":dot,"wide_dot":wide_dot,"capped":scaled.length_squared()>1.0,
            "actual":current,"clamp_local":clamp_local,"project_first":project_first,"reciprocal_world":reciprocal_world,
            "source_kernel_same_physical_inputs":wide_source}));
        if v.x <= 0.0 || v.z >= 0.0 || accel == brake { continue; }
        for z in [1.0_f32, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0] {
            let cancellation = -v.z * z / v.x;
            for offset in [-2_i32,-1,0,1,2] {
                let x = f32::from_bits((cancellation.to_bits() as i32 + offset) as u32);
                let edge = Vec3::new(x, 0.0, z);
                let dot32 = edge.dot(v);
                let dot64 = edge.as_dvec3().dot(v.as_dvec3());
                if dot32 != 0.0 || dot64 >= 0.0 { continue; }
                let target = acceleration_amount(edge,v,q,accel,brake);
                let forced = acceleration_amount(edge,v,q,brake,brake);
                let source_a = source_axes(edge.as_dvec3()) * 100.0;
                let source_v = source_axes(v.as_dvec3()) * 100.0;
                assert!(source_a.dot(source_v) < 0.0);
                let native = source_amount(source_a,source_v,source_rotation(q),accel*100.0,brake*100.0).0;
                assert_ne!(target,forced);
                edges.push(json!({"frame":row["frame"],"velocity":v,"acceleration":edge,"rotation":q,
                    "max_acceleration":accel,"max_braking":brake,"dot32":dot32,"dot64":dot64,"actual":target,
                    "braking_only":forced,"native":native,"native_velocity":source_v,"native_acceleration":source_a,
                    "native_rotation":source_rotation(q)}));
            }
        }
    }
    assert!(!edges.is_empty());
    std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec(&json!({
        "native_clamps":native_clamps,"native_brakes":native_brakes,"target":cases,"edges":edges
    })).unwrap()).unwrap();
    println!("Verified observed target Grounded and cancellation controls");
}
