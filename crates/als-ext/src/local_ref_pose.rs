use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::Pose,
};

use crate::animation_context::context_time_update;

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct LocalRefPoseNode {
    pub internalize_time: bool,
}

impl NodeLike for LocalRefPoseNode {
    fn display_name(&self) -> String {
        "ALS Local Reference Pose".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(None);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let time_update = if self.internalize_time {
            context_time_update(&ctx)?
        } else {
            ctx.time_update_fwd()?
        };
        let time = time_update
            .partial_update_basic(ctx.prev_time())
            .unwrap_or_else(|| ctx.prev_time());
        let pose = Pose {
            timestamp: time,
            ..default()
        };
        ctx.set_time(time);
        ctx.set_data_fwd("pose", pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data("pose", DataSpec::Pose);
        if !self.internalize_time {
            ctx.add_output_time();
        }
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
        registry.register::<LocalRefPoseNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<LocalRefPoseNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
