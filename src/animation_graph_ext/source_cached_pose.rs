use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, NodeId},
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
};

use crate::extensions::animation_context::{AnimationContextState, pose_input_context_state};
use crate::extensions::source_instance::linked_instance_context;

pub const CACHED_POSE_VALUE: &str = "__als_source_cached_pose";

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

fn cached_context_state(ctx: &NodeContext, cache_node: &str) -> Option<AnimationContextState> {
    let cache = ctx
        .graph_context
        .graph
        .nodes
        .values()
        .find(|node| node.name == cache_node)?;
    ctx.graph_context
        .node_states()
        .get::<AnimationContextState>(cache.id, ctx.graph_context.state_key)
        .ok()
        .cloned()
}

fn scoped_cached_context_state(
    ctx: &NodeContext,
    cache_node: &str,
    cache_key: &str,
) -> Option<AnimationContextState> {
    let requested_key = requested_cache_key(cache_node, cache_key);
    let (context_id, graph, cache_id) = scoped_cache_context(ctx, requested_key)?;
    let scoped_context = GraphContext {
        context_id,
        graph,
        ..ctx.graph_context.clone()
    }
    .with_state_key(StateKey::Default);
    scoped_context
        .node_states()
        .get::<AnimationContextState>(cache_id, StateKey::Default)
        .ok()
        .cloned()
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
}

impl NodeLike for SourceCachedPoseSaveNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let pose = ctx.data_back("Pose")?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(CACHED_POSE_VALUE, pose);
        if let Some(context) = pose_input_context_state(&ctx, "Pose") {
            *ctx.state_mut_or_else(AnimationContextState::default)? = context;
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
        let mut context = scoped_cached_context_state(&ctx, &self.cache_node, &self.cache_key)
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
        if let Some(context) = cached_context_state(&ctx, &self.cache_node) {
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
                instance_ctx.create_node_context(input_id).state_mut_or_else(AnimationContextState::default)
                    .unwrap().curves.insert("Scope".into(), expected);
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
                let sibling = instance_ctx.create_node_context(machine_id).create_child_context(state.0, state.1,
                    Some(bevy_animation_graph::core::state_machine::low_level::LowLevelStateId::HlState(default())));
                let mut sibling = sibling.create_node_context(use_id);
                assert_eq!(crate::extensions::source_instance::source_notify_instance_id(&sibling).unwrap(), notify_instance);
                assert_eq!(resolve_scoped_cached_pose(&mut sibling, "cache", "shared").unwrap().timestamp, expected);
            }
            let inputs = HashMap::new();
            let root_ctx = GraphContext::new(arena.get_toplevel_id(), root.1, &mut arena, &resources,
                &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
            let missing = root_ctx.create_node_context(missing).create_child_context(state.0, state.1, None);
            let mut missing = missing.create_node_context(use_id);
            assert!(prewarm_scoped_cached_pose(&mut missing, "cache", "shared").is_err());
            assert!(resolve_scoped_cached_pose(&mut missing, "cache", "shared").is_err());
            assert!(scoped_cached_context_state(&missing, "cache", "shared").is_none());
        }

        let mut graphs = Assets::<AnimationGraph>::default();
        let input = AnimationNode::new("input", GlobalPoseInputNode { key: "input".into(), output: "Pose".into() });
        let cache = AnimationNode::new("cache", SourceCachedPoseSaveNode { cache_key: "shared".into() });
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
