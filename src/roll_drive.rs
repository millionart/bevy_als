use als_core::input::AlsInputCommand;
use als_core::motion::{AlsInputOverride, AlsViewYaw};
use als_core::roll::{roll_displacement_dir, roll_speed_at, roll_weight_at, AlsRollState};
use als_core::state::{LocomotionState, Stance};
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

pub struct AlsRollDrivePlugin;

impl Plugin for AlsRollDrivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (ensure_roll_state, roll_drive_system.after(ensure_roll_state)),
        );
    }
}

fn ensure_roll_state(
    mut commands: Commands,
    q: Query<Entity, (With<als_core::character::AlsCharacter>, Without<AlsRollState>)>,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsRollState::default());
    }
}

#[allow(clippy::type_complexity)]
fn roll_drive_system(
    time: Res<Time>,
    mut q: Query<
        (
            &mut AlsRollState,
            &LocomotionState,
            &Stance,
            &AlsViewYaw,
            &mut LinearVelocity,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    let Ok((mut roll, loco, stance, view, mut vel, cmd, ovr)) = q.single_mut() else {
        return;
    };
    let effective = ovr.map(|o| &o.0).unwrap_or(cmd);

    roll.update(effective, view, loco, stance, dt);

    if !roll.active {
        return;
    }

    let weight = roll_weight_at(roll.time);
    let speed = roll_speed_at(roll.time / als_core::roll::ROLL_DURATION_SECS);
    let dir = roll_displacement_dir(roll.direction_yaw);
    vel.x = dir.x * speed;
    vel.z = dir.z * speed;

    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("roll_weight", DataValue::from(weight));
        }
    }
}
