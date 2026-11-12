use std::{
    collections::HashMap,
    mem::size_of,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use bevy::{
    math::{DQuat, DVec3},
    prelude::{Handle, Name, Quat, Transform, Vec3},
};
use bevy_animation_graph::core::{
    animation_clip::{EntityPath, GraphClip, loader::GraphClipSource},
    id::BoneId,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};
use serde::Deserialize;

use crate::extensions::als_curves::{AlsPoseCurves, SourceCurveName, SourceCurveValues};
use crate::extensions::control_rig_hierarchy::RigTransform;
use crate::extensions::raw_curve_sampling::evaluate_raw_curves;
use crate::extensions::raw_sequence_sampling::{normalize_source_rotation, raw_control_sample};
use crate::extensions::source_pose::SourcePose;
use crate::extensions::source_sequence_player::SourceRootLock;

#[path = "source_per_track.rs"]
mod per_track;

#[derive(Deserialize)]
struct SampleClock {
    sample_count: usize,
    sample_rate: f32,
    duration: f32,
    wrap: bool,
}

impl SampleClock {
    fn samples(&self, time: f32) -> (usize, usize, f32) {
        let sample = time.max(0.0).min(self.duration) * self.sample_rate;
        let first = sample as usize;
        if self.wrap && first >= self.sample_count {
            return (0, 0, 0.0);
        }
        let second = if self.wrap {
            (first + 1) % self.sample_count
        } else {
            (first + 1).min(self.sample_count - 1)
        };
        (first, second, sample - first as f32)
    }
}

#[derive(Deserialize)]
struct PreparedCurves {
    #[serde(flatten)]
    clock: SampleClock,
    tracks: Vec<(SourceCurveName, (usize, u8))>,
}

#[derive(Deserialize)]
struct PreparedBoneTrack(usize, (usize, u8), (usize, u8), (usize, u8));

#[derive(Deserialize)]
struct PreparedTransforms {
    #[serde(flatten)]
    clock: SampleClock,
    has_scale: bool,
    tracks: Vec<PreparedBoneTrack>,
}

#[derive(Deserialize)]
struct SourceVector {
    x: f64,
    y: f64,
    z: f64,
}

impl SourceVector {
    fn values(&self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}

#[derive(Deserialize)]
struct SourceQuaternion {
    x: f64,
    y: f64,
    z: f64,
    w: f64,
}

impl SourceQuaternion {
    fn values(&self) -> [f64; 4] {
        [self.x, self.y, self.z, self.w]
    }
}

#[derive(Deserialize)]
struct SourceTransform {
    translation: SourceVector,
    rotation: SourceQuaternion,
    scale: SourceVector,
}

impl SourceTransform {
    fn rig_transform(&self) -> RigTransform {
        RigTransform {
            translation: DVec3::from_array(self.translation.values()),
            rotation: DQuat::from_array(self.rotation.values()),
            scale: DVec3::from_array(self.scale.values()),
        }
    }
}

#[derive(Deserialize)]
struct PreparedBone {
    skeleton_pose: SourceTransform,
    mesh_pose: SourceTransform,
    #[serde(skip)]
    animation_relative: bool,
}

impl PreparedBone {
    fn retarget_relative(&self, value: &mut RigTransform) {
        let authored = self.skeleton_pose.rig_transform();
        let reference = self.mesh_pose.rig_transform();
        let rotation = source_quaternion_product(value.rotation, authored.rotation.conjugate());
        value.rotation =
            normalize_source_rotation(source_quaternion_product(rotation, reference.rotation));
        value.translation += reference.translation - authored.translation;
        let reciprocal = authored.scale.to_array().map(|component| {
            if component.abs() <= f64::from(1e-8_f32) {
                0.0
            } else {
                component.recip()
            }
        });
        value.scale *= reference.scale * DVec3::from_array(reciprocal);
    }

    fn write_raw_control(
        &self,
        pose: &mut Pose,
        id: BoneId,
        candidate: RigTransform,
        unit_scale: f64,
    ) {
        let Some(index) = pose.paths.get(&id).copied() else {
            return;
        };
        let bone = &mut pose.bones[index];
        let retained = candidate.same(self.skeleton_pose.rig_transform(), f64::from(1e-4_f32));
        let mut value = if retained {
            self.mesh_pose.rig_transform()
        } else {
            candidate
        };
        value.rotation = normalize_source_rotation(value.rotation);
        if self.animation_relative && !retained {
            self.retarget_relative(&mut value);
        }
        project_bone_transform(bone, value, unit_scale);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreparedPoseKind {
    Absolute,
    LocalAdditive,
    MeshAdditive,
}

#[derive(Deserialize)]
struct PreparedSequence {
    gltf: PathBuf,
    animation: String,
    duration: f32,
    frame_rate: [u32; 2],
    curve: Option<PreparedCurves>,
    bone: PreparedTransforms,
    additive_type: u8,
    step: bool,
    single_root: bool,
    forced_root_lock: Option<SourceRootLock>,
}

impl PreparedSequence {
    fn key(&self) -> (&Path, &str) {
        (&self.gltf, &self.animation)
    }

    fn clip_time(&self, time: f32) -> f32 {
        time.max(0.0).min(self.duration)
    }

    fn pose_samples(&self, time: f32) -> (usize, usize, f32) {
        let [numerator, denominator] = self.frame_rate.map(f64::from);
        let (frame, fraction) = source_frame_time(self.clip_time(time), self.frame_rate);
        let evaluation_time = (frame * denominator + f64::from(fraction) * denominator) / numerator;
        let (first, second, alpha) = self.bone.clock.samples(evaluation_time as f32);
        (first, second, if self.step { 0.0 } else { alpha })
    }

    fn pose_kind(&self) -> PreparedPoseKind {
        match self.additive_type {
            0 => PreparedPoseKind::Absolute,
            1 => PreparedPoseKind::LocalAdditive,
            2 => PreparedPoseKind::MeshAdditive,
            _ => unreachable!("Invalid prepared additive type"),
        }
    }
}

#[derive(Deserialize)]
struct PreparedMetadata {
    format: String,
    curve_value_scales: HashMap<SourceCurveName, f32>,
    source_length_to_runtime_scale: f64,
    bones: Vec<PreparedBone>,
    bone_paths: Vec<Vec<String>>,
    animation_relative_bones: Vec<usize>,
    sequences: Vec<PreparedSequence>,
}

struct PreparedAcl {
    keys: &'static [u8],
    curve_value_scales: HashMap<SourceCurveName, f32>,
    source_length_to_runtime_scale: f64,
    bone_references: Vec<PreparedBone>,
    bone_ids: Vec<BoneId>,
    sequences: Vec<PreparedSequence>,
}

static PREPARED_ACL: LazyLock<PreparedAcl> = LazyLock::new(|| {
    let data = include_bytes!("../../assets/ALSRuntime/Animations/als_prepared_acl.bin");
    let (header, payload) = data.split_at(4);
    let length = u32::from_le_bytes(header.try_into().expect("ACL header length")) as usize;
    let (metadata, keys) = payload.split_at(length);
    let PreparedMetadata {
        format,
        curve_value_scales,
        source_length_to_runtime_scale,
        mut bones,
        bone_paths,
        animation_relative_bones,
        mut sequences,
    } = serde_json::from_slice(metadata).expect("Generated ACL metadata");
    assert_eq!(format, "als_prepared_acl_v1");
    for index in animation_relative_bones {
        bones[index].animation_relative = true;
    }
    sequences.sort_unstable_by(|left, right| left.key().cmp(&right.key()));
    let bone_ids = bone_paths.into_iter().map(bone_id).collect::<Vec<_>>();
    assert_eq!(bone_ids.len(), bones.len());
    PreparedAcl {
        keys,
        curve_value_scales,
        source_length_to_runtime_scale,
        bone_references: bones,
        bone_ids,
        sequences,
    }
});

fn bone_id(parts: Vec<String>) -> BoneId {
    EntityPath {
        parts: parts.into_iter().map(Name::new).collect(),
    }
    .id()
}

pub(crate) fn clip_source_key(clip: &GraphClip) -> Option<(&Path, &str)> {
    let GraphClipSource::GltfNamed {
        path,
        animation_name,
    } = clip.source.as_ref()?;
    path.source()
        .as_str()
        .is_none()
        .then_some((path.path(), animation_name.as_str()))
}

pub(crate) fn source_frame_time(time: f32, frame_rate: [u32; 2]) -> (f64, f32) {
    let [numerator, denominator] = frame_rate.map(f64::from);
    let frame_time = f64::from(time) * numerator / denominator;
    let frame = frame_time.floor();
    let subframe = (frame_time - frame) as f32;
    let carry = subframe.trunc();
    (
        frame + f64::from(carry),
        normalize_source_subframe(subframe - carry),
    )
}

fn normalize_source_subframe(fraction: f32) -> f32 {
    let max_subframe = f32::from_bits(1.0_f32.to_bits() - 1);
    (fraction.min(max_subframe) + 0.5 - 0.5).clamp(0.0, max_subframe)
}

pub(crate) fn convert_source_frame_time(
    time: (f64, f32),
    source_rate: [u32; 2],
    target_rate: [u32; 2],
) -> (f64, f32) {
    if source_rate == target_rate {
        return time;
    }
    let [source_numerator, source_denominator] = source_rate.map(i64::from);
    let [target_numerator, target_denominator] = target_rate.map(i64::from);
    let numerator = target_numerator * source_denominator;
    let denominator = target_denominator * source_numerator;
    let (frame, fraction) = time;
    let integer = ((frame as i64 * numerator) / denominator) as f64;
    let integer_fraction = frame * numerator as f64 / denominator as f64 - integer;
    let float_part = f64::from(fraction) * numerator as f64 / denominator as f64 + integer_fraction;
    let carry = float_part.floor();
    (
        integer + carry,
        normalize_source_subframe((float_part - carry) as f32),
    )
}

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

fn acl_lerp<const N: usize>(start: [f32; N], end: [f32; N], alpha: f32) -> [f32; N] {
    std::array::from_fn(|axis| end[axis].mul_add(alpha, (-start[axis]).mul_add(alpha, start[axis])))
}

fn acl_dot(a: [f32; 4], b: [f32; 4], single_root: bool) -> f32 {
    if single_root {
        (a[0] * b[0] + a[2] * b[2]) + (a[1] * b[1] + a[3] * b[3])
    } else {
        a[3].mul_add(b[3], a[2].mul_add(b[2], a[1].mul_add(b[1], a[0] * b[0])))
    }
}

fn acl_rotation(start: [f32; 4], end: [f32; 4], alpha: f32, single_root: bool) -> [f32; 4] {
    let end = if acl_dot(start, end, single_root).is_sign_negative() {
        end.map(|value| -value)
    } else {
        end
    };
    let rotation = acl_lerp(start, end, alpha);
    let inverse_length = acl_dot(rotation, rotation, single_root).sqrt().recip();
    rotation.map(|value| value * inverse_length)
}

impl PreparedAcl {
    fn sequence(&self, clip: &GraphClip) -> Option<&PreparedSequence> {
        let key = clip_source_key(clip)?;
        let index = self
            .sequences
            .binary_search_by(|sequence| sequence.key().cmp(&key))
            .ok()?;
        self.sequences.get(index)
    }

    fn key_value(&self, index: usize) -> f32 {
        let start = index * size_of::<f32>();
        f32::from_le_bytes(
            self.keys[start..start + size_of::<f32>()]
                .try_into()
                .expect("Generated ACL key"),
        )
    }

    fn channels<const N: usize>(
        &self,
        (offset, mode): (usize, u8),
        (first, second, alpha): (usize, usize, f32),
        default: [f32; N],
        interpolate: impl FnOnce([f32; N], [f32; N], f32) -> [f32; N],
    ) -> [f32; N] {
        let key = |index| std::array::from_fn(|axis| self.key_value(offset + index * N + axis));
        match mode {
            0 => default,
            1 => key(0),
            2 => interpolate(key(first), key(second), alpha),
            _ => unreachable!("Invalid prepared ACL bone mode"),
        }
    }

    fn reference_pose(&self, additive: bool) -> SourcePose {
        SourcePose {
            bones: self.bone_references.iter().zip(&self.bone_ids)
                .map(|(bone, &id)| (id, Self::initial_transform(bone, additive)))
                .collect(),
            length_scale: self.source_length_to_runtime_scale,
        }
    }

    fn source_pose(&self, sequence: &PreparedSequence, time: f32) -> SourcePose {
        let additive = sequence.pose_kind() != PreparedPoseKind::Absolute;
        let mut pose = self.reference_pose(additive);
        let samples = sequence.pose_samples(time);
        for track in &sequence.bone.tracks {
            pose.bones[track.0].1 = self.sample_transform(track, samples, sequence, additive);
        }
        if let Some(lock) = sequence.forced_root_lock {
            let mut root = match lock {
                SourceRootLock::RefPose => self.bone_references[0].mesh_pose.rig_transform(),
                SourceRootLock::AnimFirstFrame => self.root_track(sequence, 0.0),
                SourceRootLock::Zero => RigTransform::default(),
            };
            if additive && lock != SourceRootLock::AnimFirstFrame {
                root.scale -= DVec3::ONE;
            }
            pose.bones[0].1 = root;
        }
        pose
    }

    fn root_track(&self, sequence: &PreparedSequence, time: f32) -> RigTransform {
        let additive = sequence.pose_kind() != PreparedPoseKind::Absolute;
        if sequence.single_root {
            self.sample_transform(
                &sequence.bone.tracks[0],
                sequence.pose_samples(time),
                sequence,
                additive,
            )
        } else {
            Self::initial_transform(&self.bone_references[0], additive)
        }
    }

    fn initial_transform(bone: &PreparedBone, additive: bool) -> RigTransform {
        if additive {
            RigTransform {
                scale: DVec3::ZERO,
                ..RigTransform::default()
            }
        } else {
            bone.mesh_pose.rig_transform()
        }
    }

    fn sample_transform(
        &self,
        &PreparedBoneTrack(index, rotation, translation, scaling): &PreparedBoneTrack,
        samples: (usize, usize, f32),
        sequence: &PreparedSequence,
        additive: bool,
    ) -> RigTransform {
        let reference = &self.bone_references[index].skeleton_pose;
        let single_root = sequence.single_root && index == 0;
        let rotation_default = if additive {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            reference.rotation.values().map(|value| value as f32)
        };
        let translation_default = if additive {
            [0.0; 3]
        } else {
            reference.translation.values().map(|value| value as f32)
        };
        let rotation = self.channels(rotation, samples, rotation_default, |start, end, alpha| {
            acl_rotation(start, end, alpha, single_root)
        });
        let translation = self.channels(translation, samples, translation_default, acl_lerp);
        let scale = if sequence.bone.has_scale {
            let scale_default = if additive {
                [0.0; 3]
            } else {
                reference.scale.values().map(|value| value as f32)
            };
            self.channels(scaling, samples, scale_default, acl_lerp)
                .map(f64::from)
        } else if additive {
            [0.0; 3]
        } else {
            self.bone_references[index].mesh_pose.scale.values()
        };
        let mut value = RigTransform {
            rotation: DQuat::from_array(rotation.map(f64::from)),
            translation: DVec3::from_array(translation.map(f64::from)),
            scale: DVec3::from_array(scale),
        };
        let reference = &self.bone_references[index];
        if reference.animation_relative && !additive {
            reference.retarget_relative(&mut value);
        }
        value
    }

    fn curves(&self, prepared: &PreparedSequence, time: f32) -> SourceCurveValues {
        let Some(curves) = &prepared.curve else {
            return SourceCurveValues::default();
        };
        let (first, second, alpha) = curves.clock.samples(prepared.clip_time(time));
        curves
            .tracks
            .iter()
            .map(|(name, (offset, mode))| {
                let value = match mode {
                    1 => self.key_value(*offset),
                    2 => {
                        let start = self.key_value(offset + first);
                        let end = self.key_value(offset + second);
                        end.mul_add(alpha, start - start * alpha)
                    }
                    _ => unreachable!("Invalid prepared ACL curve mode"),
                };
                let scale = self.curve_value_scales.get(name).copied().unwrap_or(1.0);
                (name.clone(), value * scale)
            })
            .collect()
    }
}

pub(crate) fn prepared_reference_pose(skeleton: &Handle<Skeleton>) -> Option<SourcePose> {
    skeleton.path()
        .filter(|path| path.path() == Path::new("ALS/Character/SKM_Als.skn.ron"))
        .map(|_| PREPARED_ACL.reference_pose(false))
}

pub(crate) fn sample_prepared_source_pose(
    clip: &GraphClip,
    time: f32,
) -> Option<(SourcePose, PreparedPoseKind)> {
    if let Some(sequence) = PREPARED_ACL.sequence(clip) {
        Some((
            PREPARED_ACL.source_pose(sequence, time),
            sequence.pose_kind(),
        ))
    } else {
        per_track::sample_source_pose(clip, time).map(|pose| (pose, PreparedPoseKind::Absolute))
    }
}

pub(crate) fn sample_prepared_pose(
    clip: &GraphClip,
    time: f32,
) -> Option<(Pose, PreparedPoseKind)> {
    let (source, kind) = sample_prepared_source_pose(clip, time)?;
    let mut pose = Pose {
        timestamp: time,
        skeleton: clip.skeleton.clone(),
        ..Pose::default()
    };
    source.project_into(&mut pose);
    Some((pose, kind))
}

pub(crate) fn has_prepared_pose(clip: &GraphClip) -> bool {
    PREPARED_ACL.sequence(clip).is_some() || per_track::has_clip(clip)
}

pub(crate) fn write_raw_control_pose(clip: &GraphClip, time: f32, pose: &mut Pose) {
    let Some(controls) = raw_control_sample(clip, time) else {
        return;
    };
    if PREPARED_ACL.sequence(clip).is_some() {
        let data = &*PREPARED_ACL;
        for (index, control) in controls.bones() {
            data.bone_references[index].write_raw_control(
                pose,
                data.bone_ids[index],
                control,
                data.source_length_to_runtime_scale,
            );
        }
    } else if per_track::has_clip(clip) {
        per_track::write_raw_control_pose(pose, &controls);
    }
}

pub(crate) fn sample_prepared_root(clip: &GraphClip, root: BoneId, time: f32) -> Option<Transform> {
    let data = &*PREPARED_ACL;
    let sequence = data.sequence(clip)?;
    if root != *data.bone_ids.first()? || !sequence.single_root {
        return None;
    }
    let mut root = BonePose::default();
    project_bone_transform(
        &mut root,
        data.root_track(sequence, time),
        data.source_length_to_runtime_scale,
    );
    Some(root.to_transform())
}

pub(crate) fn sample_source_curves(
    clip: Option<&GraphClip>,
    raw: &AlsPoseCurves,
    time: f32,
) -> SourceCurveValues {
    if let Some(prepared) = clip.and_then(|clip| PREPARED_ACL.sequence(clip)) {
        PREPARED_ACL.curves(prepared, time)
    } else if clip.is_some_and(per_track::has_clip) {
        SourceCurveValues::default()
    } else {
        clip.and_then(|clip| evaluate_raw_curves(clip, time))
            .unwrap_or_else(|| raw.sample_all(time))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::als_curves::AlsFloatCurve;

    fn clip(path: &'static str, animation: &str) -> GraphClip {
        GraphClip {
            source: Some(GraphClipSource::GltfNamed {
                path: path.into(),
                animation_name: animation.to_owned(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn prepared_curves_resolve_the_clip_and_keep_empty_results_authoritative() {
        let foot = SourceCurveName::from("FootPlanted");
        let raw = AlsPoseCurves(HashMap::from([(
            foot.clone(),
            AlsFloatCurve {
                times: vec![0.0],
                values: vec![42.0],
                ..Default::default()
            },
        )]));
        let run = "ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.gltf";
        let resolved = clip(run, "A_Als_Run_Right_Backward_0");
        let unrelated = clip(run, "Unrelated");
        let empty = clip(
            "ALS/Animations/Grounded/Lean/A_Als_Lean_Forward.gltf",
            "A_Als_Lean_Forward_0",
        );
        assert_eq!(
            sample_source_curves(Some(&resolved), &raw, 0.244)[&foot],
            0.24926704
        );
        assert_eq!(
            sample_source_curves(Some(&unrelated), &raw, 0.244)[&foot],
            42.0
        );
        assert_eq!(sample_source_curves(None, &raw, 0.244)[&foot], 42.0);
        assert!(sample_source_curves(Some(&empty), &raw, 0.0).is_empty());
    }

    #[test]
    fn prepared_curves_keep_signed_slot_values_and_target_heading() {
        let transition = clip(
            "ALS/Animations/Transitions/A_Als_Crouch_Transition_Right.gltf",
            "A_Als_Crouch_Transition_Right_0",
        );
        let turn = clip(
            "ALS/Animations/TurnInPlace/A_Als_Turn_90_Left.gltf",
            "A_Als_Turn_90_Left_0",
        );
        let raw = AlsPoseCurves::default();
        let locks = sample_source_curves(Some(&transition), &raw, 0.3);
        for lock in ["FootLeftLock", "FootRightLock"] {
            assert_eq!(locks[&SourceCurveName::from(lock)], -f32::EPSILON);
        }
        let yaw = sample_source_curves(Some(&turn), &raw, 0.4);
        assert_eq!(yaw[&SourceCurveName::from("RotationYawSpeed")], 111.28509);
    }

    #[test]
    fn prepared_pose_preserves_baked_additive_and_mesh_reference_channels() {
        let look = clip(
            "ALS/Animations/View/A_Als_Look_Forward.gltf",
            "A_Als_Look_Forward_0",
        );
        let stand = clip(
            "ALSRUNTIME/Animations/Base/A_Als_Stand_Pose.gltf",
            "A_Als_Stand_Pose_0",
        );
        let (look, kind) = sample_prepared_pose(&look, 0.5).unwrap();
        let pelvis = EntityPath::from_slashed_string("GltfNode0/root/pelvis".to_owned()).id();
        assert!(kind == PreparedPoseKind::MeshAdditive);
        assert_eq!(look.paths.len(), 79);
        assert_eq!(
            look.get_bone(pelvis).unwrap().translation.unwrap().x,
            3.921628e-5
        );
        assert!(look.bones.iter().all(|bone| bone.scale == Some(Vec3::ZERO)));

        let (stand, kind) = sample_prepared_pose(&stand, 0.0).unwrap();
        let virtual_foot_root =
            EntityPath::from_slashed_string("GltfNode0/root/VB foot_root".to_owned()).id();
        assert!(kind == PreparedPoseKind::Absolute);
        assert_eq!(
            stand.get_bone(virtual_foot_root).unwrap().scale,
            Some(Vec3::ONE)
        );
    }

    #[test]
    fn prepared_root_keeps_unmasked_track_when_pose_is_locked() {
        let roll = clip(
            "ALS/Animations/Actions/Roll/A_Als_Roll.gltf",
            "A_Als_Roll_0",
        );
        let root = EntityPath::from_slashed_string("GltfNode0/root".to_owned()).id();
        let single = sample_prepared_root(&roll, root, 0.1).unwrap();
        let (pose, _) = sample_prepared_pose(&roll, 0.1).unwrap();
        assert_eq!(single.translation.z, 0.0719982767);
        assert_eq!(pose.get_bone(root).unwrap().translation, Some(Vec3::ZERO));
    }

    #[test]
    fn prepared_bow_uses_the_per_track_frame_table() {
        let bow = clip(
            "ALS/OverlayObjects/Bow/A_Als_Bow_Draw.gltf",
            "A_Als_Bow_Draw_0",
        );
        let (pose, kind) = sample_prepared_pose(&bow, 0.505).unwrap();
        let bone = EntityPath::from_slashed_string("GltfNode0/root/spine_down_01".to_owned()).id();
        let rotation = pose.get_bone(bone).unwrap().rotation.unwrap();
        assert!(kind == PreparedPoseKind::Absolute);
        assert_eq!(pose.bones.len(), 24);
        assert_eq!(
            rotation,
            Quat::from_xyzw(0.0, 0.026351282, -0.99965274, 2.3107445e-5)
        );
        assert!(sample_source_curves(Some(&bow), &AlsPoseCurves::default(), 0.505).is_empty());
    }

    #[test]
    fn raw_reference_retention_uses_the_fk_control_before_gltf_writeback() {
        let crouch = clip(
            "ALSRUNTIME/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward.gltf",
            "A_Als_Crouch_Walk_Forward_0",
        );
        let bone = EntityPath::from_slashed_string(
            "GltfNode0/root/pelvis/thigh_l/calf_l/calf_twist_01_l".to_owned(),
        )
        .id();
        let sampled = Transform {
            translation: Vec3::new(-0.20476779, 6.915116e-9, -1.2133295e-8),
            rotation: Quat::from_xyzw(0.0029125006, -0.00761252, 0.001934104, 0.9999649),
            scale: Vec3::new(1.0000001, 1.0000001, 1.0000005),
        };
        let mut pose = Pose::default();
        pose.add_bone(BonePose::from_transform(sampled), bone);
        write_raw_control_pose(&crouch, 0.53333336, &mut pose);
        let written = pose.get_bone(bone).unwrap().to_transform();

        write_raw_control_pose(&crouch, 0.50371206, &mut pose);
        let actual = pose.get_bone(bone).unwrap().to_transform();
        assert_ne!(written.rotation, actual.rotation);
        assert_eq!(
            actual.translation,
            Vec3::new(-0.20476779, 7.781708e-9, -1.4251387e-8)
        );
        assert_eq!(actual.scale, sampled.scale);
        assert_eq!(
            actual.rotation,
            Quat::from_xyzw(-0.0028089457, 0.0076127234, -0.001933307, -0.9999652)
        );
    }

    #[test]
    fn prepared_relative_foot_is_retargeted_before_projection() {
        let crouch = clip(
            "ALSRUNTIME/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.gltf",
            "A_Als_Crouch_Walk_Left_Backward_0",
        );
        let foot =
            EntityPath::from_slashed_string("GltfNode0/root/ik_foot_root/ik_foot_r".to_owned())
                .id();
        let (pose, _) = sample_prepared_pose(&crouch, 0.0).unwrap();
        assert_eq!(
            pose.get_bone(foot).unwrap().rotation,
            Some(Quat::from_xyzw(
                -0.5124624,
                0.6854021,
                -0.43448853,
                0.2807597
            ))
        );
    }
}
