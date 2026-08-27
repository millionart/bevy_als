use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct GlobalPoseInputNode {
    pub key: String,
}

impl NodeLike for GlobalPoseInputNode {
    fn display_name(&self) -> String {
        "ALS Global Pose Input".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let value = ctx
            .graph_context
            .global_input_data
            .get(&self.key)
            .cloned()
            .ok_or_else(|| GraphError::GlobalInputDataMissing(self.key.clone()))?;
        let pose = value.clone().into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(self.key.clone(), value);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data(self.key.clone(), DataSpec::Pose)
            .add_output_time();
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
        registry.register::<GlobalPoseInputNode>();
        let registration = registry
            .get(std::any::TypeId::of::<GlobalPoseInputNode>())
            .unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <GlobalPoseInputNode as bevy::reflect::TypePath>::type_path(),
            "als_ext::GlobalPoseInputNode"
        );
    }
}
