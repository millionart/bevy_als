use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{
        new_context::{GraphContext, NodeContext},
        node_states::StateKey,
        spec_context::SpecContext,
    },
    edge_data::DataSpec,
    errors::GraphError,
    pose::Pose,
};

pub const CACHED_POSE_VALUE: &str = "__als_source_cached_pose";

fn saved_cache_key<'a>(cache: &'a SourceCachedPoseSaveNode, node_name: &'a str) -> &'a str {
    if cache.cache_key.is_empty() {
        node_name
    } else {
        cache.cache_key.as_str()
    }
}

fn cache_error(ctx: &NodeContext, message: String) -> GraphError {
    GraphError::UnknownErrorNode {
        graph: ctx.graph_context.context_id,
        node: ctx.node_id,
        error: message,
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

pub fn resolve_scoped_cached_pose(
    ctx: &mut NodeContext,
    cache_node: &str,
    cache_key: &str,
) -> Result<Pose, GraphError> {
    let requested_key = if cache_key.is_empty() {
        cache_node
    } else {
        cache_key
    };
    let contexts = ctx
        .graph_context
        .context_arena
        .get_ref()
        .iter_context_ids()
        .filter_map(|context_id| {
            ctx.graph_context
                .context_arena
                .get_ref()
                .get_context(context_id)
                .map(|state| (context_id, state.get_graph_id()))
        })
        .collect::<Vec<_>>();
    let (context_id, graph, cache) = contexts
        .into_iter()
        .find_map(|(context_id, graph_id)| {
            ctx.graph_context
                .resources
                .animation_graph_assets
                .get(graph_id)
                .and_then(|graph| {
                    graph
                        .nodes
                        .values()
                        .find(|node| {
                            node.try_inner_downcast_ref::<SourceCachedPoseSaveNode>()
                                .is_some_and(|cache| {
                                    saved_cache_key(cache, &node.name) == requested_key
                                })
                        })
                        .map(|cache| (context_id, graph, cache))
                })
        })
        .ok_or_else(|| {
            cache_error(
                ctx,
                format!("scoped cached pose node is missing: {cache_node}"),
            )
        })?;
    let cache_id = cache.id;
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
            format!("scoped cached pose is not prewarmed: {cache_node}"),
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
}

impl NodeLike for SourceScopedCachedPoseUseNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let pose = resolve_scoped_cached_pose(&mut ctx, &self.cache_node, &self.cache_key)?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
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
    }
}
