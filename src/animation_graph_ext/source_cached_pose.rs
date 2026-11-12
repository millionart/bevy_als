use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, NodeId, SourcePin, TargetPin},
    animation_node::{NodeLike, ReflectNodeLike},
    context::{
        graph_context_arena::GraphContextId,
        new_context::{GraphContext, NodeContext},
        node_states::StateKey,
        spec_context::SpecContext,
    },
    edge_data::DataSpec,
    errors::GraphError,
    pose::Pose,
    state_machine::low_level::LowLevelStateId,
};

use crate::extensions::animation_context::{
    AnimationContextState, SourceCachedPoseRequestOrigin, SourceCachedPoseRequestRoute, SourceInertializationRequest,
    pose_input_context_state,
};
use crate::extensions::source_instance::linked_instance_context;

pub const CACHED_POSE_VALUE: &str = "__als_source_cached_pose";

struct CachedUpdateRequest<'a> {
    owner: (GraphContextId, NodeId),
    ctx: NodeContext<'a>,
    name: String,
    key: String,
    scoped: bool,
    weight: f32,
    forward: bool,
}

fn collect_cached_requests<'a>(
    ctx: &NodeContext<'a>,
    source: &SourcePin,
    weight: f32,
    forward: bool,
    requests: &mut Vec<CachedUpdateRequest<'a>>,
    path: &mut std::collections::HashSet<NodeId>,
) -> Result<(), GraphError> {
    let SourcePin::NodeData(id, _) = source else { return Ok(()); };
    let graph = ctx.graph_context.graph;
    let node = &graph.nodes[id];
    if !path.insert(*id) { return Err(cache_error(ctx, "Cached update request has a pose cycle".into())); }
    let request = if let Some(value) = node.try_inner_downcast_ref::<SourceScopedCachedPoseUseNode>() {
        Some((value.cache_node.as_str(), value.cache_key.as_str(), true))
    } else {
        node.try_inner_downcast_ref::<SourceCachedPoseUseNode>().map(|value| (value.cache_node.as_str(), "", false))
    };
    if let Some((name, key, scoped)) = request {
        let owner = if scoped {
            let requested = requested_cache_key(name, key);
            let local = graph.nodes.values().find(|node| node.try_inner_downcast_ref::<SourceCachedPoseSaveNode>()
                .is_some_and(|cache| saved_cache_key(cache, &node.name) == requested));
            local.map(|node| (ctx.graph_context.context_id, node.id))
                .or_else(|| scoped_cache_context(ctx, requested).map(|(context, _, node)| (context, node)))
        } else { graph.nodes.values().find(|node| node.name == name).map(|node| (ctx.graph_context.context_id, node.id)) }
            .ok_or_else(|| cache_error(ctx, format!("Cached update target is missing: {name}")))?;
        if let Some(request) = requests.iter_mut().find(|request| request.owner == owner) {
            if weight > request.weight {
                *request = CachedUpdateRequest { owner, ctx: ctx.graph_context.create_node_context(*id), name: name.into(), key: key.into(), scoped, weight, forward };
            }
        } else { requests.push(CachedUpdateRequest { owner, ctx: ctx.graph_context.create_node_context(*id), name: name.into(), key: key.into(), scoped, weight, forward }); }
    } else {
        let forward = node.try_inner_downcast_ref::<crate::extensions::InertializationPassthroughNode>()
            .map_or(forward, |node| node.forward_requests_through_skipped_cached_poses);
        let node_ctx = ctx.graph_context.create_node_context(*id);
        let weights = crate::extensions::source_pose_nodes::cached_update_branch_weights(&node_ctx)?;
        let spec = node.new_spec(bevy_animation_graph::core::context::spec_context::SpecResources {
            graph_assets: &ctx.graph_context.resources.animation_graph_assets,
            fsm_assets: &ctx.graph_context.resources.state_machine_assets,
        })?;
        for input in spec.sorted_inputs() {
            let bevy_animation_graph::core::context::spec_context::NodeInput::Data(pin, DataSpec::Pose) = input else { continue; };
            let factor = weights.as_ref().map_or(1.0, |weights| weights.iter().find(|entry| entry.0 == pin).map_or(0.0, |entry| entry.1));
            if factor <= 1e-5 { continue; }
            if let Some(source) = graph.edges_inverted.get(&TargetPin::NodeData(*id, pin.clone())) {
                collect_cached_requests(ctx, source, weight * factor, forward, requests, path)?;
            }
        }
    }
    path.remove(id);
    Ok(())
}

fn update_cached_requests(requests: Vec<CachedUpdateRequest>) -> Result<(), GraphError> {
    for request in requests {
        crate::extensions::interruptible_fsm::with_machine_weight(request.weight, || {
            let mut ctx = request.ctx;
            let state_key = if request.scoped { StateKey::Default } else { ctx.graph_context.state_key };
            let started = ctx.graph_context.context_arena.get_ref().get_context(request.owner.0)
                .is_some_and(|context| context.node_caches.is_update_started(request.owner.1, state_key));
            if request.scoped { prewarm_scoped_cached_pose(&mut ctx, &request.name, &request.key)?; }
            else { resolve_cached_pose(&mut ctx, &request.name)?; }
            if !started {
                let route = SourceCachedPoseRequestRoute { frame: ctx.graph_context.frame_index(),
                    caller: (ctx.graph_context.context_id, ctx.node_id, ctx.graph_context.state_key), forward: request.forward };
                ctx.graph_context.context_arena.get_mut().get_context_mut(request.owner.0).unwrap()
                    .node_states.get_mut_or_insert_with(request.owner.1, state_key, AnimationContextState::default)?
                    .cached_pose_request_route = Some(route);
            }
            Ok(())
        })?;
    }
    Ok(())
}

pub(crate) fn prewarm_linked_pose_caches(ctx: &NodeContext, input: &str, order: &[String]) -> Result<(), GraphError> {
    if order.is_empty() { return Ok(()); }
    let graph = ctx.graph_context.graph;
    let Some(source) = graph.edges_inverted.get(&TargetPin::NodeData(ctx.node_id, input.to_owned())) else {
        return Ok(());
    };
    let mut requests = Vec::new();
    collect_cached_requests(ctx, source, 1.0, false, &mut requests, &mut default())?;
    let mut linked = Vec::new();
    for key in order {
        let Some(index) = requests.iter().position(|request| {
            request.owner.0 == ctx.graph_context.context_id && graph.nodes.get(&request.owner.1)
                .and_then(|node| node.try_inner_downcast_ref::<SourceCachedPoseSaveNode>()
                    .map(|save| saved_cache_key(save, &node.name))) == Some(key.as_str())
        }) else { continue; };
        let request = requests.remove(index);
        let Some(source) = graph.edges_inverted.get(&TargetPin::NodeData(request.owner.1, "Pose".to_owned())) else {
            continue;
        };
        if matches!(source, SourcePin::NodeData(id, _) if graph.nodes[id]
            .try_inner_downcast_ref::<crate::extensions::SourceLinkedInputPoseNode>().is_some())
        {
            linked.push(request);
        } else {
            collect_cached_requests(ctx, source, request.weight, request.forward, &mut requests, &mut default())?;
        }
    }
    if linked.len() > 1 {
        update_cached_requests(linked)?;
    }
    Ok(())
}

pub(crate) fn prewarm_input_cached_poses(ctx: &NodeContext, inputs: impl IntoIterator<Item = (String, f32)>) -> Result<(), GraphError> {
    if !ctx.graph_context.graph.nodes.values().any(|node| node.try_inner_downcast_ref::<SourceScopedCachedPoseUseNode>().is_some()
        || node.try_inner_downcast_ref::<SourceCachedPoseUseNode>().is_some()) { return Ok(()); }
    let mut requests = Vec::new();
    for (pin, weight) in inputs {
        if weight <= 1e-5 { continue; }
        if let Some(source) = ctx.graph_context.graph.edges_inverted.get(&TargetPin::NodeData(ctx.node_id, pin)) {
            collect_cached_requests(ctx, source, weight, false, &mut requests, &mut default())?;
        }
    }
    update_cached_requests(requests)
}

pub(crate) fn prewarm_graph_cached_poses<'a>(contexts: impl IntoIterator<Item = (NodeContext<'a>, f32)>) -> Result<(), GraphError> {
    let mut requests = Vec::new();
    for (ctx, weight) in contexts {
        if !ctx.graph_context.graph.nodes.values().any(|node| node.try_inner_downcast_ref::<SourceScopedCachedPoseUseNode>().is_some()
            || node.try_inner_downcast_ref::<SourceCachedPoseUseNode>().is_some()) { continue; }
        for (pin, kind) in ctx.graph_context.graph.io_spec.iter_output_data() {
            if *kind != DataSpec::Pose { continue; }
            if let Some(source) = ctx.graph_context.graph.edges_inverted.get(&TargetPin::OutputData(pin.clone())) {
                collect_cached_requests(&ctx, source, weight, false, &mut requests, &mut default())?;
            }
        }
    }
    update_cached_requests(requests)
}

fn saved_cache_key<'a>(cache: &'a SourceCachedPoseSaveNode, node_name: &'a str) -> &'a str {
    if cache.cache_key.is_empty() {
        node_name
    } else {
        cache.cache_key.as_str()
    }
}

fn requested_cache_key<'a>(cache_node: &'a str, cache_key: &'a str) -> &'a str {
    if cache_key.is_empty() {
        cache_node
    } else {
        cache_key
    }
}

fn cache_error(ctx: &NodeContext, message: String) -> GraphError {
    GraphError::UnknownErrorNode {
        graph: ctx.graph_context.context_id,
        node: ctx.node_id,
        error: message,
    }
}

fn use_cached_context_state(
    ctx: &mut NodeContext,
    context_id: GraphContextId,
    cache_id: NodeId,
    state_key: StateKey,
) -> Option<AnimationContextState> {
    let frame = ctx.graph_context.frame_index();
    let caller = (ctx.graph_context.context_id, ctx.node_id, ctx.graph_context.state_key);
    let state = ctx.graph_context.context_arena.get_mut().get_context_mut(context_id)?
        .node_states.get_mut_or_insert_with(cache_id, state_key, AnimationContextState::default).ok()?;
    let mut context = state.clone();
    let mut request = context.inertialization_request.take().filter(|request| request.frame == frame);
    let skipped = context.skipped_cached_pose_request.take().filter(|request| request.frame == frame);
    if request.is_some() || skipped.is_some() {
        if state.cached_pose_request_route.is_none_or(|route| route.frame != frame) {
            state.cached_pose_request_route = Some(SourceCachedPoseRequestRoute { frame, caller, forward: false });
        }
        let owner = state.cached_pose_request_route.is_some_and(|route| route.caller == caller);
        let forward = state.cached_pose_request_route.is_some_and(|route| route.forward);
        if !owner {
            if !forward {
                request = None;
            }
            crate::extensions::animation_context::merge_inertialization_request(&mut request, skipped.as_ref());
            if let Some(request) = request.as_mut() {
                for origin in &mut request.cached_pose_origins {
                    origin.owner = false;
                }
            }
        }
        if let Some(request) = request.as_mut() {
            request.cached_pose_origins.push(SourceCachedPoseRequestOrigin {
                context_id, node_id: cache_id, state_key, owner,
            });
        }
    }
    context.inertialization_request = request;
    Some(context)
}

pub(crate) fn forward_cached_pose_requests(
    ctx: &mut NodeContext,
    request: &SourceInertializationRequest,
    forward: bool,
) {
    for origin in request.cached_pose_origins.iter().filter(|origin| origin.owner) {
        if let Some(context) = ctx.graph_context.context_arena.get_mut().get_context_mut(origin.context_id)
            && let Ok(state) = context.node_states.get_mut_or_insert_with(
                origin.node_id, origin.state_key, AnimationContextState::default,
            )
            && let Some(route) = state.cached_pose_request_route.as_mut().filter(|route| route.frame == request.frame)
        {
            route.forward = forward;
        }
    }
}

fn cached_context_state(ctx: &mut NodeContext, cache_node: &str) -> Option<AnimationContextState> {
    let cache = ctx
        .graph_context
        .graph
        .nodes
        .values()
        .find(|node| node.name == cache_node)?;
    use_cached_context_state(ctx, ctx.graph_context.context_id, cache.id, ctx.graph_context.state_key)
}

fn scoped_cached_context_state(
    ctx: &mut NodeContext,
    cache_node: &str,
    cache_key: &str,
) -> Option<AnimationContextState> {
    let requested_key = requested_cache_key(cache_node, cache_key);
    let (context_id, _, cache_id) = scoped_cache_context(ctx, requested_key)?;
    let mut context = use_cached_context_state(ctx, context_id, cache_id, StateKey::Default)?;
    if context.relevant_asset.is_some() && context_id != ctx.graph_context.context_id {
        let arena = ctx.graph_context.context_arena.get_ref();
        let parent = |id: &GraphContextId| arena.get_parent_context(*id).map(|parent| parent.ctx_id);
        let state_root = std::iter::successors(Some(ctx.graph_context.context_id), parent)
            .find(|id| arena.get_parent_context(*id).is_some_and(|parent|
                matches!(parent.state_id.as_ref(), Some(LowLevelStateId::HlState(_)))));
        if let Some(state_root) = state_root
            && !std::iter::successors(Some(context_id), parent).any(|id| id == state_root)
        {
            context.relevant_asset = None;
        }
    }
    Some(context)
}

fn scoped_cache_context<'a>(ctx: &NodeContext<'a>, key: &str) -> Option<(GraphContextId, &'a AnimationGraph, NodeId)> {
    let owner = linked_instance_context(ctx, ctx.graph_context.context_id)?;
    let arena = ctx.graph_context.context_arena.get_ref();
    arena.iter_context_ids().find_map(|context_id| {
        let graph_id = arena.get_context(context_id)?.get_graph_id();
        let graph = ctx.graph_context.resources.animation_graph_assets.get(graph_id)?;
        let cache = graph.nodes.values().find(|node| {
            node.try_inner_downcast_ref::<SourceCachedPoseSaveNode>()
                .is_some_and(|cache| saved_cache_key(cache, &node.name) == key)
        })?;
        (linked_instance_context(ctx, context_id) == Some(owner)).then_some((context_id, graph, cache.id))
    })
}

#[derive(Reflect, Clone, Debug, Default, PartialEq)]
#[reflect(Default)]
pub struct SourceContextDefault {
    pub curve: String,
    pub value: f32,
}

fn merge_context_defaults(state: &mut AnimationContextState, defaults: &[SourceContextDefault]) {
    for default in defaults {
        if !default.curve.is_empty() {
            state
                .curves
                .entry(default.curve.clone().into())
                .or_insert(default.value);
        }
    }
}

pub fn resolve_cached_pose(ctx: &mut NodeContext, cache_node: &str) -> Result<Pose, GraphError> {
    let graph = ctx.graph_context.graph;
    let cache = graph
        .nodes
        .values()
        .find(|node| node.name == cache_node)
        .ok_or_else(|| cache_error(ctx, format!("cached pose node is missing: {cache_node}")))?;
    let cache_id = cache.id;
    let key = ctx.graph_context.state_key;
    if !ctx
        .graph_context
        .node_caches()
        .is_update_started(cache_id, key)
    {
        ctx.graph_context
            .node_caches_mut()
            .mark_update_started(cache_id, key);
        cache.update(
            ctx.graph_context
                .create_node_context(cache_id)
                .with_debugging(cache.should_debug),
        )?;
        ctx.graph_context
            .node_caches_mut()
            .mark_updated(cache_id, key);
    }
    ctx.graph_context
        .node_caches()
        .get_output_data(cache_id, key, CACHED_POSE_VALUE.to_owned())
        .ok_or_else(|| cache_error(ctx, format!("cached pose is missing: {cache_node}")))?
        .into_pose()
}

pub(crate) fn prewarm_scoped_cached_pose(
    ctx: &mut NodeContext,
    cache_node: &str,
    cache_key: &str,
) -> Result<(), GraphError> {
    let requested_key = requested_cache_key(cache_node, cache_key);
    let local_cache = ctx.graph_context.graph.nodes.values().find_map(|node| {
        node.try_inner_downcast_ref::<SourceCachedPoseSaveNode>()
            .is_some_and(|value| saved_cache_key(value, &node.name) == requested_key)
            .then(|| node.name.clone())
    });
    if let Some(local_cache) = local_cache {
        resolve_cached_pose(ctx, &local_cache)?;
        return Ok(());
    }
    let (context_id, graph, cache_id) = scoped_cache_context(ctx, requested_key)
        .ok_or_else(|| {
            cache_error(
                ctx,
                format!("scoped cached pose node is missing: {cache_node}"),
            )
        })?;
    let cache_name = graph.nodes[&cache_id].name.clone();
    let scoped_context = GraphContext {
        context_id,
        graph,
        ..ctx.graph_context.clone()
    }
    .with_state_key(StateKey::Default);
    resolve_cached_pose(
        &mut scoped_context.create_node_context(cache_id),
        &cache_name,
    )?;
    Ok(())
}

pub fn resolve_scoped_cached_pose(
    ctx: &mut NodeContext,
    cache_node: &str,
    cache_key: &str,
) -> Result<Pose, GraphError> {
    let requested_key = requested_cache_key(cache_node, cache_key);
    let local_cache = ctx.graph_context.graph.nodes.values().find_map(|node| {
        node.try_inner_downcast_ref::<SourceCachedPoseSaveNode>()
            .is_some_and(|value| saved_cache_key(value, &node.name) == requested_key)
            .then(|| node.name.clone())
    });
    if let Some(local_cache) = local_cache {
        return resolve_cached_pose(ctx, &local_cache);
    }
    let (context_id, graph, cache_id) = scoped_cache_context(ctx, requested_key)
        .ok_or_else(|| {
            cache_error(
                ctx,
                format!("scoped cached pose node is missing: {cache_node}"),
            )
        })?;
    let scoped_context = GraphContext {
        context_id,
        graph,
        ..ctx.graph_context.clone()
    }
    .with_state_key(StateKey::Default);
    if !scoped_context
        .node_caches()
        .is_updated(cache_id, StateKey::Default)
    {
        return Err(cache_error(
            ctx,
            format!(
                "scoped cached pose is not prewarmed: {cache_node}; cache_context={context_id:?}"
            ),
        ));
    }
    scoped_context
        .node_caches()
        .get_output_data(cache_id, StateKey::Default, CACHED_POSE_VALUE.to_owned())
        .ok_or_else(|| cache_error(ctx, format!("scoped cached pose is missing: {cache_node}")))?
        .into_pose()
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceCachedPoseSaveNode {
    pub cache_key: String,
    #[reflect(default)]
    pub submission_order: Option<usize>,
}

impl NodeLike for SourceCachedPoseSaveNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let pose = ctx.data_back("Pose")?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(CACHED_POSE_VALUE, pose);
        if let Some(mut context) = pose_input_context_state(&ctx, "Pose") {
            if let Some(order) = self.submission_order {
                let owner = ctx.graph_context.context_id;
                for player in &mut context.sync_players {
                    if player.cached_submission_order.is_none_or(|(context, _)| context != owner) {
                        player.cached_submission_order = Some((owner, order));
                    }
                }
            }
            *ctx.state_mut_or_else(AnimationContextState::default)? = AnimationContextState {
                cached_pose_request_route: None, ..context
            };
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Cached Pose Save".to_owned()
    }
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceCachedPoseUseNode {
    pub cache_node: String,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceScopedCachedPoseUseNode {
    pub cache_node: String,
    pub cache_key: String,
    pub context_defaults: Vec<SourceContextDefault>,
}

impl NodeLike for SourceScopedCachedPoseUseNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let pose = resolve_scoped_cached_pose(&mut ctx, &self.cache_node, &self.cache_key)?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
        let mut context = scoped_cached_context_state(&mut ctx, &self.cache_node, &self.cache_key)
            .unwrap_or_default();
        merge_context_defaults(&mut context, &self.context_defaults);
        *ctx.state_mut_or_else(AnimationContextState::default)? = context;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Scoped Cached Pose Use".to_owned()
    }
}

impl NodeLike for SourceCachedPoseUseNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let pose = resolve_cached_pose(&mut ctx, &self.cache_node)?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
        if let Some(context) = cached_context_state(&mut ctx, &self.cache_node) {
            *ctx.state_mut_or_else(AnimationContextState::default)? = context;
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Cached Pose Use".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_update_request_cycle_returns_a_readable_error() {
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context_arena::GraphContextArena, system_resources::SystemResources},
            skeleton::Skeleton, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        fn query(In(handle): In<Handle<AnimationGraph>>, resources: SystemResources) -> GraphError {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let env = TestIoEnv::default();
            let entities = default();
            let inputs = default();
            let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources,
                &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
            prewarm_graph_cached_poses([(context.create_node_context(NodeId::default()), 1.0)]).unwrap_err()
        }
        let mut graph = AnimationGraph::new();
        let node = AnimationNode::new("cycle", crate::extensions::SourcePosePassthroughNode::default());
        graph.add_node(node.clone());
        graph.add_node(AnimationNode::new("unused cache", SourceCachedPoseUseNode::default()));
        graph.add_node_parameter_edge(node.id, "Pose", node.id, "Source");
        graph.add_output_data("Pose".into(), DataSpec::Pose);
        graph.add_output_data_edge(node.id, "Pose", "Pose");
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
        let error = world.run_system_cached_with(query, handle).unwrap();
        assert!(format!("{error:?}").contains("Cached update request has a pose cycle"));
    }

    #[test]
    fn scoped_cache_keeps_pose_and_curves_in_the_owning_linked_instance() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::{AnimationGraph, NodeId}, animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context_arena::GraphContextArena, system_resources::SystemResources},
            edge_data::DataValue, skeleton::Skeleton, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        use crate::extensions::{GlobalPoseInputNode, LayeredGraphNode, interruptible_fsm::InterruptibleFsmNode};

        fn query(In((handles, ids)): In<([Handle<AnimationGraph>; 3], [NodeId; 6])>, resources: SystemResources) {
            let [root, shared, state] = handles.map(|handle| (handle.id(), resources.animation_graph_assets.get(&handle).unwrap()));
            let [first, second, missing, input_id, machine_id, use_id] = ids;
            let mut arena = GraphContextArena::new(root.0);
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            for (instance, expected) in [(first, 1.0), (second, 2.0)] {
                let inputs = [("input".to_owned(), DataValue::Pose(Pose { timestamp: expected, ..default() }))].into();
                let root_ctx = GraphContext::new(arena.get_toplevel_id(), root.1, &mut arena, &resources,
                    &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                let mut instance_ctx = root_ctx.create_node_context(instance).create_child_context(shared.0, shared.1, None);
                crate::extensions::animation_context::write_linked_input_context(&mut instance_ctx,
                    bevy_animation_graph::core::animation_graph::GraphInputPin::Passthrough("external".into()), default()).unwrap();
                let mut input_state = instance_ctx.create_node_context(input_id);
                let frame = input_state.graph_context.frame_index();
                let input_state = input_state.state_mut_or_else(AnimationContextState::default).unwrap();
                input_state.curves.insert("Scope".into(), expected);
                input_state.relevant_asset = Some(crate::extensions::SourceRelevantAsset {
                    duration: expected, weight: 1.0, ..default()
                });
                input_state.inertialization_request = Some(SourceInertializationRequest {
                    frame, duration: 0.2, ..default()
                });
                let state_ctx = instance_ctx.create_node_context(machine_id).create_child_context(state.0, state.1, None);
                let mut use_ctx = state_ctx.create_node_context(use_id);
                let notify_instance = crate::extensions::source_instance::source_notify_instance_id(&use_ctx).unwrap();
                prewarm_scoped_cached_pose(&mut use_ctx, "cache", "shared").unwrap();
                state.1.nodes[&use_id].update(use_ctx.clone()).unwrap();
                let pose = state_ctx.node_caches().get_output_data(use_id, StateKey::Default, "Pose".to_owned())
                    .unwrap().into_pose().unwrap();
                assert_eq!(pose.timestamp, expected, "Cache pose crossed linked instances");
                assert_eq!(use_ctx.state::<AnimationContextState>().unwrap().curves[&crate::extensions::SourceCurveName::from("Scope")], expected,
                    "Cache curves crossed linked instances");
                assert_eq!(use_ctx.state::<AnimationContextState>().unwrap().relevant_asset.as_ref().unwrap().duration, expected);
                let request = use_ctx.state::<AnimationContextState>().unwrap()
                    .inertialization_request.as_ref().unwrap().clone();
                assert_eq!(request.cached_pose_origins.len(), 1);
                assert!(request.cached_pose_origins[0].owner);
                let sibling = instance_ctx.create_node_context(machine_id).create_child_context(state.0, state.1,
                    Some(bevy_animation_graph::core::state_machine::low_level::LowLevelStateId::HlState(default())));
                let mut sibling = sibling.create_node_context(use_id);
                let nested = sibling.graph_context.create_node_context(machine_id).create_child_context(state.0, state.1,
                    Some(bevy_animation_graph::core::state_machine::low_level::LowLevelStateId::HlState(default())));
                let mut nested = nested.create_node_context(use_id);
                assert_eq!(crate::extensions::source_instance::source_notify_instance_id(&nested).unwrap(), notify_instance);
                assert_eq!(resolve_scoped_cached_pose(&mut nested, "cache", "shared").unwrap().timestamp, expected);
                assert_eq!(crate::extensions::source_instance::source_notify_instance_id(&sibling).unwrap(), notify_instance);
                assert_eq!(resolve_scoped_cached_pose(&mut sibling, "cache", "shared").unwrap().timestamp, expected);
                state.1.nodes[&use_id].update(sibling.clone()).unwrap();
                let state_context = sibling.state::<AnimationContextState>().unwrap();
                assert!(state_context.inertialization_request.is_none());
                assert!(state_context.relevant_asset.is_none());
                assert_eq!(state_context.curves[&crate::extensions::SourceCurveName::from("Scope")], expected);
                assert!(use_ctx.state::<AnimationContextState>().unwrap().relevant_asset.is_some());
                forward_cached_pose_requests(&mut use_ctx, &request, true);
                state.1.nodes[&use_id].update(sibling.clone()).unwrap();
                assert!(sibling.state::<AnimationContextState>().unwrap().inertialization_request.is_some());
                state_ctx.context_arena.get_mut().next_frame();
                let frame = state_ctx.frame_index();
                instance_ctx.create_node_context(input_id).state_mut_or_else(AnimationContextState::default).unwrap()
                    .inertialization_request = Some(SourceInertializationRequest { frame, duration: 0.3, ..default() });
                prewarm_graph_cached_poses([(use_ctx.clone(), 0.25), (sibling.clone(), 0.75)]).unwrap();
                prewarm_graph_cached_poses([(use_ctx.clone(), 0.25)]).unwrap();
                state.1.nodes[&use_id].update(use_ctx.clone()).unwrap();
                state.1.nodes[&use_id].update(sibling.clone()).unwrap();
                assert!(use_ctx.state::<AnimationContextState>().unwrap().inertialization_request.is_none(),
                    "The lower-weight state claimed the cached request");
                let request = sibling.state::<AnimationContextState>().unwrap().inertialization_request.as_ref().unwrap();
                assert!(request.cached_pose_origins.last().unwrap().owner);
                assert_eq!(request.duration, 0.3);
                let route = instance_ctx.context_arena.get_ref().get_context(instance_ctx.context_id).unwrap()
                    .node_states.get::<AnimationContextState>(shared.1.nodes.values().find(|node|
                        node.try_inner_downcast_ref::<SourceCachedPoseSaveNode>().is_some()).unwrap().id, StateKey::Default)
                    .unwrap().cached_pose_request_route.unwrap();
                assert_eq!(route.caller, (sibling.graph_context.context_id, use_id, StateKey::Default));
            }
            let inputs = HashMap::new();
            let root_ctx = GraphContext::new(arena.get_toplevel_id(), root.1, &mut arena, &resources,
                &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
            let missing = root_ctx.create_node_context(missing).create_child_context(state.0, state.1, None);
            let mut missing = missing.create_node_context(use_id);
            assert!(prewarm_scoped_cached_pose(&mut missing, "cache", "shared").is_err());
            assert!(resolve_scoped_cached_pose(&mut missing, "cache", "shared").is_err());
            assert!(scoped_cached_context_state(&mut missing, "cache", "shared").is_none());
        }

        let mut graphs = Assets::<AnimationGraph>::default();
        let input = AnimationNode::new("input", GlobalPoseInputNode { key: "input".into(), output: "Pose".into() });
        let cache = AnimationNode::new("cache", SourceCachedPoseSaveNode { cache_key: "shared".into(), ..default() });
        let machine = AnimationNode::new("machine", InterruptibleFsmNode::default());
        let mut shared = AnimationGraph::new();
        for node in [&input, &cache, &machine] { shared.add_node(node.clone()); }
        shared.add_node_parameter_edge(input.id, "Pose", cache.id, "Pose");
        let shared = graphs.add(shared);
        let use_cache = AnimationNode::new("use cache", SourceScopedCachedPoseUseNode {
            cache_node: "cache".into(), cache_key: "shared".into(), ..default()
        });
        let mut state = AnimationGraph::new();
        state.add_node(use_cache.clone());
        state.add_output_data("Pose".into(), DataSpec::Pose);
        state.add_output_data_edge(use_cache.id, "Pose", "Pose");
        let state = graphs.add(state);
        let first = AnimationNode::new("first instance", LayeredGraphNode::new(shared.clone()));
        let second = AnimationNode::new("second instance", LayeredGraphNode::new(shared.clone()));
        let missing = AnimationNode::new("missing cache instance", LayeredGraphNode::new(state.clone()));
        let mut root = AnimationGraph::new();
        root.add_node(first.clone());
        root.add_node(second.clone());
        root.add_node(missing.clone());
        let root = graphs.add(root);
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        world.run_system_cached_with(query, ([root, shared, state], [first.id, second.id, missing.id, input.id, machine.id, use_cache.id])).unwrap();
    }

    #[test]
    fn source_cache_nodes_use_source_visible_pose_pin_names() {
        assert_eq!(CACHED_POSE_VALUE, "__als_source_cached_pose");
        assert!(
            SourceCachedPoseSaveNode::default()
                .display_name()
                .contains("Save")
        );
        assert!(
            SourceCachedPoseUseNode::default()
                .display_name()
                .contains("Use")
        );
        assert!(
            SourceScopedCachedPoseUseNode::default()
                .display_name()
                .contains("Use")
        );
    }

    #[test]
    fn scoped_cache_key_overrides_only_the_internal_lookup_identity() {
        let cache = SourceCachedPoseSaveNode {
            cache_key: "ab_als_crouching:movement".to_owned(),
            ..default()
        };
        assert_eq!(
            saved_cache_key(&cache, "Save cached pose 'Movement'"),
            "ab_als_crouching:movement"
        );
        assert_eq!(
            saved_cache_key(
                &SourceCachedPoseSaveNode::default(),
                "Save cached pose 'Movement'"
            ),
            "Save cached pose 'Movement'"
        );
        assert_eq!(
            requested_cache_key("Save cached pose 'Movement'", "ue-cache:movement"),
            "ue-cache:movement"
        );
        assert_eq!(
            requested_cache_key("Save cached pose 'Movement'", ""),
            "Save cached pose 'Movement'"
        );
    }

    #[test]
    fn context_defaults_fill_missing_curves_without_overwriting_cached_curves() {
        let mut state = AnimationContextState::default();
        state.curves.insert("PoseStanding".into(), 1.0);
        merge_context_defaults(
            &mut state,
            &[
                SourceContextDefault {
                    curve: "PoseCrouching".to_owned(),
                    value: 0.0,
                },
                SourceContextDefault {
                    curve: "PoseStanding".to_owned(),
                    value: 0.0,
                },
            ],
        );
        assert_eq!(state.curves.get(&crate::extensions::SourceCurveName::from("PoseCrouching")), Some(&0.0));
        assert_eq!(state.curves.get(&crate::extensions::SourceCurveName::from("PoseStanding")), Some(&1.0));
    }
}
