use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

use als_core::character::{
    ALS_CHARACTER_CAPSULE_HALF_HEIGHT, AlsCharacterBundle, AlsCharacterPlugin, AlsCrushRecovery,
    AlsCrushRecoveryFloor,
};
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
    .add_systems(
        FixedPostUpdate,
        vertical_crush_motion.in_set(PhysicsSystems::First),
    )
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
struct VerticalCrushPlatform;

#[derive(Component)]
struct VerticalCrushTarget(f32);

#[derive(Component)]
struct TranslationPassenger;

#[derive(Component)]
struct RotationPassenger;

#[derive(Component)]
struct CrushPassenger;

#[derive(Component)]
struct VerticalCrushPassenger;

#[derive(Resource)]
struct GateClock(Instant);

#[derive(Clone, Copy)]
struct Sample {
    translation_relative: Vec3,
    rotation_relative: Vec3,
    world_heading: Vec3,
}

#[derive(Resource)]
struct GateState {
    translation: Option<Sample>,
    rotation: Option<Sample>,
    min_vertical_floor_bottom: f32,
    max_vertical_overlap: f32,
    max_vertical_target_error: f32,
    done: bool,
}

impl Default for GateState {
    fn default() -> Self {
        Self {
            translation: None,
            rotation: None,
            min_vertical_floor_bottom: f32::INFINITY,
            max_vertical_overlap: f32::NEG_INFINITY,
            max_vertical_target_error: 0.0,
            done: false,
        }
    }
}

const FLOOR_TOP_Y: f32 = 0.0;
const VERTICAL_PLATFORM_HIGH_Y: f32 = 2.2;
const VERTICAL_PLATFORM_LOW_Y: f32 = 1.1;
const VERTICAL_PLATFORM_HEIGHT: f32 = 0.4;
const VERTICAL_TRAVEL_SECS: f32 = 1.2;

fn vertical_target(seconds: f32) -> f32 {
    let phase = seconds.rem_euclid(VERTICAL_TRAVEL_SECS * 2.0);
    if phase <= VERTICAL_TRAVEL_SECS {
        VERTICAL_PLATFORM_HIGH_Y
            - (VERTICAL_PLATFORM_HIGH_Y - VERTICAL_PLATFORM_LOW_Y) * (phase / VERTICAL_TRAVEL_SECS)
    } else {
        VERTICAL_PLATFORM_LOW_Y
            + (VERTICAL_PLATFORM_HIGH_Y - VERTICAL_PLATFORM_LOW_Y)
                * ((phase - VERTICAL_TRAVEL_SECS) / VERTICAL_TRAVEL_SECS)
    }
}

fn vertical_velocity(seconds: f32) -> f32 {
    let phase = seconds.rem_euclid(VERTICAL_TRAVEL_SECS * 2.0);
    let speed = (VERTICAL_PLATFORM_HIGH_Y - VERTICAL_PLATFORM_LOW_Y) / VERTICAL_TRAVEL_SECS;
    if phase < VERTICAL_TRAVEL_SECS {
        -speed
    } else {
        speed
    }
}

fn vertical_crush_motion(
    time: Res<Time<Physics>>,
    mut platforms: Query<
        (&mut LinearVelocity, &mut VerticalCrushTarget),
        With<VerticalCrushPlatform>,
    >,
) {
    let seconds = time.elapsed_secs();
    let target = vertical_target(seconds);
    let velocity = vertical_velocity(seconds);
    for (mut linear_velocity, mut platform_target) in &mut platforms {
        linear_velocity.0 = Vec3::Y * velocity;
        platform_target.0 = target;
    }
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
        Transform::from_xyz(0.0, 0.25, 0.0),
    ));
    commands.spawn((
        Name::new("Gate:rotation-platform"),
        RotationPlatform,
        RigidBody::Kinematic,
        Collider::cuboid(6.0, 0.5, 6.0),
        AngularVelocity(Vec3::Y * 0.9),
        Transform::from_xyz(0.0, 0.25, -8.0),
    ));
    commands.spawn((
        Name::new("Gate:crush-platform"),
        CrushPlatform,
        RigidBody::Kinematic,
        Collider::cuboid(2.0, 0.5, 2.0),
        LinearVelocity(Vec3::X * 0.4),
        Transform::from_xyz(0.0, 0.25, 8.0),
    ));
    commands.spawn((
        Name::new("Gate:crush-wall-parallel"),
        RigidBody::Static,
        Collider::cuboid(20.0, 4.0, 0.5),
        Transform::from_xyz(0.0, 2.0, 6.75),
    ));
    commands.spawn((
        Name::new("Gate:floor"),
        RigidBody::Static,
        Collider::cuboid(80.0, 1.0, 80.0),
        AlsCrushRecoveryFloor {
            half_extents: Vec3::new(40.0, 0.5, 40.0),
        },
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    commands.spawn((
        Name::new("Gate:vertical-crush-platform"),
        VerticalCrushPlatform,
        VerticalCrushTarget(VERTICAL_PLATFORM_HIGH_Y),
        RigidBody::Kinematic,
        Collider::cuboid(8.0, VERTICAL_PLATFORM_HEIGHT, 8.0),
        LinearVelocity::default(),
        Transform::from_xyz(0.0, VERTICAL_PLATFORM_HIGH_Y, -16.0),
    ));

    let mut translation = AlsCharacterBundle::new(params.clone(), config.clone());
    translation.transform = Transform::from_xyz(0.0, 1.4, 0.0);
    commands.spawn((translation, TranslationPassenger));

    let mut rotation = AlsCharacterBundle::new(params.clone(), config.clone());
    rotation.transform = Transform::from_xyz(1.4, 1.4, -8.0);
    commands.spawn((rotation, RotationPassenger));

    let mut crush = AlsCharacterBundle::new(params.clone(), config.clone());
    crush.transform = Transform::from_xyz(0.0, ALS_CHARACTER_CAPSULE_HALF_HEIGHT, 7.0);
    commands.spawn((crush, CrushPassenger));

    let mut vertical_crush = AlsCharacterBundle::new(params, config);
    vertical_crush.transform = Transform::from_xyz(0.0, ALS_CHARACTER_CAPSULE_HALF_HEIGHT, -16.0);
    commands.spawn((vertical_crush, VerticalCrushPassenger));

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
    vertical_platform: Query<(&Transform, &VerticalCrushTarget), With<VerticalCrushPlatform>>,
    translation_passenger: Query<&Transform, With<TranslationPassenger>>,
    rotation_passenger: Query<&Transform, With<RotationPassenger>>,
    crush_passenger: Query<&Transform, With<CrushPassenger>>,
    vertical_passenger: Query<(&Transform, &AlsCrushRecovery), With<VerticalCrushPassenger>>,
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
    let Ok((vertical_platform, vertical_target)) = vertical_platform.single() else {
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
    let Ok((vertical_passenger, vertical_recovery)) = vertical_passenger.single() else {
        return;
    };

    let vertical_capsule_bottom =
        vertical_passenger.translation.y - ALS_CHARACTER_CAPSULE_HALF_HEIGHT;
    let vertical_platform_bottom = vertical_platform.translation.y - VERTICAL_PLATFORM_HEIGHT * 0.5;
    let vertical_overlap = (vertical_passenger.translation.y + ALS_CHARACTER_CAPSULE_HALF_HEIGHT
        - vertical_platform_bottom)
        .max(0.0);
    state.min_vertical_floor_bottom = state.min_vertical_floor_bottom.min(vertical_capsule_bottom);
    state.max_vertical_overlap = state.max_vertical_overlap.max(vertical_overlap);
    state.max_vertical_target_error = state
        .max_vertical_target_error
        .max((vertical_platform.translation.y - vertical_target.0).abs());

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
    let vertical_relative = local_position(vertical_platform, vertical_passenger);
    let all_finite = translation_passenger.translation.is_finite()
        && rotation_passenger.translation.is_finite()
        && crush_passenger.translation.is_finite()
        && vertical_passenger.translation.is_finite();
    let crush_safe = crush_passenger.translation.y > -0.25
        && crush_passenger.translation.length() < 40.0
        && crush_relative.x.abs() < 1.0
        && crush_relative.z.abs() < 1.25;
    let vertical_safe = state.min_vertical_floor_bottom >= FLOOR_TOP_Y - 0.03
        && state.max_vertical_overlap > 0.25
        && state.max_vertical_target_error < 0.15
        && vertical_recovery.corrections > 0
        && vertical_recovery.max_penetration > 0.0
        && vertical_passenger.translation.length() < 40.0
        && vertical_relative.x.abs() < 3.0
        && vertical_relative.z.abs() < 3.0;
    let passed = translation_error < 0.35
        && rotation_error < 0.4
        && heading_error < 0.05
        && all_finite
        && crush_safe
        && vertical_safe;
    info!(
        "[PLATFORMGATE][VERDICT] translation_error={translation_error:.6} rotation_error={rotation_error:.6} heading_error={heading_error:.6} crush=({:.3},{:.3},{:.3}) crush_relative=({:.3},{:.3},{:.3}) vertical_target={:.3} vertical_resolved={:.3} vertical_capsule_bottom={vertical_capsule_bottom:.3} vertical_min_floor_bottom={:.3} vertical_overlap={vertical_overlap:.3} vertical_max_overlap={:.3} vertical_target_error={:.6} vertical_recovery_count={} vertical_recovery_depth={:.6} vertical_relative=({:.3},{:.3},{:.3}) finite={} -> {}",
        crush_passenger.translation.x,
        crush_passenger.translation.y,
        crush_passenger.translation.z,
        crush_relative.x,
        crush_relative.y,
        crush_relative.z,
        vertical_target.0,
        vertical_platform.translation.y,
        state.min_vertical_floor_bottom,
        state.max_vertical_overlap,
        state.max_vertical_target_error,
        vertical_recovery.corrections,
        vertical_recovery.max_penetration,
        vertical_relative.x,
        vertical_relative.y,
        vertical_relative.z,
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
