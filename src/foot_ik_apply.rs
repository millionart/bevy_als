use als_core::dynamics::AlsMovementBaseDynamicsState;
use als_core::foot_ik::{
    AlsDynamicTransitionState, AlsFootBones, AlsFootIkRuntime, AlsFootIkTargets, AlsFootLockInput,
    AlsFootLockMovementBase, AlsFootLockSettings, AlsFootLockState, AlsFootLockTeleported,
    AlsFootTargetSelection, FOOT_LOCK_TELEPORT_WINDOW_SECONDS, FootSideInput, SpringFloatState,
    advance_foot_lock_movement_base, capture_foot_lock_movement_base, constrain_foot_lock_location,
    is_relevant_animation_weight, refresh_foot_lock, reset_foot_lock_to_target,
    spring_damper_float,
};
use als_core::state::{LocomotionState, Stance};
use als_core::transitions::TransitionConfig;
use als_ext::ik::{
    blend_ik_rotation, calculate_pole_vector, child_world_rotation_after_parent_rotation,
    solve_two_bone_ik, world_rotation_to_local,
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

#[derive(Component, Debug, Clone, Copy, Default)]
struct SourcePoleState {
    current: Vec3,
    initialized: bool,
}

impl SourcePoleState {
    fn update(
        &mut self,
        hip: Vec3,
        knee: Vec3,
        ankle: Vec3,
        delta_seconds: f32,
        profile: &CrAlsTargetProfile,
    ) -> Vec3 {
        let target = calculate_pole_vector(hip, knee, ankle)
            .map(|pole| knee + pole.direction * profile.foot_pole_distance_m);
        let Some(target) = target else {
            return self.current;
        };
        if !self.initialized {
            self.initialized = true;
            self.current = target;
            return target;
        }
        let alpha = 1.0
            - inv_exp_approx(
                std::f32::consts::LN_2 * delta_seconds.max(0.0)
                    / profile.foot_pole_half_life_seconds.max(f32::EPSILON),
            );
        self.current = self.current.lerp(target, alpha);
        self.current
    }

    fn direction(&self, knee: Vec3) -> Vec3 {
        (self.current - knee).normalize_or_zero()
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
                FootIkApplySet::Solve
                    .after(apply_animation_to_targets)
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
            Option<&mut CrAlsFootRigState>,
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
        source_rig,
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
    mut telemetry: Query<&mut AlsFootIkTelemetry, With<als_core::character::AlsCharacter>>,
    mut space_states: Query<&mut AlsFootIkSpaceState, With<als_core::character::AlsCharacter>>,
    mut poles: Query<&mut FootIkPoleState, With<als_core::character::AlsCharacter>>,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance, &GlobalTransform), With<AlsCharacterAvatar>>,
    players: Query<&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>,
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
            continue;
        }
        if let Ok(mut space_state) = space_states.get_mut(entity) {
            *space_state = AlsFootIkSpaceState::default();
        }
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
        let pelvis_world_offset = apply_pelvis_drop(
            bones.left_thigh,
            source_rig.pelvis_offset_y,
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
            let (Ok(current_foot), Ok(current_calf)) =
                (bone_globals.get(chain.foot), bone_globals.get(chain.calf))
            else {
                continue;
            };
            let foot_world_rotation = world_rotation_from_armature(
                armature,
                ArmatureRotation(source_foot_rotation(
                    armature_rotation_from_world(armature, WorldRotation(current_foot.rotation()))
                        .0,
                    armature_rotation_from_world(armature, WorldRotation(current_calf.rotation()))
                        .0,
                    armature_rotation_from_world(
                        armature,
                        WorldRotation(foot_state.final_rotation),
                    )
                    .0,
                    offset_normal_armature,
                    side,
                    pose_moving,
                    &profile.0.target,
                )),
            )
            .0;
            let side_pole_state = match side {
                0 => &mut pole_state.left,
                _ => &mut pole_state.right,
            };
            let pole = match (
                bone_globals.get(chain.thigh),
                bone_globals.get(chain.calf),
                foot_parent,
            ) {
                (Ok(thigh), Ok(calf), Some((ankle, _))) => {
                    let hip = thigh.translation() + pelvis_world_offset;
                    let knee = calf.translation() + pelvis_world_offset;
                    let ankle = ankle + pelvis_world_offset;
                    side_pole_state.update(hip, knee, ankle, dt, &profile.0.target) - hip
                }
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
                        - (upper_length + lower_length) * profile.0.target.max_leg_stretch_ratio)
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
            let source_offset_y = if side == 0 {
                source_rig.left.location_offset_y
            } else {
                source_rig.right.location_offset_y
            };
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
            let resolved_target = apply_chain_target(
                &chain,
                target_world.0,
                pole,
                settings,
                profile.0.target.max_leg_stretch_ratio,
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
        if let Ok(mut trace) = control_rig_traces.get_mut(entity) {
            trace.stage_mask |= AlsControlRigStageTrace::FOOT_IK;
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
            let value = match (
                bone_globals.get(chain.thigh),
                bone_globals.get(chain.calf),
                bone_globals.get(chain.foot),
            ) {
                (Ok(hip), Ok(knee), Ok(foot)) => {
                    let pole = match side {
                        0 => pole_state.left.direction(knee.translation()),
                        _ => pole_state.right.direction(knee.translation()),
                    };
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
    max_leg_stretch_ratio: f32,
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
    let target = limit_leg_stretch(hip, target, (upper_len + lower_len) * max_leg_stretch_ratio);

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
}
