use std::any::TypeId;

use bevy::{
    animation::animation_curves::WeightsCurveSample,
    asset::Handle,
    platform::hash::Hashed,
    prelude::{EvaluatorId, Transform, VariableCurve},
    reflect::prelude::*,
};
use bevy_animation_graph::core::{
    animation_clip::GraphClip,
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
    id::BoneId,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};

use crate::global_f32_input::global_f32_or_root_default;
use crate::global_f32_input::root_defaults_enabled;
use crate::sync_group_phase::{
    SourceSyncGroupRole, SourceSyncPlayerTick, SyncMarker, marker_phase_from_sample_time,
    marker_phase_sample_time, resolved_source_sync_phase,
};
use crate::{
    AnimationContextState, SourceRelevantAsset, context_time_update, make_local_difference,
    make_mesh_space_difference,
    source_pose_nodes::{
        SourceAlphaMapping, SourceInputScaleBiasClampState, apply_source_input_scale_bias_clamp,
    },
};

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct SourceSequencePlayerState {
    initialized: bool,
    position: f32,
    previous_position: f32,
    has_previous_position: bool,
    delta_seconds: f32,
    pub(crate) context: AnimationContextState,
    play_rate_state: SourceInputScaleBiasClampState,
}

enum CurveSample {
    Translation(bevy::math::Vec3),
    Rotation(bevy::math::Quat),
    Scale(bevy::math::Vec3),
    BoneWeights(Vec<f32>),
}

fn sample_curve(curve: &VariableCurve, time: f32) -> Option<CurveSample> {
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
    for (bone_id, curves) in &clip.curves {
        let mut bone_pose = BonePose::default();
        for curve in curves {
            match sample_curve(curve, time) {
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

pub(crate) fn sample_source_pose(
    ctx: &NodeContext,
    clip: &GraphClip,
    time: f32,
    additive_reference: Option<&Handle<GraphClip>>,
    reference_position: f32,
    mesh_space: bool,
    root_lock: Option<SourceRootLock>,
) -> Result<Pose, GraphError> {
    let target = sample_pose(clip, time);
    let mut pose = if let Some(reference_handle) = additive_reference {
        let reference_clip = ctx.graph_context.resources.graph_clip_assets.get(reference_handle)
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        let reference = sample_pose(reference_clip, reference_position.max(0.0));
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
        apply_source_root_lock(&mut pose, clip, skeleton, mode, additive_reference.is_some())
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
    }
    Ok(pose)
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
    let previous_position = state.position;
    let has_previous_position = state.initialized && matches!(&update, TimeUpdate::Delta(_));
    if !state.initialized {
        state.initialized = true;
        state.position = start_position;
    }
    match &update {
        TimeUpdate::Delta(value) => state.position += (*value).max(0.0) * play_rate,
        TimeUpdate::Absolute(value) => state.position = start_position + *value * play_rate,
        TimeUpdate::PercentOfEvent { .. } => {}
    }
    state.position = if loop_animation {
        state.position.rem_euclid(duration)
    } else {
        state.position.clamp(0.0, duration)
    };
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
    match crate::source_instance::source_input_data(ctx, key) {
        Some(value) => value.as_f32(),
        None if root_defaults_enabled(ctx) => Ok(fallback),
        None => Err(GraphError::GlobalInputDataMissing(key.to_owned())),
    }
}

fn global_bool_or_field(ctx: &NodeContext, key: &str, fallback: bool) -> Result<bool, GraphError> {
    if key.is_empty() {
        return Ok(fallback);
    }
    match crate::source_instance::source_input_data(ctx, key) {
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
    pub curves: crate::als_curves::AlsPoseCurves,
    pub root_lock: Option<SourceRootLock>,
    pub additive_reference: Option<Handle<GraphClip>>,
    pub additive_reference_position: f32,
    pub additive_mesh_space: bool,
    pub play_rate: f32,
    pub play_rate_key: String,
    pub play_rate_pin: String,
    pub expose_play_rate_pin: bool,
    pub play_rate_basis: f32,
    pub play_rate_mapping: SourceAlphaMapping,
    pub start_position: f32,
    pub start_position_pin: String,
    pub expose_start_position_pin: bool,
    pub loop_animation: bool,
    pub loop_animation_key: String,
    pub loop_animation_pin: String,
    pub expose_loop_animation_pin: bool,
    pub sync_group: String,
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
            play_rate_key: Default::default(),
            play_rate_pin: "Play Rate".to_owned(),
            expose_play_rate_pin: true,
            play_rate_basis: 1.0,
            play_rate_mapping: Default::default(),
            start_position: 0.0,
            start_position_pin: "Start Position".to_owned(),
            expose_start_position_pin: true,
            loop_animation: true,
            loop_animation_key: Default::default(),
            loop_animation_pin: "Loop Animation".to_owned(),
            expose_loop_animation_pin: false,
            sync_group: Default::default(),
            synchronization: None,
        }
    }
}

#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceRootLock {
    RefPose,
    AnimFirstFrame,
    Zero,
}

pub fn apply_source_root_lock(
    pose: &mut Pose,
    clip: &GraphClip,
    skeleton: &Skeleton,
    mode: SourceRootLock,
    additive: bool,
) -> Option<()> {
    let root = crate::root_motion_defaults::derive_default_root_bone(clip, skeleton)?.id();
    let reference = skeleton.default_transforms(root)?.local;
    let mut transform = match mode {
        SourceRootLock::RefPose => reference,
        SourceRootLock::Zero => Transform::IDENTITY,
        SourceRootLock::AnimFirstFrame => {
            let first = sample_pose(clip, 0.0);
            let bone = first.get_bone(root)?;
            Transform {
                translation: bone.translation.unwrap_or(reference.translation),
                rotation: bone.rotation.unwrap_or(reference.rotation),
                scale: bone.scale.unwrap_or(reference.scale),
            }
        }
    };
    if additive && mode != SourceRootLock::AnimFirstFrame {
        transform.scale -= bevy::math::Vec3::ONE;
    }
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
            global_f32_or_root_default(&ctx, &sync.delta_seconds_key)?
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
        let identity = (ctx.graph_context.context_id, ctx.node_id);
        let state = ctx.state_mut_or_else(SourceSequencePlayerState::default)?;
        let play_rate = apply_source_input_scale_bias_clamp(
            &mut state.play_rate_state,
            if self.play_rate_basis.abs() <= f32::EPSILON {
                0.0
            } else {
                play_rate / self.play_rate_basis
            },
            &self.play_rate_mapping,
            delta,
        );
        let mut sync_players = Vec::new();
        let position = if let Some(sync) = &self.synchronization {
            let previous_phase = state
                .context
                .sync_players
                .first()
                .map(|tick| tick.phase)
                .unwrap_or_else(|| {
                    marker_phase_from_sample_time(
                        start_position,
                        clip.duration(),
                        &sync.markers,
                        sync.phase_duration,
                        &sync.phase_markers,
                        loop_animation,
                    )
                });
            let mut tick = SourceSyncPlayerTick {
                identity,
                key: sync.key.clone(),
                role: sync.role,
                weight: 1.0,
                previous_phase,
                phase_duration: sync.phase_duration,
                phase_markers: sync.phase_markers.clone(),
                duration: clip.duration(),
                markers: sync.markers.clone(),
                looped: loop_animation,
                delta_seconds: delta,
                play_rate,
                ..Default::default()
            };
            tick.phase = resolved_phase.unwrap_or_else(|| tick.advance(previous_phase));
            state.previous_position = marker_phase_sample_time(
                previous_phase,
                sync.phase_duration,
                &sync.phase_markers,
                clip.duration(),
                &sync.markers,
                loop_animation,
            );
            state.has_previous_position = state.initialized;
            state.initialized = true;
            state.delta_seconds = delta;
            state.position = marker_phase_sample_time(
                tick.phase,
                sync.phase_duration,
                &sync.phase_markers,
                clip.duration(),
                &sync.markers,
                loop_animation,
            );
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
        state.context = AnimationContextState {
            curves: self.curves.sample_all(position),
            relevant_asset: Some(SourceRelevantAsset {
                duration: clip.duration(),
                position,
                previous_position: state.previous_position,
                has_previous_position: state.has_previous_position,
                delta_seconds: state.delta_seconds,
                looping: loop_animation,
                weight: 1.0,
            }),
            curves_are_authoritative: true,
            sync_players,
            ..Default::default()
        };
        let pose = sample_source_pose(&ctx, clip, position, self.additive_reference.as_ref(), self.additive_reference_position, self.additive_mesh_space, self.root_lock)?;
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
    fn source_sequence_player_applies_start_position_and_rate() {
        let mut state = SourceSequencePlayerState::default();
        assert!(
            (next_position(&mut state, TimeUpdate::Delta(0.1), 1.0, 0.25, 1.25, false) - 0.375)
                .abs()
                < 1e-6
        );
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
