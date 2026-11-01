fn c451a_transform_value(t:Transform)->serde_json::Value{serde_json::json!({"translation":t.translation.to_array(),"rotation":t.rotation.to_array(),"scale":t.scale.to_array()})}
fn c451a_head_apply(
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
            if node.id.uuid().to_string()!="d17ea3bf-4d5e-ebd5-fd47-0c867f5cc0e7"{continue;}
            let config=node.try_inner_downcast_ref::<bevy_als::extensions::SourceMeshSpaceAdditiveNode>().unwrap();
            assert!(!config.use_constant_alpha&&config.expose_alpha_pin&&config.mapping.source_semantics&&!config.mapping.map_range&&!config.mapping.clamp_result&&!config.mapping.interpolate_result);
            assert_eq!((config.mapping.scale,config.mapping.bias,config.alpha_scale,config.alpha_bias),(1.0,0.0,1.0,0.0));
            assert!(!graph.edges_inverted.contains_key(&TargetPin::NodeData(node.id,config.alpha_pin.clone())));
            for(_,key)in c451a_updated_keys(&context.node_caches).into_iter().filter(|(id,_)|*id==node.id){
                let global=player.get_global_input_data(&config.alpha_key).and_then(|v|v.as_f32().ok());
                let resolved_property=bevy_als::extensions::source_instance::source_player_property(player,&config.alpha_key).and_then(|v|v.as_f32().ok());
                let raw=resolved_property.or(global).expect("An evaluated Head alpha input is required");let alpha=raw.clamp(0.0,1.0);
                let output=context.node_caches.get_output_data(node.id,key,"Pose".into()).unwrap().into_pose().unwrap();
                let mut inputs=Vec::new();let mut poses=Vec::new();
                for pin in ["Base","Additive"]{
                    let SourcePin::NodeData(id,name)=graph.edges_inverted.get(&TargetPin::NodeData(node.id,pin.into())).unwrap()else{panic!("An evaluated Head pose input must be a node output");};assert!(context.node_caches.is_updated(*id,key));
                    let pose=context.node_caches.get_output_data(*id,key,name.clone()).unwrap().into_pose().unwrap();
                    inputs.push(serde_json::json!({"pin":pin,"source_node":id,"source_pin":name,"pose":c451a_pose_value(&pose,skeletons,assets)}));poses.push(pose);
                }
                let skeleton=skeletons.get(&poses[0].skeleton).unwrap();let mut expected=poses[0].clone();
                bevy_als::extensions::mesh_space_additive::apply_mesh_space_additive(&mut expected,&poses[1],alpha,skeleton);
                let output_path=std::env::var("ALS_C451A_OUTPUT").unwrap();let snapshot=std::path::Path::new(&output_path).with_file_name("skeleton.json");
                if !snapshot.exists(){
                    let mut bones=skeleton.iter_bones().map(|id|{let path=skeleton.id_to_path(id).unwrap();let d=skeleton.default_transforms(id);serde_json::json!({"path":path.parts.iter().map(|n|n.as_str().to_owned()).collect::<Vec<_>>(),"name":path.parts.last().map(|n|n.as_str()),"local":d.map(|v|c451a_transform_value(v.local)),"character":d.map(|v|c451a_transform_value(v.character))})}).collect::<Vec<_>>();bones.sort_by_key(|v|v["path"].to_string());
                    let root=skeleton.id_to_path(skeleton.root()).unwrap();std::fs::write(snapshot,serde_json::to_vec_pretty(&serde_json::json!({"root_path":root.parts.iter().map(|n|n.as_str().to_owned()).collect::<Vec<_>>(),"asset":assets.get_path(poses[0].skeleton.id()).map(|p|p.to_string()),"bones":bones})).unwrap()).unwrap();
                }
                rows.push(serde_json::json!({"node_guid":node.id,"context":format!("{context_id:?}"),"state_key":format!("{key:?}"),"alpha_key":config.alpha_key,"global_alpha":global,"resolved_property_alpha":resolved_property,"raw_alpha":raw,"effective_alpha":alpha,"identity_mapping_verified":true,"inputs":inputs,"actual":c451a_pose_value(&output,skeletons,assets),"public":c451a_pose_value(&expected,skeletons,assets),"pose_exact_public":output==expected}));
            }
        }
    }}rows.sort_by_key(|v|(v["context"].to_string(),v["state_key"].to_string()));rows
}
