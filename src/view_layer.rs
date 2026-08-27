use als_core::input::AlsInputCommand;
use als_core::motion::AlsInputOverride;
use als_core::view::{
    AlsViewPitch, AlsViewSmoothed, LOOK_HALF_LIFE_SECONDS, clamp_pitch, damp_view,
};
use bevy::prelude::*;

pub struct ViewLayerPlugin;

const LOOK_RADIANS_PER_COUNT: f32 = 0.0025;

impl Plugin for ViewLayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, view_input_integrator);
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
