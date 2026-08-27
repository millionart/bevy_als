use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

use crate::global_f32_input::ROOT_DEFAULTS_ENABLED_KEY;

fn root_defaults_enabled(ctx: &NodeContext) -> bool {
    ctx.graph_context
        .global_input_data
        .get(ROOT_DEFAULTS_ENABLED_KEY)
        .and_then(|value| value.as_bool().ok())
        .unwrap_or(false)
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct GlobalBoolInputNode {
    pub key: String,
}

impl NodeLike for GlobalBoolInputNode {
    fn display_name(&self) -> String {
        "ALS Global Bool Input".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let value = match ctx.graph_context.global_input_data.get(&self.key).cloned() {
            Some(value) => value,
            None if root_defaults_enabled(&ctx) => false.into(),
            None => return Err(GraphError::GlobalInputDataMissing(self.key.clone())),
        };
        value.as_bool()?;
        ctx.set_data_fwd(self.key.clone(), value);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data(self.key.clone(), DataSpec::Bool);
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
        registry.register::<GlobalBoolInputNode>();
        let registration = registry
            .get(std::any::TypeId::of::<GlobalBoolInputNode>())
            .unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <GlobalBoolInputNode as bevy::reflect::TypePath>::type_path(),
            "als_ext::GlobalBoolInputNode"
        );
    }
}
