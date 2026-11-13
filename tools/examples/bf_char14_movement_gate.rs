use avian3d::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use bevy_als::als::character::{AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin};
use bevy_als::als::control_scheme::AlsControlScheme;
use bevy_als::als::input::AlsInputCommand;
use bevy_als::als::motion::{
    AlsInputOverride, AlsMotionPlugin, AlsViewYaw, initial_als_control_scheme_config,
};
use bevy_als::als::params::AlsMovementParams;
use bevy_als::als::state::RotationMode;

const CASE_DURATION_SECS: f32 = 0.25;
const SAMPLE_AFTER_SECS: f32 = 0.16;
const MIN_SPEED: f32 = 0.4;
const MIN_DIRECTION_DOT: f32 = 0.98;

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
        AlsCharacterPlugin,
        AlsMotionPlugin,
    ))
    .init_resource::<MovementDriver>()
    .add_systems(Startup, setup)
    .add_systems(Update, run_cases);
    app.run();
}

#[derive(Component)]
struct MovementGateCamera;

#[derive(Clone, Copy)]
struct Case {
    yaw: f32,
    input: Vec2,
    rotation_mode: RotationMode,
}

#[derive(Resource)]
struct MovementDriver {
    cases: Vec<Case>,
    index: usize,
    started_at: f32,
    sampled: bool,
    failures: u32,
    max_error: f32,
}

impl Default for MovementDriver {
    fn default() -> Self {
        let mut cases = Vec::new();
        for yaw in [0.37, 1.21, -2.02, 2.68] {
            for rotation_mode in [
                RotationMode::VelocityDirection,
                RotationMode::ViewDirection,
                RotationMode::Aiming,
            ] {
                cases.push(Case {
                    yaw,
                    input: Vec2::Y,
                    rotation_mode,
                });
            }
        }
        for input in [Vec2::NEG_Y, Vec2::X, Vec2::NEG_X] {
            for rotation_mode in [
                RotationMode::VelocityDirection,
                RotationMode::ViewDirection,
                RotationMode::Aiming,
            ] {
                cases.push(Case {
                    yaw: 1.21,
                    input,
                    rotation_mode,
                });
            }
        }
        Self {
            cases,
            index: 0,
            started_at: -1.0,
            sampled: false,
            failures: 0,
            max_error: 0.0,
        }
    }
}

fn setup(
    mut commands: Commands,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<bevy_als::als::control_scheme::AlsControlSchemeConfig>>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(80.0, 0.2, 80.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    let params_value = AlsMovementParams::default();
    let params = params_assets.add(params_value.clone());
    let mut bundle = AlsCharacterBundle::new(params, &mut config_assets, initial_als_control_scheme_config(&params_value));
    bundle.transform = Transform::from_xyz(0.0, 0.925, 0.0);
    commands.spawn((
        bundle,
        AlsViewYaw::default(),
        AlsInputOverride(AlsInputCommand::default()),
    ));
    commands.spawn((
        MovementGateCamera,
        Camera3d::default(),
        Transform::default(),
    ));
}

fn expected_direction(camera: &Transform, input: Vec2) -> Vec3 {
    let forward = camera.rotation * Vec3::NEG_Z;
    let right = camera.rotation * Vec3::X;
    (forward * input.y + right * input.x).normalize_or_zero()
}

#[allow(clippy::type_complexity)]
fn run_cases(
    time: Res<Time>,
    mut driver: ResMut<MovementDriver>,
    mut character: Query<
        (
            &mut Transform,
            &mut Position,
            &mut LinearVelocity,
            &mut AlsInputOverride,
            &mut AlsViewYaw,
            &mut RotationMode,
        ),
        With<AlsCharacter>,
    >,
    mut cameras: Query<&mut Transform, (With<MovementGateCamera>, Without<AlsCharacter>)>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((mut transform, mut position, mut velocity, mut input, mut view_yaw, mut rotation_mode)) =
        character.single_mut()
    else {
        return;
    };
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    let now = time.elapsed_secs();
    if driver.index >= driver.cases.len() {
        info!(
            "[BCHAR14][VERDICT] cases={} failures={} max_error={:.6} -> {}",
            driver.cases.len(),
            driver.failures,
            driver.max_error,
            if driver.failures == 0 { "PASS" } else { "FAIL" }
        );
        exit.write(if driver.failures == 0 {
            AppExit::Success
        } else {
            AppExit::Error(std::num::NonZeroU8::new(14).unwrap())
        });
        return;
    }
    if driver.started_at < 0.0 || now - driver.started_at >= CASE_DURATION_SECS {
        let case = driver.cases[driver.index];
        transform.translation = Vec3::new(0.0, 0.925, 0.0);
        position.0 = transform.translation;
        velocity.0 = Vec3::ZERO;
        input.0.move_dir = case.input;
        input.0.look_delta = Vec2::ZERO;
        input.0.look_rate = Vec2::ZERO;
        input.0.jump_pressed = false;
        input.0.jump_held = false;
        input.0.roll_pressed = false;
        input.0.crouch_pressed = false;
        input.0.sprint_held = false;
        input.0.aim_held = false;
        view_yaw.0 = case.yaw;
        *rotation_mode = case.rotation_mode;
        camera.translation = Vec3::new(0.0, 2.0, 4.0);
        camera.rotation = Quat::from_rotation_y(case.yaw);
        driver.started_at = now;
        driver.sampled = false;
        return;
    }
    if driver.sampled || now - driver.started_at < SAMPLE_AFTER_SECS {
        return;
    }
    let case = driver.cases[driver.index];
    let expected = expected_direction(&camera, case.input);
    let actual = Vec3::new(velocity.x, 0.0, velocity.z).normalize_or_zero();
    let speed = Vec2::new(velocity.x, velocity.z).length();
    let dot = actual.dot(expected);
    let error = 1.0 - dot;
    driver.max_error = driver.max_error.max(error);
    let pass = speed >= MIN_SPEED && dot >= MIN_DIRECTION_DOT;
    info!(
        "[BCHAR14][CASE] index={} yaw={:.3} input=({:.1},{:.1}) mode={:?} speed={:.3} dot={:.6} capsule_yaw={:.3} -> {}",
        driver.index,
        case.yaw,
        case.input.x,
        case.input.y,
        case.rotation_mode,
        speed,
        dot,
        f32::atan2(transform.forward().x, -transform.forward().z),
        if pass { "PASS" } else { "FAIL" }
    );
    if !pass {
        driver.failures += 1;
    }
    driver.sampled = true;
    driver.index += 1;
}
