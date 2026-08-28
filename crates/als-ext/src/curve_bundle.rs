use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};
use serde::{Deserialize, Serialize};

use crate::curve::{CurveKey, CurveSampleMode, sample_curve};

const WEIGHT_THRESHOLD: f32 = 1e-6;

pub fn curve_weight_input(index: usize) -> String {
    format!("weight_{index}")
}

pub fn curve_value_input(index: usize, curve: &str) -> String {
    format!("curve_{index}_{curve}")
}

pub fn curve_modify_input(curve: &str) -> String {
    format!("input_{curve}")
}

pub fn curve_sample_output(sample_index: usize, curve: &str) -> String {
    format!("sample_{sample_index}_{curve}")
}

pub fn curve_compose_base_input(curve: &str) -> String {
    format!("base_{curve}")
}

pub fn curve_compose_overlay_input(curve: &str) -> String {
    format!("overlay_{curve}")
}

#[derive(Reflect, Clone, Debug, Default)]
pub struct NamedCurve {
    pub name: String,
    pub mode: CurveSampleMode,
    pub keys: Vec<CurveKey>,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct CurveSampleSetNode {
    pub sample_positions: Vec<f32>,
    pub curves: Vec<NamedCurve>,
}

impl NodeLike for CurveSampleSetNode {
    fn display_name(&self) -> String {
        "ALS Curve Sample Set".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        for (sample_index, position) in self.sample_positions.iter().copied().enumerate() {
            for curve in &self.curves {
                let value = sample_curve(curve.mode, &curve.keys, position);
                ctx.set_data_fwd(curve_sample_output(sample_index, &curve.name), value);
            }
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        for sample_index in 0..self.sample_positions.len() {
            for curve in &self.curves {
                ctx.add_output_data(
                    curve_sample_output(sample_index, &curve.name),
                    DataSpec::F32,
                );
            }
        }
        Ok(())
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct CurveBlendNode {
    pub curve_names: Vec<String>,
    pub input_count: usize,
    pub normalize_weights: bool,
}

impl Default for CurveBlendNode {
    fn default() -> Self {
        Self {
            curve_names: default(),
            input_count: 2,
            normalize_weights: true,
        }
    }
}

pub fn active_curve_weights(raw_weights: &[f32], normalize: bool) -> Vec<f32> {
    let mut weights = raw_weights
        .iter()
        .map(|weight| {
            if *weight > WEIGHT_THRESHOLD {
                *weight
            } else {
                0.0
            }
        })
        .collect::<Vec<_>>();
    if normalize {
        let total = weights.iter().sum::<f32>();
        if total > WEIGHT_THRESHOLD {
            for weight in &mut weights {
                *weight /= total;
            }
        }
    }
    weights
}

impl NodeLike for CurveBlendNode {
    fn display_name(&self) -> String {
        "ALS Curve Blend".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let raw_weights = (0..self.input_count)
            .map(|index| ctx.data_back(curve_weight_input(index))?.as_f32())
            .collect::<Result<Vec<_>, GraphError>>()?;
        let weights = active_curve_weights(&raw_weights, self.normalize_weights);
        for curve in &self.curve_names {
            let mut value = 0.0;
            for (index, weight) in weights.iter().copied().enumerate() {
                if weight > WEIGHT_THRESHOLD {
                    value += ctx.data_back(curve_value_input(index, curve))?.as_f32()? * weight;
                }
            }
            ctx.set_data_fwd(curve.clone(), value);
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        for index in 0..self.input_count {
            ctx.add_input_data(curve_weight_input(index), DataSpec::F32);
            for curve in &self.curve_names {
                ctx.add_input_data(curve_value_input(index, curve), DataSpec::F32);
            }
        }
        for curve in &self.curve_names {
            ctx.add_output_data(curve.clone(), DataSpec::F32);
        }
        Ok(())
    }
}

#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[reflect(Default, Serialize)]
pub enum CurveComposeMode {
    #[default]
    Accumulate,
    Override,
}

pub fn compose_curve_value(mode: CurveComposeMode, base: f32, overlay: f32, alpha: f32) -> f32 {
    match mode {
        CurveComposeMode::Accumulate => base + overlay * alpha,
        CurveComposeMode::Override => overlay * alpha,
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct CurveComposeNode {
    pub curve_names: Vec<String>,
    pub alpha: f32,
    pub mode: CurveComposeMode,
}

impl Default for CurveComposeNode {
    fn default() -> Self {
        Self {
            curve_names: default(),
            alpha: 1.0,
            mode: default(),
        }
    }
}

impl NodeLike for CurveComposeNode {
    fn display_name(&self) -> String {
        "ALS Curve Compose".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        for curve in &self.curve_names {
            let overlay = ctx
                .data_back(curve_compose_overlay_input(curve))?
                .as_f32()?;
            let value = match self.mode {
                CurveComposeMode::Accumulate => {
                    let base = ctx.data_back(curve_compose_base_input(curve))?.as_f32()?;
                    compose_curve_value(self.mode, base, overlay, self.alpha)
                }
                CurveComposeMode::Override => {
                    compose_curve_value(self.mode, 0.0, overlay, self.alpha)
                }
            };
            ctx.set_data_fwd(curve.clone(), value);
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        for curve in &self.curve_names {
            if self.mode == CurveComposeMode::Accumulate {
                ctx.add_input_data(curve_compose_base_input(curve), DataSpec::F32);
            }
            ctx.add_input_data(curve_compose_overlay_input(curve), DataSpec::F32)
                .add_output_data(curve.clone(), DataSpec::F32);
        }
        Ok(())
    }
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct CurveModifyNode {
    pub curve_names: Vec<String>,
    pub values: Vec<f32>,
    pub alpha: f32,
}

pub fn blend_curve_value(source: f32, target: f32, alpha: f32) -> f32 {
    source + (target - source) * alpha
}

impl NodeLike for CurveModifyNode {
    fn display_name(&self) -> String {
        "ALS Curve Modify".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        for (index, curve) in self.curve_names.iter().enumerate() {
            let source = ctx.data_back(curve_modify_input(curve))?.as_f32()?;
            let target = self.values.get(index).copied().unwrap_or_default();
            ctx.set_data_fwd(curve.clone(), blend_curve_value(source, target, self.alpha));
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        for curve in &self.curve_names {
            ctx.add_input_data(curve_modify_input(curve), DataSpec::F32)
                .add_output_data(curve.clone(), DataSpec::F32);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_curve_weights_preserve_active_sum() {
        let weights = active_curve_weights(&[0.2, 0.3, 0.0], true);
        assert!((weights[0] - 0.4).abs() < 1e-6);
        assert!((weights[1] - 0.6).abs() < 1e-6);
        assert_eq!(weights[2], 0.0);
    }

    #[test]
    fn non_normalized_curve_weights_keep_raw_values() {
        assert_eq!(active_curve_weights(&[0.25, 0.75], false), vec![0.25, 0.75]);
    }

    #[test]
    fn curve_modify_blend_matches_unreal_lerp_semantics() {
        assert!((blend_curve_value(1.0, 3.0, 0.25) - 1.5).abs() < 1e-6);
        assert_eq!(blend_curve_value(1.0, 3.0, 1.0), 3.0);
    }

    #[test]
    fn curve_compose_matches_source_accumulate_and_override_semantics() {
        assert_eq!(
            compose_curve_value(CurveComposeMode::Accumulate, 0.25, 0.5, 0.5),
            0.5
        );
        assert_eq!(
            compose_curve_value(CurveComposeMode::Override, 0.25, 0.5, 0.5),
            0.25
        );
    }

    #[test]
    fn nodes_register_node_like_dispatch_keys() {
        use bevy::reflect::TypeRegistry;
        let mut registry = TypeRegistry::default();
        registry.register::<CurveSampleSetNode>();
        registry.register::<CurveBlendNode>();
        registry.register::<CurveComposeNode>();
        registry.register::<CurveModifyNode>();
        for type_id in [
            std::any::TypeId::of::<CurveSampleSetNode>(),
            std::any::TypeId::of::<CurveBlendNode>(),
            std::any::TypeId::of::<CurveComposeNode>(),
            std::any::TypeId::of::<CurveModifyNode>(),
        ] {
            assert!(
                registry
                    .get(type_id)
                    .unwrap()
                    .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                    .is_some()
            );
        }
    }
}
