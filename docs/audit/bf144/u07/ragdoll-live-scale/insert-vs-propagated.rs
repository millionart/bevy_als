use avian3d::prelude::{Collider,PhysicsPlugins,RigidBody};
use bevy::prelude::*;

fn main() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin, TransformPlugin,
        PhysicsPlugins::new(PostUpdate)));
    app.finish();
    app.cleanup();
    for rotation in [Quat::IDENTITY, Quat::from_xyzw(0.0, 0.0, 0.707106781, 0.707106781)] {
        let body = app.world_mut().spawn((RigidBody::Kinematic, Transform::IDENTITY)).id();
        let shape = app.world_mut().spawn((Collider::capsule(0.07, 0.05),
            Transform::from_rotation(rotation), ChildOf(body))).id();
        let read = |app: &App| {
            let shape = app.world().get::<Collider>(shape).unwrap();
            (shape.scale(), shape.shape().shape_type(), shape.shape_scaled().shape_type())
        };
        println!("{rotation:?} before {:?}", read(&app));
        app.update();
        println!("{rotation:?} after {:?}", read(&app));
    }
}
