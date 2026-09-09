use bevy::{platform::collections::HashMap, prelude::*};
use bevy_animation_graph::core::{
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::{BoneId, Pose},
};

use crate::animation_context::{
    AnimationContextState, context_time_update, pose_input_context_state,
    resolve_named_context_curves,
};
use crate::inertialization::source_inertial_float;

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct InertializationPassthroughNode {
    pub reset_on_becoming_relevant: bool,
    pub forward_requests_through_skipped_cached_poses: bool,
    pub filtered_curves: Vec<String>,
    pub filtered_bones: Vec<String>,
    pub curve_names: Vec<String>,
    pub capture_context: bool,
    pub expose_context_curves: bool,
    pub exposed_output_data: Vec<String>,
    pub missing_curve_value: Option<f32>,
    pub internalize_time: bool,
    pub output: String,
    pub request_epoch_key: String,
    pub request_duration_key: String,
}

impl Default for InertializationPassthroughNode {
    fn default() -> Self {
        Self {
            reset_on_becoming_relevant: true,
            forward_requests_through_skipped_cached_poses: true,
            filtered_curves: default(),
            filtered_bones: default(),
            curve_names: Vec::new(),
            capture_context: false,
            expose_context_curves: false,
            exposed_output_data: Vec::new(),
            missing_curve_value: None,
            internalize_time: false,
            output: "pose".to_owned(),
            request_epoch_key: default(),
            request_duration_key: default(),
        }
    }
}

#[derive(Reflect, Clone, Debug, Default)]
struct SourceInertialBone {
    bone: BoneId,
    translation_direction: Vec3,
    translation_magnitude: f32,
    translation_speed: f32,
    rotation_axis: Vec3,
    rotation_angle: f32,
    rotation_speed: f32,
    scale_direction: Vec3,
    scale_magnitude: f32,
    scale_speed: f32,
}

#[derive(Reflect, Clone, Debug, Default)]
pub(crate) struct SourceInertializationState {
    previous: Option<Pose>,
    previous_previous: Option<Pose>,
    previous_delta: f32,
    last_update_frame: Option<u64>,
    last_request_epoch: f32,
    duration: f32,
    elapsed: f32,
    deficit: f32,
    active: bool,
    bones: Vec<SourceInertialBone>,
    previous_curves: HashMap<String, f32>,
    curve_differences: HashMap<String, (f32, f32)>,
    pub(crate) context: AnimationContextState,
}

impl InertializationPassthroughNode {
    fn expose_output(&self, curve: &str) -> bool {
        !self.capture_context
            || self.expose_context_curves
            || self.exposed_output_data.iter().any(|value| value == curve)
    }
}

fn optional_global_f32(ctx: &NodeContext, key: &str) -> f32 {
    if key.is_empty() {
        return 0.0;
    }
    ctx.graph_context
        .global_input_data
        .get(key)
        .and_then(|value| value.as_f32().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
}

fn source_delta(time_update: &TimeUpdate) -> f32 {
    match time_update {
        TimeUpdate::Delta(value) if value.is_finite() => value.max(0.0),
        TimeUpdate::Delta(_) | TimeUpdate::Absolute(_) | TimeUpdate::PercentOfEvent { .. } => 0.0,
    }
}

fn vector_difference(
    target: Option<Vec3>,
    previous: Option<Vec3>,
    previous_previous: Option<Vec3>,
    delta: f32,
) -> (Vec3, f32, f32) {
    let (Some(target), Some(previous), Some(previous_previous)) =
        (target, previous, previous_previous)
    else {
        return (Vec3::ZERO, 0.0, 0.0);
    };
    let difference = previous - target;
    let magnitude = difference.length();
    if !magnitude.is_finite() || magnitude <= f32::EPSILON {
        return (Vec3::ZERO, 0.0, 0.0);
    }
    let direction = difference / magnitude;
    let previous_magnitude = (previous_previous - target).dot(direction);
    let speed = if delta > f32::EPSILON {
        (magnitude - previous_magnitude) / delta
    } else {
        0.0
    };
    (direction, magnitude, speed)
}

fn rotation_difference(
    target: Option<Quat>,
    previous: Option<Quat>,
    previous_previous: Option<Quat>,
    delta: f32,
) -> (Vec3, f32, f32) {
    let (Some(target), Some(previous), Some(previous_previous)) =
        (target, previous, previous_previous)
    else {
        return (Vec3::ZERO, 0.0, 0.0);
    };
    let difference = (previous * target.inverse()).normalize();
    let (axis, angle) = difference.to_axis_angle();
    if !axis.is_finite() || !angle.is_finite() || angle <= f32::EPSILON {
        return (Vec3::ZERO, 0.0, 0.0);
    }
    let previous_difference = (previous_previous * target.inverse()).normalize();
    let previous_angle = previous_difference.to_scaled_axis().dot(axis);
    let speed = if delta > f32::EPSILON {
        (angle - previous_angle) / delta
    } else {
        0.0
    };
    (axis, angle, speed)
}

fn source_inertial_bones(
    target: &Pose,
    previous: &Pose,
    previous_previous: &Pose,
    delta: f32,
) -> Vec<SourceInertialBone> {
    target
        .paths
        .keys()
        .copied()
        .filter_map(|bone| {
            let target_bone = target.get_bone(bone)?;
            let previous_bone = previous.get_bone(bone)?;
            let previous_previous_bone = previous_previous.get_bone(bone)?;
            let (translation_direction, translation_magnitude, translation_speed) =
                vector_difference(
                    target_bone.translation,
                    previous_bone.translation,
                    previous_previous_bone.translation,
                    delta,
                );
            let (rotation_axis, rotation_angle, rotation_speed) = rotation_difference(
                target_bone.rotation,
                previous_bone.rotation,
                previous_previous_bone.rotation,
                delta,
            );
            let (scale_direction, scale_magnitude, scale_speed) = vector_difference(
                target_bone.scale,
                previous_bone.scale,
                previous_previous_bone.scale,
                delta,
            );
            Some(SourceInertialBone {
                bone,
                translation_direction,
                translation_magnitude,
                translation_speed,
                rotation_axis,
                rotation_angle,
                rotation_speed,
                scale_direction,
                scale_magnitude,
                scale_speed,
            })
        })
        .collect()
}

fn apply_source_inertialization(target: &Pose, state: &SourceInertializationState) -> Pose {
    let mut output = target.clone();
    for difference in &state.bones {
        let Some(index) = output.paths.get(&difference.bone).copied() else {
            continue;
        };
        let Some(bone) = output.bones.get_mut(index) else {
            continue;
        };
        if let Some(translation) = bone.translation {
            bone.translation = Some(
                translation
                    + difference.translation_direction
                        * source_inertial_float(
                            difference.translation_magnitude,
                            difference.translation_speed,
                            state.elapsed,
                            state.duration,
                        ),
            );
        }
        if let Some(rotation) = bone.rotation
            && difference.rotation_angle > f32::EPSILON
        {
            let angle = source_inertial_float(
                difference.rotation_angle,
                difference.rotation_speed,
                state.elapsed,
                state.duration,
            );
            bone.rotation = Some(
                (Quat::from_axis_angle(difference.rotation_axis, angle) * rotation).normalize(),
            );
        }
        if let Some(scale) = bone.scale {
            bone.scale = Some(
                scale
                    + difference.scale_direction
                        * source_inertial_float(
                            difference.scale_magnitude,
                            difference.scale_speed,
                            state.elapsed,
                            state.duration,
                        ),
            );
        }
    }
    output.normalize_quat()
}

fn begin_source_inertialization(
    state: &mut SourceInertializationState,
    target: &Pose,
    request_duration: f32,
) {
    let Some(previous) = state.previous.clone() else {
        return;
    };
    let previous_previous = state
        .previous_previous
        .clone()
        .unwrap_or_else(|| previous.clone());
    let applied_deficit = if state.active && state.deficit > 0.0 {
        (state.duration - state.elapsed).max(0.0)
    } else {
        0.0
    };
    if state.active {
        state.deficit = (state.duration - state.elapsed).max(0.0);
    }
    state.duration = (request_duration.max(0.0) - applied_deficit).max(0.0);
    state.elapsed = 0.0;
    state.bones =
        source_inertial_bones(target, &previous, &previous_previous, state.previous_delta);
    state.active = state.duration > f32::EPSILON && !state.bones.is_empty();
}

fn record_source_inertial_history(state: &mut SourceInertializationState, pose: &Pose, delta: f32) {
    state.previous_previous = state.previous.replace(pose.clone());
    state.previous_delta = delta;
}

fn became_relevant(previous: Option<u64>, frame: u64) -> bool {
    previous.is_some_and(|last| frame > last.saturating_add(1))
}

impl NodeLike for InertializationPassthroughNode {
    fn display_name(&self) -> String {
        "Inertialization".to_owned()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        if self.internalize_time {
            ctx.set_duration_fwd(None);
        } else {
            ctx.set_duration_fwd(ctx.duration_back("source_time")?);
        }
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let time_update = if self.internalize_time {
            context_time_update(&ctx)?
        } else {
            ctx.time_update_fwd()?
        };
        if !self.internalize_time {
            ctx.set_time_update_back("source_time", time_update.clone());
        }
        let source_pose = ctx.data_back("Source")?.into_pose()?;
        let mut source_context = pose_input_context_state(&ctx, "Source").unwrap_or_default();
        let context_curves = if self.capture_context {
            resolve_named_context_curves(&ctx, "Source", &self.curve_names, self.missing_curve_value)?
        } else { default() };
        for curve in &self.curve_names {
            let value = if self.capture_context {
                *context_curves.get(curve).ok_or(GraphError::MissingStateValue)?
            } else { ctx.data_back(format!("source_{curve}"))?.as_f32()? };
            source_context.curves.insert(curve.clone(), value);
        }
        let delta = source_delta(&time_update);
        let request_epoch = optional_global_f32(&ctx, &self.request_epoch_key);
        let request_duration = optional_global_f32(&ctx, &self.request_duration_key);
        let frame = ctx.graph_context.frame_index();
        let graph_request = source_context.inertialization_request.take().filter(|request| request.frame == frame);
        let pose = {
            let state = ctx.state_mut_or_else(SourceInertializationState::default)?;
            if self.reset_on_becoming_relevant && became_relevant(state.last_update_frame, frame) {
                state.previous = None;
                state.previous_previous = None;
                state.previous_delta = 0.0;
                state.duration = 0.0;
                state.elapsed = 0.0;
                state.deficit = 0.0;
                state.active = false;
                state.bones.clear();
                state.previous_curves.clear();
                state.curve_differences.clear();
                state.context = default();
            }
            state.last_update_frame = Some(frame);
            let mut duration = graph_request.map(|request| request.duration);
            if request_epoch > state.last_request_epoch + f32::EPSILON {
                state.last_request_epoch = request_epoch;
                duration = Some(duration.map_or(request_duration, |value| value.min(request_duration)));
            }
            if let Some(duration) = duration {
                let has_history = state.previous.is_some();
                begin_source_inertialization(state, &source_pose, duration);
                state.curve_differences.clear();
                if has_history {
                    state.curve_differences = source_context.curves.keys().chain(state.context.curves.keys())
                        .filter(|name| !self.filtered_curves.contains(name))
                        .map(|name| {
                            let target = source_context.curves.get(name).copied().unwrap_or(0.0);
                            let difference = state.context.curves.get(name).copied().unwrap_or(0.0) - target;
                            let derivative = if state.previous_delta > 1e-4 {
                                (difference - target - state.previous_curves.get(name).copied().unwrap_or(0.0)) / state.previous_delta
                            } else { 0.0 };
                            (name.clone(), (difference, derivative))
                        }).collect();
                    state.active |= state.duration > f32::EPSILON && state.curve_differences.values()
                        .any(|(difference, _)| difference.abs() > f32::EPSILON);
                }
            }
            let output = if state.active {
                state.elapsed += delta;
                let output = apply_source_inertialization(&source_pose, state);
                for (name, (difference, derivative)) in &state.curve_differences {
                    *source_context.curves.entry(name.clone()).or_default() +=
                        source_inertial_float(*difference, *derivative, state.elapsed, state.duration);
                }
                if state.elapsed >= state.duration {
                    state.active = false;
                    state.deficit = 0.0;
                } else {
                    state.deficit = (state.deficit - delta).max(0.0);
                }
                output
            } else {
                source_pose
            };
            record_source_inertial_history(state, &output, delta);
            state.previous_curves = state.context.curves.clone();
            output
        };
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd(self.output.clone(), pose);
        let context = source_context;
        for curve in &self.curve_names {
            if self.expose_output(curve) {
                ctx.set_data_fwd(curve.clone(), context.curves[curve]);
            }
        }
        ctx.state_mut_or_else(SourceInertializationState::default)?
            .context = context;
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Source", DataSpec::Pose)
            .add_output_data(self.output.clone(), DataSpec::Pose);
        if !self.internalize_time {
            ctx.add_input_time("source_time").add_output_time();
        }
        for curve in &self.curve_names {
            if !self.capture_context {
                ctx.add_input_data(format!("source_{curve}"), DataSpec::F32);
            }
            if self.expose_output(curve) {
                ctx.add_output_data(curve.clone(), DataSpec::F32);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_animation_graph::core::pose::BonePose;

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<InertializationPassthroughNode>();
        assert!(
            registry
                .get(std::any::TypeId::of::<InertializationPassthroughNode>())
                .unwrap()
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }

    #[test]
    fn source_request_preserves_the_previous_pose_then_converges_to_target() {
        let bone = BoneId::default();
        let mut previous = Pose::default();
        previous.add_bone(
            BonePose::from_transform(Transform::from_rotation(Quat::from_rotation_y(
                std::f32::consts::FRAC_PI_2,
            ))),
            bone,
        );
        let mut target = Pose::default();
        target.add_bone(BonePose::from_transform(Transform::IDENTITY), bone);
        let mut state = SourceInertializationState {
            previous: Some(previous.clone()),
            previous_previous: Some(previous),
            ..default()
        };
        begin_source_inertialization(&mut state, &target, 0.2);
        let initial = apply_source_inertialization(&target, &state);
        let initial_rotation = initial.get_bone(bone).unwrap().rotation.unwrap();
        assert!(
            initial_rotation
                .dot(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))
                .abs()
                > 0.9999
        );
        state.elapsed = state.duration;
        let final_pose = apply_source_inertialization(&target, &state);
        let final_rotation = final_pose.get_bone(bone).unwrap().rotation.unwrap();
        assert!(final_rotation.dot(Quat::IDENTITY).abs() > 0.9999);
        state.active = true;
        state.duration = 0.5;
        state.elapsed = 0.1;
        state.deficit = 0.3;
        begin_source_inertialization(&mut state, &target, 0.5);
        assert!((state.duration - 0.1).abs() < 1e-6);
    }

    #[test]
    fn source_relevance_reset_requires_a_skipped_frame() {
        assert!(!became_relevant(None, 4));
        assert!(!became_relevant(Some(4), 5));
        assert!(became_relevant(Some(4), 6));
    }
}
