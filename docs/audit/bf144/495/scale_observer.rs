fn c495_scales(
    player: &bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs: &Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    skeletons: &Assets<bevy_animation_graph::core::skeleton::Skeleton>,
    assets: &AssetServer,
) -> Vec<serde_json::Value> {
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
                    "rotation":value.rotation.map(|v|v.to_array()),"inputs":inputs}));
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
