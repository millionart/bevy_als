fn c507_edge_step(probe: Res<C458Probe>, mut strategy: ResMut<TimeUpdateStrategy>) {
    if probe.frame == 84 {
        *strategy = TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(0.0205125));
    } else if probe.frame == 85 {
        *strategy = TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 30.0));
    }
}

fn c507_slot_control(
    probe: Res<C458Probe>,
    avatars: Query<
        &bevy_animation_graph::core::animated_scene::AnimatedSceneInstance,
        With<bevy_als::character_assembly::AlsCharacterAvatar>,
    >,
    mut players: Query<&mut bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>,
) {
    if probe.frame != 86 || std::env::var_os("ALS_C507_NATIVE_HOLD").is_none() {
        return;
    }
    for avatar in &avatars {
        let Ok(mut player) = players.get_mut(avatar.player_entity()) else {
            continue;
        };
        let Some(weight) = player
            .get_global_input_data("post_locomotion.weight")
            .and_then(|value| value.as_f32().ok())
        else {
            continue;
        };
        assert_eq!(weight, 0.9657064080238342_f32);
        player.set_global_input_data("post_locomotion.weight", 1.0_f32);
    }
}
