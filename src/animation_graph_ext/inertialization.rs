use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

pub fn source_inertial_float(
    mut value: f32,
    mut velocity: f32,
    mut elapsed: f32,
    mut duration: f32,
) -> f32 {
    const TIME_EPSILON: f32 = 1e-7;
    const VELOCITY_EPSILON: f32 = 1e-4;

    if !value.is_finite() || !velocity.is_finite() || !elapsed.is_finite() || !duration.is_finite()
    {
        return 0.0;
    }
    elapsed = elapsed.max(0.0);
    duration = duration.max(0.0);
    if elapsed >= duration - TIME_EPSILON {
        return 0.0;
    }

    let sign = if value < 0.0 {
        value = -value;
        velocity = -velocity;
        -1.0
    } else {
        1.0
    };
    velocity = velocity.min(0.0);
    if velocity < -VELOCITY_EPSILON {
        duration = duration.min(-5.0 * value / velocity);
    }
    if duration <= TIME_EPSILON || elapsed >= duration - TIME_EPSILON {
        return 0.0;
    }

    let duration_squared = duration * duration;
    let duration_cubed = duration * duration_squared;
    let duration_fourth = duration * duration_cubed;
    let duration_fifth = duration * duration_fourth;
    let acceleration = ((-8.0 * duration * velocity - 20.0 * value) / duration_squared).max(0.0);
    let coefficient_a = -0.5
        * (acceleration * duration_squared + 6.0 * duration * velocity + 12.0 * value)
        / duration_fifth;
    let coefficient_b = 0.5
        * (3.0 * acceleration * duration_squared + 16.0 * duration * velocity + 30.0 * value)
        / duration_fourth;
    let coefficient_c = -0.5
        * (3.0 * acceleration * duration_squared + 12.0 * duration * velocity + 20.0 * value)
        / duration_cubed;
    let coefficient_d = 0.5 * acceleration;
    let result = (((((coefficient_a * elapsed) + coefficient_b) * elapsed + coefficient_c)
        * elapsed
        + coefficient_d)
        * elapsed
        + velocity)
        * elapsed
        + value;
    if result.is_finite() {
        result * sign
    } else {
        0.0
    }
}

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct InertializationState {
    pub(crate) context: crate::extensions::AnimationContextState,
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct InertializationNode {
    pub transition_duration: f32,
    pub blend_profile: Option<crate::extensions::SourceBlendProfile>,
}

impl Default for InertializationNode {
    fn default() -> Self {
        Self {
            transition_duration: 0.2,
            blend_profile: None,
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
        ctx.set_duration_fwd(ctx.duration_back(Self::IN_TIME_B)?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        ctx.set_time_update_back(Self::IN_TIME_B, update);
        let target = ctx.data_back(Self::IN_POSE_B)?.into_pose()?;
        let context =
            crate::extensions::animation_context::pose_input_context_state(&ctx, Self::IN_POSE_B)
                .unwrap_or_default();
        ctx.set_time(target.timestamp);
        ctx.set_data_fwd(Self::OUT_POSE, target);
        ctx.state_mut::<InertializationState>()?.context = context;
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
    fn source_inertial_float_reaches_zero_without_overshoot() {
        let mut previous = source_inertial_float(1.0, 0.0, 0.0, 0.2);
        assert!((previous - 1.0).abs() < 1e-6);
        for step in 1..=20 {
            let current = source_inertial_float(1.0, 0.0, step as f32 * 0.01, 0.2);
            assert!(current >= -1e-6);
            assert!(current <= previous + 1e-6);
            previous = current;
        }
        assert_eq!(source_inertial_float(1.0, 0.0, 0.2, 0.2), 0.0);
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
