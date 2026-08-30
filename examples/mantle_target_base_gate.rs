use als_core::character::AlsCharacter;
use als_core::input::AlsInputCommand;
use als_core::mantle::{AlsMantleState, AlsMantleTargetBase, MantleKind};
use als_core::orientation::world_forward_to_yaw;
use als_core::state::{LocomotionState, Stance};
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use bevy::transform::TransformPlugin;

use bevy_als::mantle_drive::AlsMantleDrivePlugin;

const EPSILON: f32 = 1e-4;

#[derive(Resource, Clone, Copy)]
struct GateEntities {
    character: Entity,
    target: Entity,
}

#[derive(Clone, Copy)]
struct MantleSnapshot {
    active: bool,
    target_base: Option<Entity>,
    start: Vec3,
    target: Vec3,
    yaw_target: f32,
    start_location_base: Vec3,
    target_location_base: Vec3,
    target_rotation_base: Quat,
    target_invalid: bool,
}

fn main() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, AlsMantleDrivePlugin))
        .add_systems(Startup, setup);

    app.update();
    let entities = *app.world().resource::<GateEntities>();
    let initial = mantle_snapshot(&app, entities.character);

    let moved_transform =
        Transform::from_xyz(3.0, 0.0, -2.0).with_rotation(Quat::from_rotation_y(0.75));
    *app.world_mut()
        .get_mut::<Transform>(entities.target)
        .unwrap() = moved_transform;
    app.update();
    app.update();
    let followed = mantle_snapshot(&app, entities.character);
    let expected_start =
        moved_transform.translation + moved_transform.rotation * initial.start_location_base;
    let expected_target =
        moved_transform.translation + moved_transform.rotation * initial.target_location_base;
    let expected_yaw =
        world_forward_to_yaw(moved_transform.rotation * initial.target_rotation_base * Vec3::NEG_Z);
    let target_followed = followed.active
        && followed.target_base == Some(entities.target)
        && vec3_close(followed.start, expected_start)
        && vec3_close(followed.target, expected_target)
        && angle_close(followed.yaw_target, expected_yaw);

    assert!(app.world_mut().despawn(entities.target));
    app.update();
    let invalidated = mantle_snapshot(&app, entities.character);
    let target_loss_handled = !invalidated.active
        && invalidated.target_base == Some(entities.target)
        && invalidated.target_invalid;

    let passed = initial.active
        && initial.target_base == Some(entities.target)
        && target_followed
        && target_loss_handled;
    let verdict = if passed { "PASS" } else { "FAIL" };
    println!(
        "[BF102.20][VERDICT] identity={:?} target_followed={} target_loss_handled={} initial_target={:?} followed_target={:?} invalidated_active={} invalidated={} -> {verdict}",
        initial.target_base,
        target_followed,
        target_loss_handled,
        initial.target,
        followed.target,
        invalidated.active,
        invalidated.target_invalid,
    );
    if !passed {
        std::process::exit(20);
    }
}

fn setup(mut commands: Commands) {
    let target = commands.spawn(Transform::default()).id();
    let mut mantle = AlsMantleState::default();
    mantle.begin_with_target_base(
        MantleKind::High,
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(2.0, 0.0, 0.0),
        1.25,
        0.0,
        0.0,
        AlsMantleTargetBase {
            entity: target,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        },
    );
    let character = commands
        .spawn((
            AlsCharacter,
            Transform::default(),
            LinearVelocity::default(),
            LocomotionState::Grounded,
            Stance::Standing,
            AlsInputCommand::default(),
            mantle,
        ))
        .id();
    commands.insert_resource(GateEntities { character, target });
}

fn mantle_snapshot(app: &App, character: Entity) -> MantleSnapshot {
    let state = app.world().get::<AlsMantleState>(character).unwrap();
    MantleSnapshot {
        active: state.active,
        target_base: state.target_base.map(|base| base.entity),
        start: state.start,
        target: state.target,
        yaw_target: state.yaw_target,
        start_location_base: state.start_location_base,
        target_location_base: state.target_location_base,
        target_rotation_base: state.target_rotation_base,
        target_invalid: state.target_invalid,
    }
}

fn vec3_close(left: Vec3, right: Vec3) -> bool {
    left.distance(right) <= EPSILON
}

fn angle_close(left: f32, right: f32) -> bool {
    let delta = (left - right + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    delta.abs() <= EPSILON
}
