use std::num::NonZeroU8;

use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
use bevy::asset::AssetPlugin;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use als_camera::{
    AlsCameraCurve, AlsCameraCurveValues, AlsCameraDebugInfo, AlsCameraFieldOfView,
    AlsCameraFovOffset, AlsCameraParams, AlsCameraPlugin, AlsCameraRig, AlsCameraRuntime,
    AlsCameraSet, AlsCameraSocketLocations, AlsCameraTarget,
};
use als_core::motion::AlsViewYaw;
use als_core::state::RotationMode;
use als_core::view::AlsViewPitch;

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
        .add_systems(PostUpdate, verify.after(AlsCameraSet))
        .run();
}

#[derive(Resource, Default)]
struct GateState {
    frames: u32,
    finished: bool,
}

fn params() -> AlsCameraParams {
    AlsCameraParams {
        ignore_time_dilation: true,
        teleport_distance_threshold: 0.0,
        trace_radius: 0.15,
        trace_override_offset: Vec3::ZERO,
        trace_distance_smoothing: false,
        trace_distance_smoothing_half_life: 0.2,
        third_person_field_of_view_deg: 90.0,
        first_person_field_of_view_deg: 90.0,
        field_of_view_min_deg: 5.0,
        field_of_view_max_deg: 175.0,
    }
}

fn curves() -> AlsCameraCurveValues {
    AlsCameraCurveValues::from_pairs([
        (AlsCameraCurve::CameraOffsetX, -200.0),
        (AlsCameraCurve::CameraOffsetY, 0.0),
        (AlsCameraCurve::CameraOffsetZ, 0.0),
        (AlsCameraCurve::FovOffset, 0.0),
        (AlsCameraCurve::PivotOffsetX, 0.0),
        (AlsCameraCurve::PivotOffsetY, 0.0),
        (AlsCameraCurve::PivotOffsetZ, 0.0),
        (AlsCameraCurve::LocationLagX, 0.0),
        (AlsCameraCurve::LocationLagY, 0.0),
        (AlsCameraCurve::LocationLagZ, 0.0),
        (AlsCameraCurve::RotationLag, 0.0),
        (AlsCameraCurve::FirstPersonOverride, 0.0),
        (AlsCameraCurve::TraceOverride, 0.0),
    ])
}

fn sockets() -> AlsCameraSocketLocations {
    AlsCameraSocketLocations {
        root: Vec3::ZERO,
        head: Vec3::ZERO,
        first_person: Vec3::ZERO,
        trace_shoulder_right: Vec3::ZERO,
        trace_shoulder_left: Vec3::ZERO,
        mesh_rotation: Quat::IDENTITY,
        mesh_scale: 1.0,
        valid: true,
    }
}

fn setup(mut commands: Commands, mut params_assets: ResMut<Assets<AlsCameraParams>>) {
    let _ = params_assets.add(params());
    commands.spawn((
        AlsCameraTarget,
        AlsViewYaw(0.0),
        AlsViewPitch(0.0),
        RotationMode::ViewDirection,
        AlsCameraFovOffset::default(),
        curves(),
        sockets(),
        Transform::IDENTITY,
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::IDENTITY,
        AlsCameraRig::default(),
        AlsCameraRuntime::default(),
        AlsCameraFieldOfView::default(),
        AlsCameraDebugInfo::default(),
    ));
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(0.05, 0.6, 0.6),
        Transform::from_xyz(-0.1, 0.0, 0.0),
    ));
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(0.6, 0.05, 0.6),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
}

fn verify(
    mut state: ResMut<GateState>,
    camera: Query<&AlsCameraDebugInfo, With<AlsCameraRig>>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.finished {
        return;
    }
    state.frames += 1;
    let Ok(debug) = camera.single() else {
        return;
    };
    if debug.valid && debug.trace_adjustment_count >= 2 {
        let pass = debug.trace_adjusted_start
            && debug.trace_adjustment.x > 0.01
            && debug.trace_adjustment.y > 0.01
            && debug.trace_start.x > 0.01
            && debug.trace_start.y > 0.01
            && debug.trace_adjustment_resolved;
        println!(
            "[BF102.56][GATE] count={} adjustment=({:.3},{:.3},{:.3}) start=({:.3},{:.3},{:.3}) resolved={} blocking={} -> {}",
            debug.trace_adjustment_count,
            debug.trace_adjustment.x,
            debug.trace_adjustment.y,
            debug.trace_adjustment.z,
            debug.trace_start.x,
            debug.trace_start.y,
            debug.trace_start.z,
            debug.trace_adjustment_resolved,
            debug.trace_blocking_hit,
            if pass { "PASS" } else { "FAIL" },
        );
        state.finished = true;
        exit.write(if pass {
            AppExit::Success
        } else {
            AppExit::Error(NonZeroU8::new(56).unwrap())
        });
    } else if state.frames >= 300 {
        println!(
            "[BF102.56][GATE] count={} adjustment=({:.3},{:.3},{:.3}) start=({:.3},{:.3},{:.3}) resolved={} blocking={} -> FAIL",
            debug.trace_adjustment_count,
            debug.trace_adjustment.x,
            debug.trace_adjustment.y,
            debug.trace_adjustment.z,
            debug.trace_start.x,
            debug.trace_start.y,
            debug.trace_start.z,
            debug.trace_adjustment_resolved,
            debug.trace_blocking_hit,
        );
        state.finished = true;
        exit.write(AppExit::Error(NonZeroU8::new(57).unwrap()));
    }
}
