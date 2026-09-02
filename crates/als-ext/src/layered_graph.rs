use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, TargetPin},
    animation_node::{NodeLike, ReflectNodeLike},
    context::{
        graph_context::QueryOutputTime,
        new_context::{GraphContext, NodeContext},
        spec_context::{NodeInput, NodeOutput, SpecContext},
    },
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
};

use crate::animation_context::{
    AnimationContextInput, AnimationContextInputAlias, AnimationContextIoEnv,
    AnimationContextState, capture_context_curve, context_time_update, is_context_curve_input,
    resolve_context_curves,
};
use crate::global_f32_input::{global_f32_or_root_default, global_f32_or_root_default_value};
use crate::source_cached_pose::resolve_cached_pose;

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct GlobalF32InputPin {
    pub pin: String,
    pub key: String,
    pub default_value: f32,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct FixedF32Input {
    pub input: String,
    pub value: f32,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct FixedGlobalF32Input {
    pub input: String,
    pub key: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct CachedPoseInput {
    pub input: String,
    pub cache_node: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct OutputDataAlias {
    pub child_output: String,
    pub outer_output: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct NamedPoseOutput {
    pub node_name: String,
    pub output: String,
    pub outer_output: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct LayeredGraphNode {
    pub graph: Handle<AnimationGraph>,
    pub context_inputs: Vec<AnimationContextInput>,
    pub input_aliases: Vec<AnimationContextInputAlias>,
    pub global_input_pins: Vec<GlobalF32InputPin>,
    pub fixed_f32_inputs: Vec<FixedF32Input>,
    pub fixed_global_f32_inputs: Vec<FixedGlobalF32Input>,
    pub cached_pose_inputs: Vec<CachedPoseInput>,
    pub prewarm_cached_pose_nodes: Vec<String>,
    pub output_aliases: Vec<OutputDataAlias>,
    pub named_pose_output: Option<NamedPoseOutput>,
    pub capture_context: bool,
    pub global_curve_prefixes: Vec<String>,
    pub expose_context_curves: bool,
    pub exposed_output_data: Vec<String>,
    pub hide_context_auxiliary_outputs: bool,
    pub internalize_time: bool,
}

impl LayeredGraphNode {
    pub fn new(graph: Handle<AnimationGraph>) -> Self {
        Self {
            graph,
            context_inputs: default(),
            input_aliases: default(),
            global_input_pins: default(),
            fixed_f32_inputs: default(),
            fixed_global_f32_inputs: default(),
            cached_pose_inputs: default(),
            prewarm_cached_pose_nodes: default(),
            output_aliases: default(),
            named_pose_output: None,
            capture_context: false,
            global_curve_prefixes: default(),
            expose_context_curves: false,
            exposed_output_data: default(),
            hide_context_auxiliary_outputs: false,
            internalize_time: false,
        }
    }

    fn expose_output(&self, id: &str, data_spec: &DataSpec) -> bool {
        if !self.capture_context {
            return true;
        }
        if id == "pose" {
            return true;
        }
        if self.exposed_output_data.iter().any(|value| value == id) {
            return true;
        }
        if *data_spec == DataSpec::F32 {
            return self.expose_context_curves;
        }
        !self.hide_context_auxiliary_outputs
    }

    fn outer_input(&self, child_input: &str) -> String {
        self.input_aliases
            .iter()
            .find(|alias| alias.child_input == child_input)
            .map(|alias| alias.outer_input.clone())
            .unwrap_or_else(|| child_input.to_owned())
    }

    fn outer_output(&self, child_output: &str) -> String {
        self.output_aliases
            .iter()
            .find(|alias| alias.child_output == child_output)
            .map(|alias| alias.outer_output.clone())
            .unwrap_or_else(|| child_output.to_owned())
    }

    fn io_input_aliases(&self) -> bevy::platform::collections::HashMap<String, String> {
        self.input_aliases
            .iter()
            .map(|alias| (alias.child_input.clone(), alias.outer_input.clone()))
            .collect()
    }

    fn child_data_values(
        &self,
        ctx: &NodeContext,
    ) -> Result<bevy::platform::collections::HashMap<String, DataValue>, GraphError> {
        let mut values = self
            .fixed_f32_inputs
            .iter()
            .map(|input| (input.input.clone(), DataValue::F32(input.value)))
            .collect::<bevy::platform::collections::HashMap<_, _>>();
        for input in &self.global_input_pins {
            values.insert(
                input.pin.clone(),
                DataValue::F32(global_f32_or_root_default_value(
                    ctx,
                    &input.key,
                    input.default_value,
                )?),
            );
        }
        for input in &self.fixed_global_f32_inputs {
            values.insert(
                input.input.clone(),
                DataValue::F32(global_f32_or_root_default(ctx, &input.key)?),
            );
        }
        for input in &self.cached_pose_inputs {
            let mut parent_ctx = ctx.clone();
            values.insert(
                input.input.clone(),
                DataValue::Pose(resolve_cached_pose(&mut parent_ctx, &input.cache_node)?),
            );
        }
        Ok(values)
    }

    fn prewarm_cached_poses(&self, ctx: &NodeContext) -> Result<(), GraphError> {
        for cache_node in &self.prewarm_cached_pose_nodes {
            let mut parent_ctx = ctx.clone();
            resolve_cached_pose(&mut parent_ctx, cache_node)?;
        }
        Ok(())
    }

    fn has_fixed_input(&self, input: &str) -> bool {
        self.fixed_f32_inputs
            .iter()
            .any(|value| value.input == input)
    }

    fn has_global_input(&self, input: &str) -> bool {
        self.global_input_pins
            .iter()
            .any(|value| value.pin == input)
    }

    fn has_hidden_input(&self, input: &str) -> bool {
        self.fixed_global_f32_inputs
            .iter()
            .any(|value| value.input == input)
            || self
                .cached_pose_inputs
                .iter()
                .any(|value| value.input == input)
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

    fn named_pose(
        &self,
        graph: &AnimationGraph,
        ctx: &mut GraphContext<'_>,
        output: &NamedPoseOutput,
    ) -> Result<bevy_animation_graph::core::pose::Pose, GraphError> {
        let node = graph
            .nodes
            .values()
            .find(|node| node.name == output.node_name)
            .ok_or_else(|| GraphError::UnknownErrorNode {
                graph: ctx.context_id,
                node: default(),
                error: format!("named pose source is missing: {}", output.node_name),
            })?;
        let node_id = node.id;
        let key = ctx.state_key;
        if !ctx.node_caches().is_updated(node_id, key)
            && graph.io_spec.get_output_data(&"pose".to_owned()).is_some()
        {
            let update = context_time_update(&ctx.create_node_context(node_id))?;
            ctx.context_mut().query_output_time = QueryOutputTime::from_key(key, update);
            graph.get_data(TargetPin::OutputData("pose".to_owned()), ctx.clone())?;
        }
        if !ctx.node_caches().is_update_started(node_id, key) {
            ctx.node_caches_mut().mark_update_started(node_id, key);
            node.update(
                ctx.create_node_context(node_id)
                    .with_debugging(node.should_debug),
            )?;
            ctx.node_caches_mut().mark_updated(node_id, key);
        } else if !ctx.node_caches().is_updated(node_id, key) {
            return Err(GraphError::UnknownErrorNode {
                graph: ctx.context_id,
                node: node_id,
                error: format!("named pose update is reentrant: {}", output.node_name),
            });
        }
        ctx.node_caches()
            .get_output_data(node_id, key, output.output.clone())
            .ok_or_else(|| GraphError::UnknownErrorNode {
                graph: ctx.context_id,
                node: node_id,
                error: format!("named pose output is missing: {}", output.output),
            })?
            .into_pose()
    }

    fn capture_global_curves(
        &self,
        ctx: &NodeContext,
        state: &mut AnimationContextState,
    ) -> Result<(), GraphError> {
        for prefix in &self.global_curve_prefixes {
            for (key, value) in ctx.graph_context.global_input_data {
                let Some(curve) = key.strip_prefix(prefix) else {
                    continue;
                };
                let value = value.as_f32()?;
                state.curves.insert(curve.to_owned(), value);
                state.curves.insert(source_curve_name(curve), value);
            }
        }
        Ok(())
    }
}

fn source_curve_name(value: &str) -> String {
    value
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            let Some(first) = characters.next() else {
                return String::new();
            };
            let mut result = first.to_uppercase().collect::<String>();
            result.push_str(characters.as_str());
            result
        })
        .collect()
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
            data_values: self.child_data_values(&ctx)?,
            curve_values: default(),
            input_aliases: self.io_input_aliases(),
            time_update: None,
        };

        let sub_ctx = ctx
            .create_child_context(self.graph.id(), graph, None)
            .with_io(&sub_ctx_io);

        if self.named_pose_output.is_some() {
            ctx.set_duration_fwd(None);
        } else if graph.io_spec.has_output_time() {
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

        self.prewarm_cached_poses(&ctx)?;

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
            data_values: self.child_data_values(&ctx)?,
            curve_values,
            input_aliases: self.io_input_aliases(),
            time_update: self.internalize_time.then(|| time_update.clone()).flatten(),
        };

        let mut sub_ctx = ctx
            .create_child_context(self.graph.id(), graph, None)
            .with_io(&sub_ctx_io);

        if let Some(output) = &self.named_pose_output {
            let pose = self.named_pose(graph, &mut sub_ctx, output)?;
            ctx.set_time(pose.timestamp);
            ctx.set_data_fwd(output.outer_output.clone(), pose);
            return Ok(());
        }

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
                ctx.set_data_fwd(self.outer_output(id), value);
            }
        }
        if self.capture_context {
            self.capture_global_curves(&ctx, &mut context_state)?;
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
                    if !self.has_fixed_input(&pin_id)
                        && !self.has_global_input(&pin_id)
                        && !self.has_hidden_input(&pin_id)
                        && !is_context_curve_input(&pin_id, &data_spec, &self.context_inputs)
                    {
                        ctx.add_input_data(self.outer_input(&pin_id), data_spec);
                    }
                }
                _ => {}
            }
        }
        for input in &self.global_input_pins {
            ctx.add_input_data(input.pin.clone(), DataSpec::F32);
        }

        if let Some(output) = &self.named_pose_output {
            ctx.add_output_data(output.outer_output.clone(), DataSpec::Pose);
            return Ok(());
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
                        ctx.add_output_data(self.outer_output(&pin_id), data_spec);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auxiliary_outputs_can_be_hidden_without_hiding_pose_or_explicit_data() {
        let node = LayeredGraphNode {
            capture_context: true,
            hide_context_auxiliary_outputs: true,
            exposed_output_data: vec!["ragdoll_config".to_owned()],
            ..LayeredGraphNode::new(default())
        };
        assert!(node.expose_output("pose", &DataSpec::Pose));
        assert!(node.expose_output("ragdoll_config", &DataSpec::RagdollConfig));
        assert!(!node.expose_output("ragdoll_dynamic", &DataSpec::Bool));
        assert!(!node.expose_output("PoseMoving", &DataSpec::F32));
    }

    #[test]
    fn global_curve_suffixes_retain_source_curve_pin_casing() {
        assert_eq!(source_curve_name("pose_moving"), "PoseMoving");
        assert_eq!(source_curve_name("foot_left_ik"), "FootLeftIk");
        assert_eq!(source_curve_name("rotation_yaw_speed"), "RotationYawSpeed");
    }
}
