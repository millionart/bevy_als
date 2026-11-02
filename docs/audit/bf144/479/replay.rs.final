use bevy::math::{DQuat,DVec3};
use bevy_als::extensions::{control_rig_hierarchy::{RigTransform,rig_normalize},control_rig_program::RigDefinition};
use serde_json::{Value,json};
mod position {
    include!("../478/replay.rs");
    pub fn evaluate(row:&serde_json::Value,mask:u8)->(bevy::math::DVec3,bevy::math::DVec3) {positions(row,mask)}
}
mod quaternion {
    include!("../471/replay.rs");
    pub fn source_normalize(v:[f64;4])->[f64;4] {normalize(v)}
}
type Q=[f64;4];
fn vector(v:&Value)->DVec3 {DVec3::from_array(std::array::from_fn(|i|v[i].as_f64().unwrap()))}
fn q(v:&Value)->Q {std::array::from_fn(|i|v[i].as_f64().unwrap())}
fn project(v:DVec3)->DVec3 {DVec3::new(v.x,v.z,v.y)}
fn project_q(v:Q)->Q {[-v[0],-v[2],-v[1],v[3]]}
fn dot(a:DVec3,b:DVec3,mask:u8)->f64 {
    if mask&1!=0 {(a.x*b.x+a.y*b.y)+a.z*b.z}else{(a.x*b.x+a.z*b.z)+a.y*b.y}
}
fn normal(v:DVec3,mask:u8)->DVec3 {
    let square=dot(v,v,mask);
    assert!(square>1e-8_f32 as f64,"Actual IK orientation cohort stays outside vector normalization fallback");
    if mask&2!=0 {v*(1.0/square.sqrt())}else{v/square.sqrt()}
}
fn normalize(v:Q,mask:u8)->Q {
    if mask&8!=0 {quaternion::source_normalize(v)}else{project_q(rig_normalize(DQuat::from_array(project_q(v))).to_array())}
}
fn product(a:Q,b:Q,mask:u8)->Q {
    if mask&16!=0 {(DQuat::from_array(a)*DQuat::from_array(b)).to_array()}
    else{project_q((DQuat::from_array(project_q(a))*DQuat::from_array(project_q(b))).to_array())}
}
fn cross(a:DVec3,b:DVec3)->DVec3 {
    DVec3::new((-a.z).mul_add(b.y,a.y*b.z),(-a.x).mul_add(b.z,a.z*b.x),(-a.y).mul_add(b.x,a.x*b.y))
}
fn rotate(rotation:Q,v:DVec3,mask:u8)->DVec3 {
    if mask&4==0 {return project(DQuat::from_array(project_q(rotation))*project(v));}
    let quat=DVec3::from_slice(&rotation[..3]);
    let first=cross(quat,v);
    let twice=first+first;
    let base=DVec3::new(rotation[3].mul_add(twice.x,v.x),rotation[3].mul_add(twice.y,v.y),rotation[3].mul_add(twice.z,v.z));
    base+cross(quat,twice)
}
fn between(a:DVec3,b:DVec3,mask:u8)->Q {
    let w=1.0+dot(a,b,mask);
    assert!(w>=1e-6_f32 as f64,"Actual IK orientation cohort stays outside opposite-direction fallback");
    let xyz=a.cross(b);
    normalize([xyz.x,xyz.y,xyz.z,w],mask)
}
fn evaluate(row:&Value,mask:u8,position_mask:u8)->Value {
    let root=vector(&row["root"]);
    let(joint,end)=position::evaluate(row,position_mask);
    let primary=vector(&row["primary"]);
    let secondary=vector(&row["secondary"]);
    let initial=q(&row["rotation"]);
    let mut rotations=[initial;2];
    let locations=[root,joint,end];
    let mut stages=Vec::new();
    for index in 0..2 {
        let axis=rotate(rotations[index],primary,mask);
        let delta=locations[index+1]-locations[index];
        assert!(axis.abs().max_element()>1e-4_f32 as f64&&delta.abs().max_element()>1e-4,"Actual primary orientation is nonzero");
        let target=normal(delta,mask);
        if index==0 {
            let base=normal(end-root,mask);
            assert!((dot(base,target,mask)-1.0).abs()>=1e-4_f32 as f64,"Actual rotation cohort does not use the fully-extended pole branch");
        }
        let primary_delta=between(axis,target,mask);
        rotations[index]=normalize(product(primary_delta,rotations[index],mask),mask);
        let primary_rotation=rotations[index];
        if row["secondary_weight"]==1 {
            let secondary_axis=rotate(rotations[index],secondary,mask);
            let secondary_raw=joint-(end+root)*0.5;
            assert!(secondary_axis.abs().max_element()>1e-4_f32 as f64&&secondary_raw.abs().max_element()>1e-4,"Actual secondary orientation is nonzero");
            let secondary_target=normal(secondary_raw-dot(secondary_raw,target,mask)*target,mask);
            let secondary_delta=between(secondary_axis,secondary_target,mask);
            rotations[index]=normalize(product(secondary_delta,rotations[index],mask),mask);
            stages.push(json!({"primary_axis":axis,"primary_target":target,"primary_delta":primary_delta,"primary_rotation":primary_rotation,"secondary_axis":secondary_axis,"secondary_target":secondary_target,"secondary_delta":secondary_delta,"result":rotations[index]}));
        }else{
            assert_eq!(row["secondary_weight"],0);
            stages.push(json!({"primary_axis":axis,"primary_target":target,"primary_delta":primary_delta,"primary_rotation":primary_rotation,"result":rotations[index]}));
        }
    }
    json!({"joint":joint,"end":end,"rotations":rotations,"stages":stages})
}
fn main() {
    let root=std::path::Path::new("docs/audit/bf144/479");
    let label=std::env::args().nth(1).unwrap();
    let inputs:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();
    let data:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/476/inputs.json").unwrap()).unwrap();
    let definition:RigDefinition=serde_json::from_value(data["definition"].clone()).unwrap();
    let cases=inputs["rows"].as_array().unwrap();
    assert!(!cases.is_empty());
    let expected_actual=cases.iter().filter(|row|row["domain"]=="current_m"&&row["secondary_weight"]==1).count();
    let mut outputs=Vec::new();
    let mut source_exact=0;
    let mut public_exact=0;
    let mut current_actual=0;
    let mut first=None;
    for(index,row)in cases.iter().enumerate() {
        let initial=DQuat::from_array(project_q(q(&row["rotation"])));
        let mut bones=[RigTransform::default();3];
        bones[0].translation=project(vector(&row["root"]));
        bones[0].rotation=initial;
        bones[1].rotation=initial;
        bones[2].translation=project(vector(&row["effector"]));
        definition.geometry.solve(&mut bones,project(vector(&row["pole"])),project(vector(&row["primary"])),project(vector(&row["secondary"])),row["secondary_weight"].as_f64().unwrap()as f32,std::array::from_fn(|i|row["lengths"][i].as_f64().unwrap()as f32),false,0.75,1.25);
        let public=[project_q(bones[0].rotation.to_array()),project_q(bones[1].rotation.to_array())];
        if row["domain"]=="current_m"&&row["secondary_weight"]==1 {
            let actual=&data["current"][row["row"].as_u64().unwrap()as usize];
            for i in 0..2 {assert_eq!(bones[i].rotation.to_array(),q(&actual["bones_after"][i][1]["rotation"]),"Actual complete public IK rotations");}
            current_actual+=1;
        }
        for(mask,position_mask)in [(31u8,3u8),(0,0),(30,3),(29,3),(27,3),(23,3),(15,3)] {
            let result=evaluate(row,mask,position_mask);
            let rotations=[q(&result["rotations"][0]),q(&result["rotations"][1])];
            if mask==0 {assert_eq!(rotations,public,"Current orientation model matches complete public solve on both data domains");public_exact+=1;}
            let same=result["rotations"]==row["expected_native_rotations"];
            if mask==31 {source_exact+=same as usize;if !same&&first.is_none(){first=Some(json!({"index":index,"input":row,"result":result}));}}
            outputs.push(json!({"row":index,"domain":row["domain"],"secondary_weight":row["secondary_weight"],"mask":mask,"position_mask":position_mask,"result":result,"native":row["expected_native_rotations"],"exact":same}));
        }
        if index%512==0 {println!("Checked IK orientation input={index}");}
    }
    std::fs::write(root.join(format!("replay-unit-{label}.json")),serde_json::to_vec(&json!({"gate":"ACTUAL_IK_COMPLETE_PRIMARY_SECONDARY_ORIENTATION_ARITHMETIC","source_exact":source_exact,"public_exact":public_exact,"current_actual":current_actual,"first_difference":first,"outputs":outputs})).unwrap()).unwrap();
    assert_eq!(public_exact,cases.len());
    assert_eq!(current_actual,expected_actual);
    assert_eq!(source_exact,cases.len(),"Complete actual/control source IK orientation arithmetic");
    println!("Qualified{} complete native and public IK orientation cases with{current_actual} actual default calls",cases.len());
}
