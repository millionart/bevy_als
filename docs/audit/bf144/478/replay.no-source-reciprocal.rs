use bevy::math::DVec3;
use bevy_als::extensions::{control_rig_hierarchy::RigTransform,control_rig_program::RigDefinition};
use serde_json::{Value,json};

fn vector(v:&Value)->DVec3 {DVec3::from_array(std::array::from_fn(|i|v[i].as_f64().unwrap()))}
fn project(v:DVec3)->DVec3 {DVec3::new(v.x,v.z,v.y)}
fn object(v:&Value)->DVec3 {DVec3::new(v["x"].as_f64().unwrap(),v["y"].as_f64().unwrap(),v["z"].as_f64().unwrap())}
fn dot(a:DVec3,b:DVec3,mask:u8)->f64 {
    if mask&1!=0 {(a.x*b.x+a.y*b.y)+a.z*b.z}
    else {(a.x*b.x+a.z*b.z)+a.y*b.y}
}
fn normal(v:DVec3,mask:u8)->DVec3 {
    let square=dot(v,v,mask);
    assert!(square>1e-8_f32 as f64,"Actual regular position cohort never uses a small-vector normalization fallback");
    if square==1.0 {return v;}
    v/square.sqrt()
}
fn positions(row:&Value,mask:u8)->(DVec3,DVec3,bool) {
    let root=vector(&row["root"]);
    let effector=vector(&row["effector"]);
    let pole=vector(&row["pole"]);
    let upper=row["lengths"][0].as_f64().unwrap();
    let lower=row["lengths"][1].as_f64().unwrap();
    let delta=effector-root;
    let distance=dot(delta,delta,mask).sqrt();
    assert!(distance>1e-4,"Actual regular cohort does not use the coincident goal branch");
    let direction=normal(delta,mask);
    let pole_delta=pole-root;
    assert!(dot(pole_delta,pole_delta,mask)>1e-8);
    let plane=direction.cross(pole_delta);
    assert!(dot(plane,plane,mask)>1e-8,"Actual regular cohort does not use random perpendicular axes");
    let bend=normal(pole_delta-direction*dot(pole_delta,direction,mask),mask);
    let maximum=upper+lower;
    if distance>=maximum {return(root+upper*direction,root+maximum*direction,true);}
    let denominator=2.0*upper*distance;
    assert_ne!(denominator,0.0);
    let cosine=((upper*upper+distance*distance)-lower*lower)/denominator;
    let perpendicular=upper*cosine.clamp(-1.0,1.0).acos().sin();
    let squared=upper*upper-perpendicular*perpendicular;
    let mut projected=if squared>0.0{squared.sqrt()}else{0.0};
    if cosine<0.0 {projected=-projected;}
    (root+projected*direction+perpendicular*bend,effector,false)
}
fn main() {
    let root=std::path::Path::new("docs/audit/bf144/478");
    let label=std::env::args().nth(1).unwrap();
    let inputs:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();
    let data:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/476/inputs.json").unwrap()).unwrap();
    let definition:RigDefinition=serde_json::from_value(data["definition"].clone()).unwrap();
    let mut exact=[0usize;4];
    let mut outputs=Vec::new();
    let mut current_actual=0;
    for(row_index,row)in inputs["rows"].as_array().unwrap().iter().enumerate() {
        let source=row["domain"]=="source_cm";
        let geometry=&definition.geometry;
        let mut bones=[RigTransform::default();3];
        bones[0].translation=project(vector(&row["root"]));
        bones[2].translation=project(vector(&row["effector"]));
        geometry.solve(&mut bones,project(vector(&row["pole"])),DVec3::X,DVec3::Y,0.0,std::array::from_fn(|i|row["lengths"][i].as_f64().unwrap()as f32),false,0.75,1.25);
        let public=[project(bones[1].translation),project(bones[2].translation)];
        if !source {
            let actual=&data["current"][row["row"].as_u64().unwrap()as usize];
            assert_eq!(bones[1].translation,vector(&actual["bones_after"][1][1]["translation"]));
            assert_eq!(bones[2].translation,vector(&actual["bones_after"][2][1]["translation"]));
            current_actual+=1;
        }
        for mask in 0..4 {
            let(joint,end,extended)=positions(row,mask);
            if mask==0 {assert_eq!([joint,end],public,"Position model matches complete public geometry in both source and current numeric domains");}
            let expected=[object(&row["expected_source_positions"][0]),object(&row["expected_source_positions"][1])];
            let same=[joint,end]==expected;
            exact[mask as usize]+=same as usize;
            outputs.push(json!({"row":row_index,"domain":row["domain"],"mask":mask,"joint":joint,"end":end,"expected":expected,"fully_extended":extended,"exact":same}));
        }
    }
    std::fs::write(root.join(format!("replay-unit-{label}.json")),serde_json::to_vec(&json!({"gate":"ACTUAL_IK_POSITION_NATIVE_PUBLIC_ARITHMETIC_FACTORS","source_exact":exact,"actual_current_public_exact":current_actual,"outputs":outputs})).unwrap()).unwrap();
    assert_eq!(current_actual,1272);
    assert_eq!(exact[3],2596,"Complete source scalar arithmetic matches native source/current position domains");
    println!("Qualified2596 complete native position inputs and1272 actual public calls; source arithmetic exact={exact:?}");
}
