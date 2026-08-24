use als_core::input::AlsInputCommand;
use als_core::mantle::{
    classify_mantle, mantle_weight_at, warp_envelope, AlsMantleState, LedgeProbe, MantleKind,
    MANTLE_PROBE_REACH,
};
use als_core::motion::AlsInputOverride;
use als_core::state::{LocomotionState, Stance};
use avian3d::prelude::{CollisionLayers, LinearVelocity, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::RootMotionOutput;

use crate::root_motion_link::{
    clamp_drive_speed, ensure_root_motion_output, read_local_delta, yaw_from_direction,
};

const MANTLE_PROBE_SHIFT: f32 = 0.45;
const MANTLE_TARGET_INSET: f32 = 0.45;

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
    spatial: SpatialQuery,
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &mut AlsMantleState,
            &mut LinearVelocity,
            &GlobalTransform,
            &LocomotionState,
            &Stance,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    layers_q: Query<Option<&CollisionLayers>, With<als_core::character::AlsCharacter>>,
    rm_q: Query<&RootMotionOutput, With<AnimatedSceneInstance>>,
    velocities: Query<&LinearVelocity, Without<als_core::character::AlsCharacter>>,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    let Ok((entity, mut m, mut vel, gtf, loco, stance, cmd, ovr)) = q.single_mut() else {
        return;
    };
    let effective = ovr.map(|o| &o.0).unwrap_or(cmd);
    let origin = gtf.translation();
    let foot_y = origin.y - 0.90;
    let fwd = Quat::from_rotation_y(m_direction_yaw(gtf))
        .mul_vec3(Vec3::NEG_Z)
        .normalize_or_zero();

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
        if effective.move_dir.length() < 0.30 || *loco != LocomotionState::Grounded || m.cooldown > 0.0
        {
            return;
        }
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let Ok(dir) = Dir3::new(fwd) else {
            return;
        };
        let mut face: Option<(f32, Vec3, f32, Entity)> = None;
        for h in [0.35f32, 0.65, 0.95] {
            let o = origin + fwd * MANTLE_PROBE_SHIFT + Vec3::Y * (h - (origin.y - foot_y));
            if let Some(hit) = spatial.cast_ray(o, dir, MANTLE_PROBE_REACH, true, &filter) {
                let best = face.map(|(d, _, _, _)| d).unwrap_or(f32::INFINITY);
                if hit.distance < best {
                    let top_o = o + fwd * hit.distance + Vec3::Y * 1.2;
                    let top_hit = spatial.cast_ray(top_o, Dir3::NEG_Y, 1.8, true, &filter);
                    let (top_y, top_normal_y) = top_hit
                        .map(|t| (top_o.y - t.distance, t.normal.y))
                        .unwrap_or((f32::NEG_INFINITY, 0.0));
                    face = Some((hit.distance, hit.normal, top_y, top_hit.map(|t| t.entity).expect("top hit")));
                    let _ = top_normal_y;
                }
            }
        }
        let Some((face_distance, normal, top_y, ledge_entity)) = face else {
            return;
        };

        let top_normal = spatial
            .cast_ray(
                origin + fwd * (MANTLE_PROBE_SHIFT + face_distance) + Vec3::Y * 1.4,
                Dir3::NEG_Y,
                2.0,
                true,
                &filter,
            )
            .map(|t| t.normal.y)
            .unwrap_or(0.0);
        if top_normal < als_core::mantle::MANTLE_SLOPE_ANGLE_COS {
            return;
        }

        if let Ok(target_vel) = velocities.get(ledge_entity) {
            if target_vel.0.length() > als_core::mantle::MANTLE_TARGET_SPEED_THRESHOLD {
                return;
            }
        }

        let ledge = LedgeProbe {
            face_distance,
            top_height: top_y - foot_y,
            lateral_sign: normal.dot(right_of(fwd)),
        };
        let Some(kind) = classify_mantle(&ledge) else {
            return;
        };
        let face_plane_dist = MANTLE_PROBE_SHIFT + face_distance;
        let target = Vec3::new(
            origin.x + fwd.x * (face_plane_dist + MANTLE_TARGET_INSET),
            top_y + 0.92,
            origin.z + fwd.z * (face_plane_dist + MANTLE_TARGET_INSET),
        );
        let wall_dir = Vec3::new(-normal.x, 0.0, -normal.z).normalize_or_zero();
        let yaw_target = yaw_from_direction(if wall_dir.length_squared() > 1e-4 {
            wall_dir
        } else {
            fwd
        });
        let headroom_o = Vec3::new(target.x, top_y + 0.30, target.z);
        let ceiling = spatial
            .cast_ray(
                headroom_o,
                Dir3::Y,
                als_core::mantle::MANTLE_HEADROOM_CLEARANCE,
                true,
                &filter,
            );
        if ceiling.is_some() {
            return;
        }
        let start_clearance = spatial.cast_ray(
            origin + Vec3::Y * 1.2,
            Dir3::Y,
            0.9,
            true,
            &filter,
        );
        if start_clearance.is_some() {
            return;
        }
        let yaw_start = m_direction_yaw(gtf);
        m.begin(kind, origin, target, ledge.top_height, yaw_start, yaw_target);
        info!(
            "[MANTLE] begin kind={kind:?} top_h={:.2} dist={:.2} offset={:.3} scale={:.3}",
            ledge.top_height, face_distance, m.start_offset, m.scale
        );
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

fn m_direction_yaw(gtf: &GlobalTransform) -> f32 {
    let f = gtf.forward();
    f32::atan2(f.x, -f.z)
}

fn right_of(fwd: Vec3) -> Vec3 {
    Vec3::Y.cross(fwd).normalize_or_zero()
}
