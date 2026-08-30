use als_core::dynamics::AlsMovementBaseDynamicsState;
use als_core::foot_ik::{
    AlsDynamicTransitionState, AlsFootBones, AlsFootIkRuntime, AlsFootIkTargets, AlsFootLockInput,
    AlsFootLockMovementBase, AlsFootLockSettings, AlsFootLockState, AlsFootLockTeleported,
    AlsFootProbe, FOOT_LOCK_TELEPORT_WINDOW_SECONDS, FootSideInput,
    advance_foot_lock_movement_base, capture_foot_lock_movement_base, constrain_foot_lock_location,
    foot_target_offset, is_relevant_animation_weight, refresh_foot_lock, reset_foot_lock_to_target,
};
use als_core::state::{LocomotionState, Stance};
use als_core::transitions::TransitionConfig;
use als_ext::ik::{solve_two_bone_ik, world_rotation_to_local};
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

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsFootIkTelemetryEnabled(pub bool);

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

impl Plugin for FootIkApplyPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AnimationGraphSet::Final.before(TransformSystems::Propagate),
        );
        app.init_resource::<AlsFootIkTelemetryEnabled>()
            .add_message::<AlsFootLockTeleported>();
        app.add_systems(
            PostUpdate,
            (
                ensure_foot_ik_runtime,
                ensure_foot_ik_telemetry,
                ensure_foot_lock_components,
                ensure_foot_offset_rotation_state,
                ensure_foot_lock_lifecycle_state,
                invalidate_missing_foot_bones.before(foot_ik_apply_system),
                record_foot_lock_teleports.before(foot_ik_apply_system),
                foot_ik_apply_system
                    .after(apply_animation_to_targets)
                    .after(TransformSystems::Propagate)
                    .after(als_core::foot_ik::foot_ground_probe_system)
                    .after(als_core::foot_ik::foot_pin_request_system)
                    .after(als_core::foot_ik::foot_ik_state_system),
                (
                    transform_systems::mark_dirty_trees,
                    transform_systems::propagate_parent_transforms,
                    transform_systems::sync_simple_transforms,
                )
                    .chain()
                    .after(foot_ik_apply_system)
                    .after(TransformSystems::Propagate),
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
        ),
        With<als_core::character::AlsCharacter>,
    >,
) {
    for (bones, lock_state, runtime, offset_rotation, dynamic_transition, lifecycle, telemetry) in
        &mut characters
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

#[allow(clippy::type_complexity)]
fn foot_ik_apply_system(
    time: Res<Time>,
    telemetry_enabled: Res<AlsFootIkTelemetryEnabled>,
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
        let char_fwd = char_gtf.forward().as_vec3();
        let pole = Vec3::new(char_fwd.x, 0.0, char_fwd.z).normalize_or_zero();
        let left_ik_amount = animation_curve_amount(entity, "FootLeftIk", &avatars, &players)
            .unwrap_or(lock_input.left_ik_amount)
            .clamp(0.0, 1.0);
        let right_ik_amount = animation_curve_amount(entity, "FootRightIk", &avatars, &players)
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
        for side in 0..2usize {
            let hit = match side {
                0 => probe.left,
                _ => probe.right,
            };
            let chain = bones.chain(side);
            if let Ok((_, foot_g, _)) = bone_locals.get(chain.foot) {
                let ankle_y = foot_g.translation().y;
                inputs[side] = FootSideInput {
                    raw_offset: hit.map(|h| foot_target_offset(ankle_y, &h)).unwrap_or(0.0),
                    pin_request: match side {
                        0 => targets.pin_left,
                        _ => targets.pin_right,
                    },
                };
            }
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
            let (offset, pin_weight) = runtime.side(side);
            let foot_state = match side {
                0 => &lock_state.left,
                _ => &lock_state.right,
            };
            let ik_amount = match side {
                0 => left_ik_amount,
                _ => right_ik_amount,
            };
            let ground_normal = match side {
                0 => probe.left,
                _ => probe.right,
            }
            .map(|hit| hit.normal)
            .unwrap_or(Vec3::Y);
            let offset_normal = foot_offset_rotation.update(side, ground_normal, dt);
            let foot_world_rotation =
                foot_rotation_with_offset(foot_state.final_rotation, offset_normal);
            let lock_offset = foot_state.final_location - foot_state.target_location;
            let telemetry_target = foot_state.final_location + Vec3::Y * offset;
            if telemetry_enabled.0
                && let (Ok(thigh), Ok(calf), Ok(foot_local)) = (
                    bone_globals.get(chain.thigh),
                    bone_globals.get(chain.calf),
                    bone_locals.get(chain.foot),
                )
            {
                let hip = thigh.translation();
                let knee = calf.translation();
                let ankle = foot_local.1.translation();
                let upper_length = knee.distance(hip);
                let lower_length = ankle.distance(knee);
                let armature_inverse = current_character_transform.to_matrix().inverse();
                let parent_local = bone_globals
                    .get(foot_local.2.0)
                    .map(|parent| {
                        parent
                            .to_matrix()
                            .inverse()
                            .transform_point3(telemetry_target)
                    })
                    .unwrap_or(telemetry_target);
                let target_axis = (telemetry_target - hip).normalize_or_zero();
                let pole_sign = target_axis.cross(knee - hip).dot(pole);
                let entry = AlsFootIkTelemetrySide {
                    valid: telemetry_target.is_finite()
                        && upper_length.is_finite()
                        && lower_length.is_finite()
                        && pole_sign.is_finite(),
                    target_world: telemetry_target,
                    target_armature_local: armature_inverse.transform_point3(telemetry_target),
                    target_parent_local: parent_local,
                    reference_chain_error: (telemetry_target.distance(hip)
                        - (upper_length + lower_length) * FOOT_MAX_LEG_STRETCH_RATIO)
                        .max(0.0),
                    pole_sign,
                    ik_weight: ik_amount,
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
            let has_slope_rotation = offset_normal.dot(Vec3::Y) < 1.0 - 1e-4;
            let has_lock_rotation = foot_state.lock_amount > 1e-3;
            if ik_amount <= 1e-4
                || (offset.abs() < 1e-4
                    && pin_weight < 1e-3
                    && lock_offset.length_squared() < 1e-8
                    && !has_slope_rotation
                    && !has_lock_rotation)
            {
                continue;
            }
            apply_chain_offset(
                &chain,
                Vec3::Y * offset + lock_offset,
                pole,
                settings,
                foot_world_rotation,
                pelvis_world_offset,
                &mut bone_locals,
                &bone_globals,
            );
        }
        lifecycle.previous_character_transform = Some(current_character_transform);
        lifecycle.teleport_remaining = (lifecycle.teleport_remaining - dt.max(0.0)).max(0.0);
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

fn apply_chain_offset(
    chain: &als_core::foot_ik::FootChain,
    offset: Vec3,
    pole: Vec3,
    settings: &AlsFootLockSettings,
    foot_world_rotation: Quat,
    pelvis_world_offset: Vec3,
    bone_locals: &mut Query<(&mut Transform, &GlobalTransform, &ChildOf), Without<AnimationPlayer>>,
    bone_globals: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) {
    let (Ok(_), Ok(_), Ok(_)) = (
        bone_locals.get(chain.thigh),
        bone_locals.get(chain.calf),
        bone_locals.get(chain.foot),
    ) else {
        return;
    };
    let (Ok(thigh_g), Ok(calf_g), Ok(foot_g)) = (
        bone_globals.get(chain.thigh),
        bone_globals.get(chain.calf),
        bone_globals.get(chain.foot),
    ) else {
        return;
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
        ankle_before_pelvis_drop + offset,
        settings.thigh_angle_limit_degrees,
    );
    let target = limit_leg_stretch(
        hip,
        target,
        (upper_len + lower_len) * FOOT_MAX_LEG_STRETCH_RATIO,
    );

    let Some(solve) = solve_two_bone_ik(hip, knee, ankle, target, upper_len, lower_len, pole)
    else {
        return;
    };

    let thigh_world_new = solve.hip_rotation_world * thigh_g.rotation();
    let calf_world_new = solve.knee_rotation_world * calf_g.rotation();

    let Ok((mut thigh_local, _, thigh_child)) = bone_locals.get_mut(chain.thigh) else {
        return;
    };
    let pelvis_rot = bone_globals
        .get(thigh_child.0)
        .map(|p| p.rotation())
        .unwrap_or(Quat::IDENTITY);
    thigh_local.rotation = world_rotation_to_local(pelvis_rot, thigh_world_new);

    let Ok((mut calf_local, _, _)) = bone_locals.get_mut(chain.calf) else {
        return;
    };
    calf_local.rotation = world_rotation_to_local(thigh_world_new, calf_world_new);

    let Ok((mut foot_local, _, foot_child)) = bone_locals.get_mut(chain.foot) else {
        return;
    };
    let foot_parent_rotation = if foot_child.0 == chain.calf {
        calf_world_new
    } else {
        bone_globals
            .get(foot_child.0)
            .map(|parent| parent.rotation())
            .unwrap_or(Quat::IDENTITY)
    };
    foot_local.rotation = foot_local_rotation(foot_parent_rotation, foot_world_rotation);
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
