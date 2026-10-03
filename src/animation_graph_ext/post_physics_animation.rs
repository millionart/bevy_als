use std::collections::HashSet;

use avian3d::prelude::{PhysicsSystems, Position, Rotation};
use bevy::{animation::{AnimatedBy, AnimationTargetId}, ecs::{query::QueryFilter, schedule::ScheduleCleanupPolicy}, prelude::*, transform::TransformSystems};
use bevy_animation_graph::core::{
    animated_scene::{AnimatedScene, AnimatedSceneHandle, AnimatedSceneInstance},
    animation_graph::DEFAULT_OUTPUT_RAGDOLL_CONFIG,
    animation_graph_player::AnimationGraphPlayer,
    context::{pose_fallback::PoseFallbackContext, system_resources::SystemResources},
    id::BoneId,
    physics_systems_avian::{read_back_poses_avian, spawn_missing_ragdolls_avian},
    plugin::AnimationGraphSet,
    pose::Pose,
    ragdoll::{bone_mapping::RagdollBoneMap, configuration::RagdollConfig, spawning::SpawnedRagdoll},
    skeleton::Skeleton,
    space_conversion::SpaceConversionContext,
    systems::{RootMotionOutput, animation_player, debug_draw_animation_players, extract_root_motion, run_animation_player},
};

#[derive(Component)]
pub struct PostPhysicsAnimation;

#[derive(Component)]
pub struct ManualAnimationPlayer;

#[derive(Component, Clone, Copy, Debug)]
pub struct AnimationPoseUpdateGate {
    pub update_graph: bool,
    pub refresh_bones: bool,
    pub pending_update: bool,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostPhysicsAnimationSet {
    Inputs,
    Evaluate,
    Readback,
}

pub struct PostPhysicsAnimationPlugin;

impl Plugin for PostPhysicsAnimationPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<crate::extensions::SourceAnimationInputsPlugin>() { app.add_plugins(crate::extensions::SourceAnimationInputsPlugin); }
        app.configure_sets(PostUpdate, (
            PostPhysicsAnimationSet::Inputs.after(PhysicsSystems::Last),
            PostPhysicsAnimationSet::Evaluate.after(PostPhysicsAnimationSet::Inputs)
                .before(AnimationGraphSet::PostPhysics),
            PostPhysicsAnimationSet::Readback.after(AnimationGraphSet::Final)
                .after(TransformSystems::Propagate),
        )).add_systems(PostUpdate,
            evaluate_players::<Without<PostPhysicsAnimation>>
                .after(spawn_missing_ragdolls_avian).before(extract_root_motion)
                .in_set(AnimationGraphSet::PrePhysics),
        ).add_systems(PostUpdate,
            (evaluate_players::<With<PostPhysicsAnimation>>, extract_late_root_motion)
                .chain().in_set(PostPhysicsAnimationSet::Evaluate),
        );
    }

    fn finish(&self, app: &mut App) {
        app.world_mut().schedule_scope(PostUpdate, |world, schedule| {
            let count = schedule.remove_systems_in_set(animation_player, world,
                ScheduleCleanupPolicy::RemoveSystemsOnly).expect("animation evaluation schedule must initialize");
            assert_eq!(count, 1, "animation evaluation must replace exactly one default player system");
            let count = schedule.remove_systems_in_set(bevy_animation_graph::core::systems::apply_animation_to_targets, world,
                ScheduleCleanupPolicy::RemoveSystemsOnly).expect("bone refresh schedule must initialize");
            assert_eq!(count, 1, "bone refresh must replace exactly one default player system");
            let count = schedule.remove_systems_in_set(read_back_poses_avian, world,
                ScheduleCleanupPolicy::RemoveSystemsOnly).expect("physics readback schedule must initialize");
            assert_eq!(count, 1, "physics readback must replace exactly one default readback system");
        });
        app.add_systems(PostUpdate, apply_animation_to_visible_targets.in_set(AnimationGraphSet::Final));
        app.add_systems(PostUpdate,
            (read_back_poses_avian, normalize_scaled_readback_pose, apply_readback_pose)
                .chain().in_set(PostPhysicsAnimationSet::Readback),
        );
    }
}

fn has_physics_readback(player: &AnimationGraphPlayer) -> bool {
    player.get_spawned_ragdoll().is_some() && player.get_outputs().get(DEFAULT_OUTPUT_RAGDOLL_CONFIG)
        .and_then(|value| value.as_ragdoll_config().ok())
        .is_none_or(|config| config.default_readback.unwrap_or(true) || config.readback_overrides.values().any(|enabled| *enabled))
}

fn successful_readback_bones(
    map: &RagdollBoneMap,
    spawned: &SpawnedRagdoll,
    config: &RagdollConfig,
    bodies: &Query<(&Position, &Rotation)>,
) -> HashSet<BoneId> {
    map.bones_from_bodies.iter().filter_map(|(path, mapping)| {
        if !config.should_readback(path.id()).unwrap_or(true) { return None; }
        mapping.bodies.iter().any(|body| spawned.bodies.get(&body.body)
            .is_some_and(|entity| bodies.get(*entity).is_ok())).then_some(path.id())
    }).collect()
}

fn normalize_ragdoll_pose_scale(
    pose: &mut Pose, map: &RagdollBoneMap, skeleton: &Skeleton,
    conversions: SpaceConversionContext, scale: f32, readback: &HashSet<BoneId>,
) {
    let mut targets = map.bones_from_bodies.keys().filter(|path| readback.contains(&path.id()))
        .filter_map(|path| {
            let index = *pose.paths.get(&path.id())?;
            let mut target = conversions.global_transform_of_bone(pose, skeleton, path.id());
            target.scale *= scale;
            Some((path, index, target))
        }).collect::<Vec<_>>();
    targets.sort_by_key(|(path, _, _)| path.parts.len());
    for (path, index, target) in targets {
        let parent = path.parent().map(|path|
            conversions.global_transform_of_bone(pose, skeleton, path.id())).unwrap_or_default();
        let local = Transform::from_matrix(parent.compute_affine().inverse().into()) * target;
        let bone = &mut pose.bones[index];
        bone.translation = Some(local.translation);
        bone.rotation = Some(local.rotation);
        bone.scale = Some(local.scale);
    }
}

fn normalize_scaled_readback_pose(
    mut players: Query<(Entity, &mut AnimationGraphPlayer, &GlobalTransform)>,
    hosts: Query<(&AnimatedSceneHandle, &AnimatedSceneInstance)>,
    scenes: Option<Res<Assets<AnimatedScene>>>,
    maps: Res<Assets<RagdollBoneMap>>,
    bodies: Query<(&Position, &Rotation)>,
    resources: SystemResources,
) {
    let Some(scenes) = scenes else { return; };
    for (entity, mut player, world) in &mut players {
        if !has_physics_readback(&player) { continue; }
        let scale = world.to_scale_rotation_translation().0;
        if !scale.is_finite() || scale.min_element() <= 0.0
            || scale.max_element() - scale.min_element() > 1e-4
            || (scale.x - 1.0).abs() <= 1e-4 { continue; }
        let Some(map) = hosts.iter().find(|(_, instance)| instance.player_entity() == entity)
            .and_then(|(handle, _)| scenes.get(&handle.handle))
            .and_then(|scene| scene.ragdoll_bone_map.as_ref())
            .and_then(|handle| maps.get(handle)) else { continue; };
        let Some(mut pose) = player.get_default_output_pose().cloned() else { continue; };
        let Some(skeleton) = resources.skeleton_assets.get(&pose.skeleton) else { continue; };
        let Some(spawned) = player.get_spawned_ragdoll() else { continue; };
        let config = player.get_outputs().get(DEFAULT_OUTPUT_RAGDOLL_CONFIG)
            .and_then(|value| value.as_ragdoll_config().ok()).cloned().unwrap_or_default();
        let readback = successful_readback_bones(map, spawned, &config, &bodies);
        if readback.is_empty() { continue; }
        let entities = skeleton.iter_bones().filter_map(|bone| player.bone_entity(bone).map(|entity| (bone, entity))).collect();
        let conversions = SpaceConversionContext { pose_fallback: PoseFallbackContext {
            entity_map: &entities, resources: &resources, fallback_to_identity: true,
        }};
        normalize_ragdoll_pose_scale(&mut pose, map, skeleton, conversions, scale.x, &readback);
        player.set_default_output_pose(pose);
    }
}

fn apply_readback_pose(
    mut targets: Query<(&mut Transform, &AnimationTargetId, &AnimatedBy)>,
    players: Query<(&AnimationGraphPlayer, Option<&AnimationPoseUpdateGate>)>,
) {
    for (mut transform, id, animated_by) in &mut targets {
        let Ok((player, gate)) = players.get(animated_by.0) else { continue; };
        if gate.is_some_and(|gate| !gate.refresh_bones) || !has_physics_readback(player) { continue; }
        let Some(pose) = player.get_default_output_pose() else { continue; };
        let Some(index) = pose.paths.get(&BoneId::from(*id)) else { continue; };
        let bone = &pose.bones[*index];
        if let Some(value) = bone.translation { transform.translation = value; }
        if let Some(value) = bone.rotation { transform.rotation = value; }
        if let Some(value) = bone.scale { transform.scale = value; }
    }
}

pub fn evaluate_players<F: QueryFilter>(
    time: Res<Time>,
    mut players: Query<(Entity, &mut AnimationGraphPlayer, &mut crate::extensions::SourceAnimationInputs, Option<&mut AnimationPoseUpdateGate>), (F, Without<ManualAnimationPlayer>)>,
    resources: SystemResources,
) {
    players.par_iter_mut().for_each(|(entity, mut player, inputs, mut gate)| {
        if gate.as_ref().is_some_and(|gate| !gate.update_graph) { return; }
        let playing = !player.is_paused();
        let initialize = gate.is_some() && player.get_context_arena().is_none_or(|arena| arena.frame_index() == 0);
        if initialize {
            let delta = player.get_global_input_data("animation.delta_seconds").cloned();
            player.set_global_input_data("animation.delta_seconds", 0.0_f32);
            inputs.evaluate(entity, || run_animation_player(entity, player.reborrow(), &Time::default(), &resources));
            if let Some(delta) = delta { player.set_global_input_data("animation.delta_seconds", delta); }
        } else {
            inputs.evaluate(entity, || run_animation_player(entity, player.reborrow(), &time, &resources));
        }
        if playing && player.get_error().is_none() && player.get_default_output_pose().is_some()
            && let Some(gate) = gate.as_mut() { gate.pending_update = false; }
    });
    players.par_iter_mut().for_each(|(_, player, mut inputs, _)| {
        debug_draw_animation_players(player, &resources);
        inputs.set_rig_query_context(None);
    });
}

fn apply_animation_to_visible_targets(
    mut targets: Query<(&mut Transform, Option<&mut bevy::mesh::morph::MorphWeights>, &AnimationTargetId, &AnimatedBy)>,
    players: Query<(&AnimationGraphPlayer, Option<&AnimationPoseUpdateGate>)>,
) {
    for (mut transform, morphs, id, animated_by) in &mut targets {
        let Ok((player, gate)) = players.get(animated_by.0) else { continue; };
        if gate.is_some_and(|gate| !gate.refresh_bones) { continue; }
        let Some(pose) = player.get_default_output_pose() else { continue; };
        let Some(bone) = pose.get_bone(BoneId::from(*id)) else { continue; };
        if let Some(value) = bone.translation { transform.translation = value; }
        if let Some(value) = bone.rotation { transform.rotation = value; }
        if let Some(value) = bone.scale { transform.scale = value; }
        if let Some(weights) = &bone.weights && let Some(mut morphs) = morphs {
            for (target, source) in morphs.weights_mut().iter_mut().zip(weights) { *target = *source; }
        }
    }
}

fn extract_late_root_motion(
    with_player: Query<(&AnimationGraphPlayer, &mut RootMotionOutput)>,
    with_scene: Query<(&AnimatedSceneInstance, &mut RootMotionOutput), Without<AnimationGraphPlayer>>,
    players: Query<(&AnimationGraphPlayer, &Transform), Without<RootMotionOutput>>,
) {
    extract_root_motion(with_player, with_scene, players);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_animation_graph::core::{
        animation_clip::{EntityPath, GraphClip}, animation_graph::AnimationGraph, animation_node::AnimationNode,
        edge_data::{DataSpec, DataValue}, pose::{BonePose, Pose, RootMotionDelta},
        ragdoll::bone_mapping::BoneMapping,
        skeleton::Skeleton, state_machine::high_level::StateMachine,
    };

    #[test]
    fn bone_visibility_gate_keeps_the_last_transforms_until_refresh_is_allowed() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let id = AnimationTargetId::from_name(&Name::new("bone"));
        let expected = Transform::from_xyz(1.0, 2.0, 3.0).with_rotation(Quat::from_rotation_y(0.4)).with_scale(Vec3::splat(2.0));
        let mut pose = Pose::default();
        pose.add_bone(BonePose::from_transform(expected), BoneId::from(id));
        let mut player = AnimationGraphPlayer::default();
        player.set_animation(bevy_animation_graph::core::animation_graph_player::AnimationSource::Pose(pose));
        let player = world.spawn((player, AnimationPoseUpdateGate { update_graph: true, refresh_bones: false, pending_update: false })).id();
        let bone = world.spawn((Transform::IDENTITY, id, AnimatedBy(player))).id();
        world.run_system_once(apply_animation_to_visible_targets).unwrap();
        assert_eq!(*world.get::<Transform>(bone).unwrap(), Transform::IDENTITY);
        world.get_mut::<AnimationPoseUpdateGate>(player).unwrap().refresh_bones = true;
        world.run_system_once(apply_animation_to_visible_targets).unwrap();
        assert_eq!(*world.get::<Transform>(bone).unwrap(), expected);
    }

    #[test]
    fn scaled_readback_preserves_physics_world_transforms_across_animated_gaps() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::platform::collections::HashMap;
        use bevy_animation_graph::core::{
            context::pose_fallback::PoseFallbackContext,
            ragdoll::{bone_mapping::BodyWeight, definition::Body, read_pose_avian::read_pose},
            space_conversion::SpaceConversionContext,
        };

        fn readback(
            In((input, map, spawned, config, entities, ids, scale)): In<(Pose, RagdollBoneMap, SpawnedRagdoll,
                RagdollConfig, HashMap<BoneId, Entity>, [BoneId; 3], f32)>,
            resources: SystemResources, bodies: Query<(&Position, &Rotation)>,
        ) -> (Pose, [Transform; 3], [Transform; 3]) {
            let skeleton = resources.skeleton_assets.get(&input.skeleton).unwrap();
            let fallback = PoseFallbackContext { entity_map: &entities, resources: &resources, fallback_to_identity: true };
            let conversions = SpaceConversionContext { pose_fallback: fallback };
            let mut pose = read_pose(&spawned, &map, skeleton, &bodies, fallback, &input, &config);
            let before = ids.map(|id| conversions.global_transform_of_bone(&pose, skeleton, id));
            let readback = successful_readback_bones(&map, &spawned, &config, &bodies);
            normalize_ragdoll_pose_scale(&mut pose, &map, skeleton, conversions, scale, &readback);
            let after = ids.map(|id| conversions.global_transform_of_bone(&pose, skeleton, id));
            (pose, before, after)
        }

        let scene = EntityPath::from_slashed_string("GltfNode0".into());
        let root = scene.child("root");
        let middle = root.child("middle");
        let child = middle.child("child");
        let paths = [root, middle, child];
        let ids = paths.each_ref().map(|path| path.id());
        for complex in [false, true] {
            for scale in [2.0, 0.5, 1.0] {
                for root_readback in [true, false] {
                    for child_readback in [true, false] {
                        for gap in ["unmapped", "masked", "missing", "unspawned", "read"] {
                            let mut world = World::new();
                            world.init_resource::<Assets<AnimationGraph>>();
                            world.init_resource::<Assets<GraphClip>>();
                            world.init_resource::<Assets<StateMachine>>();
                            world.init_resource::<Assets<Skeleton>>();
                            let scene_transform = Transform {
                                translation: if complex { Vec3::new(3.0, 2.0, -1.0) } else { Vec3::ZERO },
                                rotation: if complex { Quat::from_euler(EulerRot::YXZ, 0.4, -0.2, 0.1) } else { Quat::IDENTITY },
                                scale: Vec3::splat(scale),
                            };
                            let locals = [if complex { Transform::from_xyz(0.0, 0.3, 0.0)
                                    .with_rotation(Quat::from_rotation_y(0.15)).with_scale(Vec3::splat(1.25)) } else { Transform::IDENTITY },
                                Transform::from_xyz(1.0, if complex { 0.2 } else { 0.0 }, 0.0)
                                    .with_rotation(if complex { Quat::from_rotation_z(0.25) } else { Quat::IDENTITY })
                                    .with_scale(Vec3::splat(if complex { 1.1 } else { 1.0 })),
                                Transform::from_translation(Vec3::X)];
                            let scene_entity = world.spawn((scene_transform, GlobalTransform::from(scene_transform))).id();
                            let mut entities = HashMap::from([(scene.id(), scene_entity)]);
                            let mut skeleton = Skeleton::default();
                            skeleton.add_bone(scene.clone(), Transform::IDENTITY, Transform::IDENTITY);
                            skeleton.set_root(scene.id());
                            let mut character = Transform::IDENTITY;
                            for index in 0..3 {
                                character = character * locals[index];
                                skeleton.add_bone(paths[index].clone(), locals[index], character);
                                let entity = world.spawn((locals[index], GlobalTransform::from(scene_transform * character))).id();
                                entities.insert(ids[index], entity);
                            }
                            let skeleton = world.resource_mut::<Assets<Skeleton>>().add(skeleton);
                            let mut input = Pose { skeleton: skeleton.clone(), ..default() };
                            for index in 0..3 { input.add_bone(BonePose::from_transform(locals[index]), ids[index]); }
                            let mut map = RagdollBoneMap { skeleton, ragdoll: default(), skeleton_symmetry: default(),
                                bones_from_bodies: default(), bodies_from_bones: default() };
                            let mut spawned = SpawnedRagdoll::new(scene_entity);
                            let positions = [Vec3::ZERO, Vec3::X * 0.5, Vec3::X * 2.0]
                                .map(|point| scene_transform.transform_point(point));
                            let rotations = [0.0, 0.1, -0.15].map(|angle|
                                scene_transform.rotation * if complex { Quat::from_rotation_x(angle) } else { Quat::IDENTITY });
                            for index in 0..3 {
                                let body = Body::new().id;
                                let entity = world.spawn((Position(positions[index]), Rotation(rotations[index]))).id();
                                if index != 1 || gap != "unspawned" { spawned.bodies.insert(body, entity); }
                                if index != 1 || gap != "unmapped" {
                                    map.bones_from_bodies.insert(paths[index].clone(), BoneMapping {
                                        bone_id: paths[index].clone(), bodies: vec![BodyWeight { body, weight: 1.0, ..default() }], ..default()
                                    });
                                }
                                if index == 1 && gap == "missing" { world.despawn(entity); }
                            }
                            let config = RagdollConfig { default_readback: Some(false),
                                readback_overrides: [(ids[0], root_readback), (ids[1], gap != "masked"), (ids[2], child_readback)].into(), ..default() };
                            let (pose, before, after) = world.run_system_once_with(readback,
                                (input.clone(), map, spawned, config, entities, ids, scale)).unwrap();
                            for (index, enabled) in [root_readback, gap == "read", child_readback].into_iter().enumerate() {
                                if enabled {
                                    assert!(before[index].translation.distance(positions[index]) < 1e-4, "Raw physics readback must align");
                                    assert!(after[index].translation.distance(positions[index]) < 1e-4,
                                        "complex={complex} scale={scale} root={root_readback} child={child_readback} gap={gap} bone={index}: {} != {}",
                                        after[index].translation, positions[index]);
                                    assert!(after[index].rotation.abs_diff_eq(rotations[index], 1e-5)
                                        || after[index].rotation.abs_diff_eq(-rotations[index], 1e-5));
                                    assert!(after[index].scale.distance(Vec3::splat(scale)) < 1e-4);
                                } else {
                                    let actual = &pose.bones[pose.paths[&ids[index]]];
                                    let expected = &input.bones[input.paths[&ids[index]]];
                                    assert_eq!(actual.translation, expected.translation);
                                    assert_eq!(actual.rotation, expected.rotation);
                                    assert_eq!(actual.scale, expected.scale);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn marked_players_evaluate_once_after_physics_while_samplers_stay_before_it() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<GraphClip>>().init_resource::<Assets<AnimationGraph>>()
            .init_resource::<Assets<Skeleton>>().init_resource::<Assets<StateMachine>>()
            .init_resource::<Assets<bevy_animation_graph::core::ragdoll::bone_mapping::RagdollBoneMap>>()
            .add_systems(PostUpdate, (animation_player, extract_root_motion).chain().in_set(AnimationGraphSet::PrePhysics))
            .add_systems(PostUpdate, read_back_poses_avian.in_set(AnimationGraphSet::PostPhysics))
            .add_systems(PostUpdate, bevy_animation_graph::core::systems::apply_animation_to_targets.in_set(AnimationGraphSet::Final))
            .configure_sets(PostUpdate, (
                AnimationGraphSet::PrePhysics.before(PhysicsSystems::First),
                PhysicsSystems::Last.after(PhysicsSystems::First),
                AnimationGraphSet::PostPhysics.after(PhysicsSystems::Last),
            )).add_plugins(PostPhysicsAnimationPlugin);
        let mut graph = AnimationGraph::new();
        let node = AnimationNode::new("input", crate::extensions::GlobalPoseInputNode { key: "input".into(), output: "Pose".into() });
        graph.add_node(node.clone());
        graph.add_output_data("pose".into(), DataSpec::Pose);
        graph.add_output_data_edge(node.id, "Pose", "pose");
        let handle = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(graph);
        let pose = |value| {
            let mut pose = Pose { timestamp: value, root_motion: Some(RootMotionDelta { translation: Vec3::X * value, rotation: Quat::IDENTITY }), ..default() };
            pose.add_bone(BonePose::from_transform(Transform::from_xyz(value, 0.0, 0.0)), default());
            pose
        };
        let mut player = AnimationGraphPlayer::new(default()).with_graph(handle.clone());
        player.set_global_input_data("input", DataValue::Pose(pose(0.0)));
        let sampler = app.world_mut().spawn((Name::new("sampler"), Transform::IDENTITY, GlobalTransform::IDENTITY, player, RootMotionOutput::default())).id();
        let mut player = AnimationGraphPlayer::new(default()).with_graph(handle);
        player.set_global_input_data("input", DataValue::Pose(pose(0.0)));
        let character = app.world_mut().spawn((Name::new("character"), Transform::IDENTITY, GlobalTransform::IDENTITY, player, RootMotionOutput::default(), PostPhysicsAnimation)).id();
        for parent in [sampler, character] {
            app.world_mut().spawn((Name::new("bone"), Transform::IDENTITY, GlobalTransform::IDENTITY, ChildOf(parent)));
        }
        let mut frame = 0;
        app.add_systems(PostUpdate, (move |mut players: Query<&mut AnimationGraphPlayer>, outputs: Query<&RootMotionOutput>| {
            assert_eq!(outputs.get(sampler).unwrap().translation_delta.x, frame as f32);
            frame += 1;
            for mut player in &mut players { player.set_global_input_data("input", DataValue::Pose(pose(frame as f32))); }
        }).in_set(PhysicsSystems::First));
        app.finish();
        app.cleanup();
        for frame in 1..=2 {
            for entity in [sampler, character] {
                app.world_mut().get_mut::<crate::extensions::SourceAnimationInputs>(entity).unwrap().set_rig_query_context(Some(crate::extensions::control_rig_query::RigQueryContext {
                    world: std::sync::Arc::new(crate::extensions::control_rig_query::RigQueryWorld { trees: default(), colliders: default() }),
                    to_world: default(), channel: "Visibility".into(), filter: default(),
                }));
            }
            app.world_mut().run_schedule(PostUpdate);
            for (entity, expected) in [(sampler, (frame - 1) as f32), (character, frame as f32)] {
                assert!(app.world().get::<crate::extensions::SourceAnimationInputs>(entity).unwrap().rig_query_context().is_none());
                let player = app.world().get::<AnimationGraphPlayer>(entity).unwrap();
                assert_eq!(player.get_default_output_pose().unwrap().timestamp, expected);
                assert_eq!(player.get_context_arena().unwrap().frame_index(), frame);
                assert_eq!(app.world().get::<RootMotionOutput>(entity).unwrap().translation_delta.x, expected);
            }
        }
        app.world_mut().entity_mut(character).remove::<PostPhysicsAnimation>();
        app.world_mut().run_schedule(PostUpdate);
        let player = app.world().get::<AnimationGraphPlayer>(character).unwrap();
        assert_eq!(player.get_default_output_pose().unwrap().timestamp, 2.0);
        assert_ne!(player.get_default_output_pose().unwrap().timestamp, 3.0);
        assert_eq!(player.get_context_arena().unwrap().frame_index(), 3);
    }
}
