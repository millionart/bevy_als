use std::collections::HashMap;

use crate::extensions::player_animation_context_state;
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::{AnimationGraph, NodeId, SourcePin, TargetPin},
    animation_graph_player::{AnimationGraphPlayer, AnimationSource},
    edge_data::DataSpec,
    plugin::AnimationGraphSet,
};

const SOURCE_CURVE_DEFAULTS: [(&str, f32); 5] = [
    ("PoseMoving", 0.0),
    ("FootLeftIk", 0.0),
    ("FootRightIk", 0.0),
    ("FootLeftLock", 0.0),
    ("FootRightLock", 0.0),
];

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct AlsRootAnimationContext {
    pub curves: HashMap<crate::extensions::SourceCurveName, f32>,
}

impl AlsRootAnimationContext {
    pub fn f32(&self, name: &str) -> Option<f32> {
        self.curves
            .get(&crate::extensions::SourceCurveName::from(name))
            .copied()
            .or_else(|| SOURCE_CURVE_DEFAULTS.iter().find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(*value)))
            .filter(|value| value.is_finite())
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsRootAnimationContextSet;

pub struct AlsRootAnimationContextPlugin;

impl Plugin for AlsRootAnimationContextPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsRootAnimationContextSet.after(AnimationGraphSet::Final),
        )
        .add_systems(
            PostUpdate,
            project_root_animation_context.in_set(AlsRootAnimationContextSet),
        )
        .add_observer(|event: On<Discard<AnimationGraphPlayer>>, mut contexts: Query<&mut AlsRootAnimationContext>| {
            if let Ok(mut context) = contexts.get_mut(event.entity) {
                context.curves.clear();
            }
        });
    }
}

pub(crate) fn root_pose_source_node(graph: &AnimationGraph) -> Option<NodeId> {
    let mut pose_outputs = graph
        .io_spec
        .iter_output_data()
        .filter(|(_, spec)| **spec == DataSpec::Pose)
        .map(|(output, _)| output.clone());
    let output = pose_outputs.next()?;
    if pose_outputs.next().is_some() {
        return None;
    }
    let SourcePin::NodeData(node, _) = graph.edges_inverted.get(&TargetPin::OutputData(output))?
    else {
        return None;
    };
    Some(node.clone())
}

fn project_root_animation_context(
    mut commands: Commands,
    graphs: Res<Assets<AnimationGraph>>,
    mut players: Query<(
        Entity,
        &AnimationGraphPlayer,
        Option<&mut AlsRootAnimationContext>,
    )>,
) {
    for (entity, player, current) in &mut players {
        let AnimationSource::Graph(graph_handle) = player.get_animation_source() else {
            continue;
        };
        let Some(graph) = graphs.get(graph_handle) else {
            continue;
        };
        let curves = root_pose_source_node(graph)
            .and_then(|node| player_animation_context_state(player, node))
            .map(|state| {
                state
                    .curves
                    .iter()
                    .filter(|(_, value)| value.is_finite())
                    .map(|(name, value)| (name.clone(), *value))
                    .collect()
            })
            .unwrap_or_default();
        let projected = AlsRootAnimationContext { curves };
        if let Some(mut current) = current {
            *current = projected;
        } else {
            commands.entity(entity).insert(projected);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaced_aiming_player_starts_head_with_empty_previous_curves() {
        use crate::als::input::AlsInputCommand;
        use crate::als::motion::AlsInputOverride;
        use crate::als::state::OverlayState;
        use crate::als::transition::AlsStateRequest;
        use crate::animation_visibility::AlsAnimationTickOption;
        use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;

        for remove_first in [false, true] {
            let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().entity_mut(avatar).insert(AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones);
            app.world_mut().entity_mut(character).insert(AlsInputOverride(AlsInputCommand {
                aim_held: true,
                ..default()
            }));
            app.world_mut().get_mut::<AlsStateRequest>(character).unwrap().overlay_state = Some(OverlayState::Rifle);
            let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
            let aiming_curve = |world: &World| world.get::<AlsRootAnimationContext>(player_entity)
                .and_then(|context| context.f32("PoseAiming")).unwrap_or(0.0);
            for _ in 0..120 {
                app.update();
                if aiming_curve(app.world()) > 0.9 { break; }
            }
            assert!(aiming_curve(app.world()) > 0.9, "Rifle never published its aiming curve");
            let original = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            let AnimationSource::Graph(graph) = original.get_animation_source() else { panic!("Character must use the source graph"); };
            let mut replacement = AnimationGraphPlayer::new(original.skeleton().clone()).with_graph(graph.clone());
            replacement.play_one_frame();
            let mut entity = app.world_mut().entity_mut(player_entity);
            if remove_first { entity.remove::<AnimationGraphPlayer>(); }
            entity.insert(replacement);
            assert!(entity.get::<AlsRootAnimationContext>().unwrap().curves.is_empty());

            app.update();
            let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            assert!(player.get_error().is_none() && player.get_default_output_pose().is_some());
            assert_eq!(player.get_context_arena().unwrap().frame_index(), 1);
            assert_eq!(player.elapsed(), 0.0);
            let alpha = player.get_global_input_data("view_state.head_blend_amount").unwrap().as_f32().unwrap();
            assert_eq!(alpha, 1.0, "Replacement (remove_first={remove_first}) used an old curve");
        }
    }

    #[test]
    fn root_context_uses_pose_output_edge_not_editor_name() {
        let mut graph = AnimationGraph::default();
        let node = NodeId::default();
        graph.add_output_data("Result".to_owned(), DataSpec::Pose);
        graph.edges_inverted.insert(
            TargetPin::OutputData("Result".to_owned()),
            SourcePin::NodeData(node.clone(), "Pose".to_owned()),
        );

        assert_eq!(root_pose_source_node(&graph), Some(node));
    }

    #[test]
    fn missing_source_curves_use_the_unreal_curve_default() {
        let mut context = AlsRootAnimationContext::default();
        for name in [
            "PoseMoving",
            "FootLeftIk",
            "FootRightIk",
            "FootLeftLock",
            "FootRightLock",
        ] {
            assert_eq!(context.f32(name), Some(0.0));
            assert!(!context.curves.contains_key(&crate::extensions::SourceCurveName::from(name)));
        }
        context.curves.insert("FootLeftLock".into(), 0.75);
        assert_eq!(context.f32("FootLeftLock"), Some(0.75));
        assert_eq!(context.curves.len(), 1);
    }
}
