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
    edge_data::{DataValue, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::linear::LinearInterpolator,
};

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SkeletonAwareLayeredGraphNode {
    pub graph: Handle<AnimationGraph>,
    pub fallback_pose_key: String,
    pub slot_name: Option<String>,
    pub slot_pose_key: String,
    pub slot_weight_key: String,
    pub slot_curve_prefix: Option<String>,
    pub slot_curve_names: Vec<String>,
    pub always_update_source_pose: bool,
}

impl SkeletonAwareLayeredGraphNode {
    fn slot_weight(&self, ctx: &NodeContext) -> Result<f32, GraphError> {
        if self.slot_name.is_none() {
            return Ok(0.0);
        }
        Ok(ctx
            .graph_context
            .global_input_data
            .get(&self.slot_weight_key)
            .map(|value| value.as_f32())
            .transpose()?
            .filter(|value| value.is_finite())
            .unwrap_or_default()
            .clamp(0.0, 1.0))
    }

    fn slot_curve_value(
        &self,
        output_id: &str,
        ctx: &NodeContext,
    ) -> Result<Option<f32>, GraphError> {
        if self.slot_name.is_none() || !self.slot_curve_names.iter().any(|name| name == output_id) {
            return Ok(None);
        }
        let Some(prefix) = &self.slot_curve_prefix else {
            return Ok(None);
        };
        ctx.graph_context
            .global_input_data
            .get(&format!("{prefix}{output_id}"))
            .map(|value| value.as_f32())
            .transpose()
            .map(|value| value.filter(|value| value.is_finite()))
    }

    fn apply_fallback(
        &self,
        output_id: &str,
        value: DataValue,
        ctx: &NodeContext,
    ) -> Result<DataValue, GraphError> {
        let weight = self.slot_weight(ctx)?;
        let mut pose = match value {
            DataValue::F32(base) => {
                let slot_curve = self.slot_curve_value(output_id, ctx)?;
                return Ok(DataValue::F32(blend_slot_curve(base, slot_curve, weight)));
            }
            DataValue::Pose(pose) => pose,
            value => return Ok(value),
        };
        if ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&pose.skeleton)
            .is_none()
        {
            let fallback = ctx
                .graph_context
                .global_input_data
                .get(&self.fallback_pose_key)
                .cloned()
                .ok_or_else(|| GraphError::GlobalInputDataMissing(self.fallback_pose_key.clone()))?
                .into_pose()?;
            pose.skeleton = fallback.skeleton;
        }
        if output_id == "pose" && self.slot_name.is_some() {
            if weight > 0.0 {
                if let Some(slot_pose) = ctx
                    .graph_context
                    .global_input_data
                    .get(&self.slot_pose_key)
                    .cloned()
                    .map(|value| value.into_pose())
                    .transpose()?
                {
                    let timestamp = pose.timestamp;
                    LinearInterpolator {
                        bone_mask: BoneMask::all(),
                    }
                    .interpolate_pose(&mut pose, &slot_pose, weight);
                    pose.timestamp = timestamp;
                }
            }
        }
        Ok(DataValue::Pose(pose))
    }
}

fn blend_slot_curve(base: f32, slot: Option<f32>, weight: f32) -> f32 {
    match slot.filter(|value| value.is_finite()) {
        Some(slot) => base.lerp(slot, weight.clamp(0.0, 1.0)),
        None => base,
    }
}

impl NodeLike for SkeletonAwareLayeredGraphNode {
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
            ctx.set_duration_fwd(graph.get_duration(TargetPin::OutputTime, sub_ctx)?);
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
            ctx.set_data_fwd(id.clone(), self.apply_fallback(id.as_str(), value, &ctx)?);
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
        "ALS Skeleton Aware Layered Graph".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<SkeletonAwareLayeredGraphNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<SkeletonAwareLayeredGraphNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }

    #[test]
    fn slot_curve_blend_uses_the_pose_weight() {
        assert_eq!(blend_slot_curve(1.0, Some(0.0), 0.25), 0.75);
        assert_eq!(blend_slot_curve(0.0, Some(1.0), 0.5), 0.5);
        assert_eq!(blend_slot_curve(0.3, None, 1.0), 0.3);
        assert_eq!(blend_slot_curve(0.3, Some(f32::NAN), 1.0), 0.3);
    }
}
