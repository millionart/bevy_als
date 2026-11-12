use bevy::{
    asset::Handle,
    math::{EulerRot, Quat, Vec3},
    prelude::VariableCurve,
    reflect::prelude::*,
};
use bevy_animation_graph::core::{
    animation_clip::{EntityPath, GraphClip},
    animation_graph::TimeUpdate,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, DataValue, events::EventQueue},
    errors::GraphError,
    event_track::sample_tracks,
    id::BoneId,
    pose::{Pose, RootMotionDelta, RootMotionMode},
};

use crate::extensions::root_motion_defaults::derive_default_root_bone;
use crate::extensions::source_compression::{sample_prepared_pose, sample_prepared_root};
use crate::extensions::source_sequence_player::{CurveSample, sample_curve, sample_pose};

pub const IN_TIME: &str = "time";
pub const OUT_POSE: &str = "pose";
pub const OUT_EVENTS: &str = "events";

#[derive(Reflect, Clone, Debug, Default)]
struct RmPrevState {
    time: f32,
    cached_root: Option<EntityPath>,
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
    pub loop_animation: bool,
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
            ctx.set_data_fwd(OUT_EVENTS, EventQueue::default());
            return Ok(());
        };
        let clip_duration = clip.duration();

        let time_update = ctx.time_update_fwd()?;
        let clamped_prev = {
            let prev = ctx.state_mut_or_else(RmPrevState::default)?;
            prev.time.clamp(0.0, clip_duration)
        };
        let requested_time = time_update
            .partial_update_with_tracks(clamped_prev, &clip.event_tracks)
            .unwrap_or(clamped_prev);
        let current_time = if self.loop_animation && clip_duration > f32::EPSILON
            && matches!(&time_update, TimeUpdate::Delta(_)) {
            requested_time.rem_euclid(clip_duration)
        } else { requested_time.clamp(0.0, clip_duration) };
        let events =
            EventQueue::with_events(sample_tracks(clip.event_tracks.values(), current_time));

        let mut out_pose = sample_prepared_pose(clip, current_time)
            .map_or_else(|| sample_pose(clip, current_time), |(pose, _)| pose);

        if self.root_motion_mode != RootMotionMode::Disabled {
            let Some(skeleton) = ctx
                .graph_context
                .resources
                .skeleton_assets
                .get(&clip.skeleton)
            else {
                ctx.set_data_fwd(OUT_POSE, DataValue::Pose(out_pose));
                ctx.set_data_fwd(OUT_EVENTS, events);
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
                        && let Ok(prev) = ctx.state_mut::<RmPrevState>()
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
                let root_curves = clip.curves.get(&target_id).map_or(&[][..], Vec::as_slice);
                let root_at = |time| sample_prepared_root(clip, root_bone_id, time)
                    .map_or_else(|| sample_root(root_curves, time), |root| (root.translation, root.rotation));
                let rest_local = skeleton.default_transforms(root_bone_id).map(|dt| dt.local);
                let rest_translation = rest_local.map(|t| t.translation).unwrap_or(Vec3::ZERO);
                let rest_rotation = rest_local.map(|t| t.rotation).unwrap_or(Quat::IDENTITY);
                let mut delta_translation = Vec3::ZERO;
                let mut delta_rotation = Quat::IDENTITY;
                let mut ground_yaw_delta = Quat::IDENTITY;
                let mut accumulate = |from, to| {
                    let (start_tr, start_rot) = root_at(from);
                    let (end_tr, end_rot) = root_at(to);
                    delta_translation += delta_rotation * (start_rot.inverse() * (end_tr - start_tr));
                    delta_rotation = (delta_rotation * start_rot.inverse() * end_rot).normalize();
                    if matches!(self.root_motion_mode, RootMotionMode::GroundPlane) {
                        let start_yaw = Quat::from_rotation_y(
                            (rest_rotation.inverse() * start_rot).to_euler(EulerRot::YXZ).0);
                        let end_yaw = Quat::from_rotation_y(
                            (rest_rotation.inverse() * end_rot).to_euler(EulerRot::YXZ).0);
                        ground_yaw_delta = (ground_yaw_delta * start_yaw.inverse() * end_yaw).normalize();
                    }
                };
                if self.loop_animation && clip_duration > f32::EPSILON
                    && let TimeUpdate::Delta(delta) = &time_update {
                    let forward = *delta >= 0.0;
                    let mut remaining = delta.abs();
                    let mut from = clamped_prev;
                    while remaining > f32::EPSILON {
                        let boundary = if forward { clip_duration - from } else { from };
                        if boundary <= f32::EPSILON {
                            from = if forward { 0.0 } else { clip_duration };
                            continue;
                        }
                        let step = remaining.min(boundary);
                        let to = if forward { from + step } else { from - step };
                        accumulate(from, to);
                        remaining -= step;
                        from = if forward && remaining > f32::EPSILON && step >= boundary { 0.0 }
                            else if !forward && remaining > f32::EPSILON && step >= boundary { clip_duration }
                            else { to };
                    }
                } else {
                    accumulate(clamped_prev, current_time);
                }

                match self.root_motion_mode {
                    RootMotionMode::Full => {
                        if let Some(bone_idx) = out_pose.paths.get(&root_bone_id).copied() {
                            out_pose.bones[bone_idx].translation = Some(rest_translation);
                            out_pose.bones[bone_idx].rotation = Some(rest_rotation);
                        }
                    }
                    RootMotionMode::GroundPlane => {
                        delta_translation.y = 0.0;
                        delta_rotation = ground_yaw_delta;
                        if let Some(bone_idx) = out_pose.paths.get(&root_bone_id).copied()
                            && let Some(ref mut t) = out_pose.bones[bone_idx].translation
                        {
                            t.x = rest_translation.x;
                            t.z = rest_translation.z;
                        }
                        if let Some(bone_idx) = out_pose.paths.get(&root_bone_id).copied()
                            && let Some(rotation) = out_pose.bones[bone_idx].rotation
                        {
                            let relative = rest_rotation.inverse() * rotation;
                            let yaw = Quat::from_rotation_y(relative.to_euler(EulerRot::YXZ).0);
                            out_pose.bones[bone_idx].rotation = Some(rest_rotation * yaw.inverse() * relative);
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
        ctx.set_data_fwd(OUT_EVENTS, events);
        ctx.set_data_fwd(OUT_POSE, DataValue::Pose(out_pose));
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_time(IN_TIME);
        ctx.add_output_data(OUT_POSE, DataSpec::Pose)
            .add_output_data(OUT_EVENTS, DataSpec::EventQueue)
            .add_output_time();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{animation::animated_field, prelude::*};
    use bevy_animation_graph::core::{
        animation_graph::{AnimationGraph, TimeUpdate}, animation_node::AnimationNode,
        context::{deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
            graph_context_arena::GraphContextArena, new_context::GraphContext,
            system_resources::SystemResources}, pose::{BonePose, RootMotionMode}, skeleton::Skeleton,
        utils::testing::TestIoEnv,
    };

    #[test]
    fn root_motion_translation_uses_the_previous_root_rotation_in_both_directions() {
        fn query(In((handle, updates)): In<(Handle<AnimationGraph>, Vec<TimeUpdate>)>, resources: SystemResources) -> Vec<RootMotionDelta> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = default();
            let inputs = default();
            let env = TestIoEnv::default();
            updates.into_iter().map(|update| {
                graph.query_with_context(QueryOutputTime::Forced(update),
                    GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources,
                        &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs)).unwrap()
                    ["Pose"].as_pose().unwrap().root_motion.clone().unwrap()
            }).collect()
        }

        let root = EntityPath::default().child("root");
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<bevy_animation_graph::core::state_machine::high_level::StateMachine>>();
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root.id());
        let reference = Transform::from_rotation(Quat::from_rotation_y(0.3));
        skeleton.add_bone(root.clone(), reference, reference);
        let skeleton = world.resource_mut::<Assets<Skeleton>>().add(skeleton);
        for rotating in [false, true] {
            let mut clip = bevy::animation::AnimationClip::default();
            clip.add_curve_to_target(root.id().animation_target_id(), AnimatableCurve::new(
                animated_field!(Transform::translation), UnevenSampleAutoCurve::new([
                    (0.0, Vec3::ZERO), (0.5, Vec3::X), (1.0, Vec3::X + Vec3::Z),
                ]).unwrap()));
            let rotations = if rotating { [0.5, 0.5, 1.0] } else { [0.0; 3] };
            clip.add_curve_to_target(root.id().animation_target_id(), AnimatableCurve::new(
                animated_field!(Transform::rotation), UnevenSampleAutoCurve::new([
                    (0.0, Quat::from_rotation_y(std::f32::consts::PI * rotations[0])),
                    (0.5, Quat::from_rotation_y(std::f32::consts::PI * rotations[1])),
                    (1.0, Quat::from_rotation_y(std::f32::consts::PI * rotations[2])),
                ]).unwrap()));
            let clip = world.resource_mut::<Assets<GraphClip>>().add(
                GraphClip::from_bevy_clip(clip, skeleton.clone(), default(), None));
            for looping in [true, false] {
                let node = AnimationNode::new("root clip", RootMotionClipNode {
                    clip: clip.clone(), root_motion_mode: RootMotionMode::Full,
                    root_motion_bone: Some(root.clone()), loop_animation: looping,
                });
                let mut graph = AnimationGraph::new();
                graph.add_node(node.clone());
                graph.add_output_data("Pose".into(), DataSpec::Pose);
                graph.add_output_data_edge(node.id, "pose", "Pose");
                graph.add_output_time();
                graph.add_output_pose_edge(node.id);
                let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
                let updates = if looping {
                    vec![TimeUpdate::Absolute(0.8), TimeUpdate::Delta(0.4), TimeUpdate::Delta(-0.4),
                        TimeUpdate::Delta(2.4), TimeUpdate::Delta(-2.4)]
                } else {
                    vec![TimeUpdate::Absolute(0.5), TimeUpdate::Absolute(1.0), TimeUpdate::Absolute(0.5)]
                };
                let deltas = world.run_system_cached_with(query, (graph, updates)).unwrap();
                let half = Quat::from_rotation_y(if rotating { std::f32::consts::FRAC_PI_2 } else { 0.0 });
                let end = Quat::from_rotation_y(if rotating { std::f32::consts::PI } else { 0.0 });
                if looping {
                    let at_eight = half.slerp(end, 0.6);
                    let first = at_eight.inverse() * Vec3::Z * 0.4;
                    let turn = at_eight.inverse() * end;
                    let second = half.inverse() * Vec3::X * 0.4;
                    let forward = first + turn * second;
                    let reverse = half.inverse() * -Vec3::X * 0.4 + end.inverse() * -Vec3::Z * 0.4;
                    assert!(deltas[1].translation.distance(forward) < 1e-5, "forward wrap={:?}", deltas[1]);
                    assert!(deltas[2].translation.distance(reverse) < 1e-5, "reverse wrap={:?}", deltas[2]);
                    let segment = |(start_tr, start_rot): (Vec3, Quat), (end_tr, end_rot): (Vec3, Quat)| {
                        Transform::from_translation(start_rot.inverse() * (end_tr - start_tr))
                            .with_rotation(start_rot.inverse() * end_rot)
                    };
                    let start = (Vec3::ZERO, half);
                    let early = (Vec3::X * 0.4, half);
                    let late = (Vec3::X + Vec3::Z * 0.6, at_eight);
                    let finish = (Vec3::X + Vec3::Z, end);
                    let forward_multi = segment(late, finish) * segment(start, finish)
                        * segment(start, finish) * segment(start, early);
                    let reverse_multi = segment(early, start) * segment(finish, start)
                        * segment(finish, start) * segment(finish, late);
                    assert!(deltas[3].translation.distance(forward_multi.translation) < 1e-5);
                    assert!(deltas[4].translation.distance(reverse_multi.translation) < 1e-5);
                    assert!(1.0 - deltas[3].rotation.dot(forward_multi.rotation).abs() < 1e-5);
                    assert!(1.0 - deltas[4].rotation.dot(reverse_multi.rotation).abs() < 1e-5);
                } else {
                    assert!(deltas[1].translation.distance(half.inverse() * Vec3::Z) < 1e-5, "forward={:?}", deltas[1]);
                    assert!(deltas[2].translation.distance(end.inverse() * -Vec3::Z) < 1e-5, "reverse={:?}", deltas[2]);
                    assert!(1.0 - deltas[1].rotation.dot(half.inverse() * end).abs() < 1e-5);
                }
            }
        }
    }

    #[test]
    fn ground_plane_root_motion_removes_consumed_yaw_but_keeps_tilt_and_height() {
        fn query(In(handle): In<Handle<AnimationGraph>>, resources: SystemResources) -> Vec<(RootMotionDelta, BonePose)> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = default();
            let inputs = default();
            let env = TestIoEnv::default();
            [0.5, 1.0].into_iter().map(|time| {
                let output = graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Absolute(time)),
                    GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources,
                        &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs)).unwrap();
                let pose = output["Pose"].as_pose().unwrap();
                (pose.root_motion.clone().unwrap(),
                    pose.get_bone(EntityPath::default().child("root").id()).unwrap().clone())
            }).collect()
        }

        let root = EntityPath::default().child("root");
        let reference = Transform::from_translation(Vec3::Y * 0.1)
            .with_rotation(Quat::from_rotation_y(0.2));
        let tilt = Quat::from_rotation_x(0.3) * Quat::from_rotation_z(-0.2);
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<bevy_animation_graph::core::state_machine::high_level::StateMachine>>();
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root.id());
        skeleton.add_bone(root.clone(), reference, reference);
        let skeleton = world.resource_mut::<Assets<Skeleton>>().add(skeleton);
        for yaw in [0.0, std::f32::consts::FRAC_PI_2] {
            let mut clip = bevy::animation::AnimationClip::default();
            clip.add_curve_to_target(root.id().animation_target_id(), AnimatableCurve::new(
                animated_field!(Transform::translation), UnevenSampleAutoCurve::new([
                    (0.0, reference.translation), (1.0, Vec3::new(1.0, 0.5, 2.0)),
                ]).unwrap()));
            clip.add_curve_to_target(root.id().animation_target_id(), AnimatableCurve::new(
                animated_field!(Transform::rotation), UnevenSampleAutoCurve::new([
                    (0.0, reference.rotation),
                    (1.0, reference.rotation * Quat::from_rotation_y(yaw) * tilt),
                ]).unwrap()));
            let clip = world.resource_mut::<Assets<GraphClip>>().add(
                GraphClip::from_bevy_clip(clip, skeleton.clone(), default(), None));
            let raw_rotations = {
                let clips = world.resource::<Assets<GraphClip>>();
                let curves = &clips.get(&clip).unwrap().curves[&root.id().animation_target_id()];
                [0.5, 1.0].map(|time| sample_root(curves, time).1)
            };
            let node = AnimationNode::new("ground root", RootMotionClipNode {
                clip: clip.clone(), root_motion_mode: RootMotionMode::GroundPlane,
                root_motion_bone: Some(root.clone()), loop_animation: false,
            });
            let mut graph = AnimationGraph::new();
            graph.add_node(node.clone());
            graph.add_output_data("Pose".into(), DataSpec::Pose);
            graph.add_output_data_edge(node.id, "pose", "Pose");
            graph.add_output_time();
            graph.add_output_pose_edge(node.id);
            let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
            let frames = world.run_system_cached_with(query, graph).unwrap();
            let (motion, pose) = &frames[1];
            let expected_yaw = Quat::from_rotation_y(yaw);
            let previous_yaw = (reference.rotation.inverse() * raw_rotations[0]).to_euler(EulerRot::YXZ).0;
            let expected_step = Quat::from_rotation_y(yaw - previous_yaw);
            let expected_pose = reference.rotation * tilt;
            assert!(1.0 - motion.rotation.dot(expected_step).abs() < 1e-5, "yaw={yaw}: motion={motion:?}");
            assert!(1.0 - pose.rotation.unwrap().dot(expected_pose).abs() < 1e-5, "yaw={yaw}: pose={pose:?}");
            let mut actor = Quat::IDENTITY;
            for ((delta, pose), raw_rotation) in frames.iter().zip(raw_rotations) {
                actor = (actor * delta.rotation).normalize();
                let combined = actor * pose.rotation.unwrap();
                assert!(1.0 - combined.dot(raw_rotation).abs() < 1e-5,
                    "yaw={yaw}: accumulated root rotation doubled or drifted");
            }
            assert!(1.0 - actor.dot(expected_yaw).abs() < 1e-5);
            assert_eq!(pose.translation.unwrap(), Vec3::Y * 0.5);
            assert_eq!(motion.translation.y, 0.0);
        }
    }
}
