use crate::als::dynamics::AlsMovementBase;
use crate::als::foot_ik::{
    AlsDynamicTransitionState, AlsFootBones, AlsFootLockFootState, AlsFootLockInput,
    AlsFootLockMovementBase, AlsFootLockSettings, AlsFootLockState, AlsFootLockTeleported,
    AlsFootTargetSelection, FOOT_LOCK_TELEPORT_DISTANCE_THRESHOLD_M, FOOT_LOCK_TELEPORT_WINDOW_SECONDS,
    build_foot_bone_map, capture_foot_lock_movement_base, constrain_foot_lock_rotation,
    foot_lock_location_constraint_rotation, is_relevant_animation_weight, refresh_foot_lock,
    reset_foot_lock_to_target,
};
use crate::als::state::{LocomotionState, Stance};
use crate::als::transitions::TransitionConfig;
use crate::extensions::ik_space::{
    ArmaturePoint, ArmatureRotation, WorldPoint, WorldRotation, armature_point_from_world,
    armature_rotation_from_world, world_point_from_armature, world_rotation_from_armature,
};
use bevy::prelude::*;
use bevy::animation::AnimationTargetId;
use avian3d::prelude::{ColliderOf, RigidBody};
use bevy::transform::{TransformSystems, helper::TransformHelper};
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::{animated_scene::AnimatedSceneInstance, animation_graph::AnimationGraph, animation_graph_player::AnimationGraphPlayer};
use crate::extensions::{control_rig_hierarchy::{RigSpace, RigTransform}, control_rig_math::RigValue,
    control_rig_program::{RigDefinition, RigInstruction}, root_graph_boundaries::{ControlRigBoundaryNode, ControlRigNodeState, RigCallSample, current_control_rig}};

use crate::animation_context_projection::AlsRootAnimationContextSet;
use crate::character_assembly::AlsCharacterAssembly;
use crate::control_rig_apply::{AlsControlRigExecution, AlsControlRigInput, CrAlsRuntimeProfileTemplate};

#[derive(Component, Debug, Clone, Copy, Default)]
pub(crate) struct FootLockLifecycleState {
    previous_character_transform: Option<Transform>,
    previous_actor_location: Option<Vec3>,
    teleported_at: Option<f64>,
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
                    .before(crate::ab_als_provider::AbAlsProviderSet)
                    .after(crate::als::motion::AlsRotationResponseSet)
                    .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
                FootIkApplySet::Telemetry.after(AnimationGraphSet::Final)
                    .after(AlsRootAnimationContextSet).after(TransformSystems::Propagate),
            )).add_systems(PostUpdate, (
                refresh_current_foot_bones.before(ensure_foot_lock_components).before(FootIkApplySet::UpdateInputs),
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

pub(crate) fn refresh_current_foot_bones(
    mut commands: Commands,
    mut characters: Query<(
        Entity,
        &AlsCharacterAssembly,
        Option<&AlsFootBones>,
        Option<&mut AlsFootLockInput>,
        Option<&mut AlsFootLockState>,
        Option<&mut AlsDynamicTransitionState>,
        Option<&mut FootLockLifecycleState>,
        Option<&mut AlsFootLockTeleportTelemetry>,
    ), With<crate::als::character::AlsCharacter>>,
    instances: Query<&AnimatedSceneInstance>,
    children: Query<&Children>,
    names: Query<&Name>,
    targets: Query<&AnimationTargetId>,
) {
    for (entity, assembly, current, input, lock, dynamic, lifecycle, telemetry) in &mut characters {
        let root = instances
            .get(assembly.avatar)
            .map_or(assembly.avatar, |instance| instance.player_entity());
        let next = build_foot_bone_map(root, &children, &names, &targets);
        if current.copied() == next {
            continue;
        }
        if let Some(next) = next {
            commands.entity(entity).insert(next);
        } else {
            commands.entity(entity).remove::<AlsFootBones>();
        }
        if let Some(mut value) = input { *value = default(); }
        if let Some(mut value) = lock { *value = default(); }
        if let Some(mut value) = dynamic { *value = default(); }
        if let Some(mut value) = lifecycle { *value = default(); }
        if let Some(mut value) = telemetry { *value = default(); }
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
    characters: Query<(Entity, &AlsCharacterAssembly, Option<&AlsFootBones>, Option<&AlsFootLockState>, Option<&AlsFootIkSpaceState>, Option<&AlsControlRigExecution>), With<crate::als::character::AlsCharacter>>,
    instances: Query<&AnimatedSceneInstance>,
    players: Query<&AnimationGraphPlayer>,
    graphs: Option<Res<Assets<AnimationGraph>>>,
    names: Query<&Name>,
    transforms: TransformHelper,
) {
    for (entity, assembly, bones, lock, previous, previous_execution) in &characters {
        let mut execution = AlsControlRigExecution::default();
        let mut telemetry = AlsFootIkTelemetry::default();
        let mut spaces = AlsFootIkSpaceState::default();
        let mut apply = AlsFootIkApplyTelemetry::default();
        if let Some(graphs) = graphs.as_deref()
            && let Ok(instance) = instances.get(assembly.avatar)
            && let Ok(player) = players.get(instance.player_entity())
            && let Some((_, node, state)) = current_control_rig(player, graphs)
            && let Some(definition) = node.try_inner_downcast_ref::<ControlRigBoundaryNode>().and_then(|node| node.definition.as_ref())
        {
            let value = |name| definition.externals.get(name).and_then(|value| state.value(value).ok()).as_ref().and_then(rig_scalar);
            execution = AlsControlRigExecution { initialization_count: state.initialization_count, executed_steps: state.executed_steps,
                frame: player.get_context_arena().unwrap().frame_index(), elapsed_seconds: state.elapsed_seconds,
                foot_height_m: value("FootHeight").unwrap_or(0.0), leg_length_m: value("LegLength").unwrap_or(0.0) };
            if let Some(bones) = bones && let Ok(world) = transforms.compute_global_transform(assembly.avatar) {
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
        (Entity, &AlsCharacterAssembly, &mut FootLockLifecycleState),
        (With<crate::als::character::AlsCharacter>, With<AlsFootBones>),
    >,
    transforms: TransformHelper,
    mut telemetry: Query<&mut AlsFootLockTeleportTelemetry>,
    instances: Query<&AnimatedSceneInstance>,
    update_gates: Query<&crate::extensions::post_physics_animation::AnimationPoseUpdateGate>,
) {
    for (entity, assembly, mut lifecycle) in &mut lifecycles {
        let Ok(actor) = transforms.compute_global_transform(entity) else { continue };
        let current = actor.translation();
        let Some(previous) = lifecycle.previous_actor_location.replace(current) else { continue; };
        if instances.get(assembly.avatar).ok().and_then(|instance| update_gates.get(instance.player_entity()).ok())
            .is_some_and(|gate| gate.pending_update) { continue; }
        let translation_delta_m = previous.distance(current);
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
    time: Res<Time>,
    mut commands: Commands,
    mut teleports: MessageReader<AlsFootLockTeleported>,
    mut lifecycles: Query<&mut FootLockLifecycleState>,
    mut telemetry: Query<&mut AlsFootLockTeleportTelemetry>,
) {
    for teleport in teleports.read() {
        if let Ok(mut lifecycle) = lifecycles.get_mut(teleport.entity) {
            lifecycle.teleported_at = Some(time.elapsed_secs_f64());
        } else {
            commands
                .entity(teleport.entity)
                .insert(FootLockLifecycleState {
                    previous_character_transform: None,
                    previous_actor_location: None,
                    teleported_at: Some(time.elapsed_secs_f64()),
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
    ), (With<crate::als::character::AlsCharacter>, Without<AlsFootBones>)>,
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
    transforms: &Query<&GlobalTransform, Without<crate::als::character::AlsCharacter>>,
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
    state: &mut crate::als::foot_ik::AlsFootLockFootState,
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
        state.lock_rotation = (rotation_delta * state.lock_rotation_before_foot_limit).normalize();
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
    if let Some(offset_rotation) = foot_lock_location_constraint_rotation(
        thigh_axis_armature,
        lock_location_armature,
        settings.thigh_angle_limit_degrees,
    ) {
        lock_location_armature = offset_rotation * lock_location_armature;
        lock_rotation_armature = (offset_rotation * lock_rotation_armature).normalize();
        state.lock_location =
            world_point_from_armature(armature, ArmaturePoint(lock_location_armature)).0;
        state.lock_rotation =
            world_rotation_from_armature(armature, ArmatureRotation(lock_rotation_armature)).0;
        capture_foot_lock_movement_base(state, state.movement_base);
        state.lock_rotation_before_foot_limit = state.lock_rotation;
    }
    state.lock_rotation = constrain_foot_lock_rotation(
        state.lock_rotation,
        state.target_rotation,
        settings.foot_angle_limit_degrees,
    );
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
        Entity, &AlsCharacterAssembly, &AlsFootBones, Option<&AlsFootTargetSelection>, &LocomotionState,
        &AlsFootLockInput, &Stance, &AlsFootLockSettings, Option<&AlsMovementBase>,
        &mut FootLockLifecycleState, &mut AlsFootLockState, &mut AlsDynamicTransitionState,
        Option<&mut AlsControlRigInput>,
        Option<&crate::als_camera::AlsCameraCustomTimeDilation>,
    ), With<crate::als::character::AlsCharacter>>,
    transforms: TransformHelper,
    parents: Query<&ChildOf>,
    bone_globals: Query<&GlobalTransform, Without<crate::als::character::AlsCharacter>>,
    mut telemetry: Query<&mut AlsFootLockTeleportTelemetry>,
    owners: Query<&ColliderOf>,
    bodies: Query<&RigidBody>,
    instances: Query<&AnimatedSceneInstance>,
    update_gates: Query<&crate::extensions::post_physics_animation::AnimationPoseUpdateGate>,
) {
    for (entity, assembly, bones, selection, locomotion, lock_input, stance, settings, movement_base,
        mut lifecycle, mut lock_state,
        mut dynamic_transition, mut rig_input, custom_dilation) in &mut characters {
        let dt = time.delta_secs() * custom_dilation.map_or(1.0, |value| value.0);
        if instances.get(assembly.avatar).ok().and_then(|instance| update_gates.get(instance.player_entity()).ok())
            .is_some_and(|gate| gate.pending_update) {
            lock_state.valid = false;
            lock_state.became_valid = false;
            if let Some(input) = rig_input.as_mut() { input.foot_transforms_valid = false; }
            continue;
        }
        let Ok(armature) = transforms.compute_global_transform(assembly.avatar) else { continue };
        let current_character_transform = armature.compute_transform();
        if !lock_state.valid
            && let Ok(pelvis_parent) = parents.get(bones.left_thigh)
            && let Ok(pelvis_world) = transforms.compute_global_transform(pelvis_parent.parent())
        {
            let pelvis_component = pelvis_world.reparented_to(&armature);
            if pelvis_component.translation.abs().max_element() <= 1e-6
                && (pelvis_component.rotation.abs_diff_eq(Quat::IDENTITY, 1e-4)
                    || pelvis_component.rotation.abs_diff_eq(-Quat::IDENTITY, 1e-4))
            {
                if let Some(input) = rig_input.as_mut() { input.foot_transforms_valid = false; }
                continue;
            }
        }
        let use_ik_bones = selection.map_or(true, |selection| selection.use_ik_bones);
        if !foot_bones_are_valid(bones, use_ik_bones, &bone_globals) {
            if let Some(input) = rig_input.as_mut() { input.foot_transforms_valid = false; }
            *lock_state = AlsFootLockState::default();
            *dynamic_transition = default();
            lifecycle.previous_character_transform = Some(current_character_transform);
            lifecycle.teleported_at = None;
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
                .and_then(|entity| foot_lock_movement_base(entity, &transforms, &owners, &bodies))
        });
        let teleport_elapsed = lifecycle.teleported_at.map(|marked_at| time.elapsed_secs_f64() - marked_at);
        let teleport_active = teleport_elapsed.is_some_and(|elapsed| elapsed <= f64::from(FOOT_LOCK_TELEPORT_WINDOW_SECONDS));
        if let Some(previous_character_transform) = lifecycle.previous_character_transform {
            for (side, ik_amount) in [left_ik_amount, right_ik_amount].into_iter().enumerate() {
                let foot_state = if side == 0 { &mut lock_state.left } else { &mut lock_state.right };
                let rebase_lock = teleport_active && !became_valid
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
                current_base,
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
        if let Ok(mut value) = telemetry.get_mut(entity) {
            value.remaining_seconds = teleport_elapsed.map_or(0.0, |elapsed| (f64::from(FOOT_LOCK_TELEPORT_WINDOW_SECONDS) - elapsed).max(0.0) as f32);
        }
    }
}

fn foot_lock_movement_base(
    entity: Entity,
    transforms: &TransformHelper,
    owners: &Query<&ColliderOf>,
    bodies: &Query<&RigidBody>,
) -> Option<AlsFootLockMovementBase> {
    let owner = owners.get(entity).map_or(entity, |collider| collider.body);
    if !bodies.get(owner).is_ok_and(|body| *body != RigidBody::Static) { return None; }
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
    fn source_foot_twist_reaches_the_actual_rig_rotation() {
        use std::sync::{Arc, Mutex};
        for delta in [Quat::IDENTITY, Quat::from_rotation_y(90.0_f32.to_radians()), Quat::from_xyzw(0.0, 1.0, 0.0, 0.0), Quat::from_xyzw(0.0, -1.0, 0.0, 0.0), Quat::from_xyzw(1.0, 0.000_01, 0.0, 0.000_01).normalize()] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            for _ in 0..5 { app.update(); }
            let pelvis = app.world().get::<ChildOf>(app.world().get::<AlsFootBones>(actor).unwrap().left_thigh).unwrap().parent();
            let target = app.world().get::<AlsFootBones>(actor).unwrap().target(0, true);
            let source = Arc::new(Mutex::new(None));
            let capture = source.clone();
            app.add_systems(PostUpdate,
                (move |mut states: Query<(&mut AlsFootLockState, &mut AlsFootLockInput, &mut LocomotionState)>,
                    mut transforms: ParamSet<(Query<&mut Transform>, TransformHelper)>,
                    mut profile: ResMut<CrAlsRuntimeProfileTemplate>| {
                    transforms.p0().get_mut(pelvis).unwrap().rotation = Quat::IDENTITY;
                    profile.0.target.foot_left_thigh_axis_pelvis_space = Vec3::X.to_array();
                    let target_world = transforms.p1().compute_global_transform(target).unwrap();
                    let mesh = transforms.p1().compute_global_transform(avatar).unwrap();
                    let (mut lock, mut input, mut mode) = states.get_mut(actor).unwrap();
                    assert!(lock.valid && !lock.became_valid);
                    lock.left.lock_amount = 1.0;
                    lock.left.lock_location = mesh.affine().transform_point3(Vec3::new(0.2, -0.8, 0.0));
                    lock.left.lock_rotation = (delta * target_world.rotation()).normalize();
                    lock.left.lock_rotation_before_foot_limit = lock.left.lock_rotation;
                    lock.left.movement_base = None;
                    let actual_delta = lock.left.lock_rotation * target_world.rotation().conjugate();
                    let mut angle = 2.0 * actual_delta.y.atan2(actual_delta.w);
                    while angle > std::f32::consts::PI { angle -= std::f32::consts::TAU; }
                    while angle < -std::f32::consts::PI { angle += std::f32::consts::TAU; }
                    let degrees = angle.to_degrees();
                    let expected = if degrees.abs() > 40.0 + 1e-4 {
                        Quat::from_rotation_y((degrees.clamp(-40.0, 40.0) - degrees).to_radians()) * lock.left.lock_rotation
                    } else { lock.left.lock_rotation };
                    *capture.lock().unwrap() = Some(expected.normalize());
                    input.left_ik_amount = 1.0;
                    input.left_lock_amount = 1.0;
                    input.moving_smooth = false;
                    *mode = LocomotionState::Grounded;
                }).after(crate::standing_layer::AlsStandingLayerSet).before(FootIkApplySet::UpdateInputs));
            app.update();
            let source = source.lock().unwrap().unwrap();
            let rig = app.world().get::<AlsControlRigInput>(actor).unwrap();
            assert!(rig.foot_transforms_valid);
            let mesh = app.world().get::<GlobalTransform>(avatar).unwrap();
            let actual = (mesh.rotation() * rig.foot_left_rotation_armature).normalize();
            assert!(actual.angle_between(source) < 1e-4,
                "Source Foot twist differs at the actual Rig: delta={delta:?}, actual={actual:?}, source={source:?}");
        }
    }

    #[test]
    fn source_signed_thigh_angle_reaches_the_actual_foot_rig() {
        for (source_lock, source_final) in [
            (Vec3::new(-0.2, -0.8, 0.0), Vec3::new(-0.2, -0.8, 0.0)),
            (Vec3::new(0.2, -0.8, 0.0), Vec3::new(0.2, -0.8, 0.0)),
            (Vec3::new(-0.1, -0.8, 0.17320508), Vec3::new(0.0, -0.8, 0.2)),
            (Vec3::new(-0.000_002, -0.8, 0.000_003_464_101_6), Vec3::new(0.0, -0.8, 0.000_004)),
        ] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            for _ in 0..5 { app.update(); }
            let pelvis = app.world().get::<ChildOf>(app.world().get::<AlsFootBones>(actor).unwrap().left_thigh).unwrap().parent();
            app.add_systems(PostUpdate,
                (move |mut states: Query<(&mut AlsFootLockState, &mut AlsFootLockInput, &mut LocomotionState)>,
                    mut transforms: ParamSet<(Query<&mut Transform>, TransformHelper)>,
                    mut profile: ResMut<CrAlsRuntimeProfileTemplate>| {
                    transforms.p0().get_mut(pelvis).unwrap().rotation = Quat::IDENTITY;
                    profile.0.target.foot_left_thigh_axis_pelvis_space = Vec3::X.to_array();
                    let (mut lock, mut input, mut mode) = states.get_mut(actor).unwrap();
                    assert!(lock.valid && !lock.became_valid);
                    lock.left.lock_amount = 1.0;
                    let mesh = transforms.p1().compute_global_transform(avatar).unwrap();
                    lock.left.lock_location = mesh.affine().transform_point3(source_lock);
                    let recovered = mesh.affine().inverse().transform_point3(lock.left.lock_location);
                    assert!(recovered.distance(source_lock) < 1e-6);
                    if source_lock.z == 0.0 { assert_eq!(recovered.z, 0.0, "Fixture must preserve exact collinear component axes"); }
                    lock.left.lock_rotation = Quat::IDENTITY;
                    lock.left.lock_rotation_before_foot_limit = Quat::IDENTITY;
                    lock.left.movement_base = None;
                    input.left_ik_amount = 1.0;
                    input.left_lock_amount = 1.0;
                    input.moving_smooth = false;
                    *mode = LocomotionState::Grounded;
                }).after(crate::standing_layer::AlsStandingLayerSet).before(FootIkApplySet::UpdateInputs));
            app.update();
            let rig = app.world().get::<AlsControlRigInput>(actor).unwrap();
            assert!(rig.foot_transforms_valid);
            let expected = source_final;
            assert!(rig.foot_left_location_armature.distance(expected) < 1e-4,
                "Source signed thigh angle differs at the actual Rig: input={source_lock:?}, actual={:?}, expected={expected:?}", rig.foot_left_location_armature);
            let actual_horizontal = Vec2::new(rig.foot_left_location_armature.x, rig.foot_left_location_armature.z).normalize();
            let source_horizontal = Vec2::new(expected.x, expected.z).normalize();
            assert!(actual_horizontal.distance(source_horizontal) < 1e-4,
                "Source normalization boundary must reach the actual Rig direction: input={source_lock:?}, actual={actual_horizontal:?}, source={source_horizontal:?}");
        }
    }

    #[test]
    fn moving_foot_lock_uses_the_source_actor_animation_delta() {
        for (custom_dilation, moving_smooth, decrease_speed) in [(1.0_f32, true, 5.0_f32), (0.5, true, 5.0), (0.0, true, 5.0), (0.5, false, 0.6)] {
            #[derive(Resource, Default)]
            struct SourceClockInput(Option<(f32, Vec3)>);
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            for _ in 0..5 { app.update(); }
            app.world_mut().entity_mut(actor).insert(crate::als_camera::AlsCameraCustomTimeDilation(custom_dilation));
            app.init_resource::<SourceClockInput>().add_systems(PostUpdate,
                (move |mut states: Query<(&mut AlsFootLockState, &mut AlsFootLockInput, &mut LocomotionState)>,
                    time: Res<Time>, mut source: ResMut<SourceClockInput>| {
                    let (mut lock, mut input, mut mode) = states.get_mut(actor).unwrap();
                    assert!(lock.valid);
                    lock.left.lock_amount = 1.0;
                    lock.left.lock_location = lock.left.target_location + Vec3::X * 0.1;
                    input.left_ik_amount = 1.0;
                    input.left_lock_amount = 1.0;
                    input.moving_smooth = moving_smooth;
                    *mode = if moving_smooth { LocomotionState::Grounded } else { LocomotionState::Air };
                    let native_delta = time.delta_secs() * custom_dilation;
                    source.0 = Some((native_delta, lock.left.lock_location));
                }).after(crate::standing_layer::AlsStandingLayerSet).before(FootIkApplySet::UpdateInputs));
            app.update();
            let (native_delta, source_lock) = app.world().resource::<SourceClockInput>().0.unwrap();
            let expected_amount = (1.0 - native_delta * decrease_speed).max(0.0);
            let actual = app.world().get::<AlsFootLockState>(actor).unwrap();
            let source_final = actual.left.target_location.lerp(source_lock, expected_amount);
            let rig = app.world().get::<AlsControlRigInput>(actor).unwrap();
            let mesh = app.world().get::<GlobalTransform>(avatar).unwrap();
            let actual_final = mesh.affine().transform_point3(rig.foot_left_location_armature);
            assert!(actual_final.distance(source_final) < 1e-4,
                "Foot lock uses the wrong actor clock: actual={actual_final:?}, source={source_final:?}, native_delta={native_delta}, custom={custom_dilation}");
            assert!((actual.left.lock_amount - expected_amount).abs() < 1e-6);
        }
    }

    #[test]
    fn near_full_source_weight_keeps_the_existing_foot_lock() {
        for (requested_amount, source_amount) in [(0.999_95_f32, 0.5_f32), (1.0, 1.0), (0.000_02, 0.000_02), (0.000_01, 0.0)] {
            #[derive(Resource, Default)]
            struct SourceFootInput(Option<(f32, f32, Vec3)>);
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            for _ in 0..5 { app.update(); }
            app.init_resource::<SourceFootInput>().add_systems(PostUpdate,
                (move |mut states: Query<(&mut AlsFootLockState, &mut AlsFootLockInput)>,
                    mut source: ResMut<SourceFootInput>| {
                    let (mut lock, mut input) = states.get_mut(actor).unwrap();
                    assert!(lock.valid);
                    lock.left.lock_amount = 0.5;
                    lock.left.lock_location = lock.left.target_location + Vec3::X * 0.1;
                    input.left_ik_amount = 1.0;
                    input.left_lock_amount = requested_amount;
                    input.moving_smooth = false;
                    source.0 = Some((lock.left.lock_amount, input.left_lock_amount, lock.left.lock_location));
                }).after(crate::standing_layer::AlsStandingLayerSet).before(FootIkApplySet::UpdateInputs));
            app.update();
            let (previous, requested, source_lock) = app.world().resource::<SourceFootInput>().0.unwrap();
            assert_eq!(requested, requested_amount);
            let lock = app.world().get::<AlsFootLockState>(actor).unwrap();
            let rig = app.world().get::<AlsControlRigInput>(actor).unwrap();
            let mesh = app.world().get::<GlobalTransform>(avatar).unwrap();
            if requested < 1.0 - 1e-5 {
                let source_final = lock.left.target_location.lerp(source_lock, source_amount);
                assert!(mesh.affine().transform_point3(rig.foot_left_location_armature).distance(source_final) < 1e-4,
                    "Source FAnimWeight full threshold must reach the actual Rig foot input: requested={requested}");
                if source_amount > 0.0 { assert!(lock.left.lock_location.distance(source_lock) < 1e-5); }
            }
            assert_eq!(lock.left.lock_amount, source_amount,
                "Source FAnimWeight relevant/full boundary differs: previous={previous}, requested={requested}");
        }
    }

    #[test]
    fn invalid_feet_with_identity_pelvis_do_not_execute_the_foot_rig() {
        for prior_valid in [false, true] {
            let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
            app.world_mut().resource_mut::<AlsFootIkTelemetryEnabled>().0 = true;
            let bones = *app.world().get::<AlsFootBones>(actor).unwrap();
            let pelvis = app.world().get::<ChildOf>(bones.left_thigh).unwrap().parent();
            let pelvis_parent = app.world().get::<ChildOf>(pelvis).unwrap().parent();
            app.world_mut().get_mut::<AlsFootLockState>(actor).unwrap().valid = prior_valid;
            app.world_mut().get_mut::<AlsFootLockState>(actor).unwrap().became_valid = false;
            app.add_systems(PostUpdate, (move |mut space: ParamSet<(TransformHelper, Query<&mut Transform>)>| {
                let mesh = space.p0().compute_global_transform(avatar).unwrap();
                let parent = space.p0().compute_global_transform(pelvis_parent).unwrap();
                let source_pose = GlobalTransform::from(mesh.compute_transform()
                    .with_scale(mesh.to_scale_rotation_translation().0 * 2.0));
                *space.p1().get_mut(pelvis).unwrap() = source_pose.reparented_to(&parent);
                let actual = space.p0().compute_global_transform(pelvis).unwrap();
                let component = actual.reparented_to(&mesh);
                assert!(component.translation.abs().max_element() <= 1e-6 && component.rotation.abs_diff_eq(Quat::IDENTITY, 1e-4),
                    "Source pelvis identity fixture must use component space: {component:?}");
            }).before(FootIkApplySet::UpdateInputs).after(crate::als::motion::AlsActualVelocitySet));
            app.update();
            let instance = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            assert!(instance.get_error().is_none());
            let graphs = app.world().resource::<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>();
            let (_, node, state) = current_control_rig(instance, graphs).unwrap();
            let definition = node.try_inner_downcast_ref::<ControlRigBoundaryNode>().unwrap().definition.as_ref().unwrap();
            let calls = state.calls.iter().filter(|call| rig_call_is(definition, call, "FAlsRigUnit_ApplyFootOffsetLocation::Execute")
                || rig_call_is(definition, call, "FAlsRigUnit_ApplyFootOffsetRotation::Execute")
                || rig_call_is(definition, call, "FRigUnit_TwoBoneIKSimplePerItem::Execute")).count();
            assert_eq!(calls, if prior_valid { 8 } else { 2 },
                "Source pelvis identity is a first-validity gate and ignores scale: prior_valid={prior_valid}");
            assert_eq!(app.world().get::<AlsFootLockState>(actor).unwrap().valid, prior_valid);
        }
    }
    use crate::character_assembly::AlsCharacterAvatar;
    use bevy_animation_graph::core::animation_clip::EntityPath;

    #[test]
    fn animation_movement_base_foot_lock_keeps_custom_targets_and_rejects_static_owners() {
        use crate::als::dynamics::AlsMovementBase;
        use avian3d::prelude::*;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            TransformPlugin, PhysicsPlugins::new(PostUpdate), FootIkApplyPlugin));
        app.finish();
        app.cleanup();
        let body = app.world_mut().spawn((RigidBody::Kinematic, Transform::IDENTITY)).id();
        let child = app.world_mut().spawn((Collider::cuboid(3.0, 1.0, 3.0), ChildOf(body), Transform::IDENTITY)).id();
        app.update();
        assert_eq!(app.world().get::<ColliderOf>(child).unwrap().body, body);
        let character = app.world_mut().spawn((crate::als::character::AlsCharacter, Transform::from_xyz(0.0, 2.0, 0.0),
            LocomotionState::Grounded, Stance::Standing, AlsFootLockSettings::default(),
            AlsFootLockInput { left_ik_amount:1.0, right_ik_amount:1.0, left_lock_amount:1.0, right_lock_amount:1.0, ..default() },
            AlsControlRigInput::default())).id();
        let avatar = app.world_mut().spawn((AlsCharacterAvatar { character }, Transform::IDENTITY, ChildOf(character))).id();
        let pelvis = app.world_mut().spawn((Transform::from_xyz(0.0, 0.9, 0.0), ChildOf(avatar))).id();
        let thigh = app.world_mut().spawn((Transform::IDENTITY, ChildOf(pelvis))).id();
        let calf = app.world_mut().spawn((Transform::from_xyz(0.0, -0.4, 0.0), ChildOf(thigh))).id();
        let foot = app.world_mut().spawn((Transform::from_xyz(0.0, -0.4, 0.0), ChildOf(calf))).id();
        let right_thigh = app.world_mut().spawn((Transform::IDENTITY, ChildOf(pelvis))).id();
        let right_calf = app.world_mut().spawn((Transform::from_xyz(0.0, -0.4, 0.0), ChildOf(right_thigh))).id();
        let right_foot = app.world_mut().spawn((Transform::from_xyz(0.0, -0.4, 0.0), ChildOf(right_calf))).id();
        for (name, bone) in ["thigh_l", "calf_l", "foot_l", "thigh_r", "calf_r", "foot_r"].into_iter()
            .zip([thigh, calf, foot, right_thigh, right_calf, right_foot]) {
            app.world_mut().entity_mut(bone).insert((Name::new(name), EntityPath::from(vec![name.to_owned()]).id().animation_target_id()));
        }
        app.world_mut().entity_mut(character).insert(AlsCharacterAssembly { avatar, camera: None });
        app.world_mut().entity_mut(character).insert(AlsFootBones {
            left_thigh:thigh, left_calf:calf, left:foot, left_target:None, left_virtual_target:None,
            right_thigh, right_calf, right:right_foot, right_target:None, right_virtual_target:None,
        });
        for (kind, target, mode) in [(RigidBody::Kinematic, Some(child), LocomotionState::Grounded),
            (RigidBody::Kinematic, Some(child), LocomotionState::Custom),
            (RigidBody::Static, Some(child), LocomotionState::Grounded),
            (RigidBody::Dynamic, Some(body), LocomotionState::Grounded),
            (RigidBody::Dynamic, None, LocomotionState::Air)] {
            app.world_mut().entity_mut(body).insert(kind);
            app.world_mut().entity_mut(character).insert(mode);
            app.world_mut().get_mut::<AlsMovementBase>(character).unwrap().entity = target;
            app.update();
            let feet = app.world().get::<AlsFootLockState>(character).unwrap();
            assert!(feet.valid && feet.left.lock_amount > 0.0);
            let expected = if kind == RigidBody::Static { None } else { target };
            assert_eq!(feet.left.movement_base.map(|base| base.entity), expected, "Foot base differs: {kind:?}, {mode:?}");
            assert_eq!(feet.right.movement_base.map(|base| base.entity), expected);
        }
    }

    #[test]
    fn rotating_base_reversal_keeps_unclamped_rotation_history() {
        let base = AlsFootLockMovementBase { entity:Entity::PLACEHOLDER, translation:Vec3::ZERO, rotation:Quat::IDENTITY };
        let settings = AlsFootLockSettings::default();
        for armature_rotation in [Quat::IDENTITY, Quat::from_rotation_x(0.7)] {
            let mut foot = AlsFootLockFootState { lock_amount:1.0, ..default() };
            let armature = GlobalTransform::from(Transform::from_rotation(armature_rotation));
            capture_foot_lock_movement_base(&mut foot, Some(base));
            for angle in [100.0_f32, 90.0] {
                let current = AlsFootLockMovementBase { rotation:Quat::from_rotation_y(angle.to_radians()), ..base };
                refresh_foot_lock(&mut foot, Vec3::ZERO, Quat::IDENTITY, 1.0, 1.0, true, false, true, false, 1.0 / 60.0, Some(current));
                source_constrain_foot_lock(&mut foot, &armature, &armature, Vec3::X, &settings);
                let expected = Quat::from_rotation_y(settings.foot_angle_limit_degrees.to_radians());
                assert!(foot.lock_rotation.angle_between(expected) < 1e-5,
                    "Foot clamp changed source base-space history: angle={angle} actual={:?} expected={expected:?}", foot.lock_rotation);
                assert!(foot.lock_rotation_movement_base.angle_between(Quat::IDENTITY) < 1e-5);
                assert!(foot.lock_rotation_before_foot_limit.angle_between(current.rotation) < 1e-5);
            }
            let current = Transform::from_rotation(Quat::from_rotation_y(-0.3));
            refresh_previous_foot_lock_transform(&mut foot, &armature.compute_transform(), &current, true);
            let expected = current.rotation * armature_rotation.conjugate() * Quat::from_rotation_y(90.0_f32.to_radians());
            assert!(foot.lock_rotation.angle_between(expected) < 1e-5, "Teleport must use the component lock before the world foot-angle limit");
        }
    }

    #[test]
    fn rotating_base_retains_thigh_constraint_when_direction_reverses() {
        let base = AlsFootLockMovementBase { entity:Entity::PLACEHOLDER, translation:Vec3::ZERO, rotation:Quat::IDENTITY };
        let mut foot = AlsFootLockFootState { lock_amount:1.0, lock_location:Vec3::X, ..default() };
        capture_foot_lock_movement_base(&mut foot, Some(base));
        for (angle, expected_angle) in [(100.0_f32, 90.0_f32), (90.0, 80.0)] {
            let current = AlsFootLockMovementBase { rotation:Quat::from_rotation_y(angle.to_radians()), ..base };
            refresh_foot_lock(&mut foot, Vec3::X, Quat::IDENTITY, 1.0, 1.0, true, false, true, false, 1.0 / 60.0, Some(current));
            source_constrain_foot_lock(&mut foot, &GlobalTransform::IDENTITY, &GlobalTransform::IDENTITY, Vec3::X, &AlsFootLockSettings::default());
            let expected = Quat::from_rotation_y(expected_angle.to_radians());
            assert!(foot.lock_location.distance(expected * Vec3::X) < 1e-5);
            assert!(foot.lock_rotation_before_foot_limit.angle_between(expected) < 1e-5);
        }
    }

    #[test]
    fn rig_observer_reads_actual_execution_hits_weights_and_pose_delivery() {
        use crate::extensions::{control_rig_query::{RigQueryContext, RigQueryWorld}, SourceAnimationInputs,
            root_graph_boundaries::{RigInputBinding, RIG_CAPTURE_KEY, RIG_GAME_WORLD_KEY}};
        use avian3d::{collider_tree::ColliderTrees, prelude::*};
        use bevy::{animation::AnimatedBy, ecs::system::RunSystemOnce,
            world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner}};
        use bevy_animation_graph::{AnimationGraphPlugin, core::{animated_scene::AnimatedSceneHandle,
            animation_clip::EntityPath, animation_node::AnimationNode, edge_data::{DataSpec, DataValue},
            pose::{BonePose, Pose}, skeleton::Skeleton, systems::apply_animation_to_targets}};
        fn capture(In((entity, to_world)): In<(Entity, RigTransform)>, mut inputs: Query<&mut SourceAnimationInputs>, trees: Res<ColliderTrees>, colliders: Query<(Entity, &Position, &Rotation, &Collider)>) {
            inputs.get_mut(entity).unwrap().set_rig_query_context(Some(RigQueryContext { world: std::sync::Arc::new(RigQueryWorld {
                trees: trees.clone(), colliders: colliders.iter().map(|(entity, position, rotation, collider)| (entity, (*position, *rotation, collider.clone()))).collect(),
            }), to_world, channel: "ECC_Visibility".into(), filter: default() }));
        }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin, bevy::gizmos::GizmoPlugin,
            PhysicsPlugins::new(PostUpdate), AnimationGraphPlugin::from_physics_schedule(PostUpdate), crate::extensions::AlsExtPlugin))
            .init_asset::<WorldAsset>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20.0, 0.2, 20.0), Transform::IDENTITY));
        for _ in 0..3 { app.update(); }
        app.world_mut().resource_mut::<bevy::ecs::schedule::Schedules>().remove(PostUpdate);
        let definition: RigDefinition = serde_json::from_str(include_str!("../../../assets/ALS/Character/CR_Als.program.json")).unwrap();
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
        let source = AnimationNode::new("input", crate::extensions::GlobalPoseInputNode { key: "input".into(), output: "Pose".into() });
        let curves = AnimationNode::new("source curves", crate::extensions::source_pose_nodes::SourceCurveModifyNode {
            source_pose_pin: "Source".into(), curve_names: vec!["FootLeftIk".into(), "FootRightIk".into(), "LayerArmLeft".into(), "LayerArmRight".into(), "Unrelated".into()],
            pin_defaults: vec![1.0, 1.0, 1.0, 1.0, 0.37], alpha: 1.0, ..default() });
        let rig = AnimationNode::new("rig", ControlRigBoundaryNode { definition: Some(definition.clone()),
            input_bindings: [0, 1, 2, 5, 7, 8, 9, 10].map(|index| RigInputBinding { external: "RigInput".into(), path: vec![index], property: format!("input_{index}") }).into(),
            capture_context: true, internalize_time: true, ..default() });
        for node in [&source, &curves, &rig] { graph.add_node(node.clone()); }
        graph.add_node_parameter_edge(source.id, "Pose", curves.id, "Source");
        graph.add_node_parameter_edge(curves.id, "Pose", rig.id, "source_pose");
        graph.add_output_data("pose".into(), DataSpec::Pose);
        graph.add_output_data_edge(rig.id, "Pose", "pose");
        let original_edges = graph.edges.clone();
        let original_inputs = graph.edges_inverted.clone();
        let original_node_count = graph.nodes.len();
        let graph = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(graph);
        let mut wrapper = AnimationGraph::new();
        let linked = AnimationNode::new("wrapped rig", crate::extensions::LayeredGraphNode {
            capture_context: true, internalize_time: true, ..crate::extensions::LayeredGraphNode::new(graph.clone()) });
        wrapper.add_node(linked.clone());
        wrapper.add_output_data("pose".into(), DataSpec::Pose);
        wrapper.add_output_data_edge(linked.id, "pose", "pose");
        let wrapper = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(wrapper);
        let mut player = AnimationGraphPlayer::new(skeleton).with_graph(wrapper.clone());
        player.set_global_input_data("input", DataValue::Pose(pose.clone()));
        player.set_global_input_data("animation.delta_seconds", 1.0_f32 / 60.0);
        player.set_global_input_data(RIG_CAPTURE_KEY, true);
        player.set_global_input_data(RIG_GAME_WORLD_KEY, true);
        player.set_global_input_data("input_0", true);
        player.set_global_input_data("input_1", true);
        player.set_global_input_data("input_2", true);
        player.set_global_input_data("input_5", 0.0_f32);
        for (name, location, rotation) in [("foot_l", "input_7", "input_8"), ("foot_r", "input_9", "input_10")] {
            let transform = definition.bones.iter().find(|bone| bone.name == name).unwrap().initial_global;
            player.set_global_input_data(location, transform.translation.as_vec3());
            player.set_global_input_data(rotation, transform.rotation.as_quat());
        }
        let actor = app.world_mut().spawn((crate::als::character::AlsCharacter, Transform::IDENTITY, AlsFootLockState::default())).id();
        let world = Transform::from_xyz(2.0, 0.0, 3.0).with_rotation(Quat::from_rotation_y(0.4));
        let to_world = RigTransform { translation: world.translation.as_dvec3(), rotation: world.rotation.as_dquat(), scale: world.scale.as_dvec3() };
        let avatar = app.world_mut().spawn((AlsCharacterAvatar { character: actor }, world, ChildOf(actor), AnimatedSceneHandle::default())).id();
        app.world_mut().entity_mut(actor).insert(AlsCharacterAssembly { avatar, camera: None });
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
        app.world_mut().run_system_once_with(capture, (player_entity, to_world)).unwrap();
        app.world_mut().run_system_once(crate::extensions::post_physics_animation::evaluate_players::<()> ).unwrap();
        let mut driver = None;
        for (frame, weight) in [1.0_f32, 0.25, 0.0].into_iter().enumerate() {
            let previous_driver = driver;
            crate::manual_playground_integration::run_manual_rig_curve_fixture(app.world_mut(), actor,
                AlsFootIkWeightOverride { left: Some(weight), right: Some(weight) }, false);
            let current = app.world().resource::<Assets<AnimationGraph>>().get(&graph).unwrap();
            driver = match current.edges_inverted.get(&bevy_animation_graph::core::animation_graph::TargetPin::NodeData(rig.id, "source_pose".into())) {
                Some(bevy_animation_graph::core::animation_graph::SourcePin::NodeData(id, _)) if *id != curves.id => Some(*id),
                _ => None,
            };
            assert!(driver.is_some());
            if previous_driver.is_some() { assert_eq!(driver, previous_driver); }
            app.world_mut().run_system_once_with(capture, (player_entity, to_world)).unwrap();
            app.world_mut().run_system_once(crate::extensions::post_physics_animation::evaluate_players::<()> ).unwrap();
            assert!(app.world().get::<AnimationGraphPlayer>(player_entity).unwrap().get_error().is_none());
            app.world_mut().run_system_once(apply_animation_to_targets).unwrap();
            app.world_mut().run_system_once(observe_rig_results).unwrap();
            let (context, _, rig_state) = current_control_rig(app.world().get::<AnimationGraphPlayer>(player_entity).unwrap(), app.world().resource::<Assets<AnimationGraph>>()).unwrap();
            assert_eq!(rig_state.context.curves[&crate::extensions::SourceCurveName::from("Unrelated")], 0.37);
            let input = context.node_caches.get_output_data(driver.unwrap(), bevy_animation_graph::core::context::node_states::StateKey::Default, "Pose".into()).unwrap().into_pose().unwrap();
            assert_eq!(format!("{input:?}"), format!("{pose:?}"));
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
        for weights in [AlsFootIkWeightOverride { left: Some(0.5), right: None }, AlsFootIkWeightOverride::default()] {
            crate::manual_playground_integration::run_manual_rig_curve_fixture(app.world_mut(), actor, weights, false);
            app.world_mut().run_system_once_with(capture, (player_entity, to_world)).unwrap();
            app.world_mut().run_system_once(crate::extensions::post_physics_animation::evaluate_players::<()> ).unwrap();
            app.world_mut().run_system_once(apply_animation_to_targets).unwrap();
            app.world_mut().run_system_once(observe_rig_results).unwrap();
            let apply = app.world().get::<AlsFootIkApplyTelemetry>(actor).unwrap();
            assert_eq!(apply.left.applied_weight, weights.left.unwrap_or(1.0));
            assert_eq!(apply.right.applied_weight, 1.0);
        }
        let restored = app.world().resource::<Assets<AnimationGraph>>().get(&graph).unwrap();
        assert_eq!(restored.nodes.len(), original_node_count);
        assert_eq!(restored.edges, original_edges);
        assert_eq!(restored.edges_inverted, original_inputs);
        for active in [true, false] {
            crate::manual_playground_integration::run_manual_rig_curve_fixture(app.world_mut(), actor, default(), active);
            app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap().set_global_input_data("input_5", if active { 30.0_f32 } else { 0.0 });
            app.world_mut().run_system_once_with(capture, (player_entity, to_world)).unwrap();
            app.world_mut().run_system_once(crate::extensions::post_physics_animation::evaluate_players::<()> ).unwrap();
            app.world_mut().run_system_once(apply_animation_to_targets).unwrap();
            app.world_mut().run_system_once(observe_rig_results).unwrap();
            let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            let (_, _, rig_state) = current_control_rig(player, app.world().resource::<Assets<AnimationGraph>>()).unwrap();
            for name in ["PoseAiming", "HandLeftIk", "HandRightIk"] {
                assert_eq!(rig_state.context.curves.get(&crate::extensions::SourceCurveName::from(name)).copied().unwrap_or(0.0), if active { 1.0 } else { 0.0 }, "{name}");
            }
            for name in ["hand_l", "hand_r"] {
                let call = rig_state.calls.iter().find(|call| rig_call_is(&definition, call, "FRigUnit_TwoBoneIKSimplePerItem::Execute")
                    && call.arguments.get(2).and_then(rig_bone) == Some(name)).unwrap();
                assert_eq!(rig_scalar(&call.arguments[13]), Some(if active { 1.0 } else { 0.0 }));
            }
            let spine = definition.bones.iter().position(|bone| bone.name == "spine_01").unwrap();
            let rotation = player.get_default_output_pose().unwrap().get_bone(EntityPath::from(paths[spine].clone()).id()).unwrap().rotation.unwrap();
            if active { assert!(rotation.angle_between(pose.get_bone(EntityPath::from(paths[spine].clone()).id()).unwrap().rotation.unwrap()) > 0.05); }
        }
        let restored = app.world().resource::<Assets<AnimationGraph>>().get(&graph).unwrap();
        assert_eq!(restored.nodes.len(), original_node_count);
        assert_eq!(restored.edges, original_edges);
        assert_eq!(restored.edges_inverted, original_inputs);
        app.world_mut().get_mut::<Transform>(bones.left).unwrap().translation.x += 0.05;
        app.world_mut().run_system_once(observe_rig_results).unwrap();
        assert!(app.world().get::<AlsFootIkApplyTelemetry>(actor).unwrap().left.effector_error > 0.049);
        app.world_mut().resource_mut::<Assets<AnimationGraph>>().get_mut(&graph).unwrap().add_output_data_edge(source.id, "Pose", "pose");
        app.world_mut().run_system_once(crate::extensions::post_physics_animation::evaluate_players::<()> ).unwrap();
        app.world_mut().run_system_once(observe_rig_results).unwrap();
        assert_eq!(app.world().get::<AlsControlRigExecution>(actor).unwrap().executed_steps, 0, "an unevaluated rig must not expose its previous state");
        assert!(!app.world().get::<AlsFootIkSpaceState>(actor).unwrap().left.valid);
        app.world_mut().get_mut::<AnimationGraphPlayer>(player_entity).unwrap().start(wrapper);
        app.world_mut().run_system_once(observe_rig_results).unwrap();
        assert_eq!(app.world().get::<AlsControlRigExecution>(actor).unwrap().executed_steps, 0);
        assert!(!app.world().get::<AlsFootIkSpaceState>(actor).unwrap().left.valid);
        app.world_mut().get_mut::<SourceAnimationInputs>(player_entity).unwrap().set_rig_query_context(None);
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
        let mut foot_state = crate::als::foot_ik::AlsFootLockFootState {
            lock_amount: 1.0,
            lock_location: previous_character_transform
                .to_matrix()
                .transform_point3(local_location),
            lock_rotation: previous_character_transform.rotation * local_rotation,
            lock_rotation_before_foot_limit: previous_character_transform.rotation * local_rotation,
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
        app.init_resource::<Time>();
        app.add_message::<AlsFootLockTeleported>().add_systems(
            Update,
            (record_foot_lock_teleports, invalidate_missing_foot_bones),
        );
        let entity = app
            .world_mut()
            .spawn((
                crate::als::character::AlsCharacter,
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
                    previous_actor_location: None,
                    teleported_at: Some(0.0),
                },
            ))
            .id();
        let incomplete = app.world_mut().spawn((crate::als::character::AlsCharacter, FootLockLifecycleState {
            previous_character_transform: Some(previous_character_transform),
            previous_actor_location: None,
            teleported_at: Some(0.0),
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
            && lifecycle.teleported_at.is_none()
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
                crate::als::character::AlsCharacter,
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
            .teleported_at.unwrap()
            - app.world().resource::<Time>().elapsed_secs_f64())
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
    fn foot_targets_use_selected_avatar_space_with_old_avatar_retained() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .init_resource::<CrAlsRuntimeProfileTemplate>()
            .add_message::<AlsFootLockTeleported>()
            .add_systems(Update, (detect_foot_lock_teleports, record_foot_lock_teleports, refresh_foot_lock_inputs).chain());
        let character = app.world_mut().spawn((
            crate::als::character::AlsCharacter, Transform::IDENTITY,
            LocomotionState::Grounded, Stance::Standing, AlsFootLockSettings::default(),
            AlsFootLockInput { left_ik_amount: 1.0, right_ik_amount: 1.0, ..default() },
            AlsFootLockState::default(), AlsDynamicTransitionState::default(),
            FootLockLifecycleState::default(), AlsFootLockTeleportTelemetry::default(),
            AlsControlRigInput::default(),
        )).id();
        let old_avatar = app.world_mut().spawn((
            AlsCharacterAvatar { character }, Transform::from_xyz(20.0, 0.0, 0.0), ChildOf(character),
        )).id();
        app.world_mut().spawn((Transform::IDENTITY, ChildOf(old_avatar)));
        let selected_transform = Transform::from_xyz(2.0, 3.0, -4.0)
            .with_rotation(Quat::from_rotation_y(0.6)).with_scale(Vec3::splat(1.5));
        let avatar = app.world_mut().spawn((
            AlsCharacterAvatar { character }, selected_transform, ChildOf(character),
        )).id();
        let pelvis = app.world_mut().spawn((Transform::from_xyz(0.0, 0.9, 0.0), ChildOf(avatar))).id();
        let thigh = app.world_mut().spawn((Transform::IDENTITY, ChildOf(pelvis))).id();
        let calf = app.world_mut().spawn((Transform::from_xyz(0.0, -0.4, 0.0), ChildOf(thigh))).id();
        let rotation = Quat::from_rotation_x(0.2);
        let foot = app.world_mut().spawn((Transform::from_xyz(0.2, -0.4, 0.3).with_rotation(rotation), ChildOf(calf))).id();
        app.world_mut().entity_mut(character).insert((
            AlsCharacterAssembly { avatar, camera: None },
            AlsFootBones { left_thigh: thigh, left_calf: calf, left: foot, left_target: Some(foot), left_virtual_target: None,
                right_thigh: thigh, right_calf: calf, right: foot, right_target: Some(foot), right_virtual_target: None },
        ));
        for old_transform in [Transform::from_xyz(20.0, 0.0, 0.0),
            Transform::from_xyz(-8.0, 5.0, 9.0).with_rotation(Quat::from_rotation_y(-1.2)).with_scale(Vec3::splat(0.5))] {
            *app.world_mut().get_mut::<Transform>(old_avatar).unwrap() = old_transform;
            app.update();
            let input = app.world().get::<AlsControlRigInput>(character).unwrap();
            assert!(input.foot_transforms_valid);
            assert!(input.foot_left_location_armature.distance(Vec3::new(0.2, 0.1, 0.3)) < 1e-5,
                "Selected avatar local target differs: {:?}", input.foot_left_location_armature);
            assert!(input.foot_left_rotation_armature.dot(rotation).abs() > 0.99999);
            assert_eq!(app.world().get::<AlsFootLockTeleportTelemetry>(character).unwrap().detected_count, 0);
        }
    }

    #[test]
    fn new_foot_lock_uses_previous_final_pose_in_the_current_component_space() {
        let previous = Transform::from_xyz(2.0, 0.9, -1.0).with_rotation(Quat::from_rotation_y(0.3));
        let local = Vec3::new(0.2, -0.8, 0.3);
        let local_rotation = Quat::from_rotation_x(0.2);
        for translation in [Vec3::new(2.3, 0.9, -1.0), Vec3::new(-3.0, 30.9, 4.0)] {
            let current = Transform::from_translation(translation)
                .with_rotation(Quat::from_rotation_y(-0.6)).with_scale(Vec3::splat(1.2));
            let mut state = crate::als::foot_ik::AlsFootLockFootState {
                final_location: previous.to_matrix().transform_point3(local),
                final_rotation: previous.rotation * local_rotation,
                ..default()
            };
            refresh_previous_foot_lock_transform(&mut state, &previous, &current, false);
            let target = current.to_matrix().transform_point3(local + Vec3::X * 0.1);
            refresh_foot_lock(&mut state, target, current.rotation, 1.0, 1.0, true, false, true, false, 1.0 / 60.0, None);
            assert!(state.lock_location.distance(current.to_matrix().transform_point3(local)) < 1e-5);
            assert!(state.lock_rotation.dot(current.rotation * local_rotation).abs() > 0.99999);
        }
    }

    #[test]
    fn foot_teleport_window_uses_game_time_across_skipped_updates_and_pause() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        for _ in 0..60 { app.update(); }
        app.world_mut().write_message(AlsFootLockTeleported { entity: character });
        app.world_mut().run_system_once(record_foot_lock_teleports).unwrap();
        let marked_at = app.world().get::<FootLockLifecycleState>(character).unwrap().teleported_at.unwrap();
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(300));
        let before = *app.world().get::<AlsFootLockTeleportTelemetry>(character).unwrap();
        app.world_mut().run_system_once(refresh_foot_lock_inputs).unwrap();
        let expired = *app.world().get::<AlsFootLockTeleportTelemetry>(character).unwrap();
        assert_eq!((expired.left_rebase_count, expired.right_rebase_count), (before.left_rebase_count, before.right_rebase_count),
            "Source World game time expires the teleport window even when Foot refresh skipped updates");
        assert!(app.world().resource::<Time>().elapsed_secs_f64() - marked_at > 0.2);
        assert_eq!(expired.remaining_seconds, 0.0);
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        let game_before = app.world().resource::<Time<Virtual>>().elapsed();
        for _ in 0..3 { app.update(); }
        assert_eq!(app.world().resource::<Time<Virtual>>().elapsed(), game_before,
            "Source pause must not advance the game clock used by the teleport window");
        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
    }

    #[test]
    fn foot_teleport_window_includes_the_source_game_time_boundary() {
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        for _ in 0..60 { app.update(); }
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
        app.world_mut().get_mut::<avian3d::prelude::Position>(character).unwrap().x += 1.0;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation.x += 1.0;
        app.update();
        let marked_at = app.world().resource::<Time>().elapsed_secs_f64();
        app.update();
        let before = *app.world().get::<AlsFootLockTeleportTelemetry>(character).unwrap();
        let before_feet = *app.world().get::<AlsFootLockState>(character).unwrap();
        app.world_mut().get_mut::<avian3d::prelude::Position>(character).unwrap().x += 0.1;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation.x += 0.1;
        app.update();
        let elapsed = app.world().resource::<Time>().elapsed_secs_f64() - marked_at;
        assert!((elapsed - 0.2).abs() < 1e-8 && elapsed <= f64::from(FOOT_LOCK_TELEPORT_WINDOW_SECONDS),
            "The actual fixture must reach the source-inclusive .2 game-time boundary: elapsed={elapsed}");
        let feet = *app.world().get::<AlsFootLockState>(character).unwrap();
        assert!(feet.valid && feet.left.lock_amount > 0.9 && feet.right.lock_amount > 0.9);
        let boundary = *app.world().get::<AlsFootLockTeleportTelemetry>(character).unwrap();
        assert_eq!((boundary.left_rebase_count, boundary.right_rebase_count),
            (before.left_rebase_count + 1, before.right_rebase_count + 1),
            "Source TimeSince equal to .2 must rebase both actual locked feet");
        assert!((feet.left.final_location.x - before_feet.left.final_location.x - 0.1).abs() < 1e-4,
            "Source inclusive game-time rebase must reach the actual foot output: before={before_feet:?} current={feet:?}");
        app.world_mut().get_mut::<avian3d::prelude::Position>(character).unwrap().x += 0.1;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation.x += 0.1;
        app.update();
        let after = *app.world().get::<AlsFootLockTeleportTelemetry>(character).unwrap();
        assert_eq!((after.left_rebase_count, after.right_rebase_count),
            (boundary.left_rebase_count, boundary.right_rebase_count),
            "Source game time beyond .2 must stop rebasing both locked feet");
    }

    #[test]
    fn pending_teleport_detection_does_not_rebase_the_restored_feet() {
        use bevy::ecs::system::RunSystemOnce;
        #[derive(Resource, Default)]
        struct SourceReplay {
            active: bool,
            inputs: Option<(FootLockLifecycleState, AlsFootLockState, AlsDynamicTransitionState, AlsFootLockInput, AlsFootLockTeleportTelemetry, AlsControlRigInput)>,
        }
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.1)));
        app.world_mut().entity_mut(avatar).insert(crate::animation_visibility::AlsAnimationTickOption::OnlyTickMontagesWhenNotRendered);
        let mut pending = vec![avatar];
        let mut meshes = Vec::new();
        while let Some(entity) = pending.pop() {
            if app.world().get::<Mesh3d>(entity).is_some() { meshes.push(entity); }
            if let Some(children) = app.world().get::<Children>(entity) { pending.extend(children.iter()); }
        }
        assert!(!meshes.is_empty());
        for mesh in &meshes { app.world_mut().entity_mut(*mesh).insert((ViewVisibility::HIDDEN, InheritedVisibility::VISIBLE)); }
        for _ in 0..15 { app.update(); }
        assert!(!app.world().get::<crate::extensions::post_physics_animation::AnimationPoseUpdateGate>(player).unwrap().update_graph);
        app.init_resource::<SourceReplay>().add_systems(PostUpdate,
            (move |states: Query<(&FootLockLifecycleState, &AlsFootLockState, &AlsDynamicTransitionState, &AlsFootLockInput, &AlsFootLockTeleportTelemetry, &AlsControlRigInput)>,
                mut replay: ResMut<SourceReplay>| {
                if replay.active {
                    let (life, lock, dynamic, input, telemetry, rig) = states.get(character).unwrap();
                    replay.inputs = Some((*life, *lock, *dynamic, *input, *telemetry, *rig));
                }
            }).after(record_foot_lock_teleports).before(refresh_foot_lock_inputs));
        app.add_systems(PostUpdate, (move |world: &mut World| {
            if !world.resource::<SourceReplay>().active { return; }
            let actual = *world.get::<AlsFootLockState>(character).unwrap();
            let (mut life, lock, dynamic, input, telemetry, rig) = world.resource_mut::<SourceReplay>().inputs.take().unwrap();
            life.teleported_at = None;
            world.entity_mut(character).insert((life, lock, dynamic, input, telemetry, rig));
            world.run_system_once(refresh_foot_lock_inputs).unwrap();
            let source = *world.get::<AlsFootLockState>(character).unwrap();
            assert!(actual.left.final_location.distance(source.left.final_location) < 1e-4
                && actual.right.final_location.distance(source.right.final_location) < 1e-4,
                "Source NativeUpdate suppresses pending MarkTeleported, so restored feet must match the same-frame source no-window path: actual={actual:?}, source={source:?}");
        }).after(refresh_foot_lock_inputs).before(crate::control_rig_apply::AlsControlRigInputSet));
        app.world_mut().get_mut::<avian3d::prelude::Position>(character).unwrap().x += 1.0;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation.x += 1.0;
        for mesh in &meshes { app.world_mut().entity_mut(*mesh).insert(ViewVisibility::VISIBLE); }
        app.update();
        app.update();
        assert!(app.world().get::<AlsFootLockState>(character).unwrap().valid);
        app.world_mut().get_mut::<avian3d::prelude::Position>(character).unwrap().x += 0.1;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation.x += 0.1;
        app.world_mut().resource_mut::<SourceReplay>().active = true;
        app.update();
    }

    #[test]
    fn pending_animation_resume_rejects_stale_foot_transforms_before_the_rig() {
        #[derive(Resource, Default)]
        struct ResumeInputs(Vec<(bool, bool)>);
        let mut outputs = Vec::new();
        for source_validity in [false, true] {
            let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().resource_mut::<AlsFootIkTelemetryEnabled>().0 = true;
            let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
            let mut pending = vec![avatar];
            let mut meshes = Vec::new();
            while let Some(entity) = pending.pop() {
                if app.world().get::<Mesh3d>(entity).is_some() { meshes.push(entity); }
                if let Some(children) = app.world().get::<Children>(entity) { pending.extend(children.iter()); }
            }
            assert!(!meshes.is_empty());
            app.init_resource::<ResumeInputs>().add_systems(PostUpdate,
                (move |gates: Query<&crate::extensions::post_physics_animation::AnimationPoseUpdateGate>,
                    rigs: Query<&AlsControlRigInput>, mut players: Query<&mut AnimationGraphPlayer>,
                    mut observed: ResMut<ResumeInputs>| {
                    let gate = gates.get(player).unwrap();
                    if gate.update_graph {
                        observed.0.push((gate.pending_update, rigs.get(character).unwrap().foot_transforms_valid));
                        if source_validity && gate.pending_update {
                            crate::extensions::source_instance::set_source_frame_input(&mut players.get_mut(player).unwrap(),
                                "control_rig_input.b_foot_transforms_valid", false);
                        }
                    }
                }).after(crate::control_rig_apply::AlsControlRigInputSet)
                    .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.1)));
            app.world_mut().entity_mut(avatar).insert(crate::animation_visibility::AlsAnimationTickOption::OnlyTickMontagesWhenNotRendered);
            for mesh in &meshes { app.world_mut().entity_mut(*mesh).insert((ViewVisibility::HIDDEN, InheritedVisibility::VISIBLE)); }
            for _ in 0..15 { app.update(); }
            let gate = app.world().get::<crate::extensions::post_physics_animation::AnimationPoseUpdateGate>(player).unwrap();
            assert!(!gate.update_graph && gate.pending_update, "The actual source fixture must stop graph evaluation before testing resume: gate={gate:?}");
            app.world_mut().resource_mut::<ResumeInputs>().0.clear();
            for mesh in &meshes { app.world_mut().entity_mut(*mesh).insert(ViewVisibility::VISIBLE); }
            app.update();
            let first = app.world().resource::<ResumeInputs>().0.first().copied().unwrap();
            assert!(first.0);
            let instance = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            let graphs = app.world().resource::<Assets<AnimationGraph>>();
            let (_, node, state) = current_control_rig(instance, graphs).unwrap();
            let definition = node.try_inner_downcast_ref::<ControlRigBoundaryNode>().unwrap().definition.as_ref().unwrap();
            let calls = state.calls.iter().filter(|call| rig_call_is(definition, call, "FAlsRigUnit_ApplyFootOffsetLocation::Execute")
                || rig_call_is(definition, call, "FAlsRigUnit_ApplyFootOffsetRotation::Execute")
                || rig_call_is(definition, call, "FRigUnit_TwoBoneIKSimplePerItem::Execute")).count();
            outputs.push((first.1, calls));
            app.update();
            assert_eq!(app.world().resource::<ResumeInputs>().0.last().copied(), Some((false, true)),
                "Source registered next update validates current feet after the pending frame");
        }
        assert_eq!(outputs[0].1, outputs[1].1,
            "Source pending Feet validity must gate the actual first-resume foot solve calls identically to the source-required false input: outputs={outputs:?}");
        assert!(!outputs[0].0, "Source pending NativeUpdate invalidates Feet before the first resumed ControlRig evaluation");
    }

    #[test]
    fn teleport_detection_uses_the_source_actor_location_through_crouching() {
        use avian3d::prelude::Position;
        use bevy::ecs::system::RunSystemOnce;
        #[derive(Resource, Default)]
        struct FootReplay(Option<(FootLockLifecycleState, AlsFootLockState, AlsDynamicTransitionState, AlsFootLockInput, AlsFootLockTeleportTelemetry)>);
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        for _ in 0..60 { app.update(); }
        let before_actor = app.world().get::<Position>(character).unwrap().0;
        let before_avatar = app.world().get::<GlobalTransform>(avatar).unwrap().translation();
        assert!(app.world().get::<AlsFootLockState>(character).unwrap().valid);
        let before = *app.world().get::<AlsFootLockTeleportTelemetry>(character).unwrap();
        app.init_resource::<FootReplay>().add_systems(PostUpdate,
            (move |states: Query<(&FootLockLifecycleState, &AlsFootLockState, &AlsDynamicTransitionState, &AlsFootLockInput, &AlsFootLockTeleportTelemetry)>,
                mut replay: ResMut<FootReplay>| {
                let (life, lock, dynamic, input, telemetry) = states.get(character).unwrap();
                replay.0 = Some((*life, *lock, *dynamic, *input, *telemetry));
            }).after(record_foot_lock_teleports).before(refresh_foot_lock_inputs));
        app.add_systems(PostUpdate, (move |world: &mut World| {
            let actual = *world.get::<AlsFootLockState>(character).unwrap();
            let (mut life, lock, dynamic, input, telemetry) = world.resource_mut::<FootReplay>().0.take().unwrap();
            assert_eq!(telemetry.detected_count, before.detected_count + 1,
                "Source actor displacement must mark teleportation even when crouching offsets the avatar");
            life.teleported_at = Some(world.resource::<Time>().elapsed_secs_f64());
            world.entity_mut(character).insert((life, lock, dynamic, input, telemetry));
            world.run_system_once(refresh_foot_lock_inputs).unwrap();
            let source = *world.get::<AlsFootLockState>(character).unwrap();
            assert!(actual.left.final_location.distance(source.left.final_location) < 1e-4
                && actual.right.final_location.distance(source.right.final_location) < 1e-4,
                "Source NativeUpdate actor teleport detection must match the same-frame actual FootLock output of its required MarkTeleported event: actual={actual:?} source={source:?}");
            let restored = world.get::<AlsFootLockTeleportTelemetry>(character).unwrap();
            assert!(restored.left_rebase_count > before.left_rebase_count && restored.right_rebase_count > before.right_rebase_count);
        }).after(refresh_foot_lock_inputs).before(crate::ab_als_provider::AbAlsProviderSet));
        app.world_mut().get_mut::<Position>(character).unwrap().x += 0.49;
        app.world_mut().get_mut::<Transform>(character).unwrap().translation.x += 0.49;
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().stance = Some(Stance::Crouching);
        app.update();
        assert_eq!(app.world().get::<Stance>(character), Some(&Stance::Crouching));
        let actor_delta = app.world().get::<Position>(character).unwrap().0.distance(before_actor);
        let avatar_delta = app.world().get::<GlobalTransform>(avatar).unwrap().translation().distance(before_avatar);
        assert!(actor_delta > 0.5 && avatar_delta <= 0.5,
            "Source actor capsule adjustment must cross .5m while the avatar retains its world floor offset: actor={actor_delta} avatar={avatar_delta}");
    }

    #[test]
    fn source_teleport_threshold_marks_only_distance_above_half_meter() {
        let mut app = App::new();
        app.init_resource::<Time>();
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
                crate::als::character::AlsCharacter,
                Transform::from_xyz(FOOT_LOCK_TELEPORT_DISTANCE_THRESHOLD_M, 0.0, 0.0),
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
                    previous_actor_location: Some(Vec3::ZERO),
                    teleported_at: None,
                },
                AlsFootLockTeleportTelemetry::default(),
            ))
            .id();
        let avatar = app
            .world_mut()
            .spawn((
                AlsCharacterAvatar { character },
                ChildOf(character),
                Transform::IDENTITY,
            ))
            .id();
        app.world_mut().entity_mut(character).insert(AlsCharacterAssembly { avatar, camera: None });
        app.update();
        let at_threshold = *app
            .world()
            .get::<AlsFootLockTeleportTelemetry>(character)
            .unwrap();
        *app.world_mut().get_mut::<Transform>(character).unwrap() =
            Transform::from_xyz(2.0 * FOOT_LOCK_TELEPORT_DISTANCE_THRESHOLD_M + 0.001, 0.0, 0.0);
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
        assert!((lifecycle.teleported_at.unwrap() - app.world().resource::<Time>().elapsed_secs_f64()).abs() < 1e-8);
    }

    #[test]
    fn foot_lock_prepares_completed_pose_before_the_next_graph_evaluation() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, FootIkApplyPlugin))
            .init_resource::<avian3d::collider_tree::ColliderTrees>();
        app.configure_sets(PostUpdate, (
            AnimationGraphSet::PrePhysics.before(avian3d::prelude::PhysicsSystems::First),
            avian3d::prelude::PhysicsSystems::Last.after(avian3d::prelude::PhysicsSystems::First),
            crate::als::motion::AlsRotationResponseSet.after(avian3d::prelude::PhysicsSystems::Last),
            crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs
                .after(avian3d::prelude::PhysicsSystems::Last)
                .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
        ));
        let character = app.world_mut().spawn((
            crate::als::character::AlsCharacter, Transform::from_xyz(2.0, 0.0, 0.0),
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
        let right_thigh = app.world_mut().spawn((Transform::IDENTITY, ChildOf(pelvis))).id();
        let right_calf = app.world_mut().spawn((Transform::from_xyz(0.0, -0.4, 0.0), ChildOf(right_thigh))).id();
        let right_foot = app.world_mut().spawn((Transform::from_xyz(0.0, -0.4, 0.0), ChildOf(right_calf))).id();
        for (name, bone) in ["thigh_l", "calf_l", "foot_l", "thigh_r", "calf_r", "foot_r"].into_iter()
            .zip([thigh, calf, foot, right_thigh, right_calf, right_foot]) {
            app.world_mut().entity_mut(bone).insert((Name::new(name), EntityPath::from(vec![name.to_owned()]).id().animation_target_id()));
        }
        app.world_mut().entity_mut(character).insert(AlsCharacterAssembly { avatar, camera: None });
        app.world_mut().entity_mut(character).insert(AlsFootBones {
            left_thigh: thigh, left_calf: calf, left: foot, left_target: None, left_virtual_target: None,
            right_thigh, right_calf, right: right_foot, right_target: None, right_virtual_target: None,
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
        }).in_set(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));
        app.update();
        let feet = app.world().get::<AlsFootLockState>(character).unwrap();
        assert_eq!(feet.left.lock_amount, 1.0, "evaluation must not refresh foot lock again");
        assert_eq!(app.world().get::<Transform>(foot).unwrap().translation.y, -0.1);
        app.world_mut().get_mut::<Transform>(character).unwrap().translation.x = 3.0;
        app.update();
        assert_eq!(app.world().get::<Transform>(foot).unwrap().translation.y, -0.1);
    }

}
