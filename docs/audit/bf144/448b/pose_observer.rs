fn c448b_pose_value(
    pose:&bevy_animation_graph::core::pose::Pose,
    skeletons:&Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    assets:&AssetServer,
)->serde_json::Value{
    let skeleton=skeletons.get(&pose.skeleton);
    let mut bones=pose.paths.iter().map(|(id,index)|{
        let path=skeleton.and_then(|s|s.id_to_path(*id)).map(|p|p.parts.iter().map(|v|v.as_str().to_owned()).collect::<Vec<_>>());
        let bone=&pose.bones[*index];
        serde_json::json!({"name":path.as_ref().and_then(|p|p.last()),"path":path,"translation":bone.translation.map(|v|v.to_array()),"rotation":bone.rotation.map(|v|v.to_array()),"scale":bone.scale.map(|v|v.to_array()),"weights":bone.weights})
    }).collect::<Vec<_>>();
    bones.sort_by_key(|v|v["name"].to_string());
    serde_json::json!({"timestamp":pose.timestamp,"skeleton_asset":assets.get_path(pose.skeleton.id()).map(|p|p.to_string()),"bones":bones,"root_motion":pose.root_motion})
}
fn c448b_pose_boundaries(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs:&Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    skeletons:&Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    assets:&AssetServer,
)->serde_json::Value{
    let declarations=[("root_result","8191c2ea-49e5-bd21-ee33-86bd87b2f93c","Pose"),("rig_source","e4f33e44-4019-9bac-8e0a-dcbc0e3ae178","Pose"),("head_base","316d5caa-4268-b60f-5ce6-2e9c0beb480b","Pose"),("head_additive","5a7796c6-47c9-baf2-6479-579e079fbb67","Pose"),("base_provider_input","ccaab6a2-424d-b00d-6794-f3a705d6f8c2","Pose")];
    let mut boundaries=Vec::new();
    if let Some(arena)=player.get_context_arena(){
        for context_id in arena.iter_context_ids(){
            let context=arena.get_context(context_id).unwrap();
            let Some(graph)=graphs.get(context.get_graph_id())else{continue;};
            for node in graph.nodes.values(){
                let Some((name,_,pin))=declarations.iter().find(|(_,guid,_)|node.id.uuid().to_string()==*guid)else{continue;};
                for key in c448b_updated_keys(&context.node_caches).into_iter().filter_map(|(id,key)|(id==node.id).then_some(key)){
                    let Some(value)=context.node_caches.get_output_data(node.id,key,(*pin).into())else{continue;};
                    let Ok(pose)=value.as_pose()else{continue;};
                    let curves=bevy_als::extensions::animation_context::node_context_state(&context.node_states,key,node.id).map(|s|s.curves.iter().map(|(n,v)|(n.as_str().to_owned(),*v)).collect::<std::collections::BTreeMap<_,_>>());
                    boundaries.push(serde_json::json!({"boundary":name,"node_guid":node.id,"pin":pin,"context":format!("{context_id:?}"),"state_key":format!("{key:?}"),"graph":assets.get_path(context.get_graph_id()).map(|p|p.to_string()),"pose":c448b_pose_value(&pose,skeletons,assets),"curves":curves}));
                }
            }
        }
    }
    boundaries.sort_by_key(|v|(v["boundary"].to_string(),v["context"].to_string(),v["state_key"].to_string()));
    serde_json::json!({"default_output":player.get_default_output_pose().map(|p|c448b_pose_value(p,skeletons,assets)),"updated_boundaries":boundaries,"custom_attribute_storage":"Absent from the current BAG Pose type"})
}
