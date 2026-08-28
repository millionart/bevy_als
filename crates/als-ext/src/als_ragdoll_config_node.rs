use bevy::prelude::*;
use bevy_animation_graph::core::animation_node::{NodeLike, ReflectNodeLike};
use bevy_animation_graph::core::context::{new_context::NodeContext, spec_context::SpecContext};
use bevy_animation_graph::core::edge_data::{DataSpec, DataValue};
use bevy_animation_graph::core::errors::GraphError;
use bevy_animation_graph::core::ragdoll::configuration::RagdollConfig;
use bevy_animation_graph::core::ragdoll::definition::BodyMode;

pub const IN_DYNAMIC: &str = "dynamic";
pub const IN_READBACK: &str = "readback";
pub const OUT_CONFIG: &str = "ragdoll_config";

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct AlsRagdollConfigNode;

impl NodeLike for AlsRagdollConfigNode {
    fn display_name(&self) -> String {
        "ALS Root Ragdoll Config".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let dynamic = ctx.data_back(IN_DYNAMIC)?.as_bool()?;
        let readback = ctx.data_back(IN_READBACK)?.as_bool()?;
        let config = if dynamic {
            RagdollConfig {
                default_mode: Some(BodyMode::Dynamic),
                mode_overrides: default(),
                default_readback: Some(readback),
                readback_overrides: default(),
            }
        } else {
            RagdollConfig::default()
        };
        ctx.set_data_fwd(OUT_CONFIG, DataValue::from(config));
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_DYNAMIC, DataSpec::Bool)
            .add_input_data(IN_READBACK, DataSpec::Bool)
            .add_output_data(OUT_CONFIG, DataSpec::RagdollConfig);
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
        registry.register::<AlsRagdollConfigNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<AlsRagdollConfigNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
