use bevy::{platform::collections::HashMap, prelude::*};
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

use crate::animation_context::{AnimationContextIoEnv, context_time_update};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InertializedLinkedLayerActivation {
    pub reset_child: bool,
    pub forward_request: bool,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct GlobalDataInput {
    pub input: String,
    pub global_key: String,
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
    pub selector_global_key: Option<String>,
    pub global_data_inputs: Vec<GlobalDataInput>,
    pub reset_on_becoming_relevant: bool,
    pub forward_requests_through_skipped_cached_poses: bool,
    pub capture_context: bool,
    pub expose_context_curves: bool,
    pub exposed_output_data: Vec<String>,
    pub internalize_time: bool,
}

impl Default for InertializedLinkedAnimLayerNode {
    fn default() -> Self {
        Self {
            interface: default(),
            implementations: default(),
            selector: "layer_index".to_owned(),
            selector_global_key: None,
            global_data_inputs: default(),
            reset_on_becoming_relevant: true,
            forward_requests_through_skipped_cached_poses: true,
            capture_context: false,
            expose_context_curves: false,
            exposed_output_data: default(),
            internalize_time: false,
        }
    }
}

impl InertializedLinkedAnimLayerNode {
    fn selected_implementation_index(&self, ctx: &NodeContext) -> Result<usize, GraphError> {
        let Some(_) = self.implementations.first() else {
            return Err(GraphError::GraphAssetMissing);
        };
        let value = match &self.selector_global_key {
            Some(key) => ctx
                .graph_context
                .global_input_data
                .get(key)
                .ok_or_else(|| GraphError::GlobalInputDataMissing(key.clone()))?
                .as_f32()?,
            None => ctx.data_back(self.selector.clone())?.as_f32()?,
        };
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

    fn expose_output(&self, id: &str, data_spec: &DataSpec) -> bool {
        !self.capture_context
            || *data_spec != DataSpec::F32
            || self.expose_context_curves
            || self.exposed_output_data.iter().any(|value| value == id)
    }

    fn uses_global_data_input(&self, input: &str) -> bool {
        self.global_data_inputs
            .iter()
            .any(|binding| binding.input == input)
    }

    fn global_data_values(
        &self,
        ctx: &NodeContext,
    ) -> Result<HashMap<String, DataValue>, GraphError> {
        let mut values = HashMap::default();
        for binding in &self.global_data_inputs {
            let value = ctx
                .graph_context
                .global_input_data
                .get(&binding.global_key)
                .cloned()
                .ok_or_else(|| GraphError::GlobalInputDataMissing(binding.global_key.clone()))?;
            if values.insert(binding.input.clone(), value).is_some() {
                return Err(GraphError::MismatchedDataType(
                    "unique global data input".to_owned(),
                    binding.input.clone(),
                ));
            }
        }
        Ok(values)
    }

    fn time_update(
        &self,
        ctx: &NodeContext,
    ) -> Option<bevy_animation_graph::core::animation_graph::TimeUpdate> {
        if self.internalize_time {
            context_time_update(ctx).ok()
        } else {
            ctx.time_update_fwd().ok()
        }
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
        let data_values = self.global_data_values(&ctx)?;
        let sub_ctx_io = AnimationContextIoEnv {
            parent_ctx: ctx.clone(),
            data_values,
            curve_values: default(),
            input_aliases: default(),
            time_update: None,
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
                    .map(|(id, data_spec)| (id.clone(), *data_spec))
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
        let time_update = self.time_update(&ctx);
        let data_values = self.global_data_values(&ctx)?;
        let sub_ctx_io = AnimationContextIoEnv {
            parent_ctx: ctx.clone(),
            data_values,
            curve_values: default(),
            input_aliases: default(),
            time_update: self.internalize_time.then(|| time_update.clone()).flatten(),
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
            if let Some(time_update) = time_update.clone() {
                let key = sub_ctx.state_key;
                sub_ctx.context_mut().query_output_time =
                    QueryOutputTime::from_key(key, time_update);
            }
        }
        if !self.internalize_time
            && let Some(time_update) = time_update
        {
            for pin in input_times {
                if let GraphInputPin::Passthrough(pin_id) = pin {
                    ctx.set_time_update_back(pin_id, time_update.clone());
                }
            }
        }
        for (id, data_spec) in output_data {
            let value = graph.get_data(TargetPin::OutputData(id.clone()), sub_ctx.clone())?;
            if self.capture_context && data_spec == DataSpec::F32 {
                ctx.set_data_fwd(id.clone(), value.clone());
            }
            if self.expose_output(&id, &data_spec) {
                ctx.set_data_fwd(id, value);
            }
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        let interface = ctx
            .resources()
            .graph_assets
            .get(&self.interface)
            .ok_or(GraphError::GraphAssetMissing)?;
        if self.selector_global_key.is_none() {
            ctx.add_input_data(self.selector.clone(), DataSpec::F32);
        }
        for input in interface.io_spec.sorted_inputs() {
            match input {
                NodeInput::Time(GraphInputPin::Passthrough(pin_id)) => {
                    if !self.internalize_time {
                        ctx.add_input_time(pin_id);
                    }
                }
                NodeInput::Data(GraphInputPin::Passthrough(pin_id), data_spec) => {
                    if !self.uses_global_data_input(&pin_id) {
                        ctx.add_input_data(pin_id, data_spec);
                    }
                }
                _ => {}
            }
        }
        for output in interface.io_spec.sorted_outputs() {
            match output {
                NodeOutput::Time => {
                    if !self.internalize_time {
                        ctx.add_output_time();
                    }
                }
                NodeOutput::Data(pin_id, data_spec) => {
                    if self.expose_output(&pin_id, &data_spec) {
                        ctx.add_output_data(pin_id, data_spec);
                    }
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
