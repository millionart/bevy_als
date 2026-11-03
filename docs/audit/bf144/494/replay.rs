use bevy::prelude::*;
use serde_json::{Value, json};
use std::path::Path;

mod scale {
    include!("../../../../tools/tmp/bf144/494/scale.rs");
}
#[allow(dead_code)]
mod foot {
    include!("../../../../tools/tmp/bf144/494/foot.rs");
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn array<const N: usize>(value: &Value) -> [f32; N] {
    std::array::from_fn(|i| value[i].as_f64().unwrap() as f32)
}

fn same_sign(candidate: Quat, actual: Quat) -> Quat {
    let dot = candidate.dot(actual);
    assert!(dot.is_finite() && dot != 0.0);
    if dot < 0.0 { -candidate } else { candidate }
}

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/494");
    let measurements = scale::run(&label);
    let rows = read(Path::new("tools/tmp/bf144/494/target.json"));
    let profile = read(Path::new("assets/ALS/Character/CR_Als.runtime.json"));
    let axes = [
        "foot_left_thigh_axis_pelvis_space",
        "foot_right_thigh_axis_pelvis_space",
    ]
    .map(|name| Vec3::from_array(array(&profile["target"][name])));
    let mut outputs = Vec::new();
    let mut changed_inputs = 0;
    for (index, row) in rows["rows"].as_array().unwrap().iter().enumerate() {
        let observed = &row["observation"];
        let actual = [0, 1]
            .map(|side| Quat::from_array(array(&observed["states"][side]["target_rotation"])));
        let uniform = [0, 1].map(|side| {
            let sample = &measurements["target"][index * 2 + side];
            assert_eq!(sample["query"], row["query"]);
            assert_eq!(sample["side"], side);
            assert_eq!(Quat::from_array(array(&sample["actual_q"])), actual[side]);
            same_sign(Quat::from_array(array(&sample["uniform_q"])), actual[side])
        });
        let normalized = [0, 1].map(|side| {
            if actual[side] == uniform[side] {
                actual[side]
            } else {
                actual[side].normalize()
            }
        });
        let baseline = foot::foot_case(row, axes, actual);
        assert_eq!(
            baseline["states"], observed["states"],
            "Actual virtual foot state query {}",
            row["query"]
        );
        assert_eq!(
            baseline["rig"], observed["rig"],
            "Actual virtual RigInput query {}",
            row["query"]
        );
        let unit = foot::foot_case(row, axes, normalized);
        let control = foot::foot_case(row, axes, uniform);
        assert_eq!(baseline["rig"]["location"], unit["rig"]["location"]);
        assert_eq!(baseline["rig"]["location"], control["rig"]["location"]);
        changed_inputs += (0..2)
            .filter(|&side| baseline["rig"]["rotation"][side] != control["rig"]["rotation"][side])
            .count();
        outputs
            .push(json!({"query":row["query"],"baseline":baseline,"unit":unit,"uniform":control}));
        if index % 32 == 0 {
            println!(
                "Replayed virtual foot scale controls at query {}",
                row["query"]
            );
        }
    }
    assert_eq!(outputs.len(), 158);
    std::fs::write(
        root.join(format!("consumers-{label}.json")),
        serde_json::to_vec(&json!({"rows":outputs,"changed_inputs":changed_inputs})).unwrap(),
    )
    .unwrap();
    assert!(
        changed_inputs > 0,
        "The isolated virtual parent scale rotation must reach RigInput"
    );
}
