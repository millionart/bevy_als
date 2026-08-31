use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::{Pose, RootMotionDelta},
    utils::delaunay::Triangulation,
};
use serde::{Deserialize, Serialize};

use crate::sync_group_phase::normalize_phase;
use crate::{SyncMarker, follower_sample_time, marker_phase_sample_time, phase_to_time};

const EPSILON: f32 = 1e-6;

pub const IN_POSITION: &str = "position";
pub const IN_DELTA_SECONDS: &str = "delta_seconds";
pub const IN_PHASE: &str = "phase";
pub const OUT_VALUE: &str = "value";
pub const OUT_POSE: &str = "pose";
pub const OUT_SAMPLE_TIME: &str = "sample_time";

pub fn blend_normalized_root_motion(
    root_motions: &[(Option<RootMotionDelta>, f32)],
) -> Option<RootMotionDelta> {
    let mut translation = Vec3::ZERO;
    let mut rotation = None;
    for (root_motion, weight) in root_motions {
        let Some(root_motion) = root_motion else {
            continue;
        };
        let weighted_rotation = root_motion.rotation * *weight;
        rotation = Some(match rotation {
            Some(accumulated) if weighted_rotation.dot(accumulated) < 0.0 => {
                accumulated - weighted_rotation
            }
            Some(accumulated) => accumulated + weighted_rotation,
            None => weighted_rotation,
        });
        translation += root_motion.translation * *weight;
    }
    rotation.map(|rotation| RootMotionDelta {
        translation,
        rotation: if rotation.length_squared() > EPSILON {
            rotation.normalize()
        } else {
            Quat::IDENTITY
        },
    })
}

#[derive(Reflect, Clone, Debug, Default)]
struct CubicFilterSample {
    value: Vec2,
    time: f32,
}

#[derive(Reflect, Clone, Debug, Default)]
struct CubicFilterState {
    initialized: bool,
    time: f32,
    value: Vec2,
    samples: Vec<CubicFilterSample>,
}

fn cubic_filter_sample(
    state: &mut CubicFilterState,
    target: Vec2,
    delta_seconds: f32,
    window_seconds: f32,
) -> Vec2 {
    if !state.initialized {
        state.initialized = true;
        state.value = target;
        state.samples = vec![CubicFilterSample {
            value: target,
            time: state.time,
        }];
        return target;
    }
    if window_seconds <= EPSILON {
        state.time += delta_seconds.max(0.0);
        state.value = target;
        state.samples = vec![CubicFilterSample {
            value: target,
            time: state.time,
        }];
        return target;
    }
    if delta_seconds <= EPSILON {
        return state.value;
    }
    state.time += delta_seconds;
    state.samples.push(CubicFilterSample {
        value: target,
        time: state.time,
    });
    state
        .samples
        .retain(|sample| state.time - sample.time <= window_seconds + EPSILON);
    let (sum, total) = state
        .samples
        .iter()
        .fold((Vec2::ZERO, 0.0_f32), |(sum, total), sample| {
            let age = ((state.time - sample.time) / window_seconds).clamp(0.0, 1.0);
            let weight = 1.0 - age * age * age;
            (sum + sample.value * weight, total + weight)
        });
    state.value = if total > EPSILON { sum / total } else { target };
    state.value
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct CubicVec2FilterNode {
    pub window_seconds: f32,
}

impl NodeLike for CubicVec2FilterNode {
    fn display_name(&self) -> String {
        "ALS Cubic BlendSpace Filter".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let position = ctx.data_back(IN_POSITION)?.as_vec2()?;
        let delta_seconds = ctx.data_back(IN_DELTA_SECONDS)?.as_f32()?;
        let value = cubic_filter_sample(
            ctx.state_mut::<CubicFilterState>()?,
            position,
            delta_seconds,
            self.window_seconds,
        );
        ctx.set_data_fwd(OUT_VALUE, value);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_POSITION, DataSpec::Vec2)
            .add_input_data(IN_DELTA_SECONDS, DataSpec::F32)
            .add_output_data(OUT_VALUE, DataSpec::Vec2);
        Ok(())
    }
}

#[derive(Reflect, Clone, Debug, Default)]
struct CubicVec2AxisFilterState {
    x: CubicFilterState,
    y: CubicFilterState,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct CubicVec2AxisFilterNode {
    pub x_window_seconds: f32,
    pub y_window_seconds: f32,
}

impl NodeLike for CubicVec2AxisFilterNode {
    fn display_name(&self) -> String {
        "ALS Cubic BlendSpace Axis Filter".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let position = ctx.data_back(IN_POSITION)?.as_vec2()?;
        let delta_seconds = ctx.data_back(IN_DELTA_SECONDS)?.as_f32()?;
        let state = ctx.state_mut::<CubicVec2AxisFilterState>()?;
        let x = cubic_filter_sample(
            &mut state.x,
            Vec2::new(position.x, 0.0),
            delta_seconds,
            self.x_window_seconds,
        )
        .x;
        let y = cubic_filter_sample(
            &mut state.y,
            Vec2::new(0.0, position.y),
            delta_seconds,
            self.y_window_seconds,
        )
        .y;
        ctx.set_data_fwd(OUT_VALUE, Vec2::new(x, y));
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_POSITION, DataSpec::Vec2)
            .add_input_data(IN_DELTA_SECONDS, DataSpec::F32)
            .add_output_data(OUT_VALUE, DataSpec::Vec2);
        Ok(())
    }
}

#[derive(Reflect, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[reflect(Default, Serialize, Deserialize)]
pub struct NormalizedBlendSpacePoint {
    pub id: String,
    pub point: Vec2,
    pub duration: f32,
    pub markers: Vec<SyncMarker>,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct NormalizedBlendSpaceNode {
    pub looped: bool,
    pub phase_duration: f32,
    pub phase_markers: Vec<SyncMarker>,
    pub points: Vec<NormalizedBlendSpacePoint>,
}

impl NormalizedBlendSpaceNode {
    pub fn pose_pin(id: &str) -> String {
        format!("pose_{id}")
    }

    pub fn time_pin(id: &str) -> String {
        format!("time_{id}")
    }

    fn uses_marker_phase(&self) -> bool {
        self.phase_duration > EPSILON && self.phase_markers.len() >= 2
    }

    fn point_duration(
        &self,
        ctx: &mut NodeContext,
        point: &NormalizedBlendSpacePoint,
    ) -> Result<f32, GraphError> {
        if point.duration > EPSILON {
            return Ok(point.duration);
        }
        Ok(ctx
            .duration_back(Self::time_pin(&point.id))?
            .unwrap_or(EPSILON)
            .max(EPSILON))
    }

    fn weights(&self, position: Vec2) -> Vec<(usize, f32)> {
        if self.points.is_empty() {
            return Vec::new();
        }
        if self.points.len() == 1 {
            return vec![(0, 1.0)];
        }
        if self.points.len() == 2 {
            let start = self.points[0].point;
            let end = self.points[1].point;
            let direction = end - start;
            let length_squared = direction.length_squared();
            let alpha = if length_squared <= EPSILON {
                0.0
            } else {
                (position - start).dot(direction) / length_squared
            }
            .clamp(0.0, 1.0);
            return vec![(0, 1.0 - alpha), (1, alpha)];
        }
        let triangulation = Triangulation::from_points_delaunay(
            self.points.iter().map(|point| point.point).collect(),
        );
        let mut weights = triangulation
            .find_linear_combination(position)
            .into_iter()
            .map(|(vertex, weight)| (vertex.id.index(), weight.max(0.0)))
            .filter(|(_, weight)| *weight > EPSILON)
            .collect::<Vec<_>>();
        let total = weights.iter().map(|(_, weight)| *weight).sum::<f32>();
        if total > EPSILON {
            for (_, weight) in &mut weights {
                *weight /= total;
            }
        }
        weights
    }
}

impl NodeLike for NormalizedBlendSpaceNode {
    fn display_name(&self) -> String {
        "ALS Normalized BlendSpace".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let position = ctx.data_back(IN_POSITION)?.as_vec2()?;
        let Some((index, _)) = self
            .weights(position)
            .into_iter()
            .max_by(|left, right| left.1.total_cmp(&right.1))
        else {
            ctx.set_duration_fwd(None);
            return Ok(());
        };
        let duration = self.point_duration(&mut ctx, &self.points[index])?;
        ctx.set_duration_fwd(Some(duration));
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let graph_time = ctx
            .time_update_fwd()?
            .partial_update_basic(ctx.prev_time())
            .unwrap_or_else(|| ctx.prev_time());
        let position = ctx.data_back(IN_POSITION)?.as_vec2()?;
        let phase = normalize_phase(ctx.data_back(IN_PHASE)?.as_f32()?, self.looped);
        let weights = self.weights(position);
        let Some((leader_index, _)) = weights
            .iter()
            .copied()
            .max_by(|left, right| left.1.total_cmp(&right.1))
        else {
            ctx.set_data_fwd(OUT_POSE, Pose::default());
            return Ok(());
        };
        let leader = &self.points[leader_index];
        let leader_duration = self.point_duration(&mut ctx, leader)?;
        let leader_time = if self.uses_marker_phase() {
            marker_phase_sample_time(
                phase,
                self.phase_duration,
                &self.phase_markers,
                leader_duration,
                &leader.markers,
                self.looped,
            )
        } else {
            phase_to_time(phase, leader_duration, self.looped)
        };
        let output_time = leader_time;
        let mut poses = Vec::with_capacity(weights.len());
        for (index, weight) in weights {
            let point = &self.points[index];
            let duration = self.point_duration(&mut ctx, point)?;
            let time = if self.uses_marker_phase() {
                marker_phase_sample_time(
                    phase,
                    self.phase_duration,
                    &self.phase_markers,
                    duration,
                    &point.markers,
                    self.looped,
                )
            } else if index == leader_index {
                leader_time
            } else {
                follower_sample_time(
                    phase,
                    leader_duration,
                    &leader.markers,
                    duration,
                    &point.markers,
                    self.looped,
                )
            };
            ctx.set_time_update_back(Self::time_pin(&point.id), TimeUpdate::Absolute(time));
            poses.push((
                ctx.data_back(Self::pose_pin(&point.id))?.into_pose()?,
                weight,
            ));
        }
        let root_motion = blend_normalized_root_motion(
            &poses
                .iter()
                .map(|(pose, weight)| (pose.root_motion.clone(), *weight))
                .collect::<Vec<_>>(),
        );
        let mut output = poses
            .iter()
            .map(|(pose, weight)| pose.scalar_mult(*weight))
            .reduce(|accumulator, pose| accumulator.linear_add(&pose))
            .unwrap_or_default()
            .normalize_quat();
        output.root_motion = root_motion;
        output.timestamp = graph_time;
        ctx.set_time(graph_time);
        ctx.set_data_fwd(OUT_POSE, output);
        ctx.set_data_fwd(OUT_SAMPLE_TIME, output_time);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_POSITION, DataSpec::Vec2)
            .add_input_data(IN_PHASE, DataSpec::F32);
        for point in &self.points {
            ctx.add_input_data(Self::pose_pin(&point.id), DataSpec::Pose)
                .add_input_time(Self::time_pin(&point.id));
        }
        ctx.add_output_data(OUT_POSE, DataSpec::Pose)
            .add_output_data(OUT_SAMPLE_TIME, DataSpec::F32)
            .add_output_time();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn markers(entries: &[(&str, f32)]) -> Vec<SyncMarker> {
        entries
            .iter()
            .map(|(name, time)| SyncMarker {
                name: (*name).to_owned(),
                time: *time,
            })
            .collect()
    }

    #[test]
    fn cubic_filter_matches_the_source_fir_weight_curve() {
        let mut state = CubicFilterState::default();
        assert_eq!(
            cubic_filter_sample(&mut state, Vec2::ZERO, 0.0, 0.4),
            Vec2::ZERO
        );
        let filtered = cubic_filter_sample(&mut state, Vec2::ONE, 0.2, 0.4);
        assert!((filtered.x - (1.0 / 1.875)).abs() < 1e-5);
        assert!((filtered.y - (1.0 / 1.875)).abs() < 1e-5);
    }

    #[test]
    fn cubic_axis_filter_uses_independent_windows() {
        let mut x = CubicFilterState::default();
        let mut y = CubicFilterState::default();
        assert_eq!(
            cubic_filter_sample(&mut x, Vec2::ZERO, 0.0, 0.2),
            Vec2::ZERO
        );
        assert_eq!(
            cubic_filter_sample(&mut y, Vec2::ZERO, 0.0, 0.4),
            Vec2::ZERO
        );
        let x_value = cubic_filter_sample(&mut x, Vec2::X, 0.2, 0.2).x;
        let y_value = cubic_filter_sample(&mut y, Vec2::Y, 0.2, 0.4).y;
        assert!((x_value - 0.5).abs() < 1e-5);
        assert!((y_value - (1.0 / 1.875)).abs() < 1e-5);
    }

    #[test]
    fn normalized_blendspace_uses_marker_phase_for_different_lengths() {
        let node = NormalizedBlendSpaceNode {
            looped: true,
            points: vec![
                NormalizedBlendSpacePoint {
                    id: "walk".to_owned(),
                    point: Vec2::new(1.0, 0.0),
                    duration: 1.133333,
                    markers: markers(&[("Left", 0.083333), ("Right", 0.65)]),
                },
                NormalizedBlendSpacePoint {
                    id: "run".to_owned(),
                    point: Vec2::new(1.0, 1.0),
                    duration: 0.8,
                    markers: markers(&[("Left", 0.033333), ("Right", 0.433333)]),
                },
            ],
            ..default()
        };
        assert_eq!(node.weights(Vec2::new(1.0, 0.0)), vec![(0, 1.0), (1, 0.0)]);
        let leader_time = phase_to_time(0.2, 1.133333, true);
        let follower_time = follower_sample_time(
            0.2,
            1.133333,
            &node.points[0].markers,
            0.8,
            &node.points[1].markers,
            true,
        );
        assert!((leader_time - 0.2266666).abs() < 1e-5);
        assert!((follower_time - 0.13451).abs() < 1e-3);
    }

    #[test]
    fn marker_phase_maps_each_direction_to_the_same_foot_interval() {
        let node = NormalizedBlendSpaceNode {
            looped: true,
            phase_duration: 1.0,
            phase_markers: markers(&[("Left", 0.0), ("Right", 0.5)]),
            points: vec![
                NormalizedBlendSpacePoint {
                    id: "forward".to_owned(),
                    point: Vec2::ZERO,
                    duration: 1.133333,
                    markers: markers(&[("Left", 0.083333), ("Right", 0.65)]),
                },
                NormalizedBlendSpacePoint {
                    id: "backward".to_owned(),
                    point: Vec2::ONE,
                    duration: 1.133333,
                    markers: markers(&[("Right", 0.516667), ("Left", 1.083333)]),
                },
            ],
        };
        let phase = 0.1264706;
        let forward_time = marker_phase_sample_time(
            phase,
            node.phase_duration,
            &node.phase_markers,
            1.133333,
            &node.points[0].markers,
            true,
        );
        let backward_time = marker_phase_sample_time(
            phase,
            node.phase_duration,
            &node.phase_markers,
            1.133333,
            &node.points[1].markers,
            true,
        );
        assert!((forward_time - 0.2266666).abs() < 1e-4);
        let forward_position =
            crate::marker_position_at_time(&node.points[0].markers, 1.133333, forward_time, true)
                .unwrap();
        let backward_position =
            crate::marker_position_at_time(&node.points[1].markers, 1.133333, backward_time, true)
                .unwrap();
        assert_eq!(forward_position.previous, backward_position.previous);
        assert_eq!(forward_position.next, backward_position.next);
        assert!(
            (forward_position.alpha - backward_position.alpha).abs() < EPSILON,
            "forward={forward_position:?} backward={backward_position:?}"
        );
    }

    #[test]
    fn normalized_root_motion_blend_preserves_identical_rotation() {
        let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let delta = RootMotionDelta {
            translation: Vec3::new(2.0, 0.0, 0.0),
            rotation,
        };
        let blended =
            blend_normalized_root_motion(&[(Some(delta.clone()), 0.5), (Some(delta), 0.5)])
                .unwrap();
        assert!((blended.rotation.dot(rotation).abs() - 1.0).abs() < 1e-6);
        assert!((blended.translation - Vec3::new(2.0, 0.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn nodes_register_node_like_dispatch_keys() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<CubicVec2FilterNode>();
        registry.register::<NormalizedBlendSpaceNode>();
        for type_id in [
            std::any::TypeId::of::<CubicVec2FilterNode>(),
            std::any::TypeId::of::<NormalizedBlendSpaceNode>(),
        ] {
            assert!(
                registry
                    .get(type_id)
                    .unwrap()
                    .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                    .is_some()
            );
        }
    }
}
