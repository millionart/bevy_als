use als_ext::{
    AnimationContextState,
    animation_context::pose_input_context_state,
    source_instance::{set_source_property, source_input_data, source_instance_properties},
};
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
struct StandingCurves {
    stride_walk: als_ext::als_curves::AlsFloatCurve,
    stride_run: als_ext::als_curves::AlsFloatCurve,
    yaw_forward_backward: als_ext::als_curves::AlsFloatCurve,
    yaw_left: als_ext::als_curves::AlsFloatCurve,
    yaw_right: als_ext::als_curves::AlsFloatCurve,
}

static CURVES: LazyLock<StandingCurves> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../assets/character/als_standing_curves.json"))
        .expect("generated Standing curves are valid")
});

pub(crate) fn standing_stride_amount(running: f32, speed: f32) -> f32 {
    CURVES
        .stride_walk
        .sample(speed)
        .lerp(CURVES.stride_run.sample(speed), running)
}

pub(crate) fn standing_rotation_yaw_offsets(runtime_angle: f32) -> (f32, f32, f32, f32) {
    let forward_backward = CURVES.yaw_forward_backward.sample(runtime_angle);
    (
        forward_backward,
        forward_backward,
        CURVES.yaw_left.sample(runtime_angle),
        CURVES.yaw_right.sample(runtime_angle),
    )
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "bevy_als"]
pub struct StandingSourceRefreshNode;

impl NodeLike for StandingSourceRefreshNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let scalar =
            |key, default| source_input_data(&ctx, key).map_or(Ok(default), |value| value.as_f32());
        let boolean = |key, default| {
            source_input_data(&ctx, key).map_or(Ok(default), |value| value.as_bool())
        };
        let initialized = source_instance_properties(&ctx)?
            .write_frames
            .get("standing_state.sprint_time")
            == Some(&ctx.graph_context.frame_index());
        if !initialized && boolean("standing_source.pivot_requested", false)? {
            set_source_property(
                &ctx,
                "standing_state.b_pivot_active",
                boolean("standing_source.pivot_activated", false)?,
            )?;
        }
        let sprinting = scalar("als_standing_gait_is_sprinting", 0.0)? > 0.5;
        let sprint_time = if !sprinting {
            0.0
        } else if boolean("standing_source.pending_update", true)? {
            0.5
        } else {
            scalar("standing_state.sprint_time", 0.0)? + scalar("animation.delta_seconds", 0.0)?
        };
        let acceleration = if sprinting && sprint_time < 0.5 {
            scalar("standing_source.acceleration_forward", 0.0)?
        } else {
            0.0
        };
        set_source_property(&ctx, "standing_state.sprint_time", sprint_time)?;
        set_source_property(
            &ctx,
            "standing_state.sprint_acceleration_amount",
            acceleration,
        )?;
        let pose = ctx.data_back("Source")?.into_pose()?;
        ctx.set_time(pose.timestamp);
        ctx.set_data_fwd("Pose", pose);
        *ctx.state_mut_or_else(AnimationContextState::default)? =
            pose_input_context_state(&ctx, "Source").unwrap_or_default();
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data("Source", DataSpec::Pose)
            .add_output_data("Pose", DataSpec::Pose);
        Ok(())
    }

    fn display_name(&self) -> String {
        "ALS Refresh Standing Movement".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use als_ext::{
        GlobalPoseInputNode, SourcePosePassthroughNode, SourceTwoWayBlendNode,
        source_instance::{
            SourceInstanceProperties, SourcePropertyAssignment, SourcePropertyWriteNode,
        },
    };
    use bevy::platform::collections::HashMap;
    use bevy_animation_graph::core::{
        animation_clip::GraphClip,
        animation_graph::{AnimationGraph, NodeId, TimeUpdate},
        animation_node::AnimationNode,
        context::{
            deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
            graph_context_arena::GraphContextArena, new_context::GraphContext,
            node_states::StateKey, system_resources::SystemResources,
        },
        edge_data::DataValue,
        skeleton::Skeleton,
        state_machine::high_level::StateMachine,
        utils::testing::TestIoEnv,
    };

    #[test]
    fn standing_source_refresh_runs_only_with_the_source_branch_and_resets_on_reentry() {
        fn query(
            In(root): In<Handle<AnimationGraph>>,
            resources: SystemResources,
        ) -> Vec<(f32, f32, bool)> {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut values = Vec::new();
            for frame in 0..7 {
                let inputs = HashMap::from([
                    (
                        "active".to_owned(),
                        DataValue::F32(if frame == 2 || frame == 3 { 0.0 } else { 1.0 }),
                    ),
                    ("fixture".to_owned(), DataValue::Pose(default())),
                    ("animation.delta_seconds".to_owned(), DataValue::F32(0.1)),
                    (
                        "als_standing_gait_is_sprinting".to_owned(),
                        DataValue::F32(if frame == 5 { 0.0 } else { 1.0 }),
                    ),
                    (
                        "standing_source.pending_update".to_owned(),
                        DataValue::Bool(frame == 6),
                    ),
                    (
                        "standing_source.pivot_requested".to_owned(),
                        DataValue::Bool(frame == 1 || frame == 4),
                    ),
                    (
                        "standing_source.pivot_activated".to_owned(),
                        DataValue::Bool(true),
                    ),
                    (
                        "standing_source.acceleration_forward".to_owned(),
                        DataValue::F32(0.8),
                    ),
                ]);
                graph
                    .query_with_context(
                        QueryOutputTime::Forced(TimeUpdate::Delta(0.1)),
                        GraphContext::new(
                            arena.get_toplevel_id(),
                            graph,
                            &mut arena,
                            &resources,
                            &env,
                            Entity::PLACEHOLDER,
                            &entities,
                            &mut gizmos,
                            &inputs,
                        ),
                    )
                    .unwrap();
                let state = arena
                    .get_toplevel()
                    .node_states
                    .get::<SourceInstanceProperties>(NodeId::default(), StateKey::Default)
                    .unwrap();
                values.push((
                    state.values["standing_state.sprint_time"].as_f32().unwrap(),
                    state
                        .values
                        .get("standing_state.sprint_acceleration_amount")
                        .and_then(|v| v.as_f32().ok())
                        .unwrap_or(0.0),
                    state.values["standing_state.b_pivot_active"]
                        .as_bool()
                        .unwrap(),
                ));
            }
            values
        }
        for execute in [false, true] {
            let mut graph = AnimationGraph::new();
            let source = AnimationNode::new(
                "pose",
                GlobalPoseInputNode {
                    key: "fixture".to_owned(),
                    output: "Pose".to_owned(),
                },
            );
            let refresh = if execute {
                AnimationNode::new("refresh", StandingSourceRefreshNode)
            } else {
                AnimationNode::new(
                    "refresh",
                    SourcePosePassthroughNode {
                        output_time: false,
                        ..default()
                    },
                )
            };
            let initialize = AnimationNode::new(
                "initialize",
                SourcePropertyWriteNode {
                    assignments: vec![
                        SourcePropertyAssignment {
                            property: "standing_state.sprint_time".to_owned(),
                            value: DataValue::F32(0.0),
                        },
                        SourcePropertyAssignment {
                            property: "standing_state.b_pivot_active".to_owned(),
                            value: DataValue::Bool(false),
                        },
                    ],
                    on_become_relevant: true,
                },
            );
            let active = AnimationNode::new(
                "active",
                SourceTwoWayBlendNode {
                    alpha_key: "active".to_owned(),
                    ..default()
                },
            );
            let scope = AnimationNode::new(
                "scope",
                SourcePosePassthroughNode {
                    synchronize_groups: true,
                    output_time: false,
                    ..default()
                },
            );
            for node in [&source, &refresh, &initialize, &active, &scope] {
                graph.add_node(node.clone());
            }
            graph.add_node_parameter_edge(source.id, "Pose", refresh.id, "Source");
            graph.add_node_parameter_edge(refresh.id, "Pose", initialize.id, "Source");
            graph.add_node_parameter_edge(initialize.id, "Pose", active.id, "B");
            graph.add_node_parameter_edge(source.id, "Pose", active.id, "A");
            graph.add_node_parameter_edge(active.id, "Pose", scope.id, "Source");
            graph.add_output_data("Pose".to_owned(), DataSpec::Pose);
            graph.add_output_data_edge(scope.id, "Pose", "Pose");
            let mut graphs = Assets::<AnimationGraph>::default();
            let root = graphs.add(graph);
            let mut world = World::new();
            world.insert_resource(graphs);
            world.insert_resource(Assets::<GraphClip>::default());
            world.insert_resource(Assets::<Skeleton>::default());
            world.insert_resource(Assets::<StateMachine>::default());
            let actual = world.run_system_cached_with(query, root).unwrap();
            if execute {
                assert_eq!(
                    actual,
                    [
                        (0.1, 0.8, false),
                        (0.2, 0.8, true),
                        (0.2, 0.8, true),
                        (0.2, 0.8, true),
                        (0.1, 0.8, false),
                        (0.0, 0.0, false),
                        (0.5, 0.0, false)
                    ]
                );
            } else {
                assert_eq!(actual[0], (0.0, 0.0, false));
            }
        }
    }
}
