use serde_json::{Value,json};
use bevy_als::extensions::control_rig_program::RigDefinition;
mod calibrated{
 include!("../471/replay.rs");
 pub fn shadow(row:&serde_json::Value,mask:u8,geometry:&bevy_als::extensions::control_rig_ik::RigGeometry)->serde_json::Value{let mut value=row.clone();let initial=prod(inv(object_q(&value["calf_initial_global"]["rotation"])),object_q(&value["foot_initial_global"]["rotation"]),mask);value["initial_relative"]=serde_json::json!(initial);evaluate(&value,mask,geometry)}
}
mod source_reference{
 include!("../471/replay.prototype.rs");
 pub fn shadow(row:&serde_json::Value,geometry:&bevy_als::extensions::control_rig_ik::RigGeometry)->serde_json::Value{let mut value=row.clone();let initial=prod(inv(object_q(&value["calf_initial_global"]["rotation"])),object_q(&value["foot_initial_global"]["rotation"]),31);value["initial_relative"]=serde_json::json!(initial);evaluate(&value,31,geometry)}
}
fn rotation(value:&Value)->Value{json!({"x":value[0],"y":value[1],"z":value[2],"w":value[3]})}
fn main(){
 let root=std::path::Path::new("docs/audit/bf144/472");let label=std::env::args().nth(1).unwrap();let data:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();let definition:RigDefinition=serde_json::from_value(data["definition"].clone()).unwrap();let rows=data["rows"].as_array().unwrap();assert_eq!(rows.len(),1272);let mut output=Vec::new();let mut reusable_differences=0;
 for(index,row)in rows.iter().enumerate(){
  for data_mask in 0..16{
   let mut input=row["source"].clone();let current=&row["current"];
   if data_mask&1==0{input["calf"]=current["calf"].clone();input["foot"]=current["foot"].clone();}
   if data_mask&2==0{input["calf_initial_global"]["rotation"]=rotation(&current["calf_initial"]);input["foot_initial_global"]["rotation"]=rotation(&current["foot_initial"]);}
   if data_mask&4==0{input["target"]=current["target"].clone();input["limits"]=current["limits"].clone();}
   if data_mask&8==0{input["normal"]=current["normal"].clone();}
   let current_result=calibrated::shadow(&input,0,&definition.geometry);let source_result=source_reference::shadow(&input,&definition.geometry);let reused=calibrated::shadow(&input,31,&definition.geometry);if source_result!=reused{reusable_differences+=1;}
   if data_mask==0{assert_eq!(current_result["result"],row["current"]["expected"],"Actual current foot rotation query {} pc {} mode {}",row["query"],row["pc"],row["mode"]);}
   if data_mask==15{assert_eq!(source_result["result"],input["expected_native"],"Full source endpoint query {} pc {}",row["query"],row["pc"]);}
   for(source_math,stages)in[(false,current_result),(true,source_result)]{
    output.push(json!({"query":row["query"],"pc":row["pc"],"mode":row["mode"],"data_mask":data_mask,"source_math":source_math,"result":stages["result"],"limited_angles":stages["limited_angles"],"expected_native":input["expected_native"],"stages":if data_mask==0||data_mask==15||data_mask==1||data_mask==2||data_mask==4||data_mask==8{stages}else{Value::Null}}));
   }
  }
  if index%192==0{println!("Evaluated actual data/source algorithm composition row={index}");}
 }
 assert_eq!(output.len(),40704);std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec(&json!({"gate":"ACTUAL1272_ROTATION_DATA_AND_ALGORITHM_COMPOSITION","outputs":output,"reused_source_vs_explicit_source_stage_differences":reusable_differences,"current_actual_exact":1272,"source_endpoint_exact":1272,"full_rig_equivalence_claimed":false})).unwrap()).unwrap();println!("Verified1272 actual current rotations and40704 complete input/algorithm combinations");
}
