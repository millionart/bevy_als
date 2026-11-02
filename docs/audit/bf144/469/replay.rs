use bevy::math::DVec3;
use bevy_als::extensions::{control_rig_hierarchy::{RigHierarchy,RigTransform,RigSpace},control_rig_math::RigValue,control_rig_program::RigDefinition,control_rig_units::RigUnitContext,SourceCurveValues};
use serde_json::{Value,json};
fn vector(value:&Value)->DVec3{DVec3::new(value[0].as_f64().unwrap(),value[1].as_f64().unwrap(),value[2].as_f64().unwrap())}
fn main(){
 let root=std::path::Path::new("docs/audit/bf144/469");let label=std::env::args().nth(1).unwrap();let data:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();let definition:RigDefinition=serde_json::from_value(data["definition"].clone()).unwrap();let rows=data["rows"].as_array().unwrap();assert_eq!(rows.len(),684);let curves=SourceCurveValues::default();let mut modes=Vec::new();
 for mode in 0..2{
  let mut carried:Vec<Option<Vec<RigValue>>>=vec![None,None];let mut outputs=Vec::new();
  for row in rows.iter().filter(|x|x["dataset"]=="actual"){
   let query=row["query"].as_u64().unwrap();let pc=row["pc"].as_u64().unwrap();let side=if pc==98{0}else{assert_eq!(pc,128);1};let input:Vec<RigValue>=serde_json::from_value(row["target_domains"][mode]["before"].clone()).unwrap();let expected:Vec<RigValue>=serde_json::from_value(row["target_domains"][mode]["after"].clone()).unwrap();let args=carried[side].get_or_insert_with(||input.clone());args[..8].clone_from_slice(&input[..8]);assert_eq!(&args[8..],&input[8..],"Actual incoming rotation state query {query} pc {pc} mode {mode}");let before=args.clone();let mut hierarchy=RigHierarchy::new(definition.bones.clone(),0.0).unwrap();let mut bindings=Vec::new();
   for pair in row["target_domains"][mode]["bones_before"].as_array().unwrap(){let index=pair[0].as_u64().unwrap()as usize;let value:RigTransform=serde_json::from_value(pair[1].clone()).unwrap();hierarchy.set_transform(index,RigSpace::Global,false,value,true).unwrap();assert_eq!(hierarchy.transform(index,RigSpace::Global,false).unwrap(),value,"Actual current bone input query {query} pc {pc}");assert_eq!(hierarchy.transform(index,RigSpace::Global,true).unwrap(),definition.bones[index].initial_global);bindings.push(index);}
   assert_eq!(hierarchy.item_index(&args[0]).unwrap(),Some(bindings[0]));assert_eq!(hierarchy.item_index(&args[1]).unwrap(),Some(bindings[1]));let mut context=RigUnitContext{hierarchy:&mut hierarchy,curves:&curves,game_world:true,construction:false,delta_seconds:row["delta_seconds"].as_f64().unwrap(),geometry:&definition.geometry,to_world:RigTransform::default(),line_trace:None};context.call("FAlsRigUnit_ApplyFootOffsetRotation::Execute",args).unwrap();assert_eq!(*args,expected,"Actual public rotation query {query} pc {pc} mode {mode}");outputs.push(json!({"query":query,"pc":pc,"before":before,"after":args}));if pc==98&&query%64==0{println!("Replayed rotation mode={mode} query={query}");}
  }
  assert_eq!(outputs.len(),636);modes.push(json!({"mode":mode,"outputs":outputs}));
 }
 let mut state=std::collections::BTreeMap::<(String,u64),DVec3>::new();let mut normals=Vec::new();
 for row in rows{
  let dataset=row["dataset"].as_str().unwrap();let query=row["query"].as_u64().unwrap();let pc=row["pc"].as_u64().unwrap();let src=&row["source_arguments_after"];let input=vector(&src[3]);let expected=vector(&src[11]);let key=(dataset.to_owned(),pc);let current=state.entry(key).or_insert(input);let before=*current;let alpha=bevy_als::extensions::control_rig_math::rig_damper_alpha(row["delta_seconds"].as_f64().unwrap()as f32,src[7].as_f64().unwrap()as f32);*current+=(input-*current)*alpha as f64;assert_eq!(*current,expected,"Native source normal history dataset {dataset} query {query} pc {pc}");normals.push(json!({"dataset":dataset,"query":query,"pc":pc,"input":input,"before":before,"alpha":alpha,"after":current,"expected_native":expected}));
 }
 std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec(&json!({"gate":"ACTUAL636_PUBLIC_ROTATION_AND684_SOURCE_NORMAL_HISTORIES","modes":modes,"normals":normals,"full_rotation_math_equivalence_claimed":false})).unwrap()).unwrap();println!("Replayed1272 complete public rotation calls and684 exact source normal states");
}
