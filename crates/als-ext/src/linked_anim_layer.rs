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
    use super::LinkedAnimLayerNode;

    #[test]
    fn layer_selection_clamps_to_the_available_implementations() {
        assert_eq!(LinkedAnimLayerNode::selection_index(-1.0, 3), 0);
        assert_eq!(LinkedAnimLayerNode::selection_index(1.0, 3), 1);
        assert_eq!(LinkedAnimLayerNode::selection_index(8.0, 3), 2);
        assert_eq!(LinkedAnimLayerNode::selection_index(f32::NAN, 3), 0);
    }
}
