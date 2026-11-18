use crate::als::view::{
    AlsViewPitch, AlsViewSmoothed, LOOK_HALF_LIFE_SECONDS, clamp_pitch, damp_view, view_pitch_norm,
};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

pub struct ViewLayerPlugin;

impl Plugin for ViewLayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (view_input_integrator, view_pin_writer).chain());
    }
}

fn view_input_integrator(
    time: Res<Time>,
    mut q: Query<(&AlsViewPitch, Option<&mut AlsViewSmoothed>, Entity)>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (target, smoothed, entity) in &mut q {
        match smoothed {
            Some(mut cur) => {
                cur.0 = damp_view(cur.0, clamp_pitch(target.0), LOOK_HALF_LIFE_SECONDS, dt);
            }
            None => {
                commands
                    .entity(entity)
                    .insert(AlsViewSmoothed(clamp_pitch(target.0)));
            }
        }
    }
}

fn view_pin_writer(
    q: Query<(&AlsViewSmoothed, &AnimatedSceneInstance)>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (smoothed, instance) in &q {
        let Ok(mut player) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        player.set_input_data(
            "view_pitch_norm",
            DataValue::from(view_pitch_norm(smoothed.0)),
        );
        player.set_input_data("look_weight", DataValue::from(1.0_f32));
    }
}
