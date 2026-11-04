    let height=std::env::var("ALS_C549_HEIGHT_M").unwrap().parse::<f32>().unwrap();
    let center=origin.with_y(2.0+0.5*height)+Vec3::X*1.60;
    let size=Vec3::new(0.50,height,2.0);
    commands.spawn((
        Name::new("AuditStepLimit"),Transform::from_translation(center),
        RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
        CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
        Mesh3d(meshes.add(Cuboid::from_size(size))),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
    ));
    c516_write("step-limit.json",serde_json::json!({"height":height,"center":center,"size":size}));
