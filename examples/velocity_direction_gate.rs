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
    AlsInputOverride, AlsRotationResponseSet, AlsRotationResponseState, AlsViewYaw,
    wrap_yaw_radians,
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
    reverse: Entity,
    slide: Entity,
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
        .add_systems(Update, (verify, timeout))
        .add_systems(
            FixedPostUpdate,
            pin_reverse_velocity
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
        Collider::cuboid(20.0, 0.2, 20.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(0.1, 2.0, 10.0),
        Transform::from_xyz(1.5, 2.0, 0.0),
    ));
    let reverse = spawn_character(
        &mut commands,
        params_assets.add(AlsMovementParams::default()),
        config_assets.add(AlsControlSchemeConfig::default()),
        Vec3::new(-3.0, 0.92, 3.0),
        Vec2::NEG_Y,
    );
    let slide = spawn_character(
        &mut commands,
        params_assets.add(AlsMovementParams::default()),
        config_assets.add(AlsControlSchemeConfig::default()),
        Vec3::new(0.0, 0.92, 3.0),
        Vec2::new(1.0, 1.0),
    );
    commands.insert_resource(Probe { reverse, slide });
}

fn spawn_character(
    commands: &mut Commands,
    params: Handle<AlsMovementParams>,
    config: Handle<AlsControlSchemeConfig>,
    translation: Vec3,
    input: Vec2,
) -> Entity {
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_translation(translation)
        .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
    bundle.gait = Gait::Running;
    bundle.locomotion_state = LocomotionState::Grounded;
    bundle.rotation_mode = RotationMode::VelocityDirection;
    commands
        .spawn((
            bundle,
            AlsViewYaw(0.0),
            AlsInputOverride(AlsInputCommand {
                move_dir: input,
                ..default()
            }),
        ))
        .id()
}

fn pin_reverse_velocity(
    probe: Res<Probe>,
    mut characters: Query<
        (
            &mut LinearVelocity,
            &mut Gait,
            &mut LocomotionState,
            &mut RotationMode,
        ),
        With<AlsCharacter>,
    >,
) {
    let Ok((mut velocity, mut gait, mut locomotion, mut rotation_mode)) =
        characters.get_mut(probe.reverse)
    else {
        return;
    };
    velocity.0 = Vec3::NEG_Z;
    *gait = Gait::Running;
    *locomotion = LocomotionState::Grounded;
    *rotation_mode = RotationMode::VelocityDirection;
}

fn yaw_matches(first: f32, second: f32, tolerance: f32) -> bool {
    wrap_yaw_radians(first - second).abs() <= tolerance
}

fn verify(
    started: Res<Started>,
    probe: Res<Probe>,
    transforms: Query<&Transform, With<AlsCharacter>>,
    velocities: Query<&LinearVelocity, With<AlsCharacter>>,
    responses: Query<&AlsRotationResponseState, With<AlsCharacter>>,
    mut exit: MessageWriter<AppExit>,
) {
    if started.0.elapsed() < Duration::from_secs_f32(2.5) {
        return;
    }
    let (Ok(reverse_transform), Ok(slide_transform), Ok(reverse_velocity), Ok(slide_velocity)) = (
        transforms.get(probe.reverse),
        transforms.get(probe.slide),
        velocities.get(probe.reverse),
        velocities.get(probe.slide),
    ) else {
        return;
    };
    let (Ok(reverse_response), Ok(slide_response)) =
        (responses.get(probe.reverse), responses.get(probe.slide))
    else {
        return;
    };
    let reverse_body_yaw = world_forward_to_yaw(reverse_transform.rotation * Vec3::NEG_Z);
    let slide_body_yaw = world_forward_to_yaw(slide_transform.rotation * Vec3::NEG_Z);
    let reverse_velocity_yaw = reverse_response.velocity_yaw;
    let slide_velocity_yaw = slide_response.velocity_yaw;
    let reverse_input_yaw = reverse_response.input_yaw;
    let slide_input_yaw = slide_response.input_yaw;
    let reverse_passed = reverse_velocity_yaw.is_some_and(|velocity_yaw| {
        reverse_input_yaw.is_some_and(|input_yaw| {
            !yaw_matches(input_yaw, velocity_yaw, 2.5)
                && yaw_matches(reverse_response.target_yaw, velocity_yaw, 0.03)
                && yaw_matches(reverse_body_yaw, velocity_yaw, 0.12)
        })
    });
    let slide_passed = slide_velocity_yaw.is_some_and(|velocity_yaw| {
        slide_input_yaw.is_some_and(|input_yaw| {
            !yaw_matches(input_yaw, velocity_yaw, 0.2)
                && yaw_matches(slide_response.target_yaw, velocity_yaw, 0.03)
                && yaw_matches(slide_body_yaw, velocity_yaw, 0.15)
                && slide_transform.translation.x < 1.35
                && slide_velocity.0.z < -0.1
        })
    });
    if reverse_passed && slide_passed {
        info!(
            "[BF102.12] reverse input_yaw={:.3} velocity=({:.3},{:.3}) velocity_yaw={:.3} target_yaw={:.3} body_yaw={reverse_body_yaw:.3}",
            reverse_input_yaw.unwrap_or_default(),
            reverse_velocity.0.x,
            reverse_velocity.0.z,
            reverse_velocity_yaw.unwrap_or_default(),
            reverse_response.target_yaw,
        );
        info!(
            "[BF102.12] slide position=({:.3},{:.3}) input_yaw={:.3} velocity=({:.3},{:.3}) velocity_yaw={:.3} target_yaw={:.3} body_yaw={slide_body_yaw:.3}",
            slide_transform.translation.x,
            slide_transform.translation.z,
            slide_input_yaw.unwrap_or_default(),
            slide_velocity.0.x,
            slide_velocity.0.z,
            slide_velocity_yaw.unwrap_or_default(),
            slide_response.target_yaw,
        );
        info!("[BF102.12][VERDICT] velocity_direction_uses_post_physics_velocity -> PASS");
        exit.write(AppExit::Success);
    } else {
        error!(
            "[BF102.12][VERDICT] reverse_input={reverse_input_yaw:?} reverse_velocity={reverse_velocity_yaw:?} reverse_target={:.3} reverse_body={reverse_body_yaw:.3} slide_input={slide_input_yaw:?} slide_velocity={slide_velocity_yaw:?} slide_target={:.3} slide_body={slide_body_yaw:.3} slide_position=({:.3},{:.3}) slide_linear=({:.3},{:.3}) -> FAIL",
            reverse_response.target_yaw,
            slide_response.target_yaw,
            slide_transform.translation.x,
            slide_transform.translation.z,
            slide_velocity.0.x,
            slide_velocity.0.z,
        );
        exit.write(AppExit::Error(NonZeroU8::new(12).unwrap()));
    }
}

fn timeout(deadline: Res<Deadline>, mut exit: MessageWriter<AppExit>) {
    if Instant::now() >= deadline.0 {
        error!("[BF102.12][VERDICT] timeout -> FAIL");
        exit.write(AppExit::Error(NonZeroU8::new(13).unwrap()));
    }
}
