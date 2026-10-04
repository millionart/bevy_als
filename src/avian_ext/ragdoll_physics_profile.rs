use std::sync::Arc;

use avian3d::prelude::{
    ActiveCollisionHooks, AngularDamping, AngularInertia, CenterOfMass, Collider, ColliderDisabled, CollisionLayers,
    CoefficientCombine, CollisionHooks, Friction, GravityScale, JointCollisionDisabled, JointDamping, JointDisabled,
    LinearDamping, Mass, MaxAngularSpeed, NoAutoAngularInertia, NoAutoCenterOfMass, NoAutoMass, PhysicsSchedule,
    PhysicsSystems, RayHitData, ShapeCastConfig, ShapeHitData, SolverSystems, SpatialQuery, SpatialQueryFilter,
    Position, Restitution, RevoluteJoint, RigidBody, Rotation, Sensor, SphericalJoint,
};
use avian3d::dynamics::solver::solver_body::{SolverBody, SolverBodyFlags};
use avian3d::collider_tree::{ColliderTreeProxyKey, MovedProxies};
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

use crate::extensions::{
    ragdoll_angular_drive::RagdollAngularDriveSet,
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
    pub mass_properties: Option<RagdollMassProperties>,
    pub locked_axes_bits: u8,
    pub inertia_conditioning_enabled: bool,
    pub ccd_enabled: bool,
    pub gyroscopic_torque_enabled: bool,
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
    collision_enabled: String,
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
                    mass_properties: body.target.mass_properties.as_ref().map(|value| RagdollMassProperties {
                        mass_kg: value.mass_kg,
                        unclamped_mass_kg: value.unclamped_mass_kg,
                        scale_exponent: value.scale_exponent,
                        update_with_scale: value.update_with_scale,
                        center_of_mass_m: Vec3::from_array(value.center_of_mass_m),
                        inertia_tensor_kg_m2: Mat3::from_cols_array_2d(&value.inertia_tensor_kg_m2),
                    }),
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

#[derive(Component, Clone, Debug)]
pub struct RagdollCollisionMember {
    pub group: Entity,
    pub body_index: u8,
    pub source_layers: CollisionLayers,
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
    pub spawned_root: Option<Entity>,
    pub scale: f32,
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
}

impl CollisionHooks for RagdollCollisionHooks<'_, '_> {
    fn filter_pairs(
        &self,
        collider_1: Entity,
        collider_2: Entity,
        _commands: &mut Commands,
    ) -> bool {
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
}

impl RagdollSpatialQuery<'_, '_> {
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
        self.spatial.cast_shape_predicate(shape, origin, rotation, direction, config, filter,
            &|entity| self.queryable(entity) && predicate(entity))
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
        .add_systems(avian3d::prelude::SubstepSchedule, solve_profile_dof
            .after(avian3d::dynamics::solver::xpbd::XpbdSolverSystems::VelocityProjection)
            .after(avian3d::dynamics::solver::schedule::SubstepSolverSystems::Damping)
            .after(crate::extensions::source_physics_joint::solve_source_joint_velocity_iterations))
        .add_systems(avian3d::prelude::SubstepSchedule, solve_profile_linear_ccd
            .after(avian3d::dynamics::integrator::IntegrationSystems::Position)
            .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::Relax))
        .add_systems(PhysicsSchedule, disable_ragdoll_gyroscopic_torque
            .in_set(SolverSystems::PreSubstep));
    }
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
    colliders: Query<(&Collider, &avian3d::prelude::ColliderOf, &avian3d::prelude::ColliderTransform,
        Option<&Sensor>, Option<&CollisionLayers>, Option<&RagdollCollisionMember>), Without<ColliderDisabled>>,
    mut bodies: ParamSet<(Query<(&Position, &Rotation, &RigidBody, &avian3d::prelude::ComputedCenterOfMass, Option<&SolverBody>), Without<avian3d::prelude::RigidBodyDisabled>>,
        Query<&mut SolverBody>)>,
) {
    use avian3d::parry::{math::Pose3, query::{DefaultQueryDispatcher, QueryDispatcher, ShapeCastOptions}};
    let dt = time.delta_secs();
    if dt <= 0.0 || profiles.is_empty() { return; }
    let mut hits = Vec::new();
    {
        let read = bodies.p0();
        for (body, settings) in &profiles {
            let Ok((position, rotation, mode, center, Some(solver))) = read.get(body) else { continue; };
            if !mode.is_dynamic() { continue; }
            let velocity = solver.linear_velocity;
            let end_rotation = solver.delta_rotation.0 * rotation.0;
            let end_position = position.0 + solver.delta_position + rotation.0 * center.0 - end_rotation * center.0;
            let start_position = end_position - velocity * dt;
            let mut aggregate_min = Vec3::splat(f32::INFINITY);
            let mut aggregate_max = Vec3::splat(f32::NEG_INFINITY);
            let mut thinnest = Vec3::splat(f32::INFINITY);
            for (shape, owner, local, sensor, _, _) in &colliders {
                if owner.body != body || sensor.is_some() && !settings.include_probe_shapes { continue; }
                let bounds = shape.shape_scaled().compute_aabb(&Pose3::from_parts(local.translation, local.rotation.0));
                aggregate_min = aggregate_min.min(bounds.mins);
                aggregate_max = aggregate_max.max(bounds.maxs);
                thinnest = thinnest.min(bounds.maxs - bounds.mins);
            }
            if !thinnest.is_finite() { continue; }
            let aggregate = aggregate_max - aggregate_min;
            let axis_threshold = match settings.axis_threshold_mode {
                0 => aggregate,
                1 => Vec3::splat(aggregate.min_element()),
                2 => thinnest,
                3 => Vec3::splat(thinnest.min_element()),
                _ => continue,
            };
            let local_delta = (end_rotation.conjugate() * (velocity * dt)).abs();
            let threshold = settings.enable_threshold_bounds_scale;
            if threshold < 0.0 || threshold > 0.0 && (local_delta - axis_threshold * threshold).max_element() <= 0.0 { continue; }
            let mut earliest = dt;
            let mut hit_body = None;
            for (shape, owner, local, sensor, layers, member) in &colliders {
                if owner.body != body || sensor.is_some() && !settings.include_probe_shapes { continue; }
                let pose = Pose3::from_parts(start_position + end_rotation * local.translation, end_rotation * local.rotation.0);
                for (other, other_owner, other_local, other_sensor, other_layers, other_member) in &colliders {
                    if other_owner.body == body || other_sensor.is_some() && !settings.include_probe_shapes { continue; }
                    if !layers.copied().unwrap_or_default().interacts_with(other_layers.copied().unwrap_or_default()) { continue; }
                    if member.zip(other_member).is_some_and(|(left, right)| left.disables(right)) { continue; }
                    let Ok((other_position, other_rotation, other_mode, other_center, other_solver)) = read.get(other_owner.body) else { continue; };
                    if settings.only_dynamic_static && !other_mode.is_static() { continue; }
                    let other_velocity = other_solver.map_or(Vec3::ZERO, |solver| solver.linear_velocity);
                    let other_end_rotation = other_solver.map_or(other_rotation.0, |solver| solver.delta_rotation.0 * other_rotation.0);
                    let other_end_position = other_position.0 + other_solver.map_or(Vec3::ZERO, |solver| solver.delta_position)
                        + other_rotation.0 * other_center.0 - other_end_rotation * other_center.0;
                    let other_start = other_end_position - other_velocity * dt;
                    let other_pose = Pose3::from_parts(other_start + other_end_rotation * other_local.translation,
                        other_end_rotation * other_local.rotation.0);
                    if let Ok(Some(hit)) = DefaultQueryDispatcher.cast_shapes(&pose.inv_mul(&other_pose),
                        pose.rotation.conjugate() * (other_velocity - velocity), shape.shape_scaled().as_ref(),
                        other.shape_scaled().as_ref(), ShapeCastOptions {
                            max_time_of_impact: earliest, stop_at_penetration: false, ..default()
                        }) && hit.time_of_impact >= 0.0 && hit.time_of_impact < earliest {
                        let own_bounds = shape.shape_scaled().compute_local_aabb();
                        let other_bounds = other.shape_scaled().compute_local_aabb();
                        let min_bounds = (own_bounds.maxs - own_bounds.mins).min_element().max(
                            if profiles.get(other_owner.body).is_ok() { (other_bounds.maxs - other_bounds.mins).min_element() } else { 0.0 });
                        let speed = (velocity - other_velocity).dot(pose.rotation * hit.normal1);
                        if (dt - hit.time_of_impact) * speed < min_bounds * settings.enable_threshold_bounds_scale { continue; }
                        let depth = min_bounds * settings.allowed_depth_bounds_scale;
                        let candidate = if settings.new_target_depth_mode && depth > 0.0 {
                            ccd_time_at_depth(pose, velocity, shape, other_pose, other_velocity, other, hit.time_of_impact, earliest, depth)
                        } else {
                            (speed > 0.0).then_some(hit.time_of_impact + depth / speed).filter(|time| *time < earliest)
                        };
                        if let Some(time) = candidate {
                            earliest = time;
                            hit_body = Some((other_owner.body, other_velocity, other_mode.is_dynamic(),
                                other_solver.map_or(Vec3::ZERO, |solver| solver.delta_position - other_velocity * dt)));
                        }
                    }
                }
            }
            if let Some((other, other_velocity, other_dynamic, other_shift)) = hit_body {
                let origin_shift = solver.delta_position - velocity * dt;
                hits.push((body, other, earliest, velocity, other_velocity, origin_shift, other_dynamic, other_shift));
            }
        }
    }
    hits.sort_by(|left, right| left.2.total_cmp(&right.2));
    let mut write = bodies.p1();
    let mut advanced = bevy::platform::collections::HashSet::new();
    for (body, other, toi, velocity, other_velocity, origin_shift, other_dynamic, other_shift) in hits {
        if advanced.insert(body) && let Ok(mut solver) = write.get_mut(body) {
            solver.delta_position = origin_shift + velocity * toi;
        }
        if other_dynamic && advanced.insert(other) && let Ok(mut solver) = write.get_mut(other) {
            solver.delta_position = other_shift + other_velocity * toi;
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

fn ragdoll_positive_uniform_scale(
    player_entity: Entity,
    transform_helper: &bevy::transform::helper::TransformHelper,
) -> f32 {
    transform_helper.compute_global_transform(player_entity).ok()
        .map(|transform| transform.to_scale_rotation_translation().0)
        .filter(|scale| scale.is_finite() && scale.min_element() > 0.0
            && scale.max_element() - scale.min_element() <= 1e-4)
        .map_or(1.0, |scale| scale.x)
}

fn profile_application_change(
    applied: Option<RagdollPhysicsProfileApplied>,
    spawned_root: Entity,
    scale: f32,
) -> Option<(f32, Option<Entity>)> {
    let previous_scale = applied.map_or(1.0, |applied| {
        if applied.scale.is_finite() && applied.scale > 0.0 { applied.scale } else { 1.0 }
    });
    let old_root = applied.and_then(|applied| applied.spawned_root)
        .filter(|root| *root != spawned_root);
    if applied.is_some() && old_root.is_none() && (previous_scale - scale).abs() <= 1e-4 {
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
        let scale = ragdoll_positive_uniform_scale(player_entity, &transform_helper);
        let (scale_factor, old_root) = match profile_application_change(
            applied.as_ref().map(|applied| **applied), spawned.root, scale,
        ) {
            Some(change) => change,
            None if profile.is_changed() => (1.0, None),
            None => continue,
        };
        if let Some(old_root) = old_root { commands.entity(old_root).try_despawn(); }
        let next = apply_profile_to_spawned_ragdoll(
            &mut commands,
            spawned,
            &profile,
            scale,
            scale_factor,
            applied.is_none() || old_root.is_some() || profile.is_changed(),
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

#[allow(clippy::too_many_arguments)]
fn apply_profile_to_spawned_ragdoll(
    commands: &mut Commands,
    spawned: &SpawnedRagdoll,
    profile: &RagdollPhysicsProfile,
    scale: f32,
    scale_factor: f32,
    recalculate_mass: bool,
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
        collision_disabled_pair_count: profile.collision_disabled_pairs.len() as u32,
        ..default()
    };
    for body_entity in spawned.bodies.values() {
        if scale_factor != 1.0 && let Ok(transform) = transforms.get(*body_entity) {
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
        let mass = body.mass_properties;
        commands.entity(*body_entity).queue(move |mut entity: EntityWorldMut| {
            if let Some(mass) = mass {
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
                    let scaled_mass = (mass.unclamped_mass_kg * scale.powf(mass.scale_exponent)).max(0.001);
                    let mass_ratio = scaled_mass / mass.mass_kg;
                    entity.insert((Mass(scaled_mass), CenterOfMass(mass.center_of_mass_m * scale),
                        AngularInertia::from_mat3_unchecked(mass.inertia_tensor_kg_m2 * scale * scale * mass_ratio),
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
        if scale_factor != 1.0 && let (Ok(transform), Ok(mut collider)) =
            (transforms.get(*collider_entity), colliders.get_mut(*collider_entity)) {
            let mut scaled_transform = *transform;
            scaled_transform.translation *= scale_factor;
            scaled_transform.scale *= scale_factor;
            let collider_scale = collider.scale() * scale_factor;
            collider.set_scale(collider_scale, 16);
            commands.entity(*collider_entity).insert(scaled_transform);
        }
        let Ok(label) = collider_labels.get(*collider_entity) else {
            continue;
        };
        let Some(collider) = find_collider_profile(profile, &label.0) else {
            continue;
        };
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
        if scale != 1.0 {
            joint.parent_frame.translation *= scale;
            joint.child_frame.translation *= scale;
            joint.linear_position_target *= scale;
            joint.linear_velocity_target *= scale;
        }
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
    use super::*;

    #[test]
    fn same_entity_player_replacement_invalidates_previous_profile_application() {
        let mut world = World::new();
        let player = world.spawn((
            AnimationGraphPlayer::new(Handle::default()),
            RagdollPhysicsProfileApplied { scale: 1.0, body_count: 21, ..default() },
        )).id();
        let old_ragdoll = SpawnedRagdoll::new(world.spawn_empty().id());
        let new_ragdoll = SpawnedRagdoll::new(world.spawn_empty().id());
        world.get_mut::<RagdollPhysicsProfileApplied>(player).unwrap().spawned_root = Some(old_ragdoll.root);
        world.entity_mut(player).insert(AnimationGraphPlayer::new(Handle::default()));
        let applied = world.get::<RagdollPhysicsProfileApplied>(player).unwrap();
        assert_ne!(old_ragdoll.root, new_ragdoll.root);
        assert_eq!(profile_application_change(Some(*applied), new_ragdoll.root, 1.0),
            Some((1.0, Some(old_ragdoll.root))));
        assert_eq!(profile_application_change(Some(*applied), old_ragdoll.root, 1.0), None);
        assert_eq!(profile_application_change(Some(*applied), old_ragdoll.root, 2.0), Some((2.0, None)));
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
            ragdoll_positive_uniform_scale(player, &transform_helper)
        }).unwrap();
        assert_eq!(scale, 2.0);
        app.world_mut().get_mut::<Transform>(actor).unwrap().scale = Vec3::new(2.0, 1.0, 2.0);
        let scale = app.world_mut().run_system_once(move |
            transform_helper: bevy::transform::helper::TransformHelper| {
            ragdoll_positive_uniform_scale(player, &transform_helper)
        }).unwrap();
        assert_eq!(scale, 2.0);
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
        assert_eq!(app.world().get::<RagdollPhysicsProfileApplied>(player).unwrap().scale, 1.5);
        let preserved = app.world_mut().run_system_once(move |helper: bevy::transform::helper::TransformHelper| {
            helper.compute_global_transform(player).unwrap().compute_transform()
        }).unwrap();
        app.world_mut().entity_mut(player).remove::<ChildOf>().insert(preserved);
        for (host_scale, player_scale) in [(3.0, 1.5), (0.5, 1.5), (0.5, 0.75), (4.0, 0.75), (4.0, 2.25)] {
            app.world_mut().get_mut::<Transform>(actor).unwrap().scale = Vec3::splat(host_scale);
            app.world_mut().get_mut::<Transform>(player).unwrap().scale = Vec3::splat(player_scale);
            app.update();
            let world = app.world();
            assert_eq!(world.get::<RagdollPhysicsProfileApplied>(player).unwrap().scale, player_scale);
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
            apply_profile_to_spawned_ragdoll(&mut commands, &spawned, &profile, 1.0, 1.0, true,
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
            assert_eq!((applied.spawned_root, applied.scale, applied.body_count), (Some(root), 1.0, 2));
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
                        1.0,
                        1.0,
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
                1.0, 1.0, true, &body_labels, &collider_labels, &joint_labels,
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
                1.0, 1.0, true, &body_labels, &collider_labels, &joint_labels,
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
                2.0, 2.0, true, &body_labels, &collider_labels, &joint_labels,
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
    fn positive_uniform_scale_reaches_ragdoll_shapes_offsets_and_joint_frames() {
        use avian3d::prelude::Collider;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::reflect::{PartialReflect, ReflectMut};
        use bevy_animation_graph::core::ragdoll::{
            definition::Ragdoll, spawning::spawn_ragdoll_avian,
        };

        let profile = RagdollPhysicsProfile::from_json(include_str!(
            "../../assets/ALS/Character/PA_Als.physics.json"
        )).unwrap();
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
            apply_profile_to_spawned_ragdoll(&mut commands, &first, &profile, 1.0, 1.0, true,
                &body_labels, &collider_labels, &joint_labels, &spherical, &revolute,
                &source_joints, &transforms, &mut colliders)
        }}).unwrap();
        app.update();
        let base_mass = app.world().get::<avian3d::prelude::ComputedMass>(pelvis).unwrap().value();
        let world = app.world_mut();
        world.run_system_once(move |mut commands: Commands,
            body_labels: Query<&BodyLabel>, collider_labels: Query<&ColliderLabel>,
            joint_labels: Query<&JointLabel>, spherical: Query<&SphericalJoint>,
            revolute: Query<&RevoluteJoint>, source_joints: Query<&SourcePhysicsJoint>, transforms: Query<&Transform>,
            mut colliders: Query<&mut Collider>| {
            apply_profile_to_spawned_ragdoll(&mut commands, &spawned, &profile, 2.0, 2.0, false,
                &body_labels, &collider_labels, &joint_labels, &spherical, &revolute,
                &source_joints, &transforms, &mut colliders)
        }).unwrap();
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
                spawned_root: Some(old_root), scale: 2.0, body_count: 21,
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
}
