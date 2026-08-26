use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::camera::Exposure;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::Image;
use bevy::light::{CascadeShadowConfig, NotShadowCaster};
use bevy::prelude::*;
use bevy::render::view::ColorGrading;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::text::TextLayoutInfo;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;

use als_ext::playground_materials::{PlaygroundMaterialsPlugin, PlaygroundTilesMaterial};
use als_ext::{
    PlaygroundKinematic, PlaygroundKinematicKind, PlaygroundLandscape,
    PlaygroundLandscapeTilesMaterial, PlaygroundMaterialApplied, PlaygroundMaterialRole,
    PlaygroundPlugin, PlaygroundSky, PlaygroundStatic, PlaygroundTextFacing, PlaygroundTextFit,
    PlaygroundTextHorizontalAlignment, PlaygroundTextLabel, PlaygroundTextLayoutTarget,
    PlaygroundTextVerticalAlignment, PlaygroundWorldTextMesh,
};

const PLAYGROUND_CAMERA_EV100: f32 = 2.0;
const PLAYGROUND_CAMERA_CONTRAST: f32 = 1.05;
const LANDSCAPE_TILES_TILING: f32 = 0.08;
const LANDSCAPE_TILES_COLOR: Vec4 = Vec4::new(0.25, 0.5, 0.125, 1.0);

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        PhysicsPlugins::new(FixedPostUpdate),
        EnhancedInputPlugin,
        PlaygroundMaterialsPlugin,
        PlaygroundPlugin {
            manifest: "assets/models/playground/playground_scene.ron".into(),
        },
        AnimationGraphPlugin::default(),
    ))
    .init_resource::<SceneStats>()
    .insert_resource(GlobalAmbientLight {
        brightness: 2.0,
        ..default()
    })
    .add_systems(Startup, (spawn_camera, arm_schedule))
    .add_systems(
        Update,
        (
            stats_collector,
            landscape_material_stats_collector,
            kinematic_probe,
            verdict_system,
            screenshot_runner,
            auto_quit,
        )
            .chain(),
    );

    app.run();
}

#[derive(Resource, Default)]
struct SceneStats {
    static_count: u32,
    text_count: u32,
    native_text_count: u32,
    glyph_text_count: u32,
    world_text_mesh_count: u32,
    world_text_masked_count: u32,
    text_fitted_count: u32,
    text_fit_max_error: f32,
    text_center_aligned_count: u32,
    text_left_top_aligned_count: u32,
    text_front_matching_count: u32,
    text_front_max_error: f32,
    landscape_seen: bool,
    landscape_identity: bool,
    landscape_tiles_material_count: u32,
    landscape_tiles_tiling_error: f32,
    landscape_tiles_color_error: f32,
    landscape_tiles_mip_count: u32,
    sun_count: u32,
    sun_shadowed: bool,
    sun_ray_error: f32,
    sun_illuminance_error: f32,
    camera_exposure_count: u32,
    camera_exposure_error: f32,
    camera_contrast_error: f32,
    camera_aces_count: u32,
    sky_count: u32,
    main_material_count: u32,
    tiles_material_count: u32,
    translucent_material_count: u32,
    translucent_no_shadow_count: u32,
    extended_material_bound_count: u32,
    tiles_pbr_material_count: u32,
    translucent_pbr_material_count: u32,
    movers_total: u32,
    movers_moving: u32,
    spin_yaw_abs_deg: f32,
    cube_orientation_total: u32,
    cube_orientation_matching: u32,
    cube_orientation_max_error: f32,
    rock_scale_count: u32,
    rock_scale_error: f32,
    brush_count: u32,
    mover_prev: Vec<Vec3>,
    verdict_done: bool,
}

#[derive(Resource)]
struct Schedule {
    shot_at: Instant,
    shot_done: bool,
    quit_at: Instant,
    shot_path: String,
}

fn ue_source_rotator_quat(pitch_deg: f32, yaw_deg: f32, roll_deg: f32) -> Quat {
    let (sp, cp) = (pitch_deg.to_radians() * 0.5).sin_cos();
    let (sy, cy) = (yaw_deg.to_radians() * 0.5).sin_cos();
    let (sr, cr) = (roll_deg.to_radians() * 0.5).sin_cos();
    Quat::from_xyzw(
        cr * sp * sy - sr * cp * cy,
        -cr * sp * cy - sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
        cr * cp * cy + sr * sp * sy,
    )
}

fn expected_cube_rotator(name: &str) -> Option<(f32, f32, f32)> {
    match name {
        "StaticMeshActor_UAID_94B86D58D760241001_1128730399:box" => Some((0.0, 0.0, 25.95)),
        "StaticMeshActor_UAID_94B86D58D760231001_1754144221:box" => Some((0.0, 0.0, 35.1)),
        "StaticMeshActor_UAID_94B86D58D760221001_1788787042:box" => Some((0.0, 0.0, 44.25)),
        "StaticMeshActor_UAID_54BF6414A059D82202_1613061415:box"
        | "StaticMeshActor_UAID_54BF6414A0599C2202_1768185868:box" => Some((0.0, 90.0, -15.0)),
        _ => None,
    }
}

fn map_ue_direction(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, v.y)
}

fn spawn_camera(mut commands: Commands) {
    let text_camera = std::env::var("ALS_SCENE_CAMERA").ok().as_deref() == Some("text");
    let transform = if text_camera {
        Transform::from_xyz(3.33, 8.78, -42.0).looking_at(Vec3::new(3.33, 8.78, -46.24), Vec3::Y)
    } else {
        Transform::from_xyz(-35.0, 45.0, -60.0).looking_at(Vec3::new(-10.0, 0.0, -20.0), Vec3::Y)
    };
    commands.spawn((Camera3d::default(), transform));
}

fn arm_schedule(mut commands: Commands) {
    let path = std::env::var("ALS_SHOT_PATH")
        .unwrap_or_else(|_| "docs/bfscene-shots/gate-overview.png".to_string());
    let after = std::env::var("ALS_SHOT_AFTER_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6.0);
    let quit = std::env::var("ALS_BOOT_AUTO_QUIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20.0);
    commands.insert_resource(Schedule {
        shot_at: Instant::now() + Duration::from_secs_f64(after),
        shot_done: false,
        quit_at: Instant::now() + Duration::from_secs_f64(quit),
        shot_path: path,
    });
}

fn stats_collector(
    mut stats: ResMut<SceneStats>,
    statics: Query<(), With<PlaygroundStatic>>,
    texts: Query<(), With<PlaygroundTextLabel>>,
    native_texts: Query<(), (With<PlaygroundTextLabel>, With<Text2d>, With<Transform>)>,
    text_layouts: Query<&TextLayoutInfo, With<PlaygroundTextLabel>>,
    text_fits: Query<
        (
            &PlaygroundTextFit,
            &PlaygroundTextLayoutTarget,
            &Transform,
            &PlaygroundTextFacing,
        ),
        With<PlaygroundTextLabel>,
    >,
    world_text_meshes: Query<&MeshMaterial3d<StandardMaterial>, With<PlaygroundWorldTextMesh>>,
    landscapes: Query<&Transform, With<PlaygroundLandscape>>,
    sun_lights: Query<(&DirectionalLight, &CascadeShadowConfig, &Transform)>,
    cameras: Query<(&Exposure, &ColorGrading, &Tonemapping), With<Camera3d>>,
    skies: Query<(), With<PlaygroundSky>>,
    material_roles: Query<(&PlaygroundMaterialApplied, Has<NotShadowCaster>), With<Mesh3d>>,
    extended_materials: Query<(), With<MeshMaterial3d<PlaygroundTilesMaterial>>>,
    cube_visuals: Query<(&Name, &Transform), With<PlaygroundStatic>>,
    materials: Res<Assets<StandardMaterial>>,
    tiles_materials: Res<Assets<PlaygroundTilesMaterial>>,
) {
    stats.static_count = statics.iter().count() as u32;
    stats.text_count = texts.iter().count() as u32;
    stats.native_text_count = native_texts.iter().count() as u32;
    stats.glyph_text_count = text_layouts
        .iter()
        .filter(|layout| !layout.glyphs.is_empty())
        .count() as u32;
    stats.world_text_mesh_count = world_text_meshes.iter().count() as u32;
    stats.world_text_masked_count = world_text_meshes
        .iter()
        .filter(|handle| {
            materials.get(&handle.0).is_some_and(|material| {
                material.base_color_texture.is_some()
                    && matches!(material.alpha_mode, AlphaMode::Mask(_))
                    && material.cull_mode.is_none()
            })
        })
        .count() as u32;
    stats.text_fitted_count = 0;
    stats.text_fit_max_error = 0.0;
    stats.text_center_aligned_count = 0;
    stats.text_left_top_aligned_count = 0;
    stats.text_front_matching_count = 0;
    stats.text_front_max_error = 0.0;
    for (fit, target, transform, facing) in &text_fits {
        let error = fit.width_error.max(fit.height_error).max(fit.center_error);
        stats.text_fit_max_error = stats.text_fit_max_error.max(error);
        if error < 1e-4 {
            stats.text_fitted_count += 1;
        }
        if target.horizontal == PlaygroundTextHorizontalAlignment::Center {
            stats.text_center_aligned_count += 1;
        }
        if target.horizontal == PlaygroundTextHorizontalAlignment::Left
            && target.vertical == PlaygroundTextVerticalAlignment::Top
        {
            stats.text_left_top_aligned_count += 1;
        }
        let error = transform
            .rotation
            .mul_vec3(Vec3::Z)
            .distance(facing.expected_front);
        stats.text_front_max_error = stats.text_front_max_error.max(error);
        if error < 1e-4 {
            stats.text_front_matching_count += 1;
        }
    }
    if let Ok(transform) = landscapes.single() {
        stats.landscape_seen = true;
        stats.landscape_identity = transform.translation.length_squared() < 1e-6
            && transform.rotation.dot(Quat::IDENTITY).abs() > 1.0 - 1e-6
            && transform.scale.distance_squared(Vec3::ONE) < 1e-6;
    }
    stats.sun_count = sun_lights.iter().count() as u32;
    stats.sun_shadowed = sun_lights.iter().any(|(light, cascades, _)| {
        light.shadow_maps_enabled
            && cascades.bounds.len() == 4
            && cascades
                .bounds
                .last()
                .is_some_and(|distance| (*distance - 400.0).abs() < 0.01)
    });
    let expected_sun_ray = Vec3::new(0.5, -0.70710677, 0.5);
    stats.sun_ray_error = sun_lights
        .iter()
        .map(|(_, _, transform)| {
            transform
                .rotation
                .mul_vec3(Vec3::NEG_Z)
                .distance(expected_sun_ray)
        })
        .fold(f32::INFINITY, f32::min);
    stats.sun_illuminance_error = sun_lights
        .iter()
        .map(|(light, _, _)| (light.illuminance - 10.0).abs())
        .fold(f32::INFINITY, f32::min);
    stats.camera_exposure_count = cameras.iter().count() as u32;
    stats.camera_exposure_error = cameras
        .iter()
        .map(|(exposure, _, _)| (exposure.ev100 - PLAYGROUND_CAMERA_EV100).abs())
        .fold(f32::INFINITY, f32::min);
    stats.camera_contrast_error = cameras
        .iter()
        .map(|(_, grading, _)| {
            grading
                .all_sections()
                .map(|section| (section.contrast - PLAYGROUND_CAMERA_CONTRAST).abs())
                .fold(0.0, f32::max)
        })
        .fold(f32::INFINITY, f32::min);
    stats.camera_aces_count = cameras
        .iter()
        .filter(|(_, _, tonemapping)| matches!(**tonemapping, Tonemapping::AcesFitted))
        .count() as u32;
    stats.sky_count = skies.iter().count() as u32;
    stats.main_material_count = material_roles
        .iter()
        .filter(|(applied, _)| applied.0 == PlaygroundMaterialRole::Main)
        .count() as u32;
    stats.tiles_material_count = material_roles
        .iter()
        .filter(|(applied, _)| applied.0 == PlaygroundMaterialRole::Tiles)
        .count() as u32;
    stats.translucent_material_count = material_roles
        .iter()
        .filter(|(applied, _)| applied.0 == PlaygroundMaterialRole::Translucent)
        .count() as u32;
    stats.translucent_no_shadow_count = material_roles
        .iter()
        .filter(|(applied, no_shadow_caster)| {
            applied.0 == PlaygroundMaterialRole::Translucent && *no_shadow_caster
        })
        .count() as u32;
    stats.extended_material_bound_count = extended_materials.iter().count() as u32;
    stats.tiles_pbr_material_count = tiles_materials
        .iter()
        .filter(|(_, material)| {
            material.extension.settings.tiling_and_mode.y < 0.5
                && material.base.normal_map_texture.is_some()
                && material.base.occlusion_texture.is_some()
                && material.base.flip_normal_map_y
        })
        .count() as u32;
    stats.translucent_pbr_material_count = tiles_materials
        .iter()
        .filter(|(_, material)| {
            material.extension.settings.tiling_and_mode.y >= 0.5
                && matches!(material.base.alpha_mode, AlphaMode::Blend)
                && material.base.double_sided
                && material.base.cull_mode.is_none()
        })
        .count() as u32;
    stats.cube_orientation_total = 0;
    stats.cube_orientation_matching = 0;
    stats.cube_orientation_max_error = 0.0;
    stats.rock_scale_count = 0;
    stats.rock_scale_error = 0.0;
    stats.brush_count = 0;
    for (name, transform) in &cube_visuals {
        if let Some((pitch, yaw, roll)) = expected_cube_rotator(name.as_str()) {
            let source_rotation = ue_source_rotator_quat(pitch, yaw, roll);
            let expected = map_ue_direction(source_rotation.mul_vec3(Vec3::Y));
            let actual = transform.rotation.mul_vec3(Vec3::Z);
            let error = actual.distance(expected);
            stats.cube_orientation_total += 1;
            stats.cube_orientation_max_error = stats.cube_orientation_max_error.max(error);
            if error < 1e-4 {
                stats.cube_orientation_matching += 1;
            }
        }
        if name.as_str() == "SM_Als_Rock:S_Rock2" {
            stats.rock_scale_count += 1;
            stats.rock_scale_error = stats
                .rock_scale_error
                .max(transform.scale.distance(Vec3::new(1.5, 1.75, 1.5)));
        }
        if name.as_str().contains("Brush_0") {
            stats.brush_count += 1;
        }
    }
}

fn landscape_material_stats_collector(
    mut stats: ResMut<SceneStats>,
    landscape_materials: Query<
        &MeshMaterial3d<PlaygroundTilesMaterial>,
        With<PlaygroundLandscapeTilesMaterial>,
    >,
    tiles_materials: Res<Assets<PlaygroundTilesMaterial>>,
    images: Res<Assets<Image>>,
) {
    stats.landscape_tiles_material_count = 0;
    stats.landscape_tiles_tiling_error = 0.0;
    stats.landscape_tiles_color_error = 0.0;
    stats.landscape_tiles_mip_count = 0;
    for handle in &landscape_materials {
        let Some(material) = tiles_materials.get(&handle.0) else {
            continue;
        };
        let settings = &material.extension.settings;
        stats.landscape_tiles_material_count += 1;
        stats.landscape_tiles_tiling_error = stats
            .landscape_tiles_tiling_error
            .max((settings.tiling_and_mode.x - LANDSCAPE_TILES_TILING).abs());
        let color_error = [
            settings.side_color_a.distance(LANDSCAPE_TILES_COLOR),
            settings.top_color_a.distance(LANDSCAPE_TILES_COLOR),
            settings.side_color_b.distance(LANDSCAPE_TILES_COLOR),
            settings.top_color_b.distance(LANDSCAPE_TILES_COLOR),
        ]
        .into_iter()
        .fold(0.0, f32::max);
        stats.landscape_tiles_color_error = stats.landscape_tiles_color_error.max(color_error);
        for image_handle in [
            Some(&material.extension.grid_texture),
            material.base.normal_map_texture.as_ref(),
            material.base.occlusion_texture.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if images
                .get(image_handle)
                .is_some_and(|image| image.texture_descriptor.mip_level_count > 1)
            {
                stats.landscape_tiles_mip_count += 1;
            }
        }
    }
}

fn kinematic_probe(
    time: Res<Time>,
    mut stats: ResMut<SceneStats>,
    movings: Query<(&Transform, &PlaygroundKinematic)>,
) {
    if stats.mover_prev.len() != movings.iter().count() {
        stats.mover_prev = movings.iter().map(|(tf, _)| tf.translation).collect();
        return;
    }
    let mut moving = 0u32;
    for (i, (tf, kin)) in movings.iter().enumerate() {
        match kin.kind {
            PlaygroundKinematicKind::MovePingPong { .. } => {
                if stats.mover_prev[i].distance(tf.translation) > 0.005 {
                    moving += 1;
                }
            }
            PlaygroundKinematicKind::SpinYawDegPerSec { speed_deg } => {
                let dt = time.delta_secs();
                stats.spin_yaw_abs_deg += (speed_deg * dt).abs();
                if stats.spin_yaw_abs_deg > 1.0 {
                    moving += 1;
                }
            }
        }
    }
    stats.movers_moving = moving.max(stats.movers_moving);
    stats.movers_total = movings.iter().count() as u32;
    for (i, (tf, _)) in movings.iter().enumerate() {
        stats.mover_prev[i] = tf.translation;
    }
}

fn screenshot_runner(mut commands: Commands, mut schedule: ResMut<Schedule>) {
    if !schedule.shot_done && Instant::now() >= schedule.shot_at {
        let path = schedule.shot_path.clone();
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        info!("[SCENEGATE] screenshot requested");
        schedule.shot_done = true;
    }
}

fn auto_quit(schedule: Res<Schedule>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= schedule.quit_at {
        warn!("[SCENEGATE] wall-clock safety quit");
        exit.write(AppExit::Success);
    }
}

fn verdict_system(
    time: Res<Time>,
    mut stats: ResMut<SceneStats>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs_f64();
    if t < 10.0 || stats.verdict_done {
        return;
    }
    stats.verdict_done = true;

    let ok_static = stats.static_count >= 140;
    let ok_text = stats.text_count >= 22;
    let ok_native_text = stats.native_text_count == 0;
    let ok_glyph_text = stats.glyph_text_count == stats.text_count;
    let ok_world_text = stats.world_text_mesh_count >= stats.text_count
        && stats.world_text_masked_count == stats.world_text_mesh_count;
    let ok_text_fit = stats.text_fitted_count == stats.text_count
        && stats.text_fit_max_error < 1e-4
        && stats.text_center_aligned_count == 17
        && stats.text_left_top_aligned_count == 5;
    let ok_text_front =
        stats.text_front_matching_count == stats.text_count && stats.text_front_max_error < 1e-4;
    let ok_landscape = stats.landscape_seen
        && stats.landscape_identity
        && stats.landscape_tiles_material_count >= 4
        && stats.landscape_tiles_tiling_error < 1e-4
        && stats.landscape_tiles_color_error < 1e-4
        && stats.landscape_tiles_mip_count == stats.landscape_tiles_material_count * 3;
    let ok_sun = stats.sun_count == 1
        && stats.sun_shadowed
        && stats.sun_ray_error < 1e-4
        && stats.sun_illuminance_error < 1e-4;
    let ok_camera_exposure = stats.camera_exposure_count == 1
        && stats.camera_exposure_error < 1e-4
        && stats.camera_contrast_error < 1e-4
        && stats.camera_aces_count == 1;
    let ok_sky = stats.sky_count >= 2;
    let ok_materials = stats.main_material_count >= 10
        && stats.tiles_material_count >= 100
        && stats.translucent_material_count == 1
        && stats.translucent_no_shadow_count == stats.translucent_material_count
        && stats.extended_material_bound_count >= 100
        && stats.tiles_pbr_material_count >= 1
        && stats.translucent_pbr_material_count >= 1;
    let ok_moving = stats.movers_total >= 3 && stats.movers_moving >= 2;
    let ok_spin = stats.spin_yaw_abs_deg > 30.0;
    let ok_cube_orientation = stats.cube_orientation_total == 5
        && stats.cube_orientation_matching == stats.cube_orientation_total
        && stats.cube_orientation_max_error < 1e-4;
    let ok_rock_scale = stats.rock_scale_count == 1 && stats.rock_scale_error < 1e-4;
    let ok_brush = stats.brush_count == 0;

    info!(
        "[SCENEGATE][VERDICT] static={}(>=140) text={}(>=22) native_text={} glyph_text={}/{} world_mesh_text={} masked_world_mesh_text={} text_fit={}/{} fit_error={:.6} center={} left_top={} text_front={}/{} front_error={:.6} landscape={} identity={} tiles={}/4 tile_tiling_error={:.6} tile_color_error={:.6} tile_mips={}/{} sun={}/shadow={} lux_error={:.6} ray_error={:.6} camera_display={}/1 ev_error={:.6} contrast_error={:.6} aces={} sky={} materials=main={} tiles={} translucent={} translucent_no_shadow={} extended={} tile_pbr={} translucent_pbr={} movers={}(>=3) movers_active={} spin_yaw_abs={:.1}deg cube_orientation={}/{} cube_axis_error={:.6} rock_scale={}/1 rock_scale_error={:.6} brush={} -> {}",
        stats.static_count,
        stats.text_count,
        stats.native_text_count,
        stats.glyph_text_count,
        stats.text_count,
        stats.world_text_mesh_count,
        stats.world_text_masked_count,
        stats.text_fitted_count,
        stats.text_count,
        stats.text_fit_max_error,
        stats.text_center_aligned_count,
        stats.text_left_top_aligned_count,
        stats.text_front_matching_count,
        stats.text_count,
        stats.text_front_max_error,
        stats.landscape_seen,
        stats.landscape_identity,
        stats.landscape_tiles_material_count,
        stats.landscape_tiles_tiling_error,
        stats.landscape_tiles_color_error,
        stats.landscape_tiles_mip_count,
        stats.landscape_tiles_material_count * 3,
        stats.sun_count,
        stats.sun_shadowed,
        stats.sun_illuminance_error,
        stats.sun_ray_error,
        stats.camera_exposure_count,
        stats.camera_exposure_error,
        stats.camera_contrast_error,
        stats.camera_aces_count,
        stats.sky_count,
        stats.main_material_count,
        stats.tiles_material_count,
        stats.translucent_material_count,
        stats.translucent_no_shadow_count,
        stats.extended_material_bound_count,
        stats.tiles_pbr_material_count,
        stats.translucent_pbr_material_count,
        stats.movers_total,
        stats.movers_moving,
        stats.spin_yaw_abs_deg,
        stats.cube_orientation_matching,
        stats.cube_orientation_total,
        stats.cube_orientation_max_error,
        stats.rock_scale_count,
        stats.rock_scale_error,
        stats.brush_count,
        if ok_static
            && ok_text
            && ok_native_text
            && ok_glyph_text
            && ok_world_text
            && ok_text_fit
            && ok_text_front
            && ok_landscape
            && ok_sun
            && ok_camera_exposure
            && ok_sky
            && ok_materials
            && ok_moving
            && ok_spin
            && ok_cube_orientation
            && ok_rock_scale
            && ok_brush
        {
            "PASS"
        } else {
            "FAIL"
        }
    );
    if !(ok_static
        && ok_text
        && ok_native_text
        && ok_glyph_text
        && ok_world_text
        && ok_text_fit
        && ok_text_front
        && ok_landscape
        && ok_sun
        && ok_camera_exposure
        && ok_sky
        && ok_materials
        && ok_moving
        && ok_spin
        && ok_cube_orientation
        && ok_rock_scale
        && ok_brush)
    {
        exit.write(AppExit::Error(std::num::NonZeroU8::new(7).unwrap()));
    } else {
        exit.write(AppExit::Success);
    }
}
