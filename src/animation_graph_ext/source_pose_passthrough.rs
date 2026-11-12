use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{
        deferred_gizmos::DeferredGizmos, graph_context_arena::GraphContextArena,
        new_context::NodeContext, spec_context::SpecContext,
    },
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
};

use crate::extensions::animation_context::{AnimationContextState, pose_input_context_state};
use crate::extensions::sync_group_phase::{
    SOURCE_SYNC_RESOLVED, resolve_source_sync_phases, source_sync_excluded_key,
    source_sync_full_weight_key, source_sync_marker_name_key, source_sync_marker_set_key,
};

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourcePosePassthroughNode {
    pub input: String,
    pub output: String,
    pub input_time: String,
    pub output_time: bool,
    pub source_function: String,
    pub call_site: String,
    pub synchronize_groups: bool,
    #[reflect(default)]
    pub linked_pose_cache_order: Vec<String>,
}

impl Default for SourcePosePassthroughNode {
    fn default() -> Self {
        Self {
            input: "Source".to_owned(),
            output: "Pose".to_owned(),
            input_time: "source_time".to_owned(),
            output_time: true,
            source_function: default(),
            call_site: default(),
            synchronize_groups: false,
            linked_pose_cache_order: default(),
        }
    }
}

fn order_source_sync_players(ctx: &NodeContext, context: &mut AnimationContextState) {
    let owner = ctx.graph_context.context_id;
    context.sync_players.sort_by_key(|player| {
        player.cached_submission_order
            .filter(|(context, _)| *context == owner)
            .map(|(_, order)| order)
    });
}

impl NodeLike for SourcePosePassthroughNode {
    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if self.output_time {
            let duration = ctx.duration_back(self.input_time.clone())?;
            ctx.set_duration_fwd(duration);
        } else {
            ctx.set_duration_fwd(None);
        }
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let mut global_inputs = std::borrow::Cow::Borrowed(ctx.graph_context.global_input_data);
        if self.synchronize_groups && !global_inputs.contains_key(SOURCE_SYNC_RESOLVED) {
            let snapshot = ctx
                .graph_context
                .context_arena
                .get_ref()
                .reflect_clone()
                .map_err(|error| GraphError::UnknownErrorNode {
                    graph: ctx.graph_context.context_id,
                    node: ctx.node_id,
                    error: format!("source sync snapshot failed: {error}"),
                })?;
            let mut snapshot = snapshot
                .take::<GraphContextArena>()
                .map_err(|_| GraphError::MissingStateValue)?;
            let mut gizmos = DeferredGizmos::default();
            global_inputs
                .to_mut()
                .insert(SOURCE_SYNC_RESOLVED.to_owned(), DataValue::Bool(false));
            let mut preview_ctx = ctx.clone();
            preview_ctx.graph_context.context_arena = (&mut snapshot).into();
            preview_ctx.graph_context.deferred_gizmos = (&mut gizmos).into();
            preview_ctx.graph_context.global_input_data = &global_inputs;
            crate::extensions::source_cached_pose::prewarm_linked_pose_caches(&preview_ctx, &self.input, &self.linked_pose_cache_order)?;
            preview_ctx.data_back(self.input.clone())?;
            let mut current = pose_input_context_state(&preview_ctx, &self.input).unwrap_or_default();
            order_source_sync_players(&preview_ctx, &mut current);
            let previous = ctx
                .state::<AnimationContextState>()
                .cloned()
                .unwrap_or_default();
            for player in &current.sync_players {
                if matches!(player.role,
                    crate::extensions::sync_group_phase::SourceSyncGroupRole::TransitionLeader
                        | crate::extensions::sync_group_phase::SourceSyncGroupRole::TransitionFollower)
                    && player.weight >= 1.0 - 1e-5
                {
                    global_inputs.to_mut().insert(
                        source_sync_full_weight_key(player.identity),
                        DataValue::Bool(true),
                    );
                }
            }
            for crate::extensions::sync_group_phase::SourceSyncGroupResolution { key, phase, length_ratios, ungrouped, markers: common, marker_advance, leader_samples, marker_resets } in
                resolve_source_sync_phases(&current.sync_players, &previous.sync_players)
            {
                for identity in marker_resets {
                    global_inputs.to_mut().insert(
                        crate::extensions::sync_group_phase::source_sync_marker_reset_key(identity), DataValue::Bool(true));
                }
                if let Some((identity, ..)) = leader_samples.last() {
                    let key = crate::extensions::sync_group_phase::source_sync_sample_time_key(*identity);
                    global_inputs.to_mut().insert(format!("{key}.selected"), DataValue::Bool(true));
                }
                for (identity, step) in leader_samples {
                    let sample_key = crate::extensions::sync_group_phase::source_sync_sample_time_key(identity);
                    global_inputs.to_mut().insert(format!("{sample_key}.previous"), DataValue::F32(step.previous_time));
                    if let Some(record) = step.record { global_inputs.to_mut().extend(record.into_inputs(&sample_key)); }
                    global_inputs.to_mut().insert(sample_key, DataValue::F32(step.time));
                }
                if let Some(phase) = phase {
                    if let Some(ratios) = length_ratios {
                        global_inputs.to_mut().insert(format!("{SOURCE_SYNC_RESOLVED}.length_ratios.{key}"), DataValue::Vec2(ratios));
                    }
                    global_inputs.to_mut().insert(
                        format!("{SOURCE_SYNC_RESOLVED}.{key}"),
                        DataValue::F32(phase),
                    );
                    global_inputs.to_mut().insert(source_sync_marker_set_key(&key), DataValue::Bool(true));
                    if let Some(advance) = marker_advance {
                        global_inputs.to_mut().extend(advance.into_inputs(&key));
                    }
                    for name in common {
                        global_inputs.to_mut().insert(source_sync_marker_name_key(&key, &name), DataValue::Bool(true));
                    }
                }
                for identity in ungrouped {
                    global_inputs
                        .to_mut()
                        .insert(source_sync_excluded_key(identity), DataValue::Bool(true));
                }
            }
            global_inputs
                .to_mut()
                .insert(SOURCE_SYNC_RESOLVED.to_owned(), DataValue::Bool(true));
        }
        let mut source_ctx = ctx.clone();
        source_ctx.graph_context.global_input_data = &global_inputs;
        crate::extensions::source_cached_pose::prewarm_linked_pose_caches(&source_ctx, &self.input, &self.linked_pose_cache_order)?;
        let pose = source_ctx.data_back(self.input.clone())?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(self.output.clone(), pose);
        if let Some(mut context) = pose_input_context_state(&source_ctx, &self.input) {
            if self.synchronize_groups {
                order_source_sync_players(&source_ctx, &mut context);
                for sample in &mut context.asset_samples {
                    if sample.sync_identity.is_some_and(|identity|
                        global_inputs.contains_key(&source_sync_excluded_key(identity))) {
                        sample.sync_group.clear();
                        sample.sync_identity = None;
                    }
                }
                for player in &mut context.sync_players {
                    let key = crate::extensions::sync_group_phase::source_sync_sample_time_key(player.identity);
                    player.ticked_as_leader = global_inputs.contains_key(&key);
                    player.selected_leader = global_inputs.contains_key(&format!("{key}.selected"));
                    if player.selected_leader {
                        player.marker_advance = crate::extensions::sync_group_phase::resolved_source_marker_advance(&source_ctx, &player.key)?;
                        player.resolved_length_ratio = crate::extensions::sync_group_phase::resolved_source_sync_length_ratios(
                            &source_ctx, &player.key,
                        )?.map(|ratios| ratios.y);
                    }
                    if global_inputs.contains_key(&source_sync_excluded_key(player.identity)) {
                        continue;
                    }
                    if let Some(phase) = crate::extensions::sync_group_phase::resolved_source_sync_phase(
                        &source_ctx,
                        &player.key,
                    )? {
                        player.phase = phase;
                    }
                }
            }
            *ctx.state_mut_or_else(AnimationContextState::default)? = context;
        }
        if self.output_time {
            let time_update = ctx.time_update_fwd()?;
            ctx.set_time_update_back(self.input_time.clone(), time_update);
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(self.input.clone(), DataSpec::Pose)
            .add_output_data(self.output.clone(), DataSpec::Pose);
        if self.output_time {
            ctx.add_input_time(self.input_time.clone())
                .add_output_time();
        }
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Pose Passthrough".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_snapshot_isolates_cached_state_and_lifecycle_reset() {
        use bevy_animation_graph::core::{
            animation_graph::{AnimationGraph, NodeId},
            context::{graph_context_arena::GraphContextArena, node_states::StateKey},
        };
        let node = NodeId::default();
        let mut arena = GraphContextArena::new(Handle::<AnimationGraph>::default().id());
        arena
            .get_toplevel_mut()
            .node_states
            .get_mut_or_insert_with(node, StateKey::Default, || 7_u64)
            .unwrap();
        arena.next_frame();
        let mut snapshot = arena
            .reflect_clone()
            .unwrap()
            .take::<GraphContextArena>()
            .unwrap();
        snapshot.get_toplevel_mut().node_states.clear();
        snapshot
            .get_toplevel_mut()
            .node_states
            .get_mut_or_insert_with(node, StateKey::Default, || 11_u64)
            .unwrap();
        assert_eq!(
            *arena
                .get_toplevel()
                .node_states
                .get::<u64>(node, StateKey::Default)
                .unwrap(),
            7
        );
        assert_eq!(
            *snapshot
                .get_toplevel()
                .node_states
                .get::<u64>(node, StateKey::Default)
                .unwrap(),
            11
        );
    }

    #[test]
    fn leader_resync_uses_actual_sequence_and_blendspace_own_times_without_markers() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::{AnimationGraph, GraphInputPin, NodeId, TimeUpdate},
            animation_node::AnimationNode, context::{graph_context::QueryOutputTime, new_context::GraphContext, node_states::StateKey, system_resources::SystemResources},
            pose::Pose,
            skeleton::Skeleton, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        use crate::extensions::{SourceSequencePlayerNode, SourceTwoWayBlendNode};
        use crate::extensions::source_sequence_player::{SourceSequenceSynchronization, SourceSequencePlayerState};
        use crate::extensions::normalized_blendspace::{NormalizedBlendSpaceNode, NormalizedBlendSpacePoint, NormalizedBlendSpaceState};
        use crate::extensions::sync_group_phase::{SourceSyncGroupRole, SourceSyncPlayerBinding};
        fn query(In((handle, player_id, blendspace)): In<(Handle<AnimationGraph>, NodeId, bool)>, resources: SystemResources) -> Vec<f32> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let env = TestIoEnv::default();
            let entities = HashMap::new();
            let mut times = Vec::new();
            for (frame, alpha) in [0.0, 1.0, 1.0].into_iter().enumerate() {
                let inputs = HashMap::from([("alpha".into(), DataValue::F32(alpha)), ("delta".into(), DataValue::F32(0.1))]);
                graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.1)),
                    GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources, &env,
                        Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs)).unwrap();
                if frame == 0 { continue; }
                let time = if blendspace {
                    arena.get_toplevel().node_states.get::<NormalizedBlendSpaceState>(player_id, StateKey::Default).unwrap()
                        .context.sync_players[0].phase
                } else {
                    arena.get_toplevel().node_states.get::<SourceSequencePlayerState>(player_id, StateKey::Default).unwrap().context.sync_players[0].phase
                };
                times.push(time);
            }
            times
        }
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let clips = [0, 1].map(|_| world.resource_mut::<Assets<GraphClip>>().add(GraphClip { duration: 1.0, ..default() }));
        let mut actual = Vec::new();
        for blendspace in [false, true] {
            let sync = || SourceSequenceSynchronization { key: "Movement".into(), delta_seconds_key: "delta".into(),
                phase_duration: 1.0, role: SourceSyncGroupRole::CanBeLeader, ..default() };
            let leader = AnimationNode::new("prior", SourceSequencePlayerNode { clip: clips[0].clone(), start_position: 0.7,
                loop_animation: true, synchronization: Some(sync()), ..default() });
            let player = if blendspace {
                AnimationNode::new("new", NormalizedBlendSpaceNode { looped: true, phase_duration: 1.0,
                    points: vec![NormalizedBlendSpacePoint { id: "sample".into(), duration: 1.0, ..default() }],
                    playback: Some(SourceSyncPlayerBinding { key: "Movement".into(), delta_seconds_key: "delta".into(),
                        start_position: 0.2, role: SourceSyncGroupRole::CanBeLeader, ..default() }), ..default() })
            } else {
                AnimationNode::new("new", SourceSequencePlayerNode { clip: clips[1].clone(), start_position: 0.2,
                    loop_animation: true, synchronization: Some(sync()), ..default() })
            };
            let blend = AnimationNode::new("blend", SourceTwoWayBlendNode { alpha_key: "alpha".into(), expose_alpha_pin: false, ..default() });
            let root = AnimationNode::new("sync", SourcePosePassthroughNode { synchronize_groups: true, output_time: false, ..default() });
            let mut graph = AnimationGraph::new();
            if blendspace {
                graph.set_default_data(GraphInputPin::passthrough("sample"), DataValue::Pose(Pose::default()));
                graph.set_default_data(GraphInputPin::passthrough("position"), DataValue::Vec2(Vec2::ZERO));
                graph.add_input_data_edge(GraphInputPin::passthrough("sample"), player.id, NormalizedBlendSpaceNode::pose_pin("sample"));
                graph.add_input_data_edge(GraphInputPin::passthrough("position"), player.id, crate::extensions::normalized_blendspace::IN_POSITION);
            }
            graph.add_node_parameter_edge(leader.id, "Pose", blend.id, "A");
            graph.add_node_parameter_edge(player.id, if blendspace { "pose" } else { "Pose" }, blend.id, "B");
            graph.add_node_parameter_edge(blend.id, "Pose", root.id, "Source");
            graph.add_output_data("Pose".into(), DataSpec::Pose);
            graph.add_output_data_edge(root.id, "Pose", "Pose");
            for node in [leader, player.clone(), blend, root] { graph.add_node(node); }
            let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
            actual.push(world.run_system_cached_with(query, (handle, player.id, blendspace)).unwrap());
        }
        assert!(actual.iter().all(|times| times.iter().zip([0.3, 0.4]).all(|(time, expected)| (time - expected).abs() < 1e-6)),
            "New Sequence/BlendSpace leader used the previous group phase: {actual:?}");
    }

    #[test]
    fn excluded_blendspace_samples_keep_their_own_clock() {
        use bevy::{animation::animated_field, platform::collections::HashMap};
        use bevy_animation_graph::{builtin_nodes::clip_node::ClipNode, core::{
            animation_clip::GraphClip, animation_graph::{AnimationGraph, GraphInputPin, NodeId, TimeUpdate},
            animation_node::AnimationNode, context::{graph_context::QueryOutputTime, node_states::StateKey, system_resources::SystemResources},
            skeleton::Skeleton, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        }};
        use crate::extensions::{SourceMultiWayBlendNode, SourceSequencePlayerNode};
        use crate::extensions::normalized_blendspace::{NormalizedBlendSpaceNode, NormalizedBlendSpacePoint, NormalizedBlendSpaceState, SourceBlendSpaceTriangulation};
        use crate::extensions::source_sequence_player::{SourceSequenceSynchronization, SourceSequencePlayerState};
        use crate::extensions::sync_group_phase::{SourceSyncGroupRole, SourceSyncPlayerBinding, SOURCE_SYNC_INERTIAL_SCOPE};

        fn query(In((handle, player_id, leader_id, external_start, full_history, inertial, excluded)):
            In<(Handle<AnimationGraph>, NodeId, NodeId, f32, bool, bool, bool)>, resources: SystemResources) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut frames = if full_history { vec![(0.0, 1.0, false, 1.0), (0.0, 0.5, false, 1.0)] }
                else { vec![(0.0, 0.5, false, 1.0)] };
            let rate = if full_history { if external_start == 0.75 { 5.0 } else { 0.0 } } else { 1.0 };
            frames.extend([(0.125, 0.5, inertial, rate); 2]);
            let mut own_time = if full_history { external_start } else { 0.25 };
            let mut external_time = external_start;
            for (delta, weight, request, rate) in frames {
                let inputs = HashMap::from([
                    ("delta".into(), DataValue::F32(delta)), ("rate".into(), DataValue::F32(rate)),
                    ("player_weight".into(), DataValue::F32(weight)), ("leader_weight".into(), DataValue::F32(1.0 - weight)),
                    (SOURCE_SYNC_INERTIAL_SCOPE.into(), DataValue::Bool(request)),
                ]);
                let context = bevy_animation_graph::core::context::new_context::GraphContext::new(
                    arena.get_toplevel_id(), graph, &mut arena, &resources, &env,
                    Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                let outputs = graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(delta)), context).unwrap();
                if delta == 0.0 { continue; }
                own_time = (own_time + delta).rem_euclid(1.0);
                external_time = (external_time + delta * rate).rem_euclid(1.0);
                let expected = if excluded { own_time } else { external_time };
                let states = &arena.get_toplevel().node_states;
                let state = states.get::<NormalizedBlendSpaceState>(player_id, StateKey::Default).unwrap();
                assert_eq!(state.context.sync_players[0].has_been_full_weight, full_history);
                assert_eq!(state.context.sync_players[0].requested_inertialization, request);
                let leader = states.get::<SourceSequencePlayerState>(leader_id, StateKey::Default).unwrap();
                assert!((leader.context.relevant_asset.as_ref().unwrap().position - external_time).abs() < 1e-5,
                    "Wrong external leader: external={external_start}, full={full_history}, inertial={request}, excluded={excluded}, external_time={external_time}, actual={:?}", leader.context.relevant_asset);
                let curve = state.context.curves[&crate::extensions::SourceCurveName::from("SampleTime")];
                for sample in ["a", "b"] {
                    let update = arena.get_toplevel().node_caches.get_input_time_update(player_id, StateKey::Default,
                        NormalizedBlendSpaceNode::time_pin(sample)).unwrap();
                    assert!(matches!(update, TimeUpdate::Absolute(time) if (time - expected).abs() < 1e-5),
                        "sample={sample} external={external_start} full={full_history} inertial={request} excluded={excluded}: {update:?} != {expected}, curve={curve}");
                }
                assert!((curve - expected).abs() < 1e-5, "mixed curve {curve} != {expected}");
                let bone = bevy::animation::AnimationTargetId::from_name(&Name::new("clock")).into();
                let pose = outputs["Pose"].clone().into_pose().unwrap();
                assert!((pose.get_bone(bone).unwrap().translation.unwrap().x - (expected + external_time) * 0.5).abs() < 1e-5);
                let root = graph.nodes.values().find(|node| node.name == "sync").unwrap();
                let context = states.get::<AnimationContextState>(root.id, StateKey::Default).unwrap();
                let sample = context.asset_samples.iter().find(|sample| sample.clip == state.context.asset_samples[0].clip).unwrap();
                assert_eq!(sample.sync_group.is_empty(), excluded);
                assert_eq!(crate::extensions::sync_group_phase::source_sync_leader(&arena, &resources.animation_graph_assets, "Movement").unwrap().identity.1, leader_id);
            }
        }

        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let mut animation = bevy::animation::AnimationClip::default();
        animation.add_curve_to_target(bevy::animation::AnimationTargetId::from_name(&Name::new("clock")),
            AnimatableCurve::new(animated_field!(Transform::translation),
                UnevenSampleAutoCurve::new([(0.0, Vec3::ZERO), (1.0, Vec3::X)]).unwrap()));
        let clips = (0..3).map(|_| world.resource_mut::<Assets<GraphClip>>()
            .add(GraphClip::from_bevy_clip(animation.clone(), default(), default(), None))).collect::<Vec<_>>();
        let curve = crate::extensions::parse_curves(&serde_json::json!({"curves": [{
            "name": "SampleTime", "times": [0.0, 1.0], "values": [0.0, 1.0], "interps": ["LINEAR", "LINEAR"]
        }]})).unwrap();
        for (role, full_history, inertial, excluded) in [
            (SourceSyncGroupRole::TransitionFollower, false, false, true),
            (SourceSyncGroupRole::TransitionLeader, false, false, true),
            (SourceSyncGroupRole::TransitionFollower, true, true, true),
            (SourceSyncGroupRole::TransitionLeader, true, true, true),
            (SourceSyncGroupRole::ExclusiveAlwaysLeader, false, false, true),
            (SourceSyncGroupRole::AlwaysFollower, false, false, false),
            (SourceSyncGroupRole::TransitionFollower, true, false, false),
            (SourceSyncGroupRole::TransitionLeader, true, false, false),
        ] {
            for external_start in [0.75, 0.125] {
                let player = AnimationNode::new("blendspace", NormalizedBlendSpaceNode {
                    looped: true, phase_duration: 1.0,
                    points: ["a", "b"].into_iter().enumerate().map(|(index, id)| NormalizedBlendSpacePoint {
                        id: id.into(), point: Vec2::X * index as f32, duration: 1.0, curves: curve.clone(), ..default()
                    }).collect(),
                    source_triangulation: Some(SourceBlendSpaceTriangulation { axis_min: Vec2::ZERO, axis_max: Vec2::ONE, segments: vec![[0, 1]], ..default() }),
                    playback: Some(SourceSyncPlayerBinding { key: "Movement".into(), delta_seconds_key: "delta".into(), start_position: 0.25, role, ..default() }), ..default()
                });
                let leader = AnimationNode::new("leader", SourceSequencePlayerNode {
                    clip: clips[2].clone(), start_position: external_start, play_rate_key: "rate".into(), loop_animation: true,
                    synchronization: Some(SourceSequenceSynchronization { key: "Movement".into(), delta_seconds_key: "delta".into(),
                        phase_duration: 1.0, role: SourceSyncGroupRole::AlwaysLeader, ..default() }), ..default()
                });
                let blend = AnimationNode::new("blend", SourceMultiWayBlendNode { pose_count: 2,
                    desired_alpha_keys: vec!["player_weight".into(), "leader_weight".into()], desired_alpha_defaults: vec![0.5, 0.5], ..default() });
                let root = AnimationNode::new("sync", SourcePosePassthroughNode { synchronize_groups: true, output_time: false, ..default() });
                let mut graph = AnimationGraph::new();
                graph.set_default_data(GraphInputPin::passthrough("position"), DataValue::Vec2(Vec2::X * 0.25));
                graph.add_input_data_edge(GraphInputPin::passthrough("position"), player.id, crate::extensions::normalized_blendspace::IN_POSITION);
                for (index, id) in ["a", "b"].into_iter().enumerate() {
                    let sample = AnimationNode::new(id, ClipNode::new(clips[index].clone(), None, None));
                    graph.add_node_parameter_edge(sample.id, ClipNode::OUT_POSE, player.id, NormalizedBlendSpaceNode::pose_pin(id));
                    graph.add_node_pose_edge(sample.id, player.id, NormalizedBlendSpaceNode::time_pin(id));
                    graph.add_node(sample);
                }
                graph.add_node_parameter_edge(player.id, crate::extensions::normalized_blendspace::OUT_POSE, blend.id, "Poses 0");
                graph.add_node_parameter_edge(leader.id, "Pose", blend.id, "Poses 1");
                graph.add_node_parameter_edge(blend.id, "Pose", root.id, "Source");
                graph.add_output_data("Pose".into(), DataSpec::Pose);
                graph.add_output_data_edge(root.id, "Pose", "Pose");
                for node in [player.clone(), leader.clone(), blend, root] { graph.add_node(node); }
                let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
                world.run_system_cached_with(query, (handle, player.id, leader.id, external_start, full_history, inertial, excluded)).unwrap();
            }
        }
    }

    #[test]
    fn repeated_marker_steps_reach_sequence_and_blendspace_through_sync_scope() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::{AnimationGraph, GraphInputPin, NodeId, TimeUpdate},
            animation_node::AnimationNode,
            context::{graph_context::QueryOutputTime, node_states::StateKey, system_resources::SystemResources},
            pose::Pose, skeleton::Skeleton, state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        use crate::extensions::{SourceMultiWayBlendNode, SourceSequencePlayerNode};
        use crate::extensions::normalized_blendspace::{NormalizedBlendSpaceNode, NormalizedBlendSpacePoint, NormalizedBlendSpaceState};
        use crate::extensions::source_sequence_player::{SourceSequenceSynchronization, SourceSequencePlayerState};
        use crate::extensions::sync_group_phase::{SourceSyncGroupRole, SourceSyncPlayerBinding, SyncMarker};

        fn query(In((handle, leader, follower, blendspace, adjacent, blendspace_leads, reverse, turnaround)): In<(Handle<AnimationGraph>, NodeId, NodeId, bool, bool, bool, bool, bool)>, resources: SystemResources) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let frames = if turnaround && reverse { vec![(0.0, -1.0, 0.75), (0.125, -1.0, 0.625), (0.0625, 1.0, 0.6875), (0.0625, -1.0, 0.625), (0.0625, -1.0, 0.5625)] }
                else if turnaround { vec![(0.0, 1.0, 0.25), (0.125, 1.0, 0.375), (0.0625, -1.0, 0.3125), (0.0625, 1.0, 0.375), (0.0625, 1.0, 0.4375)] }
                else if reverse { vec![(0.0, -1.0, 0.5), (0.125, -1.0, 0.375), (0.0625, -1.0, 0.3125)] }
                else if adjacent { vec![(0.0, 5.0, 0.875), (0.0625, 5.0, 0.1875), (0.015625, 1.0, 0.203125), (0.015625, 1.0, 0.21875), (0.015625, 1.0, 0.234375), (0.015625, 1.0, 0.25)] }
                else { vec![(0.0, 3.0, 0.15), (0.2, 3.0, 0.75), (0.2, 13.0, 0.35), (0.2, -13.0, 0.75)] };
            for (delta, rate, expected) in frames {
                let mut inputs = HashMap::from([("delta".to_owned(), DataValue::F32(delta)), ("rate".to_owned(), DataValue::F32(rate))]);
                if adjacent && !reverse && expected == 0.234375 {
                    let follower = if blendspace_leads { leader } else { follower };
                    inputs.insert(crate::extensions::sync_group_phase::source_sync_marker_reset_key((arena.get_toplevel_id(), follower)), DataValue::Bool(true));
                }
                let context = bevy_animation_graph::core::context::new_context::GraphContext::new(
                    arena.get_toplevel_id(), graph, &mut arena, &resources, &env,
                    Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(delta)), context).unwrap();
                let states = &arena.get_toplevel().node_states;
                let sequence_time = |node| {
                    let state = states.get::<SourceSequencePlayerState>(node, StateKey::Default).unwrap();
                    let time = state.context.relevant_asset.as_ref().unwrap().position;
                    assert!((state.context.curves[&crate::extensions::SourceCurveName::from("SampleTime")] - time).abs() < 1e-5);
                    time
                };
                let leader_time = sequence_time(leader);
                let follower_time = if blendspace {
                    let state = states.get::<NormalizedBlendSpaceState>(follower, StateKey::Default).unwrap();
                    let time = state.context.relevant_asset.as_ref().unwrap().position;
                    assert!((state.context.curves[&crate::extensions::SourceCurveName::from("SampleTime")] - time).abs() < 1e-5);
                    time
                } else { sequence_time(follower) };
                let selected = crate::extensions::sync_group_phase::source_sync_leader(&arena, &resources.animation_graph_assets, "Movement").unwrap();
                assert_eq!(selected.identity.1, if blendspace_leads { follower } else { leader });
                if turnaround && (expected == 0.3125 || expected == 0.6875) { assert!(selected.marker_advance.as_ref().unwrap().markers.is_empty()); }
                assert!((selected.phase - expected).abs() < 1e-5);
                let (leader_time, follower_time) = if blendspace_leads { (follower_time, leader_time) } else { (leader_time, follower_time) };
                let expected_follower = if adjacent && reverse && delta == 0.0625 { 0.125 }
                    else if adjacent && delta > 0.0 && !reverse {
                        if expected < 0.234375 { 0.125 + 4.0 * (expected - 0.125) }
                        else { 0.375 + 3.0 * (expected - 0.125) }
                    } else { expected };
                for (actual, expected) in [(leader_time, expected), (follower_time, expected_follower)] {
                    assert!((actual - expected).abs() < 1e-5, "blendspace={blendspace} leads={blendspace_leads} adjacent={adjacent} delta={delta} rate={rate}: {actual} != {expected}");
                }
            }
        }

        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let clip = world.resource_mut::<Assets<GraphClip>>().add(GraphClip { duration: 1.0, ..default() });
        let clock_curve = crate::extensions::parse_curves(&serde_json::json!({"curves": [{
            "name": "SampleTime", "times": [0.0, 1.0], "values": [0.0, 1.0], "interps": ["LINEAR", "LINEAR"]
        }]})).unwrap();
        for (adjacent, filtered, reverse, turnaround) in [(false, false, false, false), (true, false, false, false), (true, true, false, false), (true, false, true, false), (false, false, false, true), (false, false, true, true), (false, true, false, true), (false, true, true, true)] {
            let markers = if turnaround { vec![("A", 0.125), ("B", 0.375), ("A", 0.625), ("B", 0.875)] } else if adjacent { vec![("A", 0.125), ("A", 0.375), ("B", 0.75)] }
                else { vec![("A", 0.1), ("B", 0.3), ("A", 0.6), ("B", 0.8)] };
            let markers = markers.into_iter().map(|(name, time)| SyncMarker { name: name.into(), time }).collect::<Vec<_>>();
            let mut sequence_markers = markers.clone();
            if filtered { sequence_markers.insert(1, SyncMarker { name: "Extra".into(), time: 0.25 }); }
            let start = if turnaround { if reverse { 0.75 } else { 0.25 } } else if reverse { 0.5 } else if adjacent { 0.875 } else { 0.15 };
            for (blendspace, blendspace_leads) in [(false, false), (true, false), (true, true)] {
                let sync = |role| SourceSequenceSynchronization { key: "Movement".into(), delta_seconds_key: "delta".into(),
                    phase_duration: 1.0, phase_markers: markers.clone(), markers: sequence_markers.clone(), role, ..default() };
                let leader = AnimationNode::new("leader", SourceSequencePlayerNode { clip: clip.clone(), curves: clock_curve.clone(), start_position: start,
                    play_rate_key: "rate".into(), synchronization: Some(sync(if blendspace_leads { SourceSyncGroupRole::AlwaysFollower } else { SourceSyncGroupRole::AlwaysLeader })), ..default() });
                let follower = if blendspace {
                    AnimationNode::new("follower", NormalizedBlendSpaceNode { looped: true, phase_duration: 1.0, phase_markers: markers.clone(),
                        points: vec![NormalizedBlendSpacePoint { id: "sample".into(), duration: 1.0, markers: markers.clone(), curves: clock_curve.clone(), ..default() }],
                        playback: Some(SourceSyncPlayerBinding { key: "Movement".into(), delta_seconds_key: "delta".into(),
                            start_position: start, play_rate_key: "rate".into(), role: if blendspace_leads { SourceSyncGroupRole::AlwaysLeader } else { SourceSyncGroupRole::AlwaysFollower }, ..default() }), ..default() })
                } else {
                    AnimationNode::new("follower", SourceSequencePlayerNode { clip: clip.clone(), curves: clock_curve.clone(), start_position: start,
                        synchronization: Some(SourceSequenceSynchronization { markers: markers.clone(), ..sync(SourceSyncGroupRole::AlwaysFollower) }), ..default() })
                };
                let blend = AnimationNode::new("blend", SourceMultiWayBlendNode { pose_count: 2,
                    desired_alpha_keys: vec![String::new(); 2], desired_alpha_defaults: vec![0.5, 0.5], ..default() });
                let root = AnimationNode::new("sync", SourcePosePassthroughNode { synchronize_groups: true, output_time: false, ..default() });
                let mut graph = AnimationGraph::new();
                if blendspace {
                    graph.set_default_data(GraphInputPin::passthrough("sample"), DataValue::Pose(Pose::default()));
                    graph.set_default_data(GraphInputPin::passthrough("position"), DataValue::Vec2(Vec2::ZERO));
                    graph.add_input_data_edge(GraphInputPin::passthrough("position"), follower.id, crate::extensions::normalized_blendspace::IN_POSITION);
                    graph.add_input_data_edge(GraphInputPin::passthrough("sample"), follower.id, NormalizedBlendSpaceNode::pose_pin("sample"));
                    graph.add_input_time_edge(GraphInputPin::passthrough("time"), follower.id, NormalizedBlendSpaceNode::time_pin("sample"));
                }
                graph.add_node_parameter_edge(leader.id, "Pose", blend.id, "Poses 0");
                graph.add_node_parameter_edge(follower.id, if blendspace { crate::extensions::normalized_blendspace::OUT_POSE } else { "Pose" }, blend.id, "Poses 1");
                graph.add_node_parameter_edge(blend.id, "Pose", root.id, "Source");
                graph.add_output_data("Pose".into(), DataSpec::Pose);
                graph.add_output_data_edge(root.id, "Pose", "Pose");
                for node in [leader.clone(), follower.clone(), blend, root] { graph.add_node(node); }
                let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
                world.run_system_cached_with(query, (handle, leader.id, follower.id, blendspace, adjacent, blendspace_leads, reverse, turnaround)).unwrap();
            }
        }
    }

    #[test]
    fn compiled_cache_queue_orders_submissions_before_sync_sorting() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::{AnimationGraph, TimeUpdate}, animation_node::AnimationNode,
            context::{graph_context::QueryOutputTime, new_context::GraphContext, node_states::StateKey,
                system_resources::SystemResources},
            skeleton::Skeleton, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        use crate::extensions::{
            SourceCachedPoseSaveNode, SourceCachedPoseUseNode, SourceMultiWayBlendNode, SourceSequencePlayerNode,
            source_sequence_player::SourceSequenceSynchronization,
        };

        #[derive(serde::Deserialize)]
        struct Frame {
            weights: [f32; 3],
            submission: Vec<String>,
            leader: String,
            old_leader: String,
        }
        #[derive(serde::Deserialize)]
        struct Reference {
            ranks: [usize; 2],
            frames: Vec<Frame>,
        }

        let reference: Reference = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d218/order.replay.json"
        )).unwrap();
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let clip = world.resource_mut::<Assets<GraphClip>>().add(GraphClip { duration: 1.0, ..default() });
        let names = ["base", "sprint", "acceleration"];
        let fixtures = [true, false].map(|scheduled| {
            let mut graph = AnimationGraph::new();
            let leaves = names.map(|name| {
                let node = AnimationNode::new(name, SourceSequencePlayerNode {
                    clip: clip.clone(), loop_animation: true,
                    expose_play_rate_pin: false, expose_start_position_pin: false, expose_loop_animation_pin: false,
                    synchronization: Some(SourceSequenceSynchronization {
                        key: "Movement".into(), delta_seconds_key: "delta".into(), ..default()
                    }), ..default()
                });
                let id = node.id;
                graph.add_node(node);
                id
            });
            let saves = ["Move Forward", "Move Forward Base"].into_iter().zip(reference.ranks).map(|(name, rank)| {
                AnimationNode::new(name, SourceCachedPoseSaveNode { submission_order: scheduled.then_some(rank), ..default() })
            }).collect::<Vec<_>>();
            let uses = saves.iter().map(|save| AnimationNode::new("Use cache", SourceCachedPoseUseNode {
                cache_node: save.name.clone()
            })).collect::<Vec<_>>();
            let blend = AnimationNode::new("Blend", SourceMultiWayBlendNode {
                pose_count: 3, desired_alpha_keys: names.map(str::to_owned).to_vec(),
                desired_alpha_defaults: vec![0.0; 3], normalize_alpha: false, ..default()
            });
            let root = AnimationNode::new("Sync", SourcePosePassthroughNode {
                synchronize_groups: true, output_time: false, ..default()
            });
            for node in saves.iter().chain(&uses).chain([&blend, &root]) {
                graph.add_node(node.clone());
            }
            graph.add_node_parameter_edge(leaves[0], "Pose", saves[1].id, "Pose");
            graph.add_node_parameter_edge(uses[1].id, "Pose", blend.id, "Poses 0");
            graph.add_node_parameter_edge(leaves[1], "Pose", blend.id, "Poses 1");
            graph.add_node_parameter_edge(leaves[2], "Pose", blend.id, "Poses 2");
            graph.add_node_parameter_edge(blend.id, "Pose", saves[0].id, "Pose");
            graph.add_node_parameter_edge(uses[0].id, "Pose", root.id, "Source");
            graph.add_output_data("Pose".into(), DataSpec::Pose);
            graph.add_output_data_edge(root.id, "Pose", "Pose");
            (world.resource_mut::<Assets<AnimationGraph>>().add(graph), leaves, root.id, scheduled)
        });
        world.run_system_once(move |resources: SystemResources| {
            let env = TestIoEnv::default();
            let entities = HashMap::new();
            let mut gizmos = DeferredGizmos::default();
            for (handle, leaves, root, scheduled) in &fixtures {
                let graph = resources.animation_graph_assets.get(handle).unwrap();
                let mut arena = GraphContextArena::new(handle.id());
                for frame in &reference.frames {
                    let delta = 1.0 / 60.0;
                    let mut inputs = HashMap::from([("delta".to_owned(), DataValue::F32(delta))]);
                    inputs.extend(names.into_iter().zip(frame.weights).map(|(name, weight)| (name.to_owned(), DataValue::F32(weight))));
                    graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(delta)),
                        GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources,
                            &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs)).unwrap();
                    let context = arena.get_toplevel().node_states.get::<AnimationContextState>(*root, StateKey::Default).unwrap();
                    let name = |id| names[leaves.iter().position(|leaf| *leaf == id).unwrap()];
                    let submission: Vec<_> = context.sync_players.iter().map(|tick| name(tick.identity.1)).collect();
                    if *scheduled {
                        assert_eq!(submission, frame.submission.iter().map(String::as_str).collect::<Vec<_>>());
                    } else {
                        assert_eq!(submission.first(), Some(&"base"));
                    }
                    let leader = context.sync_players.iter().find(|tick| tick.selected_leader).unwrap();
                    assert_eq!(name(leader.identity.1), if *scheduled { frame.leader.as_str() } else { frame.old_leader.as_str() });
                }
            }
        }).unwrap();
    }

    #[test]
    fn sync_exit_keeps_a_small_record_from_two_relevant_parent_blends() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::{AnimationGraph, TimeUpdate}, animation_node::AnimationNode,
            context::{graph_context::QueryOutputTime, new_context::GraphContext, node_states::StateKey,
                system_resources::SystemResources},
            pose::Pose, skeleton::Skeleton, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        use crate::extensions::{GlobalPoseInputNode, SourceSequencePlayerNode, SourceTwoWayBlendNode,
            source_sequence_player::SourceSequenceSynchronization};

        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let clip = world.resource_mut::<Assets<GraphClip>>().add(GraphClip { duration: 1.0, ..default() });
        let [small, base] = ["Sprint", "Base"].map(|name| AnimationNode::new(name, SourceSequencePlayerNode {
            clip: clip.clone(), loop_animation: true, expose_play_rate_pin: false, expose_start_position_pin: false,
            synchronization: Some(SourceSequenceSynchronization {
                key: "Movement".into(), delta_seconds_key: "delta".into(), ..default()
            }), ..default()
        }));
        let empty = AnimationNode::new("Empty", GlobalPoseInputNode { key: "pose".into(), output: "Pose".into() });
        let inner_alpha = 0.009837965;
        let inner = AnimationNode::new("Inner", SourceTwoWayBlendNode {
            expose_alpha_pin: false, alpha_key: "inner".into(), ..default()
        });
        let outer = AnimationNode::new("Outer", SourceTwoWayBlendNode {
            expose_alpha_pin: false, alpha_key: "block".into(), ..default()
        });
        let sync = AnimationNode::new("Sync", SourcePosePassthroughNode { synchronize_groups: true, output_time: false, ..default() });
        let mut graph = AnimationGraph::new();
        for node in [&small, &base, &empty, &inner, &outer, &sync] {
            graph.add_node(node.clone());
        }
        graph.add_node_parameter_edge(empty.id, "Pose", inner.id, "A");
        graph.add_node_parameter_edge(small.id, "Pose", inner.id, "B");
        graph.add_node_parameter_edge(inner.id, "Pose", outer.id, "A");
        graph.add_node_parameter_edge(base.id, "Pose", outer.id, "B");
        graph.add_node_parameter_edge(outer.id, "Pose", sync.id, "Source");
        graph.add_output_data("Pose".into(), DataSpec::Pose);
        graph.add_output_data_edge(sync.id, "Pose", "Pose");
        let [small_id, sync_id] = [small.id, sync.id];
        let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
        world.run_system_once(move |resources: SystemResources| {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let env = TestIoEnv::default();
            let entities = HashMap::new();
            for block in [0.999, 1.0, 0.25] {
                let delta = 1.0 / 60.0;
                let inputs = HashMap::from([
                    ("block".to_owned(), DataValue::F32(block)),
                    ("inner".to_owned(), DataValue::F32(inner_alpha)),
                    ("delta".to_owned(), DataValue::F32(delta)),
                    ("pose".to_owned(), DataValue::Pose(Pose::default())),
                ]);
                graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(delta)),
                    GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources,
                        &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs)).unwrap();
                let state = arena.get_toplevel();
                let context = state.node_states.get::<AnimationContextState>(sync_id, StateKey::Default).unwrap();
                let expected = (block < 1.0).then_some((1.0 - block) * inner_alpha);
                assert_eq!(state.node_caches.is_updated(small_id, StateKey::Default), expected.is_some());
                assert_eq!(context.sync_players.iter().find(|tick| tick.identity.1 == small_id).map(|tick| tick.weight), expected);
            }
        }).unwrap();
    }

    #[test]
    fn sync_exit_preserves_native_parent_products_and_absolute_cache_requests() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::{AnimationGraph, TimeUpdate}, animation_node::AnimationNode,
            context::{graph_context::QueryOutputTime, new_context::GraphContext, node_states::StateKey,
                system_resources::SystemResources},
            pose::Pose, skeleton::Skeleton, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        use crate::extensions::{
            GlobalPoseInputNode, SourceCachedPoseSaveNode, SourceCachedPoseUseNode, SourceMultiWayBlendNode,
            SourceSequencePlayerNode, SourceTwoWayBlendNode, interruptible_fsm::with_machine_weight,
            source_sequence_player::SourceSequenceSynchronization,
        };

        #[derive(serde::Deserialize)]
        struct Frame {
            name: String,
            root: f32,
            outer: f32,
            inner: f32,
            leaf: f32,
            cached: f32,
        }
        #[derive(serde::Deserialize)]
        struct Reference {
            frames: Vec<Frame>,
        }

        let reference: Reference = serde_json::from_str(include_str!(
            "../../docs/audit/bf144/d221/weights.replay.json"
        )).unwrap();
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let clip = world.resource_mut::<Assets<GraphClip>>().add(GraphClip { duration: 1.0, ..default() });
        let fixtures = [(false, false), (true, false), (false, true)].map(|(cached, multi_way)| {
            let mut graph = AnimationGraph::new();
            let leaf = AnimationNode::new("Sequence", SourceSequencePlayerNode {
                clip: clip.clone(), loop_animation: true,
                expose_play_rate_pin: false, expose_start_position_pin: false,
                synchronization: Some(SourceSequenceSynchronization {
                    key: "Movement".into(), delta_seconds_key: "delta".into(), ..default()
                }), ..default()
            });
            let empty = AnimationNode::new("Empty", GlobalPoseInputNode { key: "pose".into(), output: "Pose".into() });
            let inner = AnimationNode::new("Inner", SourceTwoWayBlendNode {
                expose_alpha_pin: false, alpha_key: "inner".into(), ..default()
            });
            let outer = if multi_way {
                AnimationNode::new("Outer", SourceMultiWayBlendNode {
                    desired_alpha_keys: vec!["details".into(), "outer".into()],
                    desired_alpha_defaults: vec![0.0; 2], normalize_alpha: false, ..default()
                })
            } else {
                AnimationNode::new("Outer", SourceTwoWayBlendNode {
                    expose_alpha_pin: false, alpha_key: "outer".into(), ..default()
                })
            };
            let sync = AnimationNode::new("Sync", SourcePosePassthroughNode { synchronize_groups: true, output_time: false, ..default() });
            for node in [&leaf, &empty, &inner, &outer, &sync] {
                graph.add_node(node.clone());
            }
            let cache_use = cached.then(|| {
                let save = AnimationNode::new("Cache", SourceCachedPoseSaveNode::default());
                let use_cache = AnimationNode::new("Use", SourceCachedPoseUseNode { cache_node: save.name.clone() });
                graph.add_node(save.clone());
                graph.add_node(use_cache.clone());
                graph.add_node_parameter_edge(leaf.id, "Pose", save.id, "Pose");
                use_cache.id
            });
            graph.add_node_parameter_edge(empty.id, "Pose", inner.id, "A");
            graph.add_node_parameter_edge(cache_use.unwrap_or(leaf.id), "Pose", inner.id, "B");
            let pins = if multi_way { ["Poses 0", "Poses 1"] } else { ["A", "B"] };
            graph.add_node_parameter_edge(inner.id, "Pose", outer.id, pins[0]);
            graph.add_node_parameter_edge(cache_use.unwrap_or(empty.id), "Pose", outer.id, pins[1]);
            graph.add_node_parameter_edge(outer.id, "Pose", sync.id, "Source");
            graph.add_output_data("Pose".into(), DataSpec::Pose);
            graph.add_output_data_edge(sync.id, "Pose", "Pose");
            (world.resource_mut::<Assets<AnimationGraph>>().add(graph), sync.id, cached)
        });
        world.run_system_once(move |resources: SystemResources| {
            let env = TestIoEnv::default();
            let entities = HashMap::new();
            let mut gizmos = DeferredGizmos::default();
            for (handle, sync_id, cached) in &fixtures {
                let graph = resources.animation_graph_assets.get(handle).unwrap();
                for frame in &reference.frames {
                    let mut arena = GraphContextArena::new(handle.id());
                    let delta = 1.0 / 60.0;
                    let inputs = HashMap::from([
                        ("outer".to_owned(), DataValue::F32(frame.outer)),
                        ("details".to_owned(), DataValue::F32(1.0 - frame.outer)),
                        ("inner".to_owned(), DataValue::F32(frame.inner)),
                        ("delta".to_owned(), DataValue::F32(delta)),
                        ("pose".to_owned(), DataValue::Pose(Pose::default())),
                    ]);
                    with_machine_weight(frame.root, || graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(delta)),
                        GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources,
                            &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs))).unwrap();
                    let context = arena.get_toplevel().node_states.get::<AnimationContextState>(*sync_id, StateKey::Default).unwrap();
                    assert_eq!(context.sync_players.len(), 1, "{}", frame.name);
                    let tick = &context.sync_players[0];
                    assert!(tick.weight_is_absolute, "{}", frame.name);
                    assert_eq!(tick.weight, if *cached { frame.cached } else { frame.leaf }, "{}", frame.name);
                }
            }
        }).unwrap();
    }

    #[test]
    fn sync_scope_uses_current_weights_cache_maximum_and_marker_continuity() {
        use crate::extensions::sync_group_phase::SourceSyncPlayerBinding;
        use crate::extensions::{
            NormalizedBlendSpaceNode, NormalizedBlendSpacePoint, SourceCachedPoseSaveNode,
            SourceCachedPoseUseNode, SourceMultiWayBlendNode, SyncMarker,
        };
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip,
            animation_graph::{AnimationGraph, GraphInputPin, NodeId, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                graph_context::QueryOutputTime, new_context::GraphContext,
                node_states::StateKey,
                system_resources::SystemResources,
            },
            pose::Pose,
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        fn query(
            In((handle, shifted)): In<(Handle<AnimationGraph>, bool)>,
            resources: SystemResources,
        ) -> Vec<AnimationContextState> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut result = Vec::new();
            for (weights, player_delta, global_delta) in [
                ([0.5, 0.5, 0.0], 0.1, 0.1),
                ([0.3, 0.3, 0.4], 0.1, 0.1),
                ([0.0, 0.0, 1.0], 0.1, 0.1),
                ([0.5, 0.5, 0.0], 0.1, 0.1),
                ([0.5, 0.5, 0.0], 0.0, 0.1),
                ([0.5, 0.5, 0.0], 0.1, 0.05),
            ] {
                let weights = if shifted && result.len() == 1 { [0.6, 0.6, 0.2] } else { weights };
                let inputs = HashMap::from([
                    ("delta".to_owned(), DataValue::F32(global_delta)),
                    ("rate".to_owned(), DataValue::F32(1.0)),
                    ("position".to_owned(), DataValue::Vec2(Vec2::ZERO)),
                    ("sample".to_owned(), DataValue::Pose(Pose::default())),
                    ("a".to_owned(), DataValue::F32(weights[0])),
                    ("b".to_owned(), DataValue::F32(weights[1])),
                    ("c".to_owned(), DataValue::F32(weights[2])),
                    ("group".to_owned(), DataValue::F32(0.99)),
                ]);
                let context = GraphContext::new(
                    arena.get_toplevel_id(),
                    graph,
                    &mut arena,
                    &resources,
                    &env,
                    Entity::PLACEHOLDER,
                    &entities,
                    &mut gizmos,
                    &inputs,
                );
                graph
                    .query_with_context(
                        QueryOutputTime::Forced(TimeUpdate::Delta(player_delta)),
                        context.clone(),
                    )
                    .unwrap();
                if result.is_empty() {
                    let mut child = context.create_node_context(NodeId::default())
                        .create_child_context(handle.id(), graph, None);
                    child.context_mut().query_output_time = QueryOutputTime::Forced(TimeUpdate::Delta(0.0));
                    let nested = child.create_node_context(NodeId::default());
                    assert_eq!(crate::extensions::sync_group_phase::source_sync_delta_seconds(&nested, "delta").unwrap(), 0.1);
                }
                let observed = crate::extensions::sync_group_phase::source_sync_phase(
                    &arena,
                    &resources.animation_graph_assets,
                    "group",
                );
                let synchronized = graph.nodes.values().any(|node| {
                    node.try_inner_downcast_ref::<SourcePosePassthroughNode>()
                        .is_some_and(|node| node.synchronize_groups)
                });
                assert_eq!(observed.is_some(), synchronized);
                let observed_players = crate::extensions::sync_group_phase::source_sync_players(
                    &arena,
                    &resources.animation_graph_assets,
                    "group",
                );
                assert_eq!(observed_players.is_some(), synchronized);
                if let Some(players) = observed_players {
                    assert!(
                        players
                            .iter()
                            .all(|player| player.phase == observed.unwrap())
                    );
                    let output = crate::extensions::animation_context::graph_output_context_state(
                        graph, &context, "Pose",
                    )
                    .unwrap();
                    assert_eq!(
                        players
                            .iter()
                            .map(|player| (player.identity, player.weight))
                            .collect::<Vec<_>>(),
                        output
                            .sync_players
                            .iter()
                            .map(|player| (player.identity, player.weight))
                            .collect::<Vec<_>>()
                    );
                }
                if result.len() == 1 {
                    let blend = graph.nodes.values().find(|node| node.try_inner_downcast_ref::<NormalizedBlendSpaceNode>().is_some()).unwrap();
                    let state = arena.get_toplevel().node_states.get::<crate::extensions::normalized_blendspace::NormalizedBlendSpaceState>(blend.id, StateKey::Default).unwrap();
                    assert_eq!(state.context.relevant_asset.as_ref().map(|clock| clock.duration), Some(1.0));
                    if shifted {
                        let sequence = graph.nodes.values().find(|node| node.try_inner_downcast_ref::<crate::extensions::SourceSequencePlayerNode>().is_some()).unwrap();
                        let sequence_state = arena.get_toplevel().node_states.get::<crate::extensions::source_sequence_player::SourceSequencePlayerState>(sequence.id, StateKey::Default).unwrap();
                        assert!((observed.unwrap() - 0.3076923).abs() < 1e-5, "group phase={observed:?}");
                        assert!((state.context.relevant_asset.as_ref().unwrap().position - 0.4).abs() < 1e-5);
                        assert!((sequence_state.context.relevant_asset.as_ref().unwrap().position - 0.6153846).abs() < 1e-5);
                    }
                }
                result.push(
                    crate::extensions::animation_context::graph_output_context_state(graph, &context, "Pose")
                        .unwrap(),
                );
            }
            result
        }
        let mut world = World::new();
        let mut graphs = Assets::<AnimationGraph>::default();
        let mut clips = Assets::<GraphClip>::default();
        let clip = clips.add(GraphClip {
            duration: 2.0,
            ..default()
        });
        let mut handles = Vec::new();
        for (synchronize_groups, shifted) in [(true, false), (false, false), (true, true)] {
            let mut graph = AnimationGraph::new();
            graph.set_default_data(
                GraphInputPin::Passthrough("position".into()),
                DataValue::Vec2(Vec2::ZERO),
            );
            graph.set_default_data(
                GraphInputPin::Passthrough("sample".into()),
                DataValue::Pose(Pose::default()),
            );
            let mut players = Vec::new();
            for (index, (duration, start)) in [(1.0, 0.2), (2.0, 0.7)].into_iter().enumerate() {
                let node = if index == 1 {
                    AnimationNode::new(
                        "sequence",
                        crate::extensions::SourceSequencePlayerNode {
                            clip: clip.clone(),
                            start_position: start * duration,
                            play_rate: 2.0,
                            play_rate_basis: 2.0,
                            expose_play_rate_pin: false,
                            synchronization: Some(
                                crate::extensions::source_sequence_player::SourceSequenceSynchronization {
                                    key: "group".into(),
                                    delta_seconds_key: "delta".into(),
                                    phase_duration: 1.0,
                                    phase_markers: vec![
                                        SyncMarker {
                                            name: "Left".into(),
                                            time: 0.0,
                                        },
                                        SyncMarker {
                                            name: "Right".into(),
                                            time: 0.5,
                                        },
                                    ],
                                    markers: vec![
                                        SyncMarker {
                                            name: "Left".into(),
                                            time: 0.0,
                                        },
                                        SyncMarker {
                                            name: "Right".into(),
                                            time: 1.0,
                                        },
                                    ],
                                    ..default()
                                },
                            ),
                            ..default()
                        },
                    )
                } else {
                    AnimationNode::new(
                        "player",
                        NormalizedBlendSpaceNode {
                            per_bone_interpolation: vec![],
                            per_bone_interpolation_ease_in_out: false,
                            looped: true,
                            teleport_to_normalized_time: false,
                            phase_duration: 1.0,
                            phase_markers: vec![
                                SyncMarker {
                                    name: "Left".into(),
                                    time: 0.0,
                                },
                                SyncMarker {
                                    name: "Right".into(),
                                    time: 0.5,
                                },
                            ],
                            points: vec![NormalizedBlendSpacePoint {
                                id: "sample".into(),
                                point: Vec2::ZERO,
                                duration,
                                markers: {
                                    let mut markers = vec![
                                        SyncMarker { name: "Left".into(), time: 0.0 },
                                        SyncMarker { name: "Right".into(), time: duration * if shifted { 0.65 } else { 0.5 } },
                                    ];
                                    if shifted {
                                        markers.insert(1, SyncMarker { name: "Extra".into(), time: duration * 0.25 });
                                    }
                                    markers
                                },
                                ..default()
                            }],
                            playback: Some(SourceSyncPlayerBinding {
                                key: "group".into(),
                                start_position: start,
                                play_rate_key: String::new(),
                                delta_seconds_key: "delta".into(),
                                ..default()
                            }),
                            source_triangulation: None,
                        },
                    )
                };
                graph.add_node(node.clone());
                graph.add_input_data_edge(
                    GraphInputPin::passthrough("position"),
                    node.id,
                    "position",
                );
                graph.add_input_data_edge(
                    GraphInputPin::passthrough("sample"),
                    node.id,
                    "pose_sample",
                );
                players.push(node);
            }
            let cache = AnimationNode::new("clock cache", SourceCachedPoseSaveNode::default());
            let first = AnimationNode::new(
                "first cache use",
                SourceCachedPoseUseNode {
                    cache_node: "clock cache".into(),
                },
            );
            let second = AnimationNode::new(
                "second cache use",
                SourceCachedPoseUseNode {
                    cache_node: "clock cache".into(),
                },
            );
            let blend = AnimationNode::new(
                "blend",
                SourceMultiWayBlendNode {
                    pose_count: 3,
                    desired_alpha_keys: vec!["a".into(), "b".into(), "c".into()],
                    desired_alpha_defaults: vec![0.0; 3],
                    ..default()
                },
            );
            let root = AnimationNode::new(
                "root",
                SourcePosePassthroughNode {
                    output_time: false,
                    synchronize_groups,
                    ..default()
                },
            );
            for node in [&cache, &first, &second, &blend, &root] {
                graph.add_node(node.clone());
            }
            graph.add_node_parameter_edge(players[0].id, "pose", cache.id, "Pose");
            graph.add_node_parameter_edge(first.id, "Pose", blend.id, "Poses 0");
            graph.add_node_parameter_edge(second.id, "Pose", blend.id, "Poses 1");
            graph.add_node_parameter_edge(players[1].id, "Pose", blend.id, "Poses 2");
            graph.add_node_parameter_edge(blend.id, "Pose", root.id, "Source");
            graph.add_output_data("Pose".into(), DataSpec::Pose);
            graph.add_output_data_edge(root.id, "Pose", "Pose");
            handles.push(graphs.add(graph));
        }
        world.insert_resource(graphs);
        world.insert_resource(clips);
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let actual = world
            .run_system_cached_with(query, (handles[0].clone(), false))
            .unwrap();
        for (frame, expected) in [0.3, 0.35, 0.4, 0.5, 0.5, 0.55].into_iter().enumerate() {
            for player in &actual[frame].sync_players {
                assert!(
                    (player.phase - expected).abs() < 1e-5,
                    "frame {frame}: {} != {expected}",
                    player.phase
                );
            }
        }
        assert_eq!(actual[1].sync_players.len(), 2);
        assert!(actual[1].sync_players.iter().all(|player| player.markers.len() == 2));
        assert!((actual[1].sync_players[0].weight - 0.3).abs() < 1e-5);
        assert!(actual[4].sync_players.iter().all(|player| player.delta_seconds == 0.0));
        assert!(actual[5].sync_players.iter().all(|player| (player.delta_seconds - 0.05).abs() < 1e-5));
        let ablated = world
            .run_system_cached_with(query, (handles[1].clone(), false))
            .unwrap();
        assert!((ablated[1].sync_players[1].phase - 0.35).abs() > 0.3);
        world.run_system_cached_with(query, (handles[2].clone(), true)).unwrap();
    }

    #[test]
    fn losing_exclusive_leader_keeps_its_own_phase() {
        use crate::extensions::event_lifecycle::{EventClock, EventDispatchContext, EventLifecycle, PlaybackDirection,
            StateIntervalBoundary, TimedEvent, TimedEventKind};
        use crate::extensions::sync_group_phase::SourceSyncGroupRole;
        use crate::extensions::{SourceMultiWayBlendNode, SourceSequencePlayerNode};
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip,
            animation_graph::{AnimationGraph, TimeUpdate},
            animation_node::AnimationNode,
            context::{graph_context::QueryOutputTime, system_resources::SystemResources},
            edge_data::events::EventQueue,
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };

        fn query(
            In(handle): In<Handle<AnimationGraph>>,
            resources: SystemResources,
        ) -> Vec<(AnimationContextState, f32)> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let inputs = HashMap::from([
                ("delta".to_owned(), DataValue::F32(0.1)),
                ("a".to_owned(), DataValue::F32(0.3)),
                ("b".to_owned(), DataValue::F32(0.3)),
                ("c".to_owned(), DataValue::F32(0.4)),
            ]);
            let mut results = Vec::new();
            for _ in 0..2 {
                let context = bevy_animation_graph::core::context::new_context::GraphContext::new(
                    arena.get_toplevel_id(),
                    graph,
                    &mut arena,
                    &resources,
                    &env,
                    Entity::PLACEHOLDER,
                    &entities,
                    &mut gizmos,
                    &inputs,
                );
                graph
                    .query_with_context(
                        QueryOutputTime::Forced(TimeUpdate::Delta(0.1)),
                        context.clone(),
                    )
                    .unwrap();
                let state =
                    crate::extensions::animation_context::graph_output_context_state(graph, &context, "Pose")
                        .unwrap();
                let group_phase = crate::extensions::sync_group_phase::source_sync_phase(
                    &arena,
                    &resources.animation_graph_assets,
                    "Movement",
                )
                .unwrap();
                results.push((state, group_phase));
            }
            results
        }

        let mut world = World::new();
        let mut graphs = Assets::<AnimationGraph>::default();
        let mut clips = Assets::<GraphClip>::default();
        let clip = clips.add(GraphClip {
            duration: 1.0,
            ..default()
        });
        let mut graph = AnimationGraph::new();
        let mut players = Vec::new();
        for (start, role) in [
            (0.2, SourceSyncGroupRole::ExclusiveAlwaysLeader),
            (0.7, SourceSyncGroupRole::ExclusiveAlwaysLeader),
            (0.4, SourceSyncGroupRole::AlwaysFollower),
        ] {
            let node = AnimationNode::new(
                "player",
                SourceSequencePlayerNode {
                    clip: clip.clone(),
                    start_position: start,
                    synchronization: Some(
                        crate::extensions::source_sequence_player::SourceSequenceSynchronization {
                            key: "Movement".into(),
                            role,
                            delta_seconds_key: "delta".into(),
                            phase_duration: 1.0,
                            ..default()
                        },
                    ),
                    ..default()
                },
            );
            graph.add_node(node.clone());
            players.push(node);
        }
        let blend = AnimationNode::new(
            "blend",
            SourceMultiWayBlendNode {
                pose_count: 3,
                desired_alpha_keys: vec!["a".into(), "b".into(), "c".into()],
                desired_alpha_defaults: vec![0.0; 3],
                ..default()
            },
        );
        let root = AnimationNode::new(
            "root",
            SourcePosePassthroughNode {
                output_time: false,
                synchronize_groups: true,
                ..default()
            },
        );
        graph.add_node(blend.clone());
        graph.add_node(root.clone());
        for (index, player) in players.iter().enumerate() {
            graph.add_node_parameter_edge(player.id, "Pose", blend.id, format!("Poses {index}"));
        }
        graph.add_node_parameter_edge(blend.id, "Pose", root.id, "Source");
        graph.add_output_data("Pose".into(), DataSpec::Pose);
        graph.add_output_data_edge(root.id, "Pose", "Pose");
        let handle = graphs.add(graph);
        world.insert_resource(graphs);
        world.insert_resource(clips);
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let result = world.run_system_cached_with(query, handle).unwrap();
        for (frame, (state, group_phase)) in result.iter().enumerate() {
            let phase = |index: usize| {
                state
                    .sync_players
                    .iter()
                    .find(|player| player.identity.1 == players[index].id)
                    .unwrap()
                    .phase
            };
            let expected = [0.3 + frame as f32 * 0.1, 0.8 + frame as f32 * 0.1];
            assert!(
                (phase(0) - expected[0]).abs() < 1e-5,
                "frame {frame} loser phase {}",
                phase(0)
            );
            for index in [1, 2] {
                assert!(
                    (phase(index) - expected[1]).abs() < 1e-5,
                    "frame {frame} player {index} phase {}",
                    phase(index)
                );
            }
            assert!(
                (group_phase - expected[1]).abs() < 1e-5,
                "frame {frame} group phase {group_phase}"
            );
            let sample = |index: usize| state.asset_samples.iter()
                .find(|sample| sample.identity.ends_with(&format!("{:?}", players[index].id)))
                .unwrap();
            assert!(sample(0).sync_group.is_empty() && sample(0).sync_identity.is_none(),
                "frame {frame} losing exclusive still participates in group notifications");
            assert_eq!(sample(1).sync_group, "Movement");
            assert_eq!(sample(2).sync_group, "Movement");
            if frame == 0 {
                let definitions = [0.25_f32, 0.75].into_iter().map(|time| TimedEvent {
                    id: format!("marker_{time}"), kind: TimedEventKind::Point,
                    start_time: time, end_time: time, weight_threshold: 0.2,
                    track: None, state_boundary: StateIntervalBoundary::StartExclusiveEndInclusive,
                    trigger_on_follower: false, trigger_on_dedicated_server: true,
                }).collect::<Vec<_>>();
                let mut signals = Vec::new();
                for sample in &state.asset_samples {
                    let is_leader = sample.sync_group.is_empty()
                        || sample.sync_identity.is_some_and(|identity| identity.1 == players[1].id);
                    signals.extend(EventLifecycle::default().update_with_context(
                        &definitions, &EventQueue::default(), Some(EventClock {
                            previous_time: sample.clock.previous_position,
                            current_time: sample.clock.position,
                            duration: sample.clock.duration,
                            direction: PlaybackDirection::Forward,
                            wrapped: false,
                        }), sample.clock.weight, EventDispatchContext { is_leader, is_dedicated_server: false },
                    ).into_iter().map(|signal| signal.id));
                }
                signals.sort();
                assert_eq!(signals, ["marker_0.25", "marker_0.75"]);
            }
        }
    }

    #[test]
    fn transition_player_remembers_full_weight_until_reinitialized() {
        use crate::extensions::sync_group_phase::{SourceSyncGroupRole, SyncMarker};
        use crate::extensions::{SourceMultiWayBlendNode, SourceSequencePlayerNode};
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip,
            animation_graph::{AnimationGraph, NodeId, TimeUpdate},
            animation_node::AnimationNode,
            context::{graph_context::{GraphState, QueryOutputTime}, system_resources::SystemResources},
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };

        fn query(
            In((handle, transition_id)): In<(Handle<AnimationGraph>, NodeId)>,
            resources: SystemResources,
        ) -> Vec<(AnimationContextState, Option<f32>)> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut results = Vec::new();
            for (frame, (transition, normal)) in
                [(0.4, 0.6), (1.0, 0.0), (0.4, 0.2), (0.4, 0.2), (0.4, 0.2), (0.4, 0.6)].into_iter().enumerate()
            {
                if frame == 5 {
                    *arena.get_toplevel_mut() = GraphState::new(handle.id());
                }
                let inputs = HashMap::from([
                    ("delta".to_owned(), DataValue::F32(0.1)),
                    ("transition".to_owned(), DataValue::F32(transition)),
                    ("normal".to_owned(), DataValue::F32(normal)),
                    (crate::extensions::sync_group_phase::SOURCE_SYNC_INERTIAL_SCOPE.to_owned(), DataValue::Bool(frame == 3)),
                ]);
                let context = bevy_animation_graph::core::context::new_context::GraphContext::new(
                    arena.get_toplevel_id(), graph, &mut arena, &resources, &env,
                    Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs,
                );
                graph.query_with_context(
                    QueryOutputTime::Forced(TimeUpdate::Delta(0.1)), context.clone(),
                ).unwrap();
                let state = crate::extensions::animation_context::graph_output_context_state(
                    graph, &context, "Pose",
                ).unwrap();
                let phase = crate::extensions::sync_group_phase::source_sync_phase(
                    &arena, &resources.animation_graph_assets, "Movement",
                );
                let player = state.sync_players.iter().find(|player| player.identity.1 == transition_id).unwrap();
                let grouped = state.asset_samples.iter().find(|sample| sample.sync_identity == Some(player.identity));
                assert_eq!(player.has_been_full_weight, (1..5).contains(&frame));
                assert_eq!(grouped.is_some(), (1..5).contains(&frame) && frame != 3);
                assert_eq!(player.markers.len(), if frame == 2 || frame == 4 { 2 } else { 3 }, "frame {frame}");
                results.push((state, phase));
            }
            results
        }

        let mut world = World::new();
        let mut graphs = Assets::<AnimationGraph>::default();
        let mut clips = Assets::<GraphClip>::default();
        let clip = clips.add(GraphClip { duration: 1.0, ..default() });
        let mut graph = AnimationGraph::new();
        let mut players = Vec::new();
        for (start, role) in [
            (0.8, SourceSyncGroupRole::TransitionLeader),
            (0.2, SourceSyncGroupRole::CanBeLeader),
        ] {
            let phase_markers = vec![
                SyncMarker { name: "A".into(), time: 0.0 },
                SyncMarker { name: "B".into(), time: 0.5 },
            ];
            let mut markers = phase_markers.clone();
            if role == SourceSyncGroupRole::TransitionLeader {
                markers.insert(1, SyncMarker { name: "Extra".into(), time: 0.25 });
            }
            let node = AnimationNode::new("player", SourceSequencePlayerNode {
                clip: clip.clone(), start_position: start,
                synchronization: Some(crate::extensions::source_sequence_player::SourceSequenceSynchronization {
                    key: "Movement".into(), role, delta_seconds_key: "delta".into(),
                    phase_duration: 1.0, phase_markers, markers, ..default()
                }),
                ..default()
            });
            graph.add_node(node.clone());
            players.push(node);
        }
        let blend = AnimationNode::new("blend", SourceMultiWayBlendNode {
            pose_count: 2,
            desired_alpha_keys: vec!["transition".into(), "normal".into()],
            desired_alpha_defaults: vec![0.0; 2],
            ..default()
        });
        let root = AnimationNode::new("root", SourcePosePassthroughNode {
            output_time: false, synchronize_groups: true, ..default()
        });
        graph.add_node(blend.clone());
        graph.add_node(root.clone());
        for (index, player) in players.iter().enumerate() {
            graph.add_node_parameter_edge(player.id, "Pose", blend.id, format!("Poses {index}"));
        }
        graph.add_node_parameter_edge(blend.id, "Pose", root.id, "Source");
        graph.add_output_data("Pose".into(), DataSpec::Pose);
        graph.add_output_data_edge(root.id, "Pose", "Pose");
        let handle = graphs.add(graph);
        world.insert_resource(graphs);
        world.insert_resource(clips);
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let result = world.run_system_cached_with(query, (handle, players[0].id)).unwrap();
        assert!((result[0].1.unwrap() - 0.3).abs() < 1e-5);
        assert!((result[1].1.unwrap() - 0.4).abs() < 1e-5);
        assert!((result[2].1.unwrap() - 0.5).abs() < 1e-5);
        assert!((result[3].1.unwrap() - 0.6).abs() < 1e-5);
        assert!((result[4].1.unwrap() - 0.7).abs() < 1e-5);
        assert!((result[5].1.unwrap() - 0.3).abs() < 1e-5);
    }

    #[test]
    fn default_pin_names_match_source_pose_nodes() {
        let node = SourcePosePassthroughNode::default();
        assert_eq!(node.input, "Source");
        assert_eq!(node.output, "Pose");
        assert_eq!(node.input_time, "source_time");
        assert!(node.output_time);
        assert!(node.source_function.is_empty());
        use serde::de::DeserializeSeed;
        let mut registry = bevy::reflect::TypeRegistry::default();
        registry.register::<SourcePosePassthroughNode>();
        let mut serialized = ron::de::Deserializer::from_str(r#"{"als_ext::SourcePosePassthroughNode": (input: "Source", output: "Pose", input_time: "source_time", output_time: false, source_function: "", call_site: "")}"#).unwrap();
        let reflected = bevy::reflect::serde::ReflectDeserializer::new(&registry)
            .deserialize(&mut serialized)
            .unwrap();
        let restored = SourcePosePassthroughNode::from_reflect(reflected.as_ref()).unwrap();
        assert!(!restored.synchronize_groups);
    }
}
