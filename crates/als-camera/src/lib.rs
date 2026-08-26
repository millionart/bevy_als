//! ALSCamera equivalent: spring arm with collision shrink, shoulder offsets, tri-mode camera driven by view yaw/pitch.

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::asset::{Asset, AssetLoader, AssetServer, io::Reader};
use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use als_core::motion::AlsViewYaw;
use als_core::state::RotationMode;
use als_core::view::AlsViewSmoothed;

#[derive(Asset, Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AlsCameraParams {
    pub third_person_shoulder: Vec3,
    pub third_person_arm_length: f32,
    pub aiming_shoulder: Vec3,
    pub aiming_arm_length: f32,
    pub first_person_eye_height: f32,
    pub probe_skin: f32,
    pub aim_blend_speed: f32,
    pub pivot_height: f32,
}

#[derive(Default, bevy::reflect::TypePath)]
pub struct AlsCameraParamsLoader;

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
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(CameraLoaderError::Io)?;
        ron::de::from_bytes::<AlsCameraParams>(&bytes)
            .map_err(CameraLoaderError::Ron)
    }

    fn extensions(&self) -> &[&str] {
        &["camera.ron"]
    }
}

#[derive(Debug)]
pub enum CameraLoaderError {
    Io(std::io::Error),
    Ron(ron::de::SpannedError),
}

impl std::fmt::Display for CameraLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CameraLoaderError::Io(e) => write!(f, "io error: {e}"),
            CameraLoaderError::Ron(e) => write!(f, "ron error: {e}"),
        }
    }
}

impl std::error::Error for CameraLoaderError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlsCameraMode {
    #[default]
    ThirdPerson,
    AimingShoulder,
    FirstPersonPlaceholder,
}

pub fn mode_for_rotation(rotation_mode: RotationMode) -> AlsCameraMode {
    match rotation_mode {
        RotationMode::Aiming => AlsCameraMode::AimingShoulder,
        _ => AlsCameraMode::ThirdPerson,
    }
}

pub fn spring_arm_length(max_len: f32, hit_distance: Option<f32>, skin: f32) -> f32 {
    match hit_distance {
        None => max_len,
        Some(d) => (d - skin).clamp(0.05, max_len),
    }
}

pub fn blended_shoulder(blend: f32, p: &AlsCameraParams) -> Vec3 {
    p.third_person_shoulder.lerp(p.aiming_shoulder, blend.clamp(0.0, 1.0))
}

pub fn blended_arm_length(blend: f32, p: &AlsCameraParams) -> f32 {
    p.third_person_arm_length
        .lerp(p.aiming_arm_length, blend.clamp(0.0, 1.0))
}

fn move_toward(current: f32, target: f32, max_delta: f32) -> f32 {
    let diff = target - current;
    if diff.abs() <= max_delta {
        target
    } else {
        current + diff.signum() * max_delta
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsCameraRig {
    pub mode: AlsCameraMode,
    pub aim_blend: f32,
}

impl Default for AlsCameraRig {
    fn default() -> Self {
        Self {
            mode: AlsCameraMode::ThirdPerson,
            aim_blend: 0.0,
        }
    }
}

#[derive(Component)]
pub struct AlsCameraTarget;

pub const CAMERA_CURVE_NAMES: [&str; 10] = [
    "CameraOffsetX",
    "CameraOffsetY",
    "CameraOffsetZ",
    "FirstPersonOverride",
    "LocationLagX",
    "LocationLagY",
    "LocationLagZ",
    "PivotOffsetZ",
    "RotationLag",
    "TraceOverride",
];

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
    pub valid: bool,
    pub curve_values: [f32; CAMERA_CURVE_NAMES.len()],
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

impl Plugin for AlsCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<AlsCameraParams>()
            .register_asset_loader(AlsCameraParamsLoader)
            .add_systems(Update, load_camera_params)
            .add_systems(Update, camera_rig_system);
    }
}

fn load_camera_params(
    server: Res<AssetServer>,
    mut done: Local<bool>,
    mut handle: Local<Option<bevy::asset::Handle<AlsCameraParams>>>,
) {
    if *done {
        return;
    }
    let h = handle.get_or_insert_with(|| {
        server.load::<AlsCameraParams>("params/als_camera.camera.ron")
    });
    let h = h.clone();
    match server.load_state(h.id()) {
        bevy::asset::LoadState::Loaded => {
            info!("[CAM] params loaded");
            *done = true;
        }
        bevy::asset::LoadState::Failed(e) => {
            error!("[CAM] params FAILED: {e}");
            *done = true;
        }
        _ => {}
    }
}

#[allow(clippy::type_complexity)]
fn camera_rig_system(
    mut q_target: Query<
        (
            Entity,
            &GlobalTransform,
            &RotationMode,
            &AlsViewYaw,
            Option<&AlsViewSmoothed>,
            Has<als_core::input_request::AlsFirstPersonView>,
            Has<als_core::input_request::AlsCameraLeftShoulder>,
        ),
        With<AlsCameraTarget>,
    >,
    mut q_cam: Query<(Entity, &mut Transform, &mut AlsCameraRig, Option<&mut AlsCameraDebugInfo>), Without<AlsCameraTarget>>,
    spatial: SpatialQuery,
    params: Res<Assets<AlsCameraParams>>,
    time: Res<Time>,
) {
    let Some((_, p)) = params.iter().next() else {
        return;
    };
    let Ok((
        char_entity,
        char_gtf,
        rotation_mode,
        view_yaw,
        view_smoothed,
        first_person,
        left_shoulder,
    )) = q_target.single_mut()
    else {
        return;
    };
    let Ok((cam_entity, mut cam_tf, mut rig, mut debug_info)) = q_cam.single_mut() else {
        return;
    };

    let dt = time.delta_secs();
    let mut new_mode = mode_for_rotation(*rotation_mode);
    if first_person {
        new_mode = AlsCameraMode::FirstPersonPlaceholder;
    }
    let target_blend = match new_mode {
        AlsCameraMode::AimingShoulder => 1.0,
        _ => 0.0,
    };
    rig.mode = new_mode;
    rig.aim_blend = move_toward(rig.aim_blend, target_blend, p.aim_blend_speed * dt);

    let char_pos = char_gtf.translation();
    let yaw = view_yaw.0;
    let pitch = view_smoothed.map(|s| s.0).unwrap_or(0.0);
    let rot = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);

    let (pivot, back_dir, max_len) = match rig.mode {
        AlsCameraMode::FirstPersonPlaceholder => {
            let eye = char_pos + Vec3::Y * p.first_person_eye_height;
            (eye, rot * Vec3::Z, 0.05)
        }
        _ => {
            let mut shoulder = blended_shoulder(rig.aim_blend, p);
            if left_shoulder {
                shoulder.x = -shoulder.x;
            }
            let pivot = char_pos + Vec3::Y * p.pivot_height + rot * shoulder;
            (pivot, rot * Vec3::Z, blended_arm_length(rig.aim_blend, p))
        }
    };

    let filter = SpatialQueryFilter::from_excluded_entities([char_entity, cam_entity]);
    let mut trace_start = pivot;
    let mut trace_end = pivot + back_dir * max_len;
    let mut trace_blocking_hit = false;
    let len = if rig.mode == AlsCameraMode::FirstPersonPlaceholder {
        max_len
    } else if let Ok(dir) = Dir3::new(back_dir) {
        let hit = spatial.cast_ray(
            pivot,
            dir,
            (max_len + p.probe_skin).max(0.1),
            true,
            &filter,
        );
        match hit {
            Some(h) => {
                trace_blocking_hit = true;
                trace_start = pivot;
                trace_end = pivot + dir * h.distance;
                spring_arm_length(max_len, Some(h.distance), p.probe_skin)
            }
            None => {
                trace_start = pivot;
                trace_end = pivot + dir * max_len;
                spring_arm_length(max_len, None, p.probe_skin)
            }
        }
    } else {
        max_len
    };

    cam_tf.translation = pivot + back_dir * len;
    cam_tf.look_at(pivot, Vec3::Y);

    if let Some(debug) = debug_info.as_mut() {
        let was_valid = debug.valid;
        debug.pivot_target =
            char_pos + Vec3::Y * p.pivot_height + rot * blended_shoulder(rig.aim_blend, p);
        debug.pivot_lag = None;
        debug.pivot = Some(pivot);
        debug.field_of_view_deg = 90.0;
        debug.right_shoulder = !left_shoulder;
        debug.trace_start = trace_start;
        debug.trace_end = trace_end;
        debug.trace_radius = 0.0;
        debug.trace_blocking_hit = trace_blocking_hit;
        debug.valid = true;
        if !was_valid {
            info!("[CAM] debug info valid");
        }
    }
}
