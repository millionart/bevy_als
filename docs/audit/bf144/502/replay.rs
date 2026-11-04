use bevy::{
    math::{DQuat, DVec3},
    prelude::*,
};
use bevy_als::als::animation::inv_exp_approx;
use serde_json::{Value, json};
use std::path::Path;

include!("../../../../tools/tmp/bf144/502/math.rs");
include!("../../../../tools/tmp/bf144/502/basis.rs");

const USE_PRECAST_METERS: bool = true;
const WIDEN_RUNTIME_ROTATION: bool = true;

fn expanded_unrotate_f32(v: Vec3, q: Quat) -> Vec3 {
    let u = -q.xyz();
    v * (q.w * q.w - u.dot(u)) + u * (2.0 * v.dot(u)) + u.cross(v) * (2.0 * q.w)
}

fn runtime_direction(local: Vec3) -> [f32; 4] {
    let v = local / local.abs().element_sum();
    [(-v.z).max(0.0), v.z.max(0.0), (-v.x).max(0.0), v.x.max(0.0)]
}

fn body_to_source(v: Vec3) -> Vec3 {
    Vec3::new(-v.z, v.x, v.y)
}

fn cross_unrotate(v: DVec3, q: DQuat) -> DVec3 {
    let u = -q.xyz();
    let t = u.cross(v) * 2.0;
    v + q.w * t + u.cross(t)
}

fn expanded_unrotate(v: DVec3, q: DQuat) -> DVec3 {
    let u = -q.xyz();
    v * (q.w * q.w - u.dot(u)) + u * (2.0 * v.dot(u)) + u.cross(v) * (2.0 * q.w)
}

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/502");
    let first: Value =
        serde_json::from_slice(&std::fs::read("docs/audit/bf144/501/evidence-input.json").unwrap())
            .unwrap();
    let first_control: Value = serde_json::from_slice(
        &std::fs::read("docs/audit/bf144/501/replay-restored.json").unwrap(),
    )
    .unwrap();
    let mut data: Vec<Value> =
        serde_json::from_slice(&std::fs::read("docs/audit/bf144/499/evidence-input.json").unwrap())
            .unwrap();
    data.insert(0, first);
    let modes: Vec<[u8; 4]> =
        serde_json::from_slice(&std::fs::read("tools/tmp/bf144/502/modes.json").unwrap()).unwrap();
    let mut rows = Vec::new();
    for row in data {
        let source = &row["source"];
        let v = DVec3::from_array(a64(&source["velocity"]));
        let q = DQuat::from_array(a64(&source["rotation"]));
        verify_basis(q);
        let native: [f32; 4] = a32(&source["actual"]["amounts"]);
        let previous = a32(&source["previous"]["amounts"]);
        let delta = source["delta"].as_f64().unwrap() as f32;
        let rv = project_axes(v) * 0.01;
        let rq = runtime_rotation(q);
        let fv = rv.as_vec3();
        let fq = rq.as_quat();
        let source_v_projected = project_axes(fv.as_dvec3()) * 100.0;
        let source_q_projected = source_rotation(fq.as_dquat());
        let mut reports = Vec::new();
        for mode in &modes {
            let alpha = native_alpha(delta, mode[2]);
            assert_eq!(alpha, target_alpha(delta));
            let finish = |local| {
                native_lerp(
                    previous,
                    native_direction(local, true, mode[1]),
                    alpha,
                    mode[3] != 0,
                )
            };
            let raw = native_local_f64(v, q, mode[0]);
            assert_eq!(raw.as_vec3(), native_local(v, q, mode[0]));
            assert_eq!(finish(raw.as_vec3()), native);
            let after_cast = finish((raw.as_vec3().as_dvec3() * 0.01).as_vec3());
            let mut grid = Vec::new();
            for mask in 0..8 {
                let v = if mask & 1 != 0 { source_v_projected } else { v };
                let q = if mask & 2 != 0 { source_q_projected } else { q };
                let local = native_local_f64(v, q, mode[0]);
                let local = if USE_PRECAST_METERS && mask & 4 != 0 {
                    (local * 0.01).as_vec3()
                } else {
                    local.as_vec3()
                };
                grid.push(json!({"mask":mask,"local":local,"seed":finish(local)}));
            }
            let cross64 = body_to_source(cross_unrotate(fv.as_dvec3(), fq.as_dquat()).as_vec3());
            let expanded64 = body_to_source(if WIDEN_RUNTIME_ROTATION {
                expanded_unrotate(fv.as_dvec3(), fq.as_dquat()).as_vec3()
            } else {
                expanded_unrotate_f32(fv, fq)
            });
            let u = -fq.xyz();
            let twice = u.cross(fv) * 2.0;
            let cross32 = body_to_source(fv + fq.w * twice + u.cross(twice));
            let runtime32 = body_to_source(fq.inverse() * fv);
            assert_eq!(runtime32, body_to_source(expanded_unrotate_f32(fv, fq)));
            reports.push(
                json!({"mode":mode,"native":native,"meter_after_native_cast":after_cast,
                 "grid":grid,"cross64_local":cross64,"cross32_local":cross32,
                 "expanded64_local":expanded64,"runtime32_local":runtime32,
                 "cross64":finish(cross64),"cross32":finish(cross32),
                 "expanded64":finish(expanded64),"runtime32":finish(runtime32)}),
            );
        }
        let u = fq.inverse() * fv;
        let unit = u * u.length_squared().sqrt().recip();
        let v = unit * unit.abs().element_sum().recip();
        let runtime = target_lerp(
            previous,
            [(-v.z).max(0.0), v.z.max(0.0), (-v.x).max(0.0), v.x.max(0.0)],
            target_alpha(delta),
        );
        assert_eq!(reports[0]["runtime32"], json!(runtime));
        if row["query"] == 99 {
            assert_eq!(
                json!(runtime),
                first_control["controls"][3]["with_euclidean_reciprocal"]
            );
        }
        let actual = &row["target"];
        let actual_v = Vec3::from_array(a32(&actual["velocity"]));
        let actual_q = Quat::from_array(a32(&actual["rotation"]));
        let actual_local = actual_q.inverse() * actual_v;
        assert_eq!(actual_local, expanded_unrotate_f32(actual_v, actual_q));
        let actual_previous = a32(&actual["previous"]);
        let actual_amounts = target_lerp(
            actual_previous,
            runtime_direction(actual_local),
            target_alpha(delta),
        );
        assert_eq!(json!(actual_amounts), actual["actual"]);
        let widened_local = if WIDEN_RUNTIME_ROTATION {
            expanded_unrotate(actual_v.as_dvec3(), actual_q.as_dquat()).as_vec3()
        } else {
            actual_local
        };
        let actual_widened = target_lerp(
            actual_previous,
            runtime_direction(widened_local),
            target_alpha(delta),
        );
        rows.push(json!({"query":row["query"],"runtime_combined":runtime,"actual_target":actual_amounts,"actual_target_widened":actual_widened,"reports":reports}));
        println!(
            "Replayed held-input precision and unit controls at {}",
            row["query"]
        );
    }
    std::fs::write(
        root.join(format!("replay-{label}.json")),
        serde_json::to_vec(&json!(rows)).unwrap(),
    )
    .unwrap();
    assert_ne!(
        rows[0]["reports"][0]["grid"][4]["seed"], rows[0]["reports"][0]["native"],
        "The isolated local unit conversion must change the first seed"
    );
    assert!(
        rows.iter()
            .any(|row| row["reports"][0]["runtime32"] != row["reports"][0]["expanded64"]),
        "Holding the expanded rotation formula and widening its arithmetic must change a seed"
    );
}
