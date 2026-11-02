use bevy::math::DQuat;
use bevy_als::extensions::control_rig_hierarchy::{RigTransform,rig_quat_near};
use serde_json::{Value,json};
mod math {
    include!("../479/replay.rs");
    pub fn normalized(q:[f64;4])->[f64;4] {normalize(q,8)}
    pub fn multiply(a:[f64;4],b:[f64;4])->[f64;4] {product(a,b,16)}
    pub fn rotate_source(q:[f64;4],v:bevy::math::DVec3)->bevy::math::DVec3 {rotate(q,v,4)}
}
#[derive(Clone,Debug,PartialEq)]
struct Node {local:RigTransform,global:RigTransform,dirty:u8}
fn transform(v:&Value)->RigTransform {serde_json::from_value(v.clone()).unwrap()}
fn node(v:&Value)->Node {Node{local:transform(&v["local"]),global:transform(&v["global_cache"]),dirty:match v["dirty"].as_str(){None=>0,Some("Local")=>1,Some("Global")=>2,_=>panic!("Invalid raw cache dirty state")}}}
fn serialize(nodes:&[Node])->Value {json!(nodes.iter().map(|n|json!({"local":n.local,"global_cache":n.global,"dirty":match n.dirty{0=>None,1=>Some("Local"),2=>Some("Global"),_=>unreachable!()}})).collect::<Vec<_>>())}
struct Tree {nodes:Vec<Node>,parents:Vec<Option<usize>>,source:bool,tolerance:f64,writes:usize,global_evaluations:usize,marks:usize}
impl Tree {
    fn global(&self,local:RigTransform,parent:RigTransform)->RigTransform {
        if !self.source {return local.global(parent);}
        assert!(local.scale.min_element()>1e-8&&parent.scale.min_element()>1e-8);
        RigTransform{rotation:DQuat::from_array(math::normalized(math::multiply(parent.rotation.to_array(),local.rotation.to_array()))),translation:math::rotate_source(parent.rotation.to_array(),parent.scale*local.translation)+parent.translation,scale:local.scale*parent.scale}
    }
    fn get(&mut self,index:usize,space:u8)->RigTransform {
        if space==1 {
            assert_ne!(self.nodes[index].dirty,1,"Actual IK propagation reads clean local caches");
            return self.nodes[index].local;
        }
        if self.nodes[index].dirty==space {
            let parent=self.parents[index].map(|p|self.get(p,2)).unwrap_or_default();
            self.nodes[index].global=self.global(self.nodes[index].local,parent);
            self.global_evaluations+=1;
            self.nodes[index].dirty=0;
        }
        self.nodes[index].global
    }
    fn children(&self,index:usize)->Vec<usize> {self.parents.iter().enumerate().filter_map(|(i,p)|(*p==Some(index)).then_some(i)).collect()}
    fn propagate(&mut self,index:usize) {
        for child in self.children(index) {
            if self.nodes[child].dirty==2 {continue;}
            self.get(child,1);
            self.propagate(child);
            self.nodes[child].dirty=2;
            self.marks+=1;
        }
    }
    fn set(&mut self,index:usize,value:RigTransform) {
        let before=self.nodes[index].global;
        let same=(before.translation-value.translation).abs().max_element()<=self.tolerance
            &&(before.scale-value.scale).abs().max_element()<=1e-4_f32 as f64
            &&rig_quat_near(before.rotation,value.rotation,1e-4_f32 as f64);
        assert!(self.nodes[index].dirty==2||!same,"Every active actual IK writer is dirty or differs from the stored global");
        self.get(index,2);
        self.propagate(index);
        self.nodes[index].global=value;
        self.nodes[index].dirty=1;
        self.writes+=1;
    }
}
fn main() {
    let root=std::path::Path::new("docs/audit/bf144/481");
    let label=std::env::args().nth(1).unwrap();
    let input:Value=serde_json::from_slice(&std::fs::read(root.join("inputs.json")).unwrap()).unwrap();
    let parents=input["parents"].as_array().unwrap().iter().map(|v|v.as_u64().map(|v|v as usize)).collect::<Vec<_>>();
    let mut outputs=Vec::new();
    let mut source_exact=0;
    let mut current_exact=0;
    let mut twists_exact=0;
    let mut first=None;
    for(source,rows)in [(true,input["source"].as_array().unwrap()),(false,input["current"].as_array().unwrap())] {
        for row in rows {
            let mut tree=Tree{nodes:row["before"].as_array().unwrap().iter().map(node).collect(),parents:parents.clone(),source,tolerance:if source{1e-4_f32 as f64}else{input["current_translation_tolerance"].as_f64().unwrap()},writes:0,global_evaluations:0,marks:0};
            assert_eq!(tree.nodes.len(),79);
            if row["weight"].as_f64().unwrap()>1e-8 {
                for i in 0..3 {tree.set(row["indices"][i].as_u64().unwrap()as usize,transform(&row["requested"][i]));}
            }
            let expected=row["expected"].as_array().unwrap().iter().map(node).collect::<Vec<_>>();
            let exact=tree.nodes==expected;
            if exact {if source{source_exact+=1;}else{current_exact+=1;}}else if first.is_none(){first=Some(json!({"source":source,"row":row,"after":serialize(&tree.nodes)}));}
            let after=serialize(&tree.nodes);
            let stats=json!({"writes":tree.writes,"global_evaluations":tree.global_evaluations,"marks":tree.marks});
            let mut twists=Vec::new();
            if source {
                for item in row["twists"].as_array().unwrap() {
                    let index=item["index"].as_u64().unwrap()as usize;
                    let value=tree.get(index,2);
                    let expected=transform(&item["global_transform"]);
                    let same=value==expected;
                    twists_exact+=same as usize;
                    twists.push(json!({"index":index,"result":value,"native":expected,"exact":same}));
                }
            }
            outputs.push(json!({"source":source,"kind":row["kind"],"dataset":row["dataset"],"case":row["case"],"mode":row["mode"],"query":row["query"],"pc":row["pc"],"row":row["row"],"weight":row["weight"],"after":after,"stats":stats,"twists":twists,"exact":exact}));
        }
    }
    std::fs::write(root.join(format!("replay-unit-{label}.json")),serde_json::to_vec(&json!({"gate":"COMPLETE_IK_SOURCE_CURRENT_RAW_HIERARCHY_WRITER_CACHE_AND_TWIST_CONSUMERS","source_exact":source_exact,"current_exact":current_exact,"twists_exact":twists_exact,"first_difference":first,"outputs":outputs})).unwrap()).unwrap();
    assert_eq!(source_exact,1332,"All79 raw source cache values and dirty flags");
    assert_eq!(current_exact,input["current"].as_array().unwrap().len(),"All79 actual/control public cache values and dirty flags");
    assert_eq!(twists_exact,2664,"Untouched twist consumers evaluated from exact IK post cache");
    println!("Qualified1332 source and1272 actual current complete raw cache transitions plus2664 native twist consumers");
}
