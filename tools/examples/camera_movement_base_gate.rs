use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use avian3d::prelude::{AngularVelocity, Collider, LinearVelocity, RigidBody, Rotation};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_als::als::character::{AlsCharacter, AlsCharacterBundle};
use bevy_als::als::control_scheme::AlsControlSchemeConfig;
use bevy_als::als::dynamics::AlsMovementBaseDynamicsState;
use bevy_als::als::params::AlsMovementParams;
use bevy_als::als_camera::{AlsCameraMovementBase, AlsCameraTarget};
use bevy_tnua::TnuaPipelineSystems;

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

#[derive(Resource, Default)]
struct RemovalWatch {
    destroyed_at: Option<Instant>,
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
        .init_resource::<RemovalWatch>()
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
    let mut bundle =
        AlsCharacterBundle::new(params, config_assets, AlsControlSchemeConfig::default());
    bundle.transform = Transform::from_translation(translation);
    commands.spawn((bundle, AlsCameraTarget)).id()
}

fn verify(
    started: Res<Started>,
    probe: Res<Probe>,
    mut removal: ResMut<RemovalWatch>,
    states: Query<&AlsMovementBaseDynamicsState, With<AlsCharacter>>,
    camera_bases: Query<&AlsCameraMovementBase, With<AlsCameraTarget>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    if started.0.elapsed() < Duration::from_secs_f32(2.5) {
        return;
    }
    if let Some(destroyed_at) = removal.destroyed_at {
        if destroyed_at.elapsed() < Duration::from_millis(250) {
            return;
        }
        let removed = camera_bases.get(probe.rotation_character).is_err();
        if removed {
            info!(
                "[BF102.22][VERDICT] camera_receives_live_movement_base_state_and_clears_destroyed_base -> PASS"
            );
            exit.write(AppExit::Success);
        } else {
            error!("[BF102.22][VERDICT] destroyed_base_camera_component_still_present -> FAIL");
            exit.write(AppExit::Error(NonZeroU8::new(23).unwrap()));
        }
        return;
    }
    let (Ok(translation_state), Ok(rotation_state), Ok(translation_camera), Ok(rotation_camera)) = (
        states.get(probe.translation_character),
        states.get(probe.rotation_character),
        camera_bases.get(probe.translation_character),
        camera_bases.get(probe.rotation_character),
    ) else {
        return;
    };
    let translation_passed = translation_state.entity == Some(probe.translation_platform)
        && translation_camera.entity == probe.translation_platform
        && translation_camera.has_relative_rotation;
    let rotation_passed = rotation_state.entity == Some(probe.rotation_platform)
        && rotation_camera.entity == probe.rotation_platform
        && rotation_camera.has_relative_rotation;
    if translation_passed && rotation_passed {
        info!(
            "[BF102.22] translation_state={:?} translation_camera={:?}; rotation_state={:?} rotation_camera={:?}",
            translation_state.entity,
            translation_camera.entity,
            rotation_state.entity,
            rotation_camera.entity,
        );
        commands.entity(probe.rotation_platform).despawn();
        removal.destroyed_at = Some(Instant::now());
        info!("[BF102.22] destroyed rotation base after live-state verification");
    } else {
        error!(
            "[BF102.22][VERDICT] translation_state={:?} translation_camera={:?} translation_relative={} rotation_state={:?} rotation_camera={:?} rotation_relative={} -> FAIL",
            translation_state.entity,
            translation_camera.entity,
            translation_camera.has_relative_rotation,
            rotation_state.entity,
            rotation_camera.entity,
            rotation_camera.has_relative_rotation,
        );
        exit.write(AppExit::Error(NonZeroU8::new(22).unwrap()));
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        error!("[BF102.22][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(24).unwrap()));
    }
}
