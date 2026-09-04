use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::{additive::AdditiveInterpolator, linear::LinearInterpolator},
};

use crate::animation_context::{AnimationContextState, pose_input_context_state};
use crate::curve_bundle::blend_curve_value;
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
    pub pin_default: f32,
    pub mapping_max: f32,
    pub increasing_speed: f32,
    pub decreasing_speed: f32,
}

impl Default for SourceTwoWayBlendNode {
    fn default() -> Self {
        Self {
            alpha_key: default(),
            alpha_pin: "Alpha".to_owned(),
            pin_default: 0.0,
            mapping_max: 0.0,
            increasing_speed: 0.0,
            decreasing_speed: 0.0,
        }
    }
}

impl NodeLike for SourceTwoWayBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let raw = crate::global_f32_input::global_f32_or_root_default_value(
            &ctx,
            &self.alpha_key,
            self.pin_default,
        )?;
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
    pub expose_source_alpha_controls: bool,
    pub alpha_curve_name: String,
    pub alpha_bool_enabled: bool,
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
            expose_source_alpha_controls: false,
            alpha_curve_name: "None".to_owned(),
            alpha_bool_enabled: true,
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
        if self.expose_source_alpha_controls {
            ctx.add_input_data("AlphaCurveName", DataSpec::F32)
                .add_input_data("bAlphaBoolEnabled", DataSpec::F32);
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
    pub curve_names: Vec<String>,
    pub value: f32,
    pub pin_default: f32,
    pub pin_defaults: Vec<f32>,
    pub value_pin_names: Vec<String>,
    pub value_key: String,
    pub value_keys: Vec<String>,
    pub source_value_path: String,
    pub source_value_paths: Vec<String>,
    pub alpha: f32,
    pub alpha_pin: String,
    pub expose_alpha_pin: bool,
    pub apply_mode: String,
}

impl SourceCurveModifyNode {
    fn curve_pin_names(&self) -> Vec<String> {
        if self.curve_names.is_empty() {
            vec![self.curve_name.clone()]
        } else {
            self.curve_names.clone()
        }
    }

    fn value_keys(&self) -> Vec<String> {
        if self.value_keys.is_empty() {
            vec![self.value_key.clone()]
        } else {
            self.value_keys.clone()
        }
    }

    fn value_pin_names(&self) -> Vec<String> {
        let curve_names = self.curve_pin_names();
        if self.value_pin_names.len() == curve_names.len()
            && self.value_pin_names.iter().all(|value| !value.is_empty())
        {
            self.value_pin_names.clone()
        } else {
            curve_names
        }
    }

    fn alpha_value(&self, ctx: &NodeContext) -> Result<f32, GraphError> {
        if !self.expose_alpha_pin {
            return Ok(self.alpha);
        }
        match ctx.data_back(self.alpha_pin.clone()) {
            Ok(value) => value
                .as_f32()
                .map(|value| resolved_curve_alpha(Some(value), self.alpha)),
            Err(_) => Ok(resolved_curve_alpha(None, self.alpha)),
        }
    }
}

fn source_curve_modify_value(source: f32, target: f32, alpha: f32, apply_mode: &str) -> f32 {
    let value = if apply_mode.eq_ignore_ascii_case("Add") {
        source + target
    } else if apply_mode.eq_ignore_ascii_case("Scale") {
        source * target
    } else if apply_mode.eq_ignore_ascii_case("RemapCurve") {
        ((source - target).max(0.0) / (1.0 - target).max(0.01)).min(1.0)
    } else {
        target
    };
    blend_curve_value(source, value, alpha.clamp(0.0, 1.0))
}

fn resolved_curve_alpha(pin_value: Option<f32>, fallback: f32) -> f32 {
    pin_value.unwrap_or(fallback)
}

impl NodeLike for SourceCurveModifyNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let pose = ctx.data_back("Source Pose")?.into_pose()?;
        let mut context = pose_input_context_state(&ctx, "Source Pose").unwrap_or_default();
        let curve_names = self.curve_pin_names();
        let value_keys = self.value_keys();
        let value_pin_names = self.value_pin_names();
        let alpha = self.alpha_value(&ctx)?;
        for (index, curve_name) in curve_names.iter().enumerate() {
            let default = self
                .pin_defaults
                .get(index)
                .copied()
                .unwrap_or(self.pin_default);
            let input_value = value_pin_names
                .get(index)
                .and_then(|pin| ctx.data_back(pin.clone()).ok())
                .and_then(|value| value.as_f32().ok());
            let target = if let Some(value) = input_value {
                value
            } else if let Some(key) = value_keys.get(index).filter(|key| !key.is_empty()) {
                global_f32_or_root_default(&ctx, key)?
            } else {
                default
            };
            let source = context.curves.get(curve_name).copied().unwrap_or(default);
            let value = source_curve_modify_value(source, target, alpha, &self.apply_mode);
            context.curves.insert(curve_name.clone(), value);
        }
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
        *ctx.state_mut_or_else(AnimationContextState::default)? = context;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Source Pose", DataSpec::Pose);
        for pin_name in self.value_pin_names() {
            ctx.add_input_data(pin_name, DataSpec::F32);
        }
        if self.expose_alpha_pin {
            ctx.add_input_data(self.alpha_pin.clone(), DataSpec::F32);
        }
        ctx.add_output_data("Pose", DataSpec::Pose);
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
            expose_source_alpha_controls: false,
            alpha_curve_name: "None".to_owned(),
            alpha_bool_enabled: true,
        };
        assert!(!node.expose_alpha_pin);
        assert!(node.use_constant_alpha);
        assert_eq!(node.constant_alpha, 1.0);
    }

    #[test]
    fn source_curve_modify_retains_multiple_source_curve_pins() {
        let node = SourceCurveModifyNode {
            curve_name: String::new(),
            curve_names: vec![
                "FootLeftLock".to_owned(),
                "FootRightLock".to_owned(),
                "AllowTransitions".to_owned(),
            ],
            pin_defaults: vec![1.0, 1.0, 1.0],
            value_keys: vec![String::new(), String::new(), String::new()],
            source_value_paths: vec![String::new(), String::new(), String::new()],
            ..default()
        };
        assert_eq!(
            node.curve_pin_names(),
            vec![
                "FootLeftLock".to_owned(),
                "FootRightLock".to_owned(),
                "AllowTransitions".to_owned(),
            ]
        );
        assert_eq!(
            node.value_pin_names(),
            vec![
                "FootLeftLock".to_owned(),
                "FootRightLock".to_owned(),
                "AllowTransitions".to_owned(),
            ]
        );
    }

    #[test]
    fn source_curve_modify_can_preserve_source_curve_value_pin_names() {
        let node = SourceCurveModifyNode {
            curve_names: vec!["PoseStanding".to_owned(), "PoseInAir".to_owned()],
            value_pin_names: vec!["CurveValues_0".to_owned(), "CurveValues_1".to_owned()],
            ..default()
        };
        assert_eq!(
            node.value_pin_names(),
            vec!["CurveValues_0".to_owned(), "CurveValues_1".to_owned()]
        );
    }

    #[test]
    fn source_curve_modify_matches_engine_apply_modes() {
        assert_eq!(source_curve_modify_value(2.0, 3.0, 1.0, "Add"), 5.0);
        assert_eq!(source_curve_modify_value(2.0, 3.0, 1.0, "Scale"), 6.0);
        assert_eq!(source_curve_modify_value(0.2, 0.8, 1.0, "Blend"), 0.8);
        assert!((source_curve_modify_value(0.9, 0.5, 1.0, "RemapCurve") - 0.8).abs() < 1e-6);
        assert_eq!(source_curve_modify_value(2.0, 3.0, 0.25, "Scale"), 3.0);
    }

    #[test]
    fn source_curve_modify_uses_visible_alpha_pin_before_node_default() {
        assert_eq!(resolved_curve_alpha(Some(0.25), 1.0), 0.25);
        assert_eq!(resolved_curve_alpha(None, 1.0), 1.0);
    }
}
