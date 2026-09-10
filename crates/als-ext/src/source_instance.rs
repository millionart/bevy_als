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
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Reflect, Clone, Debug)]
pub struct SourceInstanceProperties {
    pub instance_id: u64,
    pub values: HashMap<String, DataValue>,
    pub write_frames: HashMap<String, u64>,
    call_frames: HashMap<(GraphContextId, NodeId), u64>,
}

impl Default for SourceInstanceProperties {
    fn default() -> Self {
        static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(0);
        Self {
            instance_id: NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed) + 1,
            values: default(),
            write_frames: default(),
            call_frames: default(),
        }
    }
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
    let arena = ctx.graph_context.context_arena.get_ref();
    let state = arena
        .get_toplevel()
        .node_states
        .get::<SourceInstanceProperties>(NodeId::default(), StateKey::Default)
        .ok();
    source_property_value(
        state,
        ctx.graph_context.frame_index(),
        key,
        ctx.graph_context
            .global_input_data
            .get(&format!("source_frame_input.{key}")),
    )
    .or_else(|| ctx.graph_context.global_input_data.get(key))
    .cloned()
}

fn source_property_value<'a>(
    state: Option<&'a SourceInstanceProperties>,
    frame: u64,
    key: &str,
    input: Option<&'a DataValue>,
) -> Option<&'a DataValue> {
    if state.is_none_or(|state| state.write_frames.get(key) != Some(&frame)) && input.is_some() {
        return input;
    }
    state.and_then(|state| state.values.get(key)).or(input)
}

pub fn set_source_frame_input(
    player: &mut bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    key: &str,
    value: impl Into<DataValue>,
) {
    player.set_global_input_data(format!("source_frame_input.{key}"), value);
}

pub fn source_player_property<'a>(
    player: &'a bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer,
    key: &str,
) -> Option<&'a DataValue> {
    let arena = player.get_context_arena();
    let state = arena.and_then(|arena| {
        arena
            .get_toplevel()
            .node_states
            .get::<SourceInstanceProperties>(NodeId::default(), StateKey::Default)
            .ok()
    });
    source_property_value(
        state,
        arena.map_or(0, |arena| arena.frame_index()),
        key,
        player.get_global_input_data(format!("source_frame_input.{key}")),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_input_can_request_another_jump_after_the_previous_request_was_consumed() {
        let key = "in_air_state.b_jumped";
        let input = DataValue::Bool(true);
        let mut state = SourceInstanceProperties::default();
        assert_eq!(
            source_property_value(Some(&state), 1, key, Some(&input)),
            Some(&input)
        );
        state.values.insert(key.to_owned(), DataValue::Bool(false));
        state.write_frames.insert(key.to_owned(), 1);
        assert_eq!(
            source_property_value(Some(&state), 1, key, Some(&input)),
            Some(&DataValue::Bool(false))
        );
        assert_eq!(
            source_property_value(Some(&state), 2, key, Some(&input)),
            Some(&input)
        );
        assert_eq!(
            source_property_value(Some(&state), 2, key, None),
            Some(&DataValue::Bool(false))
        );
    }
}
