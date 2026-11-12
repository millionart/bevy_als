use std::{path::PathBuf, sync::LazyLock};

use bevy::{math::DQuat, prelude::Transform};
use bevy_animation_graph::core::{
    animation_clip::GraphClip,
    id::BoneId,
    pose::{BonePose, Pose},
};
use serde::{Deserialize, Deserializer};

use crate::extensions::raw_sequence_sampling::RawControlSample;

use super::{
    PreparedBone, bone_id, clip_source_key, project_rotation, project_scale, project_translation,
    source_frame_time,
};

#[derive(Deserialize)]
struct Track<T> {
    keys: Vec<T>,
    frames: Vec<u16>,
}

impl<T: Copy> Track<T> {
    fn sample(
        &self,
        position: f32,
        last_frame: usize,
        step: bool,
        interpolate: impl FnOnce(T, T, f32) -> T,
    ) -> T {
        let last_key = self.keys.len() - 1;
        if last_key == 0 || position <= 0.0 {
            return self.keys[0];
        }
        if position >= 1.0 {
            return self.keys[last_key];
        }
        let (first, second, alpha) = if self.frames.is_empty() {
            let index = position * last_key as f32;
            let first = (index as usize).min(last_key);
            (first, (first + 1).min(last_key), index - first as f32)
        } else {
            let frame = position * last_frame as f32;
            let preceding_frame = (frame as usize).min(last_frame - 1);
            let first = self
                .frames
                .partition_point(|key| usize::from(*key) <= preceding_frame)
                - 1;
            let second = first + 1;
            let low = self.frames[first];
            let high = self.frames[second];
            (
                first,
                second,
                (frame - f32::from(low)) / f32::from(high - low),
            )
        };
        if first == second {
            return self.keys[first];
        }
        interpolate(
            self.keys[first],
            self.keys[second],
            if step { 0.0 } else { alpha },
        )
    }
}

fn bone_id_from_path<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BoneId, D::Error> {
    Vec::<String>::deserialize(deserializer).map(bone_id)
}

#[derive(Deserialize)]
struct TrackBone {
    #[serde(rename = "path", deserialize_with = "bone_id_from_path")]
    id: BoneId,
    reference: PreparedBone,
    rotation: Track<[f32; 4]>,
    translation: Track<[f32; 3]>,
    scale: Track<[f32; 3]>,
}

#[derive(Deserialize)]
struct PreparedPerTrack {
    format: String,
    gltf: PathBuf,
    animation: String,
    duration: f32,
    step: bool,
    frame_rate: [u32; 2],
    frame_count: usize,
    source_length_to_runtime_scale: f64,
    bones: Vec<TrackBone>,
}

static PREPARED: LazyLock<PreparedPerTrack> = LazyLock::new(|| {
    let clip: PreparedPerTrack = serde_json::from_str(include_str!(
        "../../assets/ALSRuntime/OverlayObjects/Bow/als_prepared_per_track.json"
    ))
    .expect("Generated PerTrack clip");
    assert_eq!(clip.format, "als_prepared_per_track_v1");
    clip
});

fn vector_lerp(a: [f32; 3], b: [f32; 3], alpha: f32) -> [f32; 3] {
    std::array::from_fn(|axis| a[axis] + (b[axis] - a[axis]) * alpha)
}

fn rotation_lerp(a: [f32; 4], b: [f32; 4], alpha: f32) -> [f32; 4] {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let weight = (1.0 - alpha) * if dot < 0.0 { -1.0 } else { 1.0 };
    std::array::from_fn(|axis| b[axis] * alpha + a[axis] * weight)
}

pub(super) fn has_clip(clip: &GraphClip) -> bool {
    clip_source_key(clip) == Some((PREPARED.gltf.as_path(), PREPARED.animation.as_str()))
}

pub(super) fn retain_raw_reference_pose(pose: &mut Pose, controls: &RawControlSample) {
    for (index, control) in controls.bones() {
        let bone = &PREPARED.bones[index];
        bone.reference.retain_raw_reference(
            pose,
            bone.id,
            control,
            PREPARED.source_length_to_runtime_scale,
        );
    }
}

pub(super) fn sample_pose(clip: &GraphClip, time: f32) -> Option<Pose> {
    if !has_clip(clip) {
        return None;
    }
    let data = &*PREPARED;
    let last_frame = data.frame_count - 1;
    let (frame, fraction) = source_frame_time(time.max(0.0).min(data.duration), data.frame_rate);
    let position = ((frame + f64::from(fraction)) / last_frame as f64).min(1.0) as f32;
    let mut pose = Pose {
        timestamp: time,
        skeleton: clip.skeleton.clone(),
        ..Pose::default()
    };
    for bone in &data.bones {
        let rotation = bone
            .rotation
            .sample(position, last_frame, data.step, rotation_lerp);
        let rotation = DQuat::from_array(rotation.map(f64::from))
            .normalize()
            .to_array();
        let translation = bone
            .translation
            .sample(position, last_frame, data.step, vector_lerp);
        let scale = bone
            .scale
            .sample(position, last_frame, data.step, vector_lerp);
        pose.add_bone(
            BonePose::from_transform(Transform {
                rotation: project_rotation(rotation),
                translation: project_translation(
                    translation.map(f64::from),
                    data.source_length_to_runtime_scale,
                ),
                scale: project_scale(scale.map(f64::from)),
            }),
            bone.id,
        );
    }
    Some(pose)
}
