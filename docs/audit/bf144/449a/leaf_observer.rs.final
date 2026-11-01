fn c449a_idle_leaf(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs:&Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    clips:&Assets<bevy_animation_graph::core::animation_clip::GraphClip>,
    skeletons:&Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    assets:&AssetServer,
)->Vec<serde_json::Value>{
    let mut rows=Vec::new();
    if let Some(arena)=player.get_context_arena(){
        for context_id in arena.iter_context_ids(){
            let context=arena.get_context(context_id).unwrap();
            let Some(graph)=graphs.get(context.get_graph_id())else{continue;};
            for node in graph.nodes.values(){
                if node.id.uuid().to_string()!="a189ffa4-46d2-a4ea-01e7-728ccfb6d729"{continue;}
                let evaluator=node.try_inner_downcast_ref::<bevy_als::extensions::SourceSequenceEvaluatorNode>().unwrap();
                let Some(clip)=clips.get(&evaluator.clip)else{continue;};
                for(_,key)in c449a_updated_keys(&context.node_caches).into_iter().filter(|(id,_)|*id==node.id){
                    let Some(value)=context.node_caches.get_output_data(node.id,key,"Pose".into())else{continue;};
                    let Ok(actual)=value.as_pose()else{continue;};
                    let sampled=bevy_als::extensions::source_sequence_player::sample_pose(clip,actual.timestamp);
                    rows.push(serde_json::json!({"node_guid":node.id,"context":format!("{context_id:?}"),"state_key":format!("{key:?}"),"graph":assets.get_path(context.get_graph_id()).map(|p|p.to_string()),"clip":assets.get_path(evaluator.clip.id()).map(|p|p.to_string()),"root_lock":format!("{:?}",evaluator.root_lock),"additive_reference":evaluator.additive_reference.as_ref().map(|v|assets.get_path(v.id()).map(|p|p.to_string())),"pose_exact_public_sample":*actual==sampled,"actual":c449a_pose_value(&actual,skeletons,assets),"sampled":c449a_pose_value(&sampled,skeletons,assets)}));
                }
            }
        }
    }
    rows.sort_by_key(|v|(v["context"].to_string(),v["state_key"].to_string()));rows
}
