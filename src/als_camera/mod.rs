use crate::als::character::AlsCollisionLayer;
use crate::extensions::SceneQuery;
use avian3d::prelude::{Collider, Position, Rotation, SimpleCollider, SpatialQueryFilter};
#[path = "notifies/shake.rs"]
mod shake;
#[path = "settings/source_definition.rs"]
pub mod source_definition;
use bevy::asset::{Asset, io::Reader};
use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy::transform::{TransformSystems, systems as transform_systems};
use serde::{Deserialize, Serialize};
pub use shake::AlsCameraShakes;
pub use source_definition::{
    AlsCameraDefinition, CameraPhysicsBody, CameraShakeDefinition, CameraWave,
};

use crate::als::aim::AlsLastFreeRotationMode;
use crate::als::dynamics::AlsMovementBaseDynamicsState;
use crate::als::motion::AlsViewYaw;
use crate::als::state::RotationMode;
use crate::als::view::AlsViewPitch;
use crate::extensions::control_rig_math::rig_damper_alpha as damp_alpha;
use penetration_recovery::sum_penetration_adjustments;

mod penetration_recovery;

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
#[require(crate::extensions::overlay_materials::AvatarMaterialRoot)]
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
    pub camera_cut: bool,
}

#[derive(Component, Debug, Clone, Copy)]
#[require(AlsCameraFieldOfView, AlsCameraShakes)]
pub struct AlsCameraViewSource(pub Entity);

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsCameraSocketBinding {
    pub avatar: Entity,
    pub root_bone: Option<Entity>,
    root_avatar: Option<Entity>,
}

impl AlsCameraSocketBinding {
    pub fn new(avatar: Entity) -> Self {
        Self {
            avatar,
            root_bone: None,
            root_avatar: None,
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

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsCameraTraceOwner(pub Entity);

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsCameraRuntime {
    pub previous_pivot_target: Vec3,
    pub pivot_lag_location: Vec3,
    pub pivot_lag_location_base_space: Vec3,
    pub camera_rotation: Quat,
    pub camera_rotation_base_space: Quat,
    pub trace_distance_ratio: f32,
    pub movement_base: Option<Entity>,
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
            movement_base: None,
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

#[derive(Component, Debug, Clone, Copy, Default, Serialize, PartialEq)]
pub struct AlsCameraDebugInfo {
    pub camera_yaw: f32,
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
        app.init_asset::<AlsCameraDefinition>()
            .init_asset_loader::<source_definition::AlsCameraDefinitionLoader>()
            .add_message::<AlsCameraShakeRequest>()
            .add_systems(
                PostUpdate,
                (
                    resolve_camera_socket_locations.after(TransformSystems::Propagate),
                    camera_movement_base_bridge_system.after(TransformSystems::Propagate),
                    ApplyDeferred,
                    camera_rig_system.run_if(bevy::ecs::schedule::common_conditions::not(
                        bevy::time::common_conditions::paused,
                    )),
                    apply_camera_view,
                    shake::apply_camera_shakes,
                )
                    .chain()
                    .in_set(AlsCameraSet)
                    .before(bevy::camera::CameraUpdateSystems),
            )
            .add_systems(
                PostUpdate,
                (
                    transform_systems::mark_dirty_trees,
                    transform_systems::propagate_parent_transforms,
                    transform_systems::sync_simple_transforms,
                )
                    .chain()
                    .after(shake::apply_camera_shakes)
                    .before(bevy::camera::visibility::VisibilitySystems::UpdateFrusta),
            );
    }
}

pub fn camera_animation_rotation_mode(
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

fn find_named_descendant(
    root: Entity,
    name: &str,
    children: &Query<&Children>,
    names: &Query<&Name, With<bevy::animation::AnimationTargetId>>,
) -> Option<Entity> {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if names
            .get(entity)
            .is_ok_and(|candidate| candidate.as_str().eq_ignore_ascii_case(name))
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
        &AlsCameraDefinitionHandle,
    )>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    names: Query<&Name, With<bevy::animation::AnimationTargetId>>,
    globals: Query<&GlobalTransform>,
    definitions: Res<Assets<AlsCameraDefinition>>,
    instances: Query<&bevy_animation_graph::core::animated_scene::AnimatedSceneInstance>,
    animated_bones: Query<&bevy::animation::AnimatedBy>,
) {
    for (mut binding, mut locations, actor, collider, handle) in &mut targets {
        let Some(definition) = definitions.get(&handle.0) else {
            continue;
        };
        let player = instances
            .get(binding.avatar)
            .ok()
            .map(|instance| instance.player_entity());
        if binding.root_avatar != Some(binding.avatar)
            || player.is_some_and(|player| {
                binding.root_bone.is_some_and(|root| {
                    !animated_bones
                        .get(root)
                        .is_ok_and(|owner| owner.0 == player)
                })
            })
        {
            binding.root_bone = None;
            binding.root_avatar = Some(binding.avatar);
        }
        if binding
            .root_bone
            .is_none_or(|entity| globals.get(entity).is_err())
        {
            binding.root_bone =
                find_named_descendant(player.unwrap_or(binding.avatar), "root", &children, &names);
        }
        let Some(root_bone) = binding.root_bone else {
            locations.valid = false;
            continue;
        };
        let Ok(mesh_parent) = parents.get(root_bone) else {
            continue;
        };
        let mesh_entity = mesh_parent.parent();
        let Ok(mesh) = globals.get(mesh_entity) else {
            continue;
        };
        let location = |name: &str| {
            let socket = definition
                .sockets
                .iter()
                .find(|socket| socket.name.eq_ignore_ascii_case(name));
            let bone_name = socket.map_or(name, |socket| socket.bone.as_str());
            find_named_descendant(root_bone, bone_name, &children, &names)
                .and_then(|bone| globals.get(bone).ok())
                .map_or(mesh.translation(), |bone| {
                    bone.transform_point(socket.map_or(Vec3::ZERO, |socket| socket.location))
                })
        };
        locations.root = if parents.get(mesh_entity).is_err()
            && definition.pivot_sockets[0].eq_ignore_ascii_case("root")
        {
            let bounds = collider.aabb(actor.translation(), actor.rotation(), 0.0);
            actor.translation() - Vec3::Y * (bounds.max.y - bounds.min.y) * 0.5
        } else {
            location(&definition.pivot_sockets[0])
        };
        locations.head = location(&definition.pivot_sockets[1]);
        locations.first_person = location(&definition.first_person_socket);
        locations.trace_shoulder_left = location(&definition.shoulder_sockets[0]);
        locations.trace_shoulder_right = location(&definition.shoulder_sockets[1]);
        let (scale, rotation, _) = mesh.to_scale_rotation_translation();
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

pub fn damp_location_lag(
    current: Vec3,
    target: Vec3,
    delta_seconds: f32,
    half_lives: Vec3,
) -> Vec3 {
    Vec3::new(
        damp_exact(current.x, target.x, delta_seconds, half_lives.x),
        damp_exact(current.y, target.y, delta_seconds, half_lives.y),
        damp_exact(current.z, target.z, delta_seconds, half_lives.z),
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

fn curve_vector(
    curves: &AlsCameraCurveValues,
    x: AlsCameraCurve,
    y: AlsCameraCurve,
    z: AlsCameraCurve,
) -> Vec3 {
    Vec3::new(curves.get(x), curves.get(y), curves.get(z))
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
    spatial: &SceneQuery,
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
    spatial: &SceneQuery,
    colliders: &Query<(&Position, &Rotation, &Collider)>,
    shape: &Collider,
    start: Vec3,
    filter: &SpatialQueryFilter,
    ignored: &Query<(), With<AlsCameraTraceIgnore>>,
) -> (Vec3, u32) {
    let mut adjustments = Vec::new();
    let mut valid = true;
    spatial.shape_intersections_callback(shape, start, Quat::IDENTITY, filter, |entity| {
        if !valid {
            return false;
        }
        if ignored.get(entity).is_ok() {
            return true;
        }
        let contact = colliders
            .get(entity)
            .ok()
            .and_then(|(position, rotation, collider)| {
                avian3d::collision::collider::contact_query::contact(
                    collider,
                    position.0,
                    rotation.0,
                    shape,
                    start,
                    Quat::IDENTITY,
                    0.0,
                )
                .ok()
                .flatten()
                .map(|contact| (contact.global_normal1(rotation.0), contact.penetration))
            });
        let Some((direction, distance)) = contact else {
            valid = false;
            return false;
        };
        if distance > 1e-10 {
            adjustments.push(direction * distance);
        }
        true
    });
    if !valid {
        return (Vec3::ZERO, 0);
    }
    let count = adjustments.len() as u32;
    (sum_penetration_adjustments(adjustments), count)
}

fn trace_with_start_adjustment(
    spatial: &SceneQuery,
    colliders: &Query<(&Position, &Rotation, &Collider)>,
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
    let distance = vector.length();
    let predicate = |entity| ignored.get(entity).is_err();
    if distance > 1e-10 {
        let direction = Dir3::new(vector).unwrap();
        let Some(hit) = spatial
            .cast_source_sphere_predicate(shape, start, direction, distance, filter, &predicate)
        else {
            return result;
        };
        if hit.distance > 0.0 {
            result.blocking_hit = true;
            result.location = start + direction.as_vec3() * hit.distance;
            return result;
        }
    } else if !has_blocking_overlap(spatial, shape, start, filter, ignored) {
        return result;
    }
    let (adjustment, adjustment_count) =
        start_penetration_adjustment(spatial, colliders, adjustment_shape, start, filter, ignored);
    let adjustment_direction = adjustment.normalize_or_zero();
    let owner_offset = owner_location - start;
    let owner_direction = if owner_offset.length_squared() < 1e-12 {
        Vec3::ZERO
    } else {
        owner_offset.normalize()
    };
    if adjustment.length_squared() > 1e-12 && owner_direction.dot(adjustment_direction) >= -1e-4 {
        start += adjustment;
        result.start = start;
        result.adjusted_start = true;
        result.adjustment = adjustment;
        result.adjustment_count = adjustment_count;
        result.adjustment_resolved = !has_blocking_overlap(spatial, shape, start, filter, ignored);
        if result.adjustment_resolved {
            let adjusted_vector = end - start;
            if let Ok((adjusted_direction, adjusted_distance)) =
                Dir3::new_and_length(adjusted_vector)
            {
                if let Some(adjusted_hit) = spatial.cast_source_sphere_predicate(
                    shape,
                    start,
                    adjusted_direction,
                    adjusted_distance,
                    filter,
                    &predicate,
                ) && adjusted_hit.distance > 0.0
                {
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
    spatial: &SceneQuery,
    colliders: &Query<(&Position, &Rotation, &Collider)>,
    start: Vec3,
    end: Vec3,
    owner_location: Vec3,
    mesh_scale: f32,
    allow_lag: bool,
    params: &AlsCameraParams,
    filter: &SpatialQueryFilter,
    ignored: &Query<(), With<AlsCameraTraceIgnore>>,
    delta_seconds: f32,
) -> TraceResult {
    let shape = Collider::sphere(params.trace_radius * mesh_scale);
    let adjustment_shape = Collider::sphere(
        (params.trace_radius + TRACE_START_ADJUSTMENT_PADDING_METERS) * mesh_scale,
    );
    let mut trace = trace_with_start_adjustment(
        spatial,
        colliders,
        &shape,
        &adjustment_shape,
        start,
        end,
        owner_location,
        filter,
        ignored,
    );
    trace.end = trace.location;
    if !allow_lag || !params.trace_distance_smoothing {
        runtime.trace_distance_ratio = 1.0;
        return trace;
    }
    let vector = end - trace.start;
    let distance = vector.length();
    if distance <= 1e-6 {
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
                has_relative_rotation: false,
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
    view_aspect_ratio: f32,
) {
    match projection {
        Projection::Perspective(perspective) => {
            perspective.fov =
                vertical_fov_radians_from_horizontal_degrees(value_degrees, view_aspect_ratio);
        }
        _ => {
            let mut perspective = PerspectiveProjection::default();
            perspective.fov =
                vertical_fov_radians_from_horizontal_degrees(value_degrees, view_aspect_ratio);
            *projection = Projection::Perspective(perspective);
        }
    }
    field_of_view.0 = value_degrees;
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
            &AlsCharacterCamera,
            &AlsCameraCurveValues,
            &AlsCameraAnimationFrame,
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
            &AlsCameraSocketLocations,
            Option<&AlsCameraMovementBase>,
            Has<crate::als::input_request::AlsCameraLeftShoulder>,
            &AlsCameraDefinitionHandle,
        ),
        With<AlsCameraTarget>,
    >,
    spatial: SceneQuery,
    colliders: Query<(&Position, &Rotation, &Collider)>,
    definitions: Res<Assets<AlsCameraDefinition>>,
    ignored: Query<(), With<AlsCameraTraceIgnore>>,
    children: Query<&Children>,
    trace_owners: Query<(Entity, &AlsCameraTraceOwner)>,
    movement_base_transforms: Query<&GlobalTransform>,
) {
    for (
        camera_entity,
        mut view_info,
        mut rig,
        mut debug,
        mut runtime,
        field_of_view_override,
        owner,
        curves,
        animation_frame,
    ) in &mut cameras
    {
        view_info.camera_cut = false;
        let Ok((
            target_entity,
            target_transform,
            rotation_mode,
            view_yaw,
            view_pitch,
            socket_locations,
            movement_base,
            left_shoulder,
            definition_handle,
        )) = targets.get(owner.character)
        else {
            continue;
        };
        if !animation_frame.active {
            debug.valid = false;
            view_info.valid = false;
            continue;
        }
        let Some(definition) = definitions.get(&definition_handle.0) else {
            continue;
        };
        let params = &definition.settings;
        if !socket_locations.valid {
            continue;
        }
        let sockets = socket_locations;
        let delta_seconds = animation_frame.delta_seconds;
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
        let allow_lag = !animation_frame.force_no_lag
            && (params.teleport_distance_threshold <= 0.0
                || runtime.previous_pivot_target.distance_squared(pivot_target)
                    <= params.teleport_distance_threshold.powi(2));
        view_info.camera_cut = !view_info.valid || !allow_lag;
        runtime.previous_pivot_target = pivot_target;
        let first_person_override = curves
            .get(AlsCameraCurve::FirstPersonOverride)
            .clamp(0.0, 1.0);
        let full_first_person = is_full_weight(first_person_override);
        rig.mode = if full_first_person {
            AlsCameraMode::FirstPerson
        } else {
            mode_for_rotation(*rotation_mode)
        };
        let fov_curve = curves.get(AlsCameraCurve::FovOffset);
        let debug_curve_values = curves.values;
        if full_first_person {
            runtime.pivot_lag_location = pivot_target;
            runtime.camera_rotation = target_rotation;
            let fov = field_of_view_for_override(
                first_person_override,
                fov_curve,
                field_of_view_override.map(|value| value.0),
                params,
            );
            view_info.field_of_view_deg = fov;
            view_info.location = sockets.first_person;
            view_info.rotation = target_rotation;
            view_info.valid = true;
            debug.camera_yaw = target_rotation.to_euler(EulerRot::YXZ).0;
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
                Vec3::ZERO,
                Vec3::ZERO,
                debug_curve_values,
            );
            continue;
        }
        if let Some(frame) = base {
            runtime.camera_rotation = frame.rotation * runtime.camera_rotation_base_space;
        }
        let rotation_lag = curves.get(AlsCameraCurve::RotationLag);
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
                curves.get(AlsCameraCurve::LocationLagX),
                curves.get(AlsCameraCurve::LocationLagY),
                curves.get(AlsCameraCurve::LocationLagZ),
            );
            yaw_rotation * damp_location_lag(current, target, delta_seconds, lag)
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
        let pivot_offset = sockets.mesh_rotation * (pivot_values * sockets.mesh_scale);
        let pivot = pivot_lag + pivot_offset;
        let camera_values = curve_vector(
            curves,
            AlsCameraCurve::CameraOffsetX,
            AlsCameraCurve::CameraOffsetY,
            AlsCameraCurve::CameraOffsetZ,
        );
        let camera_offset = runtime.camera_rotation * (camera_values * sockets.mesh_scale);
        let camera_target = pivot + camera_offset;
        let shoulder = if left_shoulder {
            sockets.trace_shoulder_left
        } else {
            sockets.trace_shoulder_right
        };
        let trace_override = curves.get(AlsCameraCurve::TraceOverride).clamp(0.0, 1.0);
        let trace_start = shoulder.lerp(
            pivot_target + pivot_offset + params.trace_override_offset,
            trace_override,
        );
        let mut excluded = collect_descendants(target_entity, &children);
        excluded.extend(collect_descendants(camera_entity, &children));
        for (root, trace_owner) in &trace_owners {
            if trace_owner.0 == target_entity {
                excluded.extend(collect_descendants(root, &children));
            }
        }
        let filter = SpatialQueryFilter::from_excluded_entities(excluded)
            .with_mask(AlsCollisionLayer::Visibility);
        let trace = calculate_camera_trace(
            &mut runtime,
            &spatial,
            &colliders,
            trace_start,
            camera_target,
            target_transform.translation(),
            sockets.mesh_scale,
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
        view_info.field_of_view_deg = fov;
        view_info.location = trace.location.lerp(
            sockets.first_person,
            if first_person_override > 1e-5 {
                first_person_override
            } else {
                0.0
            },
        );
        view_info.rotation = runtime.camera_rotation;
        view_info.valid = true;
        debug.camera_yaw = camera_yaw;
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
            Vec3::ZERO,
            Vec3::ZERO,
            debug_curve_values,
        );
    }
}

fn apply_camera_view(
    mut cameras: Query<
        (
            &mut Transform,
            &mut Projection,
            &mut AlsCameraFieldOfView,
            Ref<AlsCameraViewSource>,
            &AlsCharacterCamera,
            Option<&mut bevy::anti_alias::taa::TemporalAntiAliasing>,
        ),
        With<Camera3d>,
    >,
    views: Query<&AlsCameraViewInfo>,
    definitions: Res<Assets<AlsCameraDefinition>>,
    owners: Query<(
        &GlobalTransform,
        &AlsViewYaw,
        Option<&AlsViewPitch>,
        &crate::als::state::Stance,
        &AlsCameraDefinitionHandle,
    )>,
    frames: Query<&AlsCameraAnimationFrame>,
) {
    for (mut transform, mut projection, mut field_of_view, source, owner, taa) in &mut cameras {
        let Ok((position, yaw, pitch, stance, handle)) = owners.get(owner.character) else {
            continue;
        };
        let Ok(frame) = frames.get(source.0) else {
            continue;
        };
        let Some(definition) = definitions.get(&handle.0) else {
            continue;
        };
        if let Some(mut taa) = taa {
            taa.reset |= source.is_changed()
                || frame.force_no_lag
                || views.get(source.0).is_ok_and(|view| view.camera_cut);
        }
        if !frame.active {
            let height = if *stance == crate::als::state::Stance::Crouching {
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
            transform.scale = Vec3::ONE;
            set_field_of_view(
                &mut projection,
                &mut field_of_view,
                90.0,
                definition.view_aspect_ratio,
            );
            continue;
        }
        let Ok(info) = views.get(source.0) else {
            continue;
        };
        if !info.valid {
            continue;
        }
        transform.translation = info.location;
        transform.rotation = info.rotation;
        transform.scale = Vec3::ONE;
        set_field_of_view(
            &mut projection,
            &mut field_of_view,
            info.field_of_view_deg,
            definition.view_aspect_ratio,
        );
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
    use bevy::time::{Real, Virtual};

    #[test]
    fn movement_base_presence_does_not_enable_relative_camera_rotation() {
        use bevy::ecs::system::RunSystemOnce;
        for rotation in [Quat::IDENTITY, Quat::from_rotation_y(0.5)] {
            let mut world = World::new();
            let base = world
                .spawn(GlobalTransform::from(Transform::from_rotation(rotation)))
                .id();
            let character = world
                .spawn((
                    AlsCameraTarget,
                    AlsMovementBaseDynamicsState {
                        entity: Some(base),
                        ..default()
                    },
                ))
                .id();
            world
                .run_system_once(camera_movement_base_bridge_system)
                .unwrap();
            let camera_base = world.get::<AlsCameraMovementBase>(character).unwrap();
            assert_eq!(camera_base.entity, base);
            assert!(!camera_base.has_relative_rotation);
        }
    }

    #[test]
    fn source_cameras_keep_independent_frame_and_view_validity() {
        use crate::camera_component::{AlsSourceCameraComponent, tests::loaded_camera_scene};
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::animated_scene::{
            AnimatedSceneHandle, AnimatedSceneInstance,
        };
        let (mut app, character, primary, scene) = loaded_camera_scene();
        let secondary = app
            .world_mut()
            .spawn((
                Name::new("RetainedCamera"),
                AlsCharacterCamera { character },
                AlsSourceCameraComponent::default(),
                AnimatedSceneHandle::new(scene),
                ChildOf(character),
            ))
            .id();
        for _ in 0..20 {
            app.update();
            if app
                .world()
                .get::<AnimatedSceneInstance>(secondary)
                .is_some()
            {
                break;
            }
        }
        assert!(
            app.world()
                .get::<AnimatedSceneInstance>(secondary)
                .is_some()
        );
        for (active, inactive) in [(primary, secondary), (secondary, primary)] {
            app.world_mut()
                .get_mut::<AlsSourceCameraComponent>(active)
                .unwrap()
                .activate(true);
            app.world_mut()
                .get_mut::<AlsSourceCameraComponent>(inactive)
                .unwrap()
                .deactivate();
            app.update();
            assert!(
                app.world()
                    .get::<crate::als_camera::AlsCameraViewInfo>(active)
                    .unwrap()
                    .valid
            );
            assert!(
                !app.world()
                    .get::<crate::als_camera::AlsCameraViewInfo>(inactive)
                    .unwrap()
                    .valid
            );
        }
        app.world_mut()
            .get_mut::<AlsSourceCameraComponent>(primary)
            .unwrap()
            .activate(true);
        app.update();
        for (source, offset) in [(primary, 10.0), (secondary, -10.0)] {
            app.world_mut()
                .entity_mut(source)
                .insert(AlsCameraCurveValues::from_pairs([(
                    crate::als_camera::AlsCameraCurve::FovOffset,
                    offset,
                )]));
        }
        app.world_mut()
            .run_system_once(crate::als_camera::camera_rig_system)
            .unwrap();
        let primary_view = app
            .world()
            .get::<crate::als_camera::AlsCameraViewInfo>(primary)
            .unwrap();
        let secondary_view = app
            .world()
            .get::<crate::als_camera::AlsCameraViewInfo>(secondary)
            .unwrap();
        assert!((primary_view.field_of_view_deg - 100.0).abs() < 1e-5);
        assert!((secondary_view.field_of_view_deg - 80.0).abs() < 1e-5);
        for (source, expected) in [(primary, 100.0), (secondary, 80.0)] {
            let view = app
                .world_mut()
                .spawn((
                    Camera3d::default(),
                    Transform::IDENTITY,
                    AlsCameraViewSource(source),
                    AlsCharacterCamera { character },
                ))
                .id();
            app.world_mut().run_system_once(apply_camera_view).unwrap();
            assert!(
                (app.world().get::<AlsCameraFieldOfView>(view).unwrap().0 - expected).abs() < 1e-5
            );
        }
        app.world_mut()
            .entity_mut(secondary)
            .remove::<AnimatedSceneInstance>();
        app.update();
        assert!(app.world().get::<AlsCameraViewInfo>(primary).unwrap().valid);
        assert!(
            !app.world()
                .get::<AlsCameraViewInfo>(secondary)
                .unwrap()
                .valid
        );
    }

    #[test]
    fn source_camera_requires_explicit_owner_settings_and_animation_frame() {
        use bevy::ecs::system::RunSystemOnce;
        for (has_owner, has_definition, has_frame) in [
            (false, true, true),
            (true, false, true),
            (true, true, false),
        ] {
            let mut world = World::new();
            world.init_resource::<avian3d::collider_tree::ColliderTrees>();
            world.init_resource::<Time<Virtual>>();
            world.init_resource::<Time<Real>>();
            let mut loose_settings = Assets::<AlsCameraParams>::default();
            let _settings = loose_settings.add(params());
            world.insert_resource(loose_settings);
            let mut definitions = Assets::<AlsCameraDefinition>::default();
            let definition = definitions.add(
                ron::from_str::<AlsCameraDefinition>(include_str!(
                    "../../assets/ALSCamera/Data/als_camera.camera_component.ron"
                ))
                .unwrap(),
            );
            world.insert_resource(definitions);
            let owner = world
                .spawn((
                    AlsCameraTarget,
                    GlobalTransform::IDENTITY,
                    RotationMode::ViewDirection,
                    AlsViewYaw(0.0),
                    AlsCameraCurveValues::from_pairs([(AlsCameraCurve::FirstPersonOverride, 1.0)]),
                    AlsCameraSocketLocations {
                        first_person: Vec3::Y * 1.8,
                        valid: true,
                        ..default()
                    },
                ))
                .id();
            if has_definition {
                world
                    .entity_mut(owner)
                    .insert(AlsCameraDefinitionHandle(definition));
            }
            if has_frame {
                world.entity_mut(owner).insert(AlsCameraAnimationFrame {
                    active: true,
                    ..default()
                });
            }
            let component = world.spawn(AlsCameraRig::default()).id();
            if has_owner {
                world
                    .entity_mut(component)
                    .insert(AlsCharacterCamera { character: owner });
            }
            if let Some(frame) = world.entity_mut(owner).take::<AlsCameraAnimationFrame>() {
                world.entity_mut(component).insert(frame);
            }
            if let Some(curves) = world.entity_mut(owner).take::<AlsCameraCurveValues>() {
                world.entity_mut(component).insert(curves);
            }
            world.run_system_once(camera_rig_system).unwrap();
            assert!(
                !world.get::<AlsCameraViewInfo>(component).unwrap().valid,
                "Missing source camera inputs were accepted: owner={has_owner}, definition={has_definition}, frame={has_frame}"
            );
        }
    }

    #[test]
    fn browser_review_parent_camera_shake_and_fallback_keep_the_world_view() {
        for parented in [false, true] {
            let tolerance = if parented { 1e-5 } else { 2e-7 };
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins,
                AssetPlugin::default(),
                bevy::mesh::MeshPlugin,
                TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate),
                AlsCameraPlugin,
            ))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(100),
            ));
            app.world_mut()
                .resource_mut::<Time<Virtual>>()
                .set_relative_speed(0.2);
            let mut definition: AlsCameraDefinition = ron::from_str(include_str!(
                "../../assets/ALSCamera/Data/als_camera.camera_component.ron"
            ))
            .unwrap();
            for wave in definition
                .sprint_shake
                .location
                .iter_mut()
                .chain(definition.sprint_shake.rotation_yxz.iter_mut())
                .chain(std::iter::once(&mut definition.sprint_shake.field_of_view))
            {
                wave.random_start = false;
            }
            let handle = app
                .world_mut()
                .resource_mut::<Assets<AlsCameraDefinition>>()
                .add(definition);
            let character = app
                .world_mut()
                .spawn((
                    AlsCameraTarget,
                    Transform::IDENTITY,
                    RotationMode::ViewDirection,
                    crate::als::state::Stance::Standing,
                    AlsViewYaw(0.7),
                    AlsViewPitch(0.2),
                    AlsCameraDefinitionHandle(handle),
                    AlsCameraAnimationFrame {
                        delta_seconds: 0.1,
                        active: true,
                        force_no_lag: true,
                    },
                    AlsCameraCurveValues::from_pairs([(AlsCameraCurve::FirstPersonOverride, 1.0)]),
                    AlsCameraSocketLocations {
                        head: Vec3::Y * 2.0,
                        first_person: Vec3::Y * 1.8,
                        valid: true,
                        ..default()
                    },
                ))
                .id();
            let component = app
                .world_mut()
                .spawn((AlsCameraRig::default(), AlsCharacterCamera { character }))
                .id();
            if let Some(frame) = app
                .world_mut()
                .entity_mut(character)
                .take::<AlsCameraAnimationFrame>()
            {
                app.world_mut().entity_mut(component).insert(frame);
            }
            if let Some(curves) = app
                .world_mut()
                .entity_mut(character)
                .take::<AlsCameraCurveValues>()
            {
                app.world_mut().entity_mut(component).insert(curves);
            }
            let parent = app
                .world_mut()
                .spawn(
                    Transform::from_xyz(10.0, 1.0, -2.0)
                        .with_rotation(Quat::from_rotation_y(0.6))
                        .with_scale(Vec3::splat(1.5)),
                )
                .id();
            let view = app
                .world_mut()
                .spawn((
                    Camera3d::default(),
                    Transform::IDENTITY,
                    AlsCameraViewSource(component),
                    AlsCharacterCamera { character },
                ))
                .id();
            if parented {
                app.world_mut().entity_mut(view).insert(ChildOf(parent));
                app.add_systems(
                    PostUpdate,
                    (move |mut transforms: Query<&mut Transform>| {
                        transforms.get_mut(parent).unwrap().translation.x += 0.1;
                    })
                    .after(camera_rig_system)
                    .before(apply_camera_view),
                );
            }
            app.finish();
            app.cleanup();
            app.update();
            app.update();
            let component_view = *app.world().get::<AlsCameraViewInfo>(component).unwrap();
            for scale in [0.4, 0.7] {
                app.world_mut()
                    .write_message(AlsCameraShakeRequest::sprint(character, scale));
            }
            app.update();
            let unchanged = app.world().get::<AlsCameraViewInfo>(component).unwrap();
            assert_eq!(
                unchanged.location, component_view.location,
                "Camera component view must exclude manager shakes"
            );
            assert_eq!(unchanged.rotation, component_view.rotation);
            let mut expected = Transform::from_translation(component_view.location)
                .with_rotation(component_view.rotation);
            let offset = Vec3::new(0.01, 0.01, -0.01) * (std::f32::consts::TAU * 0.02).sin() * 0.2;
            let angle = 0.1_f32.to_radians() * (std::f32::consts::TAU * 0.1 * 0.02).sin() * 0.2;
            for scale in [0.4, 0.7] {
                expected.translation += expected.rotation * offset * scale;
                expected.rotation *=
                    Quat::from_euler(EulerRot::YXZ, -angle * scale, angle * scale, -angle * scale);
            }
            let actual = if parented {
                app.world()
                    .get::<GlobalTransform>(view)
                    .unwrap()
                    .compute_transform()
            } else {
                *app.world().get::<Transform>(view).unwrap()
            };
            assert!(
                actual.translation.distance(expected.translation) < tolerance,
                "Expected {}, got {}",
                expected.translation,
                actual.translation
            );
            assert!(
                (actual.rotation * Vec3::NEG_Z).distance(expected.rotation * Vec3::NEG_Z)
                    < tolerance
            );
            assert!(actual.translation.distance(component_view.location) > 1e-5);
            app.world_mut()
                .get_mut::<AlsCameraAnimationFrame>(component)
                .unwrap()
                .active = false;
            app.update();
            assert_eq!(
                app.world()
                    .get::<AlsCameraViewInfo>(component)
                    .unwrap()
                    .location,
                component_view.location
            );
            assert!(
                app.world()
                    .get::<GlobalTransform>(view)
                    .unwrap()
                    .translation()
                    .distance(Vec3::Y * 0.64)
                    > 1e-5
            );
            for _ in 0..12 {
                app.update();
            }
            assert!(
                app.world()
                    .get::<GlobalTransform>(view)
                    .unwrap()
                    .translation()
                    .distance(Vec3::Y * 0.64)
                    < tolerance
            );
            assert!(
                app.world()
                    .get::<GlobalTransform>(view)
                    .unwrap()
                    .compute_transform()
                    .scale
                    .abs_diff_eq(Vec3::ONE, tolerance),
                "Parented camera changed the world view scale"
            );
            if parented {
                let local_before = *app.world().get::<Transform>(view).unwrap();
                let frame_before = *app
                    .world()
                    .get::<AlsCameraAnimationFrame>(component)
                    .unwrap();
                app.world_mut()
                    .entity_mut(component)
                    .remove::<AlsCameraAnimationFrame>();
                app.update();
                let local_after = *app.world().get::<Transform>(view).unwrap();
                assert!(
                    local_after.translation.distance(local_before.translation) < 1e-6,
                    "A missing world view was converted from parent space twice"
                );
                assert!(local_after.scale.abs_diff_eq(local_before.scale, 1e-6));
                app.world_mut().entity_mut(component).insert(frame_before);
                app.world_mut().entity_mut(character).remove::<AlsViewYaw>();
                app.update();
                assert!(
                    app.world()
                        .get::<Transform>(view)
                        .unwrap()
                        .translation
                        .distance(local_before.translation)
                        < 1e-6,
                    "An incomplete view owner was converted from parent space twice"
                );
            }
        }
    }

    fn trace_fixture(
        walls: &[(Vec3, Vec3)],
        start: Vec3,
        end: Vec3,
        owner: Vec3,
        scale: f32,
        smoothing: bool,
        previous_ratio: f32,
    ) -> (TraceResult, f32) {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            TransformPlugin,
            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
        ));
        app.finish();
        app.cleanup();
        for &(position, size) in walls {
            app.world_mut().spawn((
                avian3d::prelude::RigidBody::Static,
                Collider::cuboid(size.x, size.y, size.z),
                Transform::from_translation(position),
            ));
        }
        app.update();
        app.update();
        app.world_mut()
            .run_system_once(
                move |spatial: SceneQuery,
                      colliders: Query<(&Position, &Rotation, &Collider)>,
                      ignored: Query<(), With<AlsCameraTraceIgnore>>| {
                    let mut runtime = AlsCameraRuntime {
                        trace_distance_ratio: previous_ratio,
                        ..default()
                    };
                    let mut settings = params();
                    settings.trace_distance_smoothing = smoothing;
                    let result = calculate_camera_trace(
                        &mut runtime,
                        &spatial,
                        &colliders,
                        start,
                        end,
                        owner,
                        scale,
                        true,
                        &settings,
                        &SpatialQueryFilter::default(),
                        &ignored,
                        0.2,
                    );
                    (result, runtime.trace_distance_ratio)
                },
            )
            .unwrap()
    }

    #[test]
    fn zero_joint_ragdoll_completion_excludes_self_from_camera_queries() {
        use crate::character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar};
        use avian3d::prelude::CollisionLayers;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::core::{
            animated_scene::AnimatedSceneHandle,
            animation_graph_player::AnimationGraphPlayer,
            ragdoll::{
                definition::{BodyMode, ColliderShape, Ragdoll},
                spawning::spawn_ragdoll_avian,
            },
        };
        for penetrating in [false, true] {
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins,
                AssetPlugin::default(),
                bevy::mesh::MeshPlugin,
                TransformPlugin,
                bevy::gizmos::GizmoPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate),
                bevy_animation_graph::AnimationGraphPlugin::default(),
            ))
            .init_asset::<WorldAsset>()
            .init_asset::<AlsCameraDefinition>()
            .add_observer(crate::ragdoll_link::register_ragdoll_camera_trace_owner);
            app.finish();
            app.cleanup();
            let mut settings: AlsCameraDefinition = ron::from_str(include_str!(
                "../../assets/ALSCamera/Data/als_camera.camera_component.ron"
            ))
            .unwrap();
            settings.settings.trace_distance_smoothing = false;
            let definition = app
                .world_mut()
                .resource_mut::<Assets<AlsCameraDefinition>>()
                .add(settings);
            let owner = app
                .world_mut()
                .spawn((
                    AlsCameraTarget,
                    GlobalTransform::IDENTITY,
                    RotationMode::ViewDirection,
                    AlsViewYaw(0.0),
                    AlsCameraSocketLocations {
                        valid: true,
                        ..default()
                    },
                    AlsCameraDefinitionHandle(definition),
                ))
                .id();
            let avatar = app
                .world_mut()
                .spawn((
                    AlsCharacterAvatar { character: owner },
                    AnimatedSceneHandle::default(),
                ))
                .id();
            let player = app
                .world_mut()
                .spawn((AnimationGraphPlayer::new(default()), ChildOf(avatar)))
                .id();
            app.world_mut().trigger(WorldInstanceReady {
                entity: avatar,
                instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
            });
            app.world_mut().flush();
            app.world_mut()
                .entity_mut(owner)
                .insert(AlsCharacterAssembly {
                    avatar,
                    camera: None,
                });
            let camera = app
                .world_mut()
                .spawn((
                    AlsCameraRig::default(),
                    AlsCharacterCamera { character: owner },
                    AlsCameraCurveValues::from_pairs([(AlsCameraCurve::CameraOffsetZ, 2.0)]),
                    AlsCameraAnimationFrame {
                        active: true,
                        force_no_lag: true,
                        delta_seconds: 0.1,
                    },
                ))
                .id();
            let point = if penetrating {
                Vec3::new(-0.1, 0.0, 0.0)
            } else {
                Vec3::Z
            };
            let mut asset: Ragdoll =
                ron::from_str(include_str!("../../assets/ALS/Character/PA_Als.rag.ron")).unwrap();
            asset.joints.clear();
            asset.bodies.retain(|_, body| body.label == "pelvis");
            let body = asset.bodies.values_mut().next().unwrap();
            body.offset = Vec3::ZERO;
            body.default_mode = BodyMode::Kinematic;
            body.colliders.truncate(1);
            let collider_id = body.colliders[0];
            asset.colliders.retain(|id, _| *id == collider_id);
            let collider = asset.colliders.get_mut(&collider_id).unwrap();
            collider.local_offset = bevy::math::Isometry3d::IDENTITY;
            collider.shape = ColliderShape::Cuboid(bevy::shape::Cuboid::new(0.1, 0.6, 0.6));
            let spawned = app
                .world_mut()
                .run_system_once(move |mut commands: Commands| {
                    spawn_ragdoll_avian(
                        player,
                        &asset,
                        bevy::math::Isometry3d::from_translation(point),
                        None,
                        &mut commands,
                    )
                })
                .unwrap();
            app.world_mut().flush();
            assert_eq!(
                app.world()
                    .get::<AlsCameraTraceOwner>(spawned.root)
                    .unwrap()
                    .0,
                owner
            );
            app.world_mut().clear_trackers();
            app.world_mut().increment_change_tick();
            app.world_mut().trigger(
                bevy_animation_graph::core::ragdoll::spawning::RagdollSpawned {
                    ragdoll: spawned.root,
                    animation_player: player,
                },
            );
            app.world_mut().flush();
            assert!(
                !app.world()
                    .entity(spawned.root)
                    .get_ref::<AlsCameraTraceOwner>()
                    .unwrap()
                    .is_changed()
            );
            for collider in spawned.colliders.values() {
                app.world_mut()
                    .entity_mut(*collider)
                    .insert(CollisionLayers::new(
                        AlsCollisionLayer::Visibility,
                        avian3d::prelude::LayerMask::ALL,
                    ));
            }
            let other = app
                .world_mut()
                .spawn((
                    avian3d::prelude::RigidBody::Static,
                    Collider::cuboid(0.1, 0.6, 0.6),
                    Transform::from_translation(point),
                    CollisionLayers::NONE,
                ))
                .id();
            for blocked in [false, true, false] {
                app.world_mut().entity_mut(other).insert(if blocked {
                    CollisionLayers::new(
                        AlsCollisionLayer::Visibility,
                        avian3d::prelude::LayerMask::ALL,
                    )
                } else {
                    CollisionLayers::NONE
                });
                app.update();
                app.update();
                app.world_mut().run_system_once(camera_rig_system).unwrap();
                let debug = app.world().get::<AlsCameraDebugInfo>(camera).unwrap();
                assert_eq!(
                    debug.trace_blocking_hit, blocked,
                    "Self ragdoll exclusion failed, penetrating={penetrating}: {debug:?}"
                );
                assert_eq!(debug.trace_adjusted_start, blocked && penetrating);
            }
            app.world_mut().despawn(spawned.root);
            app.world_mut().flush();
            assert!(
                spawned
                    .colliders
                    .values()
                    .all(|entity| !app.world().entities().contains(*entity))
            );
        }
    }

    #[test]
    fn camera_collision_layers_control_sweep_and_penetration() {
        use avian3d::prelude::{CollisionLayers, LayerMask};
        use bevy::ecs::system::RunSystemOnce;
        for penetrating in [false, true] {
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins,
                AssetPlugin::default(),
                bevy::mesh::MeshPlugin,
                TransformPlugin,
                avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            ))
            .init_asset::<AlsCameraDefinition>();
            app.finish();
            app.cleanup();
            let obstacle = app.world_mut().spawn(Transform::IDENTITY).id();
            let collider = app
                .world_mut()
                .spawn((
                    avian3d::prelude::RigidBody::Static,
                    Collider::cuboid(0.1, 0.6, 0.6),
                    Transform::from_translation(if penetrating {
                        Vec3::new(-0.1, 0.0, 0.0)
                    } else {
                        Vec3::Z
                    }),
                    ChildOf(obstacle),
                ))
                .id();
            app.update();
            app.update();
            let mut settings: AlsCameraDefinition = ron::from_str(include_str!(
                "../../assets/ALSCamera/Data/als_camera.camera_component.ron"
            ))
            .unwrap();
            settings.settings.trace_distance_smoothing = false;
            let definition = app
                .world_mut()
                .resource_mut::<Assets<AlsCameraDefinition>>()
                .add(settings);
            let owner = app
                .world_mut()
                .spawn((
                    AlsCameraTarget,
                    GlobalTransform::IDENTITY,
                    RotationMode::ViewDirection,
                    AlsViewYaw(0.0),
                    AlsCameraCurveValues::from_pairs([(AlsCameraCurve::CameraOffsetZ, 2.0)]),
                    AlsCameraSocketLocations {
                        root: Vec3::ZERO,
                        head: Vec3::ZERO,
                        first_person: Vec3::ZERO,
                        trace_shoulder_left: Vec3::ZERO,
                        trace_shoulder_right: Vec3::ZERO,
                        valid: true,
                        ..default()
                    },
                    AlsCameraDefinitionHandle(definition.clone()),
                    AlsCameraAnimationFrame {
                        active: true,
                        force_no_lag: true,
                        delta_seconds: 0.1,
                    },
                ))
                .id();
            let camera = app
                .world_mut()
                .spawn((
                    AlsCameraRig::default(),
                    AlsCharacterCamera { character: owner },
                ))
                .id();
            if let Some(frame) = app
                .world_mut()
                .entity_mut(owner)
                .take::<AlsCameraAnimationFrame>()
            {
                app.world_mut().entity_mut(camera).insert(frame);
            }
            if let Some(curves) = app
                .world_mut()
                .entity_mut(owner)
                .take::<AlsCameraCurveValues>()
            {
                app.world_mut().entity_mut(camera).insert(curves);
            }
            for (memberships, expected) in [
                (LayerMask::from(AlsCollisionLayer::Default), false),
                (
                    LayerMask::from([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility]),
                    true,
                ),
                (LayerMask::from(AlsCollisionLayer::Visibility), true),
                (LayerMask::NONE, false),
            ] {
                app.world_mut()
                    .entity_mut(collider)
                    .insert(CollisionLayers::new(memberships, LayerMask::ALL));
                app.update();
                app.world_mut().run_system_once(camera_rig_system).unwrap();
                let debug = app.world().get::<AlsCameraDebugInfo>(camera).unwrap();
                assert_eq!(
                    debug.trace_blocking_hit, expected,
                    "{memberships:?}, penetrating={penetrating}: {debug:?}"
                );
                assert_eq!(debug.trace_adjusted_start, expected && penetrating);
                if !expected {
                    assert_eq!(debug.trace_adjustment_count, 0);
                    assert!(
                        app.world()
                            .get::<AlsCameraViewInfo>(camera)
                            .unwrap()
                            .location
                            .abs_diff_eq(Vec3::Z * 2.0, 1e-5)
                    );
                }
            }
        }
    }

    #[test]
    fn source_camera_trace_scales_all_penetration_padding_and_checks_stationary_sweeps() {
        for scale in [2.0, 0.5, 1.0] {
            let walls = [
                (
                    Vec3::new(-0.1, 0.0, 0.0) * scale,
                    Vec3::new(0.05, 0.6, 0.6) * scale,
                ),
                (
                    Vec3::new(0.0, -0.1, 0.0) * scale,
                    Vec3::new(0.6, 0.05, 0.6) * scale,
                ),
            ];
            for end in [Vec3::Z * 2.0 * scale, Vec3::ZERO] {
                let (trace, _) =
                    trace_fixture(&walls, Vec3::ZERO, end, Vec3::ZERO, scale, false, 1.0);
                assert!(trace.adjusted_start, "Scale {scale}, end {end}");
                assert!(trace.adjustment_resolved);
                assert_eq!(trace.adjustment_count, 2);
                assert!(
                    trace
                        .adjustment
                        .distance(Vec3::new(0.085, 0.085, 0.0) * scale)
                        < 2e-6,
                    "Scale {scale}: {}",
                    trace.adjustment
                );
                assert!(trace.location.is_finite());
            }
        }
    }

    #[test]
    fn source_camera_touching_start_uses_the_padded_penetration_retry() {
        let walls = [(Vec3::new(0.0, 0.0, 0.25), Vec3::new(4.0, 4.0, 0.25))];
        let (trace, _) = trace_fixture(
            &walls,
            Vec3::ZERO,
            Vec3::Z,
            Vec3::NEG_Z,
            5.0 / 6.0,
            false,
            1.0,
        );
        assert!(trace.blocking_hit);
        assert!(trace.adjusted_start && trace.adjustment_resolved);
        assert_eq!(trace.adjustment_count, 1);
        assert!(trace.start.abs_diff_eq(Vec3::NEG_Z * 0.00833333, 1e-6));
        assert!(trace.location.abs_diff_eq(Vec3::ZERO, 1e-6));
    }

    #[test]
    fn source_camera_trace_distinguishes_close_hits_from_initial_penetration() {
        let walls = [(Vec3::new(0.0, 0.0, 0.160005), Vec3::new(4.0, 4.0, 0.02))];
        let (trace, _) = trace_fixture(&walls, Vec3::ZERO, Vec3::Z, Vec3::NEG_Z, 1.0, false, 1.0);
        assert!(trace.blocking_hit);
        assert!(!trace.adjusted_start);
        assert_eq!(trace.adjustment_count, 0);
        assert!(
            (trace.location.z - 0.0000050067907).abs() < 1e-7,
            "{}",
            trace.location
        );
    }

    #[test]
    fn source_camera_trace_preserves_failed_adjustments_and_source_direction_tolerance() {
        let opposing = [
            (Vec3::new(-0.145, 0.0, 0.0), Vec3::new(0.05, 4.0, 4.0)),
            (Vec3::new(0.14502, 0.0, 0.0), Vec3::new(0.05, 4.0, 4.0)),
        ];
        let (trace, _) = trace_fixture(&opposing, Vec3::ZERO, Vec3::Z, Vec3::X, 1.0, false, 1.0);
        assert!(trace.adjusted_start);
        assert!(!trace.adjustment_resolved);
        assert_eq!(trace.location, trace.start);
        assert!(
            trace.start.distance(Vec3::X * 0.00002) < 2e-7,
            "{}",
            trace.start
        );
        let wall = [(Vec3::new(-0.1, 0.0, 0.0), Vec3::new(0.05, 4.0, 4.0))];
        let (trace, _) = trace_fixture(
            &wall,
            Vec3::ZERO,
            Vec3::Z,
            Vec3::new(-0.00005, 1.0, 0.0),
            1.0,
            false,
            1.0,
        );
        assert!(trace.adjusted_start);
        assert!(trace.adjustment_resolved);
    }

    #[test]
    fn source_camera_trace_smoothing_resets_tiny_distances_and_contracts_immediately() {
        let (trace, ratio) =
            trace_fixture(&[], Vec3::ZERO, Vec3::Z * 5e-7, Vec3::ZERO, 1.0, true, 0.4);
        assert_eq!(ratio, 1.0);
        assert_eq!(trace.location, Vec3::Z * 5e-7);
        let (trace, ratio) =
            trace_fixture(&[], Vec3::ZERO, Vec3::Z * 2.0, Vec3::ZERO, 1.0, true, 0.4);
        assert!((ratio - 0.7000676).abs() < 1e-7);
        assert!((trace.location.z - 2.0 * ratio).abs() < 1e-7);
        assert_eq!(trace.end, Vec3::Z * 2.0);
        let wall = [(Vec3::new(0.0, 0.0, 1.0), Vec3::new(4.0, 4.0, 0.2))];
        let (trace, ratio) =
            trace_fixture(&wall, Vec3::ZERO, Vec3::Z * 2.0, Vec3::ZERO, 1.0, true, 0.9);
        assert!((ratio - 0.375).abs() < 1e-6);
        assert!((trace.location.z - 0.75).abs() < 1e-6);
        assert!(
            (trace.end.z - 0.75).abs() < 1e-6,
            "Debug sweep must end at the unsmoothed collision result"
        );
    }

    #[test]
    fn camera_recovery_fixture_requires_a_sweep_blocker_beyond_the_mtd_pair() {
        let wall = (Vec3::new(0.0, 0.1, 0.0), Vec3::new(0.6, 0.05, 0.6));
        let (trace, ratio) = trace_fixture(
            &[wall, wall],
            Vec3::ZERO,
            Vec3::Z * 2.0,
            Vec3::NEG_Y,
            1.0,
            true,
            1.0,
        );
        assert_eq!(trace.adjustment_count, 2);
        assert!(trace.adjustment_resolved);
        assert!(!trace.blocking_hit);
        assert_eq!(ratio, 1.0);
        let blocker = (Vec3::Z, Vec3::new(0.6, 0.6, 0.05));
        let (trace, blocked) = trace_fixture(
            &[wall, wall, blocker],
            Vec3::ZERO,
            Vec3::Z * 2.0,
            Vec3::NEG_Y,
            1.0,
            true,
            1.0,
        );
        assert_eq!(trace.adjustment_count, 2);
        assert!(trace.adjustment_resolved && trace.blocking_hit);
        assert!(blocked < 0.98);
        let (trace, recovered) = trace_fixture(
            &[],
            Vec3::ZERO,
            Vec3::Z * 2.0,
            Vec3::NEG_Y,
            1.0,
            true,
            blocked,
        );
        assert!(!trace.blocking_hit);
        assert!(recovered > blocked + 0.02);
    }

    #[test]
    fn camera_rebinds_root_when_player_changes_on_retained_avatar() {
        use bevy::{
            ecs::system::RunSystemOnce,
            world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner},
        };
        use bevy_animation_graph::core::{
            animated_scene::AnimatedSceneHandle, animation_graph_player::AnimationGraphPlayer,
        };
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy_animation_graph::AnimationGraphPlugin::default(),
        ));
        let mut definitions = Assets::<AlsCameraDefinition>::default();
        let definition = definitions.add(
            ron::from_str::<AlsCameraDefinition>(include_str!(
                "../../assets/ALSCamera/Data/als_camera.camera_component.ron"
            ))
            .unwrap(),
        );
        app.world_mut().insert_resource(definitions);
        let actor = app
            .world_mut()
            .spawn((
                GlobalTransform::IDENTITY,
                crate::als::character::als_character_capsule(0.9, 0.3),
            ))
            .id();
        let avatar = app
            .world_mut()
            .spawn((AnimatedSceneHandle::default(), ChildOf(actor)))
            .id();
        app.world_mut().entity_mut(actor).insert((
            AlsCameraSocketBinding::new(avatar),
            AlsCameraSocketLocations::default(),
            AlsCameraDefinitionHandle(definition),
        ));
        let spawn_mesh = |app: &mut App, location: Vec3| {
            let player = app
                .world_mut()
                .spawn((
                    AnimationGraphPlayer::new(default()),
                    ChildOf(avatar),
                    GlobalTransform::from_translation(location),
                ))
                .id();
            let root = app
                .world_mut()
                .spawn((
                    Name::new("root"),
                    bevy::animation::AnimationTargetId::from_names([Name::new("root")].iter()),
                    bevy::animation::AnimatedBy(player),
                    ChildOf(player),
                    GlobalTransform::from_translation(location),
                ))
                .id();
            app.world_mut().trigger(WorldInstanceReady {
                entity: avatar,
                instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
            });
            app.world_mut().flush();
            (player, root)
        };
        let (old_player, old_root) = spawn_mesh(&mut app, Vec3::X);
        app.world_mut()
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        assert_eq!(
            app.world()
                .get::<AlsCameraSocketBinding>(actor)
                .unwrap()
                .root_bone,
            Some(old_root)
        );
        app.world_mut().entity_mut(old_player).remove::<ChildOf>();
        let (player, root) = spawn_mesh(&mut app, Vec3::Z * 2.0);
        app.world_mut()
            .entity_mut(old_player)
            .insert(ChildOf(avatar));
        app.world_mut()
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        assert!(app.world().entities().contains(old_root));
        assert_eq!(
            app.world()
                .get::<AlsCameraSocketBinding>(actor)
                .unwrap()
                .avatar,
            avatar
        );
        assert_eq!(
            app.world()
                .get::<AlsCameraSocketBinding>(actor)
                .unwrap()
                .root_bone,
            Some(root)
        );
        assert_eq!(
            app.world()
                .get::<AlsCameraSocketLocations>(actor)
                .unwrap()
                .root,
            Vec3::Z * 2.0
        );
        app.world_mut().entity_mut(player).remove::<ChildOf>();
        app.world_mut()
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        assert_eq!(
            app.world()
                .get::<AlsCameraSocketBinding>(actor)
                .unwrap()
                .root_bone,
            Some(root)
        );
        assert!(
            app.world()
                .get::<AlsCameraSocketLocations>(actor)
                .unwrap()
                .valid
        );
    }

    #[test]
    fn camera_rebinds_root_when_avatar_changes_and_old_root_survives() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut definitions = Assets::<AlsCameraDefinition>::default();
        let definition = definitions.add(
            ron::from_str::<AlsCameraDefinition>(include_str!(
                "../../assets/ALSCamera/Data/als_camera.camera_component.ron"
            ))
            .unwrap(),
        );
        world.insert_resource(definitions);
        let actor = world
            .spawn((
                GlobalTransform::IDENTITY,
                crate::als::character::als_character_capsule(0.9, 0.3),
            ))
            .id();
        let mut spawn_avatar = |location: Vec3| {
            let avatar = world.spawn(ChildOf(actor)).id();
            let mesh = world
                .spawn((ChildOf(avatar), GlobalTransform::from_translation(location)))
                .id();
            let root = world
                .spawn((
                    Name::new("root"),
                    bevy::animation::AnimationTargetId::from_names([Name::new("root")].iter()),
                    ChildOf(mesh),
                    GlobalTransform::from_translation(location),
                ))
                .id();
            (avatar, root)
        };
        let (old_avatar, old_root) = spawn_avatar(Vec3::X);
        let new_avatar = world.spawn(ChildOf(actor)).id();
        let new_mesh = world
            .spawn((
                ChildOf(new_avatar),
                GlobalTransform::from_translation(Vec3::Z * 2.0),
            ))
            .id();
        world.entity_mut(actor).insert((
            AlsCameraSocketBinding::new(old_avatar),
            AlsCameraSocketLocations::default(),
            AlsCameraDefinitionHandle(definition),
        ));
        world
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        assert_eq!(
            world
                .get::<AlsCameraSocketBinding>(actor)
                .unwrap()
                .root_bone,
            Some(old_root)
        );
        world
            .get_mut::<AlsCameraSocketBinding>(actor)
            .unwrap()
            .avatar = new_avatar;
        world
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        assert!(world.entities().contains(old_root));
        assert_eq!(
            world
                .get::<AlsCameraSocketBinding>(actor)
                .unwrap()
                .root_bone,
            None
        );
        assert!(!world.get::<AlsCameraSocketLocations>(actor).unwrap().valid);
        let new_root = world
            .spawn((
                Name::new("root"),
                bevy::animation::AnimationTargetId::from_names([Name::new("root")].iter()),
                ChildOf(new_mesh),
                GlobalTransform::from_translation(Vec3::Z * 2.0),
            ))
            .id();
        world
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        assert_eq!(
            world
                .get::<AlsCameraSocketBinding>(actor)
                .unwrap()
                .root_bone,
            Some(new_root)
        );
        assert!(world.get::<AlsCameraSocketLocations>(actor).unwrap().valid);
        assert_eq!(
            world.get::<AlsCameraSocketLocations>(actor).unwrap().root,
            Vec3::Z * 2.0
        );
    }

    #[test]
    fn detached_mesh_pivot_uses_the_current_capsule_bottom_and_restores_the_socket() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut definitions = Assets::<AlsCameraDefinition>::default();
        let definition = definitions.add(
            ron::from_str::<AlsCameraDefinition>(include_str!(
                "../../assets/ALSCamera/Data/als_camera.camera_component.ron"
            ))
            .unwrap(),
        );
        world.insert_resource(definitions);
        let actor_transform = GlobalTransform::from_translation(Vec3::new(4.0, 2.0, -3.0));
        let actor = world
            .spawn((
                actor_transform,
                crate::als::character::als_character_capsule(0.9, 0.3),
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
                bevy::animation::AnimationTargetId::from_names([Name::new("root")].iter()),
                ChildOf(mesh),
                GlobalTransform::from_translation(Vec3::X),
            ))
            .id();
        world.spawn((
            Name::new("head"),
            bevy::animation::AnimationTargetId::from_names(
                [Name::new("root"), Name::new("head")].iter(),
            ),
            ChildOf(root),
            GlobalTransform::from_translation(Vec3::new(4.0, 1.5, -3.0)),
        ));
        world.entity_mut(actor).insert((
            AlsCameraSocketBinding::new(avatar),
            AlsCameraSocketLocations::default(),
            AlsCameraDefinitionHandle(definition.clone()),
        ));
        world
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        assert_eq!(
            world.get::<AlsCameraSocketLocations>(actor).unwrap().root,
            Vec3::X
        );
        assert!(
            world
                .get::<AlsCameraSocketLocations>(actor)
                .unwrap()
                .first_person
                .distance(Vec3::new(4.04, 1.5, -2.86))
                < 1e-6
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
            .insert(crate::als::character::als_character_capsule(0.56, 0.3));
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
        {
            let mut definitions = world.resource_mut::<Assets<AlsCameraDefinition>>();
            let mut settings = definitions.get_mut(&definition).unwrap();
            settings.pivot_sockets = ["head".into(), "root".into()];
            settings.first_person_socket = "ROOT".into();
            settings.shoulder_sockets = ["HEAD".into(), "missing".into()];
        }
        world.spawn((
            Name::new("missing"),
            ChildOf(root),
            GlobalTransform::from_translation(Vec3::splat(20.0)),
        ));
        world.entity_mut(mesh).remove::<ChildOf>();
        world
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        let locations = world.get::<AlsCameraSocketLocations>(actor).unwrap();
        assert_eq!(locations.root, Vec3::new(4.0, 1.5, -3.0));
        assert_eq!(locations.head, Vec3::X);
        assert_eq!(locations.first_person, Vec3::X);
        assert_eq!(locations.trace_shoulder_left, Vec3::new(4.0, 1.5, -3.0));
        assert_eq!(locations.trace_shoulder_right, Vec3::ZERO);
        {
            let mut definitions = world.resource_mut::<Assets<AlsCameraDefinition>>();
            let mut settings = definitions.get_mut(&definition).unwrap();
            settings.first_person_socket = "FirstPersonCamera".into();
            settings
                .sockets
                .iter_mut()
                .find(|socket| socket.name == "FirstPersonCamera")
                .unwrap()
                .bone = "missing".into();
        }
        world
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        assert_eq!(
            world
                .get::<AlsCameraSocketLocations>(actor)
                .unwrap()
                .first_person,
            Vec3::ZERO
        );
        {
            let mut definitions = world.resource_mut::<Assets<AlsCameraDefinition>>();
            let mut settings = definitions.get_mut(&definition).unwrap();
            settings.first_person_socket = "ROOT".into();
            settings.pivot_sockets[0] = "root".into();
            settings.sockets.push(source_definition::CameraSocket {
                name: "root".into(),
                bone: "head".into(),
                location: Vec3::X,
            });
        }
        world
            .run_system_once(resolve_camera_socket_locations)
            .unwrap();
        let locations = world.get::<AlsCameraSocketLocations>(actor).unwrap();
        assert_eq!(locations.first_person, Vec3::new(5.0, 1.5, -3.0));
        assert!((locations.root.y - 1.44).abs() < 1e-6);
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
    fn field_of_view_preserves_source_vertical_constraint_across_aspect_ratios() {
        let mut projection = Projection::Perspective(PerspectiveProjection {
            aspect_ratio: 4.0 / 3.0,
            ..default()
        });
        let mut field_of_view = AlsCameraFieldOfView::default();
        set_field_of_view(&mut projection, &mut field_of_view, 90.0, 1.33333);
        let first_vertical = match projection {
            Projection::Perspective(ref perspective) => perspective.fov,
            _ => unreachable!(),
        };
        if let Projection::Perspective(ref mut perspective) = projection {
            perspective.aspect_ratio = 16.0 / 9.0;
        }
        set_field_of_view(&mut projection, &mut field_of_view, 90.0, 1.33333);
        let second_vertical = match projection {
            Projection::Perspective(ref perspective) => perspective.fov,
            _ => unreachable!(),
        };
        assert_eq!(second_vertical, first_vertical);
        assert_eq!(field_of_view.0, 90.0);
        assert!(
            (horizontal_fov_degrees_from_vertical_radians(second_vertical, 1.33333) - 90.0).abs()
                < 1e-4
        );
    }

    #[test]
    fn render_projection_uses_current_fov_viewport_and_final_transform() {
        use bevy::camera::{CameraProjection, primitives::Frustum};
        use bevy::window::{PrimaryWindow, WindowCreated, WindowResized, WindowScaleFactorChanged};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
            .init_asset::<Image>()
            .init_resource::<bevy::render::texture::ManualTextureViews>()
            .init_resource::<avian3d::collider_tree::ColliderTrees>()
            .add_message::<WindowCreated>()
            .add_message::<WindowResized>()
            .add_message::<WindowScaleFactorChanged>()
            .add_plugins((
                bevy::render::camera::CameraPlugin,
                bevy::camera::CameraProjectionPlugin,
                AlsCameraPlugin,
            ));
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let definition = app
            .world_mut()
            .resource_mut::<Assets<AlsCameraDefinition>>()
            .add(
                ron::from_str::<AlsCameraDefinition>(include_str!(
                    "../../assets/ALSCamera/Data/als_camera.camera_component.ron"
                ))
                .unwrap(),
            );
        let owner = app
            .world_mut()
            .spawn((
                GlobalTransform::IDENTITY,
                AlsViewYaw(0.0),
                crate::als::state::Stance::Standing,
                AlsCameraDefinitionHandle(definition),
                AlsCameraAnimationFrame {
                    active: true,
                    ..default()
                },
            ))
            .id();
        let source = app
            .world_mut()
            .spawn(AlsCameraViewInfo {
                location: Vec3::new(3.0, 2.0, 5.0),
                rotation: Quat::from_rotation_y(0.7),
                field_of_view_deg: 90.0,
                valid: true,
                ..default()
            })
            .id();
        if let Some(frame) = app
            .world_mut()
            .entity_mut(owner)
            .take::<AlsCameraAnimationFrame>()
        {
            app.world_mut().entity_mut(source).insert(frame);
        }
        if let Some(curves) = app
            .world_mut()
            .entity_mut(owner)
            .take::<AlsCameraCurveValues>()
        {
            app.world_mut().entity_mut(source).insert(curves);
        }
        let view = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Projection::Perspective(PerspectiveProjection {
                    near: 0.17,
                    far: 500.0,
                    ..default()
                }),
                AlsCameraViewSource(source),
                AlsCharacterCamera { character: owner },
            ))
            .id();
        for (width, height, fov, viewport) in [
            (1600, 900, 90.0, None),
            (900, 1200, 65.0, None),
            (1600, 900, 105.0, Some(UVec2::new(800, 600))),
        ] {
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .resolution
                .set_physical_resolution(width, height);
            app.world_mut().write_message(WindowResized {
                window,
                width: width as f32,
                height: height as f32,
            });
            app.world_mut().get_mut::<Camera>(view).unwrap().viewport =
                viewport.map(|physical_size| bevy::camera::Viewport {
                    physical_size,
                    ..default()
                });
            let mut info = app
                .world_mut()
                .get_mut::<AlsCameraViewInfo>(source)
                .unwrap();
            info.field_of_view_deg = fov;
            info.location.x += 1.0;
            let expected_transform =
                Transform::from_translation(info.location).with_rotation(info.rotation);
            app.update();
            let size = viewport.unwrap_or(UVec2::new(width, height)).as_vec2();
            let expected = PerspectiveProjection {
                fov: vertical_fov_radians_from_horizontal_degrees(fov, 1.33333),
                aspect_ratio: size.x / size.y,
                near: 0.17,
                far: 500.0,
                ..default()
            };
            let actual = app.world().get::<Camera>(view).unwrap().clip_from_view();
            assert!(
                actual.abs_diff_eq(expected.get_clip_from_view(), 1e-5),
                "Current projection: {actual:?}"
            );
            let global = app.world().get::<GlobalTransform>(view).unwrap();
            assert!(
                global
                    .translation()
                    .abs_diff_eq(expected_transform.translation, 1e-5)
            );
            let frustum = app.world().get::<Frustum>(view).unwrap();
            let expected_frustum =
                expected.compute_frustum(&GlobalTransform::from(expected_transform));
            for (actual, expected) in frustum.half_spaces.iter().zip(expected_frustum.half_spaces) {
                assert!(actual.normal_d().abs_diff_eq(expected.normal_d(), 1e-5));
            }
        }
        let info = *app.world().get::<AlsCameraViewInfo>(source).unwrap();
        let frame = *app.world().get::<AlsCameraAnimationFrame>(source).unwrap();
        let replacement = app.world_mut().spawn((info, frame)).id();
        app.world_mut().entity_mut(view).insert((
            AlsCameraViewSource(replacement),
            bevy::anti_alias::taa::TemporalAntiAliasing { reset: false },
        ));
        app.update();
        assert!(
            app.world()
                .get::<bevy::anti_alias::taa::TemporalAntiAliasing>(view)
                .unwrap()
                .reset
        );
        app.world_mut()
            .get_mut::<bevy::anti_alias::taa::TemporalAntiAliasing>(view)
            .unwrap()
            .reset = false;
        app.update();
        assert!(
            !app.world()
                .get::<bevy::anti_alias::taa::TemporalAntiAliasing>(view)
                .unwrap()
                .reset
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
        let definition = world.resource_mut::<Assets<AlsCameraDefinition>>().add(
            ron::from_str::<AlsCameraDefinition>(include_str!(
                "../../assets/ALSCamera/Data/als_camera.camera_component.ron"
            ))
            .unwrap(),
        );
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
                AlsCameraDefinitionHandle(definition),
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
        if let Some(frame) = world.entity_mut(owner).take::<AlsCameraAnimationFrame>() {
            world.entity_mut(component).insert(frame);
        }
        if let Some(curves) = world.entity_mut(owner).take::<AlsCameraCurveValues>() {
            world.entity_mut(component).insert(curves);
        }
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
            .get_mut::<AlsCameraCurveValues>(component)
            .unwrap()
            .set(AlsCameraCurve::FirstPersonOverride, 0.0);
        world
            .get_mut::<AlsCameraAnimationFrame>(component)
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
            .get_mut::<AlsCameraCurveValues>(component)
            .unwrap()
            .set(AlsCameraCurve::FirstPersonOverride, 5e-6);
        world.run_system_once(camera_rig_system).unwrap();
        assert_eq!(
            world.get::<AlsCameraViewInfo>(component).unwrap().location,
            Vec3::Y
        );
        world
            .get_mut::<AlsCameraCurveValues>(component)
            .unwrap()
            .set(AlsCameraCurve::FirstPersonOverride, 1.0);
        world.entity_mut(owner).remove::<AlsCameraMovementBase>();
        world.run_system_once(camera_rig_system).unwrap();
        let next = world.get::<AlsCameraRuntime>(component).unwrap();
        assert_eq!(next.movement_base, None);
        assert_eq!(next.pivot_lag_location_base_space, Vec3::ZERO);
        assert_eq!(next.camera_rotation_base_space, Quat::IDENTITY);

        world.entity_mut(owner).insert(AlsCameraMovementBase {
            entity: base,
            has_relative_rotation: false,
        });
        world
            .get_mut::<AlsCameraCurveValues>(component)
            .unwrap()
            .set(AlsCameraCurve::FirstPersonOverride, 0.0);
        world
            .get_mut::<AlsCameraAnimationFrame>(component)
            .unwrap()
            .force_no_lag = false;
        let absolute_view = Quat::from_euler(EulerRot::YXZ, 0.2, 0.1, 0.0);
        for yaw in [1.2, 1.5] {
            world.entity_mut(base).insert(GlobalTransform::from(
                Transform::from_translation(base_position)
                    .with_rotation(Quat::from_rotation_y(yaw)),
            ));
            world.run_system_once(camera_rig_system).unwrap();
            let runtime = world.get::<AlsCameraRuntime>(component).unwrap();
            assert_eq!(runtime.movement_base, Some(base));
            assert_eq!(runtime.pivot_lag_location_base_space, Vec3::ZERO);
            assert_eq!(runtime.camera_rotation_base_space, Quat::IDENTITY);
            assert!(runtime.camera_rotation.dot(absolute_view).abs() > 0.9999);
        }
    }

    #[test]
    fn location_lag_half_lives_follow_camera_axis_conversion() {
        let half_lives = Vec3::new(0.4, 0.8, 0.2);
        let current = Vec3::ZERO;
        let target = Vec3::ONE;
        let next = damp_location_lag(current, target, 0.2, half_lives);
        let expected = Vec3::new(0.2927782, 0.15927327, 0.50011265);
        let passed = next.distance(expected) <= 1e-6;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.52][VERDICT] bevy_half_lives=({:.3},{:.3},{:.3}) response=({:.6},{:.6},{:.6}) -> {verdict}",
            half_lives.x, half_lives.y, half_lives.z, next.x, next.y, next.z,
        );
        assert!(passed);
    }

    #[test]
    fn first_person_animation_uses_the_desired_rotation_mode() {
        let desired = AlsLastFreeRotationMode(RotationMode::VelocityDirection);
        let first_person =
            camera_animation_rotation_mode(RotationMode::ViewDirection, true, Some(desired));
        let third_person =
            camera_animation_rotation_mode(RotationMode::ViewDirection, false, Some(desired));
        let missing_desired = camera_animation_rotation_mode(RotationMode::Aiming, true, None);
        let passed = first_person == RotationMode::VelocityDirection
            && third_person == RotationMode::ViewDirection
            && missing_desired == RotationMode::Aiming;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.55][VERDICT] first_person={first_person:?} third_person={third_person:?} fallback={missing_desired:?} -> {verdict}",
        );
        assert!(passed);
    }
}
