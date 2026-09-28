use std::collections::HashSet;

use avian3d::prelude::{PhysicsSystems, Position, Rotation};
use bevy::{animation::{AnimatedBy, AnimationTargetId}, ecs::{query::QueryFilter, schedule::ScheduleCleanupPolicy}, prelude::*, transform::TransformSystems};
use bevy_animation_graph::core::{
    animated_scene::{AnimatedScene, AnimatedSceneHandle, AnimatedSceneInstance},
    animation_graph::DEFAULT_OUTPUT_RAGDOLL_CONFIG,
    animation_graph_player::AnimationGraphPlayer,
    context::system_resources::SystemResources,
    id::BoneId,
    physics_systems_avian::{read_back_poses_avian, spawn_missing_ragdolls_avian},
    plugin::AnimationGraphSet,
    pose::Pose,
    ragdoll::{bone_mapping::RagdollBoneMap, configuration::RagdollConfig, spawning::SpawnedRagdoll},
    skeleton::Skeleton,
    systems::{RootMotionOutput, animation_player, debug_draw_animation_players, extract_root_motion, run_animation_player},
};

#[derive(Component)]
pub struct PostPhysicsAnimation;

#[derive(Component)]
pub struct ManualAnimationPlayer;

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
            let count = schedule.remove_systems_in_set(read_back_poses_avian, world,
                ScheduleCleanupPolicy::RemoveSystemsOnly).expect("physics readback schedule must initialize");
            assert_eq!(count, 1, "physics readback must replace exactly one default readback system");
        });
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

fn normalize_ragdoll_pose_scale(pose: &mut Pose, map: &RagdollBoneMap, scale: f32, readback: &HashSet<BoneId>) {
    for path in map.bones_from_bodies.keys() {
        if !readback.contains(&path.id()) { continue; }
        let Some(index) = pose.paths.get(&path.id()).copied() else { continue; };
        let mut parent = path.parent();
        let mut readback_ancestor = false;
        while let Some(ancestor) = parent {
            if readback.contains(&ancestor.id()) {
                readback_ancestor = true;
                break;
            }
            parent = ancestor.parent();
        }
        let bone = &mut pose.bones[index];
        if readback_ancestor {
            if let Some(translation) = bone.translation.as_mut() { *translation /= scale; }
        } else if let Some(local_scale) = bone.scale.as_mut() {
            *local_scale *= scale;
        }
    }
}

fn normalize_scaled_readback_pose(
    mut players: Query<(Entity, &mut AnimationGraphPlayer, &GlobalTransform)>,
    hosts: Query<(&AnimatedSceneHandle, &AnimatedSceneInstance)>,
    scenes: Option<Res<Assets<AnimatedScene>>>,
    skeletons: Option<Res<Assets<Skeleton>>>,
    maps: Res<Assets<RagdollBoneMap>>,
    bodies: Query<(&Position, &Rotation)>,
) {
    let (Some(scenes), Some(skeletons)) = (scenes, skeletons) else { return; };
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
        if skeletons.get(&pose.skeleton).is_none() { continue; }
        let Some(spawned) = player.get_spawned_ragdoll() else { continue; };
        let config = player.get_outputs().get(DEFAULT_OUTPUT_RAGDOLL_CONFIG)
            .and_then(|value| value.as_ragdoll_config().ok()).cloned().unwrap_or_default();
        let readback = successful_readback_bones(map, spawned, &config, &bodies);
        if readback.is_empty() { continue; }
        normalize_ragdoll_pose_scale(&mut pose, map, scale.x, &readback);
        player.set_default_output_pose(pose);
    }
}

fn apply_readback_pose(
    mut targets: Query<(&mut Transform, &AnimationTargetId, &AnimatedBy)>,
    players: Query<&AnimationGraphPlayer>,
) {
    for (mut transform, id, animated_by) in &mut targets {
        let Ok(player) = players.get(animated_by.0) else { continue; };
        if !has_physics_readback(player) { continue; }
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
    mut players: Query<(Entity, &mut AnimationGraphPlayer, &mut crate::extensions::SourceAnimationInputs), (F, Without<ManualAnimationPlayer>)>,
    resources: SystemResources,
) {
    players.par_iter_mut().for_each(|(entity, player, inputs)| inputs.evaluate(entity, || run_animation_player(entity, player, &time, &resources)));
    players.par_iter_mut().for_each(|(_, player, mut inputs)| {
        debug_draw_animation_players(player, &resources);
        inputs.set_rig_query_context(None);
    });
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
    fn scaled_readback_restores_root_scale_and_mapped_child_offsets() {
        let root = EntityPath::from_slashed_string("GltfNode0/root".into());
        let pelvis = root.child("pelvis");
        let unmapped = pelvis.child("spine_01");
        let map = RagdollBoneMap {
            bones_from_bodies: [(root.clone(), BoneMapping::default()),
                (pelvis.clone(), BoneMapping::default())].into(),
            bodies_from_bones: default(), skeleton: default(), ragdoll: default(),
            skeleton_symmetry: default(),
        };
        let mut pose = Pose::default();
        pose.add_bone(BonePose { translation: Some(Vec3::ZERO), scale: Some(Vec3::splat(0.5)), ..default() }, root.id());
        pose.add_bone(BonePose { translation: Some(Vec3::Y * 2.0), scale: Some(Vec3::ONE), ..default() }, pelvis.id());
        pose.add_bone(BonePose { translation: Some(Vec3::X), scale: Some(Vec3::ONE), ..default() }, unmapped.id());
        normalize_ragdoll_pose_scale(&mut pose, &map, 2.0, &HashSet::from([root.id(), pelvis.id()]));
        assert_eq!(pose.bones[pose.paths[&root.id()]].scale, Some(Vec3::ONE));
        assert_eq!(pose.bones[pose.paths[&pelvis.id()]].translation, Some(Vec3::Y));
        assert_eq!(pose.bones[pose.paths[&unmapped.id()]].translation, Some(Vec3::X));
    }

    #[test]
    fn masked_scaled_readback_keeps_animation_bones_and_uses_actual_parent_space() {
        let root = EntityPath::from_slashed_string("GltfNode0/root".into());
        let child = root.child("pelvis");
        let map = RagdollBoneMap {
            bones_from_bodies: [(root.clone(), BoneMapping::default()),
                (child.clone(), BoneMapping::default())].into(),
            bodies_from_bones: default(), skeleton: default(), ragdoll: default(),
            skeleton_symmetry: default(),
        };
        for root_readback in [false, true] {
            for child_readback in [false, true] {
                let mut pose = Pose::default();
                let root_scale = if root_readback { 0.5 } else { 1.25 };
                let child_translation = if child_readback { 2.0 } else { 0.4 };
                let child_scale = if child_readback { 1.0 } else { 1.3 };
                pose.add_bone(BonePose { translation: Some(Vec3::ZERO),
                    scale: Some(Vec3::splat(root_scale)), ..default() }, root.id());
                pose.add_bone(BonePose { translation: Some(Vec3::Y * child_translation),
                    scale: Some(Vec3::splat(child_scale)), ..default() }, child.id());
                let mut readback = HashSet::new();
                if root_readback { readback.insert(root.id()); }
                if child_readback { readback.insert(child.id()); }
                normalize_ragdoll_pose_scale(&mut pose, &map, 2.0, &readback);
                let actual_root = &pose.bones[pose.paths[&root.id()]];
                let actual_child = &pose.bones[pose.paths[&child.id()]];
                assert_eq!(actual_root.scale, Some(Vec3::splat(if root_readback { 1.0 } else { 1.25 })),
                    "root={root_readback} child={child_readback}");
                assert_eq!(actual_child.translation, Some(Vec3::Y * if root_readback && child_readback { 1.0 }
                    else { child_translation }), "root={root_readback} child={child_readback}");
                assert_eq!(actual_child.scale, Some(Vec3::splat(if child_readback && !root_readback { 2.0 }
                    else { child_scale })), "root={root_readback} child={child_readback}");
            }
        }
    }

    #[test]
    fn successful_readback_tracks_config_entities_and_unmapped_middle_bones() {
        use avian3d::prelude::{Position, Rotation};
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::ragdoll::{bone_mapping::BodyWeight, definition::Body};

        fn paths(
            In((map, spawned, config)): In<(RagdollBoneMap, SpawnedRagdoll, RagdollConfig)>,
            bodies: Query<(&Position, &Rotation)>,
        ) -> HashSet<BoneId> {
            successful_readback_bones(&map, &spawned, &config, &bodies)
        }

        let root = EntityPath::from_slashed_string("GltfNode0/root".into());
        let middle = root.child("spine_01");
        let child = middle.child("pelvis");
        let root_body = Body::new().id;
        let child_body = Body::new().id;
        let map = RagdollBoneMap {
            bones_from_bodies: [
                (root.clone(), BoneMapping { bone_id: root.clone(),
                    bodies: vec![BodyWeight { body: root_body, weight: 1.0, ..default() }], ..default() }),
                (child.clone(), BoneMapping { bone_id: child.clone(),
                    bodies: vec![BodyWeight { body: child_body, weight: 1.0, ..default() }], ..default() }),
            ].into(),
            bodies_from_bones: default(), skeleton: default(), ragdoll: default(),
            skeleton_symmetry: default(),
        };
        let mut world = World::new();
        let root_entity = world.spawn((Position(Vec3::ZERO), Rotation(Quat::IDENTITY))).id();
        let child_entity = world.spawn((Position(Vec3::Y), Rotation(Quat::IDENTITY))).id();
        let spawned = || {
            let mut value = SpawnedRagdoll::new(Entity::PLACEHOLDER);
            value.bodies.insert(root_body, root_entity);
            value.bodies.insert(child_body, child_entity);
            value
        };
        let full = world.run_system_once_with(paths, (map.clone(), spawned(), RagdollConfig::default())).unwrap();
        assert_eq!(full, HashSet::from([root.id(), child.id()]));
        let only_child = world.run_system_once_with(paths, (map.clone(), spawned(), RagdollConfig {
            default_readback: Some(false), readback_overrides: [(child.id(), true)].into(), ..default()
        })).unwrap();
        assert_eq!(only_child, HashSet::from([child.id()]));
        let all_off = world.run_system_once_with(paths, (map.clone(), spawned(), RagdollConfig {
            default_readback: Some(false), ..default()
        })).unwrap();
        assert!(all_off.is_empty());

        let mut pose = Pose::default();
        pose.add_bone(BonePose { scale: Some(Vec3::splat(0.5)), ..default() }, root.id());
        pose.add_bone(BonePose { translation: Some(Vec3::X), scale: Some(Vec3::ONE), ..default() }, middle.id());
        pose.add_bone(BonePose { translation: Some(Vec3::Y * 2.0), scale: Some(Vec3::ONE), ..default() }, child.id());
        let mut full_pose = pose.clone();
        normalize_ragdoll_pose_scale(&mut full_pose, &map, 2.0, &full);
        assert_eq!(full_pose.bones[full_pose.paths[&root.id()]].scale, Some(Vec3::ONE));
        assert_eq!(full_pose.bones[full_pose.paths[&middle.id()]].translation, Some(Vec3::X));
        assert_eq!(full_pose.bones[full_pose.paths[&child.id()]].translation, Some(Vec3::Y));
        normalize_ragdoll_pose_scale(&mut pose, &map, 2.0, &only_child);
        assert_eq!(pose.bones[pose.paths[&root.id()]].scale, Some(Vec3::splat(0.5)));
        assert_eq!(pose.bones[pose.paths[&middle.id()]].translation, Some(Vec3::X));
        assert_eq!(pose.bones[pose.paths[&child.id()]].translation, Some(Vec3::Y * 2.0));
        assert_eq!(pose.bones[pose.paths[&child.id()]].scale, Some(Vec3::splat(2.0)));

        world.despawn(child_entity);
        let missing = world.run_system_once_with(paths, (map, spawned(), RagdollConfig::default())).unwrap();
        assert_eq!(missing, HashSet::from([root.id()]));
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
