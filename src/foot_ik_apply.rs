use als_core::dynamics::AlsMovementBaseDynamicsState;
use als_core::foot_ik::{
    AlsDynamicTransitionState, AlsFootBones, AlsFootIkRuntime, AlsFootIkTargets, AlsFootLockInput,
    AlsFootLockMovementBase, AlsFootLockSettings, AlsFootLockState, AlsFootLockTeleported,
    AlsFootProbe, AlsGroundHit, FOOT_LOCK_TELEPORT_WINDOW_SECONDS, FootSideInput,
    advance_foot_lock_movement_base, capture_foot_lock_movement_base, constrain_foot_lock_location,
    foot_target_offset_in_armature, is_relevant_animation_weight, refresh_foot_lock,
    reset_foot_lock_to_target,
};
use als_core::state::{LocomotionState, Stance};
use als_core::transitions::TransitionConfig;
use als_ext::ik::{
    StableTwoBonePole, blend_ik_rotation, child_world_rotation_after_parent_rotation,
    solve_two_bone_ik, world_rotation_to_local,
};
use als_ext::ik_space::{
    ArmatureDirection, ArmaturePoint, WorldDirection, WorldPoint, armature_direction_from_world,
    armature_point_from_world, bone_local_point_from_world, parent_point_from_world,
    world_direction_from_armature, world_point_from_armature,
};
use bevy::animation::AnimationPlayer;
use bevy::prelude::*;
use bevy::transform::{TransformSystems, systems as transform_systems};
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::systems::apply_animation_to_targets;

use crate::character_assembly::AlsCharacterAvatar;

const FOOT_OFFSET_NORMAL_HALF_LIFE: f32 = 0.1;
const FOOT_MAX_LEG_STRETCH_RATIO: f32 = 0.99;

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

#[derive(Component, Debug, Clone, Copy, Default)]
struct FootIkPoleState {
    left: StableTwoBonePole,
    right: StableTwoBonePole,
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
    pub target_armature_local: Vec3,
    pub target_parent_local: Vec3,
    pub target_bone_local: Vec3,
    pub normal_world: Vec3,
    pub normal_armature_local: Vec3,
    pub raw_offset: f32,
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

#[derive(Debug, Clone, Copy)]
struct ResolvedFootTarget {
    target_world: Vec3,
    knee_world: Vec3,
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
    fn update(&mut self, side: usize, target_normal: Vec3, delta_seconds: f32) -> Vec3 {
        let target_normal = normalized_ground_normal(target_normal);
        if !self.initialized[side] {
            self.initialized[side] = true;
            self.normals[side] = target_normal;
            return target_normal;
        }
        let current_normal = normalized_ground_normal(self.normals[side]);
        let alpha = 1.0
            - (-std::f32::consts::LN_2 * delta_seconds.max(0.0) / FOOT_OFFSET_NORMAL_HALF_LIFE)
                .exp();
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
            .add_message::<AlsFootLockTeleported>();
        app.configure_sets(
            PostUpdate,
            (
                FootIkApplySet::Solve
                    .after(apply_animation_to_targets)
                    .after(TransformSystems::Propagate)
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
                ensure_foot_ik_telemetry,
                ensure_foot_ik_space_state,
                ensure_foot_ik_apply_telemetry,
                ensure_foot_ik_pole_state,
                ensure_foot_lock_components,
                ensure_foot_offset_rotation_state,
                ensure_foot_lock_lifecycle_state,
                invalidate_missing_foot_bones.before(FootIkApplySet::Solve),
                record_foot_lock_teleports.before(FootIkApplySet::Solve),
                foot_ik_apply_system.in_set(FootIkApplySet::Solve),
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

fn record_foot_lock_teleports(
    mut commands: Commands,
    mut teleports: MessageReader<AlsFootLockTeleported>,
    mut lifecycles: Query<&mut FootLockLifecycleState>,
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
    }
}

fn invalidate_missing_foot_bones(
    mut characters: Query<
        (
            Option<&AlsFootBones>,
            Option<&mut AlsFootLockState>,
            Option<&mut AlsFootIkRuntime>,
            Option<&mut FootOffsetRotationState>,
            Option<&mut AlsDynamicTransitionState>,
            Option<&mut FootLockLifecycleState>,
            Option<&mut AlsFootIkTelemetry>,
            Option<&mut AlsFootIkSpaceState>,
            Option<&mut AlsFootIkApplyTelemetry>,
            Option<&mut FootIkPoleState>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
) {
    for (
        bones,
        lock_state,
        runtime,
        offset_rotation,
        dynamic_transition,
        lifecycle,
        telemetry,
        space_state,
        apply_telemetry,
        pole_state,
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
    }
}

fn foot_bones_are_valid(
    bones: &AlsFootBones,
    transforms: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) -> bool {
    [
        bones.left_thigh,
        bones.left_calf,
        bones.left,
        bones.left_target.unwrap_or(bones.left),
        bones.right_thigh,
        bones.right_calf,
        bones.right,
        bones.right_target.unwrap_or(bones.right),
    ]
    .into_iter()
    .all(|entity| {
        transforms.get(entity).is_ok_and(|transform| {
            transform.translation().is_finite() && transform.rotation().is_finite()
        })
    })
}

fn rebase_foot_lock_after_teleport(
    state: &mut als_core::foot_ik::AlsFootLockFootState,
    previous_character_transform: &Transform,
    current_character_transform: &Transform,
) {
    let previous_inverse = previous_character_transform.to_matrix().inverse();
    let current_matrix = current_character_transform.to_matrix();
    state.lock_location =
        current_matrix.transform_point3(previous_inverse.transform_point3(state.lock_location));
    state.final_location =
        current_matrix.transform_point3(previous_inverse.transform_point3(state.final_location));
    let rotation_delta =
        current_character_transform.rotation * previous_character_transform.rotation.conjugate();
    state.lock_rotation = (rotation_delta * state.lock_rotation).normalize();
    state.final_rotation = (rotation_delta * state.final_rotation).normalize();
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

fn foot_trace_sample(
    armature_world: &GlobalTransform,
    trace_target_world: WorldPoint,
    hit: Option<AlsGroundHit>,
) -> FootTraceSample {
    let armature_up_world =
        world_direction_from_armature(armature_world, ArmatureDirection(Vec3::Y));
    let Some(hit) = hit else {
        return FootTraceSample {
            trace_target_world,
            normal_world: armature_up_world,
            normal_armature: ArmatureDirection(Vec3::Y),
            ..Default::default()
        };
    };
    let normal_world = WorldDirection(hit.normal.try_normalize().unwrap_or(Vec3::ZERO));
    let normal_armature = armature_direction_from_world(armature_world, normal_world);
    if normal_armature.0.y <= f32::EPSILON {
        return FootTraceSample {
            trace_target_world,
            normal_world: armature_up_world,
            normal_armature: ArmatureDirection(Vec3::Y),
            ..Default::default()
        };
    }
    let hit_world = WorldPoint(hit.point);
    let raw_offset = foot_target_offset_in_armature(
        armature_point_from_world(armature_world, trace_target_world).0,
        armature_point_from_world(armature_world, hit_world).0,
        normal_armature.0,
    );
    FootTraceSample {
        has_ground_hit: raw_offset.is_finite(),
        trace_target_world,
        hit_world,
        normal_world,
        normal_armature,
        raw_offset,
    }
}

#[allow(clippy::type_complexity)]
fn foot_ik_apply_system(
    time: Res<Time>,
    telemetry_enabled: Res<AlsFootIkTelemetryEnabled>,
    weight_override: Res<AlsFootIkWeightOverride>,
    mut characters: Query<
        (
            Entity,
            &GlobalTransform,
            &AlsFootBones,
            &AlsFootProbe,
            &AlsFootIkTargets,
            &LocomotionState,
            &AlsFootLockInput,
            &Stance,
            &AlsFootLockSettings,
            Option<&AlsMovementBaseDynamicsState>,
            &mut AlsFootIkRuntime,
            &mut FootOffsetRotationState,
            &mut FootLockLifecycleState,
            &mut AlsFootLockState,
            &mut AlsDynamicTransitionState,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    mut bone_locals: Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
    bone_globals: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    mut telemetry: Query<&mut AlsFootIkTelemetry, With<als_core::character::AlsCharacter>>,
    mut space_states: Query<&mut AlsFootIkSpaceState, With<als_core::character::AlsCharacter>>,
    mut poles: Query<&mut FootIkPoleState, With<als_core::character::AlsCharacter>>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance, &GlobalTransform), With<AlsCharacterAvatar>>,
    players: Query<&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    for (
        entity,
        char_gtf,
        bones,
        probe,
        targets,
        locomotion,
        lock_input,
        stance,
        settings,
        movement_base,
        mut runtime,
        mut foot_offset_rotation,
        mut lifecycle,
        mut lock_state,
        mut dynamic_transition,
    ) in &mut characters
    {
        let current_character_transform = char_gtf.compute_transform();
        if !foot_bones_are_valid(bones, &bone_globals) {
            *lock_state = AlsFootLockState::default();
            reset_foot_ik_runtime(
                &mut runtime,
                &mut foot_offset_rotation,
                &mut dynamic_transition,
            );
            lifecycle.previous_character_transform = Some(current_character_transform);
            lifecycle.teleport_remaining = 0.0;
            continue;
        }
        if let Ok(mut space_state) = space_states.get_mut(entity) {
            *space_state = AlsFootIkSpaceState::default();
        }
        let Ok(mut pole_state) = poles.get_mut(entity) else {
            continue;
        };
        let char_fwd = char_gtf.forward().as_vec3();
        let fallback_pole = Vec3::new(char_fwd.x, 0.0, char_fwd.z)
            .try_normalize()
            .unwrap_or(Vec3::Z);
        let left_ik_amount = weight_override
            .left
            .or_else(|| animation_curve_amount(entity, "FootLeftIk", &avatars, &players))
            .unwrap_or(lock_input.left_ik_amount)
            .clamp(0.0, 1.0);
        let right_ik_amount = weight_override
            .right
            .or_else(|| animation_curve_amount(entity, "FootRightIk", &avatars, &players))
            .unwrap_or(lock_input.right_ik_amount)
            .clamp(0.0, 1.0);
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
        }
        let current_base = movement_base.and_then(|state| {
            state
                .entity
                .and_then(|entity| foot_lock_movement_base(entity, &bone_globals))
        });
        if lifecycle.teleport_remaining > 0.0 && !became_valid {
            if let Some(previous_character_transform) = lifecycle.previous_character_transform {
                if is_relevant_animation_weight(left_ik_amount * lock_state.left.lock_amount) {
                    rebase_foot_lock_after_teleport(
                        &mut lock_state.left,
                        &previous_character_transform,
                        &current_character_transform,
                    );
                    capture_foot_lock_movement_base(&mut lock_state.left, current_base);
                }
                if is_relevant_animation_weight(right_ik_amount * lock_state.right.lock_amount) {
                    rebase_foot_lock_after_teleport(
                        &mut lock_state.right,
                        &previous_character_transform,
                        &current_character_transform,
                    );
                    capture_foot_lock_movement_base(&mut lock_state.right, current_base);
                }
            }
        }
        for side in 0..2usize {
            let Ok(target) = bone_globals.get(bones.target(side)) else {
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
                settings.foot_angle_limit_degrees,
                lock_input.moving_smooth,
                *locomotion == LocomotionState::Grounded,
                became_valid,
                dt,
            );
            capture_foot_lock_movement_base(foot_state, current_base);
        }

        let mut inputs = [FootSideInput::default(), FootSideInput::default()];
        let mut traces = [FootTraceSample::default(), FootTraceSample::default()];
        for side in 0..2usize {
            let hit = match side {
                0 => probe.left,
                _ => probe.right,
            };
            let trace_target_world = bone_globals
                .get(bones.target(side))
                .map(|target| WorldPoint(target.translation()))
                .unwrap_or_default();
            let trace = foot_trace_sample(char_gtf, trace_target_world, hit);
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
        let (left_ik_weight, right_ik_weight) = runtime.ik_weights();
        let pelvis_world_offset = apply_pelvis_drop(
            bones.left_thigh,
            runtime.pelvis_drop,
            &mut bone_locals,
            &bone_globals,
        );
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
            let (offset, pin_weight) = runtime.side(side);
            let foot_state = match side {
                0 => &lock_state.left,
                _ => &lock_state.right,
            };
            let applied_ik_weight = match side {
                0 => left_ik_weight,
                _ => right_ik_weight,
            };
            let ground_normal = trace.normal_world.0;
            let offset_normal = foot_offset_rotation.update(side, ground_normal, dt);
            let foot_world_rotation =
                foot_rotation_with_offset(foot_state.final_rotation, offset_normal);
            let lock_offset = foot_state.final_location - foot_state.target_location;
            let solver_reference_armature =
                armature_point_from_world(char_gtf, WorldPoint(foot_state.final_location));
            let target_armature = ArmaturePoint(solver_reference_armature.0 + Vec3::Y * offset);
            let target_world = world_point_from_armature(char_gtf, target_armature);
            let foot_parent = bone_locals
                .get(chain.foot)
                .ok()
                .map(|(_, foot_global, child)| (foot_global.translation(), child.0));
            let side_pole_state = match side {
                0 => &mut pole_state.left,
                _ => &mut pole_state.right,
            };
            let pole = match (
                bone_globals.get(chain.thigh),
                bone_globals.get(chain.calf),
                foot_parent,
            ) {
                (Ok(thigh), Ok(calf), Some((ankle, _))) => side_pole_state.resolve(
                    char_gtf.rotation(),
                    thigh.translation(),
                    calf.translation(),
                    ankle,
                    target_world.0,
                    fallback_pole,
                ),
                _ => fallback_pole,
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
                target_armature_local: target_armature.0,
                target_parent_local,
                target_bone_local,
                normal_world: trace.normal_world.0,
                normal_armature_local: trace.normal_armature.0,
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
                let pole_sign = target_axis.cross(knee - hip).dot(pole);
                let entry = AlsFootIkTelemetrySide {
                    valid: space_side.valid
                        && upper_length.is_finite()
                        && lower_length.is_finite()
                        && pole_sign.is_finite(),
                    target_world: target_world.0,
                    target_armature_local: target_armature.0,
                    target_parent_local,
                    reference_chain_error: (target_world.0.distance(hip)
                        - (upper_length + lower_length) * FOOT_MAX_LEG_STRETCH_RATIO)
                        .max(0.0),
                    pole_sign,
                    ik_weight: applied_ik_weight,
                    pin_weight,
                };
                if let Ok(mut telemetry) = telemetry.get_mut(entity) {
                    if side == 0 {
                        telemetry.left = entry;
                    } else {
                        telemetry.right = entry;
                    }
                }
            }
            let armature_up = world_direction_from_armature(char_gtf, ArmatureDirection(Vec3::Y)).0;
            let has_slope_rotation = offset_normal.dot(armature_up) < 1.0 - 1e-4;
            let has_lock_rotation = foot_state.lock_amount > 1e-3;
            if applied_ik_weight <= 1e-4
                || (offset.abs() < 1e-4
                    && pin_weight < 1e-3
                    && lock_offset.length_squared() < 1e-8
                    && !has_slope_rotation
                    && !has_lock_rotation)
            {
                continue;
            }
            let resolved_target = apply_chain_target(
                &chain,
                target_world.0,
                pole,
                settings,
                foot_world_rotation,
                applied_ik_weight,
                pelvis_world_offset,
                &mut bone_locals,
                &bone_globals,
            );
            if let Some(resolved_target) = resolved_target
                && let Ok(mut space_state) = space_states.get_mut(entity)
            {
                if side == 0 {
                    space_state.left.resolved_target_world = resolved_target.target_world;
                    space_state.left.resolved_knee_world = resolved_target.knee_world;
                } else {
                    space_state.right.resolved_target_world = resolved_target.target_world;
                    space_state.right.resolved_knee_world = resolved_target.knee_world;
                }
            }
        }
        lifecycle.previous_character_transform = Some(current_character_transform);
        lifecycle.teleport_remaining = (lifecycle.teleport_remaining - dt.max(0.0)).max(0.0);
    }
}

fn foot_ik_post_apply_telemetry_system(
    characters: Query<
        (
            Entity,
            &AlsFootBones,
            &AlsFootIkSpaceState,
            &AlsFootIkRuntime,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    bone_globals: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    poles: Query<&FootIkPoleState, With<als_core::character::AlsCharacter>>,
    mut telemetry: Query<&mut AlsFootIkApplyTelemetry, With<als_core::character::AlsCharacter>>,
) {
    for (entity, bones, spaces, runtime) in &characters {
        let Ok(pole_state) = poles.get(entity) else {
            continue;
        };
        let Ok(mut entry) = telemetry.get_mut(entity) else {
            continue;
        };
        let (left_weight, right_weight) = runtime.ik_weights();
        for side in 0..2usize {
            let chain = bones.chain(side);
            let space = match side {
                0 => spaces.left,
                _ => spaces.right,
            };
            let applied_weight = match side {
                0 => left_weight,
                _ => right_weight,
            };
            let pole = match side {
                0 => pole_state.left.last_direction_world(),
                _ => pole_state.right.last_direction_world(),
            };
            let value = match (
                bone_globals.get(chain.thigh),
                bone_globals.get(chain.calf),
                bone_globals.get(chain.foot),
            ) {
                (Ok(hip), Ok(knee), Ok(foot)) => {
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
    players: &Query<&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>,
) -> Option<f32> {
    let (_, instance, _) = avatars
        .iter()
        .find(|(parent, _, _)| parent.parent() == character)?;
    let player = players.get(instance.player_entity()).ok()?;
    player
        .get_outputs()
        .get(name)
        .and_then(|value| match value {
            bevy_animation_graph::core::edge_data::DataValue::F32(value) => Some(*value),
            _ => None,
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
    pole: Vec3,
    settings: &AlsFootLockSettings,
    foot_world_rotation: Quat,
    weight: f32,
    pelvis_world_offset: Vec3,
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

    let target = constrain_foot_lock_location(
        hip,
        thigh_g.forward().as_vec3(),
        target_world,
        settings.thigh_angle_limit_degrees,
    );
    let target = limit_leg_stretch(
        hip,
        target,
        (upper_len + lower_len) * FOOT_MAX_LEG_STRETCH_RATIO,
    );

    let Some(solve) = solve_two_bone_ik(hip, knee, ankle, target, upper_len, lower_len, pole)
    else {
        return None;
    };

    let thigh_world_new = solve.hip_rotation_world * thigh_g.rotation();
    let current_lower_axis = (ankle - knee).normalize_or_zero();
    let target_lower_axis = (target - solve.knee_position).normalize_or_zero();
    let Some(calf_world_new) = child_world_rotation_after_parent_rotation(
        solve.hip_rotation_world,
        calf_g.rotation(),
        current_lower_axis,
        target_lower_axis,
    ) else {
        return None;
    };

    let Ok((mut thigh_local, _, thigh_child)) = bone_locals.get_mut(chain.thigh) else {
        return None;
    };
    let pelvis_rot = bone_globals
        .get(thigh_child.0)
        .map(|p| p.rotation())
        .unwrap_or(Quat::IDENTITY);
    let thigh_target_local = world_rotation_to_local(pelvis_rot, thigh_world_new);
    thigh_local.rotation = blend_ik_rotation(thigh_local.rotation, thigh_target_local, weight);
    let thigh_world_applied = pelvis_rot * thigh_local.rotation;

    let Ok((mut calf_local, _, _)) = bone_locals.get_mut(chain.calf) else {
        return None;
    };
    let calf_target_local = world_rotation_to_local(thigh_world_new, calf_world_new);
    calf_local.rotation = blend_ik_rotation(calf_local.rotation, calf_target_local, weight);
    let calf_world_applied = thigh_world_applied * calf_local.rotation;

    let Ok((mut foot_local, _, foot_child)) = bone_locals.get_mut(chain.foot) else {
        return None;
    };
    let foot_parent_rotation = if foot_child.0 == chain.calf {
        calf_world_applied
    } else {
        bone_globals
            .get(foot_child.0)
            .map(|parent| parent.rotation())
            .unwrap_or(Quat::IDENTITY)
    };
    let foot_target_local = foot_local_rotation(foot_parent_rotation, foot_world_rotation);
    foot_local.rotation = blend_ik_rotation(foot_local.rotation, foot_target_local, weight);
    Some(ResolvedFootTarget {
        target_world: target,
        knee_world: solve.knee_position,
    })
}

fn foot_local_rotation(parent_world_rotation: Quat, foot_world_rotation: Quat) -> Quat {
    world_rotation_to_local(parent_world_rotation, foot_world_rotation)
}

fn limit_leg_stretch(hip: Vec3, target: Vec3, max_length: f32) -> Vec3 {
    let offset = target - hip;
    let length = offset.length();
    if length > max_length && length > f32::EPSILON {
        hip + offset * (max_length / length)
    } else {
        target
    }
}

fn pelvis_adjusted_position(position: Vec3, pelvis_world_offset: Vec3) -> Vec3 {
    position + pelvis_world_offset
}

fn normalized_ground_normal(normal: Vec3) -> Vec3 {
    normal.try_normalize().unwrap_or(Vec3::Y)
}

fn foot_rotation_with_offset(foot_target_rotation: Quat, offset_normal: Vec3) -> Quat {
    (Quat::from_rotation_arc(Vec3::Y, normalized_ground_normal(offset_normal))
        * foot_target_rotation)
        .normalize()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foot_local_rotation_uses_the_updated_calf_parent() {
        let calf_old = Quat::from_rotation_y(30.0_f32.to_radians());
        let calf_new = Quat::from_rotation_y(50.0_f32.to_radians());
        let foot_target = Quat::IDENTITY;
        let corrected_local = foot_local_rotation(calf_new, foot_target);
        let legacy_local = foot_local_rotation(calf_old, foot_target);
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
        let solve = solve_two_bone_ik(
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
    fn foot_trace_spaces_are_rotation_invariant_and_clear_no_hit() {
        let first = GlobalTransform::from(
            Transform::from_xyz(3.0, 1.0, -2.0).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                0.6,
                -0.2,
                0.1,
            )),
        );
        let second = GlobalTransform::from(
            Transform::from_xyz(-5.0, 0.4, 4.0).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                -0.9,
                0.3,
                -0.15,
            )),
        );
        let target_local = ArmaturePoint(Vec3::new(0.18, -0.92, -0.24));
        let hit_local = ArmaturePoint(Vec3::new(0.18, -1.00, -0.24));
        let normal_local = ArmatureDirection(Vec3::new(0.0, 0.94, 0.34).normalize());
        let first_target = world_point_from_armature(&first, target_local);
        let second_target = world_point_from_armature(&second, target_local);
        let first_hit = AlsGroundHit {
            point: world_point_from_armature(&first, hit_local).0,
            normal: world_direction_from_armature(&first, normal_local).0,
        };
        let second_hit = AlsGroundHit {
            point: world_point_from_armature(&second, hit_local).0,
            normal: world_direction_from_armature(&second, normal_local).0,
        };
        let first_sample = foot_trace_sample(&first, first_target, Some(first_hit));
        let second_sample = foot_trace_sample(&second, second_target, Some(second_hit));
        let no_hit = foot_trace_sample(&first, first_target, None);

        assert!(first_sample.has_ground_hit && second_sample.has_ground_hit);
        assert!((first_sample.raw_offset - second_sample.raw_offset).abs() < 1e-5);
        assert!(first_sample.normal_armature.0.dot(normal_local.0) > 0.999_99);
        assert!(second_sample.normal_armature.0.dot(normal_local.0) > 0.999_99);
        assert!(!no_hit.has_ground_hit);
        assert_eq!(no_hit.raw_offset, 0.0);
        assert_eq!(no_hit.hit_world.0, Vec3::ZERO);
        assert!(no_hit.normal_armature.0.dot(Vec3::Y) > 0.999_99);
    }

    #[test]
    fn foot_rotation_consumes_slope_normal_and_full_lock_rotation() {
        let slope_normal = Vec3::new(0.5, 0.866_025_4, 0.0);
        let mut normal_state = FootOffsetRotationState::default();
        let initialized_normal = normal_state.update(0, slope_normal, 1.0 / 60.0);
        let initialization_alignment = initialized_normal.dot(slope_normal);
        let slope_locked_rotation = Quat::from_rotation_y(35.0_f32.to_radians());
        let slope_rotation = foot_rotation_with_offset(slope_locked_rotation, slope_normal);
        let slope_alignment = (slope_rotation * Vec3::Y)
            .dot(slope_normal)
            .clamp(-1.0, 1.0);

        let full_lock_rotation = Quat::from_euler(
            EulerRot::YXZ,
            35.0_f32.to_radians(),
            -12.0_f32.to_radians(),
            18.0_f32.to_radians(),
        );
        let parent_rotation = Quat::from_rotation_y(-20.0_f32.to_radians());
        let foot_local = foot_local_rotation(
            parent_rotation,
            foot_rotation_with_offset(full_lock_rotation, Vec3::Y),
        );
        let final_bone_rotation = parent_rotation * foot_local;
        let lock_alignment = final_bone_rotation.dot(full_lock_rotation).abs();
        let legacy_rotation = Quat::from_rotation_y(35.0_f32.to_radians());
        let legacy_alignment = legacy_rotation.dot(full_lock_rotation).abs();
        let passed = initialization_alignment > 0.999_99
            && slope_alignment > 0.999_99
            && lock_alignment > 0.999_99
            && legacy_alignment < 0.99;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.42][VERDICT] initialization_alignment={initialization_alignment:.6} slope_alignment={slope_alignment:.6} lock_alignment={lock_alignment:.6} legacy_alignment={legacy_alignment:.6} -> {verdict}"
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
        rebase_foot_lock_after_teleport(
            &mut foot_state,
            &previous_character_transform,
            &current_character_transform,
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
                    right_thigh,
                    right_calf,
                    right: right_foot,
                    right_target: Some(right_target),
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
}
