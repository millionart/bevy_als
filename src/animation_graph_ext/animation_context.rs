use bevy::{platform::collections::HashMap, prelude::*};
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, GraphInputPin, NodeId, SourcePin, TargetPin, TimeUpdate},
    animation_graph_player::AnimationGraphPlayer,
    context::node_states::{NodeStates, StateKey},
    context::{
        io_env::GraphIoEnv,
        new_context::{GraphContext, NodeContext},
    },
    duration_data::DurationData,
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
};

use crate::extensions::inertialization_passthrough::SourceInertializationState;
use crate::extensions::interruptible_fsm::InterruptibleFsmState;
use crate::extensions::source_pose_nodes::{
    SourceBlendState, SourceGameplayTagBlendState, SourceTwoWayBlendState,
};
use crate::extensions::source_sequence_player::SourceSequencePlayerState;

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
pub struct SourceRelevantAsset {
    pub duration: f32,
    pub position: f32,
    pub previous_position: f32,
    pub has_previous_position: bool,
    pub delta_seconds: f32,
    pub playback_delta_seconds: f32,
    pub looping: bool,
    pub weight: f32,
}

#[derive(Reflect, Clone, Debug, Default)]
pub struct SourceAssetSample {
    pub identity: String,
    pub notify_instance_id: u64,
    pub clip: Handle<bevy_animation_graph::core::animation_clip::GraphClip>,
    pub clock: SourceRelevantAsset,
    pub sync_group: String,
    pub sync_identity: Option<(bevy_animation_graph::core::context::graph_context_arena::GraphContextId, NodeId)>,
    pub montage: bool,
    pub interrupted: bool,
    pub slot_key: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct SourceInertializationPart {
    pub duration: f32,
    pub profile: Option<crate::extensions::SourceBlendProfile>,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct SourceInertializationRequest {
    pub frame: u64,
    pub duration: f32,
    pub parts: Vec<SourceInertializationPart>,
    pub cached_pose_origins: Vec<SourceCachedPoseRequestOrigin>,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct SourceCachedPoseRequestOrigin {
    pub context_id: bevy_animation_graph::core::context::graph_context_arena::GraphContextId,
    pub node_id: NodeId,
    pub state_key: StateKey,
    pub owner: bool,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct AnimationContextState {
    pub curves: crate::extensions::SourceCurveValues,
    pub relevant_asset: Option<SourceRelevantAsset>,
    pub curves_are_authoritative: bool,
    pub sync_players: Vec<crate::extensions::sync_group_phase::SourceSyncPlayerTick>,
    pub asset_samples: Vec<SourceAssetSample>,
    pub inertialization_request: Option<SourceInertializationRequest>,
    pub skipped_cached_pose_request: Option<SourceInertializationRequest>,
    pub(crate) slot_has_multiple_montages: bool,
    pub(crate) cached_pose_request_route: Option<SourceCachedPoseRequestRoute>,
}

#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SourceCachedPoseRequestRoute {
    pub frame: u64,
    pub caller: (bevy_animation_graph::core::context::graph_context_arena::GraphContextId, NodeId, StateKey),
    pub forward: bool,
}

#[derive(Reflect, Clone, Debug, Default)]
struct InputPoseContexts(HashMap<GraphInputPin, AnimationContextState>);

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
        mut ctx: GraphContext,
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
                        let parent_pin = self
                            .input_aliases
                            .get(&pin_id)
                            .cloned()
                            .unwrap_or(pin_id.clone());
                        let parent = self.parent_ctx.clone().with_state_key(ctx.state_key);
                        let value = parent.data_back(parent_pin.clone())?;
                        if matches!(value, DataValue::Pose(_)) {
                            write_linked_input_context(
                                &mut ctx,
                                GraphInputPin::Passthrough(pin_id.clone()),
                                pose_input_context_state(&parent, &parent_pin).unwrap_or_default(),
                            )?;
                        }
                        Ok(value)
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

pub(crate) fn write_linked_input_context(
    ctx: &mut GraphContext,
    pin: GraphInputPin,
    context: AnimationContextState,
) -> Result<(), GraphError> {
    let state_key = ctx.state_key;
    ctx.context_mut()
        .node_states
        .get_mut_or_insert_with(NodeId::default(), state_key, InputPoseContexts::default)?
        .0
        .insert(pin, context);
    Ok(())
}

pub(crate) fn linked_input_context(
    ctx: &GraphContext,
    pin: &GraphInputPin,
) -> AnimationContextState {
    ctx.node_states()
        .get::<InputPoseContexts>(NodeId::default(), ctx.state_key)
        .ok()
        .and_then(|state| state.0.get(pin).cloned())
        .unwrap_or_default()
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
    let states = &player.get_context_arena()?.get_toplevel().node_states;
    node_context_state(states, StateKey::Default, node_id)
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
    match ctx
        .graph_context
        .graph
        .edges_inverted
        .get(&TargetPin::NodeData(ctx.node_id, pose_input.to_owned()))?
    {
        SourcePin::NodeData(node, _) => node_context_state(ctx.graph_context.node_states(), ctx.graph_context.state_key, *node).cloned(),
        SourcePin::InputData(pin) => Some(linked_input_context(&ctx.graph_context, pin)),
        _ => None,
    }
}

pub fn node_context_state(states: &NodeStates, key: StateKey, node: NodeId) -> Option<&AnimationContextState> {
    if let Ok(state) = states.get::<crate::extensions::layered_graph::LayeredGraphState>(node, key) {
        return Some(&state.context);
    }
    if let Ok(state) = states.get::<crate::extensions::root_graph_boundaries::ControlRigNodeState>(node, key) {
        return Some(&state.context);
    }
    if let Ok(state) = states
        .get::<crate::extensions::normalized_blendspace::NormalizedBlendSpaceState>(node, key)
    {
        return Some(&state.context);
    }
    if let Ok(state) = states
        .get::<AnimationContextState>(node.clone(), key)
    {
        return Some(state);
    }
    if let Ok(state) = states
        .get::<SourceInertializationState>(node.clone(), key)
    {
        return Some(&state.context);
    }
    if let Ok(state) = states
        .get::<crate::extensions::inertialization::InertializationState>(node, key)
    {
        return Some(&state.context);
    }
    if let Ok(state) = states
        .get::<SourceSequencePlayerState>(node.clone(), key)
    {
        return Some(&state.context);
    }
    if let Ok(state) = states
        .get::<SourceTwoWayBlendState>(node.clone(), key)
    {
        return Some(&state.context);
    }
    if let Ok(state) = states
        .get::<SourceGameplayTagBlendState>(node.clone(), key)
    {
        return Some(&state.context);
    }
    if let Ok(state) = states
        .get::<SourceBlendState>(node.clone(), key)
    {
        return Some(&state.context);
    }
    if let Ok(state) = states.get::<crate::extensions::source_stop_nodes::SourceLayeredBoneBlendState>(node, key) {
        return Some(&state.context);
    }
    states
        .get::<InterruptibleFsmState>(node, key)
        .ok()
        .map(|state| &state.context)
}

pub fn graph_output_context_state(
    graph: &AnimationGraph,
    ctx: &GraphContext,
    output: &str,
) -> Option<AnimationContextState> {
    let source = graph
        .edges_inverted
        .get(&TargetPin::OutputData(output.to_owned()))?;
    match source {
        SourcePin::NodeData(node, _) => node_context_state(ctx.node_states(), ctx.state_key, *node).cloned(),
        SourcePin::InputData(pin) => Some(linked_input_context(ctx, pin)),
        _ => None,
    }
}

pub fn merge_missing_context_curves(
    target: &mut AnimationContextState,
    source: &AnimationContextState,
) {
    for (name, value) in &source.curves {
        target.curves.entry(name.clone()).or_insert(*value);
    }
    if target.relevant_asset.is_none() {
        target.relevant_asset = source.relevant_asset.clone();
    }
    target.curves_are_authoritative |= source.curves_are_authoritative;
    merge_sync_players(&mut target.sync_players, &source.sync_players, 1.0);
    merge_asset_samples(&mut target.asset_samples, &source.asset_samples, 1.0);
    merge_inertialization_request(&mut target.inertialization_request, source.inertialization_request.as_ref());
    merge_inertialization_request(&mut target.skipped_cached_pose_request, source.skipped_cached_pose_request.as_ref());
}

pub(crate) fn merge_inertialization_request(
    target: &mut Option<SourceInertializationRequest>,
    source: Option<&SourceInertializationRequest>,
) {
    let Some(source) = source else { return; };
    match target {
        Some(current) if current.frame == source.frame => {
            for origin in &source.cached_pose_origins {
                if !current.cached_pose_origins.contains(origin) {
                    current.cached_pose_origins.push(origin.clone());
                }
            }
            if current.parts.is_empty() {
                current.parts.push(SourceInertializationPart { duration: current.duration, ..default() });
            }
            if source.parts.is_empty() {
                current.parts.push(SourceInertializationPart { duration: source.duration, ..default() });
            } else {
                current.parts.extend(source.parts.iter().cloned());
            }
            current.duration = current.duration.min(source.duration);
        }
        Some(current) if current.frame > source.frame => {}
        _ => *target = Some(source.clone()),
    }
}

pub fn merge_sync_players(
    target: &mut Vec<crate::extensions::sync_group_phase::SourceSyncPlayerTick>,
    source: &[crate::extensions::sync_group_phase::SourceSyncPlayerTick],
    weight: f32,
) {
    for player in source {
        let weight = player.weight * weight.max(0.0);
        if let Some(existing) = target
            .iter_mut()
            .find(|existing| existing.identity == player.identity)
        {
            existing.weight = existing.weight.max(weight);
            existing.has_been_full_weight |= player.has_been_full_weight;
            existing.requested_inertialization |= player.requested_inertialization;
        } else {
            let mut player = player.clone();
            player.weight = weight;
            target.push(player);
        }
    }
}

pub fn merge_linked_input_context(
    target: &mut AnimationContextState,
    source: &AnimationContextState,
) {
    merge_inertialization_request(&mut target.inertialization_request, source.inertialization_request.as_ref());
    merge_inertialization_request(&mut target.skipped_cached_pose_request, source.skipped_cached_pose_request.as_ref());
    if !target.curves_are_authoritative {
        for (name, value) in &source.curves {
            target.curves.entry(name.clone()).or_insert(*value);
        }
        if target.relevant_asset.is_none() {
            target.relevant_asset = source.relevant_asset.clone();
        }
        target.curves_are_authoritative |= source.curves_are_authoritative;
        merge_sync_players(&mut target.sync_players, &source.sync_players, 1.0);
    }
}

pub fn merge_asset_samples(target: &mut Vec<SourceAssetSample>, source: &[SourceAssetSample], weight: f32) {
    for sample in source {
        if weight <= 1e-5 { continue; }
        let weight = sample.clock.weight * if sample.montage { 1.0 } else { weight.max(0.0) };
        if weight <= 1e-5 { continue; }
        if let Some(existing) = target.iter_mut().find(|existing| existing.identity == sample.identity) {
            existing.clock.weight = existing.clock.weight.max(weight);
        } else {
            let mut sample = sample.clone();
            sample.clock.weight = weight;
            target.push(sample);
        }
    }
}

pub fn select_relevant_asset(
    candidates: impl IntoIterator<Item = (Option<SourceRelevantAsset>, f32)>,
) -> Option<SourceRelevantAsset> {
    let mut selected = None;
    for (candidate, weight) in candidates {
        let Some(mut candidate) = candidate else {
            continue;
        };
        candidate.weight *= weight.max(0.0);
        if selected
            .as_ref()
            .is_none_or(|current: &SourceRelevantAsset| candidate.weight > current.weight)
        {
            selected = Some(candidate);
        }
    }
    selected
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
        ctx.graph_context.node_states(),
        state_key,
        source_node,
    ) && let Some(value) = state.curves.get(&crate::extensions::SourceCurveName::from(curve_name))
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
        state.curves.insert(output_name.into(), *value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_merge_retains_updated_zero_weight_and_uses_the_largest_cache_request() {
        use bevy_animation_graph::core::context::graph_context_arena::GraphContextId;
        use crate::extensions::sync_group_phase::SourceSyncPlayerTick;

        let tick = |index| SourceSyncPlayerTick {
            identity: (GraphContextId::default(), bevy::asset::uuid::Uuid::from_u128(index).into()),
            weight: 1.0, ..default()
        };
        let zero = tick(1);
        let small = tick(2);
        let mut merged = Vec::new();
        merge_sync_players(&mut merged, std::slice::from_ref(&zero), 0.0);
        merge_sync_players(&mut merged, std::slice::from_ref(&small), 1e-6);
        merge_sync_players(&mut merged, std::slice::from_ref(&small), 0.0);
        merge_sync_players(&mut merged, std::slice::from_ref(&small), 8e-6);
        assert_eq!(merged.iter().map(|tick| (tick.identity, tick.weight)).collect::<Vec<_>>(),
            [(zero.identity, 0.0), (small.identity, 8e-6)]);
    }

    #[test]
    fn source_asset_samples_preserve_notify_weights_and_cache_scope() {
        let sample = |identity: &str, montage| AnimationContextState {
            asset_samples: vec![SourceAssetSample {
                identity: identity.to_owned(), montage,
                clock: SourceRelevantAsset { weight: 0.8, ..default() }, ..default()
            }], ..default()
        };
        let blended = crate::extensions::source_pose_nodes::blend_animation_contexts([
            (sample("cached", false), 0.25), (sample("cached", false), 0.75),
            (sample("montage", true), 0.1), (sample("unrelated", false), 0.0),
        ]);
        assert_eq!(blended.asset_samples.len(), 2);
        assert!((blended.asset_samples[0].clock.weight - 0.6).abs() < 1e-6);
        assert_eq!(blended.asset_samples[1].clock.weight, 0.8);
        let independent = crate::extensions::source_pose_nodes::blend_animation_contexts([
            (sample("first instance", true), 1.0), (sample("second instance", true), 1.0),
        ]);
        assert_eq!(independent.asset_samples.len(), 2);
        let mut authoritative = AnimationContextState { curves_are_authoritative: true, ..default() };
        merge_linked_input_context(&mut authoritative, &blended);
        assert!(authoritative.asset_samples.is_empty());
        assert!(authoritative.curves.is_empty());
    }

    #[test]
    fn context_curve_lookup_borrows_current_or_previous_node_state() {
        let node = NodeId::default();
        let key = StateKey::Default;
        for rig in [false, true] {
            let mut states = NodeStates::default();
            let context = AnimationContextState {
                curves: HashMap::from([("PoseMoving".into(), 0.75)]),
                ..default()
            };
            if rig {
                states.get_mut_or_insert_with(node, key, || {
                    let mut state = crate::extensions::root_graph_boundaries::ControlRigNodeState::default();
                    state.context = context;
                    state
                }).unwrap();
            } else {
                states.get_mut_or_insert_with(node, key, || context).unwrap();
            }
            for _ in 0..2 {
                let read = node_context_state(&states, key, node).unwrap();
                let stored = if rig {
                    &states.get::<crate::extensions::root_graph_boundaries::ControlRigNodeState>(node, key).unwrap().context
                } else {
                    states.get::<AnimationContextState>(node, key).unwrap()
                };
                assert!(std::ptr::eq(&read.curves, &stored.curves), "curve lookup copied node state");
                assert_eq!(read.curves[&crate::extensions::SourceCurveName::from("PoseMoving")], 0.75);
                states.next_frame();
            }
        }
    }

    #[test]
    fn inertial_requests_merge_by_frame_and_shortest_duration() {
        let mut request = None;
        for (frame, duration) in [(2, 0.4), (2, 0.2), (1, 0.1), (2, 0.3)] {
            merge_inertialization_request(&mut request, Some(&SourceInertializationRequest { frame, duration, ..default() }));
        }
        assert_eq!(request.as_ref().map(|value| (value.frame, value.duration)), Some((2, 0.2)));
        assert_eq!(request.as_ref().unwrap().parts.iter().map(|part| part.duration).collect::<Vec<_>>(), [0.4, 0.2, 0.3]);
        merge_inertialization_request(&mut request, Some(&SourceInertializationRequest { frame: 3, duration: 0.5, ..default() }));
        assert_eq!(request.as_ref().map(|value| (value.frame, value.duration)), Some((3, 0.5)));
    }

    #[test]
    fn inertial_requests_keep_explicit_profile_and_unprofiled_duration() {
        let profile = crate::extensions::SourceBlendProfile {
            profile_mode: crate::extensions::BlendProfileMode::TimeFactor,
            bone_scales: vec![crate::extensions::BoneBlendScale { bone: "foot_l".into(), scale: 2.0 }],
        };
        let mut request = None;
        merge_inertialization_request(&mut request, Some(&SourceInertializationRequest {
            frame: 2, duration: 0.2,
            parts: vec![SourceInertializationPart { duration: 0.2, profile: Some(profile.clone()) }],
            ..default()
        }));
        merge_inertialization_request(&mut request, Some(&SourceInertializationRequest {
            frame: 2, duration: 0.3, ..default()
        }));
        let request = request.unwrap();
        assert_eq!(request.duration, 0.2);
        assert_eq!(request.parts.len(), 2);
        assert_eq!(request.parts[0].profile, Some(profile));
        assert_eq!(request.parts[1].duration, 0.3);
        assert!(request.parts[1].profile.is_none());
    }

    #[test]
    fn skipped_cached_pose_requests_do_not_become_direct_inertial_requests_when_merged() {
        let mut context = AnimationContextState {
            skipped_cached_pose_request: Some(SourceInertializationRequest {
                frame: 2, duration: 0.3, ..default()
            }),
            ..default()
        };
        let source = AnimationContextState {
            inertialization_request: Some(SourceInertializationRequest {
                frame: 2, duration: 0.2, ..default()
            }),
            ..default()
        };
        merge_missing_context_curves(&mut context, &source);
        assert_eq!(context.inertialization_request.unwrap().duration, 0.2);
        assert_eq!(context.skipped_cached_pose_request.unwrap().duration, 0.3);
    }

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
        assert_eq!(state.curves.get(&crate::extensions::SourceCurveName::from("PoseMoving")), Some(&0.75));
    }

    #[test]
    fn merged_output_context_preserves_explicit_output_curves() {
        let mut target = AnimationContextState {
            curves: HashMap::from([("PoseGrounded".into(), 1.0)]),
            ..default()
        };
        let source = AnimationContextState {
            curves: HashMap::from([
                ("PoseGrounded".into(), 0.0),
                ("PoseMoving".into(), 1.0),
            ]),
            ..default()
        };
        merge_missing_context_curves(&mut target, &source);
        assert_eq!(target.curves.get(&crate::extensions::SourceCurveName::from("PoseGrounded")), Some(&1.0));
        assert_eq!(target.curves.get(&crate::extensions::SourceCurveName::from("PoseMoving")), Some(&1.0));
    }

    #[test]
    fn authoritative_empty_child_context_does_not_inherit_linked_input_curves() {
        let mut child = AnimationContextState {
            curves_are_authoritative: true,
            ..default()
        };
        let input = AnimationContextState {
            curves: HashMap::from([("FootLeftIk".into(), 1.0)]),
            ..default()
        };
        merge_linked_input_context(&mut child, &input);
        assert!(!child.curves.contains_key(&crate::extensions::SourceCurveName::from("FootLeftIk")));
    }

    #[test]
    fn relevant_asset_selection_uses_the_highest_weighted_player() {
        let first = SourceRelevantAsset {
            duration: 1.0,
            weight: 1.0,
            ..default()
        };
        let second = SourceRelevantAsset {
            duration: 2.0,
            weight: 1.0,
            ..default()
        };
        assert_eq!(
            select_relevant_asset([(Some(first.clone()), 0.25), (Some(second.clone()), 0.75),])
                .unwrap()
                .duration,
            2.0
        );
        assert_eq!(
            select_relevant_asset([(Some(first), 1.0), (Some(second), 1.0)])
                .unwrap()
                .duration,
            1.0
        );
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
