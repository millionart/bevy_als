use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

use crate::extensions::{NamedCurve, active_curve_weights, curve_weight_input, sample_curve};

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct CurveWeightedSamplesNode {
    pub curves: Vec<NamedCurve>,
    pub sample_positions: Vec<f32>,
    pub normalize_weights: bool,
}

impl NodeLike for CurveWeightedSamplesNode {
    fn display_name(&self) -> String {
        "ALS Weighted Curve Samples".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let raw_weights = (0..self.sample_positions.len())
            .map(|index| ctx.data_back(curve_weight_input(index))?.as_f32())
            .collect::<Result<Vec<_>, GraphError>>()?;
        let weights = active_curve_weights(&raw_weights, self.normalize_weights);
        for curve in &self.curves {
            let value = self
                .sample_positions
                .iter()
                .zip(&weights)
                .map(|(position, weight)| sample_curve(curve.mode, &curve.keys, *position) * weight)
                .sum::<f32>();
            ctx.set_data_fwd(curve.name.clone(), value);
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        for index in 0..self.sample_positions.len() {
            ctx.add_input_data(curve_weight_input(index), DataSpec::F32);
        }
        for curve in &self.curves {
            ctx.add_output_data(curve.name.clone(), DataSpec::F32);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::extensions::{CurveKey, CurveSampleMode};

    use super::*;

    #[test]
    fn weighted_samples_preserve_unnormalized_leaf_blend() {
        let curve = NamedCurve {
            name: "Value".to_owned(),
            mode: CurveSampleMode::Linear,
            keys: vec![CurveKey { t: 0.0, v: 2.0 }, CurveKey { t: 1.0, v: 6.0 }],
        };
        let node = CurveWeightedSamplesNode {
            curves: vec![curve],
            sample_positions: vec![0.0, 1.0],
            normalize_weights: false,
        };
        assert_eq!(node.sample_positions.len(), 2);
        assert_eq!(
            sample_curve(CurveSampleMode::Linear, &node.curves[0].keys, 0.0),
            2.0
        );
        assert_eq!(
            sample_curve(CurveSampleMode::Linear, &node.curves[0].keys, 1.0),
            6.0
        );
        assert_eq!(active_curve_weights(&[0.25, 0.75], false), vec![0.25, 0.75]);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<CurveWeightedSamplesNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<CurveWeightedSamplesNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
