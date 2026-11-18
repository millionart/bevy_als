use std::sync::Arc;

use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::Pose,
    skeleton::Skeleton,
};

use crate::extensions::animation_context::{
    AnimationContextState, optional_data_back, pose_input_context_state,
};
use crate::extensions::global_f32_input::global_f32_or_root_default;
use crate::extensions::source_pose::SourcePose;
use crate::extensions::source_pose_nodes::{
    accumulate_source_poses, blend_animation_contexts, blend_precise_poses,
    normalize_source_pose_rotations,
};

fn node_error(ctx: &NodeContext, message: String) -> GraphError {
    GraphError::UnknownErrorNode {
        graph: ctx.graph_context.context_id,
        node: ctx.node_id,
        error: message,
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceMultiWayBlendNode {
    pub pose_count: usize,
    pub desired_alpha_keys: Vec<String>,
    pub desired_alpha_defaults: Vec<f32>,
    pub normalize_alpha: bool,
    pub alpha_scale: f32,
    pub reference_skeleton: Handle<Skeleton>,
    pub pose_pin_prefix: String,
    pub alpha_pin_prefix: String,
}

impl Default for SourceMultiWayBlendNode {
    fn default() -> Self {
        Self {
            pose_count: 2,
            desired_alpha_keys: default(),
            desired_alpha_defaults: default(),
            normalize_alpha: true,
            alpha_scale: 1.0,
            reference_skeleton: default(),
            pose_pin_prefix: "Poses ".to_owned(),
            alpha_pin_prefix: "Desired Alphas ".to_owned(),
        }
    }
}

impl SourceMultiWayBlendNode {
    fn pose_input(&self, index: usize) -> String {
        format!("{}{index}", self.pose_pin_prefix)
    }

    fn alpha_input(&self, index: usize) -> String {
        format!("{}{index}", self.alpha_pin_prefix)
    }

    fn alpha(&self, ctx: &NodeContext, index: usize) -> Result<f32, GraphError> {
        let key = self
            .desired_alpha_keys
            .get(index)
            .ok_or_else(|| node_error(ctx, "desired alpha key is missing".to_owned()))?;
        let default = self
            .desired_alpha_defaults
            .get(index)
            .ok_or_else(|| node_error(ctx, "desired alpha default is missing".to_owned()))?;
        if let Some(value) = optional_data_back(ctx, self.alpha_input(index))? {
            return value.as_f32();
        }
        if key.is_empty() {
            Ok(*default)
        } else {
            global_f32_or_root_default(ctx, key)
        }
    }
    pub(crate) fn active_weights(
        &self,
        ctx: &NodeContext,
    ) -> Result<Vec<(usize, f32)>, GraphError> {
        if self.desired_alpha_keys.len() != self.pose_count
            || self.desired_alpha_defaults.len() != self.pose_count
        {
            return Err(node_error(
                ctx,
                "desired alpha configuration does not match pose count".to_owned(),
            ));
        }
        let desired = (0..self.pose_count)
            .map(|index| self.alpha(ctx, index))
            .collect::<Result<Vec<_>, _>>()?;
        let total: f32 = desired.iter().sum();
        let mut active = Vec::new();
        if (total * self.alpha_scale).clamp(0.0, 1.0) > 1e-5 {
            for (index, raw) in desired.into_iter().enumerate() {
                let weight = ((if self.normalize_alpha {
                    raw / total
                } else {
                    raw
                }) * self.alpha_scale)
                    .clamp(0.0, 1.0);
                if weight > 1e-5 {
                    active.push((index, weight));
                }
            }
        }
        Ok(active)
    }
}

impl NodeLike for SourceMultiWayBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let active = self.active_weights(&ctx)?;
        let mut poses = Vec::with_capacity(active.len());
        let mut contexts = Vec::new();
        for (index, weight) in active {
            let pose = crate::extensions::interruptible_fsm::with_machine_weight(weight, || {
                ctx.data_back(self.pose_input(index))
            })?
            .into_pose()?;
            let context =
                pose_input_context_state(&ctx, &self.pose_input(index)).unwrap_or_default();
            contexts.push((context, weight));
            poses.push((pose, weight));
        }
        let (output, context) = if poses.is_empty() {
            let skeleton = ctx
                .graph_context
                .resources
                .skeleton_assets
                .get(&self.reference_skeleton)
                .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
            let timestamp = crate::extensions::context_time_update(&ctx)?
                .partial_update_basic(ctx.prev_time())
                .unwrap_or_else(|| ctx.prev_time());
            let mut pose = Pose {
                skeleton: self.reference_skeleton.clone(),
                timestamp,
                ..default()
            };
            crate::extensions::mesh_space_additive::materialize_missing_reference_bones(
                &mut pose, skeleton,
            );
            let source_pose = SourcePose::from_complete_pose(&pose, skeleton).map(Arc::new);
            (
                pose,
                AnimationContextState {
                    curves_are_authoritative: true,
                    source_pose,
                    ..default()
                },
            )
        } else {
            let passes = 1 + usize::from(poses.len() > 1);
            let precise = blend_precise_poses(
                &poses,
                &contexts,
                poses.iter().map(|(_, weight)| *weight),
                passes,
            );
            let mut context = blend_animation_contexts(contexts);
            let pose = if let Some((pose, source)) = precise {
                context.source_pose = Some(source);
                pose
            } else {
                let mut pose = accumulate_source_poses(&poses);
                normalize_source_pose_rotations(&mut pose, passes);
                pose
            };
            (pose, context)
        };
        ctx.set_time(output.timestamp);
        ctx.set_data_fwd("Pose", output);
        *ctx.state_mut_or_else(AnimationContextState::default)? = context;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        for index in 0..self.pose_count {
            ctx.add_input_data(self.pose_input(index), DataSpec::Pose);
        }
        for index in 0..self.pose_count {
            ctx.add_input_data(self.alpha_input(index), DataSpec::F32);
        }
        ctx.add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Multi Way Blend".to_owned()
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceSetHipsDirectionNode {
    pub new_hips_direction: String,
    pub on_become_relevant: bool,
    pub call_site: String,
}

impl Default for SourceSetHipsDirectionNode {
    fn default() -> Self {
        Self {
            new_hips_direction: "Forward".to_owned(),
            on_become_relevant: true,
            call_site: "OnBecomeRelevant".to_owned(),
        }
    }
}

impl NodeLike for SourceSetHipsDirectionNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let became_relevant =
            crate::extensions::source_instance::source_call_became_relevant(&ctx)?;
        if !self.on_become_relevant || became_relevant {
            let value = if let Some(value) = optional_data_back(&ctx, "NewHipsDirection")? {
                value.as_f32()?
            } else {
                [
                    "Forward",
                    "Backward",
                    "LeftForward",
                    "LeftBackward",
                    "RightForward",
                    "RightBackward",
                ]
                .iter()
                .position(|value| *value == self.new_hips_direction)
                .map(|value| value as f32)
                .ok_or_else(|| node_error(&ctx, "source hips direction is invalid".to_owned()))?
            };
            crate::extensions::source_instance::set_source_property(
                &ctx,
                "grounded_state.hips_direction",
                value,
            )?;
        }
        let pose = ctx.data_back("Source")?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
        if let Some(context) = pose_input_context_state(&ctx, "Source") {
            *ctx.state_mut_or_else(AnimationContextState::default)? = context;
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Source", DataSpec::Pose)
            .add_input_data("NewHipsDirection", DataSpec::F32)
            .add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Set Hips Direction".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_animation_graph::core::pose::BonePose;

    #[test]
    fn multi_way_blend_matches_source_weights_and_skips_irrelevant_children() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::{EntityPath, GraphClip},
            animation_graph::{AnimationGraph, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                node_states::StateKey, system_resources::SystemResources,
            },
            edge_data::DataValue,
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        fn query(
            In((handle, skeleton, bone)): In<(
                Handle<AnimationGraph>,
                Handle<Skeleton>,
                bevy_animation_graph::core::pose::BoneId,
            )>,
            resources: SystemResources,
        ) -> (Pose, [bool; 2], bool) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let mut inputs = HashMap::new();
            for (index, x) in [2.0, 8.0].into_iter().enumerate() {
                let mut pose = Pose {
                    skeleton: skeleton.clone(),
                    timestamp: 1.0,
                    ..default()
                };
                pose.add_bone(
                    BonePose::from_transform(Transform::from_xyz(x, 0.0, 0.0)),
                    bone,
                );
                inputs.insert(format!("pose_{index}"), DataValue::Pose(pose));
            }
            let entities = HashMap::new();
            let env = TestIoEnv::default();
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
            let pose = graph
                .query_with_context(
                    QueryOutputTime::Forced(TimeUpdate::Delta(0.02)),
                    context.clone(),
                )
                .unwrap()
                .remove("Pose")
                .unwrap()
                .into_pose()
                .unwrap();
            let authoritative = crate::extensions::animation_context::graph_output_context_state(
                graph, &context, "Pose",
            )
            .unwrap()
            .curves_are_authoritative;
            let updated = [0, 1].map(|index| {
                let node = graph
                    .nodes
                    .values()
                    .find(|node| node.name == format!("pose_{index}"))
                    .unwrap();
                arena
                    .get_toplevel()
                    .node_caches
                    .is_updated(node.id, StateKey::Default)
            });
            (pose, updated, authoritative)
        }
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let root = EntityPath::default().child("scene");
        let child = root.child("bone");
        let bone = child.id();
        let reference = Transform::from_xyz(4.0, 1.0, 2.0);
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root.id());
        skeleton.add_bone(root, Transform::IDENTITY, Transform::IDENTITY);
        skeleton.add_bone(child, reference, reference);
        let skeleton = world.resource_mut::<Assets<Skeleton>>().add(skeleton);
        let mut failures = Vec::new();
        for (desired, scale, normalize, expected) in [
            ([1.0, 3.0], 1.0, true, [0.25, 0.75]),
            ([1.0, 3.0], 0.5, true, [0.125, 0.375]),
            ([2.0, 0.5], 1.0, false, [1.0, 0.5]),
            ([0.0, 0.0], 1.0, true, [0.0, 0.0]),
            ([1.0, -1.0], 1.0, true, [0.0, 0.0]),
            ([0.000004, 0.000004], 1.0, true, [0.0, 0.0]),
            ([0.000001, 1.0], 1.0, true, [0.0, 1.0 / 1.000001]),
        ] {
            let mut graph = AnimationGraph::new();
            let blend = AnimationNode::new(
                "blend",
                SourceMultiWayBlendNode {
                    desired_alpha_keys: vec![String::new(); 2],
                    desired_alpha_defaults: desired.to_vec(),
                    alpha_scale: scale,
                    normalize_alpha: normalize,
                    reference_skeleton: skeleton.clone(),
                    pose_pin_prefix: "Poses_".into(),
                    ..default()
                },
            );
            graph.add_node(blend.clone());
            graph.add_output_data("Pose".into(), DataSpec::Pose);
            graph.add_output_data_edge(blend.id, "Pose", "Pose");
            for index in 0..2 {
                let key = format!("pose_{index}");
                let input = AnimationNode::new(
                    &key,
                    crate::extensions::GlobalPoseInputNode {
                        key: key.clone(),
                        output: "Pose".into(),
                    },
                );
                graph.add_node(input.clone());
                graph.add_node_parameter_edge(input.id, "Pose", blend.id, format!("Poses_{index}"));
            }
            let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
            let (pose, updated, authoritative) = world
                .run_system_cached_with(query, (handle, skeleton.clone(), bone))
                .unwrap();
            let active = expected.map(|weight| weight > 1e-5);
            let expected_translation = if active.iter().any(|value| *value) {
                Vec3::X * (2.0 * expected[0] + 8.0 * expected[1])
            } else {
                reference.translation
            };
            let expected_scale = if active.iter().any(|value| *value) {
                Vec3::splat(expected.iter().sum())
            } else {
                reference.scale
            };
            let matches = pose.skeleton == skeleton
                && pose.get_bone(bone).is_some_and(|pose| {
                    pose.translation
                        .is_some_and(|value| value.abs_diff_eq(expected_translation, 1e-5))
                        && pose
                            .scale
                            .is_some_and(|value| value.abs_diff_eq(expected_scale, 1e-5))
                });
            if updated != active
                || !matches
                || (!active.iter().any(|value| *value) && !authoritative)
            {
                failures.push(format!("desired={desired:?}, scale={scale}, normalize={normalize}: updated={updated:?}, bone={:?}", pose.get_bone(bone)));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn source_hips_direction_write_is_visible_to_the_child_graph() {
        use crate::extensions::{GlobalF32InputNode, GlobalPoseInputNode, SourceCurveModifyNode};
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip,
            animation_graph::{AnimationGraph, NodeId, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                system_resources::SystemResources,
            },
            edge_data::DataValue,
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        fn query(In(root): In<Handle<AnimationGraph>>, resources: SystemResources) -> Vec<f32> {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let inputs = HashMap::from([
                (
                    "grounded_state.hips_direction".to_owned(),
                    DataValue::F32(0.0),
                ),
                ("fixture".to_owned(), DataValue::Pose(Pose::default())),
            ]);
            let mut results = Vec::new();
            for frame in 0..6 {
                if frame == 2 {
                    arena.next_frame();
                }
                if frame == 3 {
                    arena = GraphContextArena::new(root.id());
                }
                if frame >= 4 {
                    for _ in 0..if frame == 4 {
                        u16::MAX as usize - 1
                    } else {
                        u16::MAX as usize
                    } {
                        arena.next_frame();
                    }
                }
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
                if frame == 1 || frame >= 4 {
                    crate::extensions::source_instance::set_source_property(
                        &context.create_node_context(NodeId::default()),
                        "grounded_state.hips_direction",
                        0.0_f32,
                    )
                    .unwrap();
                }
                graph
                    .query_with_context(
                        QueryOutputTime::Forced(TimeUpdate::Delta(0.1)),
                        context.clone(),
                    )
                    .unwrap();
                results.push(
                    crate::extensions::animation_context::graph_output_context_state(
                        graph, &context, "Pose",
                    )
                    .unwrap()
                    .curves[&crate::extensions::SourceCurveName::from("Probe")],
                );
            }
            results
        }
        let mut graph = AnimationGraph::new();
        let source = AnimationNode::new(
            "source",
            GlobalPoseInputNode {
                key: "fixture".to_owned(),
                output: "Pose".to_owned(),
            },
        );
        let read = AnimationNode::new(
            "read hips",
            GlobalF32InputNode {
                key: "grounded_state.hips_direction".to_owned(),
            },
        );
        let capture = AnimationNode::new(
            "capture",
            SourceCurveModifyNode {
                curve_name: "Probe".to_owned(),
                source_pose_pin: "Input".to_owned(),
                alpha: 1.0,
                ..default()
            },
        );
        let write = AnimationNode::new(
            "set hips",
            SourceSetHipsDirectionNode {
                new_hips_direction: "LeftBackward".to_owned(),
                ..default()
            },
        );
        for node in [&source, &read, &capture, &write] {
            graph.add_node(node.clone());
        }
        graph.add_node_parameter_edge(source.id, "Pose", capture.id, "Input");
        graph.add_node_parameter_edge(
            read.id,
            "grounded_state.hips_direction",
            capture.id,
            "Probe",
        );
        graph.add_node_parameter_edge(capture.id, "Pose", write.id, "Source");
        graph.add_output_data("Pose".to_owned(), DataSpec::Pose);
        graph.add_output_data_edge(write.id, "Pose", "Pose");
        let mut graphs = Assets::<AnimationGraph>::default();
        let graph = graphs.add(graph);
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        assert_eq!(
            world.run_system_cached_with(query, graph).unwrap(),
            [3.0, 0.0, 3.0, 3.0, 0.0, 0.0]
        );
    }

    #[test]
    fn call_counter_wrap_preserves_the_native_plant_foot_pose() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_graph::{AnimationGraph, NodeId, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                system_resources::SystemResources,
            },
            edge_data::DataValue,
            utils::testing::TestIoEnv,
        };
        fn query(
            In((handle, gap, initial, base)): In<(Handle<AnimationGraph>, usize, f32, Pose)>,
            resources: SystemResources,
        ) -> Pose {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let inputs = HashMap::from([
                (
                    "grounded_state.hips_direction".into(),
                    DataValue::F32(initial),
                ),
                (
                    "grounded_state.velocity_blend.forward_amount".into(),
                    DataValue::F32(0.0),
                ),
                (
                    "grounded_state.velocity_blend.backward_amount".into(),
                    DataValue::F32(0.0),
                ),
                (
                    "grounded_state.velocity_blend.left_amount".into(),
                    DataValue::F32(1.0),
                ),
                (
                    "grounded_state.velocity_blend.right_amount".into(),
                    DataValue::F32(0.0),
                ),
                ("movement_details".into(), DataValue::Pose(base)),
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
            let first = graph
                .query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.0)), context)
                .unwrap()
                .remove("Pose")
                .unwrap()
                .into_pose()
                .unwrap();
            if gap == 0 {
                return first;
            }
            for _ in 1..gap {
                arena.next_frame();
            }
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
            crate::extensions::source_instance::set_source_property(
                &context.create_node_context(NodeId::default()),
                "grounded_state.hips_direction",
                0.0_f32,
            )
            .unwrap();
            graph
                .query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.0)), context)
                .unwrap()
                .remove("Pose")
                .unwrap()
                .into_pose()
                .unwrap()
        }
        let (mut app, _, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let source:Handle<AnimationGraph>=app.world().resource::<AssetServer>().load(
            "ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph/SM6_Standing_States/S4_Stop/SM0_Stop_States/S3_Plant_Right_Foot.animgraph.ron");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !app
            .world()
            .resource::<AssetServer>()
            .is_loaded_with_dependencies(source.id())
        {
            app.update();
            assert!(
                std::time::Instant::now() < deadline,
                "Current Plant Foot graph did not load"
            );
        }
        let player = app
            .world()
            .get::<bevy_animation_graph::core::animated_scene::AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let base = app
            .world()
            .get::<bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer>(player)
            .unwrap()
            .get_default_output_pose()
            .unwrap()
            .clone();
        let cache = NodeId::from(
            bevy::asset::uuid::Uuid::parse_str("c39c0018-4c52-cdf4-3681-2cb2c6b0f489").unwrap(),
        );
        let output = NodeId::from(
            bevy::asset::uuid::Uuid::parse_str("d49f14a1-4521-847a-81db-3a888dd8e01e").unwrap(),
        );
        let source = app
            .world()
            .resource::<Assets<AnimationGraph>>()
            .get(&source)
            .unwrap();
        let mut native_leaf = AnimationGraph::new();
        for node in source.nodes.values() {
            if node.id == cache {
                let mut input = AnimationNode::new(
                    node.name.clone(),
                    crate::extensions::GlobalPoseInputNode {
                        key: "movement_details".into(),
                        output: "Pose".into(),
                    },
                );
                input.id = cache;
                native_leaf.add_node(input);
            } else {
                native_leaf.add_node(node.clone());
            }
        }
        for (target, value) in &source.edges_inverted {
            if matches!(
                target,
                bevy_animation_graph::core::animation_graph::TargetPin::NodeData(..)
            ) {
                native_leaf
                    .edges_inverted
                    .insert(target.clone(), value.clone());
            }
        }
        native_leaf.add_output_data("Pose".into(), DataSpec::Pose);
        native_leaf.add_output_data_edge(output, "Pose", "Pose");
        let mut actual_leaf = native_leaf.clone();
        let write = AnimationNode::new(
            "Current source hips call",
            SourceSetHipsDirectionNode {
                new_hips_direction: "LeftBackward".into(),
                ..default()
            },
        );
        actual_leaf.add_node(write.clone());
        actual_leaf.add_node_parameter_edge(output, "Pose", write.id, "Source");
        actual_leaf.add_output_data_edge(write.id, "Pose", "Pose");
        let native_leaf = app
            .world_mut()
            .resource_mut::<Assets<AnimationGraph>>()
            .add(native_leaf);
        let actual_leaf = app
            .world_mut()
            .resource_mut::<Assets<AnimationGraph>>()
            .add(actual_leaf);
        for (gap, expected_hips) in [
            (1, 0.0_f32),
            (2, 3.0),
            (65535, 0.0),
            (65536, 0.0),
            (65537, 3.0),
        ] {
            let expected = app
                .world_mut()
                .run_system_once_with(query, (native_leaf.clone(), 0, expected_hips, base.clone()))
                .unwrap();
            let actual = app
                .world_mut()
                .run_system_once_with(query, (actual_leaf.clone(), gap, 0.0, base.clone()))
                .unwrap();
            let mut dot = 1.0_f32;
            let mut distance = 0.0_f32;
            for bone in expected.paths.keys() {
                let a = actual.get_bone(*bone).unwrap();
                let e = expected.get_bone(*bone).unwrap();
                if let (Some(a), Some(e)) = (a.rotation, e.rotation) {
                    dot = dot.min(a.dot(e).abs());
                }
                if let (Some(a), Some(e)) = (a.translation, e.translation) {
                    distance = distance.max(a.distance(e));
                }
            }
            assert!(
                dot > 1.0 - 1e-5 && distance < 1e-5,
                "Call counter changes the actual reference Plant Foot pose: gap={gap}, expected_hips={expected_hips}, rotation_error_degrees={}, location_error={distance}",
                (2.0 * dot.clamp(-1.0, 1.0).acos()).to_degrees()
            );
        }
    }

    #[test]
    fn source_multi_way_blend_can_preserve_source_pin_names() {
        let node = SourceMultiWayBlendNode {
            pose_pin_prefix: "Poses_".to_owned(),
            alpha_pin_prefix: "DesiredAlphas_".to_owned(),
            ..default()
        };
        assert_eq!(node.pose_input(2), "Poses_2");
        assert_eq!(node.alpha_input(2), "DesiredAlphas_2");
    }

    #[test]
    fn source_hips_direction_preserves_become_relevant_configuration() {
        let node = SourceSetHipsDirectionNode::default();
        assert_eq!(node.new_hips_direction, "Forward");
        assert!(node.on_become_relevant);
        assert_eq!(node.call_site, "OnBecomeRelevant");
    }
}
