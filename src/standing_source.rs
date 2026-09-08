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
pub(crate) struct SourceMontageSettings {
    pub clip: String,
    pub duration: f32,
    pub reference_clip: Option<String>,
    pub curves: als_ext::AlsPoseCurves,
}

pub(crate) static MONTAGES: LazyLock<std::collections::HashMap<String, SourceMontageSettings>> =
    LazyLock::new(|| {
        ron::from_str(include_str!(
            "../assets/character/als_standing_montages.ron"
        ))
        .expect("generated native montage settings are valid")
    });

#[derive(Resource)]
pub(crate) struct SourceMontageAssets(
    std::collections::HashMap<
        String,
        (
            Handle<bevy_animation_graph::core::animation_clip::GraphClip>,
            Option<Handle<bevy_animation_graph::core::animation_clip::GraphClip>>,
        ),
    >,
);

pub(crate) fn load_source_montage_assets(mut commands: Commands, server: Res<AssetServer>) {
    commands.insert_resource(SourceMontageAssets(
        MONTAGES
            .iter()
            .map(|(name, settings)| {
                (
                    name.clone(),
                    (
                        server.load(&settings.clip),
                        settings
                            .reference_clip
                            .as_ref()
                            .map(|path| server.load(path)),
                    ),
                )
            })
            .collect(),
    ));
}

pub(crate) fn sample_source_montage(
    name: &str,
    time: f32,
    assets: &SourceMontageAssets,
    clips: &Assets<bevy_animation_graph::core::animation_clip::GraphClip>,
    skeletons: &Assets<bevy_animation_graph::core::skeleton::Skeleton>,
) -> Option<(
    bevy_animation_graph::core::pose::Pose,
    AnimationContextState,
)> {
    let settings = MONTAGES.get(name)?;
    let (clip, reference) = assets.0.get(name)?;
    let time = time.min(settings.duration);
    let mut pose = als_ext::source_sequence_player::sample_pose(clips.get(clip)?, time);
    if let Some(reference) = reference {
        let reference = als_ext::source_sequence_player::sample_pose(clips.get(reference)?, 0.0);
        pose = als_ext::mesh_space_difference::make_mesh_space_difference(
            &reference,
            &pose,
            skeletons.get(&pose.skeleton)?,
        );
        pose.timestamp = time;
    }
    Some((
        pose,
        crate::get_up::sampled_curve_context(&settings.curves, time),
    ))
}

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
pub enum AlsSourceRefreshFunction {
    #[default]
    RefreshStandingMovement,
    RefreshRotateInPlace,
    RefreshTurnInPlace,
    RefreshDynamicTransitions,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "bevy_als"]
pub struct AlsSourceRefreshNode {
    pub function: AlsSourceRefreshFunction,
}

impl AlsSourceRefreshNode {
    fn refresh_standing(&self, ctx: &NodeContext) -> Result<(), GraphError> {
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
        } else if boolean("animation.pending_update", true)? {
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
        Ok(())
    }

    fn refresh_native(&self, ctx: &NodeContext) -> Result<(), GraphError> {
        use als_core::rotate_in_place::{
            RotateInPlaceConfig, RotateInPlaceState, RotateSide, update_rotate_in_place,
        };
        use als_core::turn_in_place::{TurnInPlaceConfig, TurnInPlaceDetector, TurnVariant};
        let scalar = |key, default| source_input_data(ctx, key).map_or(Ok(default), |v| v.as_f32());
        let boolean =
            |key, default| source_input_data(ctx, key).map_or(Ok(default), |v| v.as_bool());
        let key = match self.function {
            AlsSourceRefreshFunction::RefreshStandingMovement => return self.refresh_standing(ctx),
            AlsSourceRefreshFunction::RefreshRotateInPlace => {
                "rotate_in_place_state.b_updated_this_frame"
            }
            AlsSourceRefreshFunction::RefreshTurnInPlace => {
                "turn_in_place_state.b_updated_this_frame"
            }
            AlsSourceRefreshFunction::RefreshDynamicTransitions => {
                "dynamic_transitions_state.b_updated_this_frame"
            }
        };
        if source_instance_properties(ctx)?.write_frames.get(key)
            == Some(&ctx.graph_context.frame_index())
        {
            return Ok(());
        }
        set_source_property(ctx, key, true)?;
        let dt = scalar("animation.delta_seconds", 0.0)?;
        match self.function {
            AlsSourceRefreshFunction::RefreshRotateInPlace => {
                let mut config = RotateInPlaceConfig::default();
                if boolean("rotate_in_place.first_person_threshold", false)? {
                    config.view_yaw_angle_threshold_degrees =
                        config.first_person_view_yaw_angle_threshold_degrees;
                }
                let mut state = RotateInPlaceState {
                    side: None,
                    play_rate: scalar("rotate_in_place_state.play_rate", 1.0)?,
                };
                update_rotate_in_place(
                    &mut state,
                    boolean("locomotion_state.b_moving", false)?,
                    boolean("rotate_in_place.allowed", false)?,
                    scalar("view_state.yaw_angle", 0.0)?,
                    scalar("view_state.yaw_speed", 0.0)?,
                    &config,
                    boolean("animation.pending_update", true)?,
                    dt,
                );
                set_source_property(
                    ctx,
                    "rotate_in_place_state.b_rotating_left",
                    state.side == Some(RotateSide::Left),
                )?;
                set_source_property(
                    ctx,
                    "rotate_in_place_state.b_rotating_right",
                    state.side == Some(RotateSide::Right),
                )?;
                set_source_property(ctx, "rotate_in_place_state.play_rate", state.play_rate)?;
            }
            AlsSourceRefreshFunction::RefreshTurnInPlace => {
                let mut detector = TurnInPlaceDetector {
                    activation_delay: scalar("turn_in_place_state.activation_delay", 0.0)?,
                };
                let yaw = scalar("view_state.yaw_angle", 0.0)?;
                let crouching = boolean("stance.crouching", false)?;
                let event = detector.update(
                    yaw,
                    scalar("view_state.yaw_speed", 0.0)?,
                    dt,
                    boolean("transitions_state.b_transitions_allowed", false)?
                        && boolean("turn_in_place.allowed", false)?,
                    crouching,
                    &TurnInPlaceConfig::default(),
                );
                set_source_property(
                    ctx,
                    "turn_in_place_state.activation_delay",
                    detector.activation_delay,
                )?;
                if let Some(event) = event {
                    let variant = match event.variant {
                        TurnVariant::Left90 => 0.0,
                        TurnVariant::Right90 => 1.0,
                        TurnVariant::Left180 => 2.0,
                        TurnVariant::Right180 => 3.0,
                    };
                    set_source_property(ctx, "turn_in_place.queued_variant", variant)?;
                    set_source_property(ctx, "turn_in_place.queued_yaw", yaw)?;
                    set_source_property(ctx, "turn_in_place.queued_crouching", crouching)?;
                }
            }
            AlsSourceRefreshFunction::RefreshDynamicTransitions => {
                use als_core::{
                    state::Stance,
                    transitions::{
                        TransitionConfig, TransitionSequence, select_dynamic_transition,
                    },
                };
                let delay = scalar("dynamic_transitions_state.frame_delay", 0.0)?;
                if delay > 0.0 {
                    set_source_property(ctx, "dynamic_transitions_state.frame_delay", delay - 1.0)?;
                } else if boolean("transitions_state.b_transitions_allowed", false)? {
                    let config = TransitionConfig::default();
                    let threshold = (config.dynamic_foot_lock_distance
                        * scalar("locomotion_state.scale_world_space", 1.0)?)
                    .powi(2);
                    let left = scalar("feet_state.left.lock_distance_squared", 0.0)?;
                    let right = scalar("feet_state.right.lock_distance_squared", 0.0)?;
                    let selection = select_dynamic_transition(
                        left,
                        right,
                        scalar("feet_state.left.lock_amount", 0.0)? > 1e-5 && left > threshold,
                        scalar("feet_state.right.lock_amount", 0.0)? > 1e-5 && right > threshold,
                        if boolean("stance.crouching", false)? {
                            Stance::Crouching
                        } else {
                            Stance::Standing
                        },
                        &config,
                    );
                    if let Some(selection) = selection {
                        let index = match selection.sequence {
                            TransitionSequence::StandingLeft => 0.0,
                            TransitionSequence::StandingRight => 1.0,
                            TransitionSequence::CrouchingLeft => 2.0,
                            TransitionSequence::CrouchingRight => 3.0,
                            _ => unreachable!(),
                        };
                        set_source_property(ctx, "dynamic_transitions.queued_sequence", index)?;
                        set_source_property(
                            ctx,
                            "dynamic_transitions_state.frame_delay",
                            config.frame_delay as f32,
                        )?;
                    }
                }
            }
            AlsSourceRefreshFunction::RefreshStandingMovement => unreachable!(),
        }
        Ok(())
    }
}

impl NodeLike for AlsSourceRefreshNode {
    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        self.refresh_native(&ctx)?;
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
        format!("ALS {:?}", self.function)
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
    fn native_montage_assets_sample_actual_poses_and_signed_curves() {
        use bevy_animation_graph::core::{
            animation_clip::loader::GraphClipLoader, skeleton::loader::SkeletonLoader,
        };
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::gltf::GltfPlugin::default(),
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<bevy::animation::AnimationClip>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<GraphClip>()
        .init_asset_loader::<GraphClipLoader>()
        .init_asset::<Skeleton>()
        .init_asset_loader::<SkeletonLoader>()
        .add_plugins(als_ext::AlsExtPlugin)
        .add_systems(Startup, load_source_montage_assets);
        app.finish();
        app.cleanup();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut previous = 0;
        loop {
            app.update();
            let assets = app.world().resource::<SourceMontageAssets>();
            let clips = app.world().resource::<Assets<GraphClip>>();
            let skeletons = app.world().resource::<Assets<Skeleton>>();
            let loaded = MONTAGES
                .keys()
                .filter(|name| sample_source_montage(name, 0.4, assets, clips, skeletons).is_some())
                .count();
            if loaded != previous {
                eprintln!("native montage assets loaded {loaded}/{}", MONTAGES.len());
                previous = loaded;
            }
            if loaded == MONTAGES.len() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "native montage assets did not load within 30 seconds: {loaded}"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let assets = app.world().resource::<SourceMontageAssets>();
        let clips = app.world().resource::<Assets<GraphClip>>();
        let skeletons = app.world().resource::<Assets<Skeleton>>();
        for (name, settings) in MONTAGES.iter() {
            let (pose, context) =
                sample_source_montage(name, 0.4, assets, clips, skeletons).unwrap();
            assert_eq!(pose.paths.len(), 79, "{name}");
            assert!(
                pose.bones
                    .iter()
                    .all(|bone| bone.translation.is_none_or(Vec3::is_finite)
                        && bone.rotation.is_none_or(Quat::is_finite)
                        && bone.scale.is_none_or(Vec3::is_finite)),
                "{name}"
            );
            assert!(context.curves_are_authoritative);
            if name.contains("_Turn_") {
                let yaw = context.curves["RotationYawSpeed"];
                assert!(
                    if name.ends_with("_Left") {
                        yaw > 0.0
                    } else {
                        yaw < 0.0
                    },
                    "{name}: {yaw}"
                );
                assert!(settings.reference_clip.is_none());
            } else {
                assert!(settings.reference_clip.is_some());
            }
            assert!(context.curves.values().all(|value| value.is_finite()));
        }
    }

    #[test]
    fn source_idle_calls_share_frame_guards_and_keep_the_source_reentry_order() {
        fn query(
            In(root): In<Handle<AnimationGraph>>,
            resources: SystemResources,
        ) -> Vec<(f32, f32, bool, f32)> {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut values = Vec::new();
            for frame in 0..7 {
                let inputs = HashMap::from([
                    ("fixture".to_owned(), DataValue::Pose(default())),
                    (
                        "active".to_owned(),
                        DataValue::F32(if matches!(frame, 2 | 3) { 0.0 } else { 1.0 }),
                    ),
                    ("animation.delta_seconds".to_owned(), DataValue::F32(0.15)),
                    ("view_state.yaw_angle".to_owned(), DataValue::F32(90.0)),
                    ("turn_in_place.allowed".to_owned(), DataValue::Bool(true)),
                    (
                        "transitions_state.b_transitions_allowed".to_owned(),
                        DataValue::Bool(frame != 6),
                    ),
                    (
                        "feet_state.left.lock_amount".to_owned(),
                        DataValue::F32(0.00002),
                    ),
                    (
                        "feet_state.right.lock_amount".to_owned(),
                        DataValue::F32(1.0),
                    ),
                    (
                        "feet_state.left.lock_distance_squared".to_owned(),
                        DataValue::F32(0.01),
                    ),
                    (
                        "feet_state.right.lock_distance_squared".to_owned(),
                        DataValue::F32(if frame == 5 { 0.04 } else { 0.01 }),
                    ),
                ]);
                graph
                    .query_with_context(
                        QueryOutputTime::Forced(TimeUpdate::Delta(0.15)),
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
                let scalar = |key| {
                    state
                        .values
                        .get(key)
                        .and_then(|v| v.as_f32().ok())
                        .unwrap_or(0.0)
                };
                values.push((
                    scalar("turn_in_place_state.activation_delay"),
                    scalar("dynamic_transitions_state.frame_delay"),
                    state.write_frames.get("turn_in_place.queued_variant")
                        == Some(&arena.frame_index()),
                    scalar("dynamic_transitions.queued_sequence"),
                ));
            }
            values
        }
        let mut graph = AnimationGraph::new();
        let pose = AnimationNode::new(
            "pose",
            GlobalPoseInputNode {
                key: "fixture".to_owned(),
                output: "Pose".to_owned(),
            },
        );
        graph.add_node(pose.clone());
        let mut previous = pose.id;
        for index in 0..5 {
            let node = if index == 2 {
                AnimationNode::new(
                    "initialize",
                    SourcePropertyWriteNode {
                        assignments: vec![SourcePropertyAssignment {
                            property: "turn_in_place_state.activation_delay".to_owned(),
                            value: DataValue::F32(0.0),
                        }],
                        on_become_relevant: true,
                    },
                )
            } else {
                AnimationNode::new(
                    format!("refresh {index}"),
                    AlsSourceRefreshNode {
                        function: if index < 2 {
                            AlsSourceRefreshFunction::RefreshTurnInPlace
                        } else {
                            AlsSourceRefreshFunction::RefreshDynamicTransitions
                        },
                    },
                )
            };
            graph.add_node(node.clone());
            graph.add_node_parameter_edge(previous, "Pose", node.id, "Source");
            previous = node.id;
        }
        let branch = AnimationNode::new(
            "branch",
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
        graph.add_node(branch.clone());
        graph.add_node(scope.clone());
        graph.add_node_parameter_edge(previous, "Pose", branch.id, "B");
        graph.add_node_parameter_edge(pose.id, "Pose", branch.id, "A");
        graph.add_node_parameter_edge(branch.id, "Pose", scope.id, "Source");
        graph.add_output_data("Pose".to_owned(), DataSpec::Pose);
        graph.add_output_data_edge(scope.id, "Pose", "Pose");
        let mut graphs = Assets::<AnimationGraph>::default();
        let root = graphs.add(graph);
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        assert_eq!(
            world.run_system_cached_with(query, root).unwrap(),
            [
                (0.15, 2.0, false, 0.0),
                (0.3, 1.0, true, 0.0),
                (0.3, 1.0, false, 0.0),
                (0.3, 1.0, false, 0.0),
                (0.15, 0.0, false, 0.0),
                (0.3, 2.0, true, 1.0),
                (0.0, 1.0, false, 1.0),
            ]
        );
    }

    #[test]
    fn source_rotate_call_updates_the_actual_instance() {
        fn query(
            In(root): In<Handle<AnimationGraph>>,
            resources: SystemResources,
        ) -> (bool, f32, u64) {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let inputs = HashMap::from([
                ("fixture".to_owned(), DataValue::Pose(default())),
                ("animation.delta_seconds".to_owned(), DataValue::F32(0.15)),
                ("animation.pending_update".to_owned(), DataValue::Bool(true)),
                ("rotate_in_place.allowed".to_owned(), DataValue::Bool(true)),
                ("view_state.yaw_angle".to_owned(), DataValue::F32(-80.0)),
                ("view_state.yaw_speed".to_owned(), DataValue::F32(460.0)),
            ]);
            graph
                .query_with_context(
                    QueryOutputTime::Forced(TimeUpdate::Delta(0.15)),
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
                .ok();
            let value = |key| state.and_then(|state| state.values.get(key));
            (
                value("rotate_in_place_state.b_rotating_left")
                    .and_then(|v| v.as_bool().ok())
                    .unwrap_or(false),
                value("rotate_in_place_state.play_rate")
                    .and_then(|v| v.as_f32().ok())
                    .unwrap_or(1.0),
                state.map_or(0, |state| state.instance_id),
            )
        }
        let mut graph = AnimationGraph::new();
        let pose = AnimationNode::new(
            "pose",
            GlobalPoseInputNode {
                key: "fixture".to_owned(),
                output: "Pose".to_owned(),
            },
        );
        let refresh = AnimationNode::new(
            "refresh",
            AlsSourceRefreshNode {
                function: AlsSourceRefreshFunction::RefreshRotateInPlace,
            },
        );
        graph.add_node(pose.clone());
        graph.add_node(refresh.clone());
        graph.add_node_parameter_edge(pose.id, "Pose", refresh.id, "Source");
        graph.add_output_data("Pose".to_owned(), DataSpec::Pose);
        graph.add_output_data_edge(refresh.id, "Pose", "Pose");
        let mut graphs = Assets::<AnimationGraph>::default();
        let root = graphs.add(graph);
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(Assets::<GraphClip>::default());
        world.insert_resource(Assets::<Skeleton>::default());
        world.insert_resource(Assets::<StateMachine>::default());
        let first = world.run_system_cached_with(query, root.clone()).unwrap();
        let second = world.run_system_cached_with(query, root).unwrap();
        assert_eq!((first.0, first.1), (true, 3.0));
        assert_eq!((second.0, second.1), (true, 3.0));
        assert_ne!(first.2, second.2);
    }

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
                        "animation.pending_update".to_owned(),
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
                AnimationNode::new("refresh", AlsSourceRefreshNode::default())
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
