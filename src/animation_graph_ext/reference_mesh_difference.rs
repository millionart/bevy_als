use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

use crate::extensions::mesh_space_difference::make_mesh_space_difference;
use crate::extensions::{AnimationContextState, reference_local_difference::precise_reference_difference};

pub const REFERENCE_POSE: &str = "reference_pose";
pub const REFERENCE_TIME: &str = "reference_time";
pub const TARGET_POSE: &str = "target_pose";
pub const TARGET_TIME: &str = "target_time";
pub const OUT_POSE: &str = "pose";

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct ReferenceMeshDifferenceNode;

impl NodeLike for ReferenceMeshDifferenceNode {
    fn display_name(&self) -> String {
        "ALS Reference Mesh Difference".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back(TARGET_TIME)?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        ctx.set_time_update_back(REFERENCE_TIME, update.clone());
        ctx.set_time_update_back(TARGET_TIME, update);
        let mut reference = ctx.data_back(REFERENCE_POSE)?.into_pose()?;
        let target = ctx.data_back(TARGET_POSE)?.into_pose()?;
        if ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&reference.skeleton)
            .is_none()
        {
            reference.skeleton = target.skeleton.clone();
        }
        let skeleton = ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&reference.skeleton)
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        let mut difference = make_mesh_space_difference(&reference, &target, skeleton);
        let source_pose = precise_reference_difference(&ctx, &reference, &target, skeleton, true);
        if let Some(source) = &source_pose {
            source.project_into(&mut difference);
        }
        difference.timestamp = target.timestamp;
        ctx.set_time(difference.timestamp);
        ctx.set_data_fwd(OUT_POSE, difference);
        ctx.state_mut::<AnimationContextState>()?.source_pose = source_pose;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(REFERENCE_POSE, DataSpec::Pose)
            .add_input_time(REFERENCE_TIME)
            .add_input_data(TARGET_POSE, DataSpec::Pose)
            .add_input_time(TARGET_TIME)
            .add_output_data(OUT_POSE, DataSpec::Pose)
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
        registry.register::<ReferenceMeshDifferenceNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<ReferenceMeshDifferenceNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
