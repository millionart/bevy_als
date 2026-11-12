use super::*;
use avian3d::dynamics::integrator::VelocityIntegrationData;
use bevy::ecs::system::RunSystemOnce;

#[test]
fn disabled_and_sense_only_motors_leave_later_motors_running() {
    let mut world = World::new();
    let toggles = [
        TnuaToggle::Enabled,
        TnuaToggle::Disabled,
        TnuaToggle::Enabled,
        TnuaToggle::SenseOnly,
        TnuaToggle::Enabled,
    ];
    let entities = toggles.map(|toggle| {
        let mut motor = TnuaMotor::default();
        motor.lin.boost = Vector3::X;
        world
            .spawn((
                RigidBody::Dynamic,
                Position(Vec3::ZERO),
                Rotation::IDENTITY,
                VelocityIntegrationData::default(),
                toggle,
                motor,
            ))
            .id()
    });
    world.run_system_once(apply_motors_system).unwrap();
    let observed = entities.map(|entity| world.get::<LinearVelocity>(entity).unwrap().0);
    assert_eq!(observed, [Vec3::X, Vec3::ZERO, Vec3::X, Vec3::ZERO, Vec3::X]);
}
