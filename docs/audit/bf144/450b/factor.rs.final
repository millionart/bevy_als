use bevy::animation::animated_field;
#[derive(Resource,Default)]
struct C450BDataFactor { applications:u32, info:serde_json::Value }
fn c450b_data_factor(
    mut state:ResMut<C450BDataFactor>,
    mut graphs:ResMut<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    mut clips:ResMut<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>,
    skeletons:Res<Assets<bevy_animation_graph::core::skeleton::Skeleton>>,
){
    if state.applications>0{return;}
    for(_,graph)in graphs.iter_mut(){for node in graph.nodes.values_mut(){
        if node.id.uuid().to_string()!="5ced9e76-5148-58a8-a591-ffda3ad6d593"{continue;}
        let mut builtin=node.try_inner_downcast_ref::<bevy_animation_graph::builtin_nodes::clip_node::ClipNode>().unwrap().clone();
        let original_handle={let bevy::reflect::ReflectRef::Struct(fields)=builtin.reflect_ref()else{panic!("The Forward clip is not a reflected struct");};fields.field("clip").unwrap().try_downcast_ref::<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>().unwrap().clone()};
        let Some(original)=clips.get(&original_handle).cloned()else{return;};let Some(skeleton)=skeletons.get(&original.skeleton)else{return;};
        let pelvis=skeleton.iter_bones().find(|id|skeleton.id_to_path(*id).and_then(|p|p.parts.last().map(|n|n.as_str().to_owned()))==Some("pelvis".to_owned())).unwrap();
        let data:serde_json::Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/450b/key-data.json").unwrap()).unwrap();
        let mut keys=data["times"].as_array().unwrap().iter().zip(data["values"].as_array().unwrap()).map(|(t,v)|(t.as_f64().unwrap()as f32,Vec3::new(v[0].as_f64().unwrap()as f32,v[1].as_f64().unwrap()as f32,v[2].as_f64().unwrap()as f32))).collect::<Vec<_>>();
        let index=data["half_index"].as_u64().unwrap()as usize;assert_eq!(keys[index].0,0.5);
        for(time,value)in &keys{assert_eq!(bevy_als::extensions::source_sequence_player::sample_pose(&original,*time).get_bone(pelvis).unwrap().translation,Some(*value));}
        let before=keys[index].1;keys[index].1.x=data["current_reference_x"].as_f64().unwrap()as f32+data["compressed_delta_x"].as_f64().unwrap()as f32;let after=keys[index].1;
        let replacement=VariableCurve::new(AnimatableCurve::new(animated_field!(Transform::translation),UnevenSampleAutoCurve::new(keys.clone()).unwrap()));
        let mut cloned=original.clone();let curves=cloned.curves.get_mut(&pelvis.animation_target_id()).unwrap();let curve_index=curves.iter().position(|v|v.0.evaluator_id()==replacement.0.evaluator_id()).unwrap();curves[curve_index]=replacement;
        for(time,_)in &keys{
            let mut expected=bevy_als::extensions::source_sequence_player::sample_pose(&original,*time);
            if *time==0.5{let i=*expected.paths.get(&pelvis).unwrap();expected.bones[i].translation=Some(after);}
            assert_eq!(bevy_als::extensions::source_sequence_player::sample_pose(&cloned,*time),expected);
        }
        let handle=clips.add(cloned);let cloned_id=handle.id();
        {let bevy::reflect::ReflectMut::Struct(fields)=builtin.reflect_mut()else{panic!("The Forward clip is not mutable reflection data");};*fields.field_mut("clip").unwrap().try_downcast_mut::<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>().unwrap()=handle;}
        let routed={let bevy::reflect::ReflectRef::Struct(fields)=builtin.reflect_ref()else{panic!("The Forward clip reflection was lost");};fields.field("clip").unwrap().try_downcast_ref::<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>().unwrap().id()==cloned_id};
        state.info=serde_json::json!({"before":before.to_array(),"after":after.to_array(),"routed_to_clone":routed,"translation_key_count":keys.len(),"changed_key":index,"only_one_key_pelvis_x_changed":true});
        node.inner=bevy_animation_graph::core::animation_node::dyn_node_like::DynNodeLike::new(builtin);state.applications+=1;return;
    }}
}
fn c450b_markers(
    player:&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs:&Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    skeletons:&Assets<bevy_animation_graph::core::skeleton::Skeleton>,
)->Vec<serde_json::Value>{
    let mut rows=Vec::new();
    if let Some(arena)=player.get_context_arena(){for context_id in arena.iter_context_ids(){
        let context=arena.get_context(context_id).unwrap();let Some(graph)=graphs.get(context.get_graph_id())else{continue;};
        for node in graph.nodes.values(){
            let guid=node.id.uuid().to_string();let label=match guid.as_str(){"ccaab6a2-424d-b00d-6794-f3a705d6f8c2"=>"base_provider_input","e4f33e44-4019-9bac-8e0a-dcbc0e3ae178"=>"rig_source","8191c2ea-49e5-bd21-ee33-86bd87b2f93c"=>"root_result",_=>continue};
            let key=bevy_animation_graph::core::context::node_states::StateKey::Default;if !context.node_caches.is_updated(node.id,key){continue;}
            let Some(value)=context.node_caches.get_output_data(node.id,key,"Pose".into())else{continue;};let Ok(pose)=value.as_pose()else{continue;};let Some(skeleton)=skeletons.get(&pose.skeleton)else{continue;};
            let id=pose.paths.keys().find(|id|skeleton.id_to_path(**id).and_then(|p|p.parts.last().map(|n|n.as_str().to_owned()))==Some("pelvis".to_owned())).unwrap();let b=pose.get_bone(*id).unwrap();
            rows.push(serde_json::json!({"boundary":label,"node_guid":node.id,"translation":b.translation.map(|v|v.to_array()),"rotation":b.rotation.map(|v|v.to_array()),"scale":b.scale.map(|v|v.to_array())}));
        }
    }}rows.sort_by_key(|v|v["boundary"].to_string());rows
}
