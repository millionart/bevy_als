use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

pub fn f32_to_vec2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct F32ToVec2Node;

impl F32ToVec2Node {
    pub const IN_X: &'static str = "x";
    pub const IN_Y: &'static str = "y";
    pub const OUT_VALUE: &'static str = "value";
}

impl NodeLike for F32ToVec2Node {
    fn display_name(&self) -> String {
        "ALS F32 To Vec2".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let x = ctx.data_back(Self::IN_X)?.as_f32()?;
        let y = ctx.data_back(Self::IN_Y)?.as_f32()?;
        ctx.set_data_fwd(Self::OUT_VALUE, f32_to_vec2(x, y));
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(Self::IN_X, DataSpec::F32)
            .add_input_data(Self::IN_Y, DataSpec::F32)
            .add_output_data(Self::OUT_VALUE, DataSpec::Vec2);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_preserves_axis_order() {
        assert_eq!(f32_to_vec2(0.25, 0.75), Vec2::new(0.25, 0.75));
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<F32ToVec2Node>();
        assert!(
            registry
                .get(std::any::TypeId::of::<F32ToVec2Node>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
