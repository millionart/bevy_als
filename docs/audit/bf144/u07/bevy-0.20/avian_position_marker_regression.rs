use super::*;
use bevy::asset::AssetPlugin;
use bevy::mesh::MeshPlugin;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

fn add_marker(mut commands: Commands, bodies: Query<Entity, With<RigidBody>>) {
    for body in &bodies {
        commands.entity(body).insert(CustomPositionIntegration);
    }
}

fn remove_marker(mut commands: Commands, bodies: Query<Entity, With<CustomPositionIntegration>>) {
    for body in &bodies {
        commands.entity(body).remove::<CustomPositionIntegration>();
    }
}

fn position_with_marker_installed(before_preparation: bool) -> Vec3 {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        MeshPlugin,
        TransformPlugin,
        PhysicsPlugins::new(PostUpdate),
    ))
    .insert_resource(Gravity(Vec3::ZERO))
    .insert_resource(SubstepCount(6))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
        1.0 / 60.0,
    )))
    .add_systems(PhysicsSchedule, remove_marker.in_set(SolverSystems::PostSubstep));
    if before_preparation {
        app.add_systems(
            PhysicsSchedule,
            add_marker.before(SolverSystems::PrepareSolverBodies),
        );
    } else {
        app.add_systems(PhysicsSchedule, add_marker.in_set(SolverSystems::PreSubstep));
    }
    app.finish();
    app.cleanup();
    let body = app
        .world_mut()
        .spawn((RigidBody::Dynamic, Transform::IDENTITY, LinearVelocity(Vec3::X)))
        .id();
    app.update();
    app.update();
    app.world().get::<Position>(body).unwrap().0
}

#[test]
fn custom_position_marker_is_snapshotted_during_solver_preparation() {
    let late = position_with_marker_installed(false);
    let early = position_with_marker_installed(true);
    assert!(late.x > 0.0, "The old registration phase permitted native integration: {late:?}");
    assert_eq!(early, Vec3::ZERO);
}
