use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::linear::LinearInterpolator,
};

use crate::extensions::merge_weighted_event_queues;

pub const IN_POSE: &str = "pose";
pub const IN_EVENTS: &str = "events";
pub const IN_TIME: &str = "time";
pub const OUT_POSE: &str = "pose";
pub const OUT_EVENTS: &str = "events";

pub fn loop_position(time: f32, duration: f32, interpolation_period: f32) -> f32 {
    time.rem_euclid((duration + interpolation_period).max(f32::EPSILON))
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct EventLoopNode {
    pub interpolation_period: f32,
}

impl EventLoopNode {
    pub fn new(interpolation_period: f32) -> Self {
        Self {
            interpolation_period,
        }
    }
}

impl NodeLike for EventLoopNode {
    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(None);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let input = ctx.time_update_fwd()?;
        let duration = ctx.duration_back(IN_TIME)?;
        let Some(duration) = duration else {
            ctx.set_time_update_back(IN_TIME, input);
            let pose = ctx.data_back(IN_POSE)?.into_pose()?;
            let events = ctx.data_back(IN_EVENTS)?.into_event_queue()?;
            ctx.set_time(pose.timestamp);
            ctx.set_data_fwd(OUT_POSE, pose);
            ctx.set_data_fwd(OUT_EVENTS, events);
            return Ok(());
        };

        let interpolation_period = self.interpolation_period.max(0.0);
        let full_duration = (duration + interpolation_period).max(f32::EPSILON);
        let previous_time = ctx.prev_time();
        let (current_time, position, forward_update) = match input {
            TimeUpdate::Delta(delta) => {
                let current = previous_time + delta;
                let position = loop_position(current, duration, interpolation_period);
                let forward = if previous_time.div_euclid(full_duration)
                    != current.div_euclid(full_duration)
                {
                    TimeUpdate::Absolute(position)
                } else {
                    TimeUpdate::Delta(delta)
                };
                (current, position, forward)
            }
            TimeUpdate::Absolute(current) => {
                let position = loop_position(current, duration, interpolation_period);
                (current, position, TimeUpdate::Absolute(position))
            }
            update @ TimeUpdate::PercentOfEvent { .. } => {
                ctx.set_time_update_back(IN_TIME, update);
                let pose = ctx.data_back(IN_POSE)?.into_pose()?;
                let events = ctx.data_back(IN_EVENTS)?.into_event_queue()?;
                ctx.set_time(pose.timestamp);
                ctx.set_data_fwd(OUT_POSE, pose);
                ctx.set_data_fwd(OUT_EVENTS, events);
                return Ok(());
            }
        };

        ctx.set_time_update_back(IN_TIME, forward_update);
        let mut pose = ctx.data_back(IN_POSE)?.into_pose()?;
        let mut events = ctx.data_back(IN_EVENTS)?.into_event_queue()?;
        if position > duration && position < full_duration {
            let alpha = (position - duration) / interpolation_period.max(f32::EPSILON);
            let mut temporary = ctx.clone().with_temp_state_key();
            temporary.set_time_update_back(IN_TIME, TimeUpdate::Absolute(0.0));
            let start_pose = temporary.data_back(IN_POSE)?.into_pose()?;
            let start_events = temporary.data_back(IN_EVENTS)?.into_event_queue()?;
            let timestamp = pose.timestamp;
            LinearInterpolator {
                bone_mask: BoneMask::all(),
            }
            .interpolate_pose(&mut pose, &start_pose, alpha);
            pose.timestamp = timestamp;
            events = merge_weighted_event_queues(events, 1.0 - alpha, start_events, alpha);
        }
        let loops_elapsed = current_time.div_euclid(full_duration) * full_duration;
        pose.timestamp += loops_elapsed;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(OUT_POSE, pose);
        ctx.set_data_fwd(OUT_EVENTS, events);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_POSE, DataSpec::Pose)
            .add_input_data(IN_EVENTS, DataSpec::EventQueue)
            .add_input_time(IN_TIME)
            .add_output_data(OUT_POSE, DataSpec::Pose)
            .add_output_data(OUT_EVENTS, DataSpec::EventQueue)
            .add_output_time();
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Event Loop".into()
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
    fn loop_position_wraps_at_the_complete_loop_period() {
        assert!((loop_position(1.25, 1.0, 0.0) - 0.25).abs() < 1e-6);
        assert!((loop_position(-0.25, 1.0, 0.0) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn interpolation_period_extends_the_loop_domain() {
        assert!((loop_position(1.1, 1.0, 0.2) - 1.1).abs() < 1e-6);
        assert!((loop_position(1.3, 1.0, 0.2) - 0.1).abs() < 1e-6);
    }

    #[test]
    fn loop_forwards_the_wrapped_event_queue() {
        let expected = EventQueue::with_events([SampledEvent {
            event: AnimationEvent::StringId("event".into()),
            weight: 1.0,
            percentage: 1.0,
            track: Some("track".into()),
        }]);
        GraphTestHarness::node(EventLoopNode::new(0.0))
            .with_const_input_data(GraphInputPin::Passthrough(IN_POSE.into()), Pose::default())
            .with_const_input_data(
                GraphInputPin::Passthrough(IN_EVENTS.into()),
                expected.clone(),
            )
            .with_const_input_duration(GraphInputPin::Passthrough(IN_TIME.into()), Some(1.0))
            .with_time_fwd(Some(TimeUpdate::Absolute(1.25)))
            .when_queried()
            .then_output_is(OUT_EVENTS, expected);
    }
}
