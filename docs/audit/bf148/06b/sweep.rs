use bevy::prelude::*;
use bevy_als::extensions::apply_mesh_space_layered_blend;
use bevy_animation_graph::core::{
    animation_clip::EntityPath,
    edge_data::bone_mask::BoneMask,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};

fn main() {
    let root = EntityPath::default().child("root");
    let child = root.child("child");
    let mut skeleton = Skeleton::default();
    skeleton.set_root(root.id());
    skeleton.add_bone(root.clone(), Transform::IDENTITY, Transform::IDENTITY);
    skeleton.add_bone(child.clone(), Transform::IDENTITY, Transform::IDENTITY);
    let mut mask = BoneMask::none();
    mask.add_bone_weight(root.id(), root.clone(), 1.0);
    assert_eq!(mask.bone_weight(&root.id()), 1.0);
    assert_eq!(mask.bone_weight(&child.id()), 0.0);
    for step in 1..4097 {
        let input = Quat::from_euler(
            EulerRot::YXZ,
            step as f32 * 0.002,
            step as f32 * 0.003,
            step as f32 * 0.005,
        );
        assert!(input.is_normalized());
        let mut base = Pose::default();
        base.add_bone(
            BonePose {
                rotation: Some(input),
                ..default()
            },
            root.id(),
        );
        let overlay = base.clone();
        apply_mesh_space_layered_blend(&mut base, &overlay, 0.5, &mask, &skeleton);
        let output = base.get_bone(root.id()).unwrap().rotation.unwrap();
        println!(
            "{{\"step\":{step},\"input\":{:?},\"parent_output\":{:?},\"child_present\":{}}}",
            input.to_array(),
            output.to_array(),
            base.get_bone(child.id()).is_some(),
        );
    }
}
