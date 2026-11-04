const C505_INIT_NODE: &str = "74f5c4c5-443b-85dc-123b-5f80f77250f5";
const C505_REFRESH_NODE: &str = "18c00b87-478a-29b9-7505-1daa6d305742";
const C505_INIT_FIELD: &str = "grounded_state.velocity_blend.b_initialization_required";
const C506_ALWAYS_UPDATE: bool = true;
const C506_SLOT: &str = "ccaab6a2-424d-b00d-6794-f3a705d6f8c2";

fn c506_configure(
    mut graphs: ResMut<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    mut configured: Local<bool>,
) {
    use bevy_als::extensions::SourceSlotNode;
    if *configured {
        return;
    }
    for (_, graph) in graphs.iter_mut() {
        for node in graph.nodes.values_mut() {
            if node.id.uuid().to_string() != C506_SLOT {
                continue;
            }
            let mut config = node.try_inner_downcast_ref::<SourceSlotNode>().unwrap().clone();
            assert_eq!(config.slot_name, "PostLocomotion");
            assert_eq!(config.weight_key, "post_locomotion.weight");
            assert!(!config.always_update_source_pose);
            if C506_ALWAYS_UPDATE {
                config.always_update_source_pose = true;
                let mut replacement = bevy_animation_graph::core::animation_node::AnimationNode::new(node.name.clone(), config);
                replacement.id = node.id;
                *node = replacement;
            }
            *configured = true;
            return;
        }
    }
}

fn c506_slot(
    player: &bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
) -> serde_json::Value {
    let scalar = |key| player.get_global_input_data(key).and_then(|value| value.as_f32().ok());
    serde_json::json!({
        "has_pose":player.get_global_input_data("post_locomotion.pose").is_some(),
        "weight":scalar("post_locomotion.weight"),
        "layer":scalar("post_locomotion.layer_weight"),
        "mesh_additive":scalar("post_locomotion.mesh_space_additive")
    })
}
