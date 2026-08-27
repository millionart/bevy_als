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
    edge_data::DataSpec,
    errors::GraphError,
};

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct InertializedLinkedAnimLayerNode {
    pub interface: Handle<AnimationGraph>,
    pub implementations: Vec<Handle<AnimationGraph>>,
    pub selector: String,
    pub reset_on_becoming_relevant: bool,
    pub forward_requests_through_skipped_cached_poses: bool,
}

impl Default for InertializedLinkedAnimLayerNode {
    fn default() -> Self {
        Self {
            interface: default(),
            implementations: default(),
            selector: "layer_index".to_owned(),
            reset_on_becoming_relevant: true,
            forward_requests_through_skipped_cached_poses: true,
        }
    }
}

impl InertializedLinkedAnimLayerNode {
    fn selected_implementation(
        &self,
        ctx: &NodeContext,
    ) -> Result<Handle<AnimationGraph>, GraphError> {
        let Some(_) = self.implementations.first() else {
            return Err(GraphError::GraphAssetMissing);
        };
        let value = ctx.data_back(self.selector.clone())?.as_f32()?;
        let index = if value.is_finite() {
            value
                .round()
                .clamp(0.0, (self.implementations.len() - 1) as f32) as usize
        } else {
            0
        };
        Ok(self.implementations[index].clone())
    }
}

impl NodeLike for InertializedLinkedAnimLayerNode {
    fn display_name(&self) -> String {
        "ALS Inertialized Linked Animation Layer".into()
    }

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
        let (has_output_time, input_times, output_data) = {
            let interface = ctx
                .graph_context
                .resources
                .animation_graph_assets
                .get(&self.interface)
                .ok_or(GraphError::GraphAssetMissing)?;
            (
                interface.io_spec.has_output_time(),
                interface
                    .io_spec
                    .iter_input_times()
                    .cloned()
                    .collect::<Vec<_>>(),
                interface
                    .io_spec
                    .iter_output_data()
                    .map(|(id, _)| id.clone())
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
        if has_output_time {
            if let Ok(time_update) = ctx.time_update_fwd() {
                let key = sub_ctx.state_key;
                sub_ctx.context_mut().query_output_time =
                    QueryOutputTime::from_key(key, time_update);
            }
        }
        if let Ok(time_update) = ctx.time_update_fwd() {
            for pin in input_times {
                if let GraphInputPin::Passthrough(pin_id) = pin {
                    ctx.set_time_update_back(pin_id, time_update.clone());
                }
            }
        }
        for id in output_data {
            let value = graph.get_data(TargetPin::OutputData(id.clone()), sub_ctx.clone())?;
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
        ctx.add_input_data(self.selector.clone(), DataSpec::F32);
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
        for output in interface.io_spec.sorted_outputs() {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<InertializedLinkedAnimLayerNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<InertializedLinkedAnimLayerNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
