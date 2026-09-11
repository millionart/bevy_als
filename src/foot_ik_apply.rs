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

use crate::animation_context_projection::AlsRootAnimationContextSet;
use crate::character_assembly::AlsCharacterAvatar;
use crate::control_rig_apply::{AlsControlRigInput, AlsControlRigPrePropagationSet, CrAlsRuntimeProfileTemplate};

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
            ));
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
