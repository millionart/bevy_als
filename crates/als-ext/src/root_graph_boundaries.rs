use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

const SOURCE_POSE: &str = "source_pose";
const SOURCE_TIME: &str = "source_time";
const OUT_POSE: &str = "pose";

fn pose_spec(mut ctx: SpecContext) -> Result<(), GraphError> {
    ctx.add_input_data(SOURCE_POSE, DataSpec::Pose)
        .add_input_time(SOURCE_TIME)
        .add_output_data(OUT_POSE, DataSpec::Pose)
        .add_output_time();
    Ok(())
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct ControlRigBoundaryNode {
    pub rig_class: String,
    pub alpha_enabled: bool,
    pub transfer_input_pose: bool,
    pub transfer_input_curves: bool,
    pub transfer_pose_in_global_space: bool,
    pub curve_names: Vec<String>,
}

impl Default for ControlRigBoundaryNode {
    fn default() -> Self {
        Self {
            rig_class: "CR_Als".to_owned(),
            alpha_enabled: true,
            transfer_input_pose: true,
            transfer_input_curves: true,
            transfer_pose_in_global_space: false,
            curve_names: default(),
        }
    }
}

impl NodeLike for ControlRigBoundaryNode {
    fn display_name(&self) -> String {
        format!("ALS Control Rig {}", self.rig_class)
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back(SOURCE_TIME)?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        ctx.set_time_update_back(SOURCE_TIME, update);
        let pose = ctx.data_back(SOURCE_POSE)?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(OUT_POSE, pose);
        for curve in &self.curve_names {
            let value = ctx.data_back(curve.clone())?.as_f32()?;
            ctx.set_data_fwd(curve.clone(), value);
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        for curve in &self.curve_names {
            ctx.add_input_data(curve.clone(), DataSpec::F32)
                .add_output_data(curve.clone(), DataSpec::F32);
        }
        pose_spec(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_rig_boundary_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<ControlRigBoundaryNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<ControlRigBoundaryNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
