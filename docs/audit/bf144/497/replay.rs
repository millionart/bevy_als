use bevy::{
    math::DVec2,
    prelude::*,
    reflect::{PartialReflect, ReflectMut},
};
use bevy_als::extensions::{
    mesh_space_additive::apply_local_space_additive,
    reference_local_difference::make_local_difference,
    source_pose_nodes::{
        SourceAlphaMapping, SourceInputScaleBiasClampState, apply_source_input_scale_bias_clamp,
        blend_source_poses,
    },
};
use bevy_animation_graph::core::{
    animation_clip::EntityPath,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};
use serde_json::{Value, json};
use std::path::Path;

fn f(value: &Value) -> f32 {
    value.as_f64().unwrap() as f32
}
fn vector(value: &Value) -> Vec3 {
    Vec3::from_array(std::array::from_fn(|i| f(&value[i])))
}
fn point(value: &Value) -> DVec2 {
    DVec2::new(f(&value[0]) as f64, f(&value[1]) as f64)
}

fn weights(position: &Value, config: &Value) -> Vec<(usize, f32)> {
    let minimum = point(&config["min"]);
    let size = point(&config["max"]) - minimum;
    let normalize = |p: DVec2| (p - minimum) / size;
    let points = config["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| normalize(point(p)))
        .collect::<Vec<_>>();
    let position = normalize(point(position)).clamp(DVec2::ZERO, DVec2::ONE);
    let mut matches = Vec::new();
    for entry in config["triangles"].as_array().unwrap() {
        let indices: [usize; 3] = std::array::from_fn(|i| entry[i].as_u64().unwrap() as usize);
        let p = indices.map(|i| points[i]);
        let first = p[1] - p[0];
        let second = p[2] - p[0];
        let offset = position - p[0];
        let area = first.perp_dot(second);
        assert!(area > 0.0);
        let y = offset.perp_dot(second) / area;
        let z = first.perp_dot(offset) / area;
        let ws = [1.0 - y - z, y, z];
        if ws.iter().all(|w| *w > 1e-5) {
            matches.push(
                indices
                    .into_iter()
                    .zip(ws.map(|w| w as f32))
                    .collect::<Vec<_>>(),
            );
        }
    }
    assert_eq!(
        matches.len(),
        1,
        "Recorded Lean samples must be strictly inside one triangle"
    );
    let mut weights = matches.pop().unwrap();
    for last in (1..weights.len()).rev() {
        let mut smallest = 0;
        for index in 1..=last {
            if weights[smallest].1 > weights[index].1 {
                smallest = index;
            }
        }
        weights.swap(smallest, last);
    }
    let sum = weights.iter().map(|x| x.1).sum::<f32>();
    for (_, weight) in &mut weights {
        *weight /= sum;
    }
    let mut indices = weights.iter().map(|x| x.0).collect::<Vec<_>>();
    indices.sort_unstable();
    assert_eq!(indices, vec![0, 1, 4]);
    weights
}

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/497");
    let input: Value =
        serde_json::from_slice(&std::fs::read(root.join("evidence-input.json")).unwrap()).unwrap();
    let mapping = &input["mapping"];
    let mapping = SourceAlphaMapping {
        source_semantics: mapping["source_semantics"].as_bool().unwrap(),
        map_range: mapping["map_range"].as_bool().unwrap(),
        input_min: f(&mapping["input_min"]),
        input_max: f(&mapping["input_max"]),
        output_min: f(&mapping["output_min"]),
        output_max: f(&mapping["output_max"]),
        scale: f(&mapping["scale"]),
        bias: f(&mapping["bias"]),
        clamp_result: mapping["clamp_result"].as_bool().unwrap(),
        clamp_min: f(&mapping["clamp_min"]),
        clamp_max: f(&mapping["clamp_max"]),
        interpolate_result: mapping["interpolate_result"].as_bool().unwrap(),
        interpolate_speed_increasing: f(&mapping["interpolate_speed_increasing"]),
        interpolate_speed_decreasing: f(&mapping["interpolate_speed_decreasing"]),
    };
    let path = EntityPath::from(vec!["VB foot_l".to_owned()]);
    let id = path.id();
    let mut skeleton = Skeleton::default();
    skeleton.add_bone(path, Transform::IDENTITY, Transform::IDENTITY);
    skeleton.set_root(id);
    let pose = |scale| {
        let mut pose = Pose::default();
        pose.add_bone(
            BonePose {
                scale: Some(scale),
                ..default()
            },
            id,
        );
        pose
    };
    let scale = |pose: &Pose| {
        pose.get_bone(id)
            .and_then(|bone| bone.scale)
            .unwrap_or_default()
    };
    let reference = vector(&input["tracks"]["reference"]);
    let raw_reference = vector(&input["raw_reference"]);
    let leaf = |index| {
        let name = match index {
            0 => "center",
            1 => "forward",
            4 => "right",
            _ => unreachable!(),
        };
        pose(vector(&input["tracks"][name]))
    };
    let native = &input["native_alpha"];
    let mut native_state = SourceInputScaleBiasClampState::default();
    let _ = apply_source_input_scale_bias_clamp(
        &mut native_state,
        f(&native["previous_tick_weight"]),
        &mapping,
        0.0,
    );
    let native_replay = apply_source_input_scale_bias_clamp(
        &mut native_state,
        f(&native["raw"]),
        &mapping,
        f(&native["delta"]),
    );
    assert_eq!(native_replay, f(&native["tick_weight"]));
    let mut rows = Vec::new();
    let mut clock_mismatches = 0;
    for row in input["timeline"].as_array().unwrap() {
        if row["active"] == false {
            continue;
        }
        let data = &row["alpha"];
        let mut state = SourceInputScaleBiasClampState::default();
        if !data["previous"].is_null() {
            let ReflectMut::Struct(fields) = state.reflect_mut() else {
                unreachable!()
            };
            *fields
                .field_mut("initialized")
                .unwrap()
                .try_downcast_mut::<bool>()
                .unwrap() = data["previous"]["initialized"].as_bool().unwrap();
            *fields
                .field_mut("value")
                .unwrap()
                .try_downcast_mut::<f32>()
                .unwrap() = f(&data["previous"]["value"]);
        }
        let delta = f(&data["delta"]);
        let mapped =
            apply_source_input_scale_bias_clamp(&mut state, f(&data["raw"]), &mapping, delta);
        let actual = f(&data["mapped"]["value"]);
        clock_mismatches += usize::from(mapped != actual);
        let weights = weights(&row["position"], &input["configuration"]);
        let blend = |reference| {
            let inputs = weights
                .iter()
                .map(|&(index, w)| {
                    let delta = make_local_difference(&pose(reference), &leaf(index), &skeleton);
                    (pose(scale(&delta)), w)
                })
                .collect::<Vec<_>>();
            blend_source_poses(&inputs)
        };
        let current = blend(reference);
        let raw = blend(raw_reference);
        assert_eq!(
            scale(&current),
            vector(&row["delta"]),
            "Actual Lean delta at {}",
            row["query"]
        );
        let mut output = pose(vector(&row["base"]));
        let mut controlled = output.clone();
        apply_local_space_additive(&mut output, &current, actual, &skeleton);
        apply_local_space_additive(&mut controlled, &raw, actual, &skeleton);
        assert_eq!(scale(&output), vector(&row["output"]));
        if row["query"] == 105 {
            let mut native_alpha_only = pose(vector(&row["base"]));
            apply_local_space_additive(&mut native_alpha_only, &current, native_replay, &skeleton);
            assert_eq!(scale(&native_alpha_only), scale(&output));
        }
        rows.push(json!({"query":row["query"],"weights":weights,"mapped":mapped,"actual_alpha":actual,
                         "base":row["base"],"output":scale(&output),"raw_delta":scale(&raw),"raw_output":scale(&controlled)}));
        println!(
            "Replayed actual Lean clock and applied scale at {}",
            row["query"]
        );
    }
    std::fs::write(
        root.join(format!("replay-{label}.json")),
        serde_json::to_vec(&json!({"rows":rows,"clock_mismatches":clock_mismatches,"native_witness_alpha":native_replay})).unwrap(),
    )
    .unwrap();
    assert_eq!(
        clock_mismatches, 0,
        "Player delta must reproduce all observed Lean interpolation transitions"
    );
    let witness = rows.iter().find(|row| row["query"] == 105).unwrap();
    assert_eq!(
        witness["raw_delta"],
        json!([0.0, 0.0, 0.0]),
        "Raw reference must remove the observed Lean additive scale"
    );
    assert_eq!(witness["raw_output"], witness["base"]);
}
