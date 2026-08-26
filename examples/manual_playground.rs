use std::path::PathBuf;
use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::params::AlsMovementParams;
use als_ext::playground::{PlaygroundLandscape, terrain_height};
use als_ext::{PlaygroundBoxCollider, PlaygroundPlugin, PlaygroundSpawnPoint};
use bevy_als::anim_probe::AnimProbePlugin;
use bevy_als::debug_draw::AlsDebugDrawPlugin;
use bevy_als::playable_character::{
    AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};
use bevy_als::show_debug_hud::AlsShowDebugHudPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        AlsPlayableCharacterPlugin,
    ));
    app.add_plugins((
        AnimProbePlugin,
        AlsDebugDrawPlugin,
        AlsShowDebugHudPlugin,
        PlaygroundPlugin {
            manifest: "assets/models/playground/playground.ron".into(),
        },
    ))
    .init_resource::<SpawnOnce>()
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .add_systems(Startup, (spawn_sun, arm_auto_quit))
    .add_systems(
        Update,
        (
            spawn_character_once,
            playground_physics_bridge,
            landscape_collider_bridge,
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
    landscapes: Query<Entity, (Added<PlaygroundLandscape>, Without<LandscapeColliderBuilt>)>,
) {
    for entity in &landscapes {
        let size = 120.0_f32;
        let segs = 64_u32;
        let step = size / segs as f32;
        let half = size * 0.5;
        let mut verts = Vec::with_capacity(((segs + 1) * (segs + 1)) as usize);
        for zi in 0..=segs {
            for xi in 0..=segs {
                let lx = xi as f32 * step - half;
                let lz = zi as f32 * step - half;
                verts.push(Vec3::new(lx, terrain_height(lx, lz), lz));
            }
        }
        let mut indices = Vec::with_capacity((segs * segs * 6) as usize);
        let w = segs + 1;
        for zi in 0..segs {
            for xi in 0..segs {
                let a = zi * w + xi;
                indices.push([a, a + w, a + 1]);
                indices.push([a + 1, a + w, a + w + 1]);
            }
        }
        commands
            .entity(entity)
            .insert((Collider::trimesh(verts, indices), LandscapeColliderBuilt));
        info!("[MANUAL] landscape trimesh collider built");
    }
}

fn spawn_sun(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 30_000.0,
            ..default()
        },
        Transform::from_xyz(5.0, 20.0, -3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
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

    let mut blueprint = AlsCharacterBlueprint::new(
        Transform::from_translation(point.position).with_rotation(point.rotation),
    );
    blueprint.spawn_camera = true;
    let _character = spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        blueprint,
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
