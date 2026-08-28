use std::num::NonZeroU8;

use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
use bevy::asset::AssetPlugin;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::time::{Real, Virtual};

use als_camera::{
    AlsCameraCurve, AlsCameraCurveValues, AlsCameraDebugInfo, AlsCameraFieldOfView,
    AlsCameraFovOffset, AlsCameraPlugin, AlsCameraRig, AlsCameraRuntime, AlsCameraSet,
    AlsCameraShakeRequest, AlsCameraSocketLocations, AlsCameraTarget, CAMERA_CURVE_NAMES,
};
use als_core::input_request::AlsCameraLeftShoulder;
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
    phase: u8,
    frames: u32,
    wall: Option<Entity>,
    reduced_ratio: f32,
    shake_seen: bool,
    dilation_checked: bool,
    dilation_frames: u8,
}

fn base_curves() -> AlsCameraCurveValues {
    AlsCameraCurveValues::from_pairs([
        (AlsCameraCurve::CameraOffsetX, -320.0),
        (AlsCameraCurve::CameraOffsetY, 0.0),
        (AlsCameraCurve::CameraOffsetZ, 50.0),
        (AlsCameraCurve::FovOffset, 12.0),
        (AlsCameraCurve::PivotOffsetX, 8.0),
        (AlsCameraCurve::PivotOffsetY, 4.0),
        (AlsCameraCurve::PivotOffsetZ, 12.0),
        (AlsCameraCurve::LocationLagX, 0.2),
        (AlsCameraCurve::LocationLagY, 0.2),
        (AlsCameraCurve::LocationLagZ, 0.2),
        (AlsCameraCurve::RotationLag, 0.2),
        (AlsCameraCurve::FirstPersonOverride, 0.0),
        (AlsCameraCurve::TraceOverride, 0.0),
    ])
}

fn sockets(offset: Vec3) -> AlsCameraSocketLocations {
    AlsCameraSocketLocations {
        root: offset,
        head: offset + Vec3::Y * 1.7,
        first_person: offset + Vec3::new(0.04, 1.7, 0.14),
        trace_shoulder_right: offset + Vec3::new(0.05, 1.9, 0.0),
        trace_shoulder_left: offset + Vec3::new(0.05, 1.5, 0.0),
        mesh_rotation: Quat::IDENTITY,
        mesh_scale: 1.0,
        valid: true,
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        AlsCameraTarget,
        AlsViewYaw(0.0),
        AlsViewPitch(0.0),
        RotationMode::ViewDirection,
        AlsCameraFovOffset::default(),
        base_curves(),
        sockets(Vec3::ZERO),
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
}

fn projection_degrees(projection: &Projection) -> f32 {
    match projection {
        Projection::Perspective(value) => value.fov.to_degrees(),
        _ => f32::NAN,
    }
}

fn curve_contract(debug: &AlsCameraDebugInfo) -> bool {
    let expected = base_curves();
    CAMERA_CURVE_NAMES
        .iter()
        .enumerate()
        .all(|(index, name)| (debug.curve_value(name) - expected.values[index]).abs() < 1e-4)
}

#[allow(clippy::type_complexity)]
fn verify(
    mut commands: Commands,
    mut state: ResMut<GateState>,
    mut target: Query<
        (
            Entity,
            &mut AlsCameraCurveValues,
            &mut AlsCameraSocketLocations,
            Has<AlsCameraLeftShoulder>,
        ),
        With<AlsCameraTarget>,
    >,
    camera: Query<(&Transform, &Projection, &AlsCameraDebugInfo), With<AlsCameraRig>>,
    real_time: Res<Time<Real>>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut shake_requests: MessageWriter<AlsCameraShakeRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    state.frames += 1;
    let Ok((target_entity, mut curves, mut socket_locations, left_shoulder)) = target.single_mut()
    else {
        return;
    };
    let Ok((camera_transform, projection, debug)) = camera.single() else {
        return;
    };
    if !debug.valid {
        return;
    }
    match state.phase {
        0 => {
            let curve_values = curve_contract(debug);
            let offsets = camera_transform.translation.z
                > debug.pivot.unwrap_or(Vec3::ZERO).z + 2.5
                && debug
                    .pivot
                    .unwrap_or(Vec3::ZERO)
                    .distance(debug.pivot_lag.unwrap_or(Vec3::ZERO))
                    > 0.01;
            let fov = (debug.field_of_view_deg - 102.0).abs() < 0.01
                && (projection_degrees(projection) - 102.0).abs() < 0.01;
            if curve_values && offsets && fov {
                let wall = commands
                    .spawn((
                        RigidBody::Static,
                        Collider::cuboid(4.0, 4.0, 0.1),
                        Transform::from_xyz(0.0, 1.0, 1.25),
                    ))
                    .id();
                state.wall = Some(wall);
                state.phase = 1;
            }
        }
        1 => {
            if debug.trace_blocking_hit && debug.trace_distance_ratio < 0.95 {
                state.reduced_ratio = debug.trace_distance_ratio;
                if let Some(wall) = state.wall.take() {
                    commands.entity(wall).despawn();
                }
                state.phase = 2;
            }
        }
        2 => {
            if debug.trace_distance_ratio > state.reduced_ratio + 0.02 {
                curves.set(AlsCameraCurve::TraceOverride, 1.0);
                state.phase = 3;
            }
        }
        3 => {
            let override_start = socket_locations.root.lerp(socket_locations.head, 0.5)
                + Vec3::new(0.08, 0.12, 0.04)
                + Vec3::Y * 0.4;
            if debug.trace_start.distance(override_start) < 0.05 {
                curves.set(AlsCameraCurve::TraceOverride, 0.0);
                curves.set(AlsCameraCurve::FirstPersonOverride, 0.5);
                state.phase = 4;
            }
        }
        4 => {
            let partial = (debug.first_person_override - 0.5).abs() < 1e-4
                && (debug.field_of_view_deg - 102.0).abs() < 0.01
                && camera_transform
                    .translation
                    .distance(socket_locations.first_person)
                    > 0.1;
            if partial {
                curves.set(AlsCameraCurve::FirstPersonOverride, 1.0);
                state.phase = 5;
            }
        }
        5 => {
            let full = (debug.first_person_override - 1.0).abs() < 1e-4
                && (debug.field_of_view_deg - 90.0).abs() < 0.01
                && camera_transform
                    .translation
                    .distance(socket_locations.first_person)
                    < 0.01;
            if full {
                curves.set(AlsCameraCurve::FirstPersonOverride, 0.0);
                *socket_locations = sockets(Vec3::new(3.0, 0.0, 0.0));
                state.phase = 6;
            }
        }
        6 => {
            let target_pivot = (socket_locations.root + socket_locations.head) * 0.5;
            if debug
                .pivot_lag
                .is_some_and(|location| location.distance(target_pivot) < 0.01)
            {
                virtual_time.set_relative_speed(0.15);
                state.phase = 7;
            }
        }
        7 => {
            state.dilation_frames += 1;
            if state.dilation_frames >= 3 && real_time.delta_secs() > 1e-5 {
                let ratio = debug.unscaled_delta_seconds / real_time.delta_secs();
                state.dilation_checked = (ratio - 1.0).abs() < 0.2;
                virtual_time.set_relative_speed(1.0);
                shake_requests.write(AlsCameraShakeRequest::sprint(target_entity, 2.0));
                state.phase = 8;
            }
        }
        8 => {
            let within_source_amplitude = debug.shake_location_offset.abs().max_element() <= 0.0201
                && debug.shake_rotation_degrees.abs().max_element() <= 0.2001
                && (debug.field_of_view_deg - 102.0).abs() < 0.01;
            state.shake_seen |= within_source_amplitude
                && (debug.shake_location_offset.length() > 1e-5
                    || debug.shake_rotation_degrees.length() > 1e-5);
            if state.frames > 70 && state.shake_seen && state.dilation_checked {
                commands.entity(target_entity).insert(AlsCameraLeftShoulder);
                state.phase = 9;
            }
        }
        9 => {
            let shoulder = left_shoulder
                && debug
                    .trace_start
                    .distance(socket_locations.trace_shoulder_left)
                    < 0.01;
            let pass = shoulder
                && !debug.trace_blocking_hit
                && debug.trace_distance_ratio > state.reduced_ratio
                && state.shake_seen
                && state.dilation_checked;
            println!(
                "[BCHAR20][VERDICT] curves=true offsets=true trace=true trace_override=true partial_first_person=true full_first_person=true teleport=true unscaled_time={} shake={} left_shoulder={} fov_no_shake=true -> {}",
                state.dilation_checked,
                state.shake_seen,
                shoulder,
                if pass { "PASS" } else { "FAIL" }
            );
            exit.write(if pass {
                AppExit::Success
            } else {
                AppExit::Error(NonZeroU8::new(20).unwrap())
            });
            state.phase = 10;
        }
        _ => {}
    }
    if state.frames > 900 && state.phase < 10 {
        eprintln!(
            "[BCHAR20][VERDICT] phase={} trace_ratio={:.4} fov={:.3} first_person={:.3} shake=({:.5},{:.5},{:.5}) -> FAIL",
            state.phase,
            debug.trace_distance_ratio,
            debug.field_of_view_deg,
            debug.first_person_override,
            debug.shake_location_offset.x,
            debug.shake_location_offset.y,
            debug.shake_location_offset.z,
        );
        exit.write(AppExit::Error(NonZeroU8::new(20).unwrap()));
    }
}
