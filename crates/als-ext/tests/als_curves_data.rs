use als_ext::parse_curves;
use std::collections::BTreeMap;
use std::fs;

const ROTATE_90_LEFT: &str = "../../assets/animations/RotateInPlace/A_Als_Rotate_90_Left.gltf";
const TORCH_POSES: &str = "../../assets/animations/Overlays/Torch/A_Als_Torch_Poses.gltf";
const WALK_FORWARD: &str = "../../assets/animations/Grounded/WalkRun/A_Als_Walk_Forward.gltf";

fn read_extension(path: &str) -> BTreeMap<String, serde_json::Value> {
    let text = fs::read_to_string(path).expect("gltf file readable");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("valid gltf json");
    let used = doc["extensionsUsed"].as_array().expect("extensionsUsed");
    assert!(
        used.iter().any(|v| v == "ALS_float_curves"),
        "{path} missing extensionsUsed entry"
    );
    let mut out = BTreeMap::new();
    for anim in doc["animations"].as_array().expect("animations") {
        let name = anim["name"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "animation".into());
        if let Some(ext) = anim.pointer("/extensions/ALS_float_curves") {
            out.insert(name, ext.clone());
        }
    }
    assert!(!out.is_empty(), "{path} carries no ALS_float_curves data");
    out
}

#[test]
fn rotate_90_left_yaw_speed_integrates_to_minus_90_degrees() {
    let ext = read_extension(ROTATE_90_LEFT);
    let (_, value) = ext.iter().next().expect("one animation entry");
    let curves = parse_curves(value).expect("parses");
    let curve = curves
        .0
        .get("RotationYawSpeed")
        .expect("RotationYawSpeed present");
    assert!(curve.times.len() >= 20, "dense keys expected");

    let mut integral = 0.0;
    for i in 0..curve.times.len() - 1 {
        let dt = curve.times[i + 1] - curve.times[i];
        assert!(dt > 0.0, "times strictly increasing");
        integral += (curve.values[i] + curve.values[i + 1]) * 0.5 * dt;
    }
    assert!(
        (integral - (-90.0)).abs() <= 5.0,
        "yaw speed integral {integral} deviates from -90 deg"
    );
}

#[test]
fn walk_forward_foot_planted_stays_in_expected_range() {
    let ext = read_extension(WALK_FORWARD);
    let (_, value) = ext.iter().next().unwrap();
    let curves = parse_curves(value).unwrap();
    let foot = curves.0.get("FootPlanted").expect("FootPlanted present");
    let min_v = foot.values.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_v = foot
        .values
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        min_v >= -1.05 && max_v <= 1.05,
        "FootPlanted range [{min_v},{max_v}] outside normalized [-1,1]"
    );
}

#[test]
fn torch_poses_exposes_layer_groups_for_dynamic_masks() {
    let ext = read_extension(TORCH_POSES);
    let (_, value) = ext.iter().next().unwrap();
    let curves = parse_curves(value).unwrap();
    for name in ["LayerArmRight", "LayerSpineAdditive", "LayerHeadAdditive"] {
        let c = curves
            .0
            .get(name)
            .unwrap_or_else(|| panic!("missing curve {name}"));
        assert!(!c.times.is_empty(), "{name} has no keys");
    }
}
