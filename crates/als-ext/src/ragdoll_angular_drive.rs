use std::collections::HashMap;

use avian3d::prelude::{
    AngularVelocity, Forces, PhysicsSystems, RevoluteJoint, RigidBody, Rotation, SphericalJoint,
    WriteRigidBodyForces,
};
use bevy::prelude::*;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::ragdoll::relative_kinematic_body::RelativeKinematicBodyPositionBased;
use bevy_animation_graph::core::ragdoll::spawning::SpawnedRagdoll;

use crate::ragdoll_physics_profile::RagdollPhysicsProfileSet;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct RagdollAngularDrive {
    pub torque_stiffness: f32,
    pub torque_damping: f32,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RagdollAngularDriveMetrics {
    pub target_joint_count: u32,
    pub driven_joint_count: u32,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RagdollAngularDriveSet;

pub struct RagdollAngularDrivePlugin;

impl Plugin for RagdollAngularDrivePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedPostUpdate,
            RagdollAngularDriveSet
                .after(AnimationGraphSet::PrePhysics)
                .after(RagdollPhysicsProfileSet)
                .before(PhysicsSystems::First),
        )
        .add_systems(
            FixedPostUpdate,
            apply_ragdoll_angular_drive.in_set(RagdollAngularDriveSet),
        );
    }
}

pub fn angular_drive_torque(
    parent_rotation: Quat,
    child_rotation: Quat,
    parent_angular_velocity: Vec3,
    child_angular_velocity: Vec3,
    target_parent_rotation: Quat,
    target_child_rotation: Quat,
    torque_stiffness: f32,
    torque_damping: f32,
) -> Vec3 {
    if (!torque_stiffness.is_finite() || torque_stiffness <= 0.0)
        && (!torque_damping.is_finite() || torque_damping <= 0.0)
    {
        return Vec3::ZERO;
    }
    let current_relative = (parent_rotation.conjugate() * child_rotation).normalize();
    let target_relative = (target_parent_rotation.conjugate() * target_child_rotation).normalize();
    let delta = (target_relative * current_relative.conjugate()).normalize();
    let (axis, angle) = delta.to_axis_angle();
    if !axis.is_finite() || !angle.is_finite() {
        return Vec3::ZERO;
    }
    let spring = if torque_stiffness > 0.0 {
        parent_rotation * axis * (angle * torque_stiffness)
    } else {
        Vec3::ZERO
    };
    let damping = if torque_damping > 0.0 {
        -(child_angular_velocity - parent_angular_velocity) * torque_damping
    } else {
        Vec3::ZERO
    };
    spring + damping
}

fn target_world_rotation(
    target: &RelativeKinematicBodyPositionBased,
    rotations: &Query<&Rotation>,
) -> Option<Quat> {
    match target.relative_to {
        Some(base) => rotations
            .get(base)
            .ok()
            .map(|rotation| (rotation.0 * target.relative_target.rotation).normalize()),
        None => Some(target.relative_target.rotation.normalize()),
    }
}

fn joint_bodies(
    spherical: Option<&SphericalJoint>,
    revolute: Option<&RevoluteJoint>,
) -> Option<(Entity, Entity)> {
    spherical
        .map(|joint| (joint.body1, joint.body2))
        .or_else(|| revolute.map(|joint| (joint.body1, joint.body2)))
}

#[allow(clippy::type_complexity)]
pub fn apply_ragdoll_angular_drive(
    mut commands: Commands,
    mut players: Query<(
        Entity,
        &AnimationGraphPlayer,
        &RagdollAngularDrive,
        Option<&mut RagdollAngularDriveMetrics>,
    )>,
    joints: Query<(Option<&SphericalJoint>, Option<&RevoluteJoint>)>,
    mut physics: ParamSet<(
        Query<(
            &Rotation,
            &RelativeKinematicBodyPositionBased,
            &RigidBody,
            Forces,
        )>,
        Query<&AngularVelocity>,
    )>,
    rotations: Query<&Rotation>,
) {
    for (player_entity, player, drive, metrics) in &mut players {
        let mut next_metrics = RagdollAngularDriveMetrics::default();
        if let Some(spawned) = player.get_spawned_ragdoll() {
            let angular_velocities = {
                let query = physics.p1();
                spawned
                    .bodies
                    .values()
                    .filter_map(|entity| query.get(*entity).ok().map(|value| (*entity, value.0)))
                    .collect::<HashMap<_, _>>()
            };
            let mut bodies = physics.p0();
            apply_drive_to_spawned_ragdoll(
                spawned,
                drive.torque_stiffness,
                drive.torque_damping,
                &joints,
                &mut bodies,
                &angular_velocities,
                &rotations,
                &mut next_metrics,
            );
        }
        if let Some(mut metrics) = metrics {
            *metrics = next_metrics;
        } else {
            commands.entity(player_entity).insert(next_metrics);
        }
    }
}

fn apply_drive_to_spawned_ragdoll(
    spawned: &SpawnedRagdoll,
    torque_stiffness: f32,
    torque_damping: f32,
    joints: &Query<(Option<&SphericalJoint>, Option<&RevoluteJoint>)>,
    bodies: &mut Query<(
        &Rotation,
        &RelativeKinematicBodyPositionBased,
        &RigidBody,
        Forces,
    )>,
    angular_velocities: &HashMap<Entity, Vec3>,
    rotations: &Query<&Rotation>,
    metrics: &mut RagdollAngularDriveMetrics,
) {
    for joint_entity in spawned.joints.values() {
        let Ok((spherical, revolute)) = joints.get(*joint_entity) else {
            continue;
        };
        let Some((body1, body2)) = joint_bodies(spherical, revolute) else {
            continue;
        };
        let (Some(angular_velocity1), Some(angular_velocity2)) = (
            angular_velocities.get(&body1),
            angular_velocities.get(&body2),
        ) else {
            continue;
        };
        let Ok(
            [
                (rotation1, target1, mode1, mut forces1),
                (rotation2, target2, mode2, mut forces2),
            ],
        ) = bodies.get_many_mut([body1, body2])
        else {
            continue;
        };
        if *mode1 != RigidBody::Dynamic || *mode2 != RigidBody::Dynamic {
            continue;
        }
        let (Some(target_rotation1), Some(target_rotation2)) = (
            target_world_rotation(target1, rotations),
            target_world_rotation(target2, rotations),
        ) else {
            continue;
        };
        metrics.target_joint_count += 1;
        let torque = angular_drive_torque(
            rotation1.0,
            rotation2.0,
            *angular_velocity1,
            *angular_velocity2,
            target_rotation1,
            target_rotation2,
            torque_stiffness,
            torque_damping,
        );
        if torque.length_squared() <= 1e-8 {
            continue;
        }
        forces1.apply_torque(-torque);
        forces2.apply_torque(torque);
        metrics.driven_joint_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angular_drive_uses_relative_rotation_error() {
        let torque = angular_drive_torque(
            Quat::IDENTITY,
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
            Quat::IDENTITY,
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            2.0,
            0.0,
        );
        let expected = std::f32::consts::FRAC_PI_2 * 2.0;
        assert!((torque.y - expected).abs() <= 1e-3);
        assert!(torque.x.abs() <= 1e-3);
        assert!(torque.z.abs() <= 1e-3);
    }
}
