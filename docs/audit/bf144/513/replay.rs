use bevy_als::als::params::GroundedDynamicsCurves;
use serde_json::{Value, json};
use std::path::Path;

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/513");
    let read = |name: &str| -> Value {
        serde_json::from_slice(&std::fs::read(root.join(name)).unwrap()).unwrap()
    };
    let facts = read("profile-facts.json");
    let mut output = Vec::new();
    for case in ["baseline", "normal"] {
        let profile = read(&format!("runtime-{case}/profile.json"));
        let original: GroundedDynamicsCurves = serde_json::from_value(profile["effective"].clone()).unwrap();
        let mut fixed = original.responsive.clone();
        let (channel, kind) = if case=="baseline" { ("braking_deceleration","responsive") }
                             else { ("acceleration","normal") };
        let tangent = facts["tangent_controls"].as_array().unwrap().iter()
            .find(|row|row["profile"]==kind && row["channel"]==channel).unwrap();
        let curve = if case=="baseline" { &mut fixed.braking_deceleration } else { &mut fixed.acceleration };
        assert_eq!(curve.keys[2].leave_tangent,tangent["current"].as_f64().unwrap() as f32);
        curve.keys[2].leave_tangent = tangent["source"].as_f64().unwrap() as f32;
        let mut changes = Vec::new();
        for row in read(&format!("runtime-{case}/parameters.json"))["dynamics"].as_array().unwrap() {
            let gait = row["gait_amount"].as_f64().unwrap() as f32;
            let sample = original.responsive.sample(gait);
            let replacement = fixed.sample(gait);
            let expected = [sample.acceleration,sample.braking_deceleration,sample.ground_friction];
            let observed: [f32;3] = serde_json::from_value(row["dynamics"].clone()).unwrap();
            assert_eq!(expected,observed);
            if sample!=replacement {
                changes.push(json!({"ordinal":row["ordinal"],"gait":gait,
                    "actual":expected,"native_key_only":[replacement.acceleration,
                        replacement.braking_deceleration,replacement.ground_friction]}));
            }
        }
        println!("Verified selected {case} profile on real motor inputs");
        output.push(json!({"case":case,"channel":channel,"changes":changes}));
    }
    std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec(&output).unwrap()).unwrap();
}
