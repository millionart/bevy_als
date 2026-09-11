use als_core::dynamics::AlsMovementBaseDynamicsState;
use als_core::foot_ik::{
    AlsDynamicTransitionState, AlsFootBones, AlsFootLockFootState, AlsFootLockInput,
    AlsFootLockMovementBase, AlsFootLockSettings, AlsFootLockState, AlsFootLockTeleported,
    AlsFootTargetSelection, FOOT_LOCK_TELEPORT_DISTANCE_THRESHOLD_M, FOOT_LOCK_TELEPORT_WINDOW_SECONDS,
    advance_foot_lock_movement_base, capture_foot_lock_movement_base, constrain_foot_lock_rotation,
    foot_lock_location_constraint_rotation, is_relevant_animation_weight, refresh_foot_lock,
    reset_foot_lock_to_target,
};
use als_core::state::{LocomotionState, Stance};
use als_core::transitions::TransitionConfig;
use als_ext::ik_space::{
    ArmaturePoint, ArmatureRotation, WorldPoint, WorldRotation, armature_point_from_world,
    armature_rotation_from_world, world_point_from_armature, world_rotation_from_armature,
};
use bevy::prelude::*;
use bevy::transform::{TransformSystems, helper::TransformHelper};
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::{animated_scene::AnimatedSceneInstance, animation_graph::AnimationGraph, animation_graph_player::AnimationGraphPlayer};
use als_ext::{control_rig_hierarchy::{RigSpace, RigTransform}, control_rig_math::RigValue,
    control_rig_program::{RigDefinition, RigInstruction}, root_graph_boundaries::{ControlRigBoundaryNode, ControlRigNodeState, RigCallSample, current_control_rig}};

use crate::animation_context_projection::AlsRootAnimationContextSet;
use crate::character_assembly::AlsCharacterAvatar;
use crate::control_rig_apply::{AlsControlRigExecution, AlsControlRigInput, AlsControlRigPrePropagationSet, CrAlsRuntimeProfileTemplate};

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
}

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsFootIkTelemetryEnabled(pub bool);

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsFootIkWeightOverride {
    pub left: Option<f32>,
    pub right: Option<f32>,
}

pub struct FootIkApplyPlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FootIkApplySet {
    UpdateInputs,
    Restore,
    Solve,
    Propagate,
    Telemetry,
}

impl Plugin for FootIkApplyPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(PostUpdate, AnimationGraphSet::Final.before(TransformSystems::Propagate))
            .init_resource::<AlsFootIkTelemetryEnabled>()
            .init_resource::<AlsFootIkWeightOverride>()
            .init_resource::<CrAlsRuntimeProfileTemplate>()
            .add_message::<AlsFootLockTeleported>()
            .configure_sets(PostUpdate, (
                FootIkApplySet::UpdateInputs
                    .after(crate::crouching_layer::AlsCrouchingLayerSet)
                    .after(crate::character_assembly::sync_avatar_stance_offset)
                    .before(crate::generated_ab_als_provider::GeneratedAbAlsProviderSet)
                    .after(als_core::motion::AlsRotationResponseSet)
                    .before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
                FootIkApplySet::Restore.after(AnimationGraphSet::Final)
                    .after(AlsRootAnimationContextSet).after(TransformSystems::Propagate),
                FootIkApplySet::Solve.after(FootIkApplySet::Restore).after(AlsControlRigPrePropagationSet),
                FootIkApplySet::Propagate.after(FootIkApplySet::Solve),
                FootIkApplySet::Telemetry.after(FootIkApplySet::Propagate),
            )).add_systems(PostUpdate, (
                ensure_foot_lock_components.before(crate::standing_layer::AlsStandingLayerSet).before(FootIkApplySet::UpdateInputs),
                ensure_foot_lock_lifecycle_state.before(FootIkApplySet::UpdateInputs),
                ensure_foot_lock_teleport_telemetry.before(FootIkApplySet::UpdateInputs),
                invalidate_missing_foot_bones.before(FootIkApplySet::UpdateInputs),
                (detect_foot_lock_teleports, record_foot_lock_teleports, refresh_foot_lock_inputs)
                    .chain().in_set(FootIkApplySet::UpdateInputs),
                observe_rig_results.in_set(FootIkApplySet::Telemetry),
            ));
    }
}

fn rig_scalar(value: &RigValue) -> Option<f32> {
    let value = match value { RigValue::F32(value) => *value, RigValue::F64(value) => *value as f32, _ => return None };
    value.is_finite().then_some(value)
}

fn rig_vector(value: &RigValue) -> Option<bevy::math::DVec3> {
    match value { RigValue::Vector(value) if value.is_finite() => Some(*value), _ => None }
}

fn rig_bone(value: &RigValue) -> Option<&str> {
    let RigValue::List(fields) = value else { return None; };
    let [RigValue::Name(kind), RigValue::Name(name)] = fields.as_slice() else { return None; };
    (kind == "Bone").then_some(name.as_str())
}

fn rig_call_is(definition: &RigDefinition, call: &RigCallSample, name: &str) -> bool {
    matches!(definition.program.instructions.get(call.instruction), Some(RigInstruction::Call { function, .. }) if function == name)
}

fn rig_foot_result(
    definition: &RigDefinition,
    state: &ControlRigNodeState,
    side: usize,
    foot_name: &str,
    to_world: RigTransform,
    actual: [Vec3; 3],
    lock_amount: f32,
) -> Option<(AlsFootIkTelemetrySide, AlsFootIkSpaceSide, AlsFootIkApplyTelemetrySide)> {
    let call = state.calls.iter().rev().find(|call| rig_call_is(definition, call, "FRigUnit_TwoBoneIKSimplePerItem::Execute")
        && call.arguments.get(2).and_then(rig_bone).is_some_and(|name| name.eq_ignore_ascii_case(foot_name)))?;
    let mut hierarchy = state.snapshot_hierarchy()?;
    let indices = [0, 1, 2].map(|index| hierarchy.item_index(call.arguments.get(index)?).ok().flatten());
    let [Some(hip), Some(knee), Some(foot)] = indices else { return None; };
    let mut before = Vec::new();
    let mut expected = Vec::new();
    for index in [hip, knee, foot] {
        before.push(call.bones_before.iter().find(|(bone, _)| *bone == index)?.1);
        expected.push(to_world.transform_point(hierarchy.transform(index, RigSpace::Global, false).ok()?.translation).as_vec3());
    }
    if actual.iter().chain(&expected).any(|value| !value.is_finite()) { return None; }
    let target = RigTransform::from_value(call.arguments.get(3)?).ok()?;
    let pole = rig_vector(call.arguments.get(7)?)?;
    if call.arguments.get(8)? != &RigValue::Name("Location".into()) || hierarchy.item_index(call.arguments.get(9)?).ok().flatten().is_some() {
        return None;
    }
    let target_world = to_world.transform_point(target.translation).as_vec3();
    let pole_world = to_world.transform_point(pole).as_vec3();
    let weight = rig_scalar(call.arguments.get(13)?)?;
    let normal_output = definition.externals.get(["FootLeftOffsetNormal", "FootRightOffsetNormal"].get(side).copied()?)?;
    let trace_call = state.calls.iter().rev().find(|sample| {
        if !rig_call_is(definition, sample, "FAlsRigUnit_FootOffsetTrace::Execute") { return false; }
        let RigInstruction::Call { arguments, .. } = &definition.program.instructions[sample.instruction] else { return false; };
        definition.program.instructions.iter().any(|instruction| matches!(instruction,
            RigInstruction::Copy { input, output } if Some(input) == arguments.get(9) && output == normal_output))
    })?;
    let trace = state.traces.iter().rev().find(|trace| trace.instruction == trace_call.instruction);
    if trace_call.arguments.get(6)? == &RigValue::Bool(true) && trace.is_none() { return None; }
    let reference_call = state.calls.iter().rev().find(|sample| rig_call_is(definition, sample, "FAlsRigUnit_ApplyFootOffsetLocation::Execute")
        && sample.arguments.get(1).and_then(rig_bone).is_some_and(|name| name.eq_ignore_ascii_case(&definition.bones[hip].name)))?;
    let reference = rig_vector(reference_call.arguments.get(2)?)?;
    let trace_target = rig_vector(trace_call.arguments.first()?)?;
    let normal = rig_vector(trace_call.arguments.get(9)?)?;
    let hit = trace.and_then(|trace| trace.hit);
    let normal_world = hit.map_or(to_world.rotation * (to_world.scale * normal), |(_, normal)| normal).as_vec3();
    let axis = (actual[2] - actual[0]).normalize_or_zero();
    let knee_delta = actual[1] - actual[0];
    let pole_delta = pole_world - actual[0];
    let pole_sign = (knee_delta - axis * knee_delta.dot(axis)).dot(pole_delta - axis * pole_delta.dot(axis));
    let errors = std::array::from_fn::<_, 3, _>(|index| actual[index].distance(expected[index]));
    let trs = |value: RigTransform| (value.translation.as_vec3().to_array(), value.rotation.as_quat().to_array(), value.scale.as_vec3().to_array());
    let target_parent_local = before[1].inverse_transform_point(target.translation).as_vec3();
    let telemetry = AlsFootIkTelemetrySide { valid: true, target_world, target_armature_local: target.translation.as_vec3(),
        target_parent_local, reference_chain_error: errors.into_iter().fold(0.0, f32::max), pole_sign, ik_weight: weight, pin_weight: lock_amount };
    let space = AlsFootIkSpaceSide { valid: true, has_ground_hit: hit.is_some(),
        trace_target_world: to_world.transform_point(trace_target).as_vec3(), hit_world: hit.map_or(Vec3::ZERO, |(point, _)| point.as_vec3()),
        solver_reference_world: to_world.transform_point(reference).as_vec3(), solver_reference_armature_local: reference.as_vec3(),
        target_world, resolved_target_world: expected[2], resolved_knee_world: expected[1], pole_location_world: pole_world,
        target_armature_local: target.translation.as_vec3(), target_parent_local,
        target_bone_local: before[2].inverse_transform_point(target.translation).as_vec3(), normal_world, normal_armature_local: normal.as_vec3(),
        raw_offset: rig_scalar(trace_call.arguments.get(8)?)?,
        solve_input: Some(FootIkSolveInput { armature_world_trs: trs(to_world), thigh_world_trs: trs(before[0].global(to_world)),
            calf_world_trs: trs(before[1].global(to_world)), foot_world_trs: trs(before[2].global(to_world)),
            target_world: target_world.to_array(), target_rotation_world: target.global(to_world).rotation.as_quat().to_array(),
            pole_world: pole_world.to_array(), primary_axis: rig_vector(call.arguments.get(4)?)?.as_vec3().to_array(),
            secondary_axis: rig_vector(call.arguments.get(5)?)?.as_vec3().to_array(), upper_length: rig_scalar(call.arguments.get(14)?)?,
            lower_length: rig_scalar(call.arguments.get(15)?)?, weight }), previous_solve_input: None };
    let apply = AlsFootIkApplyTelemetrySide { valid: true, target_world, effector_world: actual[2],
        hip_world: actual[0], knee_world: actual[1], ankle_world: actual[2], hip_error: errors[0], knee_error: errors[1],
        effector_error: errors[2], pole_sign, applied_weight: weight };
    Some((telemetry, space, apply))
}

fn observe_rig_results(
    mut commands: Commands,
    characters: Query<(Entity, Option<&AlsFootBones>, Option<&AlsFootLockState>, Option<&AlsFootIkSpaceState>, Option<&AlsControlRigExecution>), With<als_core::character::AlsCharacter>>,
    avatars: Query<(Entity, &ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    graphs: Option<Res<Assets<AnimationGraph>>>,
    names: Query<&Name>,
    transforms: TransformHelper,
) {
    for (entity, bones, lock, previous, previous_execution) in &characters {
        let mut execution = AlsControlRigExecution::default();
        let mut telemetry = AlsFootIkTelemetry::default();
        let mut spaces = AlsFootIkSpaceState::default();
        let mut apply = AlsFootIkApplyTelemetry::default();
        if let Some(graphs) = graphs.as_deref()
            && let Some((avatar, _, instance)) = avatars.iter().find(|(_, parent, _)| parent.parent() == entity)
            && let Ok(player) = players.get(instance.player_entity())
            && let Some((_, node, state)) = current_control_rig(player, graphs)
            && let Some(definition) = node.try_inner_downcast_ref::<ControlRigBoundaryNode>().and_then(|node| node.definition.as_ref())
        {
            let value = |name| definition.externals.get(name).and_then(|value| state.value(value).ok()).as_ref().and_then(rig_scalar);
            execution = AlsControlRigExecution { initialization_count: state.initialization_count, executed_steps: state.executed_steps,
                frame: player.get_context_arena().unwrap().frame_index(), elapsed_seconds: state.elapsed_seconds,
                foot_height_m: value("FootHeight").unwrap_or(0.0), leg_length_m: value("LegLength").unwrap_or(0.0) };
            if let Some(bones) = bones && let Ok(world) = transforms.compute_global_transform(avatar) {
                let world = world.compute_transform();
                let to_world = RigTransform { translation: world.translation.as_dvec3(), rotation: world.rotation.as_dquat(), scale: world.scale.as_dvec3() };
                for side in 0..2 {
                    let chain = bones.chain(side);
                    let actual = [chain.thigh, chain.calf, chain.foot].map(|bone| transforms.compute_global_transform(bone).ok().map(|transform| transform.translation()));
                    let [Some(hip), Some(knee), Some(foot)] = actual else { continue; };
                    let Ok(name) = names.get(chain.foot) else { continue; };
                    let lock_amount = lock.map_or(0.0, |lock| if side == 0 { lock.left.lock_amount } else { lock.right.lock_amount });
                    if let Some((detail, mut space, applied)) = rig_foot_result(definition, state, side, name.as_str(), to_world, [hip, knee, foot], lock_amount) {
                        if previous_execution.is_some_and(|previous| previous.frame.checked_add(1) == Some(execution.frame)) {
                            space.previous_solve_input = previous.and_then(|previous| if side == 0 { previous.left.solve_input } else { previous.right.solve_input });
                        }
                        if side == 0 { telemetry.left = detail; spaces.left = space; apply.left = applied; }
                        else { telemetry.right = detail; spaces.right = space; apply.right = applied; }
                    }
                }
            }
        }
        commands.entity(entity).insert((execution, telemetry, spaces, apply));
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
    avatars: Query<(Entity, &ChildOf), With<AlsCharacterAvatar>>,
    transforms: TransformHelper,
    mut telemetry: Query<&mut AlsFootLockTeleportTelemetry>,
) {
    for (entity, lifecycle) in &mut lifecycles {
        let Some((avatar, _)) = avatars.iter().find(|(_, parent)| parent.parent() == entity)
        else {
            continue;
        };
        let Ok(armature) = transforms.compute_global_transform(avatar) else { continue };
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
    mut characters: Query<(
        Option<&mut AlsFootLockState>, Option<&mut AlsDynamicTransitionState>,
        Option<&mut FootLockLifecycleState>, Option<&mut AlsFootLockTeleportTelemetry>,
    ), (With<als_core::character::AlsCharacter>, Without<AlsFootBones>)>,
) {
    for (lock, dynamic, lifecycle, telemetry) in &mut characters {
        if let Some(mut value) = lock { *value = default(); }
        if let Some(mut value) = dynamic { *value = default(); }
        if let Some(mut value) = lifecycle { *value = default(); }
        if let Some(mut value) = telemetry { *value = default(); }
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

fn refresh_foot_lock_inputs(
    time: Res<Time>,
    profile: Res<CrAlsRuntimeProfileTemplate>,
    mut characters: Query<(
        Entity, &AlsFootBones, Option<&AlsFootTargetSelection>, &LocomotionState,
        &AlsFootLockInput, &Stance, &AlsFootLockSettings, Option<&AlsMovementBaseDynamicsState>,
        &mut FootLockLifecycleState, &mut AlsFootLockState, &mut AlsDynamicTransitionState,
        Option<&mut AlsControlRigInput>,
    ), With<als_core::character::AlsCharacter>>,
    avatars: Query<(Entity, &ChildOf), With<AlsCharacterAvatar>>,
    transforms: TransformHelper,
    parents: Query<&ChildOf>,
    bone_globals: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    mut telemetry: Query<&mut AlsFootLockTeleportTelemetry>,
) {
    let dt = time.delta_secs();
    for (entity, bones, selection, locomotion, lock_input, stance, settings, movement_base,
        mut lifecycle, mut lock_state,
        mut dynamic_transition, mut rig_input) in &mut characters {
        let Some((avatar, _)) = avatars.iter().find(|(_, parent)| parent.parent() == entity) else { continue };
        let Ok(armature) = transforms.compute_global_transform(avatar) else { continue };
        let current_character_transform = armature.compute_transform();
        let use_ik_bones = selection.map_or(true, |selection| selection.use_ik_bones);
        if !foot_bones_are_valid(bones, use_ik_bones, &bone_globals) {
            if let Some(input) = rig_input.as_mut() { input.foot_transforms_valid = false; }
            *lock_state = AlsFootLockState::default();
            *dynamic_transition = default();
            lifecycle.previous_character_transform = Some(current_character_transform);
            lifecycle.teleport_remaining = 0.0;
            continue;
        }
        let left_ik_amount = lock_input.left_ik_amount.clamp(0.0, 1.0);
        let right_ik_amount = lock_input.right_ik_amount.clamp(0.0, 1.0);
        let left_lock_amount = lock_input.left_lock_amount.clamp(0.0, 1.0);
        let right_lock_amount = lock_input.right_lock_amount.clamp(0.0, 1.0);
        lock_state.became_valid = !lock_state.valid;
        lock_state.valid = true;
        lock_state.foot_planted_amount = lock_input.foot_planted_amount.clamp(-1.0, 1.0);
        lock_state.feet_crossing_amount = lock_input.feet_crossing_amount.clamp(0.0, 1.0);
        let became_valid = lock_state.became_valid;
        if became_valid {
            *dynamic_transition = default();
        }
        let current_base = movement_base.and_then(|state| {
            state
                .entity
                .and_then(|entity| foot_lock_movement_base(entity, &transforms))
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
                    if let Ok(mut value) = telemetry.get_mut(entity) {
                        let count = if side == 0 { &mut value.left_rebase_count } else { &mut value.right_rebase_count };
                        *count = count.saturating_add(1);
                    }
                }
            }
        }
        for side in 0..2usize {
            let Ok(target) = transforms.compute_global_transform(bones.target(side, use_ik_bones)) else {
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
                .and_then(|base| foot_lock_movement_base(base.entity, &transforms));
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
            if let Ok(thigh_parent) = parents.get(chain.thigh)
                && let Ok(pelvis_world) = transforms.compute_global_transform(thigh_parent.0)
            {
                source_constrain_foot_lock(
                    foot_state,
                    &armature,
                    &pelvis_world,
                    thigh_axis_pelvis_space,
                    settings,
                );
            }
            capture_foot_lock_movement_base(foot_state, current_base);
        }
        if let Some(input) = rig_input.as_mut() {
            input.foot_transforms_valid = lock_state.valid;
            input.foot_left_location_armature = armature_point_from_world(&armature, WorldPoint(lock_state.left.final_location)).0;
            input.foot_left_rotation_armature = armature_rotation_from_world(&armature, WorldRotation(lock_state.left.final_rotation)).0;
            input.foot_right_location_armature = armature_point_from_world(&armature, WorldPoint(lock_state.right.final_location)).0;
            input.foot_right_rotation_armature = armature_rotation_from_world(&armature, WorldRotation(lock_state.right.final_rotation)).0;
        }
        dynamic_transition.update(
            &lock_state, lock_input.allow_transitions, armature.to_scale_rotation_translation().0.z.abs(),
            *stance, &TransitionConfig::default(),
        );
        lifecycle.previous_character_transform = Some(current_character_transform);
        lifecycle.teleport_remaining = (lifecycle.teleport_remaining - dt.max(0.0)).max(0.0);
        if let Ok(mut value) = telemetry.get_mut(entity) {
            value.remaining_seconds = lifecycle.teleport_remaining;
        }
    }
}

fn foot_lock_movement_base(
    entity: Entity,
    transforms: &TransformHelper,
) -> Option<AlsFootLockMovementBase> {
    let transform = transforms.compute_global_transform(entity).ok()?;
    Some(AlsFootLockMovementBase {
        entity,
        translation: transform.translation(),
        rotation: transform.rotation(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rig_observer_reads_actual_execution_hits_weights_and_pose_delivery() {
        use als_ext::{control_rig_query::{RigQueryContext, RigQueryWorld}, source_gameplay_tag_input::set_rig_query_context,
            root_graph_boundaries::{RigInputBinding, RIG_CAPTURE_KEY, RIG_GAME_WORLD_KEY}};
        use avian3d::{collider_tree::ColliderTrees, prelude::*};
        use bevy::{animation::AnimatedBy, ecs::system::RunSystemOnce,
            world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner}};
        use bevy_animation_graph::{AnimationGraphPlugin, core::{animated_scene::AnimatedSceneHandle,
            animation_clip::EntityPath, animation_node::AnimationNode, edge_data::{DataSpec, DataValue},
            pose::{BonePose, Pose}, skeleton::Skeleton, systems::{animation_player, apply_animation_to_targets}}};
        fn capture(In((entity, to_world)): In<(Entity, RigTransform)>, trees: Res<ColliderTrees>, colliders: Query<(Entity, &Position, &Rotation, &Collider)>) {
            set_rig_query_context(entity, Some(RigQueryContext { world: std::sync::Arc::new(RigQueryWorld {
                trees: trees.clone(), colliders: colliders.iter().map(|(entity, position, rotation, collider)| (entity, (*position, *rotation, collider.clone()))).collect(),
            }), to_world, channel: "ECC_Visibility".into(), filter: default() }));
        }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin, bevy::gizmos::GizmoPlugin,
            PhysicsPlugins::new(PostUpdate), AnimationGraphPlugin::from_physics_schedule(PostUpdate), als_ext::AlsExtPlugin))
            .init_asset::<WorldAsset>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 20.0), Transform::IDENTITY));
        for _ in 0..3 { app.update(); }
        app.world_mut().resource_mut::<bevy::ecs::schedule::Schedules>().remove(PostUpdate);
        let definition: RigDefinition = serde_json::from_str(include_str!("../assets/control_rigs/cr_als.program.json")).unwrap();
        let local = |value: RigTransform| Transform { translation: value.translation.as_vec3(), rotation: value.rotation.as_quat(), scale: value.scale.as_vec3() };
        let mut skeleton = Skeleton::default();
        let mut paths: Vec<Vec<String>> = Vec::new();
        for bone in &definition.bones {
            let mut path = bone.parent.map(|index| paths[index].clone()).unwrap_or_default();
            path.push(bone.name.clone());
            skeleton.add_bone(EntityPath::from(path.clone()), local(bone.initial_local), local(bone.initial_global));
            paths.push(path);
        }
        skeleton.set_root(EntityPath::from(paths[0].clone()).id());
        let skeleton = app.world_mut().resource_mut::<Assets<Skeleton>>().add(skeleton);
        let mut pose = Pose { skeleton: skeleton.clone(), ..default() };
        for (bone, path) in definition.bones.iter().zip(&paths) { pose.add_bone(BonePose::from_transform(local(bone.initial_local)), EntityPath::from(path.clone()).id()); }
        let mut graph = AnimationGraph::new();
        let source = AnimationNode::new("input", als_ext::GlobalPoseInputNode { key: "input".into(), output: "Pose".into() });
        let weight = AnimationNode::new("weight", als_ext::GlobalF32InputNode { key: "weight".into() });
        let rig = AnimationNode::new("rig", ControlRigBoundaryNode { definition: Some(definition.clone()),
            input_bindings: [2, 7, 8, 9, 10].map(|index| RigInputBinding { external: "RigInput".into(), path: vec![index], property: format!("input_{index}") }).into(),
            curve_names: vec!["FootLeftIk".into(), "FootRightIk".into()], internalize_time: true, ..default() });
        for node in [&source, &weight, &rig] { graph.add_node(node.clone()); }
        graph.add_node_parameter_edge(source.id, "Pose", rig.id, "source_pose");
        for name in ["FootLeftIk", "FootRightIk"] { graph.add_node_parameter_edge(weight.id, "weight", rig.id, name); }
        graph.add_output_data("pose".into(), DataSpec::Pose);
        graph.add_output_data_edge(rig.id, "Pose", "pose");
        let graph = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(graph);
        let mut player = AnimationGraphPlayer::new(skeleton).with_graph(graph.clone());
        player.set_global_input_data("input", DataValue::Pose(pose));
        player.set_global_input_data("animation.delta_seconds", 1.0_f32 / 60.0);
        player.set_global_input_data(RIG_CAPTURE_KEY, true);
        player.set_global_input_data(RIG_GAME_WORLD_KEY, true);
        player.set_global_input_data("input_2", true);
        for (name, location, rotation) in [("foot_l", "input_7", "input_8"), ("foot_r", "input_9", "input_10")] {
            let transform = definition.bones.iter().find(|bone| bone.name == name).unwrap().initial_global;
            player.set_global_input_data(location, transform.translation.as_vec3());
            player.set_global_input_data(rotation, transform.rotation.as_quat());
        }
        let actor = app.world_mut().spawn((als_core::character::AlsCharacter, Transform::IDENTITY, AlsFootLockState::default())).id();
        let world = Transform::from_xyz(2.0, 0.0, 3.0).with_rotation(Quat::from_rotation_y(0.4));
        let to_world = RigTransform { translation: world.translation.as_dvec3(), rotation: world.rotation.as_dquat(), scale: world.scale.as_dvec3() };
        let avatar = app.world_mut().spawn((AlsCharacterAvatar { character: actor }, world, ChildOf(actor), AnimatedSceneHandle::default())).id();
        let player_entity = app.world_mut().spawn((player, Name::new("root"), Transform::IDENTITY, ChildOf(avatar))).id();
        let mut entities = vec![player_entity];
        for (index, bone) in definition.bones.iter().enumerate().skip(1) {
            let id = EntityPath::from(paths[index].clone()).id();
            entities.push(app.world_mut().spawn((Name::new(bone.name.clone()), local(bone.initial_local), ChildOf(entities[bone.parent.unwrap()]),
                id.animation_target_id(), AnimatedBy(player_entity))).id());
        }
        let bone = |name: &str| entities[definition.bones.iter().position(|bone| bone.name == name).unwrap()];
        let bones = AlsFootBones { left_thigh: bone("thigh_l"), left_calf: bone("calf_l"), left: bone("foot_l"), left_target: Some(bone("ik_foot_l")), left_virtual_target: None,
            right_thigh: bone("thigh_r"), right_calf: bone("calf_r"), right: bone("foot_r"), right_target: Some(bone("ik_foot_r")), right_virtual_target: None };
        app.world_mut().entity_mut(actor).insert(bones);
        app.world_mut().trigger(WorldInstanceReady { entity: avatar, instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()) });
        app.world_mut().flush();
        assert_eq!(app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity(), player_entity);
        for (frame, weight) in [1.0_f32, 0.25, 0.0].into_iter().enumerate() {
            app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap().set_global_input_data("weight", weight);
            app.world_mut().run_system_once_with(capture, (player_entity, to_world)).unwrap();
            app.world_mut().run_system_once(animation_player).unwrap();
            assert!(app.world().get::<AnimationGraphPlayer>(player_entity).unwrap().get_error().is_none());
            app.world_mut().run_system_once(apply_animation_to_targets).unwrap();
            app.world_mut().run_system_once(observe_rig_results).unwrap();
            let execution = app.world().get::<AlsControlRigExecution>(actor).unwrap();
            assert_eq!(execution.initialization_count, 1);
            assert!(execution.executed_steps > 0 && execution.foot_height_m > 0.0 && execution.leg_length_m > 0.0);
            let space = app.world().get::<AlsFootIkSpaceState>(actor).unwrap();
            let apply = app.world().get::<AlsFootIkApplyTelemetry>(actor).unwrap();
            for (space, apply) in [(&space.left, &apply.left), (&space.right, &apply.right)] {
                assert!(space.valid && apply.valid, "{space:?}");
                assert_eq!(space.has_ground_hit, weight > 0.0);
                assert_eq!(space.previous_solve_input.is_some(), frame > 0);
                assert_eq!(apply.applied_weight, weight);
                assert!(apply.effector_error < 1e-5 && apply.knee_error < 1e-5, "{apply:?}");
                if weight > 0.0 { assert!((space.raw_offset - 0.1).abs() < 1e-5, "{space:?}"); }
            }
        }
        app.world_mut().get_mut::<Transform>(bones.left).unwrap().translation.x += 0.05;
        app.world_mut().run_system_once(observe_rig_results).unwrap();
        assert!(app.world().get::<AlsFootIkApplyTelemetry>(actor).unwrap().left.effector_error > 0.049);
        app.world_mut().resource_mut::<Assets<AnimationGraph>>().get_mut(&graph).unwrap().add_output_data_edge(source.id, "Pose", "pose");
        app.world_mut().run_system_once(animation_player).unwrap();
        app.world_mut().run_system_once(observe_rig_results).unwrap();
        assert_eq!(app.world().get::<AlsControlRigExecution>(actor).unwrap().executed_steps, 0, "an unevaluated rig must not expose its previous state");
        assert!(!app.world().get::<AlsFootIkSpaceState>(actor).unwrap().left.valid);
        app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap().start(graph);
        app.world_mut().run_system_once(observe_rig_results).unwrap();
        assert_eq!(app.world().get::<AlsControlRigExecution>(actor).unwrap().executed_steps, 0);
        assert!(!app.world().get::<AlsFootIkSpaceState>(actor).unwrap().left.valid);
        set_rig_query_context(player_entity, None);
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
        let incomplete = app.world_mut().spawn((als_core::character::AlsCharacter, FootLockLifecycleState {
            previous_character_transform: Some(previous_character_transform),
            teleport_remaining: FOOT_LOCK_TELEPORT_WINDOW_SECONDS,
        })).id();
        app.update();
        assert!(app.world().get::<FootLockLifecycleState>(incomplete).unwrap().previous_character_transform.is_none());
        let lock_state = app.world().get::<AlsFootLockState>(entity).unwrap();
        let dynamic_transition = app
            .world()
            .get::<AlsDynamicTransitionState>(entity)
            .unwrap();
        let lifecycle = app.world().get::<FootLockLifecycleState>(entity).unwrap();
        let reload_reset = !lock_state.valid
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
                Transform::IDENTITY,
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
                Transform::from_xyz(
                    FOOT_LOCK_TELEPORT_DISTANCE_THRESHOLD_M,
                    0.0,
                    0.0,
                ),
            ))
            .id();
        app.update();
        let at_threshold = *app
            .world()
            .get::<AlsFootLockTeleportTelemetry>(character)
            .unwrap();
        *app.world_mut().get_mut::<Transform>(avatar).unwrap() =
            Transform::from_xyz(FOOT_LOCK_TELEPORT_DISTANCE_THRESHOLD_M + 0.001, 0.0, 0.0);
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
    fn foot_lock_prepares_completed_pose_before_the_next_graph_evaluation() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, FootIkApplyPlugin))
            .init_resource::<avian3d::collider_tree::ColliderTrees>();
        app.configure_sets(PostUpdate, (
            AnimationGraphSet::PrePhysics.before(avian3d::prelude::PhysicsSystems::First),
            avian3d::prelude::PhysicsSystems::Last.after(avian3d::prelude::PhysicsSystems::First),
            als_core::motion::AlsRotationResponseSet.after(avian3d::prelude::PhysicsSystems::Last),
            als_ext::post_physics_animation::PostPhysicsAnimationSet::Inputs
                .after(avian3d::prelude::PhysicsSystems::Last)
                .before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
        ));
        let character = app.world_mut().spawn((
            als_core::character::AlsCharacter, Transform::from_xyz(2.0, 0.0, 0.0),
            AlsFootTargetSelection::default(),
            LocomotionState::Grounded, Stance::Standing, AlsFootLockSettings::default(),
            AlsFootLockInput { left_ik_amount: 1.0, right_ik_amount: 1.0,
                left_lock_amount: 1.0, right_lock_amount: 1.0, ..default() },
            AlsFootLockState::default(), AlsDynamicTransitionState::default(),
            FootLockLifecycleState::default(),
        )).id();
        app.world_mut().entity_mut(character).insert(AlsControlRigInput::default());
        let avatar = app.world_mut().spawn((
            AlsCharacterAvatar { character }, Transform::IDENTITY, ChildOf(character),
        )).id();
        let pelvis = app.world_mut().spawn((Transform::from_xyz(0.0, 0.9, 0.0), ChildOf(avatar))).id();
        let thigh = app.world_mut().spawn((Transform::IDENTITY, ChildOf(pelvis))).id();
        let calf = app.world_mut().spawn((Transform::from_xyz(0.0, -0.4, 0.0), ChildOf(thigh))).id();
        let foot = app.world_mut().spawn((Transform::from_xyz(0.0, -0.4, 0.0), ChildOf(calf))).id();
        app.world_mut().entity_mut(character).insert(AlsFootBones {
            left_thigh: thigh, left_calf: calf, left: foot, left_target: Some(foot), left_virtual_target: None,
            right_thigh: thigh, right_calf: calf, right: foot, right_target: Some(foot), right_virtual_target: None,
        });
        app.add_systems(PostUpdate, (move |mut transforms: Query<&mut Transform>| {
            transforms.get_mut(character).unwrap().translation.y += 0.1;
        }).in_set(avian3d::prelude::PhysicsSystems::First));
        let mut frame = 0;
        app.add_systems(PostUpdate, (move |world: &mut World| {
            let feet = *world.get::<AlsFootLockState>(character).unwrap();
            assert!(feet.valid, "foot inputs must be prepared before graph evaluation");
            let expected = if frame == 0 { Vec3::new(2.0, 0.2, 0.0) } else { Vec3::new(3.0, 0.6, 0.0) };
            assert!(feet.left.target_location.distance(expected) < 1e-5);
            assert_eq!(feet.left.lock_amount, if frame == 0 { 1.0 } else { 0.0 });
            let rig = world.get::<AlsControlRigInput>(character).unwrap();
            assert!(rig.foot_transforms_valid);
            let expected_armature = expected - world.get::<Transform>(character).unwrap().translation;
            assert!(rig.foot_left_location_armature.distance(expected_armature) < 1e-5);
            world.get_mut::<Transform>(foot).unwrap().translation.y = -0.1;
            world.get_mut::<AlsFootLockInput>(character).unwrap().left_lock_amount = 0.0;
            frame += 1;
        }).in_set(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate));
        app.update();
        let feet = app.world().get::<AlsFootLockState>(character).unwrap();
        assert_eq!(feet.left.lock_amount, 1.0, "evaluation must not refresh foot lock again");
        assert_eq!(app.world().get::<Transform>(foot).unwrap().translation.y, -0.1);
        app.world_mut().get_mut::<Transform>(character).unwrap().translation.x = 3.0;
        app.update();
        assert_eq!(app.world().get::<Transform>(foot).unwrap().translation.y, -0.1);
    }

}
