use bevy::prelude::*;
use bevy_animation_graph::core::animation_graph::TimeUpdate;
use bevy_animation_graph::core::animation_node::{NodeLike, ReflectNodeLike};
use bevy_animation_graph::core::context::{new_context::NodeContext, spec_context::SpecContext};
use bevy_animation_graph::core::edge_data::{DataSpec, DataValue, events::EventQueue};
use bevy_animation_graph::core::errors::GraphError;

use crate::extensions::animation_context::optional_data_back;

pub const IN_POSE: &str = "pose";
pub const IN_TIME: &str = "time";
pub const IN_POSITION: &str = "position";
pub const OUT_POSE: &str = "pose";
pub const IN_EVENTS: &str = "events";
pub const OUT_EVENTS: &str = "events";

pub fn resolve_position(raw: f32, duration: f32) -> f32 {
    raw.clamp(0.0, duration.max(1e-4))
}

pub fn resolve_looping_position(raw: f32, duration: f32) -> f32 {
    raw.rem_euclid(duration.max(1e-4))
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct TimePositionNode {
    pub duration_fallback: f32,
    pub loop_position: bool,
}

impl NodeLike for TimePositionNode {
    fn display_name(&self) -> String {
        "ALS Time Position".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let raw = ctx.data_back(IN_POSITION)?.as_f32()?;
        let duration = ctx
            .duration_back(IN_TIME)?
            .unwrap_or(self.duration_fallback)
            .max(1e-4);
        let position = if self.loop_position {
            resolve_looping_position(raw, duration)
        } else {
            resolve_position(raw, duration)
        };

        ctx.set_time(position);
        ctx.set_time_update_back(IN_TIME, TimeUpdate::Absolute(position));

        let mut pose = ctx.data_back(IN_POSE)?.into_pose()?;
        let events = optional_data_back(&ctx, IN_EVENTS)?
            .map_or(Ok(EventQueue::default()), DataValue::into_event_queue)?;
        pose.timestamp = position;
        ctx.set_data_fwd(OUT_POSE, pose);
        ctx.set_data_fwd(OUT_EVENTS, events);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_POSITION, DataSpec::F32)
            .add_input_data(IN_POSE, DataSpec::Pose)
            .add_input_data(IN_EVENTS, DataSpec::EventQueue)
            .add_input_time(IN_TIME)
            .add_output_data(OUT_POSE, DataSpec::Pose)
            .add_output_data(OUT_EVENTS, DataSpec::EventQueue)
            .add_output_time();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bevy_animation_graph::core::{
        animation_graph::{GraphInputPin, TimeUpdate},
        edge_data::events::{AnimationEvent, EventQueue, SampledEvent},
        pose::Pose,
        utils::testing::GraphTestHarness,
    };

    use super::*;

    #[test]
    fn position_clamps_into_clip_domain() {
        assert_eq!(resolve_position(-0.5, 2.667), 0.0);
        assert!((resolve_position(9.9, 2.667) - 2.667).abs() < 1e-5);
        assert!((resolve_position(1.25, 2.667) - 1.25).abs() < 1e-6);
    }

    #[test]
    fn zero_duration_falls_back_to_epsilon_bound() {
        assert!(resolve_position(5.0, 0.0) <= 1e-3);
        assert!(resolve_position(-5.0, -1.0).abs() < 1e-5);
    }

    #[test]
    fn looping_position_wraps_across_duration() {
        assert!((resolve_looping_position(2.75, 1.0) - 0.75).abs() < 1e-6);
        assert!((resolve_looping_position(-0.25, 1.0) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::{GetTypeRegistration, TypeRegistry};
        let mut registry = TypeRegistry::default();
        registry.register::<TimePositionNode>();
        let reg = registry
            .get(std::any::TypeId::of::<TimePositionNode>())
            .unwrap();
        assert!(
            reg.data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <TimePositionNode as bevy::reflect::TypePath>::type_path(),
            "als_ext::TimePositionNode"
        );
    }

    #[test]
    fn event_queue_is_forwarded_with_the_pose_time() {
        let events = EventQueue::with_events([SampledEvent::instant(AnimationEvent::StringId(
            "event".into(),
        ))]);
        GraphTestHarness::node(TimePositionNode {
            duration_fallback: 1.0,
            loop_position: false,
        })
        .with_const_input_data(GraphInputPin::Passthrough(IN_POSITION.into()), 0.5_f32)
        .with_const_input_data(GraphInputPin::Passthrough(IN_POSE.into()), Pose::default())
        .with_const_input_data(GraphInputPin::Passthrough(IN_EVENTS.into()), events.clone())
        .with_const_input_duration(GraphInputPin::Passthrough(IN_TIME.into()), Some(1.0))
        .with_time_fwd(Some(TimeUpdate::Absolute(0.5)))
        .when_queried()
        .then_output_is(OUT_EVENTS, events);
    }
}
