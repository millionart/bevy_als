use als_core::character::AlsCharacter;
use als_core::foot_ik::AlsFootProbe;
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

pub struct AlsDebugDrawPlugin;

impl Plugin for AlsDebugDrawPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (draw_velocity, draw_foot_ik_targets, draw_bone_links),
        );
    }
}

fn draw_velocity(
    q: Query<(&Transform, &LinearVelocity), With<AlsCharacter>>,
    mut gizmos: Gizmos,
) {
    let Ok((tf, vel)) = q.single() else {
        return;
    };
    let speed = Vec2::new(vel.x, vel.z).length();
    let radius = 0.12 + (speed * 0.02).min(0.15);
    let color = Color::srgb(0.2, (1.0 - speed * 0.08).max(0.1), 0.9);
    gizmos.sphere(tf.translation, radius, color);

    if speed > 0.05 {
        let dir = Vec3::new(vel.x, 0.0, vel.z) / speed;
        let end = tf.translation + dir * (0.4 + speed * 0.12);
        gizmos.arrow(tf.translation + Vec3::Y * 0.05, end, Color::srgb(1.0, 0.85, 0.1));
    }
}

fn draw_foot_ik_targets(
    q: Query<
        (
            &als_core::foot_ik::AlsFootBones,
            &AlsFootProbe,
        ),
        With<AlsCharacter>,
    >,
    bone_globals: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
) {
    let Ok((bones, probe)) = q.single() else {
        return;
    };
    for side in 0..2usize {
        let hit = match side {
            0 => probe.left,
            _ => probe.right,
        };
        let Some(hit) = hit else {
            continue;
        };
        let chain = bones.chain(side);
        let Ok(ankle) = bone_globals.get(chain.foot) else {
            continue;
        };
        let ankle_pos = ankle.translation();
        let color = Color::srgb(0.2, 1.0, 0.35);
        gizmos.line(ankle_pos, hit.point, color);
        gizmos.sphere(hit.point, 0.035, color);
    }
}

fn draw_bone_links(
    q: Query<&GlobalTransform, With<AlsCharacter>>,
    mut gizmos: Gizmos,
) {
    let Ok(gtf) = q.single() else {
        return;
    };
    let origin = gtf.translation();
    let top = origin + Vec3::Y * 0.55;
    gizmos.line(origin + Vec3::Y * 0.2, top, Color::srgb(1.0, 1.0, 1.0));
}
