    let top=std::env::var("ALS_C559_TOP_M").unwrap().parse::<f32>().unwrap();
    let wall=origin.with_y(4.0)+Vec3::X*2.38;
    let second=origin.with_y(4.0)+Vec3::X*2.0+Vec3::Z*0.405;
    let floor=origin.with_y(top-0.1)+Vec3::X*2.0;
    for (name,center,size) in [
        ("AuditAirCornerWall",wall,Vec3::new(0.1,3.0,2.0)),
        ("AuditAirCornerSecond",second,Vec3::new(2.0,3.0,0.1)),
        ("AuditAirCornerFloor",floor,Vec3::new(2.0,0.2,2.0)),
    ] {
        commands.spawn((
            Name::new(name),Transform::from_translation(center),
            RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
    }
    c516_write("air-corner.json",serde_json::json!({"wall":wall,"second":second,"floor_top":top}));
