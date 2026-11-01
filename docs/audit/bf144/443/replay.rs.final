use bevy::prelude::*;
use serde_json::{Value, json};

fn vector(v: &Value) -> Vec3 {
    Vec3::new(v[0].as_f64().unwrap() as f32, v[1].as_f64().unwrap() as f32, v[2].as_f64().unwrap() as f32)
}
fn same_horizontal(a: Vec3, b: Vec3) -> bool { a.x == b.x && a.z == b.z }

fn main() {
    let root = "docs/audit/bf144/443";
    let data: Value = serde_json::from_slice(&std::fs::read(format!("{root}/runtime-baseline/parameters.json")).unwrap()).unwrap();
    let source: Value = serde_json::from_slice(&std::fs::read(format!("{root}/source-phases.json")).unwrap()).unwrap();
    let request: Value = serde_json::from_slice(&std::fs::read(format!("{root}/source.request.json")).unwrap()).unwrap();
    let frames = source["character_animation_probe"]["cases"][0]["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 240);
    let mut rows = Vec::new();
    let mut mismatches = Vec::new();
    let mut motor_roundtrip_differences = 0;
    for i in 0..240 {
        let r = &data["rows"][i];
        let m = &data["after_motors"][i];
        let p = &data["camera_after_post_update"][i];
        assert_eq!(p["frame_delta_ns"].as_u64().unwrap(), 33_333_335);
        assert_eq!(p["physics_delta_ns"].as_u64().unwrap(), 33_333_333);
        assert_eq!(vector(&p["center_of_mass"]), Vec3::ZERO);
        assert_eq!(vector(&r["support_velocity"]), Vec3::ZERO);
        assert_eq!(r["locomotion"], "Grounded");
        let dt = r["delta"].as_f64().unwrap() as f32;
        let count = p["substeps"].as_u64().unwrap() as u32;
        let start = vector(&m["position"]);
        let velocity = vector(&m["velocity"]);
        let step = velocity * (dt / count as f32);
        let mut delta = Vec3::ZERO;
        for _ in 0..count { delta += step; }
        let predicted_position = start + delta;
        let post_position = vector(&p["position"]);
        let predicted_velocity = (post_position - vector(&r["position"])) / (r["simulation_delta"].as_f64().unwrap() as f32);
        let post_velocity = vector(&p["velocity"]);
        let exact = same_horizontal(predicted_position, post_position) && same_horizontal(predicted_velocity, post_velocity);
        if !exact { mismatches.push(json!({"ordinal":i,"position_predicted":predicted_position.to_array(),"position_actual":post_position.to_array(),"velocity_predicted":predicted_velocity.to_array(),"velocity_actual":post_velocity.to_array()})); }
        motor_roundtrip_differences += (!same_horizontal(velocity, vector(&r["planned_velocity"]))) as usize;
        let reconstructed = vector(&r["planned_delta"]) / dt;
        let old = vector(&r["physical_velocity"]);
        let reconstructed_motor = old + (reconstructed - old);
        assert!(same_horizontal(reconstructed_motor, velocity), "Motor displacement roundtrip did not match actual");
        let f = &frames[i];
        assert_eq!(f["gravity_cm_s2"].as_f64().unwrap(), -981.0);
        assert_eq!(f["movement_before_tick"]["movement_mode_name"], "MOVE_Walking");
        assert_eq!(f["movement_after_tick"]["movement_mode_name"], "MOVE_Walking");
        for axis in ["x", "y", "z"] {
            assert_eq!(f["movement_before_tick"]["pending_input_world"][axis].as_f64().unwrap(), request["cases"][0]["frames"][i]["movement_input_world"][axis].as_f64().unwrap());
        }
        let position_cm = &f["actor_transform"]["translation"];
        let velocity_cm = &f["movement_after_tick"]["velocity_cm_s"];
        let source_position = [position_cm["x"].as_f64().unwrap()*0.01, position_cm["z"].as_f64().unwrap()*0.01, position_cm["y"].as_f64().unwrap()*0.01];
        let source_velocity = [velocity_cm["x"].as_f64().unwrap()*0.01, velocity_cm["z"].as_f64().unwrap()*0.01, velocity_cm["y"].as_f64().unwrap()*0.01];
        rows.push(json!({"ordinal":i,"post_position_predicted":predicted_position.to_array(),"post_velocity_predicted":predicted_velocity.to_array(),"horizontal_exact":exact,"single_substep_world_position":(start+velocity*dt).to_array(),"zero_origin_six_substep_delta":delta.to_array(),"zero_origin_single_delta":(velocity*dt).to_array(),"source_position_m_double":source_position,"source_velocity_m_double":source_velocity}));
    }
    let result = json!({"gate":if mismatches.is_empty(){"PASS_ACTUAL240_HORIZONTAL_PHYSICS_AND_SOURCE_INPUT_HISTORY"}else{"FAIL_ACTUAL_HORIZONTAL_REPLAY"},"rows":rows,"mismatches":mismatches,"motor_vs_raw_plan_velocity_differences":motor_roundtrip_differences,"source_target_origins_matched":false,"vertical_motor_correction_deferred":true,"new_difference_claimed":false});
    std::fs::write(std::env::args().nth(1).unwrap(), serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    assert!(mismatches.is_empty(), "Horizontal physics replay failed");
    println!("Replayed actual240 horizontal integration and matched source input history");
}
