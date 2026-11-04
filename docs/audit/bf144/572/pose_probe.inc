fn c572_release() -> usize {
    if std::env::var("ALS_C572_CASE").unwrap() == "coarse" { 62 } else { 92 }
}

fn c572_clock(probe: Res<C458Probe>, mut clock: ResMut<TimeUpdateStrategy>) {
    if (41..=51).contains(&probe.frame) {
        *clock = TimeUpdateStrategy::ManualDuration(Duration::from_millis(125));
    } else if probe.frame == 52 {
        let step = if c572_release() == 62 { 125_000 } else { 31_250 };
        *clock = TimeUpdateStrategy::ManualDuration(Duration::from_micros(step));
    }
}

fn c572_pelvis(poses: &serde_json::Value) -> serde_json::Value {
    let mut result = serde_json::Map::new();
    for name in ["rig_source", "rig_output", "root_result"] {
        let value = &poses[name];
        if let Some(bones) = value["pose"]["bones"].as_array() {
            let selected: Vec<_> = bones.iter()
                .filter(|bone| matches!(bone["name"].as_str(), Some("pelvis" | "root")))
                .collect();
            result.insert(name.to_string(), serde_json::json!({
                "updated": value["updated"], "timestamp": value["pose"]["timestamp"], "bones": selected
            }));
        }
    }
    result.into()
}
