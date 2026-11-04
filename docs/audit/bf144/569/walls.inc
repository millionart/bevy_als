    let label = std::env::var("ALS_C569_CASE").unwrap();
    let gap = match label.as_str() {
        "overlap" => 0.55,
        "wide" => 0.64,
        "clear" => 0.0,
        _ => panic!("Unknown overlap control"),
    };
    if gap > 0.0 {
        let size = Vec3::new(0.5, 2.0, 2.0);
        let center = origin.with_y(3.7) + Vec3::X * (1.52 + gap * 0.5 + 0.25);
        commands.spawn((
            Name::new("AuditStuckRight"), Transform::from_translation(center), RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            CollisionLayers::new([AlsCollisionLayer::Default, AlsCollisionLayer::Visibility], LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
    }
