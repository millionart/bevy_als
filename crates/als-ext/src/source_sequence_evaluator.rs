use bevy::{asset::Handle, prelude::*};
use bevy_animation_graph::core::{
    animation_clip::GraphClip,
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::{DataSpec, DataValue},
    errors::GraphError,
};

use crate::global_f32_input::root_defaults_enabled;
use crate::source_sequence_player::sample_source_pose;

fn frame_to_time(frame: f32, frames_per_second: f32) -> f32 {
    frame.max(0.0) / frames_per_second.max(f32::EPSILON)
}

#[derive(Reflect, Clone, Debug)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceSequenceEvaluatorNode {
    pub clip: Handle<GraphClip>,
    pub curves: crate::als_curves::AlsPoseCurves,
    pub additive_reference: Option<Handle<GraphClip>>,
    pub additive_reference_position: f32,
    pub additive_mesh_space: bool,
    pub root_lock: Option<crate::source_sequence_player::SourceRootLock>,
    pub use_explicit_frame: bool,
    pub explicit_value: f32,
    pub explicit_value_key: String,
    pub explicit_value_pin: String,
    pub expose_explicit_value_pin: bool,
    pub frames_per_second: f32,
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
            should_loop: true,
            teleport_to_explicit_time: true,
            reinitialization_behavior: "ExplicitTime".to_owned(),
        }
    }
}

impl SourceSequenceEvaluatorNode {
    fn input_value(&self, ctx: &NodeContext) -> Result<f32, GraphError> {
        if self.expose_explicit_value_pin {
            if let Ok(value) = ctx.data_back(self.explicit_value_pin.clone()) {
                return value.as_f32();
            }
        }
        if self.explicit_value_key.is_empty() {
            return Ok(self.explicit_value);
        }
        match ctx
            .graph_context
            .global_input_data
            .get(&self.explicit_value_key)
        {
            Some(value) => value.as_f32(),
            None if root_defaults_enabled(ctx) => Ok(self.explicit_value),
            None => Err(GraphError::GlobalInputDataMissing(
                self.explicit_value_key.clone(),
            )),
        }
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
        let pose = sample_source_pose(&ctx, clip, time, self.additive_reference.as_ref(), self.additive_reference_position, self.additive_mesh_space, self.root_lock)?;
        *ctx.state_mut_or_else(crate::AnimationContextState::default)? =
            crate::AnimationContextState {
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
                        let curves = crate::animation_context::graph_output_context_state(graph, &context, name).unwrap();
                        assert!((curves.curves["Aim"] - time).abs() < 1e-6);
                    }
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
        let player = AnimationNode::new("player", crate::SourceSequencePlayerNode { clip:target.clone(), additive_reference:Some(reference.clone()), additive_reference_position:7.0/30.0, additive_mesh_space:true, play_rate:0.0, start_position:0.75, loop_animation:false, expose_play_rate_pin:false, expose_start_position_pin:false, expose_loop_animation_pin:false, ..default() });
        graph.add_node(player.clone());
        graph.add_output_data("player".to_owned(), DataSpec::Pose);
        graph.add_output_data_edge(player.id, "Pose", "player");
        for (name, mesh, frame) in [("mesh_7",true,7.0),("local_7",false,7.0),("mesh_11",true,11.0),("frame_11",true,11.0)] {
            let evaluator = AnimationNode::new(name, SourceSequenceEvaluatorNode {
                clip:target.clone(), additive_reference:Some(reference.clone()), additive_reference_position:frame/30.0, additive_mesh_space:mesh,
                use_explicit_frame: name == "frame_11", frames_per_second: 60.0, explicit_value_key:if name == "frame_11" { "frame" } else { "position" }.to_owned(), curves:crate::als_curves::parse_curves(&serde_json::json!({"curves":[{"name":"Aim","times":[0.0,1.0],"values":[0.0,1.0],"interps":["LINEAR","LINEAR"]}]})).unwrap(), ..default()
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
