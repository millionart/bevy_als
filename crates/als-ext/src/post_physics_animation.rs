use avian3d::prelude::PhysicsSystems;
use bevy::{ecs::{query::QueryFilter, schedule::ScheduleCleanupPolicy}, prelude::*};
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance,
    animation_graph_player::AnimationGraphPlayer,
    context::system_resources::SystemResources,
    physics_systems_avian::spawn_missing_ragdolls_avian,
    plugin::AnimationGraphSet,
    systems::{RootMotionOutput, animation_player, debug_draw_animation_players, extract_root_motion, run_animation_player},
};

#[derive(Component)]
pub struct PostPhysicsAnimation;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostPhysicsAnimationSet {
    Inputs,
    Evaluate,
}

pub struct PostPhysicsAnimationPlugin;

impl Plugin for PostPhysicsAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(PostUpdate, (
            PostPhysicsAnimationSet::Inputs.after(PhysicsSystems::Last),
            PostPhysicsAnimationSet::Evaluate.after(PostPhysicsAnimationSet::Inputs)
                .before(AnimationGraphSet::PostPhysics),
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
        });
    }
}

fn evaluate_players<F: QueryFilter>(
    time: Res<Time>,
    mut players: Query<(Entity, &mut AnimationGraphPlayer), F>,
    resources: SystemResources,
) {
    players.par_iter_mut().for_each(|(entity, player)| run_animation_player(entity, player, &time, &resources));
    players.par_iter_mut().for_each(|(_, player)| debug_draw_animation_players(player, &resources));
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
        animation_clip::GraphClip, animation_graph::AnimationGraph, animation_node::AnimationNode,
        edge_data::{DataSpec, DataValue}, pose::{BonePose, Pose, RootMotionDelta},
        skeleton::Skeleton, state_machine::high_level::StateMachine,
    };

    #[test]
    fn marked_players_evaluate_once_after_physics_while_samplers_stay_before_it() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<GraphClip>>().init_resource::<Assets<AnimationGraph>>()
            .init_resource::<Assets<Skeleton>>().init_resource::<Assets<StateMachine>>()
            .add_systems(PostUpdate, (animation_player, extract_root_motion).chain().in_set(AnimationGraphSet::PrePhysics))
            .configure_sets(PostUpdate, (
                AnimationGraphSet::PrePhysics.before(PhysicsSystems::First),
                PhysicsSystems::Last.after(PhysicsSystems::First),
                AnimationGraphSet::PostPhysics.after(PhysicsSystems::Last),
            )).add_plugins(PostPhysicsAnimationPlugin);
        let mut graph = AnimationGraph::new();
        let node = AnimationNode::new("input", crate::GlobalPoseInputNode { key: "input".into(), output: "Pose".into() });
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
            app.world_mut().run_schedule(PostUpdate);
            for (entity, expected) in [(sampler, (frame - 1) as f32), (character, frame as f32)] {
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
