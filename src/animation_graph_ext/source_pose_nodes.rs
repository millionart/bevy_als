use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext,
        graph_context::{GraphState, QueryOutputTime}, graph_context_arena::GraphContextId},
    edge_data::DataSpec,
    errors::GraphError,
    pose::Pose,
};
use serde::{Deserialize, Serialize};

use crate::extensions::animation_context::{
    AnimationContextState, merge_asset_samples, merge_inertialization_request, merge_sync_players, pose_input_context_state, select_relevant_asset,
};
use crate::extensions::curve_bundle::blend_curve_value;
use crate::extensions::global_f32_input::{global_f32_or_root_default, global_f32_or_root_default_value};
use crate::extensions::mesh_space_additive::{apply_local_space_additive, apply_mesh_space_additive, materialize_missing_reference_bones};
use crate::extensions::mesh_space_difference::make_mesh_space_difference;
use crate::extensions::reference_local_difference::make_local_difference;
use crate::extensions::{
    context_time_update, f_interp_to,
    gameplay_tag_blend::{
        GameplayTagBlendMode, GameplayTagBlendState, advance_gameplay_tag_weights,
        select_gameplay_tag_child_index_for_tag,
    },
    source_gameplay_tag_input::active_gameplay_tag_input,
};

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct SourceBlendState {
    initialized: bool,
    value: f32,
    pub(crate) context: AnimationContextState,
}

#[derive(Reflect, Clone, Debug, Default)]
pub struct SourceTwoWayBlendState {
    alpha: SourceBlendState,
    pub input_alpha: f32,
    pub blend_alpha: f32,
    a_is_relevant: bool,
    b_is_relevant: bool,
    child_contexts: Vec<Vec<GraphContextId>>,
    pub(crate) context: AnimationContextState,
}

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct SourceGameplayTagBlendState {
    pub(crate) blend: GameplayTagBlendState,
    pub(crate) child_contexts: Vec<Vec<GraphContextId>>,
    pub(crate) context: AnimationContextState,
}

pub(crate) fn read_blend_list_child(
    ctx: &NodeContext,
    pin: &str,
    index: usize,
    scoped: bool,
    reset: bool,
    children: &mut Vec<Vec<GraphContextId>>,
) -> Result<(Pose, Option<AnimationContextState>), GraphError> {
    if !scoped {
        let pose = ctx.data_back(pin)?.into_pose()?;
        return Ok((pose, pose_input_context_state(ctx, pin)));
    }
    let source = crate::extensions::animation_context::source_node_for_pose_input(ctx, pin)?;
    let graph_id = ctx.graph_context.context().get_graph_id();
    let arena = &ctx.graph_context.context_arena;
    let previous_count = arena.get_ref().iter_context_ids().count();
    let mut branch = ctx.graph_context.create_node_context(source)
        .create_child_context(graph_id, ctx.graph_context.graph, None);
    children.resize_with(children.len().max(index + 1), Vec::new);
    let contexts = &mut children[index];
    if !contexts.contains(&branch.context_id) {
        contexts.push(branch.context_id);
    }
    if reset {
        for id in contexts.iter().copied() {
            let state = arena.get_ref().get_context(id).unwrap();
            let graph = ctx.graph_context.resources.animation_graph_assets.get(state.get_graph_id())
                .ok_or_else(|| GraphError::UnknownErrorNode { graph: ctx.graph_context.context_id,
                    node: ctx.node_id, error: "Source blend child graph asset is missing".into() })?;
            let bookkeeping = graph.nodes.keys().filter_map(|node| {
                state.node_states.get::<SourceGameplayTagBlendState>(*node, ctx.graph_context.state_key).ok()
                    .filter(|state| !state.child_contexts.is_empty())
                    .map(|state| (*node, state.child_contexts.clone()))
            }).collect::<Vec<_>>();
            let state = arena.get_mut().get_context_mut(id).unwrap();
            *state = GraphState::new(state.get_graph_id());
            for (node, children) in bookkeeping {
                state.node_states.get_mut_or_insert_with(node, ctx.graph_context.state_key, SourceGameplayTagBlendState::default)?
                    .child_contexts = children;
            }
        }
    }
    branch.context_mut().query_output_time = QueryOutputTime::from_key(ctx.graph_context.state_key, context_time_update(ctx)?);
    let child = branch.create_node_context(ctx.node_id);
    let result = child.data_back(pin);
    for id in arena.get_ref().iter_context_ids().skip(previous_count) {
        if !contexts.contains(&id) {
            contexts.push(id);
        }
    }
    Ok((result?.into_pose()?, pose_input_context_state(&child, pin)))
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
                    crate::extensions::global_f32_input::global_f32_or_root_default_value(
                        &ctx,
                        &self.alpha_key,
                        self.pin_default,
                    )
                })?
        } else {
            crate::extensions::global_f32_input::global_f32_or_root_default_value(
                &ctx,
                &self.alpha_key,
                self.pin_default,
            )?
        };
        let delta = delta_seconds(&ctx)?;
        let (alpha, a_is_relevant, b_is_relevant, reset_a, reset_b) = {
            let state = ctx.state_mut::<SourceTwoWayBlendState>()?;
            state.input_alpha = raw;
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
            state.blend_alpha = alpha;
            (alpha, a_is_relevant, b_is_relevant, reset_a, reset_b)
        };
        let mut child_contexts = std::mem::take(&mut ctx.state_mut::<SourceTwoWayBlendState>()?.child_contexts);
        let (mut base, context_a) = if a_is_relevant {
            read_blend_list_child(&ctx, "A", 0, self.reset_child_on_activation, reset_a, &mut child_contexts)?
        } else {
            (Pose::default(), None)
        };
        let (overlay, context_b) = if b_is_relevant {
            read_blend_list_child(&ctx, "B", 1, self.reset_child_on_activation, reset_b, &mut child_contexts)?
        } else {
            (Pose::default(), None)
        };
        if !a_is_relevant {
            base = overlay;
        } else if b_is_relevant {
            base = blend_source_poses(&[(base, 1.0 - alpha), (overlay, alpha)]);
        }
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd("Pose", base);
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
        let state = ctx.state_mut::<SourceTwoWayBlendState>()?;
        state.context = context;
        state.child_contexts = child_contexts;
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
        let mut base = ctx.data_back("Base")?.into_pose()?;
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
        let additive_is_relevant = alpha > 1e-5;
        if additive_is_relevant {
            let additive = ctx.data_back("Additive")?.into_pose()?;
            let skeleton = ctx.graph_context.resources.skeleton_assets.get(&base.skeleton)
                .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
            apply_local_space_additive(&mut base, &additive, alpha, skeleton);
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
        let mut base = ctx.data_back("Base")?.into_pose()?;
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
        if let Some(skeleton) = ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&base.skeleton)
        {
            materialize_missing_reference_bones(&mut base, skeleton);
        }
        let additive_is_relevant = alpha > 1e-5;
        if additive_is_relevant {
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
        let additive_context = additive_is_relevant
            .then(|| pose_input_context_state(&ctx, "Additive")).flatten();
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
        let mut difference =
            make_dynamic_additive_difference(&base, &additive, skeleton, self.mesh_space_additive);
        difference.timestamp = additive.timestamp;
        ctx.set_time(difference.timestamp);
        ctx.set_data_fwd("Pose", difference);
        let base_context = pose_input_context_state(&ctx, "Base").unwrap_or_default();
        let mut context = pose_input_context_state(&ctx, "Additive").unwrap_or_default();
        merge_sync_players(&mut context.sync_players, &base_context.sync_players, 1.0);
        merge_asset_samples(&mut context.asset_samples, &base_context.asset_samples, 1.0);
        merge_inertialization_request(&mut context.inertialization_request, base_context.inertialization_request.as_ref());
        context.relevant_asset = select_relevant_asset([(base_context.relevant_asset, 1.0), (context.relevant_asset, 1.0)]);
        context.curves_are_authoritative |= base_context.curves_are_authoritative;
        for (key, value) in base_context.curves {
            *context.curves.entry(key).or_default() -= value;
        }
        *ctx.state_mut_or_else(AnimationContextState::default)? = context;
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
    merge_asset_samples(&mut base.asset_samples, &curve.asset_samples, blend_amount);
    if blend_amount > 1e-5 {
        merge_inertialization_request(&mut base.inertialization_request, curve.inertialization_request.as_ref());
    }
    let relevant_asset = select_relevant_asset([
        (base.relevant_asset.clone(), 1.0),
        (curve.relevant_asset.clone(), blend_amount),
    ]);
    match blend_mode {
        SourceCurvesBlendMode::Accumulate => {
            for (name, value) in &curve.curves {
                *base.curves.entry(name.clone()).or_default() += value * blend_amount;
            }
            base.curves_are_authoritative |= curve.curves_are_authoritative;
            base.relevant_asset = relevant_asset;
            base
        }
        SourceCurvesBlendMode::Override => AnimationContextState {
            curves: curve
                .curves
                .iter()
                .map(|(name, value)| (name.clone(), value * blend_amount))
                .collect(),
            relevant_asset,
            curves_are_authoritative: curve.curves_are_authoritative,
            sync_players: base.sync_players,
            asset_samples: base.asset_samples,
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
        let context = if self.blend_amount <= 1e-5 {
            base_context
        } else {
            crate::extensions::interruptible_fsm::with_machine_weight(self.blend_amount, || ctx.data_back("CurvePose"))?.into_pose()?;
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
    #[reflect(default)]
    pub reset_child_on_activation: bool,
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
            reset_child_on_activation: false,
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
        active_gameplay_tag_input(root_entity, &self.active_key)
            .map(|active| select_gameplay_tag_child_index_for_tag(&self.tags, Some(&active)))
    }
}

pub fn blend_source_poses(poses: &[(Pose, f32)]) -> Pose {
    let mut output = poses.iter()
        .map(|(pose, weight)| pose.scalar_mult(*weight))
        .reduce(|accumulator, pose| accumulator.linear_add(&pose))
        .unwrap_or_default().normalize_quat();
    output.root_motion = crate::extensions::normalized_blendspace::blend_normalized_root_motion(
        &poses.iter().map(|(pose, weight)| (pose.root_motion.clone(), *weight)).collect::<Vec<_>>(),
    );
    output
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
        merge_asset_samples(&mut output.asset_samples, &context.asset_samples, weight);
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
        let reset = self.reset_child_on_activation && ctx.state_mut::<SourceGameplayTagBlendState>()?
            .blend.should_reset_child(self.child_count(), active_index);
        let mut child_contexts = std::mem::take(&mut ctx.state_mut::<SourceGameplayTagBlendState>()?.child_contexts);
        let previous = ctx.state_mut::<SourceGameplayTagBlendState>()?.blend
            .instant_transition_source(active_index, blend_time);
        if let Some(previous) = previous {
            read_blend_list_child(&ctx, &self.pose_pin_names[previous], previous, self.reset_child_on_activation, false, &mut child_contexts)?;
        }
        let weights = advance_gameplay_tag_weights(
            &mut ctx.state_mut::<SourceGameplayTagBlendState>()?.blend,
            self.child_count(),
            active_index,
            blend_time,
            self.blend_mode,
            delta,
        );
        let mut poses = Vec::new();
        let mut contexts = Vec::new();
        for (index, weight) in weights.into_iter().enumerate() {
            if weight <= crate::extensions::gameplay_tag_blend::WEIGHT_THRESHOLD {
                continue;
            }
            let (pose, context) = read_blend_list_child(&ctx, &self.pose_pin_names[index], index,
                self.reset_child_on_activation, reset && index == active_index, &mut child_contexts)?;
            if let Some(context) = context {
                contexts.push((context, weight));
            }
            poses.push((pose, weight));
        }
        let output = blend_source_poses(&poses);
        ctx.set_time(output.timestamp);
        ctx.set_data_fwd(self.output_pin.clone(), output);
        let state = ctx.state_mut::<SourceGameplayTagBlendState>()?;
        state.child_contexts = child_contexts;
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
pub struct SourceBoolBlendNode {
    #[reflect(default)]
    pub reset_child_on_activation: bool,
    pub active_key: String,
    pub active_value: bool,
    pub blend_times: Vec<f32>,
    pub source_blend_times: Vec<f32>,
    pub blend_mode: crate::extensions::profile_blend::AlphaBlendMode,
    pub custom_curve: Vec<crate::extensions::RichCurveKey>,
}

impl Default for SourceBoolBlendNode {
    fn default() -> Self {
        Self {
            reset_child_on_activation: false,
            active_key: default(),
            active_value: false,
            blend_times: vec![0.1; 2],
            source_blend_times: vec![0.1; 2],
            blend_mode: default(),
            custom_curve: default(),
        }
    }
}

impl NodeLike for SourceBoolBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if self.blend_times.len() != 2 || self.source_blend_times.len() != 2 {
            return Err(GraphError::UnknownErrorNode {
                graph: ctx.graph_context.context_id,
                node: ctx.node_id,
                error: "bool blend configuration is invalid".into(),
            });
        }
        let active = if let Ok(value) = ctx.data_back("bActiveValue") {
            value.as_bool()?
        } else if self.active_key.is_empty() {
            self.active_value
        } else {
            crate::extensions::source_instance::source_input_data(&ctx, &self.active_key)
                .ok_or_else(|| GraphError::GlobalInputDataMissing(self.active_key.clone()))?
                .as_bool()?
        };
        let index = usize::from(!active);
        let blend_time = ctx
            .data_back(format!("BlendTime_{index}"))
            .ok()
            .and_then(|value| value.as_f32().ok())
            .unwrap_or(self.blend_times[index])
            .max(0.0);
        let previous = ctx
            .state_mut::<SourceGameplayTagBlendState>()?
            .blend
            .instant_transition_source(index, blend_time);
        let reset = self.reset_child_on_activation && ctx.state_mut::<SourceGameplayTagBlendState>()?
            .blend.should_reset_child(2, index);
        let mut child_contexts = std::mem::take(&mut ctx.state_mut::<SourceGameplayTagBlendState>()?.child_contexts);
        if let Some(previous) = previous {
            read_blend_list_child(&ctx, &format!("BlendPose_{previous}"), previous, self.reset_child_on_activation, false, &mut child_contexts)?;
        }
        let delta = delta_seconds(&ctx)?;
        let weights = crate::extensions::gameplay_tag_blend::advance_blend_list_weights(
            &mut ctx.state_mut::<SourceGameplayTagBlendState>()?.blend,
            2,
            index,
            blend_time,
            delta,
            |alpha| {
                crate::extensions::profile_blend::transition_alpha(alpha, self.blend_mode, &self.custom_curve)
            },
        );
        let mut poses = Vec::new();
        let mut contexts = Vec::new();
        for (child_index, weight) in weights.into_iter().enumerate() {
            if weight <= crate::extensions::gameplay_tag_blend::WEIGHT_THRESHOLD {
                continue;
            }
            let pin = format!("BlendPose_{child_index}");
            let (pose, context) = read_blend_list_child(&ctx, &pin, child_index, self.reset_child_on_activation,
                reset && child_index == index, &mut child_contexts)?;
            poses.push((pose, weight));
            if let Some(context) = context {
                contexts.push((context, weight));
            }
        }
        let pose = blend_source_poses(&poses);
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
        ctx.state_mut::<SourceGameplayTagBlendState>()?.context =
            blend_animation_contexts(contexts);
        ctx.state_mut::<SourceGameplayTagBlendState>()?.child_contexts = child_contexts;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        for index in 0..2 {
            ctx.add_input_data(format!("BlendPose_{index}"), DataSpec::Pose)
                .add_input_data(format!("BlendTime_{index}"), DataSpec::F32);
        }
        ctx.add_input_data("bActiveValue", DataSpec::Bool)
            .add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Bool Blend".into()
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
            let curve_name = crate::extensions::SourceCurveName::from(curve_name.as_str());
            let source = context.curves.get(&curve_name).copied().unwrap_or(0.0);
            let value = source_curve_modify_value(source, target, alpha, &self.apply_mode);
            context.curves.insert(curve_name, value);
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
    fn source_blend_list_resets_inactive_nested_children_and_preserves_active_blends() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::AnimationGraph, animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                system_resources::SystemResources}, edge_data::DataValue,
            skeleton::Skeleton, state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        fn query(In((handle, switching, reentry)): In<(Handle<AnimationGraph>, bool, bool)>, resources: SystemResources) -> Vec<f32> {
            let mut source_inputs = crate::extensions::SourceAnimationInputs::default();
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut positions = Vec::new();
            let sequence = if switching { vec![true, false, true] } else if reentry { vec![true, true, false, false, true, true, true] } else { vec![true, true, false, false, true] };
            for (frame, active) in sequence.into_iter().enumerate() {
                source_inputs.set_gameplay_tag_input("source_child_reset_fixture", bevy_gameplay_tag::GameplayTag::new(if active { "" } else { "Fixture.Alternate" }));
                source_inputs.set_gameplay_tag_input("source_child_reset_inner", bevy_gameplay_tag::GameplayTag::new(if !reentry || frame != 5 { "" } else { "Fixture.Alternate" }));
                let inputs = HashMap::from([("source_child_reset_fixture".into(), DataValue::Bool(active)),
                    ("source_child_reset_enum".into(), DataValue::F32(if active { 0.0 } else { 1.0 })),
                    ("source_child_reset_inner".into(), DataValue::Bool(!reentry || frame != 5)),
                    ("source_child_reset_inner_enum".into(), DataValue::F32(if !reentry || frame != 5 { 0.0 } else { 1.0 }))]);
                let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources,
                    &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                let output = source_inputs.evaluate(context.root_entity, || graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.1)), context)).unwrap();
                positions.push(output["Pose"].clone().into_pose().unwrap().timestamp);
            }
            positions
        }
        let mut graphs = Assets::<AnimationGraph>::default();
        let mut clips = Assets::<GraphClip>::default();
        let clip = clips.add(GraphClip { duration: 10.0, ..default() });
        let mut leaf = AnimationGraph::new();
        let player = AnimationNode::new("leaf", crate::extensions::SourceSequencePlayerNode {
            clip: clip.clone(), expose_play_rate_pin: false, expose_start_position_pin: false, ..default()
        });
        leaf.add_node(player.clone());
        leaf.add_output_data("Pose".into(), DataSpec::Pose);
        leaf.add_output_data_edge(player.id, "Pose", "Pose");
        let leaf = graphs.add(leaf);
        let mut cases = Vec::new();
        for kind in 0..3 {
        let mut nested = AnimationGraph::new();
        let inner = match kind {
            0 => AnimationNode::new("inner", SourceBoolBlendNode {
            active_key: "source_child_reset_inner".into(), reset_child_on_activation: true, blend_times: vec![0.0; 2], ..default()
            }),
            1 => AnimationNode::new("inner", SourceGameplayTagBlendNode {
                active_key: "source_child_reset_inner".into(), tags: vec!["Fixture.Alternate".into()], reset_child_on_activation: true,
                blend_times: vec![0.0; 2], source_blend_times: vec![0.0; 2], pose_pin_names: vec!["BlendPose_0".into(), "BlendPose_1".into()],
                blend_time_pin_names: vec!["BlendTime_0".into(), "BlendTime_1".into()], ..default()
            }),
            _ => AnimationNode::new("inner", crate::extensions::SourceEnumPoseBlendNode {
                active_key: "source_child_reset_inner_enum".into(), selected_enum_value: 1.0, reset_child_on_activation: true,
                default_pose_pin: "BlendPose_0".into(), selected_pose_pin: "BlendPose_1".into(), ..default()
            }),
        };
        nested.add_node(inner.clone());
        nested.add_output_data("Pose".into(), DataSpec::Pose);
        nested.add_output_data_edge(inner.id, "Pose", "Pose");
        for index in 0..2 {
            let player = AnimationNode::new(format!("player_{index}"), crate::extensions::LayeredGraphNode {
                graph: leaf.clone(), internalize_time: true, capture_context: true, ..default()
            });
            nested.add_node(player.clone());
            nested.add_node_parameter_edge(player.id, "Pose", inner.id, format!("BlendPose_{index}"));
        }
        let nested = graphs.add(nested);
            for (reset, blend_time, reentry) in [(false, 0.0, false), (true, 0.0, false), (true, 0.5, false), (true, 0.0, true)] {
                let mut graph = AnimationGraph::new();
                let blend = match kind {
                    0 => AnimationNode::new("outer", SourceBoolBlendNode { active_key: "source_child_reset_fixture".into(), reset_child_on_activation: reset, blend_times: vec![blend_time; 2], ..default() }),
                    1 => AnimationNode::new("outer", SourceGameplayTagBlendNode { active_key: "source_child_reset_fixture".into(), tags: vec!["Fixture.Alternate".into()], reset_child_on_activation: reset,
                        blend_times: vec![blend_time; 2], source_blend_times: vec![blend_time; 2],
                        pose_pin_names: vec!["BlendPose_0".into(), "BlendPose_1".into()], blend_time_pin_names: vec!["BlendTime_0".into(), "BlendTime_1".into()], ..default() }),
                    _ => AnimationNode::new("outer", crate::extensions::SourceEnumPoseBlendNode { active_key: "source_child_reset_enum".into(), selected_enum_value: 1.0, reset_child_on_activation: reset,
                        default_blend_time: blend_time, selected_blend_time: blend_time, default_pose_pin: "BlendPose_0".into(), selected_pose_pin: "BlendPose_1".into(), ..default() }),
                };
                graph.add_node(blend.clone());
                let layer = AnimationNode::new("nested", crate::extensions::LayeredGraphNode { graph: nested.clone(), internalize_time: true, capture_context: true, ..default() });
                graph.add_node(layer.clone());
                graph.add_node_parameter_edge(layer.id, "Pose", blend.id, "BlendPose_0");
                let player = AnimationNode::new("other", crate::extensions::SourceSequencePlayerNode { clip: clip.clone(), expose_play_rate_pin: false, expose_start_position_pin: false, ..default() });
                graph.add_node(player.clone());
                graph.add_node_parameter_edge(player.id, "Pose", blend.id, "BlendPose_1");
                graph.add_output_data("Pose".into(), DataSpec::Pose);
                graph.add_output_data_edge(blend.id, "Pose", "Pose");
                cases.push((graphs.add(graph), kind, reset, blend_time > 0.0, reentry));
            }
        }
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(clips);
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        for (handle, kind, reset, switching, reentry) in cases {
            let positions = world.run_system_cached_with(query, (handle, switching, reentry)).unwrap();
            let expected = if switching { 0.3 } else if reset { 0.1 } else { 0.4 };
            assert!((positions.last().unwrap() - expected).abs() < 1e-6, "kind={kind}, reset={reset}, switching={switching}: {positions:?}");
        }
    }

    #[test]
    fn source_additive_nodes_match_curve_arithmetic_and_base_update_order() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::AnimationGraph, animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext, node_states::StateKey,
                system_resources::SystemResources}, edge_data::DataValue, pose::BonePose,
            skeleton::Skeleton, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        fn query(In((handle, skeleton)): In<(Handle<AnimationGraph>, Handle<Skeleton>)>, resources: SystemResources) -> (Pose, AnimationContextState, bool) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut inputs = HashMap::from([("alpha".into(), DataValue::F32(0.13))]);
            for (name, scale, timestamp) in [("Base", 2.0, 1.0), ("Additive", 3.0, 7.0)] {
                let mut pose = Pose { skeleton: skeleton.clone(), timestamp, ..default() };
                pose.add_bone(BonePose::from_transform(Transform::from_scale(Vec3::splat(scale))), default());
                inputs.insert(name.into(), DataValue::Pose(pose));
            }
            let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena,
                &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
            let pose = graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.02)), context.clone())
                .unwrap().remove("Pose").unwrap().into_pose().unwrap();
            let curves = crate::extensions::animation_context::graph_output_context_state(graph, &context, "Pose").unwrap_or_default();
            let additive = graph.nodes.values().find(|node| node.name == "Additive").unwrap();
            let updated = arena.get_toplevel().node_caches.is_updated(additive.id, StateKey::Default);
            (pose, curves, updated)
        }
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let skeleton = world.resource_mut::<Assets<Skeleton>>().add(Skeleton::default());
        let mut failures = Vec::new();
        for kind in ["apply_local", "apply_mesh", "difference_local", "difference_mesh", "curves_accumulate", "curves_override"] {
            let dynamic = kind.starts_with("difference");
            let mesh = kind.ends_with("mesh");
            let curves_only = kind.starts_with("curves");
            for alpha in [-0.25, 0.0, 0.000005, 0.00001, 0.00002, 0.25, 0.75, 1.0] {
                let mut graph = AnimationGraph::new();
                let node = if curves_only { AnimationNode::new("operator", SourceCurvesBlendNode { blend_amount: alpha, blend_mode: if kind == "curves_override" { SourceCurvesBlendMode::Override } else { SourceCurvesBlendMode::Accumulate } }) }
                    else if dynamic { AnimationNode::new("operator", SourceMakeDynamicAdditiveNode { mesh_space_additive: mesh }) }
                    else if mesh { AnimationNode::new("operator", SourceMeshSpaceAdditiveNode { alpha_key: "alpha".into(), expose_alpha_pin: false, mapping: SourceAlphaMapping { source_semantics: true, ..default() }, ..default() }) }
                    else { AnimationNode::new("operator", SourceApplyAdditiveNode { alpha_key: "alpha".into(), expose_alpha_pin: false, mapping: SourceAlphaMapping { source_semantics: true, ..default() }, ..default() }) };
                graph.add_node(node.clone());
                graph.add_output_data("Pose".into(), DataSpec::Pose);
                graph.add_output_data_edge(node.id, "Pose", "Pose");
                for (name, curves, values) in [("Base", ["Shared", "BaseOnly"], [0.2, 0.5]), ("Additive", ["Shared", "AdditiveOnly"], [0.8, 0.4])] {
                    let input = AnimationNode::new(name, crate::extensions::GlobalPoseInputNode { key: name.into(), output: "Pose".into() });
                    let curve = AnimationNode::new("curves", SourceCurveModifyNode { curve_names: curves.map(String::from).to_vec(), pin_defaults: values.to_vec(), source_pose_pin: "Source".into(), alpha: 1.0, ..default() });
                    graph.add_node(input.clone());
                    graph.add_node(curve.clone());
                    graph.add_node_parameter_edge(input.id, "Pose", curve.id, "Source");
                    let source = if name == "Base" {
                        let writer = AnimationNode::new("base_update", crate::extensions::source_instance::SourcePropertyWriteNode {
                            assignments: vec![crate::extensions::source_instance::SourcePropertyAssignment { property: "alpha".into(), value: DataValue::F32(alpha) }], ..default()
                        });
                        graph.add_node(writer.clone());
                        graph.add_node_parameter_edge(curve.id, "Pose", writer.id, "Source");
                        writer.id
                    } else { curve.id };
                    let pin = if curves_only { if name == "Base" { "BasePose" } else { "CurvePose" } } else { name };
                    graph.add_node_parameter_edge(source, "Pose", node.id, pin);
                }
                let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
                let (pose, context, updated) = world.run_system_cached_with(query, (handle, skeleton.clone())).unwrap();
                let effective = if alpha > 1e-5 { alpha } else { 0.0 };
                let expected = if dynamic { [0.6, -0.5, 0.4] }
                    else if kind == "curves_override" && effective > 0.0 { [0.8 * effective, 0.0, 0.4 * effective] }
                    else { [0.2 + 0.8 * effective, 0.5, 0.4 * effective] };
                let curve_match = ["Shared", "BaseOnly", "AdditiveOnly"].into_iter().zip(expected)
                    .all(|(key, value)| (context.curves.get(&crate::extensions::SourceCurveName::from(key)).copied().unwrap_or_default() - value).abs() < 1e-6);
                let expected_scale = if dynamic { 0.5 } else if curves_only { 2.0 } else { 2.0 * (1.0 + 3.0 * effective) };
                let scale_match = pose.get_bone(default()).and_then(|bone| bone.scale).is_some_and(|scale| scale.abs_diff_eq(Vec3::splat(expected_scale), 1e-6));
                if !curve_match || !scale_match || updated != (dynamic || effective > 0.0) || pose.timestamp != if dynamic { 7.0 } else { 1.0 } {
                    failures.push(format!("{kind} alpha={alpha}: curves={:?}, scale={:?}, updated={updated}, time={}", context.curves, pose.get_bone(default()).and_then(|bone| bone.scale), pose.timestamp));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn source_pose_blends_accumulate_weighted_quaternions_before_normalizing() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::AnimationGraph,
            animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                system_resources::SystemResources},
            edge_data::DataValue, pose::{BonePose, Pose, RootMotionDelta}, skeleton::Skeleton,
            state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        fn query(In((handle, mut inputs)): In<(Handle<AnimationGraph>, HashMap<String, DataValue>)>, resources: SystemResources) -> Pose {
            let mut source_inputs = crate::extensions::SourceAnimationInputs::default();
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            if graph.nodes.values().any(|node| node.name == "tag_blend") {
                source_inputs.set_gameplay_tag_input("source_pose_test_blend", bevy_gameplay_tag::GameplayTag::new("Default"));
                let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena,
                    &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                source_inputs.evaluate(context.root_entity, || graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.0)), context)).unwrap();
                source_inputs.set_gameplay_tag_input("source_pose_test_blend", bevy_gameplay_tag::GameplayTag::new("Second"));
            }
            if graph.nodes.values().any(|node| node.try_inner_downcast_ref::<SourceBoolBlendNode>().is_some()) {
                inputs.insert("source_pose_test_bool".into(), true.into());
                let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena,
                    &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                source_inputs.evaluate(context.root_entity, || graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.0)), context)).unwrap();
                inputs.insert("source_pose_test_bool".into(), false.into());
            }
            let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena,
                &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
            let mut output = source_inputs.evaluate(context.root_entity, || graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.02)), context)).unwrap();
            if let Some(node) = graph.nodes.values().find(|node| node.try_inner_downcast_ref::<SourceTwoWayBlendNode>().is_some()) {
                use bevy_animation_graph::core::{animation_graph::NodeId, context::node_states::StateKey};
                let state = arena.get_toplevel().node_states.get::<SourceTwoWayBlendState>(node.id, StateKey::Default).unwrap();
                assert_eq!(state.input_alpha, inputs["alpha"].as_f32().unwrap());
                let properties = arena.get_toplevel().node_states.get::<crate::extensions::source_instance::SourceInstanceProperties>(NodeId::default(), StateKey::Default).unwrap();
                assert_eq!(properties.values["alpha"].as_f32().unwrap(), 0.9);
            }
            if let Some(node) = graph.nodes.values().find(|node| node.try_inner_downcast_ref::<SourceBoolBlendNode>().is_some()) {
                use bevy_animation_graph::core::context::node_states::StateKey;
                let state = arena.get_toplevel().node_states.get::<SourceGameplayTagBlendState>(node.id, StateKey::Default).unwrap();
                assert!((state.context.curves[&crate::extensions::SourceCurveName::from("BoolCurve")] - inputs["expected_curve"].as_f32().unwrap()).abs() < 1e-5);
                if node.try_inner_downcast_ref::<SourceBoolBlendNode>().unwrap().blend_mode == crate::extensions::profile_blend::AlphaBlendMode::Custom {
                    inputs.insert("source_pose_test_bool".into(), true.into());
                    let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena,
                        &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                    let interrupted = source_inputs.evaluate(context.root_entity, || graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.015)), context)).unwrap();
                    let state = arena.get_toplevel().node_states.get::<SourceGameplayTagBlendState>(node.id, StateKey::Default).unwrap();
                    assert!((state.context.curves[&crate::extensions::SourceCurveName::from("BoolCurve")] - 5.5).abs() < 1e-5);
                    assert!((interrupted["Pose"].as_pose().unwrap().get_bone(default()).unwrap().translation.unwrap().x - 0.1875).abs() < 1e-5);
                }
            }
            output.remove("Pose").unwrap().into_pose().unwrap()
        }
        let mut world = World::new();
        world.insert_resource(Assets::<AnimationGraph>::default());
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let skeleton = world.resource_mut::<Assets<Skeleton>>().add(Skeleton::default());
        let rotations = [Quat::from_rotation_y(0.2), Quat::from_rotation_x(2.4), Quat::from_rotation_z(-1.7)];
        let mut errors = Vec::new();
        for (kind, weights) in [("two", vec![0.75, 0.25]), ("tag", vec![0.75, 0.25]), ("bool", vec![0.75, 0.25]), ("bool_custom", vec![0.25, 0.75]), ("multi", vec![0.75, 0.25]), ("multi", vec![0.4, 0.3, 0.3]), ("multi", vec![0.2, 0.3, 0.1]), ("additive", vec![1.0, 0.25]), ("additive", vec![1.0, 0.75])] {
            let multi = kind == "multi";
            let additive = kind == "additive";
            let boolean = kind.starts_with("bool");
            for sign in [1.0, -1.0] {
                let mut graph = AnimationGraph::new();
                let blend = if multi {
                    AnimationNode::new("blend", crate::extensions::SourceMultiWayBlendNode {
                        pose_count: weights.len(), normalize_alpha: false,
                        desired_alpha_keys: vec![String::new(); weights.len()],
                        desired_alpha_defaults: weights.clone(), pose_pin_prefix: "Poses_".into(), ..default()
                    })
                } else if additive {
                    AnimationNode::new("blend", SourceApplyAdditiveNode { use_constant_alpha: true, constant_alpha: weights[1], ..default() })
                } else if boolean {
                    use bevy::reflect::serde::{ReflectDeserializer, ReflectSerializer};
                    use serde::de::DeserializeSeed;
                    let value = SourceBoolBlendNode {
                        active_key: "source_pose_test_bool".into(), blend_times: vec![0.08; 2],
                        blend_mode: if kind == "bool_custom" { crate::extensions::profile_blend::AlphaBlendMode::Custom } else { crate::extensions::profile_blend::AlphaBlendMode::Linear },
                        custom_curve: if kind == "bool_custom" { [(0.0, 0.0), (0.25, 0.75), (1.0, 1.0)].into_iter().map(|(t, v)| crate::extensions::RichCurveKey { t, v, ..default() }).collect() } else { vec![] },
                        ..default()
                    };
                    let mut registry = bevy::reflect::TypeRegistry::new();
                    registry.register::<SourceBoolBlendNode>();
                    let text = ron::to_string(&ReflectSerializer::new(&value, &registry)).unwrap();
                    let mut deserializer = ron::de::Deserializer::from_str(&text).unwrap();
                    let reflected = ReflectDeserializer::new(&registry).deserialize(&mut deserializer).unwrap();
                    AnimationNode::new("bool_blend", SourceBoolBlendNode::from_reflect(reflected.as_partial_reflect()).unwrap())
                } else if kind == "tag" {
                    AnimationNode::new("tag_blend", SourceGameplayTagBlendNode {
                        active_key: "source_pose_test_blend".into(), tags: vec!["Second".into()],
                        blend_times: vec![0.1, 0.02 / weights[1]], source_blend_times: vec![0.1, 0.02 / weights[1]],
                        pose_pin_names: vec!["A".into(), "B".into()], blend_time_pin_names: vec!["Time0".into(), "Time1".into()],
                        ..default()
                    })
                } else {
                    AnimationNode::new("blend", SourceTwoWayBlendNode { alpha_key: "alpha".into(), expose_alpha_pin: false, ..default() })
                };
                graph.add_node(blend.clone());
                graph.add_output_data("Pose".into(), DataSpec::Pose);
                graph.add_output_data_edge(blend.id, "Pose", "Pose");
                let mut inputs = HashMap::from([("alpha".into(), DataValue::F32(weights[1]))]);
                inputs.insert("expected_curve".into(), (4.0 * weights[0] + 12.0 * weights[1]).into());
                let mut expected_rotation = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
                let mut expected_translation = Vec3::ZERO;
                for (index, weight) in weights.iter().copied().enumerate() {
                    let key = format!("pose_{index}");
                    let node = AnimationNode::new(&key, crate::extensions::GlobalPoseInputNode { key: key.clone(), output: "Pose".into() });
                    graph.add_node(node.clone());
                    let pin = if boolean { format!("BlendPose_{index}") } else if multi { format!("Poses_{index}") } else if additive { if index == 0 { "Base".into() } else { "Additive".into() } } else if index == 0 { "A".into() } else { "B".into() };
                    if boolean {
                        let curve = AnimationNode::new(format!("curve_{index}"), SourceCurveModifyNode {
                            curve_name: "BoolCurve".into(), pin_default: if index == 0 { 4.0 } else { 12.0 }, alpha: 1.0, apply_mode: "Blend".into(), ..default()
                        });
                        graph.add_node(curve.clone());
                        graph.add_node_parameter_edge(node.id, "Pose", curve.id, "Source Pose");
                        graph.add_node_parameter_edge(curve.id, "Pose", blend.id, pin);
                    } else if kind == "two" && index == 1 {
                        let writer = AnimationNode::new("late_alpha", crate::extensions::source_instance::SourcePropertyWriteNode {
                            assignments: vec![crate::extensions::source_instance::SourcePropertyAssignment { property: "alpha".into(), value: DataValue::F32(0.9) }],
                            ..default()
                        });
                        graph.add_node(writer.clone());
                        graph.add_node_parameter_edge(node.id, "Pose", writer.id, "Source");
                        graph.add_node_parameter_edge(writer.id, "Pose", blend.id, pin);
                    } else {
                        graph.add_node_parameter_edge(node.id, "Pose", blend.id, pin);
                    }
                    let translation = Vec3::new(index as f32, 1.0, 0.0);
                    let rotation = rotations[index] * if index == 1 { sign } else { 1.0 };
                    let mut pose = Pose { skeleton: skeleton.clone(), timestamp: 2.5, root_motion: Some(RootMotionDelta { translation, rotation }), ..default() };
                    pose.add_bone(BonePose { translation: Some(translation), rotation: Some(rotation), scale: Some(Vec3::splat(if additive && index == 0 { 2.0 } else { 1.0 })), ..default() }, default());
                    inputs.insert(key, DataValue::Pose(pose));
                    expected_rotation += rotations[index] * weight;
                    expected_translation += translation * weight;
                }
                let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
                let output = world.run_system_cached_with(query, (handle, inputs)).unwrap();
                let bone = output.get_bone(default()).unwrap();
                let expected_rotation = if additive {
                    ((Quat::IDENTITY * (1.0 - weights[1]) + rotations[1] * weights[1]).normalize() * rotations[0]).normalize()
                } else { expected_rotation.normalize() };
                let expected_scale = Vec3::splat(weights.iter().sum::<f32>() * if additive { 2.0 } else { 1.0 });
                let root_motion = output.root_motion.as_ref().unwrap();
                if 1.0 - bone.rotation.unwrap().dot(expected_rotation).abs() > 1e-6
                    || bone.translation.unwrap().distance(expected_translation) > 1e-6
                    || bone.scale.unwrap().distance(expected_scale) > 1e-6
                    || 1.0 - root_motion.rotation.dot(expected_rotation).abs() > 1e-6
                    || root_motion.translation.distance(expected_translation) > 1e-6
                    || output.timestamp != 2.5 {
                    errors.push(format!("kind={kind}, weights={weights:?}, sign={sign}: {bone:?}, root={root_motion:?}"));
                }
            }
        }
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }

    #[test]
    fn source_two_way_reinitializes_players_and_advances_the_activation_frame() {
        use crate::extensions::SourceSequencePlayerNode;
        use crate::extensions::source_sequence_player::SourceSequenceSynchronization;
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
                let blend = graph.nodes.values().find(|node| node.name == "blend").unwrap();
                assert_eq!(arena.get_toplevel().node_states.get::<SourceTwoWayBlendState>(blend.id, bevy_animation_graph::core::context::node_states::StateKey::Default).unwrap().blend_alpha, alpha);
            }
            positions
        }
        let mut graphs = Assets::<AnimationGraph>::default();
        let mut clips = Assets::<GraphClip>::default();
        let clip = clips.add(GraphClip {
            duration: 1.0,
            ..default()
        });
        let mut nested = AnimationGraph::new();
        let nested_player = AnimationNode::new("nested player", SourceSequencePlayerNode {
            clip: clip.clone(), start_position: 0.2,
            expose_play_rate_pin: false, expose_start_position_pin: false, ..default()
        });
        nested.add_node(nested_player.clone());
        nested.add_output_data("Pose".into(), DataSpec::Pose);
        nested.add_output_data_edge(nested_player.id, "Pose", "Pose");
        let nested = graphs.add(nested);
        let mut cases = Vec::new();
        for (sync, reset, indirect) in [(false, true, false), (true, true, false), (true, false, false), (false, true, true)] {
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
                let player = if indirect && pin == "A" {
                    AnimationNode::new(pin, crate::extensions::LayeredGraphNode {
                        graph: nested.clone(), internalize_time: true, capture_context: true, ..default()
                    })
                } else {
                    AnimationNode::new(
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
                    )
                };
                graph.add_node(player.clone());
                graph.add_node_parameter_edge(player.id, "Pose", blend.id, pin);
                if reset && !(indirect && pin == "A") {
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
        state.context.curves.insert("PoseGrounded".into(), 1.0);
        assert_eq!(alpha, 0.5);
        assert_eq!(state.context.curves.get(&crate::extensions::SourceCurveName::from("PoseGrounded")), Some(&1.0));
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
        base.curves.insert("BaseOnly".into(), 0.25);
        base.curves.insert("Shared".into(), 0.5);
        let mut curve = AnimationContextState::default();
        curve.curves.insert("CurveOnly".into(), 0.4);
        curve.curves.insert("sHaReD".into(), 0.6);
        let accumulated = blend_source_curve_context(
            base.clone(),
            &curve,
            0.5,
            SourceCurvesBlendMode::Accumulate,
        );
        assert!((accumulated.curves[&crate::extensions::SourceCurveName::from("BaseOnly")] - 0.25).abs() < 1e-6);
        assert!((accumulated.curves[&crate::extensions::SourceCurveName::from("CurveOnly")] - 0.2).abs() < 1e-6);
        assert!((accumulated.curves[&crate::extensions::SourceCurveName::from("Shared")] - 0.8).abs() < 1e-6);
        let overridden =
            blend_source_curve_context(base, &curve, 0.5, SourceCurvesBlendMode::Override);
        assert_eq!(overridden.curves.get(&crate::extensions::SourceCurveName::from("BaseOnly")), None);
        assert!((overridden.curves[&crate::extensions::SourceCurveName::from("CurveOnly")] - 0.2).abs() < 1e-6);
        assert!((overridden.curves[&crate::extensions::SourceCurveName::from("Shared")] - 0.3).abs() < 1e-6);
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
    fn source_curves_blend_relevant_asset_uses_update_weights() {
        use crate::extensions::animation_context::SourceRelevantAsset;
        for mode in [SourceCurvesBlendMode::Accumulate, SourceCurvesBlendMode::Override] {
            for (base_weight, expected_duration) in [(0.8, 1.0), (0.2, 2.0)] {
                let base = AnimationContextState {
                    relevant_asset: Some(SourceRelevantAsset { duration: 1.0, weight: base_weight, ..default() }),
                    ..default()
                };
                let curve = AnimationContextState {
                    relevant_asset: Some(SourceRelevantAsset { duration: 2.0, weight: 1.0, ..default() }),
                    ..default()
                };
                let result = blend_source_curve_context(base, &curve, 0.5, mode);
                let clock = result.relevant_asset.unwrap();
                assert_eq!(clock.duration, expected_duration, "mode={mode:?}, base_weight={base_weight}");
                assert_eq!(clock.weight, base_weight.max(0.5));
            }
        }
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
        use crate::extensions::SourceAnimationInputs;

        let entity = Entity::from_raw_u32(992).unwrap();
        let mut inputs = SourceAnimationInputs::default();
        inputs.set_gameplay_tag_input(
            "locomotion_action",
            bevy_gameplay_tag::GameplayTag::new("Als.LocomotionAction.Ragdolling"),
        );
        let node = SourceGameplayTagBlendNode {
            active_key: "locomotion_action".to_owned(),
            active_tag_pin: "ActiveTag".to_owned(),
            tags: vec!["Als.LocomotionAction.Ragdolling".to_owned()],
            ..default()
        };
        assert_eq!(inputs.evaluate(entity, || node.active_index(entity)), Some(1));

    }

    #[test]
    fn source_gameplay_tag_blend_weights_missing_child_curves_as_zero() {
        let input = AnimationContextState {
            curves: [("FootLeftIk".into(), 1.0)].into_iter().collect(),
            curves_are_authoritative: true,
            ..default()
        };
        let empty = AnimationContextState {
            curves_are_authoritative: true,
            ..default()
        };
        let output = blend_animation_contexts([(input, 0.25), (empty.clone(), 0.75)]);
        assert!((output.curves[&crate::extensions::SourceCurveName::from("FootLeftIk")] - 0.25).abs() <= 1e-6);
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
