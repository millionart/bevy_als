use bevy::{
    math::{DQuat, DVec3},
    prelude::*,
};
use bevy_als::{
    als::animation::{inv_exp_approx, velocity_blend_direction},
    extensions::{
        control_rig_hierarchy::{RigHierarchy, RigTransform},
        control_rig_math::RigValue,
        control_rig_program::RigDefinition,
        control_rig_units::RigUnitContext,
    },
};
use serde_json::{Value, json};
use std::path::Path;

const RETAIN_WIDE_ROTATION: bool = true;
const RETAIN_DAMPER_HISTORY: bool = true;

fn read(path: &str) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn array<const N: usize>(v: &Value) -> [f32; N] {
    std::array::from_fn(|i| v[i].as_f64().unwrap() as f32)
}

fn amounts(direction: Vec3) -> [f32; 4] {
    [
        (-direction.z).max(0.0),
        direction.z.max(0.0),
        (-direction.x).max(0.0),
        direction.x.max(0.0),
    ]
}

fn expanded_unrotate(v: DVec3, q: DQuat) -> DVec3 {
    let u = -q.xyz();
    v * (q.w * q.w - u.dot(u)) + u * (2.0 * v.dot(u)) + u.cross(v) * (2.0 * q.w)
}

fn main() {
    let root = Path::new("docs/audit/bf144/503");
    let label = std::env::args().nth(1).unwrap();
    let mut rows = read("docs/audit/bf144/499/evidence-input.json")
        .as_array()
        .unwrap()
        .clone();
    rows.insert(0, read("docs/audit/bf144/501/evidence-input.json"));
    let mut baseline = [0.0_f32; 4];
    let mut widened = baseline;
    let mut reports = Vec::new();
    for row in rows {
        let target = &row["target"];
        let v = Vec3::from_array(array(&target["velocity"]));
        let q = Quat::from_array(array(&target["rotation"]));
        let dt = target["delta"].as_f64().unwrap() as f32;
        let alpha = 1.0 - inv_exp_approx(std::f32::consts::LN_2 / (0.1_f32 + 1e-8) * dt);
        assert_eq!(baseline, array(&target["previous"]));
        let normal = amounts(velocity_blend_direction(v, q));
        let wide = expanded_unrotate(v.as_dvec3(), q.as_dquat()).as_vec3();
        let wide = if RETAIN_WIDE_ROTATION {
            amounts(wide / wide.abs().element_sum())
        } else {
            normal
        };
        baseline = std::array::from_fn(|i| baseline[i].lerp(normal[i], alpha));
        widened = std::array::from_fn(|i| widened[i].lerp(wide[i], alpha));
        assert_eq!(baseline, array(&target["actual"]));
        reports.push(json!({"query":row["query"],"baseline":baseline,"widened":widened}));
        println!(
            "Replayed continuous actual target velocity blend at {}",
            row["query"]
        );
    }
    std::fs::write(
        root.join(format!("replay-{label}.json")),
        serde_json::to_vec(&reports).unwrap(),
    )
    .unwrap();
    assert_eq!(
        reports
            .iter()
            .filter(|row| row["baseline"] != row["widened"])
            .map(|row| row["query"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [101]
    );
    replay_damper(root, &label);
}

fn replay_damper(root: &Path, label: &str) {
    let definition: RigDefinition = serde_json::from_value(read(
        "docs/audit/bf144/503/runtime-baseline/rig-definition.json",
    ))
    .unwrap();
    let data = read("docs/audit/bf144/503/damper-input.json");
    let mut hierarchy =
        RigHierarchy::new(definition.bones.clone(), definition.translation_tolerance).unwrap();
    let mut results = Vec::new();
    let mut mismatches = 0;
    for row in data.as_array().unwrap() {
        let observed = row["observed"].as_array().unwrap();
        let mut outputs = Vec::new();
        for (index, case) in observed.iter().enumerate() {
            let previous = if RETAIN_DAMPER_HISTORY {
                &case["previous"]
            } else {
                &observed[0]["previous"]
            };
            let mut args: Vec<RigValue> = serde_json::from_value(case["expected"].clone()).unwrap();
            args[2] = serde_json::from_value(previous.clone()).unwrap();
            let mut context = RigUnitContext {
                hierarchy: &mut hierarchy,
                curves: &default(),
                game_world: true,
                construction: false,
                delta_seconds: row["delta"].as_f64().unwrap(),
                geometry: &definition.geometry,
                to_world: RigTransform::default(),
                line_trace: None,
            };
            let name = row["function"].as_str().unwrap();
            context.call(name, &mut args).unwrap();
            mismatches += usize::from(json!(&args) != case["expected"]);
            let mut restored: Vec<RigValue> =
                serde_json::from_value(case["expected"].clone()).unwrap();
            restored[2] = serde_json::from_value(observed[0]["previous"].clone()).unwrap();
            context.call(name, &mut restored).unwrap();
            assert_eq!(json!(restored), observed[0]["expected"]);
            outputs.push(json!({"case":index,"actual":args,"baseline_history_restores":true}));
        }
        results.push(json!({"query":row["query"],"pc":row["pc"],"outputs":outputs}));
        println!(
            "Replayed actual pole damper state at {} pc {}",
            row["query"], row["pc"]
        );
    }
    std::fs::write(
        root.join(format!("damper-{label}.json")),
        serde_json::to_vec(&json!({"rows":results,"mismatches":mismatches})).unwrap(),
    )
    .unwrap();
    assert_eq!(
        mismatches, 0,
        "Observed pole histories must reproduce both actual damper outputs"
    );
}
