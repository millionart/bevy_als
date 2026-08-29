use als_core::input::AlsInputCommand;
use als_core::mantle::{AlsMantleState, AlsMantleTargetBase, mantle_weight_at};
use als_core::motion::AlsInputOverride;
use als_core::orientation::yaw_to_world_forward;
use als_core::state::{LocomotionState, Stance};
use avian3d::prelude::{CollisionLayers, LinearVelocity};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::RootMotionOutput;

use crate::character_assembly::AlsCharacterAvatar;
use crate::root_motion_link::{clamp_drive_speed, ensure_root_motion_output, read_local_delta};

pub struct AlsMantleDrivePlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsMantleDriveSet;

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
        app.configure_sets(
            FixedPostUpdate,
            AlsMantleDriveSet.after(AnimationGraphSet::PrePhysics),
        )
        .add_systems(
            FixedPostUpdate,
            mantle_drive_system.in_set(AlsMantleDriveSet),
        );
    }
}

fn ensure_mantle_state(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<als_core::character::AlsCharacter>,
            Without<AlsMantleState>,
        ),
    >,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsMantleState::default());
    }
}

fn mantle_pose_system(
    time: Res<Time>,
    target_transforms: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    mut q: Query<
        (&mut AlsMantleState, &mut Transform, &mut LinearVelocity),
        With<als_core::character::AlsCharacter>,
    >,
) {
    let dt = time.delta_secs();
    for (mut m, mut tf, mut vel) in &mut q {
        let target_base = mantle_target_base(&m, &target_transforms);
        if m.active && !m.refresh_target_base(target_base) {
            *vel = LinearVelocity(Vec3::ZERO);
            continue;
        }
        let yaw_now = lerp_angle(m.yaw_start, m.yaw_target, m.rotation_warp_amount());
        if m.active {
            tf.rotation = Quat::from_rotation_y(yaw_now);
            continue;
        }
        if m.settle > 0.0 {
            m.settle -= dt;
            tf.translation = m.target;
            tf.rotation = Quat::from_rotation_y(yaw_now);
            *vel = LinearVelocity(Vec3::ZERO);
        }
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
            &Transform,
            &LocomotionState,
            &Stance,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    layers_q: Query<Option<&CollisionLayers>, With<als_core::character::AlsCharacter>>,
    target_transforms: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    avatars: Query<(
        &AlsCharacterAvatar,
        &AnimatedSceneInstance,
        Option<&RootMotionOutput>,
    )>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    for (entity, mut m, mut vel, transform, loco, stance, cmd, ovr) in &mut q {
        let target_base = mantle_target_base(&m, &target_transforms);
        let target_valid = !m.active || m.refresh_target_base(target_base);
        if !target_valid {
            *vel = LinearVelocity(Vec3::ZERO);
        }
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

        if !target_valid || !m.active || dt <= 1e-5 {
            continue;
        }

        let Some((instance, root_motion)) =
            avatars.iter().find_map(|(avatar, instance, root_motion)| {
                (avatar.character == entity).then_some((instance, root_motion))
            })
        else {
            continue;
        };
        let local_delta = read_local_delta(root_motion);
        let clip_t = m.clip_time();
        let yaw_now = lerp_angle(m.yaw_start, m.yaw_target, m.rotation_warp_amount());
        let finishing = m.time + dt >= m.action_end();

        let dir = yaw_to_world_forward(yaw_now);
        let forward_speed = -local_delta.z / dt;
        let climb_speed = local_delta.y / dt;
        let root_drive = Vec3::new(dir.x * forward_speed, climb_speed, dir.z * forward_speed);
        let time_remaining = (m.action_end() - m.time).max(dt);
        let target_drive = (m.target - transform.translation) / time_remaining;
        let drive = root_drive.lerp(target_drive, m.location_warp_amount());
        if finishing {
            *vel = LinearVelocity(Vec3::ZERO);
            m.settle = als_core::mantle::MANTLE_SETTLE_SECS;
        } else {
            *vel = LinearVelocity(clamp_drive_speed(drive, 14.0));
        }

        let weight = mantle_weight_at(m.time, m.action_end());
        let high_w = if m.kind.is_high() { 1.0 } else { 0.0 };
        let lr = m.kind.graph_low_right_weight();
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("mantle_clip_time", DataValue::from(clip_t));
            player.set_input_data("mantle_weight", DataValue::from(weight));
            player.set_input_data("mantle_high_w", DataValue::from(high_w));
            player.set_input_data("mantle_lr", DataValue::from(lr));
        }
    }
}

fn mantle_target_base(
    state: &AlsMantleState,
    transforms: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) -> Option<AlsMantleTargetBase> {
    let base = state.target_base?;
    let transform = transforms.get(base.entity).ok()?;
    Some(AlsMantleTargetBase {
        entity: base.entity,
        translation: transform.translation(),
        rotation: transform.rotation(),
    })
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
