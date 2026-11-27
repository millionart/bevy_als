use std::{
    fmt,
    sync::{Arc, LazyLock},
};

use bevy::{
    asset::{AssetPath, Handle},
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use bevy_animation_graph::core::{
    animation_clip::{EntityPath, GraphClip, loader::GraphClipSource},
    id::BoneId,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};
use serde::Deserialize;

use crate::extensions::{SourceCurveName, SourceCurveValues};

#[repr(C)]
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AlsAclTransform {
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for AlsAclTransform {
    fn default() -> Self {
        Self {
            rotation: [0.0, 0.0, 0.0, 1.0],
            translation: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

/// Optional retarget correction in the original Unreal local coordinate system.
#[derive(Debug, Clone, Copy)]
pub struct AlsAclRetarget {
    pub translation_basis: Quat,
    pub translation_scale: f32,
    pub translation_offset: Vec3,
    pub rotation_offset: Quat,
    pub scale_multiplier: Vec3,
}

impl Default for AlsAclRetarget {
    fn default() -> Self {
        Self {
            translation_basis: Quat::IDENTITY,
            translation_scale: 1.0,
            translation_offset: Vec3::ZERO,
            rotation_offset: Quat::IDENTITY,
            scale_multiplier: Vec3::ONE,
        }
    }
}

impl AlsAclRetarget {
    fn animation_relative(authored: AlsAclTransform, target: AlsAclTransform) -> Self {
        Self {
            translation_offset: Vec3::from_array(target.translation)
                - Vec3::from_array(authored.translation),
            rotation_offset: Quat::from_array(authored.rotation).normalize().conjugate()
                * Quat::from_array(target.rotation).normalize(),
            scale_multiplier: Vec3::from_array(std::array::from_fn(|axis| {
                let source = authored.scale[axis];
                if source.abs() <= 1.0e-8 {
                    0.0
                } else {
                    target.scale[axis] / source
                }
            })),
            ..default()
        }
    }

    fn to_bone(self, track: AlsAclTransform) -> BonePose {
        let translation =
            self.translation_basis * Vec3::from_array(track.translation) * self.translation_scale
                + self.translation_offset;
        let rotation = (Quat::from_array(track.rotation) * self.rotation_offset).normalize();
        let scale = Vec3::from_array(track.scale) * self.scale_multiplier;
        BonePose {
            translation: Some(Vec3::new(translation.x, translation.z, translation.y) * 0.01),
            rotation: Some(Quat::from_xyzw(
                -rotation.x,
                -rotation.z,
                -rotation.y,
                rotation.w,
            )),
            scale: Some(Vec3::new(scale.x, scale.z, scale.y)),
            weights: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AlsAclBoneTrack {
    pub bone: BoneId,
    pub reference: AlsAclTransform,
    pub retarget: AlsAclRetarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlsAclError {
    InvalidBuffer,
    TrackCountMismatch,
    DuplicateBone,
    DecodeFailed,
}

impl fmt::Display for AlsAclError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidBuffer => "invalid ACL compressed tracks",
            Self::TrackCountMismatch => "ACL track count does not match its bindings",
            Self::DuplicateBone => "multiple ACL tracks target the same bone",
            Self::DecodeFailed => "ACL sample could not be decoded",
        })
    }
}

impl std::error::Error for AlsAclError {}

#[repr(C, align(16))]
struct AclBlock([u8; 16]);

struct CompressedTracks {
    storage: Box<[AclBlock]>,
    byte_len: usize,
    count: u32,
}

unsafe extern "C" {
    fn als_acl_validate(bytes: *const u8, size: usize, transform: bool, additive: bool) -> u32;
    fn als_acl_decode_pose(
        bytes: *const u8,
        size: usize,
        time: f32,
        step: bool,
        references: *const AlsAclTransform,
        required: *const u8,
        output: *mut AlsAclTransform,
        count: u32,
    ) -> bool;
    fn als_acl_decode_curves(
        bytes: *const u8,
        size: usize,
        time: f32,
        required: *const u8,
        output: *mut f32,
        count: u32,
    ) -> bool;
}

impl CompressedTracks {
    fn new(bytes: &[u8], transform: bool, additive: bool) -> Result<Self, AlsAclError> {
        let storage = bytes
            .chunks(16)
            .map(|chunk| {
                let mut block = AclBlock([0; 16]);
                block.0[..chunk.len()].copy_from_slice(chunk);
                block
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        // The allocation is aligned and spans at least byte_len initialized bytes.
        let count =
            unsafe { als_acl_validate(storage.as_ptr().cast(), bytes.len(), transform, additive) };
        if count == 0 {
            return Err(AlsAclError::InvalidBuffer);
        }
        Ok(Self {
            storage,
            byte_len: bytes.len(),
            count,
        })
    }

    fn bytes(&self) -> *const u8 {
        self.storage.as_ptr().cast()
    }
}

/// Native ACL transform and float-curve tracks for a local pose clip.
/// Retarget corrections and default transforms are supplied by the clip exporter.
pub struct AlsAclAnimation {
    bones: CompressedTracks,
    tracks: Vec<AlsAclBoneTrack>,
    references: Vec<AlsAclTransform>,
    curves: Option<(CompressedTracks, Vec<SourceCurveName>)>,
    step_interpolation: bool,
    skeleton: Option<AssetPath<'static>>,
    additive_reference: Option<AssetPath<'static>>,
}

impl AlsAclAnimation {
    /// Creates a decoder for an exported ACL payload. All bindings are in codec track order.
    ///
    /// # Safety
    ///
    /// Both payloads must come from a compatible ACL compressor. The ACL header and hash check
    /// does not validate all offsets used by the native decoder.
    pub unsafe fn new(
        bone_bytes: &[u8],
        tracks: Vec<AlsAclBoneTrack>,
        curves: Option<(&[u8], Vec<SourceCurveName>)>,
        step_interpolation: bool,
    ) -> Result<Self, AlsAclError> {
        // The caller supplies compatible compressor output for both tracks.
        unsafe { Self::new_with_reference(bone_bytes, tracks, curves, step_interpolation, None) }
    }

    unsafe fn new_with_reference(
        bone_bytes: &[u8],
        tracks: Vec<AlsAclBoneTrack>,
        curves: Option<(&[u8], Vec<SourceCurveName>)>,
        step_interpolation: bool,
        additive_reference: Option<AssetPath<'static>>,
    ) -> Result<Self, AlsAclError> {
        let bones = CompressedTracks::new(bone_bytes, true, additive_reference.is_some())?;
        if bones.count as usize != tracks.len() {
            return Err(AlsAclError::TrackCountMismatch);
        }
        let unique: HashSet<_> = tracks.iter().map(|track| track.bone).collect();
        if unique.len() != tracks.len() {
            return Err(AlsAclError::DuplicateBone);
        }
        let curves = curves
            .map(|(bytes, names)| {
                let data = CompressedTracks::new(bytes, false, false)?;
                if data.count as usize != names.len() {
                    return Err(AlsAclError::TrackCountMismatch);
                }
                Ok((data, names))
            })
            .transpose()?;
        let references = tracks.iter().map(|track| track.reference).collect();
        Ok(Self {
            bones,
            tracks,
            references,
            curves,
            step_interpolation,
            skeleton: None,
            additive_reference,
        })
    }

    pub(crate) fn is_local_additive(&self) -> bool {
        self.additive_reference.is_some()
    }

    pub(crate) fn matches_additive_reference(
        &self,
        reference: &Handle<GraphClip>,
        time: f32,
    ) -> bool {
        time.max(0.0) == 0.0
            && self
                .additive_reference
                .as_ref()
                .is_some_and(|path| reference.path() == Some(path))
    }

    pub(crate) fn pose(
        &self,
        skeleton: &Skeleton,
        clip: &GraphClip,
        time: f32,
        omitted: Option<&HashSet<BoneId>>,
    ) -> Result<Pose, AlsAclError> {
        if !self.tracks.iter().all(|track| skeleton.has_id(&track.bone)) {
            return Err(AlsAclError::TrackCountMismatch);
        }
        let included = |bone: &BoneId| omitted.is_none_or(|bones| !bones.contains(bone));
        let required = omitted.map(|_| {
            self.tracks
                .iter()
                .map(|track| u8::from(included(&track.bone)))
                .collect::<Vec<_>>()
        });
        let mut raw = vec![AlsAclTransform::default(); self.tracks.len()];
        // These slices are exactly the track count returned by the validated buffer.
        let decoded = unsafe {
            als_acl_decode_pose(
                self.bones.bytes(),
                self.bones.byte_len,
                time,
                self.step_interpolation,
                self.references.as_ptr(),
                required
                    .as_ref()
                    .map_or(std::ptr::null(), |mask| mask.as_ptr()),
                raw.as_mut_ptr(),
                self.bones.count,
            )
        };
        if !decoded {
            return Err(AlsAclError::DecodeFailed);
        }
        let mut pose = Pose {
            skeleton: clip.skeleton.clone(),
            timestamp: time,
            ..default()
        };
        pose.bones.reserve(self.tracks.len());
        pose.paths.reserve(self.tracks.len());
        let additive = self.is_local_additive();
        for (track, value) in self.tracks.iter().zip(raw) {
            if included(&track.bone) {
                let retarget = if additive {
                    AlsAclRetarget::default()
                } else {
                    track.retarget
                };
                pose.add_bone(retarget.to_bone(value), track.bone);
            }
        }
        if additive {
            let neutral = BonePose {
                translation: Some(Vec3::ZERO),
                rotation: Some(Quat::IDENTITY),
                scale: Some(Vec3::ZERO),
                weights: None,
            };
            for bone in skeleton.iter_bones() {
                if included(&bone)
                    && !pose.paths.contains_key(&bone)
                    && (clip.curves().is_empty()
                        || clip.curves().contains_key(&bone.animation_target_id()))
                {
                    pose.add_bone(neutral.clone(), bone);
                }
            }
        } else if clip.curves().is_empty() {
            for bone in skeleton.iter_bones() {
                if included(&bone)
                    && !pose.paths.contains_key(&bone)
                    && let Some(reference) = skeleton.default_transforms(bone)
                {
                    pose.add_bone(BonePose::from_transform(reference.local), bone);
                }
            }
        } else {
            for (target, curves) in clip.curves() {
                let bone = BoneId::from(*target);
                if included(&bone) && !pose.paths.contains_key(&bone) {
                    pose.add_bone(
                        super::source_sequence_player::sample_bone_pose(curves, time),
                        bone,
                    );
                }
            }
        }
        Ok(pose)
    }

    pub(crate) fn curve_values(&self, time: f32) -> Result<Option<SourceCurveValues>, AlsAclError> {
        let Some((data, names)) = &self.curves else {
            return Ok(None);
        };
        let required = super::source_gameplay_tag_input::active_curve_filter()
            .map(|filter| {
                names
                    .iter()
                    .map(|name| u8::from(filter.allows(name)))
                    .collect::<Vec<_>>()
            })
            .filter(|mask| mask.contains(&0));
        if required.as_ref().is_some_and(|mask| !mask.contains(&1)) {
            return Ok(Some(SourceCurveValues::default()));
        }
        let mut values = vec![0.0_f32; names.len()];
        // The output slice matches the validated float-track count.
        if !unsafe {
            als_acl_decode_curves(
                data.bytes(),
                data.byte_len,
                time,
                required
                    .as_ref()
                    .map_or(std::ptr::null(), |mask| mask.as_ptr()),
                values.as_mut_ptr(),
                data.count,
            )
        } {
            return Err(AlsAclError::DecodeFailed);
        }
        Ok(Some(
            names
                .iter()
                .cloned()
                .zip(values)
                .enumerate()
                .filter_map(|(index, value)| {
                    required
                        .as_ref()
                        .is_none_or(|mask| mask[index] != 0)
                        .then_some(value)
                })
                .collect(),
        ))
    }
}

/// ACL assets bound by the exact glTF source and animation name.
#[derive(Resource, Clone)]
pub struct AlsAclAnimations(
    Arc<HashMap<AssetPath<'static>, HashMap<String, Arc<AlsAclAnimation>>>>,
);

impl Default for AlsAclAnimations {
    fn default() -> Self {
        static BUNDLED: LazyLock<AlsAclAnimations> = LazyLock::new(|| {
            let bytes = include_bytes!("../../assets/ALSRuntime/Animations/als_acl.bank");
            AlsAclAnimations::from_bundled(bytes)
        });
        BUNDLED.clone()
    }
}

impl AlsAclAnimations {
    /// Uses ordinary animation assets until compressed clips are explicitly registered.
    pub fn empty() -> Self {
        Self(Arc::default())
    }

    pub fn register(
        &mut self,
        path: AssetPath<'static>,
        animation: impl Into<String>,
        tracks: AlsAclAnimation,
    ) {
        Arc::make_mut(&mut self.0)
            .entry(path)
            .or_default()
            .insert(animation.into(), Arc::new(tracks));
    }

    /// Samples the compressed source for a loaded clip. Pass the reference clip and
    /// sample time to request a matching baked local additive pose.
    pub fn sample_pose(
        &self,
        clip: &GraphClip,
        skeleton: &Skeleton,
        time: f32,
        additive: Option<(&Handle<GraphClip>, f32)>,
    ) -> Option<Pose> {
        let animation = self.get(clip)?;
        let matches = match additive {
            Some((reference, time)) => animation.matches_additive_reference(reference, time),
            None => !animation.is_local_additive(),
        };
        if !matches {
            return None;
        }
        let mut pose = animation.pose(skeleton, clip, time, None).ok()?;
        if additive.is_none() {
            super::source_sequence_player::rebuild_virtual_bones(&mut pose, skeleton);
        }
        Some(pose)
    }

    pub(crate) fn get(&self, clip: &GraphClip) -> Option<Arc<AlsAclAnimation>> {
        let GraphClipSource::GltfNamed {
            path,
            animation_name,
        } = clip.source.as_ref()?;
        let compressed = self.0.get(path)?.get(animation_name)?;
        compressed
            .skeleton
            .as_ref()
            .is_none_or(|skeleton| clip.skeleton.path() == Some(skeleton))
            .then(|| Arc::clone(compressed))
    }

    pub(crate) fn cache_identity(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn from_bundled(bank: &[u8]) -> Self {
        let (size, bank) = bank.split_at(4);
        let size = u32::from_le_bytes(size.try_into().unwrap()) as usize;
        let (metadata, payload) = bank.split_at(size);
        let bank: BundledBank = serde_json::from_slice(metadata).expect("bundled ACL bindings");
        let skeleton = crate::als_asset_path("ALS/Character/SKM_Als.skn.ron");
        let bones: Vec<_> = bank
            .bones
            .into_iter()
            .map(|entry| {
                let bone = EntityPath::from(entry.path).id();
                let retarget = entry
                    .relative_target
                    .map_or_else(AlsAclRetarget::default, |target| {
                        AlsAclRetarget::animation_relative(entry.reference, target)
                    });
                AlsAclBoneTrack {
                    bone,
                    reference: entry.reference,
                    retarget,
                }
            })
            .collect();
        let mut result = Self::empty();
        for sequence in bank.sequences {
            let [start, end] = sequence.bone_range;
            let bindings = sequence
                .bone_tracks
                .into_iter()
                .map(|index| bones[index])
                .collect();
            let curves = sequence
                .curve_range
                .map(|[start, end]| (&payload[start..end], sequence.curve_tracks));
            let additive_reference = sequence
                .additive_reference
                .as_deref()
                .map(crate::als_asset_path);
            // The bundled bank embeds output from the original UE ACL compressor.
            let mut clip = unsafe {
                AlsAclAnimation::new_with_reference(
                    &payload[start..end],
                    bindings,
                    curves,
                    sequence.step,
                    additive_reference,
                )
            }
            .expect("bundled ACL tracks");
            clip.skeleton = Some(skeleton.clone());
            result.register(
                crate::als_asset_path(&sequence.path),
                sequence.animation,
                clip,
            );
        }
        result
    }
}

#[derive(Deserialize)]
struct BundledBank {
    bones: Vec<BundledBone>,
    sequences: Vec<BundledSequence>,
}

#[derive(Deserialize)]
struct BundledBone {
    path: Vec<String>,
    reference: AlsAclTransform,
    relative_target: Option<AlsAclTransform>,
}

#[derive(Deserialize)]
struct BundledSequence {
    path: String,
    animation: String,
    bone_range: [usize; 2],
    bone_tracks: Vec<usize>,
    curve_range: Option<[usize; 2]>,
    curve_tracks: Vec<SourceCurveName>,
    step: bool,
    additive_reference: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::{
        AlsPoseCurves, SourceAnimationInputs, source_gameplay_tag_input::sample_graph_clip_curves,
        source_pose::SourceSkeletonLayouts, source_sequence_player::sample_source_clip_pose,
    };

    const BONES: &[u8] = include_bytes!("../tests/fixtures/graph/acl_walk_bone_tracks.acl");
    const CURVES: &[u8] = include_bytes!("../tests/fixtures/graph/acl_walk_curve_tracks.acl");

    #[derive(Deserialize)]
    struct Fixture {
        skeleton: Vec<Joint>,
        bone_tracks: Vec<usize>,
        curve_tracks: Vec<SourceCurveName>,
        samples: Vec<Sample>,
    }

    #[derive(Deserialize)]
    struct Joint {
        name: String,
        parent: i32,
        reference: AlsAclTransform,
    }

    #[derive(Deserialize)]
    struct Sample {
        time: f32,
        bone_index: usize,
        expected: AlsAclTransform,
        curves: SourceCurveValues,
    }

    fn source_transform(value: AlsAclTransform) -> Transform {
        let [x, y, z, w] = value.rotation;
        let [tx, ty, tz] = value.translation;
        let [sx, sy, sz] = value.scale;
        Transform {
            translation: Vec3::new(tx, tz, ty) * 0.01,
            rotation: Quat::from_xyzw(-x, -z, -y, w),
            scale: Vec3::new(sx, sz, sy),
        }
    }

    #[test]
    fn acl_registered_clip_decodes_original_ue_pose_and_curves() {
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../tests/fixtures/graph/acl_walk_sample_bindings.json"
        ))
        .unwrap();
        let mut skeleton = Skeleton::default();
        let mut paths: Vec<EntityPath> = Vec::new();
        let mut globals: Vec<Transform> = Vec::new();
        for joint in &fixture.skeleton {
            let local = source_transform(joint.reference);
            let (path, global) = if let Ok(parent) = usize::try_from(joint.parent) {
                (
                    paths[parent].child(joint.name.clone()),
                    globals[parent] * local,
                )
            } else {
                let path = EntityPath::default().child(joint.name.clone());
                skeleton.set_root(path.id());
                (path, local)
            };
            skeleton.add_bone(path.clone(), local, global);
            paths.push(path);
            globals.push(global);
        }
        let bindings = fixture
            .bone_tracks
            .iter()
            .map(|&index| AlsAclBoneTrack {
                bone: paths[index].id(),
                reference: fixture.skeleton[index].reference,
                retarget: AlsAclRetarget::default(),
            })
            .collect();
        // The fixture payloads were copied from streams compressed by Unreal's ACL plugin.
        let decoder = unsafe {
            AlsAclAnimation::new(BONES, bindings, Some((CURVES, fixture.curve_tracks)), false)
        }
        .unwrap();
        let mut skeletons = Assets::default();
        let skeleton = skeletons.add(skeleton);
        let path = AssetPath::parse("acl_fixture.glb");
        let name = "walk".to_string();
        let clip = GraphClip {
            source: Some(GraphClipSource::GltfNamed {
                path: path.clone(),
                animation_name: name.clone(),
            }),
            skeleton: skeleton.clone(),
            ..default()
        };
        let excluded = paths[fixture.samples[0].bone_index].id();
        let omitted = [excluded].into_iter().collect();
        let pose_with_lod = decoder
            .pose(
                skeletons.get(&skeleton).unwrap(),
                &clip,
                0.0,
                Some(&omitted),
            )
            .unwrap();
        assert!(pose_with_lod.get_bone(excluded).is_none());
        assert_eq!(pose_with_lod.bones.len(), paths.len() - 1);

        let mut animations = AlsAclAnimations::empty();
        animations.register(path, name, decoder);
        let layouts = Arc::new(SourceSkeletonLayouts::new(&skeletons));
        let input = SourceAnimationInputs::default();
        input.evaluate_with_skeleton_layouts(
            Entity::PLACEHOLDER,
            &layouts,
            None,
            None,
            Some(&animations),
            || {
                for sample in fixture.samples {
                    let pose = sample_source_clip_pose(&clip, sample.time, &skeletons);
                    let bone = pose.get_bone(paths[sample.bone_index].id()).unwrap();
                    let expected = source_transform(sample.expected);
                    assert!(
                        bone.translation
                            .unwrap()
                            .abs_diff_eq(expected.translation, 1.0e-6)
                    );
                    assert!(bone.rotation.unwrap().dot(expected.rotation).abs() >= 1.0 - 1.0e-6);
                    assert!(bone.scale.unwrap().abs_diff_eq(expected.scale, 1.0e-6));

                    let curves = sample_graph_clip_curves(
                        Entity::PLACEHOLDER,
                        Some(&clip),
                        &AlsPoseCurves::default(),
                        sample.time,
                    );
                    assert_eq!(curves.len(), sample.curves.len());
                    for (curve, expected) in sample.curves {
                        assert!((curves[&curve] - expected).abs() <= 1.0e-6);
                    }
                }
            },
        );
    }

    #[test]
    fn acl_bundled_clips_bind_to_the_installed_als_character() {
        let (app, _, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let world = app.world();
        let animations = world.resource::<AlsAclAnimations>();
        assert_eq!(animations.0.values().map(HashMap::len).sum::<usize>(), 22);
        let clips = world.resource::<Assets<GraphClip>>();
        let clip = clips
            .iter()
            .find_map(|(_, clip)| match clip.source.as_ref() {
                Some(GraphClipSource::GltfNamed { animation_name, .. })
                    if animation_name == "A_Als_Walk_Forward_0" =>
                {
                    Some(clip)
                }
                _ => None,
            })
            .expect("installed walk");
        let skeletons = world.resource::<Assets<Skeleton>>();
        let skeleton = skeletons.get(&clip.skeleton).unwrap();
        let decoder = animations.get(clip).expect("compressed walk binding");
        let pose = decoder.pose(skeleton, clip, 0.0, None).unwrap();
        assert_eq!(pose.bones.len(), 79);

        let player = world
            .get::<bevy_animation_graph::core::animated_scene::AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let inputs = world.get::<SourceAnimationInputs>(player).unwrap();
        let layouts = Arc::new(SourceSkeletonLayouts::new(skeletons));
        inputs.evaluate_with_skeleton_layouts(
            player,
            &layouts,
            None,
            None,
            Some(animations),
            || {
                let curves =
                    sample_graph_clip_curves(player, Some(clip), &AlsPoseCurves::default(), 0.0);
                let planted = SourceCurveName::from("FootPlanted");
                assert!((curves[&planted] + 0.2492668628692627).abs() <= 1.0e-6);
                let mut pose = pose;
                super::super::source_sequence_player::rebuild_asset_virtual_bones(
                    &mut pose,
                    skeleton,
                    clip.skeleton.id(),
                );
                assert_eq!(sample_source_clip_pose(clip, 0.0, skeletons), pose);
            },
        );
    }

    #[test]
    fn acl_idle_additive_matches_original_mesh_and_requires_the_authored_base() {
        use super::super::source_sequence_player::sample_baked_additive_clip;

        #[derive(Deserialize)]
        struct AdditiveSample {
            time: f32,
            bones: HashMap<String, AlsAclTransform>,
        }

        let samples: Vec<AdditiveSample> = serde_json::from_str(include_str!(
            "../tests/fixtures/graph/acl_idle_local_additive.json"
        ))
        .unwrap();
        let (app, _, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let world = app.world();
        let animations = world.resource::<AlsAclAnimations>();
        let clips = world.resource::<Assets<GraphClip>>();
        let idle = clips
            .iter()
            .find_map(|(_, clip)| match clip.source.as_ref() {
                Some(GraphClipSource::GltfNamed { animation_name, .. })
                    if animation_name == "A_Als_Idle_0" =>
                {
                    Some(clip)
                }
                _ => None,
            })
            .expect("installed idle");
        let reference = world
            .resource::<AssetServer>()
            .get_handle::<GraphClip>(crate::als_asset_path(
                "ALS/Animations/Base/A_Als_Stand_Pose.anim.ron",
            ))
            .expect("installed additive base");
        let skeletons = world.resource::<Assets<Skeleton>>();
        let skeleton = skeletons.get(&idle.skeleton).unwrap();
        let player = world
            .get::<bevy_animation_graph::core::animated_scene::AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let inputs = world.get::<SourceAnimationInputs>(player).unwrap();
        let layouts = Arc::new(SourceSkeletonLayouts::new(skeletons));
        inputs.evaluate_with_skeleton_layouts(
            player,
            &layouts,
            None,
            None,
            Some(animations),
            || {
                assert!(
                    sample_baked_additive_clip(idle, 0.0, skeletons, &reference, 0.1, false)
                        .is_none()
                );
                assert!(
                    sample_baked_additive_clip(idle, 0.0, skeletons, &reference, 0.0, true)
                        .is_none()
                );
                assert!(
                    sample_baked_additive_clip(
                        idle,
                        0.0,
                        skeletons,
                        &Handle::default(),
                        0.0,
                        false
                    )
                    .is_none()
                );
                for sample in samples {
                    let (pose, precision) = sample_baked_additive_clip(
                        idle,
                        sample.time,
                        skeletons,
                        &reference,
                        0.0,
                        false,
                    )
                    .expect("configured local additive sample");
                    assert_eq!(pose.bones.len(), 79);
                    assert!(precision.is_some());
                    for (name, transform) in sample.bones {
                        let bone = skeleton
                            .iter_paths()
                            .find(|path| {
                                path.parts.last().is_some_and(|bone| bone.as_str() == name)
                            })
                            .unwrap()
                            .id();
                        let actual = pose.get_bone(bone).unwrap();
                        let expected = source_transform(transform);
                        assert!(
                            actual
                                .translation
                                .unwrap()
                                .abs_diff_eq(expected.translation, 1.0e-6)
                        );
                        assert!(
                            actual.rotation.unwrap().dot(expected.rotation).abs() >= 1.0 - 1.0e-6
                        );
                        assert!(actual.scale.unwrap().abs_diff_eq(expected.scale, 1.0e-6));
                    }
                }
            },
        );
    }
}
