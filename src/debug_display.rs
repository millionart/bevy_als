use als_camera::AlsCameraDebugInfo;
#[cfg(test)]
use als_core::air::AlsAirState;
use als_core::character::AlsCharacter;
use als_core::mantle::{MantleProbeResult, MantleProbeTrace};
use als_core::motion::AlsCharacterDebug;
use avian3d::prelude::{Collider, ColliderAabb, PhysicsGizmos, PhysicsGizmoExt, SimpleCollider};
use als_core::{control_scheme::{AlsControlScheme, AlsControlSchemeConfig}, state::Stance};
use bevy_tnua::TnuaConfig;
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
    pub mantle_overlap_capsules_drawn: u32,
    pub mantle_hit_points_drawn: u32,
    pub ground_prediction_hit_points_drawn: u32,
    pub cumulative_mantle_probes: u64,
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
            + self.mantle_overlap_capsules_drawn
            + self.mantle_hit_points_drawn
            + self.ground_prediction_hit_points_drawn
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
struct DebugLines;

#[derive(Default, Reflect, GizmoConfigGroup)]
struct DebugPoints;

#[derive(Default, Reflect, GizmoConfigGroup)]
struct ShapeArrows;

#[derive(Default, Reflect, GizmoConfigGroup)]
struct TargetBounds;

#[derive(Resource, Default)]
struct MantleDebugHistory(Vec<(f64, MantleProbeTrace)>);

#[derive(Resource, Default)]
struct CameraAdjustmentHistory(Vec<(f64, Vec3, Vec3)>);

enum TraceMarkerKind { Footstep, Mantle, GroundPrediction }

fn mantle_debug_lifetime(trace: &MantleProbeTrace) -> f64 {
    if !trace.draw_failed_traces && trace.result != MantleProbeResult::Success { return 0.0; }
    match trace.result {
        MantleProbeResult::ForwardRejected => 5.0,
        MantleProbeResult::DownwardRejected | MantleProbeResult::Success => 7.5,
        MantleProbeResult::TargetBlocked | MantleProbeResult::StartBlocked => 10.0,
    }
}

fn mantle_part_visible(trace: &MantleProbeTrace, age: f64, lifetime: f64) -> bool {
    mantle_debug_lifetime(trace) == 0.0 || age < lifetime
}

fn capture_mantle_probes(
    time: Res<Time>,
    mut history: ResMut<MantleDebugHistory>,
    mut stats: ResMut<AlsDebugDisplayStats>,
    actors: Query<&AlsCharacterDebug, (With<AlsCharacter>, With<crate::debug_target::AlsDebugTarget>, Changed<AlsCharacterDebug>)>,
) {
    let now = time.elapsed_secs_f64();
    history.0.retain(|(created, trace)| now - created < mantle_debug_lifetime(trace));
    if !stats.mantling_active { return; }
    for debug in &actors {
        if let Some(trace) = debug.0.mantle_probe.filter(|trace| trace.forward_start.is_some()) {
            history.0.push((now, trace));
            stats.cumulative_mantle_probes += 1;
        }
    }
}

impl Plugin for AlsDebugDisplayPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<crate::debug_target::AlsDebugTargetPlugin>() { app.add_plugins(crate::debug_target::AlsDebugTargetPlugin); }
        app.init_resource::<AlsDebugDisplayStats>()
            .init_resource::<AlsDebugDisplayLayer>()
            .init_resource::<crate::footprint::FootstepDebugHistory>()
            .init_resource::<MantleDebugHistory>()
            .init_resource::<CameraAdjustmentHistory>()
            .insert_gizmo_config(DebugLines, GizmoConfig { line: GizmoLineConfig { width: 1.0, ..default() }, ..default() })
            .init_gizmo_group::<DebugPoints>()
            .insert_gizmo_config(ShapeArrows, GizmoConfig { line: GizmoLineConfig { width: 3.0, ..default() }, ..default() })
            .insert_gizmo_config(TargetBounds, GizmoConfig { line: GizmoLineConfig { width: 2.0, ..default() }, ..default() })
            .insert_gizmo_config(PhysicsGizmos::default(), GizmoConfig { line: GizmoLineConfig { width: 1.0, ..default() }, ..default() })
            .configure_sets(PostUpdate, AlsDebugDisplaySet.after(crate::footprint::FootprintSet)
                .after(crate::actual_motion::AlsActualMotionSet)
                .after(crate::locomotion_layer::AlsLocomotionLayerSet)
                .after(bevy::camera::visibility::VisibilitySystems::CheckVisibility))
            .add_systems(
                PostUpdate,
                (
                    reset_stats_system,
                    capture_mantle_probes,
                    (draw_character_shapes_system, draw_bounds_box_system).chain(),
                    draw_mantle_probes_system,
                    draw_ground_prediction_system,
                    draw_camera_debug_system,
                    draw_trace_markers,
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
    stats.mantle_overlap_capsules_drawn = 0;
    stats.mantle_hit_points_drawn = 0;
    stats.ground_prediction_hit_points_drawn = 0;
    stats.camera_debug_valid = 0;
    stats.footstep_trace_lines_drawn = 0;
    stats.footstep_axis_lines_drawn = 0;
    stats.footstep_hit_points_drawn = 0;
}

fn debug_point_segment(point: Vec3, camera: &Camera, view: &GlobalTransform) -> Option<(Vec3, Vec3, f32)> {
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
    use als_core::mantle::AlsMantleState;
    use als_core::{input::AlsInputCommand, motion::AlsInputOverride};
    use avian3d::prelude::LinearVelocity;
    use crate::{character_assembly::AlsPlayerCharacter, footprint::{FootstepDebugHistory, FootstepDebugTrace}};

    fn trace_points(app: &App, color: Color) -> Vec<Vec3> {
        let assets = app.world().resource::<Assets<bevy::gizmos::GizmoAsset>>();
        app.world().resource::<bevy::gizmos::GizmoHandles>().handles().values().flatten().filter_map(|handle| assets.get(handle)).flat_map(|asset| {
            let buffer = asset.buffer();
            buffer.list_positions.iter().zip(&buffer.list_colors).chain(buffer.strip_positions.iter().zip(&buffer.strip_colors))
        }).filter(|(point, value)| point.is_finite() && **value == color.to_linear()).map(|(point, _)| *point).collect()
    }

    #[test]
    fn ground_and_camera_debug_use_actual_volumes_current_owner_and_source_visibility() {
        use crate::{character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar}, locomotion_layer::AlsGroundPredictionTrace, show_debug_hud::AlsShowDebugHudState};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, bevy::gizmos::GizmoPlugin, AlsDebugDisplayPlugin))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.25)))
            .insert_resource(AlsShowDebugHudState { visible: true, traces: true, ..default() });
        let actor = app.world_mut().spawn((AlsCharacter, AlsPlayerCharacter, GlobalTransform::IDENTITY, AlsAirState::default())).id();
        let old = app.world_mut().spawn((AlsCharacterAvatar { character: actor }, AlsGroundPredictionTrace {
            start: Vec3::new(100.0, 3.0, -2.0), end: Vec3::new(100.0, -2.0, -2.0),
            shape: als_core::character::als_character_capsule(0.9, 0.3), hit: None,
        })).id();
        let current = app.world_mut().spawn((AlsCharacterAvatar { character: actor }, AlsGroundPredictionTrace {
            start: Vec3::new(0.0, 3.0, -2.0), end: Vec3::new(0.0, -2.0, -2.0),
            shape: als_core::character::als_character_capsule(0.9, 0.3), hit: Some((Vec3::new(0.0, 0.9, -2.0), Vec3::new(0.0, 0.0, -2.0))),
        })).id();
        let mut component = crate::camera_component::AlsSourceCameraComponent::default();
        component.activate(false);
        let camera = app.world_mut().spawn((component, als_camera::AlsCharacterCamera { character: actor }, AlsCameraDebugInfo {
            valid: true, camera_yaw: 0.4, pivot_target: Vec3::new(0.0, 2.0, -1.0), pivot_lag: Some(Vec3::new(0.3, 2.0, -1.0)),
            pivot: Some(Vec3::new(0.3, 2.4, -1.0)), trace_start: Vec3::Y, trace_end: Vec3::new(0.0, 1.0, -3.0),
            trace_radius: 0.3, trace_blocking_hit: true, ..default()
        })).id();
        let mut view = Camera { viewport: Some(bevy::camera::Viewport { physical_size: UVec2::new(800, 600), ..default() }), ..default() };
        view.computed.clip_from_view = Mat4::perspective_infinite_reverse_rh(std::f32::consts::FRAC_PI_2, 4.0 / 3.0, 0.1);
        let view = app.world_mut().spawn((Camera3d::default(), view, GlobalTransform::from_translation(Vec3::Z * 3.0),
            als_camera::AlsCameraViewSource(camera), als_camera::AlsCharacterCamera { character: actor })).id();
        app.world_mut().entity_mut(actor).insert(AlsCharacterAssembly { avatar: current, camera: Some(view) });
        app.update();
        let assets = app.world().resource::<Assets<bevy::gizmos::GizmoAsset>>();
        let geometry = assets.iter().find(|(_, asset)| asset.config_typeid() == std::any::TypeId::of::<PhysicsGizmos>()).unwrap().1.buffer();
        assert_eq!(geometry.list_positions.len() / 2 + geometry.strip_positions.windows(2)
            .filter(|pair| pair.iter().all(|point| point.is_finite())).count(), 3 * 68,
            "Prediction start, end and hit capsules must use the source outline");
        let points = trace_points(&app, mantle_downward_color());
        assert!(points.iter().any(|point| point.abs_diff_eq(Vec3::new(0.0, 3.9, -2.0), 1e-5)), "Current prediction start capsule is missing");
        assert!(points.iter().any(|point| point.abs_diff_eq(Vec3::new(0.0, -2.9, -2.0), 1e-5)), "Full prediction end capsule was clipped to the hit");
        assert!(points.iter().all(|point| point.x.abs() < 1.0));
        assert!(!trace_points(&app, linear(0.75, 0.0, 1.0)).is_empty());
        app.world_mut().get_mut::<AlsCharacterAssembly>(actor).unwrap().avatar = old;
        app.update();
        assert!(trace_points(&app, mantle_downward_color()).iter().all(|point| point.x > 99.0));
        app.world_mut().entity_mut(old).remove::<AlsGroundPredictionTrace>();
        app.update();
        assert!(trace_points(&app, mantle_downward_color()).is_empty());
        *app.world_mut().resource_mut::<AlsShowDebugHudState>() = AlsShowDebugHudState { visible: true, camera_traces: true, ..default() };
        app.update();
        let points = trace_points(&app, linear(1.0, 0.0, 0.0));
        assert!(points.iter().any(|point| point.abs_diff_eq(Vec3::new(0.0, 1.0, -3.3), 1e-5)), "Camera sweep radius is missing");
        assert!(points.iter().any(|point| point.x.abs() > 0.29));
        app.world_mut().resource_mut::<AlsShowDebugHudState>().camera_shapes = true;
        app.update();
        let cyan = trace_points(&app, pivot_color());
        assert!(cyan.contains(&Vec3::new(0.3, 2.4, -1.0)) && cyan.contains(&Vec3::new(0.3, 2.0, -1.0)), "Pivot connection line is missing");
        let rotated = Vec3::new(0.3, 2.4, -1.0) + Quat::from_rotation_y(0.4) * Vec3::NEG_Z * SPHERE_RADIUS_M;
        assert!(cyan.iter().any(|point| point.abs_diff_eq(rotated, 1e-5)));
        app.world_mut().get_mut::<AlsCameraDebugInfo>(camera).unwrap().first_person_override = 1.0;
        app.update();
        assert!(trace_points(&app, pivot_color()).is_empty() && trace_points(&app, linear(1.0, 0.0, 0.0)).is_empty());
        app.world_mut().resource_mut::<AlsShowDebugHudState>().camera_shapes = false;
        let before = app.world().resource::<Time>().elapsed_secs_f64();
        {
            let mut info = app.world_mut().get_mut::<AlsCameraDebugInfo>(camera).unwrap();
            info.first_person_override = 0.0;
            info.trace_adjusted_start = true;
            info.trace_adjustment = Vec3::X * 0.2;
        }
        app.update();
        assert_eq!(trace_points(&app, pivot_color()), vec![Vec3::new(-0.2, 1.0, 0.0), Vec3::Y]);
        app.world_mut().resource_mut::<AlsShowDebugHudState>().visible = false;
        for _ in 0..19 { app.update(); }
        assert_eq!(trace_points(&app, pivot_color()).len(), 2);
        for _ in 0..3 { app.update(); }
        assert!(app.world().resource::<Time>().elapsed_secs_f64() - before > 5.0);
        assert!(trace_points(&app, pivot_color()).is_empty());
        app.world_mut().resource_mut::<AlsShowDebugHudState>().visible = true;
        app.update();
        assert!(trace_points(&app, pivot_color()).is_empty(), "Old camera adjustment must not replay after enabling debug");
    }

    #[test]
    fn mantle_gizmos_preserve_source_volume_outcomes_and_independent_lifetimes() {
        use als_core::mantle::{MantleProbeResult as Outcome, MantleProbeTrace};
        use crate::show_debug_hud::AlsShowDebugHudState;
        for (delta, right, up, forward) in [(Vec3::X, Vec3::Z, Vec3::Y, Vec3::X),
            (Vec3::NEG_Z, Vec3::X, Vec3::Y, Vec3::NEG_Z), (Vec3::NEG_Y, Vec3::Z, Vec3::X, Vec3::NEG_Y),
            (Vec3::ZERO, Vec3::Z, Vec3::Y, Vec3::X)] {
            let rotation = sweep_rotation(delta);
            for (local, world) in [(Vec3::X, right), (Vec3::Y, up), (Vec3::NEG_Z, forward)] {
                assert!((rotation * local).abs_diff_eq(world, 1e-5), "Sweep rotation differs from the source world basis");
            }
        }
        for (outcome, persistent) in [(Outcome::ForwardRejected, true), (Outcome::DownwardRejected, true),
            (Outcome::TargetBlocked, true), (Outcome::StartBlocked, true), (Outcome::Success, false), (Outcome::TargetBlocked, false)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, bevy::gizmos::GizmoPlugin, AlsDebugDisplayPlugin))
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(0.25)))
                .insert_resource(AlsShowDebugHudState { visible: true, mantling: true, ..default() });
            let actor = app.world_mut().spawn((AlsCharacter, AlsPlayerCharacter, AlsMantleState::default(),
                AlsCharacterDebug(als_core::motion::CharacterDebugFrameInner { mantle_probe: Some(MantleProbeTrace {
                    forward_start: Some(Vec3::Y), forward_end: Some(Vec3::new(0.0, 1.0, -1.0)),
                    forward_hit: Some(Vec3::new(0.0, 1.0, -0.99)), forward_hit_center: Some(Vec3::new(0.0, 1.0, -0.7)),
                    downward_start: (outcome != Outcome::ForwardRejected).then_some(Vec3::new(0.0, 2.0, -0.9)),
                    downward_end: Some(Vec3::new(0.0, 0.4, -0.9)), downward_hit: Some(Vec3::new(0.0, 0.71, -0.9)),
                    downward_hit_center: Some(Vec3::new(0.0, 1.0, -0.9)), target_capsule_center: Some(Vec3::new(0.0, 1.61, -0.9)),
                    start_capsule_center: Some(Vec3::new(0.0, 0.75, -0.3)), trace_radius: 0.29, forward_half_height: 0.8,
                    target_half_height: 0.9, target_radius: 0.3, start_half_height: 0.45, result: outcome, draw_failed_traces: persistent,
                }), ..default() }))).id();
            let mut camera = Camera { viewport: Some(bevy::camera::Viewport { physical_size: UVec2::new(800, 600), ..default() }), ..default() };
            camera.computed.clip_from_view = Mat4::perspective_infinite_reverse_rh(std::f32::consts::FRAC_PI_2, 4.0 / 3.0, 0.1);
            app.world_mut().spawn((Camera3d::default(), camera, GlobalTransform::from_translation(Vec3::Z * 3.0), als_camera::AlsCharacterCamera { character: actor }));
            app.update();
            let forward = trace_points(&app, mantle_forward_color());
            assert_eq!(forward.len(), 122, "Capsule sweep must contain the source arcs, twelve connecting lines, and three arrow lines");
            assert!(forward.iter().any(|point| point.abs_diff_eq(Vec3::new(0.29, 1.51, 0.0), 1e-5)), "Forward capsule sweep volume is missing for {outcome:?}");
            assert_eq!(!trace_points(&app, linear(0.0, 0.75, 1.0)).is_empty(), outcome != Outcome::ForwardRejected);
            assert_eq!(!trace_points(&app, linear(0.75, 0.0, 1.0)).is_empty(), outcome == Outcome::Success);
            assert_eq!(!trace_points(&app, linear(1.0, 0.0, 0.0)).is_empty(), outcome == Outcome::TargetBlocked);
            assert_eq!(!trace_points(&app, linear(1.0, 0.5, 0.0)).is_empty(), outcome == Outcome::StartBlocked);
            app.world_mut().resource_mut::<AlsShowDebugHudState>().visible = false;
            app.update();
            let lasts = persistent || outcome == Outcome::Success;
            assert_eq!(!trace_points(&app, mantle_forward_color()).is_empty(), lasts);
            for (deadline, duration) in [(5.25, 5.0), (7.75, 7.5), (10.25, 10.0)] {
                for _ in 0..42 {
                    if app.world().resource::<Time>().elapsed_secs_f64() >= deadline { break; }
                    app.update();
                }
                assert!(app.world().resource::<Time>().elapsed_secs_f64() >= deadline, "Debug lifetime fixture made no time progress");
                assert!(trace_points(&app, mantle_forward_color()).is_empty());
                if duration >= 7.5 { assert!(trace_points(&app, mantle_downward_color()).is_empty()); }
                for (color, shown) in [(linear(1.0, 0.0, 0.0), outcome == Outcome::TargetBlocked), (linear(1.0, 0.5, 0.0), outcome == Outcome::StartBlocked)] {
                    assert_eq!(!trace_points(&app, color).is_empty(), lasts && shown && duration < 10.0);
                }
            }
            app.world_mut().get_mut::<AlsCharacterDebug>(actor).unwrap().0.mantle_probe.as_mut().unwrap().result = Outcome::Success;
            app.update();
            app.world_mut().resource_mut::<AlsShowDebugHudState>().visible = true;
            app.update();
            assert!(trace_points(&app, mantle_forward_color()).is_empty(), "Enabling the category must not replay a hidden probe");
        }
    }

    #[test]
    fn capsule_and_oblique_cone_match_source_line_batch_geometry() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, bevy::gizmos::GizmoPlugin, AlsDebugDisplayPlugin))
            .init_resource::<Assets<AlsControlSchemeConfig>>();
        app.add_systems(Update, |mut lines: Gizmos<DebugLines>, mut capsules: Gizmos<PhysicsGizmos>, mut stats: ResMut<AlsDebugDisplayStats>| {
            draw_view_cone(&mut lines, &mut stats, Vec3::ZERO, Vec3::new(0.3, 0.4, -0.7).normalize(), Color::WHITE);
            draw_debug_capsule(&mut capsules, Vec3::ZERO, 0.9, 0.3, Quat::IDENTITY, Color::WHITE);
        });
        app.update();
        let assets = app.world().resource::<Assets<bevy::gizmos::GizmoAsset>>();
        let capsule = assets.iter().find(|(_, asset)| asset.config_typeid() == std::any::TypeId::of::<PhysicsGizmos>()).unwrap().1.buffer();
        let segments = capsule.list_positions.len() / 2 + capsule.strip_positions.windows(2)
            .filter(|pair| pair.iter().all(|point| point.is_finite())).count();
        assert_eq!(segments, 68);
        let cone = assets.iter().find(|(_, asset)| asset.config_typeid() == std::any::TypeId::of::<DebugLines>()).unwrap().1.buffer();
        let direction = Vec3::new(0.3, 0.4, -0.7).normalize();
        let up = (Vec3::Y - direction * direction.y).normalize();
        for index in 0..8 {
            let angle = index as f32 * std::f32::consts::FRAC_PI_4;
            let point = direction * 15.0f32.to_radians().cos()
                + (up * angle.cos() + direction.cross(up) * angle.sin()) * 15.0f32.to_radians().sin();
            assert!(cone.list_positions.iter().chain(&cone.strip_positions).any(|actual| actual.abs_diff_eq(point, 1e-5)));
        }
    }

    #[test]
    fn mesh_bounds_use_current_shapes_and_exclude_source_root_body() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::ragdoll::{definition::{Collider as RagdollCollider, ColliderLabel}, spawning::SpawnedRagdoll};
        let mut world = World::new();
        let profile = crate::pa_als::parse_pa_als_profile();
        let mut spawned = SpawnedRagdoll::new(Entity::PLACEHOLDER);
        for (label, x, radius) in [
            (profile.colliders.iter().find(|shape| shape.label.starts_with("root_")).unwrap().label.clone(), 1000.0, 100.0),
            (profile.colliders.iter().find(|shape| shape.label.starts_with("hand_l_")).unwrap().label.clone(), -3.0, 0.5),
            (profile.colliders.iter().find(|shape| shape.label.starts_with("hand_r_")).unwrap().label.clone(), 5.0, 1.0),
        ] {
            let entity = world.spawn((GlobalTransform::from_translation(Vec3::X * x), Collider::sphere(radius), ColliderLabel(label))).id();
            spawned.colliders.insert(RagdollCollider::new().id, entity);
        }
        let bounds = world.run_system_once(move |query: Query<(&GlobalTransform, &Collider, &ColliderLabel)>| mesh_debug_bounds(&spawned, &profile, &query)).unwrap().unwrap();
        assert!(bounds.min.abs_diff_eq(Vec3::new(-3.5, -1.0, -1.0), 1e-5));
        assert!(bounds.max.abs_diff_eq(Vec3::new(6.0, 1.0, 1.0), 1e-5));
    }

    #[test]
    fn target_bounds_follow_master_without_shape_category() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            bevy::gizmos::GizmoPlugin, AlsDebugDisplayPlugin))
            .init_resource::<Assets<AlsControlSchemeConfig>>()
            .insert_resource(crate::show_debug_hud::AlsShowDebugHudState { visible: true, ..default() });
        app.world_mut().spawn((AlsCharacter, AlsPlayerCharacter, GlobalTransform::from_translation(Vec3::ONE),
            Collider::cuboid(4.0, 6.0, 8.0)));
        app.update();
        assert_eq!(app.world().resource::<AlsDebugDisplayStats>().bounds_boxes_drawn, 1);
        assert_eq!(app.world().resource::<AlsDebugDisplayStats>().capsule_wireframes_drawn, 0);
        let assets = app.world().resource::<Assets<bevy::gizmos::GizmoAsset>>();
        let asset = assets.iter().find(|(_, asset)| !asset.buffer().list_positions.is_empty() || !asset.buffer().strip_positions.is_empty()).unwrap().1;
        let points = asset.buffer().list_positions.iter().chain(&asset.buffer().strip_positions).filter(|p| p.is_finite());
        let (min, max) = points.fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |(min, max), p| (min.min(*p), max.max(*p)));
        assert!(min.abs_diff_eq(Vec3::ONE - Vec3::new(2.0, 3.0, 4.0) * 1.1, 1e-5));
        assert!(max.abs_diff_eq(Vec3::ONE + Vec3::new(2.0, 3.0, 4.0) * 1.1, 1e-5));
        assert_eq!(app.world().resource::<GizmoConfigStore>().get_config_dyn(&asset.config_typeid()).unwrap().0.line.width, 2.0);
        app.world_mut().resource_mut::<crate::show_debug_hud::AlsShowDebugHudState>().visible = false;
        app.update();
        assert_eq!(app.world().resource::<AlsDebugDisplayStats>().bounds_boxes_drawn, 0);
    }

    #[test]
    fn shapes_use_current_scaled_collider_eye_height_and_controller_speed() {
        use als_core::{control_scheme::{AlsControlScheme, AlsControlSchemeConfig}, state::Stance};
        use avian3d::prelude::Collider;
        use bevy_tnua::TnuaConfig;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, bevy::gizmos::GizmoPlugin, AlsDebugDisplayPlugin))
            .init_resource::<Assets<AlsControlSchemeConfig>>().init_resource::<Assets<als_camera::AlsCameraDefinition>>()
            .insert_resource(crate::show_debug_hud::AlsShowDebugHudState { visible: true, shapes: true, ..default() });
        let definition: als_camera::AlsCameraDefinition = ron::from_str(include_str!("../assets/params/als_camera.camera_component.ron")).unwrap();
        let camera = app.world_mut().resource_mut::<Assets<als_camera::AlsCameraDefinition>>().add(definition.clone());
        let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>().add(AlsControlSchemeConfig::default());
        let origin = Vec3::new(1.0, 3.0, 2.0);
        let actor = app.world_mut().spawn((AlsCharacter, AlsPlayerCharacter, GlobalTransform::from_translation(origin),
            LinearVelocity(Vec3::NEG_Z), AlsInputCommand { move_dir: Vec2::Y, ..default() },
            crate::actual_motion::AlsActualMotion { world_velocity: Vec3::NEG_Z, horizontal_speed: 1.0, has_input: true, ..default() },
            als_core::motion::AlsViewYaw(0.0), als_core::motion::AlsRotationResponseState::default(),
            TnuaConfig::<AlsControlScheme>(config.clone()), als_camera::AlsCameraDefinitionHandle(camera))).id();
        for (stance, half_height, radius, scale, max_speed) in [
            (Stance::Standing, 0.9, 0.3, Vec3::ONE, 1.75),
            (Stance::Crouching, 0.56, 0.3, Vec3::splat(2.0), 2.0),
            (Stance::Standing, 1.1, 0.4, Vec3::new(0.7, 1.5, 1.2), 3.75),
            (Stance::Crouching, 0.56, 0.3, Vec3::ONE, 0.0),
        ] {
            let mut collider = als_core::character::als_character_capsule(half_height, radius);
            collider.set_scale(scale, 16);
            let half_extents = collider.shape_scaled().compute_local_aabb().half_extents();
            app.world_mut().entity_mut(actor).insert((stance, collider));
            if max_speed == 0.0 {
                let mut motion = app.world_mut().get_mut::<crate::actual_motion::AlsActualMotion>(actor).unwrap();
                motion.horizontal_speed = 0.0;
                motion.world_velocity = Vec3::ZERO;
            }
            app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>().get_mut(&config).unwrap().basis.speed = max_speed;
            app.update();
            let assets = app.world().resource::<Assets<bevy::gizmos::GizmoAsset>>();
            let colored = |color: Color| assets.iter().flat_map(|(_, asset)| {
                let buffer = asset.buffer();
                buffer.list_positions.iter().zip(&buffer.list_colors).chain(buffer.strip_positions.iter().zip(&buffer.strip_colors))
            }).filter(|(point, value)| point.is_finite() && **value == color.to_linear()).map(|(point, _)| *point).collect::<Vec<_>>();
            let capsule: Vec<_> = assets.iter().filter(|(_, asset)| asset.config_typeid() == std::any::TypeId::of::<PhysicsGizmos>())
                .flat_map(|(_, asset)| asset.buffer().list_positions.iter().chain(&asset.buffer().strip_positions))
                .filter(|point| point.is_finite()).copied().collect();
            let min = capsule.iter().copied().fold(Vec3::INFINITY, Vec3::min);
            let max = capsule.iter().copied().fold(Vec3::NEG_INFINITY, Vec3::max);
            assert!(min.abs_diff_eq(origin - half_extents, 1e-5) && max.abs_diff_eq(origin + half_extents, 1e-5), "Collider bounds {half_extents:?}, debug bounds {min:?}..{max:?}");
            let input = colored(input_arrow_color(true));
            assert!(input[0].abs_diff_eq(origin + Vec3::Y * (0.03 - half_extents.y), 1e-5));
            assert_eq!(input.len(), 6);
            assert!((input[2].distance(input[3]) - 0.1).abs() < 1e-5);
            let velocity = colored(velocity_arrow_color(max_speed != 0.0));
            assert!(velocity.len() >= 2, "Velocity arrow must remain finite with zero speed and zero maximum speed");
            assert!((velocity[0].distance(velocity[1]) - if max_speed == 0.0 { 0.75 } else { 0.5 + 0.25 / max_speed }).abs() < 1e-5);
            let eye_height = if stance == Stance::Crouching { definition.crouching_eye_height } else { definition.standing_eye_height };
            assert!(colored(Color::linear_rgb(1.0, 0.0, 0.0)).iter().any(|point| point.abs_diff_eq(origin + Vec3::Y * eye_height, 1e-5)));
            let arrows = assets.iter().find(|(_, asset)| asset.buffer().list_colors.contains(&input_arrow_color(true).to_linear())).unwrap().1;
            assert_eq!(app.world().resource::<GizmoConfigStore>().get_config_dyn(&arrows.config_typeid()).unwrap().0.line.width, 3.0);
            assert!(app.world().get::<Collider>(actor).is_some());
        }
    }

    #[test]
    fn shapes_and_hud_follow_consumed_world_motion_and_retain_idle_directions() {
        use als_core::{control_scheme::AlsControlScheme, orientation::world_forward_to_yaw, state::*};
        use crate::{actual_motion::AlsActualMotionPlugin, show_debug_hud::{AlsShowDebugHudPlugin, AlsShowDebugHudState}};
        use bevy_tnua::prelude::TnuaController;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, bevy::gizmos::GizmoPlugin))
            .add_plugins((AlsActualMotionPlugin, AlsShowDebugHudPlugin, AlsDebugDisplayPlugin))
            .init_resource::<Assets<AlsControlSchemeConfig>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(AlsShowDebugHudState { visible: true, shapes: true, ..default() });
        let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>().add(AlsControlSchemeConfig::default());
        let character = app.world_mut().spawn((AlsCharacter, AlsPlayerCharacter, Transform::IDENTITY, GlobalTransform::IDENTITY,
            LinearVelocity::ZERO, als_core::motion::AlsViewYaw(0.0), als_core::view::AlsViewPitch(0.0),
            AlsInputCommand { move_dir: Vec2::Y, ..default() }, AlsInputOverride(AlsInputCommand::default()),
            TnuaController::<AlsControlScheme>::default(), als_core::motion::AlsRotationResponseState::default(),
            als_core::character::als_character_capsule(0.9, 0.3), TnuaConfig::<AlsControlScheme>(config),
            (LocomotionState::Air, RotationMode::ViewDirection, Stance::Standing, Gait::Running, OverlayState::Default,
                als_core::transition::AlsStateRequest::default()))).id();
        for direction in [Vec3::NEG_Z, Vec3::X, Vec3::Z, Vec3::NEG_X, Vec3::Y, Vec3::ZERO] {
            let moving = direction.x != 0.0 || direction.z != 0.0;
            let yaw = if moving { world_forward_to_yaw(direction) } else { std::f32::consts::FRAC_PI_2 };
            app.world_mut().get_mut::<LinearVelocity>(character).unwrap().0 = direction;
            app.world_mut().get_mut::<TnuaController<AlsControlScheme>>(character).unwrap().basis.desired_motion = if moving { direction * 0.0002 } else { Vec3::ZERO };
            app.world_mut().get_mut::<als_core::motion::AlsViewYaw>(character).unwrap().0 = yaw;
            app.update();
            let assets = app.world().resource::<Assets<bevy::gizmos::GizmoAsset>>();
            let buffer = assets.iter().find(|(_, asset)| asset.config_typeid() == std::any::TypeId::of::<ShapeArrows>()).unwrap().1.buffer();
            for color in [input_arrow_color(moving), velocity_arrow_color(moving)] {
                let index = buffer.list_colors.iter().position(|value| *value == color.to_linear()).unwrap();
                let actual = (buffer.list_positions[index + 1] - buffer.list_positions[index]).normalize();
                let expected = if moving { direction } else { Vec3::NEG_X };
                assert!(actual.abs_diff_eq(expected, 1e-5), "Arrow direction {actual:?} differs from consumed {expected:?}");
            }
            let values: std::collections::HashMap<_, _> = app.world_mut().query::<(&Name, &Text)>().iter(app.world())
                .map(|(name, text)| (name.as_str().to_owned(), text.0.clone())).collect();
            let heading = if moving { direction } else { Vec3::NEG_X };
            let source_yaw = als_core::turn_in_place::wrap_angle_degrees(heading.z.atan2(heading.x).to_degrees());
            assert_eq!(values["Input Yaw Angle"], format!("{source_yaw:.2}"));
            assert_eq!(values["Velocity Yaw Angle"], format!("{source_yaw:.2}"));
            assert_eq!(values["Velocity Direction"], format!("X: {:.2}Y: {:.2}Z: {:.2}", direction.x, direction.z, direction.y));
        }
    }

    #[test]
    fn persistent_footstep_gizmos_preserve_source_vertices_colors_and_point_size() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .add_plugins(bevy::mesh::MeshPlugin)
            .add_plugins((bevy::gizmos::GizmoPlugin, AlsDebugDisplayPlugin));
        app.init_resource::<Assets<AlsControlSchemeConfig>>();
        let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>().add(AlsControlSchemeConfig::default());
        let player = app.world_mut().spawn(AlsPlayerCharacter).id();
        let mut camera = Camera { viewport: Some(bevy::camera::Viewport { physical_size: UVec2::new(800, 600), ..default() }), ..default() };
        camera.computed.clip_from_view = Mat4::perspective_infinite_reverse_rh(std::f32::consts::FRAC_PI_2, 4.0 / 3.0, 0.1);
        for depth in [3.0, 30.0] {
            let (a, b, width) = debug_point_segment(Vec3::NEG_Z * depth, &camera, &GlobalTransform::IDENTITY).unwrap();
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
        let lines = assets.iter().find(|(_, asset)| asset.config_typeid() == std::any::TypeId::of::<DebugLines>()).unwrap().1.buffer();
        assert_eq!(lines.list_positions.len(), 12);
        assert_eq!(lines.list_positions[0], point + Vec3::Y);
        assert_eq!(lines.list_positions[1], point - Vec3::Y);
        for (index, axis, color) in [(3, Vec3::X, LinearRgba::RED), (5, Vec3::Z, LinearRgba::GREEN), (7, Vec3::Y, LinearRgba::BLUE)] {
            assert_eq!(lines.list_positions[index], point + axis * 0.25);
            assert_eq!(lines.list_colors[index], color);
        }
        let points = assets.iter().find(|(_, asset)| asset.config_typeid() == std::any::TypeId::of::<DebugPoints>()).unwrap().1.buffer();
        assert_eq!(points.list_positions.len(), 4);
        assert!(points.list_positions[0].abs_diff_eq(point - Vec3::X * 0.12, 1e-6));
        assert!(points.list_positions[1].abs_diff_eq(point + Vec3::X * 0.12, 1e-6));
        let configs = app.world().resource::<GizmoConfigStore>();
        assert_eq!(configs.config::<DebugLines>().0.line.width, 1.0);
        assert!((configs.config::<DebugPoints>().0.line.width - 24.0).abs() < 1e-4);
        app.world_mut().entity_mut(player).insert((AlsCharacter, Transform::default(), GlobalTransform::default(),
            crate::actual_motion::AlsActualMotion::default(),
            als_core::character::als_character_capsule(0.9, 0.3), Stance::Standing, TnuaConfig::<AlsControlScheme>(config),
            LinearVelocity::ZERO, als_core::motion::AlsViewYaw(0.0), als_core::view::AlsViewPitch(0.0), AlsInputCommand::default(),
            als_core::motion::AlsRotationResponseState { smooth_target_yaw: std::f32::consts::FRAC_PI_2, ..default() }));
        let mut selected = None;
        for character in [other, player] {
            let mut component = crate::camera_component::AlsSourceCameraComponent::default();
            component.activate(false);
            let entity = app.world_mut().spawn((component, als_camera::AlsCharacterCamera { character },
                AlsCameraDebugInfo { valid: true, pivot_target: Vec3::ONE, pivot: Some(Vec3::Y), pivot_lag: Some(Vec3::X),
                    trace_start: Vec3::ZERO, trace_end: Vec3::NEG_Z, ..default() })).id();
            let view = app.world_mut().spawn((Camera3d::default(), als_camera::AlsCameraViewSource(entity), als_camera::AlsCharacterCamera { character })).id();
            if character == player { selected = Some(view); }
        }
        app.world_mut().entity_mut(player).insert(crate::character_assembly::AlsCharacterAssembly { avatar: Entity::PLACEHOLDER, camera: selected });
        app.insert_resource(crate::show_debug_hud::AlsShowDebugHudState { visible: true, shapes: true, camera_shapes: true, camera_traces: true, ..default() });
        app.update();
        let stats = app.world().resource::<AlsDebugDisplayStats>();
        assert_eq!(stats.arrows_drawn, 4, "Three character arrows and the camera sweep arrow must be drawn");
        assert_eq!((stats.camera_pivot_spheres_drawn, stats.camera_trace_lines_drawn), (9, 1));
        let assets = app.world().resource::<Assets<bevy::gizmos::GizmoAsset>>();
        let standard = assets.iter().find(|(_, asset)| asset.config_typeid() == std::any::TypeId::of::<ShapeArrows>()).unwrap().1.buffer();
        assert!(standard.list_positions.iter().any(|point| point.abs_diff_eq(Vec3::new(-0.5, -0.84, 0.0), 1e-6)));
    }
}

fn draw_trace_markers(
    history: Res<crate::footprint::FootstepDebugHistory>,
    mantle: Res<MantleDebugHistory>,
    time: Res<Time>,
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut drawing: ParamSet<(ResMut<GizmoConfigStore>, Gizmos<DebugLines>, Gizmos<DebugPoints>)>,
    players: Query<Entity, With<crate::character_assembly::AlsPlayerCharacter>>,
    targets: Query<Entity, With<crate::debug_target::AlsDebugTarget>>,
    views: Query<(&Camera, &GlobalTransform, &als_camera::AlsCharacterCamera), With<Camera3d>>,
    assemblies: Query<&crate::character_assembly::AlsCharacterAssembly>,
    predictions: Query<(&crate::character_assembly::AlsCharacterAvatar, &crate::locomotion_layer::AlsGroundPredictionTrace)>,
) {
    let view = players.single().ok().and_then(|player| views.iter().find(|(camera, _, owner)| camera.is_active && owner.character == player));
    for trace in &history.0 {
        drawing.p1().line(trace.start, trace.end, linear(0.333333, 0.0, 0.0));
        stats.footstep_trace_lines_drawn += 1;
        if let Some(point) = trace.hit {
            if let Some(rotation) = trace.rotation {
                for (axis, color) in [(Vec3::X, linear(1.0, 0.0, 0.0)), (Vec3::Z, linear(0.0, 1.0, 0.0)), (Vec3::Y, linear(0.0, 0.0, 1.0))] {
                    drawing.p1().line(point, point + rotation * axis * 0.25, color);
                    stats.footstep_axis_lines_drawn += 1;
                }
            }
        }
    }
    let footprint_points = history.0.iter().filter_map(|trace| trace.hit.map(|point| (point, linear(1.0, 0.0, 0.0), TraceMarkerKind::Footstep)));
    let mantle_points = mantle.0.iter().flat_map(|(created, trace)| {
        let age = time.elapsed_secs_f64() - created;
        [
            (trace.result != MantleProbeResult::ForwardRejected && mantle_part_visible(trace, age, 5.0))
                .then_some(trace.forward_hit).flatten().map(|point| (point, linear(0.0, 0.75, 1.0), TraceMarkerKind::Mantle)),
            (trace.result == MantleProbeResult::Success && mantle_part_visible(trace, age, 7.5))
                .then_some(trace.downward_hit).flatten().map(|point| (point, linear(0.75, 0.0, 1.0), TraceMarkerKind::Mantle)),
        ].into_iter().flatten()
    });
    let ground_point = targets.single().ok().and_then(|player| {
        if !stats.traces_active { return None; }
        let assembly = assemblies.get(player).ok()?;
        let (owner, trace) = predictions.get(assembly.avatar).ok()?;
        (owner.character == player).then_some(trace.hit).flatten().map(|(_, point)| (point, linear(0.75, 0.0, 1.0), TraceMarkerKind::GroundPrediction))
    });
    for (point, color, kind) in footprint_points.chain(mantle_points).chain(ground_point) {
        if let Some((camera, view, _)) = view && let Some((start, end, width)) = debug_point_segment(point, camera, view) {
            drawing.p0().config_mut::<DebugPoints>().0.line.width = width;
            drawing.p2().line(start, end, color);
            match kind {
                TraceMarkerKind::Footstep => stats.footstep_hit_points_drawn += 1,
                TraceMarkerKind::Mantle => stats.mantle_hit_points_drawn += 1,
                TraceMarkerKind::GroundPrediction => stats.ground_prediction_hit_points_drawn += 1,
            }
        }
    }
}

fn draw_view_cone(
    gizmos: &mut Gizmos<DebugLines>,
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
    let axis = if dir.y.abs() > dir.x.abs() && dir.y.abs() > dir.z.abs() { Vec3::X } else { Vec3::Y };
    let x = (axis - *dir * dir.dot(axis)).normalize();
    let y = dir.cross(x);
    let mut points = Vec::with_capacity(CONE_SEGMENTS as usize + 2);
    for i in 0..=CONE_SEGMENTS {
        let angle = (i as f32 / CONE_SEGMENTS as f32) * std::f32::consts::TAU;
        points.push(center + (x * angle.cos() + y * angle.sin()) * ring_radius);
    }
    gizmos.linestrip(points.iter().copied(), color);
    for point in &points[..CONE_SEGMENTS as usize] {
        gizmos.line(apex, *point, color);
    }
    stats.cone_segments_drawn += CONE_SEGMENTS + CONE_SEGMENTS;
}

fn draw_directional_arrow<C: GizmoConfigGroup>(
    gizmos: &mut Gizmos<C>,
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
        let mut side = forward.cross(Vec3::Y);
        if (side.length_squared() - 1.0).abs() >= 0.01 {
            let up = if forward.y.abs() > forward.x.abs() && forward.y.abs() > forward.z.abs() { Vec3::X } else { Vec3::Y };
            side = (up - forward * up.dot(forward)).normalize_or_zero().cross(forward);
        }
        let extent = (head_size / UE_CM_TO_M).sqrt() * UE_CM_TO_M;
        gizmos.line(end, end + (side - forward) * extent, color);
        gizmos.line(end, end - (side + forward) * extent, color);
    }
    stats.arrows_drawn += 1;
}

#[allow(clippy::type_complexity)]
fn draw_character_shapes_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut lines: Gizmos<DebugLines>,
    mut arrows: Gizmos<ShapeArrows>,
    mut colliders: Gizmos<PhysicsGizmos>,
    configs: Option<Res<Assets<AlsControlSchemeConfig>>>,
    definitions: Option<Res<Assets<als_camera::AlsCameraDefinition>>>,
    q_character: Query<
        (
            &GlobalTransform,
            &crate::actual_motion::AlsActualMotion,
            &als_core::motion::AlsViewYaw,
            Option<&als_core::view::AlsViewPitch>,
            Option<&als_core::motion::AlsRotationResponseState>,
            &Collider,
            &Stance,
            Option<&TnuaConfig<AlsControlScheme>>,
            Option<&als_camera::AlsCameraDefinitionHandle>,
        ),
        (With<AlsCharacter>, With<crate::debug_target::AlsDebugTarget>),
    >,
) {
    if !stats.shapes_active {
        return;
    }
    for (transform, motion, view_yaw, view_pitch, response, collider, stance, config, definition) in q_character.iter() {
        let origin = transform.translation();
        let half_height = collider.shape_scaled().compute_local_aabb().half_extents().y;
        let feet_location = origin - Vec3::Y * half_height;
        let pitch = view_pitch.map(|s| s.0).unwrap_or(0.0);
        let view_rotation = Quat::from_euler(EulerRot::YXZ, view_yaw.0, pitch, 0.0);

        if let Some(definition) = definition.and_then(|handle| definitions.as_ref()?.get(&handle.0)) {
            let eye_height = if *stance == Stance::Crouching { definition.crouching_eye_height } else { definition.standing_eye_height };
            draw_view_cone(
                &mut lines,
                &mut stats,
                origin + Vec3::Y * eye_height,
                view_rotation * Vec3::NEG_Z,
                linear(1.0, 0.0, 0.0),
            );
        }

        draw_directional_arrow(
            &mut arrows,
            feet_location + Vec3::Y * INPUT_ARROW_LIFT_M,
            feet_location
                + Vec3::Y * INPUT_ARROW_LIFT_M
                + als_core::orientation::yaw_to_world_forward(motion.input_yaw_degrees.to_radians()) * ARROW_HEAD_M,
            ARROW_HEAD_M,
            input_arrow_color(motion.has_input),
            &mut stats,
        );

        let hspeed = motion.horizontal_speed;
        let has_velocity = als_core::motion::locomotion_movement_flags(motion.has_input, hspeed).0;
        if let Some(config) = config.and_then(|handle| configs.as_ref()?.get(&handle.0)) {
            let max_speed = config.basis.speed;
            let speed_ratio = if max_speed.abs() <= 1e-10 { if hspeed >= max_speed { 1.0 } else { 0.0 } } else { (hspeed / max_speed).clamp(0.0, 1.0) };
            let velocity_arrow_length = VELOCITY_MIN_LENGTH_M
                + (VELOCITY_MAX_LENGTH_M - VELOCITY_MIN_LENGTH_M) * speed_ratio;
            draw_directional_arrow(
                &mut arrows,
                feet_location,
                feet_location + als_core::orientation::yaw_to_world_forward(motion.velocity_yaw_degrees.to_radians()) * velocity_arrow_length,
                ARROW_HEAD_M,
                velocity_arrow_color(has_velocity),
                &mut stats,
            );
        }

        if let Some(target_yaw) = response.map(|response| response.smooth_target_yaw) {
            draw_directional_arrow(
                &mut arrows,
                feet_location + Vec3::Y * SMOOTH_ARROW_LIFT_M,
                feet_location
                    + Vec3::Y * SMOOTH_ARROW_LIFT_M
                    + als_core::orientation::yaw_to_world_forward(target_yaw) * ARROW_HEAD_M,
                ARROW_HEAD_M,
                smooth_target_yaw_color(),
                &mut stats,
            );
        }

        draw_debug_collider(&mut colliders, collider, origin, transform.rotation(), capsule_wire_color());
        stats.capsule_wireframes_drawn += 1;
    }
}

#[allow(clippy::type_complexity)]
fn draw_bounds_box_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    state: Option<Res<crate::show_debug_hud::AlsShowDebugHudState>>,
    mut gizmos: Gizmos<TargetBounds>,
    q_character: Query<(Entity, &GlobalTransform, &Collider, Option<&als_core::state::AlsNotifyLocomotionAction>, Option<&crate::character_assembly::AlsCharacterAssembly>), (With<AlsCharacter>, With<crate::debug_target::AlsDebugTarget>)>,
    avatars: Query<(&crate::character_assembly::AlsCharacterAvatar, &bevy_animation_graph::core::animated_scene::AnimatedSceneInstance)>,
    players: Query<(&bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer, &als_ext::RagdollPhysicsProfile)>,
    colliders: Query<(&GlobalTransform, &Collider, &bevy_animation_graph::core::ragdoll::definition::ColliderLabel)>,
) {
    if !state.is_some_and(|state| state.visible) {
        return;
    }
    let Ok((character, transform, collider, action, assembly)) = q_character.single() else {
        return;
    };
    let mut bounds = (action != Some(&als_core::state::AlsNotifyLocomotionAction::Ragdolling))
        .then(|| collider.aabb(transform.translation(), transform.rotation()));
    if let Some(assembly) = assembly
        && let Ok((owner, instance)) = avatars.get(assembly.avatar)
        && owner.character == character
        && let Ok((player, profile)) = players.get(instance.player_entity())
        && let Some(spawned) = player.get_spawned_ragdoll()
    {
        if let Some(mesh) = mesh_debug_bounds(spawned, profile, &colliders) {
            bounds = Some(bounds.map_or(mesh, |capsule| capsule.merged(mesh)));
        }
    }
    let Some(aabb) = bounds else { return; };
    let center = (aabb.min + aabb.max) * 0.5;
    let extent = (aabb.max - aabb.min) * 0.5 * BOUNDS_EXPAND;
    gizmos.primitive_3d(
        &Cuboid::from_size(extent * 2.0),
        Isometry3d::new(center, Quat::IDENTITY),
        bounds_box_color(),
    );
    stats.bounds_boxes_drawn += 1;
}

fn mesh_debug_bounds(
    spawned: &bevy_animation_graph::core::ragdoll::spawning::SpawnedRagdoll,
    profile: &als_ext::RagdollPhysicsProfile,
    colliders: &Query<(&GlobalTransform, &Collider, &bevy_animation_graph::core::ragdoll::definition::ColliderLabel)>,
) -> Option<ColliderAabb> {
    spawned.colliders.values().filter_map(|entity| {
        let (transform, collider, label) = colliders.get(*entity).ok()?;
        profile.colliders.iter().any(|entry| entry.label == label.0 && entry.consider_for_bounds)
            .then(|| collider.aabb(transform.translation(), transform.rotation()))
    }).reduce(ColliderAabb::merged)
}

#[allow(clippy::type_complexity)]
fn draw_mantle_probes_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut lines: Gizmos<DebugLines>,
    mut colliders: Gizmos<PhysicsGizmos>,
    history: Res<MantleDebugHistory>,
    time: Res<Time>,
) {
    for (created, trace) in &history.0 {
        let age = time.elapsed_secs_f64() - created;
        if let (Some(start), Some(end)) = (trace.forward_start, trace.forward_end)
            && mantle_part_visible(trace, age, 5.0) {
            draw_capsule_sweep(&mut lines, start, end, trace.trace_radius, trace.forward_half_height, mantle_forward_color(), &mut stats);
            if trace.result != MantleProbeResult::ForwardRejected && let Some(center) = trace.forward_hit_center {
                draw_debug_capsule(&mut colliders, center, trace.forward_half_height, trace.trace_radius, sweep_rotation(end - start), linear(0.0, 0.75, 1.0));
            }
            stats.mantle_probe_lines_drawn += 1;
        }
        if let (Some(start), Some(end)) = (trace.downward_start, trace.downward_end)
            && mantle_part_visible(trace, age, 7.5) {
            draw_sphere_sweep(&mut lines, &mut colliders, start, end, trace.trace_radius, mantle_downward_color(), &mut stats);
            if trace.result == MantleProbeResult::Success && let Some(center) = trace.downward_hit_center {
                draw_sphere_rings(&mut lines, center, sweep_rotation(end - start), trace.trace_radius, linear(0.75, 0.0, 1.0));
            }
            stats.mantle_probe_lines_drawn += 1;
        }
        if mantle_part_visible(trace, age, 10.0) {
            let overlap = match trace.result {
                MantleProbeResult::TargetBlocked => trace.target_capsule_center.map(|center| (center, trace.target_half_height, trace.target_radius, linear(1.0, 0.0, 0.0))),
                MantleProbeResult::StartBlocked => trace.start_capsule_center.map(|center| (center, trace.start_half_height, trace.trace_radius, linear(1.0, 0.5, 0.0))),
                _ => None,
            };
            if let Some((center, half_height, radius, color)) = overlap {
                draw_debug_capsule(&mut colliders, center, half_height, radius, Quat::IDENTITY, color);
                stats.mantle_overlap_capsules_drawn += 1;
            }
        }
    }
}

fn sweep_rotation(delta: Vec3) -> Quat {
    let forward = delta.try_normalize().unwrap_or(Vec3::X);
    let right = forward.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::Z);
    Quat::from_mat3(&Mat3::from_cols(right, right.cross(forward), -forward))
}

fn draw_debug_collider(gizmos: &mut Gizmos<PhysicsGizmos>, collider: &Collider, origin: Vec3, rotation: Quat, color: Color) {
    if let Some(capsule) = collider.shape_scaled().as_capsule() {
        let delta = capsule.segment.b - capsule.segment.a;
        let center = origin + rotation * ((capsule.segment.a + capsule.segment.b) * 0.5);
        let rotation = rotation * Quat::from_rotation_arc(Vec3::Y, delta.try_normalize().unwrap_or(Vec3::Y));
        draw_debug_capsule(gizmos, center, delta.length() * 0.5 + capsule.radius, capsule.radius, rotation, color);
    } else {
        gizmos.draw_collider(collider, origin, rotation, color);
    }
}

fn draw_debug_capsule(gizmos: &mut Gizmos<PhysicsGizmos>, center: Vec3, half_height: f32, radius: f32, rotation: Quat, color: Color) {
    let x = rotation * Vec3::X;
    let y = rotation * Vec3::Z;
    let z = rotation * Vec3::Y;
    let half_axis = (half_height - radius).max(UE_CM_TO_M);
    let top = center + z * half_axis;
    let bottom = center - z * half_axis;
    for origin in [top, bottom] { draw_arc(gizmos, origin, x, y, radius, CIRCLE_SIDES, color); }
    for axis in [x, y] {
        draw_arc(gizmos, top, axis, z, radius, CIRCLE_SIDES / 2, color);
        draw_arc(gizmos, bottom, axis, -z, radius, CIRCLE_SIDES / 2, color);
        for sign in [-1.0, 1.0] { gizmos.line(top + axis * radius * sign, bottom + axis * radius * sign, color); }
    }
}

fn draw_arc<C: GizmoConfigGroup>(gizmos: &mut Gizmos<C>, center: Vec3, x: Vec3, y: Vec3, radius: f32, segments: u32, color: Color) {
    gizmos.linestrip((0..=segments).map(|index| {
        let angle = std::f32::consts::TAU * index as f32 / CIRCLE_SIDES as f32;
        center + radius * (x * angle.cos() + y * angle.sin())
    }), color);
}

fn draw_capsule_sweep(gizmos: &mut Gizmos<DebugLines>, start: Vec3, end: Vec3, radius: f32, half_height: f32, color: Color, stats: &mut AlsDebugDisplayStats) {
    let rotation = sweep_rotation(end - start);
    let forward = rotation * Vec3::NEG_Z;
    let right = rotation * Vec3::X;
    let up = rotation * Vec3::Y;
    let height = (half_height - radius).max(0.01);
    for (center, outside) in [(start, -forward), (end, forward)] {
        for sign in [-1.0, 1.0] {
            let pole = center + up * height * sign;
            draw_arc(gizmos, pole, right, outside, radius, CIRCLE_SIDES / 2, color);
            draw_arc(gizmos, pole, right, up * sign, radius, CIRCLE_SIDES / 2, color);
            draw_arc(gizmos, pole, outside, up * sign, radius, CIRCLE_SIDES / 4, color);
        }
        for axis in [outside, right, -right] {
            gizmos.line(center + up * height + axis * radius, center - up * height + axis * radius, color);
        }
    }
    for sign in [-1.0, 1.0] {
        gizmos.line(start + up * height * sign + right * radius, end + up * height * sign + right * radius, color);
        gizmos.line(start + up * height * sign - right * radius, end + up * height * sign - right * radius, color);
        gizmos.line(start + up * (height + radius) * sign, end + up * (height + radius) * sign, color);
    }
    draw_directional_arrow(gizmos, start, end, ARROW_HEAD_M, color, stats);
}

fn draw_sphere_sweep(lines: &mut Gizmos<DebugLines>, colliders: &mut Gizmos<PhysicsGizmos>, start: Vec3, end: Vec3, radius: f32, color: Color, stats: &mut AlsDebugDisplayStats) {
    let delta = end - start;
    let rotation = Quat::from_rotation_arc(Vec3::Y, delta.try_normalize().unwrap_or(Vec3::Y));
    draw_debug_capsule(colliders, (start + end) * 0.5, delta.length() * 0.5 + radius, radius, rotation, color);
    draw_directional_arrow(lines, start, end, ARROW_HEAD_M, color, stats);
}

fn draw_sphere_rings<C: GizmoConfigGroup>(gizmos: &mut Gizmos<C>, center: Vec3, rotation: Quat, radius: f32, color: Color) {
    for (x, y) in [(Vec3::NEG_Z, Vec3::X), (Vec3::NEG_Z, Vec3::Y), (Vec3::X, Vec3::Y)] {
        draw_arc(gizmos, center, rotation * x, rotation * y, radius, CIRCLE_SIDES, color);
    }
}

fn draw_ground_prediction_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut lines: Gizmos<DebugLines>,
    mut colliders: Gizmos<PhysicsGizmos>,
    characters: Query<(Entity, &crate::character_assembly::AlsCharacterAssembly), With<crate::debug_target::AlsDebugTarget>>,
    predictions: Query<(&crate::character_assembly::AlsCharacterAvatar, &crate::locomotion_layer::AlsGroundPredictionTrace)>,
) {
    if !stats.traces_active { return; }
    let Ok((character, assembly)) = characters.single() else { return; };
    let Ok((owner, trace)) = predictions.get(assembly.avatar) else { return; };
    if owner.character != character { return; }
    let color = linear(0.25, 0.0, 1.0);
    for center in [trace.start, trace.end] { draw_debug_collider(&mut colliders, &trace.shape, center, Quat::IDENTITY, color); }
    draw_directional_arrow(&mut lines, trace.start, trace.end, ARROW_HEAD_M, color, &mut stats);
    if let Some((center, _)) = trace.hit {
        draw_debug_collider(&mut colliders, &trace.shape, center, Quat::IDENTITY, linear(0.75, 0.0, 1.0));
    }
    stats.cumulative_ground_prediction_sweeps += 1;
}

fn draw_camera_debug_system(
    mut stats: ResMut<AlsDebugDisplayStats>,
    mut gizmos: Gizmos<DebugLines>,
    mut colliders: Gizmos<PhysicsGizmos>,
    mut history: ResMut<CameraAdjustmentHistory>,
    time: Res<Time>,
    characters: Query<(Entity, &crate::character_assembly::AlsCharacterAssembly), With<crate::debug_target::AlsDebugTarget>>,
    views: Query<(&Camera, &als_camera::AlsCameraViewSource, &als_camera::AlsCharacterCamera)>,
    cameras: Query<(&crate::camera_component::AlsSourceCameraComponent, &als_camera::AlsCharacterCamera, Ref<AlsCameraDebugInfo>)>,
) {
    let now = time.elapsed_secs_f64();
    history.0.retain(|(created, _, _)| now - created < 5.0);
    for (_, start, end) in &history.0 { gizmos.line(*start, *end, pivot_color()); }
    let Ok((character, assembly)) = characters.single() else { return; };
    let Some(camera) = assembly.camera else { return; };
    let Ok((view, source, view_owner)) = views.get(camera) else { return; };
    if !view.is_active || view_owner.character != character { return; }
    let Ok((component, owner, info)) = cameras.get(source.0) else { return; };
    if !component.is_active() || owner.character != character || !info.valid { return; }
        stats.camera_debug_valid += 1;
        stats.cumulative_camera_valid += 1;
        if info.first_person_override >= 1.0 - 1e-5 { return; }

        if stats.camera_traces_active {
            draw_sphere_sweep(&mut gizmos, &mut colliders, info.trace_start, info.trace_end, info.trace_radius,
                camera_trace_color(info.trace_blocking_hit), &mut stats);
            if info.is_changed() && info.trace_adjusted_start {
                let start = info.trace_start - info.trace_adjustment;
                history.0.push((now, start, info.trace_start));
                gizmos.line(start, info.trace_start, pivot_color());
            }
            stats.camera_trace_lines_drawn += 1;
            stats.cumulative_trace_lines += 1;
        }

        if !stats.camera_shapes_active {
            return;
        }

        let rotation = Quat::from_rotation_y(info.camera_yaw);
        draw_sphere_alternative(
            &mut gizmos,
            info.pivot_target,
            rotation,
            SPHERE_RADIUS_M,
            pivot_target_color(),
            &mut stats,
        );

        if let Some(pivot) = info.pivot {
            if let Some(lag) = info.pivot_lag { gizmos.line(pivot, lag, pivot_color()); }
            draw_sphere_alternative(
                &mut gizmos,
                pivot,
                rotation,
                SPHERE_RADIUS_M,
                pivot_color(),
                &mut stats,
            );
        }

        if let Some(lag) = info.pivot_lag {
            gizmos.line(lag, info.pivot_target, pivot_lag_color());
            draw_sphere_alternative(
                &mut gizmos,
                lag,
                rotation,
                SPHERE_RADIUS_M,
                pivot_lag_color(),
                &mut stats,
            );
        }
}

fn draw_sphere_alternative(
    gizmos: &mut Gizmos<DebugLines>,
    location: Vec3,
    rotation: Quat,
    radius: f32,
    color: Color,
    stats: &mut AlsDebugDisplayStats,
) {
    draw_sphere_rings(gizmos, location, rotation, radius, color);
    stats.camera_pivot_spheres_drawn += 3;
    stats.cumulative_pivot_spheres += 3;
}
