use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_clip::GraphClip,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{
        DataSpec,
        events::{AnimationEvent, EventQueue, SampledEvent},
    },
    errors::GraphError,
    event_track::sample_tracks,
    pose::Pose,
};

use super::source_sequence_player::sample_pose;

/// A direct ALS clip with the Animation Graph clip's clock and event ports.
#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceClipNode {
    pub clip: Handle<GraphClip>,
    pub override_duration: Option<f32>,
}

impl NodeLike for SourceClipNode {
    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let clips = &ctx.graph_context.resources.graph_clip_assets;
        let duration = self
            .override_duration
            .or_else(|| clips.get(&self.clip).map(GraphClip::duration))
            .ok_or(GraphError::ClipMissing)?;
        ctx.set_duration_fwd(Some(duration));
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let clip = ctx
            .graph_context
            .resources
            .graph_clip_assets
            .get(&self.clip);
        let Some(clip) = clip else {
            if self.override_duration.is_some() {
                ctx.set_data_fwd("pose", Pose::default());
                return Ok(());
            }
            return Err(GraphError::ClipMissing);
        };
        let duration = self.override_duration.unwrap_or_else(|| clip.duration());
        let time = ctx
            .time_update_fwd()?
            .partial_update_with_tracks(ctx.prev_time(), &clip.event_tracks)
            .ok_or(GraphError::TimeUpdateFailed)?;
        ctx.set_time(time);

        let mut events = EventQueue::with_events(sample_tracks(clip.event_tracks.values(), time));
        if time > duration {
            events.add_event(SampledEvent::instant(AnimationEvent::AnimationClipFinished));
        }
        let mut pose = sample_pose(clip, time.clamp(0.0, duration));
        pose.timestamp = time;
        ctx.set_data_fwd("events", events);
        ctx.set_data_fwd("pose", pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data("pose", DataSpec::Pose)
            .add_output_data("events", DataSpec::EventQueue)
            .add_output_time();
        Ok(())
    }

    fn display_name(&self) -> String {
        "⏵ Animation Clip".into()
    }
}
