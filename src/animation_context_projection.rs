use std::collections::HashMap;

use als_ext::player_animation_context_state;
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

fn apply_source_curve_defaults(curves: &mut HashMap<String, f32>) {
    for (name, value) in SOURCE_CURVE_DEFAULTS {
        curves.entry(name.to_owned()).or_insert(value);
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

fn root_pose_source_node(graph: &AnimationGraph) -> Option<NodeId> {
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
        let mut curves = root_pose_source_node(graph)
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
        apply_source_curve_defaults(&mut curves);
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
        let mut curves = HashMap::new();
        apply_source_curve_defaults(&mut curves);
        for name in [
            "PoseMoving",
            "FootLeftIk",
            "FootRightIk",
            "FootLeftLock",
            "FootRightLock",
        ] {
            assert_eq!(curves.get(name), Some(&0.0));
        }
        curves.insert("FootLeftLock".to_owned(), 0.75);
        apply_source_curve_defaults(&mut curves);
        assert_eq!(curves.get("FootLeftLock"), Some(&0.75));
    }
}
