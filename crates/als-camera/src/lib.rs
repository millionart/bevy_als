use avian3d::prelude::{
    Collider, LinearVelocity, ShapeCastConfig, SpatialQuery, SpatialQueryFilter,
};
use bevy::asset::{Asset, AssetLoader, AssetServer, io::Reader};
use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy::time::{Real, Virtual};
use bevy::transform::{TransformSystems, systems as transform_systems};
use serde::{Deserialize, Serialize};

use als_core::dynamics::AlsMovementBaseDynamicsState;
use als_core::motion::AlsViewYaw;
use als_core::roll::{AlsRollState, ROLL_PLAY_RATE};
use als_core::state::{Gait, RotationMode};
use als_core::view::AlsViewPitch;

pub const CAMERA_CURVE_NAMES: [&str; 13] = [
    "CameraOffsetX",
    "CameraOffsetY",
    "CameraOffsetZ",
    "FovOffset",
    "PivotOffsetX",
    "PivotOffsetY",
    "PivotOffsetZ",
    "LocationLagX",
    "LocationLagY",
    "LocationLagZ",
    "RotationLag",
    "FirstPersonOverride",
    "TraceOverride",
];

pub const CAMERA_CURVE_COUNT: usize = CAMERA_CURVE_NAMES.len();
pub const SPRINT_CAMERA_SHAKE_DURATION_SECS: f32 = 0.2;
pub const SPRINT_CAMERA_SHAKE_BLEND_IN_SECS: f32 = 0.1;
pub const SPRINT_CAMERA_SHAKE_BLEND_OUT_SECS: f32 = 0.1;
pub const SPRINT_CAMERA_SHAKE_LOCATION_AMPLITUDE_M: f32 = 0.01;
pub const SPRINT_CAMERA_SHAKE_ROTATION_AMPLITUDE_DEGREES: f32 = 0.1;
pub const SPRINT_CAMERA_NOTIFY_DURATION_SECS: f32 = 0.6;
pub const SPRINT_CAMERA_NOTIFY_WEIGHT_THRESHOLD: f32 = 0.5;
pub const SPRINT_CAMERA_NOTIFY_TIMES: [(f32, f32); 2] = [(0.06666667, 0.5), (0.36666667, 0.5)];
pub const ROLL_CAMERA_NOTIFY_TIMES: [(f32, f32); 2] = [(0.10000001, 1.0), (0.46666670, 2.0)];
pub const CAMERA_PROFILE_BLEND_DURATION_SECS: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsCameraCurve {
    CameraOffsetX,
    CameraOffsetY,
    CameraOffsetZ,
    FovOffset,
    PivotOffsetX,
    PivotOffsetY,
    PivotOffsetZ,
    LocationLagX,
    LocationLagY,
    LocationLagZ,
    RotationLag,
    FirstPersonOverride,
    TraceOverride,
}

impl AlsCameraCurve {
    pub const fn index(self) -> usize {
        match self {
            Self::CameraOffsetX => 0,
            Self::CameraOffsetY => 1,
            Self::CameraOffsetZ => 2,
            Self::FovOffset => 3,
            Self::PivotOffsetX => 4,
            Self::PivotOffsetY => 5,
            Self::PivotOffsetZ => 6,
            Self::LocationLagX => 7,
            Self::LocationLagY => 8,
            Self::LocationLagZ => 9,
            Self::RotationLag => 10,
            Self::FirstPersonOverride => 11,
            Self::TraceOverride => 12,
        }
    }

    pub const fn name(self) -> &'static str {
        CAMERA_CURVE_NAMES[self.index()]
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsCameraCurveValues {
    pub values: [f32; CAMERA_CURVE_COUNT],
}

impl Default for AlsCameraCurveValues {
    fn default() -> Self {
        Self {
            values: [0.0; CAMERA_CURVE_COUNT],
        }
    }
}

impl AlsCameraCurveValues {
    pub fn get(&self, curve: AlsCameraCurve) -> f32 {
        self.values[curve.index()]
    }

    pub fn set(&mut self, curve: AlsCameraCurve, value: f32) {
        self.values[curve.index()] = value;
    }

    pub fn from_pairs(pairs: impl IntoIterator<Item = (AlsCameraCurve, f32)>) -> Self {
        let mut values = Self::default();
        for (curve, value) in pairs {
            values.set(curve, value);
        }
        values
    }
}

#[derive(Asset, Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AlsCameraParams {
    pub ignore_time_dilation: bool,
    pub teleport_distance_threshold: f32,
    pub trace_radius: f32,
    pub trace_override_offset: Vec3,
    pub trace_distance_smoothing: bool,
    pub trace_distance_smoothing_half_life: f32,
    pub third_person_field_of_view_deg: f32,
    pub first_person_field_of_view_deg: f32,
    pub field_of_view_min_deg: f32,
    pub field_of_view_max_deg: f32,
}

#[derive(Asset, Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AlsCameraSocketOffsets {
    pub first_person_head_local: Vec3,
    pub trace_shoulder_right_head_local: Vec3,
    pub trace_shoulder_left_head_local: Vec3,
}

#[derive(Asset, Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AlsCameraCurveProfiles {
    pub velocity_direction: [f32; CAMERA_CURVE_COUNT],
    pub view_direction: [f32; CAMERA_CURVE_COUNT],
    pub aiming: [f32; CAMERA_CURVE_COUNT],
}

#[derive(Default, bevy::reflect::TypePath)]
pub struct AlsCameraParamsLoader;

#[derive(Default, bevy::reflect::TypePath)]
pub struct AlsCameraSocketOffsetsLoader;

#[derive(Default, bevy::reflect::TypePath)]
pub struct AlsCameraCurveProfilesLoader;

impl AssetLoader for AlsCameraParamsLoader {
    type Asset = AlsCameraParams;
    type Settings = ();
    type Error = CameraLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        load_ron(reader).await
    }

    fn extensions(&self) -> &[&str] {
        &["camera.ron"]
    }
}

impl AssetLoader for AlsCameraSocketOffsetsLoader {
    type Asset = AlsCameraSocketOffsets;
    type Settings = ();
    type Error = CameraLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        load_ron(reader).await
    }

    fn extensions(&self) -> &[&str] {
        &["camera_sockets.ron"]
    }
}

impl AssetLoader for AlsCameraCurveProfilesLoader {
    type Asset = AlsCameraCurveProfiles;
    type Settings = ();
    type Error = CameraLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        load_ron(reader).await
    }

    fn extensions(&self) -> &[&str] {
        &["camera_profiles.ron"]
    }
}

async fn load_ron<T: for<'de> Deserialize<'de>>(
    reader: &mut dyn Reader,
) -> Result<T, CameraLoaderError> {
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .await
        .map_err(CameraLoaderError::Io)?;
    ron::de::from_bytes(&bytes).map_err(CameraLoaderError::Ron)
}

#[derive(Debug)]
pub enum CameraLoaderError {
    Io(std::io::Error),
    Ron(ron::de::SpannedError),
}

impl std::fmt::Display for CameraLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::Ron(error) => write!(f, "ron error: {error}"),
        }
    }
}

impl std::error::Error for CameraLoaderError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlsCameraMode {
    #[default]
    ThirdPerson,
    AimingShoulder,
    FirstPerson,
}

pub fn mode_for_rotation(rotation_mode: RotationMode) -> AlsCameraMode {
    match rotation_mode {
        RotationMode::Aiming => AlsCameraMode::AimingShoulder,
        _ => AlsCameraMode::ThirdPerson,
    }
}

pub fn field_of_view_degrees(first_person: bool, offset: f32, params: &AlsCameraParams) -> f32 {
    field_of_view_for_override(if first_person { 1.0 } else { 0.0 }, offset, None, params)
}

pub fn field_of_view_for_override(
    first_person_override: f32,
    offset: f32,
    override_field_of_view: Option<f32>,
    params: &AlsCameraParams,
) -> f32 {
    let weight = first_person_override.clamp(0.0, 1.0);
    if is_full_weight(weight) {
        return override_field_of_view.unwrap_or(params.first_person_field_of_view_deg);
    }
    let base = override_field_of_view.unwrap_or_else(|| {
        params
            .third_person_field_of_view_deg
            .lerp(params.first_person_field_of_view_deg, weight)
    });
    (base + offset).clamp(params.field_of_view_min_deg, params.field_of_view_max_deg)
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsCameraRig {
    pub mode: AlsCameraMode,
}

impl Default for AlsCameraRig {
    fn default() -> Self {
        Self {
            mode: AlsCameraMode::ThirdPerson,
        }
    }
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsCameraFovOffset(pub f32);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsCameraFovOverride(pub f32);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsCameraCustomTimeDilation(pub f32);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsCameraFieldOfView(pub f32);

impl Default for AlsCameraFieldOfView {
    fn default() -> Self {
        Self(90.0)
    }
}

#[derive(Component)]
pub struct AlsCameraTarget;

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsCameraCurveProfileDriver;

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsCameraCurveProfileRuntime {
    pub initialized: bool,
    pub elapsed: f32,
    pub source: [f32; CAMERA_CURVE_COUNT],
    pub target: [f32; CAMERA_CURVE_COUNT],
}

impl Default for AlsCameraCurveProfileRuntime {
    fn default() -> Self {
        Self {
            initialized: false,
            elapsed: 0.0,
            source: [0.0; CAMERA_CURVE_COUNT],
            target: [0.0; CAMERA_CURVE_COUNT],
        }
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsCameraAnimationNotifyDriver;

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsCameraAnimationTiming {
    pub sprint_time: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsCameraNotifyClip {
    Sprint,
    SprintAcceleration,
    Roll,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AlsCameraActiveClip {
    pub clip: Option<AlsCameraNotifyClip>,
    pub time: f32,
    pub weight: f32,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AlsCameraNotifyTimeline {
    pub previous_clip: Option<AlsCameraNotifyClip>,
    pub previous_time: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsCameraSocketBinding {
    pub avatar: Entity,
    pub root_bone: Option<Entity>,
    pub head_bone: Option<Entity>,
}

impl AlsCameraSocketBinding {
    pub fn new(avatar: Entity) -> Self {
        Self {
            avatar,
            root_bone: None,
            head_bone: None,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsCameraSocketLocations {
    pub root: Vec3,
    pub head: Vec3,
    pub first_person: Vec3,
    pub trace_shoulder_right: Vec3,
    pub trace_shoulder_left: Vec3,
    pub mesh_rotation: Quat,
    pub mesh_scale: f32,
    pub valid: bool,
}

impl Default for AlsCameraSocketLocations {
    fn default() -> Self {
        Self {
            root: Vec3::ZERO,
            head: Vec3::ZERO,
            first_person: Vec3::ZERO,
            trace_shoulder_right: Vec3::ZERO,
            trace_shoulder_left: Vec3::ZERO,
            mesh_rotation: Quat::IDENTITY,
            mesh_scale: 1.0,
            valid: false,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsCameraMovementBase {
    pub entity: Entity,
    pub has_relative_rotation: bool,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsCameraTraceIgnore;

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsCameraRuntime {
    pub initialized: bool,
    pub previous_pivot_target: Vec3,
    pub pivot_lag_location: Vec3,
    pub pivot_lag_location_base_space: Vec3,
    pub camera_rotation: Quat,
    pub camera_rotation_base_space: Quat,
    pub trace_distance_ratio: f32,
    pub previous_global_time_dilation: f32,
    pub movement_base: Option<Entity>,
    pub shake_elapsed: Option<f32>,
    pub shake_scale: f32,
    pub shake_phases: [f32; 6],
    pub shake_nonce: u64,
}

impl Default for AlsCameraRuntime {
    fn default() -> Self {
        Self {
            initialized: false,
            previous_pivot_target: Vec3::ZERO,
            pivot_lag_location: Vec3::ZERO,
            pivot_lag_location_base_space: Vec3::ZERO,
            camera_rotation: Quat::IDENTITY,
            camera_rotation_base_space: Quat::IDENTITY,
            trace_distance_ratio: 1.0,
            previous_global_time_dilation: 1.0,
            movement_base: None,
            shake_elapsed: None,
            shake_scale: 1.0,
            shake_phases: [0.0; 6],
            shake_nonce: 1,
        }
    }
}

#[derive(Message, Debug, Clone, Copy)]
pub struct AlsCameraShakeRequest {
    pub target: Entity,
    pub scale: f32,
}

impl AlsCameraShakeRequest {
    pub fn sprint(target: Entity, scale: f32) -> Self {
        Self { target, scale }
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AlsCameraDebugInfo {
    pub pivot_target: Vec3,
    pub pivot_lag: Option<Vec3>,
    pub pivot: Option<Vec3>,
    pub field_of_view_deg: f32,
    pub right_shoulder: bool,
    pub trace_start: Vec3,
    pub trace_end: Vec3,
    pub trace_radius: f32,
    pub trace_blocking_hit: bool,
    pub trace_adjusted_start: bool,
    pub trace_distance_ratio: f32,
    pub first_person_override: f32,
    pub unscaled_delta_seconds: f32,
    pub shake_location_offset: Vec3,
    pub shake_rotation_degrees: Vec3,
    pub valid: bool,
    pub curve_values: [f32; CAMERA_CURVE_COUNT],
}

impl AlsCameraDebugInfo {
    pub fn curve_value(&self, name: &str) -> f32 {
        CAMERA_CURVE_NAMES
            .iter()
            .position(|candidate| candidate == &name)
            .map(|index| self.curve_values[index])
            .unwrap_or(0.0)
    }
}

pub struct AlsCameraPlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsCameraSet;

impl Plugin for AlsCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<AlsCameraParams>()
            .init_asset::<AlsCameraSocketOffsets>()
            .init_asset::<AlsCameraCurveProfiles>()
            .register_asset_loader(AlsCameraParamsLoader)
            .register_asset_loader(AlsCameraSocketOffsetsLoader)
            .register_asset_loader(AlsCameraCurveProfilesLoader)
            .add_message::<AlsCameraShakeRequest>()
            .add_systems(Update, load_camera_assets)
            .add_systems(Update, ensure_camera_curve_profile_values)
            .add_systems(
                Update,
                source_camera_curve_profile_system.after(ensure_camera_curve_profile_values),
            )
            .add_systems(Update, ensure_camera_notify_timeline)
            .add_systems(
                Update,
                camera_notify_sidecar_system.after(ensure_camera_notify_timeline),
            )
            .add_systems(PostUpdate, ensure_camera_runtime)
            .add_systems(
                PostUpdate,
                (
                    resolve_camera_socket_locations.after(TransformSystems::Propagate),
                    camera_movement_base_bridge_system.after(TransformSystems::Propagate),
                    ApplyDeferred,
                    camera_rig_system.in_set(AlsCameraSet),
                )
                    .chain(),
            )
            .add_systems(
                PostUpdate,
                (
                    transform_systems::mark_dirty_trees,
                    transform_systems::propagate_parent_transforms,
                    transform_systems::sync_simple_transforms,
                )
                    .chain()
                    .after(camera_rig_system),
            );
    }
}

fn load_camera_assets(
    server: Res<AssetServer>,
    mut params_done: Local<bool>,
    mut sockets_done: Local<bool>,
    mut profiles_done: Local<bool>,
    mut params_handle: Local<Option<Handle<AlsCameraParams>>>,
    mut sockets_handle: Local<Option<Handle<AlsCameraSocketOffsets>>>,
    mut profiles_handle: Local<Option<Handle<AlsCameraCurveProfiles>>>,
) {
    if !*params_done {
        let handle = params_handle
            .get_or_insert_with(|| server.load::<AlsCameraParams>("params/als_camera.camera.ron"))
            .clone();
        match server.load_state(handle.id()) {
            bevy::asset::LoadState::Loaded => {
                info!("[CAM] params loaded");
                *params_done = true;
            }
            bevy::asset::LoadState::Failed(error) => {
                error!("[CAM] params failed: {error}");
                *params_done = true;
            }
            _ => {}
        }
    }
    if !*sockets_done {
        let handle = sockets_handle
            .get_or_insert_with(|| {
                server.load::<AlsCameraSocketOffsets>("params/als_camera_sockets.ron")
            })
            .clone();
        match server.load_state(handle.id()) {
            bevy::asset::LoadState::Loaded => {
                info!("[CAM] socket offsets loaded");
                *sockets_done = true;
            }
            bevy::asset::LoadState::Failed(error) => {
                error!("[CAM] socket offsets failed: {error}");
                *sockets_done = true;
            }
            _ => {}
        }
    }
    if !*profiles_done {
        let handle = profiles_handle
            .get_or_insert_with(|| {
                server.load::<AlsCameraCurveProfiles>("params/als_camera_profiles.ron")
            })
            .clone();
        match server.load_state(handle.id()) {
            bevy::asset::LoadState::Loaded => {
                info!("[CAM] curve profiles loaded");
                *profiles_done = true;
            }
            bevy::asset::LoadState::Failed(error) => {
                error!("[CAM] curve profiles failed: {error}");
                *profiles_done = true;
            }
            _ => {}
        }
    }
}

fn ensure_camera_curve_profile_values(
    mut commands: Commands,
    targets: Query<
        (
            Entity,
            Has<AlsCameraCurveValues>,
            Has<AlsCameraCurveProfileRuntime>,
        ),
        (With<AlsCameraTarget>, With<AlsCameraCurveProfileDriver>),
    >,
) {
    for (entity, has_values, has_runtime) in &targets {
        let mut entity_commands = commands.entity(entity);
        if !has_values {
            entity_commands.insert(AlsCameraCurveValues::default());
        }
        if !has_runtime {
            entity_commands.insert(AlsCameraCurveProfileRuntime::default());
        }
    }
}

fn source_camera_curve_profile_system(
    profiles_assets: Res<Assets<AlsCameraCurveProfiles>>,
    virtual_time: Res<Time<Virtual>>,
    mut targets: Query<
        (
            &RotationMode,
            Has<als_core::input_request::AlsFirstPersonView>,
            &mut AlsCameraCurveValues,
            &mut AlsCameraCurveProfileRuntime,
        ),
        (With<AlsCameraTarget>, With<AlsCameraCurveProfileDriver>),
    >,
) {
    let Some((_, profiles)) = profiles_assets.iter().next() else {
        return;
    };
    for (rotation_mode, first_person, mut values, mut runtime) in &mut targets {
        let mut target = match rotation_mode {
            RotationMode::VelocityDirection => profiles.velocity_direction,
            RotationMode::ViewDirection => profiles.view_direction,
            RotationMode::Aiming => profiles.aiming,
        };
        target[AlsCameraCurve::FirstPersonOverride.index()] = if first_person { 1.0 } else { 0.0 };
        if !runtime.initialized {
            runtime.initialized = true;
            runtime.target = target;
            runtime.source = target;
            values.values = target;
            continue;
        }
        if target != runtime.target {
            runtime.source = values.values;
            runtime.target = target;
            runtime.elapsed = 0.0;
        }
        runtime.elapsed =
            (runtime.elapsed + virtual_time.delta_secs()).min(CAMERA_PROFILE_BLEND_DURATION_SECS);
        let alpha = (runtime.elapsed / CAMERA_PROFILE_BLEND_DURATION_SECS).clamp(0.0, 1.0);
        values.values =
            std::array::from_fn(|index| runtime.source[index].lerp(runtime.target[index], alpha));
    }
}

fn ensure_camera_notify_timeline(
    mut commands: Commands,
    targets: Query<
        (
            Entity,
            Has<AlsCameraNotifyTimeline>,
            Has<AlsCameraActiveClip>,
            Has<AlsCameraAnimationTiming>,
        ),
        (With<AlsCameraTarget>, With<AlsCameraAnimationNotifyDriver>),
    >,
) {
    for (entity, has_timeline, has_clip, has_timing) in &targets {
        let mut entity_commands = commands.entity(entity);
        if !has_timeline {
            entity_commands.insert(AlsCameraNotifyTimeline::default());
        }
        if !has_clip {
            entity_commands.insert(AlsCameraActiveClip::default());
        }
        if !has_timing {
            entity_commands.insert(AlsCameraAnimationTiming::default());
        }
    }
}

fn crosses_notify(previous_time: f32, current_time: f32, duration: f32, notify_time: f32) -> bool {
    if current_time >= previous_time {
        previous_time < notify_time && notify_time <= current_time
    } else {
        previous_time < notify_time
            || notify_time <= current_time
            || current_time + duration < previous_time
    }
}

fn camera_notify_sample(
    gait: Gait,
    speed: f32,
    roll: Option<&AlsRollState>,
    timing: Option<&AlsCameraAnimationTiming>,
) -> (Option<AlsCameraNotifyClip>, f32, f32) {
    if roll.is_some_and(|state| state.active) {
        return (
            Some(AlsCameraNotifyClip::Roll),
            roll.map_or(0.0, |state| state.time * ROLL_PLAY_RATE),
            1.0,
        );
    }
    if gait != Gait::Sprinting {
        return (None, 0.0, 0.0);
    }
    let Some(time) = timing
        .and_then(|value| value.sprint_time)
        .filter(|value| value.is_finite())
    else {
        return (None, 0.0, 0.0);
    };
    let sprint_weight = ((speed - 3.75) / (6.5 - 3.75)).clamp(0.0, 1.0);
    (
        Some(AlsCameraNotifyClip::Sprint),
        time.rem_euclid(SPRINT_CAMERA_NOTIFY_DURATION_SECS),
        sprint_weight,
    )
}

fn camera_notify_sidecar_system(
    mut targets: Query<
        (
            Entity,
            &Gait,
            Option<&LinearVelocity>,
            Option<&AlsRollState>,
            Option<&AlsCameraAnimationTiming>,
            &mut AlsCameraActiveClip,
            &mut AlsCameraNotifyTimeline,
        ),
        (With<AlsCameraTarget>, With<AlsCameraAnimationNotifyDriver>),
    >,
    mut shake_requests: MessageWriter<AlsCameraShakeRequest>,
) {
    for (entity, gait, velocity, roll, timing, mut active_clip, mut timeline) in &mut targets {
        let speed = velocity.map_or(0.0, |value| Vec2::new(value.x, value.z).length());
        let (clip, time, weight) = camera_notify_sample(*gait, speed, roll, timing);
        active_clip.clip = clip;
        active_clip.time = time;
        active_clip.weight = weight;
        if clip != timeline.previous_clip {
            timeline.previous_clip = clip;
            timeline.previous_time = time;
            continue;
        }
        match clip {
            Some(AlsCameraNotifyClip::Sprint)
                if weight >= SPRINT_CAMERA_NOTIFY_WEIGHT_THRESHOLD =>
            {
                for (notify_time, scale) in SPRINT_CAMERA_NOTIFY_TIMES {
                    if crosses_notify(
                        timeline.previous_time,
                        time,
                        SPRINT_CAMERA_NOTIFY_DURATION_SECS,
                        notify_time,
                    ) {
                        shake_requests.write(AlsCameraShakeRequest::sprint(entity, scale));
                    }
                }
            }
            Some(AlsCameraNotifyClip::Roll) => {
                for (notify_time, scale) in ROLL_CAMERA_NOTIFY_TIMES {
                    if timeline.previous_time < notify_time && notify_time <= time {
                        shake_requests.write(AlsCameraShakeRequest::sprint(entity, scale));
                    }
                }
            }
            _ => {}
        }
        timeline.previous_time = time;
    }
}

fn ensure_camera_runtime(
    mut commands: Commands,
    cameras: Query<
        (
            Entity,
            Has<AlsCameraRuntime>,
            Has<AlsCameraFieldOfView>,
            Has<AlsCameraDebugInfo>,
        ),
        With<AlsCameraRig>,
    >,
) {
    for (entity, has_runtime, has_field_of_view, has_debug) in &cameras {
        let mut entity_commands = commands.entity(entity);
        if !has_runtime {
            entity_commands.insert(AlsCameraRuntime::default());
        }
        if !has_field_of_view {
            entity_commands.insert(AlsCameraFieldOfView::default());
        }
        if !has_debug {
            entity_commands.insert(AlsCameraDebugInfo::default());
        }
    }
}

fn find_named_descendant(
    root: Entity,
    name: &str,
    children: &Query<&Children>,
    names: &Query<&Name>,
) -> Option<Entity> {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if names
            .get(entity)
            .is_ok_and(|candidate| candidate.as_str() == name)
        {
            return Some(entity);
        }
        if let Ok(descendants) = children.get(entity) {
            stack.extend(descendants.iter());
        }
    }
    None
}

fn resolve_camera_socket_locations(
    mut targets: Query<(&mut AlsCameraSocketBinding, &mut AlsCameraSocketLocations)>,
    children: Query<&Children>,
    names: Query<&Name>,
    globals: Query<&GlobalTransform>,
    offsets_assets: Res<Assets<AlsCameraSocketOffsets>>,
) {
    let Some((_, offsets)) = offsets_assets.iter().next() else {
        return;
    };
    for (mut binding, mut locations) in &mut targets {
        if binding
            .root_bone
            .is_none_or(|entity| globals.get(entity).is_err())
        {
            binding.root_bone = find_named_descendant(binding.avatar, "root", &children, &names);
        }
        if binding
            .head_bone
            .is_none_or(|entity| globals.get(entity).is_err())
        {
            binding.head_bone = find_named_descendant(binding.avatar, "head", &children, &names);
        }
        let Some(root_bone) = binding.root_bone else {
            continue;
        };
        let Some(head_bone) = binding.head_bone else {
            continue;
        };
        let Ok(root) = globals.get(root_bone) else {
            continue;
        };
        let Ok(head) = globals.get(head_bone) else {
            continue;
        };
        let Ok(avatar) = globals.get(binding.avatar) else {
            continue;
        };
        let (scale, rotation, _) = avatar.to_scale_rotation_translation();
        locations.root = root.translation();
        locations.head = head.translation();
        locations.first_person = head.transform_point(offsets.first_person_head_local);
        locations.trace_shoulder_right =
            head.transform_point(offsets.trace_shoulder_right_head_local);
        locations.trace_shoulder_left =
            head.transform_point(offsets.trace_shoulder_left_head_local);
        locations.mesh_rotation = rotation;
        locations.mesh_scale = scale.y.abs().max(f32::EPSILON);
        let became_valid = !locations.valid;
        locations.valid = true;
        if became_valid {
            info!(
                "[CAM][SOCKET] root={:?} head={:?} first_person={:?} -> PASS",
                locations.root, locations.head, locations.first_person
            );
        }
    }
}

#[derive(Clone, Copy)]
struct CameraBaseFrame {
    entity: Entity,
    location: Vec3,
    rotation: Quat,
}

#[derive(Clone, Copy)]
struct TraceResult {
    location: Vec3,
    start: Vec3,
    end: Vec3,
    blocking_hit: bool,
    adjusted_start: bool,
}

fn is_full_weight(weight: f32) -> bool {
    weight >= 1.0 - 1e-4
}

fn damp_alpha(delta_seconds: f32, half_life_seconds: f32) -> f32 {
    if half_life_seconds <= f32::EPSILON {
        return 1.0;
    }
    1.0 - (-std::f32::consts::LN_2 * delta_seconds.max(0.0) / half_life_seconds).exp()
}

fn damp_exact(current: f32, target: f32, delta_seconds: f32, half_life_seconds: f32) -> f32 {
    current.lerp(target, damp_alpha(delta_seconds, half_life_seconds))
}

pub fn ue_location_lag_to_bevy_camera_space(ue_half_lives: Vec3) -> Vec3 {
    Vec3::new(ue_half_lives.y, ue_half_lives.z, ue_half_lives.x)
}

pub fn damp_ue_location_lag(
    current: Vec3,
    target: Vec3,
    delta_seconds: f32,
    ue_half_lives: Vec3,
) -> Vec3 {
    let lag = ue_location_lag_to_bevy_camera_space(ue_half_lives);
    Vec3::new(
        damp_exact(current.x, target.x, delta_seconds, lag.x),
        damp_exact(current.y, target.y, delta_seconds, lag.y),
        damp_exact(current.z, target.z, delta_seconds, lag.z),
    )
}

fn shortest_angle_delta(current: f32, target: f32) -> f32 {
    (target - current + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI
}

fn damp_angle(current: f32, target: f32, delta_seconds: f32, half_life_seconds: f32) -> f32 {
    if half_life_seconds <= f32::EPSILON {
        return target;
    }
    current + shortest_angle_delta(current, target) * damp_alpha(delta_seconds, half_life_seconds)
}

fn damp_rotation(current: Quat, target: Quat, delta_seconds: f32, half_life_seconds: f32) -> Quat {
    if half_life_seconds <= f32::EPSILON {
        return target;
    }
    let (current_yaw, current_pitch, current_roll) = current.to_euler(EulerRot::YXZ);
    let (target_yaw, target_pitch, target_roll) = target.to_euler(EulerRot::YXZ);
    Quat::from_euler(
        EulerRot::YXZ,
        damp_angle(current_yaw, target_yaw, delta_seconds, half_life_seconds),
        damp_angle(
            current_pitch,
            target_pitch,
            delta_seconds,
            half_life_seconds,
        ),
        damp_angle(current_roll, target_roll, delta_seconds, half_life_seconds),
    )
}

fn ue_mesh_vector_to_bevy_meters(vector: Vec3, mesh_scale: f32) -> Vec3 {
    Vec3::new(vector.x, vector.z, vector.y) * (mesh_scale * 0.01)
}

fn ue_camera_vector_to_bevy_meters(vector: Vec3, mesh_scale: f32) -> Vec3 {
    Vec3::new(vector.y, vector.z, -vector.x) * (mesh_scale * 0.01)
}

fn curve_vector(
    curves: Option<&AlsCameraCurveValues>,
    x: AlsCameraCurve,
    y: AlsCameraCurve,
    z: AlsCameraCurve,
) -> Vec3 {
    curves.map_or(Vec3::ZERO, |values| {
        Vec3::new(values.get(x), values.get(y), values.get(z))
    })
}

fn collect_descendants(root: Entity, children: &Query<&Children>) -> Vec<Entity> {
    let mut descendants = Vec::new();
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        descendants.push(entity);
        if let Ok(current_children) = children.get(entity) {
            stack.extend(current_children.iter());
        }
    }
    descendants
}

fn has_blocking_overlap(
    spatial: &SpatialQuery,
    shape: &Collider,
    location: Vec3,
    filter: &SpatialQueryFilter,
    ignored: &Query<(), With<AlsCameraTraceIgnore>>,
) -> bool {
    let mut overlaps = false;
    spatial.shape_intersections_callback(shape, location, Quat::IDENTITY, filter, |entity| {
        if ignored.get(entity).is_ok() {
            true
        } else {
            overlaps = true;
            false
        }
    });
    overlaps
}

fn trace_with_start_adjustment(
    spatial: &SpatialQuery,
    shape: &Collider,
    mut start: Vec3,
    end: Vec3,
    owner_location: Vec3,
    filter: &SpatialQueryFilter,
    ignored: &Query<(), With<AlsCameraTraceIgnore>>,
) -> TraceResult {
    let mut result = TraceResult {
        location: end,
        start,
        end,
        blocking_hit: false,
        adjusted_start: false,
    };
    let vector = end - start;
    let Ok(direction) = Dir3::new(vector) else {
        return result;
    };
    let config = ShapeCastConfig::from_max_distance(vector.length());
    let predicate = |entity| ignored.get(entity).is_err();
    let Some(hit) = spatial.cast_shape_predicate(
        shape,
        start,
        Quat::IDENTITY,
        direction,
        &config,
        filter,
        &predicate,
    ) else {
        return result;
    };
    result.blocking_hit = true;
    if hit.distance > 1e-5 {
        result.location = start + direction.as_vec3() * hit.distance;
        return result;
    }
    let adjustment = hit.point1 - hit.point2;
    let adjustment_direction = adjustment.normalize_or_zero();
    let owner_direction = (owner_location - start).normalize_or_zero();
    if adjustment.length_squared() > 1e-8 && owner_direction.dot(adjustment_direction) >= -1e-5 {
        start += adjustment;
        result.start = start;
        result.adjusted_start = true;
        if !has_blocking_overlap(spatial, shape, start, filter, ignored) {
            let adjusted_vector = end - start;
            if let Ok(adjusted_direction) = Dir3::new(adjusted_vector) {
                let adjusted_config = ShapeCastConfig::from_max_distance(adjusted_vector.length());
                if let Some(adjusted_hit) = spatial.cast_shape_predicate(
                    shape,
                    start,
                    Quat::IDENTITY,
                    adjusted_direction,
                    &adjusted_config,
                    filter,
                    &predicate,
                ) {
                    result.location = start + adjusted_direction.as_vec3() * adjusted_hit.distance;
                    result.blocking_hit = true;
                } else {
                    result.location = end;
                    result.blocking_hit = false;
                }
                return result;
            }
        }
    }
    result.location = start;
    result
}

fn calculate_camera_trace(
    runtime: &mut AlsCameraRuntime,
    spatial: &SpatialQuery,
    start: Vec3,
    end: Vec3,
    owner_location: Vec3,
    trace_radius: f32,
    allow_lag: bool,
    params: &AlsCameraParams,
    filter: &SpatialQueryFilter,
    ignored: &Query<(), With<AlsCameraTraceIgnore>>,
    delta_seconds: f32,
) -> TraceResult {
    let shape = Collider::sphere(trace_radius);
    let mut trace =
        trace_with_start_adjustment(spatial, &shape, start, end, owner_location, filter, ignored);
    if !allow_lag || !params.trace_distance_smoothing {
        runtime.trace_distance_ratio = 1.0;
        return trace;
    }
    let vector = trace.end - trace.start;
    let distance = vector.length();
    if distance <= f32::EPSILON {
        runtime.trace_distance_ratio = 1.0;
        return trace;
    }
    let target_ratio = (trace.location - trace.start).length() / distance;
    runtime.trace_distance_ratio = if target_ratio <= runtime.trace_distance_ratio {
        target_ratio
    } else {
        damp_exact(
            runtime.trace_distance_ratio,
            target_ratio,
            delta_seconds,
            params.trace_distance_smoothing_half_life,
        )
    };
    trace.location = trace.start + vector * runtime.trace_distance_ratio;
    trace
}

fn camera_delta_seconds(
    runtime: &mut AlsCameraRuntime,
    params: &AlsCameraParams,
    virtual_time: &Time<Virtual>,
    real_time: &Time<Real>,
    custom_time_dilation: f32,
) -> f32 {
    let delta = if params.ignore_time_dilation {
        let time_dilation = runtime.previous_global_time_dilation * custom_time_dilation;
        if time_dilation > f32::EPSILON {
            virtual_time.delta_secs() / time_dilation
        } else {
            real_time.delta_secs()
        }
    } else {
        virtual_time.delta_secs()
    };
    runtime.previous_global_time_dilation = virtual_time.effective_speed();
    delta.max(0.0)
}

fn camera_movement_base_bridge_system(
    mut commands: Commands,
    targets: Query<
        (
            Entity,
            Option<&AlsMovementBaseDynamicsState>,
            Option<&AlsCameraMovementBase>,
        ),
        With<AlsCameraTarget>,
    >,
    bases: Query<&GlobalTransform, Without<AlsCameraTarget>>,
) {
    for (entity, dynamics, current) in &targets {
        let next = dynamics.and_then(|state| state.entity).and_then(|base| {
            bases.get(base).ok().map(|_| AlsCameraMovementBase {
                entity: base,
                has_relative_rotation: true,
            })
        });
        if current.copied() == next {
            continue;
        }
        let mut entity_commands = commands.entity(entity);
        if let Some(next) = next {
            entity_commands.insert(next);
        } else {
            entity_commands.remove::<AlsCameraMovementBase>();
        }
    }
}

pub fn vertical_fov_radians_from_horizontal_degrees(
    horizontal_degrees: f32,
    aspect_ratio: f32,
) -> f32 {
    let horizontal_radians = horizontal_degrees.clamp(0.001, 179.999).to_radians();
    let aspect = aspect_ratio.max(f32::EPSILON);
    2.0 * (0.5 * horizontal_radians).tan().atan2(aspect)
}

pub fn horizontal_fov_degrees_from_vertical_radians(
    vertical_radians: f32,
    aspect_ratio: f32,
) -> f32 {
    let aspect = aspect_ratio.max(f32::EPSILON);
    (2.0 * ((0.5 * vertical_radians).tan() * aspect).atan()).to_degrees()
}

fn set_field_of_view(
    projection: &mut Projection,
    field_of_view: &mut AlsCameraFieldOfView,
    value_degrees: f32,
) {
    match projection {
        Projection::Perspective(perspective) => {
            perspective.fov = vertical_fov_radians_from_horizontal_degrees(
                value_degrees,
                perspective.aspect_ratio,
            );
        }
        _ => {
            let mut perspective = PerspectiveProjection::default();
            perspective.fov = vertical_fov_radians_from_horizontal_degrees(
                value_degrees,
                perspective.aspect_ratio,
            );
            *projection = Projection::Perspective(perspective);
        }
    }
    field_of_view.0 = value_degrees;
}

fn next_shake_phase(runtime: &mut AlsCameraRuntime) -> f32 {
    runtime.shake_nonce ^= runtime.shake_nonce << 13;
    runtime.shake_nonce ^= runtime.shake_nonce >> 7;
    runtime.shake_nonce ^= runtime.shake_nonce << 17;
    (runtime.shake_nonce as f64 / u64::MAX as f64) as f32 * std::f32::consts::TAU
}

fn begin_camera_shake(runtime: &mut AlsCameraRuntime, scale: f32) {
    runtime.shake_elapsed = Some(0.0);
    runtime.shake_scale = scale;
    for index in 0..runtime.shake_phases.len() {
        runtime.shake_phases[index] = next_shake_phase(runtime);
    }
}

fn camera_shake_sample(runtime: &mut AlsCameraRuntime, delta_seconds: f32) -> (Vec3, Vec3) {
    let Some(elapsed) = runtime.shake_elapsed else {
        return (Vec3::ZERO, Vec3::ZERO);
    };
    let next_elapsed = elapsed + delta_seconds;
    if next_elapsed >= SPRINT_CAMERA_SHAKE_DURATION_SECS {
        runtime.shake_elapsed = None;
        return (Vec3::ZERO, Vec3::ZERO);
    }
    runtime.shake_elapsed = Some(next_elapsed);
    let envelope = (next_elapsed / SPRINT_CAMERA_SHAKE_BLEND_IN_SECS)
        .min(
            (SPRINT_CAMERA_SHAKE_DURATION_SECS - next_elapsed) / SPRINT_CAMERA_SHAKE_BLEND_OUT_SECS,
        )
        .clamp(0.0, 1.0);
    let loc_phase = std::f32::consts::TAU * next_elapsed;
    let rot_phase = std::f32::consts::TAU * 0.1 * next_elapsed;
    let scale = envelope * runtime.shake_scale;
    let location_ue = Vec3::new(
        (loc_phase + runtime.shake_phases[0]).sin(),
        (loc_phase + runtime.shake_phases[1]).sin(),
        (loc_phase + runtime.shake_phases[2]).sin(),
    ) * SPRINT_CAMERA_SHAKE_LOCATION_AMPLITUDE_M
        * scale;
    let rotation_ue = Vec3::new(
        (rot_phase + runtime.shake_phases[3]).sin(),
        (rot_phase + runtime.shake_phases[4]).sin(),
        (rot_phase + runtime.shake_phases[5]).sin(),
    ) * SPRINT_CAMERA_SHAKE_ROTATION_AMPLITUDE_DEGREES
        * scale;
    (
        Vec3::new(location_ue.y, location_ue.z, -location_ue.x),
        rotation_ue,
    )
}

#[allow(clippy::type_complexity)]
fn camera_rig_system(
    mut cameras: Query<
        (
            Entity,
            &mut Transform,
            &mut Projection,
            &mut AlsCameraRig,
            &mut AlsCameraFieldOfView,
            &mut AlsCameraDebugInfo,
            &mut AlsCameraRuntime,
            Option<&AlsCameraFovOverride>,
        ),
        Without<AlsCameraTarget>,
    >,
    targets: Query<
        (
            Entity,
            &GlobalTransform,
            &RotationMode,
            &AlsViewYaw,
            Option<&AlsViewPitch>,
            Option<&AlsCameraFovOffset>,
            Option<&AlsCameraCustomTimeDilation>,
            Option<&AlsCameraCurveValues>,
            Option<&AlsCameraSocketLocations>,
            Option<&AlsCameraMovementBase>,
            Has<als_core::input_request::AlsFirstPersonView>,
            Has<als_core::input_request::AlsCameraLeftShoulder>,
        ),
        With<AlsCameraTarget>,
    >,
    spatial: SpatialQuery,
    params_assets: Res<Assets<AlsCameraParams>>,
    ignored: Query<(), With<AlsCameraTraceIgnore>>,
    children: Query<&Children>,
    movement_base_transforms: Query<&GlobalTransform>,
    mut shake_requests: MessageReader<AlsCameraShakeRequest>,
    virtual_time: Res<Time<Virtual>>,
    real_time: Res<Time<Real>>,
) {
    let Some((_, params)) = params_assets.iter().next() else {
        return;
    };
    let Ok((
        target_entity,
        target_transform,
        rotation_mode,
        view_yaw,
        view_pitch,
        fov_offset_adapter,
        custom_time_dilation,
        curves,
        socket_locations,
        movement_base,
        first_person_input,
        left_shoulder,
    )) = targets.single()
    else {
        return;
    };
    let Some(sockets) = socket_locations.filter(|locations| locations.valid) else {
        return;
    };
    let Ok((
        camera_entity,
        mut camera_transform,
        mut projection,
        mut rig,
        mut field_of_view,
        mut debug,
        mut runtime,
        field_of_view_override,
    )) = cameras.single_mut()
    else {
        return;
    };
    if let Some(request) = shake_requests
        .read()
        .find(|request| request.target == target_entity)
    {
        begin_camera_shake(&mut runtime, request.scale);
    }
    let delta_seconds = camera_delta_seconds(
        &mut runtime,
        params,
        &virtual_time,
        &real_time,
        custom_time_dilation.map_or(1.0, |dilation| dilation.0),
    );
    let target_rotation = Quat::from_euler(
        EulerRot::YXZ,
        view_yaw.0,
        view_pitch.map_or(0.0, |pitch| pitch.0),
        0.0,
    );
    let pivot_target = (sockets.root + sockets.head) * 0.5;
    let first_person_override = curves
        .map(|values| {
            values
                .get(AlsCameraCurve::FirstPersonOverride)
                .clamp(0.0, 1.0)
        })
        .unwrap_or(if first_person_input { 1.0 } else { 0.0 });
    let full_first_person = is_full_weight(first_person_override);
    rig.mode = if full_first_person {
        AlsCameraMode::FirstPerson
    } else {
        mode_for_rotation(*rotation_mode)
    };
    if !runtime.initialized {
        runtime.initialized = true;
        runtime.previous_pivot_target = pivot_target;
        runtime.pivot_lag_location = pivot_target;
        runtime.camera_rotation = target_rotation;
        runtime.camera_rotation_base_space = target_rotation;
        runtime.trace_distance_ratio = 1.0;
    }
    let fov_curve = curves.map_or(0.0, |values| values.get(AlsCameraCurve::FovOffset))
        + fov_offset_adapter.map_or(0.0, |offset| offset.0);
    let mut debug_curve_values = curves.map_or([0.0; CAMERA_CURVE_COUNT], |values| values.values);
    debug_curve_values[AlsCameraCurve::FovOffset.index()] = fov_curve;
    if full_first_person {
        runtime.previous_pivot_target = pivot_target;
        runtime.pivot_lag_location = pivot_target;
        runtime.camera_rotation = target_rotation;
        runtime.camera_rotation_base_space = target_rotation;
        runtime.trace_distance_ratio = 1.0;
        let fov = field_of_view_for_override(
            first_person_override,
            fov_curve,
            field_of_view_override.map(|value| value.0),
            params,
        );
        let (shake_location, shake_rotation_degrees) =
            camera_shake_sample(&mut runtime, delta_seconds);
        set_field_of_view(&mut projection, &mut field_of_view, fov);
        camera_transform.translation = sockets.first_person + shake_location;
        camera_transform.rotation = target_rotation
            * Quat::from_euler(
                EulerRot::YXZ,
                shake_rotation_degrees.y.to_radians(),
                shake_rotation_degrees.x.to_radians(),
                shake_rotation_degrees.z.to_radians(),
            );
        write_debug(
            &mut debug,
            pivot_target,
            pivot_target,
            pivot_target,
            fov,
            !left_shoulder,
            TraceResult {
                location: Vec3::ZERO,
                start: Vec3::ZERO,
                end: Vec3::ZERO,
                blocking_hit: false,
                adjusted_start: false,
            },
            params.trace_radius * sockets.mesh_scale,
            1.0,
            first_person_override,
            delta_seconds,
            shake_location,
            shake_rotation_degrees,
            debug_curve_values,
        );
        return;
    }
    let base = movement_base.and_then(|base| {
        if !base.has_relative_rotation {
            return None;
        }
        movement_base_transforms
            .get(base.entity)
            .ok()
            .map(|transform| CameraBaseFrame {
                entity: base.entity,
                location: transform.translation(),
                rotation: transform.rotation(),
            })
    });
    if runtime.movement_base != base.map(|frame| frame.entity) {
        if let Some(frame) = base {
            runtime.pivot_lag_location_base_space = frame
                .rotation
                .inverse()
                .mul_vec3(runtime.pivot_lag_location - frame.location);
            runtime.camera_rotation_base_space = frame.rotation.inverse() * runtime.camera_rotation;
        } else {
            runtime.pivot_lag_location_base_space = Vec3::ZERO;
            runtime.camera_rotation_base_space = Quat::IDENTITY;
        }
        runtime.movement_base = base.map(|frame| frame.entity);
    }
    let allow_lag = params.teleport_distance_threshold <= 0.0
        || runtime.previous_pivot_target.distance_squared(pivot_target)
            <= params.teleport_distance_threshold.powi(2);
    runtime.previous_pivot_target = pivot_target;
    if let Some(frame) = base {
        runtime.camera_rotation = frame.rotation * runtime.camera_rotation_base_space;
    }
    let rotation_lag = curves.map_or(0.0, |values| values.get(AlsCameraCurve::RotationLag));
    runtime.camera_rotation = if allow_lag {
        damp_rotation(
            runtime.camera_rotation,
            target_rotation,
            delta_seconds,
            rotation_lag,
        )
    } else {
        target_rotation
    };
    if let Some(frame) = base {
        runtime.camera_rotation_base_space = frame.rotation.inverse() * runtime.camera_rotation;
        runtime.pivot_lag_location = frame.location
            + frame
                .rotation
                .mul_vec3(runtime.pivot_lag_location_base_space);
    }
    let (camera_yaw, _, _) = runtime.camera_rotation.to_euler(EulerRot::YXZ);
    let yaw_rotation = Quat::from_rotation_y(camera_yaw);
    let pivot_lag = if allow_lag {
        let current = yaw_rotation.inverse().mul_vec3(runtime.pivot_lag_location);
        let target = yaw_rotation.inverse().mul_vec3(pivot_target);
        let ue_lag = Vec3::new(
            curves.map_or(0.0, |values| values.get(AlsCameraCurve::LocationLagX)),
            curves.map_or(0.0, |values| values.get(AlsCameraCurve::LocationLagY)),
            curves.map_or(0.0, |values| values.get(AlsCameraCurve::LocationLagZ)),
        );
        yaw_rotation.mul_vec3(damp_ue_location_lag(current, target, delta_seconds, ue_lag))
    } else {
        pivot_target
    };
    runtime.pivot_lag_location = pivot_lag;
    if let Some(frame) = base {
        runtime.pivot_lag_location_base_space = frame
            .rotation
            .inverse()
            .mul_vec3(runtime.pivot_lag_location - frame.location);
    }
    let pivot_offset = sockets
        .mesh_rotation
        .mul_vec3(ue_mesh_vector_to_bevy_meters(
            curve_vector(
                curves,
                AlsCameraCurve::PivotOffsetX,
                AlsCameraCurve::PivotOffsetY,
                AlsCameraCurve::PivotOffsetZ,
            ),
            sockets.mesh_scale,
        ));
    let pivot = pivot_lag + pivot_offset;
    let camera_offset = runtime
        .camera_rotation
        .mul_vec3(ue_camera_vector_to_bevy_meters(
            curve_vector(
                curves,
                AlsCameraCurve::CameraOffsetX,
                AlsCameraCurve::CameraOffsetY,
                AlsCameraCurve::CameraOffsetZ,
            ),
            sockets.mesh_scale,
        ));
    let camera_target = pivot + camera_offset;
    let shoulder = if left_shoulder {
        sockets.trace_shoulder_left
    } else {
        sockets.trace_shoulder_right
    };
    let trace_override = curves
        .map(|values| values.get(AlsCameraCurve::TraceOverride).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let trace_start = shoulder.lerp(
        pivot_target + pivot_offset + params.trace_override_offset,
        trace_override,
    );
    let mut excluded = collect_descendants(target_entity, &children);
    excluded.extend(collect_descendants(camera_entity, &children));
    let filter = SpatialQueryFilter::from_excluded_entities(excluded);
    let trace = calculate_camera_trace(
        &mut runtime,
        &spatial,
        trace_start,
        camera_target,
        target_transform.translation(),
        params.trace_radius * sockets.mesh_scale,
        allow_lag,
        params,
        &filter,
        &ignored,
        delta_seconds,
    );
    let fov = field_of_view_for_override(
        first_person_override,
        fov_curve,
        field_of_view_override.map(|value| value.0),
        params,
    );
    let (shake_location, shake_rotation_degrees) = camera_shake_sample(&mut runtime, delta_seconds);
    set_field_of_view(&mut projection, &mut field_of_view, fov);
    camera_transform.translation = trace
        .location
        .lerp(sockets.first_person, first_person_override)
        + shake_location;
    camera_transform.rotation = runtime.camera_rotation
        * Quat::from_euler(
            EulerRot::YXZ,
            shake_rotation_degrees.y.to_radians(),
            shake_rotation_degrees.x.to_radians(),
            shake_rotation_degrees.z.to_radians(),
        );
    write_debug(
        &mut debug,
        pivot_target,
        pivot_lag,
        pivot,
        fov,
        !left_shoulder,
        trace,
        params.trace_radius * sockets.mesh_scale,
        runtime.trace_distance_ratio,
        first_person_override,
        delta_seconds,
        shake_location,
        shake_rotation_degrees,
        debug_curve_values,
    );
}

#[allow(clippy::too_many_arguments)]
fn write_debug(
    debug: &mut AlsCameraDebugInfo,
    pivot_target: Vec3,
    pivot_lag: Vec3,
    pivot: Vec3,
    field_of_view: f32,
    right_shoulder: bool,
    trace: TraceResult,
    trace_radius: f32,
    trace_distance_ratio: f32,
    first_person_override: f32,
    unscaled_delta_seconds: f32,
    shake_location_offset: Vec3,
    shake_rotation_degrees: Vec3,
    curve_values: [f32; CAMERA_CURVE_COUNT],
) {
    let became_valid = !debug.valid;
    debug.pivot_target = pivot_target;
    debug.pivot_lag = Some(pivot_lag);
    debug.pivot = Some(pivot);
    debug.field_of_view_deg = field_of_view;
    debug.right_shoulder = right_shoulder;
    debug.trace_start = trace.start;
    debug.trace_end = trace.end;
    debug.trace_radius = trace_radius;
    debug.trace_blocking_hit = trace.blocking_hit;
    debug.trace_adjusted_start = trace.adjusted_start;
    debug.trace_distance_ratio = trace_distance_ratio;
    debug.first_person_override = first_person_override;
    debug.unscaled_delta_seconds = unscaled_delta_seconds;
    debug.shake_location_offset = shake_location_offset;
    debug.shake_rotation_degrees = shake_rotation_degrees;
    debug.curve_values = curve_values;
    debug.valid = true;
    if became_valid {
        info!("[CAM] debug info valid");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> AlsCameraParams {
        AlsCameraParams {
            ignore_time_dilation: true,
            teleport_distance_threshold: 2.0,
            trace_radius: 0.15,
            trace_override_offset: Vec3::Y * 0.4,
            trace_distance_smoothing: true,
            trace_distance_smoothing_half_life: 0.2,
            third_person_field_of_view_deg: 90.0,
            first_person_field_of_view_deg: 90.0,
            field_of_view_min_deg: 5.0,
            field_of_view_max_deg: 175.0,
        }
    }

    #[test]
    fn curve_values_cover_the_source_contract() {
        let mut values = AlsCameraCurveValues::default();
        let curves = [
            AlsCameraCurve::CameraOffsetX,
            AlsCameraCurve::CameraOffsetY,
            AlsCameraCurve::CameraOffsetZ,
            AlsCameraCurve::FovOffset,
            AlsCameraCurve::PivotOffsetX,
            AlsCameraCurve::PivotOffsetY,
            AlsCameraCurve::PivotOffsetZ,
            AlsCameraCurve::LocationLagX,
            AlsCameraCurve::LocationLagY,
            AlsCameraCurve::LocationLagZ,
            AlsCameraCurve::RotationLag,
            AlsCameraCurve::FirstPersonOverride,
            AlsCameraCurve::TraceOverride,
        ];
        for (index, curve) in curves.into_iter().enumerate() {
            values.set(curve, index as f32 + 1.0);
            assert_eq!(curve.name(), CAMERA_CURVE_NAMES[index]);
            assert_eq!(values.get(curve), index as f32 + 1.0);
        }
    }

    #[test]
    fn full_first_person_skips_fov_offset() {
        let params = params();
        assert_eq!(field_of_view_degrees(false, 12.0, &params), 102.0);
        assert_eq!(field_of_view_degrees(true, 12.0, &params), 90.0);
        assert_eq!(field_of_view_degrees(false, 500.0, &params), 175.0);
    }

    #[test]
    fn horizontal_field_of_view_converts_to_vertical_projection_field_of_view() {
        let aspect = 16.0 / 9.0;
        let vertical = vertical_fov_radians_from_horizontal_degrees(90.0, aspect);
        assert!((vertical.to_degrees() - 58.7155).abs() < 1e-3);
        let horizontal = horizontal_fov_degrees_from_vertical_radians(vertical, aspect);
        assert!((horizontal - 90.0).abs() < 1e-4);
    }

    #[test]
    fn field_of_view_recomputes_for_projection_aspect_ratio() {
        let mut projection = Projection::Perspective(PerspectiveProjection {
            aspect_ratio: 4.0 / 3.0,
            ..default()
        });
        let mut field_of_view = AlsCameraFieldOfView::default();
        set_field_of_view(&mut projection, &mut field_of_view, 90.0);
        let first_vertical = match projection {
            Projection::Perspective(ref perspective) => perspective.fov,
            _ => unreachable!(),
        };
        if let Projection::Perspective(ref mut perspective) = projection {
            perspective.aspect_ratio = 16.0 / 9.0;
        }
        set_field_of_view(&mut projection, &mut field_of_view, 90.0);
        let second_vertical = match projection {
            Projection::Perspective(ref perspective) => perspective.fov,
            _ => unreachable!(),
        };
        assert!(second_vertical < first_vertical);
        assert_eq!(field_of_view.0, 90.0);
        assert!(
            (horizontal_fov_degrees_from_vertical_radians(second_vertical, 16.0 / 9.0) - 90.0)
                .abs()
                < 1e-4
        );
    }

    #[test]
    fn exact_damper_halves_distance_at_its_half_life() {
        assert!((damp_exact(0.0, 1.0, 0.2, 0.2) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn location_lag_half_lives_follow_camera_axis_conversion() {
        let bevy = ue_location_lag_to_bevy_camera_space(Vec3::new(0.2, 0.4, 0.8));
        let current = Vec3::ZERO;
        let target = Vec3::ONE;
        let next = damp_ue_location_lag(current, target, 0.2, Vec3::new(0.2, 0.4, 0.8));
        let expected = Vec3::new(0.292_893_23, 0.159_103_59, 0.5);
        let passed =
            bevy.distance(Vec3::new(0.4, 0.8, 0.2)) <= 1e-6 && next.distance(expected) <= 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.52][VERDICT] bevy_half_lives=({:.3},{:.3},{:.3}) response=({:.6},{:.6},{:.6}) -> {verdict}",
            bevy.x, bevy.y, bevy.z, next.x, next.y, next.z,
        );
        assert!(passed);
    }

    #[test]
    fn sprint_notify_uses_the_source_animation_time() {
        let timing = AlsCameraAnimationTiming {
            sprint_time: Some(0.366_666_67),
        };
        let (clip, time, weight) = camera_notify_sample(Gait::Sprinting, 6.5, None, Some(&timing));
        let (missing_clip, missing_time, missing_weight) =
            camera_notify_sample(Gait::Sprinting, 6.5, None, None);
        let passed = clip == Some(AlsCameraNotifyClip::Sprint)
            && (time - 0.366_666_67).abs() <= 1e-6
            && (weight - 1.0).abs() <= 1e-6
            && missing_clip.is_none()
            && missing_time == 0.0
            && missing_weight == 0.0;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.53][VERDICT] sprint_time={time:.6} weight={weight:.3} missing_clip={} -> {verdict}",
            missing_clip.is_none(),
        );
        assert!(passed);
    }

    #[test]
    fn socket_vector_conversion_matches_exported_skeleton_space() {
        assert!(
            ue_mesh_vector_to_bevy_meters(Vec3::new(4.0, 14.0, 0.0), 1.0)
                .distance(Vec3::new(0.04, 0.0, 0.14))
                < 1e-6
        );
        assert!(
            ue_mesh_vector_to_bevy_meters(Vec3::new(5.0, 0.0, 20.0), 1.0)
                .distance(Vec3::new(0.05, 0.20, 0.0))
                < 1e-6
        );
    }

    #[test]
    fn camera_offset_conversion_places_negative_source_forward_behind_bevy_camera() {
        assert!(
            ue_camera_vector_to_bevy_meters(Vec3::new(-320.0, 0.0, 50.0), 1.0)
                .distance(Vec3::new(0.0, 0.5, 3.2))
                < 1e-6
        );
    }
}
