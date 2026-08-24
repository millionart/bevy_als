use als_core::input::AlsInputCommand;
use als_core::mantle::{mantle_weight_at, warp_envelope, AlsMantleState, MantleKind};
use als_core::motion::AlsInputOverride;
use als_core::state::{LocomotionState, Stance};
use avian3d::prelude::{CollisionLayers, LinearVelocity};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::RootMotionOutput;

use crate::root_motion_link::{clamp_drive_speed, ensure_root_motion_output, read_local_delta};

pub struct AlsMantleDrivePlugin;

impl Plugin for AlsMantleDrivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                ensure_root_motion_output,
                ensure_mantle_state,
                mantle_pose_system.after(ensure_mantle_state),
            ),
        );
        app.add_systems(
            FixedPostUpdate,
            mantle_drive_system
                .after(AnimationGraphSet::PrePhysics),
        );
    }
}

fn ensure_mantle_state(
    mut commands: Commands,
    q: Query<Entity, (With<als_core::character::AlsCharacter>, Without<AlsMantleState>)>,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsMantleState::default());
    }
}

fn mantle_pose_system(
    time: Res<Time>,
    mut q: Query<
        (
            &mut AlsMantleState,
            &mut Transform,
            &mut LinearVelocity,
        ),
        With<als_core::character::AlsCharacter>,
    >,
) {
    let dt = time.delta_secs();
    let Ok((mut m, mut tf, mut vel)) = q.single_mut() else {
        return;
    };
    let yaw_now = lerp_angle(
        m.yaw_start,
        m.yaw_target,
        warp_envelope(m.clip_time()),
    );
    if m.active {
        tf.rotation = Quat::from_rotation_y(yaw_now);
        return;
    }
    if m.settle > 0.0 {
        m.settle -= dt;
        tf.translation = m.target;
        tf.rotation = Quat::from_rotation_y(yaw_now);
        *vel = LinearVelocity(Vec3::ZERO);
    }
}

#[allow(clippy::type_complexity)]
fn mantle_drive_system(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &mut AlsMantleState,
            &mut LinearVelocity,
            &LocomotionState,
            &Stance,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    layers_q: Query<Option<&CollisionLayers>, With<als_core::character::AlsCharacter>>,
    rm_q: Query<&RootMotionOutput, With<AnimatedSceneInstance>>,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    let Ok((entity, mut m, mut vel, loco, stance, cmd, ovr)) = q.single_mut() else {
        return;
    };
    let effective = ovr.map(|o| &o.0).unwrap_or(cmd);

    m.tick(effective, loco, stance, dt);

    let layers_none = layers_q
        .get(entity)
        .ok()
        .flatten()
        .map(|l| *l == CollisionLayers::NONE)
        .unwrap_or(false);
    if m.active && !layers_none {
        commands.entity(entity).insert(CollisionLayers::NONE);
    } else if !m.active && layers_none {
        commands.entity(entity).remove::<CollisionLayers>();
    }

    if !m.active {
        return;
    }

    if dt <= 1e-5 {
        return;
    }

    let local_delta = read_local_delta(rm_q.iter().next());
    let clip_t = m.clip_time();
    let yaw_now = lerp_angle(m.yaw_start, m.yaw_target, warp_envelope(clip_t));
    let finishing = m.time + dt >= m.action_end();

    let dir = Vec3::new(yaw_now.sin(), 0.0, -yaw_now.cos());
    let forward_speed = -local_delta.z / dt;
    let climb_speed = local_delta.y / dt;
    if finishing {
        *vel = LinearVelocity(Vec3::ZERO);
        m.settle = als_core::mantle::MANTLE_SETTLE_SECS;
    } else {
        let drive = Vec3::new(dir.x * forward_speed, climb_speed, dir.z * forward_speed);
        *vel = LinearVelocity(clamp_drive_speed(drive, 14.0));
    }

    let weight = mantle_weight_at(m.time, m.action_end());
    let (high_w, lr) = match m.kind {
        MantleKind::High => (1.0, 0.0),
        MantleKind::LowLeft => (0.0, 0.0),
        MantleKind::LowRight => (0.0, 1.0),
    };
    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("mantle_clip_time", DataValue::from(clip_t));
            player.set_input_data("mantle_weight", DataValue::from(weight));
            player.set_input_data("mantle_high_w", DataValue::from(high_w));
            player.set_input_data("mantle_lr", DataValue::from(lr));
        }
    }
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let delta = shortest_angle(b - a);
    a + delta * t
}

fn shortest_angle(angle: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    let wrapped = angle.rem_euclid(two_pi);
    if wrapped > std::f32::consts::PI {
        wrapped - two_pi
    } else {
        wrapped
    }
}

