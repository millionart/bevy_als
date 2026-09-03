use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

use crate::animation_context::{AnimationContextState, pose_input_context_state};

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourcePosePassthroughNode {
    pub input: String,
    pub output: String,
    pub input_time: String,
    pub output_time: bool,
    pub source_function: String,
    pub call_site: String,
}

impl Default for SourcePosePassthroughNode {
    fn default() -> Self {
        Self {
            input: "Source".to_owned(),
            output: "Pose".to_owned(),
            input_time: "source_time".to_owned(),
            output_time: true,
            source_function: default(),
            call_site: default(),
        }
    }
}

impl NodeLike for SourcePosePassthroughNode {
    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if self.output_time {
            let duration = ctx.duration_back(self.input_time.clone())?;
            ctx.set_duration_fwd(duration);
        } else {
            ctx.set_duration_fwd(None);
        }
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let pose = ctx.data_back(self.input.clone())?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(self.output.clone(), pose);
        if let Some(context) = pose_input_context_state(&ctx, &self.input) {
            *ctx.state_mut_or_else(AnimationContextState::default)? = context;
        }
        if self.output_time {
            let time_update = ctx.time_update_fwd()?;
            ctx.set_time_update_back(self.input_time.clone(), time_update);
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(self.input.clone(), DataSpec::Pose)
            .add_output_data(self.output.clone(), DataSpec::Pose);
        if self.output_time {
            ctx.add_input_time(self.input_time.clone())
                .add_output_time();
        }
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Pose Passthrough".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pin_names_match_source_pose_nodes() {
        let node = SourcePosePassthroughNode::default();
        assert_eq!(node.input, "Source");
        assert_eq!(node.output, "Pose");
        assert_eq!(node.input_time, "source_time");
        assert!(node.output_time);
        assert!(node.source_function.is_empty());
    }
}
