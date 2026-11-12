use std::{path::PathBuf, sync::LazyLock};

use bevy::{
    math::{DQuat, Quat},
    prelude::VariableCurve,
};
use bevy_animation_graph::core::animation_clip::GraphClip;
use serde::Deserialize;

use crate::extensions::source_compression::clip_source_key;

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

fn interpolate_rotation(start: Quat, end: Quat, alpha: f32) -> Quat {
    let start = start.as_dquat();
    let end = end.as_dquat();
    let alpha = f64::from(alpha);
    let bias = if start.dot(end) < 0.0 { -1.0 } else { 1.0 };
    let left = start.to_array();
    let right = end.to_array();
    DQuat::from_array(std::array::from_fn(|axis| {
        (left[axis] * bias).mul_add(1.0 - alpha, right[axis] * alpha)
    }))
    .normalize()
    .as_quat()
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
        if self.alpha <= 1e-5 {
            return key(self.start);
        }
        if (self.alpha - 1.0).abs() <= 1e-5 {
            return key(self.end);
        }
        Some(interpolate_rotation(
            key(self.start)?,
            key(self.end)?,
            self.alpha,
        ))
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

    #[test]
    fn raw_pose_interpolates_the_installed_sprint_interval_with_nlerp() {
        let bone = EntityPath::from_slashed_string("ik_hand_l".to_owned()).id();
        let mut animation = AnimationClip::default();
        animation.add_curve_to_target(
            bone.animation_target_id(),
            AnimatableCurve::new(
                animated_field!(Transform::rotation),
                UnevenSampleAutoCurve::new([
                    (
                        0.33333334,
                        Quat::from_xyzw(-0.06315641, 0.5647191, 0.7948048, 0.21304677),
                    ),
                    (
                        0.36666667,
                        Quat::from_xyzw(0.076489635, -0.5301, -0.82374704, 0.1859681),
                    ),
                ])
                .unwrap(),
            ),
        );
        let mut clip =
            GraphClip::from_bevy_clip(animation, Default::default(), Default::default(), None);
        clip.source = Some(GraphClipSource::GltfNamed {
            path: "ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.gltf".into(),
            animation_name: "A_Als_Sprint_Acceleration_0".to_owned(),
        });
        let sample = |clip: &GraphClip| {
            crate::extensions::source_sequence_player::sample_pose(clip, 0.35955963)
                .get_bone(bone)
                .unwrap()
                .rotation
                .unwrap()
        };
        let actual = sample(&clip);
        let expected = Quat::from_xyzw(0.07466473, -0.54490983, -0.82887614, 0.10228802);
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
}
