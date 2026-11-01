fn c450a_difference(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs:&Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    skeletons:&Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    assets:&AssetServer,
)->Vec<serde_json::Value>{
    use bevy_animation_graph::core::animation_graph::{SourcePin,TargetPin};
    let mut rows=Vec::new();
    if let Some(arena)=player.get_context_arena(){for context_id in arena.iter_context_ids(){
        let context=arena.get_context(context_id).unwrap();let Some(graph)=graphs.get(context.get_graph_id())else{continue;};
        for node in graph.nodes.values(){
            if node.id.uuid().to_string()!="73bc3f9e-083f-5fb4-a4a1-590ad157f515"{continue;}
            for(_,key)in c450a_updated_keys(&context.node_caches).into_iter().filter(|(id,_)|*id==node.id){
                let output=context.node_caches.get_output_data(node.id,key,"pose".into()).unwrap().into_pose().unwrap();
                let mut inputs=Vec::new();let mut poses=Vec::new();
                for pin in ["reference_pose","target_pose"]{
                    let source=graph.edges_inverted.get(&TargetPin::NodeData(node.id,pin.into())).unwrap();
                    let SourcePin::NodeData(id,name)=source else{panic!("A current difference input is not a node output");};
                    assert!(context.node_caches.is_updated(*id,key));
                    let pose=context.node_caches.get_output_data(*id,key,name.clone()).unwrap().into_pose().unwrap();
                    inputs.push(serde_json::json!({"pin":pin,"source_node":id,"source_pin":name,"pose":c450a_pose_value(&pose,skeletons,assets)}));poses.push(pose);
                }
                let skeleton=skeletons.get(&poses[0].skeleton).unwrap();
                let mut expected=bevy_als::extensions::mesh_space_difference::make_mesh_space_difference(&poses[0],&poses[1],skeleton);expected.timestamp=poses[1].timestamp;
                rows.push(serde_json::json!({"node_guid":node.id,"context":format!("{context_id:?}"),"state_key":format!("{key:?}"),"graph":assets.get_path(context.get_graph_id()).map(|p|p.to_string()),"inputs":inputs,"actual":c450a_pose_value(&output,skeletons,assets),"public":c450a_pose_value(&expected,skeletons,assets),"pose_exact_public":output==expected}));
            }
        }
    }}rows.sort_by_key(|v|(v["context"].to_string(),v["state_key"].to_string()));rows
}
