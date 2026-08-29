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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InertializedLinkedLayerActivation {
    pub reset_child: bool,
    pub forward_request: bool,
}

pub fn inertialized_linked_layer_activation(
    previous_selection: Option<usize>,
    previous_frame: Option<u64>,
    selection: usize,
    frame: u64,
    reset_on_becoming_relevant: bool,
    forward_requests_through_skipped_cached_poses: bool,
) -> InertializedLinkedLayerActivation {
    let selection_changed = previous_selection.is_some_and(|previous| previous != selection);
    let became_relevant = previous_frame.is_some_and(|previous| previous.wrapping_add(1) != frame);
    InertializedLinkedLayerActivation {
        reset_child: previous_selection.is_none()
            || selection_changed
            || (reset_on_becoming_relevant && became_relevant),
        forward_request: forward_requests_through_skipped_cached_poses
            && (selection_changed || became_relevant),
    }
}

#[derive(Reflect, Clone, Debug, Default)]
struct InertializedLinkedLayerState {
    selected_implementation: Option<usize>,
    last_update_frame: Option<u64>,
    reset_generation: u64,
    forwarded_request_generation: u64,
}

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
    fn selected_implementation_index(&self, ctx: &NodeContext) -> Result<usize, GraphError> {
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
        Ok(index)
    }

    fn selected_implementation(
        &self,
        ctx: &NodeContext,
    ) -> Result<(usize, Handle<AnimationGraph>), GraphError> {
        let index = self.selected_implementation_index(ctx)?;
        Ok((index, self.implementations[index].clone()))
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
        let (_, implementation) = self.selected_implementation(&ctx)?;
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
        let (implementation_index, implementation) = self.selected_implementation(&ctx)?;
        let graph = ctx
            .graph_context
            .resources
            .animation_graph_assets
            .get(&implementation)
            .ok_or(GraphError::GraphAssetMissing)?;
        let sub_ctx_io = NestedGraphIoEnv {
            parent_ctx: ctx.clone(),
        };
        let frame = ctx.graph_context.frame_index();
        let activation = {
            let state = ctx.state_mut::<InertializedLinkedLayerState>()?;
            let activation = inertialized_linked_layer_activation(
                state.selected_implementation,
                state.last_update_frame,
                implementation_index,
                frame,
                self.reset_on_becoming_relevant,
                self.forward_requests_through_skipped_cached_poses,
            );
            state.selected_implementation = Some(implementation_index);
            state.last_update_frame = Some(frame);
            if activation.reset_child {
                state.reset_generation = state.reset_generation.wrapping_add(1);
            }
            if activation.forward_request {
                state.forwarded_request_generation =
                    state.forwarded_request_generation.wrapping_add(1);
            }
            activation
        };
        let child_context_id = ctx.create_child_context_id(implementation.id(), None);
        if activation.reset_child || activation.forward_request {
            if let Some(child_context) = ctx
                .graph_context
                .context_arena
                .get_mut()
                .get_context_mut(child_context_id)
            {
                child_context.node_states.clear();
                child_context.query_output_time = QueryOutputTime::None;
            }
        }
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

    #[test]
    fn activation_resets_on_switch_and_relevance_return() {
        let first = inertialized_linked_layer_activation(None, None, 0, 3, true, true);
        assert!(first.reset_child);
        assert!(!first.forward_request);
        let switch = inertialized_linked_layer_activation(Some(0), Some(3), 1, 4, true, true);
        assert!(switch.reset_child);
        assert!(switch.forward_request);
        let return_after_skip =
            inertialized_linked_layer_activation(Some(1), Some(4), 1, 7, true, true);
        assert!(return_after_skip.reset_child);
        assert!(return_after_skip.forward_request);
        let continuous = inertialized_linked_layer_activation(Some(1), Some(7), 1, 8, true, true);
        assert!(!continuous.reset_child);
        assert!(!continuous.forward_request);
    }
}
