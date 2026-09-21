use std::sync::Arc;

use avian3d::prelude::{
    ActiveCollisionHooks, AngularDamping, CoefficientCombine, CollisionHooks, Friction,
    GravityScale, JointDamping, JointDisabled, LinearDamping, MaxAngularSpeed, PhysicsSystems,
    Position, Restitution, RevoluteJoint, Rotation, SphericalJoint,
};
use bevy::ecs::schedule::ApplyDeferred;
use bevy::ecs::system::SystemParam;
use bevy::math::Isometry3d;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::{
    AnimatedScene, AnimatedSceneHandle, AnimatedSceneInstance,
};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::ragdoll::bone_mapping::RagdollBoneMap;
use bevy_animation_graph::core::ragdoll::definition::{
    BodyLabel, BodyMode, ColliderLabel, JointLabel,
};
use bevy_animation_graph::core::ragdoll::spawning::SpawnedRagdoll;
use serde::Deserialize;

use crate::{
    ragdoll_angular_drive::RagdollAngularDriveSet,
    source_physics_joint::{
        SourceAngularLimit, SourceAngularMotor, SourceAngularSpring, SourceJointSolverSettings,
        SourcePhysicsJoint,
    },
};

#[derive(Clone, Debug)]
pub struct RagdollBodyPhysicsProfile {
    pub label: String,
    pub source_index: u8,
    pub active_mode: BodyMode,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub max_angular_speed: f32,
    pub gravity_scale: f32,
    pub friction: Friction,
    pub restitution: Restitution,
}

#[derive(Clone, Debug)]
pub struct RagdollColliderPhysicsProfile {
    pub label: String,
    pub source_body_index: u8,
    pub consider_for_bounds: bool,
}

#[derive(Clone, Debug)]
pub struct RagdollJointPhysicsProfile {
    pub label: String,
    pub definition: SourcePhysicsJoint,
    pub enabled: bool,
}

#[derive(Component, Clone, Debug)]
pub struct RagdollPhysicsProfile {
    pub bodies: Vec<RagdollBodyPhysicsProfile>,
    pub colliders: Vec<RagdollColliderPhysicsProfile>,
    pub joints: Vec<RagdollJointPhysicsProfile>,
    pub collision_disabled_pairs: Arc<[(u8, u8)]>,
}

#[derive(Deserialize)]
struct PhysicsAssetDocument {
    format: String,
    bodies: Vec<PhysicsAssetBody>,
    constraints: Vec<PhysicsAssetConstraint>,
    collision_disabled_pairs: Vec<[u8; 2]>,
    target_solver: SourceJointSolverSettings,
}

#[derive(Deserialize)]
struct PhysicsAssetBody {
    source_index: u8,
    bone_name: String,
    source_properties: PhysicsAssetBounds,
    target: PhysicsAssetBodyTarget,
    shapes: Vec<PhysicsAssetShape>,
}

#[derive(Deserialize)]
struct PhysicsAssetBounds {
    #[serde(rename = "bConsiderForBounds")]
    consider_for_bounds: String,
}

#[derive(Deserialize)]
struct PhysicsAssetBodyTarget {
    active_mode: String,
    linear_damping: f32,
    angular_damping: f32,
    max_angular_speed_radians_per_second: f32,
    gravity_scale: f32,
    material: PhysicsAssetMaterial,
}

#[derive(Deserialize)]
struct PhysicsAssetMaterial {
    static_friction: f32,
    dynamic_friction: f32,
    restitution: f32,
    friction_combine: String,
    restitution_combine: String,
}

#[derive(Deserialize)]
struct PhysicsAssetShape {
    source_shape_index: u8,
    target: PhysicsAssetShapeTarget,
}

#[derive(Deserialize)]
struct PhysicsAssetShapeTarget {
    kind: String,
}

#[derive(Deserialize)]
struct PhysicsAssetConstraint {
    joint_name: String,
    target: PhysicsAssetConstraintTarget,
}

#[derive(Deserialize)]
struct PhysicsAssetConstraintTarget {
    frame_1: PhysicsAssetFrame,
    frame_2: PhysicsAssetFrame,
    angular_limits: [SourceAngularLimit; 3],
    swing_spring: Option<SourceAngularSpring>,
    twist_spring: Option<SourceAngularSpring>,
    drives: [SourceAngularMotor; 3],
    drive_target_xyzw: [f32; 4],
    angular_drive_stiffness_scale: f32,
    angular_drive_damping_scale: f32,
    linear_locked: bool,
    mass_conditioning: bool,
    projection_linear_alpha: f32,
    projection_linear_tolerance_m: f32,
    enabled: bool,
}

#[derive(Deserialize)]
struct PhysicsAssetFrame {
    translation_m: [f32; 3],
    rotation_xyzw: [f32; 4],
}

impl From<PhysicsAssetFrame> for Isometry3d {
    fn from(frame: PhysicsAssetFrame) -> Self {
        Isometry3d::new(
            Vec3::from_array(frame.translation_m),
            Quat::from_array(frame.rotation_xyzw),
        )
    }
}

fn coefficient_combine(value: &str) -> Result<CoefficientCombine, String> {
    match value {
        "Average" => Ok(CoefficientCombine::Average),
        "Min" => Ok(CoefficientCombine::Min),
        "Multiply" => Ok(CoefficientCombine::Multiply),
        "Max" => Ok(CoefficientCombine::Max),
        _ => Err(format!(
            "unsupported physics material combine mode: {value}"
        )),
    }
}

impl RagdollPhysicsProfile {
    pub fn from_json(text: &str) -> Result<Self, String> {
        let document: PhysicsAssetDocument =
            serde_json::from_str(text).map_err(|error| error.to_string())?;
        if document.format != "physics_asset_runtime_v1" {
            return Err(format!(
                "unsupported physics profile format: {}",
                document.format
            ));
        }
        let bodies = document
            .bodies
            .iter()
            .map(|body| {
                let material = &body.target.material;
                Ok(RagdollBodyPhysicsProfile {
                    label: body.bone_name.clone(),
                    source_index: body.source_index,
                    active_mode: match body.target.active_mode.as_str() {
                        "Dynamic" => BodyMode::Dynamic,
                        "Kinematic" => BodyMode::Kinematic,
                        value => return Err(format!("unsupported physics body mode: {value}")),
                    },
                    linear_damping: body.target.linear_damping,
                    angular_damping: body.target.angular_damping,
                    max_angular_speed: body.target.max_angular_speed_radians_per_second,
                    gravity_scale: body.target.gravity_scale,
                    friction: Friction {
                        static_coefficient: material.static_friction,
                        dynamic_coefficient: material.dynamic_friction,
                        combine_rule: coefficient_combine(&material.friction_combine)?,
                    },
                    restitution: Restitution {
                        coefficient: material.restitution,
                        combine_rule: coefficient_combine(&material.restitution_combine)?,
                    },
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let colliders = document
            .bodies
            .iter()
            .flat_map(|body| {
                body.shapes
                    .iter()
                    .map(|shape| RagdollColliderPhysicsProfile {
                        label: format!(
                            "{}_{}_{}",
                            body.bone_name, shape.source_shape_index, shape.target.kind
                        ),
                        source_body_index: body.source_index,
                        consider_for_bounds: body.source_properties.consider_for_bounds == "True",
                    })
            })
            .collect();
        let joints = document
            .constraints
            .into_iter()
            .map(|constraint| {
                let target = constraint.target;
                RagdollJointPhysicsProfile {
                    label: constraint.joint_name,
                    enabled: target.enabled,
                    definition: SourcePhysicsJoint {
                        parent: Entity::PLACEHOLDER,
                        child: Entity::PLACEHOLDER,
                        parent_frame: target.frame_2.into(),
                        child_frame: target.frame_1.into(),
                        angular_limits: target.angular_limits,
                        swing_spring: target.swing_spring,
                        twist_spring: target.twist_spring,
                        motors: target.drives,
                        motor_target: Quat::from_array(target.drive_target_xyzw),
                        motor_stiffness_scale: target.angular_drive_stiffness_scale,
                        motor_damping_scale: target.angular_drive_damping_scale,
                        linear_locked: target.linear_locked,
                        mass_conditioning: target.mass_conditioning,
                        projection_linear_alpha: target.projection_linear_alpha,
                        projection_linear_tolerance_m: target.projection_linear_tolerance_m,
                        settings: document.target_solver,
                    },
                }
            })
            .collect();
        let mut disabled_pairs = document
            .collision_disabled_pairs
            .into_iter()
            .map(|[a, b]| (a.min(b), a.max(b)))
            .collect::<Vec<_>>();
        disabled_pairs.sort_unstable();
        Ok(Self {
            bodies,
            colliders,
            joints,
            collision_disabled_pairs: disabled_pairs.into(),
        })
    }
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
    pub source_joint_count: u32,
    pub collision_disabled_pair_count: u32,
    pub current_joint_anchor_error_m: f32,
    pub current_joint_anchor_profile_index: Option<u8>,
    pub max_joint_anchor_error_m: f32,
    pub max_joint_anchor_profile_index: Option<u8>,
    pub handoff_locked_joint_anchor_error_m: f32,
    pub handoff_locked_joint_anchor_profile_index: Option<u8>,
    pub handoff_locked_joint_anchor_pending: bool,
    pub handoff_animated_body_count: u32,
    pub handoff_animated_body_position_error_m: f32,
    pub handoff_animated_body_rotation_error_radians: f32,
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

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RagdollPhysicsHandoffSet;

pub struct RagdollPhysicsProfilePlugin;

impl Plugin for RagdollPhysicsProfilePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            (
                RagdollPhysicsProfileSet
                    .after(AnimationGraphSet::PrePhysics)
                    .before(PhysicsSystems::First),
                RagdollPhysicsHandoffSet
                    .after(RagdollPhysicsProfileSet)
                    .before(PhysicsSystems::First),
            ),
        )
        .add_systems(
            PostUpdate,
            (apply_ragdoll_physics_profiles, ApplyDeferred)
                .chain()
                .in_set(RagdollPhysicsProfileSet),
        )
        .add_systems(
            PostUpdate,
            observe_ragdoll_joint_anchor_error.after(PhysicsSystems::Last),
        )
        .add_systems(
            PostUpdate,
            observe_ragdoll_handoff_anchor_error
                .after(RagdollPhysicsHandoffSet)
                .before(RagdollAngularDriveSet)
                .before(PhysicsSystems::First),
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
    revolute_joints: Query<&RevoluteJoint>,
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
            &revolute_joints,
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
    revolute_joints: &Query<&RevoluteJoint>,
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
            body.friction,
            body.restitution,
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
        let bodies = spherical_joints
            .get(*joint_entity)
            .ok()
            .map(|joint| (joint.body1, joint.body2))
            .or_else(|| {
                revolute_joints
                    .get(*joint_entity)
                    .ok()
                    .map(|joint| (joint.body1, joint.body2))
            });
        let Some((child, parent)) = bodies else {
            continue;
        };
        let mut joint = joint_profile.definition.clone();
        joint.parent = parent;
        joint.child = child;
        commands
            .entity(*joint_entity)
            .remove::<(SphericalJoint, RevoluteJoint, JointDamping)>()
            .insert(joint);
        if joint_profile.enabled {
            applied.source_joint_count += 1;
        } else {
            commands.entity(*joint_entity).insert(JointDisabled);
        }
        applied.joint_count += 1;
    }
    commands.entity(spawned.root).insert_children(0, &spawned.joints.values().copied().collect::<Vec<_>>());
    applied
}

fn observe_ragdoll_joint_anchor_error(
    mut players: Query<(
        &AnimationGraphPlayer,
        &RagdollPhysicsProfile,
        &mut RagdollPhysicsProfileApplied,
    )>,
    joint_labels: Query<&JointLabel>,
    joints: Query<&SourcePhysicsJoint>,
    body_transforms: Query<(&Position, &Rotation)>,
) {
    for (player, source_profile, mut profile) in &mut players {
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        let current = locked_joint_anchor_error(
            spawned,
            source_profile,
            &joint_labels,
            &joints,
            &body_transforms,
        );
        let Some((index, error)) = current else {
            profile.current_joint_anchor_error_m = 0.0;
            profile.current_joint_anchor_profile_index = None;
            continue;
        };
        profile.current_joint_anchor_error_m = error;
        profile.current_joint_anchor_profile_index = u8::try_from(index).ok();
        if error > profile.max_joint_anchor_error_m {
            profile.max_joint_anchor_error_m = error;
            profile.max_joint_anchor_profile_index = u8::try_from(index).ok();
        }
    }
}

fn observe_ragdoll_handoff_anchor_error(
    mut players: Query<(
        Entity,
        &AnimationGraphPlayer,
        &RagdollPhysicsProfile,
        &mut RagdollPhysicsProfileApplied,
    )>,
    joint_labels: Query<&JointLabel>,
    joints: Query<&SourcePhysicsJoint>,
    body_transforms: Query<(&Position, &Rotation)>,
    scenes: Query<(&AnimatedSceneHandle, &AnimatedSceneInstance)>,
    scene_assets: Option<Res<Assets<AnimatedScene>>>,
    maps: Option<Res<Assets<RagdollBoneMap>>>,
    bone_globals: Query<&GlobalTransform>,
) {
    for (entity, player, source_profile, mut profile) in &mut players {
        if !profile.handoff_locked_joint_anchor_pending {
            continue;
        }
        profile.handoff_locked_joint_anchor_pending = false;
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        profile.handoff_animated_body_count = 0;
        profile.handoff_animated_body_position_error_m = 0.0;
        profile.handoff_animated_body_rotation_error_radians = 0.0;
        let mapping = scenes
            .iter()
            .find(|(_, instance)| instance.player_entity() == entity)
            .and_then(|(handle, _)| scene_assets.as_ref()?.get(&handle.handle))
            .and_then(|scene| maps.as_ref()?.get(scene.ragdoll_bone_map.as_ref()?));
        if let Some(mapping) = mapping {
            for (id, body) in &spawned.bodies {
                let Some(mapping) = mapping.bodies_from_bones.get(id) else {
                    continue;
                };
                let Some(bone) = player.bone_entity(mapping.bone.bone.id()) else {
                    continue;
                };
                let (Ok(global), Ok((position, rotation))) =
                    (bone_globals.get(bone), body_transforms.get(*body))
                else {
                    continue;
                };
                let expected = global.to_isometry() * mapping.bone.offset;
                let (position_error, rotation_error) =
                    body_pose_error(expected, Isometry3d::new(position.0, rotation.0));
                profile.handoff_animated_body_count += 1;
                profile.handoff_animated_body_position_error_m = profile
                    .handoff_animated_body_position_error_m
                    .max(position_error);
                profile.handoff_animated_body_rotation_error_radians = profile
                    .handoff_animated_body_rotation_error_radians
                    .max(rotation_error);
            }
        }
        let Some((index, error)) = locked_joint_anchor_error(
            spawned,
            source_profile,
            &joint_labels,
            &joints,
            &body_transforms,
        ) else {
            profile.handoff_locked_joint_anchor_error_m = 0.0;
            profile.handoff_locked_joint_anchor_profile_index = None;
            continue;
        };
        profile.handoff_locked_joint_anchor_error_m = error;
        profile.handoff_locked_joint_anchor_profile_index = u8::try_from(index).ok();
    }
}

fn body_pose_error(expected: Isometry3d, actual: Isometry3d) -> (f32, f32) {
    (
        expected.translation.distance(actual.translation),
        (expected.rotation * actual.rotation.conjugate())
            .normalize()
            .to_scaled_axis()
            .length(),
    )
}

fn locked_joint_anchor_error(
    spawned: &SpawnedRagdoll,
    source_profile: &RagdollPhysicsProfile,
    joint_labels: &Query<&JointLabel>,
    joints: &Query<&SourcePhysicsJoint>,
    body_transforms: &Query<(&Position, &Rotation)>,
) -> Option<(usize, f32)> {
    largest_locked_anchor_error(spawned.joints.values().filter_map(|joint_entity| {
        let label = joint_labels.get(*joint_entity).ok()?;
        let (profile_index, joint_profile) = source_profile
            .joints
            .iter()
            .enumerate()
            .find(|(_, candidate)| candidate.label == label.0)?;
        let joint = joints.get(*joint_entity).ok()?;
        joint_anchor_error(
            joint.parent,
            joint.child,
            joint.parent_frame,
            joint.child_frame,
            body_transforms,
        )
        .map(|error| (profile_index, joint_profile.definition.linear_locked, error))
    }))
}

fn largest_locked_anchor_error(
    errors: impl Iterator<Item = (usize, bool, f32)>,
) -> Option<(usize, f32)> {
    errors
        .filter(|(_, locked, error)| *locked && error.is_finite())
        .map(|(index, _, error)| (index, error))
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
}

fn joint_anchor_error(
    body_1: Entity,
    body_2: Entity,
    frame_1: Isometry3d,
    frame_2: Isometry3d,
    body_transforms: &Query<(&Position, &Rotation)>,
) -> Option<f32> {
    let [(position_1, rotation_1), (position_2, rotation_2)] =
        body_transforms.get_many([body_1, body_2]).ok()?;
    Some(joint_anchor_distance(
        position_1.0,
        rotation_1.0,
        frame_1,
        position_2.0,
        rotation_2.0,
        frame_2,
    ))
}

fn joint_anchor_distance(
    position_1: Vec3,
    rotation_1: Quat,
    frame_1: Isometry3d,
    position_2: Vec3,
    rotation_2: Quat,
    frame_2: Isometry3d,
) -> f32 {
    let anchor_1 = position_1 + rotation_1 * Vec3::from(frame_1.translation);
    let anchor_2 = position_2 + rotation_2 * Vec3::from(frame_2.translation);
    anchor_1.distance(anchor_2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_replaces_every_spawned_joint_and_applies_body_materials() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::ragdoll::{
            definition::Ragdoll, spawning::spawn_ragdoll_avian,
        };

        let profile = RagdollPhysicsProfile::from_json(include_str!(
            "../../../assets/ragdolls/pa_als.physics.json"
        ))
        .unwrap();
        let ragdoll: Ragdoll =
            ron::from_str(include_str!("../../../assets/ragdolls/als_manny.rag.ron")).unwrap();
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            crate::SourcePhysicsJointPlugin,
        ));
        app.finish();
        app.cleanup();
        let world = app.world_mut();
        let player = world.spawn_empty().id();
        let spawned = world
            .run_system_once(move |mut commands: Commands| {
                spawn_ragdoll_avian(player, &ragdoll, Isometry3d::IDENTITY, None, &mut commands)
            })
            .unwrap();
        let bodies = spawned.bodies.values().copied().collect::<Vec<_>>();
        let joints = spawned
            .joints
            .values()
            .map(|entity| {
                if let Some(joint) = world.get::<SphericalJoint>(*entity) {
                    (*entity, joint.body2, joint.body1)
                } else {
                    let joint = world.get::<RevoluteJoint>(*entity).unwrap();
                    (*entity, joint.body2, joint.body1)
                }
            })
            .collect::<Vec<_>>();
        let applied = world
            .run_system_once(
                move |mut commands: Commands,
                      body_labels: Query<&BodyLabel>,
                      collider_labels: Query<&ColliderLabel>,
                      joint_labels: Query<&JointLabel>,
                      spherical: Query<&SphericalJoint>,
                      revolute: Query<&RevoluteJoint>| {
                    apply_profile_to_spawned_ragdoll(
                        &mut commands,
                        &spawned,
                        &profile,
                        &body_labels,
                        &collider_labels,
                        &joint_labels,
                        &spherical,
                        &revolute,
                    )
                },
            )
            .unwrap();
        assert_eq!(
            (
                applied.body_count,
                applied.collider_count,
                applied.joint_count,
                applied.source_joint_count
            ),
            (21, 22, 20, 19)
        );
        for entity in bodies {
            assert!((world.get::<Friction>(entity).unwrap().static_coefficient - 0.7).abs() < 1e-6);
            assert!((world.get::<Restitution>(entity).unwrap().coefficient - 0.3).abs() < 1e-6);
        }
        for (entity, parent, child) in joints {
            assert!(world.get::<SphericalJoint>(entity).is_none());
            assert!(world.get::<RevoluteJoint>(entity).is_none());
            assert!(world.get::<JointDamping>(entity).is_none());
            let joint = world.get::<SourcePhysicsJoint>(entity).unwrap();
            assert_eq!((joint.parent, joint.child), (parent, child));
            assert_eq!(joint.settings.position_iterations, 6);
            assert_eq!(joint.swing_spring.unwrap().stiffness, 5_000_000.0);
        }
    }

    #[test]
    fn joint_anchor_error_uses_the_two_joint_frames() {
        assert_eq!(
            joint_anchor_distance(
                Vec3::ZERO,
                Quat::IDENTITY,
                Isometry3d::from_translation(Vec3::X),
                Vec3::X,
                Quat::IDENTITY,
                Isometry3d::IDENTITY,
            ),
            0.0
        );
    }

    #[test]
    fn source_animation_anchor_gap_is_preserved_without_a_body_pose_error() {
        let calf =
            Isometry3d::from_translation(Vec3::new(-0.42552558, -0.0000001907, -0.0000160599));
        let reference =
            Isometry3d::from_translation(Vec3::new(-0.42572044, 0.0000000886, -0.0000000071));
        assert!(
            joint_anchor_distance(
                Vec3::ZERO,
                Quat::IDENTITY,
                reference,
                calf.translation.into(),
                calf.rotation,
                Isometry3d::IDENTITY
            ) > 1e-4
        );
        assert_eq!(body_pose_error(calf, calf), (0.0, 0.0));
        assert!(body_pose_error(calf, reference).0 > 1e-4);
    }

    #[test]
    fn largest_locked_anchor_error_excludes_free_source_joint() {
        assert_eq!(
            largest_locked_anchor_error(
                [(17, false, 0.72), (7, true, 0.4), (3, true, 0.2)].into_iter(),
            ),
            Some((7, 0.4))
        );
    }
}
