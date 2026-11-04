    let ceiling_bottom=std::env::var("ALS_C547_CEILING_Y").unwrap().parse::<f32>().unwrap();
    let ceiling=origin.with_y(ceiling_bottom+0.05)+Vec3::X*0.35;
    let wall=origin.with_y(2.215)+Vec3::X*1.45;
    for (name,center,size) in [
        ("AuditStepCeiling",ceiling,Vec3::new(1.0,0.1,1.0)),
        ("AuditStepEdge",wall,Vec3::new(0.08,0.43,1.0)),
    ] {
        commands.spawn((
            Name::new(name),Transform::from_translation(center),
            RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
    }
    c516_write("step-edge.json",serde_json::json!({"wall":wall,"ceiling":ceiling,"ceiling_bottom":ceiling_bottom}));
