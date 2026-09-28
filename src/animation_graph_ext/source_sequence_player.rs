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

use crate::extensions::global_f32_input::root_defaults_enabled;
use crate::extensions::sync_group_phase::{
    SourceSyncGroupRole, SourceSyncPlayerTick, SyncMarker, marker_phase_from_sample_time,
    resolved_source_sync_phase, source_sync_delta_seconds,
    resolved_source_sync_markers, marker_phase_sample_time_near,
    marker_phase_sample_time_continuous,
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
    let previous_position = if state.initialized { state.position } else { start_position };
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
    let root = crate::extensions::root_motion_defaults::derive_default_root_bone(clip, skeleton)?.id();
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
        let notify_instance_id = crate::extensions::source_instance::source_notify_instance_id(&ctx)?;
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
                weight: 1.0,
                has_been_full_weight: state.context.sync_players.first().is_some_and(|tick| tick.has_been_full_weight)
                    || reached_full_weight,
                requested_inertialization,
                previous_phase,
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
            tick.phase = resolved_phase.unwrap_or_else(|| tick.advance(previous_phase));
            state.previous_position = previous_sample_time;
            state.has_previous_position = state.initialized;
            state.initialized = true;
            state.delta_seconds = delta;
            state.position = if state.has_previous_position && resolved_phase.is_some() {
                marker_phase_sample_time_continuous(tick.phase, previous_phase,
                    sync.phase_duration, phase_markers, clip.duration(), markers,
                    previous_sample_time, loop_animation)
            } else {
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
            curves: self.curves.sample_all(position),
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
                slot_key: String::new(),
            });
        }
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
            for (frame, phase) in [0.2, 0.19, 0.95, 0.11, 0.94, 0.2].into_iter().enumerate() {
                if frame == 5 {
                    *arena.get_toplevel_mut() = GraphState::new(handle.id());
                }
                let inputs = HashMap::from([
                    ("delta".to_owned(), DataValue::F32(0.1)),
                    (crate::extensions::sync_group_phase::SOURCE_SYNC_RESOLVED.to_owned(), DataValue::Bool(true)),
                    (format!("{}.Movement", crate::extensions::sync_group_phase::SOURCE_SYNC_RESOLVED), DataValue::F32(phase)),
                ]);
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
        for (frame, expected) in [[0.75, 0.15], [0.73, 0.14], [1.0, 1.0],
            [0.06, 0.06], [0.9966667, 0.9966667], [0.75, 0.15]].into_iter().enumerate() {
            for index in 0..2 {
                assert!((actual[frame][index] - expected[index]).abs() < 1e-5,
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
