use bevy_als::als::{
    input::AlsInputCommand,
    roll::{AlsRollState, ROLL_MONTAGE_SEQUENCE_LENGTH_SECS, ROLL_PLAY_RATE},
    start_stop::MoveDirection,
    state::{LocomotionState, Stance},
};
use serde_json::{Value, json};
use std::path::Path;

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/507");
    let requests: Value =
        serde_json::from_slice(&std::fs::read(root.join("roll-clock.request.json")).unwrap()).unwrap();
    let mut results = Vec::new();
    for sample in requests["samples"].as_array().unwrap() {
        assert_eq!(sample["play_rate"].as_f64().unwrap() as f32, ROLL_PLAY_RATE);
        let mut state = AlsRollState::default();
        let command = AlsInputCommand::default();
        assert!(state.start_from_landing(8.0, None, 0.0, Stance::Standing).started);
        let mut frames = Vec::new();
        for (index, step) in sample["steps"].as_array().unwrap().iter().enumerate() {
            let dt = step["delta_seconds"].as_f64().unwrap() as f32;
            if index == 0 {
                state.time = dt;
            } else {
                state.update(
                    &command,
                    (MoveDirection::Forward, 0.0),
                    &LocomotionState::Grounded,
                    &Stance::Standing,
                    dt,
                );
            }
            frames.push(json!({
                "delta":dt,
                "active":state.active,
                "time":state.time,
                "natural_blend_start":state.natural_blend_start,
                "position":(state.time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS),
                "weight":if state.active { state.weight() } else { 0.0 }
            }));
        }
        println!("Replayed {}", sample["name"]);
        results.push(json!({"name":sample["name"],"frames":frames}));
    }
    std::fs::write(
        root.join(format!("replay-{label}.json")),
        serde_json::to_vec(&results).unwrap(),
    )
    .unwrap();
}
