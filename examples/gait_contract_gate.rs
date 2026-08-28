use als_core::character::{AlsCharacter, AlsMovementParamsHandle};
use als_core::gait::{
    AlsDesiredGait, AlsGaitAmount, AlsGaitPlugin, AlsMaxAllowedGait, refresh_gait_system,
};
use als_core::input::AlsInputCommand;
use als_core::input_request::{AlsFirstPersonView, AlsInputRequestPlugin, AlsSprintPrev};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, RotationMode, Stance};
use als_core::transition::{AlsStateRequest, AlsStateTransitionPlugin};
use avian3d::prelude::LinearVelocity;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        AlsStateTransitionPlugin,
        AlsInputRequestPlugin,
        AlsGaitPlugin,
    ))
    .init_asset::<AlsMovementParams>()
    .add_systems(Update, refresh_gait_system);

    let params = app
        .world_mut()
        .resource_mut::<Assets<AlsMovementParams>>()
        .add(AlsMovementParams::default());
    let entity = app
        .world_mut()
        .spawn((
            AlsCharacter,
            LocomotionState::Grounded,
            Stance::Standing,
            RotationMode::ViewDirection,
            Gait::Running,
            AlsDesiredGait(Gait::Running),
            AlsMaxAllowedGait(Gait::Running),
            AlsGaitAmount::default(),
            LinearVelocity::default(),
            AlsMovementParamsHandle(params),
            AlsInputCommand::default(),
            AlsStateRequest::default(),
        ))
        .id();

    app.update();
    let idle = snapshot(&app, entity);
    assert_eq!(idle, (Gait::Running, Gait::Running, Gait::Walking, 0.0));

    app.world_mut()
        .entity_mut(entity)
        .insert(LinearVelocity(Vec3::new(1.84, 0.0, 0.0)));
    app.update();
    let walk_threshold = snapshot(&app, entity);
    assert_eq!(walk_threshold.2, Gait::Walking);

    app.world_mut()
        .entity_mut(entity)
        .insert(LinearVelocity(Vec3::new(1.86, 0.0, 0.0)));
    app.update();
    let run_threshold = snapshot(&app, entity);
    assert_eq!(run_threshold.2, Gait::Running);

    {
        let mut entity_mut = app.world_mut().entity_mut(entity);
        entity_mut.insert((
            AlsInputCommand {
                sprint_held: true,
                move_dir: Vec2::X,
                ..default()
            },
            LinearVelocity(Vec3::new(4.0, 0.0, 0.0)),
        ));
    }
    app.update();
    let third_person = snapshot(&app, entity);
    assert_eq!(third_person.0, Gait::Sprinting);
    assert_eq!(third_person.1, Gait::Sprinting);
    assert_eq!(third_person.2, Gait::Sprinting);

    {
        let mut entity_mut = app.world_mut().entity_mut(entity);
        entity_mut.insert(AlsFirstPersonView);
    }
    app.update();
    let first_person = snapshot(&app, entity);
    assert_eq!(first_person.0, Gait::Sprinting);
    assert_eq!(first_person.1, Gait::Running);
    assert_eq!(first_person.2, Gait::Running);

    {
        let mut entity_mut = app.world_mut().entity_mut(entity);
        entity_mut.insert((
            LocomotionState::Air,
            AlsInputCommand {
                sprint_held: true,
                move_dir: Vec2::Y,
                ..default()
            },
        ));
        entity_mut.remove::<AlsSprintPrev>();
    }
    app.update();
    let airborne = snapshot(&app, entity);
    assert_eq!(airborne.0, Gait::Sprinting);
    assert_eq!(airborne.1, Gait::Running);
    assert_eq!(airborne.2, Gait::Running);

    println!(
        "[BCHAR17][VERDICT] idle={idle:?} threshold_1_84={walk_threshold:?} threshold_1_86={run_threshold:?} third_person={third_person:?} first_person={first_person:?} airborne={airborne:?} -> PASS"
    );
}

fn snapshot(app: &App, entity: Entity) -> (Gait, Gait, Gait, f32) {
    (
        app.world().get::<AlsDesiredGait>(entity).unwrap().0,
        app.world().get::<AlsMaxAllowedGait>(entity).unwrap().0,
        *app.world().get::<Gait>(entity).unwrap(),
        app.world().get::<AlsGaitAmount>(entity).unwrap().0,
    )
}
