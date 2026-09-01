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

const SOURCE_TIME: &str = "source_time";
const OUT_POSE: &str = "pose";

fn pose_spec(
    mut ctx: SpecContext,
    pose_input: &str,
    internalize_time: bool,
) -> Result<(), GraphError> {
    ctx.add_input_data(pose_input, DataSpec::Pose)
        .add_output_data(OUT_POSE, DataSpec::Pose);
    if !internalize_time {
        ctx.add_input_time(SOURCE_TIME).add_output_time();
    }
    Ok(())
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct ControlRigBoundaryNode {
    pub rig_class: String,
    pub alpha_enabled: bool,
    pub transfer_input_pose: bool,
    pub transfer_input_curves: bool,
    pub transfer_pose_in_global_space: bool,
    pub pose_input: String,
    pub curve_names: Vec<String>,
    pub capture_context: bool,
    pub expose_context_curves: bool,
    pub exposed_output_data: Vec<String>,
    pub missing_curve_value: Option<f32>,
    pub internalize_time: bool,
}

impl Default for ControlRigBoundaryNode {
    fn default() -> Self {
        Self {
            rig_class: "CR_Als".to_owned(),
            alpha_enabled: true,
            transfer_input_pose: true,
            transfer_input_curves: true,
            transfer_pose_in_global_space: false,
            pose_input: "source_pose".to_owned(),
            curve_names: default(),
            capture_context: false,
            expose_context_curves: false,
            exposed_output_data: default(),
            missing_curve_value: None,
            internalize_time: false,
        }
    }
}

impl ControlRigBoundaryNode {
    fn expose_output(&self, curve: &str) -> bool {
        !self.capture_context
            || self.expose_context_curves
            || self.exposed_output_data.iter().any(|value| value == curve)
    }
}

impl NodeLike for ControlRigBoundaryNode {
    fn display_name(&self) -> String {
        format!("ALS Control Rig {}", self.rig_class)
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if self.internalize_time {
            ctx.set_duration_fwd(None);
        } else {
            ctx.set_duration_fwd(ctx.duration_back(SOURCE_TIME)?);
        }
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = if self.internalize_time {
            context_time_update(&ctx)?
        } else {
            ctx.time_update_fwd()?
        };
        if !self.internalize_time {
            ctx.set_time_update_back(SOURCE_TIME, update);
        }
        let pose = ctx.data_back(self.pose_input.clone())?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(OUT_POSE, pose);
        let context_curves = if self.capture_context {
            resolve_named_context_curves(
                &ctx,
                &self.pose_input,
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
                ctx.data_back(curve.clone())?.as_f32()?
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
        for curve in &self.curve_names {
            if !self.capture_context {
                ctx.add_input_data(curve.clone(), DataSpec::F32);
            }
            if self.expose_output(curve) {
                ctx.add_output_data(curve.clone(), DataSpec::F32);
            }
        }
        pose_spec(ctx, &self.pose_input, self.internalize_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_rig_boundary_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<ControlRigBoundaryNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<ControlRigBoundaryNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
