use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::GraphInputPin,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{io_env::GraphIoEnv, new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

use crate::animation_context::AnimationContextState;

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceLinkedInputPoseNode {
    pub input: String,
    pub output: String,
    pub context_curve_prefix: String,
    pub context_curve_names: Vec<String>,
}

impl Default for SourceLinkedInputPoseNode {
    fn default() -> Self {
        Self {
            input: "Pose".to_owned(),
            output: "Pose".to_owned(),
            context_curve_prefix: String::new(),
            context_curve_names: Vec::new(),
        }
    }
}

impl NodeLike for SourceLinkedInputPoseNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let pose = ctx
            .graph_context
            .io
            .get_data_back(
                GraphInputPin::Passthrough(self.input.clone()),
                ctx.graph_context.clone(),
            )?
            .into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(self.output.clone(), pose);
        if !self.context_curve_names.is_empty() {
            let mut state = AnimationContextState::default();
            for curve in &self.context_curve_names {
                let key = format!("{}{}", self.context_curve_prefix, curve);
                let value = ctx
                    .graph_context
                    .io
                    .get_data_back(GraphInputPin::Passthrough(key), ctx.graph_context.clone())?
                    .as_f32()?;
                state.curves.insert(curve.clone(), value);
            }
            *ctx.state_mut_or_else(AnimationContextState::default)? = state;
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data(self.output.clone(), DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Linked Input Pose".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_input_pose_exposes_only_its_source_visible_output() {
        let node = SourceLinkedInputPoseNode::default();
        assert_eq!(node.input, "Pose");
        assert_eq!(node.output, "Pose");
        assert!(node.context_curve_names.is_empty());
    }
}
