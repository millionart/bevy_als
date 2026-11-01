type C448DSceneTarget=(Entity,Option<&'static Name>,&'static Transform,Option<&'static bevy::animation::AnimationTargetId>,Option<&'static bevy::animation::AnimatedBy>,Has<bevy::mesh::skinning::SkinnedMesh>,Has<Mesh3d>);
fn c448d_scene_entry(
    row:(Entity,Option<&Name>,&Transform,Option<&bevy::animation::AnimationTargetId>,Option<&bevy::animation::AnimatedBy>,bool,bool),
    transforms:&bevy::transform::helper::TransformHelper,
)->serde_json::Value{
    let(entity,name,local,id,owner,skin,mesh)=row;
    serde_json::json!({"entity":format!("{entity:?}"),"name":name.map(|n|n.as_str()),"target_id":id.map(|n|n.0.to_string()),"animated_by":owner.map(|n|format!("{:?}",n.0)),"translation":local.translation.to_array(),"rotation":local.rotation.to_array(),"scale":local.scale.to_array(),"computed_world":transforms.compute_global_transform(entity).ok().map(|v|v.to_matrix().to_cols_array()),"skinned_mesh":skin,"mesh":mesh})
}
fn c448d_scene_root(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    player_entity:Entity,
    skeletons:&Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    targets:&Query<C448DSceneTarget>,
    transforms:&bevy::transform::helper::TransformHelper,
)->serde_json::Value{
    let mut entries=Vec::new();
    if let Some(pose)=player.get_default_output_pose(){
        if let Some(skeleton)=skeletons.get(&pose.skeleton){
            for(id,index)in &pose.paths{
                let Some(path)=skeleton.id_to_path(*id)else{continue;};
                if path.parts.last().map(|n|n.as_str())!=Some("SKM_Als"){continue;}
                let bone=&pose.bones[*index];let mapped=player.bone_entity(*id);
                let mut matching=targets.iter().filter(|(_,_,_,target,owner,_,_)|target.is_some_and(|v|bevy_animation_graph::core::pose::BoneId::from(*v)==*id)&&owner.is_some_and(|v|v.0==player_entity)).map(|v|c448d_scene_entry(v,transforms)).collect::<Vec<_>>();
                matching.sort_by_key(|v|v["entity"].to_string());
                entries.push(serde_json::json!({"bone_id":id.id().to_string(),"path":path.parts.iter().map(|n|n.as_str().to_owned()).collect::<Vec<_>>(),"pose_translation":bone.translation.map(|v|v.to_array()),"pose_rotation":bone.rotation.map(|v|v.to_array()),"pose_scale":bone.scale.map(|v|v.to_array()),"mapped_entity":mapped.map(|v|format!("{v:?}")),"mapped_components":mapped.and_then(|v|targets.get(v).ok()).map(|v|c448d_scene_entry(v,transforms)),"actual_matching_targets":matching}));
            }
        }
    }
    serde_json::json!({"player_entity":format!("{player_entity:?}"),"pose_timestamp":player.get_default_output_pose().map(|p|p.timestamp),"entries":entries})
}
