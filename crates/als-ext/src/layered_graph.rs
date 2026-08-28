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
    errors::GraphError,
};

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct LayeredGraphNode {
    pub graph: Handle<AnimationGraph>,
}

impl LayeredGraphNode {
    pub fn new(graph: Handle<AnimationGraph>) -> Self {
        Self { graph }
    }
}

impl NodeLike for LayeredGraphNode {
    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let Some(graph) = ctx
            .graph_context
            .resources
            .animation_graph_assets
            .get(&self.graph)
        else {
            return Ok(());
        };

        let sub_ctx_io = NestedGraphIoEnv {
            parent_ctx: ctx.clone(),
        };

        let sub_ctx = ctx
            .create_child_context(self.graph.id(), graph, None)
            .with_io(&sub_ctx_io);

        if graph.io_spec.has_output_time() {
            let duration = graph.get_duration(TargetPin::OutputTime, sub_ctx)?;
            ctx.set_duration_fwd(duration);
        } else {
            ctx.set_duration_fwd(None);
        }

        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let Some(graph) = ctx
            .graph_context
            .resources
            .animation_graph_assets
            .get(&self.graph)
        else {
            return Ok(());
        };

        let sub_ctx_io = NestedGraphIoEnv {
            parent_ctx: ctx.clone(),
        };

        let mut sub_ctx = ctx
            .create_child_context(self.graph.id(), graph, None)
            .with_io(&sub_ctx_io);

        if graph.io_spec.has_output_time()
            && let Ok(time_update) = ctx.time_update_fwd()
        {
            let key = sub_ctx.state_key;
            sub_ctx.context_mut().query_output_time = QueryOutputTime::from_key(key, time_update);
        }

        if let Ok(time_update) = ctx.time_update_fwd() {
            for pin in graph.io_spec.iter_input_times() {
                if let GraphInputPin::Passthrough(pin_id) = pin {
                    ctx.set_time_update_back(pin_id.clone(), time_update.clone());
                }
            }
        }

        for (id, _) in graph.io_spec.iter_output_data() {
            let value = graph.get_data(TargetPin::OutputData(id.clone()), sub_ctx.clone())?;
            ctx.set_data_fwd(id, value);
        }

        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        let graph = ctx
            .resources()
            .graph_assets
            .get(&self.graph)
            .ok_or(GraphError::GraphAssetMissing)?;

        for input in graph.io_spec.sorted_inputs() {
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

        for output in graph.io_spec.sorted_outputs() {
            match output {
                NodeOutput::Time => {
                    ctx.add_output_time();
                }
                NodeOutput::Data(pin_id, data_spec) => {
                    ctx.add_output_data(pin_id, data_spec);
                }
            }
        }

        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Layered Graph".into()
    }
}
