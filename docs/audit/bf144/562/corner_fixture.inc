    let gap=std::env::var("ALS_C562_GAP_M").unwrap().parse::<f32>().unwrap();
    let wall=origin.with_y(4.0)+Vec3::X*2.351;
    let second=origin.with_y(4.0)+Vec3::X*2.0+Vec3::Z*0.3545;
    let rotation=Quat::from_rotation_z(-60.0_f32.to_radians());
    let normal=rotation*Vec3::Y;
    let sphere=origin.with_y(3.4)+Vec3::X*2.0;
    let slope=sphere-normal*(0.32+0.5*gap);
    for (name,center,rotation,size) in [
        ("AuditAirCornerWall",wall,Quat::IDENTITY,Vec3::new(0.1,3.0,2.0)),
        ("AuditAirCornerSecond",second,Quat::IDENTITY,Vec3::new(2.0,3.0,0.1)),
        ("AuditAirCornerSlope",slope,rotation,Vec3::new(1.3,0.04,2.0)),
    ] {
        commands.spawn((
            Name::new(name),Transform::from_translation(center).with_rotation(rotation),
            RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
    }
    c516_write("air-corner.json",serde_json::json!({"wall":wall,"second":second,"gap":gap,"slope":slope,"normal":normal}));
