    let second=std::env::var("ALS_C544_SECOND").unwrap()=="1";
    let walls = [
        ("AuditStepWallFirst", 1.13, 45.0_f32),
        ("AuditStepWallSecond", 1.16, 20.0_f32),
    ];
    let mut geometry=Vec::new();
    for (name,x,yaw) in walls.into_iter().take(if second {2} else {1}) {
        let rotation=Quat::from_rotation_y(yaw.to_radians());
        let center=origin.with_y(3.5)+Vec3::X*x+rotation*Vec3::X*0.32;
        let size=Vec3::new(0.04,1.0,3.0);
        commands.spawn((
            Name::new(name),Transform::from_translation(center).with_rotation(rotation),
            RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
        geometry.push(serde_json::json!({"name":name,"center":center,"size":size,"front_normal":rotation*Vec3::NEG_X}));
    }
    c516_write("step-walls.json",serde_json::json!(geometry));
