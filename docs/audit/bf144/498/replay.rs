use bevy::math::DVec2;
use bevy_als::extensions::{
    SourceCurveName, animation_context::AnimationContextState,
    source_pose_nodes::blend_animation_contexts,
};
use serde_json::{Value, json};
use std::path::Path;

fn f(value: &Value) -> f32 {
    value.as_f64().unwrap() as f32
}

include!("../../../../tools/tmp/bf144/498/weights.rs");

fn active_direction_weights(value: &Value) -> [f32; 2] {
    let desired: [f32; 4] = std::array::from_fn(|i| f(&value[i]));
    assert_eq!(desired[1..3], [0.0, 0.0]);
    let sum = desired.iter().sum::<f32>();
    assert!(sum > 1e-5);
    [desired[0] / sum, desired[3] / sum]
}

fn weighted_gait(children: [f32; 2], weights: [f32; 2]) -> f32 {
    let name = SourceCurveName::from("PoseGait");
    let inputs = children.into_iter().zip(weights).map(|(gait, weight)| {
        let mut state = AnimationContextState::default();
        state.curves.insert(name.clone(), gait);
        (state, weight)
    });
    blend_animation_contexts(inputs).curves[&name]
}

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/498");
    let data: Value =
        serde_json::from_slice(&std::fs::read(root.join("evidence-input.json")).unwrap()).unwrap();
    let gait: [f32; 4] = std::array::from_fn(|i| f(&data["pose_gait"][i]));
    let mut records = Vec::new();
    let mut control_mismatches = 0;
    for row in data["rows"].as_array().unwrap() {
        let mut target_children = [0.0; 2];
        let mut native_children = [0.0; 2];
        let mut sample_records = Vec::new();
        let target_weights = active_direction_weights(&row["target_desired"]);
        let native_weights = active_direction_weights(&row["native_desired"]);
        for (i, direction) in row["directions"].as_array().unwrap().iter().enumerate() {
            let config = &direction["target"];
            let target_samples = weights(&config["position"], config);
            let native_samples = direction["native_samples"]
                .as_array()
                .unwrap()
                .iter()
                .map(|sample| {
                    (
                        sample["index"].as_u64().unwrap() as usize,
                        f(&sample["weight"]),
                    )
                })
                .collect::<Vec<_>>();
            let accumulate = |samples: &[(usize, f32)]| {
                samples
                    .iter()
                    .map(|&(index, weight)| gait[index] * weight)
                    .sum::<f32>()
            };
            target_children[i] = accumulate(&target_samples);
            native_children[i] = accumulate(&native_samples);
            assert_eq!(target_children[i], f(&direction["target_gait"]));
            assert_eq!(native_weights[i], f(&direction["native_direction_weight"]));
            sample_records.push(json!({"name":direction["name"],"target_weights":target_samples,"native_weights":native_samples,
                "target_gait":target_children[i],"native_gait":native_children[i]}));
        }
        let target = weighted_gait(target_children, target_weights);
        let source = weighted_gait(native_children, native_weights);
        let controlled = weighted_gait(target_children, native_weights);
        let source_children_only = weighted_gait(native_children, target_weights);
        assert_eq!(target, f(&row["target_root_gait"]));
        assert_eq!(source, f(&row["native_root_gait"]));
        assert_eq!(
            (target - 1.0).clamp(0.0, 1.0),
            f(&row["target_running_consumer"])
        );
        assert_eq!(
            (source - 1.0).clamp(0.0, 1.0),
            f(&row["native_running_consumer"])
        );
        control_mismatches += usize::from(controlled != source);
        records.push(json!({"query":row["query"],"samples":sample_records,"target_weights":target_weights,"native_weights":native_weights,
            "target":target,"source":source,"native_weights_only":controlled,"native_children_only":source_children_only}));
        println!(
            "Replayed actual source and target directional gait at {}",
            row["query"]
        );
    }
    std::fs::write(
        root.join(format!("replay-{label}.json")),
        serde_json::to_vec(&json!({"rows":records,"control_mismatches":control_mismatches}))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        control_mismatches, 0,
        "Source direction weights must restore the observed gait curves"
    );
}
