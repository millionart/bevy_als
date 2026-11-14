use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::{BonePose, Pose, RootMotionDelta},
    skeleton::Skeleton,
};

use crate::extensions::{
    AnimationContextState,
    animation_context::pose_input_context_state,
    mesh_space_difference::source_pose_difference,
    normalized_blendspace::{clip_binding, pose_input_node},
    source_compression::sample_raw_additive_reference,
    source_pose::SourcePose,
    time_position::{IN_POSE, TimePositionNode},
};
use std::sync::Arc;

pub const REFERENCE_POSE: &str = "reference_pose";
pub const REFERENCE_TIME: &str = "reference_time";
pub const TARGET_POSE: &str = "target_pose";
pub const TARGET_TIME: &str = "target_time";
pub const OUT_POSE: &str = "pose";
const SCALE_EPSILON: f32 = 1e-8;

fn default_transform(
    skeleton: &Skeleton,
    bone: bevy_animation_graph::core::pose::BoneId,
) -> Transform {
    skeleton
        .default_transforms(bone)
        .map(|transform| transform.local)
        .unwrap_or_default()
}

fn reference_translation(reference: Option<&BonePose>, default: Transform) -> Vec3 {
    reference
        .and_then(|bone| bone.translation)
        .unwrap_or(default.translation)
}

fn reference_rotation(reference: Option<&BonePose>, default: Transform) -> Quat {
    reference
        .and_then(|bone| bone.rotation)
        .unwrap_or(default.rotation)
}

fn reference_scale(reference: Option<&BonePose>, default: Transform) -> Vec3 {
    reference
        .and_then(|bone| bone.scale)
        .unwrap_or(default.scale)
}

pub(crate) fn scale_difference(target: Vec3, reference: Vec3) -> Option<Vec3> {
    let reciprocal = Vec3::select(
        reference.abs().cmple(Vec3::splat(SCALE_EPSILON)),
        Vec3::ZERO,
        reference.recip(),
    );
    let difference = target * reciprocal - Vec3::ONE;
    (difference != Vec3::ZERO).then_some(difference)
}

fn root_motion_difference(
    reference: Option<&RootMotionDelta>,
    target: Option<&RootMotionDelta>,
) -> Option<RootMotionDelta> {
    match (reference, target) {
        (Some(reference), Some(target)) => Some(reference.difference(target)),
        (Some(_), None) => Some(RootMotionDelta::default()),
        (None, Some(target)) => Some(target.clone()),
        (None, None) => None,
    }
}

pub fn make_local_difference(reference: &Pose, target: &Pose, skeleton: &Skeleton) -> Pose {
    let mut difference = Pose {
        timestamp: target.timestamp,
        skeleton: target.skeleton.clone(),
        root_motion: root_motion_difference(
            reference.root_motion.as_ref(),
            target.root_motion.as_ref(),
        ),
        ..default()
    };
    for (bone, target_index) in &target.paths {
        let target_bone = &target.bones[*target_index];
        let reference_bone = reference.get_bone(*bone);
        let default = default_transform(skeleton, *bone);
        let bone_difference = BonePose {
            translation: target_bone
                .translation
                .map(|value| value - reference_translation(reference_bone, default)),
            rotation: target_bone.rotation.map(|value| {
                (value * reference_rotation(reference_bone, default).inverse()).normalize()
            }),
            scale: target_bone.scale.and_then(|value| {
                scale_difference(value, reference_scale(reference_bone, default))
            }),
            weights: target_bone.weights.as_ref().map(|target_weights| {
                reference_bone
                    .and_then(|bone| bone.weights.as_ref())
                    .map(|reference_weights| {
                        reference_weights
                            .iter()
                            .zip(target_weights)
                            .map(|(reference, target)| target - reference)
                            .collect()
                    })
                    .unwrap_or_else(|| target_weights.clone())
            }),
        };
        if bone_difference.translation.is_some()
            || bone_difference.rotation.is_some()
            || bone_difference.scale.is_some()
            || bone_difference.weights.is_some()
        {
            difference.add_bone(bone_difference, *bone);
        }
    }
    difference
}

pub(crate) fn precise_reference_difference(
    ctx: &NodeContext,
    reference: &mut Pose,
    target: &Pose,
    skeleton: &Skeleton,
    mesh_space: bool,
) -> Option<Arc<SourcePose>> {
    let graph = ctx.graph_context.graph;
    let raw = pose_input_node(graph, ctx.node_id, REFERENCE_POSE.to_owned()).and_then(|node| {
        let node = if node.try_inner_downcast_ref::<TimePositionNode>().is_some() {
            pose_input_node(graph, node.id, IN_POSE.to_owned())?
        } else {
            node
        };
        let (handle, eligible) = clip_binding(node)?;
        if !eligible {
            return None;
        }
        let clip = ctx.graph_context.resources.graph_clip_assets.get(handle)?;
        sample_raw_additive_reference(clip, reference.timestamp)
    });
    if let Some(raw) = &raw {
        raw.project_into(reference);
    }
    let source = pose_input_context_state(ctx, REFERENCE_POSE);
    let source = raw
        .as_ref()
        .or_else(|| source.as_ref()?.source_pose_for(reference).map(Arc::as_ref))?;
    let target_context = pose_input_context_state(ctx, TARGET_POSE)?;
    source_pose_difference(
        source,
        target_context.source_pose_for(target)?,
        skeleton,
        mesh_space,
    )
    .map(Arc::new)
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct ReferenceLocalDifferenceNode;

impl NodeLike for ReferenceLocalDifferenceNode {
    fn display_name(&self) -> String {
        "ALS Reference Local Difference".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back(TARGET_TIME)?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        ctx.set_time_update_back(REFERENCE_TIME, update.clone());
        ctx.set_time_update_back(TARGET_TIME, update);
        let mut reference = ctx.data_back(REFERENCE_POSE)?.into_pose()?;
        let target = ctx.data_back(TARGET_POSE)?.into_pose()?;
        if ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&reference.skeleton)
            .is_none()
        {
            reference.skeleton = target.skeleton.clone();
        }
        let skeleton = ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&reference.skeleton)
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        let source_pose =
            precise_reference_difference(&ctx, &mut reference, &target, skeleton, false);
        let mut difference = make_local_difference(&reference, &target, skeleton);
        if let Some(source) = &source_pose {
            source.project_into(&mut difference);
        }
        ctx.set_time(difference.timestamp);
        ctx.set_data_fwd(OUT_POSE, difference);
        ctx.state_mut::<AnimationContextState>()?.source_pose = source_pose;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(REFERENCE_POSE, DataSpec::Pose)
            .add_input_time(REFERENCE_TIME)
            .add_input_data(TARGET_POSE, DataSpec::Pose)
            .add_input_time(TARGET_TIME)
            .add_output_data(OUT_POSE, DataSpec::Pose)
            .add_output_time();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bevy_animation_graph::core::{
        animation_clip::EntityPath, edge_data::bone_mask::BoneMask,
        interpolation::additive::AdditiveInterpolator,
    };

    use super::*;

    fn close_vector(left: Vec3, right: Vec3) -> bool {
        (left - right).length() <= 1e-5
    }

    fn close_rotation(left: Quat, right: Quat) -> bool {
        (1.0 - left.dot(right).abs()).abs() <= 1e-5
    }

    fn close_weights(left: Option<&Vec<f32>>, right: Option<&Vec<f32>>) -> bool {
        left.zip(right).is_some_and(|(left, right)| {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| (left - right).abs() <= 1e-5)
        })
    }

    #[test]
    fn local_difference_reconstructs_target_and_zeroes_unit_scale() {
        assert_eq!(
            scale_difference(Vec3::new(3.0, 4.0, 2.0), Vec3::new(2.0, 0.0, 1e-10)),
            Some(Vec3::new(0.5, -1.0, -1.0))
        );
        let path = EntityPath::default().child("root");
        let bone = path.id();
        let mut skeleton = Skeleton::default();
        skeleton.set_root(bone);
        skeleton.add_bone(path, Transform::IDENTITY, Transform::IDENTITY);
        let mut reference = Pose::default();
        reference.add_bone(
            BonePose {
                translation: Some(Vec3::new(1.0, 2.0, 3.0)),
                rotation: Some(Quat::from_rotation_y(0.4)),
                scale: Some(Vec3::ONE),
                weights: Some(vec![0.2, 0.8]),
            },
            bone,
        );
        let mut target = Pose {
            timestamp: 2.0,
            ..default()
        };
        target.add_bone(
            BonePose {
                translation: Some(Vec3::new(4.0, 5.0, 6.0)),
                rotation: Some(Quat::from_rotation_z(-0.5)),
                scale: Some(Vec3::ONE),
                weights: Some(vec![0.6, 0.1]),
            },
            bone,
        );
        let difference = make_local_difference(&reference, &target, &skeleton);
        assert_eq!(difference.get_bone(bone).unwrap().scale, None);
        let mut reconstructed = reference.clone();
        AdditiveInterpolator {
            bone_mask: BoneMask::all(),
        }
        .interpolate_pose(&mut reconstructed, &difference, 1.0);
        let output = reconstructed.get_bone(bone).unwrap();
        let expected = target.get_bone(bone).unwrap();
        assert!(close_vector(
            output.translation.unwrap(),
            expected.translation.unwrap()
        ));
        assert!(close_rotation(
            output.rotation.unwrap(),
            expected.rotation.unwrap()
        ));
        assert!(close_vector(output.scale.unwrap(), expected.scale.unwrap()));
        assert!(close_weights(
            output.weights.as_ref(),
            expected.weights.as_ref()
        ));
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<ReferenceLocalDifferenceNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<ReferenceLocalDifferenceNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
