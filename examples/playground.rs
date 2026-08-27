use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_core::character::{AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::debug_hud::AlsStateHudPlugin;
use als_core::input::{AlsInputMap, AlsInputPlugin, spawn_als_actions};
use als_core::motion::AlsMotionPlugin;
use als_core::params::AlsMovementParams;
use als_core::player_start::AlsPlayerStartReady;
use als_core::state::{Gait, LocomotionState, RotationMode, Stance};
use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin};
use als_ext::{PlaygroundMovingObject, PlaygroundPlugin};

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
        AlsStateTransitionPlugin,
        AlsCharacterPlugin,
        AlsMotionPlugin,
        AlsStateHudPlugin,
        PlaygroundPlugin {
            manifest: "assets/models/playground/playground.ron".into(),
        },
    ))
    .init_resource::<SpawnOnce>()
    .add_systems(Update, spawn_character_once)
    .add_systems(
        Update,
        (
            keyboard_driver,
            position_report,
            camera_follow,
            fall_guard,
            auto_quit_system,
        ),
    );

    app.insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    });

    if let Ok(secs) = std::env::var("ALS_BOOT_AUTO_QUIT_SECS") {
        let secs: f64 = secs.parse().unwrap_or(8.0);
        info!("[PG] auto quit armed in {secs:.1}s");
        app.insert_resource(AutoQuit(Some(
            Instant::now() + Duration::from_secs_f64(secs),
        )));
    }

    app.run();
}

#[derive(Resource, Default)]
struct SpawnOnce(bool);

#[derive(Resource)]
struct AutoQuit(Option<Instant>);

fn spawn_character_once(
    mut commands: Commands,
    mut once: ResMut<SpawnOnce>,
    player_starts: Query<&AlsPlayerStartReady>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    if once.0 {
        return;
    }
    let Ok(point) = player_starts.single() else {
        return;
    };
    let point = point.transform;
    once.0 = true;
    info!(
        "[PG] spawning character at ({:.2},{:.2},{:.2})",
        point.translation.x, point.translation.y, point.translation.z
    );

    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = point;
    let character = commands.spawn(bundle).id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());

    commands.spawn((
        FollowCamera,
        Camera3d::default(),
        Transform::from_translation(point.translation + Vec3::new(4.0, 3.5, 6.0)),
    ));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(40.0, 80.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

#[derive(Component)]
struct FollowCamera;

fn keyboard_driver(
    keys: Res<ButtonInput<KeyCode>>,
    mut q: Query<(&mut AlsStateRequest, &Gait, &RotationMode, &Stance), With<AlsCharacter>>,
) {
    let Ok((mut request, gait, rotation_mode, stance_current)) = q.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::KeyC) {
        let target = match stance_current {
            Stance::Standing => Stance::Crouching,
            Stance::Crouching => Stance::Standing,
        };
        info!("[PG] crouch edge -> stance {target:?}");
        request.stance = Some(target);
    }
    if keys.just_pressed(KeyCode::KeyG) {
        let next = match gait {
            Gait::Walking => Gait::Running,
            Gait::Running => Gait::Sprinting,
            Gait::Sprinting => Gait::Walking,
        };
        info!("[PG] G pressed -> gait {next:?}");
        request.gait = Some(next);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        let next = match rotation_mode {
            RotationMode::VelocityDirection => RotationMode::ViewDirection,
            RotationMode::ViewDirection => RotationMode::Aiming,
            RotationMode::Aiming => RotationMode::VelocityDirection,
        };
        info!("[PG] R pressed -> rotation {next:?}");
        request.rotation_mode = Some(next);
    }
}

fn position_report(
    time: Res<Time>,
    mut last: Local<f32>,
    q: Query<(&Transform, &LinearVelocity, &LocomotionState), With<AlsCharacter>>,
    moving: Query<Entity, With<PlaygroundMovingObject>>,
) {
    if time.elapsed_secs() - *last < 1.0 {
        return;
    }
    *last = time.elapsed_secs();
    let Ok((char_t, vel, loco)) = q.single() else {
        return;
    };
    info!(
        "[PG][DIAG] t={:.1} pos=({:.2},{:.2},{:.2}) vy={:.2} vxz={:.2} loco={loco:?} moving_hooks={}",
        time.elapsed_secs(),
        char_t.translation.x,
        char_t.translation.y,
        char_t.translation.z,
        vel.y,
        Vec2::new(vel.x, vel.z).length(),
        moving.iter().count(),
    );
}

fn camera_follow(
    mut cameras: Query<&mut Transform, (With<FollowCamera>, Without<AlsCharacter>)>,
    characters: Query<&Transform, With<AlsCharacter>>,
) {
    let Ok(char_t) = characters.single() else {
        return;
    };
    for mut cam in cameras.iter_mut() {
        cam.translation = char_t.translation + Vec3::new(4.0, 3.5, 6.0);
        cam.look_at(char_t.translation + Vec3::Y * 0.9, Vec3::Y);
    }
}

fn fall_guard(q: Query<&Transform, With<AlsCharacter>>, mut exit: MessageWriter<AppExit>) {
    let Ok(char_t) = q.single() else {
        return;
    };
    if char_t.translation.y < -20.0 {
        error!(
            "[PG] fell below floor y={:.2} -- aborting",
            char_t.translation.y
        );
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
    }
}

fn auto_quit_system(mut quit: Option<ResMut<AutoQuit>>, mut exit: MessageWriter<AppExit>) {
    let Some(aq) = quit.as_deref_mut() else {
        return;
    };
    let Some(deadline) = aq.0 else {
        return;
    };
    if Instant::now() >= deadline {
        info!("[PG] auto quit reached");
        exit.write(AppExit::Success);
    }
}
