use avian3d::prelude::SpatialQueryFilter;
use crate::extensions::RagdollSpatialQuery as SpatialQuery;
use bevy::prelude::*;
use bevy::math::{DQuat, DVec3};

use bevy::animation::AnimationTargetId;

use crate::als::character::AlsCharacter;
use crate::als::state::{LocomotionState, Stance};
use crate::als::transitions::{
    TransitionConfig, TransitionSelection, foot_lock_distance_squared_cm, select_dynamic_transition,
};
use crate::extensions::{control_rig_hierarchy::rig_normalize, source_math::source_rotate_vector};

pub const FOOT_BONE_NAMES: [&str; 10] = [
    "thigh_l",
    "calf_l",
    "foot_l",
    "ik_foot_l",
    "VB foot_l",
    "thigh_r",
    "calf_r",
    "foot_r",
    "ik_foot_r",
    "VB foot_r",
];
pub const PROBE_UP_OFFSET: f32 = 0.50;
pub const PROBE_DEPTH: f32 = 0.80;
pub const ANKLE_TO_SOLE: f32 = 0.135;
pub const MAX_FOOT_REACH: f32 = 0.45;
pub const FOOT_IK_WALKABLE_FLOOR_ANGLE_DEGREES: f32 = 45.0;
pub const FOOT_IK_MOVING_SMOOTH_SPEED_THRESHOLD: f32 = 1.5;
pub const FOOT_LOCK_THIGH_ANGLE_LIMIT_DEGREES: f32 = 90.0;
pub const FOOT_LOCK_FOOT_ANGLE_LIMIT_DEGREES: f32 = 40.0;
pub const FOOT_LOCK_MOVING_DECREASE_PER_SECOND: f32 = 5.0;
pub const FOOT_LOCK_NOT_GROUNDED_DECREASE_PER_SECOND: f32 = 0.6;
pub const FOOT_LOCK_RELEVANCE_THRESHOLD: f32 = 1e-5;
pub const FOOT_LOCK_TELEPORT_WINDOW_SECONDS: f32 = 0.2;
pub const FOOT_LOCK_TELEPORT_DISTANCE_THRESHOLD_M: f32 = 0.5;

const WEIGHT_BLEND_SPEED: f32 = 10.0;
const FOOT_OFFSET_SPRING_FREQUENCY_HZ: f32 = 12.0;
const FOOT_OFFSET_SPRING_DAMPING_RATIO: f32 = 2.0;
const PELVIS_OFFSET_SPRING_FREQUENCY_HZ: f32 = 2.0;
const PELVIS_OFFSET_SPRING_DAMPING_RATIO: f32 = 1.0;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsFootLockSettings {
    pub allow_foot_lock: bool,
    pub moving_smooth_speed_threshold: f32,
    pub thigh_angle_limit_degrees: f32,
    pub foot_angle_limit_degrees: f32,
    pub moving_decrease_per_second: f32,
    pub not_grounded_decrease_per_second: f32,
}

impl Default for AlsFootLockSettings {
    fn default() -> Self {
        Self {
            allow_foot_lock: true,
            moving_smooth_speed_threshold: FOOT_IK_MOVING_SMOOTH_SPEED_THRESHOLD,
            thigh_angle_limit_degrees: FOOT_LOCK_THIGH_ANGLE_LIMIT_DEGREES,
            foot_angle_limit_degrees: FOOT_LOCK_FOOT_ANGLE_LIMIT_DEGREES,
            moving_decrease_per_second: FOOT_LOCK_MOVING_DECREASE_PER_SECOND,
            not_grounded_decrease_per_second: FOOT_LOCK_NOT_GROUNDED_DECREASE_PER_SECOND,
        }
    }
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsFootLockTeleported {
    pub entity: Entity,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsDynamicTransitionState {
    pub selection: Option<TransitionSelection>,
    pub frames_remaining: u32,
    pub left_distance_squared: f32,
    pub right_distance_squared: f32,
}

impl Default for AlsDynamicTransitionState {
    fn default() -> Self {
        Self {
            selection: None,
            frames_remaining: 0,
            left_distance_squared: 0.0,
            right_distance_squared: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsGroundHit {
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsFootProbe {
    pub left: Option<AlsGroundHit>,
    pub right: Option<AlsGroundHit>,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsFootBones {
    pub left_thigh: Entity,
    pub left_calf: Entity,
    pub left: Entity,
    pub left_target: Option<Entity>,
    pub left_virtual_target: Option<Entity>,
    pub right_thigh: Entity,
    pub right_calf: Entity,
    pub right: Entity,
    pub right_target: Option<Entity>,
    pub right_virtual_target: Option<Entity>,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsFootTargetSelection {
    pub use_ik_bones: bool,
}

impl Default for AlsFootTargetSelection {
    fn default() -> Self {
        Self { use_ik_bones: true }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FootChain {
    pub thigh: Entity,
    pub calf: Entity,
    pub foot: Entity,
}

impl AlsFootBones {
    pub fn chain(&self, side: usize) -> FootChain {
        match side {
            0 => FootChain {
                thigh: self.left_thigh,
                calf: self.left_calf,
                foot: self.left,
            },
            _ => FootChain {
                thigh: self.right_thigh,
                calf: self.right_calf,
                foot: self.right,
            },
        }
    }

    pub fn target(&self, side: usize, use_ik_bones: bool) -> Option<Entity> {
        match side {
            0 if use_ik_bones => self.left_target,
            0 => self.left_virtual_target,
            _ if use_ik_bones => self.right_target,
            _ => self.right_virtual_target,
        }
    }
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsFootPinRequest {
    pub left: bool,
    pub right: bool,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsFootIkTargets {
    pub grounded: bool,
    pub pin_left: bool,
    pub pin_right: bool,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsFootLockInput {
    pub left_ik_amount: f32,
    pub right_ik_amount: f32,
    pub left_lock_amount: f32,
    pub right_lock_amount: f32,
    pub allow_transitions: bool,
    pub moving_smooth: bool,
    pub foot_planted_amount: f32,
    pub feet_crossing_amount: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsFootLockMovementBase {
    pub entity: Entity,
    pub translation: Vec3,
    pub rotation: Quat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsFootLockFootState {
    pub lock_amount: f32,
    pub target_location: DVec3,
    pub target_rotation: DQuat,
    pub lock_location: DVec3,
    pub lock_rotation: DQuat,
    pub lock_rotation_before_foot_limit_armature: Quat,
    pub movement_base: Option<AlsFootLockMovementBase>,
    pub lock_location_movement_base_cm: Vec3,
    pub lock_rotation_movement_base: Quat,
    pub final_location: DVec3,
    pub final_rotation_armature: Quat,
}

impl Default for AlsFootLockFootState {
    fn default() -> Self {
        Self {
            lock_amount: 0.0,
            target_location: DVec3::ZERO,
            target_rotation: DQuat::IDENTITY,
            lock_location: DVec3::ZERO,
            lock_rotation: DQuat::IDENTITY,
            lock_rotation_before_foot_limit_armature: Quat::IDENTITY,
            movement_base: None,
            lock_location_movement_base_cm: Vec3::ZERO,
            lock_rotation_movement_base: Quat::IDENTITY,
            final_location: DVec3::ZERO,
            final_rotation_armature: Quat::IDENTITY,
        }
    }
}

pub fn reset_foot_lock_to_target(
    state: &mut AlsFootLockFootState,
    target_location: impl Into<DVec3>,
    target_rotation: DQuat,
    armature_rotation: Quat,
) {
    let target_location = target_location.into();
    let local_rotation = (armature_rotation.as_dquat().conjugate() * target_rotation).as_quat();
    *state = AlsFootLockFootState {
        target_location,
        target_rotation,
        lock_location: target_location,
        lock_rotation: target_rotation,
        lock_rotation_before_foot_limit_armature: local_rotation,
        final_location: target_location,
        final_rotation_armature: local_rotation,
        ..Default::default()
    };
}

pub fn advance_foot_lock_movement_base(
    state: &mut AlsFootLockFootState,
    latest_base: Option<AlsFootLockMovementBase>,
) {
    let Some(saved_base) = state.movement_base else {
        return;
    };
    let base = latest_base
        .filter(|base| base.entity == saved_base.entity)
        .unwrap_or(saved_base);
    state.lock_location = (base.translation.as_dvec3() * 100.0
        + source_rotate_vector(base.rotation.as_dquat(), state.lock_location_movement_base_cm.as_dvec3())) * 0.01;
    state.lock_rotation = base.rotation.as_dquat() * state.lock_rotation_movement_base.as_dquat();
    state.movement_base = Some(base);
}

pub fn capture_foot_lock_movement_base(
    state: &mut AlsFootLockFootState,
    current_base: Option<AlsFootLockMovementBase>,
) {
    if !is_relevant_animation_weight(state.lock_amount) {
        state.movement_base = None;
        state.lock_location_movement_base_cm = Vec3::ZERO;
        state.lock_rotation_movement_base = Quat::IDENTITY;
        return;
    }
    let Some(base) = current_base else {
        state.movement_base = None;
        state.lock_location_movement_base_cm = Vec3::ZERO;
        state.lock_rotation_movement_base = Quat::IDENTITY;
        return;
    };
    let inverse = base.rotation.conjugate();
    state.lock_location_movement_base_cm = source_rotate_vector(
        inverse.as_dquat(), state.lock_location * 100.0 - base.translation.as_dvec3() * 100.0,
    ).as_vec3();
    state.lock_rotation_movement_base = (inverse.as_dquat() * state.lock_rotation).as_quat();
    state.movement_base = Some(base);
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsFootLockState {
    pub valid: bool,
    pub became_valid: bool,
    pub foot_planted_amount: f32,
    pub feet_crossing_amount: f32,
    pub left: AlsFootLockFootState,
    pub right: AlsFootLockFootState,
}

pub fn is_relevant_animation_weight(value: f32) -> bool {
    value > FOOT_LOCK_RELEVANCE_THRESHOLD
}

pub fn is_full_animation_weight(value: f32) -> bool {
    value >= 1.0 - FOOT_LOCK_RELEVANCE_THRESHOLD
}

#[allow(clippy::too_many_arguments)]
pub fn refresh_foot_lock(
    state: &mut AlsFootLockFootState,
    target_location: impl Into<DVec3>,
    target_rotation: DQuat,
    armature_rotation: Quat,
    ik_amount: f32,
    requested_lock_amount: f32,
    allow_foot_lock: bool,
    moving_smooth: bool,
    grounded: bool,
    became_valid: bool,
    delta_seconds: f32,
    current_base: Option<AlsFootLockMovementBase>,
) {
    let target_location = target_location.into();
    state.target_location = target_location;
    state.target_rotation = target_rotation;

    if (became_valid || state.movement_base.map(|base| base.entity) != current_base.map(|base| base.entity))
        && is_relevant_animation_weight(ik_amount * state.lock_amount)
    {
        if became_valid {
            state.lock_location = target_location;
            state.lock_rotation = target_rotation;
        }
        capture_foot_lock_movement_base(state, current_base);
    }

    let mut new_lock_amount = requested_lock_amount.clamp(0.0, 1.0);
    if moving_smooth || !grounded {
        let decrease_speed = if moving_smooth {
            FOOT_LOCK_MOVING_DECREASE_PER_SECOND
        } else {
            FOOT_LOCK_NOT_GROUNDED_DECREASE_PER_SECOND
        };
        new_lock_amount = if became_valid {
            0.0
        } else {
            new_lock_amount
                .min((state.lock_amount - delta_seconds.max(0.0) * decrease_speed).max(0.0))
        };
    }

    if !allow_foot_lock
        || !is_relevant_animation_weight(ik_amount.clamp(0.0, 1.0) * new_lock_amount)
    {
        state.lock_amount = 0.0;
        state.lock_location = DVec3::ZERO;
        state.lock_rotation = DQuat::IDENTITY;
        state.lock_rotation_before_foot_limit_armature = Quat::IDENTITY;
        state.movement_base = None;
        state.lock_location_movement_base_cm = Vec3::ZERO;
        state.lock_rotation_movement_base = Quat::IDENTITY;
        state.final_location = target_location;
        state.final_rotation_armature =
            (armature_rotation.as_dquat().conjugate() * target_rotation).as_quat();
        return;
    }

    let full_weight = is_full_animation_weight(new_lock_amount);
    let greater_than_previous = new_lock_amount > state.lock_amount;
    if full_weight {
        if greater_than_previous && (state.lock_amount <= 0.9 || current_base.is_some()) {
            let (location, rotation) = if became_valid {
                (target_location, target_rotation)
            } else {
                (state.final_location, armature_rotation.as_dquat() * state.final_rotation_armature.as_dquat())
            };
            state.lock_location = location;
            state.lock_rotation = rotation;
        }
        state.lock_amount = 1.0;
        if greater_than_previous {
            capture_foot_lock_movement_base(state, current_base);
        }
    } else if !greater_than_previous {
        state.lock_amount = new_lock_amount;
    }

    advance_foot_lock_movement_base(state, current_base);
    state.lock_rotation_before_foot_limit_armature =
        (armature_rotation.as_dquat().conjugate() * state.lock_rotation).as_quat();
    state.final_location = target_location.lerp(state.lock_location, f64::from(state.lock_amount));
    let final_rotation = source_foot_final_rotation(target_rotation, state.lock_rotation, state.lock_amount);
    state.final_rotation_armature =
        (armature_rotation.as_dquat().conjugate() * final_rotation).as_quat();
}

pub fn foot_lock_distance_squared(state: &AlsFootLockFootState) -> f32 {
    state.target_location.distance_squared(state.lock_location) as f32
}

#[derive(Debug, Clone, Copy)]
pub struct FootSideInput {
    pub raw_offset: f32,
    pub pin_request: bool,
}

impl Default for FootSideInput {
    fn default() -> Self {
        Self {
            raw_offset: 0.0,
            pin_request: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SpringFloatState {
    velocity: f32,
    previous_target: f32,
    valid: bool,
}

pub fn spring_damper_float(
    state: &mut SpringFloatState,
    current: f32,
    target: f32,
    delta_seconds: f32,
    frequency_hz: f32,
    damping_ratio: f32,
    target_velocity_amount: f32,
) -> f32 {
    if delta_seconds <= f32::EPSILON {
        return current;
    }
    if !state.valid {
        state.velocity = 0.0;
        state.previous_target = target;
        state.valid = true;
        return target;
    }
    let target_velocity =
        (target - state.previous_target) * target_velocity_amount.clamp(0.0, 1.0) / delta_seconds;
    let angular_frequency = frequency_hz.max(0.0) * std::f32::consts::TAU;
    if angular_frequency <= f32::EPSILON {
        state.previous_target = target;
        return current + state.velocity * delta_seconds;
    }
    let damping_ratio = damping_ratio.max(0.0);
    if damping_ratio <= f32::EPSILON {
        let error = current - target;
        let velocity_scaled = state.velocity / angular_frequency;
        let angle = angular_frequency * delta_seconds;
        let (sin, cos) = angle.sin_cos();
        let value = target + error * cos + velocity_scaled * sin;
        state.velocity = state.velocity * cos - error * angular_frequency * sin;
        state.previous_target = target;
        return value;
    }
    let smoothing_time = 2.0 / angular_frequency;
    let adjusted_target = target + target_velocity * damping_ratio * smoothing_time;
    let error = current - adjusted_target;
    let (value, velocity) = if damping_ratio > 1.0 {
        let damped_frequency = angular_frequency * (damping_ratio * damping_ratio - 1.0).sqrt();
        let c2 = -(state.velocity + (angular_frequency * damping_ratio - damped_frequency) * error)
            / (2.0 * damped_frequency);
        let c1 = error - c2;
        let a1 = damped_frequency - damping_ratio * angular_frequency;
        let a2 = -(damped_frequency + damping_ratio * angular_frequency);
        let e1 = (a1 * delta_seconds).exp();
        let e2 = (a2 * delta_seconds).exp();
        (
            adjusted_target + e1 * c1 + e2 * c2,
            e1 * c1 * a1 + e2 * c2 * a2,
        )
    } else if damping_ratio < 1.0 {
        let damped_frequency = angular_frequency * (1.0 - damping_ratio * damping_ratio).sqrt();
        let b = (state.velocity + error * damping_ratio * angular_frequency) / damped_frequency;
        let angle = damped_frequency * delta_seconds;
        let (sin, cos) = angle.sin_cos();
        let decay = (-damping_ratio * angular_frequency * delta_seconds).exp();
        let displacement = decay * (error * cos + b * sin);
        (
            adjusted_target + displacement,
            -displacement * damping_ratio * angular_frequency
                + decay * (b * damped_frequency * cos - error * damped_frequency * sin),
        )
    } else {
        let c1 = error;
        let c2 = state.velocity + error * angular_frequency;
        let decay = (-angular_frequency * delta_seconds).exp();
        (
            adjusted_target + (c1 + c2 * delta_seconds) * decay,
            (c2 - c1 * angular_frequency - c2 * angular_frequency * delta_seconds) * decay,
        )
    };
    state.velocity = velocity;
    state.previous_target = target;
    value
}

#[derive(Debug, Clone, Copy, Default)]
struct SideRuntime {
    offset: f32,
    target_offset: f32,
    ik_weight: f32,
    pin_weight: f32,
    locked_offset: Option<f32>,
    offset_spring: SpringFloatState,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsFootIkRuntime {
    left: SideRuntime,
    right: SideRuntime,
    pelvis_spring: SpringFloatState,
    pub pelvis_drop: f32,
}

pub fn foot_target_offset(ankle_y: f32, hit: &AlsGroundHit) -> f32 {
    let raw = hit.point.y + ANKLE_TO_SOLE - ankle_y;
    raw.clamp(-MAX_FOOT_REACH, MAX_FOOT_REACH)
}

pub fn foot_target_offset_in_armature(
    foot_target_armature: Vec3,
    hit_armature: Vec3,
    hit_normal_armature: Vec3,
) -> f32 {
    let slope_cos = hit_normal_armature.normalize_or_zero().y;
    if slope_cos <= f32::EPSILON {
        return 0.0;
    }
    let slope_offset = ANKLE_TO_SOLE / slope_cos - ANKLE_TO_SOLE;
    (hit_armature.y + ANKLE_TO_SOLE + slope_offset - foot_target_armature.y)
        .clamp(-MAX_FOOT_REACH, MAX_FOOT_REACH)
}

pub fn foot_ik_target(hit: &AlsGroundHit) -> Vec3 {
    hit.point + Vec3::Y * ANKLE_TO_SOLE
}

pub fn slope_is_walkable(normal: Vec3, max_slope_degrees: f32) -> bool {
    slope_is_walkable_relative(normal, Vec3::Y, max_slope_degrees)
}

pub fn slope_is_walkable_relative(normal: Vec3, up: Vec3, max_slope_degrees: f32) -> bool {
    let up_dot = normal.normalize_or_zero().dot(up.normalize_or_zero());
    up_dot >= max_slope_degrees.to_radians().cos()
}

pub fn wrap_foot_yaw(mut yaw: f32) -> f32 {
    while yaw > std::f32::consts::PI { yaw -= std::f32::consts::TAU; }
    while yaw < -std::f32::consts::PI { yaw += std::f32::consts::TAU; }
    yaw
}

pub fn foot_lock_location_constraint_rotation(
    thigh_axis: Vec3,
    lock_location: Vec3,
    thigh_angle_limit_degrees: f32,
) -> Option<Quat> {
    foot_lock_location_constraint_rotation_cm(thigh_axis, (lock_location.as_dvec3() * 100.0).as_vec3(),
        thigh_angle_limit_degrees)
}

pub(crate) fn foot_lock_location_constraint_rotation_cm(
    thigh_axis: Vec3,
    lock_location_cm: Vec3,
    thigh_angle_limit_degrees: f32,
) -> Option<Quat> {
    let normalize = |value: Vec2| {
        let squared = value.length_squared();
        if squared > 1e-8 { value * squared.sqrt().recip() } else { Vec2::ZERO }
    };
    let horizontal_relative = normalize(Vec2::new(lock_location_cm.x, lock_location_cm.z));
    let horizontal_axis = normalize(Vec2::new(thigh_axis.x, thigh_axis.z));
    let cross = horizontal_axis.y * horizontal_relative.x - horizontal_axis.x * horizontal_relative.y;
    let sign = if cross > 0.0 { 1.0 } else if cross < 0.0 { -1.0 } else { 0.0 };
    let angle = horizontal_axis.dot(horizontal_relative).clamp(-1.0, 1.0).acos().to_degrees() * sign;
    let limit = thigh_angle_limit_degrees;
    if angle.abs() <= limit + 1e-4 {
        return None;
    }
    Some(Quat::from_rotation_y((angle.clamp(-limit, limit) - angle).to_radians()))
}

pub fn constrain_foot_lock_location(
    thigh_axis: Vec3,
    lock_location: Vec3,
    thigh_angle_limit_degrees: f32,
) -> Vec3 {
    foot_lock_location_constraint_rotation(thigh_axis, lock_location, thigh_angle_limit_degrees)
        .map(|rotation| rotation * lock_location)
        .unwrap_or(lock_location)
}

pub(crate) fn source_foot_final_rotation(target: DQuat, lock: DQuat, amount: f32) -> DQuat {
    let bias = if target.dot(lock) >= 0.0 { 1.0 } else { -1.0 };
    let amount = f64::from(amount);
    rig_normalize(lock * amount + target * (bias * (1.0 - amount)))
}

fn source_foot_rotation_product(left: DQuat, right: DQuat) -> DQuat {
    left * right
}

fn source_foot_atan2(y: f32, x: f32) -> f32 {
    let abs_x = x.abs();
    let abs_y = y.abs();
    let y_abs_bigger = abs_y > abs_x;
    let maximum = if y_abs_bigger { abs_y } else { abs_x };
    let minimum = if y_abs_bigger { abs_x } else { abs_y };
    if maximum == 0.0 { return 0.0; }
    let ratio = minimum / maximum;
    let square = ratio * ratio;
    let mut angle = 0.007_212_885;
    angle = angle * square - 0.035_059_68;
    angle = angle * square + 0.081_675_88;
    angle = angle * square - 0.133_746_58;
    angle = angle * square + 0.198_565_63;
    angle = angle * square - 0.333_249_99;
    angle = (angle * square + 1.0) * ratio;
    if y_abs_bigger { angle = std::f32::consts::FRAC_PI_2 - angle; }
    if x < 0.0 { angle = std::f32::consts::PI - angle; }
    if y < 0.0 { -angle } else { angle }
}

pub fn constrain_foot_lock_rotation(
    lock_rotation: DQuat,
    target_rotation: DQuat,
    foot_angle_limit_degrees: f32,
) -> DQuat {
    let delta = source_foot_rotation_product(lock_rotation, target_rotation.conjugate()).as_quat();
    let twist_angle = wrap_foot_yaw(2.0 * source_foot_atan2(delta.y, delta.w)) * (180.0_f32 / std::f32::consts::PI);
    let limit = foot_angle_limit_degrees;
    if twist_angle.abs() <= limit + 1e-4 { return lock_rotation; }
    source_foot_rotation_product(
        Quat::from_rotation_y((twist_angle.clamp(-limit, limit) - twist_angle).to_radians()).as_dquat(),
        lock_rotation,
    )
}

fn move_toward_cap(current: f32, target: f32, max_delta: f32) -> f32 {
    let diff = target - current;
    if diff.abs() <= max_delta {
        target
    } else {
        current + diff.signum() * max_delta
    }
}

pub fn pelvis_drop(offset_left: f32, offset_right: f32, w_left: f32, w_right: f32) -> f32 {
    (offset_left * w_left).min(offset_right * w_right).min(0.0)
}

fn step_side(
    rt: &mut SideRuntime,
    input: &FootSideInput,
    ik_target: f32,
    settings: &AlsFootLockSettings,
    moving_smooth: bool,
    grounded: bool,
    dt: f32,
) -> f32 {
    rt.ik_weight = move_toward_cap(rt.ik_weight, ik_target, WEIGHT_BLEND_SPEED * dt);
    rt.target_offset = input.raw_offset * rt.ik_weight;

    let lock_active = settings.allow_foot_lock && input.pin_request;
    if lock_active && grounded && !moving_smooth {
        if rt.locked_offset.is_none() {
            rt.locked_offset = Some(input.raw_offset * rt.ik_weight);
        }
        rt.pin_weight = 1.0;
    } else if !lock_active {
        rt.pin_weight = 0.0;
        rt.locked_offset = None;
    } else {
        let decrease = if moving_smooth {
            settings.moving_decrease_per_second
        } else {
            settings.not_grounded_decrease_per_second
        };
        rt.pin_weight = (rt.pin_weight - decrease * dt).max(0.0);
        if rt.pin_weight < 1e-3 {
            rt.pin_weight = 0.0;
            rt.locked_offset = None;
        }
    }

    let locked = rt.locked_offset.unwrap_or(rt.offset);
    let base = rt.target_offset;
    let target = base + (locked - base) * rt.pin_weight;

    rt.offset = spring_damper_float(
        &mut rt.offset_spring,
        rt.offset,
        target,
        dt,
        FOOT_OFFSET_SPRING_FREQUENCY_HZ,
        FOOT_OFFSET_SPRING_DAMPING_RATIO,
        0.0,
    );
    rt.offset
}

impl AlsFootIkRuntime {
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        left: &FootSideInput,
        right: &FootSideInput,
        ik_target: f32,
        dt: f32,
    ) -> (f32, f32, f32) {
        self.update_with_foot_lock(
            left,
            right,
            ik_target,
            &AlsFootLockSettings::default(),
            false,
            ik_target > 0.0,
            dt,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_with_foot_lock(
        &mut self,
        left: &FootSideInput,
        right: &FootSideInput,
        ik_target: f32,
        settings: &AlsFootLockSettings,
        moving_smooth: bool,
        grounded: bool,
        dt: f32,
    ) -> (f32, f32, f32) {
        self.update_with_foot_lock_per_side(
            left,
            right,
            ik_target,
            ik_target,
            settings,
            moving_smooth,
            grounded,
            dt,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_with_foot_lock_per_side(
        &mut self,
        left: &FootSideInput,
        right: &FootSideInput,
        left_ik_target: f32,
        right_ik_target: f32,
        settings: &AlsFootLockSettings,
        moving_smooth: bool,
        grounded: bool,
        dt: f32,
    ) -> (f32, f32, f32) {
        let off_left = step_side(
            &mut self.left,
            left,
            left_ik_target,
            settings,
            moving_smooth,
            grounded,
            dt,
        );
        let off_right = step_side(
            &mut self.right,
            right,
            right_ik_target,
            settings,
            moving_smooth,
            grounded,
            dt,
        );
        self.pelvis_drop = spring_damper_float(
            &mut self.pelvis_spring,
            self.pelvis_drop,
            pelvis_drop(
                off_left,
                off_right,
                self.left.ik_weight,
                self.right.ik_weight,
            ),
            dt,
            PELVIS_OFFSET_SPRING_FREQUENCY_HZ,
            PELVIS_OFFSET_SPRING_DAMPING_RATIO,
            0.0,
        );
        (off_left, off_right, self.pelvis_drop)
    }

    pub fn side(&self, side: usize) -> (f32, f32) {
        let rt = match side {
            0 => &self.left,
            _ => &self.right,
        };
        (rt.offset, rt.pin_weight)
    }

    pub fn ik_weights(&self) -> (f32, f32) {
        (self.left.ik_weight, self.right.ik_weight)
    }
}

impl AlsDynamicTransitionState {
    pub fn update(
        &mut self,
        foot_lock: &AlsFootLockState,
        allow_transitions: bool,
        scale_world_space: f32,
        stance: Stance,
        config: &TransitionConfig,
    ) -> Option<TransitionSelection> {
        self.left_distance_squared = foot_lock_distance_squared(&foot_lock.left);
        self.right_distance_squared = foot_lock_distance_squared(&foot_lock.right);
        if self.frames_remaining > 0 {
            self.frames_remaining -= 1;
            self.selection = None;
            return None;
        }
        if !allow_transitions || !foot_lock.valid {
            self.selection = None;
            return None;
        }
        let distances = [&foot_lock.left, &foot_lock.right].map(|foot| {
            foot_lock_distance_squared_cm(foot.target_location, foot.lock_location)
        });
        let selection = select_dynamic_transition(
            distances,
            [foot_lock.left.lock_amount, foot_lock.right.lock_amount],
            scale_world_space,
            stance,
            config,
        );
        if selection.is_some() {
            self.frames_remaining = config.frame_delay;
        }
        self.selection = selection;
        selection
    }
}

pub fn foot_ik_state_system(
    mut commands: Commands,
    q: Query<
        (
            Entity,
            &LocomotionState,
            Option<&AlsFootPinRequest>,
            Option<&AlsFootIkTargets>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, loco, pins, targets) in &q {
        let grounded = *loco == LocomotionState::Grounded;
        let desired = AlsFootIkTargets {
            grounded,
            pin_left: pins.map(|p| p.left).unwrap_or(false) && grounded,
            pin_right: pins.map(|p| p.right).unwrap_or(false) && grounded,
        };
        if targets != Some(&desired) {
            commands.entity(entity).insert(desired);
        }
    }
}

pub const TURN_PIN_HOLD_SECS: f64 = 1.2;

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsFootPinHold {
    pub until_secs: f64,
}

pub fn extend_foot_pin_hold(hold: &mut AlsFootPinHold, now_secs: f64) {
    hold.until_secs = hold.until_secs.max(now_secs + TURN_PIN_HOLD_SECS);
}

pub fn foot_pin_request_system(
    time: Res<Time>,
    mut q: Query<
        (
            Entity,
            &crate::als::start_stop::StartStopState,
            Option<&mut AlsFootPinHold>,
            Option<&mut AlsFootPinRequest>,
        ),
        With<AlsCharacter>,
    >,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    for (entity, start_stop, hold, pins) in &mut q {
        let stopping = matches!(
            start_stop.phase,
            crate::als::start_stop::StartStopPhase::Stopping(_)
        );
        let turning = hold.map(|h| now < h.until_secs).unwrap_or(false);
        let want = stopping || turning;
        let desired = AlsFootPinRequest {
            left: want,
            right: want,
        };
        match pins {
            Some(mut current) => {
                if *current != desired {
                    if want {
                        *current = desired;
                    } else {
                        commands.entity(entity).remove::<AlsFootPinRequest>();
                    }
                }
            }
            None => {
                if want {
                    commands.entity(entity).insert(desired);
                }
            }
        }
    }
}

fn collect_named_descendants(
    root: Entity,
    children_q: &Query<&Children>,
    name_q: &Query<&Name>,
    joint_q: &Query<&AnimationTargetId>,
    wanted: &[&str],
    out: &mut Vec<(String, Entity)>,
) {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        let Ok(children) = children_q.get(entity) else {
            continue;
        };
        for child in children.iter() {
            if joint_q.get(child).is_ok()
                && let Ok(name) = name_q.get(child)
            {
                let n = name.as_str();
                if wanted.contains(&n) && !out.iter().any(|(s, _)| s == n) {
                    out.push((n.to_string(), child));
                }
            }
            stack.push(child);
        }
    }
}

pub fn build_foot_bone_map(
    root: Entity,
    children_q: &Query<&Children>,
    name_q: &Query<&Name>,
    joint_q: &Query<&AnimationTargetId>,
) -> Option<AlsFootBones> {
    let mut found = Vec::new();
    collect_named_descendants(
        root,
        children_q,
        name_q,
        joint_q,
        &FOOT_BONE_NAMES,
        &mut found,
    );
    let find = |name: &str| found.iter().find(|(n, _)| n == name).map(|(_, e)| *e);
    Some(AlsFootBones {
        left_thigh: find("thigh_l")?,
        left_calf: find("calf_l")?,
        left: find("foot_l")?,
        left_target: find("ik_foot_l"),
        left_virtual_target: find("VB foot_l"),
        right_thigh: find("thigh_r")?,
        right_calf: find("calf_r")?,
        right: find("foot_r")?,
        right_target: find("ik_foot_r"),
        right_virtual_target: find("VB foot_r"),
    })
}

pub fn foot_bone_map_system(
    mut commands: Commands,
    characters: Query<Entity, (With<AlsCharacter>, Without<AlsFootBones>)>,
    children_q: Query<&Children>,
    name_q: Query<&Name>,
    joint_q: Query<&AnimationTargetId>,
) {
    for entity in &characters {
        if let Some(map) = build_foot_bone_map(entity, &children_q, &name_q, &joint_q) {
            commands.entity(entity).insert(map);
        }
    }
}

pub fn foot_ground_probe_system(
    mut commands: Commands,
    spatial: SpatialQuery,
    characters: Query<
        (
            Entity,
            &GlobalTransform,
            &AlsFootBones,
            Option<&AlsFootTargetSelection>,
            Option<&crate::character_assembly::AlsCharacterAssembly>,
        ),
        With<AlsCharacter>,
    >,
    bones_tf: Query<&GlobalTransform, Without<AlsCharacter>>,
) {
    for (entity, char_tf, feet, selection, assembly) in &characters {
        let origin_base = char_tf.translation();
        let up = char_tf.up().as_vec3();
        let down = Dir3::new(-up).unwrap_or(Dir3::NEG_Y);
        let mut probe = AlsFootProbe::default();
        for side in 0..2usize {
            let chain = feet.chain(side);
            let Some(target) = feet.target(
                side,
                selection.map_or(true, |selection| selection.use_ik_bones),
            ).or_else(|| assembly.map(|assembly| assembly.avatar)) else { continue; };
            let Ok(target_tf) = bones_tf.get(target) else {
                continue;
            };
            let target_location = target_tf.translation();
            if (target_location - origin_base).length() > 3.0 {
                continue;
            }
            let origin = target_location + up * PROBE_UP_OFFSET;
            let filter = SpatialQueryFilter::from_excluded_entities([entity, chain.foot, target])
                .with_mask(crate::als::character::AlsCollisionLayer::Visibility);
            if let Some(hit) = spatial.cast_ray(origin, down, PROBE_DEPTH, true, &filter)
                && slope_is_walkable_relative(hit.normal, up, FOOT_IK_WALKABLE_FLOOR_ANGLE_DEGREES)
            {
                let value = AlsGroundHit {
                    point: origin - up * hit.distance,
                    normal: hit.normal,
                };
                if side == 0 {
                    probe.left = Some(value);
                } else {
                    probe.right = Some(value);
                }
            }
        }
        commands.entity(entity).insert(probe);
    }
}

pub struct AlsFootProbePlugin;

impl Plugin for AlsFootProbePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                foot_bone_map_system,
                foot_ground_probe_system,
                foot_pin_request_system,
                foot_ik_state_system,
            )
                .chain()
                .after(bevy::transform::TransformSystems::Propagate),
        );
    }
}

#[cfg(test)]
mod pin_request_tests {
    use super::*;
    use crate::als::start_stop::{StartStopPhase, StartStopState};
    use bevy::MinimalPlugins;

    fn app_with_character(phase: StartStopPhase) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, foot_pin_request_system);
        let e = app
            .world_mut()
            .spawn((
                AlsCharacter,
                StartStopState {
                    phase,
                    last_direction: None,
                },
            ))
            .id();
        (app, e)
    }

    #[test]
    fn stopping_phase_pins_both_feet() {
        let (mut app, e) = app_with_character(StartStopPhase::Stopping(
            crate::als::start_stop::MoveDirection::Forward,
        ));
        app.update();
        let pins = *app.world().get::<AlsFootPinRequest>(e).unwrap();
        assert!(pins.left && pins.right);
    }

    #[test]
    fn idle_phase_never_pins() {
        let (mut app, e) = app_with_character(StartStopPhase::Idle);
        app.update();
        assert!(app.world().get::<AlsFootPinRequest>(e).is_none());
    }

    #[test]
    fn hold_component_pins_until_expiry() {
        let (mut app, e) = app_with_character(StartStopPhase::Idle);
        let now = app.world().resource::<Time>().elapsed_secs_f64();
        app.world_mut().entity_mut(e).insert(AlsFootPinHold {
            until_secs: now + TURN_PIN_HOLD_SECS,
        });
        app.update();
        let pins = *app.world().get::<AlsFootPinRequest>(e).unwrap();
        assert!(pins.left && pins.right, "hold window pins feet");

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f64(TURN_PIN_HOLD_SECS * 2.0));
        app.world_mut()
            .get_mut::<AlsFootPinHold>(e)
            .unwrap()
            .until_secs = 0.0;
        app.update();
        assert!(
            app.world().get::<AlsFootPinRequest>(e).is_none(),
            "expired hold without stop phase clears pins"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::als::transitions::TransitionSequence;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn missing_selected_foot_target_keeps_its_absence() {
        let mut world = World::new();
        let left = world.spawn_empty().id();
        let right = world.spawn_empty().id();
        let left_ik = world.spawn_empty().id();
        let right_virtual = world.spawn_empty().id();
        let bones = AlsFootBones {
            left_thigh: left, left_calf: left, left, left_target: Some(left_ik), left_virtual_target: None,
            right_thigh: right, right_calf: right, right, right_target: None,
            right_virtual_target: Some(right_virtual),
        };
        assert_eq!(bones.target(0, true), Some(left_ik));
        assert_eq!(bones.target(0, false), None);
        assert_eq!(bones.target(1, true), None);
        assert_eq!(bones.target(1, false), Some(right_virtual));
    }

    #[test]
    fn foot_ground_probe_uses_mesh_origin_when_selected_marker_is_missing() {
        use avian3d::prelude::{Collider, CollisionLayers, LayerMask, RigidBody};
        use bevy::ecs::system::RunSystemOnce;
        let mut app = crate::als::motion::spatial_query_test_app();
        app.finish();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.4, 0.2, 0.4),
            Transform::from_xyz(0.0, -0.1, 0.0), CollisionLayers::new(
                [crate::als::character::AlsCollisionLayer::Default, crate::als::character::AlsCollisionLayer::Visibility],
                LayerMask::ALL)));
        let actor = app.world_mut().spawn((AlsCharacter, Transform::IDENTITY,
            AlsFootTargetSelection { use_ik_bones: false })).id();
        let avatar = app.world_mut().spawn((Transform::from_xyz(0.0, 0.1, 0.0), ChildOf(actor))).id();
        let left = app.world_mut().spawn(Transform::from_xyz(1.0, 0.1, 0.0)).id();
        let right = app.world_mut().spawn(Transform::from_xyz(0.05, 0.1, 0.0)).id();
        app.world_mut().entity_mut(actor).insert((crate::character_assembly::AlsCharacterAssembly { avatar, camera: None },
            AlsFootBones { left_thigh: left, left_calf: left, left, left_target: None, left_virtual_target: None,
                right_thigh: right, right_calf: right, right, right_target: None, right_virtual_target: Some(right) }));
        app.update();
        app.update();
        app.world_mut().run_system_once(foot_ground_probe_system).unwrap();
        let probe = app.world().get::<AlsFootProbe>(actor).unwrap();
        assert!(probe.left.is_some(), "The missing left marker must trace from the mesh origin");
        assert!(probe.right.is_some(), "The present right marker must retain its own trace");
    }

    #[test]
    fn target_offset_positive_when_ground_above_ankle() {
        let hit = AlsGroundHit {
            point: Vec3::new(1.0, 0.20, 2.0),
            normal: Vec3::Y,
        };
        let off = foot_target_offset(0.10, &hit);
        assert!((off - (0.20 + ANKLE_TO_SOLE - 0.10)).abs() < 1e-5);
    }

    #[test]
    fn offset_clamps_to_max_reach() {
        let deep = AlsGroundHit {
            point: Vec3::new(0.0, -5.0, 0.0),
            normal: Vec3::Y,
        };
        assert!((foot_target_offset(0.0, &deep) + MAX_FOOT_REACH).abs() < 1e-5);
    }

    #[test]
    fn slope_gate_matches_als_max_slope() {
        assert!(slope_is_walkable(Vec3::Y, 50.0));
        let steep = Vec3::new(1.0, 0.5, 0.0);
        assert!(!slope_is_walkable(steep, 50.0));
        assert!(!slope_is_walkable(Vec3::ZERO, 50.0));
    }

    #[test]
    fn source_trace_contract_uses_armature_space_and_source_units() {
        assert!((PROBE_UP_OFFSET - 0.50).abs() < f32::EPSILON);
        assert!((PROBE_DEPTH - 0.80).abs() < f32::EPSILON);
        assert!((ANKLE_TO_SOLE - 0.135).abs() < f32::EPSILON);
        assert!((FOOT_IK_WALKABLE_FLOOR_ANGLE_DEGREES - 45.0).abs() < f32::EPSILON);

        let target = Vec3::new(0.15, -0.92, -0.25);
        let hit = Vec3::new(0.15, -1.00, -0.25);
        let flat = foot_target_offset_in_armature(target, hit, Vec3::Y);
        let slope_normal = Vec3::new(0.0, 0.866_025_4, 0.5);
        let slope = foot_target_offset_in_armature(target, hit, slope_normal);
        let rotated_up = Quat::from_rotation_z(0.4) * Vec3::Y;

        assert!((flat - 0.055).abs() < 1e-5);
        assert!(slope > flat);
        assert!(slope_is_walkable_relative(
            rotated_up,
            rotated_up,
            FOOT_IK_WALKABLE_FLOOR_ANGLE_DEGREES
        ));
        assert!(!slope_is_walkable_relative(
            -rotated_up,
            rotated_up,
            FOOT_IK_WALKABLE_FLOOR_ANGLE_DEGREES
        ));
        assert_eq!(foot_target_offset_in_armature(target, hit, Vec3::ZERO), 0.0);
    }

    #[test]
    fn source_foot_lock_settings_and_constraints_are_preserved() {
        let settings = AlsFootLockSettings::default();
        assert!(settings.allow_foot_lock);
        assert_eq!(settings.moving_smooth_speed_threshold, 1.5);
        assert_eq!(settings.thigh_angle_limit_degrees, 90.0);
        assert_eq!(settings.foot_angle_limit_degrees, 40.0);
        assert_eq!(settings.moving_decrease_per_second, 5.0);
        assert_eq!(settings.not_grounded_decrease_per_second, 0.6);
        let target = Vec3::X;
        let constrained = constrain_foot_lock_location(Vec3::NEG_Z, target, 90.0);
        assert!((constrained.length() - 1.0).abs() < 1e-6);
        let lock_rotation = Quat::from_rotation_y(100.0_f32.to_radians());
        let constrained = constrain_foot_lock_rotation(lock_rotation.as_dquat(), DQuat::IDENTITY, 40.0);
        let (_, constrained_yaw, _) = constrained.as_quat().to_euler(EulerRot::XYZ);
        assert!((constrained_yaw - 40.0_f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn foot_thigh_constraint_matches_native_centimeter_angles() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audit/bf144/u07/foot-thigh-angle/source-cases.json",
        )).unwrap();
        let location = |row: &serde_json::Value| Vec3::new(
            row[0].as_f64().unwrap() as f32, row[2].as_f64().unwrap() as f32, row[1].as_f64().unwrap() as f32,
        );
        for row in cases.as_array().unwrap() {
            let source = &row["source"];
            let axis = location(&row["axis"]);
            let point = location(&row["location_cm"]);
            let offset = foot_lock_location_constraint_rotation_cm(axis, point, row["limit_degrees"].as_f64().unwrap() as f32);
            assert_eq!(offset.is_some(), source["limited"].as_bool().unwrap(), "{}", row["name"]);
            let actual = offset.map_or(point, |offset| offset * point);
            let expected = &source["location_cm"];
            let expected = Vec3::new(expected["x"].as_f64().unwrap() as f32,
                expected["z"].as_f64().unwrap() as f32, expected["y"].as_f64().unwrap() as f32);
            assert!(actual.abs_diff_eq(expected, 1e-5), "{} actual {actual:?} source {expected:?}", row["name"]);
        }
    }

    #[test]
    fn foot_lock_world_products_match_native_double_projection() {
        let lock = Quat::from_xyzw(0.6779860854148865, 0.6779860854148865,
            0.20083531737327576, 0.20083531737327576);
        let target = Quat::from_xyzw(0.6657518148422241, 0.6657516360282898,
            0.2382742166519165, 0.23827414214611053);
        let relative = source_foot_rotation_product(lock.as_dquat(), target.as_dquat().conjugate()).as_quat();
        assert_eq!(relative, Quat::from_xyzw(-8.642605564546102e-8, 0.05568023771047592,
            1.0626996527207666e-7, 0.9984485507011414));
        let offset = Quat::from_xyzw(0.0, -0.803851306438446, 0.0, 0.5948302745819092);
        let limited = source_foot_rotation_product(offset.as_dquat(), lock.as_dquat()).as_quat();
        assert_eq!(limited, Quat::from_xyzw(0.24184492230415344, 0.24184492230415344,
            0.6644629240036011, 0.6644629240036011));
    }

    #[test]
    fn foot_limit_uses_source_radians_to_degrees_at_threshold() {
        let lock = Quat::from_xyzw(-0.337_396_44, 0.337_396_4, 0.621_420_74, -0.621_420_6).as_dquat();
        let limit = 56.998_836_517_333_984_f32;
        assert_eq!(constrain_foot_lock_rotation(lock, DQuat::IDENTITY, limit), lock);
    }

    #[test]
    fn foot_twist_atan2_matches_native_float_samples() {
        assert_eq!(source_foot_atan2(0.055_680_238, 0.998_448_55), 0.055_709_07);
        assert_eq!(source_foot_atan2(0.642_787_64, 0.766_044_44), 0.698_131_5);
        assert_eq!(source_foot_atan2(0.0, 0.0), 0.0);
    }

    #[test]
    fn foot_limit_uses_source_atan2_at_threshold() {
        let lock = Quat::from_xyzw(0.0, 0.642_787_64, 0.0, 0.766_044_44).as_dquat();
        let limit = 79.999_870_300_292_97_f32;
        assert_eq!(constrain_foot_lock_rotation(lock, DQuat::IDENTITY, limit), lock);
    }

    #[test]
    fn foot_lock_preserves_world_precision_until_component_storage() {
        let target = bevy::math::DQuat::from_rotation_z(0.314_159_265_358_979_3);
        let previous_component = Quat::from_rotation_y(0.3);
        let mut state = AlsFootLockFootState::default();
        reset_foot_lock_to_target(&mut state, Vec3::ZERO, target, previous_component);
        assert_ne!(target, target.as_quat().as_dquat());
        assert_eq!(state.target_rotation, target);
        assert_eq!(state.lock_rotation, target);
        let saved_local = (previous_component.as_dquat().conjugate() * target).as_quat();
        assert_eq!(state.final_rotation_armature, saved_local);

        let current_component = Quat::from_rotation_x(-0.2);
        refresh_foot_lock(&mut state, Vec3::ZERO, target, current_component,
            1.0, 1.0, true, false, true, false, 0.01, None);
        assert_eq!(state.lock_rotation, current_component.as_dquat() * saved_local.as_dquat());
    }

    #[test]
    fn foot_lock_final_rotation_uses_source_sign_for_negative_dot() {
        let target = DQuat::IDENTITY;
        let lock = -DQuat::from_rotation_y(0.4);
        let mut state = AlsFootLockFootState { lock_amount: 0.5, lock_rotation: lock, ..default() };
        refresh_foot_lock(&mut state, Vec3::ZERO, target, Quat::IDENTITY,
            1.0, 0.5, true, false, true, false, 0.01, None);
        let expected = (lock * 0.5 - target * 0.5).normalize().as_quat();
        assert_eq!(state.final_rotation_armature, expected);
        assert!(state.final_rotation_armature.w < 0.0);
    }

    #[test]
    fn foot_lock_base_cache_rounds_only_at_the_source_boundary() {
        let mut state = AlsFootLockFootState {
            lock_amount: 1.0,
            lock_rotation: bevy::math::DQuat::from_rotation_z(0.314_159_265_358_979_3),
            ..default()
        };
        let base = AlsFootLockMovementBase {
            entity: Entity::PLACEHOLDER,
            translation: Vec3::ZERO,
            rotation: Quat::from_xyzw(0.0, 0.0, 0.0, 0.99999),
        };
        let expected_local = (base.rotation.as_dquat().conjugate() * state.lock_rotation).as_quat();
        capture_foot_lock_movement_base(&mut state, Some(base));
        assert_eq!(state.lock_rotation_movement_base, expected_local);
        advance_foot_lock_movement_base(&mut state, Some(base));
        assert_eq!(state.lock_rotation, base.rotation.as_dquat() * expected_local.as_dquat());
    }

    #[test]
    fn foot_lock_tracks_translation_and_rotation_of_its_movement_base() {
        let mut state = AlsFootLockFootState {
            lock_amount: 1.0,
            lock_location: DVec3::new(2.0, 0.0, 0.0),
            lock_rotation: DQuat::IDENTITY,
            ..Default::default()
        };
        let initial_base = AlsFootLockMovementBase {
            entity: Entity::from_raw_u32(1).unwrap(),
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        };
        capture_foot_lock_movement_base(&mut state, Some(initial_base));
        let moved_base = AlsFootLockMovementBase {
            entity: initial_base.entity,
            translation: Vec3::new(3.0, 0.0, 0.0),
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        };
        advance_foot_lock_movement_base(&mut state, Some(moved_base));
        assert!(state.lock_location.distance(DVec3::new(3.0, 0.0, -2.0)) < 1e-5);
        assert!(state.lock_rotation.dot(moved_base.rotation.as_dquat()).abs() > 0.99999);
    }

    #[test]
    fn moving_foot_lock_matches_native_centimeter_cache() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audit/bf144/u07/foot-target-precision/moving-base/source-cases.json",
        )).unwrap();
        let position = |row: &serde_json::Value| DVec3::new(
            row[0].as_f64().unwrap(), row[2].as_f64().unwrap(), row[1].as_f64().unwrap(),
        );
        let vector = |row: &serde_json::Value| DVec3::new(
            row["x"].as_f64().unwrap(), row["z"].as_f64().unwrap(), row["y"].as_f64().unwrap(),
        );
        let rotation = |row: &serde_json::Value| Quat::from_xyzw(
            -row[0].as_f64().unwrap() as f32, -row[2].as_f64().unwrap() as f32,
            -row[1].as_f64().unwrap() as f32, row[3].as_f64().unwrap() as f32,
        );
        for row in cases.as_array().unwrap() {
            let mut state = AlsFootLockFootState {
                lock_amount: 1.0,
                lock_location: position(&row["lock_world_cm"]) * 0.01,
                ..default()
            };
            let entity = Entity::PLACEHOLDER;
            capture_foot_lock_movement_base(&mut state, Some(AlsFootLockMovementBase {
                entity, translation: (position(&row["start_base_cm"]) * 0.01).as_vec3(),
                rotation: rotation(&row["start_rotation_xyzw"]),
            }));
            assert_eq!(state.lock_location_movement_base_cm,
                vector(&row["relative_cm"]).as_vec3(), "{}", row["name"]);
            advance_foot_lock_movement_base(&mut state, Some(AlsFootLockMovementBase {
                entity, translation: (position(&row["end_base_cm"]) * 0.01).as_vec3(),
                rotation: rotation(&row["end_rotation_xyzw"]),
            }));
            let base = state.movement_base.unwrap().translation.as_dvec3();
            let source_offset = (vector(&row["world_cm"]) - position(&row["end_base_cm"])) * 0.01;
            assert!((state.lock_location - base).abs_diff_eq(source_offset, 1e-12),
                "{} actual {:?} source {:?}", row["name"], state.lock_location - base, source_offset);
        }
    }

    #[test]
    fn foot_lock_rebases_and_detaches_without_losing_its_world_transform() {
        let mut state = AlsFootLockFootState {
            lock_amount: 1.0,
            lock_location: DVec3::new(1.0, 0.0, 0.0),
            lock_rotation: DQuat::IDENTITY,
            ..Default::default()
        };
        let first = AlsFootLockMovementBase {
            entity: Entity::from_raw_u32(1).unwrap(),
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        };
        capture_foot_lock_movement_base(&mut state, Some(first));
        let moved_first = AlsFootLockMovementBase {
            entity: first.entity,
            translation: Vec3::new(2.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
        };
        advance_foot_lock_movement_base(&mut state, Some(moved_first));
        let second = AlsFootLockMovementBase {
            entity: Entity::from_raw_u32(2).unwrap(),
            translation: Vec3::new(5.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
        };
        capture_foot_lock_movement_base(&mut state, Some(second));
        assert_eq!(
            state.movement_base.map(|base| base.entity),
            Some(second.entity)
        );
        capture_foot_lock_movement_base(&mut state, None);
        assert_eq!(state.movement_base, None);
        assert!(state.lock_location.distance(DVec3::new(3.0, 0.0, 0.0)) < 1e-5);
    }

    #[test]
    fn foot_lock_refresh_captures_relocks_and_base_changes_at_source_boundaries() {
        let base = AlsFootLockMovementBase {
            entity: Entity::from_raw_u32(1).unwrap(),
            translation: Vec3::new(2.0, 0.0, 3.0),
            rotation: Quat::from_rotation_y(0.4),
        };
        for current_base in [None, Some(base)] {
            for previous_amount in [0.0, 0.5, 0.95] {
                for became_valid in [false, true] {
                    let mut state = AlsFootLockFootState {
                        lock_amount: previous_amount,
                        lock_location: DVec3::X,
                        final_location: DVec3::Z,
                        final_rotation_armature: Quat::from_rotation_y(0.3),
                        ..default()
                    };
                    refresh_foot_lock(&mut state, Vec3::Y, DQuat::IDENTITY, Quat::IDENTITY, 1.0, 1.0, true, false, true, became_valid, 0.01, current_base);
                    let captured = became_valid.then_some(Vec3::Y).unwrap_or(Vec3::Z);
                    let expected = if became_valid || previous_amount <= 0.9 || current_base.is_some() { captured } else { Vec3::X };
                    assert!(state.lock_location.distance(expected.as_dvec3()) < 1e-5, "Relock differs: {previous_amount}, {became_valid}, {current_base:?}");
                    assert_eq!(state.movement_base, current_base);
                    let location = state.lock_location;
                    let rotation = state.lock_rotation;
                    let second = AlsFootLockMovementBase { entity:Entity::from_raw_u32(2).unwrap(), rotation:Quat::from_rotation_y(-1.0), ..base };
                    refresh_foot_lock(&mut state, Vec3::Y, DQuat::IDENTITY, Quat::IDENTITY, 1.0, 1.0, true, false, true, false, 0.01, Some(second));
                    assert!(state.lock_location.distance(location) < 1e-5);
                    assert!(state.lock_rotation.normalize().angle_between(rotation.normalize()) < 1e-5);
                    refresh_foot_lock(&mut state, Vec3::Y, DQuat::IDENTITY, Quat::IDENTITY, 1.0, 1.0, true, false, true, false, 0.01, None);
                    assert!(state.lock_location.distance(location) < 1e-5);
                    assert_eq!(state.movement_base, None);
                    refresh_foot_lock(&mut state, Vec3::Y, DQuat::IDENTITY, Quat::IDENTITY, 1.0, 1.0, false, false, true, false, 0.01, Some(base));
                    assert_eq!(state.lock_amount, 0.0);
                    assert_eq!(state.movement_base, None);
                    assert_eq!(state.lock_rotation_before_foot_limit_armature, Quat::IDENTITY);
                }
            }
        }
    }

    #[test]
    fn source_moving_and_airborne_lock_release_rates_are_consumed() {
        let mut runtime = AlsFootIkRuntime::default();
        let input = FootSideInput {
            raw_offset: 0.1,
            pin_request: true,
        };
        let settings = AlsFootLockSettings::default();
        runtime.update_with_foot_lock(&input, &input, 1.0, &settings, false, true, 1.0 / 60.0);
        assert!(runtime.side(0).1 > 0.99);
        runtime.update_with_foot_lock(&input, &input, 1.0, &settings, true, true, 0.1);
        assert!((runtime.side(0).1 - 0.5).abs() < 1e-6);
        runtime.update_with_foot_lock(&input, &input, 0.0, &settings, false, false, 0.1);
        assert!((runtime.side(0).1 - 0.44).abs() < 1e-6);
    }

    #[test]
    fn dynamic_transition_uses_full_lock_distance_and_two_frame_block() {
        let mut foot_lock = AlsFootLockState {
            valid: true,
            ..Default::default()
        };
        foot_lock.left.lock_amount = 1.0;
        foot_lock.left.target_location = DVec3::new(0.2, 0.0, 0.0);
        foot_lock.left.lock_location = DVec3::ZERO;
        let mut dynamic = AlsDynamicTransitionState::default();
        let selected = dynamic
            .update(
                &foot_lock,
                true,
                1.0,
                Stance::Standing,
                &TransitionConfig::default(),
            )
            .unwrap();
        assert_eq!(selected.sequence, TransitionSequence::StandingLeft);
        assert_eq!(selected.play_rate, 1.5);
        assert_eq!(dynamic.frames_remaining, 2);
        assert!(
            dynamic
                .update(
                    &foot_lock,
                    true,
                    1.0,
                    Stance::Standing,
                    &TransitionConfig::default(),
                )
                .is_none()
        );
        assert_eq!(dynamic.frames_remaining, 1);
    }

    #[test]
    fn dynamic_transition_detects_horizontal_lock_drift_and_respects_gates() {
        let mut foot_lock = AlsFootLockState {
            valid: true,
            ..Default::default()
        };
        foot_lock.left.lock_amount = 1.0;
        foot_lock.left.target_location = DVec3::new(0.12, 0.0, 0.0);
        foot_lock.left.lock_location = DVec3::ZERO;
        let mut dynamic = AlsDynamicTransitionState::default();
        let blocked = dynamic.update(
            &foot_lock,
            false,
            1.0,
            Stance::Standing,
            &TransitionConfig::default(),
        );
        let selected = dynamic.update(
            &foot_lock,
            true,
            1.0,
            Stance::Standing,
            &TransitionConfig::default(),
        );
        let horizontal_distance_squared = dynamic.left_distance_squared;
        let first_sequence = selected.map(|selection| selection.sequence);
        let reentry_one = dynamic.update(
            &foot_lock,
            true,
            1.0,
            Stance::Standing,
            &TransitionConfig::default(),
        );
        let reentry_two = dynamic.update(
            &foot_lock,
            true,
            1.0,
            Stance::Standing,
            &TransitionConfig::default(),
        );
        let reentry_three = dynamic.update(
            &foot_lock,
            true,
            1.0,
            Stance::Standing,
            &TransitionConfig::default(),
        );
        let passed = blocked.is_none()
            && first_sequence == Some(TransitionSequence::StandingLeft)
            && horizontal_distance_squared
                > TransitionConfig::default()
                    .dynamic_foot_lock_distance
                    .powi(2)
            && reentry_one.is_none()
            && reentry_two.is_none()
            && reentry_three.is_some();
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.44][VERDICT] horizontal_distance_squared={horizontal_distance_squared:.6} blocked={} first={:?} reentry=({},{},{}) -> {verdict}",
            blocked.is_none(),
            first_sequence,
            reentry_one.is_none(),
            reentry_two.is_none(),
            reentry_three.is_some(),
        );
        assert!(passed);
    }

    #[test]
    fn pelvis_takes_lower_weighted_side_and_never_rises() {
        assert!((pelvis_drop(-0.10, -0.02, 1.0, 1.0) + 0.10).abs() < 1e-6);
        assert!((pelvis_drop(-0.10, -0.02, 0.0, 1.0) + 0.02).abs() < 1e-6);
        assert!(pelvis_drop(0.05, 0.03, 1.0, 1.0) <= 0.0);
        assert_eq!(pelvis_drop(0.05, 0.03, 0.0, 0.0), 0.0);
    }

    #[test]
    fn weights_blend_in_and_out_with_speed_cap() {
        let mut rt = AlsFootIkRuntime::default();
        let inp_l = FootSideInput::default();
        let inp_r = FootSideInput::default();

        let (off_l, _, pelvis) = rt.update(&inp_l, &inp_r, 0.0, DT);
        assert_eq!(off_l, 0.0);
        assert_eq!(pelvis, 0.0);

        for _ in 0..60 {
            rt.update(&inp_l, &inp_r, 1.0, DT);
        }
        let (wl, wr) = rt.ik_weights();
        assert!((wl - 1.0).abs() < 1e-4 && (wr - 1.0).abs() < 1e-4);

        for _ in 0..60 {
            rt.update(&inp_l, &inp_r, 0.0, DT);
        }
        let (wl2, _) = rt.ik_weights();
        assert!(wl2.abs() < 1e-4);
    }

    #[test]
    fn offset_tracks_raw_input_after_blend_in() {
        let mut rt = AlsFootIkRuntime::default();
        let raw = FootSideInput {
            raw_offset: 0.20,
            pin_request: false,
        };
        for _ in 0..120 {
            rt.update(&raw, &raw, 1.0, DT);
        }
        let (off_l, _) = rt.side(0);
        assert!((off_l - 0.20).abs() < 1e-3, "got {off_l}");
    }

    #[test]
    fn pin_locks_plant_frame_target_and_releases() {
        let mut rt = AlsFootIkRuntime::default();
        let raised = FootSideInput {
            raw_offset: 0.25,
            pin_request: false,
        };
        for _ in 0..90 {
            rt.update(&raised, &raised, 1.0, DT);
        }
        let (settled, _) = rt.side(0);
        assert!((settled - 0.25).abs() < 1e-2);

        let planted = FootSideInput {
            raw_offset: -0.10,
            pin_request: true,
        };
        for _ in 0..60 {
            rt.update(&planted, &planted, 1.0, DT);
        }
        let (held, pw) = rt.side(0);
        assert!(pw > 0.99);
        assert!(
            (held - (-0.10)).abs() < 1e-2,
            "pin converges to plant-frame raw target, held={held}"
        );

        let swinging = FootSideInput {
            raw_offset: 0.40,
            pin_request: true,
        };
        for _ in 0..60 {
            rt.update(&swinging, &swinging, 1.0, DT);
        }
        let (frozen, pw_frozen) = rt.side(0);
        assert!(pw_frozen > 0.99);
        assert!(
            (frozen - (-0.10)).abs() < 1e-2,
            "pinned offset ignores later raw swing, frozen={frozen}"
        );

        let released = FootSideInput {
            raw_offset: -0.10,
            pin_request: false,
        };
        for _ in 0..120 {
            rt.update(&released, &released, 1.0, DT);
        }
        let (after, pw_after) = rt.side(0);
        assert!(pw_after < 1e-3);
        assert!(
            (after - (-0.10)).abs() < 1e-3,
            "returns to raw after release"
        );
    }

    #[test]
    fn air_grounded_transition_zeroes_effective_offsets() {
        let mut rt = AlsFootIkRuntime::default();
        let raised = FootSideInput {
            raw_offset: 0.40,
            pin_request: false,
        };
        for _ in 0..120 {
            rt.update(&raised, &raised, 1.0, DT);
        }
        for _ in 0..120 {
            rt.update(&raised, &raised, 0.0, DT);
        }
        let (off_l, _) = rt.side(0);
        assert!(
            off_l.abs() < 1e-2,
            "ik weight 0 pulls offset to rest, got {off_l}"
        );
    }

    #[test]
    fn pelvis_drop_interpolates_without_overshoot() {
        let mut rt = AlsFootIkRuntime::default();
        let low = FootSideInput {
            raw_offset: -0.20,
            pin_request: false,
        };
        let flat = FootSideInput::default();
        let mut prev = 0.0;
        for _ in 0..120 {
            let (_, _, drop) = rt.update(&low, &flat, 1.0, DT);
            assert!(drop <= prev + 1e-5, "no overshoot toward negative drop");
            prev = drop;
        }
        let (_, _, drop) = rt.update(&low, &flat, 1.0, DT);
        assert!(
            (drop + 0.20).abs() < 1e-2,
            "settles at min side, got {drop}"
        );
    }

    #[test]
    fn foot_offset_and_pelvis_follow_source_spring_trajectory() {
        fn response(delta_seconds: f32) -> (f32, f32, f32) {
            let mut runtime = AlsFootIkRuntime::default();
            let flat = FootSideInput::default();
            let step = FootSideInput {
                raw_offset: -0.20,
                pin_request: false,
            };
            runtime.update(&flat, &flat, 1.0, delta_seconds);
            runtime.left.ik_weight = 1.0;
            runtime.right.ik_weight = 1.0;
            let (first, _, _) = runtime.update(&step, &step, 1.0, delta_seconds);
            let steps = (0.5 / delta_seconds).round() as usize;
            for _ in 1..steps {
                runtime.update(&step, &step, 1.0, delta_seconds);
            }
            let (foot, _, pelvis) = runtime.update(&step, &step, 1.0, delta_seconds);
            (first, foot, pelvis)
        }

        let (first_30, foot_30, pelvis_30) = response(1.0 / 30.0);
        let (first_120, foot_120, pelvis_120) = response(1.0 / 120.0);
        let legacy_first = -0.20;
        let passed = first_30 > legacy_first + 1e-3
            && first_120 > legacy_first + 1e-3
            && pelvis_30 > foot_30
            && pelvis_120 > foot_120
            && (foot_30 - foot_120).abs() < 2e-3
            && (pelvis_30 - pelvis_120).abs() < 2e-3
            && (foot_30 + 0.20).abs() < 2e-3
            && (pelvis_30 + 0.20).abs() < 6e-3;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.45][VERDICT] first30={first_30:.6} first120={first_120:.6} foot30={foot_30:.6} foot120={foot_120:.6} pelvis30={pelvis_30:.6} pelvis120={pelvis_120:.6} legacy_first={legacy_first:.6} -> {verdict}"
        );
        assert!(passed);
    }
}
