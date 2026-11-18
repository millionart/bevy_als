use avian3d::prelude::{JointDisabled, PhysicsSystems, RigidBody, Rotation};
use bevy::prelude::*;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::ragdoll::relative_kinematic_body::RelativeKinematicBodyPositionBased;

use crate::extensions::{
    physics_asset::RagdollPhysicsProfileSet, source_physics_joint::SourcePhysicsJoint,
};

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct RagdollAngularDrive {
    pub stiffness: f32,
    pub damping: f32,
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
            PostUpdate,
            RagdollAngularDriveSet
                .after(AnimationGraphSet::PrePhysics)
                .after(RagdollPhysicsProfileSet)
                .before(PhysicsSystems::First),
        )
        .add_systems(
            PostUpdate,
            apply_ragdoll_angular_drive.in_set(RagdollAngularDriveSet),
        );
    }
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

pub fn apply_ragdoll_angular_drive(
    mut commands: Commands,
    mut players: Query<(
        Entity,
        &AnimationGraphPlayer,
        &RagdollAngularDrive,
        Option<&mut RagdollAngularDriveMetrics>,
    )>,
    mut joints: Query<&mut SourcePhysicsJoint, Without<JointDisabled>>,
    bodies: Query<(&RelativeKinematicBodyPositionBased, &RigidBody)>,
    rotations: Query<&Rotation>,
) {
    for (player_entity, player, drive, metrics) in &mut players {
        let mut next = RagdollAngularDriveMetrics::default();
        if let Some(spawned) = player.get_spawned_ragdoll() {
            for entity in spawned.joints.values() {
                let Ok(mut joint) = joints.get_mut(*entity) else {
                    continue;
                };
                let stiffness = drive.stiffness * joint.motor_stiffness_scale;
                let damping = drive.damping * joint.motor_damping_scale;
                for motor in &mut joint.motors {
                    motor.spring.stiffness = stiffness;
                    motor.spring.damping = damping;
                }
                let Ok([(parent_target, parent_mode), (child_target, child_mode)]) =
                    bodies.get_many([joint.parent, joint.child])
                else {
                    continue;
                };
                let (Some(parent_rotation), Some(child_rotation)) = (
                    target_world_rotation(parent_target, &rotations),
                    target_world_rotation(child_target, &rotations),
                ) else {
                    continue;
                };
                joint.motor_target = ((parent_rotation * joint.parent_frame.rotation).conjugate()
                    * (child_rotation * joint.child_frame.rotation))
                    .normalize();
                if *parent_mode != RigidBody::Dynamic || *child_mode != RigidBody::Dynamic {
                    continue;
                }
                next.target_joint_count += 1;
                if stiffness <= 0.0 && damping <= 0.0 {
                    continue;
                }
                let Ok([parent, child]) = rotations.get_many([joint.parent, joint.child]) else {
                    continue;
                };
                let current = (parent.0 * joint.parent_frame.rotation).conjugate()
                    * (child.0 * joint.child_frame.rotation);
                if current.angle_between(joint.motor_target) > 1e-4 {
                    next.driven_joint_count += 1;
                }
            }
        }
        if let Some(mut metrics) = metrics {
            *metrics = next;
        } else {
            commands.entity(player_entity).insert(next);
        }
    }
}
