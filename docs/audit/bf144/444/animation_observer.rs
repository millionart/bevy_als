fn c444_animation(
    In(before): In<bool>,
    mut probe: ResMut<C444Probe>,
    time: Res<Time>,
    virtual_time: Res<Time<Virtual>>,
    avatars: Query<(&ChildOf,&bevy_animation_graph::core::animated_scene::AnimatedSceneInstance),With<bevy_als::character_assembly::AlsCharacterAvatar>>,
    characters: Query<(&bevy_als::actual_motion::AlsActualMotion,&Position,&LinearVelocity)>,
    players: Query<(&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,Option<&bevy_als::extensions::post_physics_animation::AnimationPoseUpdateGate>)>,
) {
    let Some((parent,instance))=avatars.iter().next() else{return;};
    let Ok((motion,position,velocity))=characters.get(parent.parent()) else{return;};
    let Ok((player,gate))=players.get(instance.player_entity()) else{return;};
    let scalar=|key|player.get_global_input_data(key).and_then(|v|v.as_f32().ok());
    let vector=|key|player.get_global_input_data(key).and_then(|v|v.as_vec3().ok()).map(|v|v.to_array());
    let boolean=|key|player.get_global_input_data(key).and_then(|v|v.as_bool().ok());
    let row=serde_json::json!({
        "ordinal":probe.actor.map(|_|probe.frame.saturating_sub(1)),
        "frame_delta":time.delta_secs(),"virtual_delta_ns":virtual_time.delta().as_nanos() as u64,
        "position":position.0.to_array(),"velocity":velocity.0.to_array(),
        "motion_world_velocity":motion.world_velocity.to_array(),"motion_acceleration":motion.world_acceleration.to_array(),
        "motion_speed":motion.horizontal_speed,"motion_has_input":motion.has_input,
        "motion_local_velocity":motion.local_velocity.to_array(),
        "provider_velocity":vector("locomotion_state.velocity_world_space"),
        "provider_acceleration":vector("locomotion_state.acceleration_world_space"),
        "provider_speed":scalar("locomotion_state.speed"),"provider_has_input":boolean("locomotion_state.b_has_input"),
        "provider_max_acceleration":scalar("locomotion_state.max_acceleration"),
        "provider_max_braking":scalar("locomotion_state.max_braking_deceleration"),
        "animation_delta":scalar("animation.delta_seconds"),
        "animation_pending":boolean("animation.pending_update"),
        "arena_frame":player.get_context_arena().map(|a|a.frame_index()),
        "root_time_update":player.get_context_arena().map(|a|format!("{:?}",a.get_toplevel().query_output_time)),
        "elapsed":player.elapsed(),"pose_timestamp":player.get_default_output_pose().map(|p|p.timestamp),
        "paused":player.is_paused(),"graph_error":player.get_error().map(|e|format!("{e:?}")),
        "gate":gate.map(|g|serde_json::json!({"update_graph":g.update_graph,"refresh_bones":g.refresh_bones,"pending_update":g.pending_update}))
    });
    if before {probe.animation_before.push(row);}else{probe.animation_after.push(row);}
}
