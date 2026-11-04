fn c510_state(player: &AnimationGraphPlayer) -> serde_json::Value {
    let raw = |key| player.get_global_input_data(key);
    let scalar = |key| raw(key).and_then(|value| value.as_f32().ok());
    let vector = |key| raw(key).and_then(|value| value.as_vec3().ok()).map(|v| v.to_array());
    let rotation = raw("locomotion_state.rotation_quaternion_world_space")
        .and_then(|value| value.as_quat().ok()).map(|q| q.to_array());
    let standing = source_player_property(player, "standing_source.acceleration_forward")
        .and_then(|v| v.as_f32().ok());
    let write_frame = player.get_context_arena().and_then(|arena| {
        arena.get_toplevel().node_states
            .get::<SourceInstanceProperties>(NodeId::default(), StateKey::Default)
            .ok()
            .and_then(|s| s.write_frames.get("standing_source.acceleration_forward").copied())
    });
    serde_json::json!({
        "velocity":vector("locomotion_state.velocity_world_space"),
        "acceleration":vector("locomotion_state.acceleration_world_space"),
        "rotation":rotation,
        "max_acceleration":scalar("locomotion_state.max_acceleration"),
        "max_braking":scalar("locomotion_state.max_braking_deceleration"),
        "standing_acceleration":standing,
        "standing_write_frame":write_frame
    })
}

fn c510_edge_input(
    probe: Res<C458Probe>,
    avatars: Query<
        &bevy_animation_graph::core::animated_scene::AnimatedSceneInstance,
        With<bevy_als::character_assembly::AlsCharacterAvatar>,
    >,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    if probe.frame != 21 {
        return;
    }
    let Ok(label) = std::env::var("ALS_C510_EDGE") else { return; };
    let data: serde_json::Value =
        serde_json::from_slice(&std::fs::read("docs/audit/bf144/510/controls.json").unwrap()).unwrap();
    let edge = &data["edge"];
    assert_eq!(edge["frame"], 20);
    let vec = |key: &str| Vec3::from_array(std::array::from_fn(|i| edge[key][i].as_f64().unwrap() as f32));
    for avatar in &avatars {
        if let Ok(mut player) = players.get_mut(avatar.player_entity()) {
            assert_eq!(
                player.get_global_input_data("locomotion_state.velocity_world_space").unwrap().as_vec3().unwrap(),
                vec("velocity")
            );
            player.set_global_input_data("locomotion_state.acceleration_world_space", vec("acceleration"));
            if label == "braking-only" {
                player.set_global_input_data("locomotion_state.max_acceleration", edge["max_braking"].as_f64().unwrap() as f32);
            }
        }
    }
}
