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

use crate::animation_context::{AnimationContextState, pose_input_context_state};
use crate::sync_group_phase::{SOURCE_SYNC_RESOLVED, resolve_source_sync_phases};

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
        }
    }
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
            preview_ctx.data_back(self.input.clone())?;
            let current = pose_input_context_state(&preview_ctx, &self.input).unwrap_or_default();
            let previous = ctx
                .state::<AnimationContextState>()
                .cloned()
                .unwrap_or_default();
            for (key, phase) in
                resolve_source_sync_phases(&current.sync_players, &previous.sync_players)
            {
                global_inputs.to_mut().insert(
                    format!("{SOURCE_SYNC_RESOLVED}.{key}"),
                    DataValue::F32(phase),
                );
            }
            global_inputs
                .to_mut()
                .insert(SOURCE_SYNC_RESOLVED.to_owned(), DataValue::Bool(true));
        }
        let mut source_ctx = ctx.clone();
        source_ctx.graph_context.global_input_data = &global_inputs;
        let pose = source_ctx.data_back(self.input.clone())?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(self.output.clone(), pose);
        if let Some(context) = pose_input_context_state(&source_ctx, &self.input) {
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
    fn sync_scope_uses_current_weights_cache_maximum_and_marker_continuity() {
        use crate::sync_group_phase::SourceSyncPlayerBinding;
        use crate::{
            NormalizedBlendSpaceNode, NormalizedBlendSpacePoint, SourceCachedPoseSaveNode,
            SourceCachedPoseUseNode, SourceMultiWayBlendNode, SyncMarker,
        };
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip,
            animation_graph::{AnimationGraph, GraphInputPin, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                graph_context::QueryOutputTime, new_context::GraphContext,
                system_resources::SystemResources,
            },
            pose::Pose,
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        fn query(
            In(handle): In<Handle<AnimationGraph>>,
            resources: SystemResources,
        ) -> Vec<AnimationContextState> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut result = Vec::new();
            for weights in [
                [0.5, 0.5, 0.0],
                [0.3, 0.3, 0.4],
                [0.0, 0.0, 1.0],
                [0.5, 0.5, 0.0],
            ] {
                let inputs = HashMap::from([
                    ("delta".to_owned(), DataValue::F32(0.1)),
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
                        QueryOutputTime::Forced(TimeUpdate::Delta(0.1)),
                        context.clone(),
                    )
                    .unwrap();
                let observed = crate::sync_group_phase::source_sync_phase(
                    &arena,
                    &resources.animation_graph_assets,
                    "group",
                );
                let synchronized = graph.nodes.values().any(|node| {
                    node.try_inner_downcast_ref::<SourcePosePassthroughNode>()
                        .is_some_and(|node| node.synchronize_groups)
                });
                assert_eq!(observed.is_some(), synchronized);
                result.push(
                    crate::animation_context::graph_output_context_state(graph, &context, "Pose")
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
        for synchronize_groups in [true, false] {
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
                        crate::SourceSequencePlayerNode {
                            clip: clip.clone(),
                            start_position: start * duration,
                            play_rate: 2.0,
                            play_rate_basis: 2.0,
                            expose_play_rate_pin: false,
                            synchronization: Some(
                                crate::source_sequence_player::SourceSequenceSynchronization {
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
                            looped: true,
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
                                markers: vec![
                                    SyncMarker {
                                        name: "Left".into(),
                                        time: 0.0,
                                    },
                                    SyncMarker {
                                        name: "Right".into(),
                                        time: duration * 0.5,
                                    },
                                ],
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
            .run_system_cached_with(query, handles[0].clone())
            .unwrap();
        for (frame, expected) in [0.3, 0.35, 0.4, 0.5].into_iter().enumerate() {
            for player in &actual[frame].sync_players {
                assert!(
                    (player.phase - expected).abs() < 1e-5,
                    "frame {frame}: {} != {expected}",
                    player.phase
                );
            }
        }
        assert_eq!(actual[1].sync_players.len(), 2);
        assert!((actual[1].sync_players[0].weight - 0.3).abs() < 1e-5);
        let ablated = world
            .run_system_cached_with(query, handles[1].clone())
            .unwrap();
        assert!((ablated[1].sync_players[1].phase - 0.35).abs() > 0.3);
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
