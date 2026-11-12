use std::{mem::size_of, path::PathBuf, sync::LazyLock};

use bevy::{
    math::{DQuat, DVec3, Quat},
    prelude::VariableCurve,
};
use bevy_animation_graph::core::animation_clip::GraphClip;
use serde::Deserialize;

use crate::extensions::control_rig_hierarchy::{RigTransform, rig_normalize};
use crate::extensions::control_rig_math::{rig_euler, rig_quaternion, rotation_basis};
use crate::extensions::source_compression::{clip_source_key, source_frame_time};

#[derive(Deserialize)]
struct RotationTimeline(PathBuf, String, Vec<f32>);

static TIMELINES: LazyLock<Vec<RotationTimeline>> = LazyLock::new(|| {
    let mut timelines: Vec<RotationTimeline> = serde_json::from_str(include_str!(
        "../../assets/ALSRuntime/Animations/als_raw_rotation_timelines.json"
    ))
    .expect("Generated raw rotation timelines");
    timelines.sort_unstable_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));
    timelines
});

#[derive(Clone, Copy)]
pub(crate) struct RawRotationInterval {
    start: f32,
    end: f32,
    alpha: f32,
}

pub(crate) fn raw_rotation_interval(clip: &GraphClip, time: f32) -> Option<RawRotationInterval> {
    let key = clip_source_key(clip)?;
    let index = TIMELINES
        .binary_search_by(|timeline| (timeline.0.as_path(), timeline.1.as_str()).cmp(&key))
        .ok()?;
    let times = &TIMELINES[index].2;
    let second = times.partition_point(|key| *key <= time);
    let start = *times.get(second.checked_sub(1)?)?;
    let end = *times.get(second)?;
    Some(RawRotationInterval {
        start,
        end,
        alpha: (time - start) / (end - start),
    })
}

fn interpolate_rotation(start: DQuat, end: DQuat, alpha: f32) -> DQuat {
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

fn euler_roundtrip(rotation: DQuat) -> DQuat {
    rig_quaternion(rig_euler(rotation).as_vec3().as_dvec3())
}

impl RawRotationInterval {
    pub(crate) fn sample(self, curve: &VariableCurve) -> Option<Quat> {
        let key = |time| {
            curve
                .0
                .sample_clamped(time)
                .downcast::<Quat>()
                .ok()
                .map(|value| *value)
        };
        if self.alpha <= 1e-8 {
            return key(self.start);
        }
        let blended = if self.alpha <= 1e-5 {
            key(self.start)?.as_dquat()
        } else if (self.alpha - 1.0).abs() <= 1e-5 {
            key(self.end)?.as_dquat()
        } else {
            rotation_basis(interpolate_rotation(
                rotation_basis(key(self.start)?.as_dquat()),
                rotation_basis(key(self.end)?.as_dquat()),
                self.alpha,
            ))
        };
        Some(euler_roundtrip(blended).as_quat())
    }
}

#[derive(Deserialize)]
struct RawControlSequence {
    gltf: PathBuf,
    animation: String,
    duration: f32,
    step: bool,
    frame_rate: [u32; 2],
    frame_count: usize,
    tracks: Vec<(usize, [usize; 2])>,
}

struct RawControls {
    sequences: Vec<RawControlSequence>,
    keys: &'static [u8],
}

static RAW_CONTROLS: LazyLock<RawControls> = LazyLock::new(|| {
    let data = include_bytes!("../../assets/ALSRuntime/Animations/als_raw_fk_controls.bin");
    let (length, payload) = data.split_at(4);
    let length = u32::from_le_bytes(length.try_into().expect("Raw control header length")) as usize;
    let (header, keys) = payload.split_at(length);
    let mut sequences: Vec<RawControlSequence> =
        serde_json::from_slice(header).expect("Generated raw FK controls");
    sequences.sort_unstable_by(|left, right| {
        (&left.gltf, &left.animation).cmp(&(&right.gltf, &right.animation))
    });
    RawControls { sequences, keys }
});

pub(crate) struct RawControlSample {
    sequence: &'static RawControlSequence,
    keys: &'static [u8],
    first: usize,
    second: usize,
    alpha: f32,
}

pub(crate) fn raw_control_sample(clip: &GraphClip, time: f32) -> Option<RawControlSample> {
    let key = clip_source_key(clip)?;
    let data = &*RAW_CONTROLS;
    let index = data
        .sequences
        .binary_search_by(|sequence| {
            (sequence.gltf.as_path(), sequence.animation.as_str()).cmp(&key)
        })
        .ok()?;
    let sequence = &data.sequences[index];
    let (frame, fraction) =
        source_frame_time(time.max(0.0).min(sequence.duration), sequence.frame_rate);
    let last = sequence.frame_count - 1;
    let first = (frame as usize).min(last);
    let second = (first + 1).min(last);
    let alpha = if sequence.step || first == second {
        0.0
    } else {
        fraction
    };
    Some(RawControlSample {
        sequence,
        keys: data.keys,
        first,
        second,
        alpha,
    })
}

impl RawControlSample {
    fn key(&self, [offset, count]: [usize; 2], frame: usize) -> RigTransform {
        let base = offset + 9 * frame.min(count - 1);
        let values: [f64; 9] = std::array::from_fn(|index| {
            let start = (base + index) * size_of::<f32>();
            f64::from(f32::from_le_bytes(
                self.keys[start..start + size_of::<f32>()]
                    .try_into()
                    .expect("Generated raw FK key"),
            ))
        });
        let [x, y, z, roll, pitch, yaw, sx, sy, sz] = values;
        RigTransform {
            translation: DVec3::new(x, y, z),
            rotation: rotation_basis(rig_quaternion(DVec3::new(roll, -yaw, pitch))),
            scale: DVec3::new(sx, sy, sz),
        }
    }

    fn control(&self, span: [usize; 2]) -> RigTransform {
        let start = self.key(span, self.first);
        if self.alpha <= 1e-8 {
            return RigTransform {
                rotation: normalize_source_rotation(start.rotation),
                ..start
            };
        }
        let mut blended = if self.alpha <= 1e-5 {
            start
        } else {
            let end = self.key(span, self.second);
            if (self.alpha - 1.0).abs() <= 1e-5 {
                end
            } else {
                let weight = f64::from(self.alpha);
                RigTransform {
                    translation: start.translation + (end.translation - start.translation) * weight,
                    rotation: interpolate_rotation(start.rotation, end.rotation, self.alpha),
                    scale: start.scale + (end.scale - start.scale) * weight,
                }
            }
        };
        blended.translation = blended.translation.as_vec3().as_dvec3();
        blended.rotation = normalize_source_rotation(rotation_basis(euler_roundtrip(
            rotation_basis(blended.rotation),
        )));
        blended.scale = blended.scale.as_vec3().as_dvec3();
        blended
    }

    pub(crate) fn bones(&self) -> impl Iterator<Item = (usize, RigTransform)> + '_ {
        self.sequence
            .tracks
            .iter()
            .map(|&(index, span)| (index, self.control(span)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        animation::{animated_field, animation_curves::AnimatableCurve},
        prelude::{AnimationClip, Transform, UnevenSampleAutoCurve},
    };
    use bevy_animation_graph::core::animation_clip::{EntityPath, loader::GraphClipSource};

    fn clip_with_rotation(
        bone: bevy_animation_graph::core::id::BoneId,
        path: &'static str,
        animation_name: &str,
        keys: [(f32, Quat); 2],
    ) -> GraphClip {
        let mut animation = AnimationClip::default();
        animation.add_curve_to_target(
            bone.animation_target_id(),
            AnimatableCurve::new(
                animated_field!(Transform::rotation),
                UnevenSampleAutoCurve::new(keys).unwrap(),
            ),
        );
        let mut clip =
            GraphClip::from_bevy_clip(animation, Default::default(), Default::default(), None);
        clip.source = Some(GraphClipSource::GltfNamed {
            path: path.into(),
            animation_name: animation_name.to_owned(),
        });
        clip
    }

    #[test]
    fn raw_pose_interpolates_the_installed_sprint_interval_with_nlerp() {
        let bone = EntityPath::from_slashed_string("ik_hand_l".to_owned()).id();
        let mut clip = clip_with_rotation(
            bone,
            "ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.gltf",
            "A_Als_Sprint_Acceleration_0",
            [
                (
                    0.33333334,
                    Quat::from_xyzw(-0.06315641, 0.5647191, 0.7948048, 0.21304677),
                ),
                (
                    0.36666667,
                    Quat::from_xyzw(0.076489635, -0.5301, -0.82374704, 0.1859681),
                ),
            ],
        );
        let sample = |clip: &GraphClip| {
            crate::extensions::source_sequence_player::sample_pose(clip, 0.35955963)
                .get_bone(bone)
                .unwrap()
                .rotation
                .unwrap()
        };
        let actual = sample(&clip);
        let expected = Quat::from_xyzw(-0.07466473, 0.54490983, 0.82887614, -0.10228802);
        assert!(actual.abs_diff_eq(expected, 1e-7));
        clip.source = None;
        let legacy = sample(&clip);
        let aligned = if actual.dot(legacy) < 0.0 {
            -legacy
        } else {
            legacy
        };
        assert!((actual.w - aligned.w).abs() > 0.001);
    }

    #[test]
    fn raw_pose_round_trips_the_singular_crouch_rotation_through_euler() {
        let bone = EntityPath::from_slashed_string("ik_foot_r".to_owned()).id();
        let clip = clip_with_rotation(
            bone,
            "ALSRuntime/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Forward.gltf",
            "A_Als_Crouch_Walk_Left_Forward_0",
            [
                (
                    0.76666665,
                    Quat::from_xyzw(0.69643927, -0.69643927, -0.12236161, 0.12236161),
                ),
                (
                    0.79999995,
                    Quat::from_xyzw(-0.6960472, 0.69667786, 0.12457539, -0.12099273),
                ),
            ],
        );
        let pose = crate::extensions::source_sequence_player::sample_pose(&clip, 0.77562714);
        let actual = pose.get_bone(bone).unwrap().rotation.unwrap();
        assert!(actual.abs_diff_eq(
            Quat::from_xyzw(0.69649875, -0.69649875, -0.122022554, 0.122022554),
            1e-7
        ));
    }
}
