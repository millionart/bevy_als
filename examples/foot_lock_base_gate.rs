use std::num::NonZeroU8;

use als_core::character::AlsCharacter;
use als_core::dynamics::AlsMovementBaseDynamicsState;
use als_core::foot_ik::{
    AlsDynamicTransitionState, AlsFootBones, AlsFootIkRuntime, AlsFootIkTargets, AlsFootLockInput,
    AlsFootLockSettings, AlsFootLockState, AlsFootProbe,
};
use als_core::state::{LocomotionState, Stance};
use bevy::prelude::*;
use bevy::transform::TransformPlugin;

use bevy_als::foot_ik_apply::FootIkApplyPlugin;

const EPSILON: f32 = 1e-4;

#[derive(Resource, Clone, Copy)]
struct GateEntities {
    character: Entity,
    first_base: Entity,
    second_base: Entity,
}

#[derive(Clone, Copy)]
struct LockSnapshot {
    movement_base: Option<Entity>,
    world_location: Vec3,
    world_rotation: Quat,
    base_location: Vec3,
    base_rotation: Quat,
}

fn main() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, FootIkApplyPlugin))
        .add_systems(Startup, setup);

    app.update();
    let entities = *app.world().resource::<GateEntities>();
    let initial = lock_snapshot(&app, entities.character);

    set_transform(
        &mut app,
        entities.first_base,
        Transform::from_xyz(3.0, 0.0, -2.0).with_rotation(Quat::from_rotation_y(0.75)),
    );
    app.update();
    let moved_first = lock_snapshot(&app, entities.character);
    let first_transform = global_transform(&app, entities.first_base);
    let first_followed = moved_first.movement_base == Some(entities.first_base)
        && vec3_close(
            moved_first.world_location,
            first_transform.translation() + first_transform.rotation() * initial.base_location,
        )
        && quat_close(
            moved_first.world_rotation,
            first_transform.rotation() * initial.base_rotation,
        );

    set_movement_base(&mut app, entities.character, Some(entities.second_base));
    app.update();
    let rebased = lock_snapshot(&app, entities.character);
    let rebase_preserved = rebased.movement_base == Some(entities.second_base)
        && vec3_close(rebased.world_location, moved_first.world_location)
        && quat_close(rebased.world_rotation, moved_first.world_rotation);

    set_transform(
        &mut app,
        entities.second_base,
        Transform::from_xyz(-4.0, 0.0, 5.0).with_rotation(Quat::from_rotation_y(-0.6)),
    );
    app.update();
    let moved_second = lock_snapshot(&app, entities.character);
    let second_transform = global_transform(&app, entities.second_base);
    let second_followed = moved_second.movement_base == Some(entities.second_base)
        && vec3_close(
            moved_second.world_location,
            second_transform.translation() + second_transform.rotation() * rebased.base_location,
        )
        && quat_close(
            moved_second.world_rotation,
            second_transform.rotation() * rebased.base_rotation,
        );

    set_movement_base(&mut app, entities.character, None);
    app.update();
    let detached = lock_snapshot(&app, entities.character);
    let detach_preserved = detached.movement_base.is_none()
        && vec3_close(detached.world_location, moved_second.world_location)
        && quat_close(detached.world_rotation, moved_second.world_rotation);

    set_transform(
        &mut app,
        entities.second_base,
        Transform::from_xyz(9.0, 0.0, -3.0).with_rotation(Quat::from_rotation_y(1.1)),
    );
    app.update();
    let after_base_loss = lock_snapshot(&app, entities.character);
    let base_loss_preserved = after_base_loss.movement_base.is_none()
        && vec3_close(after_base_loss.world_location, detached.world_location)
        && quat_close(after_base_loss.world_rotation, detached.world_rotation);

    let passed = initial.movement_base == Some(entities.first_base)
        && first_followed
        && rebase_preserved
        && second_followed
        && detach_preserved
        && base_loss_preserved;
    let verdict = if passed { "PASS" } else { "FAIL" };
    println!(
        "[BF102.19][VERDICT] initial_base={:?} first_followed={} rebase_preserved={} second_followed={} detach_preserved={} base_loss_preserved={} initial={:?} moved_first={:?} rebased={:?} moved_second={:?} detached={:?} after_base_loss={:?} -> {verdict}",
        initial.movement_base,
        first_followed,
        rebase_preserved,
        second_followed,
        detach_preserved,
        base_loss_preserved,
        initial.world_location,
        moved_first.world_location,
        rebased.world_location,
        moved_second.world_location,
        detached.world_location,
        after_base_loss.world_location,
    );
    if !passed {
        std::process::exit(NonZeroU8::new(19).unwrap().get().into());
    }
}

fn setup(mut commands: Commands) {
    let first_base = commands.spawn(Transform::default()).id();
    let second_base = commands
        .spawn(Transform::from_xyz(-1.0, 0.0, 2.0).with_rotation(Quat::from_rotation_y(0.35)))
        .id();
    let character = commands
        .spawn((
            AlsCharacter,
            Transform::default(),
            LocomotionState::Grounded,
            Stance::Standing,
            AlsFootProbe::default(),
            AlsFootIkTargets {
                grounded: true,
                ..default()
            },
            AlsFootLockInput {
                left_ik_amount: 1.0,
                right_ik_amount: 1.0,
                left_lock_amount: 1.0,
                right_lock_amount: 1.0,
                ..default()
            },
            AlsFootLockSettings::default(),
            AlsFootIkRuntime::default(),
            AlsFootLockState::default(),
            AlsDynamicTransitionState::default(),
            AlsMovementBaseDynamicsState {
                entity: Some(first_base),
                ..default()
            },
        ))
        .id();
    let pelvis = commands.spawn(Transform::default()).id();
    commands.entity(character).add_child(pelvis);
    let (left_thigh, left_calf, left_foot) = spawn_leg(&mut commands, pelvis);
    let (right_thigh, right_calf, right_foot) = spawn_leg(&mut commands, pelvis);
    commands.entity(character).insert(AlsFootBones {
        left_thigh,
        left_calf,
        left: left_foot,
        left_target: None,
        right_thigh,
        right_calf,
        right: right_foot,
        right_target: None,
    });
    commands.insert_resource(GateEntities {
        character,
        first_base,
        second_base,
    });
}

fn spawn_leg(commands: &mut Commands, pelvis: Entity) -> (Entity, Entity, Entity) {
    let thigh = commands.spawn(Transform::default()).id();
    let calf = commands.spawn(Transform::default()).id();
    let foot = commands.spawn(Transform::default()).id();
    commands.entity(pelvis).add_child(thigh);
    commands.entity(thigh).add_child(calf);
    commands.entity(calf).add_child(foot);
    (thigh, calf, foot)
}

fn lock_snapshot(app: &App, character: Entity) -> LockSnapshot {
    let state = app.world().get::<AlsFootLockState>(character).unwrap();
    LockSnapshot {
        movement_base: state.left.movement_base.map(|base| base.entity),
        world_location: state.left.lock_location,
        world_rotation: state.left.lock_rotation,
        base_location: state.left.lock_location_movement_base,
        base_rotation: state.left.lock_rotation_movement_base,
    }
}

fn set_transform(app: &mut App, entity: Entity, transform: Transform) {
    *app.world_mut().get_mut::<Transform>(entity).unwrap() = transform;
}

fn set_movement_base(app: &mut App, character: Entity, base: Option<Entity>) {
    app.world_mut()
        .get_mut::<AlsMovementBaseDynamicsState>(character)
        .unwrap()
        .entity = base;
}

fn global_transform(app: &App, entity: Entity) -> GlobalTransform {
    *app.world().get::<GlobalTransform>(entity).unwrap()
}

fn vec3_close(left: Vec3, right: Vec3) -> bool {
    left.distance(right) <= EPSILON
}

fn quat_close(left: Quat, right: Quat) -> bool {
    left.dot(right).abs() >= 1.0 - EPSILON
}
