    if std::env::var_os("ALS_C523_DECK").is_some() {
        let input:serde_json::Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/576/initial.json").unwrap()).unwrap();
        let top=input["deck_top_m"].as_f64().unwrap() as f32;
        let center=Vec3::new(origin.x+2.3,top-0.1,origin.z-0.3);
        let size=Vec3::new(2.0,0.2,1.3);
        commands.spawn((
            Name::new("AuditLandingDeck"),Transform::from_translation(center),
            RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
        c516_write("deck.json",serde_json::json!({"center":center,"top":top,"size":size}));
    }
