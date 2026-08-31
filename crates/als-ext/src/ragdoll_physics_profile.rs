use std::sync::Arc;

use avian3d::prelude::{
    ActiveCollisionHooks, AngleLimit, AngularDamping, CollisionHooks, GravityScale, JointDamping,
    JointDisabled, LinearDamping, MaxAngularSpeed, PhysicsSystems, Position, Rotation,
    SphericalJoint,
};
use bevy::ecs::schedule::ApplyDeferred;
use bevy::ecs::system::SystemParam;
use bevy::math::Isometry3d;
use bevy::prelude::*;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::ragdoll::definition::{
    BodyLabel, BodyMode, ColliderLabel, JointLabel,
};
use bevy_animation_graph::core::ragdoll::spawning::SpawnedRagdoll;

#[derive(Clone, Debug)]
pub struct RagdollBodyPhysicsProfile {
    pub label: String,
    pub source_index: u8,
    pub active_mode: BodyMode,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub max_angular_speed: f32,
    pub gravity_scale: f32,
}

#[derive(Clone, Debug)]
pub struct RagdollColliderPhysicsProfile {
    pub label: String,
    pub source_body_index: u8,
}

#[derive(Clone, Debug)]
pub struct RagdollJointPhysicsProfile {
    pub label: String,
    pub frame_1: Isometry3d,
    pub frame_2: Isometry3d,
    pub angular_rotation_offset: Quat,
    pub twist_axis: Vec3,
    pub swing_limit: Option<AngleLimit>,
    pub twist_limit: Option<AngleLimit>,
    pub point_compliance: f32,
    pub swing_compliance: f32,
    pub twist_compliance: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub calibrate_frame_2_to_initial_pose: bool,
    pub enabled: bool,
}

#[derive(Component, Clone, Debug)]
pub struct RagdollPhysicsProfile {
    pub bodies: Vec<RagdollBodyPhysicsProfile>,
    pub colliders: Vec<RagdollColliderPhysicsProfile>,
    pub joints: Vec<RagdollJointPhysicsProfile>,
    pub collision_disabled_pairs: Arc<[(u8, u8)]>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RagdollActiveMode(pub BodyMode);

#[derive(Component, Clone, Debug)]
pub struct RagdollCollisionMember {
    pub group: Entity,
    pub body_index: u8,
    pub disabled_pairs: Arc<[(u8, u8)]>,
}

impl RagdollCollisionMember {
    pub fn disables(&self, other: &Self) -> bool {
        if self.group != other.group {
            return false;
        }
        let pair = if self.body_index <= other.body_index {
            (self.body_index, other.body_index)
        } else {
            (other.body_index, self.body_index)
        };
        self.disabled_pairs.binary_search(&pair).is_ok()
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct RagdollPhysicsProfileApplied {
    pub body_count: u32,
    pub collider_count: u32,
    pub joint_count: u32,
    pub collision_disabled_pair_count: u32,
    pub max_anchor_error_m: f32,
    pub max_calibrated_anchor_error_m: f32,
}

#[derive(SystemParam)]
pub struct RagdollCollisionHooks<'w, 's> {
    members: Query<'w, 's, &'static RagdollCollisionMember>,
}

impl CollisionHooks for RagdollCollisionHooks<'_, '_> {
    fn filter_pairs(
        &self,
        collider_1: Entity,
        collider_2: Entity,
        _commands: &mut Commands,
    ) -> bool {
        let Ok([member_1, member_2]) = self.members.get_many([collider_1, collider_2]) else {
            return true;
        };
        !member_1.disables(member_2)
    }
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RagdollPhysicsProfileSet;

pub struct RagdollPhysicsProfilePlugin;

impl Plugin for RagdollPhysicsProfilePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedPostUpdate,
            RagdollPhysicsProfileSet
                .after(AnimationGraphSet::PrePhysics)
                .before(PhysicsSystems::First),
        )
        .add_systems(
            FixedPostUpdate,
            (apply_ragdoll_physics_profiles, ApplyDeferred)
                .chain()
                .in_set(RagdollPhysicsProfileSet),
        );
    }
}

fn find_body_profile<'a>(
    profile: &'a RagdollPhysicsProfile,
    label: &str,
) -> Option<&'a RagdollBodyPhysicsProfile> {
    profile
        .bodies
        .iter()
        .find(|candidate| candidate.label == label)
}

fn find_collider_profile<'a>(
    profile: &'a RagdollPhysicsProfile,
    label: &str,
) -> Option<&'a RagdollColliderPhysicsProfile> {
    profile
        .colliders
        .iter()
        .find(|candidate| candidate.label == label)
}

fn find_joint_profile<'a>(
    profile: &'a RagdollPhysicsProfile,
    label: &str,
) -> Option<&'a RagdollJointPhysicsProfile> {
    profile
        .joints
        .iter()
        .find(|candidate| candidate.label == label)
}

#[allow(clippy::type_complexity)]
fn apply_ragdoll_physics_profiles(
    mut commands: Commands,
    players: Query<
        (Entity, &AnimationGraphPlayer, &RagdollPhysicsProfile),
        Without<RagdollPhysicsProfileApplied>,
    >,
    body_labels: Query<&BodyLabel>,
    collider_labels: Query<&ColliderLabel>,
    joint_labels: Query<&JointLabel>,
    spherical_joints: Query<&SphericalJoint>,
    body_transforms: Query<(&Position, &Rotation)>,
) {
    for (player_entity, player, profile) in &players {
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        let applied = apply_profile_to_spawned_ragdoll(
            &mut commands,
            spawned,
            profile,
            &body_labels,
            &collider_labels,
            &joint_labels,
            &spherical_joints,
            &body_transforms,
        );
        commands.entity(player_entity).insert(applied);
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_profile_to_spawned_ragdoll(
    commands: &mut Commands,
    spawned: &SpawnedRagdoll,
    profile: &RagdollPhysicsProfile,
    body_labels: &Query<&BodyLabel>,
    collider_labels: &Query<&ColliderLabel>,
    joint_labels: &Query<&JointLabel>,
    spherical_joints: &Query<&SphericalJoint>,
    body_transforms: &Query<(&Position, &Rotation)>,
) -> RagdollPhysicsProfileApplied {
    let mut applied = RagdollPhysicsProfileApplied {
        collision_disabled_pair_count: profile.collision_disabled_pairs.len() as u32,
        ..default()
    };
    for body_entity in spawned.bodies.values() {
        let Ok(label) = body_labels.get(*body_entity) else {
            continue;
        };
        let Some(body) = find_body_profile(profile, &label.0) else {
            continue;
        };
        commands.entity(*body_entity).insert((
            RagdollActiveMode(body.active_mode),
            LinearDamping(body.linear_damping),
            AngularDamping(body.angular_damping),
            MaxAngularSpeed(body.max_angular_speed),
            GravityScale(body.gravity_scale),
        ));
        applied.body_count += 1;
    }
    for collider_entity in spawned.colliders.values() {
        let Ok(label) = collider_labels.get(*collider_entity) else {
            continue;
        };
        let Some(collider) = find_collider_profile(profile, &label.0) else {
            continue;
        };
        commands.entity(*collider_entity).insert((
            ActiveCollisionHooks::FILTER_PAIRS,
            RagdollCollisionMember {
                group: spawned.root,
                body_index: collider.source_body_index,
                disabled_pairs: profile.collision_disabled_pairs.clone(),
            },
        ));
        applied.collider_count += 1;
    }
    for joint_entity in spawned.joints.values() {
        let Ok(label) = joint_labels.get(*joint_entity) else {
            continue;
        };
        let Some(joint_profile) = find_joint_profile(profile, &label.0) else {
            continue;
        };
        let Ok(existing) = spherical_joints.get(*joint_entity) else {
            continue;
        };
        let mut joint = existing.clone();
        let mut frame_2 = joint_profile.frame_2;
        if let Ok([(position_1, rotation_1), (position_2, rotation_2)]) =
            body_transforms.get_many([existing.body1, existing.body2])
        {
            let anchor_1 =
                position_1.0 + rotation_1.0 * Vec3::from(joint_profile.frame_1.translation);
            frame_2.rotation *= joint_profile.angular_rotation_offset;
            let anchor_2 = position_2.0 + rotation_2.0 * Vec3::from(frame_2.translation);
            applied.max_anchor_error_m =
                applied.max_anchor_error_m.max(anchor_1.distance(anchor_2));
            if joint_profile.calibrate_frame_2_to_initial_pose {
                frame_2.translation = (rotation_2.0.conjugate() * (anchor_1 - position_2.0)).into();
                frame_2.rotation = (rotation_2.0.conjugate()
                    * rotation_1.0
                    * joint_profile.frame_1.rotation
                    * joint_profile.angular_rotation_offset)
                    .normalize();
            }
            let calibrated_anchor_2 = position_2.0 + rotation_2.0 * Vec3::from(frame_2.translation);
            applied.max_calibrated_anchor_error_m = applied
                .max_calibrated_anchor_error_m
                .max(anchor_1.distance(calibrated_anchor_2));
        } else {
            frame_2.rotation *= joint_profile.angular_rotation_offset;
        }
        joint.frame1 = avian3d::prelude::JointFrame::local(joint_profile.frame_1);
        joint.frame2 = avian3d::prelude::JointFrame::local(frame_2);
        joint.twist_axis = joint_profile.twist_axis;
        joint.swing_limit = joint_profile.swing_limit;
        joint.twist_limit = joint_profile.twist_limit;
        joint.point_compliance = joint_profile.point_compliance;
        joint.swing_compliance = joint_profile.swing_compliance;
        joint.twist_compliance = joint_profile.twist_compliance;
        commands.entity(*joint_entity).insert((
            joint,
            JointDamping {
                linear: joint_profile.linear_damping,
                angular: joint_profile.angular_damping,
            },
        ));
        if !joint_profile.enabled {
            commands.entity(*joint_entity).insert(JointDisabled);
        }
        applied.joint_count += 1;
    }
    applied
}
