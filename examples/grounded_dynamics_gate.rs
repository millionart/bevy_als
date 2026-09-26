use std::num::NonZeroU8;

use bevy_als::als::character::{AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin};
use bevy_als::als::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use bevy_als::als::dynamics::AlsGroundedDynamics;
use bevy_als::als::gait::AlsGaitAmount;
use bevy_als::als::input::AlsInputCommand;
use bevy_als::als::motion::{AlsInputOverride, AlsMotionPlugin, initial_als_control_scheme_config};
use bevy_als::als::params::{AlsMovementParams, GroundedDynamicsProfile};
use bevy_als::als::state::{LocomotionState, RotationMode, Stance};
use avian3d::prelude::*;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            PhysicsPlugins::new(FixedPostUpdate),
            TnuaAvian3dPlugin::new(FixedPostUpdate),
            TnuaControllerPlugin::<AlsControlScheme>::new(FixedPostUpdate),
            AlsCharacterPlugin,
            AlsMotionPlugin,
        ))
        .init_resource::<GateState>()
        .add_systems(Startup, setup)
        .add_systems(Update, drive_gate)
        .run();
}

#[derive(Resource, Default)]
struct GateState {
    phase: u8,
    started_at: f32,
    normal_profile: bool,
    responsive_profile: bool,
    source_samples: bool,
    start_stop_speed: f32,
    stop_speed: f32,
    max_sample_error: f32,
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
    let params_value = AlsMovementParams::default();
    let params = params_assets.add(params_value.clone());
    let config = config_assets.add(initial_als_control_scheme_config(&params_value));
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_xyz(0.0, 0.925, 0.0);
    commands.spawn((
        bundle,
        AlsInputOverride(AlsInputCommand {
            move_dir: Vec2::Y,
            ..default()
        }),
    ));
    commands.spawn((Camera3d::default(), Transform::from_xyz(3.0, 2.0, 4.0)));
}

#[allow(clippy::type_complexity)]
fn drive_gate(
    time: Res<Time>,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut state: ResMut<GateState>,
    mut character: Query<
        (
            &LocomotionState,
            &Stance,
            &mut RotationMode,
            &AlsGaitAmount,
            &AlsGroundedDynamics,
            &bevy_als::als::character::AlsMovementParamsHandle,
            &LinearVelocity,
            &mut AlsInputOverride,
        ),
        With<AlsCharacter>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((
        locomotion,
        stance,
        mut rotation_mode,
        gait_amount,
        dynamics,
        handle,
        velocity,
        mut input,
    )) = character.single_mut()
    else {
        return;
    };
    let Some(params) = params_assets.get(handle.id()) else {
        return;
    };
    let now = time.elapsed_secs();
    if state.started_at == 0.0 {
        state.started_at = now;
    }
    let elapsed = now - state.started_at;
    let expected = params.grounded_dynamics(*stance, *rotation_mode, gait_amount.0);
    let sample_error = (dynamics.0.acceleration - expected.acceleration)
        .abs()
        .max((dynamics.0.braking_deceleration - expected.braking_deceleration).abs())
        .max((dynamics.0.ground_friction - expected.ground_friction).abs());
    state.max_sample_error = state.max_sample_error.max(sample_error);
    let speed = Vec2::new(velocity.x, velocity.z).length();

    match state.phase {
        0 if elapsed >= 0.35 => {
            state.normal_profile = *locomotion == LocomotionState::Grounded
                && params.grounded_dynamics_profile(*stance, *rotation_mode)
                    == GroundedDynamicsProfile::Normal
                && speed > 0.4
                && sample_error < 1e-5;
            *rotation_mode = RotationMode::VelocityDirection;
            state.started_at = now;
            state.phase = 1;
        }
        1 if elapsed >= 0.35 => {
            state.responsive_profile = *locomotion == LocomotionState::Grounded
                && params.grounded_dynamics_profile(*stance, *rotation_mode)
                    == GroundedDynamicsProfile::Responsive
                && speed > 0.4
                && sample_error < 1e-5;
            state.start_stop_speed = speed;
            input.0.move_dir = Vec2::ZERO;
            state.started_at = now;
            state.phase = 2;
        }
        2 if elapsed >= 0.20 => {
            state.stop_speed = speed;
            let normal = params
                .acceleration
                .grounded
                .sample(GroundedDynamicsProfile::Normal, 2.5);
            let responsive = params
                .acceleration
                .grounded
                .sample(GroundedDynamicsProfile::Responsive, 2.5);
            let sluggish = params
                .acceleration
                .grounded
                .sample(GroundedDynamicsProfile::Sluggish, 2.5);
            state.source_samples = (normal.acceleration - 9.197_034).abs() < 1e-4
                && (normal.braking_deceleration - 8.75).abs() < 1e-6
                && (normal.ground_friction - 2.25).abs() < 1e-6
                && (responsive.acceleration - 7.670_757).abs() < 1e-4
                && (responsive.braking_deceleration - 10.605_449).abs() < 1e-4
                && (responsive.ground_friction - 7.5).abs() < 1e-6
                && (sluggish.acceleration - 2.445_746).abs() < 1e-4
                && (sluggish.braking_deceleration - 4.518_285).abs() < 1e-4
                && (sluggish.ground_friction - 1.734_373).abs() < 1e-4;
            let pass = state.normal_profile
                && state.responsive_profile
                && state.source_samples
                && state.stop_speed < state.start_stop_speed
                && state.max_sample_error < 1e-5;
            println!(
                "[BCHAR17D][VERDICT] normal={} responsive={} source_samples={} start_stop_speed={:.6} stop_speed={:.6} max_sample_error={:.6} -> {}",
                state.normal_profile,
                state.responsive_profile,
                state.source_samples,
                state.start_stop_speed,
                state.stop_speed,
                state.max_sample_error,
                if pass { "PASS" } else { "FAIL" },
            );
            exit.write(if pass {
                AppExit::Success
            } else {
                AppExit::Error(NonZeroU8::new(17).unwrap())
            });
            state.phase = 3;
        }
        _ => {}
    }
}
