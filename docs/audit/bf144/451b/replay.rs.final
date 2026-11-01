use bevy::prelude::*;
use bevy_animation_graph::core::{animation_clip::EntityPath,pose::{Pose,BonePose,BoneId},skeleton::Skeleton};
use serde_json::{Value,json};
fn f(v:&Value)->f32{v.as_f64().unwrap()as f32}
fn vec3(v:&Value)->Vec3{Vec3::new(f(&v[0]),f(&v[1]),f(&v[2]))}
fn quat(v:&Value)->Quat{Quat::from_xyzw(f(&v[0]),f(&v[1]),f(&v[2]),f(&v[3]))}
fn path(v:&Value)->EntityPath{EntityPath{parts:v.as_array().unwrap().iter().map(|v|Name::new(v.as_str().unwrap().to_owned())).collect()}}
fn transform(v:&Value)->Transform{Transform{translation:vec3(&v["translation"]),rotation:quat(&v["rotation"]),scale:vec3(&v["scale"])}}
fn bone(v:&Value)->BonePose{BonePose{translation:(!v["translation"].is_null()).then(||vec3(&v["translation"])),rotation:(!v["rotation"].is_null()).then(||quat(&v["rotation"])),scale:(!v["scale"].is_null()).then(||vec3(&v["scale"])),weights:None}}
fn pose(v:&Value)->Pose{assert!(v["root_motion"].is_null());let mut p=Pose{timestamp:f(&v["timestamp"]),..default()};for b in v["bones"].as_array().unwrap(){let id=path(&b["path"]).id();p.add_bone(bone(b),id);}p}
fn snapshot(p:&Pose,s:&Skeleton)->Value{let mut bones=p.paths.iter().map(|(id,index)|{let b=&p.bones[*index];json!({"name":s.id_to_path(*id).unwrap().parts.last().unwrap().as_str(),"translation":b.translation.map(|v|v.to_array()),"rotation":b.rotation.map(|v|v.to_array()),"scale":b.scale.map(|v|v.to_array()),"weights":b.weights})}).collect::<Vec<_>>();bones.sort_by_key(|b|b["name"].to_string());json!({"timestamp":p.timestamp,"bones":bones,"root_motion":p.root_motion})}
fn replace_source(p:&mut Pose,values:&Value,ids:&std::collections::HashMap<String,BoneId>){for b in values.as_array().unwrap(){let id=ids[b["name"].as_str().unwrap()];let index=*p.paths.get(&id).unwrap();p.bones[index]=bone(b);}}
fn assert_source(p:&Pose,values:&Value,ids:&std::collections::HashMap<String,BoneId>){for b in values.as_array().unwrap(){assert_eq!(p.get_bone(ids[b["name"].as_str().unwrap()]),Some(&bone(b)));}}
fn main(){
    let root=std::path::Path::new("docs/audit/bf144/451b");let label=std::env::args().nth(1).unwrap();
    let input:Value=serde_json::from_slice(&std::fs::read(root.join("replay-inputs.json")).unwrap()).unwrap();let data:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/451a/runtime-restored/skeleton.json").unwrap()).unwrap();
    let mut skeleton=Skeleton::default();skeleton.set_root(path(&data["root_path"]).id());let mut ids=std::collections::HashMap::new();
    for b in data["bones"].as_array().unwrap(){if b["local"].is_null(){assert!(b["path"].as_array().unwrap().is_empty());continue;}let p=path(&b["path"]);ids.insert(b["name"].as_str().unwrap().to_owned(),p.id());skeleton.add_bone(p,transform(&b["local"]),transform(&b["character"]));}
    assert_eq!(skeleton.iter_bones().count(),82);assert_eq!(input["frames"].as_array().unwrap().len(),318);
    let mut outputs=Vec::new();let mut cc_exact=0;
    for frame in input["frames"].as_array().unwrap(){
        let cb=pose(&frame["current_base"]);let cd=pose(&frame["current_additive"]);let expected=pose(&frame["current_output"]);
        for(mode,source_base,source_delta)in[("cc",false,false),("sc",true,false),("cs",false,true),("ss",true,true)]{
            let mut b=cb.clone();let mut d=cd.clone();if source_base{replace_source(&mut b,&frame["source_base"],&ids);assert_source(&b,&frame["source_base"],&ids);}if source_delta{replace_source(&mut d,&frame["source_additive"],&ids);assert_source(&d,&frame["source_additive"],&ids);}
            bevy_als::extensions::mesh_space_additive::apply_mesh_space_additive(&mut b,&d,1.0,&skeleton);
            if mode=="cc"{assert_eq!(snapshot(&b,&skeleton),snapshot(&expected,&skeleton),"Actual full Head replay frame {}",frame["source_frame"]);cc_exact+=1;}
            outputs.push(json!({"source_frame":frame["source_frame"],"target_query":frame["target_query"],"domain":mode,"pose":snapshot(&b,&skeleton)}));
        }
    }
    assert_eq!(cc_exact,318);assert_eq!(outputs.len(),1272);
    let result=json!({"gate":"PASS_FULL_TARGET_HEAD_FOUR_INPUT_DOMAINS_WITH_ACTUAL_CC_ENDPOINT","actual_cc_frames_exact":cc_exact,"domain_outputs":outputs,"full_root_equivalence_claimed":false});
    std::fs::write(root.join(format!("target-replay-{label}.json")),serde_json::to_vec(&result).unwrap()).unwrap();println!("Replayed318 actual full Head outputs and1272 four-domain public compositions using the captured actual skeleton");
}
