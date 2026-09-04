use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::linear::LinearInterpolator,
    pose::Pose,
};

use crate::{
    GameplayTagBlendMode,
    curve_bundle::{active_curve_weights, curve_value_input},
    gameplay_tag_blend::{
        GameplayTagBlendState, WEIGHT_THRESHOLD, advance_gameplay_tag_weights,
        gameplay_tag_blend_time, select_gameplay_tag_child_index_for_tag, weight_output,
    },
    multi_way_blend::{pose_input, time_input},
    source_gameplay_tag_input::gameplay_tag_input,
};

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct GameplayTagLayerBlendNode {
    pub active_key: String,
    pub tags: Vec<String>,
    pub blend_times: Vec<f32>,
    pub blend_mode: GameplayTagBlendMode,
    pub curve_names: Vec<String>,
    pub signal_names: Vec<String>,
    pub normalize_alpha: bool,
    pub normalize_curve_weights: bool,
}

impl Default for GameplayTagLayerBlendNode {
    fn default() -> Self {
        Self {
            active_key: default(),
            tags: default(),
            blend_times: vec![0.0],
            blend_mode: default(),
            curve_names: default(),
            signal_names: default(),
            normalize_alpha: true,
            normalize_curve_weights: false,
        }
    }
}

impl GameplayTagLayerBlendNode {
    fn child_count(&self) -> usize {
        self.tags.len() + 1
    }
}

impl NodeLike for GameplayTagLayerBlendNode {
    fn display_name(&self) -> String {
        "ALS Gameplay Tag Layer Blend".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(None);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let active_tag = gameplay_tag_input(ctx.graph_context.root_entity, &self.active_key)
            .ok_or_else(|| GraphError::GlobalInputDataMissing(self.active_key.clone()))?;
        let time_update = ctx.time_update_fwd()?;
        let delta_seconds = match &time_update {
            TimeUpdate::Delta(value) => *value,
            TimeUpdate::Absolute(_) | TimeUpdate::PercentOfEvent { .. } => 0.0,
        };
        let child_count = self.child_count();
        let active_index = select_gameplay_tag_child_index_for_tag(&self.tags, Some(&active_tag));
        let weights = {
            let state = ctx.state_mut::<GameplayTagBlendState>()?;
            advance_gameplay_tag_weights(
                state,
                child_count,
                active_index,
                gameplay_tag_blend_time(&self.blend_times, active_index),
                self.blend_mode,
                delta_seconds,
            )
        };
        let pose_weights = active_curve_weights(&weights, self.normalize_alpha);
        let curve_weights = active_curve_weights(&weights, self.normalize_curve_weights);
        for (index, weight) in pose_weights.iter().copied().enumerate() {
            if weight > WEIGHT_THRESHOLD {
                ctx.set_time_update_back(time_input(index), time_update.clone());
            }
        }

        let mut accumulated_weight = 0.0;
        let mut output: Option<Pose> = None;
        for (index, weight) in pose_weights.iter().copied().enumerate() {
            if weight <= WEIGHT_THRESHOLD {
                continue;
            }
            let pose = ctx.data_back(pose_input(index))?.into_pose()?;
            if let Some(base) = output.as_mut() {
                let factor = weight / (accumulated_weight + weight);
                LinearInterpolator {
                    bone_mask: BoneMask::all(),
                }
                .interpolate_pose(base, &pose, factor);
            } else {
                output = Some(pose);
            }
            accumulated_weight += weight;
        }

        for curve in &self.curve_names {
            let mut value: f32 = 0.0;
            for (index, weight) in curve_weights.iter().copied().enumerate() {
                if weight > WEIGHT_THRESHOLD {
                    value += ctx.data_back(curve_value_input(index, curve))?.as_f32()? * weight;
                }
            }
            ctx.set_data_fwd(curve.clone(), value);
        }
        for signal in &self.signal_names {
            let mut value: f32 = 0.0;
            for (index, weight) in pose_weights.iter().copied().enumerate() {
                if weight > WEIGHT_THRESHOLD {
                    value = value.max(ctx.data_back(signal_input(index, signal))?.as_f32()?);
                }
            }
            ctx.set_data_fwd(signal.clone(), value);
        }

        let output = output.unwrap_or_default();
        ctx.set_time(output.timestamp);
        ctx.set_data_fwd("pose", output);
        for (index, weight) in weights.iter().copied().enumerate() {
            ctx.set_data_fwd(weight_output(index), weight);
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        for index in 0..self.child_count() {
            ctx.add_input_data(pose_input(index), DataSpec::Pose)
                .add_input_time(time_input(index));
            for curve in &self.curve_names {
                ctx.add_input_data(curve_value_input(index, curve), DataSpec::F32);
            }
            for signal in &self.signal_names {
                ctx.add_input_data(signal_input(index, signal), DataSpec::F32);
            }
            ctx.add_output_data(weight_output(index), DataSpec::F32);
        }
        ctx.add_output_data("pose", DataSpec::Pose)
            .add_output_time();
        for curve in &self.curve_names {
            ctx.add_output_data(curve.clone(), DataSpec::F32);
        }
        for signal in &self.signal_names {
            ctx.add_output_data(signal.clone(), DataSpec::F32);
        }
        Ok(())
    }
}

fn signal_input(index: usize, signal: &str) -> String {
    format!("signal_{index}_{signal}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;
        let mut registry = TypeRegistry::default();
        registry.register::<GameplayTagLayerBlendNode>();
        let registration = registry
            .get(std::any::TypeId::of::<GameplayTagLayerBlendNode>())
            .unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <GameplayTagLayerBlendNode as bevy::reflect::TypePath>::type_path(),
            "als_ext::GameplayTagLayerBlendNode"
        );
    }
}
