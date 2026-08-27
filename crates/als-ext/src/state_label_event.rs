use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{
        DataSpec,
        events::{AnimationEvent, EventQueue, SampledEvent},
    },
    errors::GraphError,
};

#[derive(Reflect, Clone, Debug, Default)]
struct StateLabelEventState {
    last_index: Option<usize>,
}

pub fn state_label_index(value: f32, label_count: usize) -> Option<usize> {
    if label_count == 0 || !value.is_finite() {
        return None;
    }
    Some(value.round().clamp(0.0, (label_count - 1) as f32) as usize)
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct StateLabelEventNode {
    pub key: String,
    pub labels: Vec<String>,
}

impl StateLabelEventNode {
    pub const OUT_EVENT: &'static str = "event";
    pub const OUT_VALUE: &'static str = "value";
}

impl NodeLike for StateLabelEventNode {
    fn display_name(&self) -> String {
        "ALS State Label Event".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let value = ctx
            .graph_context
            .global_input_data
            .get(&self.key)
            .cloned()
            .ok_or_else(|| GraphError::GlobalInputDataMissing(self.key.clone()))?
            .as_f32()?;
        let index = state_label_index(value, self.labels.len());
        let event = {
            let state = ctx.state_mut::<StateLabelEventState>()?;
            let changed = state.last_index != index;
            state.last_index = index;
            if changed {
                index.map(|index| self.labels[index].clone())
            } else {
                None
            }
        };
        let queue = event.map_or_else(EventQueue::default, |label| {
            EventQueue::with_events([SampledEvent::instant(
                AnimationEvent::TransitionToStateLabel(label),
            )])
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
    fn state_index_rounds_and_clamps_to_available_labels() {
        assert_eq!(state_label_index(-1.0, 3), Some(0));
        assert_eq!(state_label_index(1.6, 3), Some(2));
        assert_eq!(state_label_index(20.0, 3), Some(2));
        assert_eq!(state_label_index(f32::NAN, 3), None);
        assert_eq!(state_label_index(1.0, 0), None);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<StateLabelEventNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<StateLabelEventNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
