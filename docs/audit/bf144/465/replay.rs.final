use bevy::prelude::*;
use bevy_als::extensions::{control_rig_hierarchy::{RigHierarchy,RigTransform},control_rig_math::RigValue,control_rig_program::RigDefinition,control_rig_units::RigUnitContext,SourceCurveValues};
use serde_json::{Value,json};
fn source(v:&Value)->Vec<RigValue>{v.as_array().unwrap().iter().enumerate().map(|(i,x)|if i==4||i==7{RigValue::Bool(x.as_bool().unwrap())}else if i==11{RigValue::List(vec![RigValue::F32(x[0].as_f64().unwrap()as f32),RigValue::F32(x[1].as_f64().unwrap()as f32),RigValue::Bool(x[2].as_bool().unwrap())])}else{RigValue::F32(x.as_f64().unwrap()as f32)}).collect()}
fn main(){
 let root=std::path::Path::new("docs/audit/bf144/465");let label=std::env::args().nth(1).unwrap();let data:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();let definition:RigDefinition=serde_json::from_value(data["definition"].clone()).unwrap();let frames=data["frames"].as_array().unwrap();assert_eq!(frames.len(),319);let mut modes=Vec::new();
 for mode in 0..3{
  let mut hierarchy=RigHierarchy::new(definition.bones.clone(),definition.translation_tolerance).unwrap();let curves=SourceCurveValues::default();let mut args=vec![RigValue::F32(0.0);12];args[11]=RigValue::List(vec![RigValue::F32(0.0),RigValue::F32(0.0),RigValue::Bool(false)]);let mut outputs=Vec::new();let mut native_exact=0;let mut first=None;
  for frame in frames{
   let query=frame["query"].as_u64().unwrap();let expected=if mode<2{serde_json::from_value::<Vec<RigValue>>(frame["target_domains"][mode]["after"].clone()).unwrap()}else{source(&frame["source_arguments_after"])};
   let inputs=if mode<2{serde_json::from_value::<Vec<RigValue>>(frame["target_domains"][mode]["before"].clone()).unwrap()}else{source(&frame["source_arguments_after"])};
   args[..8].clone_from_slice(&inputs[..8]);if mode<2{assert_eq!(&args[8..],&inputs[8..],"Actual incoming spring state query {query} mode {mode}");}
   let before=args.clone();let mut context=RigUnitContext{hierarchy:&mut hierarchy,curves:&curves,game_world:true,construction:false,delta_seconds:frame["delta_seconds"].as_f64().unwrap(),geometry:&definition.geometry,to_world:RigTransform::default(),line_trace:None};context.call("FRigUnit_SpringInterpV2::Execute",&mut args).unwrap();
   if mode<2{assert_eq!(args,expected,"Actual public spring replay query {query} mode {mode}");}else if args==expected{native_exact+=1;}else if first.is_none(){first=Some(json!({"query":query,"before":before,"actual":args,"expected_native":expected}));}
   outputs.push(json!({"query":query,"before":before,"after":args,"expected":expected,"exact":args==expected}));if query%64==0{println!("Replayed pelvis spring mode={mode} query={query}");}
  }
  modes.push(json!({"mode":mode,"native_exact":native_exact,"first_difference":first,"outputs":outputs}));
 }
 std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec(&json!({"gate":"ACTUAL319_PELVIS_SPRING_PUBLIC_AND_SOURCE_CM_HISTORY","modes":modes,"source_equivalence_claimed":false})).unwrap()).unwrap();println!("Replayed three continuous319 pelvis spring histories");
}
