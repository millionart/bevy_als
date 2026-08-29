use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use avian3d::prelude::{Collider, LinearVelocity, PhysicsSystems, RigidBody};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;

use als_core::character::{AlsCharacter, AlsCharacterBundle};
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::input::AlsInputCommand;
use als_core::motion::{
    AlsRotationAnimationState, AlsRotationResponseSet, AlsRotationResponseState, AlsViewYaw,
};
use als_core::orientation::world_forward_to_yaw;
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, RotationMode};
use bevy_als::playable_character::AlsPlayableCharacterPlugin;

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource)]
struct Started(Instant);

#[derive(Resource)]
struct Probe {
    fast: Entity,
    slow: Entity,
    curve: Entity,
    moving: Entity,
    aiming: Entity,
    stepped: bool,
    curve_seeded: bool,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            }),
            AlsPlayableCharacterPlugin,
        ))
        .insert_resource(Started(Instant::now()))
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(12)))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                drive.before(als_core::aim::aim_rotation_mode_system),
                verify,
                timeout,
            ),
        )
        .add_systems(
            FixedPostUpdate,
            pin_ground
                .after(PhysicsSystems::Last)
                .before(AlsRotationResponseSet),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 0.2, 40.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    let mut fast_params = AlsMovementParams::default();
    fast_params
        .rotation
        .grounded_stationary_view_direction
        .interpolation_half_lives
        .running = 0.05;
    fast_params
        .rotation
        .grounded_stationary_view_direction
        .interpolation_half_lives
        .walking = 0.05;
    fast_params
        .rotation
        .grounded_stationary_view_direction
        .interpolation_half_lives
        .sprinting = 0.05;
    let mut slow_params = AlsMovementParams::default();
    slow_params
        .rotation
        .grounded_stationary_view_direction
        .interpolation_half_lives
        .running = 2.0;
    slow_params
        .rotation
        .grounded_stationary_view_direction
        .interpolation_half_lives
        .walking = 2.0;
    slow_params
        .rotation
        .grounded_stationary_view_direction
        .interpolation_half_lives
        .sprinting = 2.0;
    let curve_params = AlsMovementParams::default();
    let mut moving_params = AlsMovementParams::default();
    moving_params
        .rotation
        .grounded_moving_view_direction
        .interpolation_half_lives
        .running = 0.05;
    let mut aiming_params = AlsMovementParams::default();
    aiming_params
        .rotation
        .grounded_moving_aiming
        .interpolation_half_life = 0.05;
    let fast = spawn_character(
        &mut commands,
        params_assets.add(fast_params),
        config_assets.add(AlsControlSchemeConfig::default()),
        Vec3::new(-4.0, 0.92, 0.0),
        RotationMode::ViewDirection,
    );
    let slow = spawn_character(
        &mut commands,
        params_assets.add(slow_params),
        config_assets.add(AlsControlSchemeConfig::default()),
        Vec3::new(-2.0, 0.92, 0.0),
        RotationMode::ViewDirection,
    );
    let curve = spawn_character(
        &mut commands,
        params_assets.add(curve_params),
        config_assets.add(AlsControlSchemeConfig::default()),
        Vec3::new(0.0, 0.92, 0.0),
        RotationMode::ViewDirection,
    );
    let moving = spawn_character(
        &mut commands,
        params_assets.add(moving_params),
        config_assets.add(AlsControlSchemeConfig::default()),
        Vec3::new(2.0, 0.92, 0.0),
        RotationMode::ViewDirection,
    );
    let aiming = spawn_character(
        &mut commands,
        params_assets.add(aiming_params),
        config_assets.add(AlsControlSchemeConfig::default()),
        Vec3::new(4.0, 0.92, 0.0),
        RotationMode::Aiming,
    );
    commands.insert_resource(Probe {
        fast,
        slow,
        curve,
        moving,
        aiming,
        stepped: false,
        curve_seeded: false,
    });
}

fn spawn_character(
    commands: &mut Commands,
    params: Handle<AlsMovementParams>,
    config: Handle<AlsControlSchemeConfig>,
    translation: Vec3,
    rotation_mode: RotationMode,
) -> Entity {
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_translation(translation);
    bundle.gait = Gait::Running;
    bundle.locomotion_state = LocomotionState::Grounded;
    bundle.rotation_mode = rotation_mode;
    commands.spawn((bundle, AlsViewYaw(0.0))).id()
}

fn drive(
    mut commands: Commands,
    mut probe: ResMut<Probe>,
    started: Res<Started>,
    mut characters: Query<(&mut AlsViewYaw, &mut AlsInputCommand), With<AlsCharacter>>,
) {
    for entity in [
        probe.fast,
        probe.slow,
        probe.curve,
        probe.moving,
        probe.aiming,
    ] {
        let Ok((mut view, mut input)) = characters.get_mut(entity) else {
            continue;
        };
        input.aim_held = entity == probe.aiming;
        if probe.stepped && entity != probe.curve {
            view.0 = std::f32::consts::FRAC_PI_2;
        }
    }
    let elapsed = started.0.elapsed();
    if !probe.stepped && elapsed >= Duration::from_millis(200) {
        probe.stepped = true;
        info!("[BF102.11] applied view yaw step=+90deg");
    }
    if !probe.curve_seeded && elapsed >= Duration::from_millis(350) {
        commands
            .entity(probe.curve)
            .insert(AlsRotationAnimationState {
                yaw_offset_degrees: 0.0,
                yaw_speed_degrees: 90.0,
            });
        probe.curve_seeded = true;
        info!("[BF102.11] applied stationary RotationYawSpeed=90deg/s");
    }
}

fn pin_ground(
    probe: Res<Probe>,
    mut characters: Query<
        (Entity, &mut Gait, &mut LocomotionState, &mut LinearVelocity),
        With<AlsCharacter>,
    >,
) {
    for (entity, mut gait, mut locomotion, mut velocity) in &mut characters {
        *gait = Gait::Running;
        *locomotion = LocomotionState::Grounded;
        velocity.0 = if entity == probe.moving {
            Vec3::Z
        } else {
            Vec3::ZERO
        };
    }
}

fn verify(
    started: Res<Started>,
    probe: Res<Probe>,
    transforms: Query<&Transform, With<AlsCharacter>>,
    states: Query<&AlsRotationResponseState, With<AlsCharacter>>,
    modes: Query<&RotationMode, With<AlsCharacter>>,
    velocities: Query<&LinearVelocity, With<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !probe.stepped || !probe.curve_seeded {
        return;
    }
    if started.0.elapsed() < Duration::from_millis(800) {
        return;
    }
    let (Ok(fast), Ok(slow), Ok(curve), Ok(moving), Ok(aiming)) = (
        transforms.get(probe.fast),
        transforms.get(probe.slow),
        transforms.get(probe.curve),
        transforms.get(probe.moving),
        transforms.get(probe.aiming),
    ) else {
        return;
    };
    let (Ok(fast_state), Ok(slow_state)) = (states.get(probe.fast), states.get(probe.slow)) else {
        return;
    };
    let Ok(aiming_mode) = modes.get(probe.aiming) else {
        return;
    };
    let Ok(moving_velocity) = velocities.get(probe.moving) else {
        return;
    };
    let fast_yaw = world_forward_to_yaw(fast.rotation * Vec3::NEG_Z);
    let slow_yaw = world_forward_to_yaw(slow.rotation * Vec3::NEG_Z);
    let curve_yaw = world_forward_to_yaw(curve.rotation * Vec3::NEG_Z);
    let moving_yaw = world_forward_to_yaw(moving.rotation * Vec3::NEG_Z);
    let aiming_yaw = world_forward_to_yaw(aiming.rotation * Vec3::NEG_Z);
    let moving_speed = moving_velocity.0.xz().length();
    let passed = fast_yaw > slow_yaw + 0.2
        && slow_yaw > 0.01
        && curve_yaw > 0.1
        && moving_speed > 0.5
        && moving_yaw > 1.3
        && aiming_yaw > 0.2
        && aiming_yaw < 0.5
        && *aiming_mode == RotationMode::Aiming;
    if passed {
        info!(
            "[BF102.11] fast_yaw={fast_yaw:.3} slow_yaw={slow_yaw:.3} curve_yaw={curve_yaw:.3} moving_speed={moving_speed:.3} moving_yaw={moving_yaw:.3} aiming_yaw={aiming_yaw:.3} aiming_mode={aiming_mode:?} fast_target={:.3} slow_target={:.3}",
            fast_state.smooth_target_yaw, slow_state.smooth_target_yaw,
        );
        info!("[BF102.11][VERDICT] stationary_moving_aiming_and_curve_drive_transform_yaw -> PASS");
        exit.write(AppExit::Success);
    } else {
        error!(
            "[BF102.11][VERDICT] fast_yaw={fast_yaw:.3} slow_yaw={slow_yaw:.3} curve_yaw={curve_yaw:.3} moving_speed={moving_speed:.3} moving_yaw={moving_yaw:.3} aiming_yaw={aiming_yaw:.3} aiming_mode={aiming_mode:?} fast_target={:.3} slow_target={:.3} -> FAIL",
            fast_state.smooth_target_yaw, slow_state.smooth_target_yaw,
        );
        exit.write(AppExit::Error(NonZeroU8::new(11).unwrap()));
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        error!("[BF102.11][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(12).unwrap()));
    }
}
