use std::sync::Arc;
use bevy::{prelude::*,math::DVec3,platform::collections::HashMap};
use avian3d::prelude::*;
use bevy_als::extensions::{control_rig_query::{RigQueryContext,RigQueryWorld},control_rig_hierarchy::RigTransform};
use serde_json::{Value,json};
fn dv(v:&Value)->DVec3{DVec3::from_array(std::array::from_fn(|i|v[i].as_f64().unwrap()))}
fn query(center_y:f32)->RigQueryContext{
 let mut colliders=HashMap::default();let mut world=World::new();for x in [-20.0,0.0]{let entity=world.spawn_empty().id();colliders.insert(entity,(Position(Vec3::new(x,-0.25+center_y,-20.0)),Rotation::default(),Collider::cuboid(20.0,0.5,20.0),CollisionLayers::ALL));}
 RigQueryContext{world:Arc::new(RigQueryWorld{colliders}),to_world:RigTransform::default(),channel:"ECC_Visibility".into(),filter:SpatialQueryFilter::default()}
}
fn main(){
 let root=std::path::Path::new("docs/audit/bf144/460");let label=std::env::args().nth(1).unwrap();let input:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/459/inputs.json").unwrap()).unwrap();let reference=query(0.0);let wrong=query(-1.0);
 let mut rows=Vec::new();let mut exact=0;let mut total=0;let mut first=None;let mut wrong_changed=0;
 for frame in input["frames"].as_array().unwrap(){for (i,t)in frame["expected_traces"].as_array().unwrap().iter().enumerate(){
  let start=dv(&t["start"]);let end=dv(&t["end"]);let expected=if t["hit"].is_null(){None}else{Some((dv(&t["hit"][0]),dv(&t["hit"][1])))};
  let actual=reference.trace(start,end,"ECC_Visibility",true).unwrap();let invalid=wrong.trace(start,end,"ECC_Visibility",true).unwrap();exact+=usize::from(actual==expected);total+=1;wrong_changed+=usize::from(invalid!=expected);
  let row=json!({"query":frame["query"],"trace":i,"instruction":t["instruction"],"start":start,"end":end,"expected":expected,"actual_box":actual,"exact":actual==expected});if first.is_none()&&actual!=expected{first=Some(row.clone());}rows.push(row);
 }}
 assert_eq!(total,636);assert_eq!(wrong_changed,636);let result=json!({"gate":"ACTUAL636_PUBLIC_QUERY_BOX_QUALIFICATION","total":total,"exact":exact,"different":total-exact,"wrong_world_changes":wrong_changed,"first_difference":first,"rows":rows,"source_or_world_equivalence_claimed":false});std::fs::write(root.join(format!("geometry-{label}.json")),serde_json::to_vec(&result).unwrap()).unwrap();
 println!("Qualified actual636 ray requests against a public-query box: exact={exact} different={}",total-exact);assert_eq!(exact,total,"Actual same-world geometry query data contract");
}
