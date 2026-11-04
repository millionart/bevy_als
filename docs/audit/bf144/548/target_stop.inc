    let front=std::env::var("ALS_C548_FRONT_M").unwrap().parse::<f32>().unwrap();
    let center=origin.with_y(4.15)+Vec3::X*(front+0.04);
    let size=Vec3::new(0.08,1.0,0.70);
    commands.spawn((
        Name::new("AuditStepStop"),Transform::from_translation(center),
        RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
        CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
        Mesh3d(meshes.add(Cuboid::from_size(size))),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
    ));
    c516_write("step-stop.json",serde_json::json!({"front":front,"center":center,"size":size}));
