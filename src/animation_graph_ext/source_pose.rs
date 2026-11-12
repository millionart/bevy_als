use bevy::{math::DVec3, platform::collections::HashMap};
use bevy_animation_graph::core::{
    id::BoneId,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};

use crate::extensions::{
    control_rig_hierarchy::RigTransform, raw_sequence_sampling::normalize_source_rotation,
    source_compression::{project_bone_transform, source_quaternion_product},
};

#[derive(Clone, Debug)]
pub(crate) struct SourcePose {
    pub(crate) bones: Vec<(BoneId, RigTransform)>,
    pub(crate) length_scale: f64,
}

impl SourcePose {
    pub(crate) fn project_into(&self, pose: &mut Pose) {
        for &(id, source) in &self.bones {
            if let Some(index) = pose.paths.get(&id).copied() {
                project_bone_transform(&mut pose.bones[index], source, self.length_scale);
            } else {
                let mut bone = BonePose::default();
                project_bone_transform(&mut bone, source, self.length_scale);
                pose.add_bone(bone, id);
            }
        }
    }

    pub(crate) fn matches_projection(&self, pose: &Pose) -> bool {
        self.bones.len() == pose.paths.len()
            && self.bones.iter().all(|&(id, source)| {
                pose.get_bone(id).is_some_and(|actual| {
                    let mut projected = BonePose::default();
                    project_bone_transform(&mut projected, source, self.length_scale);
                    projected.translation == actual.translation
                        && projected.rotation == actual.rotation
                        && projected.scale == actual.scale
                })
            })
    }

    pub(crate) fn normalize_rotations(&mut self, passes: usize) {
        for (_, value) in &mut self.bones {
            for _ in 0..passes {
                value.rotation = normalize_source_rotation(value.rotation);
            }
        }
    }

    pub(crate) fn same_layout(&self, other: &Self) -> bool {
        self.length_scale == other.length_scale
            && self.bones.iter().map(|(bone, _)| bone).eq(other.bones.iter().map(|(bone, _)| bone))
    }

    pub(crate) fn parent_indices(&self, skeleton: &Skeleton) -> Option<Vec<Option<usize>>> {
        let indices: HashMap<_, _> = self.bones.iter().enumerate()
            .map(|(index, (bone, _))| (*bone, index)).collect();
        self.bones.iter().enumerate().map(|(index, (bone, _))| {
            let parent = skeleton.parent(bone).and_then(|id| indices.get(&id).copied());
            parent.is_none_or(|parent| parent < index).then_some(parent)
        }).collect()
    }

    pub(crate) fn convert_rotations_to_mesh(&mut self, parents: &[Option<usize>]) {
        for (index, &parent) in parents.iter().enumerate() {
            if let Some(parent) = parent {
                self.bones[index].1.rotation = source_quaternion_product(
                    self.bones[parent].1.rotation, self.bones[index].1.rotation,
                );
            }
        }
    }

    pub(crate) fn convert_rotations_to_local(&mut self, parents: &[Option<usize>]) {
        for (index, &parent) in parents.iter().enumerate().rev() {
            if let Some(parent) = parent {
                self.bones[index].1.rotation = source_quaternion_product(
                    self.bones[parent].1.rotation.conjugate(), self.bones[index].1.rotation,
                );
            }
        }
    }

    pub(crate) fn blend_pair(
        &self,
        target: &Self,
        weights: impl Fn(BoneId) -> (f32, f32),
    ) -> Option<Self> {
        if !self.same_layout(target) {
            return None;
        }
        let mut output = self.clone();
        for ((id, value), &(_, target)) in output.bones.iter_mut().zip(&target.bones) {
            let (a, b) = weights(*id);
            scale_pose(value, f64::from(a));
            accumulate(value, target, f64::from(b));
        }
        Some(output)
    }
}

fn scale_pose(value: &mut RigTransform, weight: f64) {
    value.translation *= weight;
    value.rotation = value.rotation * weight;
    value.scale *= weight;
}

pub(crate) fn weighted_add(current: DVec3, value: DVec3, weight: f64) -> DVec3 {
    DVec3::new(
        value.x.mul_add(weight, current.x),
        value.y.mul_add(weight, current.y),
        value.z.mul_add(weight, current.z),
    )
}

fn accumulate(value: &mut RigTransform, child: RigTransform, weight: f64) {
    value.translation = weighted_add(value.translation, child.translation, weight);
    value.scale = weighted_add(value.scale, child.scale, weight);
    let rotation = child.rotation * weight;
    let current = value.rotation;
    let dot = current.z.mul_add(rotation.z, current.x * rotation.x)
        + current.w.mul_add(rotation.w, current.y * rotation.y);
    value.rotation = if dot < 0.0 {
        current - rotation
    } else {
        current + rotation
    };
}

pub(crate) fn blend_source_poses<'a>(
    poses: impl IntoIterator<Item = (&'a SourcePose, f32)>,
    normalization_passes: usize,
) -> Option<SourcePose> {
    let mut poses = poses.into_iter();
    let (first, weight) = poses.next()?;
    let mut output = first.clone();
    for (_, value) in &mut output.bones {
        scale_pose(value, f64::from(weight));
    }
    for (child, weight) in poses {
        if child.length_scale != output.length_scale || child.bones.len() != output.bones.len() {
            return None;
        }
        for ((id, value), &(source_id, transform)) in output.bones.iter_mut().zip(&child.bones) {
            if *id != source_id {
                return None;
            }
            accumulate(value, transform, f64::from(weight));
        }
    }
    output.normalize_rotations(normalization_passes);
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec3;
    use bevy_animation_graph::core::animation_clip::{
        EntityPath, GraphClip, loader::GraphClipSource,
    };

    fn source(path: &'static str, animation: &str, time: f32) -> SourcePose {
        let clip = GraphClip {
            source: Some(GraphClipSource::GltfNamed {
                path: path.into(),
                animation_name: animation.to_owned(),
            }),
            ..Default::default()
        };
        crate::extensions::source_compression::sample_prepared_source_pose(&clip, time)
            .unwrap()
            .0
    }

    #[test]
    fn prepared_walk_run_blend_projects_only_after_source_accumulation() {
        let walk = source(
            "ALSRUNTIME/Animations/Grounded/WalkRun/A_Als_Walk_Forward.gltf",
            "A_Als_Walk_Forward_0",
            0.24333334,
        );
        let run = source(
            "ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward.gltf",
            "A_Als_Run_Forward_0",
            0.14627451,
        );
        let blend = blend_source_poses([(&walk, 0.63), (&run, 0.37)], 2).unwrap();
        let mut projected = Pose::default();
        blend.project_into(&mut projected);
        let pelvis = EntityPath::from_slashed_string("GltfNode0/root/pelvis".to_owned()).id();
        assert_eq!(
            projected.get_bone(pelvis).unwrap().translation,
            Some(Vec3::new(0.012820745, 0.8939557, -0.03335212))
        );
        assert!(blend.matches_projection(&projected));
        let index = projected.paths[&pelvis];
        projected.bones[index].translation = Some(Vec3::ZERO);
        assert!(!blend.matches_projection(&projected));
    }
}
