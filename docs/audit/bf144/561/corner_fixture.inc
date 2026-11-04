    let slope_rotation=Quat::from_rotation_y(-10.0_f32.to_radians())*Quat::from_rotation_x(-60.0_f32.to_radians());
    let slope_normal=slope_rotation*Vec3::Y;
    let slope=origin.with_y(3.4)+Vec3::X*2.0-slope_normal*0.385;
    let wall=origin.with_y(4.0)+Vec3::X*2.38;
    let floor=origin.with_y(2.94)+Vec3::X*2.0;
    for (name,center,rotation,size) in [
        ("AuditAirCornerWall",wall,Quat::IDENTITY,Vec3::new(0.1,3.0,2.0)),
        ("AuditAirCornerSlope",slope,slope_rotation,Vec3::new(2.0,0.04,1.2)),
        ("AuditThirdFloor",floor,Quat::IDENTITY,Vec3::new(2.0,0.2,2.0)),
    ] {
        commands.spawn((
            Name::new(name),Transform::from_translation(center).with_rotation(rotation),
            RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
    }
    c516_write("air-corner.json",serde_json::json!({"wall":wall,"slope":slope,"slope_normal":slope_normal,"floor_top":3.04}));
