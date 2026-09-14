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
    pub curves: crate::als_curves::AlsPoseCurves,
    pub root_lock: Option<crate::source_sequence_player::SourceRootLock>,
    pub use_explicit_frame: bool,
    pub explicit_value: f32,
    pub explicit_value_key: String,
    pub explicit_value_pin: String,
    pub expose_explicit_value_pin: bool,
    pub frames_per_second: f32,
    pub should_loop: bool,
    pub teleport_to_explicit_time: bool,
    pub reinitialization_behavior: String,
}

impl Default for SourceSequenceEvaluatorNode {
    fn default() -> Self {
        Self {
            clip: default(),
            curves: default(),
            root_lock: None,
            use_explicit_frame: false,
            explicit_value: 0.0,
            explicit_value_key: default(),
            explicit_value_pin: "ExplicitTime".to_owned(),
            expose_explicit_value_pin: true,
            frames_per_second: 30.0,
            should_loop: true,
            teleport_to_explicit_time: true,
            reinitialization_behavior: "ExplicitTime".to_owned(),
        }
    }
}

impl SourceSequenceEvaluatorNode {
    fn input_value(&self, ctx: &NodeContext) -> Result<f32, GraphError> {
        if self.expose_explicit_value_pin {
            if let Ok(value) = ctx.data_back(self.explicit_value_pin.clone()) {
                return value.as_f32();
            }
        }
        if self.explicit_value_key.is_empty() {
            return Ok(self.explicit_value);
        }
        match ctx
            .graph_context
            .global_input_data
            .get(&self.explicit_value_key)
        {
            Some(value) => value.as_f32(),
            None if root_defaults_enabled(ctx) => Ok(self.explicit_value),
            None => Err(GraphError::GlobalInputDataMissing(
                self.explicit_value_key.clone(),
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
        let value = self.input_value(&ctx)?;
        let time = if self.use_explicit_frame { frame_to_time(value, self.frames_per_second) } else { value }
            .clamp(0.0, clip.duration().max(0.0));
        let mut pose = sample_pose(clip, time);
        if let Some(mode) = self.root_lock {
            let skeleton = ctx.graph_context.resources.skeleton_assets.get(&clip.skeleton)
                .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
            crate::source_sequence_player::apply_source_root_lock(&mut pose, clip, skeleton, mode, false)
                .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        }
        *ctx.state_mut_or_else(crate::AnimationContextState::default)? =
            crate::AnimationContextState {
                curves: self.curves.sample_all(time),
                curves_are_authoritative: true,
                ..default()
            };
        ctx.set_time(time);
        ctx.set_data_fwd("Pose", pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        if self.expose_explicit_value_pin {
            ctx.add_input_data(self.explicit_value_pin.clone(), DataSpec::F32);
        }
        ctx.add_output_data("Pose", DataSpec::Pose);
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
    fn source_explicit_frame_uses_the_configured_source_rate() {
        assert!((frame_to_time(21.0, 30.0) - 0.7).abs() < 1e-6);
        assert!((frame_to_time(24.0, 30.0) - 0.8).abs() < 1e-6);
        assert!((frame_to_time(23.0, 30.0) - 23.0 / 30.0).abs() < 1e-6);
        assert_eq!(frame_to_time(15.0, 60.0), 0.25);
        assert!((frame_to_time(15.0, 30000.0 / 1001.0) - 0.5005).abs() < 1e-6);
    }

    #[test]
    fn source_explicit_value_pin_name_can_be_preserved() {
        let node = SourceSequenceEvaluatorNode {
            explicit_value_pin: "ExplicitFrame".to_owned(),
            ..default()
        };
        assert_eq!(node.explicit_value_pin, "ExplicitFrame");
    }

    #[test]
    fn source_sequence_evaluator_can_hide_an_unexposed_frame_pin() {
        let node = SourceSequenceEvaluatorNode {
            expose_explicit_value_pin: false,
            ..default()
        };
        assert!(!node.expose_explicit_value_pin);
    }
}
