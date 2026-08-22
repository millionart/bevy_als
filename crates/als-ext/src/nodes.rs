use bevy::prelude::*;
use bevy_animation_graph::core::animation_node::{NodeLike, ReflectNodeLike};
use bevy_animation_graph::core::context::{
    new_context::NodeContext, spec_context::SpecContext,
};
use bevy_animation_graph::core::edge_data::DataSpec;
use bevy_animation_graph::core::errors::GraphError;

use crate::curve::{sample_curve, CurveKey, CurveSampleMode};

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct CurveSampleNode {
    pub mode: CurveSampleMode,
    pub keys: Vec<CurveKey>,
}

impl CurveSampleNode {
    pub fn from_parts(mode: CurveSampleMode, keys: Vec<CurveKey>) -> Self {
        Self { mode, keys }
    }
}

impl NodeLike for CurveSampleNode {
    fn display_name(&self) -> String {
        "ALS Curve Sample".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let x = ctx.data_back("x")?.as_f32()?;
        ctx.set_data_fwd("value", sample_curve(self.mode, &self.keys, x));
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("x", DataSpec::F32);
        ctx.add_output_data("value", DataSpec::F32);
        Ok(())
    }
}
