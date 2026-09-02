use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::Pose,
};

fn pose_time_update(pose: &Pose) -> TimeUpdate {
    TimeUpdate::Absolute(pose.timestamp)
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct PoseTimestampNode {
    pub input: String,
    pub output: String,
}

impl Default for PoseTimestampNode {
    fn default() -> Self {
        Self {
            input: "Source".to_owned(),
            output: "Pose".to_owned(),
        }
    }
}

impl NodeLike for PoseTimestampNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let pose = ctx.data_back(self.input.clone())?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(self.output.clone(), pose);
        Ok(())
    }

    fn try_get_time(&self, ctx: NodeContext, _pin: String) -> Result<TimeUpdate, GraphError> {
        let pose = ctx.data_back(self.input.clone())?.into_pose()?;
        Ok(pose_time_update(&pose))
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(self.input.clone(), DataSpec::Pose)
            .add_output_data(self.output.clone(), DataSpec::Pose)
            .add_output_time();
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Pose Timestamp".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_and_output_pin_names_are_stable() {
        let node = PoseTimestampNode::default();
        assert_eq!(node.input, "Source");
        assert_eq!(node.output, "Pose");
    }

    #[test]
    fn output_time_is_derived_from_the_evaluated_pose_timestamp() {
        let pose = Pose {
            timestamp: 2.5,
            ..default()
        };
        match pose_time_update(&pose) {
            TimeUpdate::Absolute(value) => assert_eq!(value, 2.5),
            other => panic!("unexpected pose time update: {other:?}"),
        }
    }
}
