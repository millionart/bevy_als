use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::linear::LinearInterpolator,
    pose::Pose,
};

use crate::animation_context::{AnimationContextState, pose_input_context_state};
use crate::global_f32_input::global_f32_or_root_default;
use crate::source_pose_nodes::blend_animation_contexts;

fn node_error(ctx: &NodeContext, message: String) -> GraphError {
    GraphError::UnknownErrorNode {
        graph: ctx.graph_context.context_id,
        node: ctx.node_id,
        error: message,
    }
}

fn source_reference_pose(source: &Pose) -> Pose {
    Pose {
        skeleton: source.skeleton.clone(),
        timestamp: source.timestamp,
        ..default()
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
        let fallback = if key.is_empty() {
            *default
        } else {
            global_f32_or_root_default(ctx, key)?
        };
        let input = ctx
            .data_back(self.alpha_input(index))
            .ok()
            .and_then(|value| value.as_f32().ok());
        Ok(input.unwrap_or(fallback) * self.alpha_scale)
    }
}

impl NodeLike for SourceMultiWayBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if self.desired_alpha_keys.len() != self.pose_count
            || self.desired_alpha_defaults.len() != self.pose_count
        {
            return Err(node_error(
                &ctx,
                "desired alpha configuration does not match pose count".to_owned(),
            ));
        }
        let mut active = Vec::with_capacity(self.pose_count);
        let mut total_weight = 0.0;
        for index in 0..self.pose_count {
            let weight = self.alpha(&ctx, index)?;
            if weight > f32::EPSILON {
                total_weight += weight;
                active.push((index, weight));
            }
        }
        let mut accumulated_weight = 0.0;
        let mut output = None;
        let mut contexts = Vec::new();
        for (index, raw_weight) in active {
            let weight = if self.normalize_alpha {
                raw_weight / total_weight
            } else {
                raw_weight
            };
            let pose = ctx.data_back(self.pose_input(index))?.into_pose()?;
            if let Some(context) = pose_input_context_state(&ctx, &self.pose_input(index)) {
                contexts.push((context, weight));
            }
            if let Some(base) = output.as_mut() {
                let factor = weight / (accumulated_weight + weight);
                LinearInterpolator {
                    bone_mask: BoneMask::all(),
                }
                .interpolate_pose(base, &pose, factor);
            } else {
                output = Some(pose);
            }
            accumulated_weight += weight;
        }
        let output = match output {
            Some(output) => output,
            None => source_reference_pose(&ctx.data_back(self.pose_input(0))?.into_pose()?),
        };
        ctx.set_time(output.timestamp);
        ctx.set_data_fwd("Pose", output);
        *ctx.state_mut_or_else(AnimationContextState::default)? =
            blend_animation_contexts(contexts);
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
        let became_relevant = crate::source_instance::source_call_became_relevant(&ctx)?;
        if !self.on_become_relevant || became_relevant {
            let value = ctx
                .data_back("NewHipsDirection")
                .and_then(|value| value.as_f32())
                .or_else(|_| {
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
                    .ok_or_else(|| node_error(&ctx, "source hips direction is invalid".to_owned()))
                })?;
            crate::source_instance::set_source_property(
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
    fn source_hips_direction_write_is_visible_to_the_child_graph() {
        use crate::{GlobalF32InputNode, GlobalPoseInputNode, SourceCurveModifyNode};
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
            for frame in 0..4 {
                if frame == 2 {
                    arena.next_frame();
                }
                if frame == 3 {
                    arena = GraphContextArena::new(root.id());
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
                if frame == 1 {
                    crate::source_instance::set_source_property(
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
                    crate::animation_context::graph_output_context_state(graph, &context, "Pose")
                        .unwrap()
                        .curves["Probe"],
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
            [3.0, 0.0, 3.0, 3.0]
        );
    }

    #[test]
    fn source_multi_way_blend_reference_pose_preserves_only_source_identity() {
        let mut source = Pose {
            timestamp: 2.5,
            ..default()
        };
        source.bones.push(BonePose::default());
        let reference = source_reference_pose(&source);
        assert_eq!(reference.timestamp, source.timestamp);
        assert_eq!(reference.skeleton, source.skeleton);
        assert!(reference.bones.is_empty());
        assert!(reference.paths.is_empty());
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
