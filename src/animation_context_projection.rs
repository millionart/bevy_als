use std::collections::HashMap;

use als_ext::player_animation_context_state;
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_graph::AnimationGraph,
    animation_graph_player::{AnimationGraphPlayer, AnimationSource},
    plugin::AnimationGraphSet,
};

const ROOT_CONTEXT_NODE: &str = "AB_Als_Ragdolling";
const SIGNAL_CONTEXT_NODE: &str = "Inertialization";

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct AlsRootAnimationContext {
    pub curves: HashMap<String, f32>,
}

impl AlsRootAnimationContext {
    pub fn f32(&self, name: &str) -> Option<f32> {
        self.curves
            .get(name)
            .copied()
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
        );
    }
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
        let mut curves = HashMap::default();
        for node_name in [ROOT_CONTEXT_NODE, SIGNAL_CONTEXT_NODE] {
            let Some(node) = graph.nodes.values().find(|node| node.name == node_name) else {
                continue;
            };
            let Some(state) = player_animation_context_state(player, node.id) else {
                continue;
            };
            curves.extend(
                state
                    .curves
                    .iter()
                    .filter(|(_, value)| value.is_finite())
                    .map(|(name, value)| (name.clone(), *value)),
            );
        }
        let projected = AlsRootAnimationContext { curves };
        if let Some(mut current) = current {
            *current = projected;
        } else {
            commands.entity(entity).insert(projected);
        }
    }
}
