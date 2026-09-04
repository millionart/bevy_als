use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::linear::LinearInterpolator,
};
use serde::{Deserialize, Serialize};

use crate::{
    apply_mesh_space_layered_blend, build_branch_bone_mask, build_branch_filter_bone_mask_by_name,
    context_time_update, f_interp_to,
};

#[derive(Reflect, Clone, Debug, Default)]
struct SourceEnumPoseBlendState {
    initialized: bool,
    alpha: f32,
}

fn evaluator_delta(ctx: &NodeContext) -> Result<f32, GraphError> {
    Ok(match context_time_update(ctx)? {
        TimeUpdate::Delta(value) => value.max(0.0),
        TimeUpdate::Absolute(_) | TimeUpdate::PercentOfEvent { .. } => 0.0,
    })
}

fn source_enum_alpha(
    state: &mut SourceEnumPoseBlendState,
    selected: bool,
    blend_time: f32,
    delta: f32,
) -> f32 {
    let target = selected as u8 as f32;
    if !state.initialized {
        state.initialized = true;
        state.alpha = target;
        return target;
    }
    let speed = blend_time.max(f32::EPSILON).recip();
    state.alpha = f_interp_to(state.alpha, target, delta, speed);
    state.alpha
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceEnumPoseBlendNode {
    pub active_key: String,
    pub selected_enum_value: f32,
    pub active_enum_default: f32,
    pub default_blend_time: f32,
    pub selected_blend_time: f32,
    pub default_pose_pin: String,
    pub selected_pose_pin: String,
    pub default_blend_time_pin: String,
    pub selected_blend_time_pin: String,
    pub active_enum_pin: String,
    pub output_pin: String,
    pub source_visible_enum_entry: String,
}

impl Default for SourceEnumPoseBlendNode {
    fn default() -> Self {
        Self {
            active_key: default(),
            selected_enum_value: 0.0,
            active_enum_default: 0.0,
            default_blend_time: 0.0,
            selected_blend_time: 0.0,
            default_pose_pin: "Default Pose".to_owned(),
            selected_pose_pin: "Selected Pose".to_owned(),
            default_blend_time_pin: "Default Blend Time".to_owned(),
            selected_blend_time_pin: "Selected Blend Time".to_owned(),
            active_enum_pin: "Active Enum Value".to_owned(),
            output_pin: "Pose".to_owned(),
            source_visible_enum_entry: default(),
        }
    }
}

impl SourceEnumPoseBlendNode {
    fn active_value(&self, ctx: &NodeContext) -> Result<f32, GraphError> {
        if let Ok(value) = ctx.data_back(self.active_enum_pin.clone()) {
            return value.as_f32();
        }
        if self.active_key.is_empty() {
            return Ok(self.active_enum_default);
        }
        Ok(ctx
            .graph_context
            .global_input_data
            .get(&self.active_key)
            .and_then(|value| value.as_f32().ok())
            .unwrap_or(self.active_enum_default))
    }
}

impl NodeLike for SourceEnumPoseBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let value = self.active_value(&ctx)?;
        let selected = (value.round() - self.selected_enum_value).abs() <= f32::EPSILON;
        let blend_time = if selected {
            self.selected_blend_time
        } else {
            self.default_blend_time
        };
        let delta = evaluator_delta(&ctx)?;
        let alpha = source_enum_alpha(
            ctx.state_mut_or_else(SourceEnumPoseBlendState::default)?,
            selected,
            blend_time,
            delta,
        );
        let mut base = ctx.data_back(self.default_pose_pin.clone())?.into_pose()?;
        let overlay = ctx.data_back(self.selected_pose_pin.clone())?.into_pose()?;
        LinearInterpolator {
            bone_mask: BoneMask::all(),
        }
        .interpolate_pose(&mut base, &overlay, alpha);
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd(self.output_pin.clone(), base);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(self.default_pose_pin.clone(), DataSpec::Pose)
            .add_input_data(self.selected_pose_pin.clone(), DataSpec::Pose)
            .add_input_data(self.default_blend_time_pin.clone(), DataSpec::F32)
            .add_input_data(self.selected_blend_time_pin.clone(), DataSpec::F32)
            .add_input_data(self.active_enum_pin.clone(), DataSpec::F32)
            .add_output_data(self.output_pin.clone(), DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Enum Pose Blend".to_owned()
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default)]
#[reflect(Default, Serialize)]
pub struct SourceBranchFilter {
    pub bone: String,
    pub blend_depth: i32,
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceLayeredBoneBlendNode {
    pub branch_bones: Vec<String>,
    pub branch_bone_layers: Vec<Vec<String>>,
    pub branch_filters: Vec<SourceBranchFilter>,
    pub branch_filter_layers: Vec<Vec<SourceBranchFilter>>,
    pub blend_weight: f32,
    pub blend_weights: Vec<f32>,
    pub blend_weight_pin_prefix: String,
    pub expose_blend_weight_pins: bool,
    pub mesh_space_rotation_blend: bool,
    pub blend_root_motion_based_on_root_bone: bool,
    pub base_pose_pin: String,
    pub blend_pose_pin: String,
    pub blend_pose_pins: Vec<String>,
    pub output_pin: String,
}

impl Default for SourceLayeredBoneBlendNode {
    fn default() -> Self {
        Self {
            branch_bones: default(),
            branch_bone_layers: default(),
            branch_filters: default(),
            branch_filter_layers: default(),
            blend_weight: 1.0,
            blend_weights: default(),
            blend_weight_pin_prefix: "BlendWeights_".to_owned(),
            expose_blend_weight_pins: false,
            mesh_space_rotation_blend: true,
            blend_root_motion_based_on_root_bone: true,
            base_pose_pin: "BasePose".to_owned(),
            blend_pose_pin: "BlendPoses_0".to_owned(),
            blend_pose_pins: default(),
            output_pin: "Pose".to_owned(),
        }
    }
}

impl SourceLayeredBoneBlendNode {
    fn layer_pin(&self, index: usize) -> &str {
        self.blend_pose_pins
            .get(index)
            .map(String::as_str)
            .unwrap_or(&self.blend_pose_pin)
    }

    fn layer_bones(&self, index: usize) -> &[String] {
        self.branch_bone_layers
            .get(index)
            .map(Vec::as_slice)
            .unwrap_or(&self.branch_bones)
    }

    fn layer_filters(&self, index: usize) -> &[SourceBranchFilter] {
        self.branch_filter_layers
            .get(index)
            .map(Vec::as_slice)
            .unwrap_or(&self.branch_filters)
    }

    fn layer_weight(&self, ctx: &NodeContext, index: usize) -> Result<f32, GraphError> {
        let fallback = self
            .blend_weights
            .get(index)
            .copied()
            .unwrap_or(self.blend_weight);
        if !self.expose_blend_weight_pins {
            return Ok(fallback);
        }
        ctx.data_back(format!("{}{}", self.blend_weight_pin_prefix, index))?
            .as_f32()
    }

    fn layer_count(&self) -> usize {
        self.blend_pose_pins.len().max(1)
    }
}

impl NodeLike for SourceLayeredBoneBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let mut base = ctx.data_back(self.base_pose_pin.clone())?.into_pose()?;
        for index in 0..self.layer_count() {
            let overlay = ctx
                .data_back(self.layer_pin(index).to_owned())?
                .into_pose()?;
            let skeletons = &ctx.graph_context.resources.skeleton_assets;
            let skeleton = skeletons
                .get(&base.skeleton)
                .or_else(|| skeletons.get(&overlay.skeleton));
            if let Some(skeleton) = skeleton {
                if base.skeleton != overlay.skeleton && skeletons.get(&base.skeleton).is_none() {
                    base.skeleton = overlay.skeleton.clone();
                }
                let filters = self.layer_filters(index);
                let mask = if filters.is_empty() {
                    let names = self
                        .layer_bones(index)
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>();
                    build_branch_bone_mask(skeleton, &names, 1.0)
                } else {
                    build_branch_filter_bone_mask_by_name(
                        skeleton,
                        &filters
                            .iter()
                            .map(|filter| (filter.bone.as_str(), filter.blend_depth))
                            .collect::<Vec<_>>(),
                    )
                };
                if self.mesh_space_rotation_blend {
                    apply_mesh_space_layered_blend(
                        &mut base,
                        &overlay,
                        self.layer_weight(&ctx, index)?,
                        &mask,
                        skeleton,
                    );
                } else {
                    LinearInterpolator { bone_mask: mask }.interpolate_pose(
                        &mut base,
                        &overlay,
                        self.layer_weight(&ctx, index)?.clamp(0.0, 1.0),
                    );
                }
            }
        }
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd(self.output_pin.clone(), base);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(self.base_pose_pin.clone(), DataSpec::Pose);
        for index in 0..self.layer_count() {
            ctx.add_input_data(self.layer_pin(index).to_owned(), DataSpec::Pose);
            if self.expose_blend_weight_pins {
                ctx.add_input_data(
                    format!("{}{}", self.blend_weight_pin_prefix, index),
                    DataSpec::F32,
                );
            }
        }
        ctx.add_output_data(self.output_pin.clone(), DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Layered Bone Blend".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_enum_pose_blend_initializes_to_the_selected_source_entry() {
        let mut state = SourceEnumPoseBlendState::default();
        assert_eq!(source_enum_alpha(&mut state, true, 0.1, 0.0), 1.0);
        assert_eq!(source_enum_alpha(&mut state, false, 0.1, 0.0), 1.0);
        assert!(source_enum_alpha(&mut state, false, 0.1, 0.1) < 1.0);
    }

    #[test]
    fn source_enum_pose_blend_can_preserve_source_pin_names() {
        let node = SourceEnumPoseBlendNode {
            default_pose_pin: "BlendPose_0".to_owned(),
            selected_pose_pin: "BlendPose_1".to_owned(),
            default_blend_time_pin: "BlendTime_0".to_owned(),
            selected_blend_time_pin: "BlendTime_1".to_owned(),
            active_enum_pin: "ActiveEnumValue".to_owned(),
            ..default()
        };
        assert_eq!(node.selected_pose_pin, "BlendPose_1");
        assert_eq!(node.active_enum_pin, "ActiveEnumValue");
    }

    #[test]
    fn source_layered_bone_blend_defaults_preserve_source_pin_names() {
        let node = SourceLayeredBoneBlendNode::default();
        assert_eq!(node.base_pose_pin, "BasePose");
        assert_eq!(node.blend_pose_pin, "BlendPoses_0");
        assert!(node.mesh_space_rotation_blend);
    }

    #[test]
    fn source_layered_bone_blend_keeps_each_source_layer_pin_and_mask() {
        let node = SourceLayeredBoneBlendNode {
            branch_bone_layers: vec![vec!["thigh_l".to_owned()], vec!["thigh_r".to_owned()]],
            branch_filter_layers: vec![
                vec![SourceBranchFilter {
                    bone: "thigh_l".to_owned(),
                    blend_depth: 3,
                }],
                vec![SourceBranchFilter {
                    bone: "thigh_r".to_owned(),
                    blend_depth: -1,
                }],
            ],
            blend_weights: vec![1.0, 0.5],
            blend_pose_pins: vec!["BlendPoses_0".to_owned(), "BlendPoses_1".to_owned()],
            ..default()
        };
        assert_eq!(node.layer_count(), 2);
        assert_eq!(node.layer_pin(0), "BlendPoses_0");
        assert_eq!(node.layer_pin(1), "BlendPoses_1");
        assert_eq!(node.layer_bones(0), ["thigh_l"]);
        assert_eq!(node.layer_bones(1), ["thigh_r"]);
        assert_eq!(node.layer_filters(0)[0].blend_depth, 3);
        assert_eq!(node.layer_filters(1)[0].blend_depth, -1);
    }
}
