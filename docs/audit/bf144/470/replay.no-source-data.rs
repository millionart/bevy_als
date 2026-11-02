use bevy::math::DQuat;
use serde_json::{Value,json};
type Q=[f64;4];
fn q(v:&Value)->Q{std::array::from_fn(|i|v[i].as_f64().unwrap())}
fn project(a:Q)->Q{[-a[0],-a[2],-a[1],a[3]]}
fn inverse(a:Q)->Q{[-a[0],-a[1],-a[2],a[3]]}
fn product(a:Q,b:Q)->Q{
 let mut r=b.map(|x|a[3]*x);
 for(i,swizzle,sign)in[(0,[3,2,1,0],[1.0,-1.0,1.0,-1.0]),(1,[2,3,0,1],[1.0,1.0,-1.0,-1.0]),(2,[1,0,3,2],[-1.0,1.0,1.0,-1.0])]{for j in 0..4{r[j]=(a[i]*b[swizzle[j]]).mul_add(sign[j],r[j]);}}
 r
}
fn main(){
 let root=std::path::Path::new("docs/audit/bf144/470");let label=std::env::args().nth(1).unwrap();let data:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();let rows=data["rows"].as_array().unwrap();assert_eq!(rows.len(),684);let mut outputs=Vec::new();
 for(index,row)in rows.iter().enumerate(){
  let a=q(&row["source_calf_initial"]);let b=q(&row["source_foot_initial"]);let native=product(inverse(a),b);assert_eq!(native,q(&row["expected_native_relative"]),"Native initial relative dataset {} query {} pc {}",row["dataset"],row["query"],row["pc"]);let expected=project(native);let mut variants=Vec::new();
  for source_data in[false,true]{for source_order in[false,true]{
   let(a,b)=if false{(project(a),project(b))}else{(q(&row["current_calf_initial"]),q(&row["current_foot_initial"]))};let actual=if source_order{project(product(inverse(project(a)),project(b)))}else{(DQuat::from_array(a).conjugate()*DQuat::from_array(b)).to_array()};
   if !source_data&&!source_order{for observed in row["observed_current_relative"].as_array().unwrap(){assert_eq!(q(&observed["Quaternion"]),actual,"Actual initial relative cache query {} pc {}",row["query"],row["pc"]);}}
   if source_data&&source_order{assert_eq!(actual,expected);}
   variants.push(json!({"source_data":source_data,"source_order":source_order,"actual":actual,"expected":expected,"exact":actual==expected}));
  }}
  outputs.push(json!({"dataset":row["dataset"],"query":row["query"],"pc":row["pc"],"native":native,"variants":variants}));if index%128==0{println!("Calibrated initial relative quaternion row={index}");}
 }
 std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec(&json!({"gate":"ACTUAL684_INITIAL_RELATIVE_NATIVE_AND_CURRENT_TWO_BY_TWO_MATRIX","outputs":outputs,"full_foot_rotation_equivalence_claimed":false})).unwrap()).unwrap();println!("Calibrated684 exact native initial relative quaternions and data/operator controls");
}
