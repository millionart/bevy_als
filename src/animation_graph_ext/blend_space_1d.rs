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

pub fn pose_input(id: &str) -> String {
    format!("pose_{id}")
}

pub fn time_input(id: &str) -> String {
    format!("time_{id}")
}

#[derive(Reflect, Clone, Debug, Serialize, Deserialize)]
pub struct BlendSpace1DPoint {
    pub id: String,
    pub position: f32,
}

#[derive(Reflect, Clone, Debug, Default, Serialize, Deserialize)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct BlendSpace1DNode {
    pub points: Vec<BlendSpace1DPoint>,
}

impl BlendSpace1DNode {
    pub const POSITION: &'static str = "position";
    pub const OUT_POSE: &'static str = "pose";

    fn sorted_points(&self) -> Vec<&BlendSpace1DPoint> {
        let mut points = self.points.iter().collect::<Vec<_>>();
        points.sort_by(|left, right| left.position.total_cmp(&right.position));
        points
    }
}

pub fn blend_weights(points: &[BlendSpace1DPoint], position: f32) -> Option<(usize, usize, f32)> {
    if points.is_empty() {
        return None;
    }
    if points.len() == 1 {
        return Some((0, 0, 0.0));
    }
    let mut sorted = points.iter().enumerate().collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.1.position.total_cmp(&right.1.position));
    if position <= sorted[0].1.position {
        return Some((sorted[0].0, sorted[0].0, 0.0));
    }
    if position >= sorted[sorted.len() - 1].1.position {
        let index = sorted[sorted.len() - 1].0;
        return Some((index, index, 0.0));
    }
    for pair in sorted.windows(2) {
        let (left_index, left) = pair[0];
        let (right_index, right) = pair[1];
        if position <= right.position {
            let extent = right.position - left.position;
            let factor = if extent.abs() <= f32::EPSILON {
                0.0
            } else {
                ((position - left.position) / extent).clamp(0.0, 1.0)
            };
            return Some((left_index, right_index, factor));
        }
    }
    None
}

impl NodeLike for BlendSpace1DNode {
    fn display_name(&self) -> String {
        "ALS BlendSpace 1D".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let position = ctx.data_back(Self::POSITION)?.as_f32()?;
        let Some((left, right, factor)) = blend_weights(&self.points, position) else {
            ctx.set_duration_fwd(None);
            return Ok(());
        };
        let index = if factor <= 0.5 { left } else { right };
        ctx.set_duration_fwd(ctx.duration_back(time_input(&self.points[index].id))?);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let update = ctx.time_update_fwd()?;
        let position = ctx.data_back(Self::POSITION)?.as_f32()?;
        let Some((left, right, factor)) = blend_weights(&self.points, position) else {
            return Err(GraphError::UnknownErrorNode {
                graph: ctx.graph_context.context_id,
                node: ctx.node_id,
                error: "blend space has no selected samples".to_owned(),
            });
        };
        let left_point = &self.points[left];
        let right_point = &self.points[right];
        ctx.set_time_update_back(time_input(&left_point.id), update.clone());
        let mut output = ctx.data_back(pose_input(&left_point.id))?.into_pose()?;
        if left != right {
            ctx.set_time_update_back(
                time_input(&right_point.id),
                TimeUpdate::Absolute(output.timestamp),
            );
            let right_pose = ctx.data_back(pose_input(&right_point.id))?.into_pose()?;
            LinearInterpolator {
                bone_mask: BoneMask::all(),
            }
            .interpolate_pose(&mut output, &right_pose, factor);
        }
        ctx.set_time(output.timestamp);
        ctx.set_data_fwd(Self::OUT_POSE, output);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(Self::POSITION, DataSpec::F32);
        for point in self.sorted_points() {
            ctx.add_input_data(pose_input(&point.id), DataSpec::Pose)
                .add_input_time(time_input(&point.id));
        }
        ctx.add_output_data(Self::OUT_POSE, DataSpec::Pose)
            .add_output_time();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points() -> Vec<BlendSpace1DPoint> {
        vec![
            BlendSpace1DPoint {
                id: "down".to_owned(),
                position: -90.0,
            },
            BlendSpace1DPoint {
                id: "forward".to_owned(),
                position: 0.0,
            },
            BlendSpace1DPoint {
                id: "up".to_owned(),
                position: 90.0,
            },
        ]
    }

    #[test]
    fn one_dimensional_weights_match_the_source_pitch_axis() {
        assert_eq!(blend_weights(&points(), -100.0), Some((0, 0, 0.0)));
        assert_eq!(blend_weights(&points(), 100.0), Some((2, 2, 0.0)));
        assert_eq!(blend_weights(&points(), 0.0), Some((0, 1, 1.0)));
        assert_eq!(blend_weights(&points(), 45.0), Some((1, 2, 0.5)));
    }

    #[test]
    fn pin_names_are_stable() {
        assert_eq!(pose_input("forward"), "pose_forward");
        assert_eq!(time_input("up"), "time_up");
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<BlendSpace1DNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<BlendSpace1DNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
