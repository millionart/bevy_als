fn c445_leaf_rows(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs:&Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    assets:&AssetServer,
)->Vec<serde_json::Value>{
    use bevy_animation_graph::core::context::node_states::StateKey;
    let Some(arena)=player.get_context_arena()else{return Vec::new();};
    let mut rows=Vec::new();
    for context_id in arena.iter_context_ids(){
        let context=arena.get_context(context_id).unwrap();
        let Some(graph)=graphs.get(context.get_graph_id())else{continue;};
        let updated=c445_updated_keys(&context.node_caches);
        for node in graph.nodes.values(){
            let sequence=node.try_inner_downcast_ref::<bevy_als::extensions::SourceSequencePlayerNode>();
            let evaluator=node.try_inner_downcast_ref::<bevy_als::extensions::SourceSequenceEvaluatorNode>();
            let blend=node.try_inner_downcast_ref::<bevy_als::extensions::NormalizedBlendSpaceNode>();
            let builtin=node.try_inner_downcast_ref::<bevy_animation_graph::builtin_nodes::clip_node::ClipNode>();
            let kind=if sequence.is_some(){"sequence_player"}else if evaluator.is_some(){"sequence_evaluator"}else if blend.is_some(){"blendspace"}else if builtin.is_some(){"clip"}else{continue;};
            let builtin_clip=builtin.and_then(|v|{
                use bevy::reflect::PartialReflect;
                let bevy::reflect::ReflectRef::Struct(fields)=v.reflect_ref()else{return None;};
                fields.field("clip").and_then(|v|v.try_downcast_ref::<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>()).cloned()
            });
            let clip=sequence.map(|v|&v.clip).or_else(||evaluator.map(|v|&v.clip)).or(builtin_clip.as_ref());
            let mut clocks=Vec::new();
            for (_,key) in updated.iter().filter(|(id,_)|*id==node.id){
                let state=bevy_als::extensions::animation_context::node_context_state(&context.node_states,*key,node.id);
                let relevant=state.and_then(|v|v.relevant_asset.as_ref()).map(|v|serde_json::json!({"position_seconds":v.position,"previous_seconds":v.previous_position,"duration_seconds":v.duration,"delta_seconds":v.delta_seconds,"playback_delta_seconds":v.playback_delta_seconds,"looping":v.looping}));
                let sync=state.map(|v|v.sync_players.iter().map(|t|serde_json::json!({"phase":t.phase,"previous_phase":t.previous_phase,"phase_duration":t.phase_duration,"duration_seconds":t.duration,"group":t.key,"weight":t.weight,"delta_seconds":t.delta_seconds,"play_rate":t.play_rate})).collect::<Vec<_>>()).unwrap_or_default();
                let samples=state.map(|v|v.asset_samples.iter().map(|s|serde_json::json!({"clip":assets.get_path(s.clip.id()).map(|p|p.to_string()),"position_seconds":s.clock.position,"duration_seconds":s.clock.duration,"weight":s.clock.weight})).collect::<Vec<_>>()).unwrap_or_default();
                let pose=context.node_caches.get_output_data(node.id,*key,if blend.is_some()||builtin.is_some(){"pose"}else{"Pose"}.into()).and_then(|v|v.as_pose().ok().map(|p|p.timestamp));
                let point_times=blend.map(|v|v.points.iter().map(|p|serde_json::json!({"id":p.id,"requested_time":context.node_caches.get_input_time_update(node.id,*key,bevy_als::extensions::NormalizedBlendSpaceNode::time_pin(&p.id)).ok().map(|t|format!("{t:?}"))})).collect::<Vec<_>>());
                clocks.push(serde_json::json!({"key":format!("{key:?}"),"temporary":matches!(key,StateKey::Temporary(_)),"node_time":context.node_states.get_time(node.id,*key),"sample_time":context.node_caches.get_output_data(node.id,*key,"sample_time".into()).and_then(|v|v.as_f32().ok()),"pose_timestamp":pose,"relevant":relevant,"sync":sync,"samples":samples,"point_times":point_times}));
            }
            rows.push(serde_json::json!({"context":format!("{context_id:?}"),"graph":assets.get_path(context.get_graph_id()).map(|p|p.to_string()),"node_guid":node.id,"name":node.name,"kind":kind,"clip":clip.and_then(|v|assets.get_path(v.id())).map(|p|p.to_string()),"teleport":blend.map(|v|v.teleport_to_normalized_time),"points":blend.map(|v|v.points.iter().map(|p|serde_json::json!({"id":p.id,"duration":p.duration})).collect::<Vec<_>>()),"clocks":clocks}));
        }
    }
    rows.sort_by_key(|v|(v["graph"].to_string(),v["node_guid"].to_string(),v["context"].to_string()));
    rows
}
