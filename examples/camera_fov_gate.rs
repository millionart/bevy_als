use std::num::NonZeroU8;

use avian3d::prelude::PhysicsPlugins;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;

use als_camera::{
    AlsCameraDebugInfo, AlsCameraFieldOfView, AlsCameraFovOffset, AlsCameraPlugin, AlsCameraRig,
    AlsCameraRuntime, AlsCameraSet, AlsCameraSocketLocations, AlsCameraTarget,
    horizontal_fov_degrees_from_vertical_radians, vertical_fov_radians_from_horizontal_degrees,
};
use als_core::input_request::AlsFirstPersonView;
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
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, verify_camera_fov.after(AlsCameraSet))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        AlsCameraTarget,
        AlsViewYaw(0.0),
        AlsViewPitch(0.0),
        AlsCameraFovOffset(12.0),
        AlsCameraSocketLocations {
            root: Vec3::ZERO,
            head: Vec3::Y,
            first_person: Vec3::new(0.0, 1.6, 0.0),
            trace_shoulder_right: Vec3::new(0.0, 1.6, 0.2),
            trace_shoulder_left: Vec3::new(0.0, 1.6, -0.2),
            mesh_rotation: Quat::IDENTITY,
            mesh_scale: 1.0,
            valid: true,
        },
        RotationMode::VelocityDirection,
        Transform::IDENTITY,
    ));
    commands.spawn((
        Camera3d::default(),
        AlsCameraRig::default(),
        AlsCameraFieldOfView::default(),
        AlsCameraDebugInfo::default(),
        AlsCameraRuntime::default(),
        Transform::from_xyz(0.0, 1.0, 4.0),
    ));
}

fn fov_from_projection(projection: &Projection) -> Option<f32> {
    match projection {
        Projection::Perspective(perspective) => Some(horizontal_fov_degrees_from_vertical_radians(
            perspective.fov,
            perspective.aspect_ratio,
        )),
        _ => None,
    }
}

fn projection_vertical_degrees(projection: &Projection) -> Option<f32> {
    match projection {
        Projection::Perspective(perspective) => Some(perspective.fov.to_degrees()),
        _ => None,
    }
}

fn projection_aspect_ratio(projection: &Projection) -> Option<f32> {
    match projection {
        Projection::Perspective(perspective) => Some(perspective.aspect_ratio),
        _ => None,
    }
}

fn verify_camera_fov(
    mut phase: Local<u8>,
    mut frames: Local<u32>,
    mut target: Query<
        (Entity, &mut AlsCameraFovOffset, Has<AlsFirstPersonView>),
        With<AlsCameraTarget>,
    >,
    cameras: Query<(&Projection, &AlsCameraFieldOfView, &AlsCameraDebugInfo), With<AlsCameraRig>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    *frames += 1;
    let Ok((target_entity, mut offset, first_person)) = target.single_mut() else {
        return;
    };
    let Ok((projection, field_of_view, debug)) = cameras.single() else {
        return;
    };
    let Some(projected) = fov_from_projection(projection) else {
        return;
    };
    let Some(projected_vertical) = projection_vertical_degrees(projection) else {
        return;
    };
    let Some(aspect_ratio) = projection_aspect_ratio(projection) else {
        return;
    };
    let projection_matches_field = (projected_vertical
        - vertical_fov_radians_from_horizontal_degrees(field_of_view.0, aspect_ratio).to_degrees())
    .abs()
        < 1e-4;
    match *phase {
        0 => {
            let pass = (field_of_view.0 - 102.0).abs() < 1e-4
                && (projected - 102.0).abs() < 1e-4
                && projection_matches_field
                && (debug.field_of_view_deg - 102.0).abs() < 1e-4
                && (debug.curve_value("FovOffset") - 12.0).abs() < 1e-4;
            if !pass {
                return;
            }
            commands.entity(target_entity).insert(AlsFirstPersonView);
            *phase = 1;
        }
        1 => {
            if !first_person
                || (field_of_view.0 - 90.0).abs() >= 1e-4
                || (projected - 90.0).abs() >= 1e-4
                || !projection_matches_field
                || (aspect_ratio - 16.0 / 9.0).abs() >= 1e-4
                || (projected_vertical - 58.7155).abs() >= 1e-3
            {
                return;
            }
            println!(
                "[BF102.14] horizontal=90.000 vertical={projected_vertical:.4} aspect={aspect_ratio:.4}"
            );
            commands
                .entity(target_entity)
                .remove::<AlsFirstPersonView>();
            offset.0 = 500.0;
            *phase = 2;
        }
        _ => {
            let pass = !first_person
                && (field_of_view.0 - 175.0).abs() < 1e-4
                && (projected - 175.0).abs() < 1e-4
                && projection_matches_field
                && (debug.field_of_view_deg - 175.0).abs() < 1e-4;
            if pass {
                println!(
                    "[BCHAR10][VERDICT] third_person_base=90 fov_offset=12 projected=102 first_person_ignores_offset=90 clamp_max=175 aspect={aspect_ratio:.4} vertical={projected_vertical:.4} -> PASS"
                );
                exit.write(AppExit::Success);
            } else if *frames >= 240 {
                eprintln!(
                    "[BCHAR10][VERDICT] phase=clamp projected={projected:.3} field={:.3} debug={:.3} -> FAIL",
                    field_of_view.0, debug.field_of_view_deg
                );
                exit.write(AppExit::Error(NonZeroU8::new(17).unwrap()));
            }
        }
    }
}
