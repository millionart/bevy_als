use bevy::{platform::collections::HashMap, prelude::*};
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, NodeId, SourcePin, TargetPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    context::node_states::StateKey,
    context::{
        io_env::GraphIoEnv,
        new_context::{GraphContext, NodeContext},
    },
    duration_data::DurationData,
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
};

use crate::interruptible_fsm::InterruptibleFsmState;

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct AnimationContextInput {
    pub pose_input: String,
    pub time_input: String,
    pub curve_prefix: String,
    pub missing_curve_value: Option<f32>,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct AnimationContextInputAlias {
    pub child_input: String,
    pub outer_input: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct AnimationContextState {
    pub curves: HashMap<String, f32>,
}

pub struct AnimationContextIoEnv<'a> {
    pub parent_ctx: NodeContext<'a>,
    pub data_values: HashMap<String, DataValue>,
    pub curve_values: HashMap<String, f32>,
    pub input_aliases: HashMap<String, String>,
    pub time_update: Option<TimeUpdate>,
}

impl<'a> GraphIoEnv for AnimationContextIoEnv<'a> {
    fn get_data_back(
        &self,
        pin_id: GraphInputPin,
        ctx: GraphContext,
    ) -> Result<DataValue, GraphError> {
        match pin_id {
            GraphInputPin::Passthrough(pin_id) => {
                if let Some(value) = self.data_values.get(&pin_id) {
                    return Ok(value.clone());
                }
                self.curve_values
                    .get(&pin_id)
                    .copied()
                    .map(DataValue::F32)
                    .ok_or_else(|| {
                        GraphError::MissingGraphInputData(GraphInputPin::Passthrough(
                            pin_id.clone(),
                        ))
                    })
                    .or_else(|_| {
                        let parent_pin = self.input_aliases.get(&pin_id).cloned().unwrap_or(pin_id);
                        self.parent_ctx
                            .clone()
                            .with_state_key(ctx.state_key)
                            .data_back(parent_pin)
                    })
            }
            other => Err(GraphError::InvalidGraphInputPinType(other)),
        }
    }

    fn get_duration_back(
        &self,
        pin_id: GraphInputPin,
        ctx: GraphContext,
    ) -> Result<DurationData, GraphError> {
        match pin_id {
            GraphInputPin::Passthrough(pin_id) => {
                let parent_pin = self.input_aliases.get(&pin_id).cloned().unwrap_or(pin_id);
                self.parent_ctx
                    .clone()
                    .with_state_key(ctx.state_key)
                    .duration_back(parent_pin)
            }
            other => Err(GraphError::InvalidGraphInputPinType(other)),
        }
    }

    fn get_time_fwd(&self, ctx: GraphContext) -> Result<TimeUpdate, GraphError> {
        if let Some(time_update) = &self.time_update {
            return Ok(time_update.clone());
        }
        self.parent_ctx
            .clone()
            .with_state_key(ctx.state_key)
            .time_update_fwd()
    }
}

pub fn context_time_update(ctx: &NodeContext) -> Result<TimeUpdate, GraphError> {
    if let Some(time_update) = ctx
        .graph_context
        .context()
        .query_output_time
        .get(ctx.graph_context.state_key)
    {
        return Ok(time_update);
    }
    ctx.graph_context.io.get_time_fwd(ctx.graph_context.clone())
}

pub fn player_animation_context_state<'a>(
    player: &'a AnimationGraphPlayer,
    node_id: NodeId,
) -> Option<&'a AnimationContextState> {
    player
        .get_context_arena()?
        .get_toplevel()
        .node_states
        .get::<AnimationContextState>(node_id, StateKey::Default)
        .ok()
}

pub fn is_context_curve_input(
    input_name: &str,
    data_spec: &DataSpec,
    context_inputs: &[AnimationContextInput],
) -> bool {
    *data_spec == DataSpec::F32
        && context_inputs.iter().any(|input| {
            !input.curve_prefix.is_empty() && input_name.starts_with(&input.curve_prefix)
        })
}

pub fn source_node_for_pose_input(
    ctx: &NodeContext,
    pose_input: &str,
) -> Result<NodeId, GraphError> {
    let target = TargetPin::NodeData(ctx.node_id, pose_input.to_owned());
    let source = ctx
        .graph_context
        .graph
        .edges_inverted
        .get(&target)
        .ok_or(GraphError::MissingEdgeToTarget(target))?;
    match source {
        SourcePin::NodeData(node_id, _) => Ok(node_id.clone()),
        _ => Err(GraphError::MissingEdgeToTarget(TargetPin::NodeData(
            ctx.node_id,
            pose_input.to_owned(),
        ))),
    }
}

pub fn pose_input_context_state(
    ctx: &NodeContext,
    pose_input: &str,
) -> Option<AnimationContextState> {
    let source_node = source_node_for_pose_input(ctx, pose_input).ok()?;
    node_context_state(&ctx.graph_context, source_node)
}

fn node_context_state(ctx: &GraphContext, node: NodeId) -> Option<AnimationContextState> {
    if let Ok(state) = ctx
        .node_states()
        .get::<AnimationContextState>(node.clone(), ctx.state_key)
    {
        return Some(state.clone());
    }
    ctx.node_states()
        .get::<InterruptibleFsmState>(node, ctx.state_key)
        .ok()
        .map(|state| state.context.clone())
}

pub fn graph_output_context_state(
    graph: &AnimationGraph,
    ctx: &GraphContext,
    output: &str,
) -> Option<AnimationContextState> {
    let source = graph
        .edges_inverted
        .get(&TargetPin::OutputData(output.to_owned()))?;
    let SourcePin::NodeData(node, _) = source else {
        return None;
    };
    node_context_state(ctx, node.clone())
}

pub fn merge_missing_context_curves(
    target: &mut AnimationContextState,
    source: &AnimationContextState,
) {
    for (name, value) in &source.curves {
        target.curves.entry(name.clone()).or_insert(*value);
    }
}

pub fn select_context_pose_output(
    pose_output: &mut Option<String>,
    output_name: &str,
    data_spec: &DataSpec,
) {
    if *data_spec == DataSpec::Pose && pose_output.is_none() {
        *pose_output = Some(output_name.to_owned());
    }
}

fn source_curve_value(
    ctx: &NodeContext,
    source_node: NodeId,
    curve_name: &str,
    default: Option<f32>,
) -> Result<f32, GraphError> {
    let state_key = ctx.graph_context.state_key;
    if let Some(state) = node_context_state(
        &ctx.graph_context.clone().with_state_key(state_key),
        source_node.clone(),
    ) && let Some(value) = state.curves.get(curve_name)
    {
        return Ok(*value);
    }
    ctx.graph_context
        .node_caches()
        .get_output_data(source_node.clone(), state_key, curve_name.to_owned())
        .ok_or(GraphError::OutputMissing {
            graph: ctx.graph_context.context_id,
            node: source_node,
            pin: curve_name.to_owned(),
        })
        .and_then(|value| value.as_f32())
        .or_else(|error| default.ok_or(error))
}

pub fn resolve_context_curves(
    ctx: &NodeContext,
    child_graph: &AnimationGraph,
    context_inputs: &[AnimationContextInput],
) -> Result<HashMap<String, f32>, GraphError> {
    let mut curves = HashMap::default();
    for input in context_inputs {
        ctx.data_back(input.pose_input.clone())?;
        let source_node = source_node_for_pose_input(ctx, &input.pose_input)?;
        for (child_input, data_spec) in child_graph.io_spec.iter_input_data() {
            let GraphInputPin::Passthrough(child_input) = child_input else {
                continue;
            };
            if *data_spec != DataSpec::F32 || !child_input.starts_with(&input.curve_prefix) {
                continue;
            }
            let curve_name = child_input[input.curve_prefix.len()..].to_owned();
            let value = source_curve_value(
                ctx,
                source_node.clone(),
                &curve_name,
                input.missing_curve_value,
            )?;
            if curves.insert(child_input.clone(), value).is_some() {
                return Err(GraphError::MismatchedDataType(
                    "unique animation context curve input".to_owned(),
                    child_input.clone(),
                ));
            }
        }
    }
    Ok(curves)
}

pub fn resolve_named_context_curves(
    ctx: &NodeContext,
    pose_input: &str,
    curve_names: &[String],
    default: Option<f32>,
) -> Result<HashMap<String, f32>, GraphError> {
    ctx.data_back(pose_input.to_owned())?;
    let source_node = source_node_for_pose_input(ctx, pose_input)?;
    let mut curves = HashMap::default();
    for curve_name in curve_names {
        let value = source_curve_value(ctx, source_node.clone(), curve_name, default)?;
        curves.insert(curve_name.clone(), value);
    }
    Ok(curves)
}

pub fn capture_context_curve(
    state: &mut AnimationContextState,
    output_name: &str,
    value: &DataValue,
) -> Result<(), GraphError> {
    if let DataValue::F32(value) = value {
        state.curves.insert(output_name.to_owned(), *value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_curve_prefix_hides_only_declared_float_inputs() {
        let inputs = vec![AnimationContextInput {
            curve_prefix: "source_".to_owned(),
            ..default()
        }];
        assert!(is_context_curve_input(
            "source_PoseMoving",
            &DataSpec::F32,
            &inputs
        ));
        assert!(!is_context_curve_input(
            "source_PoseMoving",
            &DataSpec::Pose,
            &inputs
        ));
        assert!(!is_context_curve_input(
            "layer_index",
            &DataSpec::F32,
            &inputs
        ));
    }

    #[test]
    fn captured_curve_state_replaces_existing_value() {
        let mut state = AnimationContextState::default();
        capture_context_curve(&mut state, "PoseMoving", &DataValue::F32(0.25)).unwrap();
        capture_context_curve(&mut state, "PoseMoving", &DataValue::F32(0.75)).unwrap();
        assert_eq!(state.curves.get("PoseMoving"), Some(&0.75));
    }

    #[test]
    fn merged_output_context_preserves_explicit_output_curves() {
        let mut target = AnimationContextState {
            curves: HashMap::from([("PoseGrounded".to_owned(), 1.0)]),
        };
        let source = AnimationContextState {
            curves: HashMap::from([
                ("PoseGrounded".to_owned(), 0.0),
                ("PoseMoving".to_owned(), 1.0),
            ]),
        };
        merge_missing_context_curves(&mut target, &source);
        assert_eq!(target.curves.get("PoseGrounded"), Some(&1.0));
        assert_eq!(target.curves.get("PoseMoving"), Some(&1.0));
    }

    #[test]
    fn pose_output_selection_uses_type_not_serialized_pin_case() {
        let mut output = None;
        select_context_pose_output(&mut output, "Pose", &DataSpec::Pose);
        select_context_pose_output(&mut output, "OtherPose", &DataSpec::Pose);
        select_context_pose_output(&mut output, "PoseStanding", &DataSpec::F32);
        assert_eq!(output, Some("Pose".to_owned()));
    }
}
