use std::collections::HashMap;

use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::linear::LinearInterpolator,
    pose::Pose,
    skeleton::Skeleton,
};
use serde::{Deserialize, Serialize};

use crate::extensions::{RichCurveKey, sample_rich_curve};

const WEIGHT_THRESHOLD: f32 = 1e-6;

pub fn curve_a_input(curve: &str) -> String {
    format!("curve_a_{curve}")
}

pub fn curve_b_input(curve: &str) -> String {
    format!("curve_b_{curve}")
}

#[derive(Reflect, Clone, Debug, Default, PartialEq)]
#[reflect(Default)]
pub struct BoneBlendScale {
    pub bone: String,
    pub scale: f32,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Default, Serialize)]
pub enum AlphaBlendMode {
    #[default]
    Linear,
    Cubic,
    HermiteCubic,
    QuadraticInOut,
    Custom,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct ProfiledBlendNode {
    pub curve_names: Vec<String>,
    pub bone_scales: Vec<BoneBlendScale>,
    pub alpha_blend_mode: AlphaBlendMode,
    pub custom_curve: Vec<RichCurveKey>,
}

impl ProfiledBlendNode {
    pub const FACTOR: &'static str = "factor";
    pub const IN_POSE_A: &'static str = "pose_a";
    pub const IN_TIME_A: &'static str = "time_a";
    pub const IN_POSE_B: &'static str = "pose_b";
    pub const IN_TIME_B: &'static str = "time_b";
    pub const OUT_POSE: &'static str = "pose";
}

pub fn profile_target_weight(alpha: f32, scale: f32) -> f32 {
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= WEIGHT_THRESHOLD {
        return 0.0;
    }
    if alpha >= 1.0 - WEIGHT_THRESHOLD {
        return 1.0;
    }
    let scale = scale.max(WEIGHT_THRESHOLD);
    let target = alpha * scale;
    let source = (1.0 - alpha) / scale;
    target / (target + source)
}

pub fn transition_alpha(
    alpha: f32,
    blend_mode: AlphaBlendMode,
    custom_curve: &[RichCurveKey],
) -> f32 {
    let alpha = alpha.clamp(0.0, 1.0);
    let value = match blend_mode {
        AlphaBlendMode::Linear => alpha,
        AlphaBlendMode::Cubic | AlphaBlendMode::HermiteCubic => alpha * alpha * (3.0 - 2.0 * alpha),
        AlphaBlendMode::QuadraticInOut => {
            if alpha < 0.5 {
                2.0 * alpha * alpha
            } else {
                1.0 - (-2.0 * alpha + 2.0).powi(2) * 0.5
            }
        }
        AlphaBlendMode::Custom => match (custom_curve.first(), custom_curve.last()) {
            (Some(first), Some(last)) => {
                sample_rich_curve(custom_curve, first.t + (last.t - first.t) * alpha)
            }
            _ => alpha,
        },
    };
    value.clamp(0.0, 1.0)
}

fn scale_lookup(scales: &[BoneBlendScale]) -> HashMap<&str, f32> {
    scales
        .iter()
        .map(|scale| (scale.bone.as_str(), scale.scale))
        .collect()
}

fn profile_mask(skeleton: &Skeleton, scales: &[BoneBlendScale], alpha: f32) -> BoneMask {
    let mut mask = BoneMask::all();
    if alpha <= WEIGHT_THRESHOLD {
        return mask;
    }
    let scales = scale_lookup(scales);
    for bone in skeleton.iter_bones() {
        let Some(path) = skeleton.id_to_path(bone) else {
            continue;
        };
        let Some(name) = path.parts.last() else {
            continue;
        };
        let Some(scale) = scales.get(name.as_str()).copied() else {
            continue;
        };
        let target_weight = profile_target_weight(alpha, scale);
        mask.add_bone_weight(bone, path, target_weight / alpha);
    }
    mask
}

fn blend_pose(
    source: &mut Pose,
    target: &Pose,
    alpha: f32,
    skeleton: Option<&Skeleton>,
    scales: &[BoneBlendScale],
) {
    let mask = match skeleton {
        Some(skeleton) if !scales.is_empty() => profile_mask(skeleton, scales, alpha),
        _ => BoneMask::all(),
    };
    LinearInterpolator { bone_mask: mask }.interpolate_pose(source, target, alpha);
}

impl NodeLike for ProfiledBlendNode {
    fn display_name(&self) -> String {
        "ALS Profiled Blend".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(ctx.duration_back(Self::IN_TIME_A)?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        ctx.set_time_update_back(Self::IN_TIME_A, update.clone());
        let mut source = ctx.data_back(Self::IN_POSE_A)?.into_pose()?;
        ctx.set_time_update_back(Self::IN_TIME_B, TimeUpdate::Absolute(source.timestamp));
        let target = ctx.data_back(Self::IN_POSE_B)?.into_pose()?;
        let source_context =
            crate::extensions::animation_context::pose_input_context_state(&ctx, Self::IN_POSE_A)
                .unwrap_or_default();
        let target_context =
            crate::extensions::animation_context::pose_input_context_state(&ctx, Self::IN_POSE_B)
                .unwrap_or_default();
        let alpha = transition_alpha(
            ctx.data_back(Self::FACTOR)?.as_f32()?,
            self.alpha_blend_mode,
            &self.custom_curve,
        );
        let skeleton = ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&source.skeleton);
        blend_pose(&mut source, &target, alpha, skeleton, &self.bone_scales);
        let mut context = crate::extensions::source_pose_nodes::blend_animation_contexts([
            (source_context, 1.0 - alpha),
            (target_context, alpha),
        ]);
        for curve in &self.curve_names {
            let source_value = ctx.data_back(curve_a_input(curve))?.as_f32()?;
            let target_value = ctx.data_back(curve_b_input(curve))?.as_f32()?;
            context.curves.insert(
                curve.clone(),
                source_value + (target_value - source_value) * alpha,
            );
            ctx.set_data_fwd(
                curve.clone(),
                source_value + (target_value - source_value) * alpha,
            );
        }
        ctx.set_time(source.timestamp);
        ctx.set_data_fwd(Self::OUT_POSE, source);
        *ctx.state_mut_or_else(crate::extensions::AnimationContextState::default)? = context;
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
        for curve in &self.curve_names {
            ctx.add_input_data(curve_a_input(curve), DataSpec::F32)
                .add_input_data(curve_b_input(curve), DataSpec::F32)
                .add_output_data(curve.clone(), DataSpec::F32);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bevy_animation_graph::core::{animation_clip::EntityPath, pose::BonePose};

    use super::*;

    fn skeleton() -> (Skeleton, bevy_animation_graph::core::pose::BoneId) {
        let root_path = EntityPath::default().child("root");
        let foot_path = root_path.child("foot_l");
        let root = root_path.id();
        let foot = foot_path.id();
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root);
        skeleton.add_bone(root_path, Transform::IDENTITY, Transform::IDENTITY);
        skeleton.add_bone(foot_path, Transform::IDENTITY, Transform::IDENTITY);
        (skeleton, foot)
    }

    #[test]
    fn weight_factor_profile_accelerates_listed_bones() {
        let global = profile_target_weight(0.1, 1.0);
        let quick_feet = profile_target_weight(0.1, 5.0);
        assert!((global - 0.1).abs() < 1e-6);
        assert!((quick_feet - 0.7352941).abs() < 1e-6);
    }

    #[test]
    fn alpha_blend_options_match_unreal_curve_mapping() {
        assert!((transition_alpha(0.25, AlphaBlendMode::Cubic, &[]) - 0.15625).abs() < 1e-6);
        assert!((transition_alpha(0.25, AlphaBlendMode::QuadraticInOut, &[]) - 0.125).abs() < 1e-6);
        assert!((transition_alpha(0.75, AlphaBlendMode::QuadraticInOut, &[]) - 0.875).abs() < 1e-6);
        let aim_in = [
            RichCurveKey {
                interpolation: crate::extensions::RichCurveInterpolation::Cubic,
                tangent_mode: crate::extensions::RichCurveTangentMode::User,
                t: 0.2,
                v: 0.25,
                arrive_tangent: 2.5235951,
                arrive_weight: 0.9048326,
                leave_tangent: 2.5235951,
                leave_weight: 0.9048368,
                ..default()
            },
            RichCurveKey {
                t: 1.0,
                v: 1.0,
                ..default()
            },
        ];
        assert!((transition_alpha(0.5, AlphaBlendMode::Custom, &aim_in) - 0.8773595).abs() < 1e-5);
    }

    #[test]
    fn single_key_custom_curve_holds_its_value() {
        for value in [-0.5_f32, 0.7, 1.5] {
            let keys = [RichCurveKey { t: 0.4, v: value, ..default() }];
            for time in [-10.0, 0.0, 0.4, 1.0, 10.0] {
                assert_eq!(sample_rich_curve(&keys, time), value);
                assert_eq!(transition_alpha(time, AlphaBlendMode::Custom, &keys), value.clamp(0.0, 1.0));
            }
        }
        assert_eq!(transition_alpha(0.4, AlphaBlendMode::Custom, &[]), 0.4);
    }

    #[test]
    fn custom_stance_curve_matches_the_source_midpoint() {
        let curve = [
            RichCurveKey {
                interpolation: crate::extensions::RichCurveInterpolation::Cubic,
                t: 0.0,
                v: 0.0,
                ..default()
            },
            RichCurveKey {
                interpolation: crate::extensions::RichCurveInterpolation::Cubic,
                tangent_mode: crate::extensions::RichCurveTangentMode::User,
                t: 0.4,
                v: 0.4,
                arrive_tangent: 2.730308,
                leave_tangent: 2.730308,
                ..default()
            },
            RichCurveKey {
                t: 1.0,
                v: 1.0,
                ..default()
            },
        ];
        assert!((transition_alpha(0.5, AlphaBlendMode::Custom, &curve) - 0.63404917).abs() < 1e-6);
    }

    #[test]
    fn profiled_blend_applies_normalized_target_weight_to_listed_bone() {
        let (skeleton, foot) = skeleton();
        let mut source = Pose::default();
        source.add_bone(
            BonePose {
                translation: Some(Vec3::ZERO),
                ..default()
            },
            foot,
        );
        let mut target = Pose::default();
        target.add_bone(
            BonePose {
                translation: Some(Vec3::X),
                ..default()
            },
            foot,
        );
        blend_pose(
            &mut source,
            &target,
            0.1,
            Some(&skeleton),
            &[BoneBlendScale {
                bone: "foot_l".to_owned(),
                scale: 5.0,
            }],
        );
        let value = source.get_bone(foot).unwrap().translation.unwrap().x;
        assert!((value - 0.7352941).abs() < 1e-6);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<ProfiledBlendNode>();
        let registration = registry
            .get(std::any::TypeId::of::<ProfiledBlendNode>())
            .unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
