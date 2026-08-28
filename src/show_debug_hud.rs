use std::collections::BTreeSet;

use als_camera::{AlsCameraMode, AlsCameraParams, AlsCameraRig, blended_shoulder};
use als_core::character::AlsCharacter;
use als_core::gait::AlsDesiredGait;
use als_core::input::AlsInputCommand;
use als_core::mantle::AlsMantleState;
use als_core::motion::{AlsInputOverride, AlsViewYaw};
use als_core::roll::AlsRollState;
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_core::transition::AlsStateRequest;
use als_core::view::AlsViewSmoothed;
use als_ext::AlsAnimCurves;
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;

const HEADER_FONT_PX: f32 = 15.0;
const BODY_FONT_PX: f32 = 12.0;
const ROOT_OFFSET_PX: f32 = 5.0;
const COLUMN_WIDTH_PX: f32 = 200.0;
const LABEL_WIDTH_STATE_PX: f32 = 120.0;
const LABEL_WIDTH_CURVES_PX: f32 = 145.0;
const NA_VALUE: &str = "N/A";
pub const HEADER_DEBUG_MODE: &str = "Debug mode is enabled! Press (Shift + 0) to disable.";

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlsShowDebugHudSet {
    Input,
    Sync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebugCategory {
    Curves,
    State,
    Shapes,
    Traces,
    Mantling,
    CameraCurves,
    CameraShapes,
    CameraTraces,
}

impl DebugCategory {
    pub const ALL: [DebugCategory; 8] = [
        DebugCategory::Curves,
        DebugCategory::State,
        DebugCategory::Shapes,
        DebugCategory::Traces,
        DebugCategory::Mantling,
        DebugCategory::CameraCurves,
        DebugCategory::CameraShapes,
        DebugCategory::CameraTraces,
    ];

    pub const fn header(self) -> &'static str {
        match self {
            DebugCategory::Curves => "Als.Curves (Shift + 1)",
            DebugCategory::State => "Als.State (Shift + 2)",
            DebugCategory::Shapes => "Als.Shapes (Shift + 3)",
            DebugCategory::Traces => "Als.Traces (Shift + 4)",
            DebugCategory::Mantling => "Als.Mantling (Shift + 5)",
            DebugCategory::CameraCurves => "Als.CameraCurves (Shift + 6)",
            DebugCategory::CameraShapes => "Als.CameraShapes (Shift + 7)",
            DebugCategory::CameraTraces => "Als.CameraTraces (Shift + 8)",
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AlsShowDebugHudState {
    pub visible: bool,
    pub curves: bool,
    pub state: bool,
    pub shapes: bool,
    pub traces: bool,
    pub mantling: bool,
    pub camera_curves: bool,
    pub camera_shapes: bool,
    pub camera_traces: bool,
}

impl AlsShowDebugHudState {
    pub fn toggle_master(&mut self) {
        self.visible = !self.visible;
    }

    pub fn is_active(&self, category: DebugCategory) -> bool {
        self.visible && self.flag(category)
    }

    pub fn toggle_category_engine(&mut self, category: DebugCategory) {
        if self.visible && self.flag(category) {
            self.set_flag(category, false);
        } else {
            self.set_flag(category, true);
            self.visible = true;
        }
    }

    fn flag(&self, category: DebugCategory) -> bool {
        match category {
            DebugCategory::Curves => self.curves,
            DebugCategory::State => self.state,
            DebugCategory::Shapes => self.shapes,
            DebugCategory::Traces => self.traces,
            DebugCategory::Mantling => self.mantling,
            DebugCategory::CameraCurves => self.camera_curves,
            DebugCategory::CameraShapes => self.camera_shapes,
            DebugCategory::CameraTraces => self.camera_traces,
        }
    }

    fn set_flag(&mut self, category: DebugCategory, value: bool) {
        match category {
            DebugCategory::Curves => self.curves = value,
            DebugCategory::State => self.state = value,
            DebugCategory::Shapes => self.shapes = value,
            DebugCategory::Traces => self.traces = value,
            DebugCategory::Mantling => self.mantling = value,
            DebugCategory::CameraCurves => self.camera_curves = value,
            DebugCategory::CameraShapes => self.camera_shapes = value,
            DebugCategory::CameraTraces => self.camera_traces = value,
        }
    }
}

pub fn apply_show_debug_key(
    state: &mut AlsShowDebugHudState,
    shift_held: bool,
    code: KeyCode,
) -> bool {
    if !shift_held {
        return false;
    }
    let category = match code {
        KeyCode::Digit1 => Some(DebugCategory::Curves),
        KeyCode::Digit2 => Some(DebugCategory::State),
        KeyCode::Digit3 => Some(DebugCategory::Shapes),
        KeyCode::Digit4 => Some(DebugCategory::Traces),
        KeyCode::Digit5 => Some(DebugCategory::Mantling),
        KeyCode::Digit6 => Some(DebugCategory::CameraCurves),
        KeyCode::Digit7 => Some(DebugCategory::CameraShapes),
        KeyCode::Digit8 => Some(DebugCategory::CameraTraces),
        _ => None,
    };
    match code {
        KeyCode::Digit0 => {
            state.toggle_master();
            true
        }
        _ => match category {
            Some(category) => {
                state.toggle_category_engine(category);
                true
            }
            None => false,
        },
    }
}

pub fn name_to_display_string(name: &str, name_is_bool: bool) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    let mut in_run = false;
    for (index, &ch) in chars.iter().enumerate() {
        if index == 0 && name_is_bool && ch == 'b' && chars.len() > 1 && chars[1].is_uppercase() {
            continue;
        }
        let upper = ch.is_uppercase();
        let digit = ch.is_ascii_digit();
        if (upper || digit) && !in_run && !out.is_empty() {
            out.push(' ');
        }
        in_run = upper || digit;
        if out.is_empty() {
            out.extend(ch.to_uppercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn linear(r: f32, g: f32, b: f32) -> Color {
    Color::linear_rgba(r, g, b, 1.0)
}

fn header_active_color() -> Color {
    linear(0.0, 1.0, 0.0)
}

fn header_inactive_color() -> Color {
    linear(0.0, 0.333_333, 0.0)
}

fn white() -> Color {
    linear(1.0, 1.0, 1.0)
}

fn red() -> Color {
    linear(1.0, 0.0, 0.0)
}

fn orange() -> Color {
    linear(1.0, 0.5, 0.0)
}

fn orange_dim() -> Color {
    linear(0.5, 0.25, 0.0)
}

fn purple() -> Color {
    linear(0.75, 0.0, 1.0)
}

fn purple_dim() -> Color {
    linear(0.375, 0.0, 0.5)
}

fn cyan() -> Color {
    linear(0.0, 0.75, 1.0)
}

fn gray() -> Color {
    linear(0.5, 0.5, 0.5)
}

fn curve_color(value: f32, use_abs: bool) -> Color {
    let t = if use_abs { value.abs() } else { value }.clamp(0.0, 1.0);
    let base = gray().to_linear();
    let target = white().to_linear();
    linear(
        base.red + (target.red - base.red) * t,
        base.green + (target.green - base.green) * t,
        base.blue + (target.blue - base.blue) * t,
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct DebugRow {
    pub label: String,
    pub value: String,
    pub color: Color,
    pub label_width_px: f32,
    pub font_px: f32,
}

impl DebugRow {
    fn header_row(text: impl Into<String>, color: Color) -> Self {
        Self {
            label: text.into(),
            value: String::new(),
            color,
            label_width_px: 0.0,
            font_px: HEADER_FONT_PX,
        }
    }

    fn line(text: impl Into<String>, color: Color) -> Self {
        Self {
            label: text.into(),
            value: String::new(),
            color,
            label_width_px: 0.0,
            font_px: BODY_FONT_PX,
        }
    }

    fn pair(
        label: impl Into<String>,
        value: impl Into<String>,
        color: Color,
        width_px: f32,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            color,
            label_width_px: width_px,
            font_px: BODY_FONT_PX,
        }
    }
}

pub type DebugColumn = Vec<DebugRow>;
pub type DebugBlock = Vec<DebugColumn>;

#[derive(Debug, Clone)]
pub struct CharacterSnapshot {
    pub locomotion_state: LocomotionState,
    pub rotation_mode: RotationMode,
    pub stance: Stance,
    pub gait: Gait,
    pub overlay_state: OverlayState,
    pub desired_rotation_mode: RotationMode,
    pub desired_stance: Stance,
    pub desired_gait: Gait,
    pub desired_aiming: bool,
    pub input_yaw_angle: Option<f32>,
    pub speed: Option<f32>,
    pub has_velocity: bool,
    pub velocity_direction: Option<Vec3>,
    pub velocity_yaw_angle: Option<f32>,
    pub mantling: bool,
    pub rolling: bool,
}

impl CharacterSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn from_components(
        locomotion_state: LocomotionState,
        rotation_mode: RotationMode,
        stance: Stance,
        gait: Gait,
        desired_gait: Gait,
        overlay_state: OverlayState,
        request: &AlsStateRequest,
        input_move_dir: Option<Vec2>,
        aiming: bool,
        velocity: Option<Vec3>,
        mantling: bool,
        rolling: bool,
    ) -> Self {
        let has_input = input_move_dir.is_some_and(|d| d.length() > 1e-4);
        let input_yaw_angle = input_move_dir
            .filter(|_| has_input)
            .map(|d| normalize_degrees(d.y.atan2(d.x).to_degrees()));
        let horizontal_speed = velocity.map(|v| Vec2::new(v.x, v.z).length());
        let has_velocity = horizontal_speed.is_some_and(|s| s > 1e-3);
        let velocity_direction = velocity
            .filter(|_| has_velocity)
            .map(|v| v.normalize_or_zero());
        let velocity_yaw_angle = velocity
            .filter(|_| has_velocity)
            .map(|v| normalize_degrees(v.x.atan2(-v.z).to_degrees()));
        Self {
            locomotion_state,
            rotation_mode,
            stance,
            gait,
            overlay_state,
            desired_rotation_mode: request.rotation_mode.unwrap_or(rotation_mode),
            desired_stance: request.stance.unwrap_or(stance),
            desired_gait,
            desired_aiming: aiming,
            input_yaw_angle,
            speed: horizontal_speed,
            has_velocity,
            velocity_direction,
            velocity_yaw_angle,
            mantling,
            rolling,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CameraSnapshot {
    pub view_rotation_deg: Option<(f32, f32, f32)>,
    pub rig_mode: Option<AlsCameraMode>,
    pub pivot_target_location: Option<Vec3>,
    pub pivot_lag_location: Option<Vec3>,
    pub pivot_location: Option<Vec3>,
    pub field_of_view_deg: Option<f32>,
    pub right_shoulder: Option<bool>,
    pub camera_curves: Vec<(String, f32)>,
}

fn normalize_degrees(degrees: f32) -> f32 {
    let mut value = degrees % 360.0;
    if value > 180.0 {
        value -= 360.0;
    }
    if value <= -180.0 {
        value += 360.0;
    }
    value
}

fn tag_value(tag: &str) -> String {
    if tag.is_empty() {
        "None".to_string()
    } else {
        name_to_display_string(tag, false)
    }
}

fn bool_value(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

fn locomotion_mode_tag(locomotion_state: LocomotionState) -> &'static str {
    match locomotion_state {
        LocomotionState::Grounded => "Grounded",
        LocomotionState::Air => "InAir",
        LocomotionState::Ragdolling => "",
    }
}

fn rotation_mode_tag(mode: RotationMode) -> &'static str {
    match mode {
        RotationMode::VelocityDirection => "VelocityDirection",
        RotationMode::ViewDirection => "ViewDirection",
        RotationMode::Aiming => "Aiming",
    }
}

fn stance_tag(stance: Stance) -> &'static str {
    match stance {
        Stance::Standing => "Standing",
        Stance::Crouching => "Crouching",
    }
}

fn gait_tag(gait: Gait) -> &'static str {
    match gait {
        Gait::Walking => "Walking",
        Gait::Running => "Running",
        Gait::Sprinting => "Sprinting",
    }
}

fn overlay_tag(overlay: OverlayState) -> &'static str {
    match overlay {
        OverlayState::Default => "Default",
        OverlayState::Masculine => "Masculine",
        OverlayState::Feminine => "Feminine",
        OverlayState::Injured => "Injured",
        OverlayState::HandsTied => "HandsTied",
        OverlayState::Rifle => "Rifle",
        OverlayState::PistolOneHanded => "PistolOneHanded",
        OverlayState::PistolTwoHanded => "PistolTwoHanded",
        OverlayState::Bow => "Bow",
        OverlayState::Torch => "Torch",
        OverlayState::Binoculars => "Binoculars",
        OverlayState::Box => "Box",
        OverlayState::Barrel => "Barrel",
    }
}

fn view_mode_tag(mode: Option<AlsCameraMode>) -> Option<&'static str> {
    match mode {
        Some(AlsCameraMode::ThirdPerson | AlsCameraMode::AimingShoulder) => Some("ThirdPerson"),
        Some(AlsCameraMode::FirstPersonPlaceholder) => Some("FirstPerson"),
        None => None,
    }
}

fn locomotion_action_value(snapshot: &CharacterSnapshot) -> &'static str {
    if snapshot.mantling {
        "Mantling"
    } else if snapshot.locomotion_state == LocomotionState::Ragdolling {
        "Ragdolling"
    } else if snapshot.rolling {
        "Rolling"
    } else {
        ""
    }
}

pub fn build_state_body(
    snapshot: &CharacterSnapshot,
    view_mode: Option<&'static str>,
) -> Vec<DebugRow> {
    vec![
        DebugRow::pair(
            name_to_display_string("ViewMode", false),
            view_mode.map(tag_value).unwrap_or_else(|| NA_VALUE.into()),
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("LocomotionMode", false),
            tag_value(locomotion_mode_tag(snapshot.locomotion_state)),
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("bDesiredAiming", true),
            bool_value(snapshot.desired_aiming),
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("DesiredRotationMode", false),
            tag_value(rotation_mode_tag(snapshot.desired_rotation_mode)),
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("RotationMode", false),
            tag_value(rotation_mode_tag(snapshot.rotation_mode)),
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("DesiredStance", false),
            tag_value(stance_tag(snapshot.desired_stance)),
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("Stance", false),
            tag_value(stance_tag(snapshot.stance)),
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("DesiredGait", false),
            tag_value(gait_tag(snapshot.desired_gait)),
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("Gait", false),
            tag_value(gait_tag(snapshot.gait)),
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("OverlayMode", false),
            tag_value(overlay_tag(snapshot.overlay_state)),
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("LocomotionAction", false),
            tag_value(locomotion_action_value(snapshot)),
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
    ]
}

pub fn build_shapes_body(snapshot: &CharacterSnapshot, camera: &CameraSnapshot) -> Vec<DebugRow> {
    let rotation_value = camera
        .view_rotation_deg
        .map(|(roll, pitch, yaw)| format!("R: {roll:.2} P: {pitch:.2} Y: {yaw:.2}"))
        .unwrap_or_else(|| NA_VALUE.into());
    let input_color = if snapshot.input_yaw_angle.is_some() {
        orange()
    } else {
        orange_dim()
    };
    let speed_color = if snapshot.has_velocity {
        purple()
    } else {
        purple_dim()
    };
    let direction_value = snapshot
        .velocity_direction
        .map(format_xyz)
        .unwrap_or_else(|| NA_VALUE.into());
    vec![
        DebugRow::pair(
            name_to_display_string("Rotation", false),
            rotation_value,
            red(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("InputYawAngle", false),
            snapshot
                .input_yaw_angle
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| NA_VALUE.into()),
            input_color,
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("Speed", false),
            snapshot
                .speed
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| NA_VALUE.into()),
            speed_color,
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            "Velocity Direction",
            direction_value,
            speed_color,
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("VelocityYawAngle", false),
            snapshot
                .velocity_yaw_angle
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| NA_VALUE.into()),
            speed_color,
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("SmoothTargetYawAngle", false),
            NA_VALUE,
            cyan(),
            LABEL_WIDTH_STATE_PX,
        ),
    ]
}

pub fn build_traces_body() -> Vec<DebugRow> {
    vec![
        DebugRow::line("Ground Prediction", purple()),
        DebugRow::line("Footstep Effects", red()),
    ]
}

pub fn build_mantling_body() -> Vec<DebugRow> {
    vec![
        DebugRow::line("Forward Trace", cyan()),
        DebugRow::line("Downward Trace", purple()),
        DebugRow::line("Failed Target Location Overlap", red()),
        DebugRow::line("Failed Start Location Overlap", orange()),
    ]
}

pub fn build_curves_body(curves: &[(String, f32)], use_abs: bool) -> Vec<DebugRow> {
    curves
        .iter()
        .map(|(name, value)| {
            DebugRow::pair(
                name_to_display_string(name, false),
                format!("{value:.2}"),
                curve_color(*value, use_abs),
                LABEL_WIDTH_CURVES_PX,
            )
        })
        .collect()
}

pub fn build_camera_shapes_body(camera: &CameraSnapshot) -> Vec<DebugRow> {
    let pivot_value = camera
        .pivot_target_location
        .map(format_xyz)
        .unwrap_or_else(|| NA_VALUE.into());
    let pivot_lag_value = camera
        .pivot_lag_location
        .map(format_xyz)
        .unwrap_or_else(|| NA_VALUE.into());
    let pivot_location_value = camera
        .pivot_location
        .map(format_xyz)
        .unwrap_or_else(|| NA_VALUE.into());
    let field_of_view_value = camera
        .field_of_view_deg
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| NA_VALUE.into());
    let right_shoulder_value = camera
        .right_shoulder
        .map(|value| value.to_string())
        .unwrap_or_else(|| NA_VALUE.into());
    vec![
        DebugRow::pair(
            name_to_display_string("PivotTargetLocation", false),
            pivot_value,
            linear(0.0, 1.0, 0.0),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("PivotLagLocation", false),
            pivot_lag_value,
            orange(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("PivotLocation", false),
            pivot_location_value,
            cyan(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("CameraFieldOfView", false),
            field_of_view_value,
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("bRightShoulder", true),
            right_shoulder_value,
            white(),
            LABEL_WIDTH_STATE_PX,
        ),
    ]
}

pub fn build_camera_traces_body() -> Vec<DebugRow> {
    vec![
        DebugRow::line("Blocking Geometry Adjustment", cyan()),
        DebugRow::line("Camera Trace (No Hit)", linear(0.0, 1.0, 0.0)),
        DebugRow::line("Camera Trace (Blocking Hit)", red()),
    ]
}

fn format_xyz(value: Vec3) -> String {
    format!("X: {:.2} Y: {:.2} Z: {:.2}", value.x, value.y, value.z)
}

pub fn layout_character_block(
    state: &AlsShowDebugHudState,
    snapshot: &CharacterSnapshot,
    camera: &CameraSnapshot,
    curves_rows: &[DebugRow],
) -> DebugBlock {
    let curves_active = state.is_active(DebugCategory::Curves);
    let mut columns: Vec<Vec<DebugRow>> = Vec::new();
    let mut first_column = vec![DebugRow::header_row(
        HEADER_DEBUG_MODE,
        header_active_color(),
    )];
    first_column.push(DebugRow::header_row(
        DebugCategory::Curves.header(),
        if curves_active {
            header_active_color()
        } else {
            header_inactive_color()
        },
    ));
    if curves_active {
        first_column.extend(curves_rows.iter().cloned());
        columns.push(first_column);
        columns.push(Vec::new());
    } else {
        columns.push(first_column);
    }
    let trailing = columns.last_mut().expect("columns non-empty");
    for category in [
        DebugCategory::State,
        DebugCategory::Shapes,
        DebugCategory::Traces,
        DebugCategory::Mantling,
    ] {
        let active = state.is_active(category);
        trailing.push(DebugRow::header_row(
            category.header(),
            if active {
                header_active_color()
            } else {
                header_inactive_color()
            },
        ));
        if active {
            trailing.extend(match category {
                DebugCategory::State => build_state_body(snapshot, view_mode_tag(camera.rig_mode)),
                DebugCategory::Shapes => build_shapes_body(snapshot, camera),
                DebugCategory::Traces => build_traces_body(),
                DebugCategory::Mantling => build_mantling_body(),
                _ => Vec::new(),
            });
        }
    }
    columns
}

pub fn layout_camera_block(
    state: &AlsShowDebugHudState,
    camera: &CameraSnapshot,
    camera_curves_rows: &[DebugRow],
) -> DebugBlock {
    let curves_active = state.is_active(DebugCategory::CameraCurves);
    let mut column = Vec::new();
    column.push(DebugRow::header_row(
        DebugCategory::CameraCurves.header(),
        if curves_active {
            header_active_color()
        } else {
            header_inactive_color()
        },
    ));
    if curves_active {
        column.extend(camera_curves_rows.iter().cloned());
    }
    for category in [DebugCategory::CameraShapes, DebugCategory::CameraTraces] {
        let active = state.is_active(category);
        column.push(DebugRow::header_row(
            category.header(),
            if active {
                header_active_color()
            } else {
                header_inactive_color()
            },
        ));
        if active {
            column.extend(match category {
                DebugCategory::CameraShapes => build_camera_shapes_body(camera),
                DebugCategory::CameraTraces => build_camera_traces_body(),
                _ => Vec::new(),
            });
        }
    }
    vec![column]
}

type Layout = Vec<DebugBlock>;

enum HudCache {
    Hidden,
    Shown(Layout),
}

#[derive(Resource, Debug, Default, Clone)]
pub struct AlsShowDebugCurveSources {
    pub character: Option<Handle<AlsAnimCurves>>,
    pub camera: Option<Handle<AlsAnimCurves>>,
}

#[derive(Component)]
pub struct AlsShowDebugHudRoot;

pub struct AlsShowDebugHudPlugin;

impl Plugin for AlsShowDebugHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AlsShowDebugHudState>()
            .init_resource::<AlsShowDebugCurveSources>()
            .configure_sets(
                Update,
                (AlsShowDebugHudSet::Input, AlsShowDebugHudSet::Sync).chain(),
            )
            .add_systems(
                Update,
                (
                    keyboard_system.in_set(AlsShowDebugHudSet::Input),
                    hud_sync_system.in_set(AlsShowDebugHudSet::Sync),
                ),
            );
    }
}

fn keyboard_system(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<AlsShowDebugHudState>) {
    let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for code in [
        KeyCode::Digit0,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
    ] {
        if keys.just_pressed(code) {
            apply_show_debug_key(&mut state, shift_held, code);
        }
    }
}

type CharacterQueryItem<'w, 's> = (
    &'w LocomotionState,
    &'w RotationMode,
    &'w Stance,
    &'w Gait,
    Option<&'w AlsDesiredGait>,
    &'w OverlayState,
    &'w AlsStateRequest,
    Option<&'w LinearVelocity>,
    Option<&'w AlsInputCommand>,
    Option<&'w AlsInputOverride>,
    Option<&'w AlsRollState>,
    Option<&'w AlsMantleState>,
    Option<&'w AlsViewYaw>,
    Option<&'w AlsViewSmoothed>,
    Option<&'w GlobalTransform>,
);

fn build_camera_snapshot<'w, 's>(
    character_item: &CharacterQueryItem<'w, 's>,
    rig: Option<(&GlobalTransform, Option<&AlsCameraRig>)>,
    params: Option<&AlsCameraParams>,
    debug_info: Option<&als_camera::AlsCameraDebugInfo>,
) -> CameraSnapshot {
    let (_, _, _, _, _, _, _, _, _, _, _, _, view_yaw, view_smoothed, char_transform) =
        character_item;

    let view_rotation_deg = rig.map(|(transform, _)| {
        let (yaw, pitch, roll) = transform.rotation().to_euler(EulerRot::YXZ);
        (yaw.to_degrees(), pitch.to_degrees(), roll.to_degrees())
    });
    let pivot_target_location = match (char_transform, params, view_yaw) {
        (Some(char_transform), Some(params), Some(view_yaw)) => {
            let blend = rig
                .and_then(|(_, rig)| rig)
                .map(|rig| rig.aim_blend)
                .unwrap_or(0.0);
            let pitch = view_smoothed.map(|s| s.0).unwrap_or(0.0);
            let rot = Quat::from_euler(EulerRot::YXZ, view_yaw.0, pitch, 0.0);
            Some(
                char_transform.translation()
                    + Vec3::Y * params.pivot_height
                    + rot * blended_shoulder(blend, params),
            )
        }
        _ => None,
    };
    let camera_curves = debug_info
        .map(|info| {
            als_camera::CAMERA_CURVE_NAMES
                .iter()
                .map(|name| ((*name).to_string(), info.curve_value(name)))
                .collect()
        })
        .unwrap_or_default();
    CameraSnapshot {
        view_rotation_deg,
        rig_mode: rig.and_then(|(_, rig)| rig).map(|rig| rig.mode),
        pivot_target_location,
        pivot_lag_location: debug_info.and_then(|info| info.pivot_lag),
        pivot_location: debug_info.and_then(|info| info.pivot),
        field_of_view_deg: debug_info.map(|info| info.field_of_view_deg),
        right_shoulder: debug_info.map(|info| info.right_shoulder),
        camera_curves,
    }
}

#[allow(clippy::type_complexity)]
fn hud_sync_system(
    mut commands: Commands,
    state: Res<AlsShowDebugHudState>,
    mut root: Local<Option<Entity>>,
    mut cache: Local<Option<HudCache>>,
    q_character: Query<
        (
            &'static LocomotionState,
            &'static RotationMode,
            &'static Stance,
            &'static Gait,
            Option<&'static AlsDesiredGait>,
            &'static OverlayState,
            &'static AlsStateRequest,
            Option<&'static LinearVelocity>,
            Option<&'static AlsInputCommand>,
            Option<&'static AlsInputOverride>,
            Option<&'static AlsRollState>,
            Option<&'static AlsMantleState>,
            Option<&'static AlsViewYaw>,
            Option<&'static AlsViewSmoothed>,
            Option<&'static GlobalTransform>,
        ),
        With<AlsCharacter>,
    >,
    q_camera: Query<
        (
            &GlobalTransform,
            Option<&AlsCameraRig>,
            Option<&als_camera::AlsCameraDebugInfo>,
        ),
        (With<Camera>, Without<AlsCharacter>),
    >,
    q_instances: Query<&AnimatedSceneInstance>,
    q_players: Query<&AnimationGraphPlayer>,
    camera_params: Option<Res<Assets<AlsCameraParams>>>,
    curve_assets: Option<Res<Assets<AlsAnimCurves>>>,
    sources: Res<AlsShowDebugCurveSources>,
) {
    let character_item = q_character.iter().next();
    let target = if state.visible && character_item.is_some() {
        let item = character_item.unwrap();
        let (
            locomotion_state,
            rotation_mode,
            stance,
            gait,
            desired_gait,
            overlay_state,
            request,
            velocity,
            input,
            input_override,
            roll,
            mantle,
            _view_yaw,
            _view_smoothed,
            _char_transform,
        ) = item;

        let aiming = input.map(|c| c.aim_held).unwrap_or(false)
            || input_override.map(|o| o.0.aim_held).unwrap_or(false);
        let move_dir = input_override
            .map(|o| o.0.move_dir)
            .filter(|d| d.length() > 1e-4)
            .or(input.map(|c| c.move_dir));
        let snapshot = CharacterSnapshot::from_components(
            *locomotion_state,
            *rotation_mode,
            *stance,
            *gait,
            desired_gait.map(|value| value.0).unwrap_or(*gait),
            *overlay_state,
            request,
            move_dir,
            aiming,
            velocity.map(|v| v.0),
            mantle.is_some_and(|m| m.active),
            roll.is_some_and(|r| r.active),
        );

        let (rig, cam_debug) = q_camera
            .iter()
            .next()
            .map(|(transform, rig, info)| (Some((transform, rig)), info))
            .unwrap_or((None, None));
        let params = camera_params
            .as_ref()
            .and_then(|assets| assets.iter().next())
            .map(|(_, params)| params);
        let camera_snapshot = build_camera_snapshot(&item, rig, params, cam_debug);

        let elapsed = q_instances
            .iter()
            .next()
            .and_then(|instance| q_players.get(instance.player_entity()).ok())
            .map(|player| player.elapsed())
            .unwrap_or(0.0);
        let character_curve_rows = sampled_curve_rows(
            curve_assets.as_ref(),
            sources.character.as_ref(),
            elapsed,
            false,
        );
        let camera_curve_rows: Vec<DebugRow> = camera_snapshot
            .camera_curves
            .iter()
            .map(|(name, value)| {
                DebugRow::pair(
                    name_to_display_string(name, false),
                    format!("{value:.2}"),
                    curve_color(*value, true),
                    LABEL_WIDTH_CURVES_PX,
                )
            })
            .collect();

        HudCache::Shown(vec![
            layout_character_block(&state, &snapshot, &camera_snapshot, &character_curve_rows),
            layout_camera_block(&state, &camera_snapshot, &camera_curve_rows),
        ])
    } else {
        HudCache::Hidden
    };

    let unchanged = match (cache.as_ref(), &target) {
        (Some(HudCache::Hidden), HudCache::Hidden) => true,
        (Some(HudCache::Shown(current)), HudCache::Shown(next)) => current == next,
        _ => false,
    };
    if unchanged {
        return;
    }
    if let Some(entity) = *root {
        commands.entity(entity).despawn();
        *root = None;
    }
    match &target {
        HudCache::Shown(blocks) => {
            *root = Some(spawn_panel(&mut commands, blocks));
        }
        HudCache::Hidden => {
            *root = Some(spawn_hidden_root(&mut commands));
        }
    }
    *cache = Some(target);
}

fn sampled_curve_rows(
    assets: Option<&Res<Assets<AlsAnimCurves>>>,
    handle: Option<&Handle<AlsAnimCurves>>,
    time: f32,
    use_abs: bool,
) -> Vec<DebugRow> {
    let Some(assets) = assets else {
        return Vec::new();
    };
    let Some(handle) = handle else {
        return Vec::new();
    };
    let Some(curves) = assets.get(handle) else {
        return Vec::new();
    };
    let names: BTreeSet<String> = curves
        .0
        .values()
        .flat_map(|anim| anim.0.keys().map(|k| k.to_string()))
        .collect();
    let samples: Vec<(String, f32)> = names
        .into_iter()
        .filter_map(|name| {
            curves
                .0
                .values()
                .find_map(|anim| anim.sample(&name, time))
                .map(|value| (name, value))
        })
        .collect();
    build_curves_body(&samples, use_abs)
}

fn spawn_hidden_root(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            AlsShowDebugHudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ROOT_OFFSET_PX),
                top: Val::Px(ROOT_OFFSET_PX),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Visibility::Hidden,
        ))
        .id()
}

fn spawn_panel(commands: &mut Commands, blocks: &[DebugBlock]) -> Entity {
    let root = commands
        .spawn((
            AlsShowDebugHudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ROOT_OFFSET_PX),
                top: Val::Px(ROOT_OFFSET_PX),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Visibility::Visible,
        ))
        .id();
    commands.entity(root).with_children(|parent| {
        for block in blocks {
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::FlexStart,
                    ..default()
                })
                .with_children(|block_parent| {
                    for column in block {
                        block_parent
                            .spawn(Node {
                                flex_direction: FlexDirection::Column,
                                width: Val::Px(COLUMN_WIDTH_PX),
                                align_items: AlignItems::FlexStart,
                                ..default()
                            })
                            .with_children(|column_parent| {
                                for row in column {
                                    spawn_row(column_parent, row);
                                }
                            });
                    }
                });
        }
    });
    root
}

fn spawn_row(parent: &mut ChildSpawnerCommands, row: &DebugRow) {
    if row.label_width_px <= 0.0 {
        spawn_leaf(parent, &row.label, row.color, row.font_px);
        return;
    }
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|row_parent| {
            row_parent
                .spawn(Node {
                    width: Val::Px(row.label_width_px),
                    ..default()
                })
                .with_children(|label_parent| {
                    spawn_leaf(label_parent, &row.label, row.color, row.font_px);
                });
            spawn_leaf(row_parent, &row.value, row.color, row.font_px);
        });
}

fn spawn_leaf(parent: &mut ChildSpawnerCommands, text: &str, color: Color, font_px: f32) {
    parent.spawn((
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(font_px),
            ..default()
        },
        TextColor(color),
        TextShadow {
            offset: Vec2::splat(2.0),
            color: Color::BLACK,
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_toggle_matches_engine_flow() {
        let mut state = AlsShowDebugHudState::default();
        assert!(!state.visible);

        assert!(apply_show_debug_key(&mut state, true, KeyCode::Digit0));
        assert!(state.visible);
        assert!(!state.state);

        apply_show_debug_key(&mut state, true, KeyCode::Digit0);
        assert!(!state.visible);
        assert!(!state.state);

        apply_show_debug_key(&mut state, true, KeyCode::Digit2);
        assert!(state.visible);
        assert!(state.state);

        apply_show_debug_key(&mut state, true, KeyCode::Digit0);
        assert!(!state.visible);
        assert!(state.state);

        apply_show_debug_key(&mut state, true, KeyCode::Digit0);
        assert!(state.visible);
        assert!(state.state);
    }

    #[test]
    fn category_toggle_without_jitter() {
        let mut state = AlsShowDebugHudState::default();

        assert!(apply_show_debug_key(&mut state, true, KeyCode::Digit2));
        assert!(state.is_active(DebugCategory::State));

        assert!(apply_show_debug_key(&mut state, true, KeyCode::Digit2));
        assert!(!state.is_active(DebugCategory::State));
        assert!(state.visible);

        assert!(apply_show_debug_key(&mut state, true, KeyCode::Digit2));
        assert!(state.is_active(DebugCategory::State));

        assert!(!apply_show_debug_key(&mut state, false, KeyCode::Digit2));
        assert!(state.is_active(DebugCategory::State));

        assert!(!apply_show_debug_key(&mut state, true, KeyCode::Digit9));
        assert_eq!(
            state,
            AlsShowDebugHudState {
                visible: true,
                state: true,
                ..Default::default()
            }
        );

        apply_show_debug_key(&mut state, true, KeyCode::Digit7);
        assert!(state.visible && state.camera_shapes);
        apply_show_debug_key(&mut state, true, KeyCode::Digit7);
        assert!(state.visible && !state.camera_shapes);
    }

    #[test]
    fn display_strings_match_ue_name_formatting() {
        assert_eq!(name_to_display_string("ViewMode", false), "View Mode");
        assert_eq!(
            name_to_display_string("LocomotionMode", false),
            "Locomotion Mode"
        );
        assert_eq!(
            name_to_display_string("bDesiredAiming", true),
            "Desired Aiming"
        );
        assert_eq!(
            name_to_display_string("bRightShoulder", true),
            "Right Shoulder"
        );
        assert_eq!(
            name_to_display_string("VelocityDirection", false),
            "Velocity Direction"
        );
        assert_eq!(name_to_display_string("InAir", false), "In Air");
        assert_eq!(
            name_to_display_string("SmoothTargetYawAngle", false),
            "Smooth Target Yaw Angle"
        );
        assert_eq!(
            name_to_display_string("PistolTwoHanded", false),
            "Pistol Two Handed"
        );
        assert_eq!(
            name_to_display_string("CameraFieldOfView", false),
            "Camera Field Of View"
        );
        assert_eq!(name_to_display_string("Stance", false), "Stance");
    }

    fn sample_snapshot() -> CharacterSnapshot {
        let mut request = AlsStateRequest::default();
        request.gait = Some(Gait::Running);
        CharacterSnapshot::from_components(
            LocomotionState::Grounded,
            RotationMode::ViewDirection,
            Stance::Standing,
            Gait::Walking,
            Gait::Running,
            OverlayState::Default,
            &request,
            Some(Vec2::new(0.0, 1.0)),
            true,
            Some(Vec3::new(3.0, 0.0, -4.0)),
            false,
            false,
        )
    }

    #[test]
    fn state_body_contains_expected_labels_and_values() {
        let rows = build_state_body(&sample_snapshot(), Some("ThirdPerson"));
        let joined: String = rows
            .iter()
            .flat_map(|r| [r.label.clone(), r.value.clone()])
            .collect::<Vec<_>>()
            .join("\n");
        for expected in [
            "View Mode",
            "Third Person",
            "Locomotion Mode",
            "Grounded",
            "Desired Aiming",
            "True",
            "Desired Rotation Mode",
            "Rotation Mode",
            "View Direction",
            "Desired Stance",
            "Stance",
            "Standing",
            "Desired Gait",
            "Gait",
            "Running",
            "Overlay Mode",
            "Locomotion Action",
            "None",
        ] {
            assert!(joined.contains(expected), "missing {expected} in {joined}");
        }
    }

    #[test]
    fn na_values_render_without_panics() {
        let camera = CameraSnapshot::default();
        let mut no_input = sample_snapshot();
        no_input.input_yaw_angle = None;
        no_input.speed = None;
        no_input.has_velocity = false;
        no_input.velocity_direction = None;
        no_input.velocity_yaw_angle = None;

        let shapes = build_shapes_body(&no_input, &camera);
        assert!(shapes.iter().all(|r| r.value == NA_VALUE));

        let camera_shapes = build_camera_shapes_body(&camera);
        assert_eq!(camera_shapes.len(), 5);
        assert!(camera_shapes.iter().all(|r| r.value == NA_VALUE));

        assert!(build_curves_body(&[], false).is_empty());

        let base = AlsShowDebugHudState {
            visible: true,
            ..Default::default()
        };
        for mask in 0..256u16 {
            let mut flags = base;
            flags.curves = mask & 1 != 0;
            flags.state = mask & 2 != 0;
            flags.shapes = mask & 4 != 0;
            flags.traces = mask & 8 != 0;
            flags.mantling = mask & 16 != 0;
            flags.camera_curves = mask & 32 != 0;
            flags.camera_shapes = mask & 64 != 0;
            flags.camera_traces = mask & 128 != 0;
            let blocks = vec![
                layout_character_block(&flags, &sample_snapshot(), &camera, &[]),
                layout_camera_block(&flags, &camera, &[]),
            ];
            assert!(!blocks.is_empty());
            for block in &blocks {
                assert!(!block.is_empty());
                for column in block {
                    assert!(!column.is_empty());
                }
            }
        }
    }

    #[test]
    fn action_precedence_follows_mantle_ragdoll_roll() {
        let mut snapshot = sample_snapshot();
        assert_eq!(locomotion_action_value(&snapshot), "");
        snapshot.rolling = true;
        assert_eq!(locomotion_action_value(&snapshot), "Rolling");
        snapshot.mantling = true;
        assert_eq!(locomotion_action_value(&snapshot), "Mantling");
        snapshot.mantling = false;
        snapshot.locomotion_state = LocomotionState::Ragdolling;
        assert_eq!(locomotion_action_value(&snapshot), "Ragdolling");
        snapshot.rolling = true;
        assert_eq!(locomotion_action_value(&snapshot), "Ragdolling");
    }

    #[test]
    fn layout_splits_second_column_only_when_first_category_active() {
        let camera = CameraSnapshot::default();
        let mut flags = AlsShowDebugHudState {
            visible: true,
            ..Default::default()
        };

        let inactive = layout_character_block(&flags, &sample_snapshot(), &camera, &[]);
        assert_eq!(inactive.len(), 1);
        assert_eq!(inactive[0].len(), 6);
        assert_eq!(inactive[0][0].label, HEADER_DEBUG_MODE);
        assert_eq!(inactive[0][0].color, header_active_color());

        flags.curves = true;
        let split = layout_character_block(&flags, &sample_snapshot(), &camera, &[]);
        assert_eq!(split.len(), 2);
        assert_eq!(split[1].len(), 4);

        flags.curves = false;
        flags.state = true;
        let expanded = layout_character_block(&flags, &sample_snapshot(), &camera, &[]);
        assert_eq!(expanded.len(), 1);
        assert_eq!(
            expanded[0].len(),
            6 + build_state_body(&sample_snapshot(), None).len()
        );
        assert_eq!(expanded[0][2].label, DebugCategory::State.header());
        assert_eq!(expanded[0][2].color, header_active_color());
        assert_eq!(expanded[0][1].color, header_inactive_color());

        let camera_block_off = layout_camera_block(&flags, &camera, &[]);
        assert_eq!(camera_block_off.len(), 1);
        assert_eq!(camera_block_off[0].len(), 3);

        flags.camera_curves = true;
        flags.camera_shapes = true;
        let camera_block_on = layout_camera_block(&flags, &camera, &[]);
        assert_eq!(
            camera_block_on[0].len(),
            1 + 2 + build_camera_shapes_body(&camera).len()
        );
    }

    #[test]
    fn camera_shape_rows_use_live_debug_values_when_available() {
        let camera = CameraSnapshot {
            pivot_target_location: Some(Vec3::new(1.0, 2.0, 3.0)),
            pivot_lag_location: Some(Vec3::new(4.0, 5.0, 6.0)),
            pivot_location: Some(Vec3::new(7.0, 8.0, 9.0)),
            field_of_view_deg: Some(102.0),
            right_shoulder: Some(true),
            ..default()
        };
        let rows = build_camera_shapes_body(&camera);
        assert_eq!(rows[1].value, "X: 4.00 Y: 5.00 Z: 6.00");
        assert_eq!(rows[2].value, "X: 7.00 Y: 8.00 Z: 9.00");
        assert_eq!(rows[3].value, "102.00");
        assert_eq!(rows[4].value, "true");
    }

    #[test]
    fn normalize_degrees_wraps_into_ue_range() {
        assert!((normalize_degrees(-181.0) - 179.0).abs() < 1e-4);
        assert!((normalize_degrees(190.0) + 170.0).abs() < 1e-4);
        assert!((normalize_degrees(90.0) - 90.0).abs() < 1e-6);
        assert!((normalize_degrees(0.0)).abs() < 1e-6);
    }

    #[test]
    fn curve_rows_use_lexical_order_and_lerped_colors() {
        let samples = vec![
            ("BasePose".to_string(), 1.0_f32),
            ("FootPlanted".to_string(), 0.5_f32),
        ];
        let rows = build_curves_body(&samples, false);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "Base Pose");
        assert_eq!(rows[1].label, "Foot Planted");
        assert_eq!(rows[1].value, "0.50");
        assert_ne!(rows[0].color, gray());
        assert_eq!(curve_color(1.0, false), white());
        assert_eq!(curve_color(-3.0, true), white());
        assert_eq!(curve_color(0.0, false), gray());
    }
}
