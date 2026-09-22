use std::sync::LazyLock;

mod canvas;

use als_camera::{AlsCameraMode, AlsCameraRig};
use als_core::character::AlsCharacter;
use als_core::gait::AlsDesiredGait;
use als_core::input::AlsInputCommand;
use als_core::mantle::AlsMantleState;
use als_core::motion::AlsInputOverride;
use als_core::roll::AlsRollState;
use als_core::state::{AlsNotifyLocomotionAction, Gait, LocomotionState, OverlayState, RotationMode, Stance};
#[cfg(test)]
use als_core::transition::AlsStateRequest;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use crate::{animation_context_projection::AlsRootAnimationContext,
    camera_component::AlsSourceCameraComponent,
    character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar}};

include!("../assets/ui/als_debug_canvas.runtime.rs");
const BODY_FONT_PX: f32 = HEADER_FONT_PX * 0.75;
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

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AlsShowDebugHudState {
    #[serde(skip)]
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

#[derive(Resource)]
pub struct AlsShowDebugSettingsFile(pub std::path::PathBuf);

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
    pub y: f32,
}

impl DebugRow {
    fn header_row(text: impl Into<String>, color: Color) -> Self {
        Self {
            label: text.into(),
            value: String::new(),
            color,
            label_width_px: 0.0,
            font_px: HEADER_FONT_PX,
            y: 0.0,
        }
    }

    fn line(text: impl Into<String>, color: Color) -> Self {
        Self {
            label: text.into(),
            value: String::new(),
            color,
            label_width_px: 0.0,
            font_px: BODY_FONT_PX,
            y: 0.0,
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
            y: 0.0,
        }
    }
}

pub type DebugColumn = Vec<DebugRow>;
pub type DebugBlock = Vec<DebugColumn>;

#[derive(Debug, Clone)]
pub struct CharacterSnapshot {
    pub first_person: bool,
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
    pub has_input: bool,
    pub speed: Option<f32>,
    pub has_velocity: bool,
    pub velocity_direction: Option<Vec3>,
    pub velocity_yaw_angle: Option<f32>,
    pub locomotion_action: AlsNotifyLocomotionAction,
    pub view_rotation_deg: Option<(f32, f32, f32)>,
    pub smooth_target_yaw_angle: Option<f32>,
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
        desired_rotation_mode: RotationMode,
        desired_stance: Stance,
        first_person: bool,
        motion: Option<&crate::actual_motion::AlsActualMotion>,
        aiming: bool,
        locomotion_action: AlsNotifyLocomotionAction,
    ) -> Self {
        let has_input = motion.is_some_and(|motion| motion.has_input);
        let input_yaw_angle = motion.map(|motion| motion.input_yaw_degrees);
        let horizontal_speed = motion.map(|motion| motion.horizontal_speed);
        let has_velocity = motion.is_some_and(|motion| als_core::motion::locomotion_movement_flags(motion.has_input, motion.horizontal_speed).0);
        let velocity_direction = motion.map(|motion| if motion.world_velocity.length_squared() < 1e-12 {
            Vec3::ZERO
        } else { motion.world_velocity.normalize_or_zero() });
        let velocity_yaw_angle = motion.map(|motion| motion.velocity_yaw_degrees);
        Self {
            first_person,
            locomotion_state,
            rotation_mode,
            stance,
            gait,
            overlay_state,
            desired_rotation_mode,
            desired_stance,
            desired_gait,
            desired_aiming: aiming,
            input_yaw_angle,
            has_input,
            speed: horizontal_speed,
            has_velocity,
            velocity_direction,
            velocity_yaw_angle,
            locomotion_action,
            view_rotation_deg: None,
            smooth_target_yaw_angle: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CameraSnapshot {
    pub rig_mode: Option<AlsCameraMode>,
    pub pivot_target_location: Option<Vec3>,
    pub pivot_lag_location: Option<Vec3>,
    pub pivot_location: Option<Vec3>,
    pub field_of_view_deg: Option<f32>,
    pub right_shoulder: Option<bool>,
    pub camera_curves: Vec<(String, f32)>,
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
    locomotion_state.as_str().rsplit('.').next().unwrap_or("")
}

fn rotation_mode_tag(mode: RotationMode) -> &'static str { mode.leaf_name() }

fn stance_tag(stance: Stance) -> &'static str { stance.leaf_name() }

fn gait_tag(gait: Gait) -> &'static str { gait.leaf_name() }

fn overlay_tag(overlay: OverlayState) -> &'static str { overlay.leaf_name() }

fn locomotion_action_value(snapshot: &CharacterSnapshot) -> &'static str { snapshot.locomotion_action.leaf_name() }

pub fn build_state_body(
    snapshot: &CharacterSnapshot,
) -> Vec<DebugRow> {
    vec![
        DebugRow::pair(
            name_to_display_string("ViewMode", false),
            tag_value(if snapshot.first_person { "FirstPerson" } else { "ThirdPerson" }),
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

pub fn build_shapes_body(snapshot: &CharacterSnapshot) -> Vec<DebugRow> {
    let rotation_value = snapshot
        .view_rotation_deg
        .map(|(roll, pitch, yaw)| format!("R: {:.2}P: {pitch:.2}Y: {:.2}", if roll == 0.0 { 0.0 } else { -roll }, source_heading(yaw)))
        .unwrap_or_else(|| NA_VALUE.into());
    let input_color = if snapshot.has_input {
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
                .map(|v| format!("{:.2}", source_heading(v)))
                .unwrap_or_else(|| NA_VALUE.into()),
            input_color,
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("Speed", false),
            snapshot
                .speed
                .map(|v| format!("{:.2}", v * 100.0))
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
                .map(|v| format!("{:.2}", source_heading(v)))
                .unwrap_or_else(|| NA_VALUE.into()),
            speed_color,
            LABEL_WIDTH_STATE_PX,
        ),
        DebugRow::pair(
            name_to_display_string("SmoothTargetYawAngle", false),
            snapshot.smooth_target_yaw_angle.map(|value| format!("{:.2}", source_heading(value))).unwrap_or_else(|| NA_VALUE.into()),
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
        .map(|value| format_xyz(value * 100.0))
        .unwrap_or_else(|| NA_VALUE.into());
    let pivot_lag_value = camera
        .pivot_lag_location
        .map(|value| format_xyz(value * 100.0))
        .unwrap_or_else(|| NA_VALUE.into());
    let pivot_location_value = camera
        .pivot_location
        .map(|value| format_xyz(value * 100.0))
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
    format!("X: {:.2}Y: {:.2}Z: {:.2}", value.x, value.z, value.y)
}

fn source_heading(value: f32) -> f32 {
    als_core::turn_in_place::wrap_angle_degrees(-90.0 - value)
}

fn append_section(column: &mut DebugColumn, y: &mut f32, category: DebugCategory, active: bool, rows: Vec<DebugRow>) {
    let mut header = DebugRow::header_row(category.header(), if active { header_active_color() } else { header_inactive_color() });
    header.y = *y;
    column.push(header);
    *y += 15.0;
    if active {
        *y += 4.0;
        for mut row in rows {
            row.y = *y;
            column.push(row);
            *y += 12.0;
        }
    }
    *y += 12.0;
}

pub fn layout_character_block(
    state: &AlsShowDebugHudState,
    snapshot: &CharacterSnapshot,
    curves_rows: &[DebugRow],
) -> DebugBlock {
    let mut columns = vec![vec![DebugRow::header_row(HEADER_DEBUG_MODE, header_active_color())]];
    if ![DebugCategory::Curves, DebugCategory::State, DebugCategory::Shapes, DebugCategory::Traces, DebugCategory::Mantling]
        .into_iter().any(|category| state.is_active(category)) { return columns; }
    let mut y = 27.0;
    let curves_active = state.is_active(DebugCategory::Curves);
    append_section(&mut columns[0], &mut y, DebugCategory::Curves, curves_active, curves_rows.to_vec());
    if curves_active { columns.push(Vec::new()); y = 27.0; }
    for category in [DebugCategory::State, DebugCategory::Shapes, DebugCategory::Traces, DebugCategory::Mantling] {
        let active = state.is_active(category);
        let rows = if active { match category {
            DebugCategory::State => build_state_body(snapshot),
            DebugCategory::Shapes => build_shapes_body(snapshot),
            DebugCategory::Traces => build_traces_body(),
            DebugCategory::Mantling => build_mantling_body(),
            _ => Vec::new(),
        }} else { Vec::new() };
        append_section(columns.last_mut().unwrap(), &mut y, category, active, rows);
    }
    columns
}

pub fn layout_camera_block(
    state: &AlsShowDebugHudState,
    camera: &CameraSnapshot,
    camera_curves_rows: &[DebugRow],
) -> DebugBlock {
    if camera.rig_mode.is_none() || ![DebugCategory::CameraCurves, DebugCategory::CameraShapes, DebugCategory::CameraTraces]
        .into_iter().any(|category| state.is_active(category)) { return Vec::new(); }
    let mut columns = vec![Vec::new()];
    let mut y = 0.0;
    let curves_active = state.is_active(DebugCategory::CameraCurves);
    append_section(&mut columns[0], &mut y, DebugCategory::CameraCurves, curves_active, camera_curves_rows.to_vec());
    if curves_active { columns.push(Vec::new()); y = 0.0; }
    for category in [DebugCategory::CameraShapes, DebugCategory::CameraTraces] {
        let active = state.is_active(category);
        let rows = if active { match category {
            DebugCategory::CameraShapes => build_camera_shapes_body(camera),
            DebugCategory::CameraTraces => build_camera_traces_body(),
            _ => Vec::new(),
        }} else { Vec::new() };
        append_section(columns.last_mut().unwrap(), &mut y, category, active, rows);
    }
    columns
}

fn block_height(block: &DebugBlock) -> f32 {
    block.iter().filter_map(|column| column.last()).map(|row| row.y + (if row.font_px == HEADER_FONT_PX { 15.0 } else { 12.0 }) + 12.0).fold(0.0, f32::max)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CanvasMetrics {
    als_scale: f32,
    canvas_to_ui: f32,
    host_scale: f32,
}

impl CanvasMetrics {
    fn new(physical_size: Vec2, dpi: f32, ui_scale: f32) -> Self {
        Self {
            als_scale: (physical_size.x / (1280.0 * dpi)).min(physical_size.y / (720.0 * dpi)) / ui_scale,
            canvas_to_ui: 1.0 / ui_scale,
            host_scale: (physical_size.x / (1920.0 * dpi)).max(1.0) / ui_scale,
        }
    }

    #[cfg(test)]
    fn actor_start_y(self) -> f32 {
        50.0 * self.canvas_to_ui + HOST_MAX_CHAR_HEIGHT * 1.15 * 2.0 * self.host_scale
    }
}
type Layout = Vec<DebugBlock>;

enum HudCache {
    Hidden,
    Shown(Layout, CanvasMetrics, canvas::HostLayout),
}

#[derive(Component)]
pub struct AlsShowDebugHudRoot;

pub struct AlsShowDebugHudPlugin;

impl Plugin for AlsShowDebugHudPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<crate::debug_target::AlsDebugTargetPlugin>() { app.add_plugins(crate::debug_target::AlsDebugTargetPlugin); }
        app.init_resource::<AlsShowDebugHudState>()
            .init_resource::<canvas::ColumnNavigation>()
            .configure_sets(
                PostUpdate,
                AlsShowDebugHudSet::Sync.after(crate::animation_context_projection::AlsRootAnimationContextSet)
                    .after(bevy::text::load_font_assets_into_font_collection).before(bevy::text::detect_text_needs_rerender)
                    .after(crate::actual_motion::AlsActualMotionSet)
                    .after(als_camera::AlsCameraSet).after(als_core::motion::AlsRotationResponseSet)
                    .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate)
                    .before(bevy::ui::UiSystems::Stack)
                    .before(bevy::ui::UiSystems::Prepare),
            )
            .add_systems(Startup, load_debug_categories)
            .add_systems(Update, keyboard_system.in_set(AlsShowDebugHudSet::Input))
            .add_systems(PostUpdate, hud_sync_system.in_set(AlsShowDebugHudSet::Sync));
    }
}

fn load_debug_categories(file: Option<Res<AlsShowDebugSettingsFile>>, mut state: ResMut<AlsShowDebugHudState>) {
    let Some(file) = file else { return; };
    match std::fs::read_to_string(&file.0) {
        Ok(text) => match ron::from_str(&text) {
            Ok(saved) => *state = saved,
            Err(error) => warn!("Debug category settings could not be parsed: {error}"),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(error) => warn!("Debug category settings could not be read: {error}"),
    }
}

fn keyboard_system(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<AlsShowDebugHudState>, mut columns: ResMut<canvas::ColumnNavigation>, file: Option<Res<AlsShowDebugSettingsFile>>) {
    if keys.just_pressed(KeyCode::ShiftRight) { columns.selected = (columns.selected + 1).min(columns.max_columns); }
    if keys.just_pressed(KeyCode::ShiftLeft) { columns.selected = columns.selected.saturating_sub(1); }
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
        if keys.just_pressed(code) && apply_show_debug_key(&mut state, shift_held, code)
            && code != KeyCode::Digit0 && let Some(file) = file.as_ref()
        {
            let save = || -> Result<(), Box<dyn std::error::Error>> {
                if let Some(parent) = file.0.parent().filter(|path| !path.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&file.0, ron::to_string(&*state)?)?;
                Ok(())
            };
            if let Err(error) = save() { warn!("Debug category settings could not be saved: {error}"); }
        }
    }
}

fn build_camera_snapshot(
    rig: Option<&AlsCameraRig>,
    debug_info: Option<&als_camera::AlsCameraDebugInfo>,
    context: Option<&AlsRootAnimationContext>,
) -> CameraSnapshot {
    let debug_info = debug_info.filter(|info| info.valid);
    static MAPPINGS: LazyLock<std::collections::HashMap<String, (String, f32)>> = LazyLock::new(||
        serde_json::from_str(include_str!("../assets/ui/als_camera_curves.runtime.json")).expect("Camera display mappings are valid"));
    let mut camera_curves: Vec<_> = context.into_iter().flat_map(|context| &context.curves).map(|(name, value)| {
        let (name, value) = MAPPINGS.get(name).map_or((name.clone(), *value), |(source, scale)| (source.clone(), value * scale));
        (name, if value == 0.0 { 0.0 } else { value })
    }).collect();
    camera_curves.sort_by(|a, b| a.0.cmp(&b.0));
    CameraSnapshot {
        rig_mode: rig.map(|rig| rig.mode),
        pivot_target_location: debug_info.map(|info| info.pivot_target),
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
    mut panel: Local<(Option<Entity>, Vec<Entity>)>,
    mut cache: Local<Option<HudCache>>,
    q_character: Query<
        (
            Entity,
            &'static LocomotionState,
            &'static RotationMode,
            &'static Stance,
            &'static Gait,
            Option<&'static AlsDesiredGait>,
            &'static OverlayState,
            Option<&'static crate::actual_motion::AlsActualMotion>,
            Option<&'static AlsInputCommand>,
            Option<&'static AlsInputOverride>,
            Option<&'static AlsRollState>,
            Option<&'static AlsMantleState>,
            Option<&'static AlsNotifyLocomotionAction>,
        ),
        (With<AlsCharacter>, With<crate::debug_target::AlsDebugTarget>),
    >,
    assemblies: Query<&AlsCharacterAssembly>,
    views: Query<(&als_camera::AlsCameraViewSource, &als_camera::AlsCharacterCamera)>,
    camera_components: Query<(&AlsSourceCameraComponent, &als_camera::AlsCharacterCamera, &AlsCameraRig, &als_camera::AlsCameraDebugInfo, Option<&AnimatedSceneInstance>)>,
    q_instances: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    q_players: Query<&AlsRootAnimationContext, With<AnimationGraphPlayer>>,
    details: Query<(Option<&als_core::motion::AlsRotationResponseState>, Option<&als_core::motion::AlsViewYaw>, Option<&als_core::view::AlsViewPitch>,
        Option<&als_core::aim::AlsLastFreeRotationMode>, Option<&als_core::transition::AlsDesiredStance>, Has<als_core::input_request::AlsFirstPersonView>)>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    ui_scale: Option<Res<UiScale>>,
    entities: Query<(Option<&Name>, Option<&GlobalTransform>, Option<&ChildOf>, Has<als_core::input::OnFoot>)>,
    mut text_measure: canvas::TextMeasure,
    mut navigation: ResMut<canvas::ColumnNavigation>,
) {
    let character_item = q_character.single().ok();
    let metrics = windows.single().ok().map(|window| CanvasMetrics::new(
        Vec2::new(window.physical_width() as f32, window.physical_height() as f32), window.scale_factor(),
        ui_scale.as_ref().map_or(1.0, |scale| scale.0))).unwrap_or_else(|| CanvasMetrics::new(Vec2::new(1280.0, 720.0), 1.0, 1.0));
    let clip_y = windows.single().ok().map(|window| window.physical_height() as f32 / (window.scale_factor() * ui_scale.as_ref().map_or(1.0, |scale| scale.0))).unwrap_or(720.0);
    let target = if state.visible && character_item.is_some() {
        let item = character_item.unwrap();
        let (
            character,
            locomotion_state,
            rotation_mode,
            stance,
            gait,
            desired_gait,
            overlay_state,
            actual_motion,
            input,
            input_override,
            roll,
            mantle,
            action,
        ) = item;

        let aiming = input_override.map(|o| &o.0).or(input).is_some_and(|input| input.aim_held);
        let (response, yaw, pitch, desired_rotation, desired_stance, first_person) = details.get(character).unwrap();
        let mut snapshot = CharacterSnapshot::from_components(
            *locomotion_state,
            *rotation_mode,
            *stance,
            *gait,
            desired_gait.map(|value| value.0).unwrap_or(*gait),
            *overlay_state,
            desired_rotation.map_or(*rotation_mode, |value| value.0),
            desired_stance.map_or(*stance, |value| value.0),
            first_person,
            actual_motion,
            aiming,
            action.copied().unwrap_or_else(|| {
                if mantle.is_some_and(|m| m.active) { AlsNotifyLocomotionAction::Mantling }
                else if roll.is_some_and(|r| r.active) { AlsNotifyLocomotionAction::Rolling }
                else { AlsNotifyLocomotionAction::None }
            }),
        );

        snapshot.smooth_target_yaw_angle = response.map(|response| response.smooth_target_yaw.to_degrees());
        snapshot.view_rotation_deg = yaw.zip(pitch).map(|(yaw, pitch)| (0.0, pitch.0.to_degrees(), yaw.0.to_degrees()));
        let assembly = assemblies.get(character).ok();
        let camera = assembly.and_then(|assembly| assembly.camera).and_then(|entity| views.get(entity).ok())
            .filter(|(_, owner)| owner.character == character)
            .and_then(|(source, _)| camera_components.get(source.0).ok())
            .filter(|(component, owner, _, _, _)| component.is_active() && owner.character == character);
        let camera_context = camera.and_then(|(_, _, _, _, instance)| instance).and_then(|instance| q_players.get(instance.player_entity()).ok());
        let camera_snapshot = build_camera_snapshot(camera.map(|(_, _, rig, _, _)| rig), camera.map(|(_, _, _, info, _)| info), camera_context);
        let context = assembly.and_then(|assembly| q_instances.get(assembly.avatar).ok())
            .filter(|(owner, _)| owner.character == character)
            .and_then(|(_, instance)| q_players.get(instance.player_entity()).ok());
        let character_curve_rows = evaluated_character_curve_rows(context);
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

        let blocks = vec![
            layout_camera_block(&state, &camera_snapshot, &camera_curve_rows),
            layout_character_block(&state, &snapshot, &character_curve_rows),
        ];
        let (name, transform, parent, on_foot) = entities.get(character).unwrap();
        let name = name.map(|name| name.to_string()).unwrap_or_else(|| format!("AlsCharacter {character}"));
        let parent = parent.map(|parent| entities.get(parent.parent()).ok().and_then(|(name, _, _, _)| name)
            .map(|name| name.to_string()).unwrap_or_else(|| parent.parent().to_string())).unwrap_or_else(|| "None".into());
        let mut diagnostics = vec![(format!("ENTITY {name} [{character}]"), linear(1.0, 0.0, 0.0))];
        if let Some(transform) = transform {
            let position = transform.translation();
            let rotation = transform.rotation();
            let source = Quat::from_xyzw(-rotation.x, -rotation.z, -rotation.y, rotation.w);
            let (yaw, pitch, roll) = source.to_euler(EulerRot::ZYX);
            diagnostics.push((format!("Location (cm): X={:.2} Y={:.2} Z={:.2} Rotation (deg): P={:.2} Y={:.2} R={:.2}",
                position.x * 100.0, position.z * 100.0, position.y * 100.0, -pitch.to_degrees(), yaw.to_degrees(), -roll.to_degrees()), white()));
        }
        diagnostics.push((format!("Owner: {parent} Instigator: {name} [{character}]"), white()));
        diagnostics.push((format!("INPUT CONTEXT {}", if on_foot { "OnFoot" } else { "None" }), linear(1.0, 0.0, 0.0)));
        diagnostics.push((format!("STATE {}", if on_foot { "Active" } else { "No input context" }), linear(1.0, 1.0, 0.0)));
        let body_height = blocks.iter().map(block_height).sum::<f32>() * metrics.als_scale;
        let host = canvas::host_layout(metrics, clip_y, &name, body_height, &diagnostics, &mut navigation, &mut text_measure);
        HudCache::Shown(blocks, metrics, host)
    } else if state.visible {
        HudCache::Shown(Vec::new(), metrics, canvas::host_layout(metrics, clip_y, "None", 0.0, &[], &mut navigation, &mut text_measure))
    } else {
        HudCache::Hidden
    };

    let unchanged = match (cache.as_ref(), &target) {
        (Some(HudCache::Hidden), HudCache::Hidden) => true,
        (Some(HudCache::Shown(current, current_scale, current_name)), HudCache::Shown(next, next_scale, next_name)) => current == next && current_scale == next_scale && current_name == next_name,
        _ => false,
    };
    if unchanged {
        return;
    }
    let (root, leaves) = &mut *panel;
    let root = *root.get_or_insert_with(|| spawn_hidden_root(&mut commands));
    match &target {
        HudCache::Shown(blocks, metrics, host) => {
            sync_panel_with_host(&mut commands, root, leaves, blocks, *metrics, host);
        }
        HudCache::Hidden => {
            commands.entity(root).insert(Visibility::Hidden);
            for entity in leaves.drain(..) { commands.entity(entity).despawn(); }
        }
    }
    *cache = Some(target);
}

fn evaluated_character_curve_rows(context: Option<&AlsRootAnimationContext>) -> Vec<DebugRow> {
    static NAMES: LazyLock<Vec<String>> = LazyLock::new(|| {
        let data: serde_json::Value = serde_json::from_str(include_str!("../assets/ui/als_character_curves.runtime.json")).unwrap();
        data["curves"].as_array().unwrap().iter().map(|name| name.as_str().unwrap().to_owned()).collect()
    });
    NAMES.iter().map(|name| {
        let value = context.map(|context| context.f32(name).unwrap_or(0.0));
        let value = value.map(|value| if value != 0.0 && matches!(name.as_str(), "RotationYawOffset" | "RotationYawSpeed") { -value } else { value });
        DebugRow::pair(name_to_display_string(name, false), value.map(|value| format!("{value:.2}")).unwrap_or_else(|| NA_VALUE.into()),
            curve_color(value.unwrap_or(0.0), false), LABEL_WIDTH_CURVES_PX)
    }).collect()
}

fn spawn_hidden_root(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            AlsShowDebugHudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            bevy::ui::LayoutConfig { use_rounding: false },
            BackgroundColor(Color::linear_rgba(0.0, 0.0, 0.0, 0.2)),
            GlobalZIndex(20),
            Pickable::IGNORE,
            Visibility::Hidden,
        ))
        .id()
}

#[cfg(test)]
fn spawn_panel(commands: &mut Commands, blocks: &[DebugBlock], metrics: CanvasMetrics, target_name: &str) -> Entity {
    let host = canvas::HostLayout {
        actor_y: metrics.actor_start_y(),
        lines: [format!("Showing Debug for {target_name}, Press [PageUp] and [PageDown] to cycle between targets."),
            "Press [Right Shift] or [Left Shift] to move the debug menu a column over to see more of the debug menu".to_owned()]
            .into_iter().enumerate().map(|(index, text)| (text, linear(1.0, 0.0, 0.0),
                Vec2::new(4.0 * metrics.canvas_to_ui, 50.0 * metrics.canvas_to_ui + index as f32 * HOST_MAX_CHAR_HEIGHT * 1.15 * metrics.host_scale))).collect(),
    };
    let root = spawn_hidden_root(commands);
    sync_panel_with_host(commands, root, &mut Vec::new(), blocks, metrics, &host);
    root
}

fn sync_panel_with_host(commands: &mut Commands, root: Entity, leaves: &mut Vec<Entity>, blocks: &[DebugBlock], metrics: CanvasMetrics, host: &canvas::HostLayout) {
    commands.entity(root).insert(Visibility::Visible).with_children(|parent| {
        let mut previous = std::mem::take(leaves).into_iter();
        let mut leaf = |text: &str, color: Color, font_px: f32, position: Vec2, weight: FontWeight, name: Option<Name>| {
            let entity = previous.next().unwrap_or_else(|| parent.spawn_empty().id());
            let mut commands = parent.commands();
            let mut entity_commands = commands.entity(entity);
            entity_commands.insert((
                Text::new(text),
                TextFont { font_size: FontSize::Px(font_px), weight, ..default() },
                TextLayout::new(Justify::Left, bevy::text::LineBreak::NoWrap),
                TextColor(color),
                Pickable::IGNORE,
                TextShadow { offset: Vec2::splat(metrics.canvas_to_ui), color: Color::BLACK },
                Node { position_type: PositionType::Absolute, left: Val::Px(position.x), top: Val::Px(position.y), ..default() },
            ));
            if let Some(name) = name { entity_commands.insert(name); } else { entity_commands.remove::<Name>(); }
            leaves.push(entity);
        };
        for (index, (text, color, position)) in host.lines.iter().enumerate() {
            leaf(text, *color, HOST_FONT_PX * metrics.host_scale, *position, FontWeight::LIGHT,
                Some(Name::new(if index == 0 { "DebugHostTarget" } else if index == 1 { "DebugHostHelp" } else { "DebugHostIdentity" })));
        }
        let mut y = host.actor_y;
        let scale = metrics.als_scale;
        for block in blocks {
            for (index, column) in block.iter().enumerate() {
                for row in column {
                    let position = Vec2::new((ROOT_OFFSET_PX + index as f32 * COLUMN_WIDTH_PX) * scale, y + row.y * scale);
                    leaf(&row.label, row.color, row.font_px * scale, position, FontWeight::NORMAL,
                        (row.font_px == HEADER_FONT_PX).then(|| Name::new(format!("DebugHeader:{}", row.label))));
                    if row.label_width_px > 0.0 {
                        leaf(&row.value, row.color, row.font_px * scale,
                            position + Vec2::X * row.label_width_px * scale, FontWeight::NORMAL, Some(Name::new(row.label.clone())));
                    }
                }
            }
            y += block_height(block) * scale;
        }
        for entity in previous { parent.commands().entity(entity).despawn(); }
    });
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_host_remains_visible_without_an_actor_target() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsShowDebugHudPlugin)).init_resource::<ButtonInput<KeyCode>>();
        app.world_mut().resource_mut::<AlsShowDebugHudState>().visible = true;
        app.update();
        assert!(app.world_mut().query::<&Text>().iter(app.world()).any(|text| text.0.starts_with("Showing Debug for None,")));
        app.world_mut().resource_mut::<AlsShowDebugHudState>().visible = false;
        app.update();
        assert!(!app.world_mut().query::<&Text>().iter(app.world()).any(|text| text.0.starts_with("Showing Debug for")));
    }

    #[test]
    fn native_font_host_diagnostics_and_column_keys_use_the_live_ecs_target() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default(), bevy::text::TextPlugin, AlsShowDebugHudPlugin))
            .init_resource::<ButtonInput<KeyCode>>();
        let actor = app.world_mut().spawn((AlsCharacter, crate::character_assembly::AlsPlayerCharacter, Name::new("DiagnosticActor"),
            GlobalTransform::from_translation(Vec3::new(1.0, 2.0, 3.0)), LocomotionState::Grounded,
            RotationMode::VelocityDirection, Stance::Standing, Gait::Walking, OverlayState::Default,
            AlsStateRequest::default(), als_core::input::OnFoot)).id();
        *app.world_mut().resource_mut::<AlsShowDebugHudState>() = AlsShowDebugHudState { visible: true, curves: true, state: true, shapes: true, ..default() };
        for _ in 0..4 { app.update(); }
        let labels: Vec<_> = app.world_mut().query::<&Text>().iter(app.world()).map(|text| text.0.clone()).collect();
        assert!(labels.iter().any(|text| text.starts_with("ENTITY DiagnosticActor")));
        assert!(labels.iter().any(|text| text.contains("X=100.00 Y=300.00 Z=200.00")));
        assert!(labels.iter().any(|text| text == "INPUT CONTEXT OnFoot"));
        let root = app.world_mut().query_filtered::<Entity, With<AlsShowDebugHudRoot>>().single(app.world()).unwrap();
        let leaves = app.world().get::<Children>(root).unwrap().to_vec();
        for x in [2.0, 3.0] {
            app.world_mut().entity_mut(actor).insert(GlobalTransform::from_translation(Vec3::new(x, 2.0, 3.0)));
            app.update();
            assert_eq!(app.world_mut().query_filtered::<Entity, With<AlsShowDebugHudRoot>>().single(app.world()).unwrap(), root);
            assert_eq!(app.world().get::<Children>(root).unwrap().to_vec(), leaves);
            let expected = format!("X={:.2} Y=300.00 Z=200.00", x * 100.0);
            assert!(app.world_mut().query::<&Text>().iter(app.world()).any(|text| text.0.contains(&expected)));
        }
        assert!(app.world().resource::<canvas::ColumnNavigation>().max_columns > 0);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftRight);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
        app.update();
        let header_x = |world: &mut World| world.query::<(&Name, &Node)>().iter(world).find(|(name, _)| name.as_str() == "DebugHostTarget").unwrap().1.left;
        assert!(matches!(header_x(app.world_mut()), Val::Px(x) if x < 0.0));
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
        app.update();
        assert_eq!(header_x(app.world_mut()), Val::Px(4.0));
    }

    #[test]
    fn host_frame_has_the_source_backdrop_and_header_before_als_rows() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let root = world.run_system_once(|mut commands: Commands| {
            spawn_panel(&mut commands, &[], CanvasMetrics::new(Vec2::new(1280.0, 720.0), 1.0, 1.0), "TestActor")
        }).unwrap();
        assert_eq!(world.get::<BackgroundColor>(root).map(|color| color.0), Some(Color::linear_rgba(0.0, 0.0, 0.0, 0.2)));
        assert_eq!(world.get::<GlobalZIndex>(root).unwrap().0, 20);
        let labels: Vec<_> = world.query::<(&Text, &Node)>().iter(&world).collect();
        let (_, first) = labels.iter().find(|(text, _)| text.0.starts_with("Showing Debug for ")).expect("The AHUD entry header must be drawn");
        assert_eq!(first.left, Val::Px(4.0));
        assert_eq!(first.top, Val::Px(50.0));
        assert!(labels.iter().any(|(text, _)| text.0.starts_with("Press [Right Shift]")));
    }

    #[test]
    fn debug_keyboard_accepts_extra_modifiers_and_only_fires_on_press() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsShowDebugHudPlugin)).init_resource::<ButtonInput<KeyCode>>();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit0);
        app.update();
        assert!(!app.world().resource::<AlsShowDebugHudState>().visible);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().reset_all();
        for key in [KeyCode::ShiftRight, KeyCode::ControlLeft, KeyCode::AltLeft, KeyCode::SuperLeft, KeyCode::Digit0] {
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
        }
        app.update();
        assert!(app.world().resource::<AlsShowDebugHudState>().visible);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
        app.update();
        assert!(app.world().resource::<AlsShowDebugHudState>().visible);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::Digit0);
        app.update();
        assert!(app.world().resource::<AlsShowDebugHudState>().visible);
    }

    #[test]
    fn categories_survive_restart_but_master_does_not() {
        let path = std::env::temp_dir().join(format!("als-debug-settings-{}.ron", std::process::id()));
        std::fs::write(&path, "(curves:true,state:false,shapes:false,traces:false,mantling:false,camera_curves:false,camera_shapes:false,camera_traces:false)").unwrap();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsShowDebugHudPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(AlsShowDebugSettingsFile(path.clone()));
        app.update();
        assert!(app.world().resource::<AlsShowDebugHudState>().curves);
        assert!(!app.world().resource::<AlsShowDebugHudState>().visible);
        let before = std::fs::read_to_string(&path).unwrap();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftRight);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit0);
        app.update();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit2);
        app.update();
        let mut restarted = App::new();
        restarted.add_plugins((MinimalPlugins, AlsShowDebugHudPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(AlsShowDebugSettingsFile(path.clone()));
        restarted.update();
        let state = restarted.world().resource::<AlsShowDebugHudState>();
        assert!(state.curves && state.state && !state.visible);
        std::fs::remove_file(path).unwrap();
    }

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
        CharacterSnapshot::from_components(
            LocomotionState::Grounded,
            RotationMode::ViewDirection,
            Stance::Standing,
            Gait::Walking,
            Gait::Running,
            OverlayState::Default,
            RotationMode::ViewDirection,
            Stance::Standing,
            false,
            Some(&crate::actual_motion::AlsActualMotion { world_velocity: Vec3::new(3.0, 0.0, -4.0), horizontal_speed: 5.0,
                has_input: true, input_yaw_degrees: 90.0, velocity_yaw_degrees: -36.869_896, ..default() }),
            true,
            AlsNotifyLocomotionAction::None,
        )
    }

    #[test]
    fn hud_reads_the_controlled_character_current_instance_and_final_curves() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::{animation_context_projection::AlsRootAnimationContext,
            camera_component::AlsSourceCameraComponent,
            character_assembly::{AlsCharacterAssembly, AlsPlayerCharacter, ALS_LINKED_ANIMATED_SCENE, attach_avatar}};
        let (mut app, character, old_avatar) = crate::roll_drive::tests::loaded_roll_character();
        let old_player = app.world().get::<AnimatedSceneInstance>(old_avatar).unwrap().player_entity();
        app.world_mut().get_mut::<AnimationGraphPlayer>(old_player).unwrap().pause();
        let avatar = app.world_mut().run_system_once_with(|In(character): In<Entity>, mut commands: Commands, server: Res<AssetServer>| {
            attach_avatar(&mut commands, &server, character, ALS_LINKED_ANIMATED_SCENE)
        }, character).unwrap();
        app.world_mut().get_mut::<AlsCharacterAssembly>(character).unwrap().avatar = avatar;
        let player = crate::roll_drive::tests::ready_roll_player(&mut app, avatar);
        let marked: Vec<_> = app.world_mut().query_filtered::<Entity, With<AlsPlayerCharacter>>().iter(app.world()).collect();
        for entity in marked { app.world_mut().entity_mut(entity).remove::<AlsPlayerCharacter>(); }
        app.world_mut().entity_mut(character).insert((AlsPlayerCharacter, crate::debug_target::AlsDebugTarget, Stance::Crouching,
            als_core::motion::AlsRotationResponseState { initialized: true, smooth_target_yaw: 70.0_f32.to_radians(), ..default() },
            als_core::motion::AlsViewYaw(40.0_f32.to_radians()), als_core::view::AlsViewPitch(-15.0_f32.to_radians())));
        app.world_mut().entity_mut(old_player).insert(AlsRootAnimationContext { curves: [("PoseGait".into(), 9.5)].into() });
        app.world_mut().entity_mut(player).insert(AlsRootAnimationContext { curves: [("PoseGait".into(), 2.25), ("FootLeftIk".into(), 0.73)].into() });
        let mut component = AlsSourceCameraComponent::default();
        component.activate(false);
        let camera = app.world_mut().spawn((component, als_camera::AlsCharacterCamera { character }, AlsCameraRig::default(),
            als_camera::AlsCameraDebugInfo { valid: true, field_of_view_deg: 53.0, ..default() })).id();
        let view = app.world_mut().spawn((Camera3d::default(), als_camera::AlsCameraViewSource(camera), als_camera::AlsCharacterCamera { character })).id();
        app.world_mut().get_mut::<AlsCharacterAssembly>(character).unwrap().camera = Some(view);
        #[derive(Component)]
        struct LateDebugTarget;
        app.world_mut().spawn((AlsCharacter, LocomotionState::Air, RotationMode::Aiming, Stance::Standing,
            Gait::Sprinting, OverlayState::Rifle, AlsStateRequest::default()));
        app.world_mut().entity_mut(character).insert(LateDebugTarget);
        app.world_mut().entity_mut(character).insert((AlsInputCommand { aim_held: true, ..default() }, AlsInputOverride::default()));
        app.init_resource::<AlsShowDebugHudState>();
        app.init_resource::<canvas::ColumnNavigation>();
        *app.world_mut().resource_mut::<AlsShowDebugHudState>() = AlsShowDebugHudState {
            visible: true, curves: true, state: true, shapes: true, camera_shapes: true, ..default()
        };
        let system = app.world_mut().register_system(hud_sync_system);
        let texts = |world: &mut World| world.query::<&Text>().iter(world).map(|text| text.0.clone()).collect::<Vec<_>>();
        app.world_mut().run_system(system).unwrap();
        let rendered = texts(app.world_mut());
        assert!(app.world_mut().query::<(&Name, &Text)>().iter(app.world())
            .any(|(name, text)| name.as_str() == "Desired Aiming" && text.0 == "False"));
        for expected in ["Crouching", "-160.00", "53.00", "2.25", "0.73", "R: 0.00P: -15.00Y: -130.00"] {
            assert!(rendered.iter().any(|text| text == expected), "Missing current value {expected}: {rendered:?}");
        }
        let metadata: serde_json::Value = serde_json::from_str(include_str!("../assets/ui/als_character_curves.runtime.json")).unwrap();
        for name in metadata["curves"].as_array().unwrap() {
            assert!(rendered.contains(&name_to_display_string(name.as_str().unwrap(), false)));
        }
        app.world_mut().get_mut::<AlsCharacterAssembly>(character).unwrap().avatar = old_avatar;
        app.world_mut().run_system(system).unwrap();
        let rendered = texts(app.world_mut());
        assert!(rendered.iter().any(|text| text == "9.50"));
        assert!(!rendered.iter().any(|text| text == "2.25"));
        app.world_mut().get_mut::<AlsCharacterAssembly>(character).unwrap().avatar = Entity::PLACEHOLDER;
        app.world_mut().run_system(system).unwrap();
        assert!(!texts(app.world_mut()).iter().any(|text| text == "9.50"));
    }

    #[test]
    fn state_rows_read_persistent_intent_and_view_without_a_render_camera() {
        use als_core::{aim::AlsLastFreeRotationMode, input_request::AlsFirstPersonView, transition::AlsDesiredStance};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsShowDebugHudPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(AlsShowDebugHudState { visible: true, state: true, ..default() });
        let actor = app.world_mut().spawn((AlsCharacter, crate::character_assembly::AlsPlayerCharacter,
            LocomotionState::Grounded, RotationMode::Aiming, Stance::Crouching, Gait::Running,
            AlsDesiredStance(Stance::Standing), AlsLastFreeRotationMode(RotationMode::ViewDirection),
            AlsDesiredGait(Gait::Sprinting), AlsFirstPersonView, OverlayState::Rifle,
            AlsInputCommand { aim_held: true, ..default() },
            AlsStateRequest { stance: Some(Stance::Crouching), rotation_mode: Some(RotationMode::VelocityDirection), ..default() })).id();
        app.update();
        let values = |world: &mut World| world.query::<(&Name, &Text)>().iter(world)
            .map(|(name, text)| (name.to_string(), text.0.clone())).collect::<std::collections::HashMap<_, _>>();
        let rows = values(app.world_mut());
        for (name, expected) in [("View Mode", "First Person"), ("Desired Stance", "Standing"), ("Stance", "Crouching"),
            ("Desired Rotation Mode", "View Direction"), ("Rotation Mode", "Aiming"), ("Desired Gait", "Sprinting"), ("Gait", "Running")] {
            assert_eq!(rows[name], expected, "{name}");
        }
        app.world_mut().entity_mut(actor).remove::<AlsFirstPersonView>().insert((
            AlsDesiredStance(Stance::new("Als.Stance.CustomLean").unwrap()),
            AlsLastFreeRotationMode(RotationMode::new("Als.RotationMode.CustomFacing").unwrap()),
            AlsNotifyLocomotionAction::None));
        app.update();
        let rows = values(app.world_mut());
        assert_eq!(rows["View Mode"], "Third Person");
        assert_eq!(rows["Desired Stance"], "Custom Lean");
        assert_eq!(rows["Desired Rotation Mode"], "Custom Facing");
        assert_eq!(rows["Locomotion Action"], "None");
    }

    #[test]
    fn state_body_contains_expected_labels_and_values() {
        let rows = build_state_body(&sample_snapshot());
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
    fn shape_values_use_source_axes_centimeters_and_heading_basis() {
        let mut snapshot = sample_snapshot();
        snapshot.view_rotation_deg = Some((0.0, 25.0, 0.0));
        snapshot.input_yaw_angle = Some(0.0);
        snapshot.velocity_yaw_angle = Some(-90.0);
        snapshot.smooth_target_yaw_angle = Some(90.0);
        let rows = build_shapes_body(&snapshot);
        assert_eq!(rows[0].value, "R: 0.00P: 25.00Y: -90.00");
        assert_eq!(rows[1].value, "-90.00");
        assert_eq!(rows[2].value, "500.00");
        assert_eq!(rows[3].value, "X: 0.60Y: -0.80Z: 0.00");
        assert_eq!(rows[4].value, "0.00");
        assert_eq!(rows[5].value, "180.00");
        for speed in [0.0, 0.5e-6, 2.0e-6] {
            let motion = crate::actual_motion::AlsActualMotion { world_velocity: Vec3::Y * speed, ..default() };
            let snapshot = CharacterSnapshot::from_components(LocomotionState::Grounded, RotationMode::ViewDirection,
                Stance::Standing, Gait::Running, Gait::Running, OverlayState::Default, RotationMode::ViewDirection,
                Stance::Standing, false, Some(&motion), false, AlsNotifyLocomotionAction::None);
            assert_eq!(snapshot.velocity_direction, Some(if speed < 1e-6 { Vec3::ZERO } else { Vec3::Y }));
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

        let shapes = build_shapes_body(&no_input);
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
                layout_character_block(&flags, &sample_snapshot(), &[]),
                layout_camera_block(&flags, &camera, &[]),
            ];
            assert!(!blocks.is_empty());
            for block in &blocks {
                for column in block {
                    assert!(!column.is_empty());
                }
            }
        }
    }

    #[test]
    fn action_display_is_independent_of_movement_mode() {
        let mut snapshot = sample_snapshot();
        for mode in [
            LocomotionState::None,
            LocomotionState::Custom,
            LocomotionState::Grounded,
        ] {
            snapshot.locomotion_state = mode;
            for (action, label) in [
                (AlsNotifyLocomotionAction::None, ""),
                (AlsNotifyLocomotionAction::Rolling, "Rolling"),
                (AlsNotifyLocomotionAction::Mantling, "Mantling"),
                (AlsNotifyLocomotionAction::Ragdolling, "Ragdolling"),
                (AlsNotifyLocomotionAction::GettingUp, "GettingUp"),
            ] {
                snapshot.locomotion_action = action;
                assert_eq!(locomotion_action_value(&snapshot), label);
            }
        }
    }

    #[test]
    fn source_canvas_coordinates_font_sizes_and_dpi_match_the_draw_contract() {
        use bevy::ecs::system::RunSystemOnce;
        let state = AlsShowDebugHudState { visible: true, curves: true, state: true, shapes: true, traces: true, mantling: true,
            camera_curves: true, camera_shapes: true, camera_traces: true };
        let camera = CameraSnapshot { rig_mode: Some(AlsCameraMode::ThirdPerson), ..default() };
        let mut camera_curves: Vec<_> = als_camera::CAMERA_CURVE_NAMES.iter().map(|name| (name.to_string(), 0.0)).collect();
        camera_curves.sort_by(|a, b| a.0.cmp(&b.0));
        let camera_block = layout_camera_block(&state, &camera, &build_curves_body(&camera_curves, true));
        let character_block = layout_character_block(&state, &sample_snapshot(), &evaluated_character_curve_rows(None));
        assert_eq!(block_height(&camera_block), 187.0);
        assert_eq!(block_height(&character_block), 574.0);
        for (size, dpi, ui_scale) in [(Vec2::new(1280.0, 720.0), 1.0, 1.0), (Vec2::new(960.0, 540.0), 1.0, 1.0),
            (Vec2::new(960.0, 540.0), 2.0, 1.0), (Vec2::new(2560.0, 1440.0), 1.5, 1.25)] {
            let metrics = CanvasMetrics::new(size, dpi, ui_scale);
            let expected_scale = (size.x / 1280.0).min(size.y / 720.0);
            let mut world = World::new();
            let blocks = vec![camera_block.clone(), character_block.clone()];
            world.run_system_once(move |mut commands: Commands| { spawn_panel(&mut commands, &blocks, metrics, "TestActor"); }).unwrap();
            for (name, point, font) in [
                ("DebugHeader:Als.CameraShapes (Shift + 7)", Vec2::new(205.0, 5.0), HEADER_FONT_PX),
                ("Camera Offset X", Vec2::new(150.0, 24.0), BODY_FONT_PX),
                ("DebugHeader:Debug mode is enabled! Press (Shift + 0) to disable.", Vec2::new(5.0, 192.0), HEADER_FONT_PX),
                ("DebugHeader:Als.State (Shift + 2)", Vec2::new(205.0, 219.0), HEADER_FONT_PX),
                ("Allow Transitions", Vec2::new(150.0, 238.0), BODY_FONT_PX),
                ("View Block", Vec2::new(150.0, 742.0), BODY_FONT_PX),
            ] {
                let (_, node, text_font, layout) = world.query::<(&Name, &Node, &TextFont, &TextLayout)>().iter(&world)
                    .find(|(key, _, _, _)| key.as_str() == name).unwrap();
                let (Val::Px(x), Val::Px(y), FontSize::Px(font_px)) = (node.left, node.top, text_font.font_size) else { panic!("Canvas values must use explicit logical pixels") };
                let expected_y = (50.0 * dpi + 29.9 * (size.x / 1920.0).max(dpi) + (point.y - 5.0) * expected_scale) / (dpi * ui_scale);
                assert!(Vec2::new(x, y).abs_diff_eq(Vec2::new(point.x * expected_scale / (dpi * ui_scale), expected_y), 1e-4));
                assert!((font_px * dpi * ui_scale - font * expected_scale).abs() < 1e-4);
                assert_eq!(layout.linebreak, bevy::text::LineBreak::NoWrap);
            }
        }
    }

    #[test]
    fn source_camera_curves_split_columns_and_inactive_blocks_stay_empty() {
        let camera = CameraSnapshot { rig_mode: Some(AlsCameraMode::ThirdPerson), ..default() };
        let mut state = AlsShowDebugHudState { visible: true, camera_curves: true, camera_shapes: true, ..default() };
        let rows = build_curves_body(&[("CameraOffsetX".into(), 1.0)], true);
        let block = layout_camera_block(&state, &camera, &rows);
        assert_eq!(block.len(), 2, "Active camera curves must occupy their own source column");
        assert_eq!(block[0][0].label, DebugCategory::CameraCurves.header());
        assert_eq!(block[1][0].label, DebugCategory::CameraShapes.header());
        state.camera_curves = false;
        state.camera_shapes = false;
        assert!(layout_camera_block(&state, &camera, &[]).is_empty());
        let character = layout_character_block(&state, &sample_snapshot(), &[]);
        assert_eq!(character[0].len(), 1, "Disabled character categories retain only the source master header");
    }

    #[test]
    fn layout_splits_second_column_only_when_first_category_active() {
        let camera = CameraSnapshot { rig_mode: Some(AlsCameraMode::ThirdPerson), ..default() };
        let mut flags = AlsShowDebugHudState {
            visible: true,
            ..Default::default()
        };

        let inactive = layout_character_block(&flags, &sample_snapshot(), &[]);
        assert_eq!(inactive.len(), 1);
        assert_eq!(inactive[0].len(), 1);
        assert_eq!(inactive[0][0].label, HEADER_DEBUG_MODE);
        assert_eq!(inactive[0][0].color, header_active_color());

        flags.curves = true;
        let split = layout_character_block(&flags, &sample_snapshot(), &[]);
        assert_eq!(split.len(), 2);
        assert_eq!(split[1].len(), 4);
        assert_eq!(split[0][1].y, 27.0);
        assert_eq!(split[1][0].y, 27.0);

        flags.curves = false;
        flags.state = true;
        let expanded = layout_character_block(&flags, &sample_snapshot(), &[]);
        assert_eq!(expanded.len(), 1);
        assert_eq!(
            expanded[0].len(),
            6 + build_state_body(&sample_snapshot()).len()
        );
        assert_eq!(expanded[0][2].label, DebugCategory::State.header());
        assert_eq!(expanded[0][2].color, header_active_color());
        assert_eq!(expanded[0][1].color, header_inactive_color());

        let camera_block_off = layout_camera_block(&flags, &camera, &[]);
        assert!(camera_block_off.is_empty());

        flags.camera_curves = true;
        flags.camera_shapes = true;
        let camera_block_on = layout_camera_block(&flags, &camera, &[]);
        assert_eq!(
            camera_block_on[1].len(),
            2 + build_camera_shapes_body(&camera).len()
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
        assert_eq!(rows[1].value, "X: 400.00Y: 600.00Z: 500.00");
        assert_eq!(rows[2].value, "X: 700.00Y: 900.00Z: 800.00");
        assert_eq!(rows[3].value, "102.00");
        assert_eq!(rows[4].value, "true");
    }

    #[test]
    fn native_turn_curves_reach_hud_with_source_heading_sign() {
        let app = crate::standing_source::tests::loaded_montage_app();
        let assets = app.world().resource::<crate::standing_source::SourceMontageAssets>();
        let clips = app.world().resource::<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>();
        let skeletons = app.world().resource::<Assets<bevy_animation_graph::core::skeleton::Skeleton>>();
        for name in crate::standing_source::MONTAGES.keys().filter(|name| name.contains("_Turn_")) {
            let (_, context) = crate::standing_source::sample_source_montage(name, 0.4, assets, clips, skeletons).unwrap();
            let runtime = context.curves["RotationYawSpeed"];
            let rows = evaluated_character_curve_rows(Some(&AlsRootAnimationContext { curves: context.curves.into_iter().collect() }));
            let row = rows.iter().find(|row| row.label == "Rotation Yaw Speed").unwrap();
            assert_eq!(row.value, format!("{:.2}", -runtime));
            assert_eq!(row.color, if name.ends_with("_Left") { gray() } else { white() });
        }
    }

    #[test]
    fn camera_hud_reads_all_current_instance_curves_in_source_units() {
        let (mut app, character, camera, _) = crate::camera_component::tests::loaded_camera_scene();
        let player = app.world().get::<AnimatedSceneInstance>(camera).unwrap().player_entity();
        let projected = app.world().get::<AlsRootAnimationContext>(player).unwrap();
        assert_eq!(projected.f32("FootLeftIk"), Some(0.0));
        assert!(!projected.curves.contains_key("FootLeftIk"), "Default reads must not invent active camera curves");
        app.world_mut().entity_mut(player).insert(AlsRootAnimationContext { curves: [
            ("CameraOffsetZ".into(), -1.0), ("CameraOffsetX".into(), 2.0),
            ("PivotOffsetZ".into(), 0.5), ("LocationLagZ".into(), 0.3), ("CustomCameraCurve".into(), -0.5)].into() });
        app.world_mut().get_mut::<AlsSourceCameraComponent>(camera).unwrap().activate(false);
        app.world_mut().entity_mut(camera).insert(als_camera::AlsCameraDebugInfo { valid: true, curve_values: [999.0; als_camera::CAMERA_CURVE_COUNT], ..default() });
        let view = app.world_mut().spawn((Camera3d::default(), Camera { is_active: false, ..default() },
            als_camera::AlsCameraViewSource(camera), als_camera::AlsCharacterCamera { character })).id();
        app.world_mut().entity_mut(character).insert((AlsCharacter, crate::debug_target::AlsDebugTarget,
            OverlayState::Default, AlsStateRequest::default(), AlsCharacterAssembly { avatar: Entity::PLACEHOLDER, camera: Some(view) }));
        app.insert_resource(AlsShowDebugHudState { visible: true, camera_curves: true, ..default() })
            .init_resource::<canvas::ColumnNavigation>();
        let system = app.world_mut().register_system(hud_sync_system);
        app.world_mut().run_system(system).unwrap();
        let values: std::collections::HashMap<_, _> = app.world_mut().query::<(&Name, &Text)>().iter(app.world())
            .map(|(name, text)| (name.to_string(), text.0.clone())).collect();
        for (name, value) in [("Camera Offset X", "100.00"), ("Camera Offset Y", "200.00"),
            ("Pivot Offset Y", "50.00"), ("Location Lag X", "0.30"), ("Custom Camera Curve", "-0.50")] {
            assert_eq!(values.get(name).map(String::as_str), Some(value), "{name}");
        }
        assert!(!values.contains_key("Camera Offset Z"), "Inactive curves must not be fabricated");
        assert!(app.world_mut().query::<(&Name, &TextColor)>().iter(app.world()).any(|(name, color)|
            name.as_str() == "Custom Camera Curve" && color.0 == linear(0.75, 0.75, 0.75)));
        app.world_mut().get_mut::<AlsSourceCameraComponent>(camera).unwrap().deactivate();
        app.world_mut().run_system(system).unwrap();
        assert!(!app.world_mut().query::<&Name>().iter(app.world()).any(|name| name.as_str() == "Custom Camera Curve"));
    }

    #[test]
    fn angular_curve_labels_and_colors_restore_source_heading_sign() {
        for (runtime, expected, color) in [(-42.0, "42.00", white()), (42.0, "-42.00", gray()), (0.0, "0.00", gray())] {
            let context = AlsRootAnimationContext { curves: [("RotationYawOffset".into(), runtime),
                ("RotationYawSpeed".into(), runtime), ("PoseGait".into(), 2.25)].into() };
            let rows = evaluated_character_curve_rows(Some(&context));
            for label in ["Rotation Yaw Offset", "Rotation Yaw Speed"] {
                let row = rows.iter().find(|row| row.label == label).unwrap();
                assert_eq!(row.value, expected);
                assert_eq!(row.color, color);
            }
            assert_eq!(rows.iter().find(|row| row.label == "Pose Gait").unwrap().value, "2.25");
        }
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
