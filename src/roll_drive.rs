use als_core::air::AlsAirState;
use als_core::input::AlsInputCommand;
use als_core::motion::{AlsInputOverride, AlsViewYaw};
use als_core::roll::{
    AlsRollState, ROLL_MONTAGE_SEQUENCE_LENGTH_SECS, ROLL_PLAY_RATE, ROLL_ROTATION_HALF_LIFE_SECS,
    damper_exact_angle, roll_displacement_dir, roll_weight_at,
};
use als_core::state::{LocomotionState, Stance};
use als_core::transition::AlsStateRequest;
use avian3d::prelude::{LinearVelocity, Rotation};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::RootMotionOutput;

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
            &mut AlsRollState,
            &LocomotionState,
            &Stance,
            &AlsViewYaw,
            &mut Transform,
            Option<&mut Rotation>,
            &mut LinearVelocity,
            &mut AlsStateRequest,
            Option<&AlsAirState>,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    rm_q: Query<&RootMotionOutput, With<AnimatedSceneInstance>>,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    let Ok((
        mut roll,
        loco,
        stance,
        view,
        mut transform,
        rotation,
        mut vel,
        mut request,
        air,
        cmd,
        ovr,
    )) = q.single_mut()
    else {
        return;
    };
    let effective = ovr.map(|o| &o.0).unwrap_or(cmd);

    let mut lifecycle = roll.update(effective, view, loco, stance, dt);
    if !lifecycle.started && !roll.active && air.is_some_and(|air| air.landing_time <= dt.max(0.05))
    {
        let (actor_yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        lifecycle = roll.start_from_landing(air.unwrap().last_impact_vy, vel.0, actor_yaw, *stance);
    }
    if lifecycle.started {
        request.stance = Some(Stance::Crouching);
    } else if lifecycle.ended {
        request.stance = Some(roll.stance_before_roll);
    }

    if !roll.active || dt <= 1e-5 {
        return;
    }

    let (current_yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
    let next_rotation = Quat::from_rotation_y(damper_exact_angle(
        current_yaw,
        roll.direction_yaw,
        dt,
        ROLL_ROTATION_HALF_LIFE_SECS,
    ));
    transform.rotation = next_rotation;
    if let Some(mut rotation) = rotation {
        rotation.0 = next_rotation;
    }
    let weight = roll_weight_at(roll.time);
    let clip_time = (roll.time * ROLL_PLAY_RATE).min(ROLL_MONTAGE_SEQUENCE_LENGTH_SECS);
    let local = read_local_delta(rm_q.iter().next());
    let forward_speed = -local.z / dt;
    let dir = roll_displacement_dir(roll.direction_yaw);
    let drive = clamp_drive_speed(dir * forward_speed, 14.0);
    vel.x = drive.x;
    vel.z = drive.z;

    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("roll_clip_time", DataValue::from(clip_time));
            player.set_input_data("roll_weight", DataValue::from(weight));
        }
    }
}
