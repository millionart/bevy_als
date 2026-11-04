    let pitch=std::env::var("ALS_C543_PITCH").unwrap().parse::<f32>().unwrap();
    let ramp_step=std::env::var("ALS_C543_STEP").unwrap()=="1";
    let rotation=Quat::from_rotation_z(pitch.to_radians());
    let toe=origin.with_y(2.0)+Vec3::X*1.20;
    let ramp_center=toe+rotation*Vec3::new(0.60,-0.05,0.0);
    let witness_center=origin.with_y(3.50)+Vec3::X*1.65;
    for (name,center,rotation,size,allows_step_up) in [
        ("AuditTransitionRamp",ramp_center,rotation,Vec3::new(1.20,0.10,2.0),ramp_step),
        ("AuditRampWitness",witness_center,Quat::IDENTITY,Vec3::new(0.10,3.0,2.0),false),
    ] {
        commands.spawn((
            Name::new(name),Transform::from_translation(center).with_rotation(rotation),
            RigidBody::Static,Collider::cuboid(size.x,size.y,size.z),
            CollisionLayers::new([AlsCollisionLayer::Default,AlsCollisionLayer::Visibility],LayerMask::ALL),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
            bevy_als::als::mantle::AlsMantleTargetPermission {allows_step_up},
        ));
    }
    c516_write("ramp.json",serde_json::json!({"pitch":pitch,"allows_step":ramp_step,
        "ramp_center":ramp_center,"normal":rotation*Vec3::Y,"witness_center":witness_center}));
