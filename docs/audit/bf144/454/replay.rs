use bevy::prelude::*;
use std::arch::aarch64::*;
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
fn multiply(a:Quat,b:Quat,source_order:bool)->Quat {
    if !source_order {return a*b;}
    let a=[-a.x,-a.z,-a.y,a.w];let b=[-b.x,-b.z,-b.y,b.w];
    let mut r=b.map(|x|a[3]*x);
    for (i,swizzle,sign) in [(0,[3,2,1,0],[1.0,-1.0,1.0,-1.0]),(1,[2,3,0,1],[1.0,1.0,-1.0,-1.0]),(2,[1,0,3,2],[-1.0,1.0,1.0,-1.0])] {
        for j in 0..4 {r[j]=(a[i]*b[swizzle[j]]).mul_add(sign[j],r[j]);}
    }
    Quat::from_xyzw(-r[0],-r[2],-r[1],r[3])
}
fn normalize(q:Quat,source_method:bool)->Quat {
    if !source_method {return q.normalize();}
    let dot=q.length_squared();assert!(dot>0.0&&dot.is_finite());
    let inv=unsafe {
        let d=vdupq_n_f32(dot);let mut x=vrsqrteq_f32(d);
        x=vmulq_f32(vrsqrtsq_f32(d,vmulq_f32(x,x)),x);
        x=vmulq_f32(vrsqrtsq_f32(d,vmulq_f32(x,x)),x);
        vgetq_lane_f32::<0>(x)
    };
    Quat::from_array(q.to_array().map(|v|v*inv))
}
fn staged(base:&Pose,delta:&Pose,s:&Skeleton,source_order:bool,source_method:bool)->std::collections::HashMap<BoneId,Quat> {
    let mut order=s.iter_bones().collect::<Vec<_>>();
    order.sort_by_key(|id|s.id_to_path(*id).unwrap().parts.len());
    let mut mesh=std::collections::HashMap::<BoneId,Quat>::new();
    for id in &order {
        let local=current_local(base,s,*id);
        let q=s.parent(id).and_then(|p|mesh.get(&p)).map(|p|multiply(*p,local,source_order)).unwrap_or(local);
        mesh.insert(*id,q);
    }
    for id in &order {
        if let Some(d)=delta.get_bone(*id).and_then(|b|b.rotation) {
            let d=normalize(d,source_method);
            let q=multiply(d,mesh[id],source_order);
            mesh.insert(*id,normalize(q,source_method));
        }
    }
    let mut local=std::collections::HashMap::new();
    for id in order.iter().rev() {
        let q=mesh[id];
        let q=s.parent(id).and_then(|p|mesh.get(&p)).map(|p|multiply(p.inverse(),q,source_order)).unwrap_or(q);
        let q=normalize(q,source_method);
        local.insert(*id,q);
    }
    local
}
fn main() {
    let r=std::path::Path::new("docs/audit/bf144/454");let label=std::env::args().nth(1).unwrap();
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
        for source_order in [false,true] {for source_method in [false,true] {
            let values=staged(&cb,&cd,&s,source_order,source_method);
            let mut controlled=public.clone();
            for (id,index) in &public.paths {
                let q=values[id];
                if !source_order&&!source_method {assert_eq!(Some(q),public.bones[*index].rotation,"Current rotation stages frame {} bone {:?}",frame["source_frame"],s.id_to_path(*id));exact+=1;}
                controlled.bones[*index].rotation=Some(q);
                assert_eq!(controlled.bones[*index].translation,public.bones[*index].translation);
                assert_eq!(controlled.bones[*index].scale,public.bones[*index].scale);
            }
            outputs.push(json!({"source_frame":frame["source_frame"],"target_query":frame["target_query"],"source_product_order":source_order,"source_reciprocal_method":source_method,"pose":snapshot(&controlled,&s)}));
        }}
        if frame["source_frame"].as_u64().unwrap()%64==0 {println!("Replayed exact current Head rotation stages at frame {}",frame["source_frame"]);}
    }
    assert_eq!(exact,318*80);assert_eq!(outputs.len(),318*4);
    let result=json!({"gate":"PASS_ALL318_CURRENT_HEAD_FLOAT_METHOD_BASELINE_AND_FOUR_CONTROLS","actual_public_frames_exact":318,"actual_rotation_bones_exact":exact,"outputs":outputs,"full_root_or_als_equivalence_claimed":false});
    std::fs::write(r.join(format!("replay-{label}.json")),serde_json::to_vec(&result).unwrap()).unwrap();
    println!("Replayed1272 complete Head F32 method factor outputs with25440 exact actual rotation rows");
}
