use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

pub const ROOT_DEFAULTS_ENABLED_KEY: &str = "als_root_defaults_enabled";

pub fn root_defaults_enabled(ctx: &NodeContext) -> bool {
    ctx.graph_context
        .global_input_data
        .get(ROOT_DEFAULTS_ENABLED_KEY)
        .and_then(|value| value.as_bool().ok())
        .unwrap_or(false)
}

pub fn global_f32_or_root_default(ctx: &NodeContext, key: &str) -> Result<f32, GraphError> {
    match ctx.graph_context.global_input_data.get(key) {
        Some(value) => value.as_f32(),
        None if root_defaults_enabled(ctx) => Ok(0.0),
        None => Err(GraphError::GlobalInputDataMissing(key.to_owned())),
    }
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct GlobalF32InputNode {
    pub key: String,
}

impl NodeLike for GlobalF32InputNode {
    fn display_name(&self) -> String {
        "ALS Global F32 Input".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let value = match ctx.graph_context.global_input_data.get(&self.key).cloned() {
            Some(value) => value,
            None if root_defaults_enabled(&ctx) => 0.0_f32.into(),
            None => return Err(GraphError::GlobalInputDataMissing(self.key.clone())),
        };
        value.as_f32()?;
        ctx.set_data_fwd(self.key.clone(), value);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data(self.key.clone(), DataSpec::F32);
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
        registry.register::<GlobalF32InputNode>();
        let registration = registry
            .get(std::any::TypeId::of::<GlobalF32InputNode>())
            .unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <GlobalF32InputNode as bevy::reflect::TypePath>::type_path(),
            "als_ext::GlobalF32InputNode"
        );
    }
}
