use bevy::{prelude::*,math::{DAffine3,DQuat,DVec3}};
use serde_json::{Value,json};

fn vector(v:&Value)->Vec3{Vec3::new(v[0].as_f64().unwrap()as f32,v[1].as_f64().unwrap()as f32,v[2].as_f64().unwrap()as f32)}
fn scalar(v:&Value)->f32{v.as_f64().unwrap()as f32}

fn main(){
    let data:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/443c/runtime-baseline/parameters.json").unwrap()).unwrap();
    let source:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/443/source-phases.json").unwrap()).unwrap();
    let mut rows=Vec::new();let mut scale_changed=0usize;let mut radius_changed=0usize;let mut final_changed=0usize;let mut double_changed=0usize;
    for i in 0..240{
        let raw=&data["before_floor_sync"][i];let s=&raw["scale_inputs"];assert_eq!(s["parented"],false);
        let local_scale=vector(&s["local_scale"]);assert_eq!(local_scale,Vec3::ONE);
        let q=&s["local_rotation"];let rotation=Quat::from_xyzw(scalar(&q[0]),scalar(&q[1]),scalar(&q[2]),scalar(&q[3]));
        let transform=Transform{translation:vector(&s["local_translation"]),rotation,scale:local_scale};
        let extracted=GlobalTransform::from(transform).to_scale_rotation_translation().0;
        assert_eq!(extracted,vector(&s["actual_scale"]));
        let consumed_scale=extracted;
        let base_half=scalar(&s["base_half"]);let base_radius=scalar(&s["base_radius"]);
        let current_radius=base_radius*consumed_scale.x.min(consumed_scale.z);let stored_radius=base_radius*local_scale.x.min(local_scale.z);
        assert_eq!(current_radius,scalar(&s["actual_radius"]));assert_eq!(stored_radius,scalar(&s["source_scale_radius"]));
        assert_eq!(base_half*extracted.y,scalar(&s["actual_half"]));
        scale_changed+=(extracted!=local_scale)as usize;radius_changed+=(current_radius!=stored_radius)as usize;
        let dq=DQuat::from_xyzw(rotation.x as f64,rotation.y as f64,rotation.z as f64,rotation.w as f64);
        let double=DAffine3::from_scale_rotation_translation(DVec3::ONE,dq,DVec3::ZERO).to_scale_rotation_translation().0;
        double_changed+=(double.as_vec3()!=local_scale)as usize;
        let source_scale=&source["character_animation_probe"]["cases"][0]["frames"][i]["actor_transform"]["scale"];
        for axis in ["x","y","z"]{assert_eq!(source_scale[axis].as_f64().unwrap(),1.0);}
        let p=vector(&raw["position"]);let floor=&raw["floor"];
        let distance=scalar(&floor["distance"]);let target=if (0.019..=0.024).contains(&distance){p.y}else{scalar(&floor["center"])+(0.019f32+0.024f32)*0.5};
        let (direction,length)=Dir3::new_and_length(Vec3::Y*(target-p.y)).unwrap_or((Dir3::NEG_Y,0.0));
        let current_final=(p+*direction*length).y;assert_eq!(current_final,scalar(&data["camera_after_post_update"][i]["position"][1]));
        let control=&s["source_scale_floor"];assert_eq!(control["adjustment_blocked"],false);
        let controlled_final=scalar(&control["unblocked_final_y"]);
        final_changed+=(current_final!=controlled_final)as usize;
        rows.push(json!({"ordinal":i,"stored_scale":local_scale.to_array(),"affine_scale":extracted.to_array(),"double_same_quaternion_affine_scale":double.to_array(),"base_radius":base_radius,"current_radius":current_radius,"stored_scale_radius":stored_radius,"current_final_y":current_final,"only_stored_scale_final_y":controlled_final}));
    }
    assert!(scale_changed>0&&radius_changed>0&&final_changed>0&&double_changed>0);
    let result=json!({"gate":"PASS_ACTUAL240_STORED_SCALE_VS_AFFINE_SCALE_FLOOR_CONSUMER", "rows":rows,"affine_scale_differs_from_stored":scale_changed,"radius_changed":radius_changed,"floor_final_y_changed":final_changed,"double_matrix_same_quaternion_still_differs":double_changed,"source_root_actual_scale_one":240,"full_source_world_restored_claimed":false});
    std::fs::write(std::env::args().nth(1).unwrap(),serde_json::to_vec_pretty(&result).unwrap()).unwrap();println!("Replayed actual scale extraction and stored-scale-only floor consumers240");
}
