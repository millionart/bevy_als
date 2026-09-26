use bevy::prelude::Vec3;
use bevy_animation_graph::core::ragdoll::bone_mapping_loader::RagdollBoneMapSerial;
use bevy_animation_graph::core::ragdoll::definition::{
    BodyMode, ColliderMassMode, ColliderShape, JointVariant, Ragdoll,
};
use std::collections::BTreeMap;
use std::fs;

const RAG_PATH: &str = "assets/ragdolls/als_manny.rag.ron";
const BM_PATH: &str = "assets/ragdoll_bone_maps/als_manny.bm.ron";

#[test]
fn physics_profile_bakes_effective_source_solver_and_material_parameters() {
    let text = fs::read_to_string("assets/ragdolls/pa_als.physics.json").unwrap();
    let profile: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(profile["target_solver"]["min_parent_mass_ratio"], 0.2);
    assert_eq!(profile["target_solver"]["max_inertia_ratio"], 5.0);
    assert_eq!(profile["target_solver"]["position_iterations"], 6);
    assert_eq!(profile["target_solver"]["projection_iterations"], 1);
    for joint in profile["constraints"].as_array().unwrap() {
        assert_eq!(joint["target"]["swing_spring"]["stiffness"], 5_000_000.0);
        assert_eq!(joint["target"]["swing_spring"]["damping"], 5_000.0);
        assert_eq!(joint["target"]["swing_spring"]["acceleration_mode"], true);
        assert!(joint["target"].get("swing_compliance").is_none());
        assert_eq!(joint["target"]["mass_conditioning"], true);
        assert_eq!(joint["target"]["projection_linear_alpha"], 1.0);
    }
    for body in profile["bodies"].as_array().unwrap() {
        let material = &body["target"]["material"];
        assert!((material["static_friction"].as_f64().unwrap() - 0.7).abs() < 1e-6);
        assert!((material["dynamic_friction"].as_f64().unwrap() - 0.7).abs() < 1e-6);
        assert!((material["restitution"].as_f64().unwrap() - 0.3).abs() < 1e-6);
    }
    for body in profile["bodies"].as_array().unwrap() {
        assert_eq!(body["target"]["material"]["friction_combine"], "Average");
    }
    let mut solver_material_only = profile.clone();
    for body in solver_material_only["bodies"].as_array_mut().unwrap() {
        let bounds = body["source_properties"]["bConsiderForBounds"].clone();
        body["source_properties"] = serde_json::json!({"bConsiderForBounds": bounds});
        body.as_object_mut()
            .unwrap()
            .remove("source_default_instance");
    }
    for joint in solver_material_only["constraints"].as_array_mut().unwrap() {
        joint.as_object_mut().unwrap().remove("source_profile");
        joint.as_object_mut().unwrap().remove("source_properties");
        joint
            .as_object_mut()
            .unwrap()
            .remove("source_instance_properties");
    }
    let runtime = bevy_als::extensions::RagdollPhysicsProfile::from_json(&solver_material_only.to_string()).unwrap();
    assert_eq!(runtime.bodies.len(), profile["bodies"].as_array().unwrap().len());
    for collider in &runtime.colliders {
        let record = profile["bodies"].as_array().unwrap().iter()
            .find(|body| body["source_index"].as_u64() == Some(u64::from(collider.source_body_index))).unwrap();
        assert_eq!(collider.consider_for_bounds, record["source_properties"]["bConsiderForBounds"] == "True");
    }
    assert_eq!(runtime.joints.len(), 20);
    let thigh = runtime
        .joints
        .iter()
        .find(|joint| joint.label == "thigh_l")
        .unwrap();
    assert!(thigh.definition.motors[0].spring.acceleration_mode);
    assert_eq!(thigh.definition.motor_stiffness_scale, 1.5);
}

#[test]
fn als_manny_ragdoll_definition_loads_with_expected_topology() {
    let text = fs::read_to_string(RAG_PATH).expect("rag RON readable");
    let rag: Ragdoll = ron::de::from_str(&text).expect("parses as Ragdoll");
    assert_eq!(rag.bodies.len(), 21, "21 source rigid bodies");
    assert_eq!(rag.colliders.len(), 22, "22 source collision primitives");
    assert_eq!(rag.joints.len(), 20, "20 source constraints");
    let dynamic = rag
        .bodies
        .values()
        .filter(|b| b.default_mode == BodyMode::Dynamic)
        .count();
    assert_eq!(dynamic, 20, "20 source bodies default to Dynamic");
    assert_eq!(
        rag.bodies
            .values()
            .filter(|body| body.default_mode == BodyMode::Kinematic)
            .map(|body| body.label.as_str())
            .collect::<Vec<_>>(),
        vec!["root"],
        "source root remains kinematic"
    );
    assert!((rag.total_mass - 83.30548).abs() <= 1e-4);

    for body in rag.bodies.values() {
        assert!(
            !body.colliders.is_empty(),
            "body {} has a collider",
            body.label
        );
    }

    let capsule_count = rag
        .colliders
        .values()
        .filter(|collider| matches!(collider.shape, ColliderShape::Capsule(_)))
        .count();
    let cuboid_count = rag
        .colliders
        .values()
        .filter(|collider| matches!(collider.shape, ColliderShape::Cuboid(_)))
        .count();
    let sphere_count = rag
        .colliders
        .values()
        .filter(|collider| matches!(collider.shape, ColliderShape::Sphere(_)))
        .count();
    assert_eq!((capsule_count, cuboid_count, sphere_count), (17, 4, 1));
    assert!(
        rag.colliders
            .values()
            .all(|collider| matches!(collider.mass_mode, ColliderMassMode::Override(_)))
    );

    let mut body_mass = BTreeMap::new();
    for body in rag.bodies.values() {
        let mass = body
            .colliders
            .iter()
            .map(
                |id| match rag.colliders.get(id).expect("collider exists").mass_mode {
                    ColliderMassMode::Override(mass) => mass,
                    ColliderMassMode::ByVolume => 0.0,
                },
            )
            .sum::<f32>();
        body_mass.insert(body.label.as_str(), mass);
    }
    for (label, expected) in [
        ("pelvis", 7.926394),
        ("spine_01", 6.391965),
        ("spine_02", 7.926394),
        ("spine_03", 11.233503),
        ("neck_01", 1.810359),
        ("head", 3.514682),
        ("thigh_l", 6.172968),
        ("thigh_r", 6.172968),
        ("calf_l", 3.969801),
        ("calf_r", 3.969801),
        ("clavicle_l", 2.567043),
        ("clavicle_r", 2.567043),
        ("upperarm_l", 2.562173),
        ("upperarm_r", 2.562173),
        ("lowerarm_l", 1.500383),
        ("lowerarm_r", 1.500383),
        ("hand_l", 1.169386),
        ("hand_r", 1.169386),
        ("root", 2.927914),
        ("foot_l", 2.845381),
        ("foot_r", 2.845381),
    ] {
        assert!(
            (body_mass[label] - expected).abs() <= 1e-5,
            "source calculated mass matches for {label}"
        );
    }

    let expected_constraints = [
        ("spine_01", "spine_01", "pelvis"),
        ("spine_02", "spine_02", "spine_01"),
        ("spine_03", "spine_03", "spine_02"),
        ("neck_01", "neck_01", "spine_03"),
        ("head", "head", "neck_01"),
        ("thigh_l", "thigh_l", "pelvis"),
        ("thigh_r", "thigh_r", "pelvis"),
        ("calf_l", "calf_l", "thigh_l"),
        ("calf_r", "calf_r", "thigh_r"),
        ("clavicle_l", "clavicle_l", "spine_03"),
        ("clavicle_r", "clavicle_r", "spine_03"),
        ("upperarm_l", "upperarm_l", "clavicle_l"),
        ("upperarm_r", "upperarm_r", "clavicle_r"),
        ("lowerarm_l", "lowerarm_l", "upperarm_l"),
        ("lowerarm_r", "lowerarm_r", "upperarm_r"),
        ("hand_l", "hand_l", "lowerarm_l"),
        ("hand_r", "hand_r", "lowerarm_r"),
        ("pelvis", "pelvis", "root"),
        ("foot_l", "foot_l", "calf_l"),
        ("foot_r", "foot_r", "calf_r"),
    ];
    let body_labels = rag
        .bodies
        .iter()
        .map(|(id, body)| (*id, body.label.as_str()))
        .collect::<std::collections::HashMap<_, _>>();
    let mut actual_constraints = rag
        .joints
        .values()
        .map(|joint| match &joint.variant {
            JointVariant::Spherical(value) => (
                joint.label.as_str(),
                body_labels[&value.body1],
                body_labels[&value.body2],
            ),
            JointVariant::Revolute(value) => (
                joint.label.as_str(),
                body_labels[&value.body1],
                body_labels[&value.body2],
            ),
        })
        .collect::<Vec<_>>();
    actual_constraints.sort_unstable();
    let mut expected_constraints = expected_constraints.to_vec();
    expected_constraints.sort_unstable();
    assert_eq!(actual_constraints, expected_constraints);

    for (label, limit) in [
        ("calf_l", 60.0_f32.to_radians()),
        ("calf_r", 60.0_f32.to_radians()),
        ("lowerarm_l", 50.0_f32.to_radians()),
        ("lowerarm_r", 50.0_f32.to_radians()),
    ] {
        let joint = rag
            .joints
            .values()
            .find(|joint| joint.label == label)
            .unwrap();
        let JointVariant::Revolute(value) = &joint.variant else {
            panic!("{label} must use the source hinge form");
        };
        assert!(value.hinge_axis.distance(Vec3::Y) <= 1e-6);
        assert_eq!(value.angle_limit.as_ref().unwrap().min, -limit);
        assert_eq!(value.angle_limit.as_ref().unwrap().max, limit);
    }
}

#[test]
fn als_manny_bone_map_serializes_with_expected_coverage() {
    let text = fs::read_to_string(BM_PATH).expect("bone map RON readable");
    let bm: RagdollBoneMapSerial = ron::de::from_str(&text).expect("parses as bone map");
    assert_eq!(
        bm.bones_from_bodies.len(),
        21,
        "non-physical child bones retain their animation transforms"
    );
    let rag_text = fs::read_to_string(RAG_PATH).expect("rag RON readable");
    let rag: Ragdoll = ron::de::from_str(&rag_text).expect("parses as Ragdoll");
    assert_eq!(
        bm.bodies_from_bones.len(),
        rag.bodies.len(),
        "one kinematic source for every rigid body"
    );
    for body_id in rag.bodies.keys() {
        assert!(
            bm.bodies_from_bones.contains_key(body_id),
            "body mapping exists for every rigid body"
        );
    }
    for (body_id, mapping) in &bm.bodies_from_bones {
        assert_eq!(body_id, &mapping.body_id, "mapping key matches body id");
        assert_eq!(
            mapping.bone.bone.parts.first().map(|part| part.as_str()),
            Some("GltfNode0"),
            "body mapping uses the real skeleton root path"
        );
    }
    for bone_path in bm.bones_from_bodies.keys() {
        assert_eq!(
            bone_path.parts.first().map(|part| part.as_str()),
            Some("GltfNode0"),
            "readback mapping uses the real skeleton root path"
        );
    }
    assert!(
        bm.skeleton.to_string().contains("SKM_Als.skn.ron"),
        "bone map references the ALS skeleton"
    );
    assert!(bm.ragdoll.to_string().contains("als_manny.rag.ron"));
}

#[test]
fn pa_als_manifest_has_full_source_matrix() {
    let text = fs::read_to_string("assets/ragdolls/pa_als.physics.json")
        .expect("PA_Als manifest readable");
    let manifest: serde_json::Value = serde_json::from_str(&text).expect("PA_Als manifest parses");
    assert_eq!(manifest["format"], "physics_asset_runtime_v1");
    assert!(
        manifest["constraints"]
            .as_array()
            .unwrap()
            .iter()
            .all(|constraint| constraint["target"]
                .get("calibrate_frame_2_to_initial_pose")
                .is_none()),
        "PhysicsAsset frames must not be replaced by the first animation frame"
    );
    assert_eq!(manifest["bodies"].as_array().map(Vec::len), Some(21));
    assert_eq!(manifest["constraints"].as_array().map(Vec::len), Some(20));
    assert_eq!(
        manifest["collision_disabled_pairs"]
            .as_array()
            .map(Vec::len),
        Some(49)
    );
    assert_eq!(
        manifest["target_axis_mapping"],
        serde_json::json!(["x", "z", "y"])
    );
    assert!(
        manifest["bodies"]
            .as_array()
            .expect("bodies array")
            .iter()
            .all(|body| body["source_properties"].is_object()
                && body["source_default_instance"].is_object())
    );
    assert!(
        manifest["constraints"]
            .as_array()
            .expect("constraints array")
            .iter()
            .all(|constraint| {
                constraint["source_properties"].is_object()
                    && constraint["source_instance_properties"].is_object()
                    && constraint["frame_1_ue"].is_object()
                    && constraint["frame_2_ue"].is_object()
            })
    );
}

#[test]
fn pa_als_manifest_does_not_reduce_independent_swing_axes_to_a_max_cone() {
    let text = fs::read_to_string("assets/ragdolls/pa_als.physics.json")
        .expect("PA_Als manifest readable");
    let manifest: serde_json::Value = serde_json::from_str(&text).expect("PA_Als manifest parses");
    let thigh = manifest["constraints"]
        .as_array()
        .expect("constraints array")
        .iter()
        .find(|constraint| constraint["joint_name"] == "thigh_l")
        .expect("source thigh constraint");
    let target = &thigh["target"];
    assert_eq!(
        target["swing_1_limit_radians"],
        serde_json::json!(65.0_f64.to_radians()),
        "source Swing1 stays separate"
    );
    assert_eq!(
        target["swing_2_limit_radians"],
        serde_json::json!(20.0_f64.to_radians()),
        "source Swing2 stays separate"
    );
    assert!(
        target["swing_limit_radians"].is_null(),
        "a non-circular source cone must not become a circular max envelope"
    );
    let non_circular_spherical = manifest["constraints"]
        .as_array()
        .expect("constraints array")
        .iter()
        .filter(|constraint| {
            let target = &constraint["target"];
            target["joint_variant"] == "Spherical"
                && target["swing_1_limit_radians"] != target["swing_2_limit_radians"]
        })
        .collect::<Vec<_>>();
    assert_eq!(non_circular_spherical.len(), 13);
    assert!(
        non_circular_spherical
            .iter()
            .all(|constraint| constraint["target"]["swing_limit_radians"].is_null())
    );
}
