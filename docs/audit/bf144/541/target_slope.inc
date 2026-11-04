    let slope_yaw=std::env::var("ALS_C541_YAW_DEG").unwrap().parse::<f32>().unwrap();
    let contact_height=std::env::var("ALS_C541_CONTACT_Y_M").unwrap().parse::<f32>().unwrap();
    let slope_rotation=Quat::from_rotation_y(-slope_yaw.to_radians())*Quat::from_rotation_x(std::f32::consts::FRAC_PI_3);
    let sphere_center=Vec3::new(origin.x+1.3325,contact_height-0.60,origin.z);
    let slope_center=sphere_center-(slope_rotation*Vec3::Y)*0.315;
    let slope_size=Vec3::new(0.24,0.03,0.24);
    commands.spawn((
        Name::new("AuditStepSlope"),Transform::from_translation(slope_center).with_rotation(slope_rotation),
        RigidBody::Static,Collider::cuboid(slope_size.x,slope_size.y,slope_size.z),
        CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
        Mesh3d(meshes.add(Cuboid::from_size(slope_size))),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
    ));
    c516_write("step-slope.json",serde_json::json!({"center":slope_center,"size":slope_size,
        "yaw_deg":slope_yaw,"contact_height_m":contact_height,"normal":slope_rotation*Vec3::Y}));
