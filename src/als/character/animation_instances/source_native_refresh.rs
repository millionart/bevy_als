use crate::extensions::{
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

#[derive(Reflect, Clone, Debug, Default)]
pub enum AlsSourceRefreshFunction {
    RefreshHead,
    RefreshInAir,
    RefreshGrounded,
    #[default]
    RefreshStandingMovement,
    RefreshRotateInPlace,
    RefreshTurnInPlace,
    RefreshDynamicTransitions,
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct AlsSourceRefreshNode {
    pub function: AlsSourceRefreshFunction,
}

impl AlsSourceRefreshNode {
    fn refresh_head(&self, ctx: &NodeContext) -> Result<(), GraphError> {
        use crate::als::head::{
            AlsHeadSettings, AlsHeadState, AlsHeadUpdateInput, update_head_state,
        };
        let scalar =
            |key, default| source_input_data(ctx, key).map_or(Ok(default), |value| value.as_f32());
        let boolean =
            |key, default| source_input_data(ctx, key).map_or(Ok(default), |value| value.as_bool());
        let mut state = AlsHeadState {
            initialization_required: boolean("head_state.b_initialization_required", true)?,
            switching_look_sides: boolean("head_state.b_switching_look_sides", false)?,
            pitch_angle: scalar("head_state.pitch_angle", 0.0)?,
            yaw_angle: scalar("head_state.yaw_angle", 0.0)?,
            yaw_velocity: scalar("head_state.yaw_velocity", 0.0)?,
            yaw_amount: scalar("head_state.yaw_amount", 0.5)?,
        };
        update_head_state(
            &mut state,
            AlsHeadUpdateInput {
                delta_seconds: scalar("animation.delta_seconds", 0.0)?,
                delta_seconds_without_time_dilation: scalar(
                    "animation.delta_seconds_without_time_dilation",
                    0.0,
                )?,
                first_person: boolean("view_mode.first_person", false)?,
                rotation_mode: if boolean("rotation_mode.velocity_direction", false)? {
                    crate::als::state::RotationMode::VelocityDirection
                } else {
                    crate::als::state::RotationMode::ViewDirection
                },
                has_input: boolean("locomotion_state.b_has_input", false)?,
                input_yaw_world: scalar("locomotion_state.input_yaw_angle_world_space", 0.0)?,
                target_yaw_world: scalar("locomotion_state.target_yaw_angle_world_space", 0.0)?,
                body_yaw_world: scalar("locomotion_state.rotation_yaw", 0.0)?,
                body_yaw_velocity_world: scalar("locomotion_state.yaw_velocity_world_space", 0.0)?,
                view_yaw_angle: scalar("view_state.yaw_angle", 0.0)?,
                view_pitch_angle: scalar("view_state.pitch_angle", 0.0)?,
                movement_base_relative_yaw: if boolean(
                    "movement_base.b_has_relative_rotation",
                    false,
                )? {
                    scalar("movement_base.delta_yaw", 0.0)?
                } else {
                    0.0
                },
            },
            AlsHeadSettings::default(),
        );
        for (key, value) in [
            (
                "head_state.b_initialization_required",
                state.initialization_required,
            ),
            (
                "head_state.b_switching_look_sides",
                state.switching_look_sides,
            ),
        ] {
            set_source_property(ctx, key, value)?;
        }
        for (key, value) in [
            ("head_state.pitch_angle", state.pitch_angle),
            ("head_state.yaw_angle", state.yaw_angle),
            ("head_state.yaw_velocity", state.yaw_velocity),
            ("head_state.yaw_amount", state.yaw_amount),
        ] {
            set_source_property(ctx, key, value)?;
        }
        Ok(())
    }

    fn refresh_in_air(&self, ctx: &NodeContext) -> Result<(), GraphError> {
        let scalar = |key, default| source_input_data(ctx, key).map_or(Ok(default), |value| value.as_f32());
        let boolean = |key, default| source_input_data(ctx, key).map_or(Ok(default), |value| value.as_bool());
        if boolean("in_air_state.b_jumped", false)? {
            set_source_property(ctx, "in_air_state.b_jumped", false)?;
            set_source_property(ctx, "in_air_state.jump_play_rate", scalar("in_air_source.jump_play_rate", 1.0)?)?;
        }
        let velocity = source_input_data(ctx, "locomotion_state.velocity_world_space")
            .map_or(Ok(Vec3::ZERO), |value| value.as_vec3())?;
        set_source_property(ctx, "in_air_state.vertical_velocity_world_space", velocity.y)?;
        set_source_property(ctx, "in_air_state.ground_prediction_amount", scalar("in_air_source.ground_prediction_amount", 0.0)?)?;
        let target = source_input_data(ctx, "in_air_source.lean_target")
            .map_or(Ok(Vec2::ZERO), |value| value.as_vec2())?;
        let half_life = scalar("in_air_source.lean_interpolation_half_life", 0.0)?;
        let alpha = if boolean("animation.pending_update", true)? || half_life <= 0.0 {
            1.0
        } else {
            1.0 - crate::als::animation::inv_exp_approx(
                std::f32::consts::LN_2 / (half_life + 1e-8) * scalar("animation.delta_seconds", 0.0)?,
            )
        };
        for (key, target) in [
            ("lean_state.forward_amount", target.x),
            ("lean_state.right_amount", target.y),
        ] {
            set_source_property(ctx, key, scalar(key, 0.0)?.lerp(target, alpha))?;
        }
        Ok(())
    }

    fn refresh_grounded(&self, ctx: &NodeContext) -> Result<(), GraphError> {
        let scalar =
            |key, default| source_input_data(ctx, key).map_or(Ok(default), |value| value.as_f32());
        let boolean =
            |key, default| source_input_data(ctx, key).map_or(Ok(default), |value| value.as_bool());
        let vector =
            |key| source_input_data(ctx, key).map_or(Ok(Vec3::ZERO), |value| value.as_vec3());
        let rotation = source_input_data(ctx, "locomotion_state.rotation_quaternion_world_space")
            .map_or(Ok(Quat::IDENTITY), |value| value.as_quat())?;
        let velocity = vector("locomotion_state.velocity_world_space")?;
        let direction = crate::als::animation::velocity_blend_direction(velocity, rotation);
        let dt = scalar("animation.delta_seconds", 0.0)?;
        let alpha = |half_life: f32| {
            1.0 - crate::als::animation::inv_exp_approx(
                std::f32::consts::LN_2 / (half_life + 1e-8) * dt,
            )
        };
        let velocity_alpha = if boolean(
            "grounded_state.velocity_blend.b_initialization_required",
            true,
        )? {
            1.0
        } else {
            alpha(0.1)
        };
        set_source_property(
            ctx,
            "grounded_state.velocity_blend.b_initialization_required",
            false,
        )?;
        for (key, target) in [
            (
                "grounded_state.velocity_blend.forward_amount",
                (-direction.z).max(0.0),
            ),
            (
                "grounded_state.velocity_blend.backward_amount",
                direction.z.max(0.0),
            ),
            (
                "grounded_state.velocity_blend.left_amount",
                (-direction.x).max(0.0),
            ),
            (
                "grounded_state.velocity_blend.right_amount",
                direction.x.max(0.0),
            ),
        ] {
            set_source_property(ctx, key, scalar(key, 0.0)?.lerp(target, velocity_alpha))?;
        }
        let target_lean = crate::als::animation::acceleration_amount(
            vector("locomotion_state.acceleration_world_space")?,
            velocity,
            rotation,
            scalar("locomotion_state.max_acceleration", 0.0)?,
            scalar("locomotion_state.max_braking_deceleration", 0.0)?,
        );
        let lean_alpha = if boolean("animation.pending_update", true)? {
            1.0
        } else {
            alpha(0.2)
        };
        for (key, target) in [
            ("lean_state.forward_amount", target_lean.x),
            ("lean_state.right_amount", target_lean.y),
        ] {
            set_source_property(ctx, key, scalar(key, 0.0)?.lerp(target, lean_alpha))?;
        }
        set_source_property(ctx, "standing_source.acceleration_forward", target_lean.x)?;
        Ok(())
    }

    fn refresh_standing(&self, ctx: &NodeContext) -> Result<(), GraphError> {
        let scalar =
            |key, default| source_input_data(&ctx, key).map_or(Ok(default), |value| value.as_f32());
        let boolean = |key, default| {
            source_input_data(&ctx, key).map_or(Ok(default), |value| value.as_bool())
        };
        for (key, default) in [
            ("standing_state.walk_run_blend_amount", 0.0),
            ("standing_state.stride_blend_amount", 1.0),
            ("standing_state.sprint_block_amount", 0.0),
            ("standing_state.play_rate", 1.0),
        ] {
            set_source_property(ctx, key, scalar(key, default)?)?;
        }
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
        use crate::als::rotate_in_place::{
            RotateInPlaceConfig, RotateInPlaceState, RotateSide, update_rotate_in_place,
        };
        use crate::als::turn_in_place::{TurnInPlaceConfig, TurnInPlaceDetector, TurnVariant};
        let scalar = |key, default| source_input_data(ctx, key).map_or(Ok(default), |v| v.as_f32());
        let boolean =
            |key, default| source_input_data(ctx, key).map_or(Ok(default), |v| v.as_bool());
        let key = match self.function {
            AlsSourceRefreshFunction::RefreshHead => return self.refresh_head(ctx),
            AlsSourceRefreshFunction::RefreshInAir => return self.refresh_in_air(ctx),
            AlsSourceRefreshFunction::RefreshGrounded => return self.refresh_grounded(ctx),
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
                use crate::als::{
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
            AlsSourceRefreshFunction::RefreshStandingMovement
            | AlsSourceRefreshFunction::RefreshHead
            | AlsSourceRefreshFunction::RefreshInAir
            | AlsSourceRefreshFunction::RefreshGrounded => unreachable!(),
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
    use crate::extensions::{
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
    fn head_source_calls_refresh_before_sampling_and_initialize_after_irrelevance() {
        fn query(
            In(root): In<Handle<AnimationGraph>>,
            resources: SystemResources,
        ) -> Vec<(f32, f32, f32, bool)> {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut values = Vec::new();
            for frame in 0..7 {
                let (pitch, yaw) = match frame {
                    0 => (30.0, 90.0),
                    4 => (15.0, -60.0),
                    5 => (-45.0, 60.0),
                    _ => (-30.0, -90.0),
                };
                let inputs = HashMap::from([
                    ("fixture".to_owned(), DataValue::Pose(default())),
                    (
                        "active".to_owned(),
                        DataValue::F32(match frame {
                            2 => 0.0,
                            3 => 1e-6,
                            _ => 1.0,
                        }),
                    ),
                    (
                        "animation.delta_seconds".to_owned(),
                        DataValue::F32(if frame == 5 { 0.01 } else { 0.1 }),
                    ),
                    (
                        "animation.delta_seconds_without_time_dilation".to_owned(),
                        DataValue::F32(0.1),
                    ),
                    (
                        "source_frame_input.view_state.pitch_angle".to_owned(),
                        DataValue::F32(pitch),
                    ),
                    (
                        "source_frame_input.view_state.yaw_angle".to_owned(),
                        DataValue::F32(yaw),
                    ),
                    (
                        "view_mode.first_person".to_owned(),
                        DataValue::Bool(frame == 5),
                    ),
                    (
                        "rotation_mode.velocity_direction".to_owned(),
                        DataValue::Bool(frame == 6),
                    ),
                    (
                        "locomotion_state.b_has_input".to_owned(),
                        DataValue::Bool(false),
                    ),
                    (
                        "locomotion_state.target_yaw_angle_world_space".to_owned(),
                        DataValue::F32(30.0),
                    ),
                    (
                        "locomotion_state.rotation_yaw".to_owned(),
                        DataValue::F32(10.0),
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
                let scalar = |key| {
                    state
                        .values
                        .get(key)
                        .and_then(|value| value.as_f32().ok())
                        .unwrap_or(0.0)
                };
                values.push((
                    scalar("head_state.pitch_angle"),
                    scalar("head_state.yaw_angle"),
                    scalar("head_state.yaw_amount"),
                    state.write_frames.get("head_state.yaw_amount") == Some(&arena.frame_index()),
                ));
            }
            values
        }
        let expected = [
            (30.0, 90.0, 0.75, true),
            (0.0, 0.0, 0.0, true),
            (0.0, 0.0, 0.0, false),
            (0.0, 0.0, 0.0, false),
            (15.0, -60.0, 1.0 / 3.0, true),
        ];
        for (refresh_enabled, initialize_enabled) in [(true, true), (false, true), (true, false)] {
            let mut graph = AnimationGraph::new();
            let pose = AnimationNode::new(
                "pose",
                GlobalPoseInputNode {
                    key: "fixture".to_owned(),
                    output: "Pose".to_owned(),
                },
            );
            let sampled = AnimationNode::new(
                "sample",
                SourcePropertyWriteNode {
                    assignments: vec![SourcePropertyAssignment {
                        property: "view_state.yaw_angle".to_owned(),
                        value: DataValue::F32(150.0),
                    }],
                    on_become_relevant: false,
                },
            );
            let refresh = if refresh_enabled {
                AnimationNode::new(
                    "RefreshHead",
                    AlsSourceRefreshNode {
                        function: AlsSourceRefreshFunction::RefreshHead,
                    },
                )
            } else {
                AnimationNode::new(
                    "RefreshHead",
                    SourcePosePassthroughNode {
                        output_time: false,
                        ..default()
                    },
                )
            };
            let initialize = AnimationNode::new(
                "InitializeHead",
                SourcePropertyWriteNode {
                    assignments: if initialize_enabled {
                        vec![SourcePropertyAssignment {
                            property: "head_state.b_initialization_required".to_owned(),
                            value: DataValue::Bool(true),
                        }]
                    } else {
                        vec![]
                    },
                    on_become_relevant: true,
                },
            );
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
            for node in [&pose, &sampled, &refresh, &initialize, &branch, &scope] {
                graph.add_node(node.clone());
            }
            for (from, to) in [
                (pose.id, sampled.id),
                (sampled.id, refresh.id),
                (refresh.id, initialize.id),
            ] {
                graph.add_node_parameter_edge(from, "Pose", to, "Source");
            }
            graph.add_node_parameter_edge(initialize.id, "Pose", branch.id, "B");
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
            let actual = world.run_system_cached_with(query, root).unwrap();
            if refresh_enabled && initialize_enabled {
                for frame in [0, 4] {
                    let (pitch, yaw, amount, updated) = actual[frame];
                    assert!((pitch - expected[frame].0).abs() < 1e-5);
                    assert!((yaw - expected[frame].1).abs() < 1e-5);
                    assert!((amount - expected[frame].2).abs() < 1e-5);
                    assert!(updated);
                }
                assert!(actual[1].0.abs() < 0.1 && actual[1].1 < 30.0 && actual[1].1 > 0.0);
                assert_eq!(actual[2], (actual[1].0, actual[1].1, actual[1].2, false));
                assert_eq!(actual[3], actual[2]);
                assert!(actual[5].0 < -44.0 && actual[5].1 > -30.0 && actual[5].1 < 10.0);
                assert!(actual[6].0 > actual[5].0 && actual[6].1 > actual[5].1);
            } else {
                assert_ne!(
                    actual[if refresh_enabled { 4 } else { 0 }],
                    expected[if refresh_enabled { 4 } else { 0 }]
                );
            }
        }
    }

    #[test]
    fn in_air_source_calls_latch_landing_values_consume_jumps_and_reset_lean_on_reentry() {
        fn query(In(root): In<Handle<AnimationGraph>>, resources: SystemResources) -> Vec<[f32; 5]> {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut values = Vec::new();
            for frame in 0..7 {
                let target = match frame {
                    0 => Vec2::new(1.0, 2.0),
                    4 => Vec2::new(-1.0, 0.5),
                    5 => Vec2::new(0.5, 0.0),
                    6 => Vec2::new(0.2, 0.3),
                    _ => Vec2::ZERO,
                };
                let inputs = HashMap::from([
                    ("fixture".to_owned(), DataValue::Pose(default())),
                    ("active".to_owned(), DataValue::F32(if matches!(frame, 2 | 3) { 0.0 } else { 1.0 })),
                    ("animation.delta_seconds".to_owned(), DataValue::F32(0.2)),
                    ("animation.pending_update".to_owned(), DataValue::Bool(frame == 0 || frame == 6)),
                    ("source_frame_input.in_air_state.b_jumped".to_owned(), DataValue::Bool(matches!(frame, 1 | 3 | 4))),
                    ("locomotion_state.velocity_world_space".to_owned(), DataValue::Vec3(Vec3::NEG_Y * (frame + 1) as f32)),
                    ("in_air_source.ground_prediction_amount".to_owned(), DataValue::F32(frame as f32 * 0.1)),
                    ("in_air_source.jump_play_rate".to_owned(), DataValue::F32(if frame == 4 { 1.5 } else { 1.35 })),
                    ("in_air_source.lean_target".to_owned(), DataValue::Vec2(target)),
                    ("in_air_source.lean_interpolation_half_life".to_owned(), DataValue::F32(if frame == 5 { 0.0 } else { 0.2 })),
                ]);
                graph.query_with_context(
                    QueryOutputTime::Forced(TimeUpdate::Delta(0.2)),
                    GraphContext::new(arena.get_toplevel_id(), graph, &mut arena, &resources, &env,
                        Entity::PLACEHOLDER, &entities, &mut gizmos, &inputs),
                ).unwrap();
                let state = arena.get_toplevel().node_states.get::<SourceInstanceProperties>(NodeId::default(), StateKey::Default).unwrap();
                values.push([
                    "in_air_state.jump_play_rate", "in_air_state.vertical_velocity_world_space",
                    "in_air_state.ground_prediction_amount", "lean_state.forward_amount", "lean_state.right_amount",
                ].map(|key| state.values.get(key).and_then(|value| value.as_f32().ok()).unwrap_or(if key.ends_with("jump_play_rate") { 1.0 } else { 0.0 })));
            }
            values
        }
        let alpha = 1.0 - crate::als::animation::inv_exp_approx(std::f32::consts::LN_2 / (0.2 + 1e-8) * 0.2);
        let expected = [
            [1.0, -1.0, 0.0, 1.0, 2.0],
            [1.35, -2.0, 0.1, 1.0 - alpha, 2.0 * (1.0 - alpha)],
            [1.35, -2.0, 0.1, 1.0 - alpha, 2.0 * (1.0 - alpha)],
            [1.35, -2.0, 0.1, 1.0 - alpha, 2.0 * (1.0 - alpha)],
            [1.5, -5.0, 0.4, -alpha, 0.5 * alpha],
            [1.5, -6.0, 0.5, 0.5, 0.0],
            [1.5, -7.0, 0.6, 0.2, 0.3],
        ];
        for (execute, initialize_enabled) in [(true, true), (false, true), (true, false)] {
            let mut graph = AnimationGraph::new();
            let pose = AnimationNode::new("pose", GlobalPoseInputNode { key: "fixture".to_owned(), output: "Pose".to_owned() });
            let refresh = if execute {
                AnimationNode::new("refresh", AlsSourceRefreshNode { function: AlsSourceRefreshFunction::RefreshInAir })
            } else {
                AnimationNode::new("refresh", SourcePosePassthroughNode { output_time: false, ..default() })
            };
            let initialize = AnimationNode::new("initialize", SourcePropertyWriteNode {
                assignments: if initialize_enabled {
                    ["lean_state.forward_amount", "lean_state.right_amount"].map(|property| SourcePropertyAssignment {
                        property: property.to_owned(), value: DataValue::F32(0.0),
                    }).to_vec()
                } else { vec![] },
                on_become_relevant: true,
            });
            let active = AnimationNode::new("active", SourceTwoWayBlendNode { alpha_key: "active".to_owned(), ..default() });
            let scope = AnimationNode::new("scope", SourcePosePassthroughNode { synchronize_groups: true, output_time: false, ..default() });
            for node in [&pose, &refresh, &initialize, &active, &scope] { graph.add_node(node.clone()); }
            graph.add_node_parameter_edge(pose.id, "Pose", refresh.id, "Source");
            graph.add_node_parameter_edge(refresh.id, "Pose", initialize.id, "Source");
            graph.add_node_parameter_edge(initialize.id, "Pose", active.id, "B");
            graph.add_node_parameter_edge(pose.id, "Pose", active.id, "A");
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
            if execute && initialize_enabled {
                for (frame, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                    for (field, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                        assert!((actual - expected).abs() < 1e-5, "frame {frame}, field {field}: {actual} != {expected}");
                    }
                }
            } else { assert_ne!(actual[4], expected[4]); }
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
    fn grounded_source_refresh_shares_stances_and_reinitializes_only_velocity_on_reentry() {
        fn query(
            In(root): In<Handle<AnimationGraph>>,
            resources: SystemResources,
        ) -> Vec<[f32; 5]> {
            let graph = resources.animation_graph_assets.get(&root).unwrap();
            let mut arena = GraphContextArena::new(root.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = HashMap::new();
            let env = TestIoEnv::default();
            let mut values = Vec::new();
            for frame in 0..7 {
                let velocity = match frame {
                    0 => Vec3::NEG_Z,
                    1..=3 => Vec3::X,
                    _ => Vec3::Z,
                };
                let acceleration = match frame {
                    1 => Vec3::X * 10.0,
                    4 => Vec3::Z * 5.0,
                    5 => Vec3::NEG_Z * 5.0,
                    _ => Vec3::ZERO,
                };
                let inputs = HashMap::from([
                    ("fixture".to_owned(), DataValue::Pose(default())),
                    (
                        "active".to_owned(),
                        DataValue::F32(if matches!(frame, 2 | 3) { 0.0 } else { 1.0 }),
                    ),
                    ("stance".to_owned(), DataValue::F32((frame % 2) as f32)),
                    ("animation.delta_seconds".to_owned(), DataValue::F32(0.1)),
                    (
                        "animation.pending_update".to_owned(),
                        DataValue::Bool(frame == 0 || frame == 6),
                    ),
                    (
                        "locomotion_state.velocity_world_space".to_owned(),
                        DataValue::Vec3(velocity),
                    ),
                    (
                        "locomotion_state.acceleration_world_space".to_owned(),
                        DataValue::Vec3(acceleration),
                    ),
                    (
                        "locomotion_state.max_acceleration".to_owned(),
                        DataValue::F32(20.0),
                    ),
                    (
                        "locomotion_state.max_braking_deceleration".to_owned(),
                        DataValue::F32(5.0),
                    ),
                    ("lean_state.right_amount".to_owned(), DataValue::F32(0.9)),
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
                values.push(
                    [
                        "grounded_state.velocity_blend.forward_amount",
                        "grounded_state.velocity_blend.backward_amount",
                        "grounded_state.velocity_blend.right_amount",
                        "lean_state.forward_amount",
                        "lean_state.right_amount",
                    ]
                    .map(|key| {
                        state
                            .values
                            .get(key)
                            .and_then(|value| value.as_f32().ok())
                            .unwrap_or(0.0)
                    }),
                );
            }
            values
        }
        let expected = [
            [1.0, 0.0, 0.0, 0.0, 0.0],
            [0.49988737, 0.0, 0.50011263, 0.0, 0.14638911],
            [0.49988737, 0.0, 0.50011263, 0.0, 0.14638911],
            [0.49988737, 0.0, 0.50011263, 0.0, 0.14638911],
            [0.0, 1.0, 0.0, -0.07319455, 0.10352957],
            [0.0, 1.0, 0.0, 0.24101343, 0.07321836],
            [0.0, 1.0, 0.0, 0.0, 0.0],
        ];
        for (execute, initialize_enabled) in [(true, true), (false, true), (true, false)] {
            let mut graph = AnimationGraph::new();
            let standing = AnimationNode::new(
                "standing",
                GlobalPoseInputNode {
                    key: "fixture".to_owned(),
                    output: "Pose".to_owned(),
                },
            );
            let crouching = AnimationNode::new(
                "crouching",
                GlobalPoseInputNode {
                    key: "fixture".to_owned(),
                    output: "Pose".to_owned(),
                },
            );
            let stance = AnimationNode::new(
                "stance",
                SourceTwoWayBlendNode {
                    alpha_key: "stance".to_owned(),
                    ..default()
                },
            );
            let refresh = if execute {
                AnimationNode::new(
                    "refresh",
                    AlsSourceRefreshNode {
                        function: AlsSourceRefreshFunction::RefreshGrounded,
                    },
                )
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
                    assignments: if initialize_enabled {
                        vec![SourcePropertyAssignment {
                            property: "grounded_state.velocity_blend.b_initialization_required"
                                .to_owned(),
                            value: DataValue::Bool(true),
                        }]
                    } else {
                        vec![]
                    },
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
            for node in [
                &standing,
                &crouching,
                &stance,
                &refresh,
                &initialize,
                &active,
                &scope,
            ] {
                graph.add_node(node.clone());
            }
            graph.add_node_parameter_edge(standing.id, "Pose", stance.id, "A");
            graph.add_node_parameter_edge(crouching.id, "Pose", stance.id, "B");
            graph.add_node_parameter_edge(stance.id, "Pose", refresh.id, "Source");
            graph.add_node_parameter_edge(refresh.id, "Pose", initialize.id, "Source");
            graph.add_node_parameter_edge(initialize.id, "Pose", active.id, "B");
            graph.add_node_parameter_edge(standing.id, "Pose", active.id, "A");
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
            if execute && initialize_enabled {
                for (frame, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                    for (field, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                        assert!(
                            (actual - expected).abs() < 1e-5,
                            "frame {frame}, field {field}: {actual} != {expected}"
                        );
                    }
                }
            } else {
                assert_ne!(actual[4], expected[4]);
            }
        }
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
