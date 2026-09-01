use bevy::prelude::*;
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

use crate::animation_context::{
    AnimationContextInput, AnimationContextInputAlias, AnimationContextIoEnv,
    AnimationContextState, capture_context_curve, context_time_update, is_context_curve_input,
    resolve_context_curves,
};

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct LayeredGraphNode {
    pub graph: Handle<AnimationGraph>,
    pub context_inputs: Vec<AnimationContextInput>,
    pub input_aliases: Vec<AnimationContextInputAlias>,
    pub capture_context: bool,
    pub expose_context_curves: bool,
    pub exposed_output_data: Vec<String>,
    pub internalize_time: bool,
}

impl LayeredGraphNode {
    pub fn new(graph: Handle<AnimationGraph>) -> Self {
        Self {
            graph,
            context_inputs: default(),
            input_aliases: default(),
            capture_context: false,
            expose_context_curves: false,
            exposed_output_data: default(),
            internalize_time: false,
        }
    }

    fn expose_output(&self, id: &str, data_spec: &DataSpec) -> bool {
        !self.capture_context
            || *data_spec != DataSpec::F32
            || self.expose_context_curves
            || self.exposed_output_data.iter().any(|value| value == id)
    }

    fn outer_input(&self, child_input: &str) -> String {
        self.input_aliases
            .iter()
            .find(|alias| alias.child_input == child_input)
            .map(|alias| alias.outer_input.clone())
            .unwrap_or_else(|| child_input.to_owned())
    }

    fn io_input_aliases(&self) -> bevy::platform::collections::HashMap<String, String> {
        self.input_aliases
            .iter()
            .map(|alias| (alias.child_input.clone(), alias.outer_input.clone()))
            .collect()
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

        let sub_ctx_io = AnimationContextIoEnv {
            parent_ctx: ctx.clone(),
            data_values: default(),
            curve_values: default(),
            input_aliases: self.io_input_aliases(),
            time_update: None,
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

        let time_update = self.time_update(&ctx);
        if !self.internalize_time
            && let Some(time_update) = &time_update
        {
            for pin in graph.io_spec.iter_input_times() {
                if let GraphInputPin::Passthrough(pin_id) = pin {
                    ctx.set_time_update_back(pin_id.clone(), time_update.clone());
                }
            }
        }

        let curve_values = if self.capture_context {
            resolve_context_curves(&ctx, graph, &self.context_inputs)?
        } else {
            default()
        };

        let sub_ctx_io = AnimationContextIoEnv {
            parent_ctx: ctx.clone(),
            data_values: default(),
            curve_values,
            input_aliases: self.io_input_aliases(),
            time_update: self.internalize_time.then(|| time_update.clone()).flatten(),
        };

        let mut sub_ctx = ctx
            .create_child_context(self.graph.id(), graph, None)
            .with_io(&sub_ctx_io);

        if graph.io_spec.has_output_time()
            && let Some(time_update) = time_update
        {
            let key = sub_ctx.state_key;
            sub_ctx.context_mut().query_output_time = QueryOutputTime::from_key(key, time_update);
        }

        let mut context_state = AnimationContextState::default();
        for (id, data_spec) in graph.io_spec.iter_output_data() {
            let value = graph.get_data(TargetPin::OutputData(id.clone()), sub_ctx.clone())?;
            if self.capture_context {
                capture_context_curve(&mut context_state, id, &value)?;
            }
            if self.expose_output(id, data_spec) {
                ctx.set_data_fwd(id, value);
            }
        }
        if self.capture_context {
            *ctx.state_mut_or_else(AnimationContextState::default)? = context_state;
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
                    if !self.internalize_time {
                        ctx.add_input_time(pin_id);
                    }
                }
                NodeInput::Data(GraphInputPin::Passthrough(pin_id), data_spec) => {
                    if !is_context_curve_input(&pin_id, &data_spec, &self.context_inputs) {
                        ctx.add_input_data(self.outer_input(&pin_id), data_spec);
                    }
                }
                _ => {}
            }
        }

        for output in graph.io_spec.sorted_outputs() {
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

    fn display_name(&self) -> String {
        "ALS Layered Graph".into()
    }
}
