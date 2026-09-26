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
    pub output: String,
}

impl GlobalPoseInputNode {
    fn output_key(&self) -> &str {
        if self.output.is_empty() {
            &self.key
        } else {
            &self.output
        }
    }
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
        ctx.set_data_fwd(self.output_key().to_owned(), value);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data(self.output_key().to_owned(), DataSpec::Pose)
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

    #[test]
    fn pose_output_defaults_to_the_global_key() {
        let node = GlobalPoseInputNode {
            key: "ragdolling_state.final_ragdoll_pose".to_owned(),
            ..default()
        };
        assert_eq!(node.output_key(), "ragdolling_state.final_ragdoll_pose");
        assert_eq!(
            GlobalPoseInputNode {
                output: "Pose".to_owned(),
                ..node
            }
            .output_key(),
            "Pose"
        );
    }
}
