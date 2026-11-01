use bevy::{prelude::*,math::{DVec3,DQuat}};
use bevy_als::extensions::{control_rig_hierarchy::{RigHierarchy,RigTransform,RigSpace},control_rig_math::RigValue,control_rig_program::RigDefinition,control_rig_units::RigUnitContext,SourceCurveValues};
use serde_json::{Value,json};
fn dv(v:&Value)->DVec3{DVec3::from_array(std::array::from_fn(|i|v[i].as_f64().unwrap()))}
fn local(v:&Value)->RigTransform{RigTransform{translation:dv(&v["translation"]),rotation:DQuat::from_array(std::array::from_fn(|i|v["rotation"][i].as_f64().unwrap())),scale:dv(&v["scale"])}}
fn input(v:&Value)->RigValue{let mut v=v.clone();let x=v.as_object_mut().unwrap();if let Some(a)=x.remove("Vec3"){x.insert("Vector".into(),a);}if let Some(a)=x.remove("Quat"){x.insert("Quaternion".into(),a);}serde_json::from_value(v).unwrap()}
fn main(){
 let r=std::path::Path::new("docs/audit/bf144/459");let label=std::env::args().nth(1).unwrap();let data:Value=serde_json::from_slice(&std::fs::read(r.join("inputs.json")).unwrap()).unwrap();
 let definition:RigDefinition=serde_json::from_value(data["definition"].clone()).unwrap();let frames=data["frames"].as_array().unwrap();assert_eq!(frames.len(),319);
 let mut hierarchy=RigHierarchy::new(definition.bones.clone(),definition.translation_tolerance).unwrap();let mut values=definition.program.initial_values.clone();let mut outputs=Vec::new();let mut total_calls=0;let mut total_traces=0;
 for (query,frame)in frames.iter().enumerate(){
  for b in frame["bindings"].as_array().unwrap(){let mut reference=definition.externals[b["external"].as_str().unwrap()].clone();reference.path.extend(b["path"].as_array().unwrap().iter().map(|x|x.as_u64().unwrap()as usize));let v=if b["resolved_player_property"].is_null(){&b["global"]}else{&b["resolved_player_property"]};reference.write(&mut values,definition.program.literal_count,input(v)).unwrap();}
  let delta=frame["delta_seconds"].as_f64().unwrap();let to_world:RigTransform=serde_json::from_value(frame["to_world"].clone()).unwrap();let game_world=frame["game_world"].as_bool().unwrap();
  if query==0{let curves=SourceCurveValues::default();let mut context=RigUnitContext{hierarchy:&mut hierarchy,curves:&curves,game_world,construction:false,delta_seconds:delta,geometry:&definition.geometry,to_world,line_trace:None};definition.program.execute("Construction",&mut values,|_,name,args|context.call(name,args)).unwrap();}
  let pose=frame["pose"]["bones"].as_array().unwrap();let local=definition.bones.iter().map(|b|local(pose.iter().find(|p|p["name"]==b.name).unwrap())).collect::<Vec<_>>();hierarchy.set_input_pose(&local).unwrap();
  let curves:SourceCurveValues=definition.curves.keys().filter_map(|name|frame["curves"].as_object().unwrap().iter().find(|(n,_)|n.eq_ignore_ascii_case(name)).map(|(_,v)|(name.clone().into(),v.as_f64().unwrap()as f32))).collect();
  let expected_calls=frame["expected_calls"].as_array().unwrap();let expected_traces=frame["expected_traces"].as_array().unwrap();let pc=std::cell::Cell::new(0);let trace_index=std::cell::Cell::new(0);let mut call_index=0;
  let mut trace=|start:DVec3,end:DVec3,_channel:&str,_complex:bool|{let i=trace_index.get();let expected=&expected_traces[i];assert_eq!(pc.get(),expected["instruction"].as_u64().unwrap()as usize);assert_eq!(start,dv(&expected["start"]),"Actual trace start query {query}");assert_eq!(end,dv(&expected["end"]),"Actual trace end query {query}");trace_index.set(i+1);Ok(if expected["hit"].is_null(){None}else{Some((dv(&expected["hit"][0]),dv(&expected["hit"][1])))})};
  let mut context=RigUnitContext{hierarchy:&mut hierarchy,curves:&curves,game_world,construction:false,delta_seconds:delta,geometry:&definition.geometry,to_world,line_trace:if expected_traces.is_empty(){None}else{Some(&mut trace)}};
  let steps=definition.program.execute("Forwards Solve",&mut values,|instruction,name,args|{
   pc.set(instruction);let expected=&expected_calls[call_index];assert_eq!(instruction,expected["instruction"].as_u64().unwrap()as usize,"Actual call order query {query}");
   let indices=args.iter().filter_map(|v|context.hierarchy.item_index(v).ok().flatten()).collect::<Vec<_>>();let mut snapshot=context.hierarchy.clone();let before=indices.into_iter().map(|i|(i,snapshot.transform(i,RigSpace::Global,false).unwrap())).collect::<Vec<_>>();assert_eq!(serde_json::to_value(before).unwrap(),expected["bones_before_call"],"Actual pre-call bones query {query} instruction {instruction}");
   context.call(name,args)?;let actual=serde_json::to_value(&*args).unwrap();if actual!=expected["arguments_after_call"]{std::fs::write(r.join(format!("failure-{label}.json")),serde_json::to_vec_pretty(&json!({"query":query,"instruction":instruction,"function":name,"actual":actual,"expected":expected["arguments_after_call"]})).unwrap()).unwrap();panic!("Actual post-call arguments differ at query {query} instruction {instruction}");}call_index+=1;Ok(())
  }).unwrap();
  assert_eq!(steps,frame["expected_steps"].as_u64().unwrap()as usize);assert_eq!(call_index,expected_calls.len());assert_eq!(trace_index.get(),expected_traces.len());drop(context);
  let actual=serde_json::to_value(&values).unwrap();assert_eq!(actual,frame["expected_registers"],"Actual complete working state query {query}");
  let local=definition.bones.iter().enumerate().map(|(i,b)|json!({"index":i,"name":b.name,"parent":b.parent,"local":hierarchy.transform(i,RigSpace::Local,false).unwrap()})).collect::<Vec<_>>();assert_eq!(json!(local),frame["expected_local"],"Actual local hierarchy query {query}");
  outputs.push(json!({"query":query,"registers":actual,"local":local,"steps":steps,"calls":call_index,"traces":trace_index.get()}));total_calls+=call_index;total_traces+=trace_index.get();if query%32==0{println!("Replayed complete actual ControlRig state query {query}");}
 }
 std::fs::write(r.join(format!("replay-{label}.json")),serde_json::to_vec(&json!({"gate":"PASS_ACTUAL319_PUBLIC_VM_COMPLETE_STATE_REPLAY","queries":319,"calls":total_calls,"traces":total_traces,"outputs":outputs,"source_equivalence_claimed":false})).unwrap()).unwrap();println!("Replayed319 complete actual public VM states and all call/trace contracts");
}
