use std::path::PathBuf;
use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_camera::AlsCameraPlugin;
use als_core::aim::AlsAimPlugin;
use als_core::air::AlsAirState;
use als_core::character::{AlsCharacter, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::foot_ik::AlsFootProbePlugin;
use als_core::input::{AlsInputCommand, AlsInputPlugin};
use als_core::input_request::AlsInputRequestPlugin;
use als_core::motion::AlsMotionPlugin;
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, Stance};
use als_core::transition::AlsStateTransitionPlugin;
use als_core::view::{AlsViewPitch, clamp_pitch};
use als_ext::overlay_materials::OverlayMaterialsPlugin;
use als_ext::playground::PlaygroundLandscape;
use als_ext::playground_materials::PlaygroundMaterialsPlugin;
use als_ext::{AlsExtPlugin, PlaygroundBoxCollider, PlaygroundPlugin, PlaygroundSpawnPoint};
use bevy_als::debug_draw::AlsDebugDrawPlugin;
use bevy_als::foot_ik_apply::FootIkApplyPlugin;
use bevy_als::mantle_drive::AlsMantleDrivePlugin;
use bevy_als::motion_graph_link::{
    compute_motion_graph_inputs, write_air_pins, write_motion_graph_pins,
};
use bevy_als::overlay_layer::OverlayLayerPlugin;
use bevy_als::ragdoll_link::AlsRagdollLinkPlugin;
use bevy_als::roll_drive::AlsRollDrivePlugin;
use bevy_als::show_debug_hud::AlsShowDebugHudPlugin;
use bevy_als::stop_link::AlsStopLinkPlugin;
use bevy_als::view_layer::ViewLayerPlugin;

const LOOK_RADIANS_PER_COUNT: f32 = 0.0025;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        PhysicsPlugins::new(FixedPostUpdate),
        TnuaAvian3dPlugin::new(FixedUpdate),
        TnuaControllerPlugin::<AlsControlScheme>::new(FixedUpdate),
        EnhancedInputPlugin,
        AlsInputPlugin,
        AlsInputRequestPlugin,
        AlsAimPlugin,
        AlsStateTransitionPlugin,
        AlsCharacterPlugin,
        AlsMotionPlugin,
        als_core::air::AlsAirPlugin,
    ));
    app.add_plugins((
        AlsFootProbePlugin,
        FootIkApplyPlugin,
        OverlayLayerPlugin,
        ViewLayerPlugin,
        AlsRollDrivePlugin,
        AlsStopLinkPlugin,
        AlsMantleDrivePlugin,
        AlsRagdollLinkPlugin,
        AlsDebugDrawPlugin,
        AlsShowDebugHudPlugin,
    ));
    app.add_plugins((
        bevy_als::als_hud::AlsHudPlugin,
        bevy_als::overlay_mode_menu::OverlayModeMenuPlugin,
        PlaygroundPlugin {
            manifest: "assets/models/playground/playground_scene.ron".into(),
        },
        AnimationGraphPlugin::default(),
        PlaygroundMaterialsPlugin,
        OverlayMaterialsPlugin,
        AlsExtPlugin,
        AlsCameraPlugin,
    ))
    .init_resource::<SpawnOnce>()
    .insert_resource(GlobalAmbientLight {
        brightness: 2.0,
        ..default()
    })
    .add_systems(Startup, arm_auto_quit)
    .add_systems(
        Update,
        (
            spawn_character_once,
            playground_physics_bridge,
            landscape_collider_bridge,
            graph_router,
            look_pitch_integrator,
            auto_quit,
            screenshot_runner,
        ),
    );

    app.run();
}

#[derive(Resource, Default)]
struct SpawnOnce(bool);

#[derive(Resource)]
struct AutoQuit(Instant);

#[derive(Resource)]
struct ShotPlan {
    at: Instant,
    path: PathBuf,
    taken: bool,
    done_at: Option<Instant>,
}

fn arm_auto_quit(mut commands: Commands) {
    let Ok(secs) = std::env::var("ALS_BOOT_AUTO_QUIT_SECS") else {
        return;
    };
    let Ok(secs) = secs.parse::<f64>() else {
        return;
    };
    commands.insert_resource(AutoQuit(Instant::now() + Duration::from_secs_f64(secs)));
    info!("[MANUAL] auto quit in {secs}s");

    if let Ok(path) = std::env::var("ALS_SHOT_PATH") {
        let after = std::env::var("ALS_SHOT_AFTER_SECS")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(4.0);
        commands.insert_resource(ShotPlan {
            at: Instant::now() + Duration::from_secs_f64(after),
            path: PathBuf::from(&path),
            taken: false,
            done_at: None,
        });
        info!("[MANUAL] screenshot scheduled +{after}s -> {path}");
    }
}

fn auto_quit(timer: Option<Res<AutoQuit>>, mut exit: MessageWriter<AppExit>) {
    let Some(t) = timer else {
        return;
    };
    if Instant::now() >= t.0 {
        info!("[MANUAL] auto quit reached");
        exit.write(AppExit::Success);
    }
}

fn screenshot_runner(
    plan: Option<ResMut<ShotPlan>>,
    mut commands: Commands,
    cameras: Query<Entity, With<Camera3d>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut plan) = plan else {
        return;
    };
    if !plan.taken && Instant::now() >= plan.at {
        let Ok(cam) = cameras.single() else {
            return;
        };
        let path = plan.path.clone();
        commands.entity(cam).insert(Screenshot::primary_window());
        commands.entity(cam).observe(save_to_disk(path.clone()));
        info!("[MANUAL] screenshot requested -> {}", path.display());
        plan.taken = true;
        plan.done_at = Some(Instant::now() + Duration::from_millis(1500));
    }
    if let Some(done) = plan.done_at
        && Instant::now() >= done
    {
        info!("[MANUAL] screenshot flow done, exiting");
        exit.write(AppExit::Success);
    }
}

#[derive(Component)]
struct LandscapeColliderBuilt;

fn landscape_collider_bridge(
    mut commands: Commands,
    landscapes: Query<
        (Entity, &GlobalTransform),
        (With<PlaygroundLandscape>, Without<LandscapeColliderBuilt>),
    >,
    scene_children: Query<&Children>,
    mesh_parts: Query<(&Mesh3d, &GlobalTransform)>,
    meshes: Res<Assets<Mesh>>,
) {
    fn append_meshes(
        entity: Entity,
        root_inverse: Mat4,
        scene_children: &Query<&Children>,
        mesh_parts: &Query<(&Mesh3d, &GlobalTransform)>,
        meshes: &Assets<Mesh>,
        vertices: &mut Vec<Vec3>,
        triangles: &mut Vec<[u32; 3]>,
    ) {
        if let Ok((mesh_handle, transform)) = mesh_parts.get(entity)
            && let Some(mesh) = meshes.get(&mesh_handle.0)
            && let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            && let Some(values) = positions.as_float3()
            && let Some(indices) = mesh.indices()
        {
            let base = vertices.len() as u32;
            let relative = root_inverse * transform.to_matrix();
            vertices.extend(
                values
                    .iter()
                    .map(|position| relative.transform_point3(Vec3::from(*position))),
            );
            match indices {
                bevy::render::mesh::Indices::U32(list) => {
                    triangles.extend(list.chunks_exact(3).map(|triangle| {
                        [base + triangle[0], base + triangle[1], base + triangle[2]]
                    }))
                }
                bevy::render::mesh::Indices::U16(list) => {
                    triangles.extend(list.chunks_exact(3).map(|triangle| {
                        [
                            base + triangle[0] as u32,
                            base + triangle[1] as u32,
                            base + triangle[2] as u32,
                        ]
                    }))
                }
            }
        }
        if let Ok(children) = scene_children.get(entity) {
            for child in children.iter() {
                append_meshes(
                    child,
                    root_inverse,
                    scene_children,
                    mesh_parts,
                    meshes,
                    vertices,
                    triangles,
                );
            }
        }
    }

    for (root, root_transform) in &landscapes {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        append_meshes(
            root,
            root_transform.to_matrix().inverse(),
            &scene_children,
            &mesh_parts,
            &meshes,
            &mut vertices,
            &mut triangles,
        );
        if vertices.is_empty() || triangles.is_empty() {
            continue;
        }
        info!(
            "[MANUAL] landscape collider built verts={} triangles={}",
            vertices.len(),
            triangles.len()
        );
        commands.entity(root).insert((
            RigidBody::Static,
            Collider::trimesh(vertices, triangles),
            LandscapeColliderBuilt,
        ));
    }
}

fn spawn_character_once(
    mut commands: Commands,
    mut once: ResMut<SpawnOnce>,
    spawn_point: Option<Res<PlaygroundSpawnPoint>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    assets: Res<AssetServer>,
) {
    if once.0 || spawn_point.is_none() {
        return;
    }
    once.0 = true;
    let point = *spawn_point.unwrap();
    info!(
        "[MANUAL] character at ({:.2},{:.2},{:.2})",
        point.position.x, point.position.y, point.position.z
    );

    let _character = bevy_als::character_assembly::spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        bevy_als::character_assembly::AlsCharacterBlueprint {
            spawn_camera: true,
            ..bevy_als::character_assembly::AlsCharacterBlueprint::new(
                Transform::from_translation(point.position).with_rotation(point.rotation),
                "animated_scenes/als_ragdoll.animscn.ron",
            )
        },
    );
}

fn playground_physics_bridge(
    mut commands: Commands,
    bodies: Query<(Entity, &PlaygroundBoxCollider), Added<PlaygroundBoxCollider>>,
) {
    for (entity, collider) in &bodies {
        commands.entity(entity).insert((
            RigidBody::Static,
            Collider::cuboid(
                collider.half_extents.x * 2.0,
                collider.half_extents.y * 2.0,
                collider.half_extents.z * 2.0,
            ),
        ));
    }
}

#[allow(clippy::type_complexity)]
fn graph_router(
    q: Query<
        (
            &Stance,
            &Gait,
            &LinearVelocity,
            &als_core::character::AlsMovementParamsHandle,
            Option<&AlsAirState>,
        ),
        With<AlsCharacter>,
    >,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
    params: Res<Assets<AlsMovementParams>>,
) {
    let Ok((stance, gait, vel, handle, air)) = q.single() else {
        return;
    };
    let Some(p) = params.get(handle.id()) else {
        return;
    };
    let hspeed = Vec2::new(vel.x, vel.z).length();
    let mgi = compute_motion_graph_inputs(p, *stance, *gait, hspeed);
    let air_state = air.copied().unwrap_or(AlsAirState::default());
    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            write_motion_graph_pins(&mut player, &mgi);
            write_air_pins(&mut player, &air_state);
        }
    }
}

fn look_pitch_integrator(mut q: Query<(&AlsInputCommand, &mut AlsViewPitch), With<AlsCharacter>>) {
    for (cmd, mut pitch) in &mut q {
        pitch.0 = clamp_pitch(pitch.0 - cmd.look_delta.y * LOOK_RADIANS_PER_COUNT);
    }
}
