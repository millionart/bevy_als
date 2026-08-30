use std::num::NonZeroU8;

use avian3d::prelude::PhysicsPlugins;
use bevy::asset::AssetPlugin;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::time::{Real, Virtual};

use als_camera::{
    AlsCameraCurve, AlsCameraCurveProfileDriver, AlsCameraCurveProfileRuntime,
    AlsCameraCurveProfiles, AlsCameraCurveValues, AlsCameraParams, AlsCameraPlugin,
    AlsCameraTarget, CAMERA_PROFILE_BLEND_DURATION_SECS,
};
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
    real_elapsed: f32,
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
        RotationMode::VelocityDirection,
    ));
}

fn verify(
    mut state: ResMut<GateState>,
    mut target: Query<
        (
            &mut RotationMode,
            &AlsCameraCurveProfileRuntime,
            &AlsCameraCurveValues,
        ),
        With<AlsCameraTarget>,
    >,
    real_time: Res<Time<Real>>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.finished {
        return;
    }
    if real_time.elapsed_secs() >= 5.0 {
        println!(
            "[BF102.54][GATE] profile_elapsed=0.000000 real_elapsed={:.6} virtual_speed={:.3} target_x=nan -> FAIL",
            real_time.elapsed_secs(),
            virtual_time.relative_speed(),
        );
        virtual_time.set_relative_speed(1.0);
        state.finished = true;
        exit.write(AppExit::Error(NonZeroU8::new(54).unwrap()));
        return;
    }
    let Ok((mut rotation_mode, runtime, values)) = target.single_mut() else {
        return;
    };
    match state.phase {
        0 if runtime.initialized => {
            *rotation_mode = RotationMode::Aiming;
            virtual_time.set_relative_speed(0.1);
            state.phase = 1;
        }
        1 => {
            state.real_elapsed += real_time.delta_secs();
            if runtime.elapsed >= CAMERA_PROFILE_BLEND_DURATION_SECS {
                let target_x = values.get(AlsCameraCurve::CameraOffsetX);
                let pass = (target_x + 200.0).abs() <= 1e-4
                    && state.real_elapsed < 1.5
                    && (virtual_time.relative_speed() - 0.1).abs() <= 1e-6;
                println!(
                    "[BF102.54][GATE] profile_elapsed={:.6} real_elapsed={:.6} virtual_speed={:.3} target_x={target_x:.3} -> {}",
                    runtime.elapsed,
                    state.real_elapsed,
                    virtual_time.relative_speed(),
                    if pass { "PASS" } else { "FAIL" },
                );
                virtual_time.set_relative_speed(1.0);
                state.finished = true;
                exit.write(if pass {
                    AppExit::Success
                } else {
                    AppExit::Error(NonZeroU8::new(55).unwrap())
                });
            } else if state.real_elapsed >= 2.0 {
                let target_x = values.get(AlsCameraCurve::CameraOffsetX);
                println!(
                    "[BF102.54][GATE] profile_elapsed={:.6} real_elapsed={:.6} virtual_speed={:.3} target_x={target_x:.3} -> FAIL",
                    runtime.elapsed,
                    state.real_elapsed,
                    virtual_time.relative_speed(),
                );
                virtual_time.set_relative_speed(1.0);
                state.finished = true;
                exit.write(AppExit::Error(NonZeroU8::new(56).unwrap()));
            }
        }
        _ => {}
    }
}
