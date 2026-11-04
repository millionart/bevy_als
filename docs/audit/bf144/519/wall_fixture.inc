    if let Ok(raw)=std::env::var("ALS_C519_WALL_CM") {
        let wall_cm=raw.parse::<u32>().unwrap();
        assert!((150..=180).contains(&wall_cm));
        let center=Vec3::new(origin.x+wall_cm as f32*0.01,3.0,origin.z);
        commands.spawn((
            Name::new("AuditWalkOffBarrier"),
            Transform::from_translation(center).with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4)),
            RigidBody::Static,
            Collider::cuboid(0.04,4.0,2.0),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::new(0.04,4.0,2.0))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
        c516_write("barrier.json",serde_json::json!({"wall_x_cm":wall_cm,"center":center,
            "size":[0.04,4.0,2.0],"yaw_source_degrees":45.0}));
    }
