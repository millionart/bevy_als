use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::linear::LinearInterpolator,
};

use crate::{
    apply_mesh_space_layered_blend, build_branch_bone_mask, context_time_update, f_interp_to,
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

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceLayeredBoneBlendNode {
    pub branch_bones: Vec<String>,
    pub blend_weight: f32,
    pub mesh_space_rotation_blend: bool,
    pub blend_root_motion_based_on_root_bone: bool,
    pub base_pose_pin: String,
    pub blend_pose_pin: String,
    pub output_pin: String,
}

impl Default for SourceLayeredBoneBlendNode {
    fn default() -> Self {
        Self {
            branch_bones: default(),
            blend_weight: 1.0,
            mesh_space_rotation_blend: true,
            blend_root_motion_based_on_root_bone: true,
            base_pose_pin: "Base Pose".to_owned(),
            blend_pose_pin: "Blend Poses 0".to_owned(),
            output_pin: "Pose".to_owned(),
        }
    }
}

impl NodeLike for SourceLayeredBoneBlendNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let mut base = ctx.data_back(self.base_pose_pin.clone())?.into_pose()?;
        let overlay = ctx.data_back(self.blend_pose_pin.clone())?.into_pose()?;
        let skeletons = &ctx.graph_context.resources.skeleton_assets;
        let skeleton = skeletons
            .get(&base.skeleton)
            .or_else(|| skeletons.get(&overlay.skeleton));
        if let Some(skeleton) = skeleton {
            if base.skeleton != overlay.skeleton && skeletons.get(&base.skeleton).is_none() {
                base.skeleton = overlay.skeleton.clone();
            }
            let names = self
                .branch_bones
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            let mask = build_branch_bone_mask(skeleton, &names, 1.0);
            if self.mesh_space_rotation_blend {
                apply_mesh_space_layered_blend(
                    &mut base,
                    &overlay,
                    self.blend_weight,
                    &mask,
                    skeleton,
                );
            } else {
                LinearInterpolator { bone_mask: mask }.interpolate_pose(
                    &mut base,
                    &overlay,
                    self.blend_weight.clamp(0.0, 1.0),
                );
            }
        }
        ctx.set_time(base.timestamp);
        ctx.set_data_fwd(self.output_pin.clone(), base);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(self.base_pose_pin.clone(), DataSpec::Pose)
            .add_input_data(self.blend_pose_pin.clone(), DataSpec::Pose)
            .add_output_data(self.output_pin.clone(), DataSpec::Pose);
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
    fn source_layered_bone_blend_defaults_preserve_visible_pin_names() {
        let node = SourceLayeredBoneBlendNode::default();
        assert_eq!(node.base_pose_pin, "Base Pose");
        assert_eq!(node.blend_pose_pin, "Blend Poses 0");
        assert!(node.mesh_space_rotation_blend);
    }
}
