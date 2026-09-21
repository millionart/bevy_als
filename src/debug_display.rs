use als_camera::AlsCameraDebugInfo;
use als_core::air::AlsAirState;
use als_core::character::AlsCharacter;
use als_core::input::AlsInputCommand;
use als_core::mantle::AlsMantleState;
use als_core::motion::{AlsCharacterDebug, AlsInputOverride};
use avian3d::prelude::{ColliderAabb, LinearVelocity};
use bevy::math::primitives::Cuboid;
use bevy::prelude::*;
use bevy::gizmos::{AppGizmoBuilder, config::{GizmoConfig, GizmoConfigGroup, GizmoConfigStore, GizmoLineConfig}};

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
    pub cumulative_ground_prediction_sweeps: u64,
    pub cumulative_pivot_spheres: u64,
    pub cumulative_trace_lines: u64,
    pub shapes_active: bool,
    pub mantling_active: bool,
    pub camera_shapes_active: bool,
    pub camera_traces_active: bool,
    pub traces_active: bool,
    pub cone_segments_drawn: u32,
    pub arrows_drawn: u32,
    pub capsule_wireframes_drawn: u32,
    pub bounds_boxes_drawn: u32,
    pub camera_pivot_spheres_drawn: u32,
    pub camera_trace_lines_drawn: u32,
    pub mantle_probe_lines_drawn: u32,
    pub camera_debug_valid: u32,
    pub footstep_trace_lines_drawn: u32,
    pub footstep_axis_lines_drawn: u32,
    pub footstep_hit_points_drawn: u32,
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
            + self.footstep_trace_lines_drawn
            + self.footstep_axis_lines_drawn
            + self.footstep_hit_points_drawn
    }
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct AlsDebugDisplayLayer(pub Option<u8>);

pub struct AlsDebugDisplayPlugin;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AlsDebugDisplaySet;

#[derive(Default, Reflect, GizmoConfigGroup)]
struct FootstepLines;

#[derive(Default, Reflect, GizmoConfigGroup)]
struct FootstepPoints;

impl Plugin for AlsDebugDisplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AlsDebugDisplayStats>()
            .init_resource::<AlsDebugDisplayLayer>()
            .init_resource::<crate::footprint::FootstepDebugHistory>()
            .insert_gizmo_config(FootstepLines, GizmoConfig { line: GizmoLineConfig { width: 1.0, ..default() }, ..default() })
            .init_gizmo_group::<FootstepPoints>()
            .configure_sets(PostUpdate, AlsDebugDisplaySet.after(crate::footprint::FootprintSet)
                .after(bevy::camera::visibility::VisibilitySystems::CheckVisibility))
            .add_systems(
                PostUpdate,
                (
                    reset_stats_system,
                    (draw_character_shapes_system, draw_bounds_box_system).chain(),
                    draw_mantle_probes_system,
                    draw_ground_prediction_system,
                    draw_camera_debug_system,
                    draw_footstep_traces,
                )
                    .chain().in_set(AlsDebugDisplaySet),
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
        stats.traces_active = state.is_active(crate::show_debug_hud::DebugCategory::Traces);
    }
    stats.cone_segments_drawn = 0;
    stats.arrows_drawn = 0;
    stats.capsule_wireframes_drawn = 0;
    stats.bounds_boxes_drawn = 0;
    stats.camera_pivot_spheres_drawn = 0;
    stats.camera_trace_lines_drawn = 0;
    stats.mantle_probe_lines_drawn = 0;
    stats.camera_debug_valid = 0;
    stats.footstep_trace_lines_drawn = 0;
    stats.footstep_axis_lines_drawn = 0;
    stats.footstep_hit_points_drawn = 0;
}

fn footstep_point_segment(point: Vec3, camera: &Camera, view: &GlobalTransform) -> Option<(Vec3, Vec3, f32)> {
    let viewport = camera.physical_viewport_size()?;
    if viewport.x == 0 || viewport.y == 0 { return None; }
    let projection = camera.clip_from_view();
    let clip = projection * view.affine().inverse().transform_point3(point).extend(1.0);
    if clip.w <= 0.0 { return None; }
    let half_width = 32.0 * if projection.w_axis.w == 1.0 { UE_CM_TO_M } else { clip.w } / viewport.x as f32;
    let side = view.right().as_vec3() * half_width;
    Some((point - side, point + side, (half_width * projection.y_axis.y * viewport.y as f32 / clip.w).abs()))
}

#[cfg(test)]
mod footstep_tests {
    use super::*;
    use crate::{character_assembly::AlsPlayerCharacter, footprint::{FootstepDebugHistory, FootstepDebugTrace}};

    #[test]
    fn persistent_footstep_gizmos_preserve_source_vertices_colors_and_point_size() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .add_plugins(bevy::mesh::MeshPlugin)
            .add_plugins((bevy::gizmos::GizmoPlugin, AlsDebugDisplayPlugin));
        let player = app.world_mut().spawn(AlsPlayerCharacter).id();
        let mut camera = Camera { viewport: Some(bevy::camera::Viewport { physical_size: UVec2::new(800, 600), ..default() }), ..default() };
        camera.computed.clip_from_view = Mat4::perspective_infinite_reverse_rh(std::f32::consts::FRAC_PI_2, 4.0 / 3.0, 0.1);
        for depth in [3.0, 30.0] {
            let (a, b, width) = footstep_point_segment(Vec3::NEG_Z * depth, &camera, &GlobalTransform::IDENTITY).unwrap();
            let a = camera.clip_from_view().project_point3(a);
            let b = camera.clip_from_view().project_point3(b);
            assert!(((b.x - a.x) * 400.0 - width).abs() < 1e-4);
            assert!((width - 24.0).abs() < 1e-4);
        }
        let other = app.world_mut().spawn_empty().id();
        let mut decoy = camera.clone();
        decoy.computed.clip_from_view = Mat4::perspective_infinite_reverse_rh(std::f32::consts::FRAC_PI_3, 4.0 / 3.0, 0.1);
        app.world_mut().spawn((Camera3d::default(), decoy, GlobalTransform::IDENTITY, als_camera::AlsCharacterCamera { character: other }));
        app.world_mut().spawn((Camera3d::default(), camera, GlobalTransform::IDENTITY, als_camera::AlsCharacterCamera { character: player }));
        let point = Vec3::NEG_Z * 3.0;
        app.world_mut().resource_mut::<FootstepDebugHistory>().0 = vec![
            FootstepDebugTrace { created_at: 0.0, start: point + Vec3::Y, end: point - Vec3::Y, hit: Some(point), rotation: Some(Quat::IDENTITY) },
            FootstepDebugTrace { created_at: 0.0, start: Vec3::ZERO, end: Vec3::NEG_Y, hit: None, rotation: None },
            FootstepDebugTrace { created_at: 0.0, start: point, end: point - Vec3::Y, hit: Some(point), rotation: None },
        ];
        app.update();
        let stats = app.world().resource::<AlsDebugDisplayStats>();
        assert_eq!((stats.footstep_trace_lines_drawn, stats.footstep_axis_lines_drawn, stats.footstep_hit_points_drawn), (3, 3, 2));
        let assets = app.world().resource::<Assets<bevy::gizmos::GizmoAsset>>();
        let lines = assets.iter().find(|(_, asset)| asset.config_typeid() == std::any::TypeId::of::<FootstepLines>()).unwrap().1.buffer();
        assert_eq!(lines.list_positions.len(), 12);
        assert_eq!(lines.list_positions[0], point + Vec3::Y);
        assert_eq!(lines.list_positions[1], point - Vec3::Y);
        for (index, axis, color) in [(3, Vec3::X, LinearRgba::RED), (5, Vec3::Z, LinearRgba::GREEN), (7, Vec3::Y, LinearRgba::BLUE)] {
            assert_eq!(lines.list_positions[index], point + axis * 0.25);
            assert_eq!(lines.list_colors[index], color);
        }
        let points = assets.iter().find(|(_, asset)| asset.config_typeid() == std::any::TypeId::of::<FootstepPoints>()).unwrap().1.buffer();
        assert_eq!(points.list_positions.len(), 4);
        assert!(points.list_positions[0].abs_diff_eq(point - Vec3::X * 0.12, 1e-6));
        assert!(points.list_positions[1].abs_diff_eq(point + Vec3::X * 0.12, 1e-6));
        let configs = app.world().resource::<GizmoConfigStore>();
        assert_eq!(configs.config::<FootstepLines>().0.line.width, 1.0);
        assert!((configs.config::<FootstepPoints>().0.line.width - 24.0).abs() < 1e-4);
        app.world_mut().entity_mut(player).insert((AlsCharacter, Transform::default(), GlobalTransform::default(),
            LinearVelocity::ZERO, als_core::motion::AlsViewYaw(0.0), als_core::view::AlsViewPitch(0.0), AlsInputCommand::default(),
            als_core::motion::AlsRotationResponseState { smooth_target_yaw: std::f32::consts::FRAC_PI_2, ..default() }));
        let mut selected = None;
        for character in [other, player] {
            let mut component = crate::camera_component::AlsSourceCameraComponent::default();
            component.activate(false);
            let entity = app.world_mut().spawn((component, als_camera::AlsCharacterCamera { character },
                AlsCameraDebugInfo { valid: true, pivot_target: Vec3::ONE, pivot: Some(Vec3::Y), pivot_lag: Some(Vec3::X),
                    trace_start: Vec3::ZERO, trace_end: Vec3::NEG_Z, ..default() })).id();
            if character == player { selected = Some(entity); }
        }
        app.world_mut().entity_mut(player).insert(crate::character_assembly::AlsCharacterAssembly { avatar: Entity::PLACEHOLDER, camera: selected });
        app.insert_resource(crate::show_debug_hud::AlsShowDebugHudState { visible: true, shapes: true, camera_shapes: true, camera_traces: true, ..default() });
        app.update();
        let stats = app.world().resource::<AlsDebugDisplayStats>();
        assert_eq!(stats.arrows_drawn, 3, "Smooth target yaw must draw from the current rotation response");
        assert_eq!((stats.camera_pivot_spheres_drawn, stats.camera_trace_lines_drawn), (9, 1));
        let assets = app.world().resource::<Assets<bevy::gizmos::GizmoAsset>>();
        let standard = assets.iter().find(|(_, asset)| asset.config_typeid() == std::any::TypeId::of::<bevy::gizmos::config::DefaultGizmoConfigGroup>()).unwrap().1.buffer();
        assert!(standard.list_positions.iter().any(|point| point.abs_diff_eq(Vec3::new(-0.5, -0.84, 0.0), 1e-6)));
    }
}

fn draw_footstep_traces(
    history: Res<crate::footprint::FootstepDebugHistory>,
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut drawing: ParamSet<(ResMut<GizmoConfigStore>, Gizmos<FootstepLines>, Gizmos<FootstepPoints>)>,
    players: Query<Entity, With<crate::character_assembly::AlsPlayerCharacter>>,
    views: Query<(&Camera, &GlobalTransform, &als_camera::AlsCharacterCamera), With<Camera3d>>,
) {
    let view = players.single().ok().and_then(|player| views.iter().find(|(camera, _, owner)| camera.is_active && owner.character == player));
    for trace in &history.0 {
        drawing.p1().line(trace.start, trace.end, linear(0.333333, 0.0, 0.0));
        stats.footstep_trace_lines_drawn += 1;
        if let Some(point) = trace.hit {
            if let Some((camera, view, _)) = view && let Some((start, end, width)) = footstep_point_segment(point, camera, view) {
                drawing.p0().config_mut::<FootstepPoints>().0.line.width = width;
                drawing.p2().line(start, end, linear(1.0, 0.0, 0.0));
                stats.footstep_hit_points_drawn += 1;
            }
            if let Some(rotation) = trace.rotation {
                for (axis, color) in [(Vec3::X, linear(1.0, 0.0, 0.0)), (Vec3::Z, linear(0.0, 1.0, 0.0)), (Vec3::Y, linear(0.0, 0.0, 1.0))] {
                    drawing.p1().line(point, point + rotation * axis * 0.25, color);
                    stats.footstep_axis_lines_drawn += 1;
                }
            }
        }
    }
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
            Option<&als_core::view::AlsViewPitch>,
            Option<&als_core::motion::AlsRotationResponseState>,
            AnyOf<(&AlsInputCommand, &AlsInputOverride)>,
        ),
        (With<AlsCharacter>, With<crate::character_assembly::AlsPlayerCharacter>),
    >,
) {
    if !stats.shapes_active {
        return;
    }
    for (transform, velocity, view_yaw, view_pitch, response, inputs) in q_character.iter() {
        let origin = transform.translation();
        let feet_location = origin - Vec3::Y * DEFAULT_CAPSULE_HALF_HEIGHT_M;
        let pitch = view_pitch.map(|s| s.0).unwrap_or(0.0);
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

        if let Some(target_yaw) = response.map(|response| response.smooth_target_yaw) {
            draw_directional_arrow(
                &mut gizmos,
                feet_location + Vec3::Y * SMOOTH_ARROW_LIFT_M,
                feet_location
                    + Vec3::Y * SMOOTH_ARROW_LIFT_M
                    + als_core::orientation::yaw_to_world_forward(target_yaw) * ARROW_HEAD_M,
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
    q_character: Query<(&GlobalTransform, &ColliderAabb), (With<AlsCharacter>, With<crate::character_assembly::AlsPlayerCharacter>)>,
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
    q_character: Query<(Entity, &AlsMantleState, Option<&AlsCharacterDebug>), (With<AlsCharacter>, With<crate::character_assembly::AlsPlayerCharacter>)>,
) {
    if !stats.mantling_active {
        return;
    }
    for (_entity, _state, debug) in q_character.iter() {
        let Some(trace) = debug.and_then(|d| d.0.mantle_probe) else {
            continue;
        };
        if let (Some(start), Some(end)) = (trace.forward_start, trace.forward_end) {
            gizmos.line(start, end, mantle_forward_color());
            stats.mantle_probe_lines_drawn += 1;
        }
        if let (Some(start), Some(end)) = (trace.downward_start, trace.downward_end) {
            gizmos.line(start, end, mantle_downward_color());
            stats.mantle_probe_lines_drawn += 1;
        }
    }
}

fn draw_ground_prediction_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut gizmos: Gizmos,
    q_character: Query<(&GlobalTransform, &AlsAirState), (With<AlsCharacter>, With<crate::character_assembly::AlsPlayerCharacter>)>,
) {
    if !stats.traces_active {
        return;
    }
    for (_transform, air) in q_character.iter() {
        let Some(prediction) = air.ground_prediction else {
            continue;
        };
        let (Some(start), Some(end)) = (prediction.sweep_start, prediction.sweep_end) else {
            continue;
        };
        let sweep_color = if prediction.ground_valid {
            linear(0.75, 0.0, 1.0)
        } else {
            linear(0.25, 0.0, 1.0)
        };
        circle_xz(
            &mut gizmos,
            start,
            prediction.capsule_radius,
            sweep_color,
            CIRCLE_SIDES,
        );
        circle_xz(
            &mut gizmos,
            end,
            prediction.capsule_radius,
            sweep_color,
            CIRCLE_SIDES,
        );
        gizmos.line(start, end, sweep_color);
        stats.cumulative_ground_prediction_sweeps += 1;
    }
}

fn draw_camera_debug_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut gizmos: Gizmos,
    characters: Query<(Entity, &crate::character_assembly::AlsCharacterAssembly), With<crate::character_assembly::AlsPlayerCharacter>>,
    cameras: Query<(&crate::camera_component::AlsSourceCameraComponent, &als_camera::AlsCharacterCamera, &AlsCameraDebugInfo)>,
) {
    let Ok((character, assembly)) = characters.single() else { return; };
    let Some(camera) = assembly.camera else { return; };
    let Ok((component, owner, info)) = cameras.get(camera) else { return; };
    if !component.is_active() || owner.character != character || !info.valid { return; }
        stats.camera_debug_valid += 1;
        stats.cumulative_camera_valid += 1;

        if stats.camera_traces_active {
            gizmos.line(info.trace_start, info.trace_end, camera_trace_color(info.trace_blocking_hit));
            stats.camera_trace_lines_drawn += 1;
            stats.cumulative_trace_lines += 1;
        }

        if !stats.camera_shapes_active {
            return;
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
