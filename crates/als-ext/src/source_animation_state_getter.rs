use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{
        new_context::{GraphContext, NodeContext},
        node_states::StateKey,
    },
    edge_data::DataSpec,
    errors::GraphError,
};

use crate::interruptible_fsm::{InterruptibleFsmNode, InterruptibleFsmState};

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceAnimationStateGetterNode {
    pub source_function: String,
    pub fsm_node: String,
    pub state_id: String,
    pub output: String,
}

impl SourceAnimationStateGetterNode {
    fn error(ctx: &NodeContext, error: &str) -> GraphError {
        GraphError::UnknownErrorNode {
            graph: ctx.graph_context.context_id,
            node: ctx.node_id,
            error: error.to_owned(),
        }
    }

    fn target_state(&self, ctx: &NodeContext) -> Result<InterruptibleFsmState, GraphError> {
        if self.fsm_node.is_empty() {
            return Err(Self::error(
                ctx,
                "source animation getter machine is unavailable",
            ));
        }
        let matches = ctx
            .graph_context
            .context_arena
            .get_ref()
            .iter_context_ids()
            .filter_map(|context_id| {
                let context = ctx
                    .graph_context
                    .context_arena
                    .get_ref()
                    .get_context(context_id)?;
                let graph = ctx
                    .graph_context
                    .resources
                    .animation_graph_assets
                    .get(context.get_graph_id())?;
                let node = graph.nodes.values().find(|node| {
                    node.id.uuid().to_string() == self.fsm_node
                        && node
                            .try_inner_downcast_ref::<InterruptibleFsmNode>()
                            .is_some()
                })?;
                Some((context_id, node.id))
            })
            .collect::<Vec<_>>();
        let [(context_id, node_id)] = matches.as_slice() else {
            return Err(Self::error(
                ctx,
                "source animation getter machine is ambiguous",
            ));
        };
        let graph_id = ctx
            .graph_context
            .context_arena
            .get_ref()
            .get_context(*context_id)
            .ok_or_else(|| Self::error(ctx, "source animation getter context is unavailable"))?
            .get_graph_id();
        let graph = ctx
            .graph_context
            .resources
            .animation_graph_assets
            .get(graph_id)
            .ok_or(GraphError::GraphAssetMissing)?;
        let target = GraphContext {
            context_id: *context_id,
            graph,
            ..ctx.graph_context.clone()
        }
        .with_state_key(StateKey::Default);
        target
            .node_states()
            .get::<InterruptibleFsmState>(*node_id, StateKey::Default)
            .map_err(|error| {
                Self::error(
                    ctx,
                    &format!("source animation getter target state is unavailable: {error}"),
                )
            })
            .cloned()
    }

    fn value(&self, ctx: &NodeContext) -> Result<f32, GraphError> {
        let state = self.target_state(ctx)?;
        match self.source_function.as_str() {
            "GetInstanceMachineWeight" if self.state_id.is_empty() => {
                Ok(state.recorded_machine_weight())
            }
            "GetInstanceCurrentStateElapsedTime" if self.state_id.is_empty() => {
                Ok(state.current_state_elapsed_time())
            }
            "GetInstanceStateWeight" if !self.state_id.is_empty() => {
                Ok(state.recorded_state_weight(&self.state_id))
            }
            _ => Err(Self::error(
                ctx,
                "source animation getter signature is unsupported",
            )),
        }
    }
}

impl NodeLike for SourceAnimationStateGetterNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if self.output.is_empty() {
            return Err(Self::error(
                &ctx,
                "source animation getter output is unavailable",
            ));
        }
        let value = self.value(&ctx)?;
        ctx.set_data_fwd(self.output.clone(), value);
        Ok(())
    }

    fn spec(
        &self,
        mut ctx: bevy_animation_graph::core::context::spec_context::SpecContext,
    ) -> Result<(), GraphError> {
        if self.output.is_empty() {
            return Err(GraphError::GlobalInputDataMissing(self.output.clone()));
        }
        ctx.add_output_data(self.output.clone(), DataSpec::F32);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Animation State Getter".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn getter_preserves_the_source_machine_and_state_contract() {
        let state = SourceAnimationStateGetterNode {
            source_function: "GetInstanceStateWeight".to_owned(),
            fsm_node: "11111111-1111-4111-8111-111111111111".to_owned(),
            state_id: "22222222-2222-4222-8222-222222222222".to_owned(),
            output: "ReturnValue".to_owned(),
        };
        assert_eq!(state.source_function, "GetInstanceStateWeight");
        assert!(!state.fsm_node.is_empty());
        assert!(!state.state_id.is_empty());
        assert_eq!(state.output, "ReturnValue");
    }
}
