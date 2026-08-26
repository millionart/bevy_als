use als_camera::AlsCameraDebugInfo;
use als_core::character::AlsCharacter;
use als_core::input::AlsInputCommand;
use als_core::mantle::AlsMantleState;
use als_core::motion::{AlsCharacterDebug, AlsInputOverride};
use avian3d::prelude::{ColliderAabb, LinearVelocity};
use bevy::math::primitives::Cuboid;
use bevy::prelude::*;

const UE_CM_TO_M: f32 = 0.01;

pub const SPHERE_RADIUS_M: f32 = 16.0 * UE_CM_TO_M;
pub const CONE_LENGTH_M: f32 = 100.0 * UE_CM_TO_M;
pub const CONE_HALF_ANGLE_DEG: f32 = 15.0;
pub const CONE_SEGMENTS: u32 = 8;
pub const ARROW_HEAD_M: f32 = 50.0 * UE_CM_TO_M;
pub const INPUT_ARROW_LIFT_M: f32 = 3.0 * UE_CM_TO_M;
pub const SMOOTH_ARROW_LIFT_M: f32 = 6.0 * UE_CM_TO_M;
pub const VELOCITY_MIN_LENGTH_M: f32 = 50.0 * UE_CM_TO_M;
pub const VELOCITY_MAX_LENGTH_M: f32 = 75.0 * UE_CM_TO_M;
pub const BOUNDS_EXPAND: f32 = 1.1;
pub const CIRCLE_SIDES: u32 = 16;
pub const DEFAULT_CAPSULE_HALF_HEIGHT_M: f32 = 90.0 * UE_CM_TO_M;
pub const DEFAULT_CAPSULE_RADIUS_M: f32 = 30.0 * UE_CM_TO_M;
pub const PAWN_VIEW_LIFT_STANDING_M: f32 = 64.0 * UE_CM_TO_M;

fn linear(r: f32, g: f32, b: f32) -> Color {
    Color::linear_rgb(r, g, b)
}

pub fn input_arrow_color(has_input: bool) -> Color {
    if has_input {
        linear(1.0, 0.5, 0.0)
    } else {
        linear(0.5, 0.25, 0.0)
    }
}

pub fn velocity_arrow_color(has_velocity: bool) -> Color {
    if has_velocity {
        linear(0.75, 0.0, 1.0)
    } else {
        linear(0.375, 0.0, 0.5)
    }
}

pub fn smooth_target_yaw_color() -> Color {
    linear(0.0, 0.75, 1.0)
}

pub fn capsule_wire_color() -> Color {
    linear(0.0, 1.0, 0.0)
}

pub fn bounds_box_color() -> Color {
    linear(0.0, 1.0, 0.0)
}

pub fn pivot_target_color() -> Color {
    linear(0.0, 1.0, 0.0)
}

pub fn pivot_lag_color() -> Color {
    linear(1.0, 0.5, 0.0)
}

pub fn pivot_color() -> Color {
    linear(0.0, 0.75, 1.0)
}

pub fn camera_trace_color(blocking_hit: bool) -> Color {
    if blocking_hit {
        linear(1.0, 0.0, 0.0)
    } else {
        linear(0.0, 1.0, 0.0)
    }
}

pub fn mantle_forward_color() -> Color {
    linear(0.0, 0.25, 1.0)
}

pub fn mantle_downward_color() -> Color {
    linear(0.25, 0.0, 1.0)
}

#[derive(Resource, Debug, Default, Clone)]
pub struct AlsDebugDisplayStats {
    pub cumulative_camera_valid: u64,
    pub cumulative_pivot_spheres: u64,
    pub cumulative_trace_lines: u64,
    pub shapes_active: bool,
    pub mantling_active: bool,
    pub camera_shapes_active: bool,
    pub camera_traces_active: bool,
    pub cone_segments_drawn: u32,
    pub arrows_drawn: u32,
    pub capsule_wireframes_drawn: u32,
    pub bounds_boxes_drawn: u32,
    pub camera_pivot_spheres_drawn: u32,
    pub camera_trace_lines_drawn: u32,
    pub mantle_probe_lines_drawn: u32,
    pub camera_debug_valid: u32,
}

impl AlsDebugDisplayStats {
    pub fn total(&self) -> u32 {
        self.cone_segments_drawn
            + self.arrows_drawn
            + self.capsule_wireframes_drawn
            + self.bounds_boxes_drawn
            + self.camera_pivot_spheres_drawn
            + self.camera_trace_lines_drawn
            + self.mantle_probe_lines_drawn
    }
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct AlsDebugDisplayLayer(pub Option<u8>);

pub struct AlsDebugDisplayPlugin;

impl Plugin for AlsDebugDisplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AlsDebugDisplayStats>()
            .init_resource::<AlsDebugDisplayLayer>()
            .add_systems(
                Update,
                (
                    reset_stats_system,
                    (draw_character_shapes_system, draw_bounds_box_system).chain(),
                    draw_mantle_probes_system,
                    (draw_camera_debug_system, draw_camera_trace_system).chain(),
                )
                    .chain(),
            );
    }
}

fn reset_stats_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    show_debug: Option<Res<crate::show_debug_hud::AlsShowDebugHudState>>,
) {
    if let Some(state) = show_debug {
        stats.shapes_active = state.is_active(crate::show_debug_hud::DebugCategory::Shapes);
        stats.mantling_active = state.is_active(crate::show_debug_hud::DebugCategory::Mantling);
        stats.camera_shapes_active =
            state.is_active(crate::show_debug_hud::DebugCategory::CameraShapes);
        stats.camera_traces_active =
            state.is_active(crate::show_debug_hud::DebugCategory::CameraTraces);
    }
    stats.cone_segments_drawn = 0;
    stats.arrows_drawn = 0;
    stats.capsule_wireframes_drawn = 0;
    stats.bounds_boxes_drawn = 0;
    stats.camera_pivot_spheres_drawn = 0;
    stats.camera_trace_lines_drawn = 0;
    stats.mantle_probe_lines_drawn = 0;
    stats.camera_debug_valid = 0;
}

fn yaw_to_dir_xy(yaw_deg: f32) -> Vec3 {
    let radians = yaw_deg.to_radians();
    Vec3::new(radians.cos(), 0.0, -radians.sin())
}

fn circle_xz(gizmos: &mut Gizmos, location: Vec3, radius: f32, color: Color, sides: u32) {
    let mut points = Vec::with_capacity(sides as usize + 1);
    for i in 0..=sides {
        let angle = (i as f32 / sides as f32) * std::f32::consts::TAU;
        points.push(location + Vec3::new(angle.cos(), 0.0, angle.sin()) * radius);
    }
    gizmos.linestrip(points, color);
}

fn draw_view_cone(
    gizmos: &mut Gizmos,
    stats: &mut AlsDebugDisplayStats,
    apex: Vec3,
    direction: Vec3,
    color: Color,
) {
    let half_angle_rad = CONE_HALF_ANGLE_DEG.to_radians();
    let Ok(dir) = Dir3::new(direction.normalize_or_zero()) else {
        return;
    };
    let ring_distance = CONE_LENGTH_M * half_angle_rad.cos();
    let ring_radius = CONE_LENGTH_M * half_angle_rad.sin();
    let center = apex + dir * ring_distance;
    let basis = Quat::from_rotation_arc(Vec3::NEG_Z, *dir);
    let mut points = Vec::with_capacity(CONE_SEGMENTS as usize + 2);
    for i in 0..=CONE_SEGMENTS {
        let angle = (i as f32 / CONE_SEGMENTS as f32) * std::f32::consts::TAU;
        let offset = Vec3::new(angle.cos() * ring_radius, angle.sin() * ring_radius, 0.0);
        points.push(center + basis * offset);
    }
    points.push(apex);
    gizmos.linestrip(points.clone(), color);
    for point in &points[..CONE_SEGMENTS as usize] {
        gizmos.line(apex, *point, color);
    }
    stats.cone_segments_drawn += CONE_SEGMENTS + CONE_SEGMENTS;
}

fn draw_directional_arrow(
    gizmos: &mut Gizmos,
    start: Vec3,
    end: Vec3,
    head_size: f32,
    color: Color,
    stats: &mut AlsDebugDisplayStats,
) {
    gizmos.line(start, end, color);
    let dir = end - start;
    let length = dir.length();
    if length > 1e-5 {
        let forward = dir / length;
        let side = Vec3::Y.cross(forward).normalize_or_zero();
        let back = end - forward * head_size * 0.35;
        let half = head_size * 0.25;
        gizmos.line(end, back + side * half, color);
        gizmos.line(end, back - side * half, color);
        gizmos.line(end, back + Vec3::Y * half, color);
        gizmos.line(end, back - Vec3::Y * half, color);
    }
    stats.arrows_drawn += 1;
}

#[allow(clippy::type_complexity)]
fn draw_character_shapes_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut gizmos: Gizmos,
    q_character: Query<
        (
            &GlobalTransform,
            &LinearVelocity,
            &als_core::motion::AlsViewYaw,
            Option<&als_core::view::AlsViewSmoothed>,
            Option<&AlsCharacterDebug>,
            AnyOf<(&AlsInputCommand, &AlsInputOverride)>,
        ),
        With<AlsCharacter>,
    >,
) {
    if !stats.shapes_active {
        return;
    }
    for (transform, velocity, view_yaw, view_smoothed, debug, inputs) in q_character.iter() {
        let origin = transform.translation();
        let feet_location = origin - Vec3::Y * DEFAULT_CAPSULE_HALF_HEIGHT_M;
        let pitch = view_smoothed.map(|s| s.0).unwrap_or(0.0);
        let view_rotation = Quat::from_euler(EulerRot::YXZ, view_yaw.0, pitch, 0.0);

        draw_view_cone(
            &mut gizmos,
            &mut stats,
            origin + Vec3::Y * PAWN_VIEW_LIFT_STANDING_M,
            view_rotation * Vec3::NEG_Z,
            linear(1.0, 0.0, 0.0),
        );

        let move_dir = match inputs {
            (Some(cmd), _) => cmd.move_dir,
            (_, Some(ovr)) => ovr.0.move_dir,
            _ => Vec2::ZERO,
        };
        let has_input = move_dir.length_squared() > 1e-6;
        let input_yaw_deg = if has_input {
            move_dir.y.atan2(move_dir.x).to_degrees()
        } else {
            0.0
        };
        draw_directional_arrow(
            &mut gizmos,
            feet_location + Vec3::Y * INPUT_ARROW_LIFT_M,
            feet_location
                + Vec3::Y * INPUT_ARROW_LIFT_M
                + yaw_to_dir_xy(input_yaw_deg) * ARROW_HEAD_M,
            ARROW_HEAD_M,
            input_arrow_color(has_input),
            &mut stats,
        );

        let hspeed = Vec2::new(velocity.x, velocity.z).length();
        let has_velocity = hspeed > 1e-3;
        let velocity_yaw_deg = if has_velocity {
            velocity.x.atan2(-velocity.z).to_degrees()
        } else {
            0.0
        };
        let max_reference_speed_mps = 6.5;
        let velocity_arrow_length = VELOCITY_MIN_LENGTH_M
            + (VELOCITY_MAX_LENGTH_M - VELOCITY_MIN_LENGTH_M)
                * (hspeed / max_reference_speed_mps).clamp(0.0, 1.0);
        draw_directional_arrow(
            &mut gizmos,
            feet_location,
            feet_location + yaw_to_dir_xy(velocity_yaw_deg) * velocity_arrow_length,
            ARROW_HEAD_M,
            velocity_arrow_color(has_velocity),
            &mut stats,
        );

        if let Some(target_yaw) = debug.and_then(|d| d.0.frame.smooth_target_yaw_deg) {
            draw_directional_arrow(
                &mut gizmos,
                feet_location + Vec3::Y * SMOOTH_ARROW_LIFT_M,
                feet_location
                    + Vec3::Y * SMOOTH_ARROW_LIFT_M
                    + yaw_to_dir_xy(target_yaw) * ARROW_HEAD_M,
                ARROW_HEAD_M,
                smooth_target_yaw_color(),
                &mut stats,
            );
        }

        circle_xz(
            &mut gizmos,
            origin,
            DEFAULT_CAPSULE_RADIUS_M,
            capsule_wire_color(),
            CIRCLE_SIDES,
        );
        circle_xz(
            &mut gizmos,
            feet_location + Vec3::Y * DEFAULT_CAPSULE_RADIUS_M,
            DEFAULT_CAPSULE_RADIUS_M,
            capsule_wire_color(),
            CIRCLE_SIDES,
        );
        circle_xz(
            &mut gizmos,
            origin + Vec3::Y * (DEFAULT_CAPSULE_HALF_HEIGHT_M - DEFAULT_CAPSULE_RADIUS_M),
            DEFAULT_CAPSULE_RADIUS_M,
            capsule_wire_color(),
            CIRCLE_SIDES,
        );
        stats.capsule_wireframes_drawn += CIRCLE_SIDES * 3;
    }
}

#[allow(clippy::type_complexity)]
fn draw_bounds_box_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut gizmos: Gizmos,
    q_character: Query<(&GlobalTransform, &ColliderAabb), With<AlsCharacter>>,
) {
    if !stats.shapes_active {
        return;
    }
    let Ok((_transform, aabb)) = q_character.single() else {
        return;
    };
    if aabb.min == Vec3::INFINITY || aabb.max == Vec3::NEG_INFINITY {
        return;
    }
    let center = (aabb.min + aabb.max) * 0.5;
    let extent = (aabb.max - aabb.min) * 0.5 * BOUNDS_EXPAND;
    gizmos.primitive_3d(
        &Cuboid::from_size(extent * 2.0),
        Isometry3d::new(center, Quat::IDENTITY),
        bounds_box_color(),
    );
    stats.bounds_boxes_drawn += 1;
}

#[allow(clippy::type_complexity)]
fn draw_mantle_probes_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut gizmos: Gizmos,
    q_character: Query<(Entity, &AlsMantleState, Option<&AlsCharacterDebug>), With<AlsCharacter>>,
) {
    if !stats.mantling_active {
        return;
    }
    for (_entity, _state, debug) in q_character.iter() {
        let Some(trace) = debug.and_then(|d| d.0.mantle_probe) else {
            continue;
        };
        for (start, end) in trace.forward_starts.iter().zip(trace.forward_ends.iter()) {
            let Some(end_point) = end else {
                continue;
            };
            gizmos.line(*start, *end_point, mantle_forward_color());
            stats.mantle_probe_lines_drawn += 1;
        }
        if let (Some(origin), Some(hit)) = (trace.downward_origin, trace.downward_hit) {
            gizmos.line(origin, hit, mantle_downward_color());
            stats.mantle_probe_lines_drawn += 1;
        }
    }
}

fn draw_camera_debug_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut gizmos: Gizmos,
    q_camera: Query<&AlsCameraDebugInfo>,
) {
    for info in q_camera.iter() {
        if !info.valid {
            warn!(
                "[BF9][CAMSYS] camera debug present but invalid so far"
            );
            continue;
        }
        stats.camera_debug_valid += 1;
        stats.cumulative_camera_valid += 1;

        if !stats.camera_shapes_active {
            continue;
        }

        draw_sphere_alternative(
            &mut gizmos,
            info.pivot_target,
            Quat::IDENTITY,
            SPHERE_RADIUS_M,
            pivot_target_color(),
            &mut stats,
        );

        if let Some(pivot) = info.pivot {
            draw_sphere_alternative(
                &mut gizmos,
                pivot,
                Quat::IDENTITY,
                SPHERE_RADIUS_M,
                pivot_color(),
                &mut stats,
            );
        }

        if let Some(lag) = info.pivot_lag {
            draw_sphere_alternative(
                &mut gizmos,
                lag,
                Quat::IDENTITY,
                SPHERE_RADIUS_M,
                pivot_lag_color(),
                &mut stats,
            );
        }
    }
}

fn draw_camera_trace_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut gizmos: Gizmos,
    q_camera: Query<&AlsCameraDebugInfo>,
) {
    for info in q_camera.iter() {
        if !info.valid || !stats.camera_traces_active {
            continue;
        }
        gizmos.line(
            info.trace_start,
            info.trace_end,
            camera_trace_color(info.trace_blocking_hit),
        );
        stats.camera_trace_lines_drawn += 1;
        stats.cumulative_trace_lines += 1;
    }
}

fn draw_sphere_alternative(
    gizmos: &mut Gizmos,
    location: Vec3,
    rotation: Quat,
    radius: f32,
    color: Color,
    stats: &mut AlsDebugDisplayStats,
) {
    let forward = rotation * Vec3::NEG_Z;
    let right = rotation * Vec3::X;
    let up = rotation * Vec3::Y;
    let axes = [(forward, right), (forward, up), (right, up)];
    for (a, b) in axes {
        let mut points = Vec::with_capacity(CIRCLE_SIDES as usize + 1);
        for i in 0..=CIRCLE_SIDES {
            let angle = (i as f32 / CIRCLE_SIDES as f32) * std::f32::consts::TAU;
            let dir = a * angle.cos() + b * angle.sin();
            points.push(location + dir * radius);
        }
        gizmos.linestrip(points, color);
    }
    stats.camera_pivot_spheres_drawn += 3;
    stats.cumulative_pivot_spheres += 3;
}
