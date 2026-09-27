use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use bevy_als::als::character::{AlsCharacter, AlsCharacterBundle, AlsEffectiveGravity};
use bevy_als::als::control_scheme::AlsControlSchemeConfig;
use bevy_als::als::params::AlsMovementParams;
use avian3d::prelude::{Gravity, GravityScale, LinearVelocity};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_tnua::{TnuaGravity, TnuaRigidBodyTracker};

use bevy_als::playable_character::AlsPlayableCharacterPlugin;

const GLOBAL_GRAVITY: Vec3 = Vec3::new(0.0, -12.0, 0.0);
const SAMPLES: [(f32, Option<Vec3>); 4] = [
    (0.5, None),
    (1.0, None),
    (2.0, None),
    (0.25, Some(Vec3::new(0.0, -5.0, 0.0))),
];

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource)]
struct Probe {
    characters: [Entity; SAMPLES.len()],
}

#[derive(Resource, Default)]
struct VelocitySamples {
    fixed_elapsed: Option<f32>,
    values: [f32; SAMPLES.len()],
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
        .insert_resource(Gravity(GLOBAL_GRAVITY))
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(14)))
        .init_resource::<VelocitySamples>()
        .add_systems(Startup, setup)
        .add_systems(Update, (verify, timeout))
        .run();
}

fn setup(
    mut commands: Commands,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    let characters = std::array::from_fn(|index| {
        let (scale, custom_gravity) = SAMPLES[index];
        let params = params_assets.add(AlsMovementParams::default());
        let config = config_assets.add(AlsControlSchemeConfig::default());
        let mut bundle = AlsCharacterBundle::new(params, config);
        bundle.transform = Transform::from_xyz(index as f32 * 8.0, 12.0, 0.0);
        let mut entity = commands.spawn((bundle, GravityScale(scale)));
        if let Some(custom_gravity) = custom_gravity {
            entity.insert(TnuaGravity(custom_gravity));
        }
        entity.id()
    });
    commands.insert_resource(Probe { characters });
}

fn verify(
    fixed_time: Res<Time<Fixed>>,
    probe: Res<Probe>,
    mut samples: ResMut<VelocitySamples>,
    gravity_scales: Query<&GravityScale, With<AlsCharacter>>,
    trackers: Query<&TnuaRigidBodyTracker, With<AlsCharacter>>,
    effective_gravity: Query<&AlsEffectiveGravity, With<AlsCharacter>>,
    velocities: Query<&LinearVelocity, With<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = fixed_time.elapsed_secs();
    if samples.fixed_elapsed.is_none() && elapsed >= 0.5 {
        let Ok(values) = collect_vertical_velocities(&velocities, &probe.characters) else {
            return;
        };
        samples.fixed_elapsed = Some(elapsed);
        samples.values = values;
        return;
    }
    let Some(sample_elapsed) = samples.fixed_elapsed else {
        return;
    };
    if elapsed < 1.25 {
        return;
    }
    let Ok(current_velocities) = collect_vertical_velocities(&velocities, &probe.characters) else {
        return;
    };
    let dt = elapsed - sample_elapsed;
    if dt <= 0.1 {
        return;
    }
    let mut trackers_match = true;
    let mut physical_match = true;
    let mut scales_match = true;
    let mut tracker_values = [Vec3::ZERO; SAMPLES.len()];
    let mut acceleration_values = [0.0; SAMPLES.len()];
    for (index, entity) in probe.characters.iter().copied().enumerate() {
        let (Ok(scale), Ok(tracker), Ok(effective)) = (
            gravity_scales.get(entity),
            trackers.get(entity),
            effective_gravity.get(entity),
        ) else {
            return;
        };
        let (expected_scale, custom_gravity) = SAMPLES[index];
        let expected = GLOBAL_GRAVITY * expected_scale + custom_gravity.unwrap_or(Vec3::ZERO);
        let acceleration = (current_velocities[index] - samples.values[index]) / dt;
        tracker_values[index] = tracker.gravity;
        acceleration_values[index] = acceleration;
        trackers_match &= tracker.gravity.distance(expected) < 1e-4
            && effective.value.distance(expected) < 1e-4
            && effective.global.distance(GLOBAL_GRAVITY) < 1e-4
            && effective.tnua_gravity == custom_gravity.unwrap_or(Vec3::ZERO);
        scales_match &= (scale.0 - expected_scale).abs() < 1e-4
            && (effective.gravity_scale - expected_scale).abs() < 1e-4;
        physical_match &= (acceleration - expected.y).abs() < 0.7;
    }
    if trackers_match && scales_match && physical_match {
        info!(
            "[BF102.23] tracker={:?} acceleration_y={:?} scales={:?}",
            tracker_values,
            acceleration_values,
            SAMPLES.map(|sample| sample.0),
        );
        info!("[BF102.23][VERDICT] tracker_and_physics_share_effective_gravity -> PASS");
        exit.write(AppExit::Success);
    } else {
        error!(
            "[BF102.23][VERDICT] tracker_match={} scale_match={} physical_match={} tracker={:?} acceleration_y={:?} -> FAIL",
            trackers_match, scales_match, physical_match, tracker_values, acceleration_values,
        );
        exit.write(AppExit::Error(NonZeroU8::new(23).unwrap()));
    }
}

fn collect_vertical_velocities(
    velocities: &Query<&LinearVelocity, With<AlsCharacter>>,
    characters: &[Entity; SAMPLES.len()],
) -> Result<[f32; SAMPLES.len()], ()> {
    let mut values = [0.0; SAMPLES.len()];
    for (index, entity) in characters.iter().copied().enumerate() {
        values[index] = velocities.get(entity).map_err(|_| ())?.0.y;
    }
    Ok(values)
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        error!("[BF102.23][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(24).unwrap()));
    }
}
