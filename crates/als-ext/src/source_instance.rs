use bevy::{platform::collections::HashMap, prelude::*};
use bevy_animation_graph::core::{
    animation_graph::NodeId,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{
        graph_context_arena::GraphContextId, new_context::NodeContext, node_states::StateKey,
        spec_context::SpecContext,
    },
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
};

use crate::animation_context::{AnimationContextState, pose_input_context_state};

#[derive(Reflect, Clone, Debug, Default)]
pub struct SourceInstanceProperties {
    pub values: HashMap<String, DataValue>,
    pub write_frames: HashMap<String, u64>,
    call_frames: HashMap<(GraphContextId, NodeId), u64>,
}

pub fn source_instance_properties<'a>(
    ctx: &'a NodeContext<'_>,
) -> Result<&'a mut SourceInstanceProperties, GraphError> {
    ctx.graph_context
        .context_arena
        .get_mut()
        .get_toplevel_mut()
        .node_states
        .get_mut_or_insert_with(
            NodeId::default(),
            StateKey::Default,
            SourceInstanceProperties::default,
        )
}

pub fn source_input_data(ctx: &NodeContext, key: &str) -> Option<DataValue> {
    ctx.graph_context
        .context_arena
        .get_ref()
        .get_toplevel()
        .node_states
        .get::<SourceInstanceProperties>(NodeId::default(), StateKey::Default)
        .ok()
        .and_then(|state| state.values.get(key))
        .or_else(|| ctx.graph_context.global_input_data.get(key))
        .cloned()
}

pub fn set_source_property(
    ctx: &NodeContext,
    key: &str,
    value: impl Into<DataValue>,
) -> Result<(), GraphError> {
    let frame = ctx.graph_context.frame_index();
    let state = source_instance_properties(ctx)?;
    state.values.insert(key.to_owned(), value.into());
    state.write_frames.insert(key.to_owned(), frame);
    Ok(())
}

pub fn source_call_became_relevant(ctx: &NodeContext) -> Result<bool, GraphError> {
    let frame = ctx.graph_context.frame_index();
    let previous = source_instance_properties(ctx)?
        .call_frames
        .insert((ctx.graph_context.context_id, ctx.node_id), frame);
    Ok(previous.is_none_or(|previous| frame > previous.saturating_add(1)))
}

#[derive(Reflect, Clone, Debug)]
pub struct SourcePropertyAssignment {
    pub property: String,
    pub value: DataValue,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourcePropertyWriteNode {
    pub assignments: Vec<SourcePropertyAssignment>,
    pub on_become_relevant: bool,
}

impl NodeLike for SourcePropertyWriteNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let became_relevant = source_call_became_relevant(&ctx)?;
        if !self.on_become_relevant || became_relevant {
            for assignment in &self.assignments {
                set_source_property(&ctx, &assignment.property, assignment.value.clone())?;
            }
        }
        let pose = ctx.data_back("Source")?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
        *ctx.state_mut_or_else(AnimationContextState::default)? =
            pose_input_context_state(&ctx, "Source").unwrap_or_default();
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Source", DataSpec::Pose)
            .add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "Source Property Write".to_owned()
    }
}
