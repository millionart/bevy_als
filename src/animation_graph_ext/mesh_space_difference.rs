use std::collections::{HashMap, HashSet};

use bevy::math::DVec3;
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::{BoneId, BonePose, Pose, RootMotionDelta},
    skeleton::Skeleton,
};

use crate::extensions::{
    raw_sequence_sampling::normalize_source_rotation,
    source_compression::source_quaternion_product,
    source_pose::SourcePose,
};

pub(crate) fn source_pose_difference(
    reference: &SourcePose,
    target: &SourcePose,
    skeleton: &Skeleton,
    mesh_space: bool,
) -> Option<SourcePose> {
    if !reference.same_layout(target) {
        return None;
    }
    let mut target = target.clone();
    let mut reference_mesh;
    let reference = if mesh_space {
        let parents = reference.parent_indices(skeleton)?;
        reference_mesh = reference.clone();
        reference_mesh.convert_rotations_to_mesh(&parents);
        target.convert_rotations_to_mesh(&parents);
        &reference_mesh
    } else {
        reference
    };
    for ((_, target), &(_, reference)) in target.bones.iter_mut().zip(&reference.bones) {
        target.rotation = normalize_source_rotation(source_quaternion_product(
            target.rotation, reference.rotation.conjugate(),
        ));
        target.translation -= reference.translation;
        let reciprocal = reference.scale.to_array().map(|value| {
            if value.abs() <= f64::from(1e-8_f32) { 0.0 } else { value.recip() }
        });
        target.scale = target.scale * DVec3::from_array(reciprocal) - DVec3::ONE;
    }
    Some(target)
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct MeshSpaceDifferenceNode;

impl MeshSpaceDifferenceNode {
    pub const IN_POSE_A: &'static str = "pose_a";
    pub const IN_TIME_A: &'static str = "time_a";
    pub const IN_POSE_B: &'static str = "pose_b";
    pub const IN_TIME_B: &'static str = "time_b";
    pub const OUT_POSE: &'static str = "pose";
}

fn hierarchy_order(skeleton: &Skeleton) -> Vec<BoneId> {
    fn visit(
        skeleton: &Skeleton,
        bone: BoneId,
        visited: &mut HashSet<BoneId>,
        order: &mut Vec<BoneId>,
    ) {
        if !visited.insert(bone) {
            return;
        }
        if let Some(parent) = skeleton.parent(&bone) {
            visit(skeleton, parent, visited, order);
        }
        order.push(bone);
    }

    let mut visited = HashSet::default();
    let mut order = Vec::new();
    visit(skeleton, skeleton.root(), &mut visited, &mut order);
    for bone in skeleton.iter_bones() {
        visit(skeleton, bone, &mut visited, &mut order);
    }
    order
}

fn local_rotation(pose: &Pose, skeleton: &Skeleton, bone: BoneId) -> Quat {
    pose.paths
        .get(&bone)
        .and_then(|index| pose.bones.get(*index))
        .and_then(|bone| bone.rotation)
        .or_else(|| {
            skeleton
                .default_transforms(bone)
                .map(|transform| transform.local.rotation)
        })
        .unwrap_or(Quat::IDENTITY)
}

fn mesh_rotations(pose: &Pose, skeleton: &Skeleton, hierarchy: &[BoneId]) -> HashMap<BoneId, Quat> {
    let mut rotations = HashMap::default();
    for bone in hierarchy {
        let local = local_rotation(pose, skeleton, *bone);
        let mesh = skeleton
            .parent(bone)
            .and_then(|parent| rotations.get(&parent).copied())
            .map(|parent| parent * local)
            .unwrap_or(local);
        rotations.insert(*bone, mesh);
    }
    rotations
}

fn default_transform(skeleton: &Skeleton, bone: BoneId) -> Transform {
    skeleton
        .default_transforms(bone)
        .map(|transform| transform.local.clone())
        .unwrap_or_default()
}

fn translation_difference(
    base: Option<&BonePose>,
    target: Option<Vec3>,
    default: Vec3,
) -> Option<Vec3> {
    target.map(|target| match base.and_then(|bone| bone.translation) {
        Some(base) => target - base,
        None if base.is_none() => target - default,
        None => target,
    })
}

fn scale_difference_for_bone(
    base: Option<&BonePose>,
    target: Option<Vec3>,
    default: Vec3,
) -> Option<Vec3> {
    target.and_then(|target| crate::extensions::reference_local_difference::scale_difference(
        target, base.and_then(|bone| bone.scale).unwrap_or(default)))
}

fn weights_difference(base: Option<&BonePose>, target: Option<&Vec<f32>>) -> Option<Vec<f32>> {
    target.map(|target| match base.and_then(|bone| bone.weights.as_ref()) {
        Some(base) => base
            .iter()
            .zip(target)
            .map(|(base, target)| *target - *base)
            .collect(),
        None => target.clone(),
    })
}

fn root_motion_difference(
    base: Option<&RootMotionDelta>,
    target: Option<&RootMotionDelta>,
) -> Option<RootMotionDelta> {
    match (base, target) {
        (Some(base), Some(target)) => Some(base.difference(target)),
        (Some(_), None) => Some(RootMotionDelta::default()),
        (None, Some(target)) => Some(target.clone()),
        (None, None) => None,
    }
}

pub fn make_mesh_space_difference(base: &Pose, target: &Pose, skeleton: &Skeleton) -> Pose {
    let hierarchy = hierarchy_order(skeleton);
    let base_mesh = mesh_rotations(base, skeleton, &hierarchy);
    let target_mesh = mesh_rotations(target, skeleton, &hierarchy);
    let mut difference = Pose {
        timestamp: base.timestamp,
        skeleton: base.skeleton.clone(),
        root_motion: root_motion_difference(base.root_motion.as_ref(), target.root_motion.as_ref()),
        ..default()
    };

    for (bone, target_index) in &target.paths {
        let target_bone = &target.bones[*target_index];
        let base_bone = base.get_bone(*bone);
        let default = default_transform(skeleton, *bone);
        let rotation = target_bone.rotation.and_then(|_| {
            let base = base_mesh.get(bone).copied()?;
            let target = target_mesh.get(bone).copied()?;
            Some((target * base.inverse()).normalize())
        });
        let difference_bone = BonePose {
            translation: translation_difference(
                base_bone,
                target_bone.translation,
                default.translation,
            ),
            rotation,
            scale: scale_difference_for_bone(base_bone, target_bone.scale, default.scale),
            weights: weights_difference(base_bone, target_bone.weights.as_ref()),
        };
        if difference_bone.translation.is_some()
            || difference_bone.rotation.is_some()
            || difference_bone.scale.is_some()
            || difference_bone.weights.is_some()
        {
            difference.add_bone(difference_bone, *bone);
        }
    }

    difference
}

impl NodeLike for MeshSpaceDifferenceNode {
    fn display_name(&self) -> String {
        "ALS Mesh Space Difference".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back(Self::IN_TIME_A)?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        ctx.set_time_update_back(Self::IN_TIME_A, update);
        let mut base = ctx.data_back(Self::IN_POSE_A)?.into_pose()?;
        ctx.set_time_update_back(Self::IN_TIME_B, TimeUpdate::Absolute(base.timestamp));
        let target = ctx.data_back(Self::IN_POSE_B)?.into_pose()?;
        if ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&base.skeleton)
            .is_none()
        {
            base.skeleton = target.skeleton.clone();
        }
        let skeleton = ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&base.skeleton)
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        let difference = make_mesh_space_difference(&base, &target, skeleton);
        ctx.set_time(difference.timestamp);
        ctx.set_data_fwd(Self::OUT_POSE, difference);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(Self::IN_POSE_A, DataSpec::Pose)
            .add_input_time(Self::IN_TIME_A)
            .add_input_data(Self::IN_POSE_B, DataSpec::Pose)
            .add_input_time(Self::IN_TIME_B)
            .add_output_data(Self::OUT_POSE, DataSpec::Pose)
            .add_output_time();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bevy_animation_graph::core::animation_clip::EntityPath;

    use crate::extensions::mesh_space_additive::apply_mesh_space_additive;

    use super::*;

    fn close_rotation(left: Quat, right: Quat) -> bool {
        (1.0 - left.dot(right).abs()).abs() < 1e-5
    }

    fn close_vector(left: Vec3, right: Vec3) -> bool {
        (left - right).length() < 1e-5
    }

    fn close_weights(left: Option<&Vec<f32>>, right: Option<&Vec<f32>>) -> bool {
        match (left, right) {
            (Some(left), Some(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| (left - right).abs() < 1e-5)
            }
            (None, None) => true,
            _ => false,
        }
    }

    fn skeleton() -> (Skeleton, BoneId, BoneId) {
        let root_path = EntityPath::default().child("root");
        let child_path = root_path.child("child");
        let root = root_path.id();
        let child = child_path.id();
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root);
        skeleton.add_bone(root_path, Transform::IDENTITY, Transform::IDENTITY);
        skeleton.add_bone(child_path, Transform::IDENTITY, Transform::IDENTITY);
        (skeleton, root, child)
    }

    #[test]
    fn mesh_space_difference_reconstructs_target_mesh_rotations() {
        let (skeleton, root, child) = skeleton();
        let mut base = Pose::default();
        base.add_bone(
            BonePose {
                rotation: Some(Quat::from_rotation_y(0.7)),
                ..default()
            },
            root,
        );
        base.add_bone(
            BonePose {
                rotation: Some(Quat::from_rotation_x(-0.4)),
                ..default()
            },
            child,
        );
        let mut target = Pose::default();
        target.add_bone(
            BonePose {
                rotation: Some(Quat::from_rotation_z(0.45)),
                ..default()
            },
            root,
        );
        target.add_bone(
            BonePose {
                rotation: Some(Quat::from_rotation_y(0.9)),
                ..default()
            },
            child,
        );

        let difference = make_mesh_space_difference(&base, &target, &skeleton);
        let mut reconstructed = base.clone();
        apply_mesh_space_additive(&mut reconstructed, &difference, 1.0, &skeleton);

        let hierarchy = hierarchy_order(&skeleton);
        let target_mesh = mesh_rotations(&target, &skeleton, &hierarchy);
        let reconstructed_mesh = mesh_rotations(&reconstructed, &skeleton, &hierarchy);
        assert!(close_rotation(
            reconstructed_mesh[&root],
            target_mesh[&root]
        ));
        assert!(close_rotation(
            reconstructed_mesh[&child],
            target_mesh[&child]
        ));
    }

    #[test]
    fn mesh_space_difference_reconstructs_local_components_and_root_motion() {
        let (skeleton, root, child) = skeleton();
        let mut base = Pose {
            timestamp: 1.25,
            root_motion: Some(RootMotionDelta {
                translation: Vec3::new(1.0, -2.0, 3.0),
                rotation: Quat::from_rotation_y(0.3),
            }),
            ..default()
        };
        base.add_bone(
            BonePose {
                rotation: Some(Quat::IDENTITY),
                ..default()
            },
            root,
        );
        base.add_bone(
            BonePose {
                translation: Some(Vec3::new(1.0, 2.0, 3.0)),
                rotation: Some(Quat::from_rotation_x(0.2)),
                scale: Some(Vec3::new(2.0, 3.0, 4.0)),
                weights: Some(vec![0.2, 0.8]),
            },
            child,
        );
        let mut target = Pose {
            timestamp: 9.5,
            root_motion: Some(RootMotionDelta {
                translation: Vec3::new(4.0, 1.0, -2.0),
                rotation: Quat::from_rotation_z(0.6),
            }),
            ..default()
        };
        target.add_bone(
            BonePose {
                rotation: Some(Quat::from_rotation_y(0.1)),
                ..default()
            },
            root,
        );
        target.add_bone(
            BonePose {
                translation: Some(Vec3::new(4.0, 6.0, 8.0)),
                rotation: Some(Quat::from_rotation_z(-0.5)),
                scale: Some(Vec3::new(5.0, 6.0, 10.0)),
                weights: Some(vec![0.7, 0.1]),
            },
            child,
        );

        let difference = make_mesh_space_difference(&base, &target, &skeleton);
        assert_eq!(difference.timestamp, base.timestamp);
        let mut reconstructed = base.clone();
        apply_mesh_space_additive(&mut reconstructed, &difference, 1.0, &skeleton);

        let output = reconstructed.get_bone(child).unwrap();
        let expected = target.get_bone(child).unwrap();
        assert!(close_vector(
            output.translation.unwrap(),
            expected.translation.unwrap()
        ));
        assert!(close_vector(output.scale.unwrap(), expected.scale.unwrap()));
        assert!(close_weights(
            output.weights.as_ref(),
            expected.weights.as_ref()
        ));
        let output_root_motion = reconstructed.root_motion.unwrap();
        let expected_root_motion = target.root_motion.unwrap();
        assert!(close_vector(
            output_root_motion.translation,
            expected_root_motion.translation
        ));
        assert!(close_rotation(
            output_root_motion.rotation,
            expected_root_motion.rotation
        ));
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<MeshSpaceDifferenceNode>();
        let registration = registry
            .get(std::any::TypeId::of::<MeshSpaceDifferenceNode>())
            .unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
