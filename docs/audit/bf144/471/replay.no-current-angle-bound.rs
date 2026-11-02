use std::arch::aarch64::*;
use bevy::math::{DQuat,DVec3};
use bevy_als::extensions::{control_rig_hierarchy::{RigHierarchy,RigTransform,RigSpace,rig_normalize},control_rig_ik::RigGeometry,control_rig_math::{RigValue,rig_euler,rig_quaternion},control_rig_program::RigDefinition,control_rig_units::RigUnitContext,SourceCurveValues};
use serde_json::{Value,json};
type Q=[f64;4];
fn q(v:&Value)->Q{std::array::from_fn(|i|v[i].as_f64().unwrap())}
fn inv(a:Q)->Q{[-a[0],-a[1],-a[2],a[3]]}
fn product(a:Q,b:Q)->Q{(DQuat::from_array(a)*DQuat::from_array(b)).to_array()}
fn normalize(v:Q)->Q{
 let dot=v[2].mul_add(v[2],v[0]*v[0])+v[3].mul_add(v[3],v[1]*v[1]);if dot<1e-8_f32 as f64{return[0.,0.,0.,1.];}let reciprocal=unsafe{let d=vdupq_n_f64(dot);let mut x=vrsqrteq_f64(d);x=vmulq_f64(vrsqrtsq_f64(d,vmulq_f64(x,x)),x);x=vmulq_f64(vrsqrtsq_f64(d,vmulq_f64(x,x)),x);vgetq_lane_f64::<0>(x)};v.map(|x|x*reciprocal)
}
fn object_q(v:&Value)->Q{[v["x"].as_f64().unwrap(),v["y"].as_f64().unwrap(),v["z"].as_f64().unwrap(),v["w"].as_f64().unwrap()]}
fn project(v:Q)->Q{[-v[0],-v[2],-v[1],v[3]]}
fn prod(a:Q,b:Q,mask:u8)->Q{if mask&1!=0{product(a,b)}else{project((DQuat::from_array(project(a))*DQuat::from_array(project(b))).to_array())}}
fn norm(v:Q,mask:u8)->Q{if mask&2!=0{normalize(v)}else{project(rig_normalize(DQuat::from_array(project(v))).to_array())}}
fn between(normal:[f64;3],mask:u8,geometry:&RigGeometry)->Q{
 if mask&16==0{return project(geometry.between_vectors(DVec3::Y,DVec3::new(normal[0],normal[2],normal[1])).to_array());}
 let dot=normal[2];let norm_ab=((normal[0]*normal[0]+normal[1]*normal[1])+normal[2]*normal[2]).sqrt();let w=norm_ab+dot;if w>=1e-6_f32 as f64*norm_ab{norm([-normal[1],normal[0],0.,w],mask)}else{norm([0.,-1.,0.,0.],mask)}
}
fn rotation(angles:[f32;3])->Q{
 let half=(std::f64::consts::PI/180.)*0.5;let p=(angles[0]as f64%360.)*half;let y=(angles[1]as f64%360.)*half;let r=(angles[2]as f64%360.)*half;let(sp,cp)=(p.sin(),p.cos());let(sy,cy)=(y.sin(),y.cos());let(sr,cr)=(r.sin(),r.cos());[cr*(sp*sy)-sr*(cp*cy),-cr*(sp*cy)-sr*(cp*sy),cr*(cp*sy)-sr*(sp*cy),cr*(cp*cy)+sr*(sp*sy)]
}
fn angles(v:Q)->[f64;3]{let v=rig_euler(DQuat::from_array(project(v)));[v.z,-v.y,v.x]}
fn construct(v:[f32;3],mask:u8)->Q{if mask&8!=0{rotation(v)}else{project(rig_quaternion(DVec3::new(v[2]as f64,-(v[1]as f64),v[0]as f64)).to_array())}}
fn evaluate(row:&Value,mask:u8,geometry:&RigGeometry)->Value{
 let calf=q(&row["calf"]);let foot=q(&row["foot"]);let target=q(&row["target"]);let initial=prod(inv(object_q(&row["calf_initial_global"]["rotation"])),object_q(&row["foot_initial_global"]["rotation"]),mask);if mask==31{assert_eq!(initial,q(&row["initial_relative"]));}
 let normal=std::array::from_fn(|i|row["normal"][i].as_f64().unwrap());let offset=between(normal,mask,geometry);let current_space=prod(prod(inv(calf),foot,mask),inv(initial),mask);let target_space=prod(prod(inv(calf),prod(offset,target,mask),mask),inv(initial),mask);let current_angles=angles(current_space);let target_angles=angles(target_space);let limited=std::array::from_fn(|i|{let current=current_angles[i]as f32;let target=target_angles[i]as f32;let a=row["limits"][i][0].as_f64().unwrap()as f32;let b=row["limits"][i][1].as_f64().unwrap()as f32;target.clamp(a.min(b),a.max(b))});let final_space=construct(limited,mask);let before_normal=prod(calf,prod(final_space,initial,mask),mask);let result=norm(before_normal,mask);json!({"initial_relative":initial,"offset_rotation":offset,"current_space":current_space,"target_space":target_space,"current_angles":current_angles,"target_angles":target_angles,"limited_angles":limited,"final_space":final_space,"before_normalize":before_normal,"result":result})
}
fn transform(v:&Value)->RigTransform{let q=&v["rotation"];let t=&v["translation"];let z=&v["scale"];RigTransform{rotation:DQuat::from_array(project([q["x"].as_f64().unwrap(),q["y"].as_f64().unwrap(),q["z"].as_f64().unwrap(),q["w"].as_f64().unwrap()])),translation:DVec3::new(t["x"].as_f64().unwrap()*0.01,t["z"].as_f64().unwrap()*0.01,t["y"].as_f64().unwrap()*0.01),scale:DVec3::new(z["x"].as_f64().unwrap(),z["z"].as_f64().unwrap(),z["y"].as_f64().unwrap())}}
fn common_public(rows:&[Value],definition:&RigDefinition)->Vec<Value>{
 let mut states=std::collections::BTreeMap::<(String,u64),Vec<RigValue>>::new();let curves=SourceCurveValues::default();let mut output=Vec::new();
 for row in rows{let dataset=row["dataset"].as_str().unwrap();let pc=row["pc"].as_u64().unwrap();let calf=row["calf_name"].as_str().unwrap();let foot=row["foot_name"].as_str().unwrap();let key=(dataset.to_owned(),pc);let args=states.entry(key).or_insert_with(||vec![RigValue::List(vec![RigValue::Name("Bone".into()),RigValue::Name(calf.into())]),RigValue::List(vec![RigValue::Name("Bone".into()),RigValue::Name(foot.into())]),RigValue::Quaternion(DQuat::IDENTITY),RigValue::Vector(DVec3::Y),RigValue::List(vec![]),RigValue::List(vec![]),RigValue::List(vec![]),RigValue::F32(0.0),RigValue::Bool(false),RigValue::List(vec![]),RigValue::List(vec![]),RigValue::Vector(DVec3::Y),RigValue::Quaternion(DQuat::IDENTITY),RigValue::Quaternion(DQuat::IDENTITY)]);
 args[2]=RigValue::Quaternion(DQuat::from_array(project(q(&row["target"]))));let n=&row["raw_normal"];args[3]=RigValue::Vector(DVec3::new(n[0].as_f64().unwrap(),n[2].as_f64().unwrap(),n[1].as_f64().unwrap()));let limits=&row["limits"];args[4]=RigValue::List(vec![RigValue::F32(-(limits[1][1].as_f64().unwrap()as f32)),RigValue::F32(-(limits[1][0].as_f64().unwrap()as f32))]);for(i,j)in[(5,0),(6,2)]{args[i]=RigValue::List(vec![RigValue::F32(limits[j][0].as_f64().unwrap()as f32),RigValue::F32(limits[j][1].as_f64().unwrap()as f32)]);}args[7]=RigValue::F32(row["half_life"].as_f64().unwrap()as f32);
 let mut bones=definition.bones.clone();let mut bindings=Vec::new();for(name,initial,current)in[(calf,"calf_initial_global","calf_global"),(foot,"foot_initial_global","foot_global")]{let index=bones.iter().position(|b|b.name==name).unwrap();bones[index].initial_global=transform(&row[initial]);bindings.push((index,transform(&row[current])));}let mut hierarchy=RigHierarchy::new(bones,0.0).unwrap();for(index,current)in bindings{hierarchy.set_transform(index,RigSpace::Global,false,current,true).unwrap();assert_eq!(hierarchy.transform(index,RigSpace::Global,false).unwrap(),current,"Exact common bone input");}
 let mut context=RigUnitContext{hierarchy:&mut hierarchy,curves:&curves,game_world:true,construction:false,delta_seconds:row["delta_seconds"].as_f64().unwrap(),geometry:&definition.geometry,to_world:RigTransform::default(),line_trace:None};context.call("FAlsRigUnit_ApplyFootOffsetRotation::Execute",args).unwrap();let stages=evaluate(row,0,&definition.geometry);assert_eq!(args[12],RigValue::Quaternion(DQuat::from_array(project(q(&stages["result"])))),"Current model matches exact public common data output");output.push(json!({"dataset":dataset,"query":row["query"],"pc":pc,"arguments_after":args,"stages":stages}));
 }output
}

fn main(){
 let root=std::path::Path::new("docs/audit/bf144/471");let label=std::env::args().nth(1).unwrap();let input:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();let rows=input["rows"].as_array().unwrap();assert_eq!(rows.len(),684);let definition:RigDefinition=serde_json::from_value(input["definition"].clone()).unwrap();let public=common_public(rows,&definition);let mut outputs=Vec::new();let mut exact=0;let mut first=None;
 for(index,row)in rows.iter().enumerate(){for mask in[31,30,29,23,15,0]{let stages=evaluate(row,mask,&definition.geometry);let correct=q(&stages["result"])==q(&row["expected_native"]);if mask==31{if correct{exact+=1;}else if first.is_none(){first=Some(json!({"dataset":row["dataset"],"query":row["query"],"pc":row["pc"],"stages":stages,"input":row}));}}outputs.push(json!({"dataset":row["dataset"],"query":row["query"],"pc":row["pc"],"mask":mask,"stages":stages,"expected_native":row["expected_native"],"exact":correct}));}if index%128==0{println!("Evaluated complete rotation source/public factors row={index}");}}
 std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec(&json!({"gate":"SOURCE_COMPLETE_FOOT_ROTATION_AND_EXACT_COMMON_PUBLIC_FACTORS","exact":exact,"first_difference":first,"outputs":outputs,"public_common":public})).unwrap()).unwrap();assert_eq!(exact,684,"Complete source foot rotation primitive chain");println!("Calibrated684 complete native source rotations and common public factor model");
}
