use bevy::prelude::*;
use bevy_animation_graph::core::animation_node::{NodeLike, ReflectNodeLike};
use bevy_animation_graph::core::context::{new_context::NodeContext, spec_context::SpecContext};
use bevy_animation_graph::core::edge_data::DataSpec;
use bevy_animation_graph::core::errors::GraphError;

use crate::extensions::curve::{CurveKey, CurveSampleMode, sample_curve};

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct ConstantF32Node {
    pub value: f32,
}

impl NodeLike for ConstantF32Node {
    fn display_name(&self) -> String {
        "ALS Constant F32".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_data_fwd("value", self.value);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data("value", DataSpec::F32);
        Ok(())
    }
}

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

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct CurveDrivenValueNode {
    pub mode: CurveSampleMode,
    pub keys: Vec<CurveKey>,
    pub scale: f32,
    pub bias: f32,
}

impl NodeLike for CurveDrivenValueNode {
    fn display_name(&self) -> String {
        "ALS Curve Driven Value".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let x = ctx.data_back("x")?.as_f32()?;
        let raw = sample_curve(self.mode, &self.keys, x);
        ctx.set_data_fwd("value", raw * self.scale + self.bias);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("x", DataSpec::F32);
        ctx.add_output_data("value", DataSpec::F32);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::reflect::{GetTypeRegistration, TypeRegistration, TypeRegistry};

    fn registry_with<T: GetTypeRegistration + Default + 'static>() -> TypeRegistry {
        let mut registry = TypeRegistry::default();
        registry.register::<T>();
        registry
    }

    #[test]
    fn curve_sample_node_registers_node_like_dispatch_key() {
        let registry = registry_with::<CurveSampleNode>();
        let reg = registry
            .get(std::any::TypeId::of::<CurveSampleNode>())
            .unwrap();
        assert!(
            reg.data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <CurveSampleNode as bevy::reflect::TypePath>::type_path(),
            "als_ext::CurveSampleNode"
        );
    }

    #[test]
    fn constant_f32_node_registers_node_like_dispatch_key() {
        let registry = registry_with::<ConstantF32Node>();
        let reg = registry
            .get(std::any::TypeId::of::<ConstantF32Node>())
            .unwrap();
        assert!(
            reg.data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <ConstantF32Node as bevy::reflect::TypePath>::type_path(),
            "als_ext::ConstantF32Node"
        );
    }

    #[test]
    fn curve_driven_value_node_registers_node_like_dispatch_key() {
        let registry = registry_with::<CurveDrivenValueNode>();
        let reg = registry
            .get(std::any::TypeId::of::<CurveDrivenValueNode>())
            .unwrap();
        assert!(
            reg.data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <CurveDrivenValueNode as bevy::reflect::TypePath>::type_path(),
            "als_ext::CurveDrivenValueNode"
        );
    }

    #[test]
    fn driven_value_remap_math() {
        let node = CurveDrivenValueNode {
            mode: CurveSampleMode::Linear,
            keys: vec![CurveKey { t: 0.0, v: 0.0 }, CurveKey { t: 1.0, v: 1.0 }],
            scale: 2.0,
            bias: -1.0,
        };
        let x = 0.5;
        let raw = sample_curve(node.mode, &node.keys, x);
        assert!((raw * node.scale + node.bias - 0.0).abs() < 1e-6);
    }
}
