use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use avian3d::prelude::{AngularVelocity, Collider, LinearVelocity, RigidBody, Rotation};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_tnua::TnuaPipelineSystems;

use als_core::character::{AlsCharacter, AlsCharacterBundle};
use als_core::control_scheme::AlsControlSchemeConfig;
use als_core::dynamics::AlsMovementBaseDynamicsState;
use als_core::params::AlsMovementParams;
use bevy_als::playable_character::AlsPlayableCharacterPlugin;

#[derive(Component)]
struct TranslationPlatform;

#[derive(Component)]
struct RotationPlatform;

#[derive(Resource)]
struct Started(Instant);

#[derive(Resource)]
struct Deadline(Instant);

#[derive(Resource)]
struct Probe {
    translation_platform: Entity,
    translation_character: Entity,
    rotation_platform: Entity,
    rotation_character: Entity,
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
        .insert_resource(Deadline(Instant::now() + Duration::from_secs(14)))
        .add_systems(Startup, setup)
        .add_systems(
            FixedUpdate,
            drive_rotation_platform.before(TnuaPipelineSystems::Sensors),
        )
        .add_systems(
            FixedPostUpdate,
            drive_rotation_platform.before(TnuaPipelineSystems::Logic),
        )
        .add_systems(Update, (verify, timeout))
        .run();
}

fn drive_rotation_platform(
    fixed_time: Res<Time<Fixed>>,
    mut platforms: Query<
        (&mut Transform, &mut Rotation, &mut AngularVelocity),
        With<RotationPlatform>,
    >,
) {
    let angle = fixed_time.elapsed_secs();
    for (mut transform, mut rotation, mut angular_velocity) in &mut platforms {
        let next_rotation = Quat::from_rotation_y(angle);
        transform.rotation = next_rotation;
        rotation.0 = next_rotation;
        angular_velocity.0 = Vec3::Y;
    }
}

fn setup(
    mut commands: Commands,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(30.0, 0.2, 30.0),
        Transform::from_xyz(0.0, -0.2, 0.0),
    ));
    let translation_platform = commands
        .spawn((
            TranslationPlatform,
            RigidBody::Kinematic,
            Collider::cuboid(3.0, 0.25, 3.0),
            LinearVelocity(Vec3::X * 2.0),
            Transform::from_xyz(-5.0, 0.25, 0.0),
        ))
        .id();
    let rotation_platform = commands
        .spawn((
            RotationPlatform,
            RigidBody::Kinematic,
            Collider::cylinder(4.0, 0.5),
            AngularVelocity(Vec3::Y),
            Transform::from_xyz(5.0, 0.25, 0.0),
        ))
        .id();
    let translation_character = spawn_character(
        &mut commands,
        &mut params_assets,
        &mut config_assets,
        Vec3::new(-5.0, 1.42, 0.0),
    );
    let rotation_character = spawn_character(
        &mut commands,
        &mut params_assets,
        &mut config_assets,
        Vec3::new(7.0, 1.42, 0.0),
    );
    commands.insert_resource(Probe {
        translation_platform,
        translation_character,
        rotation_platform,
        rotation_character,
    });
}

fn spawn_character(
    commands: &mut Commands,
    params_assets: &mut Assets<AlsMovementParams>,
    config_assets: &mut Assets<AlsControlSchemeConfig>,
    translation: Vec3,
) -> Entity {
    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_translation(translation);
    commands.spawn(bundle).id()
}

fn verify(
    started: Res<Started>,
    probe: Res<Probe>,
    states: Query<&AlsMovementBaseDynamicsState, With<AlsCharacter>>,
    velocities: Query<&LinearVelocity, With<AlsCharacter>>,
    platforms: Query<(&LinearVelocity, Option<&AngularVelocity>), Without<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    if started.0.elapsed() < Duration::from_secs_f32(2.5) {
        return;
    }
    let (
        Ok(translation_state),
        Ok(rotation_state),
        Ok(translation_velocity),
        Ok(rotation_velocity),
    ) = (
        states.get(probe.translation_character),
        states.get(probe.rotation_character),
        velocities.get(probe.translation_character),
        velocities.get(probe.rotation_character),
    )
    else {
        return;
    };
    let (Ok((translation_linear, translation_angular)), Ok((rotation_linear, rotation_angular))) = (
        platforms.get(probe.translation_platform),
        platforms.get(probe.rotation_platform),
    ) else {
        return;
    };
    let translation_passed = translation_state.entity == Some(probe.translation_platform)
        && (translation_state.support_velocity.x - 2.0).abs() < 0.25
        && translation_state.relative_velocity.length() < 0.25
        && translation_state.motor_boost.length() < 0.1
        && translation_velocity.0.x > 1.0;
    let rotation_passed = rotation_state.entity == Some(probe.rotation_platform)
        && rotation_state.support_velocity.length() > 1.0
        && rotation_state.relative_velocity.length() < 0.35
        && rotation_state.motor_boost.length() < 0.15
        && rotation_velocity.0.length() > 1.0;
    if translation_passed && rotation_passed {
        info!(
            "[BF102.17] translation world={:?} support={:?} relative={:?} boost={:?}; rotation world={:?} support={:?} relative={:?} boost={:?}",
            translation_state.world_velocity,
            translation_state.support_velocity,
            translation_state.relative_velocity,
            translation_state.motor_boost,
            rotation_state.world_velocity,
            rotation_state.support_velocity,
            rotation_state.relative_velocity,
            rotation_state.motor_boost,
        );
        info!("[BF102.17][VERDICT] braking_uses_movement_base_relative_velocity -> PASS");
        exit.write(AppExit::Success);
    } else {
        error!(
            "[BF102.17][VERDICT] expected_translation_base={:?} expected_rotation_base={:?} translation_base={:?} translation_world={:?} translation_support={:?} translation_relative={:?} translation_boost={:?} translation_platform=({:?},{:?}); rotation_base={:?} rotation_world={:?} rotation_support={:?} rotation_relative={:?} rotation_boost={:?} rotation_platform=({:?},{:?}) -> FAIL",
            probe.translation_platform,
            probe.rotation_platform,
            translation_state.entity,
            translation_state.world_velocity,
            translation_state.support_velocity,
            translation_state.relative_velocity,
            translation_state.motor_boost,
            translation_linear.0,
            translation_angular.map(|value| value.0),
            rotation_state.entity,
            rotation_state.world_velocity,
            rotation_state.support_velocity,
            rotation_state.relative_velocity,
            rotation_state.motor_boost,
            rotation_linear.0,
            rotation_angular.map(|value| value.0),
        );
        exit.write(AppExit::Error(NonZeroU8::new(17).unwrap()));
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        error!("[BF102.17][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(18).unwrap()));
    }
}
