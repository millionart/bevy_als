use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_core::character::{AlsCharacterBundle, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::params::AlsMovementParams;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins,
        PhysicsPlugins::new(FixedPostUpdate),
        TnuaAvian3dPlugin::new(FixedPostUpdate),
        TnuaControllerPlugin::<AlsControlScheme>::new(FixedPostUpdate),
        AlsCharacterPlugin,
    ))
    .init_resource::<GateState>()
    .add_systems(Startup, setup)
    .add_systems(Update, (observe, auto_quit));
    app.run();
}

#[derive(Component)]
struct TranslationPlatform;

#[derive(Component)]
struct RotationPlatform;

#[derive(Component)]
struct CrushPlatform;

#[derive(Component)]
struct TranslationPassenger;

#[derive(Component)]
struct RotationPassenger;

#[derive(Component)]
struct CrushPassenger;

#[derive(Resource)]
struct GateClock(Instant);

#[derive(Clone, Copy)]
struct Sample {
    translation_relative: Vec3,
    rotation_relative: Vec3,
    world_heading: Vec3,
}

#[derive(Resource, Default)]
struct GateState {
    translation: Option<Sample>,
    rotation: Option<Sample>,
    done: bool,
}

fn setup(
    mut commands: Commands,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());

    commands.spawn((
        Name::new("Gate:translation-platform"),
        TranslationPlatform,
        RigidBody::Kinematic,
        Collider::cuboid(20.0, 0.5, 5.0),
        LinearVelocity(Vec3::X * 0.4),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    commands.spawn((
        Name::new("Gate:rotation-platform"),
        RotationPlatform,
        RigidBody::Kinematic,
        Collider::cuboid(6.0, 0.5, 6.0),
        AngularVelocity(Vec3::Y * 0.9),
        Transform::from_xyz(0.0, 0.0, -8.0),
    ));
    commands.spawn((
        Name::new("Gate:crush-platform"),
        CrushPlatform,
        RigidBody::Kinematic,
        Collider::cuboid(2.0, 0.5, 4.0),
        LinearVelocity(Vec3::X * 0.4),
        Transform::from_xyz(0.0, 0.0, 8.0),
    ));
    commands.spawn((
        Name::new("Gate:crush-wall"),
        RigidBody::Static,
        Collider::cuboid(0.3, 4.0, 4.0),
        Transform::from_xyz(3.0, 2.0, 8.0),
    ));
    commands.spawn((
        Name::new("Gate:floor"),
        RigidBody::Static,
        Collider::cuboid(80.0, 0.5, 80.0),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));

    let mut translation = AlsCharacterBundle::new(params.clone(), config.clone());
    translation.transform = Transform::from_xyz(0.0, 1.8, 0.0);
    commands.spawn((translation, TranslationPassenger));

    let mut rotation = AlsCharacterBundle::new(params.clone(), config.clone());
    rotation.transform = Transform::from_xyz(1.4, 1.8, -8.0);
    commands.spawn((rotation, RotationPassenger));

    let mut crush = AlsCharacterBundle::new(params, config);
    crush.transform = Transform::from_xyz(1.7, 1.8, 8.0);
    commands.spawn((crush, CrushPassenger));

    commands.insert_resource(GateClock(Instant::now()));
}

fn local_position(platform: &Transform, passenger: &Transform) -> Vec3 {
    platform
        .to_matrix()
        .inverse()
        .transform_point3(passenger.translation)
}

fn heading(transform: &Transform) -> Vec3 {
    transform.rotation.mul_vec3(Vec3::NEG_Z).normalize_or_zero()
}

fn observe(
    clock: Option<Res<GateClock>>,
    mut state: ResMut<GateState>,
    translation_platform: Query<&Transform, With<TranslationPlatform>>,
    rotation_platform: Query<&Transform, With<RotationPlatform>>,
    crush_platform: Query<&Transform, With<CrushPlatform>>,
    translation_passenger: Query<&Transform, With<TranslationPassenger>>,
    rotation_passenger: Query<&Transform, With<RotationPassenger>>,
    crush_passenger: Query<&Transform, With<CrushPassenger>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(clock) = clock else {
        return;
    };
    let elapsed = clock.0.elapsed().as_secs_f32();
    let Ok(translation_platform) = translation_platform.single() else {
        return;
    };
    let Ok(rotation_platform) = rotation_platform.single() else {
        return;
    };
    let Ok(crush_platform) = crush_platform.single() else {
        return;
    };
    let Ok(translation_passenger) = translation_passenger.single() else {
        return;
    };
    let Ok(rotation_passenger) = rotation_passenger.single() else {
        return;
    };
    let Ok(crush_passenger) = crush_passenger.single() else {
        return;
    };

    if elapsed >= 1.5 && state.translation.is_none() && state.rotation.is_none() {
        state.translation = Some(Sample {
            translation_relative: local_position(translation_platform, translation_passenger),
            rotation_relative: Vec3::ZERO,
            world_heading: heading(translation_passenger),
        });
        state.rotation = Some(Sample {
            translation_relative: Vec3::ZERO,
            rotation_relative: local_position(rotation_platform, rotation_passenger),
            world_heading: heading(rotation_passenger),
        });
    }

    if elapsed < 5.0 || state.done {
        return;
    }
    state.done = true;
    let translation = state.translation.unwrap();
    let rotation = state.rotation.unwrap();
    let translation_error = local_position(translation_platform, translation_passenger)
        .distance(translation.translation_relative);
    let rotation_error =
        local_position(rotation_platform, rotation_passenger).distance(rotation.rotation_relative);
    let heading_error = heading(rotation_passenger).distance(rotation.world_heading);
    let crush_relative = local_position(crush_platform, crush_passenger);
    let all_finite = translation_passenger.translation.is_finite()
        && rotation_passenger.translation.is_finite()
        && crush_passenger.translation.is_finite();
    let crush_safe = crush_passenger.translation.y > -0.25
        && crush_passenger.translation.length() < 40.0
        && crush_relative.x.abs() < 1.0
        && crush_relative.z.abs() < 2.0;
    let passed = translation_error < 0.35
        && rotation_error < 0.4
        && heading_error < 0.05
        && all_finite
        && crush_safe;
    info!(
        "[PLATFORMGATE][VERDICT] translation_error={translation_error:.6} rotation_error={rotation_error:.6} heading_error={heading_error:.6} crush=({:.3},{:.3},{:.3}) crush_relative=({:.3},{:.3},{:.3}) finite={} -> {}",
        crush_passenger.translation.x,
        crush_passenger.translation.y,
        crush_passenger.translation.z,
        crush_relative.x,
        crush_relative.y,
        crush_relative.z,
        all_finite,
        if passed { "PASS" } else { "FAIL" }
    );
    exit.write(if passed {
        AppExit::Success
    } else {
        AppExit::Error(std::num::NonZeroU8::new(31).unwrap())
    });
}

fn auto_quit(clock: Option<Res<GateClock>>, mut exit: MessageWriter<AppExit>) {
    if clock.is_some_and(|clock| clock.0.elapsed() >= Duration::from_secs(12)) {
        error!("[PLATFORMGATE][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(std::num::NonZeroU8::new(32).unwrap()));
    }
}
