use bevy::prelude::Vec3;
use bevy_als::{als::motion::movement_sweep_travel, extensions::motion::falling_slide};
use serde_json::{Value, json};
use std::path::Path;

const REPLACE_NORMAL: bool = true;

fn main() {
    let root = Path::new("docs/audit/bf144/521");
    let read = |path: &str| -> Value {
        serde_json::from_slice(&std::fs::read(root.join(path)).unwrap()).unwrap()
    };
    let trace = read("runtime-wall-162-aligned-air/wall-trace.json");
    let hit = &trace["hits"][0];
    let vec = |value: &Value| -> Vec3 { serde_json::from_value(value.clone()).unwrap() };
    let scalar = |value: &Value| value.as_f64().unwrap() as f32;
    let native = read("geometry.native.json");
    let native_hit = &native["geometry_query_probe"]["samples"][1]["capsule_sweep"];
    let normal = &native_hit["normal"];
    let source_normal = Vec3::new(
        scalar(&normal["x"]),
        scalar(&normal["z"]),
        scalar(&normal["y"]),
    );
    let target_normal = vec(&hit["normal"]);
    let incoming = vec(&hit["incoming"]);
    let time = scalar(&hit["remaining_time"]);
    let project = |normal, remaining| falling_slide(incoming * remaining, normal, None) / remaining;
    let baseline = project(target_normal, time);
    assert_eq!(baseline, vec(&trace["actual_velocity"]));
    assert_eq!(
        falling_slide(incoming * time, target_normal, None),
        vec(&hit["slide"])
    );
    let controlled_normal = if REPLACE_NORMAL {
        source_normal.normalize()
    } else {
        target_normal
    };
    let distance = scalar(&hit["distance"]);
    let source_distance = (native_hit["distance_cm"].as_f64().unwrap() / 100.0) as f32;
    let travel = movement_sweep_travel(distance, source_distance);
    let source_distance_time = scalar(&trace["air_time"]) * (1.0 - travel / distance);
    let result = json!({
        "baseline": baseline,
        "source_normal_only": project(controlled_normal, time),
        "source_distance_only": project(target_normal, source_distance_time),
        "source_distance_and_normal": project(controlled_normal, source_distance_time),
        "source_distance_travel_m": travel,
        "source_distance_remaining_s": source_distance_time,
        "target_remaining_s": time,
        "source_normal_used": controlled_normal
    });
    let label = std::env::args().nth(1).unwrap();
    std::fs::write(
        root.join(format!("replay-{label}.json")),
        format!("{result}\n"),
    )
    .unwrap();
    assert_ne!(
        result["baseline"], result["source_normal_only"],
        "Native normal replacement must affect the live slide"
    );
    println!("Matched live slide and separated the native normal and distance controls");
}
