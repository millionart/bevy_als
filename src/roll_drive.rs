use als_core::input::AlsInputCommand;
use als_core::motion::{AlsInputOverride, AlsViewYaw};
use als_core::roll::{AlsRollState, roll_displacement_dir, roll_weight_at};
use als_core::state::{LocomotionState, Stance};
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::RootMotionOutput;

use crate::character_assembly::AlsCharacterAvatar;
use crate::root_motion_link::{clamp_drive_speed, ensure_root_motion_output, read_local_delta};

pub struct AlsRollDrivePlugin;

impl Plugin for AlsRollDrivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            (
                ensure_root_motion_output,
                ensure_roll_state,
                roll_drive_system
                    .after(ensure_roll_state)
                    .after(AnimationGraphSet::PrePhysics),
            ),
        );
    }
}

fn ensure_roll_state(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<als_core::character::AlsCharacter>,
            Without<AlsRollState>,
        ),
    >,
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
            Entity,
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
    rm_q: Query<(&ChildOf, &RootMotionOutput), With<AlsCharacterAvatar>>,
    instances: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    let Ok((entity, mut roll, loco, stance, view, mut vel, cmd, ovr)) = q.single_mut() else {
        return;
    };
    let effective = ovr.map(|o| &o.0).unwrap_or(cmd);

    roll.update(effective, view, loco, stance, dt);

    if !roll.active || dt <= 1e-5 {
        return;
    }

    let weight = roll_weight_at(roll.time);
    let local = read_local_delta(
        rm_q.iter()
            .find(|(child_of, _)| child_of.parent() == entity)
            .map(|(_, output)| output),
    );
    let forward_speed = -local.z / dt;
    let dir = roll_displacement_dir(roll.direction_yaw);
    let drive = clamp_drive_speed(dir * forward_speed, 14.0);
    vel.x = drive.x;
    vel.z = drive.z;

    for (child_of, instance) in instances.iter() {
        if child_of.parent() != entity {
            continue;
        }
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("roll_clip_time", DataValue::from(roll.time));
            player.set_input_data("roll_weight", DataValue::from(weight));
        }
    }
}
