use crate::als::character::AlsCharacter;
use crate::als::state::AlsNotifyLocomotionAction;
use avian3d::prelude::LinearVelocity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::pose::Pose;
use bevy_animation_graph::core::ragdoll::definition::BodyLabel;

use crate::character_assembly::AlsCharacterAvatar;

const PELVIS_LABEL: &str = "pelvis";
const REFERENCE_SPEED_METERS_PER_SECOND: f32 = 10.0;
pub(crate) const RAGDOLLING_FINAL_POSE_KEY: &str = "ragdolling_state.final_ragdoll_pose";
const RAGDOLLING_FLAIL_PLAY_RATE_KEY: &str = "ragdolling_state.flail_play_rate";

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AlsRagdollingAnimationState {
    pub flail_play_rate: f32,
}

impl Default for AlsRagdollingAnimationState {
    fn default() -> Self {
        Self {
            flail_play_rate: 1.0,
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsRagdollingLayerSet;

pub struct RagdollingLayerPlugin;

impl Plugin for RagdollingLayerPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsRagdollingLayerSet.before(
                crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate,
            ),
        )
        .add_systems(
            PostUpdate,
            sample_ragdolling_flail_state
                .before(bevy_animation_graph::core::plugin::AnimationGraphSet::PrePhysics)
                .before(avian3d::prelude::PhysicsSystems::First)
                .before(AlsRagdollingLayerSet),
        )
        .add_systems(
            PostUpdate,
            ragdolling_layer_writer.in_set(AlsRagdollingLayerSet),
        );
    }
}

pub fn flail_play_rate_for_speed(speed_meters_per_second: f32) -> f32 {
    (speed_meters_per_second / REFERENCE_SPEED_METERS_PER_SECOND).clamp(0.0, 1.0)
}

fn ragdoll_pelvis_speed(
    player: &AnimationGraphPlayer,
    labels: &Query<&BodyLabel>,
    velocities: &Query<&LinearVelocity>,
) -> Option<f32> {
    let spawned = player.get_spawned_ragdoll()?;
    for body in spawned.bodies.values() {
        if labels.get(*body).is_ok_and(|label| label.0 == PELVIS_LABEL)
            && let Ok(velocity) = velocities.get(*body)
        {
            return Some(velocity.0.length());
        }
    }
    None
}

fn write_globals(player: &mut AnimationGraphPlayer, ragdolling: bool, flail_play_rate: f32) {
    player.set_global_input_data(RAGDOLLING_FLAIL_PLAY_RATE_KEY, flail_play_rate);
    player.set_global_input_data("als_ragdolling_dynamic", ragdolling);
    player.set_global_input_data("als_ragdolling_readback", ragdolling);
}

fn player_pose_with_skeleton(player: &AnimationGraphPlayer) -> Option<Pose> {
    let mut pose = player.get_default_output_pose().cloned()?;
    pose.skeleton = player.skeleton().clone();
    Some(pose)
}

fn sample_ragdolling_flail_state(
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            Option<&AlsNotifyLocomotionAction>,
            Option<&mut AlsRagdollingAnimationState>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    labels: Query<&BodyLabel>,
    velocities: Query<&LinearVelocity>,
    players: Query<&AnimationGraphPlayer>,
) {
    for (entity, action, current_state) in &mut characters {
        if action != Some(&AlsNotifyLocomotionAction::Ragdolling) {
            continue;
        }
        let mut state = current_state.as_deref().copied().unwrap_or_default();
        for (parent, instance) in &avatars {
            if parent.parent() != entity {
                continue;
            }
            if let Ok(player) = players.get(instance.player_entity()) {
                state.flail_play_rate = ragdoll_pelvis_speed(player, &labels, &velocities)
                    .map(flail_play_rate_for_speed)
                    .unwrap_or_default();
            }
        }
        if let Some(mut current_state) = current_state {
            *current_state = state;
        } else {
            commands.entity(entity).insert(state);
        }
    }
}

#[allow(clippy::type_complexity)]
fn ragdolling_layer_writer(
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            Option<&AlsNotifyLocomotionAction>,
            Option<&mut AlsRagdollingAnimationState>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (entity, action, current_state) in &mut characters {
        let ragdolling = action == Some(&AlsNotifyLocomotionAction::Ragdolling);
        let state = current_state.as_deref().copied().unwrap_or_default();
        for (parent, instance) in &avatars {
            if parent.parent() != entity {
                continue;
            }
            let Ok(mut player) = players.get_mut(instance.player_entity()) else {
                continue;
            };
            if player
                .get_global_input_data(RAGDOLLING_FINAL_POSE_KEY)
                .is_none()
                && let Some(snapshot) = player_pose_with_skeleton(&player)
            {
                player.set_global_input_data(RAGDOLLING_FINAL_POSE_KEY, DataValue::Pose(snapshot));
            }
            write_globals(&mut player, ragdolling, state.flail_play_rate);
        }
        if let Some(mut current_state) = current_state {
            *current_state = state;
        } else {
            commands.entity(entity).insert(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_flail_input_uses_the_source_pre_physics_velocity() {
        fn flail_clock(
            world: &World,
            player: Entity,
        ) -> crate::extensions::animation_context::SourceRelevantAsset {
            use crate::extensions::source_sequence_player::{
                SourceSequencePlayerNode, SourceSequencePlayerState,
            };
            use bevy_animation_graph::core::animation_graph::AnimationGraph;
            let player = world.get::<AnimationGraphPlayer>(player).unwrap();
            let arena = player.get_context_arena().unwrap();
            let graphs = world.resource::<Assets<AnimationGraph>>();
            for id in arena.iter_context_ids() {
                let context = arena.get_context(id).unwrap();
                let Some(graph) = graphs.get(context.get_graph_id()) else {
                    continue;
                };
                for node in graph.nodes.values() {
                    if !node
                        .try_inner_downcast_ref::<SourceSequencePlayerNode>()
                        .is_some_and(|node| node.play_rate_key == RAGDOLLING_FLAIL_PLAY_RATE_KEY)
                    {
                        continue;
                    }
                    if let Ok(mut states) = context
                        .node_states
                        .get_all_upcoming_states::<SourceSequencePlayerState>(node.id)
                    {
                        if let Some(state) =
                            states.find(|state| !state.context.asset_samples.is_empty())
                        {
                            return state.context.asset_samples[0].clock.clone();
                        }
                    }
                }
            }
            panic!("Active Flail sequence clock is missing");
        }
        fn reference_pose(
            In((handle, time)): In<(
                Handle<bevy_animation_graph::core::animation_graph::AnimationGraph>,
                f32,
            )>,
            resources: bevy_animation_graph::core::context::system_resources::SystemResources,
        ) -> Pose {
            use bevy_animation_graph::core::{
                animation_graph::TimeUpdate,
                context::{
                    deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                    graph_context_arena::GraphContextArena, new_context::GraphContext,
                },
                utils::testing::TestIoEnv,
            };
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = bevy::platform::collections::HashMap::new();
            let inputs = bevy::platform::collections::HashMap::from([(
                RAGDOLLING_FLAIL_PLAY_RATE_KEY.to_owned(),
                DataValue::F32(1.0),
            )]);
            let env = TestIoEnv::default();
            let output = graph
                .query_with_context(
                    QueryOutputTime::Forced(TimeUpdate::Absolute(time)),
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
            output
                .values()
                .find_map(|value| value.as_pose().ok())
                .unwrap()
                .clone()
        }
        #[derive(Resource, Default)]
        struct SourceVelocity(f32);
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut()
            .get_mut::<crate::als::transition::AlsStateRequest>(character)
            .unwrap()
            .ragdoll_start = true;
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(
            app.world().get::<AlsNotifyLocomotionAction>(character),
            Some(&AlsNotifyLocomotionAction::Ragdolling)
        );
        let player = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let spawned = app
            .world()
            .get::<AnimationGraphPlayer>(player)
            .unwrap()
            .get_spawned_ragdoll()
            .unwrap();
        let pelvis = spawned
            .bodies
            .values()
            .copied()
            .find(|entity| {
                app.world()
                    .get::<BodyLabel>(*entity)
                    .is_some_and(|label| label.0 == PELVIS_LABEL)
            })
            .unwrap();
        let reference: Handle<bevy_animation_graph::core::animation_graph::AnimationGraph> = app
            .world()
            .resource::<AssetServer>()
            .load("ALS/Character/AnimationInstances/AB_Als_Ragdolling/AnimGraph.animgraph.ron");
        let mapping: Handle<bevy_animation_graph::core::ragdoll::bone_mapping::RagdollBoneMap> =
            app.world()
                .resource::<AssetServer>()
                .load("ALS/Character/PA_Als.bm.ron");
        let sequence=app.world().resource::<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>().get(&reference).unwrap()
            .nodes.values().find(|node|node.try_inner_downcast_ref::<crate::extensions::source_sequence_player::SourceSequencePlayerNode>()
                .is_some_and(|node|node.play_rate_key==RAGDOLLING_FLAIL_PLAY_RATE_KEY)).unwrap().clone();
        let mut reference_graph =
            bevy_animation_graph::core::animation_graph::AnimationGraph::new();
        reference_graph.add_node(sequence.clone());
        reference_graph.add_output_data(
            "Pose".to_owned(),
            bevy_animation_graph::core::edge_data::DataSpec::Pose,
        );
        reference_graph.add_output_data_edge(sequence.id, "Pose", "Pose");
        let reference = app
            .world_mut()
            .resource_mut::<Assets<bevy_animation_graph::core::animation_graph::AnimationGraph>>()
            .add(reference_graph);
        let before_clock = flail_clock(app.world(), player);
        app.world_mut().get_mut::<LinearVelocity>(pelvis).unwrap().0 = Vec3::new(3.0, 4.0, 0.0);
        app.init_resource::<SourceVelocity>().add_systems(
            PostUpdate,
            (move |velocities: Query<&LinearVelocity>, mut source: ResMut<SourceVelocity>| {
                source.0 = velocities.get(pelvis).unwrap().0.length();
            })
            .before(avian3d::prelude::PhysicsSystems::First),
        );
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ));
        app.update();
        let source_speed = app.world().resource::<SourceVelocity>().0;
        let source_rate = (source_speed / 10.0).clamp(0.0, 1.0);
        let after_clock = flail_clock(app.world(), player);
        let source_position = (before_clock.position
            + app.world().resource::<Time>().delta_secs() * source_rate)
            .rem_euclid(before_clock.duration);
        let reference_pose = app
            .world_mut()
            .run_system_cached_with(reference_pose, (reference, source_position))
            .unwrap();
        let mapped = app
            .world()
            .resource::<Assets<bevy_animation_graph::core::ragdoll::bone_mapping::RagdollBoneMap>>()
            .get(&mapping)
            .unwrap()
            .bones_from_bodies
            .keys()
            .map(|path| path.id())
            .collect::<std::collections::HashSet<_>>();
        let actual_pose = app
            .world()
            .get::<AnimationGraphPlayer>(player)
            .unwrap()
            .get_default_output_pose()
            .unwrap();
        let mut minimum_dot = 1.0_f32;
        let mut location_error = 0.0_f32;
        let mut compared = 0;
        let skeleton = app
            .world()
            .resource::<Assets<bevy_animation_graph::core::skeleton::Skeleton>>()
            .get(&reference_pose.skeleton)
            .unwrap();
        for bone in reference_pose
            .paths
            .keys()
            .filter(|bone| !mapped.contains(*bone))
        {
            let path = skeleton.id_to_path(*bone).unwrap();
            if path
                .parts
                .last()
                .is_some_and(|name| name.starts_with("ik_") || name.starts_with("VB "))
            {
                continue;
            }
            let Some(actual) = actual_pose.get_bone(*bone) else {
                continue;
            };
            let source = reference_pose.get_bone(*bone).unwrap();
            if let (Some(a), Some(s)) = (actual.rotation, source.rotation) {
                minimum_dot = minimum_dot.min(a.dot(s).abs());
            }
            if let (Some(a), Some(s)) = (actual.translation, source.translation) {
                location_error = location_error.max(a.distance(s));
            }
            compared += 1;
        }
        assert!(compared > 0);
        assert!(
            minimum_dot > 1.0 - 1e-5 && location_error < 1e-5,
            "Flail physics snapshot changes actual animated bones: compared={compared}, source_time={source_position}, actual_time={}, rotation_error_degrees={}, location_error={location_error}",
            after_clock.position,
            (2.0 * minimum_dot.clamp(-1.0, 1.0).acos()).to_degrees()
        );
        assert!(
            (after_clock.position - source_position).abs() < 1e-6,
            "Actual Flail sequence samples the wrong physics snapshot: source_rate={source_rate}, source_position={source_position}, actual_position={}, playback_delta={}",
            after_clock.position,
            after_clock.playback_delta_seconds
        );
        let player = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        assert!(player.get_error().is_none() && player.get_default_output_pose().is_some());
        let actual = player
            .get_global_input_data(RAGDOLLING_FLAIL_PLAY_RATE_KEY)
            .unwrap()
            .as_f32()
            .unwrap();
        assert!(
            (actual - source_rate).abs() < 1e-6,
            "Active Flail input samples the wrong physics phase: source_speed={source_speed}, source_rate={source_rate}, actual={actual}, after_velocity={:?}",
            app.world().get::<LinearVelocity>(pelvis).unwrap().0
        );
    }

    #[test]
    fn flail_rate_matches_source_reference_speed() {
        assert_eq!(flail_play_rate_for_speed(-1.0), 0.0);
        assert!((flail_play_rate_for_speed(5.0) - 0.5).abs() < 1e-6);
        assert_eq!(flail_play_rate_for_speed(10.0), 1.0);
        assert_eq!(flail_play_rate_for_speed(25.0), 1.0);
    }

    #[test]
    fn source_animation_state_default_is_one() {
        assert_eq!(AlsRagdollingAnimationState::default().flail_play_rate, 1.0);
    }
}
