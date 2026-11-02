use bevy::math::{DQuat,DVec3};
use bevy_als::extensions::{control_rig_hierarchy::{RigHierarchy,RigTransform,RigSpace},control_rig_program::RigDefinition,control_rig_math::RigValue,control_rig_units::RigUnitContext,SourceCurveValues};
use serde_json::{Value,json};
mod solver {
    include!("../479/replay.rs");
    pub fn call(row:&serde_json::Value,source:bool)->serde_json::Value {evaluate(row,if source{31}else{0},if source{3}else{0})}
    pub fn vector_rotate(q:[f64;4],v:bevy::math::DVec3,source:bool)->bevy::math::DVec3 {rotate(q,v,if source{4}else{0})}
}
mod interpolation {
    include!("../473/replay.rs");
    pub fn call(a:[f64;4],b:[f64;4],t:f32,mask:u8)->[f64;4] {slerp(a,b,t,mask)}
}
fn vector(v:&Value)->DVec3 {DVec3::from_array(std::array::from_fn(|i|v[i].as_f64().unwrap()))}
fn q(v:&Value)->[f64;4] {std::array::from_fn(|i|v[i].as_f64().unwrap())}
fn project(v:DVec3)->DVec3 {DVec3::new(v.x,v.z,v.y)}
fn project_q(v:[f64;4])->[f64;4] {[-v[0],-v[2],-v[1],v[3]]}
fn transform(v:&Value)->RigTransform {serde_json::from_value(v.clone()).unwrap()}
fn projected(v:RigTransform)->RigTransform {RigTransform{translation:project(v.translation),rotation:DQuat::from_array(project_q(v.rotation.to_array())),scale:project(v.scale)}}
fn forward(t:RigTransform,v:DVec3,source:bool)->DVec3 {
    if source {solver::vector_rotate(t.rotation.to_array(),t.scale*v,true)+t.translation}
    else{project(projected(t).transform_point(project(v)))}
}
fn inverse(t:RigTransform,v:DVec3,source:bool)->DVec3 {
    assert!(t.scale.abs().min_element()>1e-8_f32 as f64,"Actual partial scale stays outside reciprocal fallback");
    if source {solver::vector_rotate(t.rotation.conjugate().to_array(),v-t.translation,true)*t.scale.recip()}
    else{project(projected(t).inverse_transform_point(project(v)))}
}
fn evaluate(row:&Value,source_solver:bool,mask:u8)->Value {
    let mask=mask&!4u8;
    let before:[RigTransform;3]=std::array::from_fn(|i|transform(&row["before"][i]));
    let weight=row["weight"].as_f64().unwrap()as f32;
    if weight<=1e-8 {return json!({"after":before,"skip":true});}
    let core=solver::call(row,source_solver);
    let mut bones=[before[0],before[0],transform(&row["effector_transform"])];
    bones[0].rotation=DQuat::from_array(q(&core["rotations"][0]));
    bones[1].rotation=DQuat::from_array(q(&core["rotations"][1]));
    bones[1].translation=vector(&core["joint"]);
    bones[2].translation=vector(&core["end"]);
    let solved=bones;
    let mut local=Vec::new();
    if weight<1.0 {
        let joint=inverse(bones[0],bones[1].translation,mask&4!=0);
        let tip=inverse(bones[1],bones[2].translation,mask&4!=0);
        local=vec![joint,tip];
        for i in 0..3 {bones[i].rotation=DQuat::from_array(interpolation::call(before[i].rotation.to_array(),bones[i].rotation.to_array(),weight,mask&3));}
        bones[1].translation=forward(bones[0],joint,mask&4!=0);
        bones[2].translation=forward(bones[1],tip,mask&4!=0);
    }
    json!({"after":bones,"solved":solved,"locals":local,"core":core,"skip":false})
}
fn public(row:&Value,def:&RigDefinition,actual:&Value)->[RigTransform;3] {
    let index=if row["kind"]=="source_common"{row["current_template"].as_u64().unwrap()}else{row["row"].as_u64().unwrap()}as usize;
    let original=&actual["current"][index];
    let mut args:Vec<RigValue>=serde_json::from_value(original["arguments_before"].clone()).unwrap();
    let mut h=RigHierarchy::new(def.bones.clone(),0.0).unwrap();
    let indices=args[..3].iter().map(|key|h.item_index(key).unwrap().unwrap()).collect::<Vec<_>>();
    for i in 0..3 {
        let value=projected(transform(&row["before"][i]));
        h.set_transform(indices[i],RigSpace::Global,false,value,true).unwrap();
        assert_eq!(h.transform(indices[i],RigSpace::Global,false).unwrap(),value);
    }
    args[13]=RigValue::F32(row["weight"].as_f64().unwrap()as f32);
    if row["kind"]=="source_common" {
        args[3]=projected(transform(&row["effector_transform"])).value();
        args[4]=RigValue::Vector(project(vector(&row["primary"])));
        args[5]=RigValue::Vector(project(vector(&row["secondary"])));
        args[6]=RigValue::F32(row["secondary_weight"].as_f64().unwrap()as f32);
        args[7]=RigValue::Vector(project(vector(&row["pole"])));
        for i in 0..2 {args[14+i]=RigValue::F32(row["lengths"][i].as_f64().unwrap()as f32);}
    }
    let expected=args.clone();
    let curves=SourceCurveValues::default();
    let mut context=RigUnitContext{hierarchy:&mut h,curves:&curves,game_world:true,construction:false,delta_seconds:1.0/30.0,geometry:&def.geometry,to_world:RigTransform::default(),line_trace:None};
    context.call("FRigUnit_TwoBoneIKSimplePerItem::Execute",&mut args).unwrap();
    assert_eq!(args,expected,"Public complete weight and parameter echo");
    std::array::from_fn(|i|projected(h.transform(indices[i],RigSpace::Global,false).unwrap()))
}
fn main() {
    let root=std::path::Path::new("docs/audit/bf144/480");
    let label=std::env::args().nth(1).unwrap();
    let input:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();
    let actual:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/476/inputs.json").unwrap()).unwrap();
    let def:RigDefinition=serde_json::from_value(actual["definition"].clone()).unwrap();
    let source_rows=input["source"].as_array().unwrap();
    let current_rows=input["current"].as_array().unwrap();
    assert!(!source_rows.is_empty()&&!current_rows.is_empty());
    let expected_core=source_rows.iter().filter(|row|row["weight"].as_f64().unwrap()>1e-8).count();
    let expected_actual=current_rows.iter().filter(|row|row["kind"]=="actual").count();
    let mut source=Vec::new();
    let mut source_exact=0;
    let mut core_exact=0;
    let mut first=None;
    for row in input["source"].as_array().unwrap() {
        let result=evaluate(row,true,7);
        let predicted:[RigTransform;3]=std::array::from_fn(|i|transform(&result["after"][i]));
        let expected:[RigTransform;3]=std::array::from_fn(|i|transform(&row["expected_after"][i]));
        let exact=predicted==expected;
        source_exact+=exact as usize;
        if !result["skip"].as_bool().unwrap() {
            let expected=&row["expected_core"];
            assert_eq!(result["core"]["rotations"],expected["rotations"],"Fresh complete native pre-weight rotations");
            assert_eq!(result["core"]["joint"],expected["joint"]);
            assert_eq!(result["core"]["end"],expected["end"]);
            core_exact+=1;
        }
        if !exact&&first.is_none(){first=Some(json!({"row":row,"result":result}));}
        source.push(json!({"row":row["row"],"weight":row["weight"],"result":result,"native":row["expected_after"],"exact":exact}));
    }
    let mut current=Vec::new();
    let mut public_exact=0;
    let mut actual_exact=0;
    for row in input["current"].as_array().unwrap() {
        let result=evaluate(row,false,0);
        let value=public(row,&def,&actual);
        assert_eq!(result["after"],json!(value),"Complete same-input current weighted unit and ordered three-bone globals");
        public_exact+=1;
        if row["kind"]=="actual" {assert_eq!(json!(value),row["expected_after"]);actual_exact+=1;}
        current.push(json!({"kind":row["kind"],"domain":row["domain"],"row":row["row"],"mode":row["mode"],"weight":row["weight"],"result":result,"public":value}));
    }
    let mut factors=Vec::new();
    for row in input["source"].as_array().unwrap().iter().chain(input["current"].as_array().unwrap().iter().filter(|row|row["kind"]=="current_control")) {
        let weight=row["weight"].as_f64().unwrap();
        if weight<=0.0||weight>=1.0 {continue;}
        for mask in 0..8 {
            let result=evaluate(row,true,mask);
            factors.push(json!({"domain":row["domain"],"kind":row["kind"],"row":row["row"],"weight":row["weight"],"mask":mask,"result":result}));
        }
    }
    std::fs::write(root.join(format!("replay-unit-{label}.json")),serde_json::to_vec(&json!({"gate":"COMPLETE_IK_ZERO_PARTIAL_FULL_WRAPPER_AND_ORDERED_THREE_BONE_CONSUMERS","source_exact":source_exact,"core_exact":core_exact,"public_exact":public_exact,"actual_exact":actual_exact,"first_difference":first,"source":source,"current":current,"factors":factors})).unwrap()).unwrap();
    assert_eq!(core_exact,expected_core);
    assert_eq!(source_exact,source_rows.len(),"Complete source weight wrapper TQS must match exact native post");
    assert_eq!(public_exact,current_rows.len());
    assert_eq!(actual_exact,expected_actual);
    println!("Qualified{} exact source weight branches,{} public unit consumers and{actual_exact} original actual outputs",source_rows.len(),current_rows.len());
}
