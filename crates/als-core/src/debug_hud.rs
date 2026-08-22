use bevy::prelude::*;

use crate::character::AlsCharacter;
use crate::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};

#[derive(Component, Default)]
pub struct AlsStateHud;

pub struct AlsStateHudPlugin;

impl Plugin for AlsStateHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud);
        app.add_systems(Update, update_hud);
    }
}

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        AlsStateHud,
        Text::new("ALS states"),
        TextFont {
            font: default(),
            font_size: FontSize::Px(22.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(12.0),
            ..default()
        },
    ));
}

fn update_hud(
    mut q_hud: Query<&mut Text, With<AlsStateHud>>,
    q_chars: Query<
        (
            &LocomotionState,
            &RotationMode,
            &Stance,
            &Gait,
            &OverlayState,
        ),
        With<AlsCharacter>,
    >,
) {
    let Ok((loc, rot, stance, gait, overlay)) = q_chars.single() else {
        return;
    };
    for mut text in q_hud.iter_mut() {
        text.0 = format!(
            "locomotion={loc:?} rotation={rot:?} stance={stance:?} gait={gait:?} overlay={overlay:?}"
        );
    }
}
