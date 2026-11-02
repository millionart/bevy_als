use bevy::math::DVec3;
use bevy_als::extensions::{control_rig_hierarchy::{RigHierarchy,RigTransform,RigSpace},control_rig_math::RigValue,control_rig_program::RigDefinition,control_rig_units::RigUnitContext,SourceCurveValues};
use serde_json::{Value,json};
fn vector(value:&Value)->DVec3{DVec3::new(value[0].as_f64().unwrap(),value[1].as_f64().unwrap(),value[2].as_f64().unwrap())}
fn projected(value:DVec3)->DVec3{DVec3::new(value.x*0.01,value.z*0.01,value.y*0.01)}
fn reference(target:DVec3,thigh:DVec3,offset:f32,length:f32,stretch:f32)->(DVec3,bool){
 let foot=DVec3::new(target.x,target.y,target.z+offset as f64);let leg=foot-thigh;let maximum=(length*stretch)as f64;assert!(maximum>1.0);let squared=(leg.x*leg.x+leg.y*leg.y)+leg.z*leg.z;let active=squared>maximum*maximum;let leg=if active{leg*(maximum*(1.0/squared.sqrt()))}else{leg};(thigh+leg,active)
}
fn main(){
 let root=std::path::Path::new("docs/audit/bf144/468");let label=std::env::args().nth(1).unwrap();let data:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();let definition:RigDefinition=serde_json::from_value(data["definition"].clone()).unwrap();let rows=data["rows"].as_array().unwrap();assert_eq!(rows.len(),636);let curves=SourceCurveValues::default();let mut modes=Vec::new();
 for mode in 0..4{
  let mut carried:Vec<Option<Vec<RigValue>>>=vec![None,None];let mut outputs=Vec::new();let mut native_vectors_exact=0;let mut first=None;let mut active=0;
  for row in rows{
   let query=row["query"].as_u64().unwrap();let pc=row["pc"].as_u64().unwrap();let side=if pc==83{0}else{assert_eq!(pc,113);1};let src=&row["source_arguments_after"];let source_thigh=vector(&row["source_thigh_translation_cm"]);let source_pelvis=vector(&row["source_pelvis_translation_cm"]);let source_target=vector(&src[2]);let source_offset=row["predicted_source_offset_cm"].as_f64().unwrap()as f32;
   let mut input:Vec<RigValue>=serde_json::from_value(row["target_domains"][if mode==1{1}else{0}]["before"].clone()).unwrap();
   if mode>=2{
    input[2]=RigValue::Vector(if mode==2{source_target}else{projected(source_target)});
    for i in 3..11{let v=src[i].as_f64().unwrap();input[i]=RigValue::F32(if mode==3&&i<=6{(v*0.01)as f32}else{v as f32});}
   }
   let args=carried[side].get_or_insert_with(||input.clone());args[..11].clone_from_slice(&input[..11]);if mode<2{assert_eq!(&args[11..],&input[11..],"Actual incoming foot state query {query} pc {pc} mode {mode}");}
   let before=args.clone();let mut bones=definition.bones.clone();let mut geometry=definition.geometry.clone();let mut bindings=Vec::new();
   if mode<2{
    for pair in row["target_domains"][mode]["bones_before"].as_array().unwrap(){bindings.push((pair[0].as_u64().unwrap()as usize,serde_json::from_value::<RigTransform>(pair[1].clone()).unwrap()));}
   }else{
    for(i,position)in[(0,source_pelvis),(1,source_thigh)]{let RigValue::List(key)=&args[i]else{panic!("Missing item key")};let RigValue::Name(name)=&key[1]else{panic!("Missing item name")};let index=bones.iter().position(|b|b.name==*name).unwrap();let mut transform=bones[index].initial_global;transform.translation=if mode==2{position}else{projected(position)};bindings.push((index,transform));}
    if mode==2{geometry.basis[2]=DVec3::Z;geometry.near_zero=1e-4_f32 as f64;}
   }
   for(index,value)in &bindings{bones[*index].initial_global=*value;}
   let mut hierarchy=RigHierarchy::new(bones,definition.translation_tolerance).unwrap();assert_eq!(hierarchy.item_index(&args[0]).unwrap(),Some(bindings[0].0));assert_eq!(hierarchy.item_index(&args[1]).unwrap(),Some(bindings[1].0));for(index,value)in &bindings{assert_eq!(hierarchy.transform(*index,RigSpace::Global,false).unwrap(),*value);}
   let mut context=RigUnitContext{hierarchy:&mut hierarchy,curves:&curves,game_world:true,construction:false,delta_seconds:row["delta_seconds"].as_f64().unwrap(),geometry:&geometry,to_world:RigTransform::default(),line_trace:None};context.call("FAlsRigUnit_ApplyFootOffsetLocation::Execute",args).unwrap();
   if mode<2{let expected:Vec<RigValue>=serde_json::from_value(row["target_domains"][mode]["after"].clone()).unwrap();assert_eq!(*args,expected,"Actual full public foot query {query} pc {pc} mode {mode}");}
   let RigValue::Vector(location)=args[16]else{panic!("Missing foot output")};let(native,clamped)=reference(source_target,source_thigh,source_offset,src[5].as_f64().unwrap()as f32,src[7].as_f64().unwrap()as f32);assert_eq!(native,vector(&src[16]),"Native full vector qualification query {query} pc {pc}");active+=usize::from(clamped);
   let expected=if mode==2{native}else{projected(native)};let exact=location==expected;if exact{native_vectors_exact+=1;}else if first.is_none(){first=Some(json!({"query":query,"pc":pc,"actual":location,"expected_source":expected,"clamped":clamped}));}
   if mode==2{assert_eq!(args[11],RigValue::Bool(src[11].as_bool().unwrap()));assert_eq!(args[15],RigValue::F32(source_offset));let state=RigValue::List(vec![RigValue::F32(src[14][0].as_f64().unwrap()as f32),RigValue::F32(src[14][1].as_f64().unwrap()as f32),RigValue::Bool(src[14][2].as_bool().unwrap())]);assert_eq!(args[14],state,"Actual cm state query {query} pc {pc}");}
   if mode==3{let expected=&row["expected_common_meter_scalar"];assert_eq!(args[11],RigValue::Bool(expected["initialized"].as_bool().unwrap()));assert_eq!(args[15],RigValue::F32(expected["offset"].as_f64().unwrap()as f32));}
   outputs.push(json!({"query":query,"pc":pc,"before":before,"after":args,"source_expected":expected,"source_clamp_active":clamped,"source_vector_exact":exact}));if pc==83&&query%64==0{println!("Replayed full foot location mode={mode} query={query}");}
  }
  modes.push(json!({"mode":mode,"outputs":outputs,"native_vectors_exact":native_vectors_exact,"first_difference":first,"source_clamp_active":active}));
 }
 std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec(&json!({"gate":"ACTUAL636_COMPLETE_FOOT_LOCATION_AND_SAME_NUMERIC_CM_PUBLIC_CONTROL","modes":modes,"full_rig_equivalence_claimed":false})).unwrap()).unwrap();println!("Replayed four complete foot location histories and exact native vector kernel");
}
