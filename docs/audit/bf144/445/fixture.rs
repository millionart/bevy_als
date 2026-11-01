include!("keys.rs");
fn main(){
    use bevy_animation_graph::core::{animation_graph::NodeId,context::{node_caches::NodeCaches,node_states::StateKey}};
    let mut cache=NodeCaches::default();
    let node=NodeId::default();
    let temporary=StateKey::Temporary(Default::default());
    assert!(c445_updated_keys(&cache).is_empty());
    cache.mark_update_started(node,StateKey::Default);
    assert!(c445_updated_keys(&cache).is_empty());
    cache.mark_updated(node,temporary);
    assert_eq!(c445_updated_keys(&cache),vec![(node,temporary)]);
    cache.mark_updated(node,StateKey::Default);
    assert_eq!(c445_updated_keys(&cache).len(),2);
    cache.next_frame();
    assert!(c445_updated_keys(&cache).is_empty());
    let result=serde_json::json!({"gate":"PASS_PUBLIC_REFLECTION_DEFAULT_TEMPORARY_AND_FRESH_FRAME_QUALIFICATION","rejected_absent_or_started_only":true,"temporary_and_default_covered":true,"stale_previous_frame_rejected":true});
    std::fs::write("docs/audit/bf144/445/fixture.json",serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    println!("{result}");
}
