use avian3d::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_core::character::{AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::debug_hud::AlsStateHudPlugin;
use als_core::input::{spawn_als_actions, AlsInputCommand, AlsInputMap, AlsInputPlugin};
use als_core::motion::{gait_max_speed, AlsInputOverride, AlsMotionPlugin};
use als_core::character::AlsMovementParamsHandle;
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, RotationMode, Stance};
use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin};

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
    ))
    .init_resource::<DemoPhase>()
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            keyboard_driver,
            auto_demo,
            camera_follow,
            speed_report,
            safety_quit,
        ),
    );

    app.run();
}

#[derive(Resource, Default)]
struct DemoPhase {
    index: usize,
}

const PHASE_SECONDS: f64 = 2.5;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    let ground_mat = materials.add(Color::srgb(0.25, 0.28, 0.25));
    let prop_mat = materials.add(Color::srgb(0.45, 0.4, 0.35));

    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(60.0, 0.2, 60.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
        Mesh3d(meshes.add(Mesh::from(Rectangle::new(60.0, 60.0)))),
        MeshMaterial3d(ground_mat.clone()),
    ));

    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(6.0, 0.5, 8.0),
        Transform::from_xyz(10.0, 1.05, 0.0)
            .with_rotation(Quat::from_rotation_z(-12f32.to_radians())),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(6.0, 0.5, 8.0)))),
        MeshMaterial3d(prop_mat.clone()),
    ));

    for i in 0..4u32 {
        let h = 0.35 * (i + 1) as f32;
        commands.spawn((
            RigidBody::Static,
            Collider::cuboid(1.2, h, 3.0),
            Transform::from_xyz(-9.0 - i as f32 * 1.2, h / 2.0, 0.0),
            Mesh3d(meshes.add(Mesh::from(Cuboid::new(1.2, h, 3.0)))),
            MeshMaterial3d(prop_mat.clone()),
        ));
    }

    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(6.0, 0.3, 3.0),
        Transform::from_xyz(0.0, 1.1, -8.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(6.0, 0.3, 3.0)))),
        MeshMaterial3d(prop_mat.clone()),
    ));
    for side in [-3.0f32, 3.0] {
        commands.spawn((
            RigidBody::Static,
            Collider::cuboid(0.3, 2.4, 3.0),
            Transform::from_xyz(side, 1.2, -8.0),
            Mesh3d(meshes.add(Mesh::from(Cuboid::new(0.3, 2.4, 3.0)))),
            MeshMaterial3d(prop_mat.clone()),
        ));
    }

    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_xyz(0.0, 1.0, 0.0);
    let character = commands.spawn(bundle).id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());

    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() {
        commands.entity(character).insert(AlsInputOverride::default());
        info!("[M3GATE] scripted demo armed");
    }

    commands.spawn((Camera3d::default(), Transform::from_xyz(3.0, 2.4, 5.0)));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn keyboard_driver(
    keys: Res<ButtonInput<KeyCode>>,
    mut q: Query<
        (
            &AlsInputCommand,
            &Gait,
            &RotationMode,
            &Stance,
            &mut AlsStateRequest,
        ),
        With<AlsCharacter>,
    >,
) {
    let Ok((cmd, gait, rotation_mode, stance_current, mut request)) = q.single_mut() else {
        return;
    };
    if cmd.crouch_pressed {
        let target = match stance_current {
            Stance::Standing => Stance::Crouching,
            Stance::Crouching => Stance::Standing,
        };
        info!("[M3GATE] crouch edge -> stance {target:?}");
        request.stance = Some(target);
    }
    if keys.just_pressed(KeyCode::KeyG) {
        let next = match gait {
            Gait::Walking => Gait::Running,
            Gait::Running => Gait::Sprinting,
            Gait::Sprinting => Gait::Walking,
        };
        info!("[M3GATE] G pressed -> gait {next:?}");
        request.gait = Some(next);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        let next = match rotation_mode {
            RotationMode::VelocityDirection => RotationMode::ViewDirection,
            RotationMode::ViewDirection => RotationMode::Aiming,
            RotationMode::Aiming => RotationMode::VelocityDirection,
        };
        info!("[M3GATE] R pressed -> rotation {next:?}");
        request.rotation_mode = Some(next);
    }
}

fn phase_desc(phase: usize) -> &'static str {
    match phase {
        0 => "idle warmup",
        1 => "walk forward",
        2 => "run forward",
        3 => "sprint forward",
        4 => "idle brief",
        5 => "run + jump",
        6 => "crouch walk",
        7 => "stand up",
        _ => "done",
    }
}

fn auto_demo(
    time: Res<Time>,
    mut phase_res: ResMut<DemoPhase>,
    mut q: Query<
        (
            Entity,
            &mut AlsInputOverride,
            &mut AlsStateRequest,
        ),
        With<AlsCharacter>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_err() {
        return;
    }
    let Ok((entity, mut over, mut req)) = q.single_mut() else {
        return;
    };
    let t = time.elapsed_secs_f64();
    let idx = if t < 2.0 {
        0
    } else {
        ((t - 2.0) / PHASE_SECONDS) as usize + 1
    };
    if idx == phase_res.index && idx <= 8 {
        apply_phase(idx, entity, &mut over, &mut req, false);
        return;
    }
    phase_res.index = idx.min(9);
    let entered = true;
    info!(
        "[M3GATE][PHASE {}] {} (t={t:.2})",
        phase_res.index,
        phase_desc(phase_res.index)
    );
    apply_phase(phase_res.index, entity, &mut over, &mut req, entered);
    if phase_res.index >= 9 {
        info!("[M3GATE] final states dumped by HUD; done");
        exit.write(AppExit::Success);
    }
}

fn apply_phase(
    phase: usize,
    entity: Entity,
    over: &mut AlsInputOverride,
    req: &mut AlsStateRequest,
    entered: bool,
) {
    let (dir, gait, stance) = match phase {
        1 => (Vec2::Y, Some(Gait::Walking), None),
        2 => (Vec2::Y, Some(Gait::Running), None),
        3 => (Vec2::Y, Some(Gait::Sprinting), None),
        4 | 7 => (Vec2::ZERO, None, None),
        5 => (Vec2::Y, Some(Gait::Running), None),
        6 => (Vec2::Y, None, Some(Stance::Crouching)),
        _ => (Vec2::ZERO, None, None),
    };
    over.0.move_dir = dir;
    if let Some(g) = gait {
        req.gait = Some(g);
    }
    if let Some(s) = stance {
        req.stance = Some(s);
    }
    over.0.jump_pressed = entered && phase == 5;
    if entered && matches!(phase, 1..=7) {
        info!("[M3GATE] entity={entity:?} override armed for phase {phase}");
    }
}

fn speed_report(
    time: Res<Time>,
    mut last: Local<f32>,
    q: Query<
        (
            &Transform,
            &Gait,
            &Stance,
            &LocomotionState,
            &LinearVelocity,
            &AlsMovementParamsHandle,
        ),
        With<AlsCharacter>,
    >,
    params_assets: Res<Assets<AlsMovementParams>>,
) {
    if time.elapsed_secs() - *last < 0.5 {
        return;
    }
    *last = time.elapsed_secs();
    let Ok((char_t, gait, stance, loco, vel, handle)) = q.single() else {
        return;
    };
    let actual = Vec2::new(vel.x, vel.z).length();
    warn!(
        "[M3GATE][DIAG] pos_y={:.3} vy={:.3} vxz={actual:.3}",
        char_t.translation.y, vel.y
    );
    let target = params_assets
        .get(handle.id())
        .map(|p| gait_max_speed(p, stance, gait))
        .unwrap_or(f32::NAN);
    info!(
        "[M3GATE] t={:.1} gait={gait:?} stance={stance:?} loco={loco:?} target={target:.2} actual={actual:.2}",
        time.elapsed_secs()
    );
}

fn camera_follow(
    mut cameras: Query<&mut Transform, (With<Camera3d>, Without<AlsCharacter>)>,
    characters: Query<&Transform, (With<AlsCharacter>, Without<Camera3d>)>,
) {
    let Ok(char_t) = characters.single() else {
        return;
    };
    for mut cam in cameras.iter_mut() {
        cam.translation = char_t.translation + Vec3::new(2.5, 2.2, 4.0);
        cam.look_at(char_t.translation + Vec3::Y * 0.9, Vec3::Y);
    }
}

fn safety_quit(time: Res<Time>, mut exit: MessageWriter<AppExit>) {
    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() && time.elapsed_secs_f64() >= 20.0 {
        warn!("[M3GATE] safety quit");
        exit.write(AppExit::Success);
    }
}
