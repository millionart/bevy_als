    let center=Vec3::new(origin.x,1.85,origin.z);
    let size=Vec3::new(2.0,0.2,2.0);
    commands.spawn((
        Name::new("AuditJumpBaseNew"),Transform::from_translation(center),
        RigidBody::Kinematic,LinearVelocity::ZERO,Collider::cuboid(size.x,size.y,size.z),
        CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
        Mesh3d(meshes.add(Cuboid::from_size(size))),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
    ));
