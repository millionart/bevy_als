use std::arch::aarch64::*;
use serde_json::{Value,json};
type Q=[f64;4];
fn q(v:&Value)->Q {std::array::from_fn(|i|v[i].as_f64().unwrap())}
fn product(a:Q,b:Q)->Q {
    (bevy::math::DQuat::from_array(a)*bevy::math::DQuat::from_array(b)).to_array()
}

fn normalize(v:Q,standard_sqrt:bool)->Q {
    let dot=v[2].mul_add(v[2],v[0]*v[0])+v[3].mul_add(v[3],v[1]*v[1]);
    if dot<1e-8 {return [0.0,0.0,0.0,1.0];}
    let inv=if standard_sqrt {1.0/dot.sqrt()}else{unsafe {
        let d=vdupq_n_f64(dot);let mut x=vrsqrteq_f64(d);
        x=vmulq_f64(vrsqrtsq_f64(d,vmulq_f64(x,x)),x);
        x=vmulq_f64(vrsqrtsq_f64(d,vmulq_f64(x,x)),x);
        vgetq_lane_f64::<0>(x)
    }};
    v.map(|x|x*inv)
}
fn rounded(v:Q,round:bool)->Q {if round {v.map(|x|x as f32 as f64)}else{v}}
fn evaluate(base:&[Q],delta:&[Q],parents:&[i64],mode:&str)->Vec<Q> {
    let std_sqrt=mode=="standard-sqrt";let round=mode=="project-primitives";
    let mut mesh=base.to_vec();
    for i in 1..mesh.len() {mesh[i]=rounded(product(mesh[parents[i]as usize],mesh[i]),round);}
    for i in 0..mesh.len() {
        let d=if mode=="normalize-delta"{normalize(delta[i],false)}else{delta[i]};
        mesh[i]=rounded(product(d,mesh[i]),round);
        if mode=="normalize-product" {mesh[i]=normalize(mesh[i],false);}
    }
    for i in (1..mesh.len()).rev() {
        let p=mesh[parents[i]as usize];
        mesh[i]=rounded(product([-p[0],-p[1],-p[2],p[3]],mesh[i]),round);
    }
    for v in &mut mesh {*v=rounded(normalize(*v,std_sqrt),round);}
    if mode!="one-local-normalize" {
        for v in &mut mesh {*v=rounded(normalize(*v,std_sqrt),round);}
    }
    mesh
}
fn main() {
    let root=std::path::Path::new("docs/audit/bf144/452b");let label=std::env::args().nth(1).unwrap();
    let input:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();
    let parents=input["parents"].as_array().unwrap().iter().map(|x|x.as_i64().unwrap()).collect::<Vec<_>>();
    let cases=input["cases"].as_array().unwrap();assert_eq!(cases.len(),1272);assert_eq!(parents.len(),79);
    let mut outputs=Vec::new();let mut exact=0;
    for (i,case) in cases.iter().enumerate() {
        let base=case["base"].as_array().unwrap().iter().map(q).collect::<Vec<_>>();
        let delta=case["additive"].as_array().unwrap().iter().map(q).collect::<Vec<_>>();
        let expected=case["expected"].as_array().unwrap().iter().map(q).collect::<Vec<_>>();
        assert_eq!(base.len(),79);assert_eq!(delta.len(),79);assert_eq!(expected.len(),79);
        for mode in ["source","normalize-delta","normalize-product","one-local-normalize","standard-sqrt","project-primitives"] {
            let actual=evaluate(&base,&delta,&parents,mode);
            if mode=="source" {assert_eq!(actual,expected,"Source F64/ARM64 Head exact native case {}",case["name"]);exact+=actual.len();}
            outputs.push(json!({"name":case["name"],"mode":mode,"rotations":actual}));
        }
        if i%240==0 {println!("Calibrated exact native Head arithmetic at case {}",i);}
    }
    assert_eq!(exact,100488);assert_eq!(outputs.len(),7632);
    let result=json!({"gate":"PASS_ALL1272_SOURCE_F64_ARM64_HEAD_ROTATIONS_EXACT_NATIVE","native_rotation_rows_exact":exact,"outputs":outputs,"full_root_or_als_equivalence_claimed":false});
    std::fs::write(root.join(format!("replay-{label}.json")),serde_json::to_vec(&result).unwrap()).unwrap();
    println!("Calibrated all100488 source rotation rows and7632 source arithmetic factor outputs");
}
