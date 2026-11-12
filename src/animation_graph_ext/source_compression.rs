use std::{
    collections::HashMap,
    mem::size_of,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use bevy_animation_graph::core::animation_clip::{GraphClip, loader::GraphClipSource};
use serde::Deserialize;

use crate::extensions::als_curves::{AlsPoseCurves, SourceCurveName, SourceCurveValues};

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
struct PreparedSequence {
    gltf: PathBuf,
    animation: String,
    curve: Option<PreparedCurves>,
}

impl PreparedSequence {
    fn key(&self) -> (&Path, &str) {
        (&self.gltf, &self.animation)
    }
}

#[derive(Deserialize)]
struct PreparedMetadata {
    format: String,
    curve_value_scales: HashMap<SourceCurveName, f32>,
    sequences: Vec<PreparedSequence>,
}

struct PreparedAcl {
    metadata: PreparedMetadata,
    keys: &'static [u8],
}

static PREPARED_ACL: LazyLock<PreparedAcl> = LazyLock::new(|| {
    let data = include_bytes!("../../assets/ALSRuntime/Animations/als_prepared_acl.bin");
    let (header, payload) = data.split_at(4);
    let length = u32::from_le_bytes(header.try_into().expect("ACL header length")) as usize;
    let (metadata, keys) = payload.split_at(length);
    let mut metadata: PreparedMetadata =
        serde_json::from_slice(metadata).expect("Generated ACL metadata");
    assert_eq!(metadata.format, "als_prepared_acl_v1");
    metadata
        .sequences
        .sort_unstable_by(|left, right| left.key().cmp(&right.key()));
    PreparedAcl { metadata, keys }
});

impl PreparedAcl {
    fn sequence(&self, clip: &GraphClip) -> Option<&PreparedSequence> {
        let GraphClipSource::GltfNamed {
            path,
            animation_name,
        } = clip.source.as_ref()?;
        if path.source().as_str().is_some() {
            return None;
        }
        let key = (path.path(), animation_name.as_str());
        let index = self
            .metadata
            .sequences
            .binary_search_by(|sequence| sequence.key().cmp(&key))
            .ok()?;
        self.metadata.sequences.get(index)
    }

    fn key_value(&self, index: usize) -> f32 {
        let start = index * size_of::<f32>();
        f32::from_le_bytes(
            self.keys[start..start + size_of::<f32>()]
                .try_into()
                .expect("Generated ACL key"),
        )
    }

    fn curves(&self, prepared: &PreparedSequence, time: f32) -> SourceCurveValues {
        let Some(curves) = &prepared.curve else {
            return SourceCurveValues::default();
        };
        let (first, second, alpha) = curves.clock.samples(time);
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
                let scale = self
                    .metadata
                    .curve_value_scales
                    .get(name)
                    .copied()
                    .unwrap_or(1.0);
                (name.clone(), value * scale)
            })
            .collect()
    }
}

pub(crate) fn sample_source_curves(
    clip: Option<&GraphClip>,
    raw: &AlsPoseCurves,
    time: f32,
) -> SourceCurveValues {
    clip.and_then(|clip| PREPARED_ACL.sequence(clip))
        .map_or_else(
            || raw.sample_all(time),
            |prepared| PREPARED_ACL.curves(prepared, time),
        )
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
}
