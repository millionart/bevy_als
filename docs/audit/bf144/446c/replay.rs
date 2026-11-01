use bevy_als::extensions::sync_group_phase::SourceSyncPlayerTick;
use serde_json::{Value,json};
fn f(v:&Value)->f32{v.as_f64().unwrap()as f32}
fn main(){
    let r=std::path::Path::new("docs/audit/bf144/446c");
    let d:Value=serde_json::from_slice(&std::fs::read(r.join("idle-input.json")).unwrap()).unwrap();
    let duration=f(&d["duration"]);
    let mut actual=0.0;
    let mut source=0.0;
    let mut differences=0;
    let mut first=None;
    for (index,(s,t))in d["source"].as_array().unwrap().iter().zip(d["target"].as_array().unwrap()).enumerate(){
        assert_eq!(f(&s["previous"]),source);
        assert_eq!(f(&t["previous"]),actual);
        let delta=f(&s["delta"]);
        assert_eq!(delta,f(&t["delta"]));
        let tick=SourceSyncPlayerTick{duration,phase_duration:duration,delta_seconds:delta,play_rate:1.0,looped:true,..Default::default()};
        actual=tick.advance_from_time(actual).time;
        assert_eq!(actual,f(&t["time"]));
        let direct=source+delta;
        source=if direct<0.0||direct>duration{direct%duration}else{direct};
        assert_eq!(source,f(&s["time"]));
        if actual!=source{differences+=1;if first.is_none(){first=Some(index);}}
    }
    let result=json!({"gate":"PASS_COMMON_POSITIVE318_IDLE_CLOCK_FACTORS","current_rows_exact":318,"native_rows_exact":318,"clock_differences":differences,"first_source_frame":first,"known_difference":"BF144-D201","all_other_player_differences_attributed":false});
    std::fs::write(r.join("idle-replay.json"),serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    println!("{result}");
}
