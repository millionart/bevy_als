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

use crate::extensions::global_f32_input::global_f32_or_root_default;
use crate::extensions::sync_group_phase::{
    SourceSyncPlayerBinding, SourceSyncPlayerTick, resolved_source_sync_phase,
    resolved_source_sync_markers, marker_phase_sample_time_near, follower_sample_time_near,
    marker_phase_sample_time_continuous, resolved_source_marker_advance, resolved_source_sync_sample_time,
};
use crate::extensions::{AnimationContextState, marker_phase_from_sample_time};
use crate::extensions::{SyncMarker, follower_sample_time, marker_phase_sample_time, phase_to_time};
use crate::extensions::{animation_context::context_time_update, sync_group_phase::normalize_phase};

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

#[derive(Reflect, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[reflect(Default, Serialize, Deserialize)]
pub struct NormalizedBlendSpacePoint {
    pub id: String,
    pub point: Vec2,
    pub duration: f32,
    #[serde(default = "default_asset_rate_scale")]
    pub asset_rate_scale: f32,
    pub markers: Vec<SyncMarker>,
    #[serde(default)]
    pub curves: crate::extensions::als_curves::AlsPoseCurves,
}

fn default_asset_rate_scale() -> f32 { 1.0 }

impl Default for NormalizedBlendSpacePoint {
    fn default() -> Self {
        Self { id: default(), point: default(), duration: 0.0, asset_rate_scale: default_asset_rate_scale(),
            markers: default(), curves: default() }
    }
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct SourceBlendSpaceTriangulation {
    pub axis_min: Vec2,
    pub axis_max: Vec2,
    pub triangles: Vec<[usize; 3]>,
    pub segments: Vec<[usize; 2]>,
}

#[derive(Reflect, Clone, Debug, Default, PartialEq)]
#[reflect(Default)]
pub struct SourcePerBoneInterpolation {
    pub bone: String,
    pub speed: f32,
}

#[derive(Reflect, Clone, Debug, Default)]
struct SourceBlendSpaceSampleWeight {
    index: usize,
    weight: f32,
    bone_weights: Vec<f32>,
    bone_rates: Vec<f32>,
}

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct NormalizedBlendSpaceState {
    pub(crate) context: AnimationContextState,
    triangle: Option<usize>,
    samples: Vec<SourceBlendSpaceSampleWeight>,
    sample_times: Vec<Option<f32>>,
    marker_records: Vec<Option<crate::extensions::sync_group_phase::SourceMarkerRecord>>,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct NormalizedBlendSpaceNode {
    pub looped: bool,
    pub teleport_to_normalized_time: bool,
    pub phase_duration: f32,
    pub phase_markers: Vec<SyncMarker>,
    pub points: Vec<NormalizedBlendSpacePoint>,
    pub playback: Option<SourceSyncPlayerBinding>,
    pub source_triangulation: Option<SourceBlendSpaceTriangulation>,
    pub per_bone_interpolation: Vec<SourcePerBoneInterpolation>,
    pub per_bone_interpolation_ease_in_out: bool,
}

impl NormalizedBlendSpaceNode {
    fn smooth_bone_weight(&self, value: f32, rate: &mut f32, target: f32, delta: f32, speed: f32) -> f32 {
        if speed <= 0.0 { return target; }
        if !self.per_bone_interpolation_ease_in_out {
            return value + (target - value).clamp(-delta * speed, delta * speed);
        }
        let time = 1.0 / (std::f32::consts::E * speed);
        if time > 1e-4 {
            let w = 2.0 / time;
            let a = w * delta;
            let exp = 1.0 / (1.0 + 1.00746054 * a + 0.45053901 * a * a + 0.25724632 * a * a * a);
            let offset = value - target;
            let step = (*rate + w * offset) * delta;
            *rate = (*rate - w * step) * exp;
            target + (offset + step) * exp
        } else {
            if delta > 0.0 { *rate = (target - value) / delta; }
            target
        }
    }

    fn smoothed_sample(&self, old: Option<&SourceBlendSpaceSampleWeight>, index: usize, target: f32, delta: f32) -> SourceBlendSpaceSampleWeight {
        let previous = old.filter(|sample| sample.bone_weights.len() == self.per_bone_interpolation.len());
        let old_weight = old.map_or(0.0, |sample| sample.weight);
        let mut bone_weights = Vec::with_capacity(self.per_bone_interpolation.len());
        let mut bone_rates = Vec::with_capacity(self.per_bone_interpolation.len());
        for (bone, interpolation) in self.per_bone_interpolation.iter().enumerate() {
            let value = previous.map_or(old_weight, |sample| sample.bone_weights[bone]);
            let mut rate = previous.map_or(0.0, |sample| sample.bone_rates[bone]);
            bone_weights.push(self.smooth_bone_weight(value, &mut rate, target, delta, interpolation.speed));
            bone_rates.push(rate);
        }
        SourceBlendSpaceSampleWeight { index, weight: target, bone_weights, bone_rates }
    }

    fn smooth_samples(&self, previous: &[SourceBlendSpaceSampleWeight], target: &[(usize, f32)], delta: f32) -> Vec<SourceBlendSpaceSampleWeight> {
        let previous = if previous.iter().any(|sample| sample.index >= self.points.len()) { &[][..] } else { previous };
        let mut samples = Vec::new();
        for old in previous {
            let desired = target.iter().find(|(index, _)| *index == old.index).map(|(_, weight)| *weight);
            let sample = self.smoothed_sample(Some(old), old.index, desired.unwrap_or(0.0), delta);
            if sample.weight > 1e-5 || sample.bone_weights.iter().sum::<f32>() > 1e-5 {
                samples.push(sample);
            } else if desired.is_some() {
                let sample = self.smoothed_sample(Some(old), old.index, 0.0, delta);
                if sample.weight > 1e-5 || sample.bone_weights.iter().sum::<f32>() > 1e-5 { samples.push(sample); }
            }
        }
        for &(index, weight) in target {
            if samples.iter().any(|sample| sample.index == index) { continue; }
            let sample = self.smoothed_sample(None, index, weight, delta);
            if sample.weight > 1e-5 || sample.bone_weights.iter().sum::<f32>() > 1e-5 { samples.push(sample); }
        }
        for bone in 0..self.per_bone_interpolation.len() {
            let sum = samples.iter().map(|sample| sample.bone_weights[bone]).sum::<f32>();
            let count = samples.len() as f32;
            for sample in &mut samples {
                if sum <= 1e-5 { sample.bone_weights[bone] = 1.0 / count; }
                else if (sum - 1.0).abs() > 1e-5 { sample.bone_weights[bone] /= sum; }
            }
        }
        samples
    }

    fn source_weights(
        &self,
        position: Vec2,
        cached: &mut Option<usize>,
    ) -> Result<Vec<(usize, f32)>, String> {
        let Some(source) = &self.source_triangulation else {
            return Ok(self.weights(position));
        };
        let size = (source.axis_max - source.axis_min).as_dvec2();
        if !position.is_finite()
            || !source.axis_min.is_finite()
            || !size.is_finite()
            || size.cmple(bevy::math::DVec2::ZERO).any()
            || (source.triangles.is_empty() && source.segments.is_empty())
            || (!source.triangles.is_empty() && !source.segments.is_empty())
            || self.points.iter().any(|point| !point.point.is_finite())
            || source
                .triangles
                .iter()
                .flatten()
                .any(|index| *index >= self.points.len())
            || source
                .segments
                .iter()
                .flatten()
                .any(|index| *index >= self.points.len())
        {
            return Err("source BlendSpace triangulation is invalid".to_owned());
        }
        let minimum = source.axis_min.as_dvec2();
        let position = ((position.as_dvec2() - minimum) / size)
            .clamp(bevy::math::DVec2::ZERO, bevy::math::DVec2::ONE);
        let vertices = self
            .points
            .iter()
            .map(|point| (point.point.as_dvec2() - minimum) / size)
            .collect::<Vec<_>>();
        if !source.segments.is_empty() {
            if source.segments.len() == 1 && source.segments[0][0] == source.segments[0][1] {
                return Ok(vec![(source.segments[0][0], 1.0)]);
            }
            if source
                .segments
                .iter()
                .any(|[a, b]| vertices[*a].x >= vertices[*b].x)
                || source
                    .segments
                    .windows(2)
                    .any(|pair| pair[0][1] != pair[1][0])
            {
                return Err("source BlendSpace segments are invalid".to_owned());
            }
            let segment = source
                .segments
                .iter()
                .position(|[_, b]| position.x <= vertices[*b].x)
                .unwrap_or(source.segments.len() - 1);
            let [a, b] = source.segments[segment];
            let alpha = ((position.x as f32 - vertices[a].x as f32)
                / (vertices[b].x as f32 - vertices[a].x as f32))
                .clamp(0.0, 1.0);
            let mut weights = vec![(a, 1.0 - alpha), (b, alpha)];
            if weights[0].1 < weights[1].1 {
                weights.swap(0, 1);
            }
            weights.retain(|(_, weight)| *weight >= 1e-5);
            let total = weights.iter().map(|(_, weight)| *weight).sum::<f32>();
            for (_, weight) in &mut weights {
                *weight /= total;
            }
            return Ok(weights);
        }
        if source.triangles.len() == 1 {
            let [a, b, c] = source.triangles[0];
            if a == b {
                return Ok(vec![(a, 1.0)]);
            }
            if b == c {
                let delta = vertices[b] - vertices[a];
                if delta.length_squared() <= f64::EPSILON {
                    return Err("source BlendSpace segment is invalid".to_owned());
                }
                let alpha = ((position - vertices[a]).dot(delta) / delta.length_squared())
                    .clamp(0.0, 1.0) as f32;
                let mut weights = vec![(a, 1.0 - alpha), (b, alpha)];
                weights.retain(|(_, weight)| *weight > 1e-5);
                if weights.len() == 2 && weights[0].1 <= weights[1].1 {
                    weights.swap(0, 1);
                }
                let total = weights.iter().map(|(_, weight)| *weight).sum::<f32>();
                for (_, weight) in &mut weights {
                    *weight /= total;
                }
                return Ok(weights);
            }
        }
        if source.triangles.iter().any(|indices| {
            (vertices[indices[1]] - vertices[indices[0]])
                .perp_dot(vertices[indices[2]] - vertices[indices[0]])
                <= f64::EPSILON
        }) {
            return Err("source BlendSpace triangle orientation is invalid".to_owned());
        }
        let mut triangle = cached
            .filter(|index| *index < source.triangles.len())
            .unwrap_or(source.triangles.len() / 2);
        let mut weights = Vec::new();
        for _ in 0..source.triangles.len() {
            let indices = source.triangles[triangle];
            let points = indices.map(|index| vertices[index]);
            let mut edge = None;
            let mut distance = 1e-4;
            for index in 0..3 {
                let delta = points[(index + 1) % 3] - points[index];
                let normal = bevy::math::DVec2::new(delta.y, -delta.x).normalize();
                let value = (position - points[index]).dot(normal);
                if value > distance {
                    edge = Some(index);
                    distance = value;
                }
            }
            let Some(edge) = edge else {
                let first = points[1] - points[0];
                let second = points[2] - points[0];
                let offset = position - points[0];
                let area = first.perp_dot(second);
                let y = offset.perp_dot(second) / area;
                let z = first.perp_dot(offset) / area;
                weights = indices
                    .into_iter()
                    .zip([(1.0 - y - z) as f32, y as f32, z as f32])
                    .collect();
                break;
            };
            let a = indices[edge];
            let b = indices[(edge + 1) % 3];
            if let Some(next) = source
                .triangles
                .iter()
                .enumerate()
                .find_map(|(index, vertices)| {
                    (index != triangle && vertices.contains(&a) && vertices.contains(&b))
                        .then_some(index)
                })
            {
                triangle = next;
                continue;
            }
            let mut closest = f64::INFINITY;
            for (index, indices) in source.triangles.iter().enumerate() {
                for edge in 0..3 {
                    let a = indices[edge];
                    let b = indices[(edge + 1) % 3];
                    if source.triangles.iter().enumerate().any(|(other, indices)| {
                        other != index && indices.contains(&a) && indices.contains(&b)
                    }) {
                        continue;
                    }
                    let delta = vertices[b] - vertices[a];
                    let alpha = ((position - vertices[a]).dot(delta) / delta.length_squared())
                        .clamp(0.0, 1.0);
                    let distance = (position - vertices[a] - delta * alpha).length_squared();
                    if distance < closest {
                        closest = distance;
                        triangle = index;
                        weights = vec![(a, (1.0 - alpha) as f32), (b, alpha as f32)];
                    }
                }
            }
            break;
        }
        *cached = Some(triangle);
        if weights.is_empty() {
            return Err("source BlendSpace triangle search did not converge".to_owned());
        }
        weights.retain(|(_, weight)| *weight > 1e-5);
        for last in (1..weights.len()).rev() {
            let mut smallest = 0;
            for index in 1..=last {
                if weights[smallest].1 > weights[index].1 {
                    smallest = index;
                }
            }
            weights.swap(smallest, last);
        }
        let total = weights.iter().map(|(_, weight)| *weight).sum::<f32>();
        for (_, weight) in &mut weights {
            *weight /= total;
        }
        Ok(weights)
    }

    fn context_weights(
        &self,
        ctx: &mut NodeContext,
        position: Vec2,
    ) -> Result<Vec<(usize, f32)>, GraphError> {
        if self.source_triangulation.is_none() {
            return Ok(self.weights(position));
        }
        self.source_weights(
            position,
            &mut ctx.state_mut::<NormalizedBlendSpaceState>()?.triangle,
        )
        .map_err(|error| GraphError::UnknownErrorNode {
            graph: ctx.graph_context.context_id,
            node: ctx.node_id,
            error,
        })
    }

    fn player_tick(
        &self,
        binding: &SourceSyncPlayerBinding,
        leader: usize,
        previous_phase: Option<f32>,
        delta_seconds: f32,
        play_rate: f32,
        phase_markers: &[SyncMarker],
        sample_markers: &[SyncMarker],
    ) -> SourceSyncPlayerTick {
        let point = &self.points[leader];
        SourceSyncPlayerTick {
            key: binding.key.clone(),
            role: binding.role,
            weight: 1.0,
            phase: previous_phase.unwrap_or_else(|| {
                marker_phase_from_sample_time(
                    binding.start_position * point.duration,
                    point.duration,
                    sample_markers,
                    self.phase_duration,
                    phase_markers,
                    self.looped,
                )
            }),
            phase_duration: self.phase_duration,
            phase_markers: phase_markers.to_vec(),
            duration: point.duration,
            markers: sample_markers.to_vec(),
            looped: self.looped,
            delta_seconds,
            play_rate: play_rate * point.asset_rate_scale,
            ..default()
        }
    }

    pub fn pose_pin(id: &str) -> String {
        format!("pose_{id}")
    }

    pub fn time_pin(id: &str) -> String {
        format!("time_{id}")
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
        let weights = self.context_weights(&mut ctx, position)?;
        let Some((index, _)) = weights
            .into_iter()
            .rev()
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
        let graph_time = context_time_update(&ctx)?
            .partial_update_basic(ctx.prev_time())
            .unwrap_or_else(|| ctx.prev_time());
        let position = ctx.data_back(IN_POSITION)?.as_vec2()?;
        let target_weights = self.context_weights(&mut ctx, position)?;
        let Some((leader_index, _)) = target_weights
            .iter()
            .rev()
            .copied()
            .max_by(|left, right| left.1.total_cmp(&right.1))
        else {
            ctx.set_data_fwd(OUT_POSE, Pose::default());
            return Ok(());
        };
        let smoothed = if !self.per_bone_interpolation.is_empty() && self.playback.is_some() {
            let delta = crate::extensions::sync_group_phase::source_sync_delta_seconds(&ctx, &self.playback.as_ref().unwrap().delta_seconds_key)?;
            let state = ctx.state_mut::<NormalizedBlendSpaceState>()?;
            state.samples = self.smooth_samples(&state.samples, &target_weights, delta);
            Some(state.samples.clone())
        } else { None };
        let weights = smoothed.as_ref().map_or_else(|| target_weights.clone(), |samples|
            samples.iter().map(|sample| (sample.index, sample.weight)).collect());
        let mut sample_times = ctx.state::<NormalizedBlendSpaceState>().ok()
            .map(|state| state.sample_times.clone()).unwrap_or_default();
        sample_times.resize(self.points.len(), None);
        let mut marker_records = ctx.state::<NormalizedBlendSpaceState>().ok()
            .map(|state| state.marker_records.clone()).unwrap_or_default();
        marker_records.resize(self.points.len(), None);
        if crate::extensions::sync_group_phase::source_sync_markers_reset(&ctx) { marker_records.fill(None); }
        let sample_clip = |point: &NormalizedBlendSpacePoint| ctx.graph_context.graph.edges_inverted
            .get(&bevy_animation_graph::core::animation_graph::TargetPin::NodeData(ctx.node_id, Self::pose_pin(&point.id)))
            .and_then(|source| match source {
                bevy_animation_graph::core::animation_graph::SourcePin::NodeData(node, _) => ctx.graph_context.graph.nodes.get(node),
                _ => None,
            })
            .and_then(|node| node.try_inner_downcast_ref::<bevy_animation_graph::builtin_nodes::clip_node::ClipNode>())
            .and_then(|node| node.reflect_ref().as_struct().ok()?.field("clip"))
            .and_then(|value| value.try_downcast_ref::<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>())
            .cloned();
        let sample_clips = self.points.iter().map(sample_clip).collect::<Vec<_>>();
        let leader = &self.points[leader_index];
        let leader_duration = self.point_duration(&mut ctx, leader)?;
        let sample_looped = self.looped && !self.teleport_to_normalized_time;
        let phase_markers = self.playback.as_ref().map_or_else(
            || std::borrow::Cow::Borrowed(self.phase_markers.as_slice()),
            |playback| resolved_source_sync_markers(&ctx, &playback.key, &self.phase_markers));
        let leader_markers = self.playback.as_ref().map_or_else(
            || std::borrow::Cow::Borrowed(leader.markers.as_slice()),
            |playback| resolved_source_sync_markers(&ctx, &playback.key, &leader.markers));
        let uses_marker_phase = self.phase_duration > EPSILON
            && !phase_markers.is_empty() && !leader_markers.is_empty();
        let mut marker_advance = None;
        let mut leader_sample_time = resolved_source_sync_sample_time(&ctx)?;
        let resolved_time = leader_sample_time.is_some();
        let resolved_record = crate::extensions::sync_group_phase::resolved_source_marker_record(
            &ctx, &leader_markers, leader_duration, sample_clips[leader_index].clone())?;
        let resolved_previous_time = crate::extensions::sync_group_phase::resolved_source_sync_previous_time(&ctx)?;
        let phase = if let Some(playback) = &self.playback {
            let delta_seconds = crate::extensions::sync_group_phase::source_sync_delta_seconds(&ctx, &playback.delta_seconds_key)?;
            let play_rate = if playback.play_rate_key.is_empty() {
                1.0
            } else {
                global_f32_or_root_default(&ctx, &playback.play_rate_key)?
            };
            let previous = ctx
                .state::<NormalizedBlendSpaceState>()
                .ok()
                .and_then(|state| state.context.sync_players.first());
            let mut tick =
                self.player_tick(playback, leader_index, previous.map(|tick| tick.phase), delta_seconds, play_rate,
                    &phase_markers, &leader_markers);
            tick.sample_time_hint = Some(sample_times[leader_index]
                .unwrap_or(playback.start_position * leader_duration));
            tick.identity = (ctx.graph_context.context_id, ctx.node_id);
            tick.has_been_full_weight = previous.is_some_and(|tick| tick.has_been_full_weight)
                || (matches!(playback.role,
                    crate::extensions::sync_group_phase::SourceSyncGroupRole::TransitionLeader
                        | crate::extensions::sync_group_phase::SourceSyncGroupRole::TransitionFollower)
                    && ctx.graph_context.global_input_data.contains_key(
                        &crate::extensions::sync_group_phase::source_sync_full_weight_key(tick.identity)));
            tick.requested_inertialization = ctx.graph_context.global_input_data.get(
                crate::extensions::sync_group_phase::SOURCE_SYNC_INERTIAL_SCOPE)
                .is_some_and(|value| value.as_bool().unwrap_or(false));
            tick.previous_phase = tick.phase;
            tick.previous_marker_record = marker_records[leader_index].clone().filter(|record| record.asset == sample_clips[leader_index]);
            let phase = if let Some(phase) = resolved_source_sync_phase(&ctx, &playback.key)? {
                marker_advance = resolved_source_marker_advance(&ctx, &playback.key)?;
                phase
            } else {
                let mut step = tick.advance_from_time(tick.sample_time_hint.unwrap());
                if let Some(record) = &mut step.record { record.asset = sample_clips[leader_index].clone(); }
                leader_sample_time = Some(step.time);
                marker_records[leader_index] = step.record;
                marker_advance = Some(step.advance);
                step.phase
            };
            tick.phase = phase;
            tick.marker_advance = marker_advance.clone();
            ctx.state_mut::<NormalizedBlendSpaceState>()?.context = AnimationContextState {
                sync_players: vec![tick],
                ..default()
            };
            phase
        } else {
            normalize_phase(ctx.data_back(IN_PHASE)?.as_f32()?, sample_looped)
        };
        let leader_hint = sample_times[leader_index].unwrap_or_else(|| self.playback.as_ref()
            .map_or_else(|| phase_to_time(phase, leader_duration, sample_looped),
                |playback| playback.start_position * leader_duration));
        let mut map_time = |index: usize, markers: &[SyncMarker], duration: f32, hint: f32, has_history: bool| {
            if let Some(advance) = marker_advance.as_ref() {
                if !has_history { marker_records[index] = None; }
                marker_phase_sample_time_continuous(phase, advance, self.phase_duration,
                    &phase_markers, duration, markers, hint, sample_looped, &mut marker_records[index], sample_clips[index].clone())
            } else {
                marker_records[index] = None;
                marker_phase_sample_time_near(phase, self.phase_duration, &phase_markers,
                    duration, markers, hint, sample_looped)
            }
        };
        let leader_time = if let Some(time) = leader_sample_time {
            time
        } else if uses_marker_phase {
            map_time(leader_index, &leader_markers, leader_duration, leader_hint,
                sample_times[leader_index].is_some())
        } else {
            phase_to_time(phase, leader_duration, sample_looped)
        };
        let output_time = if self.teleport_to_normalized_time && leader.asset_rate_scale < 0.0 { leader_duration - leader_time } else { leader_time };
        let mut poses = Vec::with_capacity(weights.len());
        let mut pose_indices = Vec::with_capacity(weights.len());
        let mut curves = bevy::platform::collections::HashMap::new();
        for (index, weight) in weights {
            let point = &self.points[index];
            let duration = self.point_duration(&mut ctx, point)?;
            let point_markers = self.playback.as_ref().map_or_else(
                || std::borrow::Cow::Borrowed(point.markers.as_slice()),
                |playback| resolved_source_sync_markers(&ctx, &playback.key, &point.markers));
            let point_hint = sample_times[index].unwrap_or_else(|| self.playback.as_ref()
                .map_or_else(|| phase_to_time(phase, duration, sample_looped),
                    |playback| playback.start_position * duration));
            let time = if index == leader_index {
                leader_time
            } else if uses_marker_phase {
                map_time(index, &point_markers, duration, point_hint, sample_times[index].is_some())
            } else {
                follower_sample_time_near(
                    phase,
                    leader_duration,
                    &leader_markers,
                    duration,
                    &point_markers,
                    point_hint,
                    sample_looped,
                )
            };
            let time = if self.teleport_to_normalized_time && point.asset_rate_scale < 0.0 { duration - time } else { time };
            sample_times[index] = Some(time);
            ctx.set_time_update_back(Self::time_pin(&point.id), TimeUpdate::Absolute(time));
            poses.push((
                ctx.data_back(Self::pose_pin(&point.id))?.into_pose()?,
                weight,
            ));
            pose_indices.push(index);
            for (name, value) in point.curves.sample_all(time) {
                *curves.entry(name).or_insert(0.0) += value * weight;
            }
        }
        if resolved_time { marker_records[leader_index] = resolved_record; }
        let sample_clip = sample_clips[leader_index].clone();
        let notify_instance_id = crate::extensions::source_instance::source_notify_instance_id(&ctx)?;
        let state = ctx.state_mut::<NormalizedBlendSpaceState>()?;
        if self.playback.is_some() {
            state.sample_times = sample_times;
            state.marker_records = marker_records;
        }
        let context = &mut state.context;
        context.curves = curves;
        context.curves_are_authoritative = true;
        context.asset_samples.clear();
        if !self.teleport_to_normalized_time && let Some(tick) = context.sync_players.first() {
            let previous_position = resolved_previous_time.unwrap_or(leader_hint);
            let clock = crate::extensions::SourceRelevantAsset {
                duration: leader_duration,
                position: leader_time,
                previous_position,
                has_previous_position: true,
                delta_seconds: tick.delta_seconds,
                playback_delta_seconds: tick.delta_seconds * tick.play_rate,
                looping: sample_looped,
                weight: 1.0,
            };
            context.relevant_asset = Some(clock.clone());
            if tick.delta_seconds > 0.0 && let Some(clip) = sample_clip {
                context.asset_samples.push(crate::extensions::animation_context::SourceAssetSample {
                    identity: format!("{:?}:{:?}", tick.identity.0, tick.identity.1),
                    notify_instance_id,
                    clip,
                    clock,
                    sync_group: tick.key.clone(), sync_identity: Some(tick.identity),
                    montage: false,
                    slot_key: String::new(),
                });
            }
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
        if let Some(samples) = smoothed.as_ref() {
            let skeleton = ctx.graph_context.resources.skeleton_assets.get(&output.skeleton);
            for (bone, output_index) in &output.paths {
                let Some(profile_index) = skeleton.and_then(|skeleton| skeleton.id_to_path(*bone))
                    .and_then(|path| path.parts.iter().rev().find_map(|part|
                        self.per_bone_interpolation.iter().position(|entry| entry.bone.eq_ignore_ascii_case(part.as_str()))))
                else { continue; };
                let blended = poses.iter().zip(&pose_indices).filter_map(|((pose, _), index)| {
                    let sample = samples.iter().find(|sample| sample.index == *index)?;
                    pose.get_bone(*bone).map(|bone| bone.scalar_mult(sample.bone_weights[profile_index]))
                }).reduce(|left, right| left.linear_add(&right)).map(|bone| bone.normalize_quat());
                if let Some(blended) = blended { output.bones[*output_index] = blended; }
            }
        }
        output.root_motion = root_motion;
        output.timestamp = graph_time;
        ctx.set_time(graph_time);
        ctx.set_data_fwd(OUT_POSE, output);
        ctx.set_data_fwd(OUT_SAMPLE_TIME, output_time);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_POSITION, DataSpec::Vec2);
        if self.playback.is_none() {
            ctx.add_input_data(IN_PHASE, DataSpec::F32);
        }
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

    #[test]
    fn per_bone_ease_preserves_rate_and_normalizes_each_bone() {
        let node = NormalizedBlendSpaceNode {
            points: vec![NormalizedBlendSpacePoint::default(), NormalizedBlendSpacePoint::default()],
            per_bone_interpolation: vec![SourcePerBoneInterpolation { bone: "spine".into(), speed: 1.0 }],
            per_bone_interpolation_ease_in_out: true,
            ..default()
        };
        let initial = node.smooth_samples(&[], &[(0, 1.0), (1, 0.0)], 0.1);
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0].bone_weights[0], 1.0);
        assert!(initial[0].bone_rates[0] > 1.7);
        let switched = node.smooth_samples(&initial, &[(0, 0.0), (1, 1.0)], 0.1);
        assert_eq!(switched.len(), 2);
        assert!((switched[1].bone_weights[0] - 0.09428191).abs() < 1e-5);
        assert!(switched[0].bone_rates[0] < 0.0 && switched[1].bone_rates[0] > 0.0);
        let paused = node.smooth_samples(&switched, &[(0, 1.0), (1, 0.0)], 0.0);
        assert!((paused[1].bone_weights[0] - switched[1].bone_weights[0]).abs() < 1e-6);
        let rebuilt = node.smooth_samples(&[], &[(1, 1.0)], 0.1);
        assert_eq!(rebuilt.len(), 1);
        assert_eq!(rebuilt[0].bone_weights[0], 1.0);
    }

    #[test]
    fn per_bone_weights_follow_source_history_and_hierarchy() {
        use bevy_animation_graph::core::{
            animation_clip::{EntityPath, GraphClip}, animation_graph::{AnimationGraph, GraphInputPin, NodeId},
            animation_node::AnimationNode, context::{deferred_gizmos::DeferredGizmos,
                graph_context::QueryOutputTime, graph_context_arena::GraphContextArena,
                new_context::GraphContext, system_resources::SystemResources},
            pose::BonePose, skeleton::Skeleton, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        fn query(In((handle, node)): In<(Handle<AnimationGraph>, NodeId)>, resources: SystemResources) {
            let mut graph = resources.animation_graph_assets.get(&handle).unwrap().clone();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let env = TestIoEnv::default();
            let entities = default();
            for (position, delta, rebuild, spine, hand) in [
                (0.0, 0.1, false, 0.0, 0.0),
                (1.0, 0.1, false, 1.0, 2.0),
                (1.0, 0.1, false, 2.0, 4.0),
                (1.0, 0.1, true, 10.0, 10.0),
                (0.0, 0.0, false, 10.0, 10.0),
            ] {
                if rebuild { arena = GraphContextArena::new(handle.id()); } else { arena.next_frame(); }
                graph.set_default_data(GraphInputPin::passthrough(IN_POSITION), Vec2::new(position, 0.0).into());
                let inputs = [("delta".to_owned(), delta.into())].into();
                let context = GraphContext::new(arena.get_toplevel_id(), &graph, &mut arena, &resources,
                    &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                let output = graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(delta)), context).unwrap();
                let pose = output[OUT_POSE].as_pose().unwrap();
                let root = EntityPath::default().child("root");
                for (path, expected) in [(root.clone(), position * 10.0), (root.child("spine"), spine),
                    (root.child("spine").child("hand"), hand), (root.child("spine").child("head"), spine),
                    (root.child("leg"), position * 10.0)] {
                    let actual = pose.get_bone(path.id()).unwrap().translation.unwrap().x;
                    assert!((actual - expected).abs() < 1e-5, "position={position}, delta={delta}, rebuild={rebuild}, bone={path:?}: {actual} != {expected}");
                    let actual_scale = pose.get_bone(path.id()).unwrap().scale.unwrap().x;
                    assert!((actual_scale - (1.0 + expected / 10.0)).abs() < 1e-5);
                    let ratio = expected / 10.0;
                    let expected_rotation = (Quat::IDENTITY * (1.0 - ratio) + Quat::from_rotation_z(std::f32::consts::FRAC_PI_2) * ratio).normalize();
                    assert!(pose.get_bone(path.id()).unwrap().rotation.unwrap().dot(expected_rotation).abs() > 1.0 - 1e-5);
                }
                assert!((pose.root_motion.as_ref().unwrap().translation.x - position * 10.0).abs() < 1e-5);
                let state = arena.get_toplevel().node_states.get::<NormalizedBlendSpaceState>(node, default()).unwrap();
                assert!((state.context.curves[&crate::extensions::SourceCurveName::from("Fixture")] - (2.0 + 6.0 * position)).abs() < 1e-5);
            }
        }
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let root = EntityPath::default().child("root");
        let paths = [root.clone(), root.child("spine"), root.child("spine").child("hand"),
            root.child("spine").child("head"), root.child("leg")];
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root.id());
        for path in &paths { skeleton.add_bone(path.clone(), Transform::IDENTITY, Transform::IDENTITY); }
        let skeleton = world.resource_mut::<Assets<Skeleton>>().add(skeleton);
        let mut graph = AnimationGraph::new();
        let mut points = Vec::new();
        for (id, position, value) in [("a", Vec2::ZERO, 0.0), ("b", Vec2::X, 10.0)] {
            let curves = crate::extensions::parse_curves(&serde_json::json!({"curves":[{
                "name":"Fixture", "times":[0.0], "values":[if id=="a" {2.0} else {8.0}], "interps":["LINEAR"]
            }]})).unwrap();
            points.push(NormalizedBlendSpacePoint { id: id.into(), point: position, duration: 1.0, curves, ..default() });
            let mut pose = Pose { skeleton: skeleton.clone(), root_motion: Some(RootMotionDelta {
                translation: Vec3::X * value, ..default() }), ..default() };
            for path in &paths { pose.add_bone(BonePose { translation: Some(Vec3::X * value),
                rotation: Some(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2 * value / 10.0)),
                scale: Some(Vec3::splat(1.0 + value / 10.0)), ..default() }, path.id()); }
            graph.set_default_data(GraphInputPin::passthrough(NormalizedBlendSpaceNode::pose_pin(id)), pose.into());
        }
        let node = AnimationNode::new("per-bone player", NormalizedBlendSpaceNode { points, looped: true,
            playback: Some(SourceSyncPlayerBinding { key: "fixture".into(), delta_seconds_key: "delta".into(), ..default() }),
            per_bone_interpolation: vec![SourcePerBoneInterpolation { bone: "spine".into(), speed: 1.0 },
                SourcePerBoneInterpolation { bone: "hand".into(), speed: 2.0 }],
            per_bone_interpolation_ease_in_out: false, ..default() });
        graph.add_input_data_edge(GraphInputPin::passthrough(IN_POSITION), node.id, IN_POSITION);
        for id in ["a", "b"] {
            graph.add_input_data_edge(GraphInputPin::passthrough(NormalizedBlendSpaceNode::pose_pin(id)), node.id, NormalizedBlendSpaceNode::pose_pin(id));
            graph.add_input_time_edge(GraphInputPin::passthrough(NormalizedBlendSpaceNode::time_pin(id)), node.id, NormalizedBlendSpaceNode::time_pin(id));
        }
        graph.add_output_data(OUT_POSE.into(), DataSpec::Pose);
        graph.add_output_data_edge(node.id, OUT_POSE, OUT_POSE);
        graph.add_output_time();
        graph.add_output_pose_edge(node.id);
        graph.add_node(node.clone());
        let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
        world.run_system_cached_with(query, (handle, node.id)).unwrap();
    }

    #[test]
    fn blendspace_asset_rate_scale_controls_the_leader_clock() {
        for rate in [-2.0_f32, 0.0, 0.5, 2.0] {
            let node = NormalizedBlendSpaceNode {
                looped: true, phase_duration: 1.0,
                points: vec![NormalizedBlendSpacePoint { duration: 2.0, asset_rate_scale: rate, ..default() }],
                ..default()
            };
            let binding = SourceSyncPlayerBinding { start_position: 0.25, ..default() };
            let tick = node.player_tick(&binding, 0, None, 0.1, 3.0,
                &node.phase_markers, &node.points[0].markers);
            assert_eq!(tick.play_rate, 3.0 * rate);
            let time = marker_phase_sample_time(tick.advance(tick.phase).phase, 1.0, &[], 2.0, &[], true);
            assert!((time - (0.5 + 0.1 * 3.0 * rate).rem_euclid(2.0)).abs() < 1e-5);
        }
    }

    #[test]
    fn repeated_marker_pairs_keep_each_blendspace_sample_near_its_own_clock() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip,
            animation_graph::{AnimationGraph, GraphInputPin, NodeId},
            animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos,
                graph_context::{GraphState, QueryOutputTime},
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                node_states::StateKey, system_resources::SystemResources},
            edge_data::DataValue, skeleton::Skeleton,
            state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };

        fn query(In((handle, node)): In<(Handle<AnimationGraph>, NodeId)>, resources: SystemResources) -> Vec<[f32; 2]> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut results = Vec::new();
            let frames: &[(f32, bool, &[&str])] = &[(0.2, true, &[]), (0.19, false, &[]),
                (0.95, false, &["A"]), (0.11, true, &["A"]), (0.94, false, &["A"]), (0.75, false, &["B"]),
                (0.15, true, &["B", "A"]), (0.75, true, &["B", "A", "B", "A", "B", "A", "B", "A", "B", "A"]),
                (0.8, true, &[]), (0.1, false, &["A", "B", "A", "B", "A", "B", "A", "B", "A", "B"]), (0.2, true, &[])];
            for (frame, &(phase, forward, passed)) in frames.iter().enumerate() {
                if frame == 10 {
                    *arena.get_toplevel_mut() = GraphState::new(handle.id());
                }
                let mut inputs = HashMap::from([
                    ("delta".to_owned(), DataValue::F32(0.1)),
                    (crate::extensions::sync_group_phase::SOURCE_SYNC_RESOLVED.to_owned(), DataValue::Bool(true)),
                    (format!("{}.Movement", crate::extensions::sync_group_phase::SOURCE_SYNC_RESOLVED), DataValue::F32(phase)),
                ]);
                if frame != 0 && frame != 10 {
                    inputs.extend(crate::extensions::sync_group_phase::SourceMarkerAdvance {
                        forward, markers: passed.iter().map(|name| (*name).to_owned()).collect(), ..default()
                    }.into_inputs("Movement"));
                }
                let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena,
                    &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.1)), context).unwrap();
                let times = ["p0", "p1"].map(|id| {
                    match arena.get_toplevel().node_caches.get_input_time_update(node, StateKey::Default,
                        NormalizedBlendSpaceNode::time_pin(id)).unwrap() {
                        TimeUpdate::Absolute(time) => time,
                        update => panic!("unexpected sample update: {update:?}"),
                    }
                });
                results.push(times);
            }
            results
        }

        let phase_markers = [("A", 0.1), ("B", 0.3), ("A", 0.6), ("B", 0.8)]
            .map(|(name, time)| SyncMarker { name: name.into(), time }).to_vec();
        let samples = [
            [("A", 0.05), ("B", 0.25), ("A", 0.55), ("B", 0.95)],
            [("A", 0.0), ("B", 0.2), ("A", 0.5), ("B", 0.9)],
        ];
        let points = samples.into_iter().enumerate().map(|(index, entries)| NormalizedBlendSpacePoint {
            id: format!("p{index}"), point: Vec2::new(index as f32, 0.0), duration: 1.0,
            markers: entries.map(|(name, time)| SyncMarker { name: name.into(), time }).to_vec(),
            ..default()
        }).collect();
        let node = AnimationNode::new("blendspace", NormalizedBlendSpaceNode {
            looped: true, phase_duration: 1.0, phase_markers, points,
            playback: Some(SourceSyncPlayerBinding {
                key: "Movement".into(), start_position: 0.65,
                delta_seconds_key: "delta".into(), ..default()
            }),
            ..default()
        });
        let mut graph = AnimationGraph::new();
        graph.set_default_data(GraphInputPin::passthrough(IN_POSITION), DataValue::Vec2(Vec2::new(0.5, 0.0)));
        graph.set_default_data(GraphInputPin::passthrough("sample"), DataValue::Pose(Pose::default()));
        graph.add_node(node.clone());
        graph.add_input_data_edge(GraphInputPin::passthrough(IN_POSITION), node.id, IN_POSITION);
        for id in ["p0", "p1"] {
            graph.add_input_data_edge(GraphInputPin::passthrough("sample"), node.id, NormalizedBlendSpaceNode::pose_pin(id));
            graph.add_input_time_edge(GraphInputPin::passthrough(NormalizedBlendSpaceNode::time_pin(id)),
                node.id, NormalizedBlendSpaceNode::time_pin(id));
        }
        graph.add_output_data(OUT_SAMPLE_TIME.into(), DataSpec::F32);
        graph.add_output_data_edge(node.id, OUT_SAMPLE_TIME, OUT_SAMPLE_TIME);
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
        let actual = world.run_system_cached_with(query, (handle, node.id)).unwrap();
        for (frame, expected) in [[0.75, 0.7], [0.73, 0.68], [0.4, 0.35],
            [0.57, 0.52], [0.39, 0.34], [0.2, 0.15], [0.65, 0.6],
            [0.2, 0.15], [0.25, 0.2], [0.55, 0.5], [0.75, 0.7]].into_iter().enumerate() {
            for index in 0..2 {
                let error = (actual[frame][index] - expected[index]).abs();
                assert!(error.min((1.0 - error).abs()) < 1e-5,
                    "frame {frame} sample {index}: {} != {}", actual[frame][index], expected[index]);
            }
        }
    }

    #[test]
    fn source_evaluator_teleport_clamps_normalized_endpoints() {
        use bevy_animation_graph::core::{
            animation_clip::GraphClip, animation_graph::{AnimationGraph, GraphInputPin, NodeId}, animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext, node_states::StateKey,
                system_resources::SystemResources},
            skeleton::Skeleton, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        fn query(In((handle, node, expected)): In<(Handle<AnimationGraph>, NodeId, f32)>, resources: SystemResources) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let env = TestIoEnv::default();
            let entities = default();
            let inputs = default();
            let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources,
                &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
            let output = graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.0)), context).unwrap();
            assert_eq!(output[OUT_SAMPLE_TIME].as_f32().unwrap(), expected);
            let update = arena.get_toplevel().node_caches.get_input_time_update(node, StateKey::Default, "time_sample".into()).unwrap();
            assert!(matches!(update, TimeUpdate::Absolute(value) if value == expected));
        }
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        for (phase, rate) in [0.25_f32, 0.5, 1.0, -0.2, 1.2].into_iter().flat_map(|phase|
            [-2.0_f32, 0.0, 0.5, 1.0, 2.0].into_iter().map(move |rate| (phase, rate))) {
            let expected = if rate < 0.0 { (1.0 - phase.clamp(0.0, 1.0)) * 2.0 } else { phase.clamp(0.0, 1.0) * 2.0 };
            let node = AnimationNode::new("evaluator", NormalizedBlendSpaceNode {
                looped: true,
                teleport_to_normalized_time: true,
                points: vec![NormalizedBlendSpacePoint {
                    id: "sample".to_owned(),
                    duration: 2.0,
                    asset_rate_scale: rate,
                    ..default()
                }],
                ..default()
            });
            let mut graph = AnimationGraph::new();
            for (pin, value) in [(IN_POSITION, Vec2::ZERO.into()), (IN_PHASE, phase.into()), ("pose_sample", Pose::default().into())] {
                graph.set_default_data(GraphInputPin::passthrough(pin), value);
                graph.add_input_data_edge(GraphInputPin::passthrough(pin), node.id, pin);
            }
            graph.add_input_time_edge(GraphInputPin::passthrough("time_sample"), node.id, "time_sample");
            graph.add_output_data(OUT_SAMPLE_TIME.into(), DataSpec::F32);
            graph.add_output_data_edge(node.id, OUT_SAMPLE_TIME, OUT_SAMPLE_TIME);
            graph.add_node(node.clone());
            let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
            world.run_system_cached_with(query, (graph, node.id, expected)).unwrap();
        }
    }

    #[test]
    fn source_look_segments_keep_the_center_sample_and_nearest_neighbors() {
        let node = NormalizedBlendSpaceNode {
            source_triangulation: Some(SourceBlendSpaceTriangulation {
                axis_min: Vec2::new(-90.0, 0.0),
                axis_max: Vec2::new(90.0, 100.0),
                segments: vec![[1, 0], [0, 2]],
                ..default()
            }),
            points: [0.0, -90.0, 90.0]
                .into_iter()
                .enumerate()
                .map(|(index, x)| NormalizedBlendSpacePoint {
                    id: format!("sample_{index}"),
                    point: Vec2::new(x, 0.0),
                    ..default()
                })
                .collect(),
            ..default()
        };
        for (pitch, expected) in [
            (-45.0, [0.5, 0.5, 0.0]),
            (0.0, [1.0, 0.0, 0.0]),
            (45.0, [0.5, 0.0, 0.5]),
            (-120.0, [0.0, 1.0, 0.0]),
            (120.0, [0.0, 0.0, 1.0]),
        ] {
            let mut actual = [0.0; 3];
            for (index, weight) in node
                .source_weights(Vec2::new(pitch, 0.0), &mut None)
                .unwrap()
            {
                actual[index] = weight;
            }
            assert_eq!(actual, expected, "pitch {pitch}");
        }
        let mut ablated = node.clone();
        ablated.source_triangulation = None;
        assert_ne!(
            ablated
                .source_weights(Vec2::new(45.0, 0.0), &mut None)
                .unwrap(),
            node.source_weights(Vec2::new(45.0, 0.0), &mut None)
                .unwrap()
        );
    }

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
        assert!((x_value - 1.0).abs() < 1e-5);
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
                    ..default()
                },
                NormalizedBlendSpacePoint {
                    id: "run".to_owned(),
                    point: Vec2::new(1.0, 1.0),
                    duration: 0.8,
                    markers: markers(&[("Left", 0.033333), ("Right", 0.433333)]),
                    ..default()
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
            per_bone_interpolation: vec![],
            per_bone_interpolation_ease_in_out: false,
            looped: true,
            teleport_to_normalized_time: false,
            phase_duration: 1.0,
            phase_markers: markers(&[("Left", 0.0), ("Right", 0.5)]),
            playback: None,
            source_triangulation: None,
            points: vec![
                NormalizedBlendSpacePoint {
                    id: "forward".to_owned(),
                    point: Vec2::ZERO,
                    duration: 1.133333,
                    markers: markers(&[("Left", 0.083333), ("Right", 0.65)]),
                    ..default()
                },
                NormalizedBlendSpacePoint {
                    id: "backward".to_owned(),
                    point: Vec2::ONE,
                    duration: 1.133333,
                    markers: markers(&[("Right", 0.516667), ("Left", 1.083333)]),
                    ..default()
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
            crate::extensions::marker_position_at_time(&node.points[0].markers, 1.133333, forward_time, true)
                .unwrap();
        let backward_position =
            crate::extensions::marker_position_at_time(&node.points[1].markers, 1.133333, backward_time, true)
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
    fn player_clock_and_filtered_weights_match_native_blendspace_ticks() {
        let mut ablated_errors = 0;
        for source_json in [
            include_str!(
                "../../tools/animgraph_authoring/fixtures/ue_standing_walk_run_tick.json"
            ),
            include_str!(
                "../../tools/animgraph_authoring/fixtures/ue_standing_walk_run_weights.json"
            ),
        ] {
            let source: serde_json::Value = serde_json::from_str(source_json).unwrap();
            let binding = SourceSyncPlayerBinding {
                start_position: source["start_position"].as_f64().unwrap() as f32,
                ..default()
            };
            let node = NormalizedBlendSpaceNode {
                looped: true,
                teleport_to_normalized_time: false,
                phase_duration: 1.133333,
                per_bone_interpolation: vec![],
                per_bone_interpolation_ease_in_out: false,
                phase_markers: markers(&[("Right", 0.516667), ("Left", 1.083333)]),
                points: [Vec2::X, Vec2::ZERO, Vec2::ONE, Vec2::Y]
                    .into_iter()
                    .enumerate()
                    .map(|(index, point)| NormalizedBlendSpacePoint {
                        id: index.to_string(),
                        point,
                        duration: if index < 2 { 1.133333 } else { 0.8 },
                        markers: if index < 2 {
                            markers(&[("Left", 0.083333), ("Right", 0.65)])
                        } else {
                            markers(&[("Left", 0.033333), ("Right", 0.433333)])
                        },
                        ..default()
                    })
                    .collect(),
                playback: Some(binding.clone()),
                source_triangulation: Some(SourceBlendSpaceTriangulation {
                    axis_min: Vec2::ZERO,
                    axis_max: Vec2::ONE,
                    triangles: vec![[1, 0, 3], [0, 2, 3]],
                    segments: vec![],
                }),
            };
            let mut filter = CubicVec2AxisFilterState::default();
            let mut phase = None;
            let mut triangle = None;
            let mut ablated = node.clone();
            ablated.source_triangulation = None;
            let mut ablated_phase = None;
            for frame in source["frames"].as_array().unwrap() {
                let input = &frame["input"];
                let delta = input["delta_seconds"].as_f64().unwrap() as f32;
                let position = Vec2::new(
                    cubic_filter_sample(
                        &mut filter.x,
                        Vec2::X * input["x"].as_f64().unwrap() as f32,
                        delta,
                        0.2,
                    )
                    .x,
                    cubic_filter_sample(
                        &mut filter.y,
                        Vec2::Y * input["y"].as_f64().unwrap() as f32,
                        delta,
                        0.4,
                    )
                    .y,
                );
                for (axis, actual) in [("x", position.x), ("y", position.y)] {
                    let expected = frame["filtered_position"][axis].as_f64().unwrap() as f32;
                    assert!(
                        (actual - expected).abs() < 2e-5,
                        "filter {} {axis}: {actual} != {expected}",
                        frame["frame"]
                    );
                }
                let weights = node.source_weights(position, &mut triangle).unwrap();
                let leader = weights
                    .iter()
                    .rev()
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .unwrap()
                    .0;
                let tick = node.player_tick(
                    &binding,
                    leader,
                    phase,
                    delta,
                    input["play_rate"].as_f64().unwrap() as f32,
                    &node.phase_markers,
                    &node.points[leader].markers,
                );
                let current = tick.advance(tick.phase).phase;
                phase = Some(current);
                let ablated_weights = ablated.source_weights(position, &mut None).unwrap();
                let ablated_leader = ablated_weights
                    .iter()
                    .rev()
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .unwrap()
                    .0;
                let ablated_tick = ablated.player_tick(
                    &binding,
                    ablated_leader,
                    ablated_phase,
                    delta,
                    input["play_rate"].as_f64().unwrap() as f32,
                    &ablated.phase_markers,
                    &ablated.points[ablated_leader].markers,
                );
                let ablated_current = ablated_tick.advance(ablated_tick.phase).phase;
                ablated_phase = Some(ablated_current);
                for sample in frame["samples"].as_array().unwrap() {
                    let index = sample["sample_index"].as_u64().unwrap() as usize;
                    let point = &node.points[index];
                    let weight = weights
                        .iter()
                        .find(|(candidate, _)| *candidate == index)
                        .map(|(_, weight)| *weight)
                        .unwrap_or(0.0);
                    assert!((weight - sample["weight"].as_f64().unwrap() as f32).abs() < 2e-5);
                    let actual = marker_phase_sample_time(
                        current,
                        node.phase_duration,
                        &node.phase_markers,
                        point.duration,
                        &point.markers,
                        true,
                    );
                    let expected = sample["time"].as_f64().unwrap() as f32;
                    let error = (actual - expected).abs().rem_euclid(point.duration);
                    assert!(
                        error.min(point.duration - error) < 2e-5,
                        "clock {} sample {index}: {actual} != {expected}",
                        frame["frame"]
                    );
                    let ablated_time = marker_phase_sample_time(
                        ablated_current,
                        node.phase_duration,
                        &node.phase_markers,
                        point.duration,
                        &point.markers,
                        true,
                    );
                    let error = (ablated_time - expected).abs().rem_euclid(point.duration);
                    ablated_errors += (error.min(point.duration - error) > 2e-5) as usize;
                }
            }
        }
        assert!(ablated_errors > 0);
    }

    #[test]
    fn player_relevant_clock_follows_the_highest_weighted_sample_without_a_clip_handle() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::GraphClip,
            animation_graph::{AnimationGraph, GraphInputPin, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                node_states::StateKey, system_resources::SystemResources,
            },
            edge_data::DataValue,
            pose::Pose,
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };

        fn query(In((handle, delta)): In<(Handle<AnimationGraph>, f32)>, resources: SystemResources) -> (f32, f32, usize) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let node = graph.nodes.values().find(|node| node.name == "player").unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let inputs = HashMap::from([
                ("sample".to_owned(), DataValue::Pose(Pose::default())),
                ("delta".to_owned(), DataValue::F32(delta)),
            ]);
            let context = GraphContext::new(
                arena.get_toplevel_id(), graph, &mut arena, &resources, &env,
                Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs,
            );
            graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(delta)), context).unwrap();
            let state = arena.get_toplevel().node_states.get::<NormalizedBlendSpaceState>(node.id, StateKey::Default).unwrap();
            let clock = state.context.relevant_asset.as_ref().unwrap();
            (clock.duration, clock.position, state.context.asset_samples.len())
        }

        let mut graphs = Assets::<AnimationGraph>::default();
        let mut handles = Vec::new();
        for position in [Vec2::ZERO, Vec2::X] {
            let mut graph = AnimationGraph::new();
            graph.set_default_data(GraphInputPin::passthrough("position"), DataValue::Vec2(position));
            graph.set_default_data(GraphInputPin::passthrough("sample"), DataValue::Pose(Pose::default()));
            let node = AnimationNode::new("player", NormalizedBlendSpaceNode {
                looped: true,
                phase_duration: 1.0,
                points: [("a", Vec2::ZERO, 1.0), ("b", Vec2::X, 2.0)].into_iter().map(|(id, point, duration)| NormalizedBlendSpacePoint {
                    id: id.to_owned(), point, duration, ..default()
                }).collect(),
                playback: Some(SourceSyncPlayerBinding {
                    key: "group".into(), start_position: 0.2, delta_seconds_key: "delta".into(), ..default()
                }),
                ..default()
            });
            graph.add_node(node.clone());
            graph.add_input_data_edge(GraphInputPin::passthrough("position"), node.id, "position");
            for id in ["a", "b"] {
                graph.add_input_data_edge(GraphInputPin::passthrough("sample"), node.id, NormalizedBlendSpaceNode::pose_pin(id));
            }
            graph.add_output_data("Pose".into(), DataSpec::Pose);
            graph.add_output_data_edge(node.id, "pose", "Pose");
            handles.push(graphs.add(graph));
        }
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let actual = handles.iter().cloned().map(|handle| world.run_system_cached_with(query, (handle, 0.1)).unwrap()).collect::<Vec<_>>();
        assert_eq!(actual.iter().map(|(duration, _, samples)| (*duration, *samples)).collect::<Vec<_>>(), [(1.0, 0), (2.0, 0)]);
        assert!((actual[0].1 - 0.3).abs() < 1e-6);
        assert!((actual[1].1 - 0.5).abs() < 1e-6);
        let stopped = world.run_system_cached_with(query, (handles[1].clone(), 0.0)).unwrap();
        assert_eq!((stopped.0, stopped.2), (2.0, 0));
        assert!((stopped.1 - 0.4).abs() < 1e-6);
    }

    #[test]
    fn source_triangles_project_to_the_boundary_and_reject_invalid_indices() {
        let mut node = NormalizedBlendSpaceNode {
            points: [Vec2::ZERO, Vec2::X, Vec2::Y]
                .into_iter()
                .map(|point| NormalizedBlendSpacePoint { point, ..default() })
                .collect(),
            source_triangulation: Some(SourceBlendSpaceTriangulation {
                axis_min: Vec2::ZERO,
                axis_max: Vec2::ONE,
                triangles: vec![[0, 1, 2]],
                segments: vec![],
            }),
            ..default()
        };
        assert_eq!(
            node.source_weights(Vec2::splat(0.75), &mut None).unwrap(),
            vec![(2, 0.5), (1, 0.5)]
        );
        node.source_triangulation.as_mut().unwrap().triangles[0][2] = 3;
        assert!(node.source_weights(Vec2::ZERO, &mut None).is_err());
        node.points.push(NormalizedBlendSpacePoint {
            point: Vec2::ONE,
            ..default()
        });
        node.source_triangulation.as_mut().unwrap().triangles = vec![[0, 1, 2], [1, 3, 2]];
        let near_edge = Vec2::new(0.50003, 0.5);
        let retained = node.source_weights(near_edge, &mut Some(0)).unwrap();
        let reset = node.source_weights(near_edge, &mut None).unwrap();
        assert_eq!(retained.len(), 2);
        assert_eq!(reset.len(), 3);
        node.source_triangulation.as_mut().unwrap().triangles = vec![[0, 0, 0]];
        assert_eq!(
            node.source_weights(Vec2::ONE, &mut None).unwrap(),
            vec![(0, 1.0)]
        );
        node.source_triangulation.as_mut().unwrap().triangles = vec![[0, 1, 1]];
        assert_eq!(
            node.source_weights(Vec2::splat(0.5), &mut None).unwrap(),
            vec![(1, 0.5), (0, 0.5)]
        );
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
