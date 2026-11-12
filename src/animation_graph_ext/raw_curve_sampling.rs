use std::{path::PathBuf, sync::LazyLock};

use bevy_animation_graph::core::animation_clip::GraphClip;
use serde::Deserialize;

use crate::extensions::{
    als_curves::{SourceCurveName, SourceCurveValues},
    source_compression::{clip_source_key, convert_source_frame_time, source_frame_time},
};

#[derive(Deserialize)]
enum Segment {
    Linear([u64; 2]),
    Cubic([u64; 4]),
}

impl Segment {
    fn evaluate(&self, offset: f64, length: f64) -> f32 {
        match *self {
            Self::Linear([start, slope]) => {
                (f64::from_bits(start) + offset * f64::from_bits(slope)) as f32
            }
            Self::Cubic(points) => {
                let [a, b, c, d] = points.map(f64::from_bits);
                let alpha = f64::from((offset / length) as f32);
                let lerp = |start, end| start + (end - start) * alpha;
                let ab = lerp(a, b);
                let bc = lerp(b, c);
                let cd = lerp(c, d);
                lerp(lerp(ab, bc), lerp(bc, cd)) as f32
            }
        }
    }
}

#[derive(Deserialize)]
struct RawCurve {
    name: SourceCurveName,
    tick_rate: [u32; 2],
    frames: Vec<i32>,
    endpoints: [f32; 2],
    segments: Vec<Segment>,
}

impl RawCurve {
    fn sample(&self, source_time: (f64, f32), source_rate: [u32; 2]) -> f32 {
        let (frame, fraction) = convert_source_frame_time(source_time, source_rate, self.tick_rate);
        let next = self.frames.partition_point(|key| f64::from(*key) <= frame);
        if next == 0 {
            return self.endpoints[0];
        }
        if next == self.frames.len() {
            return self.endpoints[1];
        }
        let start = f64::from(self.frames[next - 1]);
        let length = f64::from(self.frames[next]) - start;
        self.segments[next - 1].evaluate(frame - start + f64::from(fraction), length)
    }
}

#[derive(Deserialize)]
struct RawSequenceCurves {
    gltf: PathBuf,
    animation: String,
    duration: f32,
    frame_rate: [u32; 2],
    curves: Vec<RawCurve>,
}

static CURVES: LazyLock<Vec<RawSequenceCurves>> = LazyLock::new(|| {
    let mut curves: Vec<RawSequenceCurves> = serde_json::from_str(include_str!(
        "../../assets/ALSRuntime/Animations/als_raw_sequencer_curves.json"
    ))
    .expect("Generated raw Sequencer curves");
    curves.sort_unstable_by(|left, right| {
        (&left.gltf, &left.animation).cmp(&(&right.gltf, &right.animation))
    });
    curves
});

pub(crate) fn sample_raw_curves(clip: &GraphClip, time: f32) -> Option<SourceCurveValues> {
    let key = clip_source_key(clip)?;
    let index = CURVES
        .binary_search_by(|sequence| {
            (sequence.gltf.as_path(), sequence.animation.as_str()).cmp(&key)
        })
        .ok()?;
    let sequence = &CURVES[index];
    let source_time = source_frame_time(time.max(0.0).min(sequence.duration), sequence.frame_rate);
    Some(
        sequence
            .curves
            .iter()
            .map(|curve| {
                (
                    curve.name.clone(),
                    curve.sample(source_time, sequence.frame_rate),
                )
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_animation_graph::core::animation_clip::loader::GraphClipSource;

    fn curve(path: &'static str, animation: &str, name: &str, time: f32) -> f32 {
        let clip = GraphClip {
            source: Some(GraphClipSource::GltfNamed {
                path: path.into(),
                animation_name: animation.to_owned(),
            }),
            ..Default::default()
        };
        sample_raw_curves(&clip, time).unwrap()[&SourceCurveName::from(name)]
    }

    #[test]
    fn raw_curves_keep_the_source_frame_and_tick_rate() {
        assert_eq!(
            curve(
                "ALSRuntime/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.gltf",
                "A_Als_Crouch_Walk_Right_Backward_0",
                "FeetCrossing",
                0.53025365,
            ),
            0.9076096
        );
        assert_eq!(
            curve(
                "ALS/Animations/Actions/GetUp/A_Als_GetUp_Back.gltf",
                "A_Als_GetUp_Back_0",
                "PoseGrounded",
                0.371,
            ),
            0.60431397
        );
        assert_eq!(
            curve(
                "ALS/Animations/Overlays/Binoculars/A_Als_Binoculars_Poses.gltf",
                "A_Als_Binoculars_Poses_0",
                "HandLeftIk",
                0.044,
            ),
            0.72
        );
    }
}
