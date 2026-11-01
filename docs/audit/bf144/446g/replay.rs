use bevy::prelude::*;
use serde_json::{Value,json};
fn scalar(v:&Value)->f32{v.as_f64().unwrap()as f32}
fn vector(v:&Value)->Vec3{let a=v.as_array().unwrap();Vec3::new(scalar(&a[0]),scalar(&a[1]),scalar(&a[2]))}
fn main(){
    let label=std::env::args().nth(1).unwrap();
    let root=std::path::Path::new("docs/audit/bf144/446g");
    let data:Value=serde_json::from_slice(&std::fs::read(root.join(format!("runtime-{label}/parameters.json"))).unwrap()).unwrap();
    let old:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/443b/runtime-baseline-motion-scope/parameters.json").unwrap()).unwrap();
    let rows=data["animation_before"].as_array().unwrap();
    let find=|key:&str,frame:u64|data[key].as_array().unwrap().iter().find(|v|v["app_frame"].as_u64()==Some(frame)).unwrap();
    let mut force_rows=0;let mut raw_rows=0;let mut cold_spring_rows=0;let mut first=None;let mut cold_changed=0;let mut sense_only=0;let mut sleeping_no_solver=0;
    for (index,row)in rows.iter().enumerate(){
        let frame=row["curve_frame_input"]["app_frame"].as_u64().unwrap();
        let m=find("physics_motors",frame);let c=find("physics_coefficients",frame);let raw=find("physics_raw",frame);
        assert_eq!(m["rigid_body"],"Dynamic");assert_eq!(c["disabled"],false);
        let active=m["toggle"].as_str().is_none_or(|v|v=="Enabled");
        sense_only+=(!active)as usize;
        let custom=if m["custom_gravity"].is_null(){Vec3::ZERO}else{vector(&m["custom_gravity"])};
        let gravity=vector(&c["gravity"])*m["gravity_scale"].as_f64().map_or(1.0,|v|v as f32);
        let acceleration=if active{vector(&m["motor_acceleration"])+custom}else{Vec3::ZERO};
        let substep=(c["substep_delta_ns"].as_u64().unwrap()as f64*1e-9)as f32;
        let frame_dt=scalar(&row["frame_delta"]);
        let physics=(c["physics_delta_ns"].as_u64().unwrap()as f64*1e-9)as f32;
        let mut expected=(acceleration+gravity)*substep;
        if c["custom_position"].as_bool().unwrap()&&c["frame_delta_ns"].as_u64().unwrap()>33_333_333{
            let ratio=frame_dt/physics;
            if c["collider_changed"].as_bool().unwrap(){expected.x*=ratio;expected.z*=ratio;}else{expected*=ratio;}
        }
        assert_eq!(expected,vector(&c["integration"]["linear_increment"]),"Force row {index}");force_rows+=1;
        let count=c["substeps"].as_u64().unwrap()as u32;
        let integrating=!c["body"].is_null();
        let mut velocity=if integrating{vector(&c["body"]["initial_velocity"])}else{
            assert_eq!(c["sleeping"],true);assert!(!active);assert_eq!(expected,Vec3::ZERO);
            sleeping_no_solver+=1;vector(&m["velocity"])
        };assert_eq!(velocity,vector(&m["velocity"]));
        if integrating{assert_eq!(vector(&c["body"]["initial_delta"]),Vec3::ZERO);}
        let damping=scalar(&c["integration"]["linear_damping_rhs"]);let mut delta=Vec3::ZERO;
        for _ in 0..count{
            if !integrating{continue;}
            velocity*=damping;velocity+=expected;
            let movement=if c["custom_position"].as_bool().unwrap(){
                if m["move_start"].is_null()&&!m["planned_delta"].is_null(){vector(&m["planned_delta"])/count as f32+Vec3::Y*velocity.y*(frame_dt/count as f32)}else{velocity*(frame_dt/count as f32)}
            }else{velocity*substep};
            delta+=movement;
        }
        let position=vector(&m["position"])+delta;
        assert_eq!(position,vector(&raw["position"]),"Raw position row {index}");
        assert_eq!(velocity,vector(&raw["velocity"]),"Raw velocity row {index}");raw_rows+=1;
        if row["ordinal"].is_null(){
            assert_eq!(position,vector(&row["position"]));
            assert_eq!(row["velocity"][1].as_f64().unwrap(),0.0);
            let walk=&m["walk"];let sensor=&m["ground"]["output"];let tracker=&m["tracker"];
            assert_eq!(vector(&sensor["normal"]),Vec3::Y);assert_eq!(vector(&sensor["entity_linvel"]),Vec3::ZERO);
            assert_eq!(vector(&tracker["velocity"]),Vec3::ZERO);assert_eq!(vector(&m["motor_boost"]),Vec3::ZERO);
            let offset=scalar(&walk["float_height"])-scalar(&sensor["distance"]);
            assert_eq!(vector(&walk["standing_offset"]),-Vec3::Y*offset);
            let spring=offset*scalar(&walk["spring_strength"]);
            assert_eq!(vector(&m["motor_acceleration"]),Vec3::Y*spring-vector(&tracker["gravity"]));cold_spring_rows+=1;
            if position!=vector(&m["position"]){
                cold_changed+=1;
                if first.is_none(){
                    assert!(active);
                    let control_spring=0.0;
                    let control_acceleration=Vec3::Y*control_spring-vector(&tracker["gravity"])+custom+gravity;
                    let mut control_v=vector(&c["body"]["initial_velocity"]);let mut control_delta=Vec3::ZERO;
                    for _ in 0..count{control_v*=damping;control_v+=control_acceleration*substep;control_delta+=Vec3::Y*control_v.y*(frame_dt/count as f32);}
                    let control=vector(&m["position"])+control_delta;
                    assert_eq!(control,vector(&m["position"]));assert_ne!(control,position);
                    first=Some(json!({"query":index,"app_frame":frame,"starting_position":m["position"],"actual_position":raw["position"],"source_band_control_position":control.to_array(),"spring_offset":offset,"spring_acceleration":spring,"net_acceleration":(acceleration+gravity).to_array(),"linear_increment":expected.to_array(),"raw_velocity":velocity.to_array(),"final_velocity":row["velocity"]}));
                }
            }
        }else{
            let ordinal=row["ordinal"].as_u64().unwrap()as usize;
            assert_eq!(raw["position"],old["before_floor_sync"][ordinal]["position"]);
            assert_eq!(raw["velocity"],old["before_floor_sync"][ordinal]["velocity"]);
            assert_eq!(row["position"],old["camera_after_post_update"][ordinal]["position"]);
        }
    }
    assert_eq!(cold_spring_rows,79);assert!(cold_changed>0);
    let result=json!({"gate":"PASS_COLD_AND_READY_FORCE_AND_RAW_PHYSICS319_WITH_D31_FIRST_CONSUMER","force_rows_exact":force_rows,"raw_physics_rows_exact":raw_rows,"sleeping_without_solver_rows_qualified":sleeping_no_solver,"cold_spring_rows_exact":cold_spring_rows,"cold_position_changed_rows":cold_changed,"sense_only_animation_rows":sense_only,"known_difference":"BF144-D31","first":first,"new_difference":false,"all_source_world_or_root_equivalence_claimed":false});
    std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec_pretty(&result).unwrap()).unwrap();println!("{result}");
}
