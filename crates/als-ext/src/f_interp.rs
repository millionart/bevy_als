use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};

pub const IN_TARGET: &str = "target";
pub const IN_DELTA_SECONDS: &str = "delta_seconds";
pub const OUT_VALUE: &str = "value";

pub fn f_interp_to(current: f32, target: f32, delta_seconds: f32, speed: f32) -> f32 {
    if speed <= 0.0 {
        return target;
    }
    let distance = target - current;
    if distance * distance < 1e-8 {
        return target;
    }
    current + distance * (delta_seconds * speed).clamp(0.0, 1.0)
}

#[derive(Reflect, Clone, Debug, Default)]
struct FInterpState {
    initialized: bool,
    value: f32,
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct FInterpNode {
    pub increasing_speed: f32,
    pub decreasing_speed: f32,
}

impl Default for FInterpNode {
    fn default() -> Self {
        Self {
            increasing_speed: 1.0,
            decreasing_speed: 1.0,
        }
    }
}

impl FInterpNode {
    fn sample(state: &mut FInterpState, target: f32, delta_seconds: f32, node: &Self) -> f32 {
        if !state.initialized {
            state.initialized = true;
            state.value = target;
            return target;
        }
        let speed = if target >= state.value {
            node.increasing_speed
        } else {
            node.decreasing_speed
        };
        state.value = f_interp_to(state.value, target, delta_seconds, speed);
        state.value
    }
}

impl NodeLike for FInterpNode {
    fn display_name(&self) -> String {
        "ALS FInterp".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let target = ctx.data_back(IN_TARGET)?.as_f32()?;
        let delta_seconds = ctx.data_back(IN_DELTA_SECONDS)?.as_f32()?.max(0.0);
        let value = {
            let state = ctx.state_mut::<FInterpState>()?;
            Self::sample(state, target, delta_seconds, self)
        };
        ctx.set_data_fwd(OUT_VALUE, value);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_TARGET, DataSpec::F32)
            .add_input_data(IN_DELTA_SECONDS, DataSpec::F32)
            .add_output_data(OUT_VALUE, DataSpec::F32);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_matches_unreal_directional_speeds() {
        let node = FInterpNode {
            increasing_speed: 20.0,
            decreasing_speed: 5.0,
        };
        let mut state = FInterpState::default();
        assert_eq!(FInterpNode::sample(&mut state, 1.0, 0.1, &node), 1.0);
        assert!((FInterpNode::sample(&mut state, 0.0, 0.1, &node) - 0.5).abs() < 1e-6);
        assert_eq!(FInterpNode::sample(&mut state, 1.0, 0.1, &node), 1.0);
    }

    #[test]
    fn zero_speed_returns_target() {
        assert_eq!(f_interp_to(0.25, 0.75, 1.0, 0.0), 0.75);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;
        let mut registry = TypeRegistry::default();
        registry.register::<FInterpNode>();
        let registration = registry.get(std::any::TypeId::of::<FInterpNode>()).unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <FInterpNode as bevy::reflect::TypePath>::type_path(),
            "als_ext::FInterpNode"
        );
    }
}
