use avian3d::prelude::{
    Collider, ColliderOf, LinearVelocity, RigidBody, ShapeCastConfig, SpatialQuery, SpatialQueryFilter,
};
use bevy::prelude::*;
use crate::als::dynamics::{AlsComponentVelocity, component_velocity};
use std::{collections::HashMap, sync::LazyLock};

use crate::als::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_CAPSULE_RADIUS, ALS_CHARACTER_FLOOR_CLEARANCE,
    als_character_capsule,
};
use crate::als::input::AlsInputCommand;
use crate::als::orientation::{world_forward_to_yaw, yaw_to_world_forward};
use crate::als::state::{LocomotionState, OverlayState, Stance};

pub const MANTLE_HIGH_HEIGHT_THRESHOLD: f32 = 1.25;
pub const MANTLE_TRACE_ANGLE_LIMIT: f32 = 110.0_f32.to_radians();
pub const MANTLE_MAX_REACH_ANGLE: f32 = 50.0_f32.to_radians();
pub const MANTLE_WALKABLE_FLOOR_COS: f32 = 0.71;
pub const MANTLE_SLOPE_ANGLE_COS: f32 = 0.819_152_06;
pub const MANTLE_TARGET_SPEED_THRESHOLD: f32 = 0.1;
pub const MANTLE_TRACE_CAPSULE_SHRINK: f32 = 0.01;
pub const MANTLE_MIN_FLOOR_DISTANCE: f32 = ALS_CHARACTER_FLOOR_CLEARANCE;
pub const MANTLE_MAX_FLOOR_DISTANCE: f32 = 0.024;

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

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
pub struct MantleActionSettings {
    pub sequence_length: f32,
    pub rate_scale: f32,
    pub montage_index: usize,
    pub blend_in_seconds: f32,
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
    pub draw_failed_grounded_traces: bool,
    pub draw_failed_in_air_traces: bool,
}

impl Default for AlsMantleControlSettings {
    fn default() -> Self {
        Self {
            allow_mantling: true,
            auto_start_in_air: true,
            draw_failed_grounded_traces: MANTLE_SETTINGS.draw_failed_grounded_traces,
            draw_failed_in_air_traces: MANTLE_SETTINGS.draw_failed_in_air_traces,
        }
    }
}

impl AlsMantleControlSettings {
    pub fn draw_failed_traces(self, locomotion: LocomotionState) -> bool {
        if locomotion == LocomotionState::Air { self.draw_failed_in_air_traces } else { self.draw_failed_grounded_traces }
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

pub const MANTLE_SOURCE_DATA: &str =
    include_str!("../../../../assets/character/als_mantle_settings.ron");

#[derive(serde::Deserialize)]
struct MantleSourceSettings {
    actions: HashMap<MantleKind, MantleActionSettings>,
    low_kinds: HashMap<OverlayState, MantleKind>,
    trace_layer: crate::als::character::AlsCollisionLayer,
    draw_failed_grounded_traces: bool,
    draw_failed_in_air_traces: bool,
}

static MANTLE_SETTINGS: LazyLock<MantleSourceSettings> = LazyLock::new(|| {
    ron::from_str(MANTLE_SOURCE_DATA).expect("generated mantle settings are valid")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
pub enum MantleKind {
    #[default]
    High,
    InAir,
    Low,
    LowLeft,
    LowRight,
    LowBothHands,
    LowBox,
}

impl MantleKind {
    pub fn action_settings(self) -> MantleActionSettings {
        MANTLE_SETTINGS.actions[&self]
    }

    pub fn is_high(self) -> bool {
        matches!(self, Self::High | Self::InAir)
    }
}

impl AlsMantleStartTimeSettings {
    pub fn mode_for(&self, kind: MantleKind) -> &MantleStartTimeMode {
        match kind {
            MantleKind::High => &self.high,
            MantleKind::InAir => &self.in_air,
            MantleKind::Low
            | MantleKind::LowLeft
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MantleProbeResult {
    #[default]
    ForwardRejected,
    DownwardRejected,
    TargetBlocked,
    StartBlocked,
    Success,
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
    pub trace_radius: f32,
    pub forward_half_height: f32,
    pub forward_hit_center: Option<Vec3>,
    pub downward_hit_center: Option<Vec3>,
    pub target_half_height: f32,
    pub target_radius: f32,
    pub start_capsule_center: Option<Vec3>,
    pub start_half_height: f32,
    pub result: MantleProbeResult,
    pub draw_failed_traces: bool,
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
    (
        base_half_height * world_scale.y,
        base_radius * world_scale.x.min(world_scale.z),
        world_scale.y,
    )
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
        _ => MANTLE_GROUNDED_TRACE
    }
}

pub fn select_low_mantle_kind(overlay: OverlayState) -> MantleKind {
    MANTLE_SETTINGS
        .low_kinds
        .get(&overlay)
        .copied()
        .unwrap_or(MANTLE_SETTINGS.low_kinds[&OverlayState::Default])
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

pub fn mantle_blend_in_amount(kind: MantleKind, time: f32) -> f32 {
    let duration = kind.action_settings().blend_in_seconds;
    let alpha = if duration > 1e-8 {
        (time / duration).clamp(0.0, 1.0)
    } else {
        1.0
    };
    alpha * alpha * (3.0 - 2.0 * alpha)
}

pub fn mantle_trace_direction(input: &MantleDetectionInput) -> Option<Vec3> {
    let velocity = Vec3::new(input.velocity.x, 0.0, input.velocity.z);
    let desired_yaw = if velocity.length() >= 0.01 {
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
    velocities: &Query<(&RigidBody, &LinearVelocity, Option<&AlsComponentVelocity>), Without<crate::als::character::AlsCharacter>>,
    target_permissions: &Query<
        Option<&AlsMantleTargetPermission>,
        Without<crate::als::character::AlsCharacter>,
    >,
    colliders: &Query<&ColliderOf>,
) -> MantleDetection {
    let mut trace = MantleProbeTrace::default();
    if !matches!(
        input.locomotion,
        LocomotionState::Grounded | LocomotionState::Air
    ) {
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
    let filter = SpatialQueryFilter::from_excluded_entities([excluded]).with_mask(MANTLE_SETTINGS.trace_layer);
    let capsule_scale = input.capsule_scale.abs().max(1e-4);
    let capsule_bottom = input.origin - Vec3::Y * input.capsule_half_height;
    let trace_radius = (input.capsule_radius - MANTLE_TRACE_CAPSULE_SHRINK).max(0.001);
    let ledge_height_delta =
        (settings.maximum_ledge_height - settings.minimum_ledge_height) * capsule_scale;
    trace.trace_radius = trace_radius;
    trace.forward_half_height = ledge_height_delta * 0.5;
    trace.target_half_height = input.capsule_half_height;
    trace.target_radius = input.capsule_radius;
    trace.draw_failed_traces = AlsMantleControlSettings::default().draw_failed_traces(input.locomotion);
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
    trace.forward_hit_center = Some(forward_start + forward * forward_hit.distance);
    let body = colliders.get(forward_hit.entity).map_or(forward_hit.entity, |collider| collider.body);
    let target_allows_step_up = target_permissions
        .get(forward_hit.entity)
        .ok()
        .flatten()
        .or_else(|| target_permissions.get(body).ok().flatten())
        .map(|permission| permission.allows_step_up)
        .unwrap_or(true);
    if (forward_hit.distance == 0.0 && (forward_hit.point2 - forward_hit.point1).dot(forward_hit.normal1) < 0.0)
        || forward_hit.normal1.y >= MANTLE_WALKABLE_FLOOR_COS
        || !target_allows_step_up
        || velocities
            .get(body)
            .ok()
            .is_some_and(|(kind, physical, component)| component_velocity(kind, physical, component).length() > MANTLE_TARGET_SPEED_THRESHOLD)
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
    trace.result = MantleProbeResult::DownwardRejected;
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
    trace.downward_hit_center = Some(downward_location);
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
    trace.result = MantleProbeResult::TargetBlocked;
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
    trace.start_capsule_center = Some(start_location);
    trace.start_half_height = start_half_height.max(trace_radius);
    trace.result = MantleProbeResult::StartBlocked;
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
    trace.result = MantleProbeResult::Success;
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsMantleTargetBase {
    pub entity: Entity,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub relative_location: bool,
}

impl AlsMantleTargetBase {
    fn to_world(self, local: Transform) -> Transform {
        if !self.relative_location {
            return local;
        }
        Transform {
            translation: self.translation + self.rotation * (self.scale * local.translation),
            rotation: (self.rotation * local.rotation).normalize(),
            ..default()
        }
    }

    fn to_local(self, world: Transform) -> Transform {
        if !self.relative_location {
            return world;
        }
        let reciprocal = Vec3::from_array(self.scale.to_array().map(|value| {
            if value.abs() <= 1e-8 {
                0.0
            } else {
                value.recip()
            }
        }));
        Transform {
            translation: reciprocal
                * (self.rotation.conjugate() * (world.translation - self.translation)),
            rotation: (self.rotation.conjugate() * world.rotation).normalize(),
            ..default()
        }
    }
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
    pub warp_start: Transform,
    pub warp_target: Transform,
    pub target_invalid: bool,
    pub root_motion_ready: bool,
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
            warp_start: Transform::IDENTITY,
            warp_target: Transform::IDENTITY,
            target_invalid: false,
            root_motion_ready: false,
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
        self.warp_start =
            Transform::from_translation(start).with_rotation(Quat::from_rotation_y(yaw_start));
        self.warp_target =
            Transform::from_translation(target).with_rotation(Quat::from_rotation_y(yaw_target));
        self.target_invalid = false;
        self.root_motion_ready = false;
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
        self.warp_start = target_base.to_local(self.warp_start);
        self.warp_target = target_base.to_local(self.warp_target);
        self.target_base = Some(target_base);
        self.target_invalid = false;
    }

    pub fn prepare_root_motion(&mut self, start_root: Transform, end_root: Transform) {
        for (warp, root) in [
            (&mut self.warp_start, start_root),
            (&mut self.warp_target, end_root),
        ] {
            let world = self.target_base.map_or(*warp, |base| base.to_world(*warp));
            let rotation = (world.rotation * root.rotation.conjugate()).normalize();
            let world = Transform {
                translation: world.translation - rotation * root.translation,
                rotation,
                ..default()
            };
            *warp = self.target_base.map_or(world, |base| base.to_local(world));
        }
        self.root_motion_ready = true;
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
        let start = target_base.to_world(
            saved_base.to_local(
                Transform::from_translation(self.start)
                    .with_rotation(Quat::from_rotation_y(self.yaw_start)),
            ),
        );
        let target = target_base.to_world(
            saved_base.to_local(
                Transform::from_translation(self.target)
                    .with_rotation(Quat::from_rotation_y(self.yaw_target)),
            ),
        );
        self.start = start.translation;
        self.target = target.translation;
        let start_rotation = start.rotation;
        let target_rotation = target.rotation;
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
        mantle_blend_in_amount(self.kind, time)
            * mantle_motion_warp_location_amount(self.kind, self.clip_time_at(time))
    }

    pub fn rotation_warp_amount_at(&self, time: f32) -> f32 {
        mantle_blend_in_amount(self.kind, time)
            * mantle_motion_warp_rotation_amount(self.kind, self.clip_time_at(time))
    }

    pub fn warp_transform_at(&self, time: f32) -> Transform {
        let start = self
            .target_base
            .map_or(self.warp_start, |base| base.to_world(self.warp_start));
        let target = self
            .target_base
            .map_or(self.warp_target, |base| base.to_world(self.warp_target));
        let location = start
            .translation
            .lerp(target.translation, self.location_warp_amount_at(time));
        let rotation = start
            .rotation
            .lerp(target.rotation, self.rotation_warp_amount_at(time))
            .normalize();
        let twist = Quat::from_xyzw(0.0, rotation.y, 0.0, rotation.w);
        let rotation = if twist.length_squared() >= 1e-8 {
            twist.normalize()
        } else {
            Quat::IDENTITY
        };
        Transform {
            translation: location,
            rotation,
            ..Default::default()
        }
    }

    pub fn target_actor_transform_at(&self, time: f32, root: Transform) -> Transform {
        let warp = self.warp_transform_at(time);
        Transform {
            translation: warp.translation + warp.rotation * root.translation,
            rotation: (warp.rotation * root.rotation).normalize(),
            ..Default::default()
        }
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
    use avian3d::prelude::{CollisionLayers, LayerMask};
    use crate::als::character::AlsCollisionLayer;

    fn debug_probe_app(rotation: Quat) -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate)));
        app.finish();
        app.cleanup();
        let actor = app.world_mut().spawn((crate::als::character::AlsCharacter, Transform::from_xyz(0.0, 0.95, 0.0))).id();
        let ledge = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.4, 0.8, 1.2),
            CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL),
            Transform::from_xyz(0.0, 0.4, -1.4).with_rotation(rotation))).id();
        app.update();
        (app, actor, ledge)
    }

    fn debug_probe(app: &mut App, actor: Entity, locomotion: LocomotionState) -> MantleDetection {
        use bevy::ecs::system::RunSystemOnce;
        app.world_mut().run_system_once_with(|In((actor, locomotion)): In<(Entity, LocomotionState)>, spatial: SpatialQuery,
            velocities: Query<(&RigidBody, &LinearVelocity, Option<&AlsComponentVelocity>), Without<crate::als::character::AlsCharacter>>,
            permissions: Query<Option<&AlsMantleTargetPermission>, Without<crate::als::character::AlsCharacter>>, colliders: Query<&ColliderOf>,
            actors: Query<&Transform, With<crate::als::character::AlsCharacter>>| {
            detect_mantle(&spatial, actor, MantleDetectionInput { origin: actors.get(actor).unwrap().translation, input_direction: Vec3::NEG_Z,
                locomotion, ..default() }, &velocities, &permissions, &colliders)
        }, (actor, locomotion)).unwrap()
    }

    #[test]
    fn mantle_rejects_an_initially_penetrating_forward_sweep() {
        let (mut app, actor, _) = debug_probe_app(Quat::IDENTITY);
        for height in [0.4, 0.6, 0.95] {
            app.world_mut().get_mut::<Transform>(actor).unwrap().translation = Vec3::new(0.0, height, -1.3);
            let detection = debug_probe(&mut app, actor, LocomotionState::Grounded);
            assert!(detection.trace.forward_hit.is_some());
            assert_eq!(detection.trace.forward_hit_center, detection.trace.forward_start);
            assert_eq!(detection.trace.result, MantleProbeResult::ForwardRejected, "height={height}");
            assert!(detection.intent.is_none());
        }
    }

    #[test]
    fn mantle_probe_records_source_shapes_and_rejection_stage() {
        let (mut app, actor, ledge) = debug_probe_app(Quat::IDENTITY);
        let success = debug_probe(&mut app, actor, LocomotionState::Grounded);
        assert!(success.intent.is_some());
        assert_eq!(success.trace.result, MantleProbeResult::Success);
        assert!((success.trace.trace_radius - 0.29).abs() < 1e-6);
        assert!((success.trace.forward_half_height - 0.875).abs() < 1e-6);
        assert_eq!((success.trace.target_half_height, success.trace.target_radius), (0.9, 0.3));
        assert!(success.trace.forward_hit_center.is_some() && success.trace.downward_hit_center.is_some() && success.trace.start_capsule_center.is_some());
        assert!(success.trace.draw_failed_traces);
        assert!(!debug_probe(&mut app, actor, LocomotionState::Air).trace.draw_failed_traces);
        app.world_mut().entity_mut(ledge).insert(AlsMantleTargetPermission { allows_step_up: false });
        let rejected = debug_probe(&mut app, actor, LocomotionState::Grounded);
        assert_eq!(rejected.trace.result, MantleProbeResult::ForwardRejected);
        assert!(rejected.trace.forward_hit.is_some() && rejected.trace.downward_start.is_none());
        app.world_mut().entity_mut(ledge).remove::<AlsMantleTargetPermission>();
        let blocker = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.01, 0.1, 0.2),
            CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL),
            Transform::from_xyz(0.296, 1.7, -0.95))).id();
        app.update();
        let blocked = debug_probe(&mut app, actor, LocomotionState::Grounded);
        assert_eq!(blocked.trace.result, MantleProbeResult::TargetBlocked, "{blocked:?}");
        assert!(blocked.intent.is_none());
        app.world_mut().despawn(blocker);
        app.world_mut().entity_mut(ledge).insert((Collider::cuboid(2.4, 3.5, 1.2), Transform::from_xyz(0.0, 1.75, -1.4)));
        app.update();
        assert_eq!(debug_probe(&mut app, actor, LocomotionState::Grounded).trace.result, MantleProbeResult::DownwardRejected);
        let (mut app, actor, _) = debug_probe_app(Quat::from_rotation_y(0.7));
        let success = debug_probe(&mut app, actor, LocomotionState::Grounded);
        assert!(success.intent.is_some(), "{success:?}");
        let center = success.trace.start_capsule_center.unwrap();
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.04, 0.1, 0.04),
            CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL),
            Transform::from_translation(center + Vec3::X * center.x.signum() * 0.2)));
        app.update();
        let blocked = debug_probe(&mut app, actor, LocomotionState::Grounded);
        assert_eq!(blocked.trace.result, MantleProbeResult::StartBlocked, "{blocked:?}");
        assert!(blocked.intent.is_none());
    }

    #[test]
    fn mantle_visibility_filter_applies_to_sweeps_and_overlaps() {
        use avian3d::prelude::{CollisionLayers, LayerMask};
        let (mut app, actor, ledge) = debug_probe_app(Quat::IDENTITY);
        app.world_mut().entity_mut(ledge).insert(CollisionLayers::new(crate::als::character::AlsCollisionLayer::Default, LayerMask::ALL));
        app.update();
        let ignored = debug_probe(&mut app, actor, LocomotionState::Grounded);
        assert!(ignored.intent.is_none() && ignored.trace.forward_hit.is_none(), "Physics contact must not imply a Visibility query hit");
        app.world_mut().entity_mut(ledge).insert(CollisionLayers::ALL);
        let blocker = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.01, 0.1, 0.2),
            Transform::from_xyz(0.296, 1.7, -0.95), CollisionLayers::new(crate::als::character::AlsCollisionLayer::Default, LayerMask::ALL))).id();
        app.update();
        assert!(debug_probe(&mut app, actor, LocomotionState::Grounded).intent.is_some());
        app.world_mut().entity_mut(blocker).insert(CollisionLayers::ALL);
        app.update();
        assert!(debug_probe(&mut app, actor, LocomotionState::Grounded).intent.is_none());
    }

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
        for overlay in [
            OverlayState::Default,
            OverlayState::Masculine,
            OverlayState::Feminine,
        ] {
            assert_eq!(select_low_mantle_kind(overlay), MantleKind::Low);
        }
        for overlay in [OverlayState::Bow, OverlayState::Torch, OverlayState::Barrel] {
            assert_eq!(select_low_mantle_kind(overlay), MantleKind::LowLeft);
        }
        assert_eq!(
            select_mantle_kind(LocomotionState::Grounded, OverlayState::Default, 1.25),
            MantleKind::Low
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
            ..default()
        };
        let permission = AlsMantleTargetPermission {
            allows_step_up: false,
        };
        let (scaled_half_height, scaled_radius, scaled_vertical) =
            scaled_mantle_capsule_dimensions(0.9, 0.3, Vec3::new(1.5, 2.0, 0.5));
        let (short_half_height, short_radius, short_vertical) =
            scaled_mantle_capsule_dimensions(0.9, 0.3, Vec3::new(0.5, 0.1, 1.5));
        let passed = mantle_start_allowed(control, false)
            && auto_mantle_start_allowed(control, false, LocomotionState::Air)
            && !mantle_start_allowed(disabled_control, false)
            && !auto_mantle_start_allowed(disabled_control, false, LocomotionState::Air)
            && !permission.allows_step_up
            && (auto_start - 0.75).abs() <= 0.02
            && (auto_start - manual_start).abs() > 0.3
            && (state.start_offset - auto_start).abs() <= 1e-6
            && (scaled_half_height - 1.8).abs() <= 1e-6
            && (scaled_radius - 0.15).abs() <= 1e-6
            && (scaled_vertical - 2.0).abs() <= 1e-6
            && (short_half_height - 0.09).abs() <= 1e-6
            && (short_radius - 0.15).abs() <= 1e-6
            && (short_vertical - 0.1).abs() <= 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.49][VERDICT] manual_start={manual_start:.6} auto_start={auto_start:.6} state_start={:.6} permission={} scaled=({scaled_half_height:.6},{scaled_radius:.6},{scaled_vertical:.6}) -> {verdict}",
            state.start_offset, permission.allows_step_up,
        );
        assert!(passed);
    }

    #[test]
    fn motion_warp_blend_modes_match_source_windows() {
        assert!((mantle_blend_in_amount(MantleKind::Low, 0.05) - 0.15625).abs() < 1e-6);
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
        for (speed, expected_yaw) in [
            (0.001, 0.0),
            (0.005, 0.0),
            (0.009_99, 0.0),
            (0.01, -MANTLE_MAX_REACH_ANGLE),
        ] {
            let input = MantleDetectionInput {
                velocity: Vec3::new(speed, 2.0, 0.0),
                ..default()
            };
            let direction = mantle_trace_direction(&input).unwrap();
            assert!((yaw_from_direction(direction) - expected_yaw).abs() < 1e-6);
        }
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
        let root = Transform::from_translation(root_translation).with_rotation(root_rotation);
        let start_root =
            Transform::from_xyz(0.1, 0.04, -0.15).with_rotation(Quat::from_rotation_y(0.1));
        state.prepare_root_motion(start_root, root);
        let initial = state.target_actor_transform_at(0.0, start_root);
        assert!(initial.translation.distance(state.start) < 1e-6);
        assert!(initial.rotation.angle_between(Quat::IDENTITY) < 1e-3);
        let actor = state.target_actor_transform_at(0.6, root);
        let target_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let expected_translation = state.target;
        let expected_rotation = target_rotation;
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
            scale: Vec3::ONE,
            relative_location: true,
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
            scale: Vec3::ONE,
            relative_location: true,
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
            scale: Vec3::ONE,
            relative_location: true,
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

    #[test]
    fn mantle_warp_keeps_scaled_base_space_and_projects_twist_after_blending() {
        let base = AlsMantleTargetBase {
            entity: Entity::from_raw_u32(2).unwrap(),
            translation: Vec3::new(0.5, 0.2, -0.3),
            rotation: Quat::from_rotation_y(0.4),
            scale: Vec3::new(2.0, 0.5, 1.5),
            relative_location: true,
        };
        let mut state = AlsMantleState::default();
        state.begin_with_target_base(
            MantleKind::LowRight,
            Vec3::new(-1.0, 0.9, 0.3),
            Vec3::new(2.0, 1.8, -2.0),
            0.8,
            -0.6,
            0.8,
            base,
        );
        let start_root =
            Transform::from_xyz(0.1, 0.25, -0.1).with_rotation(Quat::from_rotation_y(-0.2));
        let end_root =
            Transform::from_xyz(0.2, 1.0, -0.65).with_rotation(Quat::from_rotation_y(0.3));
        state.prepare_root_motion(start_root, end_root);
        assert!(
            state
                .target_actor_transform_at(0.0, start_root)
                .translation
                .distance(state.start)
                < 1e-5
        );
        assert!(
            state
                .target_actor_transform_at(state.duration, end_root)
                .translation
                .distance(state.target)
                < 1e-5
        );
        let moved = AlsMantleTargetBase {
            translation: Vec3::new(3.0, 0.0, -2.0),
            rotation: Quat::from_euler(EulerRot::YXZ, 0.7, 0.5, -0.4),
            scale: Vec3::new(0.7, 1.8, 1.2),
            ..base
        };
        assert!(state.refresh_target_base(Some(moved)));
        let local_start = state.warp_start;
        let local_target = state.warp_target;
        let world_start = moved.to_world(local_start);
        let world_target = moved.to_world(local_target);
        let time = 0.25;
        let alpha = state.rotation_warp_amount_at(time);
        let rotation = world_start.rotation.lerp(world_target.rotation, alpha);
        let expected = Quat::from_xyzw(0.0, rotation.y, 0.0, rotation.w).normalize();
        let actual = state.warp_transform_at(time);
        assert!(actual.rotation.angle_between(expected) < 1e-3);
        assert!(actual.rotation.angle_between(rotation) > 0.1);
        assert!(
            actual.translation.distance(world_start.translation.lerp(
                world_target.translation,
                state.location_warp_amount_at(time)
            )) < 1e-5
        );
        let unscaled = moved.translation + moved.rotation * local_start.translation;
        assert!(unscaled.distance(world_start.translation) > 0.1);
        let flattened =
            Quat::from_rotation_y(yaw_from_direction(world_start.rotation * Vec3::NEG_Z)).lerp(
                Quat::from_rotation_y(yaw_from_direction(world_target.rotation * Vec3::NEG_Z)),
                alpha,
            );
        assert!(actual.rotation.angle_between(flattened) > 0.01);
        assert!(state.refresh_target_base(Some(moved)));
        assert_eq!(state.warp_start, local_start);
        assert_eq!(state.warp_target, local_target);
        let zero_scale = AlsMantleTargetBase {
            scale: Vec3::ZERO,
            ..base
        };
        assert_eq!(
            zero_scale.to_local(Transform::IDENTITY).translation,
            Vec3::ZERO
        );
        state.begin(MantleKind::LowRight, Vec3::ZERO, Vec3::ONE, 0.5, 0.0, 0.0);
        assert!(!state.root_motion_ready);
    }

    #[test]
    fn mantle_static_target_keeps_world_warp_but_still_requires_target_lifetime() {
        let base = AlsMantleTargetBase {
            entity: Entity::from_raw_u32(2).unwrap(),
            translation: Vec3::new(0.5, 0.2, -0.3),
            rotation: Quat::from_rotation_y(0.4),
            scale: Vec3::new(2.0, 0.5, 1.5),
            relative_location: false,
        };
        let mut state = AlsMantleState::default();
        state.begin_with_target_base(
            MantleKind::LowRight,
            Vec3::ZERO,
            Vec3::ONE,
            0.8,
            -0.6,
            0.8,
            base,
        );
        state.prepare_root_motion(Transform::IDENTITY, Transform::from_xyz(0.0, 1.0, -0.65));
        let before = state.warp_transform_at(0.25);
        let mut forced_relative = state;
        forced_relative
            .target_base
            .as_mut()
            .unwrap()
            .relative_location = true;
        assert!(
            forced_relative
                .warp_transform_at(0.25)
                .translation
                .distance(before.translation)
                > 0.1
        );
        assert!(state.refresh_target_base(Some(AlsMantleTargetBase {
            translation: Vec3::splat(4.0),
            ..base
        })));
        assert_eq!(state.warp_transform_at(0.25), before);
        assert_eq!(state.start, Vec3::ZERO);
        assert_eq!(state.target, Vec3::ONE);
        assert!(!state.refresh_target_base(None));
        assert!(state.target_invalid);
    }
}
