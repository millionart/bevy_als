use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, TargetPin},
    animation_node::{NodeLike, ReflectNodeLike},
    context::{
        graph_context::QueryOutputTime,
        new_context::NodeContext,
        spec_context::{NodeInput, NodeOutput, SpecContext},
    },
    edge_data::{DataSpec, DataValue, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::linear::LinearInterpolator,
};

use crate::extensions::animation_context::{
    AnimationContextInput, AnimationContextInputAlias, AnimationContextIoEnv,
    AnimationContextState, capture_context_curve, context_time_update, graph_output_context_state,
    is_context_curve_input, merge_missing_context_curves, resolve_context_curves,
    select_context_pose_output,
};
use crate::extensions::layered_graph::OutputDataAlias;
use crate::extensions::mesh_space_additive::apply_mesh_space_additive;

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SkeletonAwareLayeredGraphNode {
    pub graph: Handle<AnimationGraph>,
    pub fallback_pose_key: String,
    pub slot_name: Option<String>,
    pub slot_pose_key: String,
    pub slot_weight_key: String,
    pub slot_is_additive: bool,
    pub slot_curve_prefix: Option<String>,
    pub slot_curve_names: Vec<String>,
    pub always_update_source_pose: bool,
    pub context_inputs: Vec<AnimationContextInput>,
    pub input_aliases: Vec<AnimationContextInputAlias>,
    pub output_aliases: Vec<OutputDataAlias>,
    pub capture_context: bool,
    pub expose_context_curves: bool,
    pub exposed_output_data: Vec<String>,
    pub internalize_time: bool,
}

fn apply_slot_pose(
    base: &mut bevy_animation_graph::core::pose::Pose,
    slot: &bevy_animation_graph::core::pose::Pose,
    weight: f32,
    slot_is_additive: bool,
    skeleton: &bevy_animation_graph::core::skeleton::Skeleton,
) {
    let timestamp = base.timestamp;
    if slot_is_additive {
        apply_mesh_space_additive(base, slot, weight, skeleton);
    } else {
        LinearInterpolator {
            bone_mask: BoneMask::all(),
        }
        .interpolate_pose(base, slot, weight);
    }
    base.timestamp = timestamp;
}

impl SkeletonAwareLayeredGraphNode {
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
                    let skeleton = ctx
                        .graph_context
                        .resources
                        .skeleton_assets
                        .get(&pose.skeleton)
                        .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
                    apply_slot_pose(
                        &mut pose,
                        &slot_pose,
                        weight,
                        self.slot_is_additive,
                        skeleton,
                    );
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
        let mut pose_output = None;
        for (id, data_spec) in graph.io_spec.iter_output_data() {
            let value = graph.get_data(TargetPin::OutputData(id.clone()), sub_ctx.clone())?;
            let value = self.apply_fallback(id.as_str(), value, &ctx)?;
            let outer_output = self.outer_output(id);
            if self.capture_context {
                capture_context_curve(&mut context_state, id, &value)?;
            }
            select_context_pose_output(&mut pose_output, id, data_spec);
            if self.expose_output(id, data_spec) {
                ctx.set_data_fwd(outer_output, value);
            }
        }
        if self.capture_context {
            if let Some(output) = pose_output
                && let Some(output_context) = graph_output_context_state(graph, &sub_ctx, &output)
            {
                merge_missing_context_curves(&mut context_state, &output_context);
            }
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
                        ctx.add_output_data(self.outer_output(&pin_id), data_spec);
                    }
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
    use bevy_animation_graph::core::{
        animation_clip::EntityPath,
        pose::{BonePose, Pose},
        skeleton::Skeleton,
    };

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

    #[test]
    fn additive_slot_applies_a_delta_instead_of_interpolating_it_as_an_absolute_pose() {
        let root_path = EntityPath::default().child("root");
        let root = root_path.id();
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root);
        skeleton.add_bone(root_path, Transform::IDENTITY, Transform::IDENTITY);
        let mut base = Pose::default();
        base.add_bone(
            BonePose {
                translation: Some(Vec3::new(1.0, 0.0, 0.0)),
                ..default()
            },
            root,
        );
        let mut delta = Pose::default();
        delta.add_bone(
            BonePose {
                translation: Some(Vec3::new(2.0, 0.0, 0.0)),
                ..default()
            },
            root,
        );
        apply_slot_pose(&mut base, &delta, 0.5, true, &skeleton);
        assert_eq!(
            base.get_bone(root).and_then(|bone| bone.translation),
            Some(Vec3::new(2.0, 0.0, 0.0))
        );
    }
}
