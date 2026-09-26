use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

#[derive(Reflect, Clone, Debug, Default)]
struct FirstUpdateState {
    updated: bool,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SuppressFirstUpdateNode;

impl SuppressFirstUpdateNode {
    pub const INPUT: &'static str = "condition";
    pub const OUTPUT: &'static str = "out";
}

impl NodeLike for SuppressFirstUpdateNode {
    fn display_name(&self) -> String {
        "ALS Suppress First Update".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let condition = ctx.data_back(Self::INPUT)?.as_bool()?;
        let output = {
            let state = ctx.state_mut::<FirstUpdateState>()?;
            let output = state.updated && condition;
            state.updated = true;
            output
        };
        ctx.set_data_fwd(Self::OUTPUT, output);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(Self::INPUT, DataSpec::Bool)
            .add_output_data(Self::OUTPUT, DataSpec::Bool);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<SuppressFirstUpdateNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<SuppressFirstUpdateNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
