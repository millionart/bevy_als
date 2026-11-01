fn c458_actual_tag_blend_state(
    context:&bevy_animation_graph::core::context::graph_context::GraphState,
    node:bevy_animation_graph::core::animation_graph::NodeId,
)->serde_json::Value {
    use bevy::reflect::{PartialReflect,ReflectRef};
    use bevy_animation_graph::core::context::{node_state_box::NodeStateBox,node_states::StateKey};
    type States=bevy::platform::collections::HashMap<StateKey,std::sync::Arc<NodeStateBox>>;
    let ReflectRef::Struct(root)=context.node_states.reflect_ref()else{panic!("Actual node-state store must be reflected");};
    let ReflectRef::Map(map)=root.field("states").unwrap().reflect_ref()else{panic!("Actual node-state entries must be a map");};
    let ReflectRef::Struct(state)=map.get(&node).unwrap().reflect_ref()else{panic!("Actual node state is required");};
    let upcoming=state.field("upcoming_state").unwrap().try_downcast_ref::<States>().unwrap();
    let last=state.field("last_state").unwrap().try_downcast_ref::<Option<std::sync::Arc<NodeStateBox>>>().unwrap();
    let (value,storage)=upcoming.get(&StateKey::Default).map(|v|(v,"upcoming_Default")).or_else(||last.as_ref().map(|v|(v,"last_state_fallback"))).expect("Actual evaluated blend state is required");
    let ReflectRef::Struct(value)=value.value.reflect_ref()else{panic!("Actual source blend state must be a struct");};
    let ReflectRef::Struct(blend)=value.field("blend").unwrap().reflect_ref()else{panic!("Actual gameplay-tag blend state must be a struct");};
    let weights=blend.field("weights").unwrap().try_downcast_ref::<Vec<f32>>().unwrap();
    let active=*blend.field("last_active_index").unwrap().try_downcast_ref::<i32>().unwrap();
    let initialized=*blend.field("initialized").unwrap().try_downcast_ref::<bool>().unwrap();
    serde_json::json!({"weights":weights,"last_active_index":active,"initialized":initialized,"storage":storage})
}
fn c458_ragdoll_blend(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs:&Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    skeletons:&Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    assets:&AssetServer,
)->Vec<serde_json::Value> {
    use bevy_animation_graph::core::context::node_states::StateKey;
    let mut rows=Vec::new();
    if let Some(arena)=player.get_context_arena(){for id in arena.iter_context_ids(){
        let context=arena.get_context(id).unwrap();let Some(graph)=graphs.get(context.get_graph_id())else{continue;};
        let Some(node)=graph.nodes.values().find(|n|n.id.uuid().to_string()=="a88d5696-4292-b2ae-ca2e-ac8d13e7dd2f")else{continue;};
        if !context.node_caches.is_updated(node.id,StateKey::Default){continue;}
        let state=c458_actual_tag_blend_state(context,node.id);assert_eq!(state["weights"],serde_json::json!([1.0,0.0]));assert_eq!(state["last_active_index"],0);
        let read=|guid:&str|{let node=graph.nodes.values().find(|n|n.id.uuid().to_string()==guid).unwrap();context.node_caches.get_output_data(node.id,StateKey::Default,"Pose".into()).unwrap().into_pose().unwrap()};
        let base=read("277fbd6d-4f8a-8a3e-1d35-f3bf5df88eb0");let output=read("a88d5696-4292-b2ae-ca2e-ac8d13e7dd2f");let root=read("6896e565-4a03-ce0c-11d9-2f87644d55e7");
        let public=bevy_als::extensions::source_pose_nodes::blend_source_poses(&[(base.clone(),1.0)]);
        assert_eq!(output,public);assert_eq!(root,output);
        let inner=graph.nodes.values().find(|n|n.id.uuid().to_string()=="9d8e1dd1-42cf-35df-421f-b0aff16f5e2b").unwrap();
        rows.push(serde_json::json!({"node_guid":node.id,"graph":assets.get_path(context.get_graph_id()).map(|p|p.to_string()),"state_key":"Default","actual_state":state,"base":c458_pose_value(&base,skeletons,assets),"output":c458_pose_value(&output,skeletons,assets),"public":c458_pose_value(&public,skeletons,assets),"graph_root":c458_pose_value(&root,skeletons,assets),"copy_control":c458_pose_value(&base,skeletons,assets),"copy_control_applied_to_runtime":false,"inner_node_updated":context.node_caches.is_updated(inner.id,StateKey::Default)}));
    }}
    rows
}
#[derive(Reflect,Clone,Debug)]
struct C458ReaderBlend { initialized:bool,last_active_index:i32,weights:Vec<f32> }
#[derive(Reflect,Clone,Debug)]
struct C458ReaderState { blend:C458ReaderBlend }
fn c458_state_reader_self_test()->serde_json::Value {
    use bevy_animation_graph::core::{context::{graph_context::GraphState,node_states::StateKey},animation_graph::{AnimationGraph,NodeId}};
    let mut context=GraphState::new(AssetId::<AnimationGraph>::default());let node=NodeId::default();
    context.node_states.get_mut_or_insert_with(node,StateKey::Default,||C458ReaderState{blend:C458ReaderBlend{initialized:true,last_active_index:1,weights:vec![0.25,0.75]}}).unwrap();
    let upcoming=c458_actual_tag_blend_state(&context,node);assert_eq!(upcoming["weights"],serde_json::json!([0.25,0.75]));assert_eq!(upcoming["last_active_index"],1);assert_eq!(upcoming["storage"],"upcoming_Default");
    context.node_states.next_frame();let previous=c458_actual_tag_blend_state(&context,node);assert_eq!(previous["weights"],upcoming["weights"]);assert_eq!(previous["storage"],"last_state_fallback");
    serde_json::json!({"upcoming":upcoming,"previous":previous,"isolated_fixture_not_runtime_state":true})
}
