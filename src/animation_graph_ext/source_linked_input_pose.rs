use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::GraphInputPin,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{io_env::GraphIoEnv, new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

use crate::extensions::animation_context::AnimationContextState;

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceLinkedInputPoseNode {
    pub input: String,
    pub output: String,
    pub context_curve_prefix: String,
    pub context_curve_names: Vec<String>,
}

impl Default for SourceLinkedInputPoseNode {
    fn default() -> Self {
        Self {
            input: "Pose".to_owned(),
            output: "Pose".to_owned(),
            context_curve_prefix: String::new(),
            context_curve_names: Vec::new(),
        }
    }
}

impl NodeLike for SourceLinkedInputPoseNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let input = GraphInputPin::Passthrough(self.input.clone());
        crate::extensions::animation_context::write_linked_input_context(
            &mut ctx.graph_context,
            input.clone(),
            default(),
        )?;
        let pose = ctx
            .graph_context
            .io
            .get_data_back(
                GraphInputPin::Passthrough(self.input.clone()),
                ctx.graph_context.clone(),
            )?
            .into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(self.output.clone(), pose);
        let mut state = crate::extensions::animation_context::linked_input_context(&ctx.graph_context, &input);
        if !self.context_curve_names.is_empty() {
            for curve in &self.context_curve_names {
                let key = format!("{}{}", self.context_curve_prefix, curve);
                let value = ctx
                    .graph_context
                    .io
                    .get_data_back(GraphInputPin::Passthrough(key), ctx.graph_context.clone())?
                    .as_f32()?;
                state.curves.insert(curve.clone().into(), value);
            }
        }
        *ctx.state_mut_or_else(AnimationContextState::default)? = state;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data(self.output.clone(), DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Linked Input Pose".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::{
        SourceCachedPoseSaveNode, SourceScopedCachedPoseUseNode,
        control_rig_hierarchy::RigTransform, source_pose::SourcePose,
    };
    use bevy::math::DVec3;
    use bevy_animation_graph::core::{animation_clip::EntityPath, pose::Pose};
    use std::sync::Arc;

    const SPRINT_ALPHA: f32 = 0.4052223861217499;

    fn translation(value: f32) -> f64 {
        1.0 + f64::from(value) * 1e-10
    }

    #[derive(Reflect, Clone, Debug)]
    #[reflect(NodeLike)]
    struct ContextPose {
        value: f32,
    }

    impl NodeLike for ContextPose {
        fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
            let source_pose = Arc::new(SourcePose {
                bones: vec![(EntityPath::from_slashed_string("root".to_owned()).id(), RigTransform {
                    translation: DVec3::X * translation(self.value),
                    ..default()
                })],
                length_scale: 0.01,
            });
            let mut pose = Pose::default();
            source_pose.project_into(&mut pose);
            ctx.set_data_fwd("Pose", pose);
            *ctx.state_mut_or_else(AnimationContextState::default)? = AnimationContextState {
                curves: [("Selected".into(), self.value)].into_iter().collect(),
                curves_are_authoritative: true,
                source_pose: Some(source_pose),
                sync_players: vec![crate::extensions::sync_group_phase::SourceSyncPlayerTick {
                    identity: (ctx.graph_context.context_id, ctx.node_id),
                    key: "Movement".into(),
                    weight: 1.0,
                    phase: self.value,
                    ..default()
                }],
                ..default()
            };
            Ok(())
        }
        fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
            ctx.add_output_data("Pose", DataSpec::Pose);
            Ok(())
        }
        fn display_name(&self) -> String {
            "Context fixture".into()
        }
    }

    #[test]
    fn linked_input_context_follows_the_selected_pose_through_reentry() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip,
            animation_graph::{AnimationGraph, TimeUpdate},
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
        fn query(
            In(root): In<Handle<AnimationGraph>>,
            resources: SystemResources,
        ) -> Vec<(f32, f32, f64)> {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let env = TestIoEnv::default();
            let entities = HashMap::new();
            [1.0, 0.0, 0.5, SPRINT_ALPHA, 1.0, 0.0]
                .into_iter()
                .map(|alpha| {
                    let inputs = HashMap::from([("alpha".to_owned(), DataValue::F32(alpha))]);
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
                    let output = graph
                        .query_with_context(
                            QueryOutputTime::Forced(TimeUpdate::Delta(0.1)),
                            context.clone(),
                        )
                        .unwrap();
                    let state = crate::extensions::animation_context::graph_output_context_state(
                        graph, &context, "Pose",
                    )
                    .unwrap();
                    let pose = output["Pose"].clone().into_pose().unwrap();
                    let exact = state.source_pose_for(&pose).unwrap();
                    let snapshot = state.reflect_clone().unwrap().take::<AnimationContextState>().unwrap();
                    assert!(Arc::ptr_eq(exact, snapshot.source_pose_for(&pose).unwrap()));
                    assert_eq!(pose.bones[0].translation.unwrap().x, 0.01);
                    (state.curves[&crate::extensions::SourceCurveName::from("Selected")],
                        state.sync_players[0].phase, exact.bones[0].1.translation.x)
                })
                .collect()
        }
        let mut graphs = Assets::<AnimationGraph>::default();
        let mut child = AnimationGraph::new();
        let blend = AnimationNode::new(
            "choose",
            crate::extensions::SourceTwoWayBlendNode {
                alpha_key: "alpha".into(),
                expose_alpha_pin: false,
                ..default()
            },
        );
        child.add_node(blend.clone());
        for input in ["A", "B"] {
            child.add_input_data(GraphInputPin::Passthrough(input.to_owned()), DataSpec::Pose);
            let node = AnimationNode::new(
                input,
                SourceLinkedInputPoseNode {
                    input: input.into(),
                    ..default()
                },
            );
            child.add_node(node.clone());
            child.add_node_parameter_edge(node.id, "Pose", blend.id, input);
        }
        child.add_output_data("Pose".into(), DataSpec::Pose);
        child.add_output_data_edge(blend.id, "Pose", "Pose");
        let mut parent = AnimationGraph::new();
        let link = AnimationNode::new(
            "child",
            crate::extensions::LayeredGraphNode {
                capture_context: true,
                internalize_time: true,
                ..crate::extensions::LayeredGraphNode::new(graphs.add(child))
            },
        );
        parent.add_node(link.clone());
        for (input, value) in [("A", 0.25), ("B", 0.75)] {
            let node = AnimationNode::new(input, ContextPose { value });
            let save = AnimationNode::new(format!("Save {input}"), SourceCachedPoseSaveNode::default());
            let cached = AnimationNode::new(format!("Use {input}"), SourceScopedCachedPoseUseNode {
                cache_node: save.name.clone(),
                ..default()
            });
            for node in [&node, &save, &cached] {
                parent.add_node(node.clone());
            }
            parent.add_node_parameter_edge(node.id, "Pose", save.id, "Pose");
            parent.add_node_parameter_edge(cached.id, "Pose", link.id, input);
        }
        parent.add_output_data("Pose".into(), DataSpec::Pose);
        parent.add_output_data_edge(link.id, "Pose", "Pose");
        let root = graphs.add(parent);
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let first = 1.0 - SPRINT_ALPHA;
        let second = 1.0 - first;
        assert_eq!(second, 0.40522241592407227);
        assert_eq!(
            world.run_system_cached_with(query, root).unwrap(),
            vec![
                (0.75, 0.75, translation(0.75)),
                (0.25, 0.25, translation(0.25)),
                (0.5, 0.25, translation(0.75).mul_add(0.5, translation(0.25) * 0.5)),
                (0.25 + 0.5 * second, 0.25,
                    translation(0.75).mul_add(f64::from(second), translation(0.25) * f64::from(first))),
                (0.75, 0.75, translation(0.75)),
                (0.25, 0.25, translation(0.25))
            ]
        );
    }

    #[test]
    fn linked_input_pose_exposes_only_its_source_visible_output() {
        let node = SourceLinkedInputPoseNode::default();
        assert_eq!(node.input, "Pose");
        assert_eq!(node.output, "Pose");
        assert!(node.context_curve_names.is_empty());
    }
}
