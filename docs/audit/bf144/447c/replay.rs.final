use bevy_als::als::{head::{AlsHeadState,AlsHeadUpdateInput,AlsHeadSettings,update_head_state},state::RotationMode};
use serde_json::{Value,json};
fn f(v:&Value)->f32{v.as_f64().unwrap()as f32}
fn head(v:&Value)->AlsHeadState{AlsHeadState{initialization_required:v["initialization_required"].as_bool().unwrap(),switching_look_sides:v["switching_look_sides"].as_bool().unwrap(),pitch_angle:f(&v["pitch_angle"]),yaw_angle:f(&v["yaw_angle"]),yaw_velocity:f(&v["yaw_velocity"]),yaw_amount:f(&v["yaw_amount"])}}
fn snapshot(v:AlsHeadState)->Value{json!({"initialization_required":v.initialization_required,"switching_look_sides":v.switching_look_sides,"pitch_angle":v.pitch_angle,"yaw_angle":v.yaw_angle,"yaw_velocity":v.yaw_velocity,"yaw_amount":v.yaw_amount})}
fn main(){
    let root=std::path::Path::new("docs/audit/bf144/447c");
    let label=std::env::args().nth(1).unwrap();
    let target:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/447b/runtime-restored/parameters.json").unwrap()).unwrap();
    let native:Value=serde_json::from_slice(&std::fs::read(root.join("native-head.json")).unwrap()).unwrap();
    let before=target["animation_before"].as_array().unwrap();let after=target["animation_after"].as_array().unwrap();
    assert_eq!(before.len(),319);assert_eq!(after.len(),319);assert_eq!(native["rows"].as_array().unwrap().len(),318);
    let mut state=AlsHeadState::default();let mut first_control=None;
    for (index,(a,b))in before.iter().zip(after).enumerate(){
        let h=&a["head_values"];let scalar=|key:&str|f(&h["floats"][key]["resolved"]);let boolean=|key:&str|h["booleans"][key]["resolved"].as_bool().unwrap();
        let input=AlsHeadUpdateInput{delta_seconds:if index==0{0.0}else{scalar("animation.delta_seconds")},delta_seconds_without_time_dilation:scalar("animation.delta_seconds_without_time_dilation"),first_person:boolean("view_mode.first_person"),rotation_mode:if boolean("rotation_mode.velocity_direction"){RotationMode::VelocityDirection}else{RotationMode::ViewDirection},has_input:boolean("locomotion_state.b_has_input"),input_yaw_world:scalar("locomotion_state.input_yaw_angle_world_space"),target_yaw_world:scalar("locomotion_state.target_yaw_angle_world_space"),body_yaw_world:scalar("locomotion_state.rotation_yaw"),body_yaw_velocity_world:scalar("locomotion_state.yaw_velocity_world_space"),view_yaw_angle:scalar("view_state.yaw_angle"),view_pitch_angle:scalar("view_state.pitch_angle"),movement_base_relative_yaw:if boolean("movement_base.b_has_relative_rotation"){scalar("movement_base.delta_yaw")}else{0.0}};
        let target_previous=state;update_head_state(&mut state,input,AlsHeadSettings::default());
        let output=&b["head_values"];let observed=AlsHeadState{initialization_required:output["booleans"]["head_state.b_initialization_required"]["resolved"].as_bool().unwrap(),switching_look_sides:output["booleans"]["head_state.b_switching_look_sides"]["resolved"].as_bool().unwrap(),pitch_angle:f(&output["floats"]["head_state.pitch_angle"]["resolved"]),yaw_angle:f(&output["floats"]["head_state.yaw_angle"]["resolved"]),yaw_velocity:f(&output["floats"]["head_state.yaw_velocity"]["resolved"]),yaw_amount:f(&output["floats"]["head_state.yaw_amount"]["resolved"])};
        assert_eq!(state,observed,"Continuous public Head row {index}");
        if index==79{
            let row=&native["rows"][78];let expected=head(&row["output"]);let previous=head(&row["previous"]);
            let mut input_only=target_previous;
            let mut source_input=input;source_input.view_yaw_angle=f(&row["input"]["view_yaw_angle"]);
            update_head_state(&mut input_only,source_input,AlsHeadSettings::default());
            let mut history_only=previous;update_head_state(&mut history_only,input,AlsHeadSettings::default());
            let mut both=previous;update_head_state(&mut both,source_input,AlsHeadSettings::default());
            assert_eq!(input_only,expected,"First Look difference is removed by the native view yaw alone");
            assert_eq!(both,expected);assert_eq!(history_only,observed);assert_ne!(state,expected);
            let pin=b["head_values"]["actual_look_pins"].as_array().unwrap().iter().find(|v|v["key"]=="head_state.yaw_amount").unwrap();
            assert_eq!(f(&pin["value"]),observed.yaw_amount);
            first_control=Some(json!({"source_frame":78,"target_query":79,"target_view_yaw_angle":input.view_yaw_angle,"source_view_yaw_angle":source_input.view_yaw_angle,"target":snapshot(state),"source":snapshot(expected),"source_view_only":snapshot(input_only),"source_history_only":snapshot(history_only),"source_view_and_history":snapshot(both),"actual_look_pin":pin}));
        }
    }
    let mut first=None;let mut differences=0;
    for row in native["rows"].as_array().unwrap(){
        let i=&row["input"];let mut current=head(&row["previous"]);
        let input=AlsHeadUpdateInput{delta_seconds:f(&i["delta_seconds"]),delta_seconds_without_time_dilation:f(&i["delta_seconds_without_time_dilation"]),first_person:i["first_person"].as_bool().unwrap(),rotation_mode:if i["velocity_direction"].as_bool().unwrap(){RotationMode::VelocityDirection}else{RotationMode::ViewDirection},has_input:i["has_input"].as_bool().unwrap(),input_yaw_world:f(&i["input_yaw_world"]),target_yaw_world:f(&i["target_yaw_world"]),body_yaw_world:f(&i["body_yaw_world"]),body_yaw_velocity_world:f(&i["body_yaw_velocity_world"]),view_yaw_angle:f(&i["view_yaw_angle"]),view_pitch_angle:f(&i["view_pitch_angle"]),movement_base_relative_yaw:f(&i["movement_base_relative_yaw"])};
        update_head_state(&mut current,input,AlsHeadSettings::default());
        let expected=head(&row["output"]);
        if current!=expected{differences+=1;if first.is_none(){first=Some(json!({"source_frame":row["source_frame"],"common_input":i,"previous":row["previous"],"public_current":snapshot(current),"native":row["output"]}));}}
    }
    assert_eq!(differences,0);assert!(first_control.is_some());
    let result=json!({"gate":"PASS_ACTUAL_TARGET319_HEAD_REPLAY_WITH_NATIVE_COMMON_INPUT_COMPARISON","actual_target_rows_exact":319,"native_common_input_rows":318,"common_input_difference_rows":differences,"first":first,"first_control":first_control,"new_difference_confirmed":false,"full_root_equivalence_claimed":false});
    std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec_pretty(&result).unwrap()).unwrap();println!("{result}");
}
