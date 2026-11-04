    let label = std::env::var("ALS_C580_CASE").unwrap();
    let gap = match label.as_str() {
        "tight" => 0.001,
        "wide" => 0.014,
        "without-wall" => 0.0,
        _ => panic!("Unknown virtual ditch case"),
    };
    let rotation = Quat::from_rotation_z(-40.0_f32.to_radians());
    let normal = rotation * Vec3::Y;
    let sphere = origin.with_y(3.4) + Vec3::X * 2.0;
    let ledge = sphere - normal * 0.311;
    let wall = origin.with_y(4.0) + Vec3::X * (2.35 + gap);
    let mut bodies = vec![("AuditDitchLedge", ledge, rotation, Vec3::new(0.06, 0.02, 0.6))];
    if gap > 0.0 {
        bodies.push(("AuditDitchWall", wall, Quat::IDENTITY, Vec3::new(0.1, 3.0, 2.0)));
    }
    for (name,center,rotation,size) in bodies {
        commands.spawn((
            Name::new(name), Transform::from_translation(center).with_rotation(rotation),
            RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
    }
    c516_write("ditch-geometry.json",serde_json::json!({"wall":wall,"gap":gap,"ledge":ledge,"normal":normal}));
