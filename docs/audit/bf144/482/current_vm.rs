use std::sync::Arc;
use avian3d::prelude::*;
use bevy::{math::{DVec3,DQuat},platform::collections::HashMap,prelude::*};
use bevy_als::extensions::{control_rig_hierarchy::{RigHierarchy,RigTransform,RigSpace},control_rig_math::RigValue,control_rig_program::RigDefinition,control_rig_query::{RigQueryContext,RigQueryWorld},control_rig_units::RigUnitContext,SourceCurveValues};
use serde_json::{Value,json};

fn vector(v:&Value)->DVec3{DVec3::from_array(std::array::from_fn(|i|v[i].as_f64().unwrap()))}
fn transform(v:&Value)->RigTransform{RigTransform{translation:vector(&v["translation"]),rotation:DQuat::from_array(std::array::from_fn(|i|v["rotation"][i].as_f64().unwrap())),scale:vector(&v["scale"])}}
fn input(v:&Value)->RigValue{let mut v=v.clone();let fields=v.as_object_mut().unwrap();if let Some(a)=fields.remove("Vec3"){fields.insert("Vector".into(),a);}if let Some(a)=fields.remove("Quat"){fields.insert("Quaternion".into(),a);}serde_json::from_value(v).unwrap()}
fn query()->RigQueryContext{
 let mut colliders=HashMap::default();let mut world=World::new();for x in [-20.0,0.0]{colliders.insert(world.spawn_empty().id(),(Position(Vec3::new(x,-0.25,-20.0)),Rotation::default(),Collider::cuboid(20.0,0.5,20.0),CollisionLayers::ALL));}
 RigQueryContext{world:Arc::new(RigQueryWorld{colliders}),to_world:RigTransform::default(),channel:"ECC_Visibility".into(),filter:SpatialQueryFilter::default()}
}
fn source_trace(args:&mut [RigValue],world:RigTransform,query:&RigQueryContext)->Result<(DVec3,DVec3,(DVec3,DVec3)),String>{
 let [RigValue::Vector(foot),RigValue::Name(channel),RigValue::F32(upward),RigValue::F32(downward),RigValue::F32(angle),RigValue::F32(height),RigValue::Bool(enabled),RigValue::Bool(_),RigValue::F32(_),RigValue::Vector(_)]=args else{return Err("Source trace ABI is invalid".into())};
 assert!(*enabled);assert_eq!(channel,"ECC_Visibility");assert!(world.scale.to_array().iter().all(|v|v.abs()>1e-8_f32 as f64));
 let rotate=|q:DVec3,w:f64,v:DVec3|{let t=q.cross(v)*2.0;(v+t*w)+q.cross(t)};
 let forward=|v:DVec3|rotate(world.rotation.xyz(),world.rotation.w,world.scale*v)+world.translation;
 let start=forward(DVec3::new(foot.x,*upward as f64,foot.z));let end=forward(DVec3::new(foot.x,-(*downward as f64),foot.z));
 let hit=query.trace(start,end,channel,true)?.ok_or("Source trace did not hit actual geometry")?;
 let inverse=|v:DVec3|rotate(-world.rotation.xyz(),world.rotation.w,v)*world.scale.recip();
 let normal=inverse(hit.1);assert!(normal.y>=angle.to_radians().cos()as f64);let slope=normal.y as f32;
 let correction=if slope>1e-8{*height/slope-*height}else{0.0};let point=inverse(hit.0-world.translation);
 args[8]=RigValue::F32((point.y+correction as f64)as f32);args[9]=RigValue::Vector(normal);Ok((start,end,hit))
}
fn raw_current(h:&RigHierarchy)->String{let dump=format!("{h:?}");let(_,tail)=dump.split_once("current: [").unwrap();let(raw,_)=tail.rsplit_once("], translation_tolerance:").unwrap();raw.to_owned()}
fn main(){
 let root=std::path::Path::new("docs/audit/bf144/482");let label=std::env::args().nth(1).unwrap();let data:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/459/inputs.json").unwrap()).unwrap();let reference:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/463/replay-restored.json").unwrap()).unwrap();
 let definition:RigDefinition=serde_json::from_value(data["definition"].clone()).unwrap();let frames=data["frames"].as_array().unwrap();assert_eq!(frames.len(),319);let geometry=query();let mut modes=Vec::new();
 for source in [false,true]{
  let mut hierarchy=RigHierarchy::new(definition.bones.clone(),definition.translation_tolerance).unwrap();let mut values=definition.program.initial_values.clone();let mut outputs=Vec::new();let mut traces=0;let mut calls=0;let mut qualified=0;
  for (frame_index,frame)in frames.iter().enumerate(){
   for binding in frame["bindings"].as_array().unwrap(){let mut target=definition.externals[binding["external"].as_str().unwrap()].clone();target.path.extend(binding["path"].as_array().unwrap().iter().map(|x|x.as_u64().unwrap()as usize));let v=if binding["resolved_player_property"].is_null(){&binding["global"]}else{&binding["resolved_player_property"]};target.write(&mut values,definition.program.literal_count,input(v)).unwrap();}
   let delta=frame["delta_seconds"].as_f64().unwrap();let world:RigTransform=serde_json::from_value(frame["to_world"].clone()).unwrap();let game_world=frame["game_world"].as_bool().unwrap();
   if frame_index==0{let curves=SourceCurveValues::default();let mut context=RigUnitContext{hierarchy:&mut hierarchy,curves:&curves,game_world,construction:false,delta_seconds:delta,geometry:&definition.geometry,to_world:world,line_trace:None};definition.program.execute("Construction",&mut values,|_,name,args|context.call(name,args)).unwrap();}
   let bones=frame["pose"]["bones"].as_array().unwrap();let local=definition.bones.iter().map(|b|transform(bones.iter().find(|p|p["name"]==b.name).unwrap())).collect::<Vec<_>>();hierarchy.set_input_pose(&local).unwrap();
   let curves:SourceCurveValues=definition.curves.keys().filter_map(|name|frame["curves"].as_object().unwrap().iter().find(|(n,_)|n.eq_ignore_ascii_case(name)).map(|(_,v)|(name.clone().into(),v.as_f64().unwrap()as f32))).collect();let expected_calls=frame["expected_calls"].as_array().unwrap();let expected_traces=frame["expected_traces"].as_array().unwrap();let instruction=std::cell::Cell::new(0);let trace_index=std::cell::Cell::new(0);let mut call_index=0;let mut observed_calls=Vec::new();let mut observed_traces=Vec::new();
   let mut trace=|start:DVec3,end:DVec3,channel:&str,complex:bool|{let i=trace_index.get();let expected=&expected_traces[i];assert_eq!(instruction.get(),expected["instruction"].as_u64().unwrap()as usize);assert_eq!(start,vector(&expected["start"]));assert_eq!(end,vector(&expected["end"]));let hit=geometry.trace(start,end,channel,complex)?;assert_eq!(hit,Some((vector(&expected["hit"][0]),vector(&expected["hit"][1]))));trace_index.set(i+1);Ok(hit)};
   let mut context=RigUnitContext{hierarchy:&mut hierarchy,curves:&curves,game_world,construction:false,delta_seconds:delta,geometry:&definition.geometry,to_world:world,line_trace:Some(&mut trace)};
   let steps=definition.program.execute("Forwards Solve",&mut values,|pc,name,args|{
    instruction.set(pc);let expected=&expected_calls[call_index];assert_eq!(pc,expected["instruction"].as_u64().unwrap()as usize);let before_args=json!(&*args);let setter_raw_before=if [100,130].contains(&pc){Some(raw_current(context.hierarchy))}else{None};let raw_before=if name=="FRigUnit_TwoBoneIKSimplePerItem::Execute"&&[91,121].contains(&pc){Some(raw_current(context.hierarchy))}else{None};let clean_control=if raw_before.is_some()&&!source&&frame_index==1&&pc==91{let mut copy=context.hierarchy.clone();for i in 0..definition.bones.len(){copy.transform(i,RigSpace::Global,false)?;}Some(copy)}else{None};
    let indices=args.iter().filter_map(|v|context.hierarchy.item_index(v).ok().flatten()).collect::<Vec<_>>();let mut snapshot=context.hierarchy.clone();let before=indices.into_iter().map(|i|(i,snapshot.transform(i,RigSpace::Global,false).unwrap())).collect::<Vec<_>>();if !source{assert_eq!(json!(before),expected["bones_before_call"]);}
    if source&&name=="FAlsRigUnit_FootOffsetTrace::Execute"{
     let oracle=reference["outputs"].as_array().unwrap().iter().find(|v|v["query"]==frame_index&&v["pc"]==pc).unwrap();let original:Vec<RigValue>=serde_json::from_value(oracle["current"].clone()).unwrap();assert_eq!(args[..8],original[..8],"Qualified same trace inputs");
     let (start,end,hit)=source_trace(args,world,&geometry)?;let target:Vec<RigValue>=serde_json::from_value(oracle["source_forward_full_inverse"].clone()).unwrap();assert_eq!(&*args,&target,"Qualified complete source vector trace outputs");qualified+=1;observed_traces.push(json!({"instruction":pc,"start":start,"end":end,"hit":hit}));
    }else{context.call(name,args)?;}
    if !source{assert_eq!(json!(&*args),expected["arguments_after_call"]);}
    let setter_raw_after=if setter_raw_before.is_some(){Some(raw_current(context.hierarchy))}else{None};let raw_after=if raw_before.is_some(){Some(raw_current(context.hierarchy))}else{None};
    let clean_control=if let Some(mut copy)=clean_control{let clean_before=raw_current(&copy);let mut reader=context.hierarchy.clone();let requests=args[..3].iter().map(|key|{let i=reader.item_index(key).unwrap().unwrap();(i,reader.transform(i,RigSpace::Global,false).unwrap())}).collect::<Vec<_>>();for(i,value)in &requests{copy.set_transform(*i,RigSpace::Global,false,*value,true)?;}Some(json!({"before":clean_before,"after":raw_current(&copy),"requests":requests}))}else{None};
    let after=if name=="FRigUnit_SetRotation::Execute"{let index=context.hierarchy.item_index(&args[0]).unwrap().unwrap();Some(context.hierarchy.transform(index,RigSpace::Global,false).unwrap())}else{None};let ik_after=if name=="FRigUnit_TwoBoneIKSimplePerItem::Execute"{let mut snapshot=context.hierarchy.clone();Some(args[..3].iter().map(|v|{let i=snapshot.item_index(v).unwrap().unwrap();(i,snapshot.transform(i,RigSpace::Global,false).unwrap())}).collect::<Vec<_>>())}else{None};observed_calls.push(json!({"instruction":pc,"function":name,"arguments_before_call":before_args,"arguments_after_call":args,"bones_before_call":before,"bone_after_call":after,"ik_after_call":ik_after,"raw_cache_before":raw_before,"raw_cache_after":raw_after,"clean_cache_control":clean_control,"setter_raw_before":setter_raw_before,"setter_raw_after":setter_raw_after}));call_index+=1;Ok(())
   }).unwrap();drop(context);assert_eq!(steps,frame["expected_steps"].as_u64().unwrap()as usize);assert_eq!(call_index,expected_calls.len());if !source{assert_eq!(trace_index.get(),expected_traces.len());assert_eq!(json!(values),frame["expected_registers"]);}
   let local=definition.bones.iter().enumerate().map(|(i,b)|json!({"index":i,"name":b.name,"parent":b.parent,"local":hierarchy.transform(i,RigSpace::Local,false).unwrap()})).collect::<Vec<_>>();if !source{assert_eq!(json!(local),frame["expected_local"]);}
   let mut global_clone=hierarchy.clone();let final_globals=(0..definition.bones.len()).map(|i|global_clone.transform(i,RigSpace::Global,false).unwrap()).collect::<Vec<_>>();let frame_traces=if source{observed_traces.len()}else{trace_index.get()};traces+=frame_traces;calls+=call_index;outputs.push(json!({"query":frame_index,"registers":values,"local":local,"steps":steps,"calls":observed_calls,"trace_count":frame_traces,"source_traces":observed_traces,"final_globals":final_globals}));if frame_index%32==0{println!("Replayed continuous VM source={source} query={frame_index}");}
  }
  assert_eq!(traces,636);assert_eq!(calls,13378);if source{assert_eq!(qualified,636);}
  modes.push(json!({"source":source,"queries":319,"calls":calls,"traces":traces,"qualified_source_trace_calls":qualified,"outputs":outputs}));
 }
 std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec(&json!({"gate":"ACTUAL319_CONTINUOUS_PUBLIC_VM_TRACE_VECTOR_CONTROL","modes":modes,"source_full_rig_equivalence_claimed":false})).unwrap()).unwrap();println!("Qualified both continuous319 VM modes with unchanged non-trace public units");
}
