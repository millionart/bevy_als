fn c495_scales(
    player: &bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs: &Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    skeletons: &Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    assets: &AssetServer,
) -> Vec<serde_json::Value> {
    use bevy_als::extensions::{
        SourceMultiWayBlendNode, source_instance::source_player_property,
        source_pose_nodes::SourceTwoWayBlendState,
    };
    use bevy_animation_graph::core::animation_graph::{SourcePin, TargetPin};
    let Some(arena) = player.get_context_arena() else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for id in arena.iter_context_ids() {
        let context = arena.get_context(id).unwrap();
        let Some(graph) = graphs.get(context.get_graph_id()) else {
            continue;
        };
        let path = assets
            .get_path(context.get_graph_id())
            .map(|p| p.to_string());
        for (node_id, key) in c458_updated_keys(&context.node_caches) {
            let node = &graph.nodes[&node_id];
            let guid = node_id.uuid().to_string();
            let lean = match &guid[..8] {
                "7c742ec2" => {
                    use bevy_animation_graph::core::animation_graph::TimeUpdate;
                    let update = context.query_output_time.get(key);
                    let player_delta = player
                        .get_global_input_data("animation.delta_seconds")
                        .and_then(|value| value.as_f32().ok())
                        .unwrap();
                    let delta = match update {
                        Some(TimeUpdate::Delta(value)) => value.max(0.0),
                        Some(_) => 0.0,
                        None => player_delta,
                    };
                    let raw =
                        source_player_property(player, "pose_state.unweighted_gait_running_amount")
                            .or_else(|| {
                                player.get_global_input_data(
                                    "pose_state.unweighted_gait_running_amount",
                                )
                            })
                            .and_then(|value| value.as_f32().ok());
                    Some(
                        serde_json::json!({"previous":c497_lean_alpha(context,node_id,key,true),
                        "mapped":c497_lean_alpha(context,node_id,key,false),"raw":raw,
                        "time_update":format!("{update:?}"),"delta":delta,"player_delta":player_delta}),
                    )
                }
                "34f57700" => {
                    let input = TargetPin::NodeData(node_id, "position".into());
                    let SourcePin::NodeData(position, pin) = &graph.edges_inverted[&input] else {
                        unreachable!()
                    };
                    let value = context
                        .node_caches
                        .get_output_data(*position, key, pin.clone())
                        .unwrap()
                        .as_vec2()
                        .unwrap();
                    let blend = node
                        .try_inner_downcast_ref::<bevy_als::extensions::NormalizedBlendSpaceNode>()
                        .unwrap();
                    let t = blend.source_triangulation.as_ref().unwrap();
                    Some(
                        serde_json::json!({"position":value.to_array(),"points":blend.points.iter().map(|point|point.point.to_array()).collect::<Vec<_>>(),
                        "triangles":t.triangles,"min":t.axis_min.to_array(),"max":t.axis_max.to_array(),
                        "teleport":blend.teleport_to_normalized_time}),
                    )
                }
                _ => None,
            };
            let alpha = context
                .node_states
                .get::<SourceTwoWayBlendState>(node_id, key)
                .ok()
                .map(|state| state.blend_alpha);
            let multi=node.try_inner_downcast_ref::<SourceMultiWayBlendNode>().map(|multi| {
                let desired=multi.desired_alpha_keys.iter().map(|name| source_player_property(player,name)
                    .and_then(|value|value.as_f32().ok())).collect::<Vec<_>>();
                serde_json::json!({"desired":desired,"normalize":multi.normalize_alpha,"scale":multi.alpha_scale})
            });
            let mut pins = vec!["Pose".into(), "pose".into(), "Result".into()];
            for edge in graph.edges_inverted.values() {
                if let SourcePin::NodeData(id, pin) = edge
                    && *id == node_id
                    && !pins.contains(pin)
                {
                    pins.push(pin.clone());
                }
            }
            let inputs = graph
                .edges_inverted
                .iter()
                .filter_map(|(target, source)| {
                    let TargetPin::NodeData(id, pin) = target else {
                        return None;
                    };
                    if *id != node_id {
                        return None;
                    }
                    Some(serde_json::json!({"pin":pin,"source":format!("{source:?}")}))
                })
                .collect::<Vec<_>>();
            for pin in pins {
                let Some(pose) = context
                    .node_caches
                    .get_output_data(node_id, key, pin.clone())
                    .and_then(|value| value.into_pose().ok())
                else {
                    continue;
                };
                let skeleton = skeletons.get(&pose.skeleton).unwrap();
                let Some(bone) = pose.paths.keys().find(|id| {
                    skeleton.id_to_path(**id).is_some_and(|path| {
                        path.parts
                            .last()
                            .is_some_and(|name| name.as_str() == "VB foot_l")
                    })
                }) else {
                    continue;
                };
                let value = pose.get_bone(*bone).unwrap();
                rows.push(serde_json::json!({"graph":path,"context":format!("{id:?}"),"guid":node_id,
                    "node":node.name,"key":format!("{key:?}"),"pin":pin,"time":pose.timestamp,
                    "scale":value.scale.map(|v|v.to_array()),"translation":value.translation.map(|v|v.to_array()),
                    "rotation":value.rotation.map(|v|v.to_array()),"inputs":inputs,
                    "blend_alpha":alpha,"multi":multi,"lean":lean}));
            }
        }
    }
    rows.sort_by_key(|row| {
        (
            row["context"].to_string(),
            row["guid"].to_string(),
            row["pin"].to_string(),
            row["key"].to_string(),
        )
    });
    rows
}

fn c497_lean_alpha(
    context: &bevy_animation_graph::core::context::graph_context::GraphState,
    node: bevy_animation_graph::core::animation_graph::NodeId,
    key: bevy_animation_graph::core::context::node_states::StateKey,
    previous: bool,
) -> Option<serde_json::Value> {
    use bevy::reflect::{PartialReflect, ReflectRef};
    use bevy_animation_graph::core::context::node_state_box::NodeStateBox;
    let ReflectRef::Struct(states) = context.node_states.reflect_ref() else {
        unreachable!()
    };
    let ReflectRef::Map(map) = states.field("states").unwrap().reflect_ref() else {
        unreachable!()
    };
    let ReflectRef::Struct(state) = map.get(&node)?.reflect_ref() else {
        unreachable!()
    };
    let value = if previous {
        let ReflectRef::Enum(value) = state.field("last_state").unwrap().reflect_ref() else {
            unreachable!()
        };
        value.field_at(0)?
    } else {
        let ReflectRef::Map(value) = state.field("upcoming_state").unwrap().reflect_ref() else {
            unreachable!()
        };
        value.get(&key)?
    };
    let value = value
        .try_downcast_ref::<std::sync::Arc<NodeStateBox>>()
        .unwrap();
    let ReflectRef::Struct(value) = value.value.as_reflect().reflect_ref() else {
        unreachable!()
    };
    let initialized = *value.field("initialized")?.try_downcast_ref::<bool>()?;
    let mapped = *value.field("value")?.try_downcast_ref::<f32>()?;
    Some(serde_json::json!({"initialized":initialized,"value":mapped}))
}
