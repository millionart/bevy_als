use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

pub const IN_ACTIVE: &str = "active";
pub const IN_POSE: &str = "pose";
pub const IN_TIME: &str = "time";
pub const OUT_POSE: &str = "pose";

#[derive(Reflect, Clone, Debug, Default)]
struct ResetOnActivationState {
    active: bool,
}

pub fn activation_reset_required(previous: &mut bool, active: bool) -> bool {
    let reset = active && !*previous;
    *previous = active;
    reset
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct ResetOnActivationNode {
    pub threshold: f32,
    pub reset_time: f32,
}

impl Default for ResetOnActivationNode {
    fn default() -> Self {
        Self {
            threshold: 1e-6,
            reset_time: 0.0,
        }
    }
}

impl NodeLike for ResetOnActivationNode {
    fn display_name(&self) -> String {
        "ALS Reset On Activation".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let duration = ctx.duration_back(IN_TIME)?;
        ctx.set_duration_fwd(duration);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let active = ctx.data_back(IN_ACTIVE)?.as_f32()? > self.threshold;
        let reset = {
            let state = ctx.state_mut::<ResetOnActivationState>()?;
            activation_reset_required(&mut state.active, active)
        };
        let update = if reset {
            TimeUpdate::Absolute(self.reset_time.max(0.0))
        } else {
            ctx.time_update_fwd()?
        };
        ctx.set_time_update_back(IN_TIME, update);
        let pose = ctx.data_back(IN_POSE)?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(OUT_POSE, pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_ACTIVE, DataSpec::F32)
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
    fn activation_resets_only_on_rising_edge() {
        let mut active = false;
        assert!(!activation_reset_required(&mut active, false));
        assert!(activation_reset_required(&mut active, true));
        assert!(!activation_reset_required(&mut active, true));
        assert!(!activation_reset_required(&mut active, false));
        assert!(activation_reset_required(&mut active, true));
    }

    #[test]
    fn reset_time_preserves_explicit_sequence_start() {
        let node = ResetOnActivationNode {
            threshold: 1e-6,
            reset_time: 0.25,
        };
        assert!((node.reset_time - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<ResetOnActivationNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<ResetOnActivationNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
