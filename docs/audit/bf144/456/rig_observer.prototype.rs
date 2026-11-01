fn c456_rig_snapshot(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs:&Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    skeletons:&Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    assets:&AssetServer,
    before:bool,
)->serde_json::Value {
    use bevy_als::extensions::{root_graph_boundaries::{current_control_rig,ControlRigBoundaryNode},control_rig_hierarchy::RigSpace,control_rig_program::RigValueRef};
    use bevy_animation_graph::core::context::node_states::StateKey;
    let Some((context,node,state))=current_control_rig(player,graphs)else{return serde_json::Value::Null;};
    assert_eq!(node.id.uuid().to_string(),"5514524f-4eea-c42c-19cd-349ea6f99e36");
    let config=node.try_inner_downcast_ref::<ControlRigBoundaryNode>().unwrap();let definition=config.definition.as_ref().unwrap();
    let graph=graphs.get(context.get_graph_id()).unwrap();
    let mut hierarchy=state.snapshot_hierarchy().expect("Actual initialized Rig hierarchy is required");
    let local=definition.bones.iter().enumerate().map(|(index,bone)|serde_json::json!({"index":index,"name":bone.name,"parent":bone.parent,"local":hierarchy.transform(index,RigSpace::Local,false).unwrap()})).collect::<Vec<_>>();
    let registers=(0..definition.program.initial_values.len()).map(|register|state.value(&RigValueRef{register,path:Vec::new()}).unwrap()).collect::<Vec<_>>();
    let externals=definition.externals.iter().map(|(name,reference)|(name.clone(),serde_json::json!({"reference":reference,"value":state.value(reference).unwrap()}))).collect::<std::collections::BTreeMap<_,_>>();
    let bindings=config.input_bindings.iter().map(|binding|serde_json::json!({"external":binding.external,"path":binding.path,"property":binding.property,"resolved_player_property":bevy_als::extensions::source_instance::source_player_property(player,&binding.property),"global":player.get_global_input_data(&binding.property)})).collect::<Vec<_>>();
    let mut poses=std::collections::BTreeMap::new();
    for (name,guid)in [("rig_source","e4f33e44-4019-9bac-8e0a-dcbc0e3ae178"),("rig_output","5514524f-4eea-c42c-19cd-349ea6f99e36"),("ragdolling_output","01dbd86a-4d37-d1df-d0f4-65a458a965ab"),("root_result","8191c2ea-49e5-bd21-ee33-86bd87b2f93c")] {
        let n=graph.nodes.values().find(|n|n.id.uuid().to_string()==guid).unwrap();
        let updated=context.node_caches.is_updated(n.id,StateKey::Default);
        let pose=context.node_caches.get_output_data(n.id,StateKey::Default,"Pose".into()).map(|v|c456_pose_value(v.as_pose().unwrap(),skeletons,assets));
        poses.insert(name,serde_json::json!({"node_guid":guid,"updated":updated,"pose":pose}));
    }
    let calls=state.calls.iter().map(|call|serde_json::json!({"instruction":call.instruction,"arguments_after_call":call.arguments,"bones_before_call":call.bones_before})).collect::<Vec<_>>();
    let traces=state.traces.iter().map(|trace|serde_json::json!({"instruction":trace.instruction,"start":trace.start,"end":trace.end,"hit":trace.hit})).collect::<Vec<_>>();
    if !before {
        let p=std::path::Path::new(&std::env::var("ALS_C456_OUTPUT").unwrap()).with_file_name("rig-definition.json");
        if !p.exists(){std::fs::write(p,serde_json::to_vec_pretty(definition).unwrap()).unwrap();}
    }
    serde_json::json!({"node_guid":node.id,"context_graph":assets.get_path(context.get_graph_id()).map(|p|p.to_string()),"phase":if before{"before_evaluation_previous_state"}else{"after_evaluation"},"state_key":"Default","initialization_count":state.initialization_count,"executed_steps":state.executed_steps,"elapsed_seconds":state.elapsed_seconds,"to_world":state.to_world,"upstream_context_curves":state.context.curves,"poses":poses,"hierarchy_local":local,"registers":registers,"externals":externals,"bindings":bindings,"calls":calls,"traces":traces,"capture_global":player.get_global_input_data(bevy_als::extensions::root_graph_boundaries::RIG_CAPTURE_KEY).and_then(|v|v.as_bool().ok())})
}
