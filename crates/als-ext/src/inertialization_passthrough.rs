use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct InertializationPassthroughNode {
    pub reset_on_becoming_relevant: bool,
    pub forward_requests_through_skipped_cached_poses: bool,
    pub curve_names: Vec<String>,
}

impl Default for InertializationPassthroughNode {
    fn default() -> Self {
        Self {
            reset_on_becoming_relevant: true,
            forward_requests_through_skipped_cached_poses: true,
            curve_names: Vec::new(),
        }
    }
}

impl NodeLike for InertializationPassthroughNode {
    fn display_name(&self) -> String {
        "Inertialization".to_owned()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back("source_time")?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let time_update = ctx.time_update_fwd()?;
        ctx.set_time_update_back("source_time", time_update);
        let pose = ctx.data_back("Source")?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("pose", pose);
        for curve in &self.curve_names {
            let value = ctx.data_back(format!("source_{curve}"))?;
            ctx.set_data_fwd(curve.clone(), value);
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Source", DataSpec::Pose)
            .add_input_time("source_time")
            .add_output_data("pose", DataSpec::Pose)
            .add_output_time();
        for curve in &self.curve_names {
            ctx.add_input_data(format!("source_{curve}"), DataSpec::F32)
                .add_output_data(curve.clone(), DataSpec::F32);
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
        registry.register::<InertializationPassthroughNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<InertializationPassthroughNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
