use bevy::prelude::*;
use bevy_als::als::{dynamics::simulate_grounded_velocity_with_braking_friction, motion::view_relative_move_direction, params::{AlsMovementParams, GroundedDynamics}};
use serde_json::{Value, json};

fn main() {
    let root = "docs/audit/bf144/442c2";
    let state: Value = serde_json::from_slice(&std::fs::read(format!("{root}/cold-state.json")).unwrap()).unwrap();
    let row = &state["controller_snapshot"];
    let view = &row["player_view"];
    assert_eq!(view["manager_present"], true);
    assert_eq!(view["view_target_is_character"], true);
    assert_eq!(view["cache_time"].as_f64().unwrap(), 0.0);
    let actor_degrees = view["returned_rotation_degrees"]["yaw"].as_f64().unwrap();
    let control_degrees = row["control_rotation_degrees"]["yaw"].as_f64().unwrap();
    assert_eq!(actor_degrees, view["view_target_actor_rotation_degrees"]["yaw"].as_f64().unwrap());
    assert_eq!((actor_degrees, control_degrees), (0.0, 60.0));
    let native: Value = serde_json::from_slice(&std::fs::read(format!("{root}/native.json")).unwrap()).unwrap();
    let samples = native["geometry_query_probe"]["samples"].as_array().unwrap();
    let read_native = |name: &str| {
        let packet = &samples.iter().find(|r| r["input"]["name"] == name).unwrap()["move_input"];
        let v = &packet["cmc_input"];
        Vec3::new(v["x"].as_f64().unwrap() as f32, v["z"].as_f64().unwrap() as f32, v["y"].as_f64().unwrap() as f32)
    };
    let native_actor = read_native("cold-returned-actor");
    let native_control = read_native("cold-control");
    assert_eq!(native_actor, Vec3::X);
    assert_ne!(native_control, native_actor);
    let runtime_yaw = |degrees: f64| (-(degrees.to_radians()) - std::f64::consts::FRAC_PI_2) as f32;
    let source_angle = runtime_yaw(actor_degrees);
    let raw_angle = runtime_yaw(control_degrees);
    let selected_angle = raw_angle;
    let current = view_relative_move_direction(Vec2::Y, selected_angle);
    let field_control = view_relative_move_direction(Vec2::Y, source_angle);
    assert_ne!(current, field_control);
    assert!(current.x < 0.6 && current.z > 0.8);
    assert_eq!(field_control.x, native_actor.x);
    let params = AlsMovementParams::default();
    let dynamics = GroundedDynamics { acceleration: 20.0, braking_deceleration: 15.0, ground_friction: 5.0 };
    let plan = |direction| simulate_grounded_velocity_with_braking_friction(Vec3::ZERO, direction, 3.75, dynamics, &params.cmc, 1.0, 1.0 / 30.0).0;
    let native_plan = plan(native_actor);
    let current_plan = plan(current);
    let control_plan = plan(field_control);
    assert_ne!(current_plan, control_plan);
    assert_eq!(control_plan.x, native_plan.x);
    let observed: Value = serde_json::from_slice(&std::fs::read("docs/audit/bf144/442c1/runtime-restored/parameters.json").unwrap()).unwrap();
    let mut actual = Vec::new();
    for ordinal in [20usize, 21] {
        let r = &observed["rows"][ordinal];
        assert!(r["player_view_yaw"].is_null());
        assert_eq!(r["input_blocked"], false);
        assert_eq!(r["input_move"], json!([0.0, 1.0]));
        assert_eq!(r["consumed_yaw"], r["view_yaw"]);
        let yaw = r["consumed_yaw"].as_f64().unwrap() as f32;
        let direction = view_relative_move_direction(Vec2::Y, yaw);
        assert_eq!(json!(direction.to_array()), r["desired_motion"]);
        actual.push(json!({"ordinal":ordinal,"raw_yaw":yaw,"direction":direction.to_array(),"actor_yaw":r["actor_yaw"]}));
    }
    let result = json!({"gate":"PASS_COLD_ACTOR_CONTROL_FIELD_BOUNDARY_WITH_SHARED_MATH_AND_PUBLIC_CONSUMER", "actor_degrees":actor_degrees,"control_degrees":control_degrees,"source_actor_native":native_actor.to_array(),"source_control_native":native_control.to_array(),"runtime_actor_yaw":source_angle,"runtime_control_yaw":raw_angle,"current_direction":current.to_array(),"only_selected_angle_changed":field_control.to_array(),"native_actor_public_plan":native_plan.to_array(),"current_public_plan":current_plan.to_array(),"only_selected_angle_public_plan":control_plan.to_array(),"actual_playground_raw_fallback":actual,"remaining_angular_precision_residual":(field_control-native_actor).to_array(),"natural_nonzero_cold_input_claimed":false,"full_world_feedback_claimed":false});
    std::fs::write(std::env::args().nth(1).unwrap(), serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    println!("Captured same-state cold actor/control selector boundary and two actual consumers");
}
