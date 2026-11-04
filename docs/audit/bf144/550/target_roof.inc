    let bottom=std::env::var("ALS_C550_CEILING_Y").unwrap().parse::<f32>().unwrap();
    let step=origin.with_y(2.22)+Vec3::X*1.60;
    let roof=origin.with_y(bottom+0.05)+Vec3::X*1.15;
    for (name,center,size) in [
        ("AuditStepLimit",step,Vec3::new(0.50,0.44,2.0)),
        ("AuditStepRoof",roof,Vec3::new(1.30,0.10,2.0)),
    ] {
        commands.spawn((
            Name::new(name),Transform::from_translation(center),
            RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
    }
    c516_write("step-roof.json",serde_json::json!({"step":step,"roof":roof,"roof_bottom":bottom}));
