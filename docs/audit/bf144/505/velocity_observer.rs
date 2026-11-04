const C505_KEEP_INITIALIZE: bool = true;
const C505_INIT_NODE: &str = "74f5c4c5-443b-85dc-123b-5f80f77250f5";
const C505_REFRESH_NODE: &str = "18c00b87-478a-29b9-7505-1daa6d305742";
const C505_INIT_FIELD: &str = "grounded_state.velocity_blend.b_initialization_required";

fn c505_configure(
    mut graphs: ResMut<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>,
    mut configured: Local<bool>,
    probe: Res<C458Probe>,
) {
    use bevy_als::extensions::source_instance::SourcePropertyWriteNode;
    if *configured || (!C505_KEEP_INITIALIZE && probe.frame < 36) {
        return;
    }
    for (_, graph) in graphs.iter_mut() {
        for node in graph.nodes.values_mut() {
            if node.id.uuid().to_string() != C505_INIT_NODE {
                continue;
            }
            let mut config = node.try_inner_downcast_ref::<SourcePropertyWriteNode>().unwrap().clone();
            assert!(config.on_become_relevant && config.assignments.len() == 1);
            assert_eq!(config.assignments[0].property, C505_INIT_FIELD);
            assert!(config.assignments[0].value.as_bool().unwrap());
            if !C505_KEEP_INITIALIZE {
                config.assignments.clear();
                let mut replacement = bevy_animation_graph::core::animation_node::AnimationNode::new(node.name.clone(), config);
                replacement.id = node.id;
                *node = replacement;
            }
            *configured = true;
            return;
        }
    }
}

fn c505_state(
    player: &bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    graphs: &Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>,
    before: bool,
) -> serde_json::Value {
    use bevy_als::extensions::source_instance::{SourceInstanceProperties, source_player_property};
    use bevy_animation_graph::core::animation_graph::NodeId;
    use bevy_animation_graph::core::context::node_states::StateKey;
    let Some(arena) = player.get_context_arena() else {
        return serde_json::Value::Null;
    };
    let instance = arena.get_toplevel().node_states
        .get::<SourceInstanceProperties>(NodeId::default(), StateKey::Default).ok();
    let value = |key| source_player_property(player, key).or_else(|| player.get_global_input_data(key));
    let scalars = [
        "grounded_state.velocity_blend.forward_amount",
        "grounded_state.velocity_blend.backward_amount",
        "grounded_state.velocity_blend.left_amount",
        "grounded_state.velocity_blend.right_amount",
    ];
    let amounts = scalars.map(|key| value(key).and_then(|v| v.as_f32().ok()));
    let write_frame = instance.and_then(|v| v.write_frames.get(scalars[0]));
    let velocity = value("locomotion_state.velocity_world_space").and_then(|v| v.as_vec3().ok());
    let rotation = value("locomotion_state.rotation_quaternion_world_space").and_then(|v| v.as_quat().ok());
    let delta = value("animation.delta_seconds").and_then(|v| v.as_f32().ok());
    let candidates = before.then(|| {
        let direction = bevy_als::als::animation::velocity_blend_direction(
            velocity.unwrap_or_default(), rotation.unwrap_or(Quat::IDENTITY));
        let snap = [(-direction.z).max(0.0), direction.z.max(0.0), (-direction.x).max(0.0), direction.x.max(0.0)];
        let alpha = 1.0 - bevy_als::als::animation::inv_exp_approx(
            std::f32::consts::LN_2 / (0.1_f32 + 1e-8) * delta.unwrap_or_default());
        let damp = std::array::from_fn::<_, 4, _>(|index| amounts[index].unwrap_or_default().lerp(snap[index], alpha));
        serde_json::json!({"snap":snap,"damp":damp,"alpha":alpha})
    });
    let mut calls = Vec::new();
    if !before {
        for context_id in arena.iter_context_ids() {
            let context = arena.get_context(context_id).unwrap();
            let Some(graph) = graphs.get(context.get_graph_id()) else { continue; };
            if !graph.nodes.keys().any(|id| id.uuid().to_string() == C505_INIT_NODE) { continue; }
            for (id, key) in c458_updated_keys(&context.node_caches) {
                let id = id.uuid().to_string();
                if id == C505_INIT_NODE || id == C505_REFRESH_NODE {
                    calls.push((id, format!("{context_id:?}"), format!("{key:?}")));
                }
            }
        }
        calls.sort();
    }
    serde_json::json!({
        "amounts":amounts,"required":value(C505_INIT_FIELD).and_then(|v|v.as_bool().ok()),
        "write_frame":write_frame,"calls":calls,"instance_id":instance.map(|v|v.instance_id),
        "velocity":velocity.map(|v|v.to_array()),"rotation":rotation.map(|v|v.to_array()),
        "delta":delta,"candidates":candidates
    })
}
