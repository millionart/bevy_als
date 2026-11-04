    if std::env::var_os("ALS_C530_BLOCKER").is_some() {
        let input:serde_json::Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/530/fixture-input.json").unwrap()).unwrap();
        let center:Vec3=serde_json::from_value(input["blocker_center_m"].clone()).unwrap();
        let size:Vec3=serde_json::from_value(input["blocker_size_m"].clone()).unwrap();
        let rotation=Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4)*Quat::from_rotation_z(-std::f32::consts::FRAC_PI_4);
        commands.spawn((
            Name::new("AuditFloorBlocker"),Transform::from_translation(center).with_rotation(rotation),
            RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
        ));
        c516_write("floor-blocker.json",serde_json::json!({"center":center,"size":size,"rotation":rotation}));
    }
