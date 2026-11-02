use bevy_als::extensions::{control_rig_hierarchy::{RigHierarchy,RigTransform,RigSpace},control_rig_math::RigValue,control_rig_program::RigDefinition,control_rig_units::RigUnitContext,SourceCurveValues};
use serde_json::{Value,json};

fn main() {
    let root=std::path::Path::new("docs/audit/bf144/477");
    let label=std::env::args().nth(1).unwrap();
    let data:Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/476/inputs.json").unwrap()).unwrap();
    let inputs:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();
    let definition:RigDefinition=serde_json::from_value(data["definition"].clone()).unwrap();
    let curves=SourceCurveValues::default();
    let mut results=Vec::new();
    let mut exact=[0usize;4];
    let mut length_changes=[0usize;4];
    let mut scalar_changes=[0usize;4];
    for entry in inputs["current"].as_array().unwrap() {
        let row=&data["current"][entry["row"].as_u64().unwrap()as usize];
        for domain in 0..4 {
            let mut hierarchy=RigHierarchy::new(definition.bones.clone(),0.0).unwrap();
            for bone in row["bones_before"].as_array().unwrap() {
                let index=bone[0].as_u64().unwrap()as usize;
                let transform:RigTransform=serde_json::from_value(bone[1].clone()).unwrap();
                hierarchy.set_transform(index,RigSpace::Global,false,transform,true).unwrap();
                assert_eq!(hierarchy.transform(index,RigSpace::Global,false).unwrap(),transform);
            }
            let mut args:Vec<RigValue>=serde_json::from_value(row["arguments_before"].clone()).unwrap();
            let indices=args[..3].iter().map(|v|hierarchy.item_index(v).unwrap().unwrap()).collect::<Vec<_>>();
            let initial=indices.iter().map(|&i|hierarchy.transform(i,RigSpace::Global,true).unwrap()).collect::<Vec<_>>();
            let scale_a=hierarchy.transform(indices[0],RigSpace::Global,false).unwrap().scale;
            let computed:[f32;2]=std::array::from_fn(|i| {
                assert!(initial[i].scale.length_squared()>1e-8_f32 as f64);
                let scale=scale_a/initial[i].scale;
                ((initial[i].translation-initial[i+1].translation)*scale).length()as f32
            });
            let native_m:[f32;2]=std::array::from_fn(|i|entry["lengths_m"][i].as_f64().unwrap()as f32);
            assert_eq!(computed,native_m,"Same current metre inputs match independent native length cast");
            let selected:[f32;2]=std::array::from_fn(|i|match domain {
                0=>0.0,
                1=>native_m[i],
                2=>native_m[i],
                3=>(entry["distances_lifted_cm"][i].as_f64().unwrap()*0.01)as f32,
                _=>unreachable!(),
            });
            if domain!=0 {
                for i in 0..2 {args[14+i]=RigValue::F32(selected[i]);}
                let differences=selected.iter().zip(native_m).filter(|(a,b)|**a!=*b).count();
                scalar_changes[domain]+=differences;
                length_changes[domain]+=(differences!=0)as usize;
            }
            let expected_args=args.clone();
            let mut context=RigUnitContext{hierarchy:&mut hierarchy,curves:&curves,game_world:true,construction:false,delta_seconds:1.0/30.0,geometry:&definition.geometry,to_world:RigTransform::default(),line_trace:None};
            context.call("FRigUnit_TwoBoneIKSimplePerItem::Execute",&mut args).unwrap();
            assert_eq!(args,expected_args,"Explicit length echo is accepted before checking pose");
            let after=indices.iter().map(|&i|(i,hierarchy.transform(i,RigSpace::Global,false).unwrap())).collect::<Vec<_>>();
            for (&i,original)in indices.iter().zip(initial) {assert_eq!(hierarchy.transform(i,RigSpace::Global,true).unwrap(),original);}
            let same=json!(&after)==row["bones_after"];
            exact[domain]+=same as usize;
            results.push(json!({"mode":row["mode"],"query":row["query"],"pc":row["pc"],"domain":domain,"computed_lengths_m":computed,"selected_lengths_m":selected,"after":after,"exact":same}));
        }
    }
    std::fs::write(root.join(format!("replay-unit-{label}.json")),serde_json::to_vec(&json!({"gate":"ACTUAL_AUTO_LENGTH_PUBLIC_CONSUMPTION_AND_COMMON_UNIT_PHASE","exact":exact,"length_changed_calls":length_changes,"changed_scalars":scalar_changes,"results":results})).unwrap()).unwrap();
    assert_eq!(exact[0],1272,"Actual complete public IK preserved");
    assert_eq!(exact[1],1272,"Native independent same-input metre lengths preserve complete public IK");
    assert!(length_changes[2]>0&&exact[2]<1272,"Native centimetre float stage must have a real length and public pose counterexample");
    println!("Qualified1272 actual auto lengths and explicit native-metre consumers; unit controls exact={exact:?}, length changes={length_changes:?}");
}
