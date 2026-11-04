    let center=origin.with_y(1.88)+Vec3::X*1.27;
    let size=Vec3::new(0.14,0.2,2.0);
    commands.spawn((
        Name::new("AuditWalkOffSecondBase"),Transform::from_translation(center),
        RigidBody::Kinematic,LinearVelocity::ZERO,Collider::cuboid(size.x,size.y,size.z),
        CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
        Mesh3d(meshes.add(Cuboid::from_size(size))),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
    ));
