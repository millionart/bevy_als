use bevy::prelude::*;
use serde_json::{Value,json};

fn vector(v:&Value)->Vec3{Vec3::new(v[0].as_f64().unwrap()as f32,v[1].as_f64().unwrap()as f32,v[2].as_f64().unwrap()as f32)}
fn scalar(v:&Value)->f32{v.as_f64().unwrap()as f32}

fn main(){
    let data:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/443b/runtime-baseline-motion-scope/parameters.json").unwrap()).unwrap();
    let mut rows=Vec::new();let mut mismatches=Vec::new();let mut band_moved=0usize;let mut coefficient_nonzero=0usize;
    for i in 0..240{
        let r=&data["rows"][i];let m=&data["after_motors"][i];let p=&data["before_floor_sync"][i];let end=&data["camera_after_post_update"][i];let c=&data["integration_coefficients"][i];
        assert_eq!(c["ordinal"].as_u64().unwrap(),i as u64);
        let initial=vector(&c["initial_velocity"]);assert_eq!(initial,vector(&m["velocity"]));assert_eq!(vector(&c["initial_delta"]),Vec3::ZERO);
        let frame=scalar(&r["delta"]);let physics=scalar(&end["physics_delta"]);let count=end["substeps"].as_u64().unwrap()as u32;
        let substep=(end["substep_delta_ns"].as_u64().unwrap()as f64*1e-9)as f32;
        let acceleration=vector(&m["motor_acceleration"])+vector(&m["tnua_gravity"])+vector(&end["gravity"])*scalar(&m["gravity_scale"]);
        let mut expected_increment=acceleration*substep;
        let ratio=frame/physics;
        if c["collider_changed"].as_bool().unwrap(){expected_increment.x*=ratio;expected_increment.z*=ratio;}else{expected_increment*=ratio;}
        let increment=vector(&c["linear_increment"]);assert_eq!(increment,expected_increment,"Force coefficient mismatch at {i}");
        coefficient_nonzero+=(increment!=Vec3::ZERO)as usize;
        let damping=scalar(&c["linear_damping_rhs"]);let mut velocity=initial;let mut delta=Vec3::ZERO;
        for _ in 0..count{velocity*=damping;velocity+=increment;delta+=velocity*(frame/count as f32);}
        let position=vector(&m["position"])+delta;
        if position!=vector(&p["position"])||velocity!=vector(&p["velocity"]){mismatches.push(json!({"ordinal":i,"position_predicted":position.to_array(),"position_actual":p["position"],"velocity_predicted":velocity.to_array(),"velocity_actual":p["velocity"]}));}
        let floor=&p["floor"];assert!(floor["adjustment_hit"].is_null());
        let old=vector(&p["position"]);let distance=scalar(&floor["distance"]);let center=scalar(&floor["center"]);
        let target=if (0.019..=0.024).contains(&distance){old.y}else{center+(0.019f32+0.024f32)*0.5};
        assert_eq!(target,scalar(&floor["target_y"]));
        let (direction,length)=Dir3::new_and_length(Vec3::Y*(target-old.y)).unwrap_or((Dir3::NEG_Y,0.0));
        let final_position=old+*direction*length;
        assert_eq!(final_position,vector(&end["position"]),"Post floor mismatch at {i}");
        let query=&r["floor_probe"];let before=vector(&r["position"]);let motor_y=scalar(&m["position"][1]);
        let mut source_band_control=before.y;
        if query["eligible"].as_bool().unwrap(){
            assert!(query["forward"].is_null());assert_eq!(query["edge_valid"],true);
            let hit=&query["floor"];let q=scalar(&hit["distance"]);assert!(q>0.0&&scalar(&hit["normal1"][1])>0.9);
            let destination=vector(&query["destination"]);
            let current=destination.y-q+(0.019f32+0.005f32);
            assert_eq!(current,scalar(&query["predicted_y"]));assert_eq!(motor_y,before.y.max(current));
            let source_cm=(q as f64*100.0)as f32;
            if 1.9f32<=source_cm&&source_cm<=2.4f32{
                band_moved+=(motor_y!=before.y)as usize;
            }else{source_band_control=current;}
        }else{assert_eq!(motor_y,before.y);}
        rows.push(json!({"ordinal":i,"raw_physics_position":position.to_array(),"raw_physics_velocity":velocity.to_array(),"floor_final_position":final_position.to_array(),"current_transaction_y":motor_y,"only_source_band_transaction_y":source_band_control,"linear_increment":increment.to_array()}));
    }
    assert!(band_moved>0);
    let shared_center=0.9215f32;let shared_distance=0.0215f32;
    let source_distance_cm=(shared_distance as f64*100.0)as f32;
    assert!((1.9f32..=2.4f32).contains(&source_distance_cm));
    let shared_current=shared_center-shared_distance+(0.019f32+0.005f32);
    assert_ne!(shared_current,shared_center);
    let result=json!({"gate":if mismatches.is_empty(){"PASS_FULL_CURRENT_PHYSICS_AND_FLOOR240"}else{"FAIL_FULL_PHYSICS"},"rows":rows,"mismatches":mismatches,"source_band_but_target_transaction_moved":band_moved,"nonzero_force_coefficient_rows":coefficient_nonzero,"no_query_error_protocol":{"center":shared_center,"floor_distance":shared_distance,"source_distance_cm":source_distance_cm,"current_y":shared_current,"source_band_y":shared_center},"source_world_feedback_restored_claimed":false});
    std::fs::write(std::env::args().nth(1).unwrap(),serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    assert!(mismatches.is_empty(),"Physics integration failed");println!("Replayed full current physics, floor adjustment and held source-band control240");
}
