    let label = std::env::var("ALS_C581_CASE").unwrap();
    let angle = match label.as_str() {
        "near-normal" => 0.5_f32,
        "outside-angle" => 1.25,
        "single" => 0.0,
        _ => panic!("Unknown repeated wall case"),
    };
    let center = origin.with_y(4.0) + Vec3::X * 2.0;
    let rotation = Quat::from_rotation_y(-angle.to_radians());
    let normal = rotation * Vec3::NEG_X;
    let gap = -0.14 * normal.z;
    let first = center + Vec3::X * 0.3508;
    let second = center - normal * (0.35 + gap);
    let mut walls = vec![("AuditNudgeFirst", first, Quat::IDENTITY)];
    if angle > 0.0 {
        walls.push(("AuditNudgeSecond", second, rotation));
    }
    let size = Vec3::new(0.1, 3.0, 1.6);
    for (name, position, orientation) in walls {
        commands.spawn((
            Name::new(name), Transform::from_translation(position).with_rotation(orientation),
            RigidBody::Static, Collider::cuboid(size.x,size.y,size.z),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
    }
    c516_write("nudge-geometry.json", serde_json::json!({"first":first,"second":second,"gap":gap,"angle":angle,"normal":normal}));
