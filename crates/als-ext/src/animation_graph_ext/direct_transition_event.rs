use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{
        DataSpec,
        events::{AnimationEvent, EventQueue, SampledEvent},
    },
    errors::GraphError,
    state_machine::high_level::{DirectTransitionId, TransitionId},
};

pub fn direct_transition_index(value: f32, transition_count: usize) -> Option<usize> {
    if transition_count == 0 || !value.is_finite() || value < 0.0 {
        return None;
    }
    Some(value.round().clamp(0.0, (transition_count - 1) as f32) as usize)
}

pub fn take_direct_transition_edge(
    previous: &mut Option<usize>,
    value: f32,
    transition_count: usize,
) -> Option<usize> {
    let index = direct_transition_index(value, transition_count);
    let changed = *previous != index;
    *previous = index;
    changed.then_some(index).flatten()
}

#[derive(Reflect, Clone, Debug, Default)]
struct DirectTransitionEventState {
    last_index: Option<usize>,
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct DirectTransitionEventNode {
    pub key: String,
    pub transitions: Vec<DirectTransitionId>,
    pub default_value: f32,
}

impl Default for DirectTransitionEventNode {
    fn default() -> Self {
        Self {
            key: default(),
            transitions: default(),
            default_value: -1.0,
        }
    }
}

impl DirectTransitionEventNode {
    pub const OUT_EVENT: &'static str = "event";
    pub const OUT_VALUE: &'static str = "value";
}

impl NodeLike for DirectTransitionEventNode {
    fn display_name(&self) -> String {
        "ALS Direct Transition Event".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let value = ctx
            .graph_context
            .global_input_data
            .get(&self.key)
            .map(|value| value.as_f32())
            .transpose()?
            .unwrap_or(self.default_value);
        let index = take_direct_transition_edge(
            &mut ctx.state_mut::<DirectTransitionEventState>()?.last_index,
            value,
            self.transitions.len(),
        );
        let transition = index.map(|index| self.transitions[index]);
        let queue = transition.map_or_else(EventQueue::default, |transition| {
            EventQueue::with_events([SampledEvent::instant(AnimationEvent::Transition(
                TransitionId::Direct(transition),
            ))])
        });
        ctx.set_data_fwd(Self::OUT_EVENT, queue);
        ctx.set_data_fwd(Self::OUT_VALUE, value);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data(Self::OUT_EVENT, DataSpec::EventQueue)
            .add_output_data(Self::OUT_VALUE, DataSpec::F32);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_index_rejects_negative_values() {
        assert_eq!(direct_transition_index(-1.0, 3), None);
        assert_eq!(direct_transition_index(1.6, 3), Some(2));
        assert_eq!(direct_transition_index(20.0, 3), Some(2));
        assert_eq!(direct_transition_index(f32::NAN, 3), None);
        assert_eq!(direct_transition_index(1.0, 0), None);
    }

    #[test]
    fn transition_index_emits_once_until_cleared() {
        let mut previous = None;
        assert_eq!(take_direct_transition_edge(&mut previous, 1.0, 3), Some(1));
        assert_eq!(take_direct_transition_edge(&mut previous, 1.0, 3), None);
        assert_eq!(take_direct_transition_edge(&mut previous, -1.0, 3), None);
        assert_eq!(take_direct_transition_edge(&mut previous, 1.0, 3), Some(1));
    }

    #[test]
    fn default_value_is_a_no_transition_sentinel() {
        let node = DirectTransitionEventNode::default();
        assert_eq!(node.default_value, -1.0);
        assert_eq!(direct_transition_index(node.default_value, 1), None);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<DirectTransitionEventNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<DirectTransitionEventNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
