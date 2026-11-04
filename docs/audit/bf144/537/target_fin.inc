    let fin_center=Vec3::new(origin.x+1.145,2.08,origin.z+0.27);
    let fin_size=Vec3::new(0.02,0.20,0.02);
    commands.spawn((
        Name::new("AuditStepFin"),Transform::from_translation(fin_center),
        RigidBody::Static,Collider::cuboid(fin_size.x,fin_size.y,fin_size.z),
        CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
        Mesh3d(meshes.add(Cuboid::from_size(fin_size))),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
    ));
    c516_write("step-fin.json",serde_json::json!({"center":fin_center,"size":fin_size}));
