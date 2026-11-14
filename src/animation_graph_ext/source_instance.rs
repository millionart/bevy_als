use bevy::{platform::collections::HashMap, prelude::*, reflect::ReflectMut};
use bevy_animation_graph::core::{
    animation_graph::NodeId,
    animation_graph_player::AnimationGraphPlayer,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{
        graph_context_arena::{GraphContextArena, GraphContextId},
        new_context::NodeContext,
        node_states::StateKey,
        spec_context::SpecContext,
    },
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
};

use crate::extensions::animation_context::{AnimationContextState, pose_input_context_state};
use crate::extensions::interruptible_fsm::SourceFsmStateWeight;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Reflect, Clone, Debug)]
pub struct SourceInstanceProperties {
    pub instance_id: u64,
    pub values: HashMap<String, DataValue>,
    pub write_frames: HashMap<String, u64>,
    pub(crate) slot_notify_frames: HashMap<(String, String), u64>,
    call_frames: HashMap<(GraphContextId, NodeId), u64>,
    machine_weights: HashMap<NodeId, SourceMachineWeights>,
}

impl Default for SourceInstanceProperties {
    fn default() -> Self {
        static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(0);
        Self {
            instance_id: NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed) + 1,
            values: default(),
            write_frames: default(),
            slot_notify_frames: default(),
            call_frames: default(),
            machine_weights: default(),
        }
    }
}

#[derive(Reflect, Clone, Debug, Default)]
struct SourceMachineWeights {
    frame: Option<u64>,
    context: Option<GraphContextId>,
    current_machine: f32,
    current_states: Vec<SourceFsmStateWeight>,
    previous_machine: f32,
    previous_states: Vec<SourceFsmStateWeight>,
}

fn machine_weights<'a>(
    ctx: &'a NodeContext,
    node: NodeId,
) -> Result<&'a mut SourceMachineWeights, GraphError> {
    let frame = ctx.graph_context.frame_index();
    let owner = linked_instance_context(ctx, ctx.graph_context.context_id).ok_or_else(|| {
        GraphError::UnknownErrorNode {
            graph: ctx.graph_context.context_id,
            node: ctx.node_id,
            error: "Machine weights have no owning animation instance".into(),
        }
    })?;
    let properties = ctx
        .graph_context
        .context_arena
        .get_mut()
        .get_context_mut(owner)
        .unwrap()
        .node_states
        .get_mut_or_insert_with(
            source_notify_instance_node(),
            StateKey::Default,
            SourceInstanceProperties::default,
        )?;
    let state = properties.machine_weights.entry(node).or_default();
    if state.frame != Some(frame) {
        let continuous = state
            .frame
            .is_some_and(|previous| previous.wrapping_add(1) == frame);
        state.previous_machine = if continuous {
            state.current_machine
        } else {
            0.0
        };
        state.previous_states = if continuous {
            std::mem::take(&mut state.current_states)
        } else {
            Vec::new()
        };
        state.current_machine = 0.0;
        state.current_states.clear();
        state.frame = Some(frame);
    }
    Ok(state)
}

pub(crate) fn begin_source_machine_update(
    ctx: &NodeContext,
    weight: f32,
) -> Result<(), GraphError> {
    if ctx.graph_context.state_key == StateKey::Default {
        let state = machine_weights(ctx, ctx.node_id)?;
        state.context = Some(ctx.graph_context.context_id);
        state.current_machine = weight;
    }
    Ok(())
}

pub(crate) fn record_source_state_weights(
    ctx: &NodeContext,
    values: Vec<SourceFsmStateWeight>,
) -> Result<(), GraphError> {
    if ctx.graph_context.state_key == StateKey::Default {
        machine_weights(ctx, ctx.node_id)?.current_states = values;
    }
    Ok(())
}

pub(crate) fn source_machine_context(
    ctx: &NodeContext,
    node: NodeId,
) -> Result<Option<GraphContextId>, GraphError> {
    Ok(machine_weights(ctx, node)?.context)
}

pub(crate) fn source_machine_weight(ctx: &NodeContext, node: NodeId) -> Result<f32, GraphError> {
    Ok(machine_weights(ctx, node)?.previous_machine)
}

pub(crate) fn source_state_weight(
    ctx: &NodeContext,
    node: NodeId,
    state_id: &str,
) -> Result<f32, GraphError> {
    Ok(machine_weights(ctx, node)?
        .previous_states
        .iter()
        .find(|value| value.state_id == state_id)
        .map_or(0.0, |value| value.value))
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

pub(crate) fn linked_instance_context(
    ctx: &NodeContext,
    mut context_id: GraphContextId,
) -> Option<GraphContextId> {
    let arena = ctx.graph_context.context_arena.get_ref();
    arena.get_context(context_id)?;
    while let Some(parent) = arena.get_parent_context(context_id) {
        let graph_id = arena.get_context(parent.ctx_id)?.get_graph_id();
        let graph = ctx
            .graph_context
            .resources
            .animation_graph_assets
            .get(graph_id)?;
        if graph.nodes.get(&parent.node_id).is_some_and(|node| {
            node.try_inner_downcast_ref::<crate::extensions::LayeredGraphNode>()
                .is_some()
        }) {
            return Some(context_id);
        }
        context_id = parent.ctx_id;
    }
    Some(context_id)
}

pub(crate) fn source_notify_instance_id(ctx: &NodeContext) -> Result<u64, GraphError> {
    source_instance_properties(ctx)?;
    let owner = linked_instance_context(ctx, ctx.graph_context.context_id).ok_or_else(|| {
        GraphError::UnknownErrorNode {
            graph: ctx.graph_context.context_id,
            node: ctx.node_id,
            error: "Notify queue has no owning animation instance".to_owned(),
        }
    })?;
    Ok(ctx
        .graph_context
        .context_arena
        .get_mut()
        .get_context_mut(owner)
        .unwrap()
        .node_states
        .get_mut_or_insert_with(
            source_notify_instance_node(),
            StateKey::Default,
            SourceInstanceProperties::default,
        )?
        .instance_id)
}

pub(crate) fn source_notify_instance_node() -> NodeId {
    bevy::asset::uuid::Uuid::from_u128(0x73c93986_c1b9_4711_ab60_e628fa6b36e6).into()
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
    Ok(previous.is_none_or(|previous| frame.wrapping_sub(previous) % u64::from(u16::MAX) > 1))
}

pub(crate) fn reset_source_call_relevancy(player: &mut AnimationGraphPlayer) {
    let ReflectMut::Struct(fields) = player.reflect_mut() else {
        panic!("Animation graph player must reflect as a struct");
    };
    let arena = fields
        .field_mut("context_arena")
        .and_then(|field| field.try_downcast_mut::<Option<GraphContextArena>>())
        .expect("Animation graph player must expose its context arena");
    let Some(arena) = arena.as_mut() else {
        return;
    };
    let states = &mut arena.get_toplevel_mut().node_states;
    let node = NodeId::default();
    if states
        .get::<SourceInstanceProperties>(node, StateKey::Default)
        .is_ok_and(|state| !state.call_frames.is_empty())
    {
        states
            .get_mut_or_insert_with(node, StateKey::Default, SourceInstanceProperties::default)
            .expect("source instance property state must keep its type")
            .call_frames
            .clear();
    }
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
