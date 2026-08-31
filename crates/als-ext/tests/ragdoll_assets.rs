use bevy_animation_graph::core::ragdoll::bone_mapping_loader::RagdollBoneMapSerial;
use bevy_animation_graph::core::ragdoll::definition::{
    BodyMode, ColliderMassMode, ColliderShape, JointVariant, Ragdoll,
};
use std::collections::BTreeMap;
use std::fs;

const RAG_PATH: &str = "../../assets/ragdolls/als_manny.rag.ron";
const BM_PATH: &str = "../../assets/ragdoll_bone_maps/als_manny.bm.ron";

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
            JointVariant::Revolute(_) => panic!("PA_Als source uses spherical constraints"),
        })
        .collect::<Vec<_>>();
    actual_constraints.sort_unstable();
    let mut expected_constraints = expected_constraints.to_vec();
    expected_constraints.sort_unstable();
    assert_eq!(actual_constraints, expected_constraints);
}

#[test]
fn als_manny_bone_map_serializes_with_expected_coverage() {
    let text = fs::read_to_string(BM_PATH).expect("bone map RON readable");
    let bm: RagdollBoneMapSerial = ron::de::from_str(&text).expect("parses as bone map");
    assert_eq!(
        bm.bones_from_bodies.len(),
        23,
        "21 body bones plus two ball bones"
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
    let text = fs::read_to_string("../../assets/ragdolls/pa_als.physics.json")
        .expect("PA_Als manifest readable");
    let manifest: serde_json::Value = serde_json::from_str(&text).expect("PA_Als manifest parses");
    assert_eq!(manifest["format"], "pa_als_physics_v1");
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
