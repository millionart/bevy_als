use bevy_als::extensions::sync_group_phase::SourceSyncPlayerTick;
use serde_json::{Value,json};
fn scalar(v:&Value)->f32{v.as_f64().unwrap()as f32}
fn main(){
    let label=std::env::args().nth(1).unwrap();
    let root=std::path::Path::new("docs/audit/bf144/445");
    let data:Value=serde_json::from_slice(&std::fs::read(root.join(format!("runtime-{label}/parameters.json"))).unwrap()).unwrap();
    let input:Value=serde_json::from_slice(&std::fs::read(root.join("idle-input.json")).unwrap()).unwrap();
    let duration=scalar(&input["duration"]);
    let dt=scalar(&input["delta"]);
    let frames=data["animation_after"].as_array().unwrap();
    let mut position=0.0;
    let mut actual_times=Vec::new();
    for (index,frame)in frames.iter().enumerate(){
        let nodes=frame["leaf_nodes"].as_array().unwrap();
        let leaf=nodes.iter().find(|n|n["node_guid"]==input["node_guid"]).unwrap();
        let clock=&leaf["clocks"][0];
        let delta=if index==0{0.0}else{dt};
        let tick=SourceSyncPlayerTick{duration,phase_duration:scalar(&clock["sync"][0]["phase_duration"]),delta_seconds:delta,play_rate:1.0,looped:true,..Default::default()};
        position=tick.advance_from_time(position).time;
        assert_eq!(position,scalar(&clock["node_time"]),"Current Idle clock row {index}");
        actual_times.push(position);
    }
    let mut source_position=0.0;
    let mut first=None;
    let mut different=0;
    for (index,native)in input["native_ticks"].as_array().unwrap().iter().enumerate(){
        assert_eq!(source_position,scalar(&native["tick_previous"]));
        let direct=source_position+dt;
        source_position=if direct<0.0||direct>duration{direct%duration}else{direct};
        assert_eq!(source_position,scalar(&native["current_asset_time"]),"Native direct-second clock row {index}");
        if actual_times[index+1]!=source_position{
            different+=1;
            if first.is_none(){first=Some(json!({"source_frame":index,"target_arena_row":index+1,"source_seconds":source_position,"target_seconds":actual_times[index+1],"native_previous":native["tick_previous"],"delta_seconds":dt}));}
        }
    }
    assert!(different>0);
    let result=json!({"current_idle_rows_exact":frames.len(),"native_direct_second_rows_exact":240,"aligned_positive_tick_count_differences":different,"first":first,"known_difference":"BF144-D201","new_difference":false,"all_own_history_clock_differences_attributed":false});
    std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    println!("{result}");
}
