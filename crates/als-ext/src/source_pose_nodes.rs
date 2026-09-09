use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::{additive::AdditiveInterpolator, linear::LinearInterpolator},
};
use serde::{Deserialize, Serialize};

use crate::animation_context::{
    AnimationContextState, merge_inertialization_request, merge_sync_players, pose_input_context_state, select_relevant_asset,
};
use crate::curve_bundle::blend_curve_value;
use crate::global_f32_input::{global_f32_or_root_default, global_f32_or_root_default_value};
use crate::mesh_space_additive::{apply_mesh_space_additive, materialize_missing_reference_bones};
use crate::mesh_space_difference::make_mesh_space_difference;
use crate::reference_local_difference::make_local_difference;
use crate::{
    context_time_update, f_interp_to,
    gameplay_tag_blend::{
        GameplayTagBlendMode, GameplayTagBlendState, advance_gameplay_tag_weights,
        select_gameplay_tag_child_index_for_tag,
    },
    source_gameplay_tag_input::gameplay_tag_input,
};

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct SourceBlendState {
    initialized: bool,
    value: f32,
    pub(crate) context: AnimationContextState,
}

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct SourceTwoWayBlendState {
    alpha: SourceBlendState,
    a_is_relevant: bool,
    b_is_relevant: bool,
    pub(crate) context: AnimationContextState,
}

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct SourceGameplayTagBlendState {
    blend: GameplayTagBlendState,
    pub(crate) context: AnimationContextState,
}

#[derive(Reflect, Clone, Debug, Default)]
pub struct SourceInputScaleBiasClampState {
    initialized: bool,
    value: f32,
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default)]
pub struct SourceAlphaMapping {
    pub source_semantics: bool,
    pub map_range: bool,
    pub input_min: f32,
    pub input_max: f32,
    pub output_min: f32,
    pub output_max: f32,
    pub scale: f32,
    pub bias: f32,
    pub clamp_result: bool,
    pub clamp_min: f32,
    pub clamp_max: f32,
    pub interpolate_result: bool,
    pub interpolate_speed_increasing: f32,
    pub interpolate_speed_decreasing: f32,
}

impl Default for SourceAlphaMapping {
    fn default() -> Self {
        Self {
            source_semantics: false,
            map_range: false,
            input_min: 0.0,
            input_max: 1.0,
            output_min: 0.0,
            output_max: 1.0,
            scale: 1.0,
            bias: 0.0,
            clamp_result: false,
            clamp_min: 0.0,
            clamp_max: 1.0,
            interpolate_result: false,
            interpolate_speed_increasing: 10.0,
            interpolate_speed_decreasing: 10.0,
        }
    }
}

fn delta_seconds(ctx: &NodeContext) -> Result<f32, GraphError> {
    Ok(match context_time_update(ctx)? {
        TimeUpdate::Delta(value) => value.max(0.0),
        TimeUpdate::Absolute(_) | TimeUpdate::PercentOfEvent { .. } => 0.0,
    })
}

pub fn apply_source_input_scale_bias_clamp(
    state: &mut SourceInputScaleBiasClampState,
    raw: f32,
    mapping: &SourceAlphaMapping,
    delta: f32,
) -> f32 {
    if !mapping.source_semantics {
        return raw;
    }
    let mut target = raw;
    if mapping.map_range {
        let divisor = mapping.input_max - mapping.input_min;
        let percent = if divisor.abs() <= 1.0e-8 {
            if raw >= mapping.input_max { 1.0 } else { 0.0 }
        } else {
            (raw - mapping.input_min) / divisor
        };
        target = mapping.output_min + (mapping.output_max - mapping.output_min) * percent;
    }
    target = target * mapping.scale + mapping.bias;
    if mapping.clamp_result {
        target = target.clamp(mapping.clamp_min, mapping.clamp_max);
    }
    if mapping.interpolate_result {
        if state.initialized {
            let speed = if target >= state.value {
                mapping.interpolate_speed_increasing
            } else {
                mapping.interpolate_speed_decreasing
            };
            target = f_interp_to(state.value, target, delta, speed);
        }
        state.value = target;
    }
    state.initialized = true;
    target
}

fn source_blend_alpha(
    state: &mut SourceBlendState,
    raw: f32,
    mapping_max: f32,
    mapping: &SourceAlphaMapping,
    alpha_scale: f32,
    alpha_bias: f32,
    minimum: f32,
    maximum: f32,
    increasing_speed: f32,
    decreasing_speed: f32,
    delta: f32,
) -> f32 {
    if mapping.source_semantics {
        let mut mapping_state = SourceInputScaleBiasClampState {
            initialized: state.initialized,
            value: state.value,
        };
        let target = apply_source_input_scale_bias_clamp(&mut mapping_state, raw, mapping, delta);
        state.initialized = mapping_state.initialized;
        state.value = mapping_state.value;
        return (target * alpha_scale + alpha_bias).clamp(0.0, 1.0);
    }
    let mapped = if mapping_max > f32::EPSILON {
        (raw / mapping_max).clamp(0.0, 1.0)
    } else {
        raw
    };
    let target = mapped.clamp(minimum, maximum);
    if !state.initialized {
        state.initialized = true;
        state.value = target;
        return (target * alpha_scale + alpha_bias).clamp(0.0, 1.0);
    }
    let speed = if target >= state.value {
        increasing_speed
    } else {
        decreasing_speed
    };
    state.value = f_interp_to(state.value, target, delta, speed);
    (state.value * alpha_scale + alpha_bias).clamp(0.0, 1.0)
}

fn source_two_way_child_relevancy(
    state: &mut SourceTwoWayBlendState,
    alpha: f32,
) -> (bool, bool, bool, bool) {
    const ZERO_ANIMWEIGHT_THRESHOLD: f32 = 1.0e-5;

    let a_is_relevant = alpha < 1.0 - ZERO_ANIMWEIGHT_THRESHOLD;
    let b_is_relevant = alpha > ZERO_ANIMWEIGHT_THRESHOLD;
    let reset_a = a_is_relevant && !state.a_is_relevant;
    let reset_b = b_is_relevant && !state.b_is_relevant;
    state.a_is_relevant = a_is_relevant;
    state.b_is_relevant = b_is_relevant;
    (a_is_relevant, b_is_relevant, reset_a, reset_b)
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceTwoWayBlendNode {
    pub alpha_key: String,
    pub alpha_pin: String,
    pub expose_alpha_pin: bool,
    pub pin_default: f32,
    pub mapping_max: f32,
    pub mapping: SourceAlphaMapping,
    pub alpha_scale: f32,
    pub alpha_bias: f32,
    pub increasing_speed: f32,
    pub decreasing_speed: f32,
    pub reset_child_on_activation: bool,
}

impl Default for SourceTwoWayBlendNode {
    fn default() -> Self {
        Self {
            alpha_key: default(),
            alpha_pin: "Alpha".to_owned(),
            expose_alpha_pin: true,
            pin_default: 0.0,
            mapping_max: 0.0,
            mapping: default(),
            alpha_scale: 1.0,
            alpha_bias: 0.0,
            increasing_speed: 0.0,
            decreasing_speed: 0.0,
            reset_child_on_activation: false,
        }
    }
}

impl NodeLike for SourceTwoWayBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let raw = if self.expose_alpha_pin {
            ctx.data_back(self.alpha_pin.clone())
                .and_then(|value| value.as_f32())
                .or_else(|_| {
                    crate::global_f32_input::global_f32_or_root_default_value(
                        &ctx,
                        &self.alpha_key,
                        self.pin_default,
                    )
                })?
        } else {
            crate::global_f32_input::global_f32_or_root_default_value(
                &ctx,
                &self.alpha_key,
                self.pin_default,
            )?
        };
        let delta = delta_seconds(&ctx)?;
        let (alpha, a_is_relevant, b_is_relevant, reset_a, reset_b) = {
            let state = ctx.state_mut::<SourceTwoWayBlendState>()?;
            let alpha = source_blend_alpha(
                &mut state.alpha,
                raw,
                self.mapping_max,
                &self.mapping,
                self.alpha_scale,
                self.alpha_bias,
                0.0,
                1.0,
                self.increasing_speed,
                self.decreasing_speed,
                delta,
            );
            let (a_is_relevant, b_is_relevant, reset_a, reset_b) =
                source_two_way_child_relevancy(state, alpha);
            (alpha, a_is_relevant, b_is_relevant, reset_a, reset_b)
        };
        if self.reset_child_on_activation {
            for (pin, reset) in [("A", reset_a), ("B", reset_b)] {
                if !reset {
                    continue;
                }
                let node = crate::animation_context::source_node_for_pose_input(&ctx, pin)?;
                if ctx.graph_context.graph.nodes[&node]
                    .try_inner_downcast_ref::<crate::SourceSequencePlayerNode>()
                    .is_none()
                {
                    return Err(GraphError::UnknownErrorNode {
                        graph: ctx.graph_context.context_id,
                        node: ctx.node_id,
                        error: "Source child initialization requires a direct sequence player"
                            .into(),
                    });
                }
                *ctx.graph_context
                    .create_node_context(node)
                    .state_mut::<crate::source_sequence_player::SourceSequencePlayerState>()? =
                    default();
            }
        }
        let mut base = if a_is_relevant {
            ctx.data_back("A")?.into_pose()?
        } else {
            ctx.data_back("B")?.into_pose()?
        };
        if b_is_relevant && a_is_relevant {
            let overlay = ctx.data_back("B")?.into_pose()?;
            LinearInterpolator {
                bone_mask: BoneMask::all(),
            }
            .interpolate_pose(&mut base, &overlay, alpha);
        }
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd("Pose", base);
        let context_a = a_is_relevant
            .then(|| pose_input_context_state(&ctx, "A"))
            .flatten();
        let context_b = b_is_relevant
            .then(|| pose_input_context_state(&ctx, "B"))
            .flatten();
        let context = blend_animation_contexts([
            (
                context_a.unwrap_or_default(),
                if b_is_relevant { 1.0 - alpha } else { 1.0 },
            ),
            (
                context_b.unwrap_or_default(),
                if a_is_relevant { alpha } else { 1.0 },
            ),
        ]);
        ctx.state_mut::<SourceTwoWayBlendState>()?.context = context;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("A", DataSpec::Pose)
            .add_input_data("B", DataSpec::Pose);
        if self.expose_alpha_pin {
            ctx.add_input_data(self.alpha_pin.clone(), DataSpec::F32);
        }
        ctx.add_output_data("Pose", DataSpec::Pose);
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
    pub mapping: SourceAlphaMapping,
    pub alpha_scale: f32,
    pub alpha_bias: f32,
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
            mapping: default(),
            alpha_scale: 1.0,
            alpha_bias: 0.0,
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
        } else if self.expose_alpha_pin {
            ctx.data_back(self.alpha_pin.clone())
                .and_then(|value| value.as_f32())
                .or_else(|_| global_f32_or_root_default(&ctx, &self.alpha_key))?
        } else {
            global_f32_or_root_default(&ctx, &self.alpha_key)?
        };
        let delta = delta_seconds(&ctx)?;
        let alpha = source_blend_alpha(
            ctx.state_mut::<SourceBlendState>()?,
            raw,
            0.0,
            &self.mapping,
            self.alpha_scale,
            self.alpha_bias,
            self.minimum_alpha,
            self.maximum_alpha,
            self.increasing_speed,
            self.decreasing_speed,
            delta,
        );
        let mut base = ctx.data_back("Base")?.into_pose()?;
        let additive_is_relevant = alpha > 1e-5;
        if additive_is_relevant {
            let additive = ctx.data_back("Additive")?.into_pose()?;
            AdditiveInterpolator {
                bone_mask: BoneMask::all(),
            }
            .interpolate_pose(&mut base, &additive, alpha);
        }
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd("Pose", base);
        let base_context = pose_input_context_state(&ctx, "Base");
        let additive_context = additive_is_relevant
            .then(|| pose_input_context_state(&ctx, "Additive"))
            .flatten();
        let context = blend_animation_contexts([
            (base_context.unwrap_or_default(), 1.0),
            (additive_context.unwrap_or_default(), alpha),
        ]);
        ctx.state_mut::<SourceBlendState>()?.context = context;
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

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceMeshSpaceAdditiveNode {
    pub alpha_key: String,
    pub alpha_pin: String,
    pub expose_alpha_pin: bool,
    pub use_constant_alpha: bool,
    pub constant_alpha: f32,
    pub mapping: SourceAlphaMapping,
    pub alpha_scale: f32,
    pub alpha_bias: f32,
    pub minimum_alpha: f32,
    pub maximum_alpha: f32,
    pub increasing_speed: f32,
    pub decreasing_speed: f32,
    pub expose_source_alpha_controls: bool,
    pub alpha_curve_name: String,
    pub alpha_bool_enabled: bool,
}

impl Default for SourceMeshSpaceAdditiveNode {
    fn default() -> Self {
        Self {
            alpha_key: default(),
            alpha_pin: "Alpha".to_owned(),
            expose_alpha_pin: true,
            use_constant_alpha: false,
            constant_alpha: 0.0,
            mapping: default(),
            alpha_scale: 1.0,
            alpha_bias: 0.0,
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

impl NodeLike for SourceMeshSpaceAdditiveNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let raw = if self.use_constant_alpha {
            self.constant_alpha
        } else if self.expose_alpha_pin {
            ctx.data_back(self.alpha_pin.clone())
                .and_then(|value| value.as_f32())
                .or_else(|_| global_f32_or_root_default(&ctx, &self.alpha_key))?
        } else {
            global_f32_or_root_default(&ctx, &self.alpha_key)?
        };
        let delta = delta_seconds(&ctx)?;
        let alpha = source_blend_alpha(
            ctx.state_mut::<SourceBlendState>()?,
            raw,
            0.0,
            &self.mapping,
            self.alpha_scale,
            self.alpha_bias,
            self.minimum_alpha,
            self.maximum_alpha,
            self.increasing_speed,
            self.decreasing_speed,
            delta,
        );
        let mut base = ctx.data_back("Base")?.into_pose()?;
        if let Some(skeleton) = ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&base.skeleton)
        {
            materialize_missing_reference_bones(&mut base, skeleton);
        }
        if alpha.abs() > f32::EPSILON {
            let additive = ctx.data_back("Additive")?.into_pose()?;
            if ctx
                .graph_context
                .resources
                .skeleton_assets
                .get(&base.skeleton)
                .is_none()
            {
                base.skeleton = additive.skeleton.clone();
            }
            let skeleton = ctx
                .graph_context
                .resources
                .skeleton_assets
                .get(&base.skeleton)
                .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
            apply_mesh_space_additive(&mut base, &additive, alpha, skeleton);
        }
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd("Pose", base);
        let base_context = pose_input_context_state(&ctx, "Base");
        let additive_context = pose_input_context_state(&ctx, "Additive");
        let additive_is_relevant = alpha > f32::EPSILON;
        let mut context = if additive_is_relevant {
            additive_context.clone().or(base_context.clone())
        } else {
            base_context.clone().or(additive_context.clone())
        }
        .unwrap_or_default();
        context.relevant_asset = select_relevant_asset([
            (
                additive_context.and_then(|context| context.relevant_asset),
                if additive_is_relevant { 1.0 } else { 0.0 },
            ),
            (
                base_context.and_then(|context| context.relevant_asset),
                if additive_is_relevant { 0.0 } else { 1.0 },
            ),
        ]);
        ctx.state_mut::<SourceBlendState>()?.context = context;
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
        "ALS Source Mesh Space Additive".to_owned()
    }
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceMakeDynamicAdditiveNode {
    pub mesh_space_additive: bool,
}

fn make_dynamic_additive_difference(
    base: &bevy_animation_graph::core::pose::Pose,
    additive: &bevy_animation_graph::core::pose::Pose,
    skeleton: &bevy_animation_graph::core::skeleton::Skeleton,
    mesh_space_additive: bool,
) -> bevy_animation_graph::core::pose::Pose {
    if mesh_space_additive {
        make_mesh_space_difference(base, additive, skeleton)
    } else {
        make_local_difference(base, additive, skeleton)
    }
}

impl NodeLike for SourceMakeDynamicAdditiveNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let mut base = ctx.data_back("Base")?.into_pose()?;
        let additive = ctx.data_back("Additive")?.into_pose()?;
        if ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&base.skeleton)
            .is_none()
        {
            base.skeleton = additive.skeleton.clone();
        }
        let skeleton = ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&base.skeleton)
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        let difference =
            make_dynamic_additive_difference(&base, &additive, skeleton, self.mesh_space_additive);
        ctx.set_time(difference.timestamp);
        ctx.set_data_fwd("Pose", difference);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Base", DataSpec::Pose)
            .add_input_data("Additive", DataSpec::Pose)
            .add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Make Dynamic Additive".to_owned()
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Default, Serialize)]
pub enum SourceCurvesBlendMode {
    #[default]
    Accumulate,
    Override,
}

fn blend_source_curve_context(
    mut base: AnimationContextState,
    curve: &AnimationContextState,
    blend_amount: f32,
    blend_mode: SourceCurvesBlendMode,
) -> AnimationContextState {
    merge_sync_players(&mut base.sync_players, &curve.sync_players, blend_amount);
    if blend_amount > 1e-5 {
        merge_inertialization_request(&mut base.inertialization_request, curve.inertialization_request.as_ref());
    }
    match blend_mode {
        SourceCurvesBlendMode::Accumulate => {
            for (name, value) in &curve.curves {
                *base.curves.entry(name.clone()).or_default() += value * blend_amount;
            }
            base.curves_are_authoritative |= curve.curves_are_authoritative;
            base
        }
        SourceCurvesBlendMode::Override => AnimationContextState {
            curves: curve
                .curves
                .iter()
                .map(|(name, value)| (name.clone(), value * blend_amount))
                .collect(),
            relevant_asset: curve.relevant_asset.clone(),
            curves_are_authoritative: curve.curves_are_authoritative,
            sync_players: base.sync_players,
            inertialization_request: base.inertialization_request,
        },
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceCurvesBlendNode {
    pub blend_amount: f32,
    pub blend_mode: SourceCurvesBlendMode,
}

impl Default for SourceCurvesBlendNode {
    fn default() -> Self {
        Self {
            blend_amount: 1.0,
            blend_mode: default(),
        }
    }
}

impl NodeLike for SourceCurvesBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let base = ctx.data_back("BasePose")?.into_pose()?;
        let base_context = pose_input_context_state(&ctx, "BasePose").unwrap_or_default();
        let context = if self.blend_amount.abs() <= f32::EPSILON {
            base_context
        } else {
            ctx.data_back("CurvePose")?.into_pose()?;
            blend_source_curve_context(
                base_context,
                &pose_input_context_state(&ctx, "CurvePose").unwrap_or_default(),
                self.blend_amount,
                self.blend_mode,
            )
        };
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd("Pose", base);
        *ctx.state_mut_or_else(AnimationContextState::default)? = context;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("BasePose", DataSpec::Pose)
            .add_input_data("CurvePose", DataSpec::Pose)
            .add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Curves Blend".to_owned()
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceGameplayTagBlendNode {
    pub active_key: String,
    pub active_tag_pin: String,
    pub tags: Vec<String>,
    pub blend_times: Vec<f32>,
    pub source_blend_times: Vec<f32>,
    pub blend_mode: GameplayTagBlendMode,
    pub pose_pin_names: Vec<String>,
    pub blend_time_pin_names: Vec<String>,
    pub output_pin: String,
}

impl Default for SourceGameplayTagBlendNode {
    fn default() -> Self {
        Self {
            active_key: default(),
            active_tag_pin: "Active Tag".to_owned(),
            tags: default(),
            blend_times: vec![0.0],
            source_blend_times: vec![0.0],
            blend_mode: default(),
            pose_pin_names: vec!["Default Pose".to_owned()],
            blend_time_pin_names: vec!["Default Blend Time".to_owned()],
            output_pin: "Pose".to_owned(),
        }
    }
}

impl SourceGameplayTagBlendNode {
    fn child_count(&self) -> usize {
        self.tags.len() + 1
    }

    fn validate(&self, ctx: &NodeContext) -> Result<(), GraphError> {
        let count = self.child_count();
        if self.active_key.is_empty()
            || self.output_pin.is_empty()
            || self.blend_times.len() != count
            || self.source_blend_times.len() != count
            || self.pose_pin_names.len() != count
            || self.blend_time_pin_names.len() != count
        {
            return Err(GraphError::UnknownErrorNode {
                graph: ctx.graph_context.context_id,
                node: ctx.node_id,
                error: "gameplay tag blend configuration is invalid".to_owned(),
            });
        }
        Ok(())
    }

    fn blend_time(&self, ctx: &NodeContext, index: usize) -> f32 {
        ctx.data_back(self.blend_time_pin_names[index].clone())
            .ok()
            .and_then(|value| value.as_f32().ok())
            .unwrap_or(self.blend_times[index])
            .max(0.0)
    }

    fn active_index(&self, root_entity: Entity) -> Option<usize> {
        gameplay_tag_input(root_entity, &self.active_key)
            .map(|active| select_gameplay_tag_child_index_for_tag(&self.tags, Some(&active)))
    }
}

pub fn blend_animation_contexts(
    contexts: impl IntoIterator<Item = (AnimationContextState, f32)>,
) -> AnimationContextState {
    let mut output = AnimationContextState::default();
    let mut relevant_assets = Vec::new();
    for (context, weight) in contexts {
        let weight = weight.max(0.0);
        if weight > 1e-5 {
            merge_inertialization_request(&mut output.inertialization_request, context.inertialization_request.as_ref());
        }
        merge_sync_players(&mut output.sync_players, &context.sync_players, weight);
        output.curves_are_authoritative |= context.curves_are_authoritative;
        for (name, value) in context.curves {
            *output.curves.entry(name).or_default() += value * weight;
        }
        relevant_assets.push((context.relevant_asset, weight));
    }
    output.relevant_asset = select_relevant_asset(relevant_assets);
    output
}

impl NodeLike for SourceGameplayTagBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        self.validate(&ctx)?;
        let active_index = self
            .active_index(ctx.graph_context.root_entity)
            .ok_or_else(|| GraphError::GlobalInputDataMissing(self.active_key.clone()))?;
        let delta = delta_seconds(&ctx)?;
        let blend_time = self.blend_time(&ctx, active_index);
        let weights = advance_gameplay_tag_weights(
            &mut ctx.state_mut::<SourceGameplayTagBlendState>()?.blend,
            self.child_count(),
            active_index,
            blend_time,
            self.blend_mode,
            delta,
        );
        let mut output = None;
        let mut accumulated_weight = 0.0;
        let mut contexts = Vec::new();
        for (index, weight) in weights.into_iter().enumerate() {
            if weight <= f32::EPSILON {
                continue;
            }
            let pose = ctx
                .data_back(self.pose_pin_names[index].clone())?
                .into_pose()?;
            if let Some(context) = pose_input_context_state(&ctx, &self.pose_pin_names[index]) {
                contexts.push((context, weight));
            }
            if let Some(base) = output.as_mut() {
                LinearInterpolator {
                    bone_mask: BoneMask::all(),
                }
                .interpolate_pose(
                    base,
                    &pose,
                    weight / (accumulated_weight + weight),
                );
            } else {
                output = Some(pose);
            }
            accumulated_weight += weight;
        }
        let output = output.unwrap_or_default();
        ctx.set_time(output.timestamp);
        ctx.set_data_fwd(self.output_pin.clone(), output);
        let state = ctx.state_mut::<SourceGameplayTagBlendState>()?;
        state.context = blend_animation_contexts(contexts);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        let count = self.child_count();
        if self.pose_pin_names.len() != count || self.blend_time_pin_names.len() != count {
            return Err(GraphError::GlobalInputDataMissing(self.active_key.clone()));
        }
        for pin in &self.pose_pin_names {
            ctx.add_input_data(pin.clone(), DataSpec::Pose);
        }
        for pin in &self.blend_time_pin_names {
            ctx.add_input_data(pin.clone(), DataSpec::F32);
        }
        if !self.active_tag_pin.is_empty() {
            ctx.add_input_data(self.active_tag_pin.clone(), DataSpec::Bool);
        }
        ctx.add_output_data(self.output_pin.clone(), DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Gameplay Tag Blend".to_owned()
    }
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceCurveModifyNode {
    pub source_pose_pin: String,
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
    pub alpha_key: String,
    pub alpha_pin: String,
    pub expose_alpha_pin: bool,
    pub apply_mode: String,
}

impl Default for SourceCurveModifyNode {
    fn default() -> Self {
        Self {
            source_pose_pin: "Source Pose".to_owned(),
            curve_name: default(),
            curve_names: default(),
            value: 0.0,
            pin_default: 0.0,
            pin_defaults: default(),
            value_pin_names: default(),
            value_key: default(),
            value_keys: default(),
            source_value_path: default(),
            source_value_paths: default(),
            alpha: 0.0,
            alpha_key: default(),
            alpha_pin: default(),
            expose_alpha_pin: false,
            apply_mode: default(),
        }
    }
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
            Err(_) if self.alpha_key.is_empty() => Ok(resolved_curve_alpha(None, self.alpha)),
            Err(_) => global_f32_or_root_default_value(&ctx, &self.alpha_key, self.alpha),
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
        let pose = ctx.data_back(self.source_pose_pin.clone())?.into_pose()?;
        let mut context = pose_input_context_state(&ctx, &self.source_pose_pin).unwrap_or_default();
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
            let source = context.curves.get(curve_name).copied().unwrap_or(0.0);
            let value = source_curve_modify_value(source, target, alpha, &self.apply_mode);
            context.curves.insert(curve_name.clone(), value);
        }
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
        *ctx.state_mut_or_else(AnimationContextState::default)? = context;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(self.source_pose_pin.clone(), DataSpec::Pose);
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
    fn source_two_way_reinitializes_players_and_advances_the_activation_frame() {
        use crate::SourceSequencePlayerNode;
        use crate::source_sequence_player::SourceSequenceSynchronization;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip,
            animation_graph::AnimationGraph,
            animation_node::AnimationNode,
            context::{
                deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                system_resources::SystemResources,
            },
            edge_data::DataValue,
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        fn query(In(handle): In<Handle<AnimationGraph>>, resources: SystemResources) -> Vec<f32> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut positions = Vec::new();
            for alpha in [0.0, 0.0, 1.0, 1.0, 0.0] {
                let inputs = HashMap::from([
                    ("alpha".into(), DataValue::F32(alpha)),
                    ("delta".into(), DataValue::F32(0.1)),
                ]);
                let context = GraphContext::new(
                    arena.get_toplevel_id(),
                    graph,
                    &mut arena,
                    &resources,
                    &env,
                    Entity::PLACEHOLDER,
                    &entities,
                    &mut gizmos,
                    &inputs,
                );
                let output = graph
                    .query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.1)), context)
                    .unwrap();
                positions.push(output["Pose"].clone().into_pose().unwrap().timestamp);
            }
            positions
        }
        let mut graphs = Assets::<AnimationGraph>::default();
        let mut clips = Assets::<GraphClip>::default();
        let clip = clips.add(GraphClip {
            duration: 1.0,
            ..default()
        });
        let mut cases = Vec::new();
        for (sync, reset) in [(false, true), (true, true), (true, false)] {
            let mut graph = AnimationGraph::new();
            let blend = AnimationNode::new(
                "blend",
                SourceTwoWayBlendNode {
                    alpha_key: "alpha".into(),
                    expose_alpha_pin: false,
                    reset_child_on_activation: reset,
                    ..default()
                },
            );
            graph.add_node(blend.clone());
            graph.add_output_data("Pose".into(), DataSpec::Pose);
            graph.add_output_data_edge(blend.id, "Pose", "Pose");
            graph.add_output_time();
            graph.add_output_pose_edge(blend.id);
            for (pin, start) in [("A", 0.2), ("B", 0.6)] {
                let player = AnimationNode::new(
                    pin,
                    SourceSequencePlayerNode {
                        clip: clip.clone(),
                        start_position: start,
                        expose_play_rate_pin: false,
                        expose_start_position_pin: false,
                        synchronization: sync.then(|| SourceSequenceSynchronization {
                            key: "group".into(),
                            delta_seconds_key: "delta".into(),
                            ..default()
                        }),
                        ..default()
                    },
                );
                graph.add_node(player.clone());
                graph.add_node_parameter_edge(player.id, "Pose", blend.id, pin);
                if reset {
                    graph.add_node_pose_edge(player.id, blend.id, pin);
                }
            }
            cases.push(graphs.add(graph));
        }
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(clips);
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        for (index, handle) in cases.into_iter().enumerate() {
            let positions = world.run_system_once_with(query, handle).unwrap();
            let last = if index == 2 { 0.5 } else { 0.3 };
            for (actual, expected) in positions.into_iter().zip([0.3, 0.4, 0.7, 0.8, last]) {
                assert!(
                    (actual - expected).abs() < 1e-6,
                    "case {index}: {actual} != {expected}"
                );
            }
        }
    }

    #[test]
    fn source_two_way_alpha_matches_range_and_directional_interpolation() {
        let mut state = SourceBlendState::default();
        assert_eq!(
            source_blend_alpha(
                &mut state,
                0.125,
                0.25,
                &SourceAlphaMapping::default(),
                1.0,
                0.0,
                0.0,
                1.0,
                20.0,
                4.0,
                0.1,
            ),
            0.5
        );
        assert!(
            (source_blend_alpha(
                &mut state,
                0.0,
                0.25,
                &SourceAlphaMapping::default(),
                1.0,
                0.0,
                0.0,
                1.0,
                20.0,
                4.0,
                0.1,
            ) - 0.3)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn source_blend_state_keeps_context_with_the_alpha_history() {
        let mut state = SourceBlendState::default();
        let alpha = source_blend_alpha(
            &mut state,
            0.5,
            0.0,
            &SourceAlphaMapping::default(),
            1.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
        );
        state.context.curves.insert("PoseGrounded".to_owned(), 1.0);
        assert_eq!(alpha, 0.5);
        assert_eq!(state.context.curves.get("PoseGrounded"), Some(&1.0));
    }

    #[test]
    fn source_alpha_scale_bias_applies_on_the_first_legacy_sample() {
        let mut state = SourceBlendState::default();
        assert_eq!(
            source_blend_alpha(
                &mut state,
                0.25,
                0.0,
                &SourceAlphaMapping::default(),
                -1.0,
                1.0,
                0.0,
                1.0,
                0.0,
                0.0,
                0.0,
            ),
            0.75
        );
    }

    #[test]
    fn source_alpha_mapping_matches_unclamped_range_then_final_alpha_clamp() {
        let mapping = SourceAlphaMapping {
            source_semantics: true,
            map_range: true,
            input_min: 0.0,
            input_max: 1.0,
            output_min: 1.0,
            output_max: 0.5,
            ..default()
        };
        let mut state = SourceBlendState::default();
        assert_eq!(
            source_blend_alpha(
                &mut state, 0.0, 0.0, &mapping, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0
            ),
            1.0
        );
        assert_eq!(
            source_blend_alpha(
                &mut state, 1.0, 0.0, &mapping, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0
            ),
            0.5
        );
        assert_eq!(
            source_blend_alpha(
                &mut state, 3.0, 0.0, &mapping, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0
            ),
            0.0
        );
    }

    #[test]
    fn source_alpha_mapping_applies_source_clamp_and_interpolation_before_alpha_bias() {
        let mapping = SourceAlphaMapping {
            source_semantics: true,
            map_range: true,
            input_min: 0.0,
            input_max: 1.0,
            output_min: 0.0,
            output_max: 2.0,
            clamp_result: true,
            clamp_min: 0.25,
            clamp_max: 0.75,
            interpolate_result: true,
            interpolate_speed_increasing: 2.0,
            interpolate_speed_decreasing: 2.0,
            ..default()
        };
        let mut state = SourceBlendState::default();
        assert_eq!(
            source_blend_alpha(
                &mut state, 0.0, 0.0, &mapping, -1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.1
            ),
            0.75
        );
        assert!(
            (source_blend_alpha(
                &mut state, 1.0, 0.0, &mapping, -1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.1
            ) - 0.65)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn source_tag_blend_uses_real_tag_selection() {
        use bevy_gameplay_tag::GameplayTag;

        let tags = vec!["Als.Gait.Sprinting".to_owned()];
        assert_eq!(
            select_gameplay_tag_child_index_for_tag(
                &tags,
                Some(&GameplayTag::new("Als.Gait.Sprinting")),
            ),
            1
        );
        assert_eq!(
            select_gameplay_tag_child_index_for_tag(
                &tags,
                Some(&GameplayTag::new("Als.Gait.Running")),
            ),
            0
        );
    }

    #[test]
    fn source_apply_additive_supports_a_hidden_constant_alpha() {
        let node = SourceApplyAdditiveNode {
            alpha_key: String::new(),
            alpha_pin: "Alpha".to_owned(),
            expose_alpha_pin: false,
            use_constant_alpha: true,
            constant_alpha: 1.0,
            mapping: default(),
            alpha_scale: 1.0,
            alpha_bias: 0.0,
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
    fn source_two_way_blend_can_hide_an_unexposed_alpha_pin() {
        let node = SourceTwoWayBlendNode {
            expose_alpha_pin: false,
            ..default()
        };
        assert!(!node.expose_alpha_pin);
    }

    #[test]
    fn source_two_way_child_reset_matches_ue_relevancy_thresholds() {
        let mut state = SourceTwoWayBlendState::default();
        assert_eq!(
            source_two_way_child_relevancy(&mut state, 0.0),
            (true, false, true, false)
        );
        assert_eq!(
            source_two_way_child_relevancy(&mut state, 1.0),
            (false, true, false, true)
        );
        assert_eq!(
            source_two_way_child_relevancy(&mut state, 0.5),
            (true, true, true, false)
        );
        assert_eq!(
            source_two_way_child_relevancy(&mut state, 0.5),
            (true, true, false, false)
        );
    }

    #[test]
    fn source_mesh_space_additive_preserves_source_pose_pin_names() {
        let node = SourceMeshSpaceAdditiveNode::default();
        assert_eq!(node.alpha_pin, "Alpha");
        assert!(node.expose_alpha_pin);
    }

    #[test]
    fn source_make_dynamic_additive_defaults_to_local_space() {
        assert!(!SourceMakeDynamicAdditiveNode::default().mesh_space_additive);
        let mut registry = bevy::reflect::TypeRegistry::default();
        registry.register::<SourceMakeDynamicAdditiveNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<SourceMakeDynamicAdditiveNode>())
                .unwrap()
                .data::<ReflectNodeLike>()
                .is_some()
        );
    }

    #[test]
    fn source_make_dynamic_additive_selects_mesh_space_difference() {
        use bevy_animation_graph::core::{
            animation_clip::EntityPath,
            pose::{BonePose, Pose},
            skeleton::Skeleton,
        };

        let root_path = EntityPath::default().child("root");
        let child_path = root_path.child("child");
        let root = root_path.id();
        let child = child_path.id();
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root);
        skeleton.add_bone(root_path, Transform::IDENTITY, Transform::IDENTITY);
        skeleton.add_bone(child_path, Transform::IDENTITY, Transform::IDENTITY);
        let mut base = Pose::default();
        base.add_bone(
            BonePose {
                rotation: Some(Quat::from_rotation_y(0.7)),
                ..default()
            },
            root,
        );
        base.add_bone(
            BonePose {
                rotation: Some(Quat::from_rotation_x(-0.4)),
                ..default()
            },
            child,
        );
        let mut additive = Pose::default();
        additive.add_bone(
            BonePose {
                rotation: Some(Quat::from_rotation_z(0.45)),
                ..default()
            },
            root,
        );
        additive.add_bone(
            BonePose {
                rotation: Some(Quat::from_rotation_y(0.9)),
                ..default()
            },
            child,
        );
        let local = make_dynamic_additive_difference(&base, &additive, &skeleton, false);
        let mesh = make_dynamic_additive_difference(&base, &additive, &skeleton, true);
        assert!(
            local
                .get_bone(child)
                .unwrap()
                .rotation
                .unwrap()
                .dot(mesh.get_bone(child).unwrap().rotation.unwrap())
                .abs()
                < 0.999
        );
    }

    #[test]
    fn source_curves_blend_preserves_source_modes() {
        let mut base = AnimationContextState::default();
        base.curves.insert("BaseOnly".to_owned(), 0.25);
        base.curves.insert("Shared".to_owned(), 0.5);
        let mut curve = AnimationContextState::default();
        curve.curves.insert("CurveOnly".to_owned(), 0.4);
        curve.curves.insert("Shared".to_owned(), 0.6);
        let accumulated = blend_source_curve_context(
            base.clone(),
            &curve,
            0.5,
            SourceCurvesBlendMode::Accumulate,
        );
        assert!((accumulated.curves["BaseOnly"] - 0.25).abs() < 1e-6);
        assert!((accumulated.curves["CurveOnly"] - 0.2).abs() < 1e-6);
        assert!((accumulated.curves["Shared"] - 0.8).abs() < 1e-6);
        let overridden =
            blend_source_curve_context(base, &curve, 0.5, SourceCurvesBlendMode::Override);
        assert_eq!(overridden.curves.get("BaseOnly"), None);
        assert!((overridden.curves["CurveOnly"] - 0.2).abs() < 1e-6);
        assert!((overridden.curves["Shared"] - 0.3).abs() < 1e-6);
        let mut registry = bevy::reflect::TypeRegistry::default();
        registry.register::<SourceCurvesBlendMode>();
        registry.register::<SourceCurvesBlendNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<SourceCurvesBlendNode>())
                .unwrap()
                .data::<ReflectNodeLike>()
                .is_some()
        );
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
    fn source_gameplay_tag_blend_can_preserve_source_pin_names() {
        let node = SourceGameplayTagBlendNode {
            active_tag_pin: "ActiveTag".to_owned(),
            pose_pin_names: vec!["BlendPose_0".to_owned(), "BlendPose_1".to_owned()],
            blend_time_pin_names: vec!["BlendTime_0".to_owned(), "BlendTime_1".to_owned()],
            ..default()
        };
        assert_eq!(node.active_tag_pin, "ActiveTag");
        assert_eq!(node.pose_pin_names[1], "BlendPose_1");
    }

    #[test]
    fn source_gameplay_tag_blend_uses_the_bound_tag_without_a_bool_pin() {
        use crate::source_gameplay_tag_input::{clear_gameplay_tag_inputs, set_gameplay_tag_input};

        let entity = Entity::from_raw_u32(992).unwrap();
        set_gameplay_tag_input(
            entity,
            "locomotion_action",
            bevy_gameplay_tag::GameplayTag::new("Als.LocomotionAction.Ragdolling"),
        );
        let node = SourceGameplayTagBlendNode {
            active_key: "locomotion_action".to_owned(),
            active_tag_pin: "ActiveTag".to_owned(),
            tags: vec!["Als.LocomotionAction.Ragdolling".to_owned()],
            ..default()
        };
        assert_eq!(node.active_index(entity), Some(1));
        clear_gameplay_tag_inputs(entity);
    }

    #[test]
    fn source_gameplay_tag_blend_weights_missing_child_curves_as_zero() {
        let input = AnimationContextState {
            curves: [("FootLeftIk".to_owned(), 1.0)].into_iter().collect(),
            curves_are_authoritative: true,
            ..default()
        };
        let empty = AnimationContextState {
            curves_are_authoritative: true,
            ..default()
        };
        let output = blend_animation_contexts([(input, 0.25), (empty.clone(), 0.75)]);
        assert!((output.curves["FootLeftIk"] - 0.25).abs() <= 1e-6);
        assert!(output.curves_are_authoritative);
        let output = blend_animation_contexts([(empty, 1.0)]);
        assert!(output.curves_are_authoritative);
        assert!(output.curves.is_empty());
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
    fn source_curve_modify_can_preserve_source_pose_pin_name() {
        let node = SourceCurveModifyNode {
            source_pose_pin: "SourcePose".to_owned(),
            ..default()
        };
        assert_eq!(node.source_pose_pin, "SourcePose");
    }

    #[test]
    fn source_curve_modify_retains_alpha_global_fallback_key() {
        let node = SourceCurveModifyNode {
            alpha_key: "als_locomotion_ground_prediction".to_owned(),
            ..default()
        };
        assert_eq!(node.alpha_key, "als_locomotion_ground_prediction");
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
