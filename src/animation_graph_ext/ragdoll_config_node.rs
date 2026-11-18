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
pub struct RagdollConfigNode;

impl NodeLike for RagdollConfigNode {
    fn display_name(&self) -> String {
        "ALS Ragdoll Config".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let dynamic = ctx.data_back(IN_DYNAMIC)?.as_bool()?;
        let readback = ctx.data_back(IN_READBACK)?.as_bool()?;
        let config = RagdollConfig {
            default_mode: if dynamic {
                None
            } else {
                Some(BodyMode::Kinematic)
            },
            mode_overrides: Default::default(),
            default_readback: Some(readback),
            readback_overrides: Default::default(),
        };
        ctx.set_data_fwd(OUT_CONFIG, DataValue::from(config));
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_DYNAMIC, DataSpec::Bool)
            .add_input_data(IN_READBACK, DataSpec::Bool);
        ctx.add_output_data(OUT_CONFIG, DataSpec::RagdollConfig);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::reflect::TypeRegistry;
    use bevy_animation_graph::core::{
        animation_graph::GraphInputPin, utils::testing::GraphTestHarness,
    };

    #[test]
    fn registers_node_like_dispatch_key() {
        let mut registry = TypeRegistry::default();
        registry.register::<RagdollConfigNode>();
        let reg = registry
            .get(std::any::TypeId::of::<RagdollConfigNode>())
            .unwrap();
        assert!(
            reg.data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <RagdollConfigNode as bevy::reflect::TypePath>::type_path(),
            "als_ext::RagdollConfigNode"
        );
    }

    #[test]
    fn activation_preserves_asset_body_modes_and_independent_readback() {
        GraphTestHarness::node(RagdollConfigNode)
            .with_const_input_data(GraphInputPin::Passthrough(IN_DYNAMIC.into()), true)
            .with_const_input_data(GraphInputPin::Passthrough(IN_READBACK.into()), false)
            .when_queried()
            .then_output_is(
                OUT_CONFIG,
                RagdollConfig {
                    default_mode: None,
                    mode_overrides: default(),
                    default_readback: Some(false),
                    readback_overrides: default(),
                },
            );
    }
}
