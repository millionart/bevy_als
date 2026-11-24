use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
    pose::{BonePose, Pose},
    skeleton::Skeleton,
};

use crate::extensions::animation_context::context_time_update;

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct LocalRefPoseNode {
    pub internalize_time: bool,
}

impl NodeLike for LocalRefPoseNode {
    fn display_name(&self) -> String {
        "ALS Local Reference Pose".into()
    }

    fn duration(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        ctx.set_duration_fwd(None);
        Ok(())
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let time_update = if self.internalize_time {
            context_time_update(&ctx)?
        } else {
            ctx.time_update_fwd()?
        };
        let time = time_update
            .partial_update_basic(ctx.prev_time())
            .unwrap_or_else(|| ctx.prev_time());
        let pose = Pose {
            timestamp: time,
            ..default()
        };
        ctx.set_time(time);
        ctx.set_data_fwd("pose", pose);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data("pose", DataSpec::Pose);
        if !self.internalize_time {
            ctx.add_output_time();
        }
        Ok(())
    }
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct SourceLocalRefPoseNode {
    pub reference_skeleton: Handle<Skeleton>,
}

impl NodeLike for SourceLocalRefPoseNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let skeleton = ctx
            .graph_context
            .resources
            .skeleton_assets
            .get(&self.reference_skeleton)
            .ok_or(GraphError::SkeletonMissing(ctx.node_id))?;
        let timestamp = context_time_update(&ctx)?
            .partial_update_basic(ctx.prev_time())
            .unwrap_or_else(|| ctx.prev_time());
        let mut pose = Pose {
            skeleton: self.reference_skeleton.clone(),
            timestamp,
            ..default()
        };
        if !crate::extensions::sync_group_phase::source_sync_preview(&ctx) {
            let omitted = crate::extensions::source_gameplay_tag_input::active_omitted_bones(self.reference_skeleton.id());
            for bone in skeleton.iter_bones() {
                if omitted.as_ref().is_none_or(|omitted| !omitted.contains(&bone))
                    && let Some(transform) = skeleton.default_transforms(bone)
                {
                    pose.add_bone(BonePose::from_transform(transform.local), bone);
                }
            }
        }
        ctx.set_time(timestamp);
        ctx.set_data_fwd("Pose", pose);
        *ctx.state_mut_or_else(crate::extensions::AnimationContextState::default)? =
            crate::extensions::AnimationContextState {
                curves_are_authoritative: true,
                ..default()
            };
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Source Local Reference Pose".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_reference_pose_outputs_every_bone_and_empty_curve_context() {
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            animation_clip::{EntityPath, GraphClip},
            animation_graph::{AnimationGraph, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                system_resources::SystemResources,
            },
            edge_data::DataValue,
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        fn query(
            In((handle, preview)): In<(Handle<AnimationGraph>, bool)>,
            resources: SystemResources,
        ) -> Result<(Pose, Option<crate::extensions::AnimationContextState>), GraphError> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let mut inputs = HashMap::new();
            if preview {
                inputs.insert(
                    crate::extensions::sync_group_phase::SOURCE_SYNC_RESOLVED.to_owned(),
                    DataValue::Bool(false),
                );
            }
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let context = GraphContext::new(
                arena.get_toplevel_id(),
                graph,
                &mut arena,
                &resources,
                &env,
                Entity::PLACEHOLDER,
                &entities,
                &mut gizmos,
                &inputs,
            );
            let pose = graph
                .query_with_context(
                    QueryOutputTime::Forced(TimeUpdate::Delta(0.02)),
                    context.clone(),
                )?
                .remove("Pose")
                .unwrap()
                .into_pose()
                .unwrap();
            let curves = crate::extensions::animation_context::graph_output_context_state(
                graph, &context, "Pose",
            );
            Ok((pose, curves))
        }
        let mut world = World::new();
        world.init_resource::<Assets<AnimationGraph>>();
        world.init_resource::<Assets<GraphClip>>();
        world.init_resource::<Assets<Skeleton>>();
        world.init_resource::<Assets<StateMachine>>();
        let root = EntityPath::default().child("root");
        let child = root.child("end");
        let mut source_skeleton = Skeleton::default();
        source_skeleton.set_root(root.id());
        let local = Transform::from_xyz(1.0, 2.0, 3.0)
            .with_rotation(Quat::from_rotation_y(0.7))
            .with_scale(Vec3::new(2.0, 3.0, 4.0));
        source_skeleton.add_bone(root.clone(), local, local);
        source_skeleton.add_bone(child.clone(), Transform::IDENTITY, local);
        let skeleton = world
            .resource_mut::<Assets<Skeleton>>()
            .add(source_skeleton);
        let mut graph = AnimationGraph::new();
        let node = AnimationNode::new(
            "reference",
            SourceLocalRefPoseNode {
                reference_skeleton: skeleton.clone(),
            },
        );
        graph.add_node(node.clone());
        graph.add_output_data("Pose".into(), DataSpec::Pose);
        graph.add_output_data_edge(node.id, "Pose", "Pose");
        let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
        let (pose, curves) = world
            .run_system_cached_with(query, (graph.clone(), false))
            .unwrap()
            .unwrap();
        assert_eq!(pose.skeleton, skeleton);
        assert_eq!(pose.bones.len(), 2);
        assert_eq!(
            pose.get_bone(root.id()).unwrap().translation,
            Some(local.translation)
        );
        assert_eq!(
            pose.get_bone(root.id()).unwrap().rotation,
            Some(local.rotation)
        );
        assert_eq!(pose.get_bone(root.id()).unwrap().scale, Some(local.scale));
        assert_eq!(
            pose.get_bone(child.id()).unwrap().translation,
            Some(Vec3::ZERO)
        );
        assert_eq!(pose.timestamp, 0.02);
        let mut curves = curves.unwrap_or_default();
        crate::extensions::animation_context::merge_linked_input_context(
            &mut curves,
            &crate::extensions::AnimationContextState {
                curves: [("FixtureCurve".into(), 1.0)].into(),
                ..default()
            },
        );
        assert!(curves.curves.is_empty());
        assert!(curves.relevant_asset.is_none() && curves.sync_players.is_empty());
        let (preview, context) = world
            .run_system_cached_with(query, (graph.clone(), true))
            .unwrap()
            .unwrap();
        assert_eq!(preview.timestamp, pose.timestamp);
        assert_eq!(preview.skeleton, pose.skeleton);
        assert!(context.unwrap().curves_are_authoritative);
        assert!(preview.bones.is_empty());
        world
            .resource_mut::<Assets<Skeleton>>()
            .remove(skeleton.id());
        let error = world
            .run_system_cached_with(query, (graph, false))
            .unwrap()
            .unwrap_err();
        assert!(matches!(error, GraphError::SkeletonMissing(_)));
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<LocalRefPoseNode>();
        registry.register::<SourceLocalRefPoseNode>();
        for node_type in [
            std::any::TypeId::of::<LocalRefPoseNode>(),
            std::any::TypeId::of::<SourceLocalRefPoseNode>(),
        ] {
            assert!(
                registry
                    .get(node_type)
                    .unwrap()
                    .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                    .is_some()
            );
        }
    }
}
