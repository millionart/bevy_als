use bevy_animation_graph::core::ragdoll::bone_mapping_loader::RagdollBoneMapSerial;
use bevy_animation_graph::core::ragdoll::definition::{BodyMode, Ragdoll};
use std::fs;

const RAG_PATH: &str = "../../assets/ragdolls/als_manny.rag.ron";
const BM_PATH: &str = "../../assets/ragdoll_bone_maps/als_manny.bm.ron";

#[test]
fn als_manny_ragdoll_definition_loads_with_expected_topology() {
    let text = fs::read_to_string(RAG_PATH).expect("rag RON readable");
    let rag: Ragdoll = ron::de::from_str(&text).expect("parses as Ragdoll");
    assert_eq!(rag.bodies.len(), 16, "16 rigid bodies");
    assert_eq!(rag.colliders.len(), 16, "one capsule per body");
    assert_eq!(rag.joints.len(), 15, "15 spherical joints");
    let dynamic = rag
        .bodies
        .values()
        .filter(|b| b.default_mode == BodyMode::Dynamic)
        .count();
    assert_eq!(dynamic, 16, "all bodies default to Dynamic");
    assert!((rag.total_mass - 75.0).abs() < f32::EPSILON);

    for body in rag.bodies.values() {
        assert!(
            !body.colliders.is_empty(),
            "body {} has a collider",
            body.label
        );
    }
}

#[test]
fn als_manny_bone_map_serializes_with_expected_coverage() {
    let text = fs::read_to_string(BM_PATH).expect("bone map RON readable");
    let bm: RagdollBoneMapSerial = ron::de::from_str(&text).expect("parses as bone map");
    assert_eq!(bm.bones_from_bodies.len(), 22, "22 mapped bones");
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
