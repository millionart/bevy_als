use bevy::{asset::Handle, prelude::*};
use bevy_animation_graph::core::{
    animation_clip::GraphClip,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
};

use crate::extensions::global_f32_input::root_defaults_enabled;
use crate::extensions::source_instance::source_input_data;
use crate::extensions::source_sequence_player::sample_source_pose;

fn frame_to_time(frame: f32, frames_per_second: f64) -> f32 {
    (frame.max(0.0) as f64 / frames_per_second.max(f64::EPSILON)) as f32
}

fn step_sample_time(time: f32, frames_per_second: f64, duration: f32) -> f32 {
    let intervals = (duration as f64 * frames_per_second).round().max(1.0) as f32;
    let frame = (time * frames_per_second as f32).floor().clamp(0.0, intervals);
    (frame * (duration / intervals)).min(duration)
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceSequenceEvaluatorNode {
    pub clip: Handle<GraphClip>,
    pub curves: crate::extensions::als_curves::AlsPoseCurves,
    pub additive_reference: Option<Handle<GraphClip>>,
    pub additive_reference_position: f32,
    pub additive_mesh_space: bool,
    pub root_lock: Option<crate::extensions::source_sequence_player::SourceRootLock>,
    pub use_explicit_frame: bool,
    pub explicit_value: f32,
    pub explicit_value_key: String,
    pub explicit_value_pin: String,
    pub expose_explicit_value_pin: bool,
    pub frames_per_second: f64,
    pub step_interpolation: bool,
    pub should_loop: bool,
    pub teleport_to_explicit_time: bool,
    pub reinitialization_behavior: String,
}

impl Default for SourceSequenceEvaluatorNode {
    fn default() -> Self {
        Self {
            clip: default(),
            curves: default(),
            additive_reference: None,
            additive_reference_position: 0.0,
            additive_mesh_space: false,
            root_lock: None,
            use_explicit_frame: false,
            explicit_value: 0.0,
            explicit_value_key: default(),
            explicit_value_pin: "ExplicitTime".to_owned(),
            expose_explicit_value_pin: true,
            frames_per_second: 30.0,
            step_interpolation: false,
            should_loop: true,
            teleport_to_explicit_time: true,
            reinitialization_behavior: "ExplicitTime".to_owned(),
        }
    }
}

impl SourceSequenceEvaluatorNode {
    fn input_value(&self, ctx: &NodeContext) -> Result<f32, GraphError> {
        if !self.explicit_value_key.is_empty() {
            return match source_input_data(ctx, &self.explicit_value_key) {
                Some(value) => value.as_f32(),
                None if root_defaults_enabled(ctx) => Ok(self.explicit_value),
                None => Err(GraphError::GlobalInputDataMissing(self.explicit_value_key.clone())),
            };
        }
        if self.expose_explicit_value_pin {
            if let Ok(value) = ctx.data_back(self.explicit_value_pin.clone()) {
                return value.as_f32();
            }
        }
        Ok(self.explicit_value)
    }
}

impl NodeLike for SourceSequenceEvaluatorNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let Some(clip) = ctx
            .graph_context
            .resources
            .graph_clip_assets
            .get(&self.clip)
        else {
            ctx.set_data_fwd("Pose", DataValue::Pose(default()));
            return Ok(());
        };
        let value = self.input_value(&ctx)?;
        let time = if self.use_explicit_frame { frame_to_time(value, self.frames_per_second) } else { value }
            .clamp(0.0, clip.duration().max(0.0));
        let sample_time = if self.step_interpolation { step_sample_time(time, self.frames_per_second, clip.duration()) } else { time };
        let mut pose = sample_source_pose(&ctx, clip, sample_time, self.additive_reference.as_ref(), self.additive_reference_position, self.additive_mesh_space, self.root_lock)?;
        pose.timestamp = time;
        *ctx.state_mut_or_else(crate::extensions::AnimationContextState::default)? =
            crate::extensions::AnimationContextState {
                curves: self.curves.sample_all(time),
                curves_are_authoritative: true,
                ..default()
            };
        ctx.set_time(time);
        ctx.set_data_fwd("Pose", pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        if self.expose_explicit_value_pin {
            ctx.add_input_data(self.explicit_value_pin.clone(), DataSpec::F32);
        }
        ctx.add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Sequence Evaluator".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_sampling_matches_native_acl_frame_selection() {
        use bevy::animation::gltf_curves::SteppedKeyframeCurve;
        for (frames_per_second, intervals) in [(30.0_f64, 11), (24.0, 5), (30.0, 23), (60.0, 59), (30000.0 / 1001.0, 37)] {
            let duration = (intervals as f64 / frames_per_second) as f32;
            let frame_length = duration / intervals as f32;
            let curve = SteppedKeyframeCurve::new((0..=intervals).map(|frame| {
                ((frame as f32 * frame_length).min(duration), frame)
            })).unwrap();
            for frame in 0..=intervals {
                let native_time = ((frame as f64 / frames_per_second) as f32).clamp(0.0, duration);
                let native_frame = (native_time * frames_per_second as f32).floor().min(intervals as f32) as i32;
                let time = frame_to_time(frame as f32, frames_per_second).clamp(0.0, duration);
                assert_eq!(time, native_time);
                assert_eq!(curve.sample_clamped(step_sample_time(time, frames_per_second, duration)), native_frame,
                    "rate={frames_per_second}, intervals={intervals}, frame={frame}, time={time:?}");
                for time in [time.next_down().max(0.0), time, time.next_up().min(duration)] {
                    let native_frame = (time * frames_per_second as f32).floor().min(intervals as f32) as i32;
                    assert_eq!(curve.sample_clamped(step_sample_time(time, frames_per_second, duration)), native_frame);
                }
            }
        }
    }

    #[test]
    fn additive_sequence_nodes_preserve_sample_time_and_reference_space() {
        use bevy::animation::animated_field;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::EntityPath, animation_graph::{AnimationGraph, TimeUpdate}, animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime, graph_context_arena::GraphContextArena, new_context::GraphContext, system_resources::SystemResources},
            skeleton::Skeleton, state_machine::high_level::StateMachine, utils::testing::TestIoEnv,
        };
        fn query(In(handle): In<Handle<AnimationGraph>>, resources: SystemResources) {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let child = EntityPath::default().child("root").child("child").id();
            for position in [-0.5_f32, 0.0, 0.25, 0.75, 1.5] {
                let inputs = HashMap::from([("position".to_owned(), DataValue::F32(position)), ("frame".to_owned(), DataValue::F32(position * 60.0))]);
                let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                let outputs = graph.query_with_context(QueryOutputTime::Forced(TimeUpdate::Delta(0.02)), context.clone()).unwrap();
                for (name, mesh, reference_time, time) in [
                    ("player", true, 7.0/30.0, 0.75),
                    ("mesh_7", true, 7.0/30.0, position.clamp(0.0, 1.0)),
                    ("local_7", false, 7.0/30.0, position.clamp(0.0, 1.0)),
                    ("mesh_11", true, 11.0/30.0, position.clamp(0.0, 1.0)),
                    ("frame_11", true, 11.0/30.0, position.clamp(0.0, 1.0)),
                ] {
                    let pose = outputs[name].as_pose().unwrap();
                    assert!((pose.timestamp - time).abs() < 1e-6, "{name}: reference time leaked into sampled pose");
                    let bone = pose.get_bone(child).unwrap();
                    let target = Quat::from_rotation_x(-0.4 + 0.9 * time);
                    let reference = Quat::from_rotation_x(0.25);
                    let expected = if mesh {
                        (Quat::from_rotation_y(0.1 + 0.5 * time) * target) * (Quat::from_rotation_y(0.7) * reference).inverse()
                    } else { target * reference.inverse() };
                    let rotation = bone.rotation.unwrap();
                    assert!((rotation - expected).length().min((rotation + expected).length()) < 1e-5, "{name}: incorrect reference space");
                    assert!(bone.translation.unwrap().distance(Vec3::X * (2.0 + time - reference_time)) < 1e-5);
                    assert!(bone.scale.unwrap().distance(Vec3::splat(0.2)) < 1e-5);
                    if name != "player" {
                        let curves = crate::extensions::animation_context::graph_output_context_state(graph, &context, name).unwrap();
                        assert!((curves.curves[&crate::extensions::SourceCurveName::from("Aim")] - time).abs() < 1e-6);
                    }
                }
                for name in ["step_frame", "step_time"] {
                    let pose = outputs[name].as_pose().unwrap();
                    assert_eq!(pose.get_bone(child).unwrap().translation, Some(Vec3::X * 6.0));
                    assert_eq!(pose.timestamp, 0.2);
                    assert_eq!(crate::extensions::animation_context::graph_output_context_state(graph, &context, name).unwrap().curves[&crate::extensions::SourceCurveName::from("Aim")], 0.2);
                }
            }
        }
        let root = EntityPath::default().child("root");
        let child = root.child("child");
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root.id());
        skeleton.add_bone(root.clone(), Transform::IDENTITY, Transform::IDENTITY);
        skeleton.add_bone(child.clone(), Transform::IDENTITY, Transform::IDENTITY);
        let mut skeletons = Assets::<Skeleton>::default();
        let skeleton = skeletons.add(skeleton);
        let mut clips = Assets::<GraphClip>::default();
        let mut make_clip = |reference: bool| {
            let mut clip = bevy::animation::AnimationClip::default();
            let root_angles = if reference { [0.7,0.7] } else { [0.1,0.6] };
            let child_angles = if reference { [0.25,0.25] } else { [-0.4,0.5] };
            clip.add_curve_to_target(root.id().animation_target_id(), AnimatableCurve::new(animated_field!(Transform::rotation), UnevenSampleAutoCurve::new([(0.0,Quat::from_rotation_y(root_angles[0])),(1.0,Quat::from_rotation_y(root_angles[1]))]).unwrap()));
            clip.add_curve_to_target(child.id().animation_target_id(), AnimatableCurve::new(animated_field!(Transform::rotation), UnevenSampleAutoCurve::new([(0.0,Quat::from_rotation_x(child_angles[0])),(1.0,Quat::from_rotation_x(child_angles[1]))]).unwrap()));
            let offset = if reference { 0.0 } else { 2.0 };
            clip.add_curve_to_target(child.id().animation_target_id(), AnimatableCurve::new(animated_field!(Transform::translation), UnevenSampleAutoCurve::new([(0.0,Vec3::X * offset),(1.0,Vec3::X * (offset + 1.0))]).unwrap()));
            let scale = if reference { 1.0 } else { 1.2 };
            clip.add_curve_to_target(child.id().animation_target_id(), AnimatableCurve::new(animated_field!(Transform::scale), UnevenSampleAutoCurve::new([(0.0,Vec3::splat(scale)),(1.0,Vec3::splat(scale))]).unwrap()));
            clips.add(GraphClip::from_bevy_clip(clip, skeleton.clone(), default(), None))
        };
        let reference = make_clip(true);
        let target = make_clip(false);
        let mut graph = AnimationGraph::new();
        let mut step_clip = bevy::animation::AnimationClip::default();
        step_clip.add_curve_to_target(child.id().animation_target_id(), AnimatableCurve::new(animated_field!(Transform::translation),
            bevy::animation::gltf_curves::SteppedKeyframeCurve::new((0..12).map(|frame| (frame as f32 * ((11.0_f32 / 30.0) / 11.0), Vec3::X * frame as f32))).unwrap()));
        let step_clip = clips.add(GraphClip::from_bevy_clip(step_clip, skeleton.clone(), default(), None));
        for (name, use_explicit_frame, explicit_value) in [("step_frame", true, 6.0), ("step_time", false, 0.2)] {
            let evaluator = AnimationNode::new(name, SourceSequenceEvaluatorNode { clip: step_clip.clone(), use_explicit_frame,
                explicit_value, step_interpolation: true, curves:crate::extensions::als_curves::parse_curves(&serde_json::json!({"curves":[{"name":"Aim","times":[0.0,1.0],"values":[0.0,1.0],"interps":["LINEAR","LINEAR"]}]})).unwrap(), ..default() });
            graph.add_node(evaluator.clone());
            graph.add_output_data(name.to_owned(), DataSpec::Pose);
            graph.add_output_data_edge(evaluator.id, "Pose", name);
        }
        let player = AnimationNode::new("player", crate::extensions::SourceSequencePlayerNode { clip:target.clone(), additive_reference:Some(reference.clone()), additive_reference_position:7.0/30.0, additive_mesh_space:true, play_rate:0.0, start_position:0.75, loop_animation:false, expose_play_rate_pin:false, expose_start_position_pin:false, expose_loop_animation_pin:false, ..default() });
        graph.add_node(player.clone());
        graph.add_output_data("player".to_owned(), DataSpec::Pose);
        graph.add_output_data_edge(player.id, "Pose", "player");
        for (name, mesh, frame) in [("mesh_7",true,7.0),("local_7",false,7.0),("mesh_11",true,11.0),("frame_11",true,11.0)] {
            let evaluator = AnimationNode::new(name, SourceSequenceEvaluatorNode {
                clip:target.clone(), additive_reference:Some(reference.clone()), additive_reference_position:frame/30.0, additive_mesh_space:mesh,
                use_explicit_frame: name == "frame_11", frames_per_second: 60.0, explicit_value_key:if name == "frame_11" { "frame" } else { "position" }.to_owned(), curves:crate::extensions::als_curves::parse_curves(&serde_json::json!({"curves":[{"name":"Aim","times":[0.0,1.0],"values":[0.0,1.0],"interps":["LINEAR","LINEAR"]}]})).unwrap(), ..default()
            });
            graph.add_node(evaluator.clone());
            graph.add_output_data(name.to_owned(), DataSpec::Pose);
            graph.add_output_data_edge(evaluator.id, "Pose", name);
        }
        let mut graphs = Assets::<AnimationGraph>::default();
        let handle = graphs.add(graph);
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(clips);
        world.insert_resource(skeletons);
        world.insert_resource(Assets::<StateMachine>::default());
        world.run_system_once_with(query, handle).unwrap();
    }

    #[test]
    fn source_explicit_frame_uses_the_configured_source_rate() {
        assert!((frame_to_time(21.0, 30.0) - 0.7).abs() < 1e-6);
        assert!((frame_to_time(24.0, 30.0) - 0.8).abs() < 1e-6);
        assert!((frame_to_time(23.0, 30.0) - 23.0 / 30.0).abs() < 1e-6);
        assert_eq!(frame_to_time(15.0, 60.0), 0.25);
        assert!((frame_to_time(15.0, 30000.0 / 1001.0) - 0.5005).abs() < 1e-6);
        assert_eq!(frame_to_time(-1.0, 30.0), 0.0);
        assert_eq!(step_sample_time(0.2, 30.0, 0.0), 0.0);
    }

    #[test]
    fn bound_explicit_time_uses_current_instance_then_frame_input() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_graph::{AnimationGraph, GraphInputPin}, animation_node::AnimationNode,
            context::{deferred_gizmos::DeferredGizmos, graph_context_arena::GraphContextArena,
                new_context::GraphContext, system_resources::SystemResources},
            skeleton::Skeleton, state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        fn query(In(handle): In<Handle<AnimationGraph>>, resources: SystemResources) -> Vec<f32> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let node = graph.nodes.values().find(|node| node.name == "bound").unwrap();
            let evaluator = node.try_inner_downcast_ref::<SourceSequenceEvaluatorNode>().unwrap();
            let unbound = graph.nodes.values().find(|node| node.name == "unbound").unwrap();
            let unbound_evaluator = unbound.try_inner_downcast_ref::<SourceSequenceEvaluatorNode>().unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let inputs = HashMap::from([
                ("source_frame_input.draw_amount".to_owned(), DataValue::F32(0.55)),
                ("draw_amount".to_owned(), DataValue::F32(0.25)),
            ]);
            let mut values = Vec::new();
            for frame in 0..2 {
                let context = GraphContext::new(arena.get_toplevel_id(), graph, &mut arena,
                    &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs);
                let node_ctx = context.create_node_context(node.id);
                if frame == 0 {
                    crate::extensions::source_instance::set_source_property(&node_ctx, "draw_amount", DataValue::F32(0.75)).unwrap();
                }
                assert_eq!(node_ctx.data_back("ExplicitTime").unwrap().as_f32().unwrap(), 0.1);
                values.push(evaluator.input_value(&node_ctx).unwrap());
                drop(node_ctx);
                let unbound_ctx = context.create_node_context(unbound.id);
                assert_eq!(unbound_evaluator.input_value(&unbound_ctx).unwrap(), 0.1);
                drop(unbound_ctx);
                drop(context);
                arena.next_frame();
            }
            let mut empty_arena = GraphContextArena::new(handle.id());
            let empty = HashMap::new();
            let context = GraphContext::new(empty_arena.get_toplevel_id(), graph, &mut empty_arena,
                &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &empty);
            let node_ctx = context.create_node_context(node.id);
            assert!(matches!(evaluator.input_value(&node_ctx), Err(GraphError::GlobalInputDataMissing(key)) if key == "draw_amount"));
            drop(node_ctx);
            drop(context);
            let defaults = HashMap::from([(
                crate::extensions::global_f32_input::ROOT_DEFAULTS_ENABLED_KEY.to_owned(), DataValue::Bool(true),
            )]);
            let context = GraphContext::new(empty_arena.get_toplevel_id(), graph, &mut empty_arena,
                &resources, &env, Entity::PLACEHOLDER, &entities, &mut gizmos, &defaults);
            assert_eq!(evaluator.input_value(&context.create_node_context(node.id)).unwrap(), 0.4);
            values
        }
        let mut graph = AnimationGraph::new();
        graph.set_default_data(GraphInputPin::passthrough("pin"), DataValue::F32(0.1));
        let node = AnimationNode::new("bound", SourceSequenceEvaluatorNode {
            explicit_value: 0.4, explicit_value_key: "draw_amount".into(), ..default()
        });
        graph.add_node(node.clone());
        graph.add_input_data_edge(GraphInputPin::passthrough("pin"), node.id, "ExplicitTime");
        let unbound = AnimationNode::new("unbound", SourceSequenceEvaluatorNode::default());
        graph.add_node(unbound.clone());
        graph.add_input_data_edge(GraphInputPin::passthrough("pin"), unbound.id, "ExplicitTime");
        let mut graphs = Assets::<AnimationGraph>::default();
        let handle = graphs.add(graph);
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        assert_eq!(world.run_system_cached_with(query, handle).unwrap(), [0.75, 0.55]);
    }

    #[test]
    fn source_explicit_value_pin_name_can_be_preserved() {
        let node = SourceSequenceEvaluatorNode {
            explicit_value_pin: "ExplicitFrame".to_owned(),
            ..default()
        };
        assert_eq!(node.explicit_value_pin, "ExplicitFrame");
    }

    #[test]
    fn source_sequence_evaluator_can_hide_an_unexposed_frame_pin() {
        let node = SourceSequenceEvaluatorNode {
            expose_explicit_value_pin: false,
            ..default()
        };
        assert!(!node.expose_explicit_value_pin);
    }
}
