use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use avian3d::prelude::{Collider, LinearVelocity, RigidBody};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_tnua::{TnuaMotor, TnuaToggle};

use bevy_als::als::character::AlsCharacterBundle;
use bevy_als::als::control_scheme::AlsControlSchemeConfig;
use bevy_als::als::input::AlsInputCommand;
use bevy_als::als::motion::{AlsInputOverride, AlsViewYaw};
use bevy_als::als::params::AlsMovementParams;
use bevy_als::playable_character::AlsPlayableCharacterPlugin;

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource)]
struct Started(Instant);

#[derive(Resource)]
struct Probe {
    enabled_before: Entity,
    disabled: Entity,
    enabled_middle: Entity,
    sense_only: Entity,
    enabled_after: Entity,
    starts: Option<[Vec3; 5]>,
    initial_verified: bool,
    restored_at: Option<Instant>,
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
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(15)))
        .add_systems(Startup, setup)
        .add_systems(Update, (drive, verify, timeout))
        .run();
}

fn setup(
    mut commands: Commands,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(30.0, 0.2, 30.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    let toggles = [
        TnuaToggle::Enabled,
        TnuaToggle::Disabled,
        TnuaToggle::Enabled,
        TnuaToggle::SenseOnly,
        TnuaToggle::Enabled,
    ];
    let mut entities = Vec::with_capacity(toggles.len());
    for (index, toggle) in toggles.into_iter().enumerate() {
        let params = params_assets.add(AlsMovementParams::default());
        let mut bundle = AlsCharacterBundle::new(
            params,
            &mut config_assets,
            AlsControlSchemeConfig::default(),
        );
        bundle.transform = Transform::from_xyz(index as f32 * 2.0 - 4.0, 0.92, 0.0);
        entities.push(
            commands
                .spawn((
                    bundle,
                    AlsViewYaw(0.0),
                    AlsInputOverride(AlsInputCommand {
                        move_dir: Vec2::Y,
                        ..default()
                    }),
                    toggle,
                ))
                .id(),
        );
    }
    commands.insert_resource(Probe {
        enabled_before: entities[0],
        disabled: entities[1],
        enabled_middle: entities[2],
        sense_only: entities[3],
        enabled_after: entities[4],
        starts: None,
        initial_verified: false,
        restored_at: None,
    });
}

fn drive(
    mut commands: Commands,
    mut probe: ResMut<Probe>,
    started: Res<Started>,
    transforms: Query<&Transform>,
) {
    if probe.starts.is_none() && started.0.elapsed() >= Duration::from_millis(500) {
        let entities = [
            probe.enabled_before,
            probe.disabled,
            probe.enabled_middle,
            probe.sense_only,
            probe.enabled_after,
        ];
        let starts = entities.map(|entity| {
            transforms
                .get(entity)
                .map_or(Vec3::ZERO, |value| value.translation)
        });
        probe.starts = Some(starts);
        info!("[BF102.16] captured enabled-disabled-enabled-sense-only-enabled baseline");
    }
    if probe.starts.is_none() {
        return;
    }
    if probe.restored_at.is_none() && probe.initial_verified {
        commands.entity(probe.disabled).insert(TnuaToggle::Enabled);
        commands
            .entity(probe.sense_only)
            .insert(TnuaToggle::Enabled);
        probe.restored_at = Some(Instant::now());
        info!("[BF102.16] restored disabled and sense-only motors");
    }
}

fn verify(
    mut probe: ResMut<Probe>,
    transforms: Query<&Transform>,
    velocities: Query<&LinearVelocity>,
    motors: Query<Has<TnuaMotor>>,
    toggles: Query<&TnuaToggle>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(starts) = probe.starts else {
        return;
    };
    let entities = [
        probe.enabled_before,
        probe.disabled,
        probe.enabled_middle,
        probe.sense_only,
        probe.enabled_after,
    ];
    let (Ok(transform_a), Ok(transform_b), Ok(transform_c), Ok(transform_d), Ok(transform_e)) = (
        transforms.get(entities[0]),
        transforms.get(entities[1]),
        transforms.get(entities[2]),
        transforms.get(entities[3]),
        transforms.get(entities[4]),
    ) else {
        return;
    };
    let (Ok(velocity_a), Ok(velocity_b), Ok(velocity_c), Ok(velocity_d), Ok(velocity_e)) = (
        velocities.get(entities[0]),
        velocities.get(entities[1]),
        velocities.get(entities[2]),
        velocities.get(entities[3]),
        velocities.get(entities[4]),
    ) else {
        return;
    };
    let (Ok(motor_a), Ok(motor_b), Ok(motor_c), Ok(motor_d), Ok(motor_e)) = (
        motors.get(entities[0]),
        motors.get(entities[1]),
        motors.get(entities[2]),
        motors.get(entities[3]),
        motors.get(entities[4]),
    ) else {
        return;
    };
    let (Ok(toggle_a), Ok(toggle_b), Ok(toggle_c), Ok(toggle_d), Ok(toggle_e)) = (
        toggles.get(entities[0]),
        toggles.get(entities[1]),
        toggles.get(entities[2]),
        toggles.get(entities[3]),
        toggles.get(entities[4]),
    ) else {
        return;
    };
    let transforms = [
        transform_a,
        transform_b,
        transform_c,
        transform_d,
        transform_e,
    ];
    let velocities = [velocity_a, velocity_b, velocity_c, velocity_d, velocity_e];
    let motors = [motor_a, motor_b, motor_c, motor_d, motor_e];
    let toggles = [*toggle_a, *toggle_b, *toggle_c, *toggle_d, *toggle_e];
    let travel: [f32; 5] =
        std::array::from_fn(|index| transforms[index].translation.z - starts[index].z);
    let enabled_progress = travel[0] < -0.5 && travel[2] < -0.5 && travel[4] < -0.5;
    let disabled_suspended = motors[1]
        && motors[3]
        && toggles[1] == TnuaToggle::Disabled
        && toggles[3] == TnuaToggle::SenseOnly;
    let Some(restored_at) = probe.restored_at else {
        if enabled_progress && disabled_suspended {
            if !probe.initial_verified {
                probe.initial_verified = true;
                info!(
                    "[BF102.16] enabled travel=({:.3},{:.3},{:.3}) while disabled and sense-only motors are suspended",
                    travel[0], travel[2], travel[4],
                );
            }
            return;
        }
        return;
    };
    if restored_at.elapsed() < Duration::from_millis(700) {
        return;
    }
    let restored = motors[1]
        && motors[3]
        && toggles[1] == TnuaToggle::Enabled
        && toggles[3] == TnuaToggle::Enabled;
    let restored_progress = travel[1] < -0.2 && travel[3] < -0.2;
    let passed = probe.initial_verified && enabled_progress && restored && restored_progress;
    if passed {
        info!(
            "[BF102.16] travel=({:.3},{:.3},{:.3},{:.3},{:.3}) velocity_after=({:.3},{:.3},{:.3}) motors=({},{},{}) restored=({}, {})",
            travel[0],
            travel[1],
            travel[2],
            travel[3],
            travel[4],
            velocities[0].0.length(),
            velocities[2].0.length(),
            velocities[4].0.length(),
            motors[0],
            motors[2],
            motors[4],
            motors[1],
            motors[3],
        );
        info!("[BF102.16][VERDICT] disabled_entities_do_not_stop_other_motors -> PASS");
        exit.write(AppExit::Success);
    } else {
        error!(
            "[BF102.16][VERDICT] travel={travel:?} enabled_progress={enabled_progress} restored={restored} restored_progress={restored_progress} motors={motors:?} toggles={toggles:?} -> FAIL"
        );
        exit.write(AppExit::Error(NonZeroU8::new(16).unwrap()));
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        error!("[BF102.16][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(17).unwrap()));
    }
}
