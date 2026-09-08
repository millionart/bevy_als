use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, bone_mask::BoneMask},
    errors::GraphError,
    interpolation::linear::LinearInterpolator,
    pose::Pose,
};

use crate::animation_context::{
    AnimationContextState, pose_input_context_state, select_relevant_asset,
};

const WEIGHT_THRESHOLD: f32 = 1e-6;

pub fn pose_input(index: usize) -> String {
    format!("pose_{index}")
}

pub fn time_input(index: usize) -> String {
    format!("time_{index}")
}

pub fn weight_input(index: usize) -> String {
    format!("weight_{index}")
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct MultiWayBlendNode {
    pub pose_count: usize,
    pub normalize_alpha: bool,
}

impl Default for MultiWayBlendNode {
    fn default() -> Self {
        Self {
            pose_count: 2,
            normalize_alpha: true,
        }
    }
}

impl NodeLike for MultiWayBlendNode {
    fn display_name(&self) -> String {
        "ALS Multi Way Blend".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if self.pose_count == 0 {
            ctx.set_duration_fwd(None);
        } else {
            ctx.set_duration_fwd(ctx.duration_back(time_input(0))?);
        }
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        let mut active = Vec::with_capacity(self.pose_count);
        let mut total_weight = 0.0;
        for index in 0..self.pose_count {
            let weight = ctx.data_back(weight_input(index))?.as_f32()?;
            if weight > WEIGHT_THRESHOLD {
                total_weight += weight;
                active.push((index, weight));
            }
        }

        for (index, _) in &active {
            ctx.set_time_update_back(time_input(*index), update.clone());
        }

        let mut accumulated_weight = 0.0;
        let mut output: Option<Pose> = None;
        for (index, raw_weight) in &active {
            let weight = if self.normalize_alpha {
                *raw_weight / total_weight
            } else {
                *raw_weight
            };
            let pose = ctx.data_back(pose_input(*index))?.into_pose()?;
            if let Some(base) = output.as_mut() {
                let factor = weight / (accumulated_weight + weight);
                LinearInterpolator {
                    bone_mask: BoneMask::all(),
                }
                .interpolate_pose(base, &pose, factor);
            } else {
                output = Some(pose);
            }
            accumulated_weight += weight;
        }

        let output = output.unwrap_or_default();
        ctx.set_time(output.timestamp);
        ctx.set_data_fwd("pose", output);
        let mut context = active
            .first()
            .and_then(|(index, _)| pose_input_context_state(&ctx, &pose_input(*index)))
            .unwrap_or_default();
        context.relevant_asset = select_relevant_asset(active.iter().map(|(index, weight)| {
            (
                pose_input_context_state(&ctx, &pose_input(*index))
                    .and_then(|context| context.relevant_asset),
                *weight,
            )
        }));
        *ctx.state_mut_or_else(AnimationContextState::default)? = context;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        for index in 0..self.pose_count {
            ctx.add_input_data(pose_input(index), DataSpec::Pose)
                .add_input_time(time_input(index))
                .add_input_data(weight_input(index), DataSpec::F32);
        }
        ctx.add_output_data("pose", DataSpec::Pose)
            .add_output_time();
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
        registry.register::<MultiWayBlendNode>();
        let registration = registry
            .get(std::any::TypeId::of::<MultiWayBlendNode>())
            .unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <MultiWayBlendNode as bevy::reflect::TypePath>::type_path(),
            "als_ext::MultiWayBlendNode"
        );
    }

    #[test]
    fn pin_names_are_stable() {
        assert_eq!(pose_input(2), "pose_2");
        assert_eq!(time_input(2), "time_2");
        assert_eq!(weight_input(2), "weight_2");
    }
}
