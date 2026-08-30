use avian3d::prelude::{
    Collider, LinearVelocity, ShapeCastConfig, SpatialQuery, SpatialQueryFilter,
};
use bevy::prelude::*;

use crate::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_CAPSULE_RADIUS, ALS_CHARACTER_FLOOR_CLEARANCE,
    als_character_capsule,
};
use crate::input::AlsInputCommand;
use crate::orientation::{world_forward_to_yaw, yaw_to_world_forward};
use crate::state::{LocomotionState, OverlayState, Stance};

pub const MANTLE_HIGH_HEIGHT_THRESHOLD: f32 = 1.25;
pub const MANTLE_TRACE_ANGLE_LIMIT: f32 = 110.0_f32.to_radians();
pub const MANTLE_MAX_REACH_ANGLE: f32 = 50.0_f32.to_radians();
pub const MANTLE_WALKABLE_FLOOR_COS: f32 = 0.71;
pub const MANTLE_SLOPE_ANGLE_COS: f32 = 0.819_152_06;
pub const MANTLE_TARGET_SPEED_THRESHOLD: f32 = 0.1;
pub const MANTLE_TRACE_CAPSULE_SHRINK: f32 = 0.01;
pub const MANTLE_MIN_FLOOR_DISTANCE: f32 = ALS_CHARACTER_FLOOR_CLEARANCE;
pub const MANTLE_MAX_FLOOR_DISTANCE: f32 = 0.024;
pub const MANTLE_BLEND_IN_SECS: f32 = 0.2;
pub const MANTLE_BLEND_OUT_SECS: f32 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MantleTraceSettings {
    pub minimum_ledge_height: f32,
    pub maximum_ledge_height: f32,
    pub reach_distance: f32,
    pub target_location_offset: f32,
    pub start_location_offset: f32,
}

pub const MANTLE_GROUNDED_TRACE: MantleTraceSettings = MantleTraceSettings {
    minimum_ledge_height: 0.5,
    maximum_ledge_height: 2.25,
    reach_distance: 0.75,
    target_location_offset: 0.15,
    start_location_offset: 0.55,
};

pub const MANTLE_IN_AIR_TRACE: MantleTraceSettings = MantleTraceSettings {
    minimum_ledge_height: 0.5,
    maximum_ledge_height: 1.5,
    reach_distance: 0.7,
    target_location_offset: 0.15,
    start_location_offset: 0.55,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MantleActionSettings {
    pub sequence_length: f32,
    pub rate_scale: f32,
    pub start_time_reference_min_height: f32,
    pub start_time_reference_max_height: f32,
    pub start_time_at_min_height: f32,
    pub start_time_at_max_height: f32,
    pub motion_warping_start_time: f32,
    pub motion_warping_end_time: f32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsMantleControlSettings {
    pub allow_mantling: bool,
    pub auto_start_in_air: bool,
}

impl Default for AlsMantleControlSettings {
    fn default() -> Self {
        Self {
            allow_mantling: true,
            auto_start_in_air: true,
        }
    }
}

pub fn mantle_start_allowed(settings: AlsMantleControlSettings, action_active: bool) -> bool {
    settings.allow_mantling && !action_active
}

pub fn auto_mantle_start_allowed(
    settings: AlsMantleControlSettings,
    action_active: bool,
    locomotion: LocomotionState,
) -> bool {
    mantle_start_allowed(settings, action_active)
        && settings.auto_start_in_air
        && locomotion == LocomotionState::Air
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsMantleTargetPermission {
    pub allows_step_up: bool,
}

impl Default for AlsMantleTargetPermission {
    fn default() -> Self {
        Self {
            allows_step_up: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MantleRootHeightSample {
    pub time: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum MantleStartTimeMode {
    #[default]
    SourceDefault,
    AutoRootHeight {
        samples: Vec<MantleRootHeightSample>,
        sampling_interval: f32,
    },
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct AlsMantleStartTimeSettings {
    pub high: MantleStartTimeMode,
    pub in_air: MantleStartTimeMode,
    pub low: MantleStartTimeMode,
}

impl Default for AlsMantleStartTimeSettings {
    fn default() -> Self {
        Self {
            high: MantleStartTimeMode::SourceDefault,
            in_air: MantleStartTimeMode::SourceDefault,
            low: MantleStartTimeMode::SourceDefault,
        }
    }
}

pub const MANTLE_HIGH_ACTION: MantleActionSettings = MantleActionSettings {
    sequence_length: 2.666_666_7,
    rate_scale: 1.2,
    start_time_reference_min_height: 1.25,
    start_time_reference_max_height: 2.0,
    start_time_at_min_height: 0.6,
    start_time_at_max_height: 0.0,
    motion_warping_start_time: 0.3,
    motion_warping_end_time: 0.6,
};

pub const MANTLE_IN_AIR_ACTION: MantleActionSettings = MantleActionSettings {
    sequence_length: 2.666_666_7,
    rate_scale: 1.2,
    start_time_reference_min_height: 1.25,
    start_time_reference_max_height: 2.0,
    start_time_at_min_height: 0.8,
    start_time_at_max_height: 0.0,
    motion_warping_start_time: 0.3,
    motion_warping_end_time: 0.6,
};

pub const MANTLE_LOW_ACTION: MantleActionSettings = MantleActionSettings {
    sequence_length: 1.5,
    rate_scale: 1.0,
    start_time_reference_min_height: 0.5,
    start_time_reference_max_height: 1.0,
    start_time_at_min_height: 0.5,
    start_time_at_max_height: 0.0,
    motion_warping_start_time: 0.3,
    motion_warping_end_time: 0.7,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MantleKind {
    #[default]
    High,
    InAir,
    LowLeft,
    LowRight,
    LowBothHands,
    LowBox,
}

impl MantleKind {
    pub fn action_settings(self) -> MantleActionSettings {
        match self {
            Self::High => MANTLE_HIGH_ACTION,
            Self::InAir => MANTLE_IN_AIR_ACTION,
            Self::LowLeft | Self::LowRight | Self::LowBothHands | Self::LowBox => MANTLE_LOW_ACTION,
        }
    }

    pub fn is_high(self) -> bool {
        matches!(self, Self::High | Self::InAir)
    }

    pub fn graph_low_right_weight(self) -> f32 {
        match self {
            Self::LowLeft => 0.0,
            Self::High | Self::InAir | Self::LowRight | Self::LowBothHands | Self::LowBox => 1.0,
        }
    }
}

impl AlsMantleStartTimeSettings {
    pub fn mode_for(&self, kind: MantleKind) -> &MantleStartTimeMode {
        match kind {
            MantleKind::High => &self.high,
            MantleKind::InAir => &self.in_air,
            MantleKind::LowLeft
            | MantleKind::LowRight
            | MantleKind::LowBothHands
            | MantleKind::LowBox => &self.low,
        }
    }

    pub fn start_time_for(&self, kind: MantleKind, mantling_height: f32) -> f32 {
        mantle_start_time_with_mode(kind, mantling_height, self.mode_for(kind))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MantleIntent {
    pub kind: MantleKind,
    pub target: Vec3,
    pub mantling_height: f32,
    pub yaw_target: f32,
    pub target_entity: Entity,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MantleProbeTrace {
    pub forward_start: Option<Vec3>,
    pub forward_end: Option<Vec3>,
    pub forward_hit: Option<Vec3>,
    pub downward_start: Option<Vec3>,
    pub downward_end: Option<Vec3>,
    pub downward_hit: Option<Vec3>,
    pub target_capsule_center: Option<Vec3>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MantleDetection {
    pub intent: Option<MantleIntent>,
    pub trace: MantleProbeTrace,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MantleDetectionInput {
    pub origin: Vec3,
    pub actor_yaw: f32,
    pub velocity: Vec3,
    pub input_direction: Vec3,
    pub locomotion: LocomotionState,
    pub stance: Stance,
    pub overlay: OverlayState,
    pub capsule_half_height: f32,
    pub capsule_radius: f32,
    pub capsule_scale: f32,
}

impl Default for MantleDetectionInput {
    fn default() -> Self {
        Self {
            origin: Vec3::ZERO,
            actor_yaw: 0.0,
            velocity: Vec3::ZERO,
            input_direction: Vec3::ZERO,
            locomotion: LocomotionState::Grounded,
            stance: Stance::Standing,
            overlay: OverlayState::Default,
            capsule_half_height: ALS_CHARACTER_CAPSULE_HALF_HEIGHT,
            capsule_radius: ALS_CHARACTER_CAPSULE_RADIUS,
            capsule_scale: 1.0,
        }
    }
}

pub fn scaled_mantle_capsule_dimensions(
    base_half_height: f32,
    base_radius: f32,
    world_scale: Vec3,
) -> (f32, f32, f32) {
    let world_scale = world_scale.abs();
    let vertical_scale = world_scale.y.max(1e-4);
    let horizontal_scale = world_scale.x.max(world_scale.z).max(1e-4);
    let capsule_radius = base_radius * horizontal_scale;
    let capsule_half_height = (base_half_height * vertical_scale).max(capsule_radius);
    (capsule_half_height, capsule_radius, vertical_scale)
}

pub fn yaw_from_direction(direction: Vec3) -> f32 {
    world_forward_to_yaw(direction)
}

pub fn wrap_angle(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

pub fn mantle_trace_settings(locomotion: LocomotionState) -> MantleTraceSettings {
    match locomotion {
        LocomotionState::Air => MANTLE_IN_AIR_TRACE,
        LocomotionState::Grounded | LocomotionState::Ragdolling => MANTLE_GROUNDED_TRACE,
    }
}

pub fn select_low_mantle_kind(overlay: OverlayState) -> MantleKind {
    match overlay {
        OverlayState::Injured => MantleKind::LowLeft,
        OverlayState::HandsTied => MantleKind::LowBothHands,
        OverlayState::Box => MantleKind::LowBox,
        OverlayState::Rifle
        | OverlayState::Default
        | OverlayState::Masculine
        | OverlayState::Feminine
        | OverlayState::PistolOneHanded
        | OverlayState::PistolTwoHanded
        | OverlayState::Bow
        | OverlayState::Torch
        | OverlayState::Binoculars
        | OverlayState::Barrel => MantleKind::LowRight,
    }
}

pub fn select_mantle_kind(
    locomotion: LocomotionState,
    overlay: OverlayState,
    mantling_height: f32,
) -> MantleKind {
    if locomotion == LocomotionState::Air {
        MantleKind::InAir
    } else if mantling_height > MANTLE_HIGH_HEIGHT_THRESHOLD {
        MantleKind::High
    } else {
        select_low_mantle_kind(overlay)
    }
}

pub fn mantle_start_time(kind: MantleKind, mantling_height: f32) -> f32 {
    let settings = kind.action_settings();
    let alpha = ((mantling_height - settings.start_time_reference_min_height)
        / (settings.start_time_reference_max_height - settings.start_time_reference_min_height))
        .clamp(0.0, 1.0);
    settings.start_time_at_min_height
        + (settings.start_time_at_max_height - settings.start_time_at_min_height) * alpha
}

pub fn mantle_start_time_with_mode(
    kind: MantleKind,
    mantling_height: f32,
    mode: &MantleStartTimeMode,
) -> f32 {
    match mode {
        MantleStartTimeMode::SourceDefault => mantle_start_time(kind, mantling_height),
        MantleStartTimeMode::AutoRootHeight {
            samples,
            sampling_interval,
        } => auto_mantle_start_time(kind, mantling_height, samples, *sampling_interval)
            .unwrap_or_else(|| mantle_start_time(kind, mantling_height)),
    }
}

pub fn configured_mantle_start_time(
    settings: Option<&AlsMantleStartTimeSettings>,
    kind: MantleKind,
    mantling_height: f32,
) -> f32 {
    settings.map_or_else(
        || mantle_start_time(kind, mantling_height),
        |settings| settings.start_time_for(kind, mantling_height),
    )
}

pub fn auto_mantle_start_time(
    kind: MantleKind,
    mantling_height: f32,
    samples: &[MantleRootHeightSample],
    sampling_interval: f32,
) -> Option<f32> {
    if samples.len() < 2 || !sampling_interval.is_finite() || sampling_interval <= 0.0 {
        return None;
    }
    let mut previous = samples[0];
    if !previous.time.is_finite() || !previous.height.is_finite() {
        return None;
    }
    for sample in &samples[1..] {
        if !sample.time.is_finite() || !sample.height.is_finite() || sample.time <= previous.time {
            return None;
        }
        previous = *sample;
    }

    let sequence_length = kind.action_settings().sequence_length;
    let mut search_start_time = 0.0;
    let mut search_end_time = sequence_length;
    let search_start_height = root_height_at(samples, search_start_time)?;
    let search_end_height = root_height_at(samples, search_end_time)?;
    let target_height = (search_end_height - mantling_height).max(0.0);
    const LOCATION_TOLERANCE: f32 = 0.01;

    if (search_start_height - target_height).abs() <= LOCATION_TOLERANCE {
        return Some(search_start_time);
    }

    for _ in 0..128 {
        let time = (search_start_time + search_end_time) * 0.5;
        let height = root_height_at(samples, time)?;
        if (height - target_height).abs() <= LOCATION_TOLERANCE
            || search_end_time - search_start_time <= sampling_interval
        {
            return Some(time);
        }
        if height < target_height {
            search_start_time = time;
        } else {
            search_end_time = time;
        }
    }
    None
}

fn root_height_at(samples: &[MantleRootHeightSample], time: f32) -> Option<f32> {
    let first = *samples.first()?;
    let last = *samples.last()?;
    if time < first.time || time > last.time {
        return None;
    }
    if time <= first.time {
        return Some(first.height);
    }
    for pair in samples.windows(2) {
        let left = pair[0];
        let right = pair[1];
        if time <= right.time {
            let alpha = (time - left.time) / (right.time - left.time);
            return Some(left.height + (right.height - left.height) * alpha);
        }
    }
    Some(last.height)
}

pub fn mantle_duration_from_start_time(kind: MantleKind, start_time: f32) -> f32 {
    let settings = kind.action_settings();
    (settings.sequence_length - start_time.clamp(0.0, settings.sequence_length))
        / settings.rate_scale
}

pub fn mantle_duration(kind: MantleKind, mantling_height: f32) -> f32 {
    mantle_duration_from_start_time(kind, mantle_start_time(kind, mantling_height))
}

pub fn mantle_motion_warp_location_amount(kind: MantleKind, clip_time: f32) -> f32 {
    let settings = kind.action_settings();
    ((clip_time - settings.motion_warping_start_time)
        / (settings.motion_warping_end_time - settings.motion_warping_start_time))
        .clamp(0.0, 1.0)
}

pub fn mantle_motion_warp_rotation_amount(kind: MantleKind, clip_time: f32) -> f32 {
    let alpha = mantle_motion_warp_location_amount(kind, clip_time);
    alpha * alpha * (3.0 - 2.0 * alpha)
}

pub fn mantle_blend_in_amount(time: f32) -> f32 {
    (time / MANTLE_BLEND_IN_SECS).clamp(0.0, 1.0)
}

pub fn mantle_trace_direction(input: &MantleDetectionInput) -> Option<Vec3> {
    let velocity = Vec3::new(input.velocity.x, 0.0, input.velocity.z);
    let desired_yaw = if velocity.length_squared() > 1e-6 {
        let velocity_yaw = yaw_from_direction(velocity);
        if input.input_direction.length_squared() > 1e-6 {
            let input_yaw = yaw_from_direction(input.input_direction);
            velocity_yaw
                + wrap_angle(input_yaw - velocity_yaw)
                    .clamp(-MANTLE_MAX_REACH_ANGLE, MANTLE_MAX_REACH_ANGLE)
        } else {
            velocity_yaw
        }
    } else if input.input_direction.length_squared() > 1e-6 {
        yaw_from_direction(input.input_direction)
    } else {
        input.actor_yaw
    };
    let delta = wrap_angle(desired_yaw - input.actor_yaw);
    if delta.abs() > MANTLE_TRACE_ANGLE_LIMIT {
        return None;
    }
    let clamped_yaw =
        input.actor_yaw + delta.clamp(-MANTLE_MAX_REACH_ANGLE, MANTLE_MAX_REACH_ANGLE);
    Some(yaw_to_world_forward(clamped_yaw))
}

pub fn detect_mantle(
    spatial: &SpatialQuery,
    excluded: Entity,
    input: MantleDetectionInput,
    velocities: &Query<&LinearVelocity, Without<crate::character::AlsCharacter>>,
    target_permissions: &Query<
        Option<&AlsMantleTargetPermission>,
        Without<crate::character::AlsCharacter>,
    >,
) -> MantleDetection {
    let mut trace = MantleProbeTrace::default();
    if input.locomotion == LocomotionState::Ragdolling {
        return MantleDetection {
            intent: None,
            trace,
        };
    }
    let Some(forward) = mantle_trace_direction(&input) else {
        return MantleDetection {
            intent: None,
            trace,
        };
    };
    let Ok(forward_direction) = Dir3::new(forward) else {
        return MantleDetection {
            intent: None,
            trace,
        };
    };
    let settings = mantle_trace_settings(input.locomotion);
    let filter = SpatialQueryFilter::from_excluded_entities([excluded]);
    let capsule_scale = input.capsule_scale.abs().max(1e-4);
    let capsule_bottom = input.origin - Vec3::Y * input.capsule_half_height;
    let trace_radius = (input.capsule_radius - MANTLE_TRACE_CAPSULE_SHRINK).max(0.001);
    let ledge_height_delta =
        (settings.maximum_ledge_height - settings.minimum_ledge_height) * capsule_scale;
    let forward_start = capsule_bottom - forward * input.capsule_radius
        + Vec3::Y
            * ((settings.minimum_ledge_height + settings.maximum_ledge_height)
                * 0.5
                * capsule_scale
                - MANTLE_MAX_FLOOR_DISTANCE);
    let forward_distance = input.capsule_radius
        + (settings.reach_distance + MANTLE_TRACE_CAPSULE_SHRINK) * capsule_scale;
    let forward_end = forward_start + forward * forward_distance;
    trace.forward_start = Some(forward_start);
    trace.forward_end = Some(forward_end);
    let forward_shape = als_character_capsule(ledge_height_delta * 0.5, trace_radius);
    let forward_config = ShapeCastConfig::from_max_distance(forward_distance);
    let Some(forward_hit) = spatial.cast_shape(
        &forward_shape,
        forward_start,
        Quat::IDENTITY,
        forward_direction,
        &forward_config,
        &filter,
    ) else {
        return MantleDetection {
            intent: None,
            trace,
        };
    };
    trace.forward_hit = Some(forward_hit.point1);
    let target_allows_step_up = target_permissions
        .get(forward_hit.entity)
        .ok()
        .flatten()
        .map(|permission| permission.allows_step_up)
        .unwrap_or(true);
    if forward_hit.normal1.y >= MANTLE_WALKABLE_FLOOR_COS
        || !target_allows_step_up
        || velocities
            .get(forward_hit.entity)
            .ok()
            .is_some_and(|velocity| velocity.0.length() > MANTLE_TARGET_SPEED_THRESHOLD)
    {
        return MantleDetection {
            intent: None,
            trace,
        };
    }
    let target_direction =
        Vec3::new(-forward_hit.normal1.x, 0.0, -forward_hit.normal1.z).normalize_or_zero();
    if target_direction.length_squared() <= 1e-6 {
        return MantleDetection {
            intent: None,
            trace,
        };
    }
    let downward_start = Vec3::new(
        forward_hit.point1.x + target_direction.x * settings.target_location_offset * capsule_scale,
        capsule_bottom.y + ledge_height_delta + 2.5 * trace_radius + MANTLE_MIN_FLOOR_DISTANCE,
        forward_hit.point1.z + target_direction.z * settings.target_location_offset * capsule_scale,
    );
    let downward_end = Vec3::new(
        downward_start.x,
        capsule_bottom.y + settings.minimum_ledge_height * capsule_scale + trace_radius
            - MANTLE_MAX_FLOOR_DISTANCE,
        downward_start.z,
    );
    let downward_distance = (downward_start.y - downward_end.y).max(0.0);
    trace.downward_start = Some(downward_start);
    trace.downward_end = Some(downward_end);
    let downward_shape = Collider::sphere(trace_radius);
    let downward_config = ShapeCastConfig::from_max_distance(downward_distance);
    let Some(downward_hit) = spatial.cast_shape(
        &downward_shape,
        downward_start,
        Quat::IDENTITY,
        Dir3::NEG_Y,
        &downward_config,
        &filter,
    ) else {
        return MantleDetection {
            intent: None,
            trace,
        };
    };
    trace.downward_hit = Some(downward_hit.point1);
    let downward_location = downward_start - Vec3::Y * downward_hit.distance;
    let approximate_slope_normal = (downward_location - downward_hit.point1).normalize_or_zero();
    if downward_hit.normal1.y < MANTLE_SLOPE_ANGLE_COS
        || approximate_slope_normal.y < MANTLE_SLOPE_ANGLE_COS
    {
        return MantleDetection {
            intent: None,
            trace,
        };
    }
    let target_ground = downward_hit.point1.y + MANTLE_MIN_FLOOR_DISTANCE;
    let target = Vec3::new(
        downward_location.x,
        target_ground + input.capsule_half_height,
        downward_location.z,
    );
    trace.target_capsule_center = Some(target);
    let target_capsule = als_character_capsule(input.capsule_half_height, input.capsule_radius);
    if !spatial
        .shape_intersections(&target_capsule, target, Quat::IDENTITY, &filter)
        .is_empty()
    {
        return MantleDetection {
            intent: None,
            trace,
        };
    }
    let start_location = Vec3::new(
        forward_hit.point1.x - target_direction.x * settings.start_location_offset * capsule_scale,
        (downward_location.y + downward_end.y) * 0.5,
        forward_hit.point1.z - target_direction.z * settings.start_location_offset * capsule_scale,
    );
    let start_half_height = (downward_location.y - downward_end.y) * 0.5 + trace_radius;
    let start_capsule = als_character_capsule(start_half_height.max(trace_radius), trace_radius);
    if !spatial
        .shape_intersections(&start_capsule, start_location, Quat::IDENTITY, &filter)
        .is_empty()
    {
        return MantleDetection {
            intent: None,
            trace,
        };
    }
    let mantling_height = (target_ground - capsule_bottom.y) / capsule_scale;
    let kind = select_mantle_kind(input.locomotion, input.overlay, mantling_height);
    MantleDetection {
        intent: Some(MantleIntent {
            kind,
            target,
            mantling_height,
            yaw_target: yaw_from_direction(target_direction),
            target_entity: forward_hit.entity,
        }),
        trace,
    }
}

pub fn mantle_weight_at(elapsed: f32, duration: f32) -> f32 {
    let remaining = (duration - elapsed).max(0.0);
    (elapsed / MANTLE_BLEND_IN_SECS)
        .clamp(0.0, 1.0)
        .min((remaining / MANTLE_BLEND_OUT_SECS).clamp(0.0, 1.0))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsMantleTargetBase {
    pub entity: Entity,
    pub translation: Vec3,
    pub rotation: Quat,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsMantleState {
    pub active: bool,
    pub kind: MantleKind,
    pub time: f32,
    pub duration: f32,
    pub start: Vec3,
    pub target: Vec3,
    pub cooldown: f32,
    pub start_offset: f32,
    pub rate_scale: f32,
    pub yaw_start: f32,
    pub yaw_target: f32,
    pub target_base: Option<AlsMantleTargetBase>,
    pub start_location_base: Vec3,
    pub start_rotation_base: Quat,
    pub target_location_base: Vec3,
    pub target_rotation_base: Quat,
    pub target_invalid: bool,
    pub root_motion_translation: Vec3,
    pub root_motion_rotation: Quat,
}

impl Default for AlsMantleState {
    fn default() -> Self {
        Self {
            active: false,
            kind: MantleKind::High,
            time: 0.0,
            duration: mantle_duration(MantleKind::LowRight, 0.5),
            start: Vec3::ZERO,
            target: Vec3::ZERO,
            cooldown: 0.0,
            start_offset: 0.0,
            rate_scale: 1.0,
            yaw_start: 0.0,
            yaw_target: 0.0,
            target_base: None,
            start_location_base: Vec3::ZERO,
            start_rotation_base: Quat::IDENTITY,
            target_location_base: Vec3::ZERO,
            target_rotation_base: Quat::IDENTITY,
            target_invalid: false,
            root_motion_translation: Vec3::ZERO,
            root_motion_rotation: Quat::IDENTITY,
        }
    }
}

impl AlsMantleState {
    pub fn begin(
        &mut self,
        kind: MantleKind,
        start: Vec3,
        target: Vec3,
        mantling_height: f32,
        yaw_start: f32,
        yaw_target: f32,
    ) {
        self.begin_with_start_time(
            kind,
            start,
            target,
            mantle_start_time(kind, mantling_height),
            yaw_start,
            yaw_target,
        );
    }

    pub fn begin_with_start_time(
        &mut self,
        kind: MantleKind,
        start: Vec3,
        target: Vec3,
        start_time: f32,
        yaw_start: f32,
        yaw_target: f32,
    ) {
        let settings = kind.action_settings();
        let start_time = start_time.clamp(0.0, settings.sequence_length);
        self.active = true;
        self.kind = kind;
        self.time = 0.0;
        self.duration = mantle_duration_from_start_time(kind, start_time);
        self.start = start;
        self.target = target;
        self.cooldown = 0.0;
        self.start_offset = start_time;
        self.rate_scale = settings.rate_scale;
        self.yaw_start = yaw_start;
        self.yaw_target = yaw_target;
        self.target_base = None;
        self.start_location_base = Vec3::ZERO;
        self.start_rotation_base = Quat::IDENTITY;
        self.target_location_base = Vec3::ZERO;
        self.target_rotation_base = Quat::IDENTITY;
        self.target_invalid = false;
        self.root_motion_translation = Vec3::ZERO;
        self.root_motion_rotation = Quat::IDENTITY;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn begin_with_target_base(
        &mut self,
        kind: MantleKind,
        start: Vec3,
        target: Vec3,
        mantling_height: f32,
        yaw_start: f32,
        yaw_target: f32,
        target_base: AlsMantleTargetBase,
    ) {
        self.begin(kind, start, target, mantling_height, yaw_start, yaw_target);
        self.capture_target_base(target_base);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn begin_with_target_base_and_start_time(
        &mut self,
        kind: MantleKind,
        start: Vec3,
        target: Vec3,
        start_time: f32,
        yaw_start: f32,
        yaw_target: f32,
        target_base: AlsMantleTargetBase,
    ) {
        self.begin_with_start_time(kind, start, target, start_time, yaw_start, yaw_target);
        self.capture_target_base(target_base);
    }

    pub fn capture_target_base(&mut self, target_base: AlsMantleTargetBase) {
        let inverse = target_base.rotation.conjugate();
        self.start_location_base = inverse * (self.start - target_base.translation);
        self.start_rotation_base = (inverse * Quat::from_rotation_y(self.yaw_start)).normalize();
        self.target_location_base = inverse * (self.target - target_base.translation);
        self.target_rotation_base = (inverse * Quat::from_rotation_y(self.yaw_target)).normalize();
        self.target_base = Some(target_base);
        self.target_invalid = false;
    }

    pub fn refresh_target_base(&mut self, latest_base: Option<AlsMantleTargetBase>) -> bool {
        let Some(saved_base) = self.target_base else {
            return true;
        };
        let Some(target_base) = latest_base.filter(|base| base.entity == saved_base.entity) else {
            self.active = false;
            self.target_invalid = true;
            return false;
        };
        self.start = target_base.translation + target_base.rotation * self.start_location_base;
        self.target = target_base.translation + target_base.rotation * self.target_location_base;
        let start_rotation = target_base.rotation * self.start_rotation_base;
        let target_rotation = target_base.rotation * self.target_rotation_base;
        self.yaw_start = yaw_from_direction(start_rotation * Vec3::NEG_Z);
        self.yaw_target = yaw_from_direction(target_rotation * Vec3::NEG_Z);
        self.target_base = Some(target_base);
        self.target_invalid = false;
        true
    }

    pub fn clip_time(&self) -> f32 {
        self.clip_time_at(self.time)
    }

    pub fn clip_time_at(&self, time: f32) -> f32 {
        let settings = self.kind.action_settings();
        (self.start_offset + time * self.rate_scale).min(settings.sequence_length)
    }

    pub fn action_end(&self) -> f32 {
        self.duration
    }

    pub fn location_warp_amount(&self) -> f32 {
        self.location_warp_amount_at(self.time)
    }

    pub fn rotation_warp_amount(&self) -> f32 {
        self.rotation_warp_amount_at(self.time)
    }

    pub fn location_warp_amount_at(&self, time: f32) -> f32 {
        mantle_blend_in_amount(time)
            * mantle_motion_warp_location_amount(self.kind, self.clip_time_at(time))
    }

    pub fn rotation_warp_amount_at(&self, time: f32) -> f32 {
        mantle_blend_in_amount(time)
            * mantle_motion_warp_rotation_amount(self.kind, self.clip_time_at(time))
    }

    pub fn warp_transform_at(&self, time: f32) -> Transform {
        let location = self
            .start
            .lerp(self.target, self.location_warp_amount_at(time));
        let rotation = self
            .start_rotation()
            .lerp(self.target_rotation(), self.rotation_warp_amount_at(time))
            .normalize();
        Transform {
            translation: location,
            rotation,
            ..Default::default()
        }
    }

    pub fn target_actor_transform_at(&self, time: f32) -> Transform {
        let warp = self.warp_transform_at(time);
        Transform {
            translation: warp.translation + warp.rotation * self.root_motion_translation,
            rotation: (warp.rotation * self.root_motion_rotation).normalize(),
            ..Default::default()
        }
    }

    pub fn accumulate_root_motion(&mut self, translation_delta: Vec3, rotation_delta: Quat) {
        let rotation_delta = rotation_delta.normalize();
        self.root_motion_translation += self.root_motion_rotation * translation_delta;
        self.root_motion_rotation = (self.root_motion_rotation * rotation_delta).normalize();
    }

    fn start_rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw_start)
    }

    fn target_rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw_target)
    }

    pub fn tick(
        &mut self,
        _cmd: &AlsInputCommand,
        _loco: &LocomotionState,
        _stance: &Stance,
        dt: f32,
    ) {
        if !self.active {
            return;
        }
        self.time += dt;
        if self.time >= self.action_end() {
            self.active = false;
            self.time = self.action_end();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_settings_match_grounded_and_in_air_source_values() {
        assert_eq!(MANTLE_GROUNDED_TRACE.minimum_ledge_height, 0.5);
        assert_eq!(MANTLE_GROUNDED_TRACE.maximum_ledge_height, 2.25);
        assert_eq!(MANTLE_GROUNDED_TRACE.reach_distance, 0.75);
        assert_eq!(MANTLE_IN_AIR_TRACE.maximum_ledge_height, 1.5);
        assert_eq!(MANTLE_IN_AIR_TRACE.reach_distance, 0.7);
        assert_eq!(MANTLE_WALKABLE_FLOOR_COS, 0.71);
        assert!((MANTLE_SLOPE_ANGLE_COS - 35.0_f32.to_radians().cos()).abs() < 1e-6);
    }

    #[test]
    fn source_type_selection_uses_strict_high_threshold_and_overlay_route() {
        assert_eq!(
            select_mantle_kind(LocomotionState::Grounded, OverlayState::Default, 1.25),
            MantleKind::LowRight
        );
        assert_eq!(
            select_mantle_kind(LocomotionState::Grounded, OverlayState::Default, 1.250_01),
            MantleKind::High
        );
        assert_eq!(
            select_mantle_kind(LocomotionState::Air, OverlayState::Injured, 0.6),
            MantleKind::InAir
        );
        assert_eq!(
            select_low_mantle_kind(OverlayState::Injured),
            MantleKind::LowLeft
        );
        assert_eq!(
            select_low_mantle_kind(OverlayState::HandsTied),
            MantleKind::LowBothHands
        );
        assert_eq!(
            select_low_mantle_kind(OverlayState::Box),
            MantleKind::LowBox
        );
    }

    #[test]
    fn source_start_times_and_rate_scaled_durations_are_preserved() {
        assert!((mantle_start_time(MantleKind::High, 1.25) - 0.6).abs() < 1e-6);
        assert!(mantle_start_time(MantleKind::High, 2.0).abs() < 1e-6);
        assert!((mantle_start_time(MantleKind::InAir, 1.25) - 0.8).abs() < 1e-6);
        assert!((mantle_start_time(MantleKind::LowRight, 0.5) - 0.5).abs() < 1e-6);
        assert!((mantle_start_time(MantleKind::LowRight, 1.0)).abs() < 1e-6);
        assert!((mantle_duration(MantleKind::High, 2.0) - 2.666_666_7 / 1.2).abs() < 1e-6);
        assert!(
            (mantle_duration(MantleKind::InAir, 1.25) - (2.666_666_7 - 0.8) / 1.2).abs() < 1e-6
        );
    }

    #[test]
    fn auto_root_height_start_time_uses_source_binary_search_policy() {
        let settings = AlsMantleStartTimeSettings {
            low: MantleStartTimeMode::AutoRootHeight {
                samples: vec![
                    MantleRootHeightSample {
                        time: 0.0,
                        height: 0.0,
                    },
                    MantleRootHeightSample {
                        time: 0.75,
                        height: 0.75,
                    },
                    MantleRootHeightSample {
                        time: 1.5,
                        height: 1.5,
                    },
                ],
                sampling_interval: 1.0 / 60.0,
            },
            ..default()
        };
        let height = 0.75;
        let manual_start = mantle_start_time(MantleKind::LowRight, height);
        let auto_start = settings.start_time_for(MantleKind::LowRight, height);
        let mut state = AlsMantleState::default();
        state.begin_with_start_time(
            MantleKind::LowRight,
            Vec3::ZERO,
            Vec3::ONE,
            auto_start,
            0.0,
            0.0,
        );
        let control = AlsMantleControlSettings::default();
        let disabled_control = AlsMantleControlSettings {
            allow_mantling: false,
            auto_start_in_air: false,
        };
        let permission = AlsMantleTargetPermission {
            allows_step_up: false,
        };
        let (scaled_half_height, scaled_radius, scaled_vertical) =
            scaled_mantle_capsule_dimensions(0.9, 0.3, Vec3::new(1.5, 2.0, 0.5));
        let passed = mantle_start_allowed(control, false)
            && auto_mantle_start_allowed(control, false, LocomotionState::Air)
            && !mantle_start_allowed(disabled_control, false)
            && !auto_mantle_start_allowed(disabled_control, false, LocomotionState::Air)
            && !permission.allows_step_up
            && (auto_start - 0.75).abs() <= 0.02
            && (auto_start - manual_start).abs() > 0.3
            && (state.start_offset - auto_start).abs() <= 1e-6
            && (scaled_half_height - 1.8).abs() <= 1e-6
            && (scaled_radius - 0.45).abs() <= 1e-6
            && (scaled_vertical - 2.0).abs() <= 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.49][VERDICT] manual_start={manual_start:.6} auto_start={auto_start:.6} state_start={:.6} permission={} scaled=({scaled_half_height:.6},{scaled_radius:.6},{scaled_vertical:.6}) -> {verdict}",
            state.start_offset, permission.allows_step_up,
        );
        assert!(passed);
    }

    #[test]
    fn motion_warp_blend_modes_match_source_windows() {
        assert!(mantle_motion_warp_location_amount(MantleKind::High, 0.3) <= 1e-6);
        assert!((mantle_motion_warp_location_amount(MantleKind::High, 0.6) - 1.0).abs() < 1e-6);
        assert!((mantle_motion_warp_rotation_amount(MantleKind::High, 0.45) - 0.5).abs() < 1e-6);
        assert!((mantle_motion_warp_location_amount(MantleKind::LowRight, 0.7) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn forward_trace_direction_limits_input_to_source_angles() {
        let input = MantleDetectionInput {
            actor_yaw: 0.0,
            input_direction: Vec3::X,
            ..default()
        };
        let direction = mantle_trace_direction(&input).unwrap();
        assert!((yaw_from_direction(direction) + MANTLE_MAX_REACH_ANGLE).abs() < 1e-6);
        let behind = MantleDetectionInput {
            actor_yaw: 0.0,
            input_direction: Vec3::Z,
            ..default()
        };
        assert!(mantle_trace_direction(&behind).is_none());
    }

    #[test]
    fn state_drives_montage_time_with_rate_scale_without_custom_cooldown() {
        let mut state = AlsMantleState::default();
        state.begin(MantleKind::High, Vec3::ZERO, Vec3::ONE, 1.25, 0.0, 1.0);
        assert!((state.clip_time() - 0.6).abs() < 1e-6);
        state.tick(
            &AlsInputCommand::default(),
            &LocomotionState::Grounded,
            &Stance::Standing,
            0.25,
        );
        assert!((state.clip_time() - 0.9).abs() < 1e-6);
        assert_eq!(state.cooldown, 0.0);
    }

    #[test]
    fn mantle_completion_has_no_fixed_target_lock_phase() {
        let mut state = AlsMantleState::default();
        state.begin(MantleKind::LowRight, Vec3::ZERO, Vec3::ONE, 0.5, 0.0, 0.0);
        let duration = state.action_end();
        state.tick(
            &AlsInputCommand::default(),
            &LocomotionState::Grounded,
            &Stance::Standing,
            duration + 0.1,
        );
        let completion_time = state.time;
        state.tick(
            &AlsInputCommand::default(),
            &LocomotionState::Grounded,
            &Stance::Standing,
            1.0,
        );
        let released = !state.active
            && (completion_time - duration).abs() < 1e-6
            && (state.time - duration).abs() < 1e-6;
        let verdict = if released { "PASS" } else { "FAIL" };
        println!(
            "[BF102.47][VERDICT] completion_time={completion_time:.6} duration={duration:.6} released={released} -> {verdict}"
        );
        assert!(released);
    }

    #[test]
    fn mantle_root_motion_composes_full_transform_in_warp_space() {
        let mut state = AlsMantleState::default();
        state.begin(
            MantleKind::High,
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(2.0, 1.0, -3.0),
            1.25,
            0.0,
            std::f32::consts::FRAC_PI_2,
        );
        let root_translation = Vec3::new(0.15, 0.25, -0.35);
        let root_rotation = Quat::from_rotation_y(20.0_f32.to_radians());
        state.accumulate_root_motion(root_translation, root_rotation);
        let actor = state.target_actor_transform_at(0.6);
        let target_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let expected_translation = state.target + target_rotation * root_translation;
        let expected_rotation = target_rotation * root_rotation;
        let translation_error = actor.translation.distance(expected_translation);
        let rotation_alignment = actor.rotation.dot(expected_rotation).abs();
        let legacy_translation =
            state.target + Vec3::new(0.0, root_translation.y, -root_translation.z);
        let legacy_error = actor.translation.distance(legacy_translation);
        let passed =
            translation_error < 1e-6 && rotation_alignment > 0.999_99 && legacy_error > 0.1;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.46][VERDICT] translation_error={translation_error:.6} rotation_alignment={rotation_alignment:.6} legacy_error={legacy_error:.6} -> {verdict}"
        );
        assert!(passed);
    }

    #[test]
    fn mantle_target_base_updates_start_target_and_yaw() {
        let target = Entity::from_raw_u32(1).unwrap();
        let initial_base = AlsMantleTargetBase {
            entity: target,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        };
        let mut state = AlsMantleState::default();
        state.begin_with_target_base(
            MantleKind::High,
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            1.25,
            0.0,
            0.0,
            initial_base,
        );
        let moved_base = AlsMantleTargetBase {
            entity: target,
            translation: Vec3::new(3.0, 0.0, -2.0),
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        };
        assert!(state.refresh_target_base(Some(moved_base)));
        assert!(
            state
                .start
                .distance(moved_base.translation + moved_base.rotation * Vec3::new(-1.0, 0.0, 0.0))
                < 1e-5
        );
        assert!(
            state
                .target
                .distance(moved_base.translation + moved_base.rotation * Vec3::new(2.0, 0.0, 0.0))
                < 1e-5
        );
        assert!((state.yaw_target - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert_eq!(state.target_base.map(|base| base.entity), Some(target));
    }

    #[test]
    fn mantle_target_base_loss_cancels_immediately() {
        let base = AlsMantleTargetBase {
            entity: Entity::from_raw_u32(1).unwrap(),
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        };
        let mut state = AlsMantleState::default();
        state.begin_with_target_base(
            MantleKind::High,
            Vec3::ZERO,
            Vec3::ONE,
            1.25,
            0.0,
            0.0,
            base,
        );
        assert!(!state.refresh_target_base(None));
        assert!(!state.active);
        assert!(state.target_invalid);
        assert_eq!(
            state.target_base.map(|value| value.entity),
            Some(base.entity)
        );
    }
}
