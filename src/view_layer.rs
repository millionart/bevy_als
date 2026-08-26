use als_core::input::AlsInputCommand;
use als_core::motion::AlsInputOverride;
use als_core::view::{
    AlsViewPitch, AlsViewSmoothed, LOOK_HALF_LIFE_SECONDS, clamp_pitch, damp_view, view_pitch_norm,
};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

use crate::character_assembly::AlsCharacterAvatar;

pub struct ViewLayerPlugin;

const LOOK_RADIANS_PER_COUNT: f32 = 0.0025;

impl Plugin for ViewLayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (view_input_integrator, view_pin_writer).chain());
    }
}

fn view_input_integrator(
    time: Res<Time>,
    mut q: Query<
        (
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            &mut AlsViewPitch,
            Option<&mut AlsViewSmoothed>,
            Entity,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (command, override_command, mut target, smoothed, entity) in &mut q {
        let effective = override_command.map(|value| &value.0).unwrap_or(command);
        target.0 = clamp_pitch(target.0 - effective.look_delta.y * LOOK_RADIANS_PER_COUNT);
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
    characters: Query<&AlsViewSmoothed, With<als_core::character::AlsCharacter>>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (child_of, instance) in &avatars {
        let Ok(smoothed) = characters.get(child_of.parent()) else {
            continue;
        };
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
