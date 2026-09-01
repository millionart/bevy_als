use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::{additive::AdditiveInterpolator, linear::LinearInterpolator},
};

use crate::global_f32_input::global_f32_or_root_default;
use crate::{context_time_update, f_interp_to};

#[derive(Reflect, Clone, Debug, Default)]
struct SourceBlendState {
    initialized: bool,
    value: f32,
}

fn delta_seconds(ctx: &NodeContext) -> Result<f32, GraphError> {
    Ok(match context_time_update(ctx)? {
        TimeUpdate::Delta(value) => value.max(0.0),
        TimeUpdate::Absolute(_) | TimeUpdate::PercentOfEvent { .. } => 0.0,
    })
}

fn source_blend_alpha(
    state: &mut SourceBlendState,
    raw: f32,
    mapping_max: f32,
    minimum: f32,
    maximum: f32,
    increasing_speed: f32,
    decreasing_speed: f32,
    delta: f32,
) -> f32 {
    let mapped = if mapping_max > f32::EPSILON {
        (raw / mapping_max).clamp(0.0, 1.0)
    } else {
        raw
    };
    let target = mapped.clamp(minimum, maximum);
    if !state.initialized {
        state.initialized = true;
        state.value = target;
        return target;
    }
    let speed = if target >= state.value {
        increasing_speed
    } else {
        decreasing_speed
    };
    state.value = f_interp_to(state.value, target, delta, speed);
    state.value
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceTwoWayBlendNode {
    pub alpha_key: String,
    pub alpha_pin: String,
    pub mapping_max: f32,
    pub increasing_speed: f32,
    pub decreasing_speed: f32,
}

impl Default for SourceTwoWayBlendNode {
    fn default() -> Self {
        Self {
            alpha_key: default(),
            alpha_pin: "Alpha".to_owned(),
            mapping_max: 0.0,
            increasing_speed: 0.0,
            decreasing_speed: 0.0,
        }
    }
}

impl NodeLike for SourceTwoWayBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let raw = global_f32_or_root_default(&ctx, &self.alpha_key)?;
        let delta = delta_seconds(&ctx)?;
        let alpha = source_blend_alpha(
            ctx.state_mut::<SourceBlendState>()?,
            raw,
            self.mapping_max,
            0.0,
            1.0,
            self.increasing_speed,
            self.decreasing_speed,
            delta,
        );
        let mut base = ctx.data_back("A")?.into_pose()?;
        let overlay = ctx.data_back("B")?.into_pose()?;
        LinearInterpolator {
            bone_mask: BoneMask::all(),
        }
        .interpolate_pose(&mut base, &overlay, alpha);
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd("Pose", base);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("A", DataSpec::Pose)
            .add_input_data("B", DataSpec::Pose)
            .add_input_data(self.alpha_pin.clone(), DataSpec::F32)
            .add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Two Way Blend".to_owned()
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceApplyAdditiveNode {
    pub alpha_key: String,
    pub alpha_pin: String,
    pub expose_alpha_pin: bool,
    pub use_constant_alpha: bool,
    pub constant_alpha: f32,
    pub minimum_alpha: f32,
    pub maximum_alpha: f32,
    pub increasing_speed: f32,
    pub decreasing_speed: f32,
}

impl Default for SourceApplyAdditiveNode {
    fn default() -> Self {
        Self {
            alpha_key: default(),
            alpha_pin: "Alpha".to_owned(),
            expose_alpha_pin: true,
            use_constant_alpha: false,
            constant_alpha: 0.0,
            minimum_alpha: 0.0,
            maximum_alpha: 1.0,
            increasing_speed: 0.0,
            decreasing_speed: 0.0,
        }
    }
}

impl NodeLike for SourceApplyAdditiveNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let raw = if self.use_constant_alpha {
            self.constant_alpha
        } else {
            global_f32_or_root_default(&ctx, &self.alpha_key)?
        };
        let delta = delta_seconds(&ctx)?;
        let alpha = source_blend_alpha(
            ctx.state_mut::<SourceBlendState>()?,
            raw,
            0.0,
            self.minimum_alpha,
            self.maximum_alpha,
            self.increasing_speed,
            self.decreasing_speed,
            delta,
        );
        let mut base = ctx.data_back("Base")?.into_pose()?;
        let additive = ctx.data_back("Additive")?.into_pose()?;
        AdditiveInterpolator {
            bone_mask: BoneMask::all(),
        }
        .interpolate_pose(&mut base, &additive, alpha);
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd("Pose", base);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Base", DataSpec::Pose)
            .add_input_data("Additive", DataSpec::Pose);
        if self.expose_alpha_pin {
            ctx.add_input_data(self.alpha_pin.clone(), DataSpec::F32);
        }
        ctx.add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Apply Additive".to_owned()
    }
}

#[derive(Reflect, Clone, Debug, Default)]
struct SourceTagBlendState {
    initialized: bool,
    sprinting: bool,
    elapsed: f32,
    duration: f32,
    start: f32,
}

fn source_tag_weight(
    state: &mut SourceTagBlendState,
    sprinting: bool,
    default_blend_time: f32,
    sprinting_blend_time: f32,
    delta: f32,
) -> f32 {
    if !state.initialized {
        state.initialized = true;
        state.sprinting = sprinting;
        return sprinting as u8 as f32;
    }
    let current = if state.duration <= f32::EPSILON {
        state.sprinting as u8 as f32
    } else {
        let alpha = (state.elapsed / state.duration).clamp(0.0, 1.0);
        let smooth = alpha * alpha * (3.0 - 2.0 * alpha);
        state.start + ((state.sprinting as u8 as f32) - state.start) * smooth
    };
    if sprinting != state.sprinting {
        state.start = current;
        state.sprinting = sprinting;
        state.elapsed = 0.0;
        state.duration = if sprinting {
            sprinting_blend_time.max(0.0)
        } else {
            default_blend_time.max(0.0)
        };
    }
    if state.duration <= f32::EPSILON {
        return sprinting as u8 as f32;
    }
    state.elapsed = (state.elapsed + delta).min(state.duration);
    let alpha = (state.elapsed / state.duration).clamp(0.0, 1.0);
    let smooth = alpha * alpha * (3.0 - 2.0 * alpha);
    state.start + ((state.sprinting as u8 as f32) - state.start) * smooth
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceGameplayTagBlendNode {
    pub active_key: String,
    pub default_blend_time: f32,
    pub sprinting_blend_time: f32,
}

impl Default for SourceGameplayTagBlendNode {
    fn default() -> Self {
        Self {
            active_key: default(),
            default_blend_time: 0.3,
            sprinting_blend_time: 0.2,
        }
    }
}

impl NodeLike for SourceGameplayTagBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let sprinting = global_f32_or_root_default(&ctx, &self.active_key)? > 0.5;
        let delta = delta_seconds(&ctx)?;
        let weight = source_tag_weight(
            ctx.state_mut::<SourceTagBlendState>()?,
            sprinting,
            self.default_blend_time,
            self.sprinting_blend_time,
            delta,
        );
        let mut base = ctx.data_back("Default Pose")?.into_pose()?;
        let sprint = ctx.data_back("Sprinting Pose")?.into_pose()?;
        LinearInterpolator {
            bone_mask: BoneMask::all(),
        }
        .interpolate_pose(&mut base, &sprint, weight);
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd("Pose", base);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Default Pose", DataSpec::Pose)
            .add_input_data("Sprinting Pose", DataSpec::Pose)
            .add_input_data("Default Blend Time", DataSpec::F32)
            .add_input_data("Sprinting Blend Time", DataSpec::F32)
            .add_input_data("Active Tag", DataSpec::F32)
            .add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Gameplay Tag Blend".to_owned()
    }
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceCurveModifyNode {
    pub curve_name: String,
    pub value: f32,
}

impl NodeLike for SourceCurveModifyNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let pose = ctx.data_back("Source Pose")?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Source Pose", DataSpec::Pose)
            .add_input_data(self.curve_name.clone(), DataSpec::F32)
            .add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Curve Modify".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_two_way_alpha_matches_range_and_directional_interpolation() {
        let mut state = SourceBlendState::default();
        assert_eq!(
            source_blend_alpha(&mut state, 0.125, 0.25, 0.0, 1.0, 20.0, 4.0, 0.1),
            0.5
        );
        assert!(
            (source_blend_alpha(&mut state, 0.0, 0.25, 0.0, 1.0, 20.0, 4.0, 0.1) - 0.3).abs()
                < 1e-6
        );
    }

    #[test]
    fn source_tag_blend_switches_between_default_and_sprinting() {
        let mut state = SourceTagBlendState::default();
        assert_eq!(source_tag_weight(&mut state, false, 0.3, 0.2, 0.0), 0.0);
        assert!((source_tag_weight(&mut state, true, 0.3, 0.2, 0.1) - 0.5).abs() < 1e-6);
        assert_eq!(source_tag_weight(&mut state, true, 0.3, 0.2, 0.1), 1.0);
    }

    #[test]
    fn source_apply_additive_supports_a_hidden_constant_alpha() {
        let node = SourceApplyAdditiveNode {
            alpha_key: String::new(),
            alpha_pin: "Alpha".to_owned(),
            expose_alpha_pin: false,
            use_constant_alpha: true,
            constant_alpha: 1.0,
            minimum_alpha: 0.0,
            maximum_alpha: 1.0,
            increasing_speed: 0.0,
            decreasing_speed: 0.0,
        };
        assert!(!node.expose_alpha_pin);
        assert!(node.use_constant_alpha);
        assert_eq!(node.constant_alpha, 1.0);
    }
}
