use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_core::character::{AlsCharacter, AlsCharacterPlugin};
use als_core::control_scheme::AlsControlScheme;
use als_core::debug_hud::AlsStateHudPlugin;
use als_core::input::AlsInputPlugin;
use als_core::motion::AlsMotionPlugin;
use als_core::player_start::AlsPlayerStartReady;
use als_core::state::{Gait, LocomotionState, RotationMode, Stance};
use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin};
use als_ext::{AlsExtPlugin, PlaygroundMovingObject, PlaygroundPlugin, PlaygroundStatic};
use bevy_als::game_mode::{
    AlsGameModePlugin, AlsPlayerAdmission, AlsScenePhysicsReady, request_als_player,
};

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
        AlsGameModePlugin,
        PlaygroundPlugin {
            manifest: "assets/models/playground/playground.ron".into(),
        },
        AnimationGraphPlugin::default(),
        AlsExtPlugin,
    ))
    .init_resource::<SpawnOnce>()
    .init_resource::<AlsScenePhysicsReady>()
    .add_systems(
        Update,
        (
            scene_physics_ready,
            keyboard_driver,
            position_report,
            camera_follow,
            fall_guard,
            auto_quit_system,
            spawn_character_once.after(scene_physics_ready),
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

fn scene_physics_ready(
    mut ready: ResMut<AlsScenePhysicsReady>,
    starts: Query<&AlsPlayerStartReady>,
    statics: Query<Entity, With<PlaygroundStatic>>,
    mut stable_frames: Local<u32>,
) {
    if ready.ready {
        return;
    }
    let total = statics.iter().count();
    if starts.single().is_err() || total == 0 {
        *stable_frames = 0;
        return;
    }
    *stable_frames += 1;
    if *stable_frames >= 2 {
        ready.ready = true;
        info!("[PG][BARRIER] starts=1 static_roots={total} -> PASS");
    }
}

fn spawn_character_once(mut commands: Commands, mut once: ResMut<SpawnOnce>) {
    if once.0 {
        return;
    }
    once.0 = true;
    request_als_player(&mut commands, AlsPlayerAdmission::new());
    info!("[PG] player admission requested");

    commands.spawn((
        FollowCamera,
        Camera3d::default(),
        Transform::from_xyz(4.0, 3.5, 6.0),
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
