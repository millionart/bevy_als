use als_core::input::AlsInputCommand;
use als_core::motion::AlsInputOverride;
use als_core::mantle::{
    classify_mantle, mantle_motion_at, mantle_weight_at, AlsMantleState, LedgeProbe,
    MantleKind, MANTLE_PROBE_REACH,
};
use als_core::state::{LocomotionState, Stance};
use avian3d::prelude::{LinearVelocity, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

pub struct AlsMantleDrivePlugin;

impl Plugin for AlsMantleDrivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                ensure_mantle_state,
                mantle_drive_system.after(ensure_mantle_state),
            ),
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

#[allow(clippy::type_complexity)]
fn mantle_drive_system(
    time: Res<Time>,
    spatial: SpatialQuery,
    mut q: Query<
        (
            Entity,
            &mut AlsMantleState,
            &mut Transform,
            &mut LinearVelocity,
            &GlobalTransform,
            &LocomotionState,
            &Stance,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    let Ok((entity, mut m, mut tf, mut vel, gtf, loco, stance, cmd, ovr)) = q.single_mut() else {
        return;
    };
    let effective = ovr.map(|o| &o.0).unwrap_or(cmd);
    let origin = gtf.translation();
    let foot_y = origin.y - 0.90;
    let fwd = Quat::from_rotation_y(m_direction_yaw(gtf))
        .mul_vec3(Vec3::NEG_Z)
        .normalize_or_zero();

    m.tick(effective, loco, stance, dt);

    if !m.active {
        if effective.move_dir.length() < 0.30 || *loco != LocomotionState::Grounded || m.cooldown > 0.0 {
            return;
        }
        let probe_origin_base = origin + Vec3::Y * 0.1;
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let Ok(dir) = Dir3::new(fwd) else {
            return;
        };
        let mut face: Option<(f32, Vec3, f32)> = None;
        for h in [0.35f32, 0.65, 0.95] {
            let o = origin + fwd * 0.45 + Vec3::Y * (h - (origin.y - foot_y));
            let _ = probe_origin_base;
            if let Some(hit) = spatial.cast_ray(o, dir, MANTLE_PROBE_REACH, true, &filter) {
                let best = face.map(|(d, _, _)| d).unwrap_or(f32::INFINITY);
                if hit.distance < best {
                    let top_o = o + fwd * hit.distance + Vec3::Y * 1.2;
                    let top_hit = spatial.cast_ray(top_o, Dir3::NEG_Y, 1.8, true, &filter);
                    let top_y = top_hit
                        .map(|t| top_o.y - t.distance)
                        .unwrap_or(f32::NEG_INFINITY);
                    face = Some((hit.distance, hit.normal, top_y));
                }
            }
        }
        let Some((face_distance, normal, top_y)) = face else {
            return;
        };

        let ledge = LedgeProbe {
            face_distance,
            top_height: top_y - foot_y,
            lateral_sign: normal.dot(right_of(fwd)),
        };
        let Some(kind) = classify_mantle(&ledge) else {
            return;
        };
        let target = Vec3::new(
            origin.x + fwd.x * (face_distance + 0.45),
            top_y + 0.92,
            origin.z + fwd.z * (face_distance + 0.45),
        );
        m.begin(kind, origin, target);
        info!(
            "[MANTLE] begin kind={kind:?} top_h={:.2} dist={:.2}",
            ledge.top_height, ledge.face_distance
        );
    }

    let t_norm = (m.time / m.duration).clamp(0.0, 1.0);
    let motion = mantle_motion_at(t_norm);
    let delta = m.target - m.start;
    let desired = Vec3::new(
        m.start.x + delta.x * motion.forward_frac,
        m.start.y + delta.y * motion.up_frac,
        m.start.z + delta.z * motion.forward_frac,
    );
    tf.translation = desired;
    *vel = LinearVelocity(Vec3::ZERO);

    let weight = mantle_weight_at(m.time, m.duration);
    let (high_w, lr) = match m.kind {
        MantleKind::High => (1.0, 0.0),
        MantleKind::LowLeft => (0.0, 0.0),
        MantleKind::LowRight => (0.0, 1.0),
    };
    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("mantle_weight", DataValue::from(weight));
            player.set_input_data("mantle_high_w", DataValue::from(high_w));
            player.set_input_data("mantle_lr", DataValue::from(lr));
        }
    }
}

fn m_direction_yaw(gtf: &GlobalTransform) -> f32 {
    let f = gtf.forward();
    f32::atan2(f.x, -f.z)
}

fn right_of(fwd: Vec3) -> Vec3 {
    Vec3::Y.cross(fwd).normalize_or_zero()
}

