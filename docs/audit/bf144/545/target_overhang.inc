    let rotation=Quat::from_rotation_y(45.0_f32.to_radians())*Quat::from_rotation_z(30.0_f32.to_radians());
    let center=origin.with_y(3.95)+Vec3::X*1.13+rotation*Vec3::X*0.32;
    let size=Vec3::new(0.04,1.0,1.5);
    commands.spawn((
        Name::new("AuditStepOverhang"),Transform::from_translation(center).with_rotation(rotation),
        RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
        CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
        Mesh3d(meshes.add(Cuboid::from_size(size))),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
    ));
    c516_write("step-overhang.json",serde_json::json!({"center":center,"size":size,"front_normal":rotation*Vec3::NEG_X}));
