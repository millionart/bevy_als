use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

use crate::animation_context::{
    AnimationContextState, context_time_update, resolve_named_context_curves,
};

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct InertializationPassthroughNode {
    pub reset_on_becoming_relevant: bool,
    pub forward_requests_through_skipped_cached_poses: bool,
    pub curve_names: Vec<String>,
    pub capture_context: bool,
    pub expose_context_curves: bool,
    pub exposed_output_data: Vec<String>,
    pub missing_curve_value: Option<f32>,
    pub internalize_time: bool,
    pub output: String,
}

impl Default for InertializationPassthroughNode {
    fn default() -> Self {
        Self {
            reset_on_becoming_relevant: true,
            forward_requests_through_skipped_cached_poses: true,
            curve_names: Vec::new(),
            capture_context: false,
            expose_context_curves: false,
            exposed_output_data: Vec::new(),
            missing_curve_value: None,
            internalize_time: false,
            output: "pose".to_owned(),
        }
    }
}

impl InertializationPassthroughNode {
    fn expose_output(&self, curve: &str) -> bool {
        !self.capture_context
            || self.expose_context_curves
            || self.exposed_output_data.iter().any(|value| value == curve)
    }
}

impl NodeLike for InertializationPassthroughNode {
    fn display_name(&self) -> String {
        "Inertialization".to_owned()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if self.internalize_time {
            ctx.set_duration_fwd(None);
        } else {
            ctx.set_duration_fwd(ctx.duration_back("source_time")?);
        }
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let time_update = if self.internalize_time {
            context_time_update(&ctx)?
        } else {
            ctx.time_update_fwd()?
        };
        if !self.internalize_time {
            ctx.set_time_update_back("source_time", time_update);
        }
        let pose = ctx.data_back("Source")?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(self.output.clone(), pose);
        let context_curves = if self.capture_context {
            resolve_named_context_curves(
                &ctx,
                "Source",
                &self.curve_names,
                self.missing_curve_value,
            )?
        } else {
            default()
        };
        let mut state = AnimationContextState::default();
        for curve in &self.curve_names {
            let value = if self.capture_context {
                *context_curves
                    .get(curve)
                    .ok_or(GraphError::MissingStateValue)?
            } else {
                ctx.data_back(format!("source_{curve}"))?.as_f32()?
            };
            state.curves.insert(curve.clone(), value);
            if self.expose_output(curve) {
                ctx.set_data_fwd(curve.clone(), value);
            }
        }
        if self.capture_context {
            *ctx.state_mut_or_else(AnimationContextState::default)? = state;
        }
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Source", DataSpec::Pose)
            .add_output_data(self.output.clone(), DataSpec::Pose);
        if !self.internalize_time {
            ctx.add_input_time("source_time").add_output_time();
        }
        for curve in &self.curve_names {
            if !self.capture_context {
                ctx.add_input_data(format!("source_{curve}"), DataSpec::F32);
            }
            if self.expose_output(curve) {
                ctx.add_output_data(curve.clone(), DataSpec::F32);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<InertializationPassthroughNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<InertializationPassthroughNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
