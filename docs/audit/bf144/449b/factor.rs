use bevy::animation::animated_field;
#[derive(Resource,Default)]
struct C449BUnitFactor { applications:u32, info:serde_json::Value }
fn c449b_unit_factor(
    mut state:ResMut<C449BUnitFactor>,
    mut graphs:ResMut<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    mut clips:ResMut<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>,
    skeletons:Res<Assets<bevy_animation_graph::core::skeleton::Skeleton>>,
){
    if state.applications>0{return;}
    for(_,graph)in graphs.iter_mut(){
        for node in graph.nodes.values_mut(){
            if node.id.uuid().to_string()!="a189ffa4-46d2-a4ea-01e7-728ccfb6d729"{continue;}
            let mut evaluator=node.try_inner_downcast_ref::<bevy_als::extensions::SourceSequenceEvaluatorNode>().unwrap().clone();
            let Some(original)=clips.get(&evaluator.clip).cloned()else{return;};
            let Some(skeleton)=skeletons.get(&original.skeleton)else{return;};
            let pelvis=skeleton.iter_bones().find(|id|skeleton.id_to_path(*id).and_then(|p|p.parts.last().map(|n|n.as_str().to_owned()))==Some("pelvis".to_owned())).unwrap();
            let sample=bevy_als::extensions::source_sequence_player::sample_pose(&original,0.0);
            let before=sample.get_bone(pelvis).unwrap().translation.unwrap();
            let endpoints:serde_json::Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/449/endpoints.json").unwrap()).unwrap();
            assert_eq!(before.y,endpoints["target_pelvis_translation"][1].as_f64().unwrap()as f32);
            let mut after=before;after.y=endpoints["physical_unit_projection"].as_f64().unwrap()as f32;
            let replacement=VariableCurve::new(AnimatableCurve::new(animated_field!(Transform::translation),UnevenSampleAutoCurve::new([(0.0,after),(original.duration(),after)]).unwrap()));
            let mut cloned=original.clone();let curves=cloned.curves.get_mut(&pelvis.animation_target_id()).unwrap();
            let index=curves.iter().position(|v|v.0.evaluator_id()==replacement.0.evaluator_id()).unwrap();
            assert_eq!(curves[index].0.sample_clamped(original.duration()).downcast_ref::<Vec3>().copied(),Some(before));
            curves[index]=replacement;
            let mut expected=sample.clone();expected.bones[*sample.paths.get(&pelvis).unwrap()].translation=Some(after);
            let actual=bevy_als::extensions::source_sequence_player::sample_pose(&cloned,0.0);assert_eq!(actual,expected);
            let handle=clips.add(cloned);let cloned_id=handle.id();
            evaluator.clip=handle;
            state.info=serde_json::json!({"before":before.to_array(),"after":after.to_array(),"routed_to_clone":evaluator.clip.id()==cloned_id,"public_sample_only_pelvis_y_changed":true});
            node.inner=bevy_animation_graph::core::animation_node::dyn_node_like::DynNodeLike::new(evaluator);
            state.applications+=1;return;
        }
    }
}
fn c449b_markers(
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
