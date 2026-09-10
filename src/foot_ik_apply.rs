use als_core::dynamics::AlsMovementBaseDynamicsState;
use als_core::foot_ik::{
    AlsDynamicTransitionState, AlsFootBones, AlsFootIkRuntime, AlsFootIkTargets,
    AlsFootLockFootState, AlsFootLockInput, AlsFootLockMovementBase, AlsFootLockSettings,
    AlsFootLockState, AlsFootLockTeleported, AlsFootTargetSelection,
    FOOT_LOCK_TELEPORT_DISTANCE_THRESHOLD_M, FOOT_LOCK_TELEPORT_WINDOW_SECONDS, FootSideInput,
    SpringFloatState, advance_foot_lock_movement_base, capture_foot_lock_movement_base,
    constrain_foot_lock_rotation, foot_lock_location_constraint_rotation,
    is_relevant_animation_weight, refresh_foot_lock, reset_foot_lock_to_target,
    spring_damper_float,
};
use als_core::state::{LocomotionState, Stance};
use als_core::transitions::TransitionConfig;
use als_ext::ik::{
    BasicTwoBoneSolve, blend_ik_rotation, calculate_pole_vector, solve_basic_two_bone_ik,
    world_rotation_to_local,
};
use als_ext::ik_space::{
    ArmatureDirection, ArmaturePoint, ArmatureRotation, WorldDirection, WorldPoint, WorldRotation,
    armature_direction_from_world, armature_point_from_world, armature_rotation_from_world,
    bone_local_point_from_world, parent_point_from_world, world_direction_from_armature,
    world_point_from_armature, world_rotation_from_armature,
};
use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::animation::AnimationPlayer;
use bevy::prelude::*;
use bevy::transform::{TransformSystems, systems as transform_systems};
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::apply_animation_to_targets;

use crate::animation_context_projection::{AlsRootAnimationContext, AlsRootAnimationContextSet};
use crate::character_assembly::AlsCharacterAvatar;
use crate::control_rig_apply::{
    AlsControlRigConstruction, AlsControlRigInput, AlsControlRigPrePropagationSet,
    AlsControlRigStageTrace, CrAlsRuntimeProfileTemplate, CrAlsTargetProfile,
};
use crate::head_layer::inv_exp_approx;

#[derive(Component, Debug, Clone, Copy)]
struct FootOffsetRotationState {
    normals: [Vec3; 2],
    initialized: [bool; 2],
}

#[derive(Component, Debug, Clone, Copy, Default)]
struct FootLockLifecycleState {
    previous_character_transform: Option<Transform>,
    teleport_remaining: f32,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct AlsFootLockTeleportTelemetry {
    pub detected_count: u32,
    pub left_rebase_count: u32,
    pub right_rebase_count: u32,
    pub last_translation_delta_m: f32,
    pub remaining_seconds: f32,
}

#[derive(Component, Debug, Clone, Copy, Default)]
struct SourcePoleState {
    current_armature: Vec3,
    initialized: bool,
}

impl SourcePoleState {
    fn update(
        &mut self,
        armature: &GlobalTransform,
        hip: Vec3,
        knee: Vec3,
        ankle: Vec3,
        delta_seconds: f32,
        profile: &CrAlsTargetProfile,
    ) -> Vec3 {
        let hip = armature_point_from_world(armature, WorldPoint(hip)).0;
        let knee = armature_point_from_world(armature, WorldPoint(knee)).0;
        let ankle = armature_point_from_world(armature, WorldPoint(ankle)).0;
        let target = calculate_pole_vector(hip, knee, ankle)
            .map(|pole| knee + pole.direction * profile.foot_pole_distance_m);
        let Some(target) = target else {
            return world_point_from_armature(armature, ArmaturePoint(self.current_armature)).0;
        };
        if !self.initialized {
            self.initialized = true;
            self.current_armature = target;
        }
        let alpha = 1.0
            - inv_exp_approx(
                std::f32::consts::LN_2 * delta_seconds.max(0.0)
                    / profile.foot_pole_half_life_seconds.max(f32::EPSILON),
            );
        self.current_armature = self.current_armature.lerp(target, alpha);
        world_point_from_armature(armature, ArmaturePoint(self.current_armature)).0
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
struct FootIkPoleState {
    left: SourcePoleState,
    right: SourcePoleState,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct AlsFootIkTelemetrySide {
    pub valid: bool,
    pub target_world: Vec3,
    pub target_armature_local: Vec3,
    pub target_parent_local: Vec3,
    pub reference_chain_error: f32,
    pub pole_sign: f32,
    pub ik_weight: f32,
    pub pin_weight: f32,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct AlsFootIkTelemetry {
    pub left: AlsFootIkTelemetrySide,
    pub right: AlsFootIkTelemetrySide,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct AlsFootIkSpaceSide {
    pub valid: bool,
    pub has_ground_hit: bool,
    pub trace_target_world: Vec3,
    pub hit_world: Vec3,
    pub solver_reference_world: Vec3,
    pub solver_reference_armature_local: Vec3,
    pub target_world: Vec3,
    pub resolved_target_world: Vec3,
    pub resolved_knee_world: Vec3,
    pub pole_location_world: Vec3,
    pub target_armature_local: Vec3,
    pub target_parent_local: Vec3,
    pub target_bone_local: Vec3,
    pub normal_world: Vec3,
    pub normal_armature_local: Vec3,
    pub raw_offset: f32,
    pub solve_input: Option<FootIkSolveInput>,
    pub previous_solve_input: Option<FootIkSolveInput>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize)]
pub struct FootIkSolveInput {
    pub armature_world_trs: ([f32; 3], [f32; 4], [f32; 3]),
    pub thigh_world_trs: ([f32; 3], [f32; 4], [f32; 3]),
    pub calf_world_trs: ([f32; 3], [f32; 4], [f32; 3]),
    pub foot_world_trs: ([f32; 3], [f32; 4], [f32; 3]),
    pub target_world: [f32; 3],
    pub target_rotation_world: [f32; 4],
    pub pole_world: [f32; 3],
    pub primary_axis: [f32; 3],
    pub secondary_axis: [f32; 3],
    pub upper_length: f32,
    pub lower_length: f32,
    pub weight: f32,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct AlsFootIkSpaceState {
    pub left: AlsFootIkSpaceSide,
    pub right: AlsFootIkSpaceSide,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct AlsFootIkApplyTelemetrySide {
    pub valid: bool,
    pub target_world: Vec3,
    pub effector_world: Vec3,
    pub hip_world: Vec3,
    pub knee_world: Vec3,
    pub ankle_world: Vec3,
    pub hip_error: f32,
    pub knee_error: f32,
    pub effector_error: f32,
    pub pole_sign: f32,
    pub applied_weight: f32,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct AlsFootIkApplyTelemetry {
    pub left: AlsFootIkApplyTelemetrySide,
    pub right: AlsFootIkApplyTelemetrySide,
    pub restore_translation_delta_m: f32,
    pub restore_rotation_delta_radians: f32,
    pub solver_translation_delta_m: f32,
    pub solver_rotation_delta_radians: f32,
    pub pelvis_write_delta_m: f32,
    pub left_chain_write_delta_m: f32,
    pub right_chain_write_delta_m: f32,
}

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsFootIkTelemetryEnabled(pub bool);

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsFootIkWeightOverride {
    pub left: Option<f32>,
    pub right: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default)]
struct FootTraceSample {
    has_ground_hit: bool,
    trace_target_world: WorldPoint,
    hit_world: WorldPoint,
    normal_world: WorldDirection,
    normal_armature: ArmatureDirection,
    raw_offset: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct CrAlsFootSideState {
    trace_offset_location_y: f32,
    location_spring: SpringFloatState,
    location_offset_y: f32,
}

#[derive(Component, Debug, Clone, Copy, Default)]
struct CrAlsFootRigState {
    left: CrAlsFootSideState,
    right: CrAlsFootSideState,
    pelvis_spring: SpringFloatState,
    pelvis_spring_value: f32,
    pelvis_offset_y: f32,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedFootTarget {
    target_world: Vec3,
    knee_world: Vec3,
    input: FootIkSolveInput,
}

#[derive(Debug, Clone, Copy)]
struct SourceBasicIkWriteback {
    thigh: Transform,
    calf: Transform,
    foot: Transform,
}

const FOOT_IK_POSE_BONE_COUNT: usize = 7;

#[derive(Component, Debug, Clone, Copy)]
struct FootIkPoseBaseState {
    entities: [Option<Entity>; FOOT_IK_POSE_BONE_COUNT],
    animation_base: [Transform; FOOT_IK_POSE_BONE_COUNT],
    previous_applied: [Transform; FOOT_IK_POSE_BONE_COUNT],
    initialized: [bool; FOOT_IK_POSE_BONE_COUNT],
    restore_translation_delta_m: f32,
    restore_rotation_delta_radians: f32,
    solver_translation_delta_m: f32,
    solver_rotation_delta_radians: f32,
}

impl Default for FootIkPoseBaseState {
    fn default() -> Self {
        Self {
            entities: [None; FOOT_IK_POSE_BONE_COUNT],
            animation_base: [Transform::IDENTITY; FOOT_IK_POSE_BONE_COUNT],
            previous_applied: [Transform::IDENTITY; FOOT_IK_POSE_BONE_COUNT],
            initialized: [false; FOOT_IK_POSE_BONE_COUNT],
            restore_translation_delta_m: 0.0,
            restore_rotation_delta_radians: 0.0,
            solver_translation_delta_m: 0.0,
            solver_rotation_delta_radians: 0.0,
        }
    }
}

fn transforms_approximately_equal(left: Transform, right: Transform) -> bool {
    left.translation.distance_squared(right.translation) <= 1e-10
        && left.rotation.dot(right.rotation).abs() >= 1.0 - 1e-6
        && left.scale.distance_squared(right.scale) <= 1e-10
}

fn foot_ik_stage_delta(before: Transform, after: Transform) -> (f32, f32) {
    let translation = before.translation.distance(after.translation);
    let rotation = 2.0
        * before
            .rotation
            .dot(after.rotation)
            .abs()
            .clamp(-1.0, 1.0)
            .acos();
    (translation, rotation)
}

fn foot_ik_max_translation_delta<const N: usize>(
    before: [Transform; N],
    after: [Transform; N],
) -> f32 {
    before
        .into_iter()
        .zip(after)
        .map(|(before, after)| before.translation.distance(after.translation))
        .fold(0.0, f32::max)
}

fn foot_ik_chain_locals(
    chain: &als_core::foot_ik::FootChain,
    bone_locals: &mut Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
) -> Option<[Transform; 3]> {
    Some([
        *bone_locals.get(chain.thigh).ok()?.0,
        *bone_locals.get(chain.calf).ok()?.0,
        *bone_locals.get(chain.foot).ok()?.0,
    ])
}

fn restore_foot_ik_animation_transform(
    state: &mut FootIkPoseBaseState,
    index: usize,
    entity: Entity,
    current: Transform,
) -> Transform {
    if !state.initialized[index] || state.entities[index] != Some(entity) {
        state.entities[index] = Some(entity);
        state.animation_base[index] = current;
        state.previous_applied[index] = current;
        state.initialized[index] = true;
        return current;
    }
    if transforms_approximately_equal(current, state.previous_applied[index]) {
        return state.animation_base[index];
    }
    state.animation_base[index] = current;
    current
}

fn foot_ik_pose_entities(
    bones: &AlsFootBones,
    bone_locals: &mut Query<(&mut Transform, &ChildOf), Without<AnimationPlayer>>,
) -> Option<[Entity; FOOT_IK_POSE_BONE_COUNT]> {
    let pelvis = bone_locals.get(bones.left_thigh).ok()?.1.0;
    Some([
        pelvis,
        bones.left_thigh,
        bones.left_calf,
        bones.left,
        bones.right_thigh,
        bones.right_calf,
        bones.right,
    ])
}

fn source_transform_point(transform: &Transform, point: Vec3) -> Vec3 {
    transform.to_matrix().transform_point3(point)
}

fn source_inverse_transform_point(transform: &Transform, point: Vec3) -> Option<Vec3> {
    if transform.scale.abs().min_element() <= f32::EPSILON {
        return None;
    }
    let value = transform.to_matrix().inverse().transform_point3(point);
    value.is_finite().then_some(value)
}

fn source_local_transform(parent: &Transform, global: &Transform) -> Option<Transform> {
    let translation = source_inverse_transform_point(parent, global.translation)?;
    if parent.scale.abs().min_element() <= f32::EPSILON {
        return None;
    }
    let scale = global.scale / parent.scale;
    (translation.is_finite() && scale.is_finite() && global.rotation.is_finite()).then_some(
        Transform {
            translation,
            rotation: source_local_rotation(parent.rotation, global.rotation),
            scale,
        },
    )
}

fn source_basic_ik_writeback(
    current_thigh: Transform,
    current_calf: Transform,
    current_foot: Transform,
    solve: BasicTwoBoneSolve,
    foot_target_rotation: Quat,
    weight: f32,
) -> Option<SourceBasicIkWriteback> {
    let solved_thigh = Transform {
        translation: current_thigh.translation,
        rotation: solve.hip_rotation_world,
        scale: current_thigh.scale,
    };
    let solved_calf = Transform {
        translation: solve.knee_position,
        rotation: solve.knee_rotation_world,
        scale: current_thigh.scale,
    };
    let solved_foot = Transform {
        translation: solve.effector_position,
        rotation: foot_target_rotation,
        scale: Vec3::ONE,
    };
    let calf_local = source_inverse_transform_point(&solved_thigh, solved_calf.translation)?;
    let foot_local = source_inverse_transform_point(&solved_calf, solved_foot.translation)?;
    let thigh = Transform {
        translation: current_thigh.translation,
        rotation: blend_ik_rotation(current_thigh.rotation, solved_thigh.rotation, weight),
        scale: solved_thigh.scale,
    };
    let calf = Transform {
        translation: source_transform_point(&thigh, calf_local),
        rotation: blend_ik_rotation(current_calf.rotation, solved_calf.rotation, weight),
        scale: solved_calf.scale,
    };
    let foot = Transform {
        translation: source_transform_point(&calf, foot_local),
        rotation: blend_ik_rotation(current_foot.rotation, solved_foot.rotation, weight),
        scale: solved_foot.scale,
    };
    Some(SourceBasicIkWriteback { thigh, calf, foot })
}

impl Default for FootOffsetRotationState {
    fn default() -> Self {
        Self {
            normals: [Vec3::Y; 2],
            initialized: [false; 2],
        }
    }
}

impl FootOffsetRotationState {
    fn update(
        &mut self,
        side: usize,
        target_normal: Vec3,
        delta_seconds: f32,
        half_life_seconds: f32,
    ) -> Vec3 {
        let target_normal = normalized_ground_normal(target_normal);
        if !self.initialized[side] {
            self.initialized[side] = true;
            self.normals[side] = target_normal;
            return target_normal;
        }
        let current_normal = normalized_ground_normal(self.normals[side]);
        let alpha = 1.0
            - inv_exp_approx(
                std::f32::consts::LN_2 * delta_seconds.max(0.0)
                    / half_life_seconds.max(f32::EPSILON),
            );
        let normal = current_normal
            .lerp(target_normal, alpha)
            .try_normalize()
            .unwrap_or(target_normal);
        self.normals[side] = normal;
        normal
    }
}

pub struct FootIkApplyPlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FootIkApplySet {
    Restore,
    Solve,
    Propagate,
    Telemetry,
}

impl Plugin for FootIkApplyPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AnimationGraphSet::Final.before(TransformSystems::Propagate),
        );
        app.init_resource::<AlsFootIkTelemetryEnabled>()
            .init_resource::<AlsFootIkWeightOverride>()
            .init_resource::<CrAlsRuntimeProfileTemplate>()
            .add_message::<AlsFootLockTeleported>();
        app.configure_sets(
            PostUpdate,
            (
                FootIkApplySet::Restore
                    .after(apply_animation_to_targets)
                    .after(AlsRootAnimationContextSet)
                    .after(TransformSystems::Propagate),
                FootIkApplySet::Solve
                    .after(FootIkApplySet::Restore)
                    .after(apply_animation_to_targets)
                    .after(AlsRootAnimationContextSet)
                    .after(TransformSystems::Propagate)
                    .after(AlsControlRigPrePropagationSet)
                    .after(als_core::foot_ik::foot_ground_probe_system)
                    .after(als_core::foot_ik::foot_pin_request_system)
                    .after(als_core::foot_ik::foot_ik_state_system),
                FootIkApplySet::Propagate.after(FootIkApplySet::Solve),
                FootIkApplySet::Telemetry.after(FootIkApplySet::Propagate),
            ),
        );
        app.add_systems(
            PostUpdate,
            (
                ensure_foot_ik_runtime,
                ensure_cr_als_foot_rig_state,
                ensure_foot_ik_telemetry,
                ensure_foot_ik_space_state,
                ensure_foot_ik_apply_telemetry,
                ensure_foot_ik_pole_state,
                ensure_foot_ik_pose_base_state,
                ensure_foot_lock_components,
                ensure_foot_offset_rotation_state,
                ensure_foot_lock_lifecycle_state,
                ensure_foot_lock_teleport_telemetry,
                invalidate_missing_foot_bones.before(FootIkApplySet::Solve),
                detect_foot_lock_teleports
                    .after(TransformSystems::Propagate)
                    .before(record_foot_lock_teleports),
                record_foot_lock_teleports
                    .after(detect_foot_lock_teleports)
                    .before(FootIkApplySet::Solve),
                restore_foot_ik_animation_pose_base.in_set(FootIkApplySet::Restore),
                (
                    transform_systems::mark_dirty_trees,
                    transform_systems::propagate_parent_transforms,
                    transform_systems::sync_simple_transforms,
                )
                    .chain()
                    .after(FootIkApplySet::Restore)
                    .before(FootIkApplySet::Solve),
                foot_ik_apply_system.in_set(FootIkApplySet::Solve),
                capture_foot_ik_applied_pose_base
                    .after(FootIkApplySet::Solve)
                    .before(FootIkApplySet::Propagate),
                (
                    transform_systems::mark_dirty_trees,
                    transform_systems::propagate_parent_transforms,
                    transform_systems::sync_simple_transforms,
                )
                    .chain()
                    .in_set(FootIkApplySet::Propagate),
                foot_ik_post_apply_telemetry_system.in_set(FootIkApplySet::Telemetry),
            ),
        );
    }
}

fn ensure_foot_lock_components(
    mut commands: Commands,
    states: Query<Entity, (With<AlsFootBones>, Without<AlsFootLockState>)>,
    inputs: Query<Entity, (With<AlsFootBones>, Without<AlsFootLockInput>)>,
) {
    for entity in &states {
        commands.entity(entity).insert(AlsFootLockState::default());
    }
    for entity in &inputs {
        commands.entity(entity).insert(AlsFootLockInput::default());
    }
}

fn ensure_foot_ik_runtime(
    mut commands: Commands,
    q: Query<Entity, (With<AlsFootBones>, Without<AlsFootIkRuntime>)>,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsFootIkRuntime::default());
    }
}

fn ensure_cr_als_foot_rig_state(
    mut commands: Commands,
    q: Query<Entity, (With<AlsFootBones>, Without<CrAlsFootRigState>)>,
) {
    for entity in &q {
        commands.entity(entity).insert(CrAlsFootRigState::default());
    }
}

fn ensure_foot_ik_telemetry(
    mut commands: Commands,
    q: Query<Entity, (With<AlsFootBones>, Without<AlsFootIkTelemetry>)>,
) {
    for entity in &q {
        commands
            .entity(entity)
            .insert(AlsFootIkTelemetry::default());
    }
}

fn ensure_foot_ik_space_state(
    mut commands: Commands,
    q: Query<Entity, (With<AlsFootBones>, Without<AlsFootIkSpaceState>)>,
) {
    for entity in &q {
        commands
            .entity(entity)
            .insert(AlsFootIkSpaceState::default());
    }
}

fn ensure_foot_ik_apply_telemetry(
    mut commands: Commands,
    q: Query<Entity, (With<AlsFootBones>, Without<AlsFootIkApplyTelemetry>)>,
) {
    for entity in &q {
        commands
            .entity(entity)
            .insert(AlsFootIkApplyTelemetry::default());
    }
}

fn ensure_foot_ik_pole_state(
    mut commands: Commands,
    q: Query<Entity, (With<AlsFootBones>, Without<FootIkPoleState>)>,
) {
    for entity in &q {
        commands.entity(entity).insert(FootIkPoleState::default());
    }
}

fn ensure_foot_ik_pose_base_state(
    mut commands: Commands,
    q: Query<Entity, (With<AlsFootBones>, Without<FootIkPoseBaseState>)>,
) {
    for entity in &q {
        commands
            .entity(entity)
            .insert(FootIkPoseBaseState::default());
    }
}

fn restore_foot_ik_animation_pose_base(
    mut characters: Query<
        (&AlsFootBones, &mut FootIkPoseBaseState),
        With<als_core::character::AlsCharacter>,
    >,
    mut bone_locals: Query<(&mut Transform, &ChildOf), Without<AnimationPlayer>>,
) {
    for (bones, mut state) in &mut characters {
        state.restore_translation_delta_m = 0.0;
        state.restore_rotation_delta_radians = 0.0;
        let Some(entities) = foot_ik_pose_entities(bones, &mut bone_locals) else {
            *state = FootIkPoseBaseState::default();
            continue;
        };
        for (index, entity) in entities.into_iter().enumerate() {
            let Ok((mut transform, _)) = bone_locals.get_mut(entity) else {
                *state = FootIkPoseBaseState::default();
                break;
            };
            let before = *transform;
            *transform = restore_foot_ik_animation_transform(&mut state, index, entity, before);
            let (translation, rotation) = foot_ik_stage_delta(before, *transform);
            state.restore_translation_delta_m = state.restore_translation_delta_m.max(translation);
            state.restore_rotation_delta_radians =
                state.restore_rotation_delta_radians.max(rotation);
        }
    }
}

fn capture_foot_ik_applied_pose_base(
    mut characters: Query<
        (&AlsFootBones, &mut FootIkPoseBaseState),
        With<als_core::character::AlsCharacter>,
    >,
    mut bone_locals: Query<(&mut Transform, &ChildOf), Without<AnimationPlayer>>,
) {
    for (bones, mut state) in &mut characters {
        state.solver_translation_delta_m = 0.0;
        state.solver_rotation_delta_radians = 0.0;
        let Some(entities) = foot_ik_pose_entities(bones, &mut bone_locals) else {
            *state = FootIkPoseBaseState::default();
            continue;
        };
        for (index, entity) in entities.into_iter().enumerate() {
            let Ok((transform, _)) = bone_locals.get_mut(entity) else {
                *state = FootIkPoseBaseState::default();
                break;
            };
            let (translation, rotation) =
                foot_ik_stage_delta(state.animation_base[index], *transform);
            state.solver_translation_delta_m = state.solver_translation_delta_m.max(translation);
            state.solver_rotation_delta_radians = state.solver_rotation_delta_radians.max(rotation);
            state.previous_applied[index] = *transform;
        }
    }
}

fn ensure_foot_offset_rotation_state(
    mut commands: Commands,
    q: Query<Entity, (With<AlsFootBones>, Without<FootOffsetRotationState>)>,
) {
    for entity in &q {
        commands
            .entity(entity)
            .insert(FootOffsetRotationState::default());
    }
}

fn ensure_foot_lock_lifecycle_state(
    mut commands: Commands,
    q: Query<Entity, (With<AlsFootBones>, Without<FootLockLifecycleState>)>,
) {
    for entity in &q {
        commands
            .entity(entity)
            .insert(FootLockLifecycleState::default());
    }
}

fn ensure_foot_lock_teleport_telemetry(
    mut commands: Commands,
    q: Query<Entity, (With<AlsFootBones>, Without<AlsFootLockTeleportTelemetry>)>,
) {
    for entity in &q {
        commands
            .entity(entity)
            .insert(AlsFootLockTeleportTelemetry::default());
    }
}

fn detect_foot_lock_teleports(
    mut teleports: MessageWriter<AlsFootLockTeleported>,
    mut lifecycles: Query<
        (Entity, &mut FootLockLifecycleState),
        (With<als_core::character::AlsCharacter>, With<AlsFootBones>),
    >,
    avatars: Query<(&ChildOf, &GlobalTransform), With<AlsCharacterAvatar>>,
    mut telemetry: Query<&mut AlsFootLockTeleportTelemetry>,
) {
    for (entity, lifecycle) in &mut lifecycles {
        let Some((_, armature)) = avatars.iter().find(|(parent, _)| parent.parent() == entity)
        else {
            continue;
        };
        let current = armature.compute_transform();
        let Some(previous) = lifecycle.previous_character_transform else {
            continue;
        };
        let translation_delta_m = previous.translation.distance(current.translation);
        if translation_delta_m <= FOOT_LOCK_TELEPORT_DISTANCE_THRESHOLD_M {
            continue;
        }
        teleports.write(AlsFootLockTeleported { entity });
        if let Ok(mut value) = telemetry.get_mut(entity) {
            value.detected_count = value.detected_count.saturating_add(1);
            value.last_translation_delta_m = translation_delta_m;
        }
    }
}

fn record_foot_lock_teleports(
    mut commands: Commands,
    mut teleports: MessageReader<AlsFootLockTeleported>,
    mut lifecycles: Query<&mut FootLockLifecycleState>,
    mut telemetry: Query<&mut AlsFootLockTeleportTelemetry>,
) {
    for teleport in teleports.read() {
        if let Ok(mut lifecycle) = lifecycles.get_mut(teleport.entity) {
            lifecycle.teleport_remaining = FOOT_LOCK_TELEPORT_WINDOW_SECONDS;
        } else {
            commands
                .entity(teleport.entity)
                .insert(FootLockLifecycleState {
                    previous_character_transform: None,
                    teleport_remaining: FOOT_LOCK_TELEPORT_WINDOW_SECONDS,
                });
        }
        if let Ok(mut value) = telemetry.get_mut(teleport.entity) {
            value.remaining_seconds = FOOT_LOCK_TELEPORT_WINDOW_SECONDS;
        }
    }
}

fn invalidate_missing_foot_bones(
    mut characters: Query<
        (
            Option<&AlsFootBones>,
            Option<&mut AlsFootLockState>,
            Option<&mut AlsFootIkRuntime>,
            Option<&mut CrAlsFootRigState>,
            Option<&mut FootOffsetRotationState>,
            Option<&mut AlsDynamicTransitionState>,
            Option<&mut FootLockLifecycleState>,
            Option<&mut AlsFootIkTelemetry>,
            Option<&mut AlsFootIkSpaceState>,
            Option<&mut AlsFootIkApplyTelemetry>,
            Option<&mut FootIkPoleState>,
            Option<&mut AlsFootLockTeleportTelemetry>,
            Option<&mut FootIkPoseBaseState>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
) {
    for (
        bones,
        lock_state,
        runtime,
        source_rig,
        offset_rotation,
        dynamic_transition,
        lifecycle,
        telemetry,
        space_state,
        apply_telemetry,
        pole_state,
        teleport_telemetry,
        pose_base,
    ) in &mut characters
    {
        if bones.is_some() {
            continue;
        }
        if let Some(mut lock_state) = lock_state {
            *lock_state = AlsFootLockState::default();
        }
        if let Some(mut runtime) = runtime {
            *runtime = AlsFootIkRuntime::default();
        }
        if let Some(mut source_rig) = source_rig {
            *source_rig = CrAlsFootRigState::default();
        }
        if let Some(mut offset_rotation) = offset_rotation {
            *offset_rotation = FootOffsetRotationState::default();
        }
        if let Some(mut dynamic_transition) = dynamic_transition {
            *dynamic_transition = AlsDynamicTransitionState::default();
        }
        if let Some(mut lifecycle) = lifecycle {
            *lifecycle = FootLockLifecycleState::default();
        }
        if let Some(mut telemetry) = telemetry {
            *telemetry = AlsFootIkTelemetry::default();
        }
        if let Some(mut space_state) = space_state {
            *space_state = AlsFootIkSpaceState::default();
        }
        if let Some(mut apply_telemetry) = apply_telemetry {
            *apply_telemetry = AlsFootIkApplyTelemetry::default();
        }
        if let Some(mut pole_state) = pole_state {
            *pole_state = FootIkPoleState::default();
        }
        if let Some(mut teleport_telemetry) = teleport_telemetry {
            *teleport_telemetry = AlsFootLockTeleportTelemetry::default();
        }
        if let Some(mut pose_base) = pose_base {
            *pose_base = FootIkPoseBaseState::default();
        }
    }
}

fn foot_bones_are_valid(
    bones: &AlsFootBones,
    use_ik_bones: bool,
    transforms: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) -> bool {
    [
        bones.left_thigh,
        bones.left_calf,
        bones.left,
        bones.target(0, use_ik_bones),
        bones.right_thigh,
        bones.right_calf,
        bones.right,
        bones.target(1, use_ik_bones),
    ]
    .into_iter()
    .all(|entity| {
        transforms.get(entity).is_ok_and(|transform| {
            transform.translation().is_finite() && transform.rotation().is_finite()
        })
    })
}

fn refresh_previous_foot_lock_transform(
    state: &mut als_core::foot_ik::AlsFootLockFootState,
    previous_character_transform: &Transform,
    current_character_transform: &Transform,
    rebase_lock: bool,
) {
    let previous_inverse = previous_character_transform.to_matrix().inverse();
    let current_matrix = current_character_transform.to_matrix();
    state.final_location =
        current_matrix.transform_point3(previous_inverse.transform_point3(state.final_location));
    let rotation_delta =
        current_character_transform.rotation * previous_character_transform.rotation.conjugate();
    state.final_rotation = (rotation_delta * state.final_rotation).normalize();
    if rebase_lock {
        state.lock_location =
            current_matrix.transform_point3(previous_inverse.transform_point3(state.lock_location));
        state.lock_rotation = (rotation_delta * state.lock_rotation).normalize();
    }
}

fn reset_foot_ik_runtime(
    runtime: &mut AlsFootIkRuntime,
    offset_rotation: &mut FootOffsetRotationState,
    dynamic_transition: &mut AlsDynamicTransitionState,
) {
    *runtime = AlsFootIkRuntime::default();
    *offset_rotation = FootOffsetRotationState::default();
    *dynamic_transition = AlsDynamicTransitionState::default();
}

fn source_constrain_foot_lock(
    state: &mut AlsFootLockFootState,
    armature: &GlobalTransform,
    pelvis_world: &GlobalTransform,
    thigh_axis_pelvis_space: Vec3,
    settings: &AlsFootLockSettings,
) {
    if !is_relevant_animation_weight(state.lock_amount) {
        return;
    }
    let pelvis_rotation_armature =
        armature_rotation_from_world(armature, WorldRotation(pelvis_world.rotation())).0;
    let thigh_axis_armature = pelvis_rotation_armature * thigh_axis_pelvis_space;
    let mut lock_location_armature =
        armature_point_from_world(armature, WorldPoint(state.lock_location)).0;
    let mut lock_rotation_armature =
        armature_rotation_from_world(armature, WorldRotation(state.lock_rotation)).0;
    let target_rotation_armature =
        armature_rotation_from_world(armature, WorldRotation(state.target_rotation)).0;
    if let Some(offset_rotation) = foot_lock_location_constraint_rotation(
        thigh_axis_armature,
        lock_location_armature,
        settings.thigh_angle_limit_degrees,
    ) {
        lock_location_armature = offset_rotation * lock_location_armature;
        lock_rotation_armature = (offset_rotation * lock_rotation_armature).normalize();
    }
    lock_rotation_armature = constrain_foot_lock_rotation(
        lock_rotation_armature,
        target_rotation_armature,
        settings.foot_angle_limit_degrees,
    );
    state.lock_location =
        world_point_from_armature(armature, ArmaturePoint(lock_location_armature)).0;
    state.lock_rotation =
        world_rotation_from_armature(armature, ArmatureRotation(lock_rotation_armature)).0;
    state.final_location = state
        .target_location
        .lerp(state.lock_location, state.lock_amount);
    state.final_rotation = state
        .target_rotation
        .lerp(state.lock_rotation, state.lock_amount)
        .normalize();
}

fn source_foot_trace_sample(
    spatial: &SpatialQuery,
    armature_world: &GlobalTransform,
    trace_target_world: WorldPoint,
    character: Entity,
    foot: Entity,
    target: Entity,
    foot_ik_amount: f32,
    profile: &CrAlsTargetProfile,
    foot_height_m: f32,
) -> FootTraceSample {
    let armature_up_world =
        world_direction_from_armature(armature_world, ArmatureDirection(Vec3::Y));
    if foot_ik_amount < 1e-4 {
        return FootTraceSample {
            trace_target_world,
            normal_world: armature_up_world,
            normal_armature: ArmatureDirection(Vec3::Y),
            ..Default::default()
        };
    }
    let target_armature = armature_point_from_world(armature_world, trace_target_world).0;
    let trace_start = world_point_from_armature(
        armature_world,
        ArmaturePoint(Vec3::new(
            target_armature.x,
            profile.trace_distance_up_m,
            target_armature.z,
        )),
    );
    let trace_end = world_point_from_armature(
        armature_world,
        ArmaturePoint(Vec3::new(
            target_armature.x,
            -profile.trace_distance_down_m,
            target_armature.z,
        )),
    );
    let delta = trace_end.0 - trace_start.0;
    let Some(direction) = Dir3::new(delta).ok() else {
        return FootTraceSample {
            trace_target_world,
            normal_world: armature_up_world,
            normal_armature: ArmatureDirection(Vec3::Y),
            ..Default::default()
        };
    };
    let filter = SpatialQueryFilter::from_excluded_entities([character, foot, target]);
    let Some(hit) = spatial.cast_ray(trace_start.0, direction, delta.length(), true, &filter)
    else {
        return FootTraceSample {
            trace_target_world,
            normal_world: armature_up_world,
            normal_armature: ArmatureDirection(Vec3::Y),
            ..Default::default()
        };
    };
    let normal_world = WorldDirection(hit.normal.try_normalize().unwrap_or(Vec3::ZERO));
    let normal_armature = armature_direction_from_world(armature_world, normal_world);
    let hit_world = WorldPoint(trace_start.0 + direction.as_vec3() * hit.distance);
    let hit_armature = armature_point_from_world(armature_world, hit_world).0;
    let Some(raw_offset) =
        source_trace_offset(hit_armature, normal_armature.0, profile, foot_height_m)
    else {
        return FootTraceSample {
            trace_target_world,
            normal_world: armature_up_world,
            normal_armature: ArmatureDirection(Vec3::Y),
            ..Default::default()
        };
    };
    FootTraceSample {
        has_ground_hit: raw_offset.is_finite(),
        trace_target_world,
        hit_world,
        normal_world,
        normal_armature,
        raw_offset,
    }
}

fn source_trace_offset(
    hit_location_armature: Vec3,
    hit_normal_armature: Vec3,
    profile: &CrAlsTargetProfile,
    foot_height_m: f32,
) -> Option<f32> {
    let slope_cos = hit_normal_armature.try_normalize()?.y;
    if slope_cos < profile.walkable_floor_angle_degrees.to_radians().cos() {
        return None;
    }
    let slope_offset = foot_height_m / slope_cos - foot_height_m;
    Some(hit_location_armature.y + slope_offset)
}

fn source_apply_foot_offset_location(
    side: &mut CrAlsFootSideState,
    foot_target_armature: Vec3,
    foot_offset_y: f32,
    pelvis_offset_y: f32,
    pelvis_location_y: f32,
    thigh_location_armature: Vec3,
    pose_moving: f32,
    delta_seconds: f32,
    profile: &CrAlsTargetProfile,
    leg_length_m: f32,
) -> Vec3 {
    let min_pelvis_to_foot = profile.pelvis_min_to_foot_standing_m.lerp(
        profile.pelvis_min_to_foot_moving_m,
        pose_moving.clamp(0.0, 1.0),
    );
    let max_foot_location_y = pelvis_location_y - min_pelvis_to_foot;
    let max_offset_y = max_foot_location_y - foot_target_armature.y;
    let target_offset_y = foot_offset_y.min(max_offset_y).max(pelvis_offset_y);
    side.location_offset_y = spring_damper_float(
        &mut side.location_spring,
        side.location_offset_y,
        target_offset_y,
        delta_seconds,
        profile.foot_offset_frequency_hz,
        profile.foot_offset_damping_ratio,
        0.0,
    );
    let foot_location = foot_target_armature + Vec3::Y * side.location_offset_y;
    let leg_vector = foot_location - thigh_location_armature;
    let max_leg_length = leg_length_m * profile.max_leg_stretch_ratio;
    if leg_vector.length() > max_leg_length {
        thigh_location_armature + leg_vector.normalize_or_zero() * max_leg_length
    } else {
        foot_location
    }
}

#[allow(clippy::type_complexity)]
fn foot_ik_apply_system(
    time: Res<Time>,
    spatial: SpatialQuery,
    telemetry_enabled: Res<AlsFootIkTelemetryEnabled>,
    weight_override: Res<AlsFootIkWeightOverride>,
    profile: Res<CrAlsRuntimeProfileTemplate>,
    mut characters: Query<
        (
            Entity,
            &AlsFootBones,
            Option<&AlsFootTargetSelection>,
            &AlsFootIkTargets,
            &LocomotionState,
            &AlsFootLockInput,
            &Stance,
            &AlsFootLockSettings,
            Option<&AlsMovementBaseDynamicsState>,
            &mut AlsFootIkRuntime,
            &mut CrAlsFootRigState,
            &mut FootOffsetRotationState,
            &mut FootLockLifecycleState,
            &mut AlsFootLockState,
            &mut AlsDynamicTransitionState,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    mut bone_locals: Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
    bone_globals: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    mut telemetry: ParamSet<(
        Query<&mut AlsFootIkTelemetry, With<als_core::character::AlsCharacter>>,
        Query<&mut AlsFootLockTeleportTelemetry, With<als_core::character::AlsCharacter>>,
        Query<&mut AlsFootIkApplyTelemetry, With<als_core::character::AlsCharacter>>,
    )>,
    mut space_states: Query<&mut AlsFootIkSpaceState, With<als_core::character::AlsCharacter>>,
    mut poles: Query<&mut FootIkPoleState, With<als_core::character::AlsCharacter>>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance, &GlobalTransform), With<AlsCharacterAvatar>>,
    players: Query<(
        &bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
        Option<&AlsRootAnimationContext>,
    )>,
    control_rig_constructions: Query<&AlsControlRigConstruction>,
    control_rig_inputs: Query<&AlsControlRigInput>,
    mut control_rig_traces: Query<&mut AlsControlRigStageTrace>,
) {
    let dt = time.delta_secs();
    for (
        entity,
        bones,
        selection,
        targets,
        locomotion,
        lock_input,
        stance,
        settings,
        movement_base,
        mut runtime,
        mut source_rig,
        mut foot_offset_rotation,
        mut lifecycle,
        mut lock_state,
        mut dynamic_transition,
    ) in &mut characters
    {
        let mut pelvis_write_delta_m = 0.0;
        let mut chain_write_delta_m = [0.0; 2];
        let Some((_, _, armature)) = avatars
            .iter()
            .find(|(parent, _, _)| parent.parent() == entity)
        else {
            continue;
        };
        let construction = control_rig_constructions.get(entity).ok();
        let foot_height_m =
            construction.map_or(profile.0.target.foot_height_m, |value| value.foot_height_m);
        let leg_length_m =
            construction.map_or(profile.0.target.leg_length_m, |value| value.leg_length_m);
        let current_character_transform = armature.compute_transform();
        let use_ik_bones = selection.map_or(true, |selection| selection.use_ik_bones);
        if !foot_bones_are_valid(bones, use_ik_bones, &bone_globals) {
            *lock_state = AlsFootLockState::default();
            reset_foot_ik_runtime(
                &mut runtime,
                &mut foot_offset_rotation,
                &mut dynamic_transition,
            );
            *source_rig = CrAlsFootRigState::default();
            lifecycle.previous_character_transform = Some(current_character_transform);
            lifecycle.teleport_remaining = 0.0;
            if let Ok(mut value) = telemetry.p1().get_mut(entity) {
                value.remaining_seconds = 0.0;
            }
            continue;
        }
        let previous_solve_inputs = if let Ok(mut space_state) = space_states.get_mut(entity) {
            let previous = [space_state.left.solve_input, space_state.right.solve_input];
            *space_state = AlsFootIkSpaceState::default();
            previous
        } else { [None; 2] };
        let Ok(mut pole_state) = poles.get_mut(entity) else {
            continue;
        };
        let char_fwd = armature.forward().as_vec3();
        let fallback_pole = Vec3::new(char_fwd.x, 0.0, char_fwd.z)
            .try_normalize()
            .unwrap_or(Vec3::Z);
        let left_ik_curve = weight_override
            .left
            .or_else(|| animation_curve_amount(entity, "FootLeftIk", &avatars, &players))
            .unwrap_or(lock_input.left_ik_amount);
        let right_ik_curve = weight_override
            .right
            .or_else(|| animation_curve_amount(entity, "FootRightIk", &avatars, &players))
            .unwrap_or(lock_input.right_ik_amount);
        let left_ik_amount = left_ik_curve.clamp(0.0, 1.0);
        let right_ik_amount = right_ik_curve.clamp(0.0, 1.0);
        let left_lock_amount = animation_curve_amount(entity, "FootLeftLock", &avatars, &players)
            .unwrap_or(lock_input.left_lock_amount)
            .clamp(0.0, 1.0);
        let right_lock_amount = animation_curve_amount(entity, "FootRightLock", &avatars, &players)
            .unwrap_or(lock_input.right_lock_amount)
            .clamp(0.0, 1.0);
        lock_state.became_valid = !lock_state.valid;
        lock_state.valid = true;
        lock_state.foot_planted_amount = lock_input.foot_planted_amount.clamp(-1.0, 1.0);
        lock_state.feet_crossing_amount = lock_input.feet_crossing_amount.clamp(0.0, 1.0);
        let became_valid = lock_state.became_valid;
        if became_valid {
            reset_foot_ik_runtime(
                &mut runtime,
                &mut foot_offset_rotation,
                &mut dynamic_transition,
            );
            *source_rig = CrAlsFootRigState::default();
        }
        let current_base = movement_base.and_then(|state| {
            state
                .entity
                .and_then(|entity| foot_lock_movement_base(entity, &bone_globals))
        });
        if let Some(previous_character_transform) = lifecycle.previous_character_transform {
            for (side, ik_amount) in [left_ik_amount, right_ik_amount].into_iter().enumerate() {
                let foot_state = if side == 0 { &mut lock_state.left } else { &mut lock_state.right };
                let rebase_lock = lifecycle.teleport_remaining > 0.0 && !became_valid
                    && is_relevant_animation_weight(ik_amount * foot_state.lock_amount);
                refresh_previous_foot_lock_transform(
                    foot_state, &previous_character_transform, &current_character_transform, rebase_lock,
                );
                if rebase_lock {
                    capture_foot_lock_movement_base(foot_state, current_base);
                    if let Ok(mut value) = telemetry.p1().get_mut(entity) {
                        let count = if side == 0 { &mut value.left_rebase_count } else { &mut value.right_rebase_count };
                        *count = count.saturating_add(1);
                    }
                }
            }
        }
        for side in 0..2usize {
            let Ok(target) = bone_globals.get(bones.target(side, use_ik_bones)) else {
                continue;
            };
            let (foot_state, ik_amount, lock_amount) = match side {
                0 => (&mut lock_state.left, left_ik_amount, left_lock_amount),
                _ => (&mut lock_state.right, right_ik_amount, right_lock_amount),
            };
            if became_valid {
                reset_foot_lock_to_target(foot_state, target.translation(), target.rotation());
            }
            let latest_base = foot_state
                .movement_base
                .and_then(|base| foot_lock_movement_base(base.entity, &bone_globals));
            advance_foot_lock_movement_base(foot_state, latest_base);
            refresh_foot_lock(
                foot_state,
                target.translation(),
                target.rotation(),
                ik_amount,
                lock_amount,
                settings.allow_foot_lock,
                lock_input.moving_smooth,
                *locomotion == LocomotionState::Grounded,
                became_valid,
                dt,
            );
            let chain = bones.chain(side);
            let thigh_axis_pelvis_space = Vec3::from(if side == 0 {
                profile.0.target.foot_left_thigh_axis_pelvis_space
            } else {
                profile.0.target.foot_right_thigh_axis_pelvis_space
            });
            if let Ok((_, _, thigh_parent)) = bone_locals.get(chain.thigh)
                && let Ok(pelvis_world) = bone_globals.get(thigh_parent.0)
            {
                source_constrain_foot_lock(
                    foot_state,
                    armature,
                    pelvis_world,
                    thigh_axis_pelvis_space,
                    settings,
                );
            }
            capture_foot_lock_movement_base(foot_state, current_base);
        }

        let mut inputs = [FootSideInput::default(), FootSideInput::default()];
        let mut traces = [FootTraceSample::default(), FootTraceSample::default()];
        for side in 0..2usize {
            let target = bones.target(side, use_ik_bones);
            let trace_target_world = bone_globals
                .get(target)
                .map(|target| WorldPoint(target.translation()))
                .unwrap_or_default();
            let trace = source_foot_trace_sample(
                &spatial,
                armature,
                trace_target_world,
                entity,
                bones.chain(side).foot,
                target,
                if side == 0 {
                    left_ik_curve
                } else {
                    right_ik_curve
                },
                &profile.0.target,
                foot_height_m,
            );
            inputs[side] = FootSideInput {
                raw_offset: if trace.has_ground_hit {
                    trace.raw_offset
                } else {
                    0.0
                },
                pin_request: match side {
                    0 => targets.pin_left,
                    _ => targets.pin_right,
                },
            };
            traces[side] = trace;
        }

        let control_rig_input = control_rig_inputs.get(entity).copied().unwrap_or_default();
        source_rig.left.trace_offset_location_y = if traces[0].has_ground_hit {
            traces[0].raw_offset
        } else {
            0.0
        };
        source_rig.right.trace_offset_location_y = if traces[1].has_ground_hit {
            traces[1].raw_offset
        } else {
            0.0
        };
        let pelvis_target = source_rig
            .left
            .trace_offset_location_y
            .min(source_rig.right.trace_offset_location_y)
            .clamp(
                profile.0.target.pelvis_offset_min_m,
                profile.0.target.pelvis_offset_max_m,
            )
            * control_rig_input.pelvis_offset_amount;
        let pelvis_spring_value = source_rig.pelvis_spring_value;
        source_rig.pelvis_spring_value = spring_damper_float(
            &mut source_rig.pelvis_spring,
            pelvis_spring_value,
            pelvis_target,
            dt,
            profile.0.target.pelvis_offset_frequency_hz,
            profile.0.target.pelvis_offset_damping_ratio,
            0.0,
        );
        source_rig.pelvis_offset_y =
            source_rig.pelvis_spring_value * control_rig_input.pelvis_offset_amount;
        if let Ok(mut trace) = control_rig_traces.get_mut(entity) {
            trace.stage_mask |=
                AlsControlRigStageTrace::FOOT_OFFSET | AlsControlRigStageTrace::PELVIS_OFFSET;
        }

        runtime.update_with_foot_lock_per_side(
            &inputs[0],
            &inputs[1],
            left_ik_amount,
            right_ik_amount,
            settings,
            lock_input.moving_smooth,
            targets.grounded,
            dt,
        );
        let pelvis_entity = bone_locals
            .get(bones.left_thigh)
            .ok()
            .map(|(_, _, child)| child.0);
        let pelvis_before = pelvis_entity.and_then(|entity| {
            bone_locals
                .get(entity)
                .ok()
                .map(|(transform, _, _)| *transform)
        });
        let pelvis_world_offset = apply_pelvis_drop(
            bones.left_thigh,
            source_rig.pelvis_offset_y,
            &mut bone_locals,
            &bone_globals,
        );
        if let (Some(_), Some(before), Some((after, _, _))) = (
            pelvis_entity,
            pelvis_before,
            pelvis_entity.and_then(|entity| bone_locals.get(entity).ok()),
        ) {
            pelvis_write_delta_m = before.translation.distance(after.translation);
        }
        dynamic_transition.update(
            &lock_state,
            lock_input.allow_transitions,
            animation_world_scale(entity, &avatars),
            *stance,
            &TransitionConfig::default(),
        );

        for side in 0..2usize {
            let chain = bones.chain(side);
            let trace = traces[side];
            let (_, pin_weight) = runtime.side(side);
            let foot_state = match side {
                0 => &lock_state.left,
                _ => &lock_state.right,
            };
            let applied_ik_weight = match side {
                0 => left_ik_amount,
                _ => right_ik_amount,
            };
            let offset_normal_armature = foot_offset_rotation.update(
                side,
                trace.normal_armature.0,
                dt,
                profile.0.target.foot_offset_half_life_seconds,
            );
            let offset_normal_world =
                world_direction_from_armature(armature, ArmatureDirection(offset_normal_armature))
                    .0;
            let pose_moving =
                animation_curve_amount(entity, "PoseMoving", &avatars, &players).unwrap_or(0.0);
            let lock_offset = foot_state.final_location - foot_state.target_location;
            let solver_reference_armature =
                armature_point_from_world(armature, WorldPoint(foot_state.final_location));
            let Some(pelvis_location_y) = source_pelvis_location_armature_y(
                chain.thigh,
                pelvis_world_offset,
                armature,
                &mut bone_locals,
                &bone_globals,
            ) else {
                continue;
            };
            let Some(thigh_world) = bone_globals.get(chain.thigh).ok() else {
                continue;
            };
            let thigh_location_armature = armature_point_from_world(
                armature,
                WorldPoint(thigh_world.translation() + pelvis_world_offset),
            )
            .0;
            let pelvis_offset_y = source_rig.pelvis_offset_y;
            let side_state = if side == 0 {
                &mut source_rig.left
            } else {
                &mut source_rig.right
            };
            let target_armature = ArmaturePoint(source_apply_foot_offset_location(
                side_state,
                solver_reference_armature.0,
                side_state.trace_offset_location_y,
                pelvis_offset_y,
                pelvis_location_y,
                thigh_location_armature,
                pose_moving,
                dt,
                &profile.0.target,
                leg_length_m,
            ));
            let target_world = world_point_from_armature(armature, target_armature);
            let foot_parent = bone_locals
                .get(chain.foot)
                .ok()
                .map(|(_, foot_global, child)| (foot_global.translation(), child.0));
            let side_pole_state = match side {
                0 => &mut pole_state.left,
                _ => &mut pole_state.right,
            };
            let (pole_location, pole_direction) = match (
                bone_globals.get(chain.thigh),
                bone_globals.get(chain.calf),
                foot_parent,
            ) {
                (Ok(thigh), Ok(calf), Some((ankle, _))) => {
                    let hip = thigh.translation() + pelvis_world_offset;
                    let knee = calf.translation() + pelvis_world_offset;
                    let ankle = ankle + pelvis_world_offset;
                    let location =
                        side_pole_state.update(armature, hip, knee, ankle, dt, &profile.0.target);
                    (location, location - hip)
                }
                _ => (
                    fallback_pole * profile.0.target.foot_pole_distance_m,
                    fallback_pole,
                ),
            };
            let (target_parent_local, target_bone_local, parent_ready) = foot_parent
                .and_then(|(_, parent)| {
                    bone_globals.get(parent).ok().map(|parent_world| {
                        (
                            parent_point_from_world(parent_world, target_world).0,
                            bone_local_point_from_world(parent_world, target_world).0,
                            true,
                        )
                    })
                })
                .unwrap_or((Vec3::ZERO, Vec3::ZERO, false));
            let space_side = AlsFootIkSpaceSide {
                valid: trace.trace_target_world.0.is_finite()
                    && target_world.0.is_finite()
                    && target_armature.0.is_finite()
                    && trace.normal_world.0.is_finite()
                    && trace.normal_armature.0.is_finite()
                    && parent_ready,
                has_ground_hit: trace.has_ground_hit,
                trace_target_world: trace.trace_target_world.0,
                hit_world: if trace.has_ground_hit {
                    trace.hit_world.0
                } else {
                    Vec3::ZERO
                },
                solver_reference_world: foot_state.final_location,
                solver_reference_armature_local: solver_reference_armature.0,
                target_world: target_world.0,
                resolved_target_world: target_world.0,
                resolved_knee_world: Vec3::ZERO,
                pole_location_world: pole_location,
                target_armature_local: target_armature.0,
                target_parent_local,
                target_bone_local,
                normal_world: trace.normal_world.0,
                normal_armature_local: trace.normal_armature.0,
                solve_input: None,
                previous_solve_input: previous_solve_inputs[side],
                raw_offset: if trace.has_ground_hit {
                    trace.raw_offset
                } else {
                    0.0
                },
            };
            if let Ok(mut space_state) = space_states.get_mut(entity) {
                if side == 0 {
                    space_state.left = space_side;
                } else {
                    space_state.right = space_side;
                }
            }
            if telemetry_enabled.0
                && let (Ok(thigh), Ok(calf), Some((ankle, _))) = (
                    bone_globals.get(chain.thigh),
                    bone_globals.get(chain.calf),
                    foot_parent,
                )
            {
                let hip = thigh.translation();
                let knee = calf.translation();
                let upper_length = knee.distance(hip);
                let lower_length = ankle.distance(knee);
                let target_axis = (target_world.0 - hip).normalize_or_zero();
                let pole_sign = target_axis.cross(knee - hip).dot(pole_direction);
                let entry = AlsFootIkTelemetrySide {
                    valid: space_side.valid
                        && upper_length.is_finite()
                        && lower_length.is_finite()
                        && pole_sign.is_finite(),
                    target_world: target_world.0,
                    target_armature_local: target_armature.0,
                    target_parent_local,
                    reference_chain_error: (target_world.0.distance(hip)
                        - (upper_length + lower_length) * profile.0.target.max_leg_stretch_ratio)
                        .max(0.0),
                    pole_sign,
                    ik_weight: applied_ik_weight,
                    pin_weight,
                };
                if let Ok(mut telemetry) = telemetry.p0().get_mut(entity) {
                    if side == 0 {
                        telemetry.left = entry;
                    } else {
                        telemetry.right = entry;
                    }
                }
            }
            let source_offset_y = if side == 0 {
                source_rig.left.location_offset_y
            } else {
                source_rig.right.location_offset_y
            };
            let (primary_axis, secondary_axis) = source_foot_ik_axes(side, &profile.0.target);
            let armature_up = world_direction_from_armature(armature, ArmatureDirection(Vec3::Y)).0;
            let has_slope_rotation = offset_normal_world.dot(armature_up) < 1.0 - 1e-4;
            let has_lock_rotation = foot_state.lock_amount > 1e-3;
            if applied_ik_weight <= 1e-4
                || (source_offset_y.abs() < 1e-4
                    && pin_weight < 1e-3
                    && lock_offset.length_squared() < 1e-8
                    && !has_slope_rotation
                    && !has_lock_rotation)
            {
                continue;
            }
            let before_chain = foot_ik_chain_locals(&chain, &mut bone_locals);
            let resolved_target = apply_chain_target(
                &chain,
                target_world.0,
                pole_location,
                foot_state.final_rotation,
                applied_ik_weight,
                pelvis_world_offset,
                primary_axis,
                secondary_axis,
                armature,
                offset_normal_armature,
                side,
                pose_moving,
                &profile.0.target,
                &mut bone_locals,
                &bone_globals,
            );
            if let (Some(before), Some(after)) =
                (before_chain, foot_ik_chain_locals(&chain, &mut bone_locals))
            {
                let delta = foot_ik_max_translation_delta(before, after);
                chain_write_delta_m[side] = delta;
            }
            if let Some(resolved_target) = resolved_target
                && let Ok(mut space_state) = space_states.get_mut(entity)
            {
                let side = if side == 0 { &mut space_state.left } else { &mut space_state.right };
                side.resolved_target_world = resolved_target.target_world;
                side.resolved_knee_world = resolved_target.knee_world;
                side.solve_input = Some(resolved_target.input);
            }
        }
        if let Ok(mut trace) = control_rig_traces.get_mut(entity) {
            trace.stage_mask |= AlsControlRigStageTrace::FOOT_IK;
        }
        lifecycle.previous_character_transform = Some(current_character_transform);
        lifecycle.teleport_remaining = (lifecycle.teleport_remaining - dt.max(0.0)).max(0.0);
        if let Ok(mut value) = telemetry.p1().get_mut(entity) {
            value.remaining_seconds = lifecycle.teleport_remaining;
        }
        if let Ok(mut value) = telemetry.p2().get_mut(entity) {
            value.pelvis_write_delta_m = pelvis_write_delta_m;
            value.left_chain_write_delta_m = chain_write_delta_m[0];
            value.right_chain_write_delta_m = chain_write_delta_m[1];
        }
    }
}

fn foot_ik_post_apply_telemetry_system(
    characters: Query<
        (
            Entity,
            &AlsFootBones,
            &AlsFootIkSpaceState,
            &FootIkPoseBaseState,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    bone_globals: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    mut telemetry: Query<&mut AlsFootIkApplyTelemetry, With<als_core::character::AlsCharacter>>,
) {
    for (entity, bones, spaces, pose_base) in &characters {
        let Ok(mut entry) = telemetry.get_mut(entity) else {
            continue;
        };
        entry.restore_translation_delta_m = pose_base.restore_translation_delta_m;
        entry.restore_rotation_delta_radians = pose_base.restore_rotation_delta_radians;
        entry.solver_translation_delta_m = pose_base.solver_translation_delta_m;
        entry.solver_rotation_delta_radians = pose_base.solver_rotation_delta_radians;
        for side in 0..2usize {
            let chain = bones.chain(side);
            let space = match side {
                0 => spaces.left,
                _ => spaces.right,
            };
            let applied_weight = space.solve_input.map_or(0.0, |input| input.weight);
            let value = match (
                bone_globals.get(chain.thigh),
                bone_globals.get(chain.calf),
                bone_globals.get(chain.foot),
            ) {
                (Ok(hip), Ok(knee), Ok(foot)) => {
                    let pole = (space.pole_location_world - knee.translation()).normalize_or_zero();
                    let target_axis =
                        (space.resolved_target_world - hip.translation()).normalize_or_zero();
                    let pole_sign = target_axis
                        .cross(knee.translation() - hip.translation())
                        .dot(pole);
                    AlsFootIkApplyTelemetrySide {
                        valid: space.valid
                            && space.resolved_target_world.is_finite()
                            && foot.translation().is_finite()
                            && pole_sign.is_finite()
                            && pole.length_squared() > 1e-8,
                        target_world: space.resolved_target_world,
                        effector_world: foot.translation(),
                        hip_world: hip.translation(),
                        knee_world: knee.translation(),
                        ankle_world: foot.translation(),
                        hip_error: 0.0,
                        knee_error: knee.translation().distance(space.resolved_knee_world),
                        effector_error: foot.translation().distance(space.resolved_target_world),
                        pole_sign,
                        applied_weight,
                    }
                }
                _ => AlsFootIkApplyTelemetrySide::default(),
            };
            if side == 0 {
                entry.left = value;
            } else {
                entry.right = value;
            }
        }
    }
}

fn foot_lock_movement_base(
    entity: Entity,
    transforms: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) -> Option<AlsFootLockMovementBase> {
    let transform = transforms.get(entity).ok()?;
    Some(AlsFootLockMovementBase {
        entity,
        translation: transform.translation(),
        rotation: transform.rotation(),
    })
}

fn animation_curve_amount(
    character: Entity,
    name: &str,
    avatars: &Query<(&ChildOf, &AnimatedSceneInstance, &GlobalTransform), With<AlsCharacterAvatar>>,
    players: &Query<(
        &bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
        Option<&AlsRootAnimationContext>,
    )>,
) -> Option<f32> {
    let (_, instance, _) = avatars
        .iter()
        .find(|(parent, _, _)| parent.parent() == character)?;
    let (player, context) = players.get(instance.player_entity()).ok()?;
    context.and_then(|context| context.f32(name)).or_else(|| {
        player
            .get_outputs()
            .get(name)
            .and_then(|value| match value {
                bevy_animation_graph::core::edge_data::DataValue::F32(value)
                    if value.is_finite() =>
                {
                    Some(*value)
                }
                _ => None,
            })
    })
}

fn animation_world_scale(
    character: Entity,
    avatars: &Query<(&ChildOf, &AnimatedSceneInstance, &GlobalTransform), With<AlsCharacterAvatar>>,
) -> f32 {
    avatars
        .iter()
        .find(|(parent, _, _)| parent.parent() == character)
        .map(|(_, _, transform)| transform.to_scale_rotation_translation().0.z.abs())
        .unwrap_or(1.0)
        .max(f32::EPSILON)
}

fn apply_chain_target(
    chain: &als_core::foot_ik::FootChain,
    target_world: Vec3,
    pole_location: Vec3,
    foot_target_rotation: Quat,
    weight: f32,
    pelvis_world_offset: Vec3,
    primary_axis: Vec3,
    secondary_axis: Vec3,
    armature: &GlobalTransform,
    offset_normal_armature: Vec3,
    side: usize,
    pose_moving: f32,
    profile: &CrAlsTargetProfile,
    bone_locals: &mut Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
    bone_globals: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) -> Option<ResolvedFootTarget> {
    let weight = weight.clamp(0.0, 1.0);
    if weight <= 1e-4 {
        return None;
    }
    let (Ok(_), Ok(_), Ok(_)) = (
        bone_locals.get(chain.thigh),
        bone_locals.get(chain.calf),
        bone_locals.get(chain.foot),
    ) else {
        return None;
    };
    let (Ok(thigh_g), Ok(calf_g), Ok(foot_g)) = (
        bone_globals.get(chain.thigh),
        bone_globals.get(chain.calf),
        bone_globals.get(chain.foot),
    ) else {
        return None;
    };

    let hip = pelvis_adjusted_position(thigh_g.translation(), pelvis_world_offset);
    let knee = pelvis_adjusted_position(calf_g.translation(), pelvis_world_offset);
    let ankle_before_pelvis_drop = foot_g.translation();
    let ankle = pelvis_adjusted_position(ankle_before_pelvis_drop, pelvis_world_offset);

    let upper_len = (knee - hip).length();
    let lower_len = (ankle - knee).length();

    let target = target_world;

    let Some(solve) = solve_basic_two_bone_ik(
        hip,
        thigh_g.rotation(),
        knee,
        target,
        pole_location,
        upper_len,
        lower_len,
        primary_axis,
        secondary_axis,
        1.0,
    ) else {
        return None;
    };
    let (thigh_scale, _, _) = thigh_g.to_scale_rotation_translation();
    let (calf_scale, _, _) = calf_g.to_scale_rotation_translation();
    let (foot_scale, _, _) = foot_g.to_scale_rotation_translation();
    let current_thigh = Transform {
        translation: hip,
        rotation: thigh_g.rotation(),
        scale: thigh_scale,
    };
    let current_calf = Transform {
        translation: knee,
        rotation: calf_g.rotation(),
        scale: calf_scale,
    };
    let current_foot = Transform {
        translation: ankle,
        rotation: foot_g.rotation(),
        scale: foot_scale,
    };
    let mut writeback = source_basic_ik_writeback(
        current_thigh,
        current_calf,
        current_foot,
        solve,
        foot_target_rotation,
        weight,
    )?;
    let foot_rotation = world_rotation_from_armature(
        armature,
        ArmatureRotation(source_foot_rotation(
            armature_rotation_from_world(armature, WorldRotation(writeback.foot.rotation)).0,
            armature_rotation_from_world(armature, WorldRotation(writeback.calf.rotation)).0,
            armature_rotation_from_world(armature, WorldRotation(foot_target_rotation)).0,
            offset_normal_armature,
            side,
            pose_moving,
            profile,
        )),
    ).0;
    writeback.foot.rotation = blend_ik_rotation(writeback.foot.rotation, foot_rotation, weight);
    let thigh_parent = bone_locals.get(chain.thigh).ok()?.2.0;
    let calf_parent = bone_locals.get(chain.calf).ok()?.2.0;
    let foot_parent = bone_locals.get(chain.foot).ok()?.2.0;
    if calf_parent != chain.thigh || foot_parent != chain.calf {
        return None;
    }
    let pelvis_global = bone_globals.get(thigh_parent).ok()?;
    let (pelvis_scale, pelvis_rotation, pelvis_translation) =
        pelvis_global.to_scale_rotation_translation();
    let pelvis = Transform {
        translation: pelvis_translation + pelvis_world_offset,
        rotation: pelvis_rotation,
        scale: pelvis_scale,
    };
    let thigh_local_target = source_local_transform(&pelvis, &writeback.thigh)?;
    let calf_local_target = source_local_transform(&writeback.thigh, &writeback.calf)?;
    let foot_local_target = source_local_transform(&writeback.calf, &writeback.foot)?;
    let Ok((mut thigh_local, _, _)) = bone_locals.get_mut(chain.thigh) else {
        return None;
    };
    *thigh_local = thigh_local_target;
    let Ok((mut calf_local, _, _)) = bone_locals.get_mut(chain.calf) else {
        return None;
    };
    *calf_local = calf_local_target;
    let Ok((mut foot_local, _, _)) = bone_locals.get_mut(chain.foot) else {
        return None;
    };
    *foot_local = foot_local_target;
    let trs = |value: Transform| (value.translation.to_array(), value.rotation.to_array(), value.scale.to_array());
    Some(ResolvedFootTarget {
        target_world: writeback.foot.translation,
        knee_world: writeback.calf.translation,
        input: FootIkSolveInput {
            armature_world_trs: trs(armature.compute_transform()),
            thigh_world_trs: trs(current_thigh), calf_world_trs: trs(current_calf), foot_world_trs: trs(current_foot),
            target_world: target.to_array(), target_rotation_world: foot_target_rotation.to_array(), pole_world: pole_location.to_array(),
            primary_axis: primary_axis.to_array(), secondary_axis: secondary_axis.to_array(), upper_length: upper_len, lower_length: lower_len, weight,
        },
    })
}

fn source_foot_ik_axes(side: usize, profile: &CrAlsTargetProfile) -> (Vec3, Vec3) {
    if side == 0 {
        (
            Vec3::from(profile.foot_left_primary_axis),
            Vec3::from(profile.foot_left_secondary_axis),
        )
    } else {
        (
            Vec3::from(profile.foot_right_primary_axis),
            Vec3::from(profile.foot_right_secondary_axis),
        )
    }
}

fn source_local_rotation(parent_world_rotation: Quat, world_rotation: Quat) -> Quat {
    world_rotation_to_local(parent_world_rotation, world_rotation)
}

fn pelvis_adjusted_position(position: Vec3, pelvis_world_offset: Vec3) -> Vec3 {
    position + pelvis_world_offset
}

fn normalized_ground_normal(normal: Vec3) -> Vec3 {
    normal.try_normalize().unwrap_or(Vec3::Y)
}

fn calf_space_angles_degrees(rotation: Quat) -> Vec3 {
    let rotation = rotation.normalize();
    let source_x = -rotation.x;
    let source_y = -rotation.z;
    let source_z = -rotation.y;
    let source_w = rotation.w;
    let sin_pitch = 2.0 * (source_w * source_y - source_z * source_x);
    let pitch = if sin_pitch.abs() >= 1.0 {
        std::f32::consts::FRAC_PI_2.copysign(sin_pitch)
    } else {
        sin_pitch.asin()
    };
    let yaw = (2.0 * (source_w * source_z + source_x * source_y))
        .atan2(1.0 - 2.0 * (source_y * source_y + source_z * source_z));
    let roll = (2.0 * (source_w * source_x + source_y * source_z))
        .atan2(1.0 - 2.0 * (source_x * source_x + source_y * source_y));
    Vec3::new(-pitch.to_degrees(), yaw.to_degrees(), -roll.to_degrees())
}

fn calf_space_rotation_from_angles(
    pitch_degrees: f32,
    yaw_degrees: f32,
    roll_degrees: f32,
) -> Quat {
    let pitch = pitch_degrees.to_radians();
    let yaw = yaw_degrees.to_radians();
    let roll = roll_degrees.to_radians();
    let source =
        Quat::from_rotation_z(yaw) * Quat::from_rotation_y(-pitch) * Quat::from_rotation_x(-roll);
    Quat::from_xyzw(-source.x, -source.z, -source.y, source.w)
}

fn source_constraint_target_angle(current: f32, target: f32, minimum: f32, maximum: f32) -> f32 {
    target.clamp(
        current.min(minimum).min(maximum),
        current.max(minimum).max(maximum),
    )
}

fn source_foot_rotation(
    current_foot_armature: Quat,
    calf_armature: Quat,
    foot_target_armature: Quat,
    offset_normal_armature: Vec3,
    side: usize,
    pose_moving: f32,
    profile: &CrAlsTargetProfile,
) -> Quat {
    let initial_calf_space = Quat::from_array(if side == 0 {
        profile.foot_left_initial_rotation_calf_space
    } else {
        profile.foot_right_initial_rotation_calf_space
    });
    let offset_rotation =
        Quat::from_rotation_arc(Vec3::Y, normalized_ground_normal(offset_normal_armature));
    let current_calf_space =
        calf_armature.inverse() * current_foot_armature * initial_calf_space.inverse();
    let target_calf_space = calf_armature.inverse()
        * (offset_rotation * foot_target_armature)
        * initial_calf_space.inverse();
    let current = calf_space_angles_degrees(current_calf_space);
    let target = calf_space_angles_degrees(target_calf_space);
    let moving = pose_moving.clamp(0.0, 1.0);
    let swing2_min = profile.foot_swing2_min_idle_degrees.lerp(0.0, moving);
    let swing2_max = profile.foot_swing2_max_idle_degrees.lerp(0.0, moving);
    let final_rotation = calf_space_rotation_from_angles(
        source_constraint_target_angle(current.x, target.x, swing2_min, swing2_max),
        source_constraint_target_angle(
            current.y,
            target.y,
            profile.foot_swing1_min_degrees,
            profile.foot_swing1_max_degrees,
        ),
        source_constraint_target_angle(current.z, target.z, 0.0, 0.0),
    );
    (calf_armature * (final_rotation * initial_calf_space)).normalize()
}

fn apply_pelvis_drop(
    thigh: Entity,
    drop: f32,
    bone_locals: &mut Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
    bone_globals: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) -> Vec3 {
    if drop.abs() < 1e-4 {
        return Vec3::ZERO;
    }
    let (pelvis_entity, pelvis_parent) = match bone_locals.get(thigh) {
        Ok((_, _, c)) => match bone_locals.get(c.0) {
            Ok((_, _, pelvis_child)) => (c.0, pelvis_child.0),
            Err(_) => return Vec3::ZERO,
        },
        Err(_) => return Vec3::ZERO,
    };
    if let Ok((mut pelvis_local, _, _)) = bone_locals.get_mut(pelvis_entity) {
        pelvis_local.translation.y += drop;
    }
    bone_globals
        .get(pelvis_parent)
        .map(|parent| parent.rotation() * Vec3::Y * drop)
        .unwrap_or(Vec3::Y * drop)
}

fn source_pelvis_location_armature_y(
    thigh: Entity,
    pelvis_world_offset: Vec3,
    armature_world: &GlobalTransform,
    bone_locals: &mut Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
    bone_globals: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) -> Option<f32> {
    let (_, _, thigh_parent) = bone_locals.get(thigh).ok()?;
    let pelvis = bone_globals.get(thigh_parent.parent()).ok()?;
    Some(
        armature_point_from_world(
            armature_world,
            WorldPoint(pelvis.translation() + pelvis_world_offset),
        )
        .0
        .y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control_rig_apply::{CR_ALS_RUNTIME_PROFILE, CrAlsRuntimeProfileDocument};

    #[test]
    fn pole_damper_keeps_rig_space_under_character_motion() {
        let profile: CrAlsRuntimeProfileDocument =
            serde_json::from_str(CR_ALS_RUNTIME_PROFILE).unwrap();
        let mut reference = SourcePoleState::default();
        let mut moving = SourcePoleState::default();
        for frame in 0..3 {
            let armature = GlobalTransform::from(Transform {
                translation: Vec3::new(frame as f32 * 4.0, 0.0, frame as f32 * -3.0),
                rotation: Quat::from_rotation_y(frame as f32 * 0.4),
                scale: Vec3::splat(1.0 + frame as f32 * 0.1),
            });
            let points = [
                Vec3::Y,
                Vec3::new(0.1 + frame as f32 * 0.02, 0.5, 0.0),
                Vec3::ZERO,
            ];
            let expected = reference.update(
                &GlobalTransform::IDENTITY,
                points[0],
                points[1],
                points[2],
                0.016,
                &profile.target,
            );
            let world = points.map(|point| armature.to_matrix().transform_point3(point));
            let actual = moving.update(
                &armature,
                world[0],
                world[1],
                world[2],
                0.016,
                &profile.target,
            );
            assert!(
                actual.distance(armature.to_matrix().transform_point3(expected)) < 1e-5,
                "pole space frame={frame}"
            );
        }
    }

    #[test]
    fn source_local_rotation_uses_the_updated_parent() {
        let calf_old = Quat::from_rotation_y(30.0_f32.to_radians());
        let calf_new = Quat::from_rotation_y(50.0_f32.to_radians());
        let foot_target = Quat::IDENTITY;
        let corrected_local = source_local_rotation(calf_new, foot_target);
        let legacy_local = source_local_rotation(calf_old, foot_target);
        let corrected_world = calf_new * corrected_local;
        let legacy_world = calf_new * legacy_local;
        let corrected_alignment = corrected_world.dot(foot_target).abs();
        let legacy_alignment = legacy_world.dot(foot_target).abs();
        let passed = corrected_alignment > 0.99999 && legacy_alignment < 0.99;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.40][VERDICT] corrected_alignment={corrected_alignment:.6} legacy_alignment={legacy_alignment:.6} -> {verdict}"
        );
        assert!(passed);
    }

    #[test]
    fn pelvis_drop_is_applied_before_solving_the_leg_target() {
        let hip = Vec3::ZERO;
        let knee = Vec3::new(0.0, -0.5, 0.0);
        let ankle = Vec3::new(0.0, -1.0, 0.0);
        let target = ankle;
        let pelvis_drop = Vec3::new(0.0, -0.1, 0.0);
        let solve = als_ext::ik::solve_two_bone_ik(
            pelvis_adjusted_position(hip, pelvis_drop),
            pelvis_adjusted_position(knee, pelvis_drop),
            pelvis_adjusted_position(ankle, pelvis_drop),
            target,
            0.5,
            0.5,
            Vec3::Z,
        )
        .unwrap();
        let corrected_final = target;
        let legacy_final = target + pelvis_drop;
        let corrected_error = corrected_final.distance(target);
        let legacy_error = legacy_final.distance(target);
        let passed = solve.knee_position.is_finite()
            && corrected_error < 1e-6
            && (legacy_error - 0.1).abs() < 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.41][VERDICT] corrected_error={corrected_error:.3} legacy_error={legacy_error:.3} -> {verdict}"
        );
        assert!(passed);
    }

    #[test]
    fn source_trace_uses_component_vertical_coordinate_and_floor_limit() {
        let profile: CrAlsRuntimeProfileDocument =
            serde_json::from_str(CR_ALS_RUNTIME_PROFILE).unwrap();
        let normal = Vec3::new(0.0, 0.94, 0.34).normalize();
        let first = source_trace_offset(
            Vec3::new(0.18, -1.0, -0.24),
            normal,
            &profile.target,
            profile.target.foot_height_m,
        )
        .unwrap();
        let second = source_trace_offset(
            Vec3::new(-4.0, -1.0, 6.0),
            normal,
            &profile.target,
            profile.target.foot_height_m,
        )
        .unwrap();
        let expected =
            -1.0 + profile.target.foot_height_m / normal.y - profile.target.foot_height_m;
        assert!((first - expected).abs() < 1e-6);
        assert!((second - expected).abs() < 1e-6);
        assert!(
            source_trace_offset(
                Vec3::ZERO,
                Vec3::X,
                &profile.target,
                profile.target.foot_height_m,
            )
            .is_none()
        );
    }

    #[test]
    fn source_basic_ik_uses_per_side_axes() {
        let profile: CrAlsRuntimeProfileDocument =
            serde_json::from_str(CR_ALS_RUNTIME_PROFILE).unwrap();
        let (left_primary, left_secondary) = source_foot_ik_axes(0, &profile.target);
        let (right_primary, right_secondary) = source_foot_ik_axes(1, &profile.target);
        assert!(left_primary.dot(Vec3::NEG_X) > 0.999_99);
        assert!(left_secondary.dot(Vec3::Z) > 0.999_99);
        assert!(right_primary.dot(Vec3::X) > 0.999_99);
        assert!(right_secondary.dot(Vec3::NEG_Z) > 0.999_99);
    }

    #[test]
    fn source_basic_ik_writeback_updates_global_child_positions_after_blending() {
        let current_thigh = Transform::IDENTITY;
        let current_calf = Transform::from_translation(Vec3::X * 0.45);
        let current_foot = Transform::from_translation(Vec3::X * 0.90);
        let solve = BasicTwoBoneSolve {
            hip_rotation_world: Quat::from_rotation_z(0.35),
            knee_rotation_world: Quat::from_rotation_z(-0.20),
            knee_position: Vec3::new(0.31, -0.29, 0.16),
            effector_position: Vec3::new(0.67, -0.52, 0.05),
        };
        let full = source_basic_ik_writeback(
            current_thigh,
            current_calf,
            current_foot,
            solve,
            Quat::from_rotation_y(0.24),
            1.0,
        )
        .unwrap();
        let half = source_basic_ik_writeback(
            current_thigh,
            current_calf,
            current_foot,
            solve,
            Quat::from_rotation_y(0.24),
            0.5,
        )
        .unwrap();
        assert!(full.calf.translation.distance(solve.knee_position) < 1e-6);
        assert!(full.foot.translation.distance(solve.effector_position) < 1e-6);
        assert!(half.calf.translation.distance(current_calf.translation) > 1e-3);
        assert!(half.foot.translation.distance(current_foot.translation) > 1e-3);
        let calf_local = source_local_transform(&half.thigh, &half.calf).unwrap();
        let foot_local = source_local_transform(&half.calf, &half.foot).unwrap();
        assert!(
            source_transform_point(&half.thigh, calf_local.translation)
                .distance(half.calf.translation)
                < 1e-6
        );
        assert!(
            source_transform_point(&half.calf, foot_local.translation)
                .distance(half.foot.translation)
                < 1e-6
        );
    }

    #[test]
    fn pose_base_restores_unwritten_animation_transforms_before_ik() {
        let entity = Entity::from_raw_u32(17).unwrap();
        let base = Transform::from_xyz(-0.03, 0.92, 0.01);
        let mut state = FootIkPoseBaseState::default();
        let captured = restore_foot_ik_animation_transform(&mut state, 0, entity, base);
        let applied = Transform::from_xyz(-0.03, 0.72, 0.01);
        state.previous_applied[0] = applied;
        let restored = restore_foot_ik_animation_transform(&mut state, 0, entity, applied);
        let animated = Transform::from_xyz(-0.03, 0.88, 0.01);
        let accepted = restore_foot_ik_animation_transform(&mut state, 0, entity, animated);
        assert!(transforms_approximately_equal(captured, base));
        assert!(transforms_approximately_equal(restored, base));
        assert!(transforms_approximately_equal(accepted, animated));
        assert!(transforms_approximately_equal(
            state.animation_base[0],
            animated
        ));
    }

    #[test]
    fn foot_rotation_reads_the_blended_ik_calf_before_local_writeback() {
        use bevy::ecs::system::SystemState;

        let profile: CrAlsRuntimeProfileDocument =
            serde_json::from_str(CR_ALS_RUNTIME_PROFILE).unwrap();
        for weight in [0.5, 1.0] {
            let mut world = World::new();
            let parent = world.spawn((Transform::IDENTITY, GlobalTransform::IDENTITY)).id();
            let current_thigh = Transform::IDENTITY;
            let current_calf = Transform::from_translation(Vec3::X * 0.45);
            let current_foot = Transform::from_translation(Vec3::X * 0.90)
                .with_rotation(Quat::from_rotation_z(0.1));
            let thigh = world.spawn((current_thigh, GlobalTransform::from(current_thigh), ChildOf(parent))).id();
            let calf = world.spawn((current_calf, GlobalTransform::from(current_calf), ChildOf(thigh))).id();
            let foot = world.spawn((Transform::from_translation(Vec3::X * 0.45).with_rotation(current_foot.rotation), GlobalTransform::from(current_foot), ChildOf(calf))).id();
            let chain = als_core::foot_ik::FootChain { thigh, calf, foot };
            let target = Vec3::new(0.67, -0.52, 0.05);
            let pole = Vec3::new(0.0, -1.0, 0.5);
            let rotation = Quat::from_rotation_y(0.8);
            let normal = Vec3::new(0.3, 0.94, 0.0).normalize();
            let (primary, secondary) = source_foot_ik_axes(1, &profile.target);
            let solve = solve_basic_two_bone_ik(
                current_thigh.translation, current_thigh.rotation, current_calf.translation,
                target, pole, 0.45, 0.45, primary, secondary, 1.0,
            ).unwrap();
            let basic = source_basic_ik_writeback(
                current_thigh, current_calf, current_foot, solve, rotation, weight,
            ).unwrap();
            let expected = blend_ik_rotation(
                basic.foot.rotation,
                source_foot_rotation(basic.foot.rotation, basic.calf.rotation, rotation, normal, 1, 0.0, &profile.target),
                weight,
            );
            let mut queries = SystemState::<(
                Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
                Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
            )>::new(&mut world);
            let (mut locals, globals) = queries.get_mut(&mut world).unwrap();
            assert!(apply_chain_target(
                &chain, target, pole, rotation, weight, Vec3::ZERO, primary, secondary,
                &GlobalTransform::IDENTITY, normal, 1, 0.0, &profile.target, &mut locals, &globals,
            ).is_some());
            let actual = locals.get(thigh).unwrap().0.rotation
                * locals.get(calf).unwrap().0.rotation * locals.get(foot).unwrap().0.rotation;
            assert!(actual.dot(expected).abs() > 1.0 - 1e-5, "foot rotation weight={weight}");
        }
    }

    #[test]
    fn foot_rotation_consumes_slope_normal_and_full_lock_rotation() {
        let profile: CrAlsRuntimeProfileDocument =
            serde_json::from_str(CR_ALS_RUNTIME_PROFILE).unwrap();
        let slope_normal = Vec3::new(0.5, 0.866_025_4, 0.0);
        let mut normal_state = FootOffsetRotationState::default();
        let initialized_normal = normal_state.update(0, slope_normal, 1.0 / 60.0, 0.1);
        let initialization_alignment = initialized_normal.dot(slope_normal);
        let initial = Quat::from_array(profile.target.foot_left_initial_rotation_calf_space);
        let flat = source_foot_rotation(
            initial,
            Quat::IDENTITY,
            initial,
            Vec3::Y,
            0,
            0.0,
            &profile.target,
        );
        let slope = source_foot_rotation(
            initial,
            Quat::IDENTITY,
            initial,
            slope_normal,
            0,
            0.0,
            &profile.target,
        );
        let flat_alignment = flat.dot(initial).abs();
        let slope_finite = slope.is_finite();
        let passed =
            initialization_alignment > 0.999_99 && flat_alignment > 0.999_99 && slope_finite;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.42][VERDICT] initialization_alignment={initialization_alignment:.6} flat_alignment={flat_alignment:.6} slope_finite={slope_finite} -> {verdict}"
        );
        assert!(passed);
    }

    #[test]
    fn foot_lock_lifecycle_rebases_teleports_and_resets_missing_bones() {
        let previous_character_transform = Transform::from_xyz(2.0, 0.5, -1.0)
            .with_rotation(Quat::from_rotation_y(20.0_f32.to_radians()));
        let current_character_transform = Transform::from_xyz(-3.0, 1.5, 4.0)
            .with_rotation(Quat::from_rotation_y(-45.0_f32.to_radians()));
        let local_location = Vec3::new(0.2, -0.9, 0.3);
        let local_rotation = Quat::from_euler(
            EulerRot::YXZ,
            10.0_f32.to_radians(),
            -5.0_f32.to_radians(),
            7.0_f32.to_radians(),
        );
        let mut foot_state = als_core::foot_ik::AlsFootLockFootState {
            lock_amount: 1.0,
            lock_location: previous_character_transform
                .to_matrix()
                .transform_point3(local_location),
            lock_rotation: previous_character_transform.rotation * local_rotation,
            final_location: previous_character_transform
                .to_matrix()
                .transform_point3(local_location),
            final_rotation: previous_character_transform.rotation * local_rotation,
            ..Default::default()
        };
        refresh_previous_foot_lock_transform(
            &mut foot_state,
            &previous_character_transform,
            &current_character_transform,
            true,
        );
        let expected_location = current_character_transform
            .to_matrix()
            .transform_point3(local_location);
        let expected_rotation = current_character_transform.rotation * local_rotation;
        let teleport_location_error = foot_state.lock_location.distance(expected_location);
        let teleport_rotation_alignment = foot_state.lock_rotation.dot(expected_rotation).abs();

        let target_location = Vec3::new(8.0, 0.4, -2.0);
        let target_rotation = Quat::from_rotation_y(60.0_f32.to_radians());
        reset_foot_lock_to_target(&mut foot_state, target_location, target_rotation);
        let recovery_fresh = foot_state.lock_amount == 0.0
            && foot_state.lock_location == target_location
            && foot_state.final_location == target_location
            && foot_state.lock_rotation.dot(target_rotation).abs() > 0.999_99
            && foot_state.movement_base.is_none();

        let mut app = App::new();
        app.add_message::<AlsFootLockTeleported>().add_systems(
            Update,
            (record_foot_lock_teleports, invalidate_missing_foot_bones),
        );
        let entity = app
            .world_mut()
            .spawn((
                als_core::character::AlsCharacter,
                AlsFootLockState {
                    valid: true,
                    became_valid: true,
                    ..Default::default()
                },
                AlsFootIkRuntime::default(),
                FootOffsetRotationState {
                    normals: [Vec3::X, Vec3::Z],
                    initialized: [true, true],
                },
                AlsDynamicTransitionState {
                    frames_remaining: 2,
                    ..Default::default()
                },
                FootLockLifecycleState {
                    previous_character_transform: Some(previous_character_transform),
                    teleport_remaining: FOOT_LOCK_TELEPORT_WINDOW_SECONDS,
                },
            ))
            .id();
        app.update();
        let lock_state = app.world().get::<AlsFootLockState>(entity).unwrap();
        let runtime = app.world().get::<AlsFootIkRuntime>(entity).unwrap();
        let offset_rotation = app.world().get::<FootOffsetRotationState>(entity).unwrap();
        let dynamic_transition = app
            .world()
            .get::<AlsDynamicTransitionState>(entity)
            .unwrap();
        let lifecycle = app.world().get::<FootLockLifecycleState>(entity).unwrap();
        let reload_reset = !lock_state.valid
            && runtime.side(0) == (0.0, 0.0)
            && runtime.ik_weights() == (0.0, 0.0)
            && !offset_rotation.initialized[0]
            && !offset_rotation.initialized[1]
            && *dynamic_transition == AlsDynamicTransitionState::default()
            && lifecycle.teleport_remaining == 0.0
            && lifecycle.previous_character_transform.is_none();
        let left_thigh = app.world_mut().spawn_empty().id();
        let left_calf = app.world_mut().spawn_empty().id();
        let left_foot = app.world_mut().spawn_empty().id();
        let left_target = app.world_mut().spawn_empty().id();
        let right_thigh = app.world_mut().spawn_empty().id();
        let right_calf = app.world_mut().spawn_empty().id();
        let right_foot = app.world_mut().spawn_empty().id();
        let right_target = app.world_mut().spawn_empty().id();
        let teleport_entity = app
            .world_mut()
            .spawn((
                als_core::character::AlsCharacter,
                AlsFootBones {
                    left_thigh,
                    left_calf,
                    left: left_foot,
                    left_target: Some(left_target),
                    left_virtual_target: None,
                    right_thigh,
                    right_calf,
                    right: right_foot,
                    right_target: Some(right_target),
                    right_virtual_target: None,
                },
                FootLockLifecycleState::default(),
            ))
            .id();
        let _ = app.world_mut().write_message(AlsFootLockTeleported {
            entity: teleport_entity,
        });
        app.update();
        let teleport_message = (app
            .world()
            .get::<FootLockLifecycleState>(teleport_entity)
            .unwrap()
            .teleport_remaining
            - FOOT_LOCK_TELEPORT_WINDOW_SECONDS)
            .abs()
            < 1e-6;
        let passed = (FOOT_LOCK_TELEPORT_WINDOW_SECONDS - 0.2).abs() < 1e-6
            && teleport_location_error < 1e-5
            && teleport_rotation_alignment > 0.999_99
            && recovery_fresh
            && reload_reset
            && teleport_message;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.43][VERDICT] teleport_location_error={teleport_location_error:.6} teleport_rotation_alignment={teleport_rotation_alignment:.6} recovery_fresh={recovery_fresh} reload_reset={reload_reset} teleport_message={teleport_message} -> {verdict}"
        );
        assert!(passed);
    }

    #[test]
    fn new_foot_lock_uses_previous_final_pose_in_the_current_component_space() {
        let previous = Transform::from_xyz(2.0, 0.9, -1.0).with_rotation(Quat::from_rotation_y(0.3));
        let local = Vec3::new(0.2, -0.8, 0.3);
        let local_rotation = Quat::from_rotation_x(0.2);
        for translation in [Vec3::new(2.3, 0.9, -1.0), Vec3::new(-3.0, 30.9, 4.0)] {
            let current = Transform::from_translation(translation)
                .with_rotation(Quat::from_rotation_y(-0.6)).with_scale(Vec3::splat(1.2));
            let mut state = als_core::foot_ik::AlsFootLockFootState {
                final_location: previous.to_matrix().transform_point3(local),
                final_rotation: previous.rotation * local_rotation,
                ..default()
            };
            refresh_previous_foot_lock_transform(&mut state, &previous, &current, false);
            let target = current.to_matrix().transform_point3(local + Vec3::X * 0.1);
            refresh_foot_lock(&mut state, target, current.rotation, 1.0, 1.0, true, false, true, false, 1.0 / 60.0);
            assert!(state.lock_location.distance(current.to_matrix().transform_point3(local)) < 1e-5);
            assert!(state.lock_rotation.dot(current.rotation * local_rotation).abs() > 0.99999);
        }
    }

    #[test]
    fn source_teleport_threshold_marks_only_distance_above_half_meter() {
        let mut app = App::new();
        app.add_message::<AlsFootLockTeleported>().add_systems(
            Update,
            (detect_foot_lock_teleports, record_foot_lock_teleports).chain(),
        );
        let fake_bones = (0..10)
            .map(|_| app.world_mut().spawn_empty().id())
            .collect::<Vec<_>>();
        let character = app
            .world_mut()
            .spawn((
                als_core::character::AlsCharacter,
                AlsFootBones {
                    left_thigh: fake_bones[0],
                    left_calf: fake_bones[1],
                    left: fake_bones[2],
                    left_target: Some(fake_bones[3]),
                    left_virtual_target: Some(fake_bones[4]),
                    right_thigh: fake_bones[5],
                    right_calf: fake_bones[6],
                    right: fake_bones[7],
                    right_target: Some(fake_bones[8]),
                    right_virtual_target: Some(fake_bones[9]),
                },
                FootLockLifecycleState {
                    previous_character_transform: Some(Transform::IDENTITY),
                    teleport_remaining: 0.0,
                },
                AlsFootLockTeleportTelemetry::default(),
            ))
            .id();
        let avatar = app
            .world_mut()
            .spawn((
                AlsCharacterAvatar { character },
                ChildOf(character),
                GlobalTransform::from(Transform::from_xyz(
                    FOOT_LOCK_TELEPORT_DISTANCE_THRESHOLD_M,
                    0.0,
                    0.0,
                )),
            ))
            .id();
        app.update();
        let at_threshold = *app
            .world()
            .get::<AlsFootLockTeleportTelemetry>(character)
            .unwrap();
        *app.world_mut().get_mut::<GlobalTransform>(avatar).unwrap() = GlobalTransform::from(
            Transform::from_xyz(FOOT_LOCK_TELEPORT_DISTANCE_THRESHOLD_M + 0.001, 0.0, 0.0),
        );
        app.update();
        let above_threshold = *app
            .world()
            .get::<AlsFootLockTeleportTelemetry>(character)
            .unwrap();
        let lifecycle = app
            .world()
            .get::<FootLockLifecycleState>(character)
            .unwrap();
        assert_eq!(at_threshold.detected_count, 0);
        assert_eq!(above_threshold.detected_count, 1);
        assert!(
            (above_threshold.last_translation_delta_m
                - (FOOT_LOCK_TELEPORT_DISTANCE_THRESHOLD_M + 0.001))
                .abs()
                < 1e-6
        );
        assert!(
            (above_threshold.remaining_seconds - FOOT_LOCK_TELEPORT_WINDOW_SECONDS).abs() < 1e-6
        );
        assert!((lifecycle.teleport_remaining - FOOT_LOCK_TELEPORT_WINDOW_SECONDS).abs() < 1e-6);
    }

    #[test]
    fn applied_ik_telemetry_reports_the_actual_solver_weight() {
        let mut app = App::new();
        app.add_systems(Update, foot_ik_post_apply_telemetry_system);
        let bones = [1.0, 0.6, 0.2].map(|height| app.world_mut().spawn(GlobalTransform::from_translation(Vec3::Y * height)).id());
        let input = FootIkSolveInput { weight: 0.20929879, upper_length: 0.4, lower_length: 0.4, target_world: [0.0, 0.2, 0.0], ..default() };
        let entity = app.world_mut().spawn((als_core::character::AlsCharacter,
            AlsFootBones { left_thigh: bones[0], left_calf: bones[1], left: bones[2], left_target: None, left_virtual_target: None,
                right_thigh: bones[0], right_calf: bones[1], right: bones[2], right_target: None, right_virtual_target: None },
            AlsFootIkSpaceState { left: AlsFootIkSpaceSide { solve_input: Some(input), ..default() }, right: default() },
            AlsFootIkRuntime::default(), FootIkPoseBaseState::default(), AlsFootIkApplyTelemetry::default(),
        )).id();
        app.update();
        let actual = app.world().get::<AlsFootIkApplyTelemetry>(entity).unwrap();
        assert_eq!(actual.left.applied_weight, input.weight);
        assert_eq!(actual.right.applied_weight, 0.0);
        let serialized = serde_json::to_value(app.world().get::<AlsFootIkSpaceState>(entity).unwrap().left.solve_input).unwrap();
        assert_eq!(serialized["weight"].as_f64().unwrap() as f32, input.weight);
        assert_eq!(serialized["target_world"], serde_json::json!([0.0, 0.2_f32, 0.0]));
    }

    #[test]
    fn foot_ik_stage_delta_reads_translation_and_rotation() {
        let (translation, rotation) = foot_ik_stage_delta(
            Transform::IDENTITY,
            Transform::from_translation(Vec3::X).with_rotation(Quat::from_rotation_y(0.5)),
        );
        assert!((translation - 1.0).abs() <= 1e-6);
        assert!((rotation - 0.5).abs() <= 1e-6);
    }

    #[test]
    fn foot_ik_chain_delta_uses_the_largest_write() {
        assert_eq!(
            foot_ik_max_translation_delta(
                [Transform::IDENTITY, Transform::IDENTITY],
                [
                    Transform::from_translation(Vec3::X),
                    Transform::from_translation(Vec3::new(0.0, 2.0, 0.0)),
                ],
            ),
            2.0
        );
    }
}
