use std::num::NonZeroU8;

use avian3d::prelude::*;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy_als::als::air::{AlsAirPlugin, AlsAirState, ground_prediction_amount};
use bevy_als::als::character::{
    AlsCharacter, AlsCharacterBundle, AlsCharacterPlugin, AlsCmcAdapterRuntime,
};
use bevy_als::als::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use bevy_als::als::mantle::yaw_from_direction;
use bevy_als::als::motion::{
    AlsAimingYawConstraint, AlsInputOverride, AlsMotionPlugin, AlsViewYaw,
    initial_als_control_scheme_config, view_relative_move_direction, wrap_yaw_radians,
};
use bevy_als::als::params::AlsMovementParams;
use bevy_als::als::state::{LocomotionState, RotationMode};
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
            AlsAirPlugin,
        ))
        .init_resource::<GateState>()
        .add_systems(Startup, setup)
        .add_systems(Update, run_gate)
        .run();
}

#[derive(Resource)]
struct GateState {
    phase: u8,
    started_at: f32,
    aiming_limit: bool,
    ground_prediction: bool,
    perch: bool,
    min_analog: bool,
    simulation: bool,
    ground_prediction_detail: String,
}

impl Default for GateState {
    fn default() -> Self {
        Self {
            phase: 0,
            started_at: -1.0,
            aiming_limit: false,
            ground_prediction: false,
            perch: false,
            min_analog: false,
            simulation: false,
            ground_prediction_detail: String::new(),
        }
    }
}

fn setup(
    mut commands: Commands,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 0.2, 40.0),
        avian3d::prelude::CollisionLayers::new(
            [
                bevy_als::als::character::AlsCollisionLayer::Default,
                bevy_als::als::character::AlsCollisionLayer::Visibility,
            ],
            avian3d::prelude::LayerMask::ALL,
        ),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    let params_value = AlsMovementParams::default();
    let params = params_assets.add(params_value.clone());
    let mut bundle = AlsCharacterBundle::new(
        params,
        &mut config_assets,
        initial_als_control_scheme_config(&params_value),
    );
    bundle.transform = Transform::from_xyz(0.0, 3.0, 0.0);
    bundle.rotation_mode = RotationMode::Aiming;
    bundle.locomotion_state = LocomotionState::Air;
    commands.spawn((
        bundle,
        LinearVelocity(Vec3::new(0.0, -10.0, 0.0)),
        AlsViewYaw(-std::f32::consts::PI),
        AlsInputOverride::default(),
    ));
    commands.spawn((Camera3d::default(), Transform::from_xyz(3.0, 2.0, 4.0)));
}

#[allow(clippy::type_complexity)]
fn run_gate(
    time: Res<Time>,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut state: ResMut<GateState>,
    mut character: Query<
        (
            &mut Transform,
            &mut LinearVelocity,
            &mut LocomotionState,
            &mut RotationMode,
            &AlsViewYaw,
            &AlsAimingYawConstraint,
            &AlsCmcAdapterRuntime,
            &AlsAirState,
            &bevy_als::als::character::AlsMovementParamsHandle,
            &mut AlsInputOverride,
        ),
        With<AlsCharacter>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((
        mut transform,
        mut velocity,
        mut locomotion,
        mut rotation_mode,
        view_yaw,
        aiming_constraint,
        cmc_runtime,
        air_state,
        params_handle,
        mut input,
    )) = character.single_mut()
    else {
        return;
    };
    let Some(params) = params_assets.get(params_handle.id()) else {
        return;
    };
    let now = time.elapsed_secs();
    if state.started_at < 0.0 {
        state.started_at = now;
    }
    let elapsed = now - state.started_at;
    match state.phase {
        0 if elapsed >= 0.10 => {
            let actor_forward = transform.forward();
            let actor_yaw = yaw_from_direction(*actor_forward);
            let view_yaw_target =
                yaw_from_direction(view_relative_move_direction(Vec2::Y, view_yaw.0));
            let yaw_difference = wrap_yaw_radians(view_yaw_target - actor_yaw)
                .abs()
                .to_degrees();
            state.aiming_limit = yaw_difference <= params.cmc.aiming_yaw_angle_limit_degrees + 0.5
                && aiming_constraint.active_limit_degrees
                    >= params.cmc.aiming_yaw_angle_limit_degrees;
            state.ground_prediction_detail = format!("{:?}", air_state.ground_prediction);
            state.ground_prediction = air_state.ground_prediction.is_some_and(|prediction| {
                prediction.ground_valid
                    && prediction.capsule_radius == 0.3
                    && prediction.capsule_half_height == 0.9
                    && prediction.hit_fraction.is_some_and(|fraction| {
                        (prediction.amount - ground_prediction_amount(fraction)).abs() < 1e-6
                    })
            });
            state.perch = (cmc_runtime.support_radius
                - (params.crouch.standing_capsule_radius - params.cmc.perch_radius_threshold))
                .abs()
                < 1e-6;
            transform.translation = Vec3::new(0.0, 0.925, 0.0);
            velocity.0 = Vec3::ZERO;
            *locomotion = LocomotionState::Grounded;
            *rotation_mode = RotationMode::ViewDirection;
            input.0.move_dir = Vec2::new(0.01, 0.0);
            state.started_at = now;
            state.phase = 1;
        }
        1 if elapsed >= 0.25 => {
            state.min_analog = cmc_runtime.min_analog_speed_applied;
            state.simulation = cmc_runtime.simulation_delta_seconds
                <= params.cmc.max_simulation_time_step + 1e-6
                && cmc_runtime.simulation_substeps > 0
                && cmc_runtime.simulation_substeps <= params.cmc.max_simulation_iterations;
            let pass = state.aiming_limit
                && state.ground_prediction
                && state.perch
                && state.min_analog
                && state.simulation;
            println!(
                "[BCHAR17C][VERDICT] aiming_limit={} ground_prediction={} perch={} min_analog={} simulation={} prediction={} support_radius={:.3} physics_dt={:.6} substeps={} -> {}",
                state.aiming_limit,
                state.ground_prediction,
                state.perch,
                state.min_analog,
                state.simulation,
                state.ground_prediction_detail,
                cmc_runtime.support_radius,
                cmc_runtime.simulation_delta_seconds,
                cmc_runtime.simulation_substeps,
                if pass { "PASS" } else { "FAIL" },
            );
            exit.write(if pass {
                AppExit::Success
            } else {
                AppExit::Error(NonZeroU8::new(18).unwrap())
            });
            state.phase = 2;
        }
        _ => {}
    }
}
