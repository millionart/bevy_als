use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

use crate::slot_node::{blend_slot_pose, slot_blend_weight};

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceSlotNode {
    pub slot_name: String,
    pub group_name: String,
    pub pose_key: String,
    pub weight_key: String,
    pub layer_weight_key: String,
}

impl Default for SourceSlotNode {
    fn default() -> Self {
        Self {
            slot_name: "DefaultSlot".to_owned(),
            group_name: "DefaultGroup".to_owned(),
            pose_key: "slot_pose".to_owned(),
            weight_key: "slot_weight".to_owned(),
            layer_weight_key: "layer_slot_weight".to_owned(),
        }
    }
}

impl NodeLike for SourceSlotNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let base = ctx.data_back("Source")?.into_pose()?;
        let slot_weight = slot_blend_weight(
            ctx.graph_context
                .global_input_data
                .get(&self.weight_key)
                .map(|value| value.as_f32())
                .transpose()?,
        );
        let layer_weight = slot_blend_weight(
            ctx.graph_context
                .global_input_data
                .get(&self.layer_weight_key)
                .map(|value| value.as_f32())
                .transpose()?,
        );
        let overlay = if slot_weight > 0.0 && layer_weight > 0.0 {
            ctx.graph_context
                .global_input_data
                .get(&self.pose_key)
                .cloned()
                .map(|value| value.into_pose())
                .transpose()?
        } else {
            None
        };
        let slot_pose = blend_slot_pose(base.clone(), overlay.as_ref(), Some(slot_weight));
        let pose = blend_slot_pose(
            base,
            overlay.as_ref().map(|_| &slot_pose),
            Some(layer_weight),
        );
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Source", DataSpec::Pose)
            .add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        format!("ALS Source Slot {}", self.slot_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_slot_preserves_source_visible_pin_names() {
        let node = SourceSlotNode::default();
        assert_eq!(node.slot_name, "DefaultSlot");
        assert_eq!(node.group_name, "DefaultGroup");
    }
}
