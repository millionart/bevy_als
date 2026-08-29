use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use avian3d::prelude::{Collider, RigidBody};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;

use als_core::character::{AlsCharacter, AlsCharacterBundle};
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::input::AlsInputCommand;
use als_core::motion::AlsInputOverride;
use als_core::params::AlsMovementParams;
use als_core::state::RotationMode;
use bevy_als::playable_character::AlsPlayableCharacterPlugin;

#[derive(Clone, Copy)]
struct Case {
    raw_aim: bool,
    override_aim: Option<bool>,
    expected: RotationMode,
}

const CASES: [Case; 4] = [
    Case {
        raw_aim: false,
        override_aim: None,
        expected: RotationMode::ViewDirection,
    },
    Case {
        raw_aim: true,
        override_aim: None,
        expected: RotationMode::Aiming,
    },
    Case {
        raw_aim: false,
        override_aim: Some(true),
        expected: RotationMode::Aiming,
    },
    Case {
        raw_aim: true,
        override_aim: Some(false),
        expected: RotationMode::ViewDirection,
    },
];

const CASE_DURATION: Duration = Duration::from_millis(400);
const SAMPLE_DELAY: Duration = Duration::from_millis(180);

#[derive(Resource)]
struct Started(Instant);

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource)]
struct Probe {
    character: Entity,
    current: Option<usize>,
    samples: [Option<RotationMode>; CASES.len()],
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
                record
                    .after(drive)
                    .after(als_core::transition::state_transition_system),
                timeout,
            ),
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
        Collider::cuboid(20.0, 0.2, 20.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    let mut bundle = AlsCharacterBundle::new(
        params_assets.add(AlsMovementParams::default()),
        config_assets.add(AlsControlSchemeConfig::default()),
    );
    bundle.transform = Transform::from_xyz(0.0, 0.92, 0.0);
    bundle.rotation_mode = RotationMode::ViewDirection;
    let character = commands.spawn(bundle).id();
    commands.insert_resource(Probe {
        character,
        current: None,
        samples: [None; CASES.len()],
    });
}

fn drive(
    mut commands: Commands,
    started: Res<Started>,
    mut probe: ResMut<Probe>,
    mut inputs: Query<&mut AlsInputCommand, With<AlsCharacter>>,
) {
    let index = (started.0.elapsed().as_millis() / CASE_DURATION.as_millis()) as usize;
    let Some(case) = CASES.get(index).copied() else {
        return;
    };
    if probe.current == Some(index) {
        return;
    }
    let Ok(mut input) = inputs.get_mut(probe.character) else {
        return;
    };
    input.aim_held = case.raw_aim;
    let mut entity = commands.entity(probe.character);
    if let Some(aim_held) = case.override_aim {
        entity.insert(AlsInputOverride(AlsInputCommand {
            aim_held,
            ..default()
        }));
    } else {
        entity.remove::<AlsInputOverride>();
    }
    probe.current = Some(index);
    info!(
        "[BF102.13] applied case={index} raw_aim={} override_aim={:?}",
        case.raw_aim, case.override_aim,
    );
}

fn record(
    started: Res<Started>,
    mut probe: ResMut<Probe>,
    modes: Query<&RotationMode, With<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = started.0.elapsed();
    for (index, case) in CASES.iter().enumerate() {
        if probe.samples[index].is_some() || elapsed < CASE_DURATION * index as u32 + SAMPLE_DELAY {
            continue;
        }
        let Ok(mode) = modes.get(probe.character) else {
            return;
        };
        probe.samples[index] = Some(*mode);
        info!(
            "[BF102.13] case={index} raw_aim={} override_aim={:?} actual={mode:?} expected={:?}",
            case.raw_aim, case.override_aim, case.expected,
        );
    }
    if probe.samples.iter().any(Option::is_none) {
        return;
    }
    let passed = probe
        .samples
        .iter()
        .zip(CASES)
        .all(|(actual, case)| *actual == Some(case.expected));
    if passed {
        info!("[BF102.13][VERDICT] complete_override_truth_table -> PASS");
        exit.write(AppExit::Success);
    } else {
        error!("[BF102.13][VERDICT] samples={:?} -> FAIL", probe.samples,);
        exit.write(AppExit::Error(NonZeroU8::new(13).unwrap()));
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        error!("[BF102.13][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(14).unwrap()));
    }
}
