use serde_json::{Value,json};
type V=[f64;3];
fn v(x:&Value)->V {std::array::from_fn(|i|x[i].as_f64().unwrap())}
fn project(p:V)->V {[p[0]*0.01,p[2]*0.01,p[1]*0.01].map(|x|x as f32 as f64)}
fn direction(p:V)->V {[p[0],p[2],p[1]]}
fn narrow(p:V)->V {p.map(|x|x as f32 as f64)}
fn main() {
    let r=std::path::Path::new("docs/audit/bf144/453");let label=std::env::args().nth(1).unwrap();
    let input:Value=serde_json::from_slice(&std::fs::read(r.join("inputs.json")).unwrap()).unwrap();
    let cases=input["cases"].as_array().unwrap();assert_eq!(cases.len(),1272);
    let mut outputs=Vec::new();let mut native_exact=0;let mut current_exact=0;
    for (i,case) in cases.iter().enumerate() {
        let base=case["base"].as_array().unwrap();let delta=case["additive"].as_array().unwrap();
        let expected_native=case["expected_native"].as_array().unwrap();let expected_current=case["expected_current"].as_array().unwrap();
        assert_eq!(base.len(),79);assert_eq!(delta.len(),79);assert_eq!(expected_native.len(),79);assert_eq!(expected_current.len(),79);
        let mut modes=[Vec::new(),Vec::new(),Vec::new(),Vec::new()];
        for j in 0..79 {
            let b=v(&base[j]["translation_cm"]);let d=v(&delta[j]["translation_cm"]);let bs=v(&base[j]["scale"]);let ds=v(&delta[j]["scale"]);
            let translation:V=std::array::from_fn(|k|d[k].mul_add(1.0,b[k]));
            let scale:V=std::array::from_fn(|k|bs[k]*ds[k].mul_add(1.0,1.0));
            assert_eq!(translation,v(&expected_native[j]["translation_cm"]),"Native Head T {} bone {}",case["name"],j);
            assert_eq!(scale,v(&expected_native[j]["scale"]),"Native Head S {} bone {}",case["name"],j);native_exact+=1;
            modes[0].push(json!({"translation":project(translation),"scale":narrow(direction(scale))}));
            let br=direction(b).map(|x|x*0.01);let dr=direction(d).map(|x|x*0.01);
            let early:V=std::array::from_fn(|k|br[k]+dr[k]);
            modes[1].push(json!({"translation":narrow(early),"scale":narrow(direction(scale))}));
            let bf=project(b);let df=project(d);let bsf=narrow(direction(bs));let dsf=narrow(direction(ds));
            let children_t:V=std::array::from_fn(|k|bf[k]+df[k]);let children_s:V=std::array::from_fn(|k|bsf[k]*(1.0+dsf[k]));
            modes[2].push(json!({"translation":narrow(children_t),"scale":narrow(children_s)}));
            let current_t:V=std::array::from_fn(|k|(bf[k]as f32+df[k]as f32)as f64);
            let current_s:V=std::array::from_fn(|k|(bsf[k]as f32*(1.0f32+dsf[k]as f32))as f64);
            assert_eq!(current_t,v(&expected_current[j]["translation"]),"Current Head T {} bone {}",case["name"],j);
            assert_eq!(current_s,v(&expected_current[j]["scale"]),"Current Head S {} bone {}",case["name"],j);current_exact+=1;
            modes[3].push(json!({"translation":current_t,"scale":current_s}));
        }
        for (mode,bones)in ["source","early-unit","children-rounded","current"].into_iter().zip(modes) {
            outputs.push(json!({"name":case["name"],"mode":mode,"bones":bones}));
        }
        if i%240==0 {println!("Calibrated native/current Head T/S at case {}",i);}
    }
    assert_eq!(native_exact,100488);assert_eq!(current_exact,100488);assert_eq!(outputs.len(),5088);
    std::fs::write(r.join(format!("replay-{label}.json")),serde_json::to_vec(&json!({"gate":"PASS_FULL_NATIVE_AND_CURRENT_HEAD_TRANSLATION_SCALE_ENDPOINTS","native_rows_exact":native_exact,"current_rows_exact":current_exact,"outputs":outputs,"full_root_or_als_equivalence_claimed":false})).unwrap()).unwrap();
    println!("Replayed100488 native/current T/S rows and5088 precision-stage outputs");
}
