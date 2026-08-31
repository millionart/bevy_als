use std::sync::Arc;

use als_ext::{
    RagdollBodyPhysicsProfile, RagdollColliderPhysicsProfile, RagdollJointPhysicsProfile,
    RagdollPhysicsProfile,
};
use avian3d::prelude::AngleLimit;
use bevy::math::Isometry3d;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::ragdoll::definition::BodyMode;
use serde::Deserialize;

use crate::character_assembly::AlsCharacterAvatar;

const PA_ALS_PROFILE: &str = include_str!("../assets/ragdolls/pa_als.physics.json");

#[derive(Resource, Clone)]
pub struct PaAlsPhysicsProfileTemplate(pub RagdollPhysicsProfile);

impl FromWorld for PaAlsPhysicsProfileTemplate {
    fn from_world(_world: &mut World) -> Self {
        Self(parse_pa_als_profile())
    }
}

#[derive(Deserialize)]
struct PaAlsDocument {
    bodies: Vec<PaAlsBody>,
    constraints: Vec<PaAlsConstraint>,
    collision_disabled_pairs: Vec<[u8; 2]>,
}

#[derive(Deserialize)]
struct PaAlsBody {
    source_index: u8,
    bone_name: String,
    target: PaAlsBodyTarget,
    shapes: Vec<PaAlsShape>,
}

#[derive(Deserialize)]
struct PaAlsBodyTarget {
    active_mode: String,
    linear_damping: f32,
    angular_damping: f32,
    max_angular_speed_radians_per_second: f32,
    gravity_scale: f32,
}

#[derive(Deserialize)]
struct PaAlsShape {
    source_shape_index: u8,
    target: PaAlsShapeTarget,
}

#[derive(Deserialize)]
struct PaAlsShapeTarget {
    kind: String,
}

#[derive(Deserialize)]
struct PaAlsConstraint {
    joint_name: String,
    source_profile: PaAlsConstraintSourceProfile,
    target: PaAlsConstraintTarget,
}

#[derive(Deserialize)]
struct PaAlsConstraintSourceProfile {
    cone_damping: f32,
    twist_damping: f32,
    linear_drive_damping: f32,
}

#[derive(Deserialize)]
struct PaAlsConstraintTarget {
    frame_1: PaAlsFrame,
    frame_2: PaAlsFrame,
    angular_rotation_offset_xyzw: [f32; 4],
    twist_axis: [f32; 3],
    swing_limit_radians: Option<[f32; 2]>,
    twist_limit_radians: Option<[f32; 2]>,
    point_compliance: f32,
    swing_compliance: f32,
    twist_compliance: f32,
    calibrate_frame_2_to_initial_pose: bool,
    enabled: bool,
}

#[derive(Deserialize)]
struct PaAlsFrame {
    translation_m: [f32; 3],
    rotation_xyzw: [f32; 4],
}

pub struct PaAlsPhysicsPlugin;

impl Plugin for PaAlsPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaAlsPhysicsProfileTemplate>()
            .add_systems(Update, attach_pa_als_physics_profile);
    }
}

fn body_mode(value: &str) -> BodyMode {
    match value {
        "Dynamic" => BodyMode::Dynamic,
        "Kinematic" => BodyMode::Kinematic,
        _ => panic!("unsupported PA_Als body mode: {value}"),
    }
}

fn frame(value: PaAlsFrame) -> Isometry3d {
    Isometry3d::new(
        Vec3::from(value.translation_m),
        Quat::from_array(value.rotation_xyzw),
    )
}

fn collider_label(body: &str, shape: &PaAlsShape) -> String {
    format!("{body}_{}_{}", shape.source_shape_index, shape.target.kind)
}

pub fn parse_pa_als_profile() -> RagdollPhysicsProfile {
    let document: PaAlsDocument =
        serde_json::from_str(PA_ALS_PROFILE).expect("PA_Als physics profile parses");
    let bodies = document
        .bodies
        .iter()
        .map(|body| RagdollBodyPhysicsProfile {
            label: body.bone_name.clone(),
            source_index: body.source_index,
            active_mode: body_mode(&body.target.active_mode),
            linear_damping: body.target.linear_damping,
            angular_damping: body.target.angular_damping,
            max_angular_speed: body.target.max_angular_speed_radians_per_second,
            gravity_scale: body.target.gravity_scale,
        })
        .collect();
    let colliders = document
        .bodies
        .iter()
        .flat_map(|body| {
            body.shapes
                .iter()
                .map(|shape| RagdollColliderPhysicsProfile {
                    label: collider_label(&body.bone_name, shape),
                    source_body_index: body.source_index,
                })
        })
        .collect();
    let joints = document
        .constraints
        .into_iter()
        .map(|constraint| RagdollJointPhysicsProfile {
            label: constraint.joint_name,
            frame_1: frame(constraint.target.frame_1),
            frame_2: frame(constraint.target.frame_2),
            angular_rotation_offset: Quat::from_array(
                constraint.target.angular_rotation_offset_xyzw,
            ),
            twist_axis: Vec3::from(constraint.target.twist_axis),
            swing_limit: constraint
                .target
                .swing_limit_radians
                .map(|[min, max]| AngleLimit { min, max }),
            twist_limit: constraint
                .target
                .twist_limit_radians
                .map(|[min, max]| AngleLimit { min, max }),
            point_compliance: constraint.target.point_compliance,
            swing_compliance: constraint.target.swing_compliance,
            twist_compliance: constraint.target.twist_compliance,
            linear_damping: constraint.source_profile.linear_drive_damping,
            angular_damping: constraint
                .source_profile
                .cone_damping
                .max(constraint.source_profile.twist_damping),
            calibrate_frame_2_to_initial_pose: constraint.target.calibrate_frame_2_to_initial_pose,
            enabled: constraint.target.enabled,
        })
        .collect();
    let mut disabled_pairs: Vec<(u8, u8)> = document
        .collision_disabled_pairs
        .into_iter()
        .map(|[body_1, body_2]| (body_1, body_2))
        .collect();
    disabled_pairs.sort_unstable();
    RagdollPhysicsProfile {
        bodies,
        colliders,
        joints,
        collision_disabled_pairs: Arc::from(disabled_pairs.into_boxed_slice()),
    }
}

fn attach_pa_als_physics_profile(
    mut commands: Commands,
    avatars: Query<&AnimatedSceneInstance, With<AlsCharacterAvatar>>,
    profiles: Query<(), With<RagdollPhysicsProfile>>,
    template: Res<PaAlsPhysicsProfileTemplate>,
) {
    for instance in &avatars {
        let player = instance.player_entity();
        if profiles.contains(player) {
            continue;
        }
        commands.entity(player).insert(template.0.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pa_als_profile_has_full_source_topology() {
        let profile = parse_pa_als_profile();
        assert_eq!(profile.bodies.len(), 21);
        assert_eq!(profile.colliders.len(), 22);
        assert_eq!(profile.joints.len(), 20);
        assert_eq!(profile.collision_disabled_pairs.len(), 49);
        assert!(
            profile
                .bodies
                .iter()
                .any(|body| body.label == "root" && body.active_mode == BodyMode::Kinematic)
        );
        assert!(
            profile
                .joints
                .iter()
                .any(|joint| joint.label == "pelvis" && !joint.enabled)
        );
    }
}
