use avian3d::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use als_core::character::{AlsCharacter, AlsMovementParamsHandle};
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::debug_hud::AlsStateHudPlugin;
use als_core::input::AlsInputCommand;
use als_core::motion::{AlsCapsuleCrouched, AlsInputOverride, effective_gait, gait_max_speed};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, RotationMode, Stance};
use als_core::transition::AlsStateRequest;

use bevy_als::playable_character::{
    AlsCharacterBlueprint, AlsPlayableCharacterPlugin, spawn_als_character,
};

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        AlsPlayableCharacterPlugin,
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

const PHASE_DURATIONS: [f64; 19] = [
    2.0, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 3.0, 3.0, 6.8, 2.4, 5.2, 1.2, 3.6, 1.5, 1.5, 2.4, 1.5,
];
const LAST_PHASE: usize = 18;
const VIEW_SPIN_DEG_PER_S: f32 = 250.0;
const AIM_SPIN_DEG_PER_S: f32 = 450.0;
const LOOK_UNITS_PER_RAD: f32 = 400.0;
const REPORT_INTERVAL: f32 = 0.25;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
    assets: Res<AssetServer>,
) {
    let ground_mat = materials.add(Color::srgb(0.25, 0.28, 0.25));
    let prop_mat = materials.add(Color::srgb(0.45, 0.4, 0.35));

    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(240.0, 0.2, 240.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
        Mesh3d(meshes.add(Mesh::from(Rectangle::new(240.0, 240.0)))),
        MeshMaterial3d(ground_mat.clone()),
    ));

    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(6.0, 0.5, 8.0),
        Transform::from_xyz(10.0, 0.66, 0.0)
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
        Transform::from_xyz(6.0, 1.1, -8.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(6.0, 0.3, 3.0)))),
        MeshMaterial3d(prop_mat.clone()),
    ));
    for side in [-3.0f32, 3.0] {
        commands.spawn((
            RigidBody::Static,
            Collider::cuboid(0.3, 2.4, 3.0),
            Transform::from_xyz(6.0 + side, 1.2, -8.0),
            Mesh3d(meshes.add(Mesh::from(Cuboid::new(0.3, 2.4, 3.0)))),
            MeshMaterial3d(prop_mat.clone()),
        ));
    }

    let character = spawn_als_character(
        &mut commands,
        &assets,
        &mut params_assets,
        &mut config_assets,
        AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 1.0, 0.0)),
    );

    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() {
        commands
            .entity(character)
            .insert(AlsInputOverride::default());
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

fn phase_start(index: usize) -> f64 {
    PHASE_DURATIONS[..index].iter().sum()
}

fn phase_at(t: f64) -> usize {
    let mut acc = 0.0;
    for (i, d) in PHASE_DURATIONS.iter().enumerate() {
        acc += d;
        if t < acc {
            return i;
        }
    }
    LAST_PHASE
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
        8 => "rotate in place (ViewDirection)",
        9 => "rotate in place (Aiming)",
        10 => "sprint toward ramp east base",
        11 => "climb ramp westward",
        12 => "sprint across stairs",
        13 => "reposition south lane",
        14 => "approach tunnel entrance",
        15 => "crouch into tunnel",
        16 => "stand attempt under slab (expect block)",
        17 => "exit tunnel and recover stand",
        _ => "done",
    }
}

fn waypoint(phase: usize) -> Option<(f32, f32)> {
    match phase {
        10 => Some((15.5, -2.0)),
        12 => Some((-19.0, 0.0)),
        13 => Some((-16.0, -6.0)),
        14 => Some((6.0, -5.5)),
        _ => None,
    }
}

fn auto_demo(
    time: Res<Time>,
    mut phase_res: ResMut<DemoPhase>,
    mut q: Query<
        (
            Entity,
            &Transform,
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
    let Ok((entity, char_t, mut over, mut req)) = q.single_mut() else {
        return;
    };
    let t = time.elapsed_secs_f64();
    let idx = phase_at(t);
    let entered = idx != phase_res.index;
    if !entered && idx < LAST_PHASE {
        apply_phase(
            idx,
            entity,
            char_t.translation,
            time.delta_secs_f64() as f32,
            &mut over,
            &mut req,
            false,
        );
        return;
    }
    phase_res.index = idx;
    info!(
        "[M3GATE][PHASE {}] {} (t={t:.2}, start={:.2})",
        idx,
        phase_desc(idx),
        phase_start(idx)
    );
    apply_phase(
        idx,
        entity,
        char_t.translation,
        time.delta_secs_f64() as f32,
        &mut over,
        &mut req,
        true,
    );
    if idx >= LAST_PHASE {
        info!("[M3GATE] demo complete; final states dumped by HUD");
        exit.write(AppExit::Success);
    }
}

fn apply_phase(
    phase: usize,
    entity: Entity,
    char_pos: Vec3,
    dt: f32,
    over: &mut AlsInputOverride,
    req: &mut AlsStateRequest,
    entered: bool,
) {
    let mut move_dir = Vec2::ZERO;
    let mut gait_req: Option<Gait> = None;
    let mut stance_req: Option<Stance> = None;
    let mut rotation_req: Option<RotationMode> = None;
    let mut look_rate_rad_s = 0.0f32;
    let mut jump_pressed = false;

    match phase {
        1 => {
            move_dir = Vec2::Y;
            gait_req = Some(Gait::Walking);
        }
        2 => {
            move_dir = Vec2::Y;
            gait_req = Some(Gait::Running);
        }
        3 => {
            move_dir = Vec2::Y;
            gait_req = Some(Gait::Sprinting);
        }
        4 => {}
        5 => {
            move_dir = Vec2::Y;
            gait_req = Some(Gait::Running);
            jump_pressed = entered;
        }
        6 => {
            move_dir = Vec2::Y;
            stance_req = Some(Stance::Crouching);
        }
        7 => {
            stance_req = Some(Stance::Standing);
        }
        8 => {
            rotation_req = Some(RotationMode::ViewDirection);
            look_rate_rad_s = VIEW_SPIN_DEG_PER_S.to_radians();
        }
        9 => {
            rotation_req = Some(RotationMode::Aiming);
            look_rate_rad_s = AIM_SPIN_DEG_PER_S.to_radians();
        }
        10 | 12 | 13 | 14 => {
            if let Some(target) = waypoint(phase) {
                move_dir = move_toward(char_pos, target);
            }
            gait_req = Some(Gait::Sprinting);
            rotation_req = Some(RotationMode::VelocityDirection);
        }
        11 => {
            move_dir = Vec2::NEG_X;
            gait_req = Some(Gait::Running);
        }
        15 => {
            move_dir = Vec2::Y;
            stance_req = Some(Stance::Crouching);
        }
        16 => {
            stance_req = Some(Stance::Standing);
            gait_req = Some(Gait::Walking);
        }
        17 => {
            move_dir = Vec2::Y;
        }
        _ => {}
    }

    over.0.move_dir = move_dir;
    over.0.look_delta = Vec2::new(look_rate_rad_s * dt * LOOK_UNITS_PER_RAD, 0.0);
    over.0.jump_pressed = jump_pressed;
    over.0.jump_held = false;
    if entered {
        if let Some(g) = gait_req {
            req.gait = Some(g);
        }
        if let Some(s) = stance_req {
            req.stance = Some(s);
        }
        if let Some(r) = rotation_req {
            req.rotation_mode = Some(r);
        }
        info!("[M3GATE] entity={entity:?} override armed for phase {phase}");
    }
}

fn move_toward(pos: Vec3, target: (f32, f32)) -> Vec2 {
    let dx = target.0 - pos.x;
    let dz = target.1 - pos.z;
    let len = (dx * dx + dz * dz).sqrt();
    if len < 1e-3 {
        return Vec2::ZERO;
    }
    Vec2::new(dx / len, -dz / len)
}

type ReportQueryItem<'a> = (
    &'a Transform,
    &'a Gait,
    &'a Stance,
    Has<AlsCapsuleCrouched>,
    &'a LocomotionState,
    &'a LinearVelocity,
    &'a AlsMovementParamsHandle,
);

fn speed_report(
    time: Res<Time>,
    mut last: Local<f32>,
    mut prev_yaw: Local<Option<(f64, f32)>>,
    mut exit: MessageWriter<AppExit>,
    q: Query<ReportQueryItem, With<AlsCharacter>>,
    params_assets: Res<Assets<AlsMovementParams>>,
) {
    let t = time.elapsed_secs();
    if t - *last < REPORT_INTERVAL {
        return;
    }
    *last = t;
    let Ok((char_t, gait, stance, capsule_crouched, loco, vel, handle)) = q.single() else {
        return;
    };
    let actual = Vec2::new(vel.x, vel.z).length();
    if char_t.translation.y < -15.0 {
        error!(
            "[M3GATE] fell off world at y={:.1} -- aborting",
            char_t.translation.y
        );
        exit.write(AppExit::Error(std::num::NonZeroU8::new(3).unwrap()));
    }
    let fwd = char_t.forward();
    let yaw_deg = f32::atan2(fwd.x, -fwd.z).to_degrees();
    let yaw_rate = match *prev_yaw {
        Some((pt, py)) => {
            let dt = (time.elapsed_secs_f64() - pt) as f32;
            let mut dy = yaw_deg - py;
            if dy > 180.0 {
                dy -= 360.0;
            }
            if dy < -180.0 {
                dy += 360.0;
            }
            if dt > 1e-4 { dy / dt } else { 0.0 }
        }
        None => 0.0,
    };
    *prev_yaw = Some((time.elapsed_secs_f64(), yaw_deg));
    let eff_gait = effective_gait(*stance, *gait);
    let phys_stance = if capsule_crouched {
        Stance::Crouching
    } else {
        *stance
    };
    let target = params_assets
        .get(handle.id())
        .map(|p| gait_max_speed(p, &phys_stance, &eff_gait))
        .unwrap_or(f32::NAN);
    info!(
        "[M3GATE] t={t:.1} gait={gait:?} eff={eff_gait:?} stance={stance:?} crouch_cap={capsule_crouched} loco={loco:?} y={:.2} vy={:.2} yaw={yaw_deg:.1} yaw_rate={yaw_rate:.0} target={target:.2} actual={actual:.2}",
        char_t.translation.y, vel.y
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
    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() && time.elapsed_secs_f64() >= 57.0 {
        warn!("[M3GATE] safety quit");
        exit.write(AppExit::Success);
    }
}
