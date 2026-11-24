use bevy::prelude::*;
use bevy_animation_graph::builtin_nodes::graph_node::NestedGraphIoEnv;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, TargetPin},
    animation_node::{NodeLike, ReflectNodeLike},
    context::{
        graph_context::QueryOutputTime,
        new_context::NodeContext,
        spec_context::{NodeInput, NodeOutput, SpecContext},
    },
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
};

use crate::extensions::animation_context::{context_time_update, graph_output_context_state};

pub const ALS_LAYER_CURVE_NAMES: [&str; 18] = [
    "LayerArmLeft",
    "LayerArmLeftAdditive",
    "LayerArmLeftLocalSpace",
    "LayerArmRight",
    "LayerArmRightAdditive",
    "LayerArmRightLocalSpace",
    "LayerHead",
    "LayerHeadAdditive",
    "LayerLegs",
    "LayerPelvis",
    "LayerSpine",
    "LayerSpineAdditive",
    "HandLeftIk",
    "HandRightIk",
    "HipsDirectionLock",
    "LayerHandLeft",
    "LayerHandRight",
    "PoseAiming",
];

pub const ALS_LAYER_SIGNAL_NAMES: [&str; 1] = ["transition_left_request"];

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct LinkedAnimLayerNode {
    pub interface: Handle<AnimationGraph>,
    pub implementations: Vec<Handle<AnimationGraph>>,
    pub selector: String,
}

impl Default for LinkedAnimLayerNode {
    fn default() -> Self {
        Self {
            interface: default(),
            implementations: default(),
            selector: "layer_index".to_owned(),
        }
    }
}

fn source_pose_output_alias(output: &str, data_spec: &DataSpec) -> bool {
    *data_spec == DataSpec::Pose && output != "Pose"
}

impl LinkedAnimLayerNode {
    fn selection_index(value: f32, count: usize) -> usize {
        if count <= 1 || !value.is_finite() {
            return 0;
        }
        value.round().clamp(0.0, (count - 1) as f32) as usize
    }

    fn selected_implementation(
        &self,
        ctx: &NodeContext,
    ) -> Result<Handle<AnimationGraph>, GraphError> {
        let Some(_) = self.implementations.first() else {
            return Err(GraphError::GraphAssetMissing);
        };
        let value = match ctx.data_back(self.selector.clone())? {
            DataValue::F32(value) => value,
            value => {
                return Err(GraphError::MismatchedDataType(
                    "F32".to_owned(),
                    format!("{value:?}"),
                ));
            }
        };
        Ok(self.implementations[Self::selection_index(value, self.implementations.len())].clone())
    }
}

impl NodeLike for LinkedAnimLayerNode {
    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let has_output_time = ctx
            .graph_context
            .resources
            .animation_graph_assets
            .get(&self.interface)
            .ok_or(GraphError::GraphAssetMissing)?
            .io_spec
            .has_output_time();
        let implementation = self.selected_implementation(&ctx)?;
        let graph = ctx
            .graph_context
            .resources
            .animation_graph_assets
            .get(&implementation)
            .ok_or(GraphError::GraphAssetMissing)?;
        let sub_ctx_io = NestedGraphIoEnv {
            parent_ctx: ctx.clone(),
        };
        let sub_ctx = ctx
            .create_child_context(implementation.id(), graph, None)
            .with_io(&sub_ctx_io);

        if has_output_time {
            ctx.set_duration_fwd(graph.get_duration(TargetPin::OutputTime, sub_ctx)?);
        } else {
            ctx.set_duration_fwd(None);
        }

        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let (input_times, output_data) = {
            let interface = ctx
                .graph_context
                .resources
                .animation_graph_assets
                .get(&self.interface)
                .ok_or(GraphError::GraphAssetMissing)?;
            (
                interface
                    .io_spec
                    .iter_input_times()
                    .cloned()
                    .collect::<Vec<_>>(),
                interface
                    .io_spec
                    .iter_output_data()
                    .map(|(id, data_spec)| (id.clone(), data_spec.clone()))
                    .collect::<Vec<_>>(),
            )
        };
        let implementation = self.selected_implementation(&ctx)?;
        let graph = ctx
            .graph_context
            .resources
            .animation_graph_assets
            .get(&implementation)
            .ok_or(GraphError::GraphAssetMissing)?;
        let sub_ctx_io = NestedGraphIoEnv {
            parent_ctx: ctx.clone(),
        };
        let mut sub_ctx = ctx
            .create_child_context(implementation.id(), graph, None)
            .with_io(&sub_ctx_io);

        if let Ok(time_update) = context_time_update(&ctx) {
            let key = sub_ctx.state_key;
            sub_ctx.context_mut().query_output_time =
                QueryOutputTime::from_key(key, time_update.clone());
            for pin in input_times {
                if let GraphInputPin::Passthrough(pin_id) = pin {
                    ctx.set_time_update_back(pin_id, time_update.clone());
                }
            }
        }

        for (id, data_spec) in output_data {
            let value = graph.get_data(TargetPin::OutputData(id.clone()), sub_ctx.clone())?;
            if data_spec == DataSpec::Pose {
                ctx.replace_state(
                    graph_output_context_state(graph, &sub_ctx, &id).unwrap_or_default(),
                )?;
            }
            if source_pose_output_alias(&id, &data_spec) {
                ctx.set_data_fwd("Pose", value.clone());
            }
            ctx.set_data_fwd(id, value);
        }

        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        let interface = ctx
            .resources()
            .graph_assets
            .get(&self.interface)
            .ok_or(GraphError::GraphAssetMissing)?;

        for input in interface.io_spec.sorted_inputs() {
            match input {
                NodeInput::Time(GraphInputPin::Passthrough(pin_id)) => {
                    ctx.add_input_time(pin_id);
                }
                NodeInput::Data(GraphInputPin::Passthrough(pin_id), data_spec) => {
                    ctx.add_input_data(pin_id, data_spec);
                }
                _ => {}
            }
        }
        ctx.add_input_data(self.selector.clone(), DataSpec::F32);

        for output in interface.io_spec.sorted_outputs() {
            match output {
                NodeOutput::Time => {
                    ctx.add_output_time();
                }
                NodeOutput::Data(pin_id, data_spec) => {
                    if source_pose_output_alias(&pin_id, &data_spec) {
                        ctx.add_output_data("Pose", DataSpec::Pose);
                    }
                    ctx.add_output_data(pin_id, data_spec);
                }
            }
        }

        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Linked Anim Layer".into()
    }
}

#[cfg(test)]
mod tests {
    use super::{LinkedAnimLayerNode, source_pose_output_alias};
    use bevy_animation_graph::core::edge_data::DataSpec;

    #[test]
    fn layer_selection_clamps_to_the_available_implementations() {
        assert_eq!(LinkedAnimLayerNode::selection_index(-1.0, 3), 0);
        assert_eq!(LinkedAnimLayerNode::selection_index(1.0, 3), 1);
        assert_eq!(LinkedAnimLayerNode::selection_index(8.0, 3), 2);
        assert_eq!(LinkedAnimLayerNode::selection_index(f32::NAN, 3), 0);
    }

    #[test]
    fn pose_result_is_exposed_through_the_source_pose_pin() {
        assert!(source_pose_output_alias("Result", &DataSpec::Pose));
        assert!(!source_pose_output_alias("Pose", &DataSpec::Pose));
        assert!(!source_pose_output_alias("Result", &DataSpec::F32));
    }

    #[test]
    fn linked_output_retains_source_curves_and_sync_ticks() {
        use bevy::{platform::collections::HashMap, prelude::*};
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
        fn query(In(handle): In<Handle<AnimationGraph>>, resources: SystemResources) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let inputs = HashMap::from([("delta".into(), DataValue::F32(0.1))]);
            for expected_phase in [0.05, 0.1, 0.15] {
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
                let output = crate::extensions::animation_context::graph_output_context_state(
                    graph, &context, "Result",
                )
                .expect("linked context must exist");
                assert_eq!(
                    output
                        .curves
                        .get(&crate::extensions::SourceCurveName::from("LayerSpine")),
                    Some(&0.7)
                );
                assert!(output.curves_are_authoritative);
                assert_eq!(output.sync_players.len(), 1);
                assert!((output.sync_players[0].phase - expected_phase).abs() < 1e-5);
            }
        }
        let mut world = World::new();
        let mut graphs = Assets::<AnimationGraph>::default();
        let mut clips = Assets::<GraphClip>::default();
        let clip = clips.add(GraphClip {
            duration: 2.0,
            ..default()
        });
        let sequence = AnimationNode::new(
            "Sequence",
            crate::extensions::SourceSequencePlayerNode {
                clip,
                expose_play_rate_pin: false,
                curves: crate::extensions::AlsPoseCurves(std::collections::HashMap::from([(
                    "LayerSpine".into(),
                    crate::extensions::AlsFloatCurve {
                        times: vec![0.0],
                        values: vec![0.7],
                        ..default()
                    },
                )])),
                synchronization: Some(
                    crate::extensions::source_sequence_player::SourceSequenceSynchronization {
                        key: "secondary".into(),
                        delta_seconds_key: "delta".into(),
                        phase_duration: 2.0,
                        ..default()
                    },
                ),
                ..default()
            },
        );
        let mut child = AnimationGraph::new();
        child.add_node(sequence.clone());
        child.add_output_data("Result".into(), DataSpec::Pose);
        child.add_output_data_edge(sequence.id, "Pose", "Result");
        let child = graphs.add(child);
        let mut graph = AnimationGraph::new();
        let selection = AnimationNode::new(
            "Selection",
            crate::extensions::ConstantF32Node { value: 0.0 },
        );
        let linked = AnimationNode::new(
            "Linked",
            LinkedAnimLayerNode {
                interface: child.clone(),
                implementations: vec![child],
                ..default()
            },
        );
        let root = AnimationNode::new(
            "Root",
            crate::extensions::SourcePosePassthroughNode {
                output_time: false,
                synchronize_groups: true,
                ..default()
            },
        );
        for node in [&selection, &linked, &root] {
            graph.add_node(node.clone());
        }
        graph.add_node_parameter_edge(selection.id, "value", linked.id, "layer_index");
        graph.add_node_parameter_edge(linked.id, "Pose", root.id, "Source");
        graph.add_output_data("Result".into(), DataSpec::Pose);
        graph.add_output_data_edge(root.id, "Pose", "Result");
        let handle = graphs.add(graph);
        world.insert_resource(graphs);
        world.insert_resource(clips);
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        world.run_system_cached_with(query, handle).unwrap();
    }
}
