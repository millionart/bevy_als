use bevy::prelude::*;
use bevy_als::als::{orientation::world_forward_to_yaw,turn_in_place::wrap_angle_degrees};
use serde_json::{Value,json};
fn f(v:&Value)->f32{v.as_f64().unwrap()as f32}
fn main(){
    let root=std::path::Path::new("docs/audit/bf144/447d");let label=std::env::args().nth(1).unwrap();
    let target:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/447b/runtime-restored/parameters.json").unwrap()).unwrap();
    let input=target["animation_before"].as_array().unwrap();assert_eq!(input.len(),319);
    let mut rows=Vec::new();let mut samples=Vec::new();
    for(index,row)in input.iter().enumerate(){
        let h=&row["history_input"];assert_eq!(h["actor_parented"],false);
        let q=&h["local_rotation"];let s=&h["local_scale"];let p=&h["local_translation"];
        let local=Transform{translation:Vec3::new(f(&p[0]),f(&p[1]),f(&p[2])),rotation:Quat::from_xyzw(f(&q[0]),f(&q[1]),f(&q[2]),f(&q[3])),scale:Vec3::new(f(&s[0]),f(&s[1]),f(&s[2]))};
        let transform=GlobalTransform::from(local);let body=world_forward_to_yaw(*transform.forward()).to_degrees();
        let raw=f(&h["raw_view_yaw"]);let view=wrap_angle_degrees(raw.to_degrees());
        let current=-wrap_angle_degrees(view-body);
        let actual=f(&row["head_values"]["floats"]["view_state.yaw_angle"]["resolved"]);
        assert_eq!(current,actual,"Actual View producer query {index}");
        let late=wrap_angle_degrees((f64::from(body)-f64::from(raw).to_degrees())as f32);
        rows.push(json!({"target_query":index,"raw_view_radians":raw,"body_runtime_degrees":body,"view_runtime_degrees":view,"current":current,"actual":actual,"late_view_cast_control":late}));
        samples.push(json!({"name":format!("actual-target-query-{index}"),"rotation_xyzw":[0,0,0,1],"yaw_difference":{"velocity_yaw_degrees":body,"view_yaw_radians":raw}}));
    }
    let result=json!({"actual_rows_exact":319,"common_input_native_request_scope":"Same target raw view radians and exactly representable body degrees; coordinate origin cancels in relative yaw; this is a scalar control, not native actual actor replay","rows":rows});
    std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    std::fs::write(root.join("request.json"),serde_json::to_vec_pretty(&json!({"samples":samples})).unwrap()).unwrap();
    println!("Replayed 319 actual target View producers and prepared same-input late-cast native controls");
}
