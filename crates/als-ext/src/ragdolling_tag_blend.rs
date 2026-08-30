use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::linear::LinearInterpolator,
    pose::Pose,
};

use crate::{
    GameplayTagBlendMode,
    curve_bundle::{active_curve_weights, curve_value_input},
    gameplay_tag_blend::{
        GameplayTagBlendState, WEIGHT_THRESHOLD, advance_gameplay_tag_weights,
        select_gameplay_tag_child_index,
    },
};

pub const ACTIVE_INDEX: &str = "active_index";
pub const INPUT_POSE: &str = "input_pose";
pub const INPUT_TIME: &str = "input_time";
pub const SNAPSHOT_POSE: &str = "snapshot_pose";
pub const SNAPSHOT_TIME: &str = "snapshot_time";
pub const FLAIL_POSE: &str = "flail_pose";
pub const FLAIL_TIME: &str = "flail_time";
pub const OUT_POSE: &str = "pose";
pub const OUTER_INPUT_WEIGHT: &str = "outer_input_weight";
pub const OUTER_RAGDOLL_WEIGHT: &str = "outer_ragdoll_weight";
pub const INNER_SNAPSHOT_WEIGHT: &str = "inner_snapshot_weight";
pub const INNER_FLAIL_WEIGHT: &str = "inner_flail_weight";

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct RagdollingTagBlendState {
    outer: GameplayTagBlendState,
    inner: GameplayTagBlendState,
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct RagdollingTagBlendNode {
    pub tag: String,
    pub outer_blend_times: [f32; 2],
    pub inner_blend_times: [f32; 2],
    pub blend_mode: GameplayTagBlendMode,
    pub curve_names: Vec<String>,
    pub normalize_curve_weights: bool,
}

impl Default for RagdollingTagBlendNode {
    fn default() -> Self {
        Self {
            tag: "Als.LocomotionAction.Ragdolling".to_owned(),
            outer_blend_times: [0.0, 0.0],
            inner_blend_times: [0.0, 0.0],
            blend_mode: GameplayTagBlendMode::Linear,
            curve_names: default(),
            normalize_curve_weights: false,
        }
    }
}

fn advance_weights(
    state: &mut GameplayTagBlendState,
    active_index: usize,
    blend_times: [f32; 2],
    blend_mode: GameplayTagBlendMode,
    delta_seconds: f32,
) -> Vec<f32> {
    advance_gameplay_tag_weights(
        state,
        2,
        active_index,
        blend_times[active_index],
        blend_mode,
        delta_seconds,
    )
}

fn blend_poses(mut poses: Vec<(Pose, f32)>, fallback: Pose) -> Pose {
    let fallback_skeleton = fallback.skeleton.clone();
    let Some((mut output, mut accumulated_weight)) = poses.pop() else {
        return fallback;
    };
    for (pose, weight) in poses {
        if weight <= WEIGHT_THRESHOLD {
            continue;
        }
        let factor = weight / (accumulated_weight + weight);
        LinearInterpolator {
            bone_mask: BoneMask::all(),
        }
        .interpolate_pose(&mut output, &pose, factor);
        accumulated_weight += weight;
    }
    output.skeleton = fallback_skeleton;
    output
}

impl NodeLike for RagdollingTagBlendNode {
    fn display_name(&self) -> String {
        "ALS Ragdolling Tag Blend".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(None);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let active_index = select_gameplay_tag_child_index(
            std::slice::from_ref(&self.tag),
            ctx.data_back(ACTIVE_INDEX)?.as_f32()?,
        );
        let time_update = ctx.time_update_fwd()?;
        let delta_seconds = match &time_update {
            TimeUpdate::Delta(value) => *value,
            TimeUpdate::Absolute(_) | TimeUpdate::PercentOfEvent { .. } => 0.0,
        };
        let outer_weights = {
            let state = ctx.state_mut::<RagdollingTagBlendState>()?;
            advance_weights(
                &mut state.outer,
                active_index,
                self.outer_blend_times,
                self.blend_mode,
                delta_seconds,
            )
        };
        let input_pose = ctx.data_back(INPUT_POSE)?.into_pose()?;
        let mut inner_weights = vec![0.0, 0.0];
        let mut inner_pose = None;
        if outer_weights[1] > WEIGHT_THRESHOLD {
            inner_weights = {
                let state = ctx.state_mut::<RagdollingTagBlendState>()?;
                advance_weights(
                    &mut state.inner,
                    active_index,
                    self.inner_blend_times,
                    self.blend_mode,
                    delta_seconds,
                )
            };
            let mut inner_poses = Vec::new();
            if inner_weights[0] > WEIGHT_THRESHOLD {
                ctx.set_time_update_back(SNAPSHOT_TIME, time_update.clone());
                inner_poses.push((ctx.data_back(SNAPSHOT_POSE)?.into_pose()?, inner_weights[0]));
            }
            if inner_weights[1] > WEIGHT_THRESHOLD {
                ctx.set_time_update_back(FLAIL_TIME, time_update.clone());
                inner_poses.push((ctx.data_back(FLAIL_POSE)?.into_pose()?, inner_weights[1]));
            }
            inner_pose = Some(blend_poses(inner_poses, input_pose.clone()));
        }
        let mut outer_poses = Vec::new();
        if outer_weights[0] > WEIGHT_THRESHOLD {
            ctx.set_time_update_back(INPUT_TIME, time_update.clone());
            outer_poses.push((input_pose.clone(), outer_weights[0]));
        }
        if outer_weights[1] > WEIGHT_THRESHOLD {
            outer_poses.push((inner_pose.unwrap_or_default(), outer_weights[1]));
        }
        let output = blend_poses(outer_poses, input_pose);
        let curve_weights = active_curve_weights(&outer_weights, self.normalize_curve_weights);
        for curve in &self.curve_names {
            let value = if curve_weights[0] > WEIGHT_THRESHOLD {
                ctx.data_back(curve_value_input(0, curve))?.as_f32()? * curve_weights[0]
            } else {
                0.0
            };
            ctx.set_data_fwd(curve.clone(), value);
        }
        ctx.set_time(output.timestamp);
        ctx.set_data_fwd(OUT_POSE, output);
        ctx.set_data_fwd(OUTER_INPUT_WEIGHT, outer_weights[0]);
        ctx.set_data_fwd(OUTER_RAGDOLL_WEIGHT, outer_weights[1]);
        ctx.set_data_fwd(INNER_SNAPSHOT_WEIGHT, inner_weights[0]);
        ctx.set_data_fwd(INNER_FLAIL_WEIGHT, inner_weights[1]);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(ACTIVE_INDEX, DataSpec::F32)
            .add_input_data(INPUT_POSE, DataSpec::Pose)
            .add_input_time(INPUT_TIME)
            .add_input_data(SNAPSHOT_POSE, DataSpec::Pose)
            .add_input_time(SNAPSHOT_TIME)
            .add_input_data(FLAIL_POSE, DataSpec::Pose)
            .add_input_time(FLAIL_TIME)
            .add_output_data(OUT_POSE, DataSpec::Pose)
            .add_output_time()
            .add_output_data(OUTER_INPUT_WEIGHT, DataSpec::F32)
            .add_output_data(OUTER_RAGDOLL_WEIGHT, DataSpec::F32)
            .add_output_data(INNER_SNAPSHOT_WEIGHT, DataSpec::F32)
            .add_output_data(INNER_FLAIL_WEIGHT, DataSpec::F32);
        for curve in &self.curve_names {
            ctx.add_input_data(curve_value_input(0, curve), DataSpec::F32)
                .add_output_data(curve.clone(), DataSpec::F32);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<RagdollingTagBlendNode>();
        let registration = registry
            .get(std::any::TypeId::of::<RagdollingTagBlendNode>())
            .unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }

    #[test]
    fn nested_weights_keep_inactive_inner_frozen() {
        let mut state = RagdollingTagBlendState::default();
        let outer = advance_weights(
            &mut state.outer,
            0,
            [0.4, 0.5],
            GameplayTagBlendMode::Linear,
            0.0,
        );
        assert_eq!(outer, vec![1.0, 0.0]);
        let outer = advance_weights(
            &mut state.outer,
            1,
            [0.4, 0.5],
            GameplayTagBlendMode::Linear,
            0.25,
        );
        assert_eq!(outer, vec![0.5, 0.5]);
        let inner = advance_weights(
            &mut state.inner,
            1,
            [0.0, 0.0],
            GameplayTagBlendMode::Linear,
            0.25,
        );
        assert_eq!(inner, vec![0.0, 1.0]);
    }

    #[test]
    fn empty_blend_preserves_the_input_pose() {
        let fallback = Pose {
            timestamp: 0.75,
            ..default()
        };
        assert_eq!(blend_poses(Vec::new(), fallback.clone()), fallback);
    }

    #[test]
    fn blend_uses_the_input_pose_skeleton() {
        let mut skeletons = Assets::<bevy_animation_graph::core::skeleton::Skeleton>::default();
        let mut fallback = Pose::default();
        fallback.skeleton =
            skeletons.add(bevy_animation_graph::core::skeleton::Skeleton::default());
        let output = blend_poses(vec![(Pose::default(), 1.0)], fallback.clone());
        assert_eq!(output.skeleton, fallback.skeleton);
    }
}
