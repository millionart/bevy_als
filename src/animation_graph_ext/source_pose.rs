use bevy::{
    math::{DQuat, DVec3, Quat, Vec3},
    platform::collections::HashMap,
};
use bevy_animation_graph::core::{
    id::BoneId,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};

use crate::extensions::{
    control_rig_hierarchy::{RigTransform, rig_normalize},
    control_rig_math::rotation_basis,
};

fn project_translation([x, y, z]: [f64; 3], scale: f64) -> Vec3 {
    Vec3::new((x * scale) as f32, (z * scale) as f32, (y * scale) as f32)
}

fn project_rotation([x, y, z, w]: [f64; 4]) -> Quat {
    Quat::from_xyzw(-x as f32, -z as f32, -y as f32, w as f32)
}

fn project_scale([x, y, z]: [f64; 3]) -> Vec3 {
    Vec3::new(x as f32, z as f32, y as f32)
}

pub(crate) fn project_bone_transform(bone: &mut BonePose, source: RigTransform, unit_scale: f64) {
    bone.translation = Some(project_translation(
        source.translation.to_array(),
        unit_scale,
    ));
    bone.rotation = Some(project_rotation(source.rotation.to_array()));
    bone.scale = Some(project_scale(source.scale.to_array()));
}

pub(crate) fn source_quaternion_product(a: DQuat, b: DQuat) -> DQuat {
    DQuat::from_xyzw(
        a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
    )
}

pub(crate) fn interpolate_rotation(start: DQuat, end: DQuat, alpha: f32) -> DQuat {
    let alpha = f64::from(alpha);
    let dot = start.z.mul_add(end.z, start.x * end.x) + start.w.mul_add(end.w, start.y * end.y);
    let bias = if dot < 0.0 { -1.0 } else { 1.0 };
    let left = start.to_array();
    let right = end.to_array();
    normalize_source_rotation(DQuat::from_array(std::array::from_fn(|axis| {
        (left[axis] * bias).mul_add(1.0 - alpha, right[axis] * alpha)
    })))
}

pub(crate) fn normalize_source_rotation(rotation: DQuat) -> DQuat {
    rotation_basis(rig_normalize(rotation_basis(rotation)))
}

#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct SourcePose {
    pub(crate) bones: Vec<(BoneId, RigTransform)>,
    pub(crate) length_scale: f64,
}

impl SourcePose {
    #[doc(hidden)]
    pub fn from_complete_pose(pose: &Pose, skeleton: &Skeleton) -> Option<Self> {
        const UNIT_SCALE: f64 = 0.01;
        let mut bones = pose
            .paths
            .iter()
            .map(|(&id, &index)| {
                let bone = &pose.bones[index];
                if bone.weights.is_some() {
                    return None;
                }
                let [x, y, z] = bone.translation?.as_dvec3().to_array();
                let [sx, sy, sz] = bone.scale?.as_dvec3().to_array();
                let depth = skeleton.id_to_path(id)?.parts.len();
                Some((
                    depth,
                    id,
                    RigTransform {
                        translation: DVec3::new(x, z, y) / UNIT_SCALE,
                        rotation: rotation_basis(bone.rotation?.as_dquat()),
                        scale: DVec3::new(sx, sz, sy),
                    },
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        if bones.is_empty() {
            return None;
        }
        bones.sort_unstable_by_key(|(depth, id, _)| (*depth, id.animation_target_id().0));
        Some(Self {
            bones: bones
                .into_iter()
                .map(|(_, id, transform)| (id, transform))
                .collect(),
            length_scale: UNIT_SCALE,
        })
    }

    #[doc(hidden)]
    pub fn project_into(&self, pose: &mut Pose) {
        let additional = self.bones.len().saturating_sub(pose.paths.len());
        pose.bones.reserve(additional);
        pose.paths.reserve(additional);
        for &(id, source) in &self.bones {
            let index = *pose.paths.entry(id).or_insert_with(|| {
                let index = pose.bones.len();
                pose.bones.push(BonePose::default());
                index
            });
            project_bone_transform(&mut pose.bones[index], source, self.length_scale);
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
            && self
                .bones
                .iter()
                .map(|(bone, _)| bone)
                .eq(other.bones.iter().map(|(bone, _)| bone))
    }

    pub(crate) fn parent_indices(&self, skeleton: &Skeleton) -> Option<Vec<Option<usize>>> {
        let indices: HashMap<_, _> = self
            .bones
            .iter()
            .enumerate()
            .map(|(index, (bone, _))| (*bone, index))
            .collect();
        self.bones
            .iter()
            .enumerate()
            .map(|(index, (bone, _))| {
                let parent = skeleton
                    .parent(bone)
                    .and_then(|id| indices.get(&id).copied());
                parent.is_none_or(|parent| parent < index).then_some(parent)
            })
            .collect()
    }

    pub(crate) fn convert_rotations_to_mesh(&mut self, parents: &[Option<usize>]) {
        for (index, &parent) in parents.iter().enumerate() {
            if let Some(parent) = parent {
                self.bones[index].1.rotation = source_quaternion_product(
                    self.bones[parent].1.rotation,
                    self.bones[index].1.rotation,
                );
            }
        }
    }

    pub(crate) fn convert_rotations_to_local(&mut self, parents: &[Option<usize>]) {
        for (index, &parent) in parents.iter().enumerate().rev() {
            if let Some(parent) = parent {
                self.bones[index].1.rotation = source_quaternion_product(
                    self.bones[parent].1.rotation.conjugate(),
                    self.bones[index].1.rotation,
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

#[doc(hidden)]
pub fn blend_source_poses<'a>(
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
