use bevy::{asset::Handle, prelude::*};
use bevy_animation_graph::core::{
    animation_clip::GraphClip,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
};

use crate::global_f32_input::root_defaults_enabled;
use crate::source_sequence_player::sample_pose;

fn frame_to_time(frame: f32, frames_per_second: f32) -> f32 {
    frame.max(0.0) / frames_per_second.max(f32::EPSILON)
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceSequenceEvaluatorNode {
    pub clip: Handle<GraphClip>,
    pub explicit_frame: f32,
    pub explicit_frame_key: String,
    pub explicit_frame_pin: String,
    pub frames_per_second: f32,
    pub should_loop: bool,
    pub teleport_to_explicit_time: bool,
    pub reinitialization_behavior: String,
}

impl Default for SourceSequenceEvaluatorNode {
    fn default() -> Self {
        Self {
            clip: default(),
            explicit_frame: 0.0,
            explicit_frame_key: default(),
            explicit_frame_pin: "Explicit Frame".to_owned(),
            frames_per_second: 30.0,
            should_loop: true,
            teleport_to_explicit_time: true,
            reinitialization_behavior: "ExplicitTime".to_owned(),
        }
    }
}

impl SourceSequenceEvaluatorNode {
    fn explicit_frame_value(&self, ctx: &NodeContext) -> Result<f32, GraphError> {
        if let Ok(value) = ctx.data_back(self.explicit_frame_pin.clone()) {
            return value.as_f32();
        }
        if self.explicit_frame_key.is_empty() {
            return Ok(self.explicit_frame);
        }
        match ctx
            .graph_context
            .global_input_data
            .get(&self.explicit_frame_key)
        {
            Some(value) => value.as_f32(),
            None if root_defaults_enabled(ctx) => Ok(self.explicit_frame),
            None => Err(GraphError::GlobalInputDataMissing(
                self.explicit_frame_key.clone(),
            )),
        }
    }
}

impl NodeLike for SourceSequenceEvaluatorNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let Some(clip) = ctx
            .graph_context
            .resources
            .graph_clip_assets
            .get(&self.clip)
        else {
            ctx.set_data_fwd("Pose", DataValue::Pose(default()));
            return Ok(());
        };
        let mut time = frame_to_time(self.explicit_frame_value(&ctx)?, self.frames_per_second);
        let duration = clip.duration().max(f32::EPSILON);
        time = if self.should_loop {
            time.rem_euclid(duration)
        } else {
            time.clamp(0.0, duration)
        };
        let pose = sample_pose(clip, time);
        ctx.set_time(time);
        ctx.set_data_fwd("Pose", pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(self.explicit_frame_pin.clone(), DataSpec::F32)
            .add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Sequence Evaluator".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_explicit_frame_uses_the_exported_thirty_fps_rate() {
        assert!((frame_to_time(21.0, 30.0) - 0.7).abs() < 1e-6);
        assert!((frame_to_time(24.0, 30.0) - 0.8).abs() < 1e-6);
        assert!((frame_to_time(23.0, 30.0) - 23.0 / 30.0).abs() < 1e-6);
    }

    #[test]
    fn source_explicit_frame_pin_name_is_visible() {
        assert_eq!(
            SourceSequenceEvaluatorNode::default().explicit_frame_pin,
            "Explicit Frame"
        );
    }
}
