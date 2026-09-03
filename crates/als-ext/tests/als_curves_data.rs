use als_ext::parse_curves;
use std::collections::BTreeMap;
use std::fs;

const ROTATE_90_LEFT: &str = "../../assets/animations/RotateInPlace/A_Als_Rotate_90_Left.gltf";
const CROUCH_ROTATE_90_LEFT: &str =
    "../../assets/animations/RotateInPlace/A_Als_Crouch_Rotate_90_Left.gltf";
const CROUCH_ROTATE_90_RIGHT: &str =
    "../../assets/animations/RotateInPlace/A_Als_Crouch_Rotate_90_Right.gltf";
const CROUCH_TURN_90_LEFT: &str =
    "../../assets/animations/TurnInPlace/A_Als_Crouch_Turn_90_Left.gltf";
const CROUCH_TURN_90_RIGHT: &str =
    "../../assets/animations/TurnInPlace/A_Als_Crouch_Turn_90_Right.gltf";
const CROUCH_TURN_180_LEFT: &str =
    "../../assets/animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.gltf";
const CROUCH_TURN_180_RIGHT: &str =
    "../../assets/animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.gltf";
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

fn curve_integral(path: &str, name: &str) -> f32 {
    let ext = read_extension(path);
    let (_, value) = ext.iter().next().expect("one animation entry");
    let curves = parse_curves(value).expect("parses");
    let curve = curves
        .0
        .get(name)
        .unwrap_or_else(|| panic!("{name} is missing"));
    curve
        .times
        .windows(2)
        .zip(curve.values.windows(2))
        .map(|(times, values)| (values[0] + values[1]) * 0.5 * (times[1] - times[0]))
        .sum()
}

fn rotation_yaw_speed_integral(path: &str) -> f32 {
    curve_integral(path, "RotationYawSpeed")
}

#[test]
fn crouch_rotate_yaw_speed_curves_preserve_source_turn_directions() {
    let left = rotation_yaw_speed_integral(CROUCH_ROTATE_90_LEFT);
    let right = rotation_yaw_speed_integral(CROUCH_ROTATE_90_RIGHT);
    assert!((left - (-90.0)).abs() <= 5.0, "left yaw integral is {left}");
    assert!((right - 90.0).abs() <= 5.0, "right yaw integral is {right}");
}

#[test]
fn crouch_turn_target_yaw_speed_curves_preserve_source_turn_side_after_basis_conversion() {
    for (path, side) in [
        (CROUCH_ROTATE_90_LEFT, -1.0),
        (CROUCH_ROTATE_90_RIGHT, 1.0),
        (CROUCH_TURN_90_LEFT, -1.0),
        (CROUCH_TURN_90_RIGHT, 1.0),
        (CROUCH_TURN_180_LEFT, -1.0),
        (CROUCH_TURN_180_RIGHT, 1.0),
    ] {
        let source = curve_integral(path, "RotationYawSpeed");
        let target = curve_integral(path, "RotationYawSpeedTarget");
        assert!(
            source * side > 20.0,
            "source yaw side is wrong for {path}: {source}"
        );
        assert!(
            target * -side > 20.0 && (target + source).abs() <= 0.01,
            "target yaw basis conversion differs for {path}: source={source} target={target}"
        );
    }
}

#[test]
fn crouch_target_yaw_speed_curves_preserve_the_source_cubic_shape_between_keys() {
    for path in [
        CROUCH_ROTATE_90_LEFT,
        CROUCH_ROTATE_90_RIGHT,
        CROUCH_TURN_90_LEFT,
        CROUCH_TURN_90_RIGHT,
        CROUCH_TURN_180_LEFT,
        CROUCH_TURN_180_RIGHT,
    ] {
        let ext = read_extension(path);
        let (_, value) = ext.iter().next().expect("one animation entry");
        let curves = parse_curves(value).expect("parses");
        let source = curves.0.get("RotationYawSpeed").expect("source curve");
        let target = curves
            .0
            .get("RotationYawSpeedTarget")
            .expect("target curve");
        for times in source.times.windows(2) {
            let time = (times[0] + times[1]) * 0.5;
            let expected = -source.sample(time);
            let actual = target.sample(time);
            assert!(
                (actual - expected).abs() <= 1e-3,
                "target cubic sample differs for {path} at {time}: source={expected} target={actual}"
            );
        }
    }
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
