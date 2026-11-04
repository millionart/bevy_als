fn c501_blend(
    player: &bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
) -> serde_json::Value {
    use bevy_als::extensions::source_instance::source_player_property;
    let value =
        |key| source_player_property(player, key).or_else(|| player.get_global_input_data(key));
    let amounts = [
        "grounded_state.velocity_blend.forward_amount",
        "grounded_state.velocity_blend.backward_amount",
        "grounded_state.velocity_blend.left_amount",
        "grounded_state.velocity_blend.right_amount",
    ]
    .map(|key| value(key).and_then(|v| v.as_f32().ok()));
    serde_json::json!({
        "amounts":amounts,
        "initialization_required":value("grounded_state.velocity_blend.b_initialization_required").and_then(|v|v.as_bool().ok()),
        "velocity":value("locomotion_state.velocity_world_space").and_then(|v|v.as_vec3().ok()).map(|v|v.to_array()),
        "rotation":value("locomotion_state.rotation_quaternion_world_space").and_then(|v|v.as_quat().ok()).map(|v|v.to_array()),
        "delta":value("animation.delta_seconds").and_then(|v|v.as_f32().ok())
    })
}
