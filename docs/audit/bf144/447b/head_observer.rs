fn c447b_head_values(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs:&Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
)->serde_json::Value{
    let mut floats=serde_json::Map::new();
    for key in ["animation.delta_seconds","animation.delta_seconds_without_time_dilation","locomotion_state.input_yaw_angle_world_space","locomotion_state.target_yaw_angle_world_space","locomotion_state.rotation_yaw","locomotion_state.yaw_velocity_world_space","view_state.yaw_angle","view_state.pitch_angle","movement_base.delta_yaw","head_state.pitch_angle","head_state.yaw_angle","head_state.yaw_velocity","head_state.yaw_amount"]{
        let global=player.get_global_input_data(key).and_then(|v|v.as_f32().ok());
        let property=bevy_als::extensions::source_instance::source_player_property(player,key).and_then(|v|v.as_f32().ok());
        let frame_input=player.get_global_input_data(format!("source_frame_input.{key}")).and_then(|v|v.as_f32().ok());
        floats.insert(key.to_owned(),serde_json::json!({"global":global,"property":property,"frame_input":frame_input,"resolved":property.or(global)}));
    }
    let mut booleans=serde_json::Map::new();
    for key in ["view_mode.first_person","rotation_mode.velocity_direction","locomotion_state.b_has_input","movement_base.b_has_relative_rotation","head_state.b_initialization_required","head_state.b_switching_look_sides"]{
        let global=player.get_global_input_data(key).and_then(|v|v.as_bool().ok());
        let property=bevy_als::extensions::source_instance::source_player_property(player,key).and_then(|v|v.as_bool().ok());
        let frame_input=player.get_global_input_data(format!("source_frame_input.{key}")).and_then(|v|v.as_bool().ok());
        booleans.insert(key.to_owned(),serde_json::json!({"global":global,"property":property,"frame_input":frame_input,"resolved":property.or(global)}));
    }
    let mut pins=Vec::new();
    if let Some(arena)=player.get_context_arena(){
        for context_id in arena.iter_context_ids(){
            let context=arena.get_context(context_id).unwrap();
            let Some(graph)=graphs.get(context.get_graph_id())else{continue;};
            for node in graph.nodes.values(){
                let Some(input)=node.try_inner_downcast_ref::<bevy_als::extensions::GlobalF32InputNode>()else{continue;};
                if input.key!="head_state.yaw_amount"&&input.key!="head_state.pitch_angle"{continue;}
                let key=bevy_animation_graph::core::context::node_states::StateKey::Default;
                if !context.node_caches.is_updated(node.id,key){continue;}
                let value=context.node_caches.get_output_data(node.id,key,input.key.clone()).and_then(|v|v.as_f32().ok());
                pins.push(serde_json::json!({"key":input.key,"node_guid":node.id,"value":value}));
            }
        }
    }
    pins.sort_by_key(|v|(v["key"].to_string(),v["node_guid"].to_string()));
    serde_json::json!({"floats":floats,"booleans":booleans,"actual_look_pins":pins})
}
