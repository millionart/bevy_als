use bevy::math::DVec3;
use bevy_als::extensions::{control_rig_hierarchy::{RigHierarchy,RigTransform},control_rig_math::{RigValue,rig_spring_damper,rig_clamp01},control_rig_program::RigDefinition,control_rig_units::RigUnitContext,SourceCurveValues};
use serde_json::{Value,json};
fn main(){
 let root=std::path::Path::new("docs/audit/bf144/466");let label=std::env::args().nth(1).unwrap();let data:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();let definition:RigDefinition=serde_json::from_value(data["definition"].clone()).unwrap();let rows=data["rows"].as_array().unwrap();assert_eq!(rows.len(),636);assert_eq!(definition.geometry.basis[2],DVec3::Y);let mut domains=Vec::new();
 for mode in 0..4{
  let mut carried:Vec<Option<Vec<RigValue>>>=vec![None,None];let mut initialized=[false;2];let mut values=[0.0_f32;2];let mut velocity=[0.0_f32;2];let mut previous=[0.0_f32;2];let mut valid=[false;2];let mut outputs=Vec::new();
  for row in rows{
   let query=row["query"].as_u64().unwrap();let pc=row["pc"].as_u64().unwrap();let side=if pc==83{0}else{assert_eq!(pc,113);1};let delta=row["delta_seconds"].as_f64().unwrap();let source=&row["source_arguments_after"];
   if mode<2{
    let input:Vec<RigValue>=serde_json::from_value(row["target_domains"][mode]["before"].clone()).unwrap();let expected:Vec<RigValue>=serde_json::from_value(row["target_domains"][mode]["after"].clone()).unwrap();let args=carried[side].get_or_insert_with(||input.clone());args[..11].clone_from_slice(&input[..11]);assert_eq!(&args[11..],&input[11..],"Actual incoming foot state query {query} pc {pc} mode {mode}");let before=args.clone();
    let mut bones=definition.bones.clone();let mut read_bones=Vec::new();for pair in row["target_domains"][mode]["bones_before"].as_array().unwrap(){let index=pair[0].as_u64().unwrap()as usize;let value:RigTransform=serde_json::from_value(pair[1].clone()).unwrap();bones[index].initial_global=value;read_bones.push((index,value));}
    let mut hierarchy=RigHierarchy::new(bones,definition.translation_tolerance).unwrap();for(index,value)in &read_bones{assert_eq!(hierarchy.transform(*index,bevy_als::extensions::control_rig_hierarchy::RigSpace::Global,false).unwrap(),*value);}
    assert_eq!(read_bones.len(),2);assert_eq!(hierarchy.item_index(&args[0]).unwrap(),Some(read_bones[0].0));assert_eq!(hierarchy.item_index(&args[1]).unwrap(),Some(read_bones[1].0));let RigValue::Vector(target_location)=args[2]else{panic!("Missing actual target vector")};let scalar=|i:usize|{let RigValue::F32(v)=args[i]else{panic!("Missing scalar input")};v};let target=scalar(3).min(read_bones[0].1.translation.y as f32-scalar(6)-target_location.y as f32).max(scalar(4));let curves=SourceCurveValues::default();if query>2{args[14]=RigValue::List(vec![RigValue::F32(0.0),RigValue::F32(0.0),RigValue::Bool(false)]);}let mut context=RigUnitContext{hierarchy:&mut hierarchy,curves:&curves,game_world:true,construction:false,delta_seconds:delta,geometry:&definition.geometry,to_world:RigTransform::default(),line_trace:None};context.call("FAlsRigUnit_ApplyFootOffsetLocation::Execute",args).unwrap();assert_eq!(*args,expected,"Actual public foot replay query {query} pc {pc} mode {mode}");
    outputs.push(json!({"query":query,"pc":pc,"target":target,"before":before,"after":args,"complete_actual_exact":true}));
   }else{
    let target=if mode==2{row["source_target_cm"].as_f64().unwrap()as f32}else{(row["source_target_cm"].as_f64().unwrap()*0.01)as f32};let dt=delta as f32;let frequency=source[8].as_f64().unwrap()as f32;let damping=source[9].as_f64().unwrap()as f32;let amount=source[10].as_f64().unwrap()as f32;let before=json!({"initialized":initialized[side],"offset":values[side],"state":[velocity[side],previous[side],valid[side]]});
    if !initialized[side]{initialized[side]=true;velocity[side]=0.0;previous[side]=0.0;valid[side]=false;values[side]=target;}else if dt>1e-8{if !valid[side]{velocity[side]=0.0;previous[side]=target;valid[side]=true;values[side]=target;}else{let target_velocity=(target-previous[side])*rig_clamp01(amount)/dt;rig_spring_damper(&mut values[side],&mut velocity[side],target,target_velocity,dt,frequency,damping);previous[side]=target;}}
    let after=json!({"initialized":initialized[side],"offset":values[side],"state":[velocity[side],previous[side],valid[side]]});if mode==2{assert_eq!(initialized[side],source[11].as_bool().unwrap());assert_eq!(values[side],source[15].as_f64().unwrap()as f32,"Source cm offset query {query} pc {pc}");assert_eq!(velocity[side],source[14][0].as_f64().unwrap()as f32);assert_eq!(previous[side],source[14][1].as_f64().unwrap()as f32);assert_eq!(valid[side],source[14][2].as_bool().unwrap());}
    outputs.push(json!({"query":query,"pc":pc,"target":target,"before":before,"after":after,"source_native_exact":mode==2}));
   }
   if pc==83&&query%64==0{println!("Replayed foot spring mode={mode} query={query}");}
  }
  domains.push(json!({"mode":mode,"outputs":outputs}));
 }
 std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec(&json!({"gate":"ACTUAL636_FOOT_LOCATION_PUBLIC_AND_SOURCE_SCALAR_HISTORIES","domains":domains,"final_leg_vector_source_equivalence_claimed":false})).unwrap()).unwrap();println!("Replayed four independent continuous foot histories");
}
