use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, events::EventQueue},
    errors::GraphError,
};

pub const IN_EVENTS_A: &str = "events_a";
pub const IN_EVENTS_B: &str = "events_b";
pub const IN_WEIGHT_A: &str = "weight_a";
pub const IN_WEIGHT_B: &str = "weight_b";
pub const OUT_EVENTS: &str = "events";

pub fn weighted_event_queue(mut queue: EventQueue, weight: f32) -> EventQueue {
    let weight = weight.clamp(0.0, 1.0);
    for event in &mut queue.events {
        event.weight *= weight;
    }
    queue
}

pub fn merge_weighted_event_queues(
    first: EventQueue,
    first_weight: f32,
    second: EventQueue,
    second_weight: f32,
) -> EventQueue {
    weighted_event_queue(first, first_weight).concat(weighted_event_queue(second, second_weight))
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct EventQueueBlendNode;

impl NodeLike for EventQueueBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let first = ctx.data_back(IN_EVENTS_A)?.into_event_queue()?;
        let second = ctx.data_back(IN_EVENTS_B)?.into_event_queue()?;
        let first_weight = ctx.data_back(IN_WEIGHT_A)?.as_f32()?;
        let second_weight = ctx.data_back(IN_WEIGHT_B)?.as_f32()?;
        ctx.set_data_fwd(
            OUT_EVENTS,
            merge_weighted_event_queues(first, first_weight, second, second_weight),
        );
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_EVENTS_A, DataSpec::EventQueue)
            .add_input_data(IN_EVENTS_B, DataSpec::EventQueue)
            .add_input_data(IN_WEIGHT_A, DataSpec::F32)
            .add_input_data(IN_WEIGHT_B, DataSpec::F32)
            .add_output_data(OUT_EVENTS, DataSpec::EventQueue);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Event Queue Blend".into()
    }
}

#[cfg(test)]
mod tests {
    use bevy_animation_graph::core::edge_data::events::{AnimationEvent, SampledEvent};

    use super::*;

    #[test]
    fn merge_scales_each_input_without_cross_contamination() {
        let first = EventQueue::with_events([SampledEvent::instant(AnimationEvent::StringId(
            "first".into(),
        ))]);
        let second = EventQueue::with_events([SampledEvent::instant(AnimationEvent::StringId(
            "second".into(),
        ))]);
        let merged = merge_weighted_event_queues(first, 0.25, second, 0.75);
        assert_eq!(merged.events.len(), 2);
        assert!((merged.events[0].weight - 0.25).abs() < 1e-6);
        assert!((merged.events[1].weight - 0.75).abs() < 1e-6);
    }

    #[test]
    fn invalid_weights_are_clamped() {
        let queue = EventQueue::with_events([SampledEvent::instant(AnimationEvent::StringId(
            "event".into(),
        ))]);
        assert_eq!(
            weighted_event_queue(queue.clone(), -1.0).events[0].weight,
            0.0
        );
        assert_eq!(weighted_event_queue(queue, 2.0).events[0].weight, 1.0);
    }
}
