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
fn current_local(p:&Pose,s:&Skeleton,id:BoneId)->Quat {
    p.get_bone(id).and_then(|b|b.rotation).or_else(||s.default_transforms(id).map(|t|t.local.rotation)).unwrap_or(Quat::IDENTITY)
}
fn staged(base:&Pose,delta:&Pose,s:&Skeleton,delta_n:bool,product_n:bool,second_local_n:bool)->std::collections::HashMap<BoneId,Quat> {
    let mut order=s.iter_bones().collect::<Vec<_>>();
    order.sort_by_key(|id|s.id_to_path(*id).unwrap().parts.len());
    let mut mesh=std::collections::HashMap::<BoneId,Quat>::new();
    for id in &order {
        let local=current_local(base,s,*id);
        let q=s.parent(id).and_then(|p|mesh.get(&p)).map(|p|*p*local).unwrap_or(local);
        mesh.insert(*id,q);
    }
    for id in &order {
        if let Some(d)=delta.get_bone(*id).and_then(|b|b.rotation) {
            let d=if delta_n {d.normalize()} else {d};
            let q=d*mesh[id];
            mesh.insert(*id,if product_n {q.normalize()}else{q});
        }
    }
    let mut local=std::collections::HashMap::new();
    for id in order.iter().rev() {
        let q=mesh[id];
        let q=s.parent(id).and_then(|p|mesh.get(&p)).map(|p|p.inverse()*q).unwrap_or(q).normalize();
        local.insert(*id,if second_local_n {q.normalize()}else{q});
    }
    local
}
fn main() {
    let r=std::path::Path::new("docs/audit/bf144/452a");let label=std::env::args().nth(1).unwrap();
    let input:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/451b/replay-inputs.json").unwrap()).unwrap();
    let data:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/451a/runtime-restored/skeleton.json").unwrap()).unwrap();
    let mut s=Skeleton::default();s.set_root(path(&data["root_path"]).id());
    for b in data["bones"].as_array().unwrap() {
        if b["local"].is_null(){assert!(b["path"].as_array().unwrap().is_empty());continue;}
        s.add_bone(path(&b["path"]),transform(&b["local"]),transform(&b["character"]));
    }
    assert_eq!(s.iter_bones().count(),82);assert_eq!(input["frames"].as_array().unwrap().len(),318);
    let mut outputs=Vec::new();let mut exact=0;
    for frame in input["frames"].as_array().unwrap() {
        let cb=pose(&frame["current_base"]);let cd=pose(&frame["current_additive"]);let expected=pose(&frame["current_output"]);
        let mut public=cb.clone();bevy_als::extensions::mesh_space_additive::apply_mesh_space_additive(&mut public,&cd,1.0,&s);
        assert_eq!(snapshot(&public,&s),snapshot(&expected,&s));
        for delta_n in [false,true] {for product_n in [false,true] {for second_n in [false,true] {
            let values=staged(&cb,&cd,&s,delta_n,product_n,second_n);
            let mut controlled=public.clone();
            for (id,index) in &public.paths {
                let q=values[id];
                if delta_n&&product_n&&!second_n {assert_eq!(Some(q),public.bones[*index].rotation,"Current rotation stages frame {} bone {:?}",frame["source_frame"],s.id_to_path(*id));exact+=1;}
                controlled.bones[*index].rotation=Some(q);
                assert_eq!(controlled.bones[*index].translation,public.bones[*index].translation);
                assert_eq!(controlled.bones[*index].scale,public.bones[*index].scale);
            }
            outputs.push(json!({"source_frame":frame["source_frame"],"target_query":frame["target_query"],"delta_normalize":delta_n,"product_normalize":product_n,"second_local_normalize":second_n,"pose":snapshot(&controlled,&s)}));
        }}}
        if frame["source_frame"].as_u64().unwrap()%64==0 {println!("Replayed exact current Head rotation stages at frame {}",frame["source_frame"]);}
    }
    assert_eq!(exact,318*80);assert_eq!(outputs.len(),318*8);
    let result=json!({"gate":"PASS_ALL318_CURRENT_HEAD_ROTATION_STAGE_CALIBRATION_AND_EIGHT_CONTROLS","actual_public_frames_exact":318,"actual_rotation_bones_exact":exact,"outputs":outputs,"full_root_or_als_equivalence_claimed":false});
    std::fs::write(r.join(format!("replay-{label}.json")),serde_json::to_vec(&result).unwrap()).unwrap();
    println!("Replayed2544 complete Head rotation factor outputs with25440 exact actual rotation rows");
}
