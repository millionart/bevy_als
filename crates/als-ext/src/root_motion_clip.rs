use std::any::TypeId;

use bevy::{
    animation::animation_curves::WeightsCurveSample,
    asset::Handle,
    math::{Quat, Vec3},
    platform::hash::Hashed,
    prelude::{EvaluatorId, Transform, VariableCurve},
    reflect::prelude::*,
};
use bevy_animation_graph::core::{
    animation_clip::{EntityPath, GraphClip},
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
    id::BoneId,
    pose::{BonePose, Pose, RootMotionDelta, RootMotionMode},
};

use crate::root_motion_defaults::derive_default_root_bone;

pub const IN_TIME: &str = "time";
pub const OUT_POSE: &str = "pose";

#[derive(Reflect, Clone, Debug, Default)]
struct RmPrevState {
    time: f32,
    cached_root: Option<EntityPath>,
}

enum CurveSample {
    Translation(Vec3),
    Rotation(Quat),
    Scale(Vec3),
    BoneWeights(Vec<f32>),
}

fn sample_curve(curve: &VariableCurve, time: f32) -> Option<CurveSample> {
    let evaluator_id = curve.0.evaluator_id();
    let translation_id = Hashed::new((TypeId::of::<Transform>(), 0));
    let rotation_id = Hashed::new((TypeId::of::<Transform>(), 1));
    let scale_id = Hashed::new((TypeId::of::<Transform>(), 2));

    match evaluator_id {
        EvaluatorId::ComponentField(id) => {
            if id == &translation_id {
                Some(CurveSample::Translation(
                    *curve.0.sample_clamped(time).downcast_ref()?,
                ))
            } else if id == &rotation_id {
                Some(CurveSample::Rotation(
                    *curve.0.sample_clamped(time).downcast_ref()?,
                ))
            } else if id == &scale_id {
                Some(CurveSample::Scale(
                    *curve.0.sample_clamped(time).downcast_ref()?,
                ))
            } else {
                None
            }
        }
        EvaluatorId::Type(_) => Some(CurveSample::BoneWeights(
            (*curve
                .0
                .sample_clamped(time)
                .downcast::<WeightsCurveSample>()
                .ok()?)
            .0,
        )),
    }
}

fn sample_root(curves: &[VariableCurve], t: f32) -> (Vec3, Quat) {
    let mut tr = Vec3::ZERO;
    let mut rot = Quat::IDENTITY;
    for curve in curves {
        match sample_curve(curve, t) {
            Some(CurveSample::Translation(v)) => tr = v,
            Some(CurveSample::Rotation(v)) => rot = v,
            _ => {}
        }
    }
    (tr, rot)
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct RootMotionClipNode {
    pub clip: Handle<GraphClip>,
    pub root_motion_mode: RootMotionMode,
    pub root_motion_bone: Option<EntityPath>,
}

impl RootMotionClipNode {
    fn clip_duration(&self, ctx: &NodeContext) -> Option<f32> {
        ctx.graph_context
            .resources
            .graph_clip_assets
            .get(&self.clip)
            .map(|c| c.duration())
    }
}

impl NodeLike for RootMotionClipNode {
    fn display_name(&self) -> String {
        "ALS Root Motion Clip".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let duration = self.clip_duration(&ctx);
        ctx.set_duration_fwd(duration);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let Some(clip) = ctx
            .graph_context
            .resources
            .graph_clip_assets
            .get(&self.clip)
        else {
            ctx.set_data_fwd(OUT_POSE, DataValue::Pose(Pose::default()));
            return Ok(());
        };
        let clip_duration = clip.duration();

        let time_update = ctx.time_update_fwd()?;
        let clamped_prev = {
            let prev = ctx.state_mut_or_else(RmPrevState::default)?;
            prev.time.clamp(0.0, clip_duration)
        };
        let current_time = match time_update {
            TimeUpdate::Absolute(t) => t,
            TimeUpdate::Delta(dt) => clamped_prev + dt,
            TimeUpdate::PercentOfEvent { .. } => clamped_prev,
        }
        .clamp(0.0, clip_duration);

        let mut out_pose = Pose {
            timestamp: current_time,
            skeleton: clip.skeleton.clone(),
            ..Pose::default()
        };

        for (bone_id, curves) in &clip.curves {
            let mut bone_pose = BonePose::default();
            for curve in curves {
                match sample_curve(curve, current_time) {
                    Some(CurveSample::Translation(t)) => bone_pose.translation = Some(t),
                    Some(CurveSample::Rotation(r)) => bone_pose.rotation = Some(r),
                    Some(CurveSample::Scale(s)) => bone_pose.scale = Some(s),
                    Some(CurveSample::BoneWeights(w)) => bone_pose.weights = Some(w),
                    None => {}
                }
            }
            out_pose.add_bone(bone_pose, BoneId::from(*bone_id));
        }

        if self.root_motion_mode != RootMotionMode::Disabled {
            let Some(skeleton) = ctx
                .graph_context
                .resources
                .skeleton_assets
                .get(&clip.skeleton)
            else {
                ctx.set_data_fwd(OUT_POSE, DataValue::Pose(out_pose));
                return Ok(());
            };
            let cached_root = {
                let prev = ctx.state_mut_or_else(RmPrevState::default)?;
                prev.cached_root.clone()
            };
            let root_path = match cached_root {
                Some(p) => Some(p),
                None => {
                    let derived = self
                        .root_motion_bone
                        .clone()
                        .or_else(|| derive_default_root_bone(clip, skeleton));
                    if let Some(p) = derived.clone()
                        && let Ok(mut prev) = ctx.state_mut::<RmPrevState>()
                    {
                        prev.cached_root = Some(p.clone());
                    }
                    derived
                }
            };
            if let Some(root_path) = root_path {
                let root_bone_id: BoneId = root_path.id();
                let target_id = root_bone_id.animation_target_id();
                if !clip.curves.contains_key(&target_id) {
                    bevy::log::warn_once!(
                        "RootMotionClipNode: no curves for root bone {:?}",
                        root_path
                    );
                }
                let empty: Vec<VariableCurve> = Vec::new();
                let root_curves = clip.curves.get(&target_id).unwrap_or(&empty);
                let (cur_tr, cur_rot) = sample_root(root_curves, current_time);
                let (prev_tr, prev_rot) = sample_root(root_curves, clamped_prev);
                let mut delta_translation = cur_tr - prev_tr;
                let mut delta_rotation = prev_rot.inverse() * cur_rot;

                let rest_local = skeleton.default_transforms(root_bone_id).map(|dt| dt.local);
                let rest_translation = rest_local.map(|t| t.translation).unwrap_or(Vec3::ZERO);
                let rest_rotation = rest_local.map(|t| t.rotation).unwrap_or(Quat::IDENTITY);

                match self.root_motion_mode {
                    RootMotionMode::Full => {
                        if let Some(bone_idx) = out_pose.paths.get(&root_bone_id).copied() {
                            out_pose.bones[bone_idx].translation = Some(rest_translation);
                            out_pose.bones[bone_idx].rotation = Some(rest_rotation);
                        }
                    }
                    RootMotionMode::GroundPlane => {
                        delta_translation.y = 0.0;
                        let (axis, angle) = delta_rotation.to_axis_angle();
                        let y_angle = angle * axis.y;
                        delta_rotation = Quat::from_rotation_y(y_angle);
                        if let Some(bone_idx) = out_pose.paths.get(&root_bone_id).copied()
                            && let Some(ref mut t) = out_pose.bones[bone_idx].translation
                        {
                            t.x = rest_translation.x;
                            t.z = rest_translation.z;
                        }
                    }
                    RootMotionMode::Disabled => {}
                }

                out_pose.root_motion = Some(RootMotionDelta {
                    translation: delta_translation,
                    rotation: delta_rotation,
                });
            }
        }

        if let Ok(prev) = ctx.state_mut::<RmPrevState>() {
            prev.time = current_time;
        }

        ctx.set_time(current_time);
        ctx.set_data_fwd(OUT_POSE, DataValue::Pose(out_pose));
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_time(IN_TIME);
        ctx.add_output_data(OUT_POSE, DataSpec::Pose)
            .add_output_time();
        Ok(())
    }
}
