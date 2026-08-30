use std::num::NonZeroU8;

use avian3d::prelude::PhysicsPlugins;
use bevy::asset::AssetPlugin;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::time::Real;

use als_camera::{
    AlsCameraCurve, AlsCameraCurveProfileDriver, AlsCameraCurveProfileRuntime,
    AlsCameraCurveProfiles, AlsCameraCurveValues, AlsCameraParams, AlsCameraPlugin,
    AlsCameraTarget, CAMERA_PROFILE_BLEND_DURATION_SECS,
};
use als_core::aim::AlsLastFreeRotationMode;
use als_core::input_request::AlsFirstPersonView;
use als_core::state::RotationMode;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            PhysicsPlugins::new(FixedPostUpdate),
            AlsCameraPlugin,
        ))
        .init_resource::<GateState>()
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, verify)
        .run();
}

#[derive(Resource, Default)]
struct GateState {
    phase: u8,
    first_person_x: f32,
    finished: bool,
}

fn params() -> AlsCameraParams {
    AlsCameraParams {
        ignore_time_dilation: true,
        teleport_distance_threshold: 2.0,
        trace_radius: 0.15,
        trace_override_offset: Vec3::Y * 0.4,
        trace_distance_smoothing: true,
        trace_distance_smoothing_half_life: 0.2,
        third_person_field_of_view_deg: 90.0,
        first_person_field_of_view_deg: 90.0,
        field_of_view_min_deg: 5.0,
        field_of_view_max_deg: 175.0,
    }
}

fn profiles() -> AlsCameraCurveProfiles {
    AlsCameraCurveProfiles {
        velocity_direction: [
            -325.0, 0.0, 20.0, 0.0, 0.0, 0.0, 0.0, 0.15, 0.15, 0.05, 0.05, 0.0, 0.0,
        ],
        view_direction: [
            -250.0, 55.0, 35.0, 0.0, 0.0, 0.0, 0.0, 0.15, 0.15, 0.05, 0.05, 0.0, 0.0,
        ],
        aiming: [
            -200.0, 70.0, 20.0, 0.0, 0.0, 0.0, 0.0, 0.05, 0.05, 0.05, 0.05, 0.0, 0.0,
        ],
    }
}

fn setup(
    mut commands: Commands,
    mut params_assets: ResMut<Assets<AlsCameraParams>>,
    mut profiles_assets: ResMut<Assets<AlsCameraCurveProfiles>>,
) {
    let _ = params_assets.add(params());
    let _ = profiles_assets.add(profiles());
    commands.spawn((
        AlsCameraTarget,
        AlsCameraCurveProfileDriver,
        AlsFirstPersonView,
        RotationMode::ViewDirection,
        AlsLastFreeRotationMode(RotationMode::VelocityDirection),
    ));
}

fn verify(
    mut commands: Commands,
    mut state: ResMut<GateState>,
    target: Query<
        (
            Entity,
            &RotationMode,
            &AlsLastFreeRotationMode,
            Has<AlsFirstPersonView>,
            &AlsCameraCurveProfileRuntime,
            &AlsCameraCurveValues,
        ),
        With<AlsCameraTarget>,
    >,
    real_time: Res<Time<Real>>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.finished {
        return;
    }
    if real_time.elapsed_secs() >= 5.0 {
        println!(
            "[BF102.55][GATE] first_person_x={:.3} third_person_x=nan actual=ViewDirection desired=VelocityDirection -> FAIL",
            state.first_person_x,
        );
        state.finished = true;
        exit.write(AppExit::Error(NonZeroU8::new(55).unwrap()));
        return;
    }
    let Ok((entity, actual, desired, first_person, runtime, values)) = target.single() else {
        return;
    };
    match state.phase {
        0 if runtime.initialized => {
            state.first_person_x = values.get(AlsCameraCurve::CameraOffsetX);
            let first_pass = first_person
                && *actual == RotationMode::ViewDirection
                && desired.0 == RotationMode::VelocityDirection
                && (state.first_person_x + 325.0).abs() <= 1e-4;
            if !first_pass {
                println!(
                    "[BF102.55][GATE] first_person_x={:.3} third_person_x=nan actual={actual:?} desired={:?} -> FAIL",
                    state.first_person_x, desired.0,
                );
                state.finished = true;
                exit.write(AppExit::Error(NonZeroU8::new(56).unwrap()));
                return;
            }
            commands.entity(entity).remove::<AlsFirstPersonView>();
            state.phase = 1;
        }
        1 if runtime.elapsed >= CAMERA_PROFILE_BLEND_DURATION_SECS => {
            let third_person_x = values.get(AlsCameraCurve::CameraOffsetX);
            let pass = !first_person
                && *actual == RotationMode::ViewDirection
                && desired.0 == RotationMode::VelocityDirection
                && (third_person_x + 250.0).abs() <= 1e-4;
            println!(
                "[BF102.55][GATE] first_person_x={:.3} third_person_x={third_person_x:.3} actual={actual:?} desired={:?} -> {}",
                state.first_person_x,
                desired.0,
                if pass { "PASS" } else { "FAIL" },
            );
            state.finished = true;
            exit.write(if pass {
                AppExit::Success
            } else {
                AppExit::Error(NonZeroU8::new(57).unwrap())
            });
        }
        _ => {}
    }
}
