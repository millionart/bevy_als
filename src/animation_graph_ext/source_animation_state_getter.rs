use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    animation_graph::NodeId,
    context::{
        new_context::NodeContext,
        node_states::StateKey,
    },
    edge_data::DataSpec,
    errors::GraphError,
};

use crate::extensions::interruptible_fsm::{InterruptibleFsmNode, InterruptibleFsmState};
use crate::extensions::source_instance::linked_instance_context;

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

    fn machine_node(&self, ctx: &NodeContext) -> Result<NodeId, GraphError> {
        if self.fsm_node.is_empty() {
            return Err(Self::error(
                ctx,
                "source animation getter machine is unavailable",
            ));
        }
        let owner = linked_instance_context(ctx, ctx.graph_context.context_id)
            .ok_or_else(|| Self::error(ctx, "source animation getter owning instance is unavailable"))?;
        ctx
            .graph_context
            .context_arena
            .get_ref()
            .iter_context_ids()
            .filter(|context_id| linked_instance_context(ctx, *context_id) == Some(owner))
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
                Some(node.id)
            })
            .next()
            .ok_or_else(|| Self::error(ctx, "source animation getter machine is unavailable"))
    }

    fn target_state(&self, ctx: &NodeContext, node_id: NodeId) -> Result<Option<InterruptibleFsmState>, GraphError> {
        let Some(context_id) = crate::extensions::source_instance::source_machine_context(ctx, node_id)? else {
            return Ok(None);
        };
        ctx
            .graph_context
            .context_arena
            .get_ref()
            .get_context(context_id)
            .ok_or_else(|| Self::error(ctx, "source animation getter context is unavailable"))?
            .node_states
            .get::<InterruptibleFsmState>(node_id, StateKey::Default)
            .map_err(|error| {
                Self::error(
                    ctx,
                    &format!("source animation getter target state is unavailable: {error}"),
                )
            })
            .map(|state| Some(state.clone()))
    }

    fn value(&self, ctx: &NodeContext) -> Result<f32, GraphError> {
        let node = self.machine_node(ctx)?;
        match self.source_function.as_str() {
            "GetInstanceMachineWeight" if self.state_id.is_empty() => {
                crate::extensions::source_instance::source_machine_weight(ctx, node)
            }
            "GetInstanceCurrentStateElapsedTime" if self.state_id.is_empty() => {
                Ok(self.target_state(ctx, node)?.map_or(0.0, |state| state.current_state_elapsed_time()))
            }
            "GetInstanceStateWeight" if !self.state_id.is_empty() => {
                crate::extensions::source_instance::source_state_weight(ctx, node, &self.state_id)
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
