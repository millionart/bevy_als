use std::{any::TypeId, path::Path, sync::{Arc, LazyLock}};

use bevy::{
    animation::animation_curves::WeightsCurveSample,
    asset::{Assets, Handle},
    platform::{collections::HashMap, hash::Hashed},
    prelude::{EvaluatorId, Transform, VariableCurve},
    reflect::prelude::*,
};
use bevy_animation_graph::core::{
    animation_clip::{GraphClip, loader::GraphClipSource},
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
    id::BoneId,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};
use serde::Deserialize;

use crate::extensions::control_rig_hierarchy::RigTransform;
use crate::extensions::global_f32_input::root_defaults_enabled;
use crate::extensions::raw_sequence_sampling::{RawRotationInterval, raw_rotation_interval};
use crate::extensions::source_compression::{
    PreparedPoseKind, write_raw_control_pose, sample_prepared_pose, sample_prepared_root, sample_prepared_source_pose,
    sample_source_curves,
};
use crate::extensions::source_pose::SourcePose;
use crate::extensions::sync_group_phase::{
    SourceSyncGroupRole, SourceSyncPlayerTick, SyncMarker, marker_phase_from_sample_time,
    resolved_source_sync_phase, source_sync_delta_seconds,
    resolved_source_sync_markers, marker_phase_sample_time_near,
    marker_phase_sample_time_continuous, resolved_source_marker_advance, resolved_source_sync_sample_time,
};
use crate::extensions::{
    AnimationContextState, SourceRelevantAsset, context_time_update, make_local_difference,
    make_mesh_space_difference,
    source_pose_nodes::{
        SourceAlphaMapping, SourceInputScaleBiasClampState, apply_source_input_scale_bias_clamp,
    },
};

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct SourceSequencePlayerState {
    marker_record: Option<crate::extensions::sync_group_phase::SourceMarkerRecord>,
    initialized: bool,
    position: f32,
    previous_position: f32,
    has_previous_position: bool,
    delta_seconds: f32,
    pub(crate) context: AnimationContextState,
    play_rate_state: SourceInputScaleBiasClampState,
}

pub(crate) enum CurveSample {
    Translation(bevy::math::Vec3),
    Rotation(bevy::math::Quat),
    Scale(bevy::math::Vec3),
    BoneWeights(Vec<f32>),
}

pub(crate) fn sample_curve(
    curve: &VariableCurve,
    time: f32,
    rotation_interval: Option<RawRotationInterval>,
) -> Option<CurveSample> {
    let evaluator_id = curve.0.evaluator_id();
    let translation_id = Hashed::new((TypeId::of::<Transform>(), 0));
    let rotation_id = Hashed::new((TypeId::of::<Transform>(), 1));
    let scale_id = Hashed::new((TypeId::of::<Transform>(), 2));
    match evaluator_id {
        EvaluatorId::ComponentField(id) => {
            if id == &translation_id {
                Some(CurveSample::Translation(
                    *curve.0.sample_clamped(time).downcast_ref()?,
                ))
            } else if id == &rotation_id {
                if let Some(interval) = rotation_interval {
                    return interval.sample(curve).map(CurveSample::Rotation);
                }
                Some(CurveSample::Rotation(
                    *curve.0.sample_clamped(time).downcast_ref()?,
                ))
            } else if id == &scale_id {
                Some(CurveSample::Scale(
                    *curve.0.sample_clamped(time).downcast_ref()?,
                ))
            } else {
                None
            }
        }
        EvaluatorId::Type(_) => Some(CurveSample::BoneWeights(
            (*curve
                .0
                .sample_clamped(time)
                .downcast::<WeightsCurveSample>()
                .ok()?)
            .0,
        )),
    }
}

pub fn sample_pose(clip: &GraphClip, time: f32) -> Pose {
    let mut pose = Pose {
        timestamp: time,
        skeleton: clip.skeleton.clone(),
        ..Pose::default()
    };
    let rotation_interval = raw_rotation_interval(clip, time);
    for (bone_id, curves) in &clip.curves {
        let mut bone_pose = BonePose::default();
        for curve in curves {
            match sample_curve(curve, time, rotation_interval) {
                Some(CurveSample::Translation(value)) => bone_pose.translation = Some(value),
                Some(CurveSample::Rotation(value)) => bone_pose.rotation = Some(value),
                Some(CurveSample::Scale(value)) => bone_pose.scale = Some(value),
                Some(CurveSample::BoneWeights(value)) => bone_pose.weights = Some(value),
                None => {}
            }
        }
        pose.add_bone(bone_pose, BoneId::from(*bone_id));
    }
    pose
}

#[derive(Deserialize)]
struct VirtualBoneBinding {
    source_bone: String,
    target_bone: String,
    virtual_bone: String,
}

#[derive(Deserialize)]
struct RawVirtualBones {
    raw_gltf_paths: Vec<String>,
    virtual_bones: Vec<VirtualBoneBinding>,
}

static RAW_VIRTUAL_BONES: LazyLock<RawVirtualBones> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../assets/ALS/Data/AnimationInstance/als_raw_virtual_bones.json"
    ))
    .expect("generated raw virtual bone bindings")
});

fn source_component_transform(
    pose: &Pose,
    skeleton: &Skeleton,
    bone: BoneId,
    components: &mut HashMap<BoneId, RigTransform>,
) -> Option<RigTransform> {
    if let Some(component) = components.get(&bone) {
        return Some(*component);
    }
    let reference = skeleton.default_transforms(bone)?.local;
    let local = pose
        .get_bone(bone)
        .map_or(reference, |pose| pose.to_transform_with_base(reference));
    let local = RigTransform {
        translation: local.translation.as_dvec3() * 100.0,
        rotation: local.rotation.as_dquat(),
        scale: local.scale.as_dvec3(),
    };
    let component = match skeleton.parent(&bone) {
        Some(parent) => local.global(source_component_transform(
            pose, skeleton, parent, components,
        )?),
        None => local,
    };
    components.insert(bone, component);
    Some(component)
}

fn sample_raw_source_clip_pose(
    clip: &GraphClip,
    time: f32,
    skeletons: &Assets<Skeleton>,
) -> Option<Pose> {
    let mut pose = sample_pose(clip, time);
    let Some(GraphClipSource::GltfNamed { path, .. }) = &clip.source else {
        return Some(pose);
    };
    write_raw_control_pose(clip, time, &mut pose);
    if !RAW_VIRTUAL_BONES
        .raw_gltf_paths
        .iter()
        .any(|source| Path::new(source) == path.path())
    {
        return Some(pose);
    }
    let skeleton = skeletons.get(&clip.skeleton)?;
    let bones: HashMap<_, _> = skeleton
        .iter_bones()
        .filter_map(|bone| {
            let name = skeleton.id_to_path(bone)?.parts.into_iter().last()?;
            Some((name.as_str().to_owned(), bone))
        })
        .collect();
    let mut components = HashMap::new();
    for binding in &RAW_VIRTUAL_BONES.virtual_bones {
        let source = *bones.get(&binding.source_bone)?;
        let target = *bones.get(&binding.target_bone)?;
        let virtual_bone = *bones.get(&binding.virtual_bone)?;
        if skeleton.parent(&virtual_bone) != Some(source) {
            return None;
        }
        let source = source_component_transform(&pose, skeleton, source, &mut components)?;
        let target = source_component_transform(&pose, skeleton, target, &mut components)?;
        let local = target.relative(source);
        components.insert(virtual_bone, local.global(source));
        let transform = Transform {
            translation: (local.translation * 0.01).as_vec3(),
            rotation: local.rotation.as_quat(),
            scale: local.scale.as_vec3(),
        };
        if let Some(index) = pose.paths.get(&virtual_bone).copied() {
            let bone = &mut pose.bones[index];
            bone.translation = Some(transform.translation);
            bone.rotation = Some(transform.rotation);
            bone.scale = Some(transform.scale);
        } else {
            pose.add_bone(BonePose::from_transform(transform), virtual_bone);
        }
    }
    Some(pose)
}

pub(crate) fn sample_source_clip_pose(
    clip: &GraphClip,
    time: f32,
    skeletons: &Assets<Skeleton>,
) -> Option<Pose> {
    sample_prepared_pose(clip, time)
        .map(|(pose, _)| pose)
        .or_else(|| sample_raw_source_clip_pose(clip, time, skeletons))
}

pub(crate) fn sample_source_pose(
    ctx: &NodeContext,
    clip: &GraphClip,
    time: f32,
    additive_reference: Option<&Handle<GraphClip>>,
    reference_position: f32,
    mesh_space: bool,
    root_lock: Option<SourceRootLock>,
) -> Result<(Pose, Option<Arc<SourcePose>>), GraphError> {
    let skeletons = &ctx.graph_context.resources.skeleton_assets;
    let (target, baked_additive, mut source_pose) = match sample_prepared_source_pose(clip, time) {
        Some((source, kind)) => {
            let mut pose = Pose { skeleton: clip.skeleton.clone(), ..Default::default() };
            source.project_into(&mut pose);
            (pose, kind != PreparedPoseKind::Absolute, Some(source))
        }
        None => (sample_raw_source_clip_pose(clip, time, skeletons)
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?, false, None),
    };
    let mut pose = if let Some(reference_handle) = additive_reference.filter(|_| !baked_additive) {
        source_pose = None;
        let reference_clip = ctx.graph_context.resources.graph_clip_assets.get(reference_handle)
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        let reference = sample_raw_source_clip_pose(reference_clip, reference_position.max(0.0), skeletons)
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        let skeleton = ctx.graph_context.resources.skeleton_assets.get(&reference.skeleton)
            .or_else(|| ctx.graph_context.resources.skeleton_assets.get(&target.skeleton))
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        if mesh_space { make_mesh_space_difference(&reference, &target, skeleton) }
        else { make_local_difference(&reference, &target, skeleton) }
    } else { target };
    pose.timestamp = time;
    if let Some(mode) = root_lock {
        let skeleton = ctx.graph_context.resources.skeleton_assets.get(&pose.skeleton)
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        apply_source_root_lock(&mut pose, clip, skeleton, mode, baked_additive || additive_reference.is_some())
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        source_pose = source_pose.filter(|source| source.matches_projection(&pose));
    }
    Ok((pose, source_pose.map(Arc::new)))
}

pub(crate) fn bound_source_sample_time(position: f32, duration: f32, looped: bool) -> f32 {
    let duration = duration.max(0.0);
    if looped && (position < 0.0 || position > duration) {
        if duration <= 1e-8 { 0.0 } else { position.rem_euclid(duration) }
    } else {
        position.clamp(0.0, duration)
    }
}

fn next_position(
    state: &mut SourceSequencePlayerState,
    update: TimeUpdate,
    duration: f32,
    start_position: f32,
    play_rate: f32,
    loop_animation: bool,
) -> f32 {
    let duration = duration.max(1e-4);
    let previous_position = if state.initialized { state.position } else { start_position }.clamp(0.0, duration);
    let has_previous_position = state.initialized && matches!(&update, TimeUpdate::Delta(_));
    if !state.initialized {
        state.initialized = true;
    }
    state.position = previous_position;
    match &update {
        TimeUpdate::Delta(value) => state.position += (*value).max(0.0) * play_rate,
        TimeUpdate::Absolute(value) => state.position = start_position + *value * play_rate,
        TimeUpdate::PercentOfEvent { .. } => {}
    }
    state.position = bound_source_sample_time(state.position, duration, loop_animation);
    state.previous_position = previous_position;
    state.has_previous_position = has_previous_position;
    state.delta_seconds = match &update {
        TimeUpdate::Delta(value) => (*value).max(0.0),
        TimeUpdate::Absolute(_) | TimeUpdate::PercentOfEvent { .. } => 0.0,
    };
    state.position
}

fn global_f32_or_field(ctx: &NodeContext, key: &str, fallback: f32) -> Result<f32, GraphError> {
    if key.is_empty() {
        return Ok(fallback);
    }
    match crate::extensions::source_instance::source_input_data(ctx, key) {
        Some(value) => value.as_f32(),
        None if root_defaults_enabled(ctx) => Ok(fallback),
        None => Err(GraphError::GlobalInputDataMissing(key.to_owned())),
    }
}

fn global_bool_or_field(ctx: &NodeContext, key: &str, fallback: bool) -> Result<bool, GraphError> {
    if key.is_empty() {
        return Ok(fallback);
    }
    match crate::extensions::source_instance::source_input_data(ctx, key) {
        Some(value) => value.as_bool(),
        None if root_defaults_enabled(ctx) => Ok(fallback),
        None => Err(GraphError::GlobalInputDataMissing(key.to_owned())),
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default)]
pub struct SourceSequenceSynchronization {
    pub key: String,
    pub role: SourceSyncGroupRole,
    pub phase_duration: f32,
    pub phase_markers: Vec<SyncMarker>,
    pub markers: Vec<SyncMarker>,
    pub delta_seconds_key: String,
}

impl Default for SourceSequenceSynchronization {
    fn default() -> Self {
        Self {
            key: Default::default(),
            role: Default::default(),
            phase_duration: 1.0,
            phase_markers: Default::default(),
            markers: Default::default(),
            delta_seconds_key: Default::default(),
        }
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceSequencePlayerNode {
    pub clip: Handle<GraphClip>,
    pub curves: crate::extensions::als_curves::AlsPoseCurves,
    pub root_lock: Option<SourceRootLock>,
    pub additive_reference: Option<Handle<GraphClip>>,
    pub additive_reference_position: f32,
    pub additive_mesh_space: bool,
    pub play_rate: f32,
    pub asset_rate_scale: f32,
    pub play_rate_key: String,
    pub play_rate_pin: String,
    pub expose_play_rate_pin: bool,
    pub play_rate_basis: f32,
    pub play_rate_mapping: SourceAlphaMapping,
    pub start_position: f32,
    pub start_position_pin: String,
    pub expose_start_position_pin: bool,
    pub loop_animation: bool,
    pub ignore_for_relevancy_test: bool,
    pub loop_animation_key: String,
    pub loop_animation_pin: String,
    pub expose_loop_animation_pin: bool,
    pub sync_group: String,
    pub override_position_when_joining_sync_group_as_leader: bool,
    pub synchronization: Option<SourceSequenceSynchronization>,
}

impl Default for SourceSequencePlayerNode {
    fn default() -> Self {
        Self {
            clip: Default::default(),
            curves: Default::default(),
            root_lock: None,
            additive_reference: None,
            additive_reference_position: 0.0,
            additive_mesh_space: false,
            play_rate: 1.0,
            asset_rate_scale: 1.0,
            play_rate_key: Default::default(),
            play_rate_pin: "Play Rate".to_owned(),
            expose_play_rate_pin: true,
            play_rate_basis: 1.0,
            play_rate_mapping: Default::default(),
            start_position: 0.0,
            start_position_pin: "Start Position".to_owned(),
            expose_start_position_pin: true,
            loop_animation: true,
            ignore_for_relevancy_test: false,
            loop_animation_key: Default::default(),
            loop_animation_pin: "Loop Animation".to_owned(),
            expose_loop_animation_pin: false,
            sync_group: Default::default(),
            override_position_when_joining_sync_group_as_leader: false,
            synchronization: None,
        }
    }
}

#[derive(Reflect, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceRootLock {
    RefPose,
    AnimFirstFrame,
    Zero,
}

pub(crate) fn source_root_lock_transform(
    mode: SourceRootLock,
    reference: Transform,
    additive: bool,
    first_frame: impl FnOnce() -> Option<Transform>,
) -> Option<Transform> {
    let mut transform = match mode {
        SourceRootLock::RefPose => reference,
        SourceRootLock::AnimFirstFrame => first_frame()?,
        SourceRootLock::Zero => Transform::IDENTITY,
    };
    if additive && mode != SourceRootLock::AnimFirstFrame {
        transform.scale -= bevy::math::Vec3::ONE;
    }
    Some(transform)
}

pub fn apply_source_root_lock(
    pose: &mut Pose,
    clip: &GraphClip,
    skeleton: &Skeleton,
    mode: SourceRootLock,
    additive: bool,
) -> Option<()> {
    let root = crate::extensions::root_motion_defaults::derive_default_root_bone(clip, skeleton)?.id();
    let reference = skeleton.default_transforms(root)?.local;
    let transform = source_root_lock_transform(mode, reference, additive, || {
        if let Some(first) = sample_prepared_root(clip, root, 0.0) {
            Some(first)
        } else {
            let first = sample_pose(clip, 0.0);
            Some(first.get_bone(root)?.to_transform_with_base(reference))
        }
    })?;
    let index = *pose.paths.get(&root)?;
    pose.bones[index].translation = Some(transform.translation);
    pose.bones[index].rotation = Some(transform.rotation);
    pose.bones[index].scale = Some(transform.scale);
    Some(())
}

impl NodeLike for SourceSequencePlayerNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let Some(clip) = ctx
            .graph_context
            .resources
            .graph_clip_assets
            .get(&self.clip)
        else {
            ctx.set_data_fwd("Pose", DataValue::Pose(Pose::default()));
            return Ok(());
        };
        let update = context_time_update(&ctx)?;
        let play_rate = if self.expose_play_rate_pin {
            ctx.data_back(self.play_rate_pin.clone())
                .and_then(|value| value.as_f32())
                .or_else(|_| global_f32_or_field(&ctx, &self.play_rate_key, self.play_rate))?
        } else {
            global_f32_or_field(&ctx, &self.play_rate_key, self.play_rate)?
        };
        let loop_animation = if self.expose_loop_animation_pin {
            ctx.data_back(self.loop_animation_pin.clone())
                .and_then(|value| value.as_bool())
                .or_else(|_| {
                    global_bool_or_field(&ctx, &self.loop_animation_key, self.loop_animation)
                })?
        } else {
            global_bool_or_field(&ctx, &self.loop_animation_key, self.loop_animation)?
        };
        let start_position = if self.expose_start_position_pin {
            ctx.data_back(self.start_position_pin.clone())
                .and_then(|value| value.as_f32())
                .unwrap_or(self.start_position)
        } else {
            self.start_position
        };
        let delta = if let Some(sync) = &self.synchronization {
            source_sync_delta_seconds(&ctx, &sync.delta_seconds_key)?
        } else {
            match &update {
                TimeUpdate::Delta(value) => value.max(0.0),
                TimeUpdate::Absolute(_) | TimeUpdate::PercentOfEvent { .. } => 0.0,
            }
        };
        let resolved_phase = self
            .synchronization
            .as_ref()
            .map(|sync| resolved_source_sync_phase(&ctx, &sync.key))
            .transpose()?
            .flatten();
        let resolved_advance = self.synchronization.as_ref()
            .map(|sync| resolved_source_marker_advance(&ctx, &sync.key)).transpose()?.flatten();
        let length_ratios = self.synchronization.as_ref()
            .map(|sync| crate::extensions::sync_group_phase::resolved_source_sync_length_ratios(&ctx, &sync.key)).transpose()?.flatten();
        let mut leader_sample_time = resolved_source_sync_sample_time(&ctx)?;
        let resolved_previous_time = crate::extensions::sync_group_phase::resolved_source_sync_previous_time(&ctx)?;
        let identity = (ctx.graph_context.context_id, ctx.node_id);
        let reached_full_weight = self.synchronization.as_ref().is_some_and(|sync|
            matches!(sync.role, SourceSyncGroupRole::TransitionLeader | SourceSyncGroupRole::TransitionFollower))
            && ctx.graph_context.global_input_data.contains_key(
                &crate::extensions::sync_group_phase::source_sync_full_weight_key(identity));
        let requested_inertialization = ctx.graph_context.global_input_data.get(
            crate::extensions::sync_group_phase::SOURCE_SYNC_INERTIAL_SCOPE)
            .is_some_and(|value| value.as_bool().unwrap_or(false));
        let sync_markers = self.synchronization.as_ref().map(|sync| (
            resolved_source_sync_markers(&ctx, &sync.key, &sync.phase_markers),
            resolved_source_sync_markers(&ctx, &sync.key, &sync.markers),
        ));
        let resolved_record = if let Some((_, markers)) = sync_markers.as_ref() {
            crate::extensions::sync_group_phase::resolved_source_marker_record(&ctx, markers, clip.duration(), Some(self.clip.clone()))?
        } else { None };
        let reset_markers = crate::extensions::sync_group_phase::source_sync_markers_reset(&ctx);
        let notify_instance_id = crate::extensions::source_instance::source_notify_instance_id(&ctx)?;
        let state = ctx.state_mut_or_else(SourceSequencePlayerState::default)?;
        if reset_markers { state.marker_record = None; }
        let play_rate = apply_source_input_scale_bias_clamp(
            &mut state.play_rate_state,
            if self.play_rate_basis.abs() <= f32::EPSILON {
                0.0
            } else {
                play_rate / self.play_rate_basis
            },
            &self.play_rate_mapping,
            delta,
        ) * self.asset_rate_scale;
        let start_position = if start_position == 0.0 && play_rate < 0.0 {
            clip.duration()
        } else { start_position };
        let mut sync_players = Vec::new();
        let position = if let Some(sync) = &self.synchronization {
            let (phase_markers, markers) = sync_markers.as_ref().unwrap();
            let previous_sample_time = if state.initialized { state.position } else { start_position };
            let previous_phase = state
                .context
                .sync_players
                .first()
                .map(|tick| tick.phase)
                .unwrap_or_else(|| {
                    marker_phase_from_sample_time(
                        start_position,
                        clip.duration(),
                        markers,
                        sync.phase_duration,
                        phase_markers,
                        loop_animation,
                    )
                });
            let mut tick = SourceSyncPlayerTick {
                identity,
                key: sync.key.clone(),
                role: sync.role,
                source_asset: Some(self.clip.clone()),
                override_position_when_joining_sync_group_as_leader: self.override_position_when_joining_sync_group_as_leader,
                weight: crate::extensions::interruptible_fsm::current_machine_weight(),
                weight_is_absolute: true,
                has_been_full_weight: state.context.sync_players.first().is_some_and(|tick| tick.has_been_full_weight)
                    || reached_full_weight,
                requested_inertialization,
                previous_phase,
                previous_marker_record: state.marker_record.clone().filter(|record| record.asset.as_ref() == Some(&self.clip)),
                sample_time_hint: Some(previous_sample_time),
                phase_duration: sync.phase_duration,
                phase_markers: phase_markers.to_vec(),
                duration: clip.duration(),
                markers: markers.to_vec(),
                looped: loop_animation,
                delta_seconds: delta,
                play_rate,
                ..Default::default()
            };
            state.previous_position = resolved_previous_time.unwrap_or(previous_sample_time);
            state.has_previous_position = state.initialized || resolved_previous_time.is_some();
            state.initialized = true;
            state.delta_seconds = delta;
            let marker_advance = if let Some(phase) = resolved_phase {
                tick.phase = phase;
                resolved_advance
            } else {
                let step = tick.advance_from_time(previous_sample_time);
                tick.phase = step.phase;
                leader_sample_time = Some(step.time);
                state.marker_record = step.record;
                Some(step.advance)
            };
            tick.marker_advance = marker_advance.clone();
            state.position = if let Some(time) = leader_sample_time {
                if resolved_phase.is_some() { state.marker_record = resolved_record; }
                time
            } else if let Some(ratios) = length_ratios.filter(|_| phase_markers.is_empty() || markers.is_empty()) {
                state.marker_record = None;
                state.previous_position = ratios.x * clip.duration();
                state.has_previous_position = true;
                ratios.y * clip.duration()
            } else if let Some(advance) = &marker_advance {
                let (time, previous_time) = marker_phase_sample_time_continuous(tick.phase, advance,
                    sync.phase_duration, phase_markers, clip.duration(), markers,
                    previous_sample_time, loop_animation, &mut state.marker_record, Some(self.clip.clone()));
                state.previous_position = previous_time;
                state.has_previous_position = true;
                time
            } else {
                state.marker_record = None;
                marker_phase_sample_time_near(tick.phase, sync.phase_duration, phase_markers,
                    clip.duration(), markers, previous_sample_time, loop_animation)
            };
            sync_players.push(tick);
            state.position
        } else {
            next_position(
                state,
                update,
                clip.duration(),
                start_position,
                play_rate,
                loop_animation,
            )
        };
        let clock = SourceRelevantAsset {
            duration: clip.duration(),
            position,
            previous_position: state.previous_position,
            has_previous_position: state.has_previous_position,
            delta_seconds: state.delta_seconds,
            playback_delta_seconds: state.delta_seconds * play_rate,
            looping: loop_animation,
            weight: 1.0,
        };
        state.context = AnimationContextState {
            curves: sample_source_curves(Some(clip), &self.curves, position),
            relevant_asset: (!self.ignore_for_relevancy_test).then_some(clock.clone()),
            curves_are_authoritative: true,
            sync_players,
            ..Default::default()
        };
        if delta > 0.0 {
            let mut clock = clock;
            if !clock.has_previous_position { clock.previous_position = start_position; }
            clock.has_previous_position = true;
            state.context.asset_samples.push(crate::extensions::animation_context::SourceAssetSample {
                identity: format!("{:?}:{:?}", identity.0, identity.1),
                notify_instance_id,
                clip: self.clip.clone(), clock,
                sync_group: self.synchronization.as_ref().map_or_else(String::new, |sync| sync.key.clone()),
                sync_identity: self.synchronization.as_ref().map(|_| identity),
                montage: false,
                interrupted: false,
                slot_key: String::new(),
            });
        }
        let (pose, source_pose) = sample_source_pose(&ctx, clip, position, self.additive_reference.as_ref(), self.additive_reference_position, self.additive_mesh_space, self.root_lock)?;
        ctx.state_mut::<SourceSequencePlayerState>()?.context.source_pose = source_pose;
        ctx.set_time(position);
        ctx.set_data_fwd("Pose", pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        if self.expose_play_rate_pin {
            ctx.add_input_data(self.play_rate_pin.clone(), DataSpec::F32);
        }
        if self.expose_start_position_pin {
            ctx.add_input_data(self.start_position_pin.clone(), DataSpec::F32);
        }
        if self.expose_loop_animation_pin {
            ctx.add_input_data(self.loop_animation_pin.clone(), DataSpec::Bool);
        }
        ctx.add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Sequence Player".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    #[test]
    fn installed_clip_durations_use_the_native_timeline_endpoints() {
        use std::time::{Duration, Instant};

        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d199/timeline-compiler.json"
        )).unwrap();
        let (mut app, _, _) = crate::roll_drive::tests::loaded_roll_character();
        let server = app.world().resource::<AssetServer>();
        let clips = reference["corrected"].as_array().unwrap().iter().map(|case| {
            let path = case["clip"].as_str().unwrap();
            let duration = case["native_duration"].as_f64().unwrap() as f32;
            (server.load::<GraphClip>(path.to_owned()), path, duration)
        }).collect::<Vec<_>>();
        let mut last_progress = Instant::now();
        let mut previous_ready = 0;
        loop {
            let loaded = app.world().resource::<Assets<GraphClip>>();
            let ready = clips.iter().filter(|(handle, _, _)| loaded.contains(handle)).count();
            if ready == clips.len() {
                for (handle, path, duration) in &clips {
                    assert_eq!(loaded.get(handle).unwrap().duration(), *duration, "{path}");
                }
                break;
            }
            if ready > previous_ready {
                last_progress = Instant::now();
                previous_ready = ready;
            }
            assert!(last_progress.elapsed() < Duration::from_secs(10), "Timeline clips stalled at {ready}/{}", clips.len());
            app.update();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn rifle_follower_only_group_keeps_the_source_zero_play_rate() {
        use bevy_animation_graph::core::{animated_scene::AnimatedSceneInstance,
            animation_graph::AnimationGraph, animation_graph_player::AnimationGraphPlayer};
        use crate::als::{state::{Gait, Stance, LocomotionState, OverlayState}, input::AlsInputCommand};
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().entity_mut(avatar).insert(crate::animation_visibility::AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones);
        app.world_mut().entity_mut(character).insert((avian3d::prelude::RigidBody::Kinematic, bevy_tnua::TnuaToggle::Disabled));
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().overlay_state = Some(OverlayState::Rifle);
        #[derive(Resource)]
        struct Speed(f32);
        app.insert_resource(Speed(0.0));
        app.add_systems(PostUpdate, (move |speed: Res<Speed>, mut actors: Query<(&mut crate::actual_motion::AlsActualMotion,
            &mut Gait, &mut Stance, &mut LocomotionState, &mut AlsInputCommand)>| {
            let (mut motion, mut gait, mut stance, mut mode, mut input) = actors.get_mut(character).unwrap();
            *gait = Gait::Running; *stance = Stance::Standing; *mode = LocomotionState::Grounded;
            motion.horizontal_speed = speed.0; motion.world_velocity = Vec3::NEG_Z * speed.0;
            motion.local_velocity = Vec2::new(0.0, speed.0); motion.has_input = speed.0 > 0.0;
            input.move_dir = if speed.0 > 0.0 { Vec2::Y } else { Vec2::ZERO };
        }).after(crate::standing_layer::AlsStandingLayerSet).before(crate::ab_als_root::AlsRootGraphSet));
        let mut observed = false;
        for frame in 0..220 {
            app.world_mut().resource_mut::<Speed>().0 = if (90..186).contains(&frame) { 3.5 } else { 0.0 };
            app.update();
            if frame < 186 { continue; }
            let player = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
            let instance = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            assert!(instance.get_error().is_none(), "Actual Rifle graph failed: {:?}", instance.get_error());
            let arena = instance.get_context_arena().unwrap();
            let graphs = app.world().resource::<Assets<AnimationGraph>>();
            let Some(ticks) = crate::extensions::sync_group_phase::source_sync_players(arena, graphs, "sync_groups.movement.phase") else { continue; };
            let group = ticks.iter().filter(|tick| tick.key == "sync_groups.movement.phase").collect::<Vec<_>>();
            if group.iter().any(|tick| tick.role != SourceSyncGroupRole::AlwaysFollower) { continue; }
            let Some(tick) = group.into_iter().find(|tick| tick.selected_leader) else { continue; };
            let context = arena.get_context(tick.identity.0).unwrap();
            let states = context.node_states.get_all_upcoming_states::<SourceSequencePlayerState>(tick.identity.1).unwrap().collect::<Vec<_>>();
            assert_eq!(states.len(), 1, "Follower-only sampling state is ambiguous");
            let state = states[0];
            assert!(tick.delta_seconds > 0.0, "Follower-only fixture has no animation update delta");
            assert_eq!(tick.play_rate, 0.0, "Source Rifle follower-only rate changed");
            assert!((state.position - state.previous_position).abs() < 1e-4,
                "Source zero-rate follower-only sequence advanced: frame={frame}, position={}, previous={}",
                state.position, state.previous_position);
            observed = true;
            break;
        }
        assert!(observed, "Actual Rifle stop did not expose a follower-only group");
    }

    #[test]
    fn asset_rate_scale_follows_node_mapping_in_sequence_clocks() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_graph::{AnimationGraph, NodeId}, animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext, node_states::StateKey,
                system_resources::SystemResources},
            edge_data::DataSpec, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        fn query(In((handle, node)): In<(Handle<AnimationGraph>, NodeId)>, resources: SystemResources) -> (f32, f32, f32, f32) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let inputs = [("rate".to_owned(), 4.0_f32.into()), ("delta".to_owned(), 0.6_f32.into())].into();
            let env = TestIoEnv::default();
            let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources,
                &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
            let output = graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.6)), context).unwrap();
            let state = arena.get_toplevel().node_states.get::<SourceSequencePlayerState>(node, StateKey::Default).unwrap();
            (output["Pose"].as_pose().unwrap().timestamp,
                state.context.relevant_asset.as_ref().unwrap().playback_delta_seconds,
                state.context.relevant_asset.as_ref().unwrap().previous_position,
                state.context.asset_samples[0].clock.previous_position)
        }
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let clip = world.resource_mut::<Assets<GraphClip>>().add(GraphClip { duration: 2.0, ..default() });
        for rate_scale in [2.0_f32, -2.0, 0.0] {
            for start_position in [0.0_f32, 0.5] {
                for looped in [false, true] {
                    for synchronized in [false, true] {
                        let node = AnimationNode::new("rate-scaled sequence", SourceSequencePlayerNode {
                            clip: clip.clone(), asset_rate_scale: rate_scale, play_rate: 99.0, play_rate_key: "rate".to_owned(),
                            expose_play_rate_pin: false, play_rate_basis: 2.0,
                            play_rate_mapping: SourceAlphaMapping { source_semantics: true, clamp_result: true,
                                clamp_min: -1.5, clamp_max: 1.5, ..default() },
                            start_position, expose_start_position_pin: false, loop_animation: looped, expose_loop_animation_pin: false,
                            synchronization: synchronized.then(|| SourceSequenceSynchronization { key: "fixture".into(),
                                delta_seconds_key: "delta".into(), ..default() }), ..default()
                        });
                        let mut graph = AnimationGraph::new();
                        graph.add_output_data("Pose".into(), DataSpec::Pose);
                        graph.add_output_data_edge(node.id, "Pose", "Pose");
                        graph.add_node(node.clone());
                        let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
                        let (position, delta, previous, sample_previous) = world.run_system_cached_with(query, (graph, node.id)).unwrap();
                        let expected_delta = 0.6 * 1.5 * rate_scale;
                        let initial = if start_position == 0.0 && expected_delta < 0.0 { 2.0 } else { start_position };
                        let expected = if looped { (initial + expected_delta).rem_euclid(2.0) } else { (initial + expected_delta).clamp(0.0, 2.0) };
                        assert!((position - expected).abs() < 1e-5, "rate={rate_scale}, start={start_position}, looped={looped}, sync={synchronized}: {position} != {expected}");
                        assert!((delta - expected_delta).abs() < 1e-5);
                        assert!((previous - initial).abs() < 1e-5, "rate={rate_scale}, start={start_position}, looped={looped}, sync={synchronized}: previous {previous} != {initial}");
                        assert!((sample_previous - initial).abs() < 1e-5, "rate={rate_scale}, start={start_position}, looped={looped}, sync={synchronized}: sample previous {sample_previous} != {initial}");
                    }
                }
            }
        }
    }

    #[test]
    fn repeated_sync_pairs_stay_near_each_sequence_players_current_time() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_graph::{AnimationGraph, NodeId}, animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos,
                graph_context::{GraphState, QueryOutputTime},
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                node_states::StateKey, system_resources::SystemResources},
            edge_data::{DataSpec, DataValue}, state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };

        fn query(In((handle, players)): In<(Handle<AnimationGraph>, [NodeId; 2])>, resources: SystemResources) -> Vec<[f32; 2]> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut results = Vec::new();
            let frames: &[(f32, bool, &[&str])] = &[(0.2, true, &[]), (0.19, false, &[]),
                (0.95, false, &["A"]), (0.11, true, &["A"]), (0.94, false, &["A"]), (0.75, false, &["B"]),
                (0.15, true, &["B", "A"]), (0.75, true, &["B", "A", "B", "A", "B", "A", "B", "A", "B", "A"]),
                (0.8, true, &[]), (0.1, false, &["A", "B", "A", "B", "A", "B", "A", "B", "A", "B"]), (0.2, true, &[])];
            for (frame, &(phase, forward, passed)) in frames.iter().enumerate() {
                if frame == 10 {
                    *arena.get_toplevel_mut() = GraphState::new(handle.id());
                }
                let mut inputs = HashMap::from([
                    ("delta".to_owned(), DataValue::F32(0.1)),
                    (crate::extensions::sync_group_phase::SOURCE_SYNC_RESOLVED.to_owned(), DataValue::Bool(true)),
                    (format!("{}.Movement", crate::extensions::sync_group_phase::SOURCE_SYNC_RESOLVED), DataValue::F32(phase)),
                ]);
                if frame != 0 && frame != 10 {
                    inputs.extend(crate::extensions::sync_group_phase::SourceMarkerAdvance {
                        forward, markers: passed.iter().map(|name| (*name).to_owned()).collect(), ..default()
                    }.into_inputs("Movement"));
                }
                let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources,
                    &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.1)), context).unwrap();
                results.push(players.map(|node| arena.get_toplevel().node_states
                    .get::<SourceSequencePlayerState>(node, StateKey::Default).unwrap().position));
            }
            results
        }

        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let clip = world.resource_mut::<Assets<GraphClip>>().add(GraphClip { duration: 1.0, ..default() });
        let phase_markers = [("A", 0.1), ("B", 0.3), ("A", 0.6), ("B", 0.8)]
            .map(|(name, time)| SyncMarker { name: name.into(), time }).to_vec();
        let sample_markers = [("A", 0.05), ("B", 0.25), ("A", 0.55), ("B", 0.95)]
            .map(|(name, time)| SyncMarker { name: name.into(), time }).to_vec();
        let mut graph = AnimationGraph::new();
        let players = [0.65, 0.1].map(|start_position| {
            let node = AnimationNode::new("player", SourceSequencePlayerNode {
                clip: clip.clone(), start_position, loop_animation: true,
                synchronization: Some(SourceSequenceSynchronization {
                    key: "Movement".into(), phase_duration: 1.0,
                    phase_markers: phase_markers.clone(), markers: sample_markers.clone(),
                    delta_seconds_key: "delta".into(), ..default()
                }),
                ..default()
            });
            graph.add_node(node.clone());
            node
        });
        for (index, player) in players.iter().enumerate() {
            let output = format!("Pose{index}");
            graph.add_output_data(output.clone(), DataSpec::Pose);
            graph.add_output_data_edge(player.id, "Pose", output);
        }
        let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
        let actual = world.run_system_cached_with(query, (handle, [players[0].id, players[1].id])).unwrap();
        for (frame, expected) in [[0.75, 0.15], [0.73, 0.14], [0.4, 0.0],
            [0.57, 0.06], [0.39, 0.9966667], [0.2, 0.85], [0.65, 0.1],
            [0.2, 0.85], [0.25, 0.95], [0.55, 0.05], [0.75, 0.15]].into_iter().enumerate() {
            for index in 0..2 {
                let error = (actual[frame][index] - expected[index]).abs();
                assert!(error.min((1.0 - error).abs()) < 1e-5,
                    "frame {frame} player {index}: {} != {}", actual[frame][index], expected[index]);
            }
        }
    }

    #[test]
    fn excluded_sequence_player_keeps_sampling_but_cannot_win_relevancy() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_graph::{AnimationGraph, NodeId, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                node_states::StateKey, system_resources::SystemResources,
            },
            edge_data::DataSpec,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };

        fn query(
            In((root, excluded, included)): In<(Handle<AnimationGraph>, NodeId, NodeId)>,
            resources: SystemResources,
        ) -> (Option<f32>, Option<f32>, f32, usize) {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let inputs = HashMap::new();
            let env = TestIoEnv::default();
            graph.query_with_context(
                QueryOutputTime::Forced(TimeUpdate::Delta(0.1)),
                GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources, &env,
                    Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs),
            ).unwrap();
            let states = &arena.get_toplevel().node_states;
            let excluded = &states.get::<SourceSequencePlayerState>(excluded, StateKey::Default).unwrap().context;
            let included = &states.get::<SourceSequencePlayerState>(included, StateKey::Default).unwrap().context;
            assert!(crate::extensions::animation_context::select_relevant_asset([
                (excluded.relevant_asset.clone(), 0.9),
            ]).is_none());
            let chosen = crate::extensions::animation_context::select_relevant_asset([
                (excluded.relevant_asset.clone(), 0.9),
                (included.relevant_asset.clone(), 0.1),
            ]).unwrap();
            (excluded.relevant_asset.as_ref().map(|asset| asset.duration),
                included.relevant_asset.as_ref().map(|asset| asset.duration),
                chosen.duration, excluded.asset_samples.len())
        }

        let mut world = World::new();
        let mut clips = Assets::<GraphClip>::default();
        let excluded_clip = clips.add(GraphClip { duration: 3.0, ..default() });
        let included_clip = clips.add(GraphClip { duration: 1.0, ..default() });
        world.insert_resource(clips);
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let mut graph = AnimationGraph::new();
        let player = |clip, ignore_for_relevancy_test| SourceSequencePlayerNode {
            clip, ignore_for_relevancy_test, expose_play_rate_pin: false,
            expose_start_position_pin: false, expose_loop_animation_pin: false,
            ..default()
        };
        let excluded = AnimationNode::new("excluded", player(excluded_clip, true));
        let included = AnimationNode::new("included", player(included_clip, false));
        for (node, output) in [(&excluded, "Excluded"), (&included, "Included")] {
            graph.add_node(node.clone());
            graph.add_output_data(output.to_owned(), DataSpec::Pose);
            graph.add_output_data_edge(node.id, "Pose", output);
        }
        let mut graphs = Assets::<AnimationGraph>::default();
        let root = graphs.add(graph);
        world.insert_resource(graphs);
        assert_eq!(world.run_system_cached_with(query, (root, excluded.id, included.id)).unwrap(),
            (None, Some(1.0), 1.0, 1));
    }

    #[test]
    fn raw_als_clips_reconstruct_their_virtual_bones_after_sampling() {
        use bevy::prelude::{AssetServer, Quat, Vec3};

        let (mut app, _, _) = crate::roll_drive::tests::loaded_roll_character();
        let cases = [
            (
                "ALS/Animations/Actions/GetUp/A_Als_GetUp_Front.anim.ron",
                "front.native.json",
                [3, 5],
            ),
            (
                "ALS/Animations/Actions/GetUp/A_Als_GetUp_Back.anim.ron",
                "back.native.json",
                [3, 5],
            ),
            (
                "ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.anim.ron",
                "sprint.native.json",
                [6, 7],
            ),
        ];
        let handles = cases.map(|(clip, _, _)| {
            app.world()
                .resource::<AssetServer>()
                .load::<GraphClip>(clip)
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let clips = app.world().resource::<Assets<GraphClip>>();
            let skeletons = app.world().resource::<Assets<Skeleton>>();
            if handles.iter().all(|handle| {
                clips
                    .get(handle)
                    .is_some_and(|clip| skeletons.contains(&clip.skeleton))
            }) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Raw sequence clips did not become ready"
            );
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let clips = app.world().resource::<Assets<GraphClip>>();
        let skeletons = app.world().resource::<Assets<Skeleton>>();
        let evidence = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/audit/bf144/d124");
        for ((_, file, samples), handle) in cases.into_iter().zip(handles) {
            let clip = clips.get(&handle).unwrap();
            let skeleton = skeletons.get(&clip.skeleton).unwrap();
            let native: serde_json::Value =
                serde_json::from_slice(&std::fs::read(evidence.join(file)).unwrap()).unwrap();
            let mut unbound = clip.clone();
            unbound.source = None;
            for index in samples {
                let frame = &native["sequence_pose_probe"]["frames"][index];
                let time = frame["time"].as_f64().unwrap() as f32;
                let baked = sample_pose(clip, time);
                let reconstructed = sample_raw_source_clip_pose(clip, time, skeletons).unwrap();
                let control = sample_raw_source_clip_pose(&unbound, time, skeletons).unwrap();
                let native_bones = frame["bones"].as_array().unwrap();
                let mut baked_error = 0.0_f32;
                for binding in &RAW_VIRTUAL_BONES.virtual_bones {
                    let name = binding.virtual_bone.as_str();
                    let id = skeleton
                        .iter_bones()
                        .find(|id| {
                            skeleton.id_to_path(*id).is_some_and(|path| {
                                path.parts.last().is_some_and(|bone| bone.as_str() == name)
                            })
                        })
                        .unwrap();
                    let bone = native_bones
                        .iter()
                        .find(|bone| bone["name"] == name)
                        .unwrap();
                    let transform = &bone["local_transform"];
                    let component =
                        |group: &str, axis: &str| transform[group][axis].as_f64().unwrap();
                    let position = Vec3::new(
                        (component("translation", "x") * 0.01) as f32,
                        (component("translation", "z") * 0.01) as f32,
                        (component("translation", "y") * 0.01) as f32,
                    );
                    let rotation = Quat::from_xyzw(
                        -component("rotation", "x") as f32,
                        -component("rotation", "z") as f32,
                        -component("rotation", "y") as f32,
                        component("rotation", "w") as f32,
                    );
                    let raw = baked.get_bone(id).unwrap();
                    let result = reconstructed.get_bone(id).unwrap();
                    let unbound = control.get_bone(id).unwrap();
                    assert_eq!(
                        (unbound.translation, unbound.rotation, unbound.scale),
                        (raw.translation, raw.rotation, raw.scale)
                    );
                    baked_error =
                        baked_error.max((raw.translation.unwrap() - position).abs().max_element());
                    assert!(
                        (result.translation.unwrap() - position).abs().max_element() < 3e-5,
                        "{file} {name} at {time}"
                    );
                    let actual = result.rotation.unwrap();
                    assert!(
                        (actual - rotation)
                            .length()
                            .min((actual + rotation).length())
                            < 3e-5,
                        "{file} {name} at {time}"
                    );
                }
                assert!(baked_error > 1e-4, "{file} at {time}");
            }
        }
    }

    #[test]
    fn source_root_lock_uses_reference_first_frame_and_additive_scale() {
        use bevy::animation::animated_field;
        use bevy_animation_graph::core::{animation_clip::EntityPath, pose::RootMotionDelta};
        let path = EntityPath::default().child("root");
        let root = path.id();
        let reference = Transform::from_translation(Vec3::Y * 0.2)
            .with_rotation(Quat::from_rotation_y(0.3)).with_scale(Vec3::splat(1.25));
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root);
        skeleton.add_bone(path, reference, reference);
        let mut animation = bevy::animation::AnimationClip::default();
        animation.add_curve_to_target(root.animation_target_id(), AnimatableCurve::new(
            animated_field!(Transform::translation),
            UnevenSampleAutoCurve::new([(0.0, Vec3::X * 0.4), (1.0, Vec3::Z * 2.75)]).unwrap(),
        ));
        let clip = GraphClip::from_bevy_clip(animation, default(), default(), None);
        let raw = sample_pose(&clip, 1.0);
        assert_eq!(raw.get_bone(root).unwrap().translation, Some(Vec3::Z * 2.75));
        for additive in [false, true] {
            for (mode, mut expected) in [
                (SourceRootLock::RefPose, reference),
                (SourceRootLock::AnimFirstFrame, reference.with_translation(Vec3::X * 0.4)),
                (SourceRootLock::Zero, Transform::IDENTITY),
            ] {
                let mut pose = raw.clone();
                pose.root_motion = Some(RootMotionDelta { translation: Vec3::X, ..default() });
                apply_source_root_lock(&mut pose, &clip, &skeleton, mode, additive).unwrap();
                if additive && mode != SourceRootLock::AnimFirstFrame { expected.scale -= Vec3::ONE; }
                let bone = pose.get_bone(root).unwrap();
                assert_eq!(bone.translation, Some(expected.translation));
                assert_eq!(bone.rotation, Some(expected.rotation));
                assert_eq!(bone.scale, Some(expected.scale));
                assert_eq!(pose.timestamp, 1.0);
                assert_eq!(pose.root_motion.unwrap().translation, Vec3::X);
            }
        }
    }

    #[test]
    fn sequence_endpoints_remain_inclusive_when_playback_stops() {
        for (start, delta, rate, expected) in [
            (1.5, 1.0 / 60.0, 0.0, 1.5),
            (1.375, 0.125, 1.0, 1.5),
            (0.125, 0.125, -1.0, 0.0),
            (1.375, 0.25, 1.0, 0.125),
            (0.125, 0.25, -1.0, 1.375),
        ] {
            let mut state = SourceSequencePlayerState::default();
            assert_eq!(next_position(&mut state, TimeUpdate::Delta(delta), 1.5, start, rate, true), expected);
            assert_eq!(next_position(&mut state, TimeUpdate::Delta(0.25), 1.5, start, 0.0, true), expected);
        }
    }

    #[test]
    fn replacing_a_sequence_clamps_the_previous_time_before_advancing() {
        for (looping, rate, expected) in [(false, -1.0, 0.875), (true, -1.0, 0.875),
            (false, 0.0, 1.0), (true, 0.0, 1.0), (true, 1.0, 0.125)] {
            let mut state = SourceSequencePlayerState::default();
            assert_eq!(next_position(&mut state, TimeUpdate::Delta(1.5), 2.0, 0.0, 1.0, false), 1.5);
            assert_eq!(next_position(&mut state, TimeUpdate::Delta(0.125), 1.0, 0.0, rate, looping), expected);
            assert_eq!(state.previous_position, 1.0);
        }
    }

    #[test]
    fn source_sequence_player_applies_start_position_and_rate() {
        let mut state = SourceSequencePlayerState::default();
        assert!(
            (next_position(&mut state, TimeUpdate::Delta(0.1), 1.0, 0.25, 1.25, false) - 0.375)
                .abs()
                < 1e-6
        );
        let mut long = SourceSequencePlayerState::default();
        assert!((next_position(&mut long, TimeUpdate::Delta(0.1), 2.5, 1.75, 1.0, true) - 1.85).abs() < 1e-6);
        assert!((marker_phase_from_sample_time(1.75, 2.5, &[], 2.5, &[], true) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn reverse_sequence_restarts_from_the_effective_start_on_reinitialization() {
        let mut state = SourceSequencePlayerState::default();
        assert!((next_position(&mut state, TimeUpdate::Delta(0.2), 1.0, 1.0, -1.0, false) - 0.8).abs() < 1e-6);
        assert_eq!(state.previous_position, 1.0);
        assert!((next_position(&mut state, TimeUpdate::Delta(0.2), 1.0, 1.0, -1.0, false) - 0.6).abs() < 1e-6);
        state = SourceSequencePlayerState::default();
        assert!((next_position(&mut state, TimeUpdate::Absolute(0.2), 1.0, 1.0, -1.0, false) - 0.8).abs() < 1e-6);
        assert_eq!(state.previous_position, 1.0);
    }

    #[test]
    fn source_sequence_player_wraps_looping_playback() {
        let mut state = SourceSequencePlayerState::default();
        assert!(
            (next_position(&mut state, TimeUpdate::Absolute(1.0), 1.0, 0.25, 1.0, true) - 0.25)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn source_sequence_player_records_the_loop_boundary_for_automatic_rules() {
        let mut state = SourceSequencePlayerState::default();
        assert!(
            (next_position(&mut state, TimeUpdate::Delta(0.9), 1.0, 0.0, 1.0, true) - 0.9).abs()
                < 1e-6
        );
        assert!(
            (next_position(&mut state, TimeUpdate::Delta(0.2), 1.0, 0.0, 1.0, true) - 0.1).abs()
                < 1e-6
        );
        assert!(state.has_previous_position);
        assert!((state.previous_position - 0.9).abs() < 1e-6);
        assert!((state.delta_seconds - 0.2).abs() < 1e-6);
    }

    #[test]
    fn source_sequence_player_can_hide_an_unexposed_play_rate_pin() {
        let node = SourceSequencePlayerNode {
            expose_play_rate_pin: false,
            ..Default::default()
        };
        assert!(!node.expose_play_rate_pin);
    }

    #[test]
    fn source_sequence_player_can_hide_an_unexposed_loop_pin() {
        let node = SourceSequencePlayerNode {
            expose_loop_animation_pin: false,
            ..Default::default()
        };
        assert!(!node.expose_loop_animation_pin);
    }

    #[test]
    fn source_sequence_player_preserves_source_visible_pin_names() {
        let node = SourceSequencePlayerNode {
            play_rate_pin: "PlayRate".to_owned(),
            start_position_pin: "StartPosition".to_owned(),
            loop_animation_pin: "bLoopAnimation".to_owned(),
            ..Default::default()
        };
        assert_eq!(node.play_rate_pin, "PlayRate");
        assert_eq!(node.start_position_pin, "StartPosition");
        assert_eq!(node.loop_animation_pin, "bLoopAnimation");
    }

    #[test]
    fn source_sequence_player_can_hide_an_unexposed_start_position_pin() {
        let node = SourceSequencePlayerNode {
            expose_start_position_pin: false,
            ..Default::default()
        };
        assert!(!node.expose_start_position_pin);
    }

    #[test]
    fn source_sequence_player_defaults_additive_references_to_local_space() {
        assert!(!SourceSequencePlayerNode::default().additive_mesh_space);
    }

    #[test]
    fn source_sequence_player_applies_basis_before_source_scale_bias_clamp() {
        let mapping = SourceAlphaMapping {
            source_semantics: true,
            map_range: true,
            input_min: 0.0,
            input_max: 1.0,
            output_min: 0.5,
            output_max: 1.5,
            scale: 2.0,
            bias: -0.25,
            clamp_result: true,
            clamp_min: 0.0,
            clamp_max: 2.0,
            ..Default::default()
        };
        let mut state = SourceInputScaleBiasClampState::default();
        let rate = apply_source_input_scale_bias_clamp(&mut state, 50.0 / 100.0, &mapping, 0.0);
        assert!((rate - 1.75).abs() < 1e-6);
    }
}
