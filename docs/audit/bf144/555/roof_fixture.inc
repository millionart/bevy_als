    if std::env::var_os("ALS_C555_ROOF").is_some() {
        let center=origin.with_y(4.9515)+Vec3::X*2.0;
        let size=Vec3::new(1.0,0.1,1.0);
        commands.spawn((
            Name::new("AuditApexRoof"),Transform::from_translation(center),
            RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
        c516_write("apex-roof.json",serde_json::json!({"center":center,"size":size,"bottom":4.9015}));
    }
