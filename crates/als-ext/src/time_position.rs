use bevy::prelude::*;
use bevy_animation_graph::core::animation_graph::TimeUpdate;
use bevy_animation_graph::core::animation_node::{NodeLike, ReflectNodeLike};
use bevy_animation_graph::core::context::{new_context::NodeContext, spec_context::SpecContext};
use bevy_animation_graph::core::edge_data::DataSpec;
use bevy_animation_graph::core::errors::GraphError;

pub const IN_POSE: &str = "pose";
pub const IN_TIME: &str = "time";
pub const IN_POSITION: &str = "position";
pub const OUT_POSE: &str = "pose";

pub fn resolve_position(raw: f32, duration: f32) -> f32 {
    raw.clamp(0.0, duration.max(1e-4))
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct TimePositionNode {
    pub duration_fallback: f32,
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
        let position = resolve_position(raw, duration);

        ctx.set_time(position);
        ctx.set_time_update_back(IN_TIME, TimeUpdate::Absolute(position));

        let mut pose = ctx.data_back(IN_POSE)?.into_pose()?;
        pose.timestamp = position;
        ctx.set_data_fwd(OUT_POSE, pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_POSITION, DataSpec::F32)
            .add_input_data(IN_POSE, DataSpec::Pose)
            .add_input_time(IN_TIME)
            .add_output_data(OUT_POSE, DataSpec::Pose)
            .add_output_time();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
}
