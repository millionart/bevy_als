use avian3d::prelude::{
    Collider, ShapeCastConfig, SimpleCollider, SpatialQuery, SpatialQueryFilter,
};
pub mod source_definition;
use bevy::asset::{Asset, AssetLoader, AssetServer, io::Reader};
use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy::time::{Real, Virtual};
use bevy::transform::{TransformSystems, systems as transform_systems};
use serde::{Deserialize, Serialize};
pub use source_definition::{
    AlsCameraDefinition, CameraPhysicsBody, CameraShakeDefinition, CameraWave,
};

use als_core::aim::AlsLastFreeRotationMode;
use als_core::dynamics::AlsMovementBaseDynamicsState;
use als_core::motion::AlsViewYaw;
use als_core::state::RotationMode;
use als_core::view::AlsViewPitch;
use als_ext::control_rig_math::rig_damper_alpha as damp_alpha;
use als_ext::{sum_penetration_adjustments, unscaled_delta_seconds};

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
pub const CAMERA_PROFILE_BLEND_DURATION_SECS: f32 = 0.5;
pub const TRACE_START_ADJUSTMENT_PADDING_METERS: f32 = 0.01;

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
    let weight = if weight > 1e-5 { weight } else { 0.0 };
    let base = override_field_of_view.unwrap_or_else(|| {
        params
            .third_person_field_of_view_deg
            .lerp(params.first_person_field_of_view_deg, weight)
    });
    (base + offset).clamp(params.field_of_view_min_deg, params.field_of_view_max_deg)
}

#[derive(Component, Debug, Clone, Copy)]
#[require(AlsCameraViewInfo, AlsCameraRuntime, AlsCameraDebugInfo)]
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

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsCharacterCamera {
    pub character: Entity,
}

#[derive(Component, Clone)]
pub struct AlsCameraDefinitionHandle(pub Handle<AlsCameraDefinition>);

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsCameraAnimationFrame {
    pub delta_seconds: f32,
    pub active: bool,
    pub force_no_lag: bool,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AlsCameraViewInfo {
    pub location: Vec3,
    pub rotation: Quat,
    pub field_of_view_deg: f32,
    pub valid: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsCameraViewSource(pub Entity);

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsCameraCurveProfileDriver;

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsCameraCurveProfileRuntime {
    pub initialized: bool,
    pub elapsed: f32,
    pub source: [f32; CAMERA_CURVE_COUNT],
    pub target: [f32; CAMERA_CURVE_COUNT],
    pub previous_global_time_dilation: f32,
}

impl Default for AlsCameraCurveProfileRuntime {
    fn default() -> Self {
        Self {
            initialized: false,
            elapsed: 0.0,
            source: [0.0; CAMERA_CURVE_COUNT],
            target: [0.0; CAMERA_CURVE_COUNT],
            previous_global_time_dilation: 1.0,
        }
    }
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
    pub trace_adjustment: Vec3,
    pub trace_adjustment_count: u32,
    pub trace_adjustment_resolved: bool,
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
            .init_asset::<AlsCameraDefinition>()
            .init_asset_loader::<source_definition::AlsCameraDefinitionLoader>()
            .register_asset_loader(AlsCameraParamsLoader)
            .register_asset_loader(AlsCameraSocketOffsetsLoader)
            .register_asset_loader(AlsCameraCurveProfilesLoader)
            .add_message::<AlsCameraShakeRequest>()
            .add_systems(Update, load_camera_assets)
            .add_systems(Update, ensure_camera_curve_profile_values)
            .add_systems(
                Update,
                source_camera_curve_profile_system
                    .after(ensure_camera_curve_profile_values)
                    .after(als_core::input_request::input_state_request_system)
                    .after(als_core::transition::state_transition_system),
            )
            .add_systems(PostUpdate, ensure_camera_view_fields.before(AlsCameraSet))
            .add_systems(
                PostUpdate,
                (
                    resolve_camera_socket_locations.after(TransformSystems::Propagate),
                    camera_movement_base_bridge_system.after(TransformSystems::Propagate),
                    ApplyDeferred,
                    camera_rig_system,
                    apply_camera_view,
                )
                    .chain()
                    .in_set(AlsCameraSet),
            )
            .add_systems(
                PostUpdate,
                (
                    transform_systems::mark_dirty_trees,
                    transform_systems::propagate_parent_transforms,
                    transform_systems::sync_simple_transforms,
                )
                    .chain()
                    .after(apply_camera_view),
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
    params_assets: Res<Assets<AlsCameraParams>>,
    virtual_time: Res<Time<Virtual>>,
    real_time: Res<Time<Real>>,
    mut targets: Query<
        (
            &RotationMode,
            Has<als_core::input_request::AlsFirstPersonView>,
            Option<&AlsLastFreeRotationMode>,
            Option<&AlsCameraCustomTimeDilation>,
            &mut AlsCameraCurveValues,
            &mut AlsCameraCurveProfileRuntime,
        ),
        (With<AlsCameraTarget>, With<AlsCameraCurveProfileDriver>),
    >,
) {
    let Some((_, profiles)) = profiles_assets.iter().next() else {
        return;
    };
    let Some((_, params)) = params_assets.iter().next() else {
        return;
    };
    for (
        rotation_mode,
        first_person,
        desired_rotation_mode,
        custom_time_dilation,
        mut values,
        mut runtime,
    ) in &mut targets
    {
        let profile_rotation_mode = camera_profile_rotation_mode(
            *rotation_mode,
            first_person,
            desired_rotation_mode.copied(),
        );
        let mut target = match profile_rotation_mode {
            RotationMode::VelocityDirection => profiles.velocity_direction,
            RotationMode::ViewDirection => profiles.view_direction,
            RotationMode::Aiming => profiles.aiming,
        };
        target[AlsCameraCurve::FirstPersonOverride.index()] = if first_person { 1.0 } else { 0.0 };
        let delta_seconds = camera_delta_seconds_from_times(
            &mut runtime.previous_global_time_dilation,
            params,
            virtual_time.delta_secs(),
            real_time.delta_secs(),
            virtual_time.effective_speed(),
            custom_time_dilation.map_or(1.0, |dilation| dilation.0),
        );
        advance_camera_curve_profile(&mut runtime, &mut values, target, delta_seconds);
    }
}

pub fn camera_profile_rotation_mode(
    rotation_mode: RotationMode,
    first_person: bool,
    desired_rotation_mode: Option<AlsLastFreeRotationMode>,
) -> RotationMode {
    if first_person {
        desired_rotation_mode.map_or(rotation_mode, |desired| desired.0)
    } else {
        rotation_mode
    }
}

fn advance_camera_curve_profile(
    runtime: &mut AlsCameraCurveProfileRuntime,
    values: &mut AlsCameraCurveValues,
    target: [f32; CAMERA_CURVE_COUNT],
    delta_seconds: f32,
) {
    if !runtime.initialized {
        runtime.initialized = true;
        runtime.target = target;
        runtime.source = target;
        values.values = target;
        return;
    }
    if target != runtime.target {
        runtime.source = values.values;
        runtime.target = target;
        runtime.elapsed = 0.0;
    }
    runtime.elapsed = (runtime.elapsed + delta_seconds).min(CAMERA_PROFILE_BLEND_DURATION_SECS);
    let alpha = (runtime.elapsed / CAMERA_PROFILE_BLEND_DURATION_SECS).clamp(0.0, 1.0);
    values.values =
        std::array::from_fn(|index| runtime.source[index].lerp(runtime.target[index], alpha));
}

fn ensure_camera_view_fields(
    mut commands: Commands,
    cameras: Query<
        Entity,
        (
            With<Camera3d>,
            Without<AlsCameraFieldOfView>,
            Or<(With<AlsCameraRig>, With<AlsCameraViewSource>)>,
        ),
    >,
) {
    for entity in &cameras {
        commands
            .entity(entity)
            .insert(AlsCameraFieldOfView::default());
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
    mut targets: Query<(
        &mut AlsCameraSocketBinding,
        &mut AlsCameraSocketLocations,
        &GlobalTransform,
        &Collider,
    )>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    globals: Query<&GlobalTransform>,
    offsets_assets: Res<Assets<AlsCameraSocketOffsets>>,
) {
    let Some((_, offsets)) = offsets_assets.iter().next() else {
        return;
    };
    for (mut binding, mut locations, actor, collider) in &mut targets {
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
        let Ok(mesh_parent) = parents.get(root_bone) else {
            continue;
        };
        let mesh_entity = mesh_parent.parent();
        let Ok(mesh) = globals.get(mesh_entity) else {
            continue;
        };
        let (scale, rotation, _) = mesh.to_scale_rotation_translation();
        locations.root = if parents.get(mesh_entity).is_err() {
            let bounds = collider.aabb(actor.translation(), actor.rotation());
            actor.translation() - Vec3::Y * (bounds.max.y - bounds.min.y) * 0.5
        } else {
            root.translation()
        };
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
    adjustment: Vec3,
    adjustment_count: u32,
    adjustment_resolved: bool,
}

fn is_full_weight(weight: f32) -> bool {
    weight >= 1.0 - 1e-5
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

fn damp_rotation(current: Quat, target: Quat, delta_seconds: f32, half_life_seconds: f32) -> Quat {
    let (current_yaw, current_pitch, current_roll) = current.to_euler(EulerRot::YXZ);
    let (target_yaw, target_pitch, target_roll) = target.to_euler(EulerRot::YXZ);
    let current = Vec3::new(-current_yaw, current_pitch, -current_roll);
    let target_angles = Vec3::new(-target_yaw, target_pitch, -target_roll);
    let delta = (target_angles - current).to_array().map(|angle| {
        let angle = angle.rem_euclid(std::f32::consts::TAU);
        if angle > std::f32::consts::PI {
            angle - std::f32::consts::TAU
        } else {
            angle
        }
    });
    if delta
        .iter()
        .all(|angle| angle.abs() <= 1e-4_f32.to_radians())
    {
        return target;
    }
    let delta = Vec3::from_array(delta.map(|angle| {
        if angle >= 175_f32.to_radians() {
            angle - std::f32::consts::TAU
        } else {
            angle
        }
    }));
    let result = current + delta * damp_alpha(delta_seconds, half_life_seconds);
    Quat::from_euler(EulerRot::YXZ, -result.x, result.y, -result.z)
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

fn start_penetration_adjustment(
    spatial: &SpatialQuery,
    shape: &Collider,
    start: Vec3,
    direction: Dir3,
    config: &ShapeCastConfig,
    filter: &SpatialQueryFilter,
    ignored: &Query<(), With<AlsCameraTraceIgnore>>,
) -> (Vec3, u32) {
    let mut adjustments = Vec::new();
    spatial.shape_hits_callback(
        shape,
        start,
        Quat::IDENTITY,
        direction,
        config,
        filter,
        |hit| {
            if ignored.get(hit.entity).is_ok() || hit.distance > 1e-5 {
                return true;
            }
            let adjustment = hit.point1 - hit.point2;
            if adjustment.length_squared() > 1e-8 && adjustment.is_finite() {
                adjustments.push(adjustment);
            }
            true
        },
    );
    let count = adjustments.len() as u32;
    (sum_penetration_adjustments(adjustments), count)
}

fn trace_with_start_adjustment(
    spatial: &SpatialQuery,
    shape: &Collider,
    adjustment_shape: &Collider,
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
        adjustment: Vec3::ZERO,
        adjustment_count: 0,
        adjustment_resolved: false,
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
    let (adjustment, adjustment_count) = start_penetration_adjustment(
        spatial,
        adjustment_shape,
        start,
        direction,
        &config,
        filter,
        ignored,
    );
    let adjustment_direction = adjustment.normalize_or_zero();
    let owner_direction = (owner_location - start).normalize_or_zero();
    if adjustment.length_squared() > 1e-8 && owner_direction.dot(adjustment_direction) >= -1e-5 {
        start += adjustment;
        result.start = start;
        result.adjusted_start = true;
        result.adjustment = adjustment;
        result.adjustment_count = adjustment_count;
        result.adjustment_resolved = !has_blocking_overlap(spatial, shape, start, filter, ignored);
        if result.adjustment_resolved {
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
    let adjustment_shape = Collider::sphere(trace_radius + TRACE_START_ADJUSTMENT_PADDING_METERS);
    let mut trace = trace_with_start_adjustment(
        spatial,
        &shape,
        &adjustment_shape,
        start,
        end,
        owner_location,
        filter,
        ignored,
    );
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

pub fn camera_delta_seconds_from_times(
    previous_global_time_dilation: &mut f32,
    params: &AlsCameraParams,
    virtual_delta_seconds: f32,
    real_delta_seconds: f32,
    effective_global_time_dilation: f32,
    custom_time_dilation: f32,
) -> f32 {
    if params.ignore_time_dilation {
        unscaled_delta_seconds(
            previous_global_time_dilation,
            virtual_delta_seconds,
            real_delta_seconds,
            effective_global_time_dilation,
            custom_time_dilation,
        )
    } else {
        *previous_global_time_dilation = effective_global_time_dilation;
        virtual_delta_seconds.max(0.0)
    }
}

fn camera_delta_seconds(
    runtime: &mut AlsCameraRuntime,
    params: &AlsCameraParams,
    virtual_time: &Time<Virtual>,
    real_time: &Time<Real>,
    custom_time_dilation: f32,
) -> f32 {
    camera_delta_seconds_from_times(
        &mut runtime.previous_global_time_dilation,
        params,
        virtual_time.delta_secs(),
        real_time.delta_secs(),
        virtual_time.effective_speed(),
        custom_time_dilation,
    )
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
            &mut AlsCameraViewInfo,
            &mut AlsCameraRig,
            &mut AlsCameraDebugInfo,
            &mut AlsCameraRuntime,
            Option<&AlsCameraFovOverride>,
            Option<&AlsCharacterCamera>,
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
            Option<&AlsCameraDefinitionHandle>,
            Option<&AlsCameraAnimationFrame>,
        ),
        With<AlsCameraTarget>,
    >,
    spatial: SpatialQuery,
    params_assets: Res<Assets<AlsCameraParams>>,
    definitions: Res<Assets<AlsCameraDefinition>>,
    ignored: Query<(), With<AlsCameraTraceIgnore>>,
    children: Query<&Children>,
    movement_base_transforms: Query<&GlobalTransform>,
    mut shake_requests: MessageReader<AlsCameraShakeRequest>,
    virtual_time: Res<Time<Virtual>>,
    real_time: Res<Time<Real>>,
) {
    let requests = shake_requests.read().copied().collect::<Vec<_>>();
    for (
        camera_entity,
        mut view_info,
        mut rig,
        mut debug,
        mut runtime,
        field_of_view_override,
        owner,
    ) in &mut cameras
    {
        let Some((
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
            definition_handle,
            animation_frame,
        )) = owner.map_or_else(
            || targets.single().ok(),
            |owner| targets.get(owner.character).ok(),
        )
        else {
            continue;
        };
        if animation_frame.is_some_and(|frame| !frame.active) {
            debug.valid = false;
            continue;
        }
        let definition = definition_handle.and_then(|handle| definitions.get(&handle.0));
        let Some(params) = definition
            .map(|value| &value.settings)
            .or_else(|| params_assets.iter().next().map(|(_, value)| value))
        else {
            continue;
        };
        let Some(sockets) = socket_locations.filter(|locations| locations.valid) else {
            continue;
        };
        if let Some(request) = requests
            .iter()
            .find(|request| request.target == target_entity)
        {
            begin_camera_shake(&mut runtime, request.scale);
        }
        let delta_seconds = animation_frame.map_or_else(
            || {
                camera_delta_seconds(
                    &mut runtime,
                    params,
                    &virtual_time,
                    &real_time,
                    custom_time_dilation.map_or(1.0, |dilation| dilation.0),
                )
            },
            |frame| frame.delta_seconds,
        );
        let base = movement_base.and_then(|base| {
            if !base.has_relative_rotation {
                return None;
            }
            movement_base_transforms
                .get(base.entity)
                .ok()
                .map(|transform| CameraBaseFrame {
                    location: transform.translation(),
                    rotation: transform.rotation(),
                })
        });
        if runtime.movement_base != movement_base.map(|base| base.entity) {
            if let Some(frame) = base {
                runtime.pivot_lag_location_base_space = frame
                    .rotation
                    .inverse()
                    .mul_vec3(runtime.pivot_lag_location - frame.location);
                runtime.camera_rotation_base_space =
                    frame.rotation.inverse() * runtime.camera_rotation;
            } else {
                runtime.pivot_lag_location_base_space = Vec3::ZERO;
                runtime.camera_rotation_base_space = Quat::IDENTITY;
            }
            runtime.movement_base = movement_base.map(|base| base.entity);
        }
        let target_rotation = Quat::from_euler(
            EulerRot::YXZ,
            view_yaw.0,
            view_pitch.map_or(0.0, |pitch| pitch.0),
            0.0,
        );
        let pivot_target = (sockets.root + sockets.head) * 0.5;
        let allow_lag = !animation_frame.is_some_and(|frame| frame.force_no_lag)
            && (params.teleport_distance_threshold <= 0.0
                || runtime.previous_pivot_target.distance_squared(pivot_target)
                    <= params.teleport_distance_threshold.powi(2));
        runtime.previous_pivot_target = pivot_target;
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
        let fov_curve = curves.map_or(0.0, |values| values.get(AlsCameraCurve::FovOffset))
            + fov_offset_adapter.map_or(0.0, |offset| offset.0);
        let mut debug_curve_values =
            curves.map_or([0.0; CAMERA_CURVE_COUNT], |values| values.values);
        debug_curve_values[AlsCameraCurve::FovOffset.index()] = fov_curve;
        if full_first_person {
            runtime.pivot_lag_location = pivot_target;
            runtime.camera_rotation = target_rotation;
            let fov = field_of_view_for_override(
                first_person_override,
                fov_curve,
                field_of_view_override.map(|value| value.0),
                params,
            );
            let (shake_location, shake_rotation_degrees) =
                camera_shake_sample(&mut runtime, delta_seconds);
            view_info.field_of_view_deg = fov;
            view_info.location = sockets.first_person + shake_location;
            view_info.rotation = target_rotation
                * Quat::from_euler(
                    EulerRot::YXZ,
                    shake_rotation_degrees.y.to_radians(),
                    shake_rotation_degrees.x.to_radians(),
                    shake_rotation_degrees.z.to_radians(),
                );
            view_info.valid = true;
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
                    adjustment: Vec3::ZERO,
                    adjustment_count: 0,
                    adjustment_resolved: false,
                },
                params.trace_radius * sockets.mesh_scale,
                runtime.trace_distance_ratio,
                first_person_override,
                delta_seconds,
                shake_location,
                shake_rotation_degrees,
                debug_curve_values,
            );
            continue;
        }
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
            let lag = Vec3::new(
                curves.map_or(0.0, |values| values.get(AlsCameraCurve::LocationLagX)),
                curves.map_or(0.0, |values| values.get(AlsCameraCurve::LocationLagY)),
                curves.map_or(0.0, |values| values.get(AlsCameraCurve::LocationLagZ)),
            );
            let lag = if animation_frame.is_some() {
                lag
            } else {
                ue_location_lag_to_bevy_camera_space(lag)
            };
            yaw_rotation.mul_vec3(Vec3::new(
                damp_exact(current.x, target.x, delta_seconds, lag.x),
                damp_exact(current.y, target.y, delta_seconds, lag.y),
                damp_exact(current.z, target.z, delta_seconds, lag.z),
            ))
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
        let pivot_values = curve_vector(
            curves,
            AlsCameraCurve::PivotOffsetX,
            AlsCameraCurve::PivotOffsetY,
            AlsCameraCurve::PivotOffsetZ,
        );
        let pivot_offset = sockets.mesh_rotation
            * if animation_frame.is_some() {
                pivot_values * sockets.mesh_scale
            } else {
                ue_mesh_vector_to_bevy_meters(pivot_values, sockets.mesh_scale)
            };
        let pivot = pivot_lag + pivot_offset;
        let camera_values = curve_vector(
            curves,
            AlsCameraCurve::CameraOffsetX,
            AlsCameraCurve::CameraOffsetY,
            AlsCameraCurve::CameraOffsetZ,
        );
        let camera_offset = runtime.camera_rotation
            * if animation_frame.is_some() {
                camera_values * sockets.mesh_scale
            } else {
                ue_camera_vector_to_bevy_meters(camera_values, sockets.mesh_scale)
            };
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
        let (shake_location, shake_rotation_degrees) =
            camera_shake_sample(&mut runtime, delta_seconds);
        view_info.field_of_view_deg = fov;
        view_info.location = trace.location.lerp(
            sockets.first_person,
            if first_person_override > 1e-5 {
                first_person_override
            } else {
                0.0
            },
        ) + shake_location;
        view_info.rotation = runtime.camera_rotation
            * Quat::from_euler(
                EulerRot::YXZ,
                shake_rotation_degrees.y.to_radians(),
                shake_rotation_degrees.x.to_radians(),
                shake_rotation_degrees.z.to_radians(),
            );
        view_info.valid = true;
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
}

fn apply_camera_view(
    mut cameras: Query<
        (
            Entity,
            &mut Transform,
            &mut Projection,
            &mut AlsCameraFieldOfView,
            Option<&AlsCameraViewSource>,
            Option<&AlsCharacterCamera>,
        ),
        With<Camera3d>,
    >,
    views: Query<&AlsCameraViewInfo>,
    definitions: Res<Assets<AlsCameraDefinition>>,
    owners: Query<(
        &GlobalTransform,
        &AlsViewYaw,
        Option<&AlsViewPitch>,
        &als_core::state::Stance,
        &AlsCameraDefinitionHandle,
        &AlsCameraAnimationFrame,
    )>,
) {
    for (entity, mut transform, mut projection, mut field_of_view, source, owner) in &mut cameras {
        if let Some(owner) = owner
            && let Ok((position, yaw, pitch, stance, handle, frame)) = owners.get(owner.character)
            && !frame.active
            && let Some(definition) = definitions.get(&handle.0)
        {
            let height = if *stance == als_core::state::Stance::Crouching {
                definition.crouching_eye_height
            } else {
                definition.standing_eye_height
            };
            transform.translation = position.translation() + Vec3::Y * height;
            transform.rotation = Quat::from_euler(
                EulerRot::YXZ,
                yaw.0,
                pitch.map_or(0.0, |value| value.0),
                0.0,
            );
            set_field_of_view(&mut projection, &mut field_of_view, 90.0);
            continue;
        }
        let Ok(info) = views.get(source.map_or(entity, |source| source.0)) else {
            continue;
        };
        if !info.valid {
            continue;
        }
        transform.translation = info.location;
        transform.rotation = info.rotation;
        set_field_of_view(&mut projection, &mut field_of_view, info.field_of_view_deg);
    }
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
    debug.trace_adjustment = trace.adjustment;
    debug.trace_adjustment_count = trace.adjustment_count;
    debug.trace_adjustment_resolved = trace.adjustment_resolved;
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

    #[test]
    fn detached_mesh_pivot_uses_the_current_capsule_bottom_and_restores_the_socket() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut offsets = Assets::<AlsCameraSocketOffsets>::default();
        let _offsets = offsets.add(AlsCameraSocketOffsets {
            first_person_head_local: Vec3::ZERO,
            trace_shoulder_right_head_local: Vec3::ZERO,
            trace_shoulder_left_head_local: Vec3::ZERO,
        });
        world.insert_resource(offsets);
        let actor_transform = GlobalTransform::from_translation(Vec3::new(4.0, 2.0, -3.0));
        let actor = world
            .spawn((
                actor_transform,
                als_core::character::als_character_capsule(0.9, 0.3),
            ))
            .id();
        let avatar = world
            .spawn((ChildOf(actor), GlobalTransform::IDENTITY))
            .id();
        let mesh_rotation = Quat::from_rotation_y(0.6);
        let mesh = world
            .spawn((
                ChildOf(avatar),
                GlobalTransform::from(Transform::from_rotation(mesh_rotation)),
            ))
            .id();
        let root = world
            .spawn((
                Name::new("root"),
                ChildOf(mesh),
                GlobalTransform::from_translation(Vec3::X),
            ))
            .id();
        world.spawn((
            Name::new("head"),
            ChildOf(root),
            GlobalTransform::from_translation(Vec3::new(4.0, 1.5, -3.0)),
        ));
        world.entity_mut(actor).insert((
            AlsCameraSocketBinding::new(avatar),
            AlsCameraSocketLocations::default(),
        ));
        world
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        assert_eq!(
            world.get::<AlsCameraSocketLocations>(actor).unwrap().root,
            Vec3::X
        );
        world.entity_mut(mesh).remove::<ChildOf>();
        world
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        let locations = world.get::<AlsCameraSocketLocations>(actor).unwrap();
        assert!(locations.root.abs_diff_eq(Vec3::new(4.0, 1.1, -3.0), 1e-6));
        assert!(locations.mesh_rotation.angle_between(mesh_rotation) < 1e-6);
        world
            .entity_mut(actor)
            .insert(als_core::character::als_character_capsule(0.56, 0.3));
        world
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        assert!((world.get::<AlsCameraSocketLocations>(actor).unwrap().root.y - 1.44).abs() < 1e-6);
        world.entity_mut(mesh).insert(ChildOf(avatar));
        world
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        assert_eq!(
            world.get::<AlsCameraSocketLocations>(actor).unwrap().root,
            Vec3::X
        );
    }

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
    fn source_camera_damping_preserves_native_approximation_and_zero_delta() {
        for (delta, half_life, expected) in [
            (0.2, 0.2, 0.50011265),
            (0.2, 0.4, 0.2927782),
            (0.2, 0.8, 0.15927327),
            (0.0, 0.0, 0.0),
            (1e-8, 1e-8, 0.2927782),
            (1.0, 0.1, 0.99132687),
        ] {
            assert!((damp_exact(0.0, 1.0, delta, half_life) - expected).abs() < 1e-7);
        }
    }

    #[test]
    fn source_camera_first_person_weight_uses_native_relevance_boundaries() {
        let mut settings = params();
        settings.first_person_field_of_view_deg = 60.0;
        assert!(!is_full_weight(0.99995));
        assert!(is_full_weight(1.0 - 1e-5));
        assert_eq!(field_of_view_for_override(5e-6, 7.0, None, &settings), 97.0);
        assert!((field_of_view_for_override(0.99995, 7.0, None, &settings) - 67.0015).abs() < 1e-5);
        assert_eq!(
            field_of_view_for_override(1.0 - 1e-5, 7.0, None, &settings),
            60.0
        );
    }

    #[test]
    fn source_camera_rotation_uses_signed_axes_and_counterclockwise_turns() {
        let alpha = 0.1878438;
        for (yaw, expected_delta) in [
            (-174.9_f32, -174.9_f32),
            (-175.1, 184.9),
            (-179.0, 181.0),
            (174.9, 174.9),
            (175.1, 175.1),
            (179.0, 179.0),
            (180.0, 180.0),
            (-180.0, 180.0),
        ] {
            let actual = damp_rotation(
                Quat::IDENTITY,
                Quat::from_rotation_y(yaw.to_radians()),
                0.03,
                0.1,
            );
            let expected = Quat::from_rotation_y((expected_delta * alpha).to_radians());
            assert!(
                (actual * Vec3::NEG_Z).distance(expected * Vec3::NEG_Z) < 1e-5,
                "Yaw {yaw}: {actual:?}"
            );
        }
        let target = Quat::from_euler(
            EulerRot::YXZ,
            -179_f32.to_radians(),
            40_f32.to_radians(),
            -178_f32.to_radians(),
        );
        let actual = damp_rotation(Quat::IDENTITY, target, 0.03, 0.1);
        let expected = Quat::from_euler(
            EulerRot::YXZ,
            (181_f32 * alpha).to_radians(),
            (40_f32 * alpha).to_radians(),
            (182_f32 * alpha).to_radians(),
        );
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            assert!((actual * axis).distance(expected * axis) < 1e-5);
        }
        let current = Quat::from_euler(EulerRot::YXZ, 0.2, -0.1, 0.3);
        assert!(
            (damp_rotation(current, target, 0.0, 0.0) * Vec3::X).distance(current * Vec3::X) < 1e-6
        );
        let near = Quat::from_rotation_y(0.00005_f32.to_radians());
        assert_eq!(damp_rotation(Quat::IDENTITY, near, 0.0, 0.2), near);
    }

    #[test]
    fn source_camera_refreshes_base_before_first_person_without_resetting_caches() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<avian3d::collider_tree::ColliderTrees>();
        world.init_resource::<Assets<AlsCameraDefinition>>();
        world.init_resource::<Messages<AlsCameraShakeRequest>>();
        world.init_resource::<Time<Virtual>>();
        world.init_resource::<Time<Real>>();
        let mut assets = Assets::<AlsCameraParams>::default();
        let _params = assets.add(params());
        world.insert_resource(assets);
        let base_position = Vec3::new(2.0, 0.0, 1.0);
        let base_rotation = Quat::from_rotation_y(0.6);
        let base = world
            .spawn(GlobalTransform::from(
                Transform::from_translation(base_position).with_rotation(base_rotation),
            ))
            .id();
        let owner = world
            .spawn((
                AlsCameraTarget,
                GlobalTransform::IDENTITY,
                RotationMode::ViewDirection,
                AlsViewYaw(0.2),
                AlsViewPitch(0.1),
                AlsCameraAnimationFrame {
                    active: true,
                    delta_seconds: 0.1,
                    force_no_lag: false,
                },
                AlsCameraCurveValues::from_pairs([(AlsCameraCurve::FirstPersonOverride, 1.0)]),
                AlsCameraSocketLocations {
                    head: Vec3::Y * 2.0,
                    first_person: Vec3::Y * 1.8,
                    valid: true,
                    ..default()
                },
                AlsCameraMovementBase {
                    entity: base,
                    has_relative_rotation: true,
                },
            ))
            .id();
        let previous_pivot = Vec3::new(0.4, 1.0, 0.3);
        let previous_rotation = Quat::from_rotation_y(0.3);
        let component = world
            .spawn((
                AlsCameraRig::default(),
                AlsCharacterCamera { character: owner },
                AlsCameraRuntime {
                    pivot_lag_location: previous_pivot,
                    camera_rotation: previous_rotation,
                    trace_distance_ratio: 0.4,
                    ..default()
                },
                AlsCameraViewInfo {
                    valid: true,
                    ..default()
                },
            ))
            .id();
        world.run_system_once(camera_rig_system).unwrap();
        let runtime = *world.get::<AlsCameraRuntime>(component).unwrap();
        assert_eq!(runtime.movement_base, Some(base));
        assert!(
            (runtime.pivot_lag_location_base_space
                - base_rotation.inverse() * (previous_pivot - base_position))
                .length()
                < 1e-6
        );
        assert!(
            (runtime.camera_rotation_base_space * Vec3::X)
                .distance(base_rotation.inverse() * previous_rotation * Vec3::X)
                < 1e-6
        );
        assert_eq!(runtime.trace_distance_ratio, 0.4);
        assert_eq!(
            world
                .get::<AlsCameraDebugInfo>(component)
                .unwrap()
                .trace_distance_ratio,
            0.4
        );
        assert_eq!(runtime.pivot_lag_location, Vec3::Y);
        let view = world.get::<AlsCameraViewInfo>(component).unwrap();
        assert_eq!(view.location, Vec3::Y * 1.8);
        world.entity_mut(base).insert(GlobalTransform::from(
            Transform::from_translation(base_position + Vec3::X)
                .with_rotation(Quat::from_rotation_y(0.9)),
        ));
        world.run_system_once(camera_rig_system).unwrap();
        let next = *world.get::<AlsCameraRuntime>(component).unwrap();
        assert_eq!(
            next.pivot_lag_location_base_space,
            runtime.pivot_lag_location_base_space
        );
        assert_eq!(
            next.camera_rotation_base_space,
            runtime.camera_rotation_base_space
        );
        assert_eq!(next.trace_distance_ratio, 0.4);
        world
            .get_mut::<AlsCameraCurveValues>(owner)
            .unwrap()
            .set(AlsCameraCurve::FirstPersonOverride, 0.0);
        world
            .get_mut::<AlsCameraAnimationFrame>(owner)
            .unwrap()
            .force_no_lag = true;
        world.run_system_once(camera_rig_system).unwrap();
        let next = world.get::<AlsCameraRuntime>(component).unwrap();
        assert!(
            (next.pivot_lag_location_base_space
                - Quat::from_rotation_y(-0.9) * (Vec3::Y - base_position - Vec3::X))
                .length()
                < 1e-6
        );
        world
            .get_mut::<AlsCameraCurveValues>(owner)
            .unwrap()
            .set(AlsCameraCurve::FirstPersonOverride, 5e-6);
        world.run_system_once(camera_rig_system).unwrap();
        assert_eq!(
            world.get::<AlsCameraViewInfo>(component).unwrap().location,
            Vec3::Y
        );
        world
            .get_mut::<AlsCameraCurveValues>(owner)
            .unwrap()
            .set(AlsCameraCurve::FirstPersonOverride, 1.0);
        world.entity_mut(owner).remove::<AlsCameraMovementBase>();
        world.run_system_once(camera_rig_system).unwrap();
        let next = world.get::<AlsCameraRuntime>(component).unwrap();
        assert_eq!(next.movement_base, None);
        assert_eq!(next.pivot_lag_location_base_space, Vec3::ZERO);
        assert_eq!(next.camera_rotation_base_space, Quat::IDENTITY);
    }

    #[test]
    fn location_lag_half_lives_follow_camera_axis_conversion() {
        let bevy = ue_location_lag_to_bevy_camera_space(Vec3::new(0.2, 0.4, 0.8));
        let current = Vec3::ZERO;
        let target = Vec3::ONE;
        let next = damp_ue_location_lag(current, target, 0.2, Vec3::new(0.2, 0.4, 0.8));
        let expected = Vec3::new(0.2927782, 0.15927327, 0.50011265);
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
    fn curve_profile_uses_the_camera_unscaled_delta() {
        let params = params();
        let mut profile = AlsCameraCurveProfileRuntime {
            initialized: true,
            previous_global_time_dilation: 0.2,
            ..default()
        };
        let mut values = AlsCameraCurveValues::default();
        let mut rig_previous_global_time_dilation = 0.2;
        let profile_delta = camera_delta_seconds_from_times(
            &mut profile.previous_global_time_dilation,
            &params,
            0.01,
            0.1,
            0.2,
            0.5,
        );
        let rig_delta = camera_delta_seconds_from_times(
            &mut rig_previous_global_time_dilation,
            &params,
            0.01,
            0.1,
            0.2,
            0.5,
        );
        advance_camera_curve_profile(
            &mut profile,
            &mut values,
            [1.0; CAMERA_CURVE_COUNT],
            profile_delta,
        );
        let passed = (profile_delta - 0.1).abs() <= 1e-6
            && (rig_delta - profile_delta).abs() <= 1e-6
            && (profile.elapsed - 0.1).abs() <= 1e-6
            && (values.values[0] - 0.2).abs() <= 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.54][VERDICT] profile_delta={profile_delta:.6} rig_delta={rig_delta:.6} elapsed={:.6} value={:.6} -> {verdict}",
            profile.elapsed, values.values[0],
        );
        assert!(passed);
    }

    #[test]
    fn first_person_profile_uses_the_desired_rotation_mode() {
        let desired = AlsLastFreeRotationMode(RotationMode::VelocityDirection);
        let first_person =
            camera_profile_rotation_mode(RotationMode::ViewDirection, true, Some(desired));
        let third_person =
            camera_profile_rotation_mode(RotationMode::ViewDirection, false, Some(desired));
        let missing_desired = camera_profile_rotation_mode(RotationMode::Aiming, true, None);
        let passed = first_person == RotationMode::VelocityDirection
            && third_person == RotationMode::ViewDirection
            && missing_desired == RotationMode::Aiming;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.55][VERDICT] first_person={first_person:?} third_person={third_person:?} fallback={missing_desired:?} -> {verdict}",
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
