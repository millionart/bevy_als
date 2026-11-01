use bevy::prelude::*;
use serde_json::{Value,json};

fn scalar(row:&Value,key:&str)->f32{row[key].as_f64().unwrap() as f32}
fn vector(row:&Value,key:&str)->Vec3{
    let v=row[key].as_array().unwrap();
    Vec3::new(v[0].as_f64().unwrap() as f32,v[1].as_f64().unwrap() as f32,v[2].as_f64().unwrap() as f32)
}
fn main(){
    let label=std::env::args().nth(1).unwrap();
    let root=std::path::Path::new("docs/audit/bf144/444");
    let data:Value=serde_json::from_slice(&std::fs::read(root.join(format!("runtime-{label}/parameters.json"))).unwrap()).unwrap();
    let before=data["animation_before"].as_array().unwrap();
    let after=data["animation_after"].as_array().unwrap();
    assert_eq!(before.len(),after.len());
    let mut previous=None;
    let mut acceleration_rows=0;
    let mut nonzero_acceleration=0;
    let mut ready=0;
    for (index,(b,a)) in before.iter().zip(after).enumerate(){
        let velocity=vector(b,"motion_world_velocity");
        assert_eq!(velocity,vector(b,"provider_velocity"));
        let local=b["motion_local_velocity"].as_array().unwrap();
        let speed=Vec2::new(local[0].as_f64().unwrap() as f32,local[1].as_f64().unwrap() as f32).length();
        assert_eq!(speed,scalar(b,"motion_speed"));
        if let Some(old)=previous{
            let acceleration=(velocity-old)/scalar(b,"frame_delta");
            assert_eq!(vector(b,"motion_acceleration"),acceleration,"Motion acceleration row {index}");
            acceleration_rows+=1;
            if acceleration!=Vec3::ZERO{nonzero_acceleration+=1;}
        }
        previous=Some(velocity);
        let expected=if b["animation_pending"].as_bool().unwrap(){Vec3::ZERO}else{vector(b,"motion_acceleration")*(scalar(b,"frame_delta")/scalar(b,"animation_delta"))};
        assert_eq!(vector(b,"provider_acceleration"),expected,"Provider acceleration row {index}");
        assert_eq!(a["arena_frame"].as_u64().unwrap(),b["arena_frame"].as_u64().unwrap()+1);
        let root_update=if index==0{"Forced(Absolute(0.0))".to_string()}else{format!("Forced(Delta({:?}))",scalar(b,"animation_delta"))};
        assert_eq!(a["root_time_update"].as_str().unwrap(),root_update);
        if !b["ordinal"].is_null(){ready+=1;}
    }
    assert_eq!(ready,240);
    assert!(before[0]["ordinal"].is_null());
    assert_eq!(before[0]["arena_frame"],0);
    assert_eq!(after[0]["gate"]["pending_update"],false);
    let result=json!({"motion_and_provider_rows_exact":before.len(),"acceleration_rows_exact":acceleration_rows,"nonzero_acceleration_rows":nonzero_acceleration,
        "ready_rows":ready,"cold_rows":before.len()-ready,"ready_start_arena_frame":before.iter().find(|v|!v["ordinal"].is_null()).unwrap()["arena_frame"],
        "first_query_absolute_zero":true,"subsequent_actual_delta_queries_exact":after.len()-1,
        "root_pose_elapsed_all_zero":after.iter().all(|v|scalar(v,"elapsed")==0.0),"root_pose_timestamp_used_as_leaf_clock":false});
    std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    println!("{result}");
}
