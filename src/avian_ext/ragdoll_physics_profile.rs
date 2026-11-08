use std::sync::Arc;

use avian3d::prelude::{
    ActiveCollisionHooks, AngularDamping, AngularInertia, AngularVelocity, CenterOfMass, Collider, ColliderDisabled, ColliderOf, CollisionLayers, CustomPositionIntegration, CustomVelocityIntegration,
    CoefficientCombine, CollisionHooks, ComputedCenterOfMass, Friction, GravityScale, JointCollisionDisabled, JointDamping, JointDisabled,
    LinearDamping, LinearVelocity, Mass, MaxAngularSpeed, NoAutoAngularInertia, NoAutoCenterOfMass, NoAutoMass, Physics, PhysicsSchedule,
    PhysicsSystems, RayHitData, ShapeCastConfig, ShapeHitData, SimpleCollider, SolverSystems, SpatialQuery, SpatialQueryFilter,
    Position, Restitution, RevoluteJoint, RigidBody, Rotation, Sensor, SphericalJoint, Substeps,
};
use avian3d::dynamics::integrator::{IntegrationSystems, VelocityIntegrationData, pre_process_velocity_increments};
use avian3d::dynamics::solver::{constraint_graph::ConstraintGraph, softness_parameters::SoftnessCoefficients};
use avian3d::dynamics::solver::solver_body::{SolverBody, SolverBodyFlags};
use avian3d::dynamics::solver::islands::{BodyIslandNode, IslandId, PhysicsIslands, SleepIslands};
use avian3d::collider_tree::{ColliderTreeProxyKey, ColliderTrees, MovedProxies};
use avian3d::schedule::LastPhysicsTick;
use bevy::ecs::component::Tick;
use bevy::ecs::schedule::ApplyDeferred;
use bevy::ecs::system::{SystemChangeTick, SystemParam};
use bevy::math::Isometry3d;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::{
    AnimatedScene, AnimatedSceneHandle, AnimatedSceneInstance,
};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::ragdoll::bone_mapping::RagdollBoneMap;
use bevy_animation_graph::core::ragdoll::definition::{
    BodyLabel, BodyMode, ColliderLabel, ColliderShape, JointLabel,
};
use bevy_animation_graph::core::ragdoll::spawning::SpawnedRagdoll;
use serde::Deserialize;

use crate::extensions::{
    ragdoll_angular_drive::RagdollAngularDriveSet,
    source_contact_friction::{SourceContactFrictionFrame, SourceContactFrictionSettings,
        capture_source_friction_contacts, save_source_contact_history, solve_source_position_contacts,
        update_source_friction_history_ratios},
    source_contact_normal::{SourceContactSeparationRestore, refresh_source_contact_normal_speed,
        restore_source_velocity_separation, suppress_source_velocity_separation},
    source_physics_joint::{
        SourceAngularLimit, SourceAngularMotor, SourceAngularSpring, SourceJointSolverSettings, SourceLinearMotor,
        SourcePhysicsJoint,
    },
};

#[derive(Clone, Debug)]
pub struct RagdollBodyPhysicsProfile {
    pub label: String,
    pub source_index: u8,
    pub source_layers: CollisionLayers,
    pub active_mode: BodyMode,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub max_angular_speed: f32,
    pub gravity_scale: f32,
    pub friction: Friction,
    pub restitution: Restitution,
    pub sleeping_disabled: bool,
    pub sleep: SourceBodySleep,
    pub mass_properties: Option<RagdollMassProperties>,
    pub center_of_mass_nudge_m: Vec3,
    pub inertia_scale: Vec3,
    pub locked_axes_bits: u8,
    pub inertia_conditioning_enabled: bool,
    pub ccd_enabled: bool,
    pub gyroscopic_torque_enabled: bool,
}

#[derive(Component, Clone, Copy, Debug, Deserialize)]
pub struct SourceBodySleep {
    pub linear_threshold_m_per_second: f32,
    pub angular_threshold_radians_per_second: f32,
    pub counter_threshold: i32,
    #[serde(skip)]
    counter: i32,
    #[serde(skip)]
    smooth_linear: Vec3,
    #[serde(skip)]
    smooth_angular: Vec3,
    #[serde(skip)]
    explicit_wake: bool,
    #[serde(skip)]
    pending_linear_impulse: Vec3,
    #[serde(skip)]
    last_impulse_tick: Option<Tick>,
    #[serde(skip)]
    linear_write_before_impulse: bool,
}

impl SourceBodySleep {
    pub(crate) fn has_external_linear_write(&self, changed: Tick, physics: Tick, current: Tick) -> bool {
        changed.is_newer_than(physics, current) && Some(changed) != self.last_impulse_tick
    }

    pub(crate) fn record_linear_impulse(&mut self, delta: Vec3, tick: Tick, had_velocity_write: bool) {
        if delta != Vec3::ZERO {
            self.pending_linear_impulse += delta;
            self.last_impulse_tick = Some(tick);
            self.linear_write_before_impulse |= had_velocity_write;
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RagdollMassProperties {
    pub mass_kg: f32,
    pub unclamped_mass_kg: f32,
    pub scale_exponent: f32,
    pub update_with_scale: bool,
    pub center_of_mass_m: Vec3,
    pub inertia_tensor_kg_m2: Mat3,
}

#[derive(Component)]
struct RagdollMassPropertiesBaseline {
    mass: Option<Mass>,
    center_of_mass: Option<CenterOfMass>,
    angular_inertia: Option<AngularInertia>,
    no_auto_mass: bool,
    no_auto_center_of_mass: bool,
    no_auto_angular_inertia: bool,
}

#[derive(Component)]
struct RagdollGyroscopicTorqueDisabled;

#[derive(Component)]
struct SourceBodyDof {
    axes: avian3d::prelude::LockedAxes,
    anchor: Option<Isometry3d>,
    settings: SourceJointSolverSettings,
}

#[derive(Clone, Debug)]
pub struct RagdollColliderPhysicsProfile {
    pub label: String,
    pub source_body_index: u8,
    pub source_layers: CollisionLayers,
    pub consider_for_bounds: bool,
    pub collision_mode: RagdollShapeCollisionMode,
    pub shape: ColliderShape,
    pub local_transform: Transform,
}

impl RagdollColliderPhysicsProfile {
    fn scaled_collider(&self, scale: Vec3, initial_shape: bool) -> Collider {
        match &self.shape {
            ColliderShape::Sphere(sphere) => Collider::sphere(sphere.radius * scale.min_element()),
            ColliderShape::Cuboid(cuboid) => {
                let size = cuboid.half_size * scale * 2.0;
                Collider::cuboid(size.x, size.y, size.z)
            }
            ColliderShape::Capsule(capsule) => {
                let half_height = (capsule.half_length + capsule.radius) * scale.y;
                if initial_shape {
                    let half_height = half_height.max(0.001);
                    let radius = (capsule.radius * scale.x.max(scale.z)).clamp(0.001, half_height);
                    Collider::capsule(radius, ((half_height - radius) * 2.0).max(0.001))
                } else {
                    let radius = (capsule.radius * scale.x.max(scale.z)).max(0.001).min(half_height).max(0.000001);
                    let half_length = (half_height - radius).max(0.000001);
                    Collider::capsule(radius, half_length * 2.0)
                }
            }
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum RagdollShapeCollisionMode {
    NoCollision,
    QueryOnly,
    PhysicsOnly,
    QueryAndPhysics,
    ProbeOnly,
    QueryAndProbe,
}

impl RagdollShapeCollisionMode {
    fn from_source(value: &str) -> Result<Self, String> {
        match value {
            "NoCollision" => Ok(Self::NoCollision),
            "QueryOnly" => Ok(Self::QueryOnly),
            "PhysicsOnly" => Ok(Self::PhysicsOnly),
            "QueryAndPhysics" => Ok(Self::QueryAndPhysics),
            "ProbeOnly" => Ok(Self::ProbeOnly),
            "QueryAndProbe" => Ok(Self::QueryAndProbe),
            _ => Err(format!("unsupported shape collision mode: {value}")),
        }
    }

    pub fn has_query(self) -> bool {
        matches!(self, Self::QueryOnly | Self::QueryAndPhysics | Self::QueryAndProbe)
    }

    fn has_physics_response(self) -> bool {
        matches!(self, Self::PhysicsOnly | Self::QueryAndPhysics)
    }
}

#[derive(Clone, Debug)]
pub struct RagdollJointPhysicsProfile {
    pub label: String,
    pub definition: SourcePhysicsJoint,
    pub enabled: bool,
    pub collision_disabled: bool,
}

#[derive(Component, Clone, Copy, Debug, Deserialize)]
pub struct SourceCcdSettings {
    enable_threshold_bounds_scale: f32,
    allowed_depth_bounds_scale: f32,
    axis_threshold_mode: u8,
    new_target_depth_mode: bool,
    include_probe_shapes: bool,
    only_dynamic_static: bool,
}

#[derive(Component, Clone, Copy, Debug, Deserialize)]
pub struct SourceInertiaConditioningSettings {
    enabled: bool,
    distance_m: f32,
    rotation_ratio: f32,
    max_inverse_inertia_component_ratio: f32,
    inverse_mass_tolerance: f32,
    inverse_inertia_tolerance: f32,
    extent_tolerance_m: f32,
    exclude_free_joints: bool,
}

impl SourceInertiaConditioningSettings {
    fn inverse_scale(self, inverse_mass: f32, inverse: Vec3, extents: Vec3) -> Vec3 {
        if !(self.enabled && inverse_mass > self.inverse_mass_tolerance
            && inverse.min_element() > self.inverse_inertia_tolerance && extents.min_element() > self.extent_tolerance_m) {
            return Vec3::ONE;
        }
        let squared = extents * extents;
        let ratio = inverse / inverse_mass * Vec3::new(squared.y.max(squared.z), squared.x.max(squared.z), squared.x.max(squared.y));
        let maximum = self.rotation_ratio * if self.distance_m > 0.0 { (extents.max_element() / self.distance_m).min(1.0) } else { 1.0 };
        let mut scale = (Vec3::splat(maximum) / ratio).min(Vec3::ONE);
        if self.max_inverse_inertia_component_ratio > 1.0 {
            let scaled = scale * inverse;
            let limit = scaled.min_element() * self.max_inverse_inertia_component_ratio;
            for index in 0..3 { if scaled[index] > limit { scale[index] = limit / scaled[index]; } }
            scale = scale.min(Vec3::ONE);
        }
        scale
    }
}

#[derive(Component, Clone, Debug)]
pub struct RagdollPhysicsProfile {
    pub bodies: Vec<RagdollBodyPhysicsProfile>,
    pub colliders: Vec<RagdollColliderPhysicsProfile>,
    pub joints: Vec<RagdollJointPhysicsProfile>,
    pub collision_disabled_pairs: Arc<[(u8, u8)]>,
    pub ccd_settings: SourceCcdSettings,
    pub solver_settings: SourceJointSolverSettings,
    pub inertia_conditioning_settings: SourceInertiaConditioningSettings,
}

#[derive(Deserialize)]
struct PhysicsAssetDocument {
    format: String,
    bodies: Vec<PhysicsAssetBody>,
    constraints: Vec<PhysicsAssetConstraint>,
    collision_disabled_pairs: Vec<[u8; 2]>,
    target_solver: SourceJointSolverSettings,
    target_ccd: SourceCcdSettings,
    target_inertia_conditioning: SourceInertiaConditioningSettings,
}

#[derive(Deserialize)]
struct PhysicsAssetBody {
    source_index: u8,
    bone_name: String,
    collision_response: String,
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
    #[serde(default)]
    mass_properties: Option<PhysicsAssetMassProperties>,
    locked_axes_bits: u8,
    inertia_conditioning_enabled: bool,
    ccd_enabled: bool,
    #[serde(default)]
    gyroscopic_torque_enabled: bool,
}

#[derive(Deserialize)]
struct PhysicsAssetMassProperties {
    mass_kg: f32,
    unclamped_mass_kg: f32,
    scale_exponent: f32,
    update_with_scale: bool,
    center_of_mass_m: [f32; 3],
    inertia_tensor_kg_m2: [[f32; 3]; 3],
    center_of_mass_nudge_m: [f32; 3],
    inertia_scale: [f32; 3],
}

#[derive(Deserialize)]
struct PhysicsAssetMaterial {
    static_friction: f32,
    dynamic_friction: f32,
    restitution: f32,
    friction_combine: String,
    restitution_combine: String,
    #[serde(default)]
    sleeping_disabled: bool,
    sleep: SourceBodySleep,
}

#[derive(Deserialize)]
struct PhysicsAssetShape {
    source_shape_index: u8,
    target: PhysicsAssetShapeTarget,
}

#[derive(Deserialize)]
struct PhysicsAssetShapeTarget {
    kind: String,
    collision_enabled: String,
    translation_m: [f32; 3],
    rotation_xyzw: [f32; 4],
    radius_m: Option<f32>,
    half_length_m: Option<f32>,
    half_extents_m: Option<[f32; 3]>,
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
    use_linear_solver: bool,
    angular_limits: [SourceAngularLimit; 3],
    swing_spring: Option<SourceAngularSpring>,
    twist_spring: Option<SourceAngularSpring>,
    drives: [SourceAngularMotor; 3],
    drive_target_xyzw: [f32; 4],
    #[serde(default)]
    drive_velocity_target_radians_per_second: [f32; 3],
    angular_drive_stiffness_scale: f32,
    angular_drive_damping_scale: f32,
    linear_locked: bool,
    #[serde(default)]
    linear_drives: [SourceLinearMotor; 3],
    #[serde(default)]
    linear_position_target_m: [f32; 3],
    #[serde(default)]
    linear_velocity_target_m_per_second: [f32; 3],
    #[serde(default)]
    linear_drive_acceleration_mode: bool,
    mass_conditioning: bool,
    projection_linear_alpha: f32,
    projection_linear_tolerance_m: f32,
    enabled: bool,
    #[serde(default)]
    collision_disabled: bool,
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

fn source_body_collision_layers(value: &str) -> Result<CollisionLayers, String> {
    match value {
        "BodyCollision_Enabled" => Ok(CollisionLayers::from_bits(1, 1)),
        "BodyCollision_Disabled" => Ok(CollisionLayers::NONE),
        _ => Err(format!("unsupported body collision response: {value}")),
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
                    source_layers: source_body_collision_layers(&body.collision_response)?,
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
                    sleeping_disabled: material.sleeping_disabled,
                    sleep: material.sleep,
                    mass_properties: body.target.mass_properties.as_ref().map(|value| RagdollMassProperties {
                        mass_kg: value.mass_kg,
                        unclamped_mass_kg: value.unclamped_mass_kg,
                        scale_exponent: value.scale_exponent,
                        update_with_scale: value.update_with_scale,
                        center_of_mass_m: Vec3::from_array(value.center_of_mass_m),
                        inertia_tensor_kg_m2: Mat3::from_cols_array_2d(&value.inertia_tensor_kg_m2),
                    }),
                    center_of_mass_nudge_m: body.target.mass_properties.as_ref()
                        .map_or(Vec3::ZERO, |value| Vec3::from_array(value.center_of_mass_nudge_m)),
                    inertia_scale: body.target.mass_properties.as_ref()
                        .map_or(Vec3::ONE, |value| Vec3::from_array(value.inertia_scale)),
                    locked_axes_bits: body.target.locked_axes_bits,
                    inertia_conditioning_enabled: body.target.inertia_conditioning_enabled,
                    ccd_enabled: body.target.ccd_enabled,
                    gyroscopic_torque_enabled: body.target.gyroscopic_torque_enabled,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let colliders = document
            .bodies
            .iter()
            .zip(&bodies)
            .flat_map(|(body, profile)| {
                body.shapes
                    .iter()
                    .map(|shape| Ok(RagdollColliderPhysicsProfile {
                        label: format!(
                            "{}_{}_{}",
                            body.bone_name, shape.source_shape_index, shape.target.kind
                        ),
                        source_body_index: body.source_index,
                        source_layers: profile.source_layers,
                        consider_for_bounds: body.source_properties.consider_for_bounds == "True",
                        collision_mode: RagdollShapeCollisionMode::from_source(&shape.target.collision_enabled)?,
                        shape: match shape.target.kind.as_str() {
                            "sphere" => ColliderShape::Sphere(Sphere::new(shape.target.radius_m.ok_or("Missing sphere radius")?)),
                            "box" => ColliderShape::Cuboid(Cuboid { half_size: Vec3::from_array(shape.target.half_extents_m.ok_or("Missing box half extents")?) }),
                            "capsule" => ColliderShape::Capsule(Capsule3d { radius: shape.target.radius_m.ok_or("Missing capsule radius")?,
                                half_length: shape.target.half_length_m.ok_or("Missing capsule half length")? }),
                            value => return Err(format!("Unsupported ragdoll primitive: {value}")),
                        },
                        local_transform: Transform::from_translation(Vec3::from_array(shape.target.translation_m))
                            .with_rotation(Quat::from_array(shape.target.rotation_xyzw)),
                    }))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let joints = document
            .constraints
            .into_iter()
            .map(|constraint| {
                let target = constraint.target;
                RagdollJointPhysicsProfile {
                    label: constraint.joint_name,
                    enabled: target.enabled,
                    collision_disabled: target.collision_disabled,
                    definition: SourcePhysicsJoint {
                        parent: Entity::PLACEHOLDER,
                        child: Entity::PLACEHOLDER,
                        parent_frame: target.frame_2.into(),
                        child_frame: target.frame_1.into(),
                        use_linear_solver: target.use_linear_solver,
                        angular_limits: target.angular_limits,
                        swing_spring: target.swing_spring,
                        twist_spring: target.twist_spring,
                        motors: target.drives,
                        motor_target: Quat::from_array(target.drive_target_xyzw),
                        motor_velocity_target: Vec3::from_array(target.drive_velocity_target_radians_per_second),
                        motor_stiffness_scale: target.angular_drive_stiffness_scale,
                        motor_damping_scale: target.angular_drive_damping_scale,
                        linear_locked: target.linear_locked,
                        linear_motors: target.linear_drives,
                        linear_position_target: Vec3::from_array(target.linear_position_target_m),
                        linear_velocity_target: Vec3::from_array(target.linear_velocity_target_m_per_second),
                        linear_drive_acceleration_mode: target.linear_drive_acceleration_mode,
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
            ccd_settings: document.target_ccd,
            solver_settings: document.target_solver,
            inertia_conditioning_settings: document.target_inertia_conditioning,
        })
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RagdollActiveMode(pub BodyMode);

#[derive(Component, Default)]
struct SourceRagdollFrameRotation {
    integrated: bool,
}

#[derive(Component, Clone, Debug)]
pub struct RagdollCollisionMember {
    pub group: Entity,
    pub body_index: u8,
    pub source_layers: CollisionLayers,
    pub disabled_pairs: Arc<[(u8, u8)]>,
}

#[derive(Component, Default)]
pub struct SourceCollisionPawn;

#[derive(Component, Default)]
pub struct SourceCollisionIgnorePawn;

fn ignores_pawn_pair(ignored_a: bool, pawn_a: bool, ignored_b: bool, pawn_b: bool) -> bool {
    ignored_a && pawn_b || ignored_b && pawn_a
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
    pub spawned_root: Option<Entity>,
    pub scale: Vec3,
    pub live_scale_applied: bool,
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
    modes: Query<'w, 's, &'static RagdollShapeCollisionMode>,
    ignored: Query<'w, 's, (), With<SourceCollisionIgnorePawn>>,
    pawns: Query<'w, 's, (), With<SourceCollisionPawn>>,
    owners: Query<'w, 's, &'static ColliderOf>,
}

impl CollisionHooks for RagdollCollisionHooks<'_, '_> {
    fn filter_pairs(
        &self,
        collider_1: Entity,
        collider_2: Entity,
        _commands: &mut Commands,
    ) -> bool {
        let is_pawn = |entity| self.pawns.contains(entity)
            || self.owners.get(entity).is_ok_and(|owner| self.pawns.contains(owner.body));
        if ignores_pawn_pair(self.ignored.contains(collider_1), is_pawn(collider_1),
            self.ignored.contains(collider_2), is_pawn(collider_2)) {
            return false;
        }
        if [collider_1, collider_2].into_iter().any(|entity| self.modes.get(entity)
            .is_ok_and(|mode| matches!(mode, RagdollShapeCollisionMode::NoCollision | RagdollShapeCollisionMode::QueryOnly))) {
            return false;
        }
        let Ok([member_1, member_2]) = self.members.get_many([collider_1, collider_2]) else {
            return true;
        };
        !member_1.disables(member_2)
    }
}

#[derive(SystemParam)]
pub struct RagdollSpatialQuery<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    modes: Query<'w, 's, &'static RagdollShapeCollisionMode>,
    colliders: Query<'w, 's, (&'static Position, &'static Rotation, &'static Collider)>,
    trees: Res<'w, ColliderTrees>,
}

impl RagdollSpatialQuery<'_, '_> {
    pub fn collider_geometry(&self, entity: Entity) -> Option<(&Position, &Rotation, &Collider)> {
        self.colliders.get(entity).ok()
    }

    fn queryable(&self, entity: Entity) -> bool {
        self.modes.get(entity).map_or(true, |mode| mode.has_query())
    }

    pub fn cast_ray(
        &self,
        origin: Vec3,
        direction: Dir3,
        max_distance: f32,
        solid: bool,
        filter: &SpatialQueryFilter,
    ) -> Option<RayHitData> {
        self.cast_ray_predicate(origin, direction, max_distance, solid, filter, &|_| true)
    }

    pub fn cast_ray_predicate(
        &self,
        origin: Vec3,
        direction: Dir3,
        max_distance: f32,
        solid: bool,
        filter: &SpatialQueryFilter,
        predicate: &dyn Fn(Entity) -> bool,
    ) -> Option<RayHitData> {
        self.spatial.cast_ray_predicate(origin, direction, max_distance, solid, filter,
            &|entity| self.queryable(entity) && predicate(entity))
    }

    pub fn cast_shape(
        &self,
        shape: &Collider,
        origin: Vec3,
        rotation: Quat,
        direction: Dir3,
        config: &ShapeCastConfig,
        filter: &SpatialQueryFilter,
    ) -> Option<ShapeHitData> {
        self.cast_shape_predicate(shape, origin, rotation, direction, config, filter, &|_| true)
    }

    pub fn cast_shape_predicate(
        &self,
        shape: &Collider,
        origin: Vec3,
        rotation: Quat,
        direction: Dir3,
        config: &ShapeCastConfig,
        filter: &SpatialQueryFilter,
        predicate: &dyn Fn(Entity) -> bool,
    ) -> Option<ShapeHitData> {
        let scaled = shape.shape_scaled();
        let Some(capsule) = scaled.as_capsule().filter(|capsule| capsule.segment.a != capsule.segment.b)
            .filter(|_| config.target_distance == 0.0) else {
            return self.spatial.cast_shape_predicate(shape, origin, rotation, direction, config, filter,
                &|entity| self.queryable(entity) && predicate(entity));
        };
        use avian3d::parry::{math::Pose3, query::{cast_shapes, ShapeCastOptions}};
        use crate::extensions::source_capsule_sweep::{capsule_cuboid_sweep, CapsuleSweepResult};

        let swept_pose = Pose3::from_parts(origin, rotation);
        let bounds = shape.aabb(origin, rotation);
        let mut best_rank = config.max_distance;
        let mut closest = None;
        for tree in self.trees.iter_trees() {
            tree.sweep_traverse_closest(bounds.into(), direction, best_rank.max(0.0), config.target_distance, |proxy_id| {
                let Some(proxy) = tree.get_proxy(proxy_id) else { return f32::MAX };
                let entity = proxy.collider;
                if !filter.test(entity, proxy.layers) || !self.queryable(entity) || !predicate(entity) {
                    return f32::MAX;
                }
                let Ok((position, obstacle_rotation, collider)) = self.colliders.get(entity) else { return f32::MAX };
                let fixed_pose = Pose3::from_parts(position.0, obstacle_rotation.0);
                let fixed_shape = collider.shape_scaled();
                let result = fixed_shape.as_cuboid().map(|cuboid| capsule_cuboid_sweep(capsule, origin, rotation,
                    cuboid, position.0, obstacle_rotation.0, *direction, config.max_distance));
                let fallback = || {
                    let options = ShapeCastOptions { max_time_of_impact: config.max_distance,
                        target_distance: config.target_distance, stop_at_penetration: !config.ignore_origin_penetration,
                        compute_impact_geometry_on_penetration: config.compute_contact_on_penetration };
                    let hit = cast_shapes(&fixed_pose, Vec3::ZERO, fixed_shape.as_ref(), &swept_pose,
                        *direction, scaled.as_ref(), options).ok()??;
                    Some(ShapeHitData { entity, distance: hit.time_of_impact,
                        point1: fixed_pose * hit.witness1,
                        point2: swept_pose * hit.witness2 + *direction * hit.time_of_impact,
                        normal1: fixed_pose.rotation * hit.normal1, normal2: swept_pose.rotation * hit.normal2 })
                };
                let hit = match result {
                    Some(CapsuleSweepResult::Miss) => None,
                    Some(CapsuleSweepResult::Hit { distance, point, normal }) => Some(ShapeHitData {
                        entity, distance, point1: point, point2: point, normal1: normal, normal2: -normal,
                    }),
                    Some(CapsuleSweepResult::InitialOverlap { depth, point, normal }) => {
                        if config.ignore_origin_penetration && normal.dot(*direction) >= 0.0 { None }
                        else { Some(ShapeHitData { entity, distance: 0.0, point1: point,
                            point2: point - normal * depth, normal1: normal, normal2: -normal }) }
                    }
                    Some(CapsuleSweepResult::DeepOverlap) | None => fallback(),
                };
                let Some(hit) = hit else { return f32::MAX };
                let rank = if hit.distance == 0.0 { -(hit.point1 - hit.point2).dot(hit.normal1).max(0.0) }
                    else { hit.distance };
                if rank < best_rank {
                    best_rank = rank;
                    closest = Some(hit);
                }
                hit.distance
            });
        }
        closest
    }

    pub fn shape_intersections(
        &self,
        shape: &Collider,
        position: Vec3,
        rotation: Quat,
        filter: &SpatialQueryFilter,
    ) -> Vec<Entity> {
        let mut hits = Vec::new();
        self.shape_intersections_callback(shape, position, rotation, filter, |entity| {
            hits.push(entity);
            true
        });
        hits
    }

    pub fn shape_intersections_callback(
        &self,
        shape: &Collider,
        position: Vec3,
        rotation: Quat,
        filter: &SpatialQueryFilter,
        mut callback: impl FnMut(Entity) -> bool,
    ) {
        self.spatial.shape_intersections_callback(shape, position, rotation, filter,
            |entity| !self.queryable(entity) || callback(entity));
    }
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RagdollPhysicsProfileSet;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RagdollPhysicsHandoffSet;

pub struct RagdollPhysicsProfilePlugin;

impl Plugin for RagdollPhysicsProfilePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SourceContactFrictionFrame>()
        .init_resource::<SourceContactSeparationRestore>().configure_sets(
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
        )
        .add_systems(PhysicsSchedule, initialize_profile_dof.in_set(SolverSystems::PrepareJoints))
        .add_systems(PhysicsSchedule, condition_profile_solver_inertia
            .in_set(SolverSystems::PrepareJoints)
            .after(initialize_profile_dof)
            .after(avian3d::dynamics::solver::xpbd::prepare_xpbd_joint::<SourcePhysicsJoint>))
        .add_systems(avian3d::prelude::SubstepSchedule, condition_profile_solver_inertia
            .after(avian3d::dynamics::integrator::IntegrationSystems::Position)
            .after(solve_profile_linear_ccd)
            .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::Relax))
        .add_systems(avian3d::prelude::SubstepSchedule, capture_source_friction_contacts
            .after(IntegrationSystems::Velocity)
            .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::WarmStart)
            .run_if(resource_exists::<SourceContactFrictionSettings>))
        .add_systems(avian3d::prelude::SubstepSchedule, refresh_source_contact_normal_speed
            .after(IntegrationSystems::Velocity)
            .before(capture_source_friction_contacts)
            .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::WarmStart))
        .add_systems(avian3d::prelude::SubstepSchedule, solve_source_position_contacts
            .after(condition_profile_solver_inertia)
            .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::Relax)
            .run_if(resource_exists::<SourceContactFrictionSettings>))
        .add_systems(avian3d::prelude::SubstepSchedule, suppress_source_velocity_separation
            .after(solve_source_position_contacts)
            .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::Relax))
        .add_systems(avian3d::prelude::SubstepSchedule, update_source_friction_history_ratios
            .after(suppress_source_velocity_separation)
            .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::Relax)
            .run_if(resource_exists::<SourceContactFrictionSettings>))
        .add_systems(avian3d::prelude::SubstepSchedule, save_source_contact_history
            .after(avian3d::dynamics::solver::schedule::SubstepSolverSystems::Relax)
            .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::Damping)
            .run_if(resource_exists::<SourceContactFrictionSettings>))
        .add_systems(avian3d::prelude::SubstepSchedule, restore_source_velocity_separation
            .after(avian3d::dynamics::solver::schedule::SubstepSolverSystems::Relax)
            .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::Damping))
        .add_systems(avian3d::prelude::SubstepSchedule, solve_profile_dof
            .after(avian3d::dynamics::solver::xpbd::XpbdSolverSystems::VelocityProjection)
            .after(avian3d::dynamics::solver::schedule::SubstepSolverSystems::Damping)
            .after(crate::extensions::source_physics_joint::solve_source_joint_velocity_iterations))
        .add_systems(avian3d::prelude::SubstepSchedule, solve_profile_linear_ccd
            .after(avian3d::dynamics::integrator::IntegrationSystems::Position)
            .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::Relax))
        .add_systems(PhysicsSchedule, disable_ragdoll_gyroscopic_torque
            .in_set(SolverSystems::PreSubstep))
        .add_systems(PhysicsSchedule, apply_source_ragdoll_hard_contacts
            .in_set(SolverSystems::PreSubstep))
        .add_systems(PhysicsSchedule, apply_source_linear_damping
            .in_set(IntegrationSystems::UpdateVelocityIncrements)
            .after(pre_process_velocity_increments))
        .add_systems(PhysicsSchedule, seed_source_sleep_from_forces
            .in_set(IntegrationSystems::UpdateVelocityIncrements)
            .after(avian3d::dynamics::rigid_body::forces::ForceSystems::ApplyConstantForces)
            .before(pre_process_velocity_increments))
        .add_systems(PhysicsSchedule, (enable_source_ragdoll_rotation, ApplyDeferred)
            .chain().in_set(SolverSystems::PreSubstep))
        .add_systems(avian3d::prelude::SubstepSchedule, integrate_source_ragdoll_rotation
            .in_set(avian3d::dynamics::integrator::IntegrationSystems::Position)
            .after(avian3d::dynamics::integrator::integrate_positions))
        .add_systems(PhysicsSchedule, (disable_source_ragdoll_rotation, ApplyDeferred)
            .chain().in_set(SolverSystems::PostSubstep))
        .add_systems(PhysicsSchedule, prepare_profile_sleep_islands
            .before(SolverSystems::Finalize)
            .after(SolverSystems::Restitution)
            .run_if(resource_exists::<PhysicsIslands>))
        .add_systems(PhysicsSchedule, capture_source_sleep_wakes
            .after(avian3d::prelude::PhysicsStepSystems::NarrowPhase)
            .before(SolverSystems::PrepareSolverBodies))
        .add_systems(PhysicsSchedule, update_profile_sleep
            .after(avian3d::prelude::PhysicsStepSystems::Sleeping)
            .before(avian3d::prelude::PhysicsStepSystems::Finalize)
            .run_if(resource_exists::<PhysicsIslands>));
    }
}

fn apply_source_ragdoll_hard_contacts(
    time: Res<Time<Substeps>>,
    ragdoll_bodies: Query<(), With<RagdollActiveMode>>,
    mut graph: ResMut<ConstraintGraph>,
) {
    let softness = SoftnessCoefficients { bias: time.delta_secs().recip(), mass_scale: 1.0, impulse_scale: 0.0 };
    for constraint in graph.colors.iter_mut().flat_map(|color| &mut color.contact_constraints) {
        if ragdoll_bodies.contains(constraint.body1) || ragdoll_bodies.contains(constraint.body2) {
            for point in &mut constraint.points { point.normal_part.softness = softness; }
        }
    }
}

pub(crate) fn capture_source_sleep_wakes(
    last_physics_tick: Res<LastPhysicsTick>,
    tick: SystemChangeTick,
    mut bodies: Query<(&RigidBody, &mut LinearVelocity, Ref<AngularVelocity>, &mut SourceBodySleep)>,
) {
    let [linear_tolerance, angular_tolerance]: [f32; 2] = include!("sleep_velocity_tolerance.rs");
    for (body, mut linear, angular, mut sleep) in &mut bodies {
        sleep.explicit_wake = false;
        let external_linear = linear.is_changed()
            && sleep.has_external_linear_write(linear.last_changed(), last_physics_tick.0, tick.this_run());
        let changed = sleep.linear_write_before_impulse || external_linear
            || (angular.is_changed() && angular.last_changed().is_newer_than(last_physics_tick.0, tick.this_run()));
        if body.is_dynamic() && changed {
            let source_linear = if sleep.last_impulse_tick == Some(linear.last_changed()) {
                linear.0 - sleep.pending_linear_impulse
            } else { linear.0 };
            if external_linear && sleep.last_impulse_tick.is_some() {
                linear.0 += sleep.pending_linear_impulse;
            }
            sleep.smooth_linear = source_linear;
            sleep.smooth_angular = angular.0;
            sleep.explicit_wake = source_linear.abs().max_element() > linear_tolerance
                || angular.0.abs().max_element() > angular_tolerance;
            if sleep.explicit_wake { sleep.counter = 0; }
        }
    }
}

fn seed_source_sleep_from_forces(
    mut bodies: Query<(&RigidBody, &LinearVelocity, &AngularVelocity, &Rotation, &VelocityIntegrationData,
        Option<&avian3d::dynamics::rigid_body::forces::AccumulatedLocalAcceleration>, &mut SourceBodySleep)>,
) {
    let [linear_tolerance, angular_tolerance]: [f32; 2] = include!("sleep_velocity_tolerance.rs");
    let [prediction_time, rate]: [f32; 2] = include!("sleep_force_parameters.rs");
    for (body, linear, angular, rotation, integration, local, mut sleep) in &mut bodies {
        if body.is_dynamic() {
            let linear_acceleration = integration.linear_increment + local.map_or(Vec3::ZERO, |local| rotation.0 * local.linear);
            let angular_acceleration = integration.angular_increment + local.map_or(Vec3::ZERO, |local| rotation.0 * local.angular);
            if linear_acceleration.abs().max_element() > linear_tolerance
                || sleep.pending_linear_impulse.abs().max_element() > linear_tolerance {
                sleep.smooth_linear = sleep.smooth_linear.lerp(linear.0 + linear_acceleration * prediction_time, rate);
            }
            if angular_acceleration.abs().max_element() > angular_tolerance {
                sleep.smooth_angular = sleep.smooth_angular.lerp(angular.0 + angular_acceleration * prediction_time, rate);
            }
        }
        sleep.pending_linear_impulse = Vec3::ZERO;
        sleep.last_impulse_tick = None;
        sleep.linear_write_before_impulse = false;
    }
}

fn apply_source_linear_damping(
    time: Res<Time<Substeps>>,
    mut bodies: Query<(&RigidBody, &LinearDamping, &mut VelocityIntegrationData),
        (With<RagdollActiveMode>, Without<CustomVelocityIntegration>)>,
) {
    let dt = time.delta_secs();
    for (body, damping, mut integration) in &mut bodies {
        if body.is_dynamic() {
            let multiplier = (1.0 - damping.0 * dt).max(0.0);
            integration.linear_increment *= multiplier;
            integration.linear_damping_rhs = multiplier;
        }
    }
}

fn enable_source_ragdoll_rotation(
    mut commands: Commands,
    bodies: Query<(Entity, &RigidBody), (With<RagdollActiveMode>, Without<CustomPositionIntegration>)>,
) {
    for (entity, body) in &bodies {
        if body.is_dynamic() {
            commands.entity(entity).insert((CustomPositionIntegration, SourceRagdollFrameRotation::default()));
        }
    }
}

pub(crate) fn integrate_source_ragdoll_rotation(
    frame: Res<Time<Physics>>,
    step: Res<Time>,
    mut bodies: Query<(&mut SolverBody, &mut SourceRagdollFrameRotation)>,
) {
    for (mut body, mut state) in &mut bodies {
        let velocity = body.linear_velocity;
        body.delta_position += velocity * step.delta_secs();
        if !state.integrated {
            let angular = body.angular_velocity * (0.5 * frame.delta_secs());
            body.delta_rotation.0 = (Quat::from_xyzw(angular.x, angular.y, angular.z, 1.0) * body.delta_rotation.0).normalize();
            state.integrated = true;
        }
    }
}

fn disable_source_ragdoll_rotation(
    mut commands: Commands,
    bodies: Query<Entity, With<SourceRagdollFrameRotation>>,
) {
    for entity in &bodies {
        commands.entity(entity).remove::<(CustomPositionIntegration, SourceRagdollFrameRotation)>();
    }
}

fn prepare_profile_sleep_islands(
    mut islands: ResMut<PhysicsIslands>,
    bodies: Query<&BodyIslandNode, With<SourceBodySleep>>,
) {
    if let Some(node) = bodies.iter().find(|node| islands.get(node.island_id())
        .is_some_and(|island| island.constraints_removed() > 0)) {
        islands.split_candidate = Some(node.island_id());
    }
}

fn update_profile_sleep(
    mut commands: Commands,
    islands: Res<PhysicsIslands>,
    time: Res<Time>,
    time_to_sleep: Res<avian3d::prelude::TimeToSleep>,
    mut bodies: Query<(&BodyIslandNode, &RigidBody, &SolverBody, &Rotation, &ComputedCenterOfMass, &mut SourceBodySleep,
        Has<avian3d::prelude::SleepingDisabled>, Has<avian3d::prelude::Sleeping>)>,
    other_bodies: Query<(&BodyIslandNode, &avian3d::prelude::SleepTimer,
        Has<avian3d::prelude::SleepingDisabled>), Without<SourceBodySleep>>,
    topology: Query<(Entity, &BodyIslandNode)>,
    mut previous_connected: Local<bevy::platform::collections::HashMap<Entity, bool>>,
) {
    let [_, smooth_rate]: [f32; 2] = include!("sleep_force_parameters.rs");
    let retained_islands = topology.iter().filter_map(|(entity, node)| {
        previous_connected.get(&entity).copied().unwrap_or(false).then_some(node.island_id().0)
    }).collect::<bevy::platform::collections::HashSet<_>>();
    let mut groups = bevy::platform::collections::HashMap::<u32, (i32, i32, bool, bool)>::default();
    for (node, mode, solver, rotation, center, mut sleep, disabled, sleeping) in &mut bodies {
        if !mode.is_dynamic() {
            sleep.counter = 0;
            sleep.smooth_linear = solver.linear_velocity;
            sleep.smooth_angular = solver.angular_velocity;
            continue;
        }
        if sleeping { continue; }
        let Some(island) = islands.get(node.island_id()) else { continue; };
        let isolated = island.contact_count() == 0 && island.joint_count() == 0;
        if time.delta_secs() > 1e-8 {
            if isolated {
                sleep.smooth_linear = solver.linear_velocity;
                sleep.smooth_angular = solver.angular_velocity;
            } else {
                let world_center = rotation.0 * center.0;
                let origin_delta = solver.delta_position + (solver.delta_rotation.0.conjugate() * world_center - world_center);
                sleep.smooth_linear = sleep.smooth_linear.lerp(origin_delta / time.delta_secs(), smooth_rate);
                let delta = solver.delta_rotation.0;
                let delta = if delta.w < 0.0 { -delta } else { delta };
                let angular = Vec3::new(delta.x, delta.y, delta.z) * (2.0 / time.delta_secs());
                sleep.smooth_angular = sleep.smooth_angular.lerp(angular, smooth_rate);
            }
        }
        let within = !disabled
            && (sleep.linear_threshold_m_per_second > 0.0 || sleep.angular_threshold_radians_per_second > 0.0)
            && sleep.smooth_linear.length_squared() <= sleep.linear_threshold_m_per_second.powi(2)
            && sleep.smooth_angular.length_squared() <= sleep.angular_threshold_radians_per_second.powi(2);
        let group = groups.entry(node.island_id().0).or_insert((i32::MAX, 0, true, isolated));
        group.0 = group.0.min(sleep.counter);
        if !isolated && !retained_islands.contains(&node.island_id().0) { group.0 = 0; }
        group.1 = group.1.max(if isolated { sleep.counter_threshold.min(127) } else { sleep.counter_threshold });
        group.2 &= within && (!sleep.explicit_wake || isolated);
    }
    for (node, timer, disabled) in &other_bodies {
        if let Some(group) = groups.get_mut(&node.island_id().0) {
            group.2 &= !disabled && timer.0 >= time_to_sleep.0;
        }
    }
    let mut to_sleep = Vec::new();
    for (id, group) in &mut groups {
        if group.2 {
            group.0 = group.0.saturating_add(1);
            if group.3 { group.0 = group.0.min(127); }
            if group.0 > group.1 {
                to_sleep.push(IslandId(*id));
                group.0 = 0;
            }
        } else if !group.3 {
            group.0 = 0;
        }
    }
    for (node, _, _, _, _, mut sleep, _, _) in &mut bodies {
        if let Some(group) = groups.get(&node.island_id().0) { sleep.counter = group.0; }
    }
    previous_connected.clear();
    for (entity, node) in &topology {
        if let Some(island) = islands.get(node.island_id()) {
            previous_connected.insert(entity, island.contact_count() > 0 || island.joint_count() > 0);
        }
    }
    if !to_sleep.is_empty() { commands.queue(SleepIslands(to_sleep)); }
}

fn initialize_profile_dof(mut bodies: Query<(&RigidBody, &Position, &Rotation, &mut SourceBodyDof)>) {
    for (mode, position, rotation, mut dof) in &mut bodies {
        if !mode.is_dynamic() { dof.anchor = None; }
        else if dof.anchor.is_none() { dof.anchor = Some(Isometry3d::new(position.0, rotation.0)); }
    }
}

#[allow(clippy::type_complexity)]
fn condition_profile_solver_inertia(
    mut bodies: Query<(Entity, &RigidBody, &Rotation, &avian3d::prelude::ComputedMass,
        &avian3d::prelude::ComputedAngularInertia, &avian3d::prelude::ComputedCenterOfMass,
        &SourceInertiaConditioningSettings, Option<&SourceBodyDof>, &SolverBody, &mut avian3d::dynamics::solver::solver_body::SolverBodyInertia)>,
    colliders: Query<(&Collider, &avian3d::prelude::ColliderOf, &avian3d::prelude::ColliderTransform)>,
    joints: Query<&SourcePhysicsJoint, Without<JointDisabled>>,
) {
    for (entity, mode, rotation, mass, angular, com, settings, dof, body, mut solver) in &mut bodies {
        if !mode.is_dynamic() { continue; }
        let principal = AngularInertia::from_tensor(angular.inverse());
        let local_basis = Mat3::from_quat(principal.local_frame.conjugate());
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for (shape, owner, transform) in &colliders {
            if owner.body != entity { continue; }
            let pose = avian3d::parry::math::Pose3::from_parts(transform.translation, transform.rotation.0);
            let bounds = shape.shape_scaled().compute_aabb(&pose);
            min = min.min(bounds.mins);max = max.max(bounds.maxs);
        }
        let half = ((max - min) * 0.5).max(Vec3::ZERO);
        let mut extents = local_basis.x_axis.abs() * half.x + local_basis.y_axis.abs() * half.y + local_basis.z_axis.abs() * half.z;
        for joint in &joints {
            if settings.exclude_free_joints && !joint.linear_locked && !joint.linear_motors.iter().any(|motor| motor.position_enabled) { continue; }
            let arm = if joint.parent == entity { Some(Vec3::from(joint.parent_frame.translation)) }
                else if joint.child == entity { Some(Vec3::from(joint.child_frame.translation)) } else { None };
            if let Some(arm) = arm { extents = extents.max((local_basis * (arm - com.0)).abs()); }
        }
        if dof.is_some_and(|dof| !settings.exclude_free_joints || dof.axes.is_translation_x_locked() || dof.axes.is_translation_y_locked() || dof.axes.is_translation_z_locked()) {
            extents = extents.max((local_basis * -com.0).abs());
        }
        let scale = settings.inverse_scale(mass.inverse(), principal.principal, extents);
        let basis = Mat3::from_quat(body.delta_rotation.0 * rotation.0 * principal.local_frame);
        let inverse = avian3d::math::SymmetricMatrix::from_mat3_unchecked(
            basis * Mat3::from_diagonal(principal.principal * scale) * basis.transpose());
        *solver = avian3d::dynamics::solver::solver_body::SolverBodyInertia::new(mass.inverse(), inverse,
            solver.flags().locked_axes(), solver.dominance() as i8, true);
    }
}

fn solve_profile_dof(
    mut bodies: Query<(&RigidBody, &Position, &Rotation, &avian3d::prelude::ComputedCenterOfMass,
        &SourceBodyDof, &avian3d::dynamics::solver::solver_body::SolverBodyInertia, &mut SolverBody),
        Without<avian3d::prelude::RigidBodyDisabled>>,
) {
    for (mode, position, rotation, com, dof, inertia, mut body) in &mut bodies {
        let Some(anchor) = dof.anchor.filter(|_| mode.is_dynamic()) else { continue; };
        let translation_locked = [dof.axes.is_translation_x_locked(), dof.axes.is_translation_y_locked(), dof.axes.is_translation_z_locked()];
        let rotation_locked = [dof.axes.is_rotation_x_locked(), dof.axes.is_rotation_y_locked(), dof.axes.is_rotation_z_locked()];
        let inverse_mass = inertia.effective_inv_mass();
        let inverse_angular = inertia.effective_inv_angular_inertia();
        for _ in 0..dof.settings.position_iterations {
            for index in 0..3 {
                if !rotation_locked[index] { continue; }
                let mut relative = (body.delta_rotation.0 * rotation.0 * anchor.rotation.conjugate()).normalize();
                if relative.w < 0.0 { relative = -relative; }
                let vector = Vec3::new(relative.x, relative.y, relative.z);
                let axis = 0.5 * (Vec3::AXES[index] * relative.w + vector.cross(Vec3::AXES[index]));
                let response = inverse_angular * axis;
                let denominator = axis.dot(response);
                if denominator > f32::EPSILON {
                    body.delta_rotation.0 = (Quat::from_scaled_axis(-response * vector[index] / denominator) * body.delta_rotation.0).normalize();
                }
            }
            for index in 0..3 {
                if !translation_locked[index] { continue; }
                let axis = Vec3::AXES[index];
                let arm = -(body.delta_rotation.0 * rotation.0 * com.0);
                let angular_axis = arm.cross(axis);
                let response = inverse_angular * angular_axis;
                let denominator = inverse_mass[index] + angular_axis.dot(response);
                let origin = position.0 + body.delta_position + rotation.0 * com.0 + arm;
                if denominator > f32::EPSILON {
                    let lambda = (origin[index] - anchor.translation[index]) / denominator;
                    body.delta_position -= inverse_mass * axis * lambda;
                    body.delta_rotation.0 = (Quat::from_scaled_axis(-response * lambda) * body.delta_rotation.0).normalize();
                }
            }
        }
        for _ in 0..dof.settings.velocity_iterations {
            for index in 0..3 {
                if rotation_locked[index] {
                    let relative = (body.delta_rotation.0 * rotation.0 * anchor.rotation.conjugate()).normalize();
                    let vector = Vec3::new(relative.x, relative.y, relative.z);
                    let axis = 0.5 * (Vec3::AXES[index] * relative.w + vector.cross(Vec3::AXES[index]));
                    let response = inverse_angular * axis;
                    let denominator = axis.dot(response);
                    if denominator > f32::EPSILON {
                        let lambda = body.angular_velocity.dot(axis) / denominator;
                        body.angular_velocity -= response * lambda;
                    }
                }
                if translation_locked[index] {
                    let arm = -(body.delta_rotation.0 * rotation.0 * com.0);
                    let angular_axis = arm.cross(Vec3::AXES[index]);
                    let response = inverse_angular * angular_axis;
                    let denominator = inverse_mass[index] + angular_axis.dot(response);
                    if denominator > f32::EPSILON {
                        let lambda = (body.linear_velocity + body.angular_velocity.cross(arm))[index] / denominator;
                        body.linear_velocity -= inverse_mass * Vec3::AXES[index] * lambda;
                        body.angular_velocity -= response * lambda;
                    }
                }
            }
        }
    }
}

fn ccd_time_at_depth(
    pose: avian3d::parry::math::Pose3, velocity: Vec3, shape: &Collider,
    other_pose: avian3d::parry::math::Pose3, other_velocity: Vec3, other: &Collider,
    first_time: f32, max_time: f32, depth: f32,
) -> Option<f32> {
    use avian3d::parry::query::contact;
    let mut time = first_time;
    for _ in 0..32 {
        let mut first = pose; first.translation += velocity * time;
        let mut second = other_pose; second.translation += other_velocity * time;
        let contact = contact(&first, shape.shape_scaled().as_ref(), &second, other.shape_scaled().as_ref(), depth).ok()??;
        if contact.dist <= -depth { return Some(time); }
        let speed = (velocity - other_velocity).dot(contact.normal1);
        if speed <= 0.0 { return None; }
        let next = time + (contact.dist + depth) / speed;
        if next <= time { return Some(time); }
        if next >= max_time { return None; }
        time = next;
    }
    None
}

#[allow(clippy::type_complexity)]
fn solve_profile_linear_ccd(
    time: Res<Time>,
    profiles: Query<(Entity, &SourceCcdSettings), With<avian3d::prelude::SweptCcd>>,
    joint_graph: Option<Res<avian3d::dynamics::solver::joint_graph::JointGraph>>,
    colliders: Query<(&Collider, &avian3d::prelude::ColliderOf, &avian3d::prelude::ColliderTransform,
        Option<&Sensor>, Option<&CollisionLayers>, Option<&RagdollCollisionMember>,
        Option<&RagdollShapeCollisionMode>, Option<&Restitution>, Has<SourceCollisionIgnorePawn>), Without<ColliderDisabled>>,
    pawns: Query<(), With<SourceCollisionPawn>>,
    mut bodies: ParamSet<(Query<(&Position, &Rotation, &RigidBody, &avian3d::prelude::ComputedCenterOfMass,
        &avian3d::prelude::ComputedMass, Option<&SolverBody>, Option<&Restitution>),
        Without<avian3d::prelude::RigidBodyDisabled>>, Query<&mut SolverBody>)>,
) {
    use avian3d::parry::{math::Pose3, query::{DefaultQueryDispatcher, QueryDispatcher, ShapeCastOptions}};
    struct CcdBody {
        entity: Entity,
        origin: Vec3,
        start: Vec3,
        end: Vec3,
        rotation: Quat,
        velocity: Vec3,
        inverse_mass: f32,
        dynamic: bool,
        mode: RigidBody,
        done: bool,
        bounds: (Vec3, Vec3, Vec3),
        settings: Option<SourceCcdSettings>,
        restitution: Restitution,
    }
    struct CcdPair {
        bodies: [usize; 2],
        shapes: [usize; 2],
        settings: SourceCcdSettings,
        restitution: f32,
    }
    let dt = time.delta_secs();
    if dt <= 0.0 || profiles.is_empty() { return; }
    let shapes = colliders.iter().filter(|(_, _, _, sensor, _, _, mode, _, _)|
        sensor.is_none() && mode.is_none_or(|mode| mode.has_physics_response())).collect::<Vec<_>>();
    let mut states = Vec::<CcdBody>::new();
    let mut indices = bevy::platform::collections::HashMap::<Entity, usize>::new();
    {
        let read = bodies.p0();
        for (shape, owner, local, _, _, _, _, _, _) in &shapes {
            let index = if let Some(index) = indices.get(&owner.body) { *index } else {
                let Ok((position, rotation, mode, center, mass, solver, restitution)) = read.get(owner.body) else { continue; };
                let end_rotation = solver.map_or(rotation.0, |solver| solver.delta_rotation.0 * rotation.0);
                let origin = position.0 + rotation.0 * center.0 - end_rotation * center.0;
                let velocity = solver.map_or(Vec3::ZERO, |solver| solver.linear_velocity);
                let end = origin + solver.map_or(Vec3::ZERO, |solver| solver.delta_position);
                let index = states.len();
                states.push(CcdBody {
                    entity: owner.body, origin, start: end - velocity * dt, end, rotation: end_rotation,
                    velocity, inverse_mass: mass.inverse(), dynamic: mode.is_dynamic() && solver.is_some(),
                    mode: *mode, done: false,
                    bounds: (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY), Vec3::splat(f32::INFINITY)),
                    settings: profiles.get(owner.body).ok().map(|(_, settings)| *settings),
                    restitution: restitution.copied().unwrap_or_default(),
                });
                indices.insert(owner.body, index);
                index
            };
            let bounds = shape.shape_scaled().compute_aabb(&Pose3::from_parts(local.translation, local.rotation.0));
            states[index].bounds.0 = states[index].bounds.0.min(bounds.mins);
            states[index].bounds.1 = states[index].bounds.1.max(bounds.maxs);
            states[index].bounds.2 = states[index].bounds.2.min(bounds.maxs - bounds.mins);
        }
    }
    let axis_threshold = |body: &CcdBody, mode: u8| match mode {
        0 => body.bounds.1 - body.bounds.0,
        1 => Vec3::splat((body.bounds.1 - body.bounds.0).min_element()),
        2 => body.bounds.2,
        3 => Vec3::splat(body.bounds.2.min_element()),
        _ => Vec3::splat(f32::INFINITY),
    };
    let mut pairs = Vec::<CcdPair>::new();
    for (first, (_, owner, _, _, layers, member, _, restitution, ignored)) in shapes.iter().enumerate() {
        let Some(&a) = indices.get(&owner.body) else { continue; };
        let Some(settings) = states[a].settings else { continue; };
        if !states[a].dynamic || settings.enable_threshold_bounds_scale < 0.0 { continue; }
        for (second, (_, other_owner, _, _, other_layers, other_member, _, other_restitution, other_ignored)) in shapes.iter().enumerate() {
            let Some(&b) = indices.get(&other_owner.body) else { continue; };
            if a == b || states[b].dynamic && states[b].settings.is_some() && first > second { continue; }
            if settings.only_dynamic_static && !states[b].mode.is_static() { continue; }
            if !layers.copied().unwrap_or_default().interacts_with(other_layers.copied().unwrap_or_default()) { continue; }
            if ignores_pawn_pair(*ignored, pawns.contains(owner.body), *other_ignored, pawns.contains(other_owner.body)) { continue; }
            if member.zip(*other_member).is_some_and(|(left, right)| left.disables(right)) { continue; }
            if joint_graph.as_ref().is_some_and(|graph|
                graph.joints_between(owner.body, other_owner.body).any(|edge| edge.collision_disabled)) { continue; }
            let threshold = settings.enable_threshold_bounds_scale;
            let combined = axis_threshold(&states[a], settings.axis_threshold_mode).min(
                (states[a].rotation * states[b].rotation.conjugate()
                    * axis_threshold(&states[b], settings.axis_threshold_mode)).abs());
            let relative = (states[a].rotation.conjugate() * ((states[b].velocity - states[a].velocity) * dt)).abs();
            if threshold > 0.0 && (relative - combined * threshold).max_element() <= 0.0 { continue; }
            let material = restitution.copied().unwrap_or(states[a].restitution)
                .combine(other_restitution.copied().unwrap_or(states[b].restitution));
            pairs.push(CcdPair { bodies: [a, b], shapes: [first, second], settings, restitution: material.coefficient });
        }
    }
    let sweep = |pair: &CcdPair, states: &[CcdBody], from: f32| -> Option<(f32, Vec3)> {
        let a = &states[pair.bodies[0]];
        let b = &states[pair.bodies[1]];
        if (a.done || !a.dynamic) && (b.done || !b.dynamic) { return None; }
        let velocity = if a.done { Vec3::ZERO } else { a.velocity };
        let other_velocity = if b.done { Vec3::ZERO } else { b.velocity };
        let position = if a.done { a.end } else { a.start + a.velocity * from };
        let other_position = if b.done { b.end } else { b.start + b.velocity * from };
        let (shape, _, local, _, _, _, _, _, _) = shapes[pair.shapes[0]];
        let (other, _, other_local, _, _, _, _, _, _) = shapes[pair.shapes[1]];
        let pose = Pose3::from_parts(position + a.rotation * local.translation, a.rotation * local.rotation.0);
        let other_pose = Pose3::from_parts(other_position + b.rotation * other_local.translation, b.rotation * other_local.rotation.0);
        let remaining = dt - from;
        let hit = DefaultQueryDispatcher.cast_shapes(&pose.inv_mul(&other_pose),
            pose.rotation.conjugate() * (other_velocity - velocity), shape.shape_scaled().as_ref(),
            other.shape_scaled().as_ref(), ShapeCastOptions { max_time_of_impact: remaining,
                stop_at_penetration: false, ..default() }).ok()??;
        let normal = pose.rotation * hit.normal1;
        let own_bounds = shape.shape_scaled().compute_local_aabb();
        let other_bounds = other.shape_scaled().compute_local_aabb();
        let min_bounds = (own_bounds.maxs - own_bounds.mins).min_element().max(
            if b.settings.is_some() { (other_bounds.maxs - other_bounds.mins).min_element() } else { 0.0 });
        let speed = (velocity - other_velocity).dot(normal);
        if (remaining - hit.time_of_impact) * speed < min_bounds * pair.settings.enable_threshold_bounds_scale { return None; }
        let depth = min_bounds * pair.settings.allowed_depth_bounds_scale;
        let candidate = if pair.settings.new_target_depth_mode && depth > 0.0 {
            ccd_time_at_depth(pose, velocity, shape, other_pose, other_velocity, other, hit.time_of_impact, remaining, depth)?
        } else { hit.time_of_impact + depth / speed.max(f32::MIN_POSITIVE) };
        (candidate >= 0.0 && candidate < remaining).then_some((from + candidate, normal))
    };
    let mut hits = pairs.iter().map(|pair| sweep(pair, &states, 0.0)).collect::<Vec<_>>();
    for _ in 0..states.iter().filter(|body| body.dynamic).count() {
        let Some((index, (toi, normal))) = hits.iter().enumerate().filter_map(|(index, hit)| hit.map(|hit| (index, hit)))
            .min_by(|left, right| left.1.0.total_cmp(&right.1.0)) else { break; };
        let pair = &pairs[index];
        let [a, b] = pair.bodies;
        let fast_kinematic = [a, b].into_iter().find(|index| {
            let body = &states[*index];
            body.mode.is_kinematic() && (pair.settings.enable_threshold_bounds_scale == 0.0
                || ((body.rotation.conjugate() * (body.velocity * dt)).abs()
                    - axis_threshold(body, pair.settings.axis_threshold_mode) * pair.settings.enable_threshold_bounds_scale).max_element() > 0.0)
        });
        let offset = fast_kinematic.map_or(Vec3::ZERO, |index|
            normal * (states[index].velocity * (dt - toi)).dot(normal));
        let positions = [a, b].map(|index| states[index].start + states[index].velocity * toi + offset);
        if pair.settings.allowed_depth_bounds_scale <= 0.0 {
            let direction = -normal;
            let normal_velocity = (states[a].velocity - states[b].velocity).dot(direction);
            let inverse_a = if states[a].dynamic && !states[a].done { states[a].inverse_mass } else { 0.0 };
            let inverse_b = if states[b].dynamic && !states[b].done { states[b].inverse_mass } else { 0.0 };
            if normal_velocity < 0.0 && inverse_a + inverse_b > 0.0 {
                let impulse = direction * (-(1.0 + pair.restitution) * normal_velocity / (inverse_a + inverse_b));
                states[a].velocity += impulse * inverse_a;
                states[b].velocity -= impulse * inverse_b;
            }
        }
        for (index, position) in [a, b].into_iter().zip(positions) {
            if states[index].dynamic && !states[index].done {
                states[index].end = position;
                states[index].done = true;
            }
        }
        for (index, next) in pairs.iter().enumerate() {
            if next.bodies.iter().any(|index| *index == a || *index == b) {
                hits[index] = sweep(next, &states, toi);
            }
        }
    }
    let mut write = bodies.p1();
    for state in states {
        if state.done && let Ok(mut solver) = write.get_mut(state.entity) {
            solver.delta_position = state.end - state.origin;
            solver.linear_velocity = state.velocity;
        }
    }
}


fn refresh_ragdoll_collision_pairs(world: &mut World, colliders: &[Entity]) {
    let active = colliders.iter().copied().filter(|entity|
        world.get::<Collider>(*entity).is_some() && world.get::<ColliderDisabled>(*entity).is_none())
        .collect::<Vec<_>>();
    for entity in &active { world.entity_mut(*entity).insert(ColliderDisabled); }
    for entity in active { world.entity_mut(entity).remove::<ColliderDisabled>(); }
}

fn set_joint_collision_disabled(world: &mut World, joint: Entity, colliders: &[Entity], disabled: bool) {
    if world.get::<JointCollisionDisabled>(joint).is_some() == disabled { return; }
    if disabled {
        refresh_ragdoll_collision_pairs(world, colliders);
        world.entity_mut(joint).insert(JointCollisionDisabled);
    } else {
        world.entity_mut(joint).remove::<JointCollisionDisabled>();
        let keys = colliders.iter().filter_map(|entity| world.get::<ColliderTreeProxyKey>(*entity).copied())
            .filter(|key| *key != ColliderTreeProxyKey::PLACEHOLDER).collect::<Vec<_>>();
        for key in keys {
            world.resource_mut::<MovedProxies>().insert(key);
        }
    }
}

fn disable_ragdoll_gyroscopic_torque(
    mut bodies: Query<&mut SolverBody, With<RagdollGyroscopicTorqueDisabled>>,
) {
    for mut body in &mut bodies {
        body.flags.remove(SolverBodyFlags::GYROSCOPIC_MOTION);
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

fn ragdoll_positive_scale(
    player_entity: Entity,
    transform_helper: &bevy::transform::helper::TransformHelper,
) -> Vec3 {
    transform_helper.compute_global_transform(player_entity).ok()
        .map(|transform| transform.to_scale_rotation_translation().0)
        .filter(|scale| scale.is_finite() && scale.min_element() > 0.0)
        .unwrap_or(Vec3::ONE)
}

fn profile_application_change(
    applied: Option<RagdollPhysicsProfileApplied>,
    spawned_root: Entity,
    scale: Vec3,
) -> Option<(Vec3, Option<Entity>)> {
    let previous_scale = applied.map_or(Vec3::ONE, |applied| {
        if applied.scale.is_finite() && applied.scale.min_element() > 0.0 { applied.scale } else { Vec3::ONE }
    });
    let old_root = applied.and_then(|applied| applied.spawned_root)
        .filter(|root| *root != spawned_root);
    if applied.is_some() && old_root.is_none() && previous_scale.abs_diff_eq(scale, 1e-4) {
        return None;
    }
    Some((if old_root.is_some() { scale } else { scale / previous_scale }, old_root))
}

#[allow(clippy::type_complexity)]
fn apply_ragdoll_physics_profiles(
    mut commands: Commands,
    mut players: Query<
        (Entity, &AnimationGraphPlayer, Ref<RagdollPhysicsProfile>, Option<&mut RagdollPhysicsProfileApplied>),
    >,
    body_labels: Query<&BodyLabel>,
    collider_labels: Query<&ColliderLabel>,
    joint_labels: Query<&JointLabel>,
    spherical_joints: Query<&SphericalJoint>,
    revolute_joints: Query<&RevoluteJoint>,
    source_joints: Query<&SourcePhysicsJoint>,
    transforms: Query<&Transform>,
    mut colliders: Query<&mut Collider>,
    transform_helper: bevy::transform::helper::TransformHelper,
) {
    for (player_entity, player, profile, applied) in &mut players {
        let Some(spawned) = player.get_spawned_ragdoll() else {
            continue;
        };
        let requested_scale = ragdoll_positive_scale(player_entity, &transform_helper);
        let scale = applied.as_ref().filter(|applied| applied.spawned_root == Some(spawned.root)
            && requested_scale.max_element() - requested_scale.min_element() > 1e-4)
            .map_or(requested_scale, |applied| applied.scale);
        let (scale_factor, old_root) = match profile_application_change(
            applied.as_ref().map(|applied| **applied), spawned.root, scale,
        ) {
            Some(change) => change,
            None if profile.is_changed() => (Vec3::ONE, None),
            None => continue,
        };
        if let Some(old_root) = old_root { commands.entity(old_root).try_despawn(); }
        let live_scale_applied = applied.as_ref().is_some_and(|applied| applied.spawned_root == Some(spawned.root)
            && (applied.live_scale_applied || scale_factor != Vec3::ONE));
        let next = apply_profile_to_spawned_ragdoll(
            &mut commands,
            spawned,
            &profile,
            scale,
            scale_factor,
            applied.is_none() || old_root.is_some() || profile.is_changed(),
            !live_scale_applied,
            &body_labels,
            &collider_labels,
            &joint_labels,
            &spherical_joints,
            &revolute_joints,
            &source_joints,
            &transforms,
            &mut colliders,
        );
        if profile.is_changed() && applied.is_some() && old_root.is_none() {
            let collider_entities = spawned.colliders.values().copied().collect::<Vec<_>>();
            commands.queue(move |world: &mut World| refresh_ragdoll_collision_pairs(world, &collider_entities));
        }
        if let Some(mut applied) = applied {
            if old_root.is_some() {
                *applied = next;
            } else {
                applied.scale = scale;
                applied.live_scale_applied = next.live_scale_applied;
                applied.body_count = next.body_count;
                applied.collider_count = next.collider_count;
                applied.joint_count = next.joint_count;
                applied.source_joint_count = next.source_joint_count;
                applied.collision_disabled_pair_count = next.collision_disabled_pair_count;
            }
        } else {
            commands.entity(player_entity).insert(next);
        }
    }
}

fn scaled_ragdoll_mass_properties(
    profile: &RagdollPhysicsProfile, body: &RagdollBodyPhysicsProfile, mass: RagdollMassProperties, scale: Vec3,
    initial_shape: bool,
) -> (f32, Vec3, Mat3) {
    let clamped_capsule = profile.colliders.iter()
        .filter(|shape| shape.source_body_index == body.source_index)
        .any(|shape| match &shape.shape {
            ColliderShape::Capsule(capsule) if initial_shape => (capsule.half_length + capsule.radius) * scale.y < 0.001
                || capsule.radius * scale.x < 0.001 || capsule.half_length * scale.y * 2.0 < 0.001,
            ColliderShape::Capsule(capsule) => capsule.radius * scale.x < 0.001
                || capsule.half_length * scale.y < 0.000001,
            _ => false,
        });
    if scale.x == scale.y && scale.y == scale.z && !clamped_capsule {
        let size = scale.x;
        let value = (mass.unclamped_mass_kg * size.powf(mass.scale_exponent)).max(0.001);
        return (value, mass.center_of_mass_m * size, mass.inertia_tensor_kg_m2 * (size * size * value / mass.mass_kg));
    }
    let mut base_volume = 0.0;
    let mut combined = avian3d::parry::mass_properties::MassProperties::default();
    for shape in profile.colliders.iter().filter(|shape| shape.source_body_index == body.source_index) {
        base_volume += shape.shape.avian_collider().shape().mass_properties(1.0).mass();
        let scaled = shape.scaled_collider(scale, initial_shape);
        let pose = avian3d::parry::math::Pose3::from_parts(shape.local_transform.translation * scale, shape.local_transform.rotation);
        combined += scaled.shape().mass_properties(1.0).transform_by(&pose);
    }
    let volume = combined.mass();
    let value = (mass.unclamped_mass_kg * (volume / base_volume).powf(mass.scale_exponent / 3.0)).max(0.001);
    let mut inertia = combined.reconstruct_inertia_matrix() * (value / volume);
    let diagonal = Vec3::new(inertia.x_axis.x, inertia.y_axis.y, inertia.z_axis.z);
    let moments = (Vec3::splat(diagonal.element_sum() * 0.5) - diagonal) * body.inertia_scale * body.inertia_scale;
    let basis = Mat3::from_diagonal(body.inertia_scale);
    inertia = basis * inertia * basis;
    let nudge = body.center_of_mass_nudge_m * scale;
    inertia.x_axis.x = moments.y + moments.z + value * nudge.x * nudge.x;
    inertia.y_axis.y = moments.x + moments.z + value * nudge.y * nudge.y;
    inertia.z_axis.z = moments.x + moments.y + value * nudge.z * nudge.z;
    (value, combined.local_com + nudge, inertia)
}

fn source_angular_inertia(inertia: Mat3) -> AngularInertia {
    let diagonal = Vec3::new(inertia.x_axis.x, inertia.y_axis.y, inertia.z_axis.z);
    let trace = (f64::from(diagonal.x) + f64::from(diagonal.y) + f64::from(diagonal.z)) / 3.0;
    let off_diagonal = [inertia.x_axis.y, inertia.x_axis.z, inertia.y_axis.z]
        .into_iter().map(|value| f64::from(value).powi(2)).sum::<f64>();
    if trace * 10_000.0 <= f64::from(1e-8_f32)
        || off_diagonal / (trace * trace) < f64::from(1e-8_f32) {
        return AngularInertia::new(diagonal);
    }
    let mut result = AngularInertia::from_mat3_unchecked(inertia / trace as f32);
    result.principal *= trace as f32;
    result
}

#[allow(clippy::too_many_arguments)]
fn apply_profile_to_spawned_ragdoll(
    commands: &mut Commands,
    spawned: &SpawnedRagdoll,
    profile: &RagdollPhysicsProfile,
    scale: Vec3,
    scale_factor: Vec3,
    recalculate_mass: bool,
    initial_shape: bool,
    body_labels: &Query<&BodyLabel>,
    collider_labels: &Query<&ColliderLabel>,
    joint_labels: &Query<&JointLabel>,
    spherical_joints: &Query<&SphericalJoint>,
    revolute_joints: &Query<&RevoluteJoint>,
    source_joints: &Query<&SourcePhysicsJoint>,
    transforms: &Query<&Transform>,
    colliders: &mut Query<&mut Collider>,
) -> RagdollPhysicsProfileApplied {
    let mut applied = RagdollPhysicsProfileApplied {
        spawned_root: Some(spawned.root),
        scale,
        live_scale_applied: !initial_shape,
        collision_disabled_pair_count: profile.collision_disabled_pairs.len() as u32,
        ..default()
    };
    for body_entity in spawned.bodies.values() {
        if scale_factor != Vec3::ONE && let Ok(transform) = transforms.get(*body_entity) {
            let mut scaled = *transform;
            scaled.translation *= scale_factor;
            commands.entity(*body_entity).insert(scaled);
        }
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
        if body.sleeping_disabled {
            commands.entity(*body_entity).insert(avian3d::prelude::SleepingDisabled);
        } else {
            commands.entity(*body_entity).remove::<avian3d::prelude::SleepingDisabled>();
        }
        let source_sleep = body.sleep;
        commands.entity(*body_entity).insert(avian3d::prelude::SleepThreshold { linear: -1.0, angular: -1.0 });
        commands.entity(*body_entity).queue(move |mut entity: EntityWorldMut| {
            if let Some(mut sleep) = entity.get_mut::<SourceBodySleep>() {
                sleep.linear_threshold_m_per_second = source_sleep.linear_threshold_m_per_second;
                sleep.angular_threshold_radians_per_second = source_sleep.angular_threshold_radians_per_second;
                sleep.counter_threshold = source_sleep.counter_threshold;
            } else {
                let mut sleep = source_sleep;
                sleep.smooth_linear = entity.get::<avian3d::prelude::LinearVelocity>().map_or(Vec3::ZERO, |velocity| velocity.0);
                sleep.smooth_angular = entity.get::<avian3d::prelude::AngularVelocity>().map_or(Vec3::ZERO, |velocity| velocity.0);
                entity.insert(sleep);
            }
        });
        let bits = body.locked_axes_bits;
        if body.inertia_conditioning_enabled {
            commands.entity(*body_entity).insert(profile.inertia_conditioning_settings);
        } else {
            commands.entity(*body_entity).remove::<SourceInertiaConditioningSettings>();
        }
        let settings = profile.solver_settings;
        commands.entity(*body_entity).queue(move |mut entity: EntityWorldMut| {
            entity.remove::<avian3d::prelude::LockedAxes>();
            if bits == 0 { entity.remove::<SourceBodyDof>(); }
            else if let Some(mut dof) = entity.get_mut::<SourceBodyDof>().filter(|dof| dof.axes.to_bits() == bits) {
                dof.settings = settings;
            } else {
                entity.insert(SourceBodyDof { axes: avian3d::prelude::LockedAxes::from_bits(bits), anchor: None, settings });
            }
        });
        if body.ccd_enabled {
            commands.entity(*body_entity).insert((profile.ccd_settings,
                avian3d::prelude::SweptCcd::LINEAR.with_velocity_threshold(f32::INFINITY, f32::INFINITY)));
        } else {
            commands.entity(*body_entity).remove::<(avian3d::prelude::SweptCcd, SourceCcdSettings)>();
        }
        let mass = body.mass_properties.map(|mass| (mass,
            scaled_ragdoll_mass_properties(profile, body, mass, scale, initial_shape)));
        commands.entity(*body_entity).queue(move |mut entity: EntityWorldMut| {
            if let Some((mass, (scaled_mass, center, inertia))) = mass {
                if !entity.contains::<RagdollMassPropertiesBaseline>() {
                    let baseline = RagdollMassPropertiesBaseline {
                        mass: entity.get::<Mass>().copied(),
                        center_of_mass: entity.get::<CenterOfMass>().copied(),
                        angular_inertia: entity.get::<AngularInertia>().copied(),
                        no_auto_mass: entity.contains::<NoAutoMass>(),
                        no_auto_center_of_mass: entity.contains::<NoAutoCenterOfMass>(),
                        no_auto_angular_inertia: entity.contains::<NoAutoAngularInertia>(),
                    };
                    entity.insert(baseline);
                }
                if recalculate_mass || mass.update_with_scale {
                    entity.insert((Mass(scaled_mass), CenterOfMass(center),
                        source_angular_inertia(inertia),
                        NoAutoMass, NoAutoCenterOfMass, NoAutoAngularInertia));
                }
            } else if let Some(baseline) = entity.take::<RagdollMassPropertiesBaseline>() {
                if let Some(value) = baseline.mass { entity.insert(value); } else { entity.remove::<Mass>(); }
                if let Some(value) = baseline.center_of_mass { entity.insert(value); } else { entity.remove::<CenterOfMass>(); }
                if let Some(value) = baseline.angular_inertia { entity.insert(value); } else { entity.remove::<AngularInertia>(); }
                if !baseline.no_auto_mass { entity.remove::<NoAutoMass>(); }
                if !baseline.no_auto_center_of_mass { entity.remove::<NoAutoCenterOfMass>(); }
                if !baseline.no_auto_angular_inertia { entity.remove::<NoAutoAngularInertia>(); }
            }
        });
        if body.gyroscopic_torque_enabled {
            commands.entity(*body_entity).remove::<RagdollGyroscopicTorqueDisabled>();
        } else {
            commands.entity(*body_entity).insert(RagdollGyroscopicTorqueDisabled);
        }
        applied.body_count += 1;
    }
    for collider_entity in spawned.colliders.values() {
        let collider = collider_labels.get(*collider_entity).ok()
            .and_then(|label| find_collider_profile(profile, &label.0));
        if scale_factor != Vec3::ONE && let (Ok(local), Ok(mut shape)) =
            (transforms.get(*collider_entity), colliders.get_mut(*collider_entity)) {
            let mut transform = *local;
            if let Some(collider) = collider {
                *shape = collider.scaled_collider(scale, initial_shape);
                transform = collider.local_transform;
                transform.translation *= scale;
            } else {
                transform.translation *= scale_factor;
                transform.scale *= scale_factor;
                let next = shape.scale() * scale_factor;
                shape.set_scale(next, 16);
            }
            commands.entity(*collider_entity).insert(transform);
        }
        let Some(collider) = collider else { continue; };
        if let Some(body) = profile.bodies.iter().find(|body| body.source_index == collider.source_body_index) {
            commands.entity(*collider_entity).insert(body.restitution);
        }
        commands.entity(*collider_entity).insert((
            ActiveCollisionHooks::FILTER_PAIRS,
            collider.collision_mode,
            RagdollCollisionMember {
                group: spawned.root,
                body_index: collider.source_body_index,
                source_layers: collider.source_layers,
                disabled_pairs: profile.collision_disabled_pairs.clone(),
            },
        ));
        if collider.collision_mode == RagdollShapeCollisionMode::NoCollision {
            commands.entity(*collider_entity).insert(ColliderDisabled).remove::<Sensor>();
        } else {
            commands.entity(*collider_entity).remove::<ColliderDisabled>();
            if matches!(collider.collision_mode, RagdollShapeCollisionMode::ProbeOnly | RagdollShapeCollisionMode::QueryAndProbe) {
                commands.entity(*collider_entity).insert(Sensor);
            } else {
                commands.entity(*collider_entity).remove::<Sensor>();
            }
        }
        if collider.source_layers == CollisionLayers::NONE {
            commands.entity(*collider_entity).insert(CollisionLayers::NONE);
        }
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
            })
            .or_else(|| source_joints.get(*joint_entity).ok().map(|joint| (joint.child, joint.parent)));
        let Some((child, parent)) = bodies else {
            continue;
        };
        let mut joint = joint_profile.definition.clone();
        joint.parent = parent;
        joint.child = child;
        joint.parent_frame.translation *= bevy::math::Vec3A::from(scale);
        joint.child_frame.translation *= bevy::math::Vec3A::from(scale);
        commands
            .entity(*joint_entity)
            .remove::<(SphericalJoint, RevoluteJoint, JointDamping)>()
            .insert(joint);
        let collision_disabled = joint_profile.collision_disabled;
        let collider_entities = spawned.colliders.values().copied().collect::<Vec<_>>();
        let joint_entity = *joint_entity;
        commands.queue(move |world: &mut World| {
            set_joint_collision_disabled(world, joint_entity, &collider_entities, collision_disabled);
        });
        if joint_profile.enabled {
            commands.entity(joint_entity).remove::<JointDisabled>();
            applied.source_joint_count += 1;
        } else {
            commands.entity(joint_entity).insert(JointDisabled);
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
    #[test]
    fn source_ragdoll_shape_restitution_tracks_body_material() {
        let (mut app, _, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, _) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
        app.update();
        let profile = app.world().get::<RagdollPhysicsProfile>(player).unwrap();
        let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        let spawned = graph.get_spawned_ragdoll().unwrap();
        let shapes = spawned.colliders.values().map(|entity| {
            let label = &app.world().get::<ColliderLabel>(*entity).unwrap().0;
            let collider = find_collider_profile(profile, label).unwrap();
            (*entity, collider.source_body_index)
        }).collect::<Vec<_>>();
        assert_eq!(shapes.len(), 22);
        let head = profile.bodies.iter().find(|body| body.label == "head").unwrap().source_index;
        for (entity, index) in &shapes {
            let body = profile.bodies.iter().find(|body| body.source_index == *index).unwrap();
            assert_eq!(*app.world().get::<Restitution>(*entity).unwrap(), body.restitution);
        }
        app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().bodies.iter_mut()
            .find(|body| body.source_index == head).unwrap().restitution = Restitution {
                coefficient: 0.6, combine_rule: CoefficientCombine::Max,
            };
        app.update();
        for (entity, index) in shapes {
            let material = app.world().get::<Restitution>(entity).unwrap();
            if index == head { assert_eq!(*material, Restitution { coefficient: 0.6, combine_rule: CoefficientCombine::Max }); }
            else { assert!((material.coefficient - 0.3).abs() < 1e-6); }
        }
    }

    #[test]
    fn source_ragdoll_shape_restitution_reaches_contact_manifold() {
        use avian3d::prelude::{ColliderDisabled, ColliderTransform, Collisions, Gravity, JointDisabled};
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
        for _ in 0..12 { app.update(); }
        let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        let spawned = graph.get_spawned_ragdoll().unwrap();
        let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
        let joints = spawned.joints.values().copied().collect::<Vec<_>>();
        let head = bodies.iter().find(|(label, _)| label == "head").unwrap().1;
        let profile = app.world().get::<RagdollPhysicsProfile>(player).unwrap();
        let head_index = profile.bodies.iter().find(|body| body.label == "head").unwrap().source_index;
        let shape = colliders.iter().copied().find(|entity| {
            let label = &app.world().get::<ColliderLabel>(*entity).unwrap().0;
            find_collider_profile(profile, label).unwrap().source_body_index == head_index
        }).unwrap();
        for entity in colliders { if entity != shape { app.world_mut().entity_mut(entity).insert(ColliderDisabled); } }
        for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
        app.world_mut().insert_resource(Gravity(Vec3::ZERO));
        let position = app.world().get::<Position>(head).unwrap().0;
        let rotation = app.world().get::<Rotation>(head).unwrap().0;
        let center = position + rotation * app.world().get::<ColliderTransform>(shape).unwrap().translation;
        let surface = app.world_mut().spawn((RigidBody::Static, Collider::sphere(0.02),
            Transform::from_translation(center), Restitution::new(0.3))).id();
        let mut observed = None;
        for _ in 0..3 {
            app.update();
            observed = app.world_mut().run_system_once(move |collisions: Collisions| {
                collisions.get(surface, shape).and_then(|pair| pair.manifolds.first().map(|manifold| manifold.restitution))
            }).unwrap();
            if observed.is_some() { break; }
        }
        assert!((observed.expect("A ragdoll shape contact was not generated") - 0.3).abs() < 1e-6);
    }

    #[test]
    fn source_ragdoll_contact_bounce_uses_physics_frame_threshold() {
        use avian3d::prelude::{AngularVelocity, ColliderDisabled, ComputedCenterOfMass, Gravity, JointDisabled,
            LinearVelocity, Sleeping, SleepingDisabled};
        use bevy::time::TimeUpdateStrategy;
        for dt in [1.0_f32 / 60.0, 1.0 / 30.0] {
            for speed in [0.05_f32, 0.25, 0.5, 1.5] {
                let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
                let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
                app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
                for _ in 0..12 { app.update(); }
                let spawned = app.world().get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap();
                let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
                let joints = spawned.joints.values().copied().collect::<Vec<_>>();
                let head = bodies.iter().find(|(label, _)| label == "head").unwrap().1;
                let profile = app.world().get::<RagdollPhysicsProfile>(player).unwrap();
                let head_index = profile.bodies.iter().find(|body| body.label == "head").unwrap().source_index;
                let shape = colliders.iter().copied().find(|entity| {
                    let label = &app.world().get::<ColliderLabel>(*entity).unwrap().0;
                    find_collider_profile(profile, label).unwrap().source_body_index == head_index
                }).unwrap();
                for entity in colliders { if entity != shape { app.world_mut().entity_mut(entity).insert(ColliderDisabled); } }
                for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
                app.world_mut().insert_resource(Gravity(Vec3::ZERO));
                app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(dt)));
                let local_center = app.world().get::<ComputedCenterOfMass>(head).unwrap().0;
                let position = app.world().get::<Position>(head).unwrap().0;
                let rotation = app.world().get::<Rotation>(head).unwrap().0;
                let center = position + rotation * local_center;
                app.world_mut().entity_mut(shape).insert((Collider::sphere(0.1), Transform::from_translation(local_center)));
                app.world_mut().entity_mut(head).insert((SleepingDisabled, LinearDamping(0.0), AngularDamping(0.0),
                    LinearVelocity(Vec3::NEG_Y * speed), AngularVelocity::ZERO)).remove::<Sleeping>();
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                    Transform::from_translation(center - Vec3::Y * 0.199), Restitution::new(0.3)));
                app.update();
                let actual = app.world().get::<LinearVelocity>(head).unwrap().y;
                let expected = if speed > 10.0 * dt { speed * 0.3 } else { 0.0 };
                assert!((actual - expected).abs() < 0.025,
                    "dt={dt} incoming={speed} actual={actual} source={expected}");
            }
        }
    }

    #[test]
    fn source_ragdoll_restitution_samples_gravity_before_contact() {
        use avian3d::prelude::{AngularVelocity, ColliderDisabled, ComputedCenterOfMass, Gravity, JointDisabled,
            LinearVelocity, Sleeping, SleepingDisabled};
        use bevy::time::TimeUpdateStrategy;
        for dt in [1.0_f32 / 60.0, 1.0 / 30.0] {
            for gravity in [0.0_f32, 9.81] {
                let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
                let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
                app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
                for _ in 0..12 { app.update(); }
                let spawned = app.world().get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap();
                let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
                let joints = spawned.joints.values().copied().collect::<Vec<_>>();
                let head = bodies.iter().find(|(label, _)| label == "head").unwrap().1;
                let profile = app.world().get::<RagdollPhysicsProfile>(player).unwrap();
                let head_index = profile.bodies.iter().find(|body| body.label == "head").unwrap().source_index;
                let shape = colliders.iter().copied().find(|entity| {
                    let label = &app.world().get::<ColliderLabel>(*entity).unwrap().0;
                    find_collider_profile(profile, label).unwrap().source_body_index == head_index
                }).unwrap();
                for entity in colliders { if entity != shape { app.world_mut().entity_mut(entity).insert(ColliderDisabled); } }
                for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
                app.world_mut().insert_resource(Gravity(Vec3::NEG_Y * gravity));
                app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(dt)));
                let local_center = app.world().get::<ComputedCenterOfMass>(head).unwrap().0;
                let position = app.world().get::<Position>(head).unwrap().0;
                let rotation = app.world().get::<Rotation>(head).unwrap().0;
                let center = position + rotation * local_center;
                app.world_mut().entity_mut(shape).insert((Collider::sphere(0.1), Transform::from_translation(local_center),
                    Friction::new(0.0), Restitution::new(0.3)));
                app.world_mut().entity_mut(head).insert((SleepingDisabled, LinearDamping(0.0), AngularDamping(0.0),
                    LinearVelocity(Vec3::NEG_Y * 2.0), AngularVelocity::ZERO)).remove::<Sleeping>();
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                    Transform::from_translation(center - Vec3::Y * 0.199), Friction::new(0.0), Restitution::new(0.3)));
                app.update();
                let actual = app.world().get::<LinearVelocity>(head).unwrap().y;
                let expected = 0.3 * (2.0 + gravity * dt);
                assert!((actual - expected).abs() < 0.005,
                    "dt={dt} gravity={gravity} actual={actual} source={expected}");
            }
        }
    }

    #[test]
    fn source_ragdoll_child_bounds_track_post_solver_pose_before_prediction() {
        use avian3d::prelude::{AngularVelocity, ColliderAabb, ColliderDisabled, ComputedCenterOfMass, Gravity,
            JointDisabled, LinearVelocity, Sleeping, SleepingDisabled, SubstepCount};
        use bevy::{ecs::system::RunSystemOnce, time::TimeUpdateStrategy};
        for dt in [1.0_f32 / 60.0, 1.0 / 30.0] {
            for substeps in [1, 6] {
                for reaches_wall in [false, true] {
                    let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
                    let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
                    app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
                    for _ in 0..12 { app.update(); }
                    let spawned = app.world().get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap();
                    let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
                    let joints = spawned.joints.values().copied().collect::<Vec<_>>();
                    let head = bodies.iter().find(|(label, _)| label == "head").unwrap().1;
                    let profile = app.world().get::<RagdollPhysicsProfile>(player).unwrap();
                    let index = profile.bodies.iter().find(|body| body.label == "head").unwrap().source_index;
                    let shape = colliders.iter().copied().find(|entity| {
                        let label = &app.world().get::<ColliderLabel>(*entity).unwrap().0;
                        find_collider_profile(profile, label).unwrap().source_body_index == index
                    }).unwrap();
                    for entity in colliders { if entity != shape { app.world_mut().entity_mut(entity).insert(ColliderDisabled); } }
                    for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
                    app.world_mut().insert_resource(Gravity(Vec3::ZERO));
                    app.world_mut().insert_resource(SubstepCount(substeps));
                    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(dt)));
                    let center_local = app.world().get::<ComputedCenterOfMass>(head).unwrap().0;
                    let rotation = app.world().get::<Rotation>(head).unwrap().0;
                    let current_center = app.world().get::<Position>(head).unwrap().0 + rotation * center_local;
                    let start_center = Vec3::new(-0.65, current_center.y, current_center.z);
                    let start_body = start_center - rotation * center_local;
                    let parent = app.world().get::<ChildOf>(head).unwrap().parent();
                    let local_body = app.world_mut().run_system_once(move |transforms: bevy::transform::helper::TransformHelper| {
                        transforms.compute_global_transform(parent).unwrap().affine().inverse().transform_point3(start_body)
                    }).unwrap();
                    app.world_mut().get_mut::<Transform>(head).unwrap().translation = local_body;
                    app.world_mut().entity_mut(shape).insert((Collider::sphere(0.1), Transform::from_translation(center_local),
                        Friction::new(0.0), Restitution::new(0.0)));
                    let speed = if reaches_wall { 0.25 / dt } else { 0.0625 / dt };
                    app.world_mut().entity_mut(head).insert((Position(start_body), SleepingDisabled,
                        LinearDamping(0.0), AngularDamping(0.0), LinearVelocity(Vec3::X * speed), AngularVelocity::ZERO)).remove::<Sleeping>();
                    app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.2, 4.0, 4.0),
                        Transform::from_translation(Vec3::new(0.0, start_center.y, start_center.z)),
                        Friction::new(0.0), Restitution::new(0.0)));
                    app.update();
                    if reaches_wall {
                        let aabb = app.world().get::<ColliderAabb>(shape).unwrap();
                        assert!(aabb.max.x > -0.1, "dt={dt} substeps={substeps} aabb={aabb:?}");
                    }
                    app.update();
                    let final_center = app.world().get::<Position>(head).unwrap().0
                        + app.world().get::<Rotation>(head).unwrap().0 * center_local;
                    let velocity = app.world().get::<LinearVelocity>(head).unwrap().x;
                    let expected = if reaches_wall { -0.2 } else { -0.65 + speed * dt * 2.0 };
                    assert!((final_center.x - expected).abs() < 0.006,
                        "dt={dt} substeps={substeps} reaches={reaches_wall} actual={final_center:?} source={expected}");
                    if reaches_wall { assert!(velocity.abs() < 0.01); }
                    else { assert!((velocity - speed).abs() < 0.01); }
                }
            }
        }
    }

    #[test]
    fn source_ragdoll_hard_contacts_recover_source_penetration() {
        use avian3d::prelude::{AngularVelocity, ColliderDisabled, ComputedCenterOfMass, Gravity, JointDisabled,
            LinearVelocity, Sleeping, SleepingDisabled};
        use bevy::time::TimeUpdateStrategy;
        for dt in [1.0_f32 / 60.0, 1.0 / 30.0] {
            for depth in [0.001_f32, 0.01, 0.1, 0.2] {
                let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
                let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
                app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
                for _ in 0..12 { app.update(); }
                let spawned = app.world().get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap();
                let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
                let joints = spawned.joints.values().copied().collect::<Vec<_>>();
                let head = bodies.iter().find(|(label, _)| label == "head").unwrap().1;
                let profile = app.world().get::<RagdollPhysicsProfile>(player).unwrap();
                let head_index = profile.bodies.iter().find(|body| body.label == "head").unwrap().source_index;
                let shape = colliders.iter().copied().find(|entity| {
                    let label = &app.world().get::<ColliderLabel>(*entity).unwrap().0;
                    find_collider_profile(profile, label).unwrap().source_body_index == head_index
                }).unwrap();
                for entity in colliders { if entity != shape { app.world_mut().entity_mut(entity).insert(ColliderDisabled); } }
                for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
                app.world_mut().insert_resource(Gravity(Vec3::ZERO));
                app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(dt)));
                let local_center = app.world().get::<ComputedCenterOfMass>(head).unwrap().0;
                let position = app.world().get::<Position>(head).unwrap().0;
                let rotation = app.world().get::<Rotation>(head).unwrap().0;
                let center = position + rotation * local_center;
                app.world_mut().entity_mut(shape).insert((Collider::sphere(0.1), Transform::from_translation(local_center),
                    Restitution::new(0.0), Friction::new(0.0)));
                app.world_mut().entity_mut(head).insert((SleepingDisabled, LinearDamping(0.0), AngularDamping(0.0),
                    LinearVelocity::ZERO, AngularVelocity::ZERO)).remove::<Sleeping>();
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                    Transform::from_translation(center - Vec3::Y * (0.2 - depth)), Restitution::new(0.0), Friction::new(0.0)));
                let ordinary = app.world_mut().spawn((RigidBody::Dynamic, Collider::sphere(0.1),
                    Transform::from_translation(center + Vec3::X), SleepingDisabled,
                    Restitution::new(0.0), Friction::new(0.0))).id();
                app.update();
                let moved = app.world().get::<Position>(head).unwrap().y - position.y;
                let ordinary_moved = app.world().get::<Position>(ordinary).unwrap().y - center.y;
                let expected = depth.min(10.0 * dt);
                assert!((moved - expected).abs() < 0.003,
                    "dt={dt} depth={depth} actual={moved} source={expected}");
                assert!(ordinary_moved < expected * 0.8, "ordinary contact must keep its own solver behavior");
            }
        }
    }

    #[test]
    fn source_ragdoll_position_friction_reaches_contact_solver() {
        use avian3d::dynamics::solver::solver_body::SolverBodyInertia;
        use avian3d::prelude::{AngularVelocity, ColliderDisabled, ComputedCenterOfMass, Gravity, JointDisabled,
            LinearVelocity, Sleeping, SleepingDisabled, SubstepSchedule};
        use bevy::time::TimeUpdateStrategy;
        #[derive(Resource, Default)]
        struct ObservedInertia { inverse_mass: f32, inverse_inertia: Mat3, head_suspended: bool, ordinary_retained: bool }
        for dt in [1.0_f32 / 60.0, 1.0 / 30.0] {
            for friction in [0.0_f32, 0.7] {
                let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
                let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
                app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
                for _ in 0..12 { app.update(); }
                let spawned = app.world().get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap();
                let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
                let joints = spawned.joints.values().copied().collect::<Vec<_>>();
                let head = bodies.iter().find(|(label, _)| label == "head").unwrap().1;
                let profile = app.world().get::<RagdollPhysicsProfile>(player).unwrap();
                let head_index = profile.bodies.iter().find(|body| body.label == "head").unwrap().source_index;
                let shape = colliders.iter().copied().find(|entity| {
                    let label = &app.world().get::<ColliderLabel>(*entity).unwrap().0;
                    find_collider_profile(profile, label).unwrap().source_body_index == head_index
                }).unwrap();
                for entity in colliders { if entity != shape { app.world_mut().entity_mut(entity).insert(ColliderDisabled); } }
                for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
                app.world_mut().insert_resource(Gravity(Vec3::ZERO));
                app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(dt)));
                let local_center = app.world().get::<ComputedCenterOfMass>(head).unwrap().0;
                let position = app.world().get::<Position>(head).unwrap().0;
                let rotation = app.world().get::<Rotation>(head).unwrap().0;
                let center = position + rotation * local_center;
                app.world_mut().entity_mut(shape).insert((Collider::sphere(0.1), Transform::from_translation(local_center),
                    Restitution::new(0.0), Friction::new(friction)));
                app.world_mut().entity_mut(head).insert((SleepingDisabled, LinearDamping(0.0), AngularDamping(0.0),
                    LinearVelocity(Vec3::new(1.0, -1.0, 0.0)), AngularVelocity::ZERO)).remove::<Sleeping>();
                app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 0.2, 4.0),
                    Transform::from_translation(center - Vec3::Y * 0.199), Restitution::new(0.0), Friction::new(friction)));
                let ordinary = app.world_mut().spawn((RigidBody::Dynamic, Collider::sphere(0.1),
                    Transform::from_translation(center + Vec3::X), SleepingDisabled,
                    LinearVelocity(Vec3::new(1.0, -1.0, 0.0)), Friction::new(0.7))).id();
                app.init_resource::<ObservedInertia>();
                app.add_systems(SubstepSchedule, (move |bodies: Query<&SolverBodyInertia>,
                    graph: Res<ConstraintGraph>, mut observed: ResMut<ObservedInertia>| {
                    let inertia = bodies.get(head).unwrap();
                    observed.inverse_mass = inertia.effective_inv_mass().x;
                    observed.inverse_inertia = Mat3::from(inertia.effective_inv_angular_inertia());
                    for constraint in graph.colors.iter().flat_map(|color| &color.contact_constraints) {
                        if constraint.body1 == head || constraint.body2 == head {
                            observed.head_suspended |= constraint.points.iter().any(|point| point.tangent_part.is_none());
                        }
                        if constraint.body1 == ordinary || constraint.body2 == ordinary {
                            observed.ordinary_retained |= constraint.points.iter().any(|point| point.tangent_part.is_some());
                        }
                    }
                }).after(condition_profile_solver_inertia)
                    .before(crate::extensions::source_contact_friction::solve_source_position_contacts));
                app.update();
                let observed = app.world().resource::<ObservedInertia>();
                if friction > 0.0 { assert!(observed.head_suspended && observed.ordinary_retained); }
                let arm = Vec3::NEG_Y * 0.1;
                let crosses = [arm.cross(Vec3::X), arm.cross(Vec3::Z)];
                let kx = observed.inverse_mass + crosses[0].dot(observed.inverse_inertia * crosses[0]);
                let kz = observed.inverse_mass + crosses[1].dot(observed.inverse_inertia * crosses[1]);
                let coupling = crosses[0].dot(observed.inverse_inertia * crosses[1]);
                let normal = (0.001 + dt) / observed.inverse_mass;
                let mut impulse = Vec2::ZERO;
                for _ in 0..4 {
                    let error = Vec2::new(dt + kx * impulse.x + coupling * impulse.y,
                        coupling * impulse.x + kz * impulse.y);
                    impulse += Vec2::new(-0.5 * error.x / kx, -0.5 * error.y / kz);
                    impulse = impulse.clamp_length_max(friction * normal);
                }
                let expected = dt + observed.inverse_mass * impulse.x;
                let actual = app.world().get::<Position>(head).unwrap().x - position.x;
                assert!((actual - expected).abs() < 0.001,
                    "dt={dt} friction={friction} actual={actual} source={expected}");
                let normal_speed = app.world().get::<LinearVelocity>(head).unwrap().y;
                assert!(normal_speed.abs() < 1e-4,
                    "dt={dt} friction={friction} source normal speed=0 actual={normal_speed}");
            }
        }
    }

    #[test]
    fn source_ragdoll_linear_damping_follows_acceleration_and_impulse() {
        use avian3d::prelude::{AngularVelocity, ColliderDisabled, ComputedMass, ConstantForce, ConstantLocalForce,
            Forces, Gravity, JointDisabled, LinearVelocity, Sleeping, SleepingDisabled, WriteRigidBodyForces};
        use bevy::{ecs::system::RunSystemOnce, time::TimeUpdateStrategy};

        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
        for _ in 0..12 { app.update(); }
        let spawned = app.world().get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap();
        let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
        let joints = spawned.joints.values().copied().collect::<Vec<_>>();
        for entity in colliders { app.world_mut().entity_mut(entity).insert(ColliderDisabled); }
        for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
        let body = bodies.iter().find(|(label, _)| label == "spine_01").unwrap().1;
        let native_damping = app.world().get::<LinearDamping>(body).unwrap().0;
        assert!((native_damping - 0.01).abs() < 1e-7);
        app.world_mut().entity_mut(body).insert(SleepingDisabled).remove::<Sleeping>();
        let cases = [
            (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO),
            (Vec3::X * 60.0, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO),
            (Vec3::X * 5.0, Vec3::NEG_Y * 9.81, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO),
            (Vec3::ZERO, Vec3::ZERO, Vec3::X * 50.0, Vec3::ZERO, Vec3::ZERO),
            (Vec3::ZERO, Vec3::NEG_Y * 9.81, Vec3::ZERO, Vec3::Z * 50.0, Vec3::X),
        ];
        for frame_dt in [1.0_f32 / 60.0, 1.0 / 30.0, 0.1] {
            for damping in [0.0, native_damping, 100.0] {
                for (initial, gravity, force, local_force, impulse) in cases {
                    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(frame_dt)));
                    app.world_mut().insert_resource(Gravity(gravity));
                    app.world_mut().entity_mut(body).insert((LinearVelocity(initial), AngularVelocity::ZERO,
                        LinearDamping(damping), ConstantForce(force), ConstantLocalForce(local_force))).remove::<Sleeping>();
                    let inverse_mass = app.world().get::<ComputedMass>(body).unwrap().inverse();
                    let rotation = app.world().get::<Rotation>(body).unwrap().0;
                    let before = app.world().get::<Position>(body).unwrap().0;
                    app.world_mut().run_system_once(move |mut forces: Query<Forces>| {
                        forces.get_mut(body).unwrap().apply_linear_impulse(impulse);
                    }).unwrap();
                    app.update();
                    let dt = frame_dt.min(crate::source_physics_timing::SOURCE_MAX_PHYSICS_DELTA.as_secs_f32());
                    let expected = (initial + impulse * inverse_mass + (gravity + (force + rotation * local_force) * inverse_mass) * dt)
                        * (1.0 - damping * dt).max(0.0);
                    let actual = app.world().get::<LinearVelocity>(body).unwrap().0;
                    let displacement = app.world().get::<Position>(body).unwrap().0 - before;
                    assert!(actual.abs_diff_eq(expected, 1e-5),
                        "frame_dt={frame_dt} damping={damping} actual={actual:?} source={expected:?}");
                    assert!(displacement.abs_diff_eq(expected * dt, 2e-5));
                }
            }
        }
    }

    #[test]
    fn source_ragdoll_damping_preserves_other_integrators() {
        use avian3d::prelude::{CustomVelocityIntegration, Gravity, LinearVelocity, PhysicsPlugins, RigidBody, SubstepCount};
        use bevy::time::TimeUpdateStrategy;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
            PhysicsPlugins::new(PostUpdate), RagdollPhysicsProfilePlugin))
            .insert_resource(Gravity(Vec3::ZERO)).insert_resource(SubstepCount(1))
            .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 30.0)));
        app.finish(); app.cleanup();
        let free = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY,
            LinearVelocity(Vec3::X * 5.0), LinearDamping(0.5))).id();
        let owned = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY,
            LinearVelocity(Vec3::X * 5.0), LinearDamping(0.5), RagdollActiveMode(BodyMode::Dynamic), CustomVelocityIntegration)).id();
        app.update();
        for entity in [free, owned] { app.world_mut().entity_mut(entity).insert(LinearVelocity(Vec3::X * 5.0)); }
        app.update();
        let implicit = 5.0 / (1.0 + 0.5 / 30.0);
        assert!((app.world().get::<LinearVelocity>(free).unwrap().x - implicit).abs() < 1e-5);
        assert_eq!(app.world().get::<LinearVelocity>(owned).unwrap().0, Vec3::X * 5.0);
    }

    #[test]
    fn source_ragdoll_free_rotation_uses_one_physics_frame() {
        use avian3d::prelude::{AngularVelocity, ColliderDisabled, Gravity, JointDisabled, Physics, Rotation, Sleeping, SleepingDisabled};
        use bevy::{ecs::system::RunSystemOnce, time::TimeUpdateStrategy};
        #[derive(Resource, Default)]
        struct ObservedPhysicsDelta(f32);

        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
        for _ in 0..12 { app.update(); }
        let graph = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        let spawned = graph.get_spawned_ragdoll().unwrap();
        let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
        let joints = spawned.joints.values().copied().collect::<Vec<_>>();
        let body = bodies.iter().find(|(label, _)| label == "spine_01").unwrap().1;
        for entity in colliders { app.world_mut().entity_mut(entity).insert(ColliderDisabled); }
        for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
        app.world_mut().insert_resource(Gravity(Vec3::ZERO));
        app.world_mut().entity_mut(body).insert(SleepingDisabled).remove::<Sleeping>();
        app.init_resource::<ObservedPhysicsDelta>();
        app.add_systems(PhysicsSchedule, (|time: Res<Time<Physics>>, mut observed: ResMut<ObservedPhysicsDelta>| {
            observed.0 = time.delta_secs();
        }).in_set(SolverSystems::PreSubstep));
        let axis = Vec3::new(0.2, 0.8, -0.3).normalize();
        let initial = Quat::from_euler(EulerRot::YXZ, 0.3, -0.2, 0.4);
        let maximum = app.world().get::<MaxAngularSpeed>(body).unwrap().0;
        for frame_dt in [1.0_f32 / 60.0, 1.0 / 30.0, 0.1] {
            for speed in [0.0_f32, 0.1, 30.0, 60.0, 80.0] {
                app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(frame_dt)));
                let parent = app.world().get::<ChildOf>(body).unwrap().parent();
                let parent_rotation = app.world_mut().run_system_once(move |transforms: bevy::transform::helper::TransformHelper| {
                    transforms.compute_global_transform(parent).unwrap().rotation()
                }).unwrap();
                app.world_mut().get_mut::<Transform>(body).unwrap().rotation = parent_rotation.conjugate() * initial;
                app.world_mut().entity_mut(body).insert((Rotation(initial), AngularVelocity(axis * speed))).remove::<Sleeping>();
                app.update();
                let dt = app.world().resource::<ObservedPhysicsDelta>().0;
                assert!((dt - frame_dt.min(crate::source_physics_timing::SOURCE_MAX_PHYSICS_DELTA.as_secs_f32())).abs() < 1e-6);
                let angular = axis * speed.min(maximum);
                let delta = angular * (0.5 * dt);
                let expected = (Quat::from_xyzw(delta.x, delta.y, delta.z, 1.0) * initial).normalize();
                let actual = app.world().get::<Rotation>(body).unwrap().0;
                let error = (actual - expected).length().min((actual + expected).length());
                assert!(error < 1e-5, "frame_dt={frame_dt} speed={speed} dt={dt} actual={actual:?} source={expected:?}");
            }
        }
    }

    #[test]
    fn source_ragdoll_rotation_preserves_other_integration_owners() {
        use avian3d::prelude::{AngularVelocity, CustomPositionIntegration, Gravity, LinearVelocity, PhysicsPlugins, Position, RigidBody, Rotation, SubstepCount};
        use bevy::time::TimeUpdateStrategy;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
            PhysicsPlugins::new(PostUpdate), RagdollPhysicsProfilePlugin))
            .insert_resource(Gravity(Vec3::ZERO)).insert_resource(SubstepCount(6))
            .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        app.finish(); app.cleanup();
        let velocity = Vec3::new(1.0, 2.0, 3.0);
        let free = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY,
            LinearVelocity(velocity), AngularVelocity(Vec3::X * 30.0))).id();
        let owned = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY, AngularVelocity(Vec3::X * 30.0),
            RagdollActiveMode(BodyMode::Dynamic), CustomPositionIntegration)).id();
        let ragdoll = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY, LinearVelocity(velocity),
            AngularVelocity(Vec3::X * 30.0), RagdollActiveMode(BodyMode::Dynamic))).id();
        app.update();
        for entity in [free, ragdoll] {
            app.world_mut().entity_mut(entity).insert((Position(Vec3::ZERO), Rotation::IDENTITY, Transform::IDENTITY,
                LinearVelocity(velocity), AngularVelocity(Vec3::X * 30.0)));
        }
        app.world_mut().entity_mut(owned).insert((Rotation::IDENTITY, Transform::IDENTITY, AngularVelocity(Vec3::X * 30.0)));
        app.update();
        let expected = Quat::from_scaled_axis(Vec3::X * 0.5);
        assert!(app.world().get::<Rotation>(free).unwrap().0.abs_diff_eq(expected, 1e-5));
        let source = Quat::from_xyzw(0.25, 0.0, 0.0, 1.0).normalize();
        assert!(app.world().get::<Rotation>(ragdoll).unwrap().0.abs_diff_eq(source, 1e-5));
        assert!(app.world().get::<Rotation>(owned).unwrap().0.abs_diff_eq(Quat::IDENTITY, 1e-5));
        assert!(app.world().get::<CustomPositionIntegration>(owned).is_some());
        assert!(app.world().get::<Position>(ragdoll).unwrap().0.abs_diff_eq(app.world().get::<Position>(free).unwrap().0, 1e-6));
    }

    #[test]
    fn live_ragdoll_scale_matches_source_capsules_and_body_mass() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::{definition::Ragdoll, spawning::spawn_ragdoll_avian};
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../docs/audit/bf144/d73/live.runtime.json")).unwrap();
        let ragdoll: Ragdoll = ron::from_str(include_str!("../../assets/ALS/Character/PA_Als.rag.ron")).unwrap();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate), crate::extensions::SourcePhysicsJointPlugin));
        app.finish(); app.cleanup();
        let world = app.world_mut();
        let player = world.spawn((AnimationGraphPlayer::new(Handle::default()),
            crate::pa_als::parse_pa_als_profile(), Transform::IDENTITY)).id();
        let spawned = world.run_system_once(move |mut commands: Commands| {
            spawn_ragdoll_avian(player, &ragdoll, Isometry3d::IDENTITY, None, &mut commands)
        }).unwrap();
        let bodies = spawned.bodies.values().map(|entity| (world.get::<BodyLabel>(*entity).unwrap().0.clone(), *entity))
            .collect::<Vec<_>>();
        let colliders = spawned.colliders.values().map(|entity| (world.get::<ColliderLabel>(*entity).unwrap().0.clone(), *entity))
            .collect::<Vec<_>>();
        let mut graph = world.get_mut::<AnimationGraphPlayer>(player).unwrap();
        let ReflectMut::Struct(fields) = graph.reflect_mut() else { panic!("player must be a struct") };
        *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
        for sample in fixture["samples"].as_array().unwrap() {
            let scale = sample["scale"][0].as_f64().unwrap() as f32;
            world.get_mut::<Transform>(player).unwrap().scale = Vec3::splat(scale);
            world.run_system_once(apply_ragdoll_physics_profiles).unwrap();
            for expected in sample["capsules"].as_array().unwrap() {
                let label = expected["label"].as_str().unwrap();
                let entity = colliders.iter().find(|(name, _)| name == label).unwrap().1;
                let shape = world.get::<Collider>(entity).unwrap().shape_scaled();
                let capsule = shape.as_capsule().unwrap();
                let radius = expected["radius_m"].as_f64().unwrap() as f32;
                let half_length = expected["half_length_m"].as_f64().unwrap() as f32;
                assert!((capsule.radius - radius).abs() < 1e-7, "scale={scale} collider={label} radius={}", capsule.radius);
                assert!((capsule.segment.a.distance(capsule.segment.b) * 0.5 - half_length).abs() < 1e-7,
                    "scale={scale} collider={label} half_length={}", capsule.segment.a.distance(capsule.segment.b) * 0.5);
            }
            for label in ["pelvis", "spine_03", "foot_l", "root"] {
                let expected = sample["bodies"].as_array().unwrap().iter().find(|body| body["bone_name"] == label).unwrap();
                let entity = bodies.iter().find(|(name, _)| name == label).unwrap().1;
                let expected_mass = expected["mass_kg"].as_f64().unwrap() as f32;
                let mass = world.get::<Mass>(entity).unwrap().0;
                let center = Vec3::new(expected["center_m"][0].as_f64().unwrap() as f32,
                    expected["center_m"][1].as_f64().unwrap() as f32, expected["center_m"][2].as_f64().unwrap() as f32);
                assert!((mass - expected_mass).abs() < expected_mass * 1e-4, "scale={scale} body={label} mass={mass}");
                assert!(world.get::<CenterOfMass>(entity).unwrap().0.abs_diff_eq(center, 1e-7),
                    "scale={scale} body={label} center={:?} native={center:?}", world.get::<CenterOfMass>(entity).unwrap().0);
            }
        }
        assert!(world.get::<RagdollPhysicsProfileApplied>(player).unwrap().live_scale_applied);
    }

    #[test]
    fn tiny_initial_ragdoll_mass_uses_clamped_shapes_and_preserves_live_updates() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::{definition::Ragdoll, spawning::spawn_ragdoll_avian};

        fn replace_ragdoll(app: &mut App, player: Entity) -> Vec<(String, Entity)> {
            let ragdoll: Ragdoll = ron::from_str(include_str!("../../assets/ALS/Character/PA_Als.rag.ron")).unwrap();
            let spawned = app.world_mut().run_system_once(move |mut commands: Commands| {
                spawn_ragdoll_avian(player, &ragdoll, Isometry3d::IDENTITY, None, &mut commands)
            }).unwrap();
            let bodies = spawned.bodies.values().map(|entity| {
                (app.world().get::<BodyLabel>(*entity).unwrap().0.clone(), *entity)
            }).collect();
            let mut graph = app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap();
            let ReflectMut::Struct(fields) = graph.reflect_mut() else { panic!("player must be a struct") };
            *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
            bodies
        }

        fn check(world: &World, bodies: &[(String, Entity)], fixture: &serde_json::Value, scale: f32) {
            let sample = fixture["samples"].as_array().unwrap().iter()
                .find(|sample| (sample["scale"][0].as_f64().unwrap() as f32 - scale).abs() < 1e-6).unwrap();
            for label in ["pelvis", "spine_03", "foot_l", "root"] {
                let expected = sample["bodies"].as_array().unwrap().iter().find(|body| body["bone_name"] == label).unwrap();
                let body = bodies.iter().find(|(name, _)| name == label).unwrap().1;
                let mass = world.get::<Mass>(body).unwrap().0;
                let expected_mass = expected["mass_kg"].as_f64().unwrap() as f32;
                assert!((mass - expected_mass).abs() < expected_mass * 1e-4, "scale={scale} body={label} mass={mass} native={expected_mass}");
                let vector = |key: &str| Vec3::new(expected[key][0].as_f64().unwrap() as f32,
                    expected[key][1].as_f64().unwrap() as f32, expected[key][2].as_f64().unwrap() as f32);
                if label == "spine_03" && fixture["rescale_from_unit"] == true { continue; }
                let center = world.get::<CenterOfMass>(body).unwrap().0;
                assert!(center.abs_diff_eq(vector("center_m"), 1e-7), "scale={scale} body={label} center={center:?}");
                if label == "spine_03" { continue; }
                let rotation = &expected["rotation_xyzw"];
                let frame = Quat::from_xyzw(rotation[0].as_f64().unwrap() as f32, rotation[1].as_f64().unwrap() as f32,
                    rotation[2].as_f64().unwrap() as f32, rotation[3].as_f64().unwrap() as f32);
                let basis = Mat3::from_quat(frame);
                let native = basis * Mat3::from_diagonal(vector("principal_kg_m2")) * basis.transpose();
                let inertia = world.get::<AngularInertia>(body).unwrap();
                let basis = Mat3::from_quat(inertia.local_frame);
                let actual = basis * Mat3::from_diagonal(inertia.principal) * basis.transpose();
                let largest = native.x_axis.abs().max(native.y_axis.abs()).max(native.z_axis.abs()).max_element();
                assert!(actual.abs_diff_eq(native, largest * 1e-4), "scale={scale} body={label} actual={actual:?} native={native:?}");
            }
        }

        let initial: serde_json::Value = serde_json::from_str(include_str!("../../docs/audit/bf144/d72/initial.runtime.json")).unwrap();
        let live: serde_json::Value = serde_json::from_str(include_str!("../../docs/audit/bf144/d72/live.runtime.json")).unwrap();
        for starting_scale in [0.01_f32, 0.2, 1.0, 2.0] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate), RagdollPhysicsProfilePlugin))
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::ZERO));
            app.finish(); app.cleanup();
            let player = app.world_mut().spawn((AnimationGraphPlayer::new(Handle::default()),
                crate::pa_als::parse_pa_als_profile(), Transform::from_scale(Vec3::splat(starting_scale)))).id();
            let bodies = replace_ragdoll(&mut app, player);
            app.update();
            check(app.world(), &bodies, &initial, starting_scale);
            assert!(!app.world().get::<RagdollPhysicsProfileApplied>(player).unwrap().live_scale_applied);
            if starting_scale == 0.01 {
                app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().bodies[0].linear_damping += 0.0;
                app.update();
                check(app.world(), &bodies, &initial, starting_scale);
                assert!(!app.world().get::<RagdollPhysicsProfileApplied>(player).unwrap().live_scale_applied);
            }
            if starting_scale == 1.0 {
                app.world_mut().get_mut::<Transform>(player).unwrap().scale = Vec3::splat(0.01);
                app.update();
                check(app.world(), &bodies, &live, 0.01);
                assert!(app.world().get::<RagdollPhysicsProfileApplied>(player).unwrap().live_scale_applied);
                let replacement = replace_ragdoll(&mut app, player);
                app.update();
                check(app.world(), &replacement, &initial, 0.01);
                assert!(!app.world().get::<RagdollPhysicsProfileApplied>(player).unwrap().live_scale_applied);
            }
        }
    }

    #[test]
    fn angular_inertia_uses_source_relative_and_unit_thresholds() {
        for scale in [1.0_f32, 0.001, 0.000001, 2e-12] {
            for (off_diagonal, diagonalized) in [(0.00009_f32, false), (0.00011, true), (0.2, true)] {
                let matrix = Mat3::from_cols(
                    Vec3::new(1.2, 0.0, off_diagonal), Vec3::new(0.0, 0.8, 0.0),
                    Vec3::new(off_diagonal, 0.0, 1.0)) * scale;
                let inertia = source_angular_inertia(matrix);
                let basis = Mat3::from_quat(inertia.local_frame);
                let actual = basis * Mat3::from_diagonal(inertia.principal) * basis.transpose();
                let expected = if diagonalized { off_diagonal * scale } else { 0.0 };
                assert!((actual.x_axis.z - expected).abs() < scale * 2e-5,
                    "scale={scale} off_diagonal={off_diagonal} actual={}", actual.x_axis.z);
            }
        }
        let tiny = Mat3::from_cols(Vec3::new(1.2, 0.0, 0.2), Vec3::new(0.0, 0.8, 0.0),
            Vec3::new(0.2, 0.0, 1.0)) * 0.5e-12;
        let inertia = source_angular_inertia(tiny);
        assert_eq!(inertia.local_frame, Quat::IDENTITY);
        assert_eq!(inertia.principal, Vec3::new(1.2, 0.8, 1.0) * 0.5e-12);
    }

    #[test]
    fn scaled_ragdoll_body_inertia_matches_native_frames() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::ragdoll::{definition::Ragdoll, spawning::spawn_ragdoll_avian};
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d71/body_probe.runtime.json")).unwrap();
        for sample in fixture["samples"].as_array().unwrap() {
            let scale = sample["scale"][0].as_f64().unwrap() as f32;
            let profile = crate::pa_als::parse_pa_als_profile();
            let ragdoll: Ragdoll = ron::from_str(include_str!("../../assets/ALS/Character/PA_Als.rag.ron")).unwrap();
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate), crate::extensions::SourcePhysicsJointPlugin));
            app.finish(); app.cleanup();
            let world = app.world_mut();
            let player = world.spawn((AnimationGraphPlayer::new(Handle::default()), Transform::IDENTITY)).id();
            let spawned = world.run_system_once(move |mut commands: Commands| {
                spawn_ragdoll_avian(player, &ragdoll, Isometry3d::IDENTITY, None, &mut commands)
            }).unwrap();
            let bodies = spawned.bodies.values().map(|entity| {
                (world.get::<BodyLabel>(*entity).unwrap().0.clone(), *entity)
            }).collect::<Vec<_>>();
            world.run_system_once(move |mut commands: Commands,
                body_labels: Query<&BodyLabel>, collider_labels: Query<&ColliderLabel>, joint_labels: Query<&JointLabel>,
                spherical: Query<&SphericalJoint>, revolute: Query<&RevoluteJoint>, source_joints: Query<&SourcePhysicsJoint>,
                transforms: Query<&Transform>, mut colliders: Query<&mut Collider>| {
                apply_profile_to_spawned_ragdoll(&mut commands, &spawned, &profile, Vec3::splat(scale), Vec3::splat(scale), true, true,
                    &body_labels, &collider_labels, &joint_labels, &spherical, &revolute, &source_joints, &transforms, &mut colliders)
            }).unwrap();
            for label in ["pelvis", "spine_03", "head", "hand_l", "hand_r"] {
                let expected = sample["bodies"].as_array().unwrap().iter().find(|body| body["bone_name"] == label).unwrap();
                let vector = |key: &str| Vec3::new(expected[key][0].as_f64().unwrap() as f32,
                    expected[key][1].as_f64().unwrap() as f32, expected[key][2].as_f64().unwrap() as f32);
                let rotation = &expected["rotation_xyzw"];
                let frame = Quat::from_xyzw(rotation[0].as_f64().unwrap() as f32, rotation[1].as_f64().unwrap() as f32,
                    rotation[2].as_f64().unwrap() as f32, rotation[3].as_f64().unwrap() as f32);
                let basis = Mat3::from_quat(frame);
                let native = basis * Mat3::from_diagonal(vector("principal_kg_m2")) * basis.transpose();
                let body = bodies.iter().find(|(name, _)| name == label).unwrap().1;
                let inertia = world.get::<AngularInertia>(body).unwrap();
                let basis = Mat3::from_quat(inertia.local_frame);
                let actual = basis * Mat3::from_diagonal(inertia.principal) * basis.transpose();
                let largest = native.x_axis.abs().max(native.y_axis.abs()).max(native.z_axis.abs()).max_element();
                assert!(actual.abs_diff_eq(native, largest * 1e-4), "scale={scale} body={label} actual={actual:?} native={native:?}");
                if scale == 0.2 && label == "head" {
                    let impulse = Vec3::X * native.x_axis.x;
                    let expected_response = native.inverse() * impulse;
                    let actual_response = actual.inverse() * impulse;
                    assert!(expected_response.z.abs() > 0.1);
                    assert!((actual_response.z - expected_response.z).abs() < 3e-4,
                        "actual={actual_response:?} native={expected_response:?}");
                }
            }
        }
    }

    fn source_sleep_fixture(dt: f32, substeps: u32, linear: f32, angular: f32, counter: i32,
        velocity: Vec3, spin: Vec3) -> (App, Entity) {
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::definition::Body;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate), RagdollPhysicsProfilePlugin))
            .insert_resource(avian3d::prelude::Gravity(Vec3::ZERO))
            .insert_resource(avian3d::prelude::SubstepCount(substeps))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(dt)));
        app.finish(); app.cleanup();
        let mut profile = RagdollPhysicsProfile::from_json(include_str!("../../assets/ALS/Character/PA_Als.physics.json")).unwrap();
        profile.bodies.retain(|body| body.label == "pelvis");
        profile.colliders.clear(); profile.joints.clear();
        let source = &mut profile.bodies[0];
        source.linear_damping = 0.0; source.angular_damping = 0.0;
        source.sleeping_disabled = linear <= 0.0 && angular <= 0.0;
        source.sleep.linear_threshold_m_per_second = linear;
        source.sleep.angular_threshold_radians_per_second = angular;
        source.sleep.counter_threshold = counter;
        let root = app.world_mut().spawn_empty().id();
        let body = app.world_mut().spawn((BodyLabel("pelvis".into()), RigidBody::Dynamic, Transform::IDENTITY,
            Mass(1.0), NoAutoMass, avian3d::prelude::LinearVelocity(velocity), avian3d::prelude::AngularVelocity(spin))).id();
        let mut spawned = SpawnedRagdoll::new(root);
        spawned.bodies.insert(Body::new().id, body);
        let mut player = AnimationGraphPlayer::new(Handle::default());
        let ReflectMut::Struct(fields) = player.reflect_mut() else { panic!("Player must expose struct fields") };
        *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
        app.world_mut().spawn((player, profile, Transform::IDENTITY));
        app.update();
        assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 0);
        (app, body)
    }

    #[test]
    fn source_sleep_thresholds_counts_and_physics_steps() {
        use avian3d::prelude::Sleeping;
        for dt in [1.0 / 30.0, 1.0 / 120.0] {
            for substeps in [1, 4] {
                for (linear, angular, counter, speed, spin, first_sleep) in [
                    (0.01, 0.05, 4, 0.03, 0.0, None),
                    (0.01, 0.05, 4, 0.01, 0.05, Some(5)),
                    (0.01, 0.05, 4, 0.01001, 0.0, None),
                    (0.01, 0.05, 4, 0.0, 0.05001, None),
                    (0.0, 0.05, 4, 0.0, 0.04, Some(5)),
                    (0.01, 0.0, 4, 0.005, 0.0, Some(5)),
                    (0.0, 0.0, 4, 0.0, 0.0, None),
                    (0.02, 0.1, 4, 0.015, 0.07, Some(5)),
                    (0.01, 0.05, 0, 0.0, 0.0, Some(1)),
                    (0.01, 0.05, 1, 0.0, 0.0, Some(2)),
                    (0.01, 0.05, 9, 0.0, 0.0, Some(10)),
                    (0.01, 0.05, 127, 0.0, 0.0, None),
                ] {
                    let (mut app, body) = source_sleep_fixture(dt, substeps, linear, angular, counter,
                        Vec3::X * speed, Vec3::Y * spin);
                    for step in 1..=140 {
                        app.update();
                        assert_eq!(app.world().get::<Sleeping>(body).is_some(), first_sleep.is_some_and(|first| step >= first),
                            "Source sleep step: dt={dt} substeps={substeps} thresholds=({linear},{angular}) counter={counter} velocity=({speed},{spin}) step={step}");
                        if app.world().get::<Sleeping>(body).is_some() { break; }
                    }
                }
            }
        }
    }

    #[test]
    fn source_sleep_external_velocity_resets_awake_island_counter() {
        use avian3d::prelude::{AngularVelocity, LinearVelocity, Sleeping};
        for dt in [1.0 / 60.0, 1.0 / 30.0] {
            for connected in [false, true] {
                for angular in [false, true] {
                    let (mut app, first) = source_sleep_fixture(dt, 1, 0.01, 0.05, 4, Vec3::ZERO, Vec3::ZERO);
                    let mut bodies = vec![first];
                    if connected {
                        let settings = *app.world().get::<SourceBodySleep>(first).unwrap();
                        let second = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY, Mass(1.0), NoAutoMass,
                            LinearVelocity::ZERO, AngularVelocity::ZERO, settings,
                            avian3d::prelude::SleepThreshold { linear: -1.0, angular: -1.0 })).id();
                        app.world_mut().spawn(SphericalJoint::new(first, second));
                        bodies.push(second);
                    }
                    for _ in 0..3 { app.update(); }
                    for entity in &bodies {
                        assert_eq!(app.world().get::<SourceBodySleep>(*entity).unwrap().counter, 3);
                        if angular { app.world_mut().entity_mut(*entity).insert(AngularVelocity(Vec3::X * 0.005)); }
                        else { app.world_mut().entity_mut(*entity).insert(LinearVelocity(Vec3::X * 0.005)); }
                    }
                    let first_sleep = if connected { 9 } else { 8 };
                    for step in 4..=first_sleep {
                        app.update();
                        for entity in &bodies {
                            assert_eq!(app.world().get::<Sleeping>(*entity).is_some(), step == first_sleep,
                                "dt={dt} connected={connected} angular={angular} step={step}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn source_sleep_velocity_wake_uses_componentwise_source_tolerance() {
        use avian3d::prelude::{AngularVelocity, LinearVelocity};
        for (linear, angular, wakes) in [
            (Vec3::ZERO, Vec3::ZERO, false),
            (Vec3::splat(0.9e-6), Vec3::ZERO, false),
            (Vec3::X * 1.1e-6, Vec3::ZERO, true),
            (Vec3::ZERO, Vec3::splat(0.9e-4), false),
            (Vec3::ZERO, Vec3::X * 1.1e-4, true),
        ] {
            let (mut app, body) = source_sleep_fixture(1.0 / 60.0, 1, 0.01, 0.05, 4, Vec3::ZERO, Vec3::ZERO);
            for _ in 0..3 { app.update(); }
            app.world_mut().entity_mut(body).insert((LinearVelocity(linear), AngularVelocity(angular)));
            app.update();
            assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, if wakes { 1 } else { 4 },
                "linear={linear:?} angular={angular:?}");
        }
    }

    #[test]
    fn source_sleep_repeated_same_velocity_write_wakes_again() {
        use avian3d::prelude::LinearVelocity;
        let (mut app, body) = source_sleep_fixture(1.0 / 60.0, 1, 0.01, 0.05, 4, Vec3::ZERO, Vec3::ZERO);
        for _ in 0..3 { app.update(); }
        app.world_mut().entity_mut(body).insert(LinearVelocity(Vec3::X * 0.005));
        for _ in 0..3 { app.update(); }
        assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 3);
        app.world_mut().entity_mut(body).insert(LinearVelocity(Vec3::X * 0.005));
        app.update();
        assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 1);
    }

    #[test]
    fn source_sleep_captures_velocity_writes_after_narrow_phase() {
        use avian3d::prelude::{AngularVelocity, LinearVelocity, PhysicsStepSystems};
        #[derive(Resource, Default)]
        struct WriteVelocity(bool);
        let (mut app, body) = source_sleep_fixture(1.0 / 60.0, 1, 0.01, 0.05, 4, Vec3::ZERO, Vec3::ZERO);
        app.world_mut().entity_mut(body).insert(AngularVelocity(Vec3::X * 0.005));
        app.init_resource::<WriteVelocity>();
        app.add_systems(PhysicsSchedule, (move |mut write: ResMut<WriteVelocity>, mut velocities: Query<&mut LinearVelocity>| {
            if write.0 {
                velocities.get_mut(body).unwrap().0 = Vec3::ZERO;
                write.0 = false;
            }
        }).after(PhysicsStepSystems::NarrowPhase).before(capture_source_sleep_wakes));
        for _ in 0..3 { app.update(); }
        assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 3);
        app.world_mut().resource_mut::<WriteVelocity>().0 = true;
        app.update();
        assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 1);
    }

    #[test]
    fn source_sleep_seeds_connected_island_from_external_acceleration() {
        use avian3d::prelude::{AngularVelocity, ConstantAngularAcceleration, ConstantLinearAcceleration,
            ConstantLocalAngularAcceleration, ConstantLocalLinearAcceleration, LinearVelocity, Sleeping};
        for dt in [1.0_f32 / 60.0, 1.0 / 30.0] {
            for (world_linear, local_linear, world_angular, local_angular) in [
                (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO),
                (Vec3::X, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO),
                (Vec3::ZERO, Vec3::X, Vec3::ZERO, Vec3::ZERO),
                (Vec3::ZERO, Vec3::ZERO, Vec3::X * 6.0, Vec3::ZERO),
                (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::X * 6.0),
            ] {
                let (mut app, first) = source_sleep_fixture(dt, 1, 0.01, 0.05, 4, Vec3::ZERO, Vec3::ZERO);
                app.world_mut().entity_mut(first).insert((CenterOfMass(Vec3::ZERO), NoAutoCenterOfMass));
                let settings = *app.world().get::<SourceBodySleep>(first).unwrap();
                let second = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY, Mass(1.0), NoAutoMass,
                    LinearVelocity::ZERO, AngularVelocity::ZERO, settings,
                    avian3d::prelude::SleepThreshold { linear: -1.0, angular: -1.0 })).id();
                app.world_mut().spawn(SphericalJoint::new(first, second));
                for _ in 0..3 { app.update(); }
                for entity in [first, second] {
                    app.world_mut().entity_mut(entity).insert((ConstantLinearAcceleration(world_linear),
                        ConstantLocalLinearAcceleration(local_linear), ConstantAngularAcceleration(world_angular),
                        ConstantLocalAngularAcceleration(local_angular)));
                }
                app.update();
                let solver = app.world().get::<SolverBody>(first).unwrap();
                let delta = solver.delta_rotation.0;
                let angular_step = Vec3::new(delta.x, delta.y, delta.z) * (2.0 / dt);
                let expected_linear = ((world_linear + local_linear) * (0.3 / 30.0)).lerp(solver.delta_position / dt, 0.3);
                let expected_angular = ((world_angular + local_angular) * (0.3 / 30.0)).lerp(angular_step, 0.3);
                let sleep = app.world().get::<SourceBodySleep>(first).unwrap();
                assert!(sleep.smooth_linear.abs_diff_eq(expected_linear, 1e-5),
                    "dt={dt} linear={:?} source={expected_linear:?}", sleep.smooth_linear);
                assert!(sleep.smooth_angular.abs_diff_eq(expected_angular, 1e-5),
                    "dt={dt} angular={:?} source={expected_angular:?}", sleep.smooth_angular);
                if world_linear == Vec3::ZERO && local_linear == Vec3::ZERO
                    && world_angular == Vec3::ZERO && local_angular == Vec3::ZERO {
                    assert_eq!(sleep.counter, 4);
                    app.update();
                    assert!(app.world().get::<Sleeping>(first).is_some());
                } else {
                    assert_eq!(sleep.counter, 0);
                }
            }
        }
    }

    #[test]
    fn source_sleep_loaded_ragdoll_seeds_force_before_gravity() {
        use avian3d::prelude::{AngularVelocity, ColliderDisabled, ComputedMass, ConstantForce, Gravity, JointDisabled,
            LinearVelocity, Sleeping, SleepingDisabled};
        #[derive(Resource, Default)]
        struct ObservedSleepVelocity(Vec3);
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
        for _ in 0..12 { app.update(); }
        let spawned = app.world().get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap();
        let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
        let joints = spawned.joints.values().copied().collect::<Vec<_>>();
        for entity in colliders { app.world_mut().entity_mut(entity).insert(ColliderDisabled); }
        for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
        let body = bodies.iter().find(|(label, _)| label == "hand_l").unwrap().1;
        let mass = app.world().get::<ComputedMass>(body).unwrap().value();
        app.world_mut().entity_mut(body).insert(SleepingDisabled).remove::<Sleeping>();
        app.init_resource::<ObservedSleepVelocity>();
        app.add_systems(PhysicsSchedule, (move |bodies: Query<&SourceBodySleep>, mut observed: ResMut<ObservedSleepVelocity>| {
            observed.0 = bodies.get(body).unwrap().smooth_linear;
        }).in_set(IntegrationSystems::UpdateVelocityIncrements)
            .after(seed_source_sleep_from_forces).before(pre_process_velocity_increments));
        app.world_mut().insert_resource(Gravity(Vec3::NEG_Y * 9.81));
        for acceleration in [Vec3::ZERO, Vec3::X] {
            app.world_mut().entity_mut(body).insert((LinearVelocity::ZERO, AngularVelocity::ZERO,
                ConstantForce(acceleration * mass)));
            app.world_mut().get_mut::<SourceBodySleep>(body).unwrap().smooth_linear = Vec3::ZERO;
            app.update();
            let actual = app.world().resource::<ObservedSleepVelocity>().0;
            let expected = acceleration * (0.3 / 30.0);
            assert!(actual.abs_diff_eq(expected, 1e-5), "actual={actual:?} source={expected:?}");
        }
    }

    #[test]
    fn source_sleep_loaded_ragdoll_resets_awake_counter() {
        use avian3d::prelude::{AngularVelocity, ColliderDisabled, Gravity, JointDisabled, LinearVelocity, Sleeping};
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
        for _ in 0..12 { app.update(); }
        let spawned = app.world().get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap();
        let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
        let joints = spawned.joints.values().copied().collect::<Vec<_>>();
        for entity in colliders { app.world_mut().entity_mut(entity).insert(ColliderDisabled); }
        for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
        app.world_mut().insert_resource(Gravity(Vec3::ZERO));
        let body = bodies.iter().find(|(label, _)| label == "hand_l").unwrap().1;
        app.world_mut().entity_mut(body).insert((LinearVelocity::ZERO, AngularVelocity::ZERO)).remove::<Sleeping>();
        let mut sleep = app.world_mut().get_mut::<SourceBodySleep>(body).unwrap();
        sleep.counter = 0;
        sleep.smooth_linear = Vec3::ZERO;
        sleep.smooth_angular = Vec3::ZERO;
        for _ in 0..3 { app.update(); }
        assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 3);
        app.world_mut().entity_mut(body).insert(LinearVelocity(Vec3::X * 0.005));
        for step in 4..=8 {
            app.update();
            assert_eq!(app.world().get::<Sleeping>(body).is_some(), step == 8, "step={step}");
        }
    }

    #[test]
    fn source_sleep_connected_island_uses_longest_counter_and_all_members() {
        use avian3d::prelude::{LinearVelocity, Sleeping};
        for moving in [false, true] {
            let speed = if moving { 0.03 } else { 0.0 };
            let (mut app, first) = source_sleep_fixture(1.0 / 60.0, 1, 0.01, 0.05, 1, Vec3::X * speed, Vec3::ZERO);
            let mut settings = *app.world().get::<SourceBodySleep>(first).unwrap();
            settings.counter_threshold = 7;
            settings.linear_threshold_m_per_second = 0.05;
            let second = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY, Mass(1.0), NoAutoMass,
                LinearVelocity(Vec3::X * speed), settings, avian3d::prelude::SleepThreshold { linear: -1.0, angular: -1.0 })).id();
            app.world_mut().spawn(SphericalJoint::new(first, second));
            for step in 1..=12 {
                app.update();
                let first_island = app.world().get::<BodyIslandNode>(first).unwrap().island_id();
                assert_eq!(first_island, app.world().get::<BodyIslandNode>(second).unwrap().island_id());
                assert_eq!(app.world().get::<Sleeping>(first).is_some(), !moving && step >= 8,
                    "Source connected island counter and threshold: moving={moving} step={step}");
                assert_eq!(app.world().get::<Sleeping>(first).is_some(), app.world().get::<Sleeping>(second).is_some());
            }
        }
    }

    #[test]
    fn source_sleep_new_connection_resets_counter_but_split_preserves_it() {
        use avian3d::prelude::Sleeping;
        for dt in [1.0 / 60.0, 1.0 / 30.0] {
            for connect in [false, true] {
                let (mut app, first) = source_sleep_fixture(dt, 1, 0.01, 0.05, 4, Vec3::ZERO, Vec3::ZERO);
                let settings = *app.world().get::<SourceBodySleep>(first).unwrap();
                let second = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY, Mass(1.0), NoAutoMass,
                    settings, avian3d::prelude::SleepThreshold { linear: -1.0, angular: -1.0 })).id();
                let initial_joint = (!connect).then(|| app.world_mut().spawn(SphericalJoint::new(first, second)).id());
                for _ in 0..3 { app.update(); }
                for body in [first, second] { assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 3); }
                if let Some(joint) = initial_joint { app.world_mut().despawn(joint); }
                else { app.world_mut().spawn(SphericalJoint::new(first, second)); }
                let first_sleep = if connect { 8 } else { 5 };
                for step in 4..=first_sleep {
                    app.update();
                    for body in [first, second] {
                        assert_eq!(app.world().get::<Sleeping>(body).is_some(), step == first_sleep,
                            "dt={dt} connect={connect} step={step}");
                    }
                }
            }
        }
    }

    #[test]
    fn source_sleep_adding_an_isolated_body_preserves_an_existing_island() {
        use avian3d::prelude::Sleeping;
        let (mut app, first) = source_sleep_fixture(1.0 / 60.0, 1, 0.01, 0.05, 4, Vec3::ZERO, Vec3::ZERO);
        let settings = *app.world().get::<SourceBodySleep>(first).unwrap();
        let second = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY, Mass(1.0), NoAutoMass,
            settings, avian3d::prelude::SleepThreshold { linear: -1.0, angular: -1.0 })).id();
        let third = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY, Mass(1.0), NoAutoMass,
            settings, avian3d::prelude::SleepThreshold { linear: -1.0, angular: -1.0 })).id();
        app.world_mut().spawn(SphericalJoint::new(first, second));
        for _ in 0..3 { app.update(); }
        for body in [first, second, third] { assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 3); }
        app.world_mut().spawn(SphericalJoint::new(first, third));
        app.update();
        for body in [first, second, third] { assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 4); }
        app.update();
        for body in [first, second, third] { assert!(app.world().get::<Sleeping>(body).is_some()); }
    }

    #[test]
    fn source_sleep_loaded_ragdoll_new_island_starts_a_fresh_counter() {
        use avian3d::prelude::{AngularVelocity, ColliderDisabled, Gravity, JointDisabled, LinearVelocity, Sleeping};
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let (player, bodies) = crate::ragdoll_link::tests::loaded_ragdoll_bodies(&mut app, avatar);
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(actor).unwrap().ragdoll_start = true;
        for _ in 0..12 { app.update(); }
        let spawned = app.world().get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap();
        let colliders = spawned.colliders.values().copied().collect::<Vec<_>>();
        let joints = spawned.joints.values().copied().collect::<Vec<_>>();
        for entity in colliders { app.world_mut().entity_mut(entity).insert(ColliderDisabled); }
        for entity in joints { app.world_mut().entity_mut(entity).insert(JointDisabled); }
        app.world_mut().insert_resource(Gravity(Vec3::ZERO));
        let first = bodies.iter().find(|(label, _)| label == "lowerarm_l").unwrap().1;
        let second = bodies.iter().find(|(label, _)| label == "hand_l").unwrap().1;
        for body in [first, second] {
            app.world_mut().entity_mut(body).insert((LinearVelocity::ZERO, AngularVelocity::ZERO)).remove::<Sleeping>();
            let mut sleep = app.world_mut().get_mut::<SourceBodySleep>(body).unwrap();
            sleep.counter = 0; sleep.smooth_linear = Vec3::ZERO; sleep.smooth_angular = Vec3::ZERO;
        }
        for _ in 0..3 { app.update(); }
        for body in [first, second] {
            let node = app.world().get::<BodyIslandNode>(body).unwrap();
            let island = app.world().resource::<PhysicsIslands>().get(node.island_id()).unwrap();
            assert_eq!(island.contact_count() + island.joint_count(), 0);
            assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 3);
        }
        let first_position = app.world().get::<Position>(first).unwrap().0;
        let second_position = app.world().get::<Position>(second).unwrap().0;
        let center = (first_position + second_position) * 0.5;
        let first_anchor = app.world().get::<Rotation>(first).unwrap().0.inverse() * (center - first_position);
        let second_anchor = app.world().get::<Rotation>(second).unwrap().0.inverse() * (center - second_position);
        app.world_mut().spawn(SphericalJoint::new(first, second).with_local_anchor1(first_anchor).with_local_anchor2(second_anchor));
        for step in 4..=8 {
            app.update();
            for body in [first, second] {
                assert_eq!(app.world().get::<Sleeping>(body).is_some(), step == 8, "step={step}");
            }
        }
    }

    #[test]
    fn source_sleep_connected_metrics_use_solver_displacement_and_reset_counter() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, first) = source_sleep_fixture(0.1, 1, 0.01, 0.05, 4, Vec3::ZERO, Vec3::ZERO);
        let second = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY, Mass(1.0), NoAutoMass)).id();
        app.world_mut().spawn(SphericalJoint::new(first, second));
        app.update();
        app.world_mut().entity_mut(first).insert(ComputedCenterOfMass(Vec3::ZERO));
        let mut sleep = app.world_mut().get_mut::<SourceBodySleep>(first).unwrap();
        sleep.counter = 3;
        sleep.smooth_linear = Vec3::X * 0.02;
        sleep.smooth_angular = Vec3::Y * 0.1;
        let mut solver = app.world_mut().get_mut::<SolverBody>(first).unwrap();
        solver.linear_velocity = Vec3::ZERO; solver.angular_velocity = Vec3::ZERO;
        solver.delta_position = Vec3::X * 0.01;
        solver.delta_rotation.0 = Quat::from_rotation_y(0.8);
        app.world_mut().run_system_once(update_profile_sleep).unwrap();
        let sleep = app.world().get::<SourceBodySleep>(first).unwrap();
        assert!((sleep.smooth_linear.x - 0.044).abs() < 1e-6, "Source sleep smoothing uses projected position delta");
        assert!((sleep.smooth_angular.y - (0.07 + 6.0 * 0.4_f32.sin())).abs() < 1e-5, "Source sleep smoothing uses shortest-arc quaternion derivative");
        assert_eq!(sleep.counter, 0, "Source connected motion resets the common sleep counter");
    }

    #[test]
    fn source_sleep_refresh_preserves_progress_and_wake_starts_a_new_counter() {
        use avian3d::prelude::Sleeping;
        let (mut app, body) = source_sleep_fixture(1.0 / 60.0, 1, 0.01, 0.05, 4, Vec3::ZERO, Vec3::ZERO);
        for _ in 0..3 { app.update(); }
        assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 3);
        app.world_mut().query::<&mut RagdollPhysicsProfile>().single_mut(app.world_mut()).unwrap()
            .bodies[0].sleep.counter_threshold = 5;
        app.update();
        assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 4,
            "Source material refresh preserves existing sleep progress");
        app.update();
        assert!(app.world().get::<Sleeping>(body).is_none());
        app.update();
        assert!(app.world().get::<Sleeping>(body).is_some());
        app.world_mut().entity_mut(body).remove::<Sleeping>();
        for step in 1..=6 {
            app.update();
            assert_eq!(app.world().get::<Sleeping>(body).is_some(), step == 6,
                "Source wake starts a fresh sleep counter: step={step}");
        }
    }

    #[test]
    fn source_sleep_isolated_motion_retains_the_existing_counter() {
        use avian3d::prelude::Sleeping;
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, body) = source_sleep_fixture(0.1, 1, 0.01, 0.05, 4, Vec3::ZERO, Vec3::ZERO);
        for _ in 0..2 { app.update(); }
        assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 2);
        app.world_mut().get_mut::<SolverBody>(body).unwrap().linear_velocity = Vec3::X * 0.03;
        app.world_mut().run_system_once(update_profile_sleep).unwrap();
        assert_eq!(app.world().get::<SourceBodySleep>(body).unwrap().counter, 2,
            "Source isolated velocity rejection retains the existing counter");
        app.world_mut().get_mut::<SolverBody>(body).unwrap().linear_velocity = Vec3::ZERO;
        for step in 1..=3 {
            app.world_mut().run_system_once(update_profile_sleep).unwrap();
            assert_eq!(app.world().get::<Sleeping>(body).is_some(), step == 3,
                "Source isolated counter resumes after fast motion: step={step}");
        }
    }

    #[test]
    fn source_sleep_removed_joint_splits_independent_bodies() {
        use avian3d::prelude::Sleeping;
        let (mut app, first) = source_sleep_fixture(1.0 / 60.0, 1, 0.01, 0.05, 127, Vec3::ZERO, Vec3::ZERO);
        let mut settings = *app.world().get::<SourceBodySleep>(first).unwrap();
        settings.counter_threshold = 4;
        let second = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY, Mass(1.0), NoAutoMass,
            settings, avian3d::prelude::SleepThreshold { linear: -1.0, angular: -1.0 })).id();
        let joint = app.world_mut().spawn(SphericalJoint::new(first, second)).id();
        app.update();
        assert_eq!(app.world().get::<BodyIslandNode>(first).unwrap().island_id(),
            app.world().get::<BodyIslandNode>(second).unwrap().island_id());
        app.world_mut().despawn(joint);
        for _ in 0..10 { app.update(); }
        assert_ne!(app.world().get::<BodyIslandNode>(first).unwrap().island_id(),
            app.world().get::<BodyIslandNode>(second).unwrap().island_id(),
            "Source removed joint separates independent sleep groups");
        assert!(app.world().get::<Sleeping>(first).is_none(), "Source isolated counter 127 remains awake");
        assert!(app.world().get::<Sleeping>(second).is_some(), "Source disconnected body uses its own sleep threshold");
    }

    #[test]
    fn source_sleep_preserves_native_members_in_the_same_island() {
        use avian3d::prelude::{LinearVelocity, Sleeping, SleepingDisabled};
        for disabled in [false, true] {
            let (mut app, first) = source_sleep_fixture(1.0 / 60.0, 1, 1.0, 1.0, 4, Vec3::ZERO, Vec3::ZERO);
            let second = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY, Mass(1.0), NoAutoMass,
                LinearVelocity(Vec3::X * 0.2))).id();
            if disabled { app.world_mut().entity_mut(second).insert(SleepingDisabled); }
            app.world_mut().spawn(SphericalJoint::new(first, second));
            for step in 1..=10 {
                app.update();
                assert!(app.world().get::<Sleeping>(first).is_none() && app.world().get::<Sleeping>(second).is_none(),
                    "Source sleep must retain native member eligibility: disabled={disabled} step={step}");
            }
        }
    }

    #[test]
    fn source_sleep_connected_metrics_use_body_origin_with_offset_mass_center() {
        use avian3d::prelude::{ComputedCenterOfMass, Sleeping};
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, first) = source_sleep_fixture(0.1, 1, 0.01, 0.05, 4, Vec3::ZERO, Vec3::ZERO);
        let settings = *app.world().get::<SourceBodySleep>(first).unwrap();
        let second = app.world_mut().spawn((RigidBody::Dynamic, Transform::IDENTITY, Mass(1.0), NoAutoMass,
            settings, avian3d::prelude::SleepThreshold { linear: -1.0, angular: -1.0 })).id();
        app.world_mut().spawn(SphericalJoint::new(first, second));
        app.update();
        let profile = RagdollPhysicsProfile::from_json(include_str!("../../assets/ALS/Character/PA_Als.physics.json")).unwrap();
        let center = profile.bodies.iter().find(|body| body.label == "hand_l").unwrap()
            .mass_properties.unwrap().center_of_mass_m * 8.0;
        assert!((center - Vec3::new(0.4, 0.0, -0.08)).length() < 1e-6);
        app.world_mut().entity_mut(first).insert(ComputedCenterOfMass(center));
        for body in [first, second] {
            let mut sleep = app.world_mut().get_mut::<SourceBodySleep>(body).unwrap();
            sleep.counter = 0; sleep.smooth_linear = Vec3::ZERO; sleep.smooth_angular = Vec3::ZERO;
        }
        for step in 1..=8 {
            app.world_mut().get_mut::<Rotation>(first).unwrap().0 = Quat::from_rotation_y(step as f32 * 0.004);
            let mut solver = app.world_mut().get_mut::<SolverBody>(first).unwrap();
            solver.linear_velocity = Vec3::ZERO; solver.angular_velocity = Vec3::Y * 0.04;
            solver.delta_position = Vec3::ZERO; solver.delta_rotation.0 = Quat::from_rotation_y(0.004);
            app.world_mut().run_system_once(update_profile_sleep).unwrap();
            let sleep = app.world().get::<SourceBodySleep>(first).unwrap();
            if step >= 3 {
                assert!(sleep.smooth_linear.length() > 0.01,
                    "Source actor-origin displacement includes mass-center rotation: step={step} velocity={:?}", sleep.smooth_linear);
            }
            assert!(app.world().get::<Sleeping>(first).is_none(), "Source rotating offset body must remain awake");
        }
    }

    #[test]
    fn browser_review_zero_sleep_thresholds_disable_automatic_body_sleep() {
        use avian3d::prelude::{Gravity, Mass, NoAutoMass, PhysicsPlugins, Sleeping, SubstepCount};
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::ragdoll::definition::Body;
        for disabled in [false, true] {
            let mut source: serde_json::Value = serde_json::from_str(include_str!("../../assets/ALS/Character/PA_Als.physics.json")).unwrap();
            for body in source["bodies"].as_array_mut().unwrap() {
                body["target"]["material"]["sleeping_disabled"] = serde_json::json!(disabled);
            }
            let mut profile = RagdollPhysicsProfile::from_json(&source.to_string()).unwrap();
            profile.bodies.retain(|body| body.label == "pelvis");
            profile.colliders.clear();profile.joints.clear();
            profile.bodies[0].linear_damping = 0.0;
            profile.bodies[0].angular_damping = 0.0;
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
                PhysicsPlugins::new(PostUpdate), RagdollPhysicsProfilePlugin))
                .insert_resource(Gravity(Vec3::ZERO)).insert_resource(SubstepCount(1))
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
            app.finish();app.cleanup();
            let root = app.world_mut().spawn_empty().id();
            let body = app.world_mut().spawn((BodyLabel("pelvis".into()), RigidBody::Dynamic, Transform::IDENTITY,
                Mass(1.0), NoAutoMass)).id();
            if !disabled { app.world_mut().entity_mut(body).insert(avian3d::prelude::SleepingDisabled); }
            let mut spawned = SpawnedRagdoll::new(root);
            spawned.bodies.insert(Body::new().id, body);
            app.world_mut().run_system_once(move |mut commands: Commands,
                body_labels: Query<&BodyLabel>, collider_labels: Query<&ColliderLabel>, joint_labels: Query<&JointLabel>,
                spherical: Query<&SphericalJoint>, revolute: Query<&RevoluteJoint>, source_joints: Query<&SourcePhysicsJoint>,
                transforms: Query<&Transform>, mut colliders: Query<&mut Collider>| {
                apply_profile_to_spawned_ragdoll(&mut commands, &spawned, &profile, Vec3::ONE, Vec3::ONE, true, true,
                    &body_labels, &collider_labels, &joint_labels, &spherical, &revolute,
                    &source_joints, &transforms, &mut colliders)
            }).unwrap();
            for _ in 0..180 { app.update(); }
            assert_eq!(app.world().get::<Sleeping>(body).is_none(), disabled,
                "Zero effective source thresholds still allowed automatic sleep: disabled={disabled}");
        }
    }

    fn browser_ccd_body(
        world: &mut World, mode: RigidBody, origin: Vec3, velocity: Vec3, shape: Collider,
        settings: Option<SourceCcdSettings>,
    ) -> Entity {
        use avian3d::prelude::{ColliderOf, ColliderTransform, ComputedCenterOfMass, ComputedMass, SweptCcd};
        let body = world.spawn((mode, Position(origin), Rotation::IDENTITY,
            ComputedCenterOfMass(Vec3::ZERO), ComputedMass::new(1.0), Restitution::new(0.5),
            SolverBody { linear_velocity: velocity, delta_position: velocity / 64.0,
                flags: if mode.is_dynamic() { SolverBodyFlags::empty() } else { SolverBodyFlags::IS_KINEMATIC },
                ..default() })).id();
        if let Some(settings) = settings { world.entity_mut(body).insert((settings, SweptCcd::LINEAR)); }
        world.spawn((shape, ColliderOf { body }, ColliderTransform::default(), Restitution::new(0.5)));
        body
    }

    #[test]
    fn pawn_ignore_filters_ragdoll_contacts_and_ccd() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let pawn = world.spawn((SourceCollisionPawn, RigidBody::Kinematic)).id();
        let pawn_collider = world.spawn(ColliderOf { body: pawn }).id();
        let rig = world.spawn(SourceCollisionIgnorePawn).id();
        let floor = world.spawn_empty().id();
        let pairs = world.run_system_once(move |hooks: RagdollCollisionHooks, mut commands: Commands| {
            (hooks.filter_pairs(rig, pawn, &mut commands), hooks.filter_pairs(pawn_collider, rig, &mut commands),
                hooks.filter_pairs(rig, floor, &mut commands), hooks.filter_pairs(pawn, floor, &mut commands))
        }).unwrap();
        assert_eq!(pairs, (false, false, true, true));

        for ignore in [false, true] {
            for pawn in [false, true] {
                let mut world = World::new();
                let mut time = Time::<()>::default();
                time.advance_by(std::time::Duration::from_secs_f32(1.0 / 64.0));
                world.insert_resource(time);
                let settings = crate::pa_als::parse_pa_als_profile().ccd_settings;
                let body = browser_ccd_body(&mut world, RigidBody::Dynamic, Vec3::NEG_X, Vec3::X * 128.0,
                    Collider::sphere(0.1), Some(settings));
                let wall = browser_ccd_body(&mut world, RigidBody::Static, Vec3::ZERO, Vec3::ZERO,
                    Collider::cuboid(0.02, 2.0, 2.0), None);
                if pawn { world.entity_mut(wall).insert(SourceCollisionPawn); }
                if ignore {
                    let collider = world.query::<(Entity, &ColliderOf)>().iter(&world)
                        .find(|(_, owner)| owner.body == body).unwrap().0;
                    world.entity_mut(collider).insert(SourceCollisionIgnorePawn);
                }
                world.run_system_once(solve_profile_linear_ccd).unwrap();
                let delta = world.get::<SolverBody>(body).unwrap().delta_position.x;
                assert_eq!(delta == 2.0, ignore && pawn, "ignore={ignore} pawn={pawn} delta={delta}");
            }
        }
    }

    #[test]
    fn pawn_ignore_reaches_avian_collision_pairs() {
        use avian3d::prelude::{Collisions, Gravity, PhysicsPlugins};
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
            PhysicsPlugins::new(PostUpdate).with_collision_hooks::<RagdollCollisionHooks>()))
            .insert_resource(Gravity(Vec3::ZERO))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        app.finish(); app.cleanup();
        let mut pairs = Vec::new();
        for (index, (ignore, pawn)) in [(false, false), (false, true), (true, false), (true, true)].into_iter().enumerate() {
            let x = index as f32 * 4.0;
            let rig = app.world_mut().spawn((RigidBody::Dynamic, Collider::sphere(0.5),
                Transform::from_xyz(x, 0.0, 0.0), ActiveCollisionHooks::FILTER_PAIRS)).id();
            let other = app.world_mut().spawn((RigidBody::Static, Collider::sphere(0.5),
                Transform::from_xyz(x + 0.8, 0.0, 0.0))).id();
            if ignore { app.world_mut().entity_mut(rig).insert(SourceCollisionIgnorePawn); }
            if pawn { app.world_mut().entity_mut(other).insert(SourceCollisionPawn); }
            pairs.push((rig, other, ignore && pawn));
        }
        app.update(); app.update();
        let observed = app.world_mut().run_system_once(move |collisions: Collisions| {
            pairs.iter().map(|(rig, other, ignored)| (collisions.get(*rig, *other).is_some(), *ignored)).collect::<Vec<_>>()
        }).unwrap();
        for (contact, ignored) in observed { assert_eq!(contact, !ignored); }
    }

    #[test]
    fn browser_review_ccd_relative_motion_activates_a_stationary_body() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();time.advance_by(std::time::Duration::from_secs_f32(1.0 / 64.0));world.insert_resource(time);
        let settings = crate::pa_als::parse_pa_als_profile().ccd_settings;
        browser_ccd_body(&mut world, RigidBody::Dynamic, Vec3::ZERO, Vec3::ZERO, Collider::sphere(0.2), Some(settings));
        let moving = browser_ccd_body(&mut world, RigidBody::Dynamic, Vec3::NEG_X, Vec3::X * 128.0,
            Collider::cuboid(0.02, 2.0, 2.0), None);
        world.run_system_once(solve_profile_linear_ccd).unwrap();
        assert!(world.get::<SolverBody>(moving).unwrap().delta_position.x < 1.0,
            "Stationary CCD body ignored the other body's relative sweep");
    }

    #[test]
    fn browser_review_ccd_kinematic_push_keeps_remaining_normal_motion() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();time.advance_by(std::time::Duration::from_secs_f32(1.0 / 64.0));world.insert_resource(time);
        let mut settings = crate::pa_als::parse_pa_als_profile().ccd_settings;
        settings.enable_threshold_bounds_scale = 0.0;
        let body = browser_ccd_body(&mut world, RigidBody::Dynamic, Vec3::ZERO, Vec3::ZERO,
            Collider::sphere(0.1), Some(settings));
        browser_ccd_body(&mut world, RigidBody::Kinematic, Vec3::NEG_X, Vec3::X * 128.0, Collider::sphere(0.1), None);
        world.run_system_once(solve_profile_linear_ccd).unwrap();
        let actual = world.get::<SolverBody>(body).unwrap().delta_position.x;
        assert!((actual - 1.16).abs() < 1e-5,
            "Fast kinematic CCD lost its remaining normal displacement: {actual}");
    }

    #[test]
    fn browser_review_ccd_respects_runtime_joint_collision_policy() {
        use avian3d::dynamics::solver::joint_graph::{JointGraph, JointGraphEdge};
        use bevy::ecs::system::RunSystemOnce;
        for disabled in [true, false] {
            let mut world = World::new();
            let mut time = Time::<()>::default();time.advance_by(std::time::Duration::from_secs_f32(1.0 / 64.0));world.insert_resource(time);
            let settings = crate::pa_als::parse_pa_als_profile().ccd_settings;
            let body = browser_ccd_body(&mut world, RigidBody::Dynamic, Vec3::NEG_X, Vec3::X * 128.0,
                Collider::sphere(0.1), Some(settings));
            let wall = browser_ccd_body(&mut world, RigidBody::Static, Vec3::ZERO, Vec3::ZERO,
                Collider::cuboid(0.02, 2.0, 2.0), None);
            let joint = world.spawn_empty().id();
            let mut graph = JointGraph::default();
            graph.add_joint(body, wall, JointGraphEdge::new(joint, body, wall, disabled));
            world.insert_resource(graph);
            if disabled { world.entity_mut(joint).insert(JointCollisionDisabled); }
            world.run_system_once(solve_profile_linear_ccd).unwrap();
            let actual = world.get::<SolverBody>(body).unwrap().delta_position.x;
            assert_eq!(actual == 2.0, disabled,
                "Runtime joint collision policy was bypassed by CCD: disabled={disabled}, delta={actual}");
        }
    }

    #[test]
    fn browser_review_ccd_zero_depth_applies_the_source_point_mass_impulse() {
        use avian3d::prelude::ComputedMass;
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();time.advance_by(std::time::Duration::from_secs_f32(1.0 / 64.0));world.insert_resource(time);
        let mut settings = crate::pa_als::parse_pa_als_profile().ccd_settings;
        settings.enable_threshold_bounds_scale = 0.0;
        settings.allowed_depth_bounds_scale = 0.0;
        let body = browser_ccd_body(&mut world, RigidBody::Dynamic, Vec3::NEG_X * 0.25, Vec3::X * 64.0,
            Collider::sphere(0.0625), Some(settings));
        world.entity_mut(body).insert(ComputedMass::new(2.0));
        browser_ccd_body(&mut world, RigidBody::Static, Vec3::ZERO, Vec3::ZERO,
            Collider::cuboid(0.000001, 2.0, 2.0), None);
        world.run_system_once(solve_profile_linear_ccd).unwrap();
        let solver = world.get::<SolverBody>(body).unwrap();
        assert!((solver.linear_velocity.x + 32.0).abs() < 1e-5,
            "Zero-depth CCD did not apply the source point-mass restitution: {:?}", solver.linear_velocity);
        assert!((-0.25 + solver.delta_position.x + 0.0625).abs() < 1e-5);
        assert_eq!(solver.angular_velocity, Vec3::ZERO);
        use avian3d::prelude::{Gravity, LinearVelocity, Mass, PhysicsPlugins, SpeculativeMargin, SubstepCount, SweptCcd};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
            PhysicsPlugins::new(PostUpdate), RagdollPhysicsProfilePlugin))
            .insert_resource(Gravity(Vec3::ZERO)).insert_resource(SubstepCount(1))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 64.0)));
        app.finish();app.cleanup();
        let body = app.world_mut().spawn((RigidBody::Dynamic, Collider::sphere(0.0625), Mass(2.0),
            Transform::from_xyz(-0.25, 0.0, 0.0), LinearDamping(0.0), Restitution::new(0.5),
            SpeculativeMargin(0.0), settings, SweptCcd::LINEAR.with_velocity_threshold(f32::INFINITY, f32::INFINITY))).id();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.000001, 2.0, 2.0),
            Transform::IDENTITY, Restitution::new(0.5), SpeculativeMargin(0.0)));
        app.update();
        app.world_mut().get_mut::<LinearVelocity>(body).unwrap().0 = Vec3::X * 64.0;
        app.update();
        let actual = app.world().get::<LinearVelocity>(body).unwrap().0.x;
        assert!((actual + 32.0).abs() < 1e-4,
            "Full substep discarded the source zero-depth impulse baseline: {actual}");
        assert!((app.world().get::<Position>(body).unwrap().0.x + 0.0625).abs() < 1e-5);
    }

    #[test]
    fn browser_review_ccd_recomputes_the_sweep_after_a_related_body_is_clipped() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();time.advance_by(std::time::Duration::from_secs_f32(1.0 / 64.0));world.insert_resource(time);
        let settings = crate::pa_als::parse_pa_als_profile().ccd_settings;
        let a = browser_ccd_body(&mut world, RigidBody::Dynamic, Vec3::ZERO, Vec3::X * 64.0,
            Collider::sphere(1.0 / 32.0), Some(settings));
        let b = browser_ccd_body(&mut world, RigidBody::Dynamic, Vec3::new(0.5, 0.0, 0.5), Vec3::NEG_Z * 64.0,
            Collider::sphere(1.0 / 32.0), Some(settings));
        browser_ccd_body(&mut world, RigidBody::Static, Vec3::X * 0.25, Vec3::ZERO,
            Collider::sphere(1.0 / 32.0), None);
        world.run_system_once(solve_profile_linear_ccd).unwrap();
        assert!((world.get::<SolverBody>(a).unwrap().delta_position.x - 0.2).abs() < 1e-5);
        let actual = 0.5 + world.get::<SolverBody>(b).unwrap().delta_position.z;
        assert!((actual + 0.5).abs() < 1e-5,
            "A stale CCD time of impact clipped the unrelated remaining path: {actual}");
    }

    use super::*;

    #[test]
    fn same_entity_player_replacement_invalidates_previous_profile_application() {
        let mut world = World::new();
        let player = world.spawn((
            AnimationGraphPlayer::new(Handle::default()),
            RagdollPhysicsProfileApplied { scale: Vec3::ONE, body_count: 21, ..default() },
        )).id();
        let old_ragdoll = SpawnedRagdoll::new(world.spawn_empty().id());
        let new_ragdoll = SpawnedRagdoll::new(world.spawn_empty().id());
        world.get_mut::<RagdollPhysicsProfileApplied>(player).unwrap().spawned_root = Some(old_ragdoll.root);
        world.entity_mut(player).insert(AnimationGraphPlayer::new(Handle::default()));
        let applied = world.get::<RagdollPhysicsProfileApplied>(player).unwrap();
        assert_ne!(old_ragdoll.root, new_ragdoll.root);
        assert_eq!(profile_application_change(Some(*applied), new_ragdoll.root, Vec3::ONE),
            Some((Vec3::ONE, Some(old_ragdoll.root))));
        assert_eq!(profile_application_change(Some(*applied), old_ragdoll.root, Vec3::ONE), None);
        assert_eq!(profile_application_change(Some(*applied), old_ragdoll.root, Vec3::splat(2.0)), Some((Vec3::splat(2.0), None)));
    }

    #[test]
    fn ragdoll_scale_follows_live_player_after_world_preserving_detach() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner, WorldSerializationPlugin};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            WorldSerializationPlugin, bevy_animation_graph::AnimationGraphPlugin::default()))
            .init_asset::<WorldAsset>();
        app.finish();
        app.cleanup();
        let actor = app.world_mut().spawn(Transform::from_scale(Vec3::splat(2.0))).id();
        let scene = app.world_mut().spawn((AnimatedSceneHandle::default(), Transform::IDENTITY,
            ChildOf(actor))).id();
        let player = app.world_mut().spawn((AnimationGraphPlayer::new(Handle::default()),
            Transform::IDENTITY, ChildOf(scene))).id();
        app.world_mut().trigger(WorldInstanceReady { entity: scene,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()) });
        app.world_mut().flush();
        assert_eq!(app.world().get::<AnimatedSceneInstance>(scene).unwrap().player_entity(), player);
        app.world_mut().entity_mut(player).remove::<ChildOf>().insert(Transform::from_scale(Vec3::splat(2.0)));
        let scale = app.world_mut().run_system_once(move |
            transform_helper: bevy::transform::helper::TransformHelper| {
            ragdoll_positive_scale(player, &transform_helper)
        }).unwrap();
        assert_eq!(scale, Vec3::splat(2.0));
        app.world_mut().get_mut::<Transform>(actor).unwrap().scale = Vec3::new(2.0, 1.0, 2.0);
        let scale = app.world_mut().run_system_once(move |
            transform_helper: bevy::transform::helper::TransformHelper| {
            ragdoll_positive_scale(player, &transform_helper)
        }).unwrap();
        assert_eq!(scale, Vec3::splat(2.0));
    }

    #[test]
    fn detached_player_scale_reconfigures_mass_shapes_and_joints_without_host_updates() {
        use avian3d::prelude::{ComputedMass, PhysicsPlugins};
        use bevy::ecs::system::RunSystemOnce;
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner, WorldSerializationPlugin};
        use bevy_animation_graph::core::ragdoll::{definition::Ragdoll, spawning::spawn_ragdoll_avian};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
            WorldSerializationPlugin, bevy::gizmos::GizmoPlugin, bevy_animation_graph::AnimationGraphPlugin::default(),
            PhysicsPlugins::new(PostUpdate), crate::extensions::SourcePhysicsJointPlugin, RagdollPhysicsProfilePlugin))
            .init_asset::<WorldAsset>();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::ZERO));
        app.finish(); app.cleanup();
        let profile = RagdollPhysicsProfile::from_json(include_str!("../../assets/ALS/Character/PA_Als.physics.json")).unwrap();
        let base_mass = profile.bodies.iter().find(|body| body.label == "pelvis").unwrap().mass_properties.unwrap().mass_kg;
        let base_frame = profile.joints.iter().find(|joint| joint.label == "clavicle_l").unwrap().definition.parent_frame.translation;
        let world = app.world_mut();
        let actor = world.spawn(Transform::from_scale(Vec3::splat(2.0))).id();
        let scene = world.spawn((AnimatedSceneHandle::default(), Transform::from_scale(Vec3::splat(0.5)), ChildOf(actor))).id();
        let player = world.spawn((AnimationGraphPlayer::new(Handle::default()), profile,
            Transform::from_scale(Vec3::splat(1.5)), ChildOf(scene))).id();
        world.trigger(WorldInstanceReady { entity: scene,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()) });
        world.flush();
        let ragdoll: Ragdoll = ron::from_str(include_str!("../../assets/ALS/Character/PA_Als.rag.ron")).unwrap();
        let spawned = world.run_system_once(move |mut commands: Commands| {
            spawn_ragdoll_avian(player, &ragdoll, Isometry3d::IDENTITY, None, &mut commands)
        }).unwrap();
        let body = *spawned.bodies.values().find(|entity| world.get::<BodyLabel>(**entity).unwrap().0 == "pelvis").unwrap();
        let collider = *spawned.colliders.values().find(|entity| world.get::<ColliderLabel>(**entity).unwrap().0 == "pelvis_0_capsule").unwrap();
        let joint = *spawned.joints.values().find(|entity| world.get::<JointLabel>(**entity).unwrap().0 == "clavicle_l").unwrap();
        let base_extent = world.get::<Collider>(collider).unwrap().shape_scaled().compute_local_aabb().half_extents();
        let mut graph_player = world.get_mut::<AnimationGraphPlayer>(player).unwrap();
        let ReflectMut::Struct(fields) = graph_player.reflect_mut() else { panic!("player must be a struct") };
        *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
        app.update();
        assert_eq!(app.world().get::<RagdollPhysicsProfileApplied>(player).unwrap().scale, Vec3::splat(1.5));
        let preserved = app.world_mut().run_system_once(move |helper: bevy::transform::helper::TransformHelper| {
            helper.compute_global_transform(player).unwrap().compute_transform()
        }).unwrap();
        app.world_mut().entity_mut(player).remove::<ChildOf>().insert(preserved);
        for (host_scale, player_scale) in [(3.0, 1.5), (0.5, 1.5), (0.5, 0.75), (4.0, 0.75), (4.0, 2.25)] {
            app.world_mut().get_mut::<Transform>(actor).unwrap().scale = Vec3::splat(host_scale);
            app.world_mut().get_mut::<Transform>(player).unwrap().scale = Vec3::splat(player_scale);
            app.update();
            let world = app.world();
            assert_eq!(world.get::<RagdollPhysicsProfileApplied>(player).unwrap().scale, Vec3::splat(player_scale));
            let expected_mass = base_mass * player_scale.powf(2.25);
            assert!((world.get::<ComputedMass>(body).unwrap().value() - expected_mass).abs() < expected_mass * 1e-5);
            assert!(world.get::<Collider>(collider).unwrap().shape_scaled().compute_local_aabb().half_extents()
                .abs_diff_eq(base_extent * player_scale, 1e-5));
            assert!(world.get::<SourcePhysicsJoint>(joint).unwrap().parent_frame.translation
                .abs_diff_eq(base_frame * player_scale, 1e-5));
        }
    }

    #[test]
    fn linear_drive_json_fields_reach_the_joint_definition() {
        let mut source: serde_json::Value = serde_json::from_str(include_str!(
            "../../assets/ALS/Character/PA_Als.physics.json"
        )).unwrap();
        let pelvis = source["constraints"].as_array_mut().unwrap().iter_mut()
            .find(|joint| joint["joint_name"] == "pelvis").unwrap();
        pelvis["target"]["enabled"] = serde_json::json!(true);
        pelvis["target"]["linear_drives"][0]["position_enabled"] = serde_json::json!(true);
        pelvis["target"]["linear_drives"][0]["stiffness"] = serde_json::json!(123.0);
        pelvis["target"]["linear_drives"][0]["max_force"] = serde_json::json!(2.0);
        pelvis["target"]["linear_position_target_m"] = serde_json::json!([0.2, 0.0, 0.0]);
        let profile = RagdollPhysicsProfile::from_json(&source.to_string()).unwrap();
        let joint = profile.joints.iter().find(|joint| joint.label == "pelvis").unwrap();
        assert!(joint.enabled && joint.definition.linear_motors[0].position_enabled);
        assert_eq!(joint.definition.linear_motors[0].stiffness, 123.0);
        assert_eq!(joint.definition.linear_motors[0].max_force, 2.0);
        assert_eq!(joint.definition.linear_position_target, Vec3::X * 0.2);
    }

    #[test]
    fn swing_velocity_json_fields_reach_a_pure_drive_joint() {
        let mut source: serde_json::Value = serde_json::from_str(include_str!(
            "../../assets/ALS/Character/PA_Als.physics.json"
        )).unwrap();
        let pelvis = source["constraints"].as_array_mut().unwrap().iter_mut()
            .find(|joint| joint["joint_name"] == "pelvis").unwrap();
        pelvis["target"]["enabled"] = serde_json::json!(true);
        pelvis["target"]["drives"][1]["velocity_enabled"] = serde_json::json!(true);
        pelvis["target"]["drives"][1]["damping"] = serde_json::json!(50.0);
        pelvis["target"]["drive_velocity_target_radians_per_second"] = serde_json::json!([0.0, 1.0, 0.0]);
        let profile = RagdollPhysicsProfile::from_json(&source.to_string()).unwrap();
        let joint = profile.joints.iter().find(|joint| joint.label == "pelvis").unwrap();
        assert!(joint.enabled && joint.definition.motors[1].velocity_enabled);
        assert_eq!(joint.definition.motor_velocity_target, Vec3::Y);
    }

    #[test]
    fn profile_mass_overrides_restore_automatic_and_preexisting_mass_properties() {
        use avian3d::prelude::{ComputedAngularInertia, ComputedCenterOfMass, ComputedMass, PhysicsPlugins, RigidBody};
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::ragdoll::definition::Body;

        fn apply(In((profile, spawned)): In<(RagdollPhysicsProfile, SpawnedRagdoll)>,
            mut commands: Commands, body_labels: Query<&BodyLabel>, collider_labels: Query<&ColliderLabel>,
            joint_labels: Query<&JointLabel>, spherical: Query<&SphericalJoint>, revolute: Query<&RevoluteJoint>,
            source_joints: Query<&SourcePhysicsJoint>, transforms: Query<&Transform>, mut colliders: Query<&mut Collider>,
        ) -> RagdollPhysicsProfileApplied {
            apply_profile_to_spawned_ragdoll(&mut commands, &spawned, &profile, Vec3::ONE, Vec3::ONE, true, true,
                &body_labels, &collider_labels, &joint_labels, &spherical, &revolute, &source_joints, &transforms, &mut colliders)
        }

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            TransformPlugin, PhysicsPlugins::new(PostUpdate)));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        let root = app.world_mut().spawn(Transform::default()).id();
        let bodies = ["pelvis", "spine_03"].map(|label| {
            let entity = app.world_mut().spawn((RigidBody::Dynamic, BodyLabel(label.into()), ChildOf(root),
                Transform::from_xyz(if label == "pelvis" { 0.0 } else { 4.0 }, 0.0, 0.0))).id();
            app.world_mut().spawn((Collider::sphere(0.5), Mass(2.0), Transform::from_xyz(0.0, 0.4, 0.0), ChildOf(entity)));
            (Body::new().id, entity)
        });
        app.world_mut().entity_mut(bodies[1].1).insert((Mass(9.0), CenterOfMass(Vec3::X * 0.6),
            AngularInertia::new(Vec3::new(0.9, 1.2, 1.5)), NoAutoMass, NoAutoCenterOfMass, NoAutoAngularInertia));
        app.update();
        let profile = RagdollPhysicsProfile::from_json(include_str!("../../assets/ALS/Character/PA_Als.physics.json")).unwrap();
        for mass in [None, Some(3.0), Some(5.0), None, Some(3.0)] {
            let mut profile = profile.clone();
            for body in &mut profile.bodies {
                body.mass_properties = mass.map(|mass_kg| RagdollMassProperties { mass_kg, unclamped_mass_kg: mass_kg, scale_exponent: 0.0, update_with_scale: true,
                    center_of_mass_m: Vec3::X * mass_kg * 0.1,
                    inertia_tensor_kg_m2: Mat3::from_diagonal(Vec3::new(0.1, 0.2, 0.3) * mass_kg) });
            }
            let mut spawned = SpawnedRagdoll::new(root);
            spawned.bodies.extend(bodies);
            let applied = app.world_mut().run_system_once_with(apply, (profile, spawned)).unwrap();
            assert_eq!((applied.spawned_root, applied.scale, applied.body_count), (Some(root), Vec3::ONE, 2));
            app.update();
            for (index, (_, entity)) in bodies.iter().enumerate() {
                let expected_mass = mass.unwrap_or(if index == 0 { 2.0 } else { 9.0 });
                let expected_center = mass.map_or(if index == 0 { Vec3::Y * 0.4 } else { Vec3::X * 0.6 }, |mass| Vec3::X * mass * 0.1);
                let expected_inertia = mass.map_or(if index == 0 { Vec3::splat(0.2) } else { Vec3::new(0.9, 1.2, 1.5) },
                    |mass| Vec3::new(0.1, 0.2, 0.3) * mass);
                let world = app.world();
                let actual_mass = world.get::<ComputedMass>(*entity).unwrap().value();
                let actual_center = world.get::<ComputedCenterOfMass>(*entity).unwrap().0;
                let actual_inertia = world.get::<ComputedAngularInertia>(*entity).unwrap().value();
                assert!((actual_mass - expected_mass).abs() < 1e-5, "mass={mass:?} body={index}: {actual_mass} != {expected_mass}");
                assert!(actual_center.distance(expected_center) < 1e-5, "mass={mass:?} body={index}: {actual_center} != {expected_center}");
                assert!(Vec3::new(actual_inertia.m00, actual_inertia.m11, actual_inertia.m22).distance(expected_inertia) < 1e-5,
                    "mass={mass:?} body={index}: {actual_inertia:?}");
                if mass.is_none() {
                    assert_eq!(world.get::<NoAutoMass>(*entity).is_some(), index == 1);
                    assert_eq!(world.get::<NoAutoCenterOfMass>(*entity).is_some(), index == 1);
                    assert_eq!(world.get::<NoAutoAngularInertia>(*entity).is_some(), index == 1);
                }
            }
        }
    }

    #[test]
    fn profile_replaces_every_spawned_joint_and_applies_body_materials() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::ragdoll::{
            definition::Ragdoll, spawning::spawn_ragdoll_avian,
        };

        let profile = RagdollPhysicsProfile::from_json(include_str!(
            "../../assets/ALS/Character/PA_Als.physics.json"
        ))
        .unwrap();
        let ragdoll: Ragdoll =
            ron::from_str(include_str!("../../assets/ALS/Character/PA_Als.rag.ron")).unwrap();
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            crate::extensions::SourcePhysicsJointPlugin,
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
        let root_collider = *spawned.colliders.values().find(|entity|
            world.get::<ColliderLabel>(**entity).unwrap().0.starts_with("root_")).unwrap();
        world.entity_mut(root_collider).insert(avian3d::prelude::CollisionLayers::from_bits(1, 1));
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
        let pelvis = *spawned.joints.values().find(|entity|
            world.get::<JointLabel>(**entity).unwrap().0 == "pelvis").unwrap();
        let allowed_joint = *spawned.joints.values().find(|entity|
            world.get::<JointLabel>(**entity).unwrap().0 == "clavicle_l").unwrap();
        assert!(world.get::<JointCollisionDisabled>(allowed_joint).is_some());
        let pelvis_body = *spawned.bodies.values().find(|entity|
            world.get::<BodyLabel>(**entity).unwrap().0 == "pelvis").unwrap();
        let spine_body = *spawned.bodies.values().find(|entity|
            world.get::<BodyLabel>(**entity).unwrap().0 == "spine_01").unwrap();
        let spine_main = *spawned.colliders.values().find(|entity|
            world.get::<ColliderLabel>(**entity).unwrap().0 == "spine_03_0_capsule").unwrap();
        let spine_aux = *spawned.colliders.values().find(|entity|
            world.get::<ColliderLabel>(**entity).unwrap().0 == "spine_03_1_capsule").unwrap();
        let clavicle_collider = *spawned.colliders.values().find(|entity|
            world.get::<ColliderLabel>(**entity).unwrap().0 == "clavicle_l_0_capsule").unwrap();
        let pelvis_collider = *spawned.colliders.values().find(|entity|
            world.get::<ColliderLabel>(**entity).unwrap().0 == "pelvis_0_capsule").unwrap();
        let probe_collider = *spawned.colliders.values().find(|entity|
            world.get::<ColliderLabel>(**entity).unwrap().0 == "spine_01_0_capsule").unwrap();
        let physics_only_collider = *spawned.colliders.values().find(|entity|
            world.get::<ColliderLabel>(**entity).unwrap().0 == "spine_02_0_capsule").unwrap();
        let outside = world.spawn_empty().id();
        let again = SpawnedRagdoll {
            root: spawned.root, bodies: spawned.bodies.clone(),
            colliders: spawned.colliders.clone(), joints: spawned.joints.clone(),
        };
        let mass_again = SpawnedRagdoll {
            root: spawned.root, bodies: spawned.bodies.clone(),
            colliders: spawned.colliders.clone(), joints: spawned.joints.clone(),
        };
        let mut driven_profile = profile.clone();
        let applied = world
            .run_system_once(
                move |mut commands: Commands,
                      body_labels: Query<&BodyLabel>,
                      collider_labels: Query<&ColliderLabel>,
                      joint_labels: Query<&JointLabel>,
                      spherical: Query<&SphericalJoint>,
                      revolute: Query<&RevoluteJoint>,
                      source_joints: Query<&SourcePhysicsJoint>,
                      transforms: Query<&Transform>, mut colliders: Query<&mut Collider>| {
                    apply_profile_to_spawned_ragdoll(
                        &mut commands,
                        &spawned,
                        &profile,
                        Vec3::ONE,
                        Vec3::ONE,
                        true,
                        true,
                        &body_labels,
                        &collider_labels,
                        &joint_labels,
                        &spherical,
                        &revolute,
                        &source_joints,
                        &transforms,
                        &mut colliders,
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
            assert!((world.get::<MaxAngularSpeed>(entity).unwrap().0 - std::f32::consts::TAU * 10.0).abs() < 1e-5);
            assert!(world.get::<RagdollGyroscopicTorqueDisabled>(entity).is_some());
        }
        assert_eq!(world.get::<avian3d::prelude::CollisionLayers>(root_collider),
            Some(&avian3d::prelude::CollisionLayers::NONE));
        for (entity, parent, child) in joints {
            assert!(world.get::<SphericalJoint>(entity).is_none());
            assert!(world.get::<RevoluteJoint>(entity).is_none());
            assert!(world.get::<JointDamping>(entity).is_none());
            let joint = world.get::<SourcePhysicsJoint>(entity).unwrap();
            assert_eq!((joint.parent, joint.child), (parent, child));
            assert_eq!(joint.settings.position_iterations, 6);
            assert_eq!(joint.swing_spring.unwrap().stiffness, 5_000_000.0);
        }
        assert!(world.get::<JointDisabled>(pelvis).is_some());
        let driven = driven_profile.joints.iter_mut().find(|joint| joint.label == "pelvis").unwrap();
        driven.enabled = true;
        driven.definition.linear_motors[0] = SourceLinearMotor {
            stiffness: 100.0, position_enabled: true, ..default()
        };
        driven.definition.linear_position_target = Vec3::X * 0.2;
        let reapplied = world.run_system_once(move |mut commands: Commands,
            body_labels: Query<&BodyLabel>, collider_labels: Query<&ColliderLabel>,
            joint_labels: Query<&JointLabel>, spherical: Query<&SphericalJoint>,
            revolute: Query<&RevoluteJoint>, source_joints: Query<&SourcePhysicsJoint>,
            transforms: Query<&Transform>, mut colliders: Query<&mut Collider>| {
            apply_profile_to_spawned_ragdoll(&mut commands, &again, &driven_profile,
                Vec3::ONE, Vec3::ONE, true, false, &body_labels, &collider_labels, &joint_labels,
                &spherical, &revolute, &source_joints, &transforms, &mut colliders)
        }).unwrap();
        assert_eq!(reapplied.source_joint_count, 20);
        assert!(world.get::<JointDisabled>(pelvis).is_none());
        let motor = world.get::<SourcePhysicsJoint>(pelvis).unwrap();
        assert!(motor.linear_motors[0].position_enabled);
        assert_eq!(motor.linear_position_target, Vec3::X * 0.2);
        let mut source: serde_json::Value = serde_json::from_str(include_str!(
            "../../assets/ALS/Character/PA_Als.physics.json"
        )).unwrap();
        source["collision_disabled_pairs"].as_array_mut().unwrap()
            .retain(|pair| *pair != serde_json::json!([3, 10]));
        let body = source["bodies"].as_array_mut().unwrap().iter_mut()
            .find(|body| body["bone_name"] == "pelvis").unwrap();
        body["target"]["mass_properties"] = serde_json::json!({
            "mass_kg": 7.926393985748291,
            "unclamped_mass_kg": 7.926393985748291, "scale_exponent": 0.0, "update_with_scale": true,
            "center_of_mass_m": [0.1, 0.3, 0.2],
            "center_of_mass_nudge_m": [0.0, 0.0, 0.0], "inertia_scale": [1.0, 1.0, 1.0],
            "inertia_tensor_kg_m2": [[0.1, 0.0, 0.0], [0.0, 0.2, 0.0], [0.0, 0.0, 0.3]],
        });
        body["target"]["max_angular_speed_radians_per_second"] = serde_json::json!(2.0);
        body["target"]["material"]["dynamic_friction"] = serde_json::json!(0.8);
        body["target"]["material"]["static_friction"] = serde_json::json!(0.8);
        body["target"]["material"]["restitution"] = serde_json::json!(0.6);
        body["target"]["gyroscopic_torque_enabled"] = serde_json::json!(true);
        body["shapes"][0]["target"]["collision_enabled"] = serde_json::json!("QueryAndProbe");
        let spine = source["bodies"].as_array_mut().unwrap().iter_mut()
            .find(|body| body["bone_name"] == "spine_03").unwrap();
        spine["shapes"][1]["target"]["collision_enabled"] = serde_json::json!("QueryOnly");
        for (name, mode) in [("spine_01", "ProbeOnly"), ("spine_02", "PhysicsOnly"), ("root", "NoCollision")] {
            let body = source["bodies"].as_array_mut().unwrap().iter_mut()
                .find(|body| body["bone_name"] == name).unwrap();
            body["shapes"][0]["target"]["collision_enabled"] = serde_json::json!(mode);
        }
        let mass_profile = RagdollPhysicsProfile::from_json(&source.to_string()).unwrap();
        let mut mass_profile_scaled = mass_profile.clone();
        mass_profile_scaled.bodies.iter_mut().find(|body| body.label == "pelvis").unwrap()
            .gyroscopic_torque_enabled = false;
        for label in ["pelvis_0_capsule", "spine_03_1_capsule", "spine_01_0_capsule", "spine_02_0_capsule", "root_0_sphere"] {
            mass_profile_scaled.colliders.iter_mut().find(|collider| collider.label == label).unwrap()
                .collision_mode = RagdollShapeCollisionMode::QueryAndPhysics;
        }
        mass_profile_scaled.joints.iter_mut().find(|joint| joint.label == "clavicle_l").unwrap()
            .collision_disabled = true;
        let mass_again_scaled = SpawnedRagdoll {
            root: mass_again.root, bodies: mass_again.bodies.clone(),
            colliders: mass_again.colliders.clone(), joints: mass_again.joints.clone(),
        };
        world.run_system_once(move |mut commands: Commands,
            body_labels: Query<&BodyLabel>, collider_labels: Query<&ColliderLabel>,
            joint_labels: Query<&JointLabel>, spherical: Query<&SphericalJoint>,
            revolute: Query<&RevoluteJoint>, source_joints: Query<&SourcePhysicsJoint>,
            transforms: Query<&Transform>, mut colliders: Query<&mut Collider>| {
            apply_profile_to_spawned_ragdoll(&mut commands, &mass_again, &mass_profile,
                Vec3::ONE, Vec3::ONE, true, false, &body_labels, &collider_labels, &joint_labels,
                &spherical, &revolute, &source_joints, &transforms, &mut colliders)
        }).unwrap();
        assert!(world.get::<JointCollisionDisabled>(allowed_joint).is_none());
        let hook_allows = world.run_system_once(move |hooks: RagdollCollisionHooks,
            mut commands: Commands| hooks.filter_pairs(spine_main, clavicle_collider, &mut commands)).unwrap();
        assert!(hook_allows);
        assert_eq!(world.get::<avian3d::prelude::CenterOfMass>(pelvis_body).unwrap().0,
            Vec3::new(0.1, 0.3, 0.2));
        assert_eq!(world.get::<MaxAngularSpeed>(pelvis_body).unwrap().0, 2.0);
        assert_eq!(world.get::<Friction>(pelvis_body).unwrap().dynamic_coefficient, 0.8);
        assert_eq!(world.get::<Restitution>(pelvis_body).unwrap().coefficient, 0.6);
        assert_eq!(world.get::<Friction>(spine_body).unwrap().dynamic_coefficient, 0.7);
        assert_eq!(world.get::<Restitution>(spine_body).unwrap().coefficient, 0.3);
        assert!(world.get::<RagdollGyroscopicTorqueDisabled>(pelvis_body).is_none());
        assert!(world.get::<RagdollGyroscopicTorqueDisabled>(spine_body).is_some());
        let pair_filters = world.run_system_once(move |hooks: RagdollCollisionHooks,
            mut commands: Commands| (hooks.filter_pairs(spine_main, outside, &mut commands),
                hooks.filter_pairs(spine_aux, outside, &mut commands))).unwrap();
        assert_eq!(pair_filters, (true, false));
        assert!(world.get::<Sensor>(pelvis_collider).is_some());
        assert!(world.get::<Sensor>(probe_collider).is_some());
        assert!(world.get::<Sensor>(physics_only_collider).is_none());
        assert!(world.get::<ColliderDisabled>(root_collider).is_some());
        assert!(!world.get::<RagdollShapeCollisionMode>(probe_collider).unwrap().has_query());
        assert!(!world.get::<RagdollShapeCollisionMode>(physics_only_collider).unwrap().has_query());
        assert!(world.get::<NoAutoMass>(pelvis_body).is_some());
        assert!(world.get::<NoAutoCenterOfMass>(pelvis_body).is_some());
        assert!(world.get::<NoAutoAngularInertia>(pelvis_body).is_some());
        assert!((world.get::<Mass>(pelvis_body).unwrap().0 - 7.926394).abs() < 1e-5);
        world.run_system_once(move |mut commands: Commands,
            body_labels: Query<&BodyLabel>, collider_labels: Query<&ColliderLabel>,
            joint_labels: Query<&JointLabel>, spherical: Query<&SphericalJoint>,
            revolute: Query<&RevoluteJoint>, source_joints: Query<&SourcePhysicsJoint>,
            transforms: Query<&Transform>, mut colliders: Query<&mut Collider>| {
            apply_profile_to_spawned_ragdoll(&mut commands, &mass_again_scaled, &mass_profile_scaled,
                Vec3::splat(2.0), Vec3::splat(2.0), true, false, &body_labels, &collider_labels, &joint_labels,
                &spherical, &revolute, &source_joints, &transforms, &mut colliders)
        }).unwrap();
        assert_eq!(world.get::<CenterOfMass>(pelvis_body).unwrap().0, Vec3::new(0.2, 0.6, 0.4));
        assert_eq!(world.get::<MaxAngularSpeed>(pelvis_body).unwrap().0, 2.0);
        assert!(world.get::<RagdollGyroscopicTorqueDisabled>(pelvis_body).is_some());
        assert!(world.get::<JointCollisionDisabled>(allowed_joint).is_some());
        assert!(world.get::<Sensor>(pelvis_collider).is_none());
        assert!(world.get::<Sensor>(probe_collider).is_none());
        assert!(world.get::<ColliderDisabled>(root_collider).is_none());
        let restored = world.run_system_once(move |hooks: RagdollCollisionHooks,
            mut commands: Commands| hooks.filter_pairs(spine_aux, outside, &mut commands)).unwrap();
        assert!(restored);
        let mut principal = world.get::<AngularInertia>(pelvis_body).unwrap().principal.to_array();
        principal.sort_by(f32::total_cmp);
        assert!(principal.iter().zip([0.4, 0.8, 1.2]).all(|(actual, expected)| (actual - expected).abs() < 1e-5));
        app.update();
        assert_eq!(app.world().get::<avian3d::prelude::ComputedCenterOfMass>(pelvis_body).unwrap().0,
            Vec3::new(0.2, 0.6, 0.4));
        assert!((app.world().get::<avian3d::prelude::ComputedMass>(pelvis_body).unwrap().value() - 7.926394).abs() < 1e-5);
        let tensor = app.world().get::<avian3d::prelude::ComputedAngularInertia>(pelvis_body).unwrap().value();
        assert!((tensor.m00 - 0.4).abs() + (tensor.m11 - 0.8).abs() + (tensor.m22 - 1.2).abs() < 1e-5);
    }

    #[test]
    fn avian_max_angular_speed_clamps_vector_magnitude() {
        use avian3d::prelude::{AngularVelocity, RigidBody};
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        let entity = app.world_mut().spawn((RigidBody::Dynamic, Collider::sphere(0.5),
            GravityScale(0.0), AngularVelocity(Vec3::new(3.0, 4.0, 0.0)),
            MaxAngularSpeed(2.0))).id();
        app.update();
        app.update();
        let velocity = app.world().get::<AngularVelocity>(entity).unwrap().0;
        assert!((velocity.length() - 2.0).abs() < 1e-5, "{velocity:?}");
        assert!((velocity.normalize() - Vec3::new(0.6, 0.8, 0.0)).length() < 1e-5);
    }

    #[test]
    fn disabled_ragdoll_gyroscopic_torque_preserves_multi_axis_spin() {
        use avian3d::prelude::{AngularVelocity, ConstantTorque, RigidBody, Sleeping};
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            RagdollPhysicsProfilePlugin));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        let initial = Vec3::new(1.0, 2.0, 3.0);
        let spawn = |world: &mut World, x: f32, inertia: Vec3| world.spawn((RigidBody::Dynamic,
            Collider::sphere(0.5), Position(Vec3::X * x), GravityScale(0.0),
            AngularVelocity(initial), AngularDamping(0.0), Mass(1.0), NoAutoMass,
            AngularInertia::new(inertia), NoAutoAngularInertia)).id();
        let anisotropic = Vec3::new(1.0, 2.0, 3.0);
        let disabled = spawn(app.world_mut(), 0.0, anisotropic);
        app.world_mut().entity_mut(disabled).insert(RagdollGyroscopicTorqueDisabled);
        let enabled = spawn(app.world_mut(), 5.0, anisotropic);
        let isotropic = spawn(app.world_mut(), 10.0, Vec3::ONE);
        app.update();
        app.update();
        let disabled_velocity = app.world().get::<AngularVelocity>(disabled).unwrap().0;
        let enabled_velocity = app.world().get::<AngularVelocity>(enabled).unwrap().0;
        assert!((disabled_velocity - initial).length() < 1e-4, "{disabled_velocity:?}");
        assert!((enabled_velocity - initial).length() > 1e-3, "{enabled_velocity:?}");
        assert!((app.world().get::<AngularVelocity>(isotropic).unwrap().0 - initial).length() < 1e-4);
        assert!(!app.world().get::<SolverBody>(disabled).unwrap().is_gyroscopic());
        assert!(app.world().get::<SolverBody>(enabled).unwrap().is_gyroscopic());
        app.world_mut().entity_mut(disabled).remove::<RagdollGyroscopicTorqueDisabled>();
        app.world_mut().entity_mut(disabled).insert(AngularVelocity(initial));
        app.update();
        assert!((app.world().get::<AngularVelocity>(disabled).unwrap().0 - initial).length() > 1e-3);
        app.world_mut().entity_mut(disabled).insert((RagdollGyroscopicTorqueDisabled, AngularVelocity(initial)));
        app.update();
        assert!((app.world().get::<AngularVelocity>(disabled).unwrap().0 - initial).length() < 1e-4);
        app.world_mut().entity_mut(disabled).insert(Sleeping);
        app.update();
        assert!(app.world().get::<SolverBody>(disabled).is_none());
        app.world_mut().entity_mut(disabled).remove::<Sleeping>();
        app.world_mut().entity_mut(disabled).insert(AngularVelocity(initial));
        app.update();
        assert!((app.world().get::<AngularVelocity>(disabled).unwrap().0 - initial).length() < 1e-4);
        assert!(!app.world().get::<SolverBody>(disabled).unwrap().is_gyroscopic());
        let rebuilt = spawn(app.world_mut(), 15.0, anisotropic);
        app.world_mut().entity_mut(rebuilt).insert(RagdollGyroscopicTorqueDisabled);
        app.update();
        assert!((app.world().get::<AngularVelocity>(rebuilt).unwrap().0 - initial).length() < 1e-4);
        app.world_mut().entity_mut(disabled).insert((AngularVelocity(initial), ConstantTorque::new(6.0, 0.0, 0.0)));
        app.update();
        assert!(app.world().get::<AngularVelocity>(disabled).unwrap().0.x > initial.x + 0.05);
    }

    #[test]
    fn query_only_shape_remains_queryable_without_a_physics_pair() {
        use avian3d::prelude::{Collisions, RigidBody, SpatialQuery, SpatialQueryFilter};
        use bevy::{ecs::system::RunSystemOnce, time::TimeUpdateStrategy};
        use std::time::Duration;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)
                .with_collision_hooks::<RagdollCollisionHooks>()));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        let world = app.world_mut();
        let main = world.spawn((RigidBody::Dynamic, Collider::sphere(0.5), Position(Vec3::ZERO),
            ActiveCollisionHooks::FILTER_PAIRS, RagdollShapeCollisionMode::QueryAndPhysics)).id();
        let main_other = world.spawn((RigidBody::Static, Collider::sphere(0.5), Position(Vec3::X * 0.75))).id();
        let auxiliary = world.spawn((RigidBody::Dynamic, Collider::sphere(0.5), Position(Vec3::X * 5.0),
            ActiveCollisionHooks::FILTER_PAIRS, RagdollShapeCollisionMode::QueryOnly)).id();
        let auxiliary_other = world.spawn((RigidBody::Static, Collider::sphere(0.5), Position(Vec3::X * 5.75))).id();
        let probe = world.spawn((RigidBody::Dynamic, Collider::sphere(0.5), Position(Vec3::X * 10.0),
            ActiveCollisionHooks::FILTER_PAIRS, RagdollShapeCollisionMode::ProbeOnly, Sensor,
            Mass(3.0), NoAutoMass, AngularInertia::new(Vec3::splat(0.1)), NoAutoAngularInertia)).id();
        let probe_other = world.spawn((RigidBody::Static, Collider::sphere(0.5), Position(Vec3::X * 10.75))).id();
        let disabled = world.spawn((RigidBody::Dynamic, Collider::sphere(0.5), Position(Vec3::X * 20.0),
            ActiveCollisionHooks::FILTER_PAIRS, RagdollShapeCollisionMode::NoCollision, ColliderDisabled)).id();
        let disabled_other = world.spawn((RigidBody::Static, Collider::sphere(0.5), Position(Vec3::X * 20.75))).id();
        let physics_only = world.spawn((RigidBody::Dynamic, Collider::sphere(0.5), Position(Vec3::X * 25.0),
            ActiveCollisionHooks::FILTER_PAIRS, RagdollShapeCollisionMode::PhysicsOnly)).id();
        let physics_other = world.spawn((RigidBody::Static, Collider::sphere(0.5), Position(Vec3::X * 25.75))).id();
        let query_probe = world.spawn((RigidBody::Dynamic, Collider::sphere(0.5), Position(Vec3::X * 30.0),
            ActiveCollisionHooks::FILTER_PAIRS, RagdollShapeCollisionMode::QueryAndProbe, Sensor)).id();
        let query_probe_other = world.spawn((RigidBody::Static, Collider::sphere(0.5), Position(Vec3::X * 30.75))).id();
        app.update();
        app.update();
        assert!((app.world().get::<avian3d::prelude::ComputedMass>(probe).unwrap().value() - 3.0).abs() < 1e-5);
        let contacts = app.world_mut().run_system_once(move |collisions: Collisions| {
            (collisions.get(main, main_other).is_some(), collisions.get(auxiliary, auxiliary_other).is_some(),
                collisions.get(probe, probe_other).map(|pair| (pair.is_touching(), pair.generates_constraints())),
                collisions.get(disabled, disabled_other).is_some(), collisions.get(physics_only, physics_other).is_some(),
                collisions.get(query_probe, query_probe_other).map(|pair| (pair.is_touching(), pair.generates_constraints())))
        }).unwrap();
        assert_eq!(contacts, (true, false, Some((true, false)), false, true, Some((true, false))));
        let hit = app.world_mut().run_system_once(move |spatial: SpatialQuery| {
            spatial.cast_ray(Vec3::X * 4.0, Dir3::X, 2.0, true, &SpatialQueryFilter::default())
        }).unwrap().unwrap();
        assert_eq!(hit.entity, auxiliary);
        let no_hit = app.world_mut().run_system_once(move |spatial: SpatialQuery| {
            spatial.cast_ray(Vec3::X * 18.5, Dir3::X, 1.1, true, &SpatialQueryFilter::default())
        }).unwrap();
        assert!(no_hit.is_none());
        let raw_hit = app.world_mut().run_system_once(move |spatial: SpatialQuery| {
            spatial.cast_ray(Vec3::X * 23.5, Dir3::X, 1.1, true, &SpatialQueryFilter::default())
        }).unwrap().unwrap();
        assert_eq!(raw_hit.entity, physics_only);
        let filtered = app.world_mut().run_system_once(move |spatial: RagdollSpatialQuery| {
            let filter = SpatialQueryFilter::default();
            let shape = Collider::sphere(0.1);
            let config = ShapeCastConfig { max_distance: 1.1, ..default() };
            let mut callback_hits = Vec::new();
            spatial.shape_intersections_callback(&shape, Vec3::X * 25.0, Quat::IDENTITY,
                &filter, |entity| { callback_hits.push(entity); true });
            (
                spatial.cast_ray(Vec3::X * 23.5, Dir3::X, 1.1, true, &filter).is_none(),
                spatial.cast_ray_predicate(Vec3::X * 23.5, Dir3::X, 1.1, true, &filter, &|_| true).is_none(),
                spatial.cast_shape(&shape, Vec3::X * 23.5, Quat::IDENTITY, Dir3::X, &config, &filter).is_none(),
                spatial.cast_shape_predicate(&shape, Vec3::X * 23.5, Quat::IDENTITY, Dir3::X,
                    &config, &filter, &|_| true).is_none(),
                spatial.shape_intersections(&shape, Vec3::X * 25.0, Quat::IDENTITY, &filter).is_empty(),
                callback_hits.is_empty(),
                spatial.cast_ray(Vec3::X * 4.0, Dir3::X, 2.0, true, &filter).map(|hit| hit.entity),
                spatial.cast_ray(Vec3::X * 8.5, Dir3::X, 1.1, true, &filter).is_none(),
                spatial.cast_ray(Vec3::X * 28.5, Dir3::X, 1.1, true, &filter).map(|hit| hit.entity),
            )
        }).unwrap();
        assert_eq!(filtered, (true, true, true, true, true, true, Some(auxiliary), true, Some(query_probe)));
    }

    #[test]
    fn joint_and_asset_collision_switches_reach_joint_graph_and_broad_phase() {
        use avian3d::{dynamics::solver::joint_graph::JointGraph, prelude::{Collisions, GravityScale, RigidBody}};
        use bevy::{ecs::system::RunSystemOnce, time::TimeUpdateStrategy};
        use std::time::Duration;

        let profile = RagdollPhysicsProfile::from_json(include_str!(
            "../../assets/ALS/Character/PA_Als.physics.json"
        )).unwrap();
        let template = &profile.joints.iter().find(|joint| joint.label == "clavicle_l").unwrap().definition;
        for joint_disabled in [false, true] {
            for asset_disabled in [false, true] {
                let mut app = App::new();
                app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
                    TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)
                        .with_collision_hooks::<RagdollCollisionHooks>(),
                    crate::extensions::SourcePhysicsJointPlugin, RagdollPhysicsProfilePlugin));
                app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
                app.finish();
                app.cleanup();
                let world = app.world_mut();
                let group = world.spawn_empty().id();
                let pairs: Arc<[(u8, u8)]> = if asset_disabled { Arc::from([(0, 1)]) } else { Arc::from([]) };
                let make_body = |world: &mut World, index: u8, x: f32| world.spawn((
                    RigidBody::Dynamic, Collider::sphere(0.5), Position(Vec3::X * x), GravityScale(0.0),
                    ActiveCollisionHooks::FILTER_PAIRS,
                    RagdollCollisionMember { group, body_index: index, source_layers: CollisionLayers::from_bits(1, 1),
                        disabled_pairs: pairs.clone() },
                )).id();
                let parent = make_body(world, 0, 0.0);
                let child = make_body(world, 1, 0.75);
                let mut source_joint = template.clone();
                source_joint.parent = parent;
                source_joint.child = child;
                source_joint.settings.position_iterations = 0;
                source_joint.settings.projection_iterations = 0;
                source_joint.angular_limits = [SourceAngularLimit::Free; 3];
                source_joint.linear_locked = false;
                source_joint.swing_spring = None;
                source_joint.twist_spring = None;
                source_joint.motors = [SourceAngularMotor::default(); 3];
                source_joint.linear_motors = [SourceLinearMotor::default(); 3];
                let joint = world.spawn((source_joint, JointCollisionDisabled)).id();
                if !joint_disabled { set_joint_collision_disabled(world, joint, &[parent, child], false); }
                app.update();
                app.update();
                let edge = app.world().resource::<JointGraph>().get(joint).unwrap();
                assert_eq!(edge.collision_disabled, joint_disabled);
                let pair = app.world_mut().run_system_once(move |collisions: Collisions| collisions.get(parent, child).is_some()).unwrap();
                assert_eq!(pair, !joint_disabled && !asset_disabled, "joint={joint_disabled} asset={asset_disabled}");
                if !joint_disabled && !asset_disabled {
                    set_joint_collision_disabled(app.world_mut(), joint, &[parent, child], true);
                    app.update();
                    assert!(app.world().resource::<JointGraph>().get(joint).unwrap().collision_disabled);
                    let blocked = app.world_mut().run_system_once(move |collisions: Collisions| collisions.get(parent, child).is_none()).unwrap();
                    assert!(blocked);
                    set_joint_collision_disabled(app.world_mut(), joint, &[parent, child], false);
                    app.update();
                    assert!(!app.world().resource::<JointGraph>().get(joint).unwrap().collision_disabled);
                    let restored = app.world_mut().run_system_once(move |collisions: Collisions| collisions.get(parent, child).is_some()).unwrap();
                    assert!(restored, "distance={} parent={:?} child={:?}",
                        app.world().get::<Position>(parent).unwrap().0.distance(app.world().get::<Position>(child).unwrap().0),
                        app.world().get::<Position>(parent).unwrap().0, app.world().get::<Position>(child).unwrap().0);
                    for entity in [parent, child] {
                        app.world_mut().get_mut::<RagdollCollisionMember>(entity).unwrap().disabled_pairs = Arc::from([(0, 1)]);
                    }
                    refresh_ragdoll_collision_pairs(app.world_mut(), &[parent, child]);
                    app.update();
                    let asset_blocked = app.world_mut().run_system_once(move |collisions: Collisions| collisions.get(parent, child).is_none()).unwrap();
                    assert!(asset_blocked);
                    for entity in [parent, child] {
                        app.world_mut().get_mut::<RagdollCollisionMember>(entity).unwrap().disabled_pairs = Arc::from([]);
                    }
                    refresh_ragdoll_collision_pairs(app.world_mut(), &[parent, child]);
                    app.update();
                    let asset_restored = app.world_mut().run_system_once(move |collisions: Collisions| collisions.get(parent, child).is_some()).unwrap();
                    assert!(asset_restored);
                }
            }
        }
    }

    #[test]
    fn changed_profile_pair_table_refreshes_existing_contact() {
        use avian3d::prelude::{Collisions, GravityScale, RigidBody};
        use bevy::{ecs::system::RunSystemOnce, reflect::{PartialReflect, ReflectMut}, time::TimeUpdateStrategy};
        use bevy_animation_graph::core::ragdoll::definition::Ragdoll;
        use std::time::Duration;

        let ragdoll: Ragdoll = ron::from_str(include_str!(
            "../../assets/ALS/Character/PA_Als.rag.ron"
        )).unwrap();
        let mut profile = RagdollPhysicsProfile::from_json(include_str!(
            "../../assets/ALS/Character/PA_Als.physics.json"
        )).unwrap();
        profile.collision_disabled_pairs = Arc::from([]);
        let source_joint = profile.joints.iter_mut().find(|joint| joint.label == "clavicle_l").unwrap();
        source_joint.definition.settings.position_iterations = 0;
        source_joint.definition.settings.projection_iterations = 0;
        source_joint.definition.angular_limits = [SourceAngularLimit::Free; 3];
        source_joint.definition.linear_locked = false;
        source_joint.definition.swing_spring = None;
        source_joint.definition.twist_spring = None;
        source_joint.definition.motors = [SourceAngularMotor::default(); 3];
        source_joint.definition.linear_motors = [SourceLinearMotor::default(); 3];

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            TransformPlugin, avian3d::prelude::PhysicsPlugins::new(PostUpdate)
                .with_collision_hooks::<RagdollCollisionHooks>(),
            crate::extensions::SourcePhysicsJointPlugin, RagdollPhysicsProfilePlugin));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        let world = app.world_mut();
        let root = world.spawn_empty().id();
        let parent = world.spawn((RigidBody::Dynamic, Collider::sphere(0.5), Position(Vec3::ZERO),
            Transform::IDENTITY, GravityScale(0.0), BodyLabel("spine_03".into()),
            ColliderLabel("spine_03_0_capsule".into()))).id();
        let child = world.spawn((RigidBody::Dynamic, Collider::sphere(0.5), Position(Vec3::X * 0.75),
            Transform::from_xyz(0.75, 0.0, 0.0), GravityScale(0.0), BodyLabel("clavicle_l".into()),
            ColliderLabel("clavicle_l_0_capsule".into()))).id();
        let joint = world.spawn((JointLabel("clavicle_l".into()), SphericalJoint::new(parent, child),
            JointCollisionDisabled)).id();
        let mut spawned = SpawnedRagdoll::new(root);
        let mut body_ids = ragdoll.bodies.keys();
        spawned.bodies.insert(body_ids.next().unwrap().clone(), parent);
        spawned.bodies.insert(body_ids.next().unwrap().clone(), child);
        let mut collider_ids = ragdoll.colliders.keys();
        spawned.colliders.insert(collider_ids.next().unwrap().clone(), parent);
        spawned.colliders.insert(collider_ids.next().unwrap().clone(), child);
        spawned.joints.insert(ragdoll.joints.keys().next().unwrap().clone(), joint);
        let mut player = AnimationGraphPlayer::new(Handle::default());
        let ReflectMut::Struct(fields) = player.reflect_mut() else { panic!("player must reflect as a struct") };
        *fields.field_mut("spawned_ragdoll").unwrap()
            .try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
        let player = world.spawn((player, profile, Transform::IDENTITY)).id();
        app.update();
        app.update();
        let initial = app.world_mut().run_system_once(move |collisions: Collisions| collisions.get(parent, child).is_some()).unwrap();
        assert!(initial);
        app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().collision_disabled_pairs = Arc::from([(3, 10)]);
        app.update();
        let blocked = app.world_mut().run_system_once(move |collisions: Collisions| collisions.get(parent, child).is_none()).unwrap();
        assert!(blocked);
        app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().collision_disabled_pairs = Arc::from([]);
        app.update();
        let restored = app.world_mut().run_system_once(move |collisions: Collisions| collisions.get(parent, child).is_some()).unwrap();
        assert!(restored);
    }

    #[test]
    fn source_mass_scale_policy_reaches_initialization_updates_and_rebuilds() {
        use avian3d::prelude::{ComputedAngularInertia, ComputedCenterOfMass, ComputedMass, PhysicsPlugins, RigidBody};
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::definition::Body;
        fn attach(world: &mut World, player: Entity) -> (Entity, Entity) {
            let root = world.spawn((Transform::IDENTITY, ChildOf(player))).id();
            let body = world.spawn((RigidBody::Dynamic, BodyLabel("pelvis".into()), Transform::IDENTITY, ChildOf(root))).id();
            world.spawn((Collider::sphere(0.5), Mass(1.0), Transform::IDENTITY, ChildOf(body)));
            let mut spawned = SpawnedRagdoll::new(root);
            spawned.bodies.insert(Body::new().id, body);
            let mut graph_player = world.get_mut::<AnimationGraphPlayer>(player).unwrap();
            let ReflectMut::Struct(fields) = graph_player.reflect_mut() else { panic!("player must be a struct") };
            *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
            (root, body)
        }
        fn check(world: &World, body: Entity, input: RagdollMassProperties, scale: f32) {
            let expected = (input.unclamped_mass_kg * scale.powf(input.scale_exponent)).max(0.001);
            let mass = world.get::<ComputedMass>(body).unwrap().value();
            assert!((mass - expected).abs() < expected * 1e-5, "Mass policy input={input:?} scale={scale}: {mass} != {expected}");
            assert!(world.get::<ComputedCenterOfMass>(body).unwrap().0.abs_diff_eq(input.center_of_mass_m * scale, 1e-5));
            let tensor = world.get::<ComputedAngularInertia>(body).unwrap().value();
            let expected_tensor = input.inertia_tensor_kg_m2 * scale * scale * (expected / input.mass_kg);
            assert!(Vec3::new(tensor.m00, tensor.m11, tensor.m22).abs_diff_eq(
                Vec3::new(expected_tensor.x_axis.x, expected_tensor.y_axis.y, expected_tensor.z_axis.z), 1e-5));
        }
        for exponent in [0.0, 2.25, 3.0] {
            for update in [false, true] {
                for base_mass in [0.0_f32, 0.0005, 2.0] {
                    for initial in [0.5, 2.0] {
                        let mut app = App::new();
                        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
                            PhysicsPlugins::new(PostUpdate), RagdollPhysicsProfilePlugin));
                        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::ZERO));
                        app.finish(); app.cleanup();
                        let input = RagdollMassProperties { mass_kg: base_mass.max(0.001), unclamped_mass_kg: base_mass,
                            scale_exponent: exponent, update_with_scale: update,
                            center_of_mass_m: Vec3::new(0.1, 0.2, 0.3),
                            inertia_tensor_kg_m2: Mat3::from_diagonal(Vec3::new(0.1, 0.2, 0.3) * base_mass.max(0.001)) };
                        let mut profile = RagdollPhysicsProfile::from_json(include_str!("../../assets/ALS/Character/PA_Als.physics.json")).unwrap();
                        profile.bodies.retain(|body| body.label == "pelvis");
                        profile.bodies[0].mass_properties = Some(input);
                        profile.bodies[0].ccd_enabled = update;
                        profile.colliders.clear(); profile.joints.clear();
                        let player = app.world_mut().spawn((AnimationGraphPlayer::new(Handle::default()), profile,
                            Transform::from_scale(Vec3::splat(initial)))).id();
                        let (old_root, body) = attach(app.world_mut(), player);
                        for scale in [initial, initial * 2.0, initial * 0.5] {
                            app.world_mut().get_mut::<Transform>(player).unwrap().scale = Vec3::splat(scale);
                            app.update();
                            check(app.world(), body, input, if update { scale } else { initial });
                            assert_eq!(app.world().get::<avian3d::prelude::SweptCcd>(body).is_some(), update);
                        }
                        let mut changed = input;
                        changed.unclamped_mass_kg += 2.0;
                        changed.mass_kg = changed.unclamped_mass_kg;
                        app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().bodies[0].mass_properties = Some(changed);
                        app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().bodies[0].ccd_enabled = !update;
                        app.update();
                        check(app.world(), body, changed, initial * 0.5);
                        assert_eq!(app.world().get::<avian3d::prelude::SweptCcd>(body).is_some(), !update);
                        let (new_root, new_body) = attach(app.world_mut(), player);
                        app.update();
                        assert!(app.world().get_entity(old_root).is_err() && app.world().get_entity(new_root).is_ok());
                        check(app.world(), new_body, changed, initial * 0.5);
                        assert_eq!(app.world().get::<avian3d::prelude::SweptCcd>(new_body).is_some(), !update);
                        app.update();
                        check(app.world(), new_body, changed, initial * 0.5);
                    }
                }
            }
        }
    }

    #[test]
    fn ccd_sweep_uses_end_rotation_com_and_scaled_child_geometry() {
        use avian3d::prelude::{ColliderOf, ColliderTransform, ComputedCenterOfMass, RigidBodyDisabled, SweptCcd};
        use bevy::ecs::system::RunSystemOnce;
        let settings = RagdollPhysicsProfile::from_json(include_str!("../../assets/ALS/Character/PA_Als.physics.json")).unwrap().ccd_settings;
        for initial_angle in [-0.4_f32, 0.7] {
            for delta_angle in [0.0_f32, 0.6] {
                for scale in [0.5_f32, 1.0, 2.0] {
                    let mut world = World::new();
                    let mut time = Time::<()>::default();time.advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));world.insert_resource(time);
                    let position = Vec3::new(-1.0, 0.0, 0.0);
                    let rotation = Quat::from_rotation_z(initial_angle);
                    let delta_rotation = Quat::from_rotation_z(delta_angle);
                    let end_rotation = delta_rotation * rotation;
                    let center = Vec3::new(0.0, 0.1, 0.0);
                    let offset = Vec3::new(0.2, 0.1, 0.0);
                    let body = world.spawn((RigidBody::Dynamic, Position(position), Rotation(rotation),
                        ComputedCenterOfMass(center), SolverBody { delta_position: Vec3::X * 4.0,
                            linear_velocity: Vec3::X * 240.0, delta_rotation: Rotation(delta_rotation), ..default() },
                        settings, SweptCcd::LINEAR)).id();
                    let mut sphere = Collider::sphere(0.05);sphere.set_scale(Vec3::splat(scale), 16);
                    world.spawn((sphere, ColliderOf { body }, ColliderTransform { translation: offset, ..default() }));
                    let wall = world.spawn((RigidBody::Static, Position(Vec3::X), Rotation::IDENTITY, ComputedCenterOfMass(Vec3::ZERO))).id();
                    world.spawn((Collider::cuboid(0.01, 4.0, 4.0), ColliderOf { body: wall }, ColliderTransform::default()));
                    world.run_system_once(solve_profile_linear_ccd).unwrap();
                    let solver = world.get::<SolverBody>(body).unwrap();
                    let actual = position + solver.delta_position + rotation * center - end_rotation * center;
                    let expected_x = 1.0 - 0.005 - 0.05 * scale + 0.1 * scale * settings.allowed_depth_bounds_scale
                        - (end_rotation * offset).x;
                    assert!((actual.x - expected_x).abs() < 1e-5,
                        "End-pose CCD: initial={initial_angle} delta={delta_angle} scale={scale} {actual:?} != {expected_x}");
                    assert!(solver.delta_rotation.0.abs_diff_eq(delta_rotation, 1e-6));
                    let before = solver.delta_position;
                    world.entity_mut(body).insert(RigidBodyDisabled);
                    world.run_system_once(solve_profile_linear_ccd).unwrap();
                    assert_eq!(world.get::<SolverBody>(body).unwrap().delta_position, before);
                }
            }
        }
    }

    #[test]
    fn ccd_shape_modes_control_actual_source_and_obstacle_motion() {
        use avian3d::prelude::{Gravity, LinearVelocity, PhysicsPlugins, SpeculativeMargin, SubstepCount};
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::definition::Body;
        let modes = [Some(RagdollShapeCollisionMode::QueryOnly), Some(RagdollShapeCollisionMode::PhysicsOnly),
            Some(RagdollShapeCollisionMode::QueryAndPhysics), Some(RagdollShapeCollisionMode::NoCollision),
            Some(RagdollShapeCollisionMode::ProbeOnly), Some(RagdollShapeCollisionMode::QueryAndProbe), None];
        for source in [false, true] {
         for mode in modes {
          for probe_sweeps in [false, true] {
            let mut profile = crate::pa_als::parse_pa_als_profile();
            profile.bodies.retain(|body| body.label == "pelvis");profile.colliders.clear();profile.joints.clear();
            profile.bodies[0].ccd_enabled = true;profile.bodies[0].linear_damping = 0.0;
            profile.ccd_settings.include_probe_shapes = probe_sweeps;
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
                PhysicsPlugins::new(PostUpdate).with_collision_hooks::<RagdollCollisionHooks>(), RagdollPhysicsProfilePlugin));
            app.insert_resource(Gravity(Vec3::ZERO));app.insert_resource(SubstepCount(1));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
            app.finish();app.cleanup();
            let player = app.world_mut().spawn((AnimationGraphPlayer::new(Handle::default()), profile, Transform::IDENTITY)).id();
            let root = app.world_mut().spawn(Transform::IDENTITY).id();
            let body = app.world_mut().spawn((RigidBody::Dynamic, BodyLabel("pelvis".into()), Transform::from_xyz(-1.0, 0.0, 0.0), ChildOf(root))).id();
            let collider = app.world_mut().spawn((Collider::sphere(0.05), SpeculativeMargin(0.0), Transform::IDENTITY, ChildOf(body),
                ActiveCollisionHooks::FILTER_PAIRS, if source { mode.unwrap_or(RagdollShapeCollisionMode::QueryAndPhysics) } else { RagdollShapeCollisionMode::QueryAndPhysics })).id();
            let wall = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.01, 2.0, 2.0), Transform::IDENTITY,
                ActiveCollisionHooks::FILTER_PAIRS, if source { RagdollShapeCollisionMode::QueryAndPhysics } else { mode.unwrap_or(RagdollShapeCollisionMode::QueryAndPhysics) })).id();
            let selected = if source { collider } else { wall };
            if mode == Some(RagdollShapeCollisionMode::NoCollision) { app.world_mut().entity_mut(selected).insert(ColliderDisabled); }
            if matches!(mode, Some(RagdollShapeCollisionMode::ProbeOnly | RagdollShapeCollisionMode::QueryAndProbe)) { app.world_mut().entity_mut(selected).insert(Sensor); }
            if mode.is_none() { app.world_mut().entity_mut(selected).remove::<RagdollShapeCollisionMode>().insert(Sensor); }
            let mut spawned = SpawnedRagdoll::new(root);spawned.bodies.insert(Body::new().id, body);
            let mut graph_player = app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap();
            let ReflectMut::Struct(fields) = graph_player.reflect_mut() else { panic!("player must be a struct") };
            *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
            app.update();app.world_mut().entity_mut(body).insert(LinearVelocity(Vec3::X * 240.0));app.update();
            let position = app.world().get::<Position>(body).unwrap().0;
            let solver = app.world().get::<SolverBody>(body).unwrap();
            let blocks = matches!(mode, Some(RagdollShapeCollisionMode::PhysicsOnly | RagdollShapeCollisionMode::QueryAndPhysics));
            assert_eq!(solver.delta_position.x < 4.0, blocks, "CCD delta mode={mode:?}, source={source}, probes={probe_sweeps}: {:?}", solver.delta_position);
            if blocks { assert!((position.x + 0.035).abs() < 1e-5, "Physical mode must keep source TOI/depth: {position:?}"); }
            else { assert!(position.x > 2.9, "Nonphysical mode must not rewind CCD: {mode:?}, source={source}, probes={probe_sweeps}, {position:?}"); }
          }
         }
        }
    }

    #[test]
    fn source_ccd_switch_reaches_high_speed_thin_wall_collision() {
        use avian3d::prelude::{Gravity, LinearVelocity, PhysicsPlugins, RigidBody, SpeculativeMargin, SubstepCount, SweptCcd};
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::definition::Body;
        for enabled in [false, true] {
          for child_layout in [0, 1, 2] {
           for obstacle in ["static", "child_static", "kinematic", "dynamic", "sensor", "layers", "disabled"] {
            for threshold in [-1.0_f32, 0.0, 0.4, 1e6] {
            let mut document: serde_json::Value = serde_json::from_str(include_str!("../../assets/ALS/Character/PA_Als.physics.json")).unwrap();
            document["target_ccd"]["enable_threshold_bounds_scale"] = serde_json::json!(threshold);
            for body in document["bodies"].as_array_mut().unwrap() { body["target"]["ccd_enabled"] = serde_json::json!(enabled); }
            let mut profile = RagdollPhysicsProfile::from_json(&document.to_string()).unwrap();
            profile.bodies.retain(|body| body.label == "pelvis");
            profile.colliders.clear(); profile.joints.clear();
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
                PhysicsPlugins::new(PostUpdate), RagdollPhysicsProfilePlugin));
            app.insert_resource(Gravity(Vec3::ZERO));
            app.insert_resource(SubstepCount(if enabled && child_layout == 2 && obstacle == "static" && threshold == 0.4 { 4 } else { 1 }));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
            app.finish(); app.cleanup();
            let player = app.world_mut().spawn((AnimationGraphPlayer::new(Handle::default()), profile, Transform::IDENTITY)).id();
            let root = app.world_mut().spawn((Transform::IDENTITY, ChildOf(player))).id();
            let body = app.world_mut().spawn((RigidBody::Dynamic, BodyLabel("pelvis".into()),
                Transform::from_xyz(-1.0, 0.0, 0.0), ChildOf(root))).id();
            if child_layout != 0 {
                app.world_mut().spawn((Collider::sphere(0.05), SpeculativeMargin(0.0), Transform::from_xyz(0.0, 0.2, 0.0), ChildOf(body)));
                if child_layout == 2 { app.world_mut().spawn((Collider::sphere(0.05), SpeculativeMargin(0.0),
                    Transform::from_xyz(0.2, -0.2, 0.0), ChildOf(body))); }
            } else { app.world_mut().entity_mut(body).insert((Collider::sphere(0.05), SpeculativeMargin(0.0))); }
            let mode = match obstacle { "kinematic" => RigidBody::Kinematic, "dynamic" => RigidBody::Dynamic, _ => RigidBody::Static };
            let wall = app.world_mut().spawn((mode, Transform::IDENTITY)).id();
            let wall_collider = if obstacle == "child_static" {
                app.world_mut().spawn((Collider::cuboid(0.01, 2.0, 2.0), Transform::IDENTITY, ChildOf(wall))).id()
            } else { app.world_mut().entity_mut(wall).insert(Collider::cuboid(0.01, 2.0, 2.0)); wall };
            match obstacle {
                "sensor" => { app.world_mut().entity_mut(wall_collider).insert(Sensor); }
                "layers" => { app.world_mut().entity_mut(wall_collider).insert(CollisionLayers::NONE); }
                "disabled" => { app.world_mut().entity_mut(wall_collider).insert(ColliderDisabled); }
                _ => {}
            }
            let mut spawned = SpawnedRagdoll::new(root);spawned.bodies.insert(Body::new().id, body);
            let mut graph_player = app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap();
            let ReflectMut::Struct(fields) = graph_player.reflect_mut() else { panic!("player must be a struct") };
            *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
            app.update();
            app.world_mut().entity_mut(body).insert(LinearVelocity(Vec3::X * 240.0));
            if matches!(obstacle, "dynamic" | "kinematic") { app.world_mut().entity_mut(wall).insert(LinearVelocity(Vec3::X * 30.0)); }
            app.update();
            let position = app.world().get::<avian3d::prelude::Position>(body).unwrap().0;
            assert_eq!(app.world().get::<SweptCcd>(body).is_some(), enabled,
                "Source CCD flag must reach the actual body, enabled={enabled}, child={child_layout}, position={position:?}");
            if enabled && threshold >= 0.0 && threshold < 1e6 && !matches!(obstacle, "sensor" | "layers" | "disabled") {
                let wall_position = app.world().get::<avian3d::prelude::Position>(wall).unwrap().0;
                assert!(position.x < wall_position.x, "CCD body tunneled: child={child_layout} wall={obstacle} threshold={threshold} {position:?} >= {wall_position:?}");
                if obstacle == "static" && child_layout == 0 {
                    assert!((position.x + 0.035).abs() < 1e-5, "Source target depth must be .2 * .1: {position:?}");
                    if threshold == 0.4 {
                        for _ in 0..3 {
                            app.update();
                            let continued = app.world().get::<avian3d::prelude::Position>(body).unwrap().0;
                            assert!(continued.x < 0.0, "CCD contact must remain blocking on later frames: {continued:?}");
                        }
                    }
                }
            } else { assert!(position.x > 1.0, "Discrete/filtered control must expose tunneling: {obstacle} threshold={threshold} {position:?}"); }
            }
           }
          }
        }
    }

    #[test]
    fn source_body_inertia_algorithm_matches_native_ue_samples() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../tools/tests/fixtures/body_inertia_conditioning.native.json")).unwrap();
        let settings: SourceInertiaConditioningSettings = serde_json::from_value(fixture["settings"].clone()).unwrap();
        let vector = |value: &serde_json::Value| Vec3::new(value[0].as_f64().unwrap() as f32, value[1].as_f64().unwrap() as f32, value[2].as_f64().unwrap() as f32);
        for (index, sample) in fixture["samples"].as_array().unwrap().iter().enumerate() {
            let settings = SourceInertiaConditioningSettings { enabled: true,
                distance_m: sample["maximum_distance_m"].as_f64().unwrap() as f32,
                rotation_ratio: sample["maximum_rotation_ratio"].as_f64().unwrap() as f32,
                max_inverse_inertia_component_ratio: sample["maximum_component_ratio"].as_f64().unwrap() as f32, ..settings };
            let actual = settings.inverse_scale(sample["inverse_mass"].as_f64().unwrap() as f32, vector(&sample["inverse_inertia"]), vector(&sample["extents_m"]));
            assert!(actual.abs_diff_eq(vector(&sample["inverse_scale"]), 1e-6), "native inertia sample {index}: {actual:?} != {:?}", vector(&sample["inverse_scale"]));
        }
    }

    #[test]
    fn source_body_inertia_conditioning_reaches_contact_response() {
        use avian3d::prelude::{AngularVelocity, Gravity, LinearVelocity, PhysicsPlugins};
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::definition::Body;
        let mut angular_response = Vec::new();
        for enabled in [false, true] {
            let mut profile = crate::pa_als::parse_pa_als_profile();
            profile.bodies.retain(|body| body.label == "pelvis");profile.colliders.clear();profile.joints.clear();
            let body_profile = &mut profile.bodies[0];
            body_profile.inertia_conditioning_enabled = enabled;
            body_profile.linear_damping = 0.0;body_profile.angular_damping = 0.0;
            body_profile.friction = Friction::new(0.0);body_profile.restitution = Restitution::new(0.0);
            body_profile.mass_properties = Some(RagdollMassProperties { mass_kg: 1.0, unclamped_mass_kg: 1.0,
                scale_exponent: 0.0, update_with_scale: false, center_of_mass_m: Vec3::ZERO,
                inertia_tensor_kg_m2: Mat3::IDENTITY / 600.0 });
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
                PhysicsPlugins::new(PostUpdate), RagdollPhysicsProfilePlugin));
            app.insert_resource(Gravity(Vec3::ZERO));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
            app.finish();app.cleanup();
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(10.0, 0.1, 10.0), Friction::new(0.0), Restitution::new(0.0), Transform::from_xyz(0.0, -0.05, 0.0)));
            let player = app.world_mut().spawn((AnimationGraphPlayer::new(Handle::default()), profile, Transform::IDENTITY)).id();
            let root = app.world_mut().spawn(Transform::IDENTITY).id();
            let body = app.world_mut().spawn((RigidBody::Dynamic, BodyLabel("pelvis".into()), Transform::from_xyz(0.0, 0.06, 0.0).with_rotation(Quat::from_rotation_z(0.3)), ChildOf(root))).id();
            app.world_mut().spawn((Collider::cuboid(0.1, 0.1, 0.1), Transform::IDENTITY, ChildOf(body)));
            let mut spawned = SpawnedRagdoll::new(root);spawned.bodies.insert(Body::new().id, body);
            let mut graph_player = app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap();
            let ReflectMut::Struct(fields) = graph_player.reflect_mut() else { panic!("player must be a struct") };
            *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
            app.update();
            app.world_mut().entity_mut(body).insert(LinearVelocity(Vec3::NEG_Y));
            app.update();
            let angular = app.world().get::<AngularVelocity>(body).unwrap().0.z.abs();
            assert!(angular.is_finite() && angular > 0.1, "contact control must create angular correction: {angular}");
            angular_response.push(angular);
        }
        assert!(angular_response[1] < angular_response[0] * 0.9, "Body conditioning must reach contact constraints: {angular_response:?}");
    }

    #[test]
    fn source_body_inertia_conditioning_changes_solver_without_changing_mass() {
        use avian3d::prelude::{ComputedAngularInertia, ComputedMass, Gravity, PhysicsPlugins};
        use avian3d::dynamics::solver::solver_body::SolverBodyInertia;
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::definition::Body;
        for enabled in [false, true] {
            let mut document: serde_json::Value = serde_json::from_str(include_str!("../../assets/ALS/Character/PA_Als.physics.json")).unwrap();
            for body in document["bodies"].as_array_mut().unwrap() { body["target"]["inertia_conditioning_enabled"] = serde_json::json!(enabled); }
            let mut profile = RagdollPhysicsProfile::from_json(&document.to_string()).unwrap();
            profile.bodies.retain(|body| body.label == "pelvis");profile.colliders.clear();profile.joints.clear();
            profile.bodies[0].sleeping_disabled = true;
            profile.bodies[0].mass_properties = Some(RagdollMassProperties { mass_kg: 1.0, unclamped_mass_kg: 1.0,
                scale_exponent: 0.0, update_with_scale: false, center_of_mass_m: Vec3::ZERO,
                inertia_tensor_kg_m2: Mat3::IDENTITY / 600.0 });
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
                PhysicsPlugins::new(PostUpdate), crate::extensions::SourcePhysicsJointPlugin, RagdollPhysicsProfilePlugin));
            app.insert_resource(Gravity(Vec3::ZERO));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
            app.finish();app.cleanup();
            let player = app.world_mut().spawn((AnimationGraphPlayer::new(Handle::default()), profile, Transform::IDENTITY)).id();
            let root = app.world_mut().spawn(Transform::IDENTITY).id();
            let body = app.world_mut().spawn((RigidBody::Dynamic, BodyLabel("pelvis".into()), Transform::IDENTITY, ChildOf(root))).id();
            let collider = app.world_mut().spawn((Collider::cuboid(0.1, 0.1, 0.1), Transform::IDENTITY, ChildOf(body))).id();
            let mut spawned = SpawnedRagdoll::new(root);spawned.bodies.insert(Body::new().id, body);
            spawned.colliders.insert(bevy_animation_graph::core::ragdoll::definition::Collider::new().id, collider);
            let mut graph_player = app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap();
            let ReflectMut::Struct(fields) = graph_player.reflect_mut() else { panic!("player must be a struct") };
            *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
            app.update();app.update();
            assert_eq!(app.world().get::<ComputedMass>(body).unwrap().value(), 1.0);
            assert!((app.world().get::<ComputedAngularInertia>(body).unwrap().inverse() * Vec3::X).abs_diff_eq(Vec3::X * 600.0, 1e-3));
            let inverse = app.world().get::<SolverBodyInertia>(body).unwrap().effective_inv_angular_inertia() * Vec3::X;
            let expected = if enabled { 250.0 } else { 600.0 };
            assert!(inverse.abs_diff_eq(Vec3::X * expected, 1e-3), "Body conditioning must only change solver inertia: enabled={enabled}, inverse={inverse:?}");
            if !enabled { continue; }
            let assert_inverse = |app: &App, expected: Vec3| {
                let actual = app.world().get::<SolverBodyInertia>(body).unwrap().effective_inv_angular_inertia();
                for index in 0..3 {
                    assert!((actual * Vec3::AXES[index]).abs_diff_eq(Vec3::AXES[index] * expected[index], 1e-3), "conditioned inertia expected={expected:?}, axis={index}: {:?}", actual * Vec3::AXES[index]);
                }
                assert_eq!(app.world().get::<ComputedMass>(body).unwrap().value(), 1.0);
                assert!((app.world().get::<ComputedAngularInertia>(body).unwrap().inverse() * Vec3::X).abs_diff_eq(Vec3::X * 600.0, 1e-3));
            };
            app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().bodies[0].inertia_conditioning_enabled = false;
            app.update();assert_inverse(&app, Vec3::splat(600.0));
            app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().bodies[0].inertia_conditioning_enabled = true;
            app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().inertia_conditioning_settings.enabled = false;
            app.update();assert_inverse(&app, Vec3::splat(600.0));
            app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().inertia_conditioning_settings.enabled = true;
            app.update();assert_inverse(&app, Vec3::splat(250.0));
            app.world_mut().get_mut::<Transform>(collider).unwrap().translation = Vec3::X * 0.3;
            app.update();assert_inverse(&app, Vec3::splat(250.0));
            app.world_mut().get_mut::<Transform>(player).unwrap().scale = Vec3::splat(2.0);
            app.update();assert_inverse(&app, Vec3::splat(125.0));
            app.world_mut().get_mut::<Transform>(player).unwrap().scale = Vec3::ONE;
            app.update();assert_inverse(&app, Vec3::splat(250.0));
            let extra = app.world_mut().spawn((Collider::cuboid(0.1, 0.1, 0.1), Transform::from_xyz(-0.3, 0.0, 0.0), ChildOf(body))).id();
            app.update();assert_inverse(&app, Vec3::new(600.0, 2.5 / (0.35 * 0.35), 2.5 / (0.35 * 0.35)));
            app.world_mut().entity_mut(extra).despawn();
            app.update();assert_inverse(&app, Vec3::splat(250.0));
            let anchor = app.world_mut().spawn((RigidBody::Static, Transform::from_xyz(0.3, 0.0, 0.0))).id();
            let mut joint = crate::pa_als::parse_pa_als_profile().joints.iter().find(|joint| joint.definition.linear_locked).unwrap().definition.clone();
            joint.parent = anchor;joint.child = body;
            joint.parent_frame = Isometry3d::IDENTITY;joint.child_frame = Isometry3d::from_translation(Vec3::X * 0.3);
            joint.angular_limits = [SourceAngularLimit::Free; 3];joint.mass_conditioning = false;
            let joint_entity = app.world_mut().spawn(joint).id();
            app.update();assert_inverse(&app, Vec3::new(600.0, 2.5 / 0.09, 2.5 / 0.09));
            app.world_mut().get_mut::<SourcePhysicsJoint>(joint_entity).unwrap().mass_conditioning = true;
            app.update();assert_inverse(&app, Vec3::new(600.0, 2.5 / 0.09, 2.5 / 0.09));
            app.world_mut().get_mut::<SourcePhysicsJoint>(joint_entity).unwrap().linear_locked = false;
            app.update();assert_inverse(&app, Vec3::splat(250.0));
            app.world_mut().get_mut::<SourcePhysicsJoint>(joint_entity).unwrap().linear_motors[0].velocity_enabled = true;
            app.update();assert_inverse(&app, Vec3::splat(250.0));
            app.world_mut().get_mut::<SourcePhysicsJoint>(joint_entity).unwrap().linear_motors[0].position_enabled = true;
            app.update();assert_inverse(&app, Vec3::new(600.0, 2.5 / 0.09, 2.5 / 0.09));
            app.world_mut().entity_mut(joint_entity).insert(JointDisabled);
            app.update();assert_inverse(&app, Vec3::splat(250.0));
            app.world_mut().entity_mut(joint_entity).remove::<JointDisabled>();
            app.update();assert_inverse(&app, Vec3::new(600.0, 2.5 / 0.09, 2.5 / 0.09));
            app.world_mut().entity_mut(joint_entity).despawn();
            app.update();assert_inverse(&app, Vec3::splat(250.0));
            app.world_mut().entity_mut(body).insert(RigidBody::Kinematic);
            app.update();
            assert_inverse(&app, Vec3::splat(600.0));
            app.world_mut().entity_mut(body).insert(RigidBody::Dynamic);
            app.update();assert_inverse(&app, Vec3::splat(250.0));
            let mass_rotation = Quat::from_rotation_z(0.4);
            let mass_basis = Mat3::from_quat(mass_rotation);
            let physical = mass_basis * Mat3::from_diagonal(Vec3::new(1.0 / 600.0, 1.0 / 300.0, 1.0 / 200.0)) * mass_basis.transpose();
            let body_rotation = Quat::from_euler(EulerRot::XYZ, 0.7, -0.5, 0.2);
            let shape_rotation = Quat::from_rotation_z(0.3);
            {
                let mut profile = app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap();
                let mass = profile.bodies[0].mass_properties.as_mut().unwrap();
                mass.inertia_tensor_kg_m2 = physical;mass.center_of_mass_m = Vec3::new(0.1, -0.2, 0.3);
            }
            app.world_mut().get_mut::<Transform>(body).unwrap().rotation = body_rotation;
            app.world_mut().entity_mut(collider).insert(Collider::cuboid(0.1, 0.2, 0.4));
            app.world_mut().get_mut::<Transform>(collider).unwrap().rotation = shape_rotation;
            app.update();
            let shape_basis = Mat3::from_quat(shape_rotation);
            let half = shape_basis.x_axis.abs() * 0.05 + shape_basis.y_axis.abs() * 0.1 + shape_basis.z_axis.abs() * 0.2;
            let inverse_basis = mass_basis.transpose();
            let extents = inverse_basis.x_axis.abs() * half.x + inverse_basis.y_axis.abs() * half.y + inverse_basis.z_axis.abs() * half.z;
            let diagonal = Vec3::new(2.5 / extents.y.powi(2).max(extents.z.powi(2)),
                2.5 / extents.x.powi(2).max(extents.z.powi(2)), 2.5 / extents.x.powi(2).max(extents.y.powi(2)));
            let world_basis = Mat3::from_quat(body_rotation * mass_rotation);
            let expected = world_basis * Mat3::from_diagonal(diagonal) * world_basis.transpose();
            let actual = app.world().get::<SolverBodyInertia>(body).unwrap().effective_inv_angular_inertia();
            for axis in Vec3::AXES { assert!((actual * axis).abs_diff_eq(expected * axis, 1e-3), "actor AABB then principal frame must match the source: {:?} != {:?}", actual * axis, expected * axis); }
            assert!((app.world().get::<ComputedAngularInertia>(body).unwrap().value() * Vec3::X).abs_diff_eq(physical * Vec3::X, 1e-6));
            app.world_mut().entity_mut(body).insert(avian3d::prelude::AngularVelocity(Vec3::new(0.2, 0.4, 0.6)));
            app.update();
            let current_basis = Mat3::from_quat(app.world().get::<Rotation>(body).unwrap().0 * mass_rotation);
            let expected = current_basis * Mat3::from_diagonal(diagonal) * current_basis.transpose();
            let actual = app.world().get::<SolverBodyInertia>(body).unwrap().effective_inv_angular_inertia();
            for axis in Vec3::AXES { assert!((actual * axis).abs_diff_eq(expected * axis, 1e-3), "solver inertia must use the integrated pose: {:?} != {:?}", actual * axis, expected * axis); }
            app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().bodies[0].mass_properties = Some(RagdollMassProperties {
                mass_kg: 1.0, unclamped_mass_kg: 1.0, scale_exponent: 0.0, update_with_scale: false,
                center_of_mass_m: Vec3::ZERO, inertia_tensor_kg_m2: Mat3::IDENTITY / 600.0 });
            let fresh_root = app.world_mut().spawn(Transform::IDENTITY).id();
            let fresh_body = app.world_mut().spawn((RigidBody::Dynamic, BodyLabel("pelvis".into()), Transform::from_xyz(4.0, 5.0, 6.0), ChildOf(fresh_root))).id();
            app.world_mut().spawn((Collider::cuboid(0.1, 0.1, 0.1), Transform::IDENTITY, ChildOf(fresh_body)));
            let mut fresh = SpawnedRagdoll::new(fresh_root);fresh.bodies.insert(Body::new().id, fresh_body);
            let mut graph_player = app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap();
            let ReflectMut::Struct(fields) = graph_player.reflect_mut() else { panic!("player must be a struct") };
            *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(fresh);
            app.update();
            assert!(app.world().get_entity(root).is_err() && app.world().get_entity(body).is_err());
            assert!(app.world().entity(fresh_body).contains::<SourceInertiaConditioningSettings>());
            assert!((app.world().get::<SolverBodyInertia>(fresh_body).unwrap().effective_inv_angular_inertia() * Vec3::X).abs_diff_eq(Vec3::X * 250.0, 1e-3));
        }
    }

    #[test]
    fn source_six_dof_translation_lock_blocks_world_axis_motion() {
        use avian3d::prelude::{AngularVelocity, Gravity, LinearVelocity, LockedAxes, PhysicsPlugins};
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::definition::Body;
        let mut document: serde_json::Value = serde_json::from_str(include_str!("../../assets/ALS/Character/PA_Als.physics.json")).unwrap();
        for body in document["bodies"].as_array_mut().unwrap() { body["target"]["locked_axes_bits"] = serde_json::json!(LockedAxes::new().lock_translation_x().to_bits()); }
        let mut profile = RagdollPhysicsProfile::from_json(&document.to_string()).unwrap();
        profile.bodies.retain(|body| body.label == "pelvis");profile.colliders.clear();profile.joints.clear();
        profile.bodies[0].mass_properties.as_mut().unwrap().center_of_mass_m = Vec3::new(0.0, 0.1, 0.0);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
            PhysicsPlugins::new(PostUpdate), RagdollPhysicsProfilePlugin));
        app.insert_resource(Gravity(Vec3::ZERO));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();app.cleanup();
        let player = app.world_mut().spawn((AnimationGraphPlayer::new(Handle::default()), profile, Transform::IDENTITY)).id();
        let root = app.world_mut().spawn((Transform::IDENTITY, ChildOf(player))).id();
        let body = app.world_mut().spawn((RigidBody::Dynamic, BodyLabel("pelvis".into()),
            Transform::from_rotation(Quat::from_rotation_z(0.7)), ChildOf(root))).id();
        app.world_mut().spawn((Collider::sphere(0.1), Transform::IDENTITY, ChildOf(body)));
        let mut spawned = SpawnedRagdoll::new(root);spawned.bodies.insert(Body::new().id, body);
        let mut graph_player = app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap();
        let ReflectMut::Struct(fields) = graph_player.reflect_mut() else { panic!("player must be a struct") };
        *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
        app.update();
        app.world_mut().entity_mut(body).insert((LinearVelocity(Vec3::new(1.0, 2.0, 3.0)), AngularVelocity(Vec3::new(0.0, 0.0, 2.0))));
        app.update();
        let position = app.world().get::<Position>(body).unwrap().0;
        assert!(position.x.abs() < 1e-6 && position.y > 0.01 && position.z > 0.02,
            "SixDOF world X must lock while Y/Z remain free: {position:?}");
    }

    #[test]
    fn source_six_dof_all_masks_preserve_world_anchors_and_rebuild() {
        use avian3d::prelude::{AngularVelocity, ConstantForce, ConstantTorque, Gravity, LinearVelocity, PhysicsPlugins};
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::definition::Body;
        for initial_rotation in [Quat::IDENTITY, Quat::from_euler(EulerRot::XYZ, 0.4, -0.7, 0.6)] {
            let mut profile = RagdollPhysicsProfile::from_json(include_str!("../../assets/ALS/Character/PA_Als.physics.json")).unwrap();
            profile.bodies.retain(|body| body.label == "pelvis");
            profile.colliders.clear();profile.joints.clear();
            profile.bodies[0].linear_damping = 0.0;profile.bodies[0].angular_damping = 0.0;
            profile.bodies[0].mass_properties.as_mut().unwrap().center_of_mass_m = Vec3::ZERO;
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
                PhysicsPlugins::new(PostUpdate), RagdollPhysicsProfilePlugin));
            app.insert_resource(Gravity(Vec3::ZERO));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
            app.finish();app.cleanup();
            let mut fixtures = Vec::new();
            for bits in 0..64u8 {
                let mut profile = profile.clone();profile.bodies[0].locked_axes_bits = bits;
                let player = app.world_mut().spawn((AnimationGraphPlayer::new(Handle::default()), profile, Transform::IDENTITY)).id();
                let root = app.world_mut().spawn((Transform::IDENTITY, ChildOf(player))).id();
                let origin = Vec3::new(bits as f32 * 4.0, 2.0, -3.0);
                let body = app.world_mut().spawn((RigidBody::Dynamic, BodyLabel("pelvis".into()),
                    Transform::from_translation(origin).with_rotation(initial_rotation), ChildOf(root))).id();
                app.world_mut().spawn((Collider::sphere(0.1), Transform::IDENTITY, ChildOf(body)));
                let mut spawned = SpawnedRagdoll::new(root);spawned.bodies.insert(Body::new().id, body);
                let mut graph_player = app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap();
                let ReflectMut::Struct(fields) = graph_player.reflect_mut() else { panic!("player must be a struct") };
                *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
                fixtures.push((bits, player, root, body, origin));
            }
            app.update();
            for &(_, _, _, body, _) in &fixtures {
                app.world_mut().entity_mut(body).insert((LinearVelocity(Vec3::new(1.0, 2.0, 3.0)),
                    AngularVelocity(Vec3::new(1.0, 2.0, 3.0)), ConstantForce(Vec3::new(3.0, 2.0, 1.0)),
                    ConstantTorque(Vec3::new(0.0001, 0.0002, 0.0003))));
            }
            for frame in 0..4 {
                app.update();
                for &(bits, _, _, body, origin) in &fixtures {
                    let displacement = app.world().get::<Position>(body).unwrap().0 - origin;
                    let relative = app.world().get::<Rotation>(body).unwrap().0 * initial_rotation.conjugate();
                    let vector = Vec3::new(relative.x, relative.y, relative.z);
                    for (index, mask) in [32, 16, 8].into_iter().enumerate() {
                        if bits & mask != 0 {
                            assert!(displacement[index].abs() < 1e-5, "locked translation bits={bits} axis={index} frame={frame}: {displacement:?}");
                            assert!(app.world().get::<LinearVelocity>(body).unwrap().0[index].abs() < 1e-5, "locked translation velocity bits={bits} axis={index}");
                        }
                        else { assert!(displacement[index] > 0.01, "free translation bits={bits} axis={index}: {displacement:?}"); }
                    }
                    for (index, mask) in [4, 2, 1].into_iter().enumerate() {
                        if bits & mask != 0 { assert!(vector[index].abs() < 1e-5, "locked rotation bits={bits} axis={index} frame={frame}: {relative:?}"); }
                    }
                    if bits & 7 == 0 { assert!(vector.length() > 0.01, "free rotation must advance"); }
                }
            }
            for &(bits, player, _, body, _) in &fixtures {
                if bits != 63 { continue; }
                let original = app.world().get::<Position>(body).unwrap().0;
                let displaced = original + Vec3::ONE;
                app.world_mut().entity_mut(body).insert((Position(displaced), Transform::from_translation(displaced).with_rotation(initial_rotation)));
                app.update();
                assert!(app.world().get::<Position>(body).unwrap().0.abs_diff_eq(original, 1e-5), "world anchor must persist across frames");
                app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().bodies[0].locked_axes_bits = 0;
                app.update();
                assert!(!app.world().entity(body).contains::<SourceBodyDof>());
                app.world_mut().entity_mut(body).insert((LinearVelocity(Vec3::ONE), AngularVelocity(Vec3::ONE)));
                let before = app.world().get::<Position>(body).unwrap().0;
                app.update();
                assert!((app.world().get::<Position>(body).unwrap().0 - before).min_element() > 0.01);
                app.world_mut().get_mut::<RagdollPhysicsProfile>(player).unwrap().bodies[0].locked_axes_bits = 63;
                app.update();
                let anchor = app.world().get::<SourceBodyDof>(body).unwrap().anchor.unwrap();
                app.world_mut().entity_mut(body).insert((RigidBody::Kinematic, LinearVelocity(Vec3::ONE), AngularVelocity(Vec3::ONE)));
                let before_kinematic = app.world().get::<Position>(body).unwrap().0;
                app.update();
                assert!(app.world().get::<SourceBodyDof>(body).unwrap().anchor.is_none());
                assert!((app.world().get::<Position>(body).unwrap().0 - before_kinematic).min_element() > 0.01, "kinematic controls must stay unconstrained");
                let shifted = Vec3::from(anchor.translation) + Vec3::new(1.0, 2.0, 3.0);
                app.world_mut().entity_mut(body).insert((Position(shifted), Transform::from_translation(shifted), LinearVelocity(Vec3::ZERO), AngularVelocity(Vec3::ZERO), RigidBody::Dynamic));
                app.update();
                let rebuilt = app.world().get::<SourceBodyDof>(body).unwrap().anchor.unwrap();
                assert!(rebuilt.translation.abs_diff_eq(shifted.into(), 1e-5));
                app.world_mut().entity_mut(body).insert((LinearVelocity(Vec3::ONE), AngularVelocity(Vec3::ONE)));
                app.update();
                assert!(app.world().get::<Position>(body).unwrap().0.abs_diff_eq(shifted, 1e-5));
                let root = app.world_mut().spawn((Transform::IDENTITY, ChildOf(player))).id();
                let fresh_body = app.world_mut().spawn((RigidBody::Dynamic, BodyLabel("pelvis".into()),
                    Transform::from_translation(shifted + Vec3::ONE), ChildOf(root))).id();
                app.world_mut().spawn((Collider::sphere(0.1), Transform::IDENTITY, ChildOf(fresh_body)));
                let mut spawned = SpawnedRagdoll::new(root);spawned.bodies.insert(Body::new().id, fresh_body);
                let mut graph_player = app.world_mut().get_mut::<AnimationGraphPlayer>(player).unwrap();
                let ReflectMut::Struct(fields) = graph_player.reflect_mut() else { panic!("player must be a struct") };
                *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
                app.update();
                assert!(app.world().get_entity(body).is_err(), "old root and DOF state must be removed");
                let anchor = app.world().get::<SourceBodyDof>(fresh_body).unwrap().anchor.unwrap();
                assert!(anchor.translation.abs_diff_eq((shifted + Vec3::ONE).into(), 1e-5));
                app.world_mut().entity_mut(fresh_body).insert((LinearVelocity(Vec3::ONE), AngularVelocity(Vec3::ONE)));
                app.update();
                assert!(app.world().get::<Position>(fresh_body).unwrap().0.abs_diff_eq(shifted + Vec3::ONE, 1e-5));
            }
        }
    }

    #[test]
    fn initial_nonuniform_scale_reaches_source_shapes_mass_and_joint_frames() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::{definition::Ragdoll, spawning::spawn_ragdoll_avian};

        for scale in [Vec3::ONE, Vec3::splat(2.0), Vec3::new(2.0, 3.0, 4.0), Vec3::new(4.0, 0.5, 2.0), Vec3::new(2.0, 2.00005, 2.0)] {
            let profile = crate::pa_als::parse_pa_als_profile();
            let ragdoll: Ragdoll = ron::from_str(include_str!("../../assets/ALS/Character/PA_Als.rag.ron")).unwrap();
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate), crate::extensions::SourcePhysicsJointPlugin));
            app.finish();app.cleanup();
            let world = app.world_mut();
            let parent = world.spawn((Transform::from_scale(scale).with_rotation(Quat::from_rotation_y(0.7)), GlobalTransform::IDENTITY)).id();
            let player = world.spawn((AnimationGraphPlayer::new(Handle::default()), Transform::IDENTITY,
                GlobalTransform::IDENTITY, ChildOf(parent), profile.clone())).id();
            let spawned = world.run_system_once(move |mut commands: Commands| {
                spawn_ragdoll_avian(player, &ragdoll, Isometry3d::IDENTITY, None, &mut commands)
            }).unwrap();
            for entity in spawned.colliders.values() {
                world.get_mut::<Collider>(*entity).unwrap().set_scale(Vec3::ONE, 16);
            }
            let shapes = spawned.colliders.values().map(|entity| (*entity,
                world.get::<ColliderLabel>(*entity).unwrap().0.clone(),
                world.get::<Collider>(*entity).unwrap().clone(), *world.get::<Transform>(*entity).unwrap())).collect::<Vec<_>>();
            let pelvis = *spawned.bodies.values().find(|entity| world.get::<BodyLabel>(**entity).unwrap().0 == "pelvis").unwrap();
            let pelvis_offset = world.get::<Transform>(pelvis).unwrap().translation;
            let joints = spawned.joints.values().map(|entity| (*entity, world.get::<JointLabel>(*entity).unwrap().0.clone())).collect::<Vec<_>>();
            let mut replacement = AnimationGraphPlayer::new(Handle::default());
            let ReflectMut::Struct(fields) = replacement.reflect_mut() else { panic!("Expected a reflected animation player") };
            *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(spawned);
            world.entity_mut(player).insert(replacement);
            world.run_system_once(apply_ragdoll_physics_profiles).unwrap();
            for (entity, label, original, local) in &shapes {
                let current = world.get::<Collider>(*entity).unwrap().shape_scaled();
                let source = original.shape();
                if let Some(sphere) = source.as_ball() {
                    let actual = current.as_ball().expect("A source sphere must remain spherical").radius;
                    assert!((actual - sphere.radius * scale.min_element()).abs() < 1e-5, "Source sphere size: {label} scale={scale:?}");
                } else if let Some(cuboid) = source.as_cuboid() {
                    let actual = current.as_cuboid().expect("A source box must remain a box").half_extents;
                    assert!(actual.abs_diff_eq(cuboid.half_extents * scale, 1e-5), "Source box dimensions: {label} scale={scale:?}");
                } else if let Some(capsule) = source.as_capsule() {
                    let half_height = ((capsule.segment.a.distance(capsule.segment.b) * 0.5 + capsule.radius) * scale.y).max(0.001);
                    let radius = (capsule.radius * scale.x.max(scale.z)).clamp(0.001, half_height);
                    let half_length = (half_height - radius).max(0.0005);
                    let actual = current.as_capsule().expect("A source capsule must remain a capsule");
                    assert!((actual.radius - radius).abs() < 1e-5, "Source capsule radius: {label} scale={scale:?}");
                    assert!((actual.segment.a.distance(actual.segment.b) * 0.5 - half_length).abs() < 1e-5,
                        "Source capsule length: {label} scale={scale:?}");
                } else { panic!("Unexpected source primitive: {label}") }
                let actual = world.get::<Transform>(*entity).unwrap();
                assert!(actual.translation.abs_diff_eq(local.translation * scale, 1e-5), "Source shape center: {label}");
                assert!(actual.rotation.abs_diff_eq(local.rotation, 1e-5), "Source shape orientation: {label}");
            }
            assert!(world.get::<Transform>(pelvis).unwrap().translation.abs_diff_eq(pelvis_offset * scale, 1e-5), "Source bone offset");
            for (entity, label) in &joints {
                let source = &profile.joints.iter().find(|joint| joint.label == *label).unwrap().definition;
                let current = world.get::<SourcePhysicsJoint>(*entity).unwrap();
                assert!(current.child_frame.translation.abs_diff_eq(source.child_frame.translation * bevy::math::Vec3A::from(scale), 1e-5), "Source child joint frame: {label}");
                assert!(current.parent_frame.translation.abs_diff_eq(source.parent_frame.translation * bevy::math::Vec3A::from(scale), 1e-5), "Source parent joint frame: {label}");
                assert_eq!(current.child_frame.rotation, source.child_frame.rotation);
                assert_eq!(current.parent_frame.rotation, source.parent_frame.rotation);
            }
            let (_, _, original, _) = shapes.iter().find(|(_, label, _, _)| label == "pelvis_0_capsule").unwrap();
            let original_volume = original.shape().mass_properties(1.0).mass();
            let pelvis_shape = shapes.iter().find(|(_, label, _, _)| label == "pelvis_0_capsule").unwrap().0;
            let current = world.get::<Collider>(pelvis_shape).unwrap().shape_scaled().mass_properties(1.0);
            let source_mass = profile.bodies.iter().find(|body| body.label == "pelvis").unwrap().mass_properties.unwrap();
            let expected = (source_mass.unclamped_mass_kg * (current.mass() / original_volume).powf(source_mass.scale_exponent / 3.0)).max(0.001);
            assert!((world.get::<Mass>(pelvis).unwrap().0 - expected).abs() < expected * 1e-5, "Source scaled volume mass: scale={scale:?}");
            if scale == Vec3::new(2.0, 3.0, 4.0) {
                for (label, expected_mass, center, tensor) in [
                    ("pelvis", 122.2897, Vec3::new(0.01, 0.0, -0.02),
                        [13.499299, 0.0, 0.0, 0.0, 13.487247, 0.0, 0.0, 0.0, 13.499299]),
                    ("spine_03", 175.62312, Vec3::new(0.021087594, 0.0, -0.08500984),
                        [21.782988, 0.0, 0.8297611, 0.0, 22.21885, 0.0, 0.8297611, 0.0, 22.641094]),
                    ("hand_l", 12.679918, Vec3::new(0.1, 0.0, -0.04),
                        [0.19862649, 0.0005872733, 0.0043457096, 0.0005872733, 0.19109628, -0.00010355213,
                            0.0043457096, -0.00010355213, 0.22250597]),
                ] {
                    let body = *world.get::<AnimationGraphPlayer>(player).unwrap().get_spawned_ragdoll().unwrap().bodies.values()
                        .find(|entity| world.get::<BodyLabel>(**entity).unwrap().0 == label).unwrap();
                    assert!((world.get::<Mass>(body).unwrap().0 - expected_mass).abs() < expected_mass * 1e-5,
                        "Offline source mass oracle: {label}");
                    assert!(world.get::<CenterOfMass>(body).unwrap().0.abs_diff_eq(center, 1e-5), "Offline source CoM oracle: {label}");
                    let inertia = world.get::<AngularInertia>(body).unwrap();
                    let basis = Mat3::from_quat(inertia.local_frame);
                    let actual = basis * Mat3::from_diagonal(inertia.principal) * basis.transpose();
                    assert!(actual.abs_diff_eq(Mat3::from_cols_array(&tensor), expected_mass * 1e-5), "Offline source inertia oracle: {label}: {actual:?}");
                }
                let mass = world.get::<Mass>(pelvis).unwrap().0;
                {
                    let mut profile = world.get_mut::<RagdollPhysicsProfile>(player).unwrap();
                    let body = profile.bodies.iter_mut().find(|body| body.label == "pelvis").unwrap();
                    body.center_of_mass_nudge_m = Vec3::new(0.1, -0.2, 0.3);
                    body.inertia_scale = Vec3::new(2.0, 0.5, 1.0);
                }
                world.run_system_once(apply_ragdoll_physics_profiles).unwrap();
                assert!(world.get::<CenterOfMass>(pelvis).unwrap().0.abs_diff_eq(Vec3::new(0.21, -0.6, 1.18), 1e-5),
                    "Nonuniform source CoM modifier did not reach the body");
                let original_diagonal = Vec3::new(13.499299, 13.487247, 13.499299);
                let moments = (Vec3::splat(original_diagonal.element_sum() * 0.5) - original_diagonal) * Vec3::new(4.0, 0.25, 1.0);
                let expected_diagonal = Vec3::new(moments.y + moments.z, moments.x + moments.z, moments.x + moments.y)
                    + mass * Vec3::new(0.04, 0.36, 1.44);
                let inertia = world.get::<AngularInertia>(pelvis).unwrap();
                let basis = Mat3::from_quat(inertia.local_frame);
                let actual = basis * Mat3::from_diagonal(inertia.principal) * basis.transpose();
                assert!(actual.abs_diff_eq(Mat3::from_diagonal(expected_diagonal), expected_diagonal.max_element() * 1e-5),
                    "Nonuniform source inertia modifiers did not reach the body");
            }
            let before = world.get::<Transform>(pelvis).unwrap().translation;
            let before_mass = world.get::<Mass>(pelvis).unwrap().0;
            world.get_mut::<Transform>(parent).unwrap().scale = Vec3::new(3.0, 5.0, 7.0);
            world.run_system_once(apply_ragdoll_physics_profiles).unwrap();
            assert_eq!(world.get::<Transform>(pelvis).unwrap().translation, before, "A runtime nonuniform change must not rescale existing source bodies");
            assert_eq!(world.get::<Mass>(pelvis).unwrap().0, before_mass);
            if scale == Vec3::new(2.0, 3.0, 4.0) {
                let old_root = world.get::<RagdollPhysicsProfileApplied>(player).unwrap().spawned_root.unwrap();
                let ragdoll: Ragdoll = ron::from_str(include_str!("../../assets/ALS/Character/PA_Als.rag.ron")).unwrap();
                let fresh = world.run_system_once(move |mut commands: Commands| {
                    spawn_ragdoll_avian(player, &ragdoll, Isometry3d::IDENTITY, None, &mut commands)
                }).unwrap();
                let root = *fresh.colliders.values().find(|entity| world.get::<ColliderLabel>(**entity).unwrap().0 == "root_0_sphere").unwrap();
                let mut graph_player = world.get_mut::<AnimationGraphPlayer>(player).unwrap();
                let ReflectMut::Struct(fields) = graph_player.reflect_mut() else { panic!("Expected a reflected animation player") };
                *fields.field_mut("spawned_ragdoll").unwrap().try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(fresh);
                world.run_system_once(apply_ragdoll_physics_profiles).unwrap();
                assert!(world.get_entity(old_root).is_err(), "A rebuilt source body tree must replace the old root");
                assert!((world.get::<Collider>(root).unwrap().shape_scaled().as_ball().unwrap().radius - 0.3).abs() < 1e-5,
                    "A rebuilt source body tree must accept its new initial nonuniform scale");
            }
        }
    }

    #[test]
    fn positive_uniform_scale_reaches_ragdoll_shapes_offsets_and_joint_frames() {
        use avian3d::prelude::Collider;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::{
            definition::Ragdoll, spawning::spawn_ragdoll_avian,
        };

        let mut profile = RagdollPhysicsProfile::from_json(include_str!(
            "../../assets/ALS/Character/PA_Als.physics.json"
        )).unwrap();
        let position_target = Vec3::new(1.0, -0.5, 0.25);
        let velocity_target = Vec3::new(0.1, -0.2, 0.3);
        for joint in &mut profile.joints {
            joint.definition.linear_position_target = position_target;
            joint.definition.linear_velocity_target = velocity_target;
        }
        let ragdoll: Ragdoll = ron::from_str(include_str!(
            "../../assets/ALS/Character/PA_Als.rag.ron"
        )).unwrap();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate), crate::extensions::SourcePhysicsJointPlugin));
        app.finish();
        app.cleanup();
        let world = app.world_mut();
        let player = world.spawn((AnimationGraphPlayer::new(Handle::default()), Transform::IDENTITY)).id();
        let spawned = world.run_system_once(move |mut commands: Commands| {
            spawn_ragdoll_avian(player, &ragdoll, Isometry3d::IDENTITY, None, &mut commands)
        }).unwrap();
        let body = *spawned.bodies.values().find(|entity|
            world.get::<Transform>(**entity).unwrap().translation.length() > 0.01).unwrap();
        let body_label = world.get::<BodyLabel>(body).unwrap().0.clone();
        let body_offset = world.get::<Transform>(body).unwrap().translation;
        let collider = *spawned.colliders.values().find(|entity|
            world.get::<ColliderLabel>(**entity).unwrap().0.starts_with("hand_l_")).unwrap();
        let collider_label = world.get::<ColliderLabel>(collider).unwrap().0.clone();
        let collider_offset = world.get::<Transform>(collider).unwrap().translation;
        let collider_extent = world.get::<Collider>(collider).unwrap().shape_scaled().compute_local_aabb().half_extents();
        let joint_profile = profile.joints.iter().find(|joint|
            joint.definition.parent_frame.translation.length() > 0.01).unwrap();
        let joint = *spawned.joints.values().find(|entity|
            world.get::<JointLabel>(**entity).unwrap().0 == joint_profile.label).unwrap();
        let joint_label = joint_profile.label.clone();
        let joint_offset = joint_profile.definition.parent_frame.translation;
        let pelvis = *spawned.bodies.values().find(|entity| world.get::<BodyLabel>(**entity).unwrap().0 == "pelvis").unwrap();
        let old_root = spawned.root;
        let first = SpawnedRagdoll { root: spawned.root, bodies: spawned.bodies.clone(),
            colliders: spawned.colliders.clone(), joints: spawned.joints.clone() };
        world.run_system_once({ let profile = profile.clone(); move |mut commands: Commands,
            body_labels: Query<&BodyLabel>, collider_labels: Query<&ColliderLabel>,
            joint_labels: Query<&JointLabel>, spherical: Query<&SphericalJoint>,
            revolute: Query<&RevoluteJoint>, source_joints: Query<&SourcePhysicsJoint>,
            transforms: Query<&Transform>, mut colliders: Query<&mut Collider>| {
            apply_profile_to_spawned_ragdoll(&mut commands, &first, &profile, Vec3::ONE, Vec3::ONE, true, true,
                &body_labels, &collider_labels, &joint_labels, &spherical, &revolute,
                &source_joints, &transforms, &mut colliders)
        }}).unwrap();
        app.update();
        let base_mass = app.world().get::<avian3d::prelude::ComputedMass>(pelvis).unwrap().value();
        let world = app.world_mut();
        for (scale, scale_factor) in [(0.5, 0.5), (1.0, 2.0), (2.0, 2.0)] {
            let current = SpawnedRagdoll { root: spawned.root, bodies: spawned.bodies.clone(),
                colliders: spawned.colliders.clone(), joints: spawned.joints.clone() };
            world.run_system_once({ let profile = profile.clone(); move |mut commands: Commands,
                body_labels: Query<&BodyLabel>, collider_labels: Query<&ColliderLabel>,
                joint_labels: Query<&JointLabel>, spherical: Query<&SphericalJoint>,
                revolute: Query<&RevoluteJoint>, source_joints: Query<&SourcePhysicsJoint>, transforms: Query<&Transform>,
                mut colliders: Query<&mut Collider>| {
                apply_profile_to_spawned_ragdoll(&mut commands, &current, &profile, Vec3::splat(scale), Vec3::splat(scale_factor), false, false,
                    &body_labels, &collider_labels, &joint_labels, &spherical, &revolute,
                    &source_joints, &transforms, &mut colliders)
            }}).unwrap();
            let motor = world.get::<SourcePhysicsJoint>(joint).unwrap();
            assert_eq!((motor.linear_position_target, motor.linear_velocity_target),
                (position_target, velocity_target), "Geometry scale changed source linear drive targets: scale={scale}");
            assert!(motor.parent_frame.translation.abs_diff_eq(joint_offset * scale, 1e-5));
        }
        assert!(world.get::<Transform>(body).unwrap().translation.abs_diff_eq(body_offset * 2.0, 1e-5));
        assert!(world.get::<Transform>(collider).unwrap().translation.abs_diff_eq(collider_offset * 2.0, 1e-5));
        let scaled_collider = world.get::<Collider>(collider).unwrap();
        let scaled_extent = scaled_collider.shape_scaled().compute_local_aabb().half_extents();
        assert!(scaled_extent.abs_diff_eq(collider_extent * 2.0, 1e-5),
            "baseline={collider_extent:?} scaled={scaled_extent:?} collider_scale={:?}", scaled_collider.scale());
        assert!(world.get::<SourcePhysicsJoint>(joint).unwrap().parent_frame.translation
            .abs_diff_eq(joint_offset * 2.0, 1e-5));
        assert!(world.get::<JointCollisionDisabled>(joint).is_none());
        app.update();
        let expected_mass = base_mass * 2.0_f32.powf(3.0 * 0.75);
        let actual_mass = app.world().get::<avian3d::prelude::ComputedMass>(pelvis).unwrap().value();
        assert!((actual_mass - expected_mass).abs() < expected_mass * 1e-5,
            "Scaled source mass must reach the solver: {actual_mass} != {expected_mass}");
        let next_extent = app.world().get::<Collider>(collider).unwrap().shape_scaled().compute_local_aabb().half_extents();
        assert!(next_extent.abs_diff_eq(collider_extent * 2.0, 1e-5),
            "Collider scale was lost after physics: {next_extent:?}");
        let fresh_ragdoll: Ragdoll = ron::from_str(include_str!(
            "../../assets/ALS/Character/PA_Als.rag.ron")).unwrap();
        let fresh = app.world_mut().run_system_once(move |mut commands: Commands| {
            spawn_ragdoll_avian(player, &fresh_ragdoll, Isometry3d::IDENTITY, None, &mut commands)
        }).unwrap();
        let fresh_root = fresh.root;
        let world = app.world_mut();
        let fresh_body = *fresh.bodies.values().find(|entity|
            world.get::<BodyLabel>(**entity).unwrap().0 == body_label).unwrap();
        let fresh_collider = *fresh.colliders.values().find(|entity|
            world.get::<ColliderLabel>(**entity).unwrap().0 == collider_label).unwrap();
        let fresh_joint = *fresh.joints.values().find(|entity|
            world.get::<JointLabel>(**entity).unwrap().0 == joint_label).unwrap();
        let fresh_profile = RagdollPhysicsProfile::from_json(include_str!(
            "../../assets/ALS/Character/PA_Als.physics.json")).unwrap();
        let mut replacement = AnimationGraphPlayer::new(Handle::default());
        let ReflectMut::Struct(fields) = replacement.reflect_mut() else { panic!("player must reflect as a struct") };
        *fields.field_mut("spawned_ragdoll").unwrap()
            .try_downcast_mut::<Option<SpawnedRagdoll>>().unwrap() = Some(fresh);
        world.entity_mut(player).insert((Transform::from_scale(Vec3::splat(2.0)), fresh_profile,
            RagdollPhysicsProfileApplied {
                spawned_root: Some(old_root), scale: Vec3::splat(2.0), body_count: 21,
                max_joint_anchor_error_m: 42.0, ..default()
            }));
        world.entity_mut(player).insert(replacement);
        world.run_system_once(apply_ragdoll_physics_profiles).unwrap();
        let fresh_applied = world.get::<RagdollPhysicsProfileApplied>(player).unwrap();
        assert_eq!(fresh_applied.spawned_root, Some(fresh_root));
        assert_eq!((fresh_applied.body_count, fresh_applied.collider_count,
            fresh_applied.joint_count, fresh_applied.source_joint_count), (21, 22, 20, 19));
        assert_eq!(fresh_applied.max_joint_anchor_error_m, 0.0);
        assert!(world.get::<Transform>(fresh_body).unwrap().translation.abs_diff_eq(body_offset * 2.0, 1e-5));
        assert!(world.get::<Collider>(fresh_collider).unwrap().shape_scaled().compute_local_aabb().half_extents()
            .abs_diff_eq(collider_extent * 2.0, 1e-5));
        assert!(world.get::<SourcePhysicsJoint>(fresh_joint).unwrap().parent_frame.translation
            .abs_diff_eq(joint_offset * 2.0, 1e-5));
        assert!(world.get::<JointCollisionDisabled>(fresh_joint).is_none());
        assert!(world.get::<RagdollGyroscopicTorqueDisabled>(fresh_body).is_some());
        world.get_mut::<RagdollPhysicsProfile>(player).unwrap().bodies.iter_mut()
            .find(|body| body.label == body_label).unwrap().gyroscopic_torque_enabled = true;
        world.run_system_once(apply_ragdoll_physics_profiles).unwrap();
        assert!(world.get::<RagdollGyroscopicTorqueDisabled>(fresh_body).is_none());
        world.get_mut::<RagdollPhysicsProfile>(player).unwrap().bodies.iter_mut()
            .find(|body| body.label == body_label).unwrap().gyroscopic_torque_enabled = false;
        world.run_system_once(apply_ragdoll_physics_profiles).unwrap();
        assert!(world.get::<RagdollGyroscopicTorqueDisabled>(fresh_body).is_some());
        assert!(world.get_entity(old_root).is_err());
        assert!(world.get_entity(body).is_err());
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

    #[test]
    fn capsule_scene_sweep_matches_native_large_floor() {
        use avian3d::prelude::PhysicsPlugins;
        use bevy::{ecs::system::RunSystemOnce, time::TimeUpdateStrategy};
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            TransformPlugin, PhysicsPlugins::new(PostUpdate)));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.5, 20.0),
            Position(Vec3::new(0.0, -0.25, -20.0)), Rotation::IDENTITY,
            CollisionLayers::new([crate::als::character::AlsCollisionLayer::Default,
                crate::als::character::AlsCollisionLayer::Visibility], avian3d::prelude::LayerMask::ALL))).id();
        app.update();
        app.update();
        let (hit, prediction, prediction_distance) = app.world_mut().run_system_once(move |spatial: RagdollSpatialQuery| {
            let capsule = Collider::capsule(0.3, 1.2);
            let origin = Vec3::new(-7.9832387, 1.0044739, -15.300977);
            let direction = Dir3::new(Vec3::new(0.76588976, -0.64297193, -0.00017596605)).unwrap();
            let hit = spatial.cast_shape(&capsule, origin, Quat::IDENTITY, direction,
                &ShapeCastConfig::from_max_distance(0.3182573), &SpatialQueryFilter::default());
            let velocity = *direction * 4.0;
            let prediction = crate::als::air::ground_prediction_sweep(&spatial, Entity::PLACEHOLDER,
                origin, velocity, 0.9, 0.3);
            let prediction_distance = 1.5 + (velocity.y + 2.0) / -38.0 * 18.5;
            (hit, prediction, prediction_distance)
        }).unwrap();
        let hit = hit.unwrap();
        assert_eq!(hit.entity, floor);
        assert!((hit.distance - 0.162486).abs() <= 1e-6,
            "Native large floor distance differs: {}", hit.distance);
        assert!(hit.normal1.abs_diff_eq(Vec3::Y, 1e-6));
        assert!(prediction.ground_valid);
        assert!((prediction.hit_fraction.unwrap() * prediction_distance - 0.162486).abs() <= 1e-6);
    }

    #[test]
    fn capsule_scene_sweep_preserves_filters_and_nearest_hit() {
        use avian3d::prelude::PhysicsPlugins;
        use bevy::{ecs::system::RunSystemOnce, time::TimeUpdateStrategy};
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            TransformPlugin, PhysicsPlugins::new(PostUpdate)));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        let hidden = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.2, 4.0, 4.0),
            Position(Vec3::X), RagdollShapeCollisionMode::PhysicsOnly)).id();
        let sphere = app.world_mut().spawn((RigidBody::Static, Collider::sphere(0.2),
            Position(Vec3::X * 2.0))).id();
        let wall = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.2, 4.0, 4.0),
            Position(Vec3::X * 2.5))).id();
        app.update();
        app.update();
        let (first, filtered, only_hidden) = app.world_mut().run_system_once(move |spatial: RagdollSpatialQuery| {
            let capsule = Collider::capsule(0.3, 1.2);
            let config = ShapeCastConfig::from_max_distance(4.0);
            let first = spatial.cast_shape(&capsule, Vec3::ZERO, Quat::IDENTITY,
                Dir3::X, &config, &SpatialQueryFilter::default());
            let filtered = spatial.cast_shape(&capsule, Vec3::ZERO, Quat::IDENTITY,
                Dir3::X, &config, &SpatialQueryFilter::from_excluded_entities([sphere]));
            let only_hidden = spatial.cast_shape_predicate(&capsule, Vec3::ZERO, Quat::IDENTITY,
                Dir3::X, &config, &SpatialQueryFilter::default(), &|entity| entity == hidden);
            (first, filtered, only_hidden)
        }).unwrap();
        assert_eq!(first.unwrap().entity, sphere);
        let filtered = filtered.unwrap();
        assert_eq!(filtered.entity, wall);
        assert!((filtered.distance - 2.1).abs() <= 1e-5);
        assert!(only_hidden.is_none());
    }

    #[test]
    fn capsule_scene_sweep_keeps_initial_penetration_and_outward_filter() {
        use avian3d::prelude::PhysicsPlugins;
        use bevy::{ecs::system::RunSystemOnce, time::TimeUpdateStrategy};
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            TransformPlugin, PhysicsPlugins::new(PostUpdate)));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        let floor = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(100.0, 0.2, 100.0),
            Position(Vec3::new(0.0, -0.1, 0.0)))).id();
        app.update();
        app.update();
        let (hit, outward) = app.world_mut().run_system_once(move |spatial: RagdollSpatialQuery| {
            let capsule = Collider::capsule(0.3, 1.2);
            let origin = Vec3::Y * 0.89;
            let config = ShapeCastConfig::from_max_distance(0.5);
            let hit = spatial.cast_shape(&capsule, origin, Quat::IDENTITY, Dir3::NEG_Y,
                &config, &SpatialQueryFilter::default());
            let outward = spatial.cast_shape(&capsule, origin, Quat::IDENTITY, Dir3::Y,
                &ShapeCastConfig { ignore_origin_penetration: true, ..config }, &SpatialQueryFilter::default());
            (hit, outward)
        }).unwrap();
        let hit = hit.unwrap();
        assert_eq!(hit.entity, floor);
        assert_eq!(hit.distance, 0.0);
        assert!(((hit.point1 - hit.point2).dot(hit.normal1) - 0.01).abs() <= 1e-6);
        assert!(outward.is_none());
    }
}
