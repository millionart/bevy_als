    let front=std::env::var("ALS_C551_FRONT_M").unwrap().parse::<f32>().unwrap();
    let center=origin.with_y(3.5)+Vec3::X*(front+0.05);
    let size=Vec3::new(0.10,3.0,2.0);
    commands.spawn((
        Name::new("AuditStallWall"),Transform::from_translation(center),
        RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
        CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
        Mesh3d(meshes.add(Cuboid::from_size(size))),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
        bevy_als::als::mantle::AlsMantleTargetPermission {allows_step_up:false},
    ));
    c516_write("stall-wall.json",serde_json::json!({"front":front,"center":center,"size":size}));
