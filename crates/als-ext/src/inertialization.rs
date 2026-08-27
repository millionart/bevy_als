use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

pub fn inertial_alpha(factor: f32, transition_duration: f32, half_life: f32) -> f32 {
    let elapsed = factor.clamp(0.0, 1.0) * transition_duration.max(0.0);
    if half_life <= f32::EPSILON {
        return 1.0;
    }
    1.0 - 2.0_f32.powf(-elapsed / half_life)
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct InertializationNode {
    pub transition_duration: f32,
    pub half_life: f32,
}

impl Default for InertializationNode {
    fn default() -> Self {
        Self {
            transition_duration: 0.2,
            half_life: 0.05,
        }
    }
}

impl InertializationNode {
    pub const FACTOR: &'static str = "factor";
    pub const IN_POSE_A: &'static str = "pose_a";
    pub const IN_TIME_A: &'static str = "time_a";
    pub const IN_POSE_B: &'static str = "pose_b";
    pub const IN_TIME_B: &'static str = "time_b";
    pub const OUT_POSE: &'static str = "pose";
}

impl NodeLike for InertializationNode {
    fn display_name(&self) -> String {
        "ALS Inertialization".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back(Self::IN_TIME_A)?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        ctx.set_time_update_back(Self::IN_TIME_A, update.clone());
        let source = ctx.data_back(Self::IN_POSE_A)?.into_pose()?;
        ctx.set_time_update_back(Self::IN_TIME_B, TimeUpdate::Absolute(source.timestamp));
        let target = ctx.data_back(Self::IN_POSE_B)?.into_pose()?;
        let alpha = inertial_alpha(
            ctx.data_back(Self::FACTOR)?.as_f32()?,
            self.transition_duration,
            self.half_life,
        );
        let mut pose = source
            .scalar_mult(1.0 - alpha)
            .linear_add(&target.scalar_mult(alpha))
            .normalize_quat();
        pose.timestamp = target.timestamp;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(Self::OUT_POSE, pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(Self::FACTOR, DataSpec::F32)
            .add_input_data(Self::IN_POSE_A, DataSpec::Pose)
            .add_input_time(Self::IN_TIME_A)
            .add_input_data(Self::IN_POSE_B, DataSpec::Pose)
            .add_input_time(Self::IN_TIME_B)
            .add_output_data(Self::OUT_POSE, DataSpec::Pose)
            .add_output_time();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decay_matches_half_life_contract() {
        assert!((inertial_alpha(0.5, 0.1, 0.05) - 0.5).abs() < 1e-6);
        assert!((inertial_alpha(1.0, 0.1, 0.05) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<InertializationNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<InertializationNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
