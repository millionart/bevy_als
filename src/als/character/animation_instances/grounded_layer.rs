use crate::als::character::AlsCharacter;
use crate::extensions::{InterruptibleFsmNode, InterruptibleFsmState};
use crate::{character_assembly::AlsCharacterAvatar, crouching_layer::AlsCrouchingLayerSet};
use bevy::{ecs::hierarchy::ChildOf, prelude::*};
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance,
    animation_graph::AnimationGraph,
    animation_graph_player::AnimationGraphPlayer,
    context::node_states::StateKey,
    plugin::AnimationGraphSet,
    state_machine::{high_level::StateMachine, low_level::LowLevelStateId},
};

const GROUNDED_GRAPH: &str =
    "ALS/Character/AnimationInstances/AB_Als_Grounded/AnimGraph.animgraph.ron";
const GROUNDED_STATES_NODE_NAME: &str = "Grounded States\nState Machine";

pub use crate::als::state::AlsGroundedEntryMode;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsGroundedLayerSet;

#[derive(Resource)]
struct GroundedGraphHandle(Handle<AnimationGraph>);

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsGroundedGraphTelemetry {
    pub actual_state: Option<u8>,
    pub transition_active: Option<bool>,
    pub pose_ready: Option<bool>,
}

pub struct GroundedLayerPlugin;

impl Plugin for GroundedLayerPlugin {
    fn build(&self, app: &mut App) {
        crate::builtin_assets::install(app);
        app.configure_sets(
            PostUpdate,
            AlsGroundedLayerSet.after(AlsCrouchingLayerSet).before(
                crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate,
            ),
        )
        .add_systems(Startup, load_grounded_graph)
        .add_systems(
            PostUpdate,
            capture_grounded_graph_telemetry.after(AnimationGraphSet::Final),
        );
    }
}

fn load_grounded_graph(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GroundedGraphHandle(assets.load(crate::als_asset_path(GROUNDED_GRAPH))));
}

fn grounded_state_index(label: &str) -> Option<u8> {
    match label {
        "Entry Conduit" => Some(0),
        "Standing" => Some(1),
        "Crouching" => Some(2),
        "Standing To Crouching" => Some(3),
        "Crouching To Standing" => Some(4),
        "Roll" => Some(5),
        _ => None,
    }
}

fn actual_grounded_fsm_telemetry(
    player: &AnimationGraphPlayer,
    graph: &AnimationGraph,
    graph_handle: &Handle<AnimationGraph>,
    state_machines: &Assets<StateMachine>,
) -> Option<(Option<u8>, bool, bool)> {
    let node = graph
        .nodes
        .values()
        .find(|node| node.name == GROUNDED_STATES_NODE_NAME)?;
    let fsm = node.try_inner_downcast_ref::<InterruptibleFsmNode>()?;
    let (fsm_handle, fsm_node, fsm_graph) = (fsm.fsm.clone(), node.id, graph_handle.id());
    let machine = state_machines.get(&fsm_handle)?;
    let arena = player.get_context_arena()?;
    arena.iter_context_ids().find_map(|context_id| {
        let context = arena.get_context(context_id)?;
        if context.get_graph_id() != fsm_graph
            || !context.node_caches.is_updated(fsm_node, StateKey::Default)
        {
            return None;
        }
        let state = context
            .node_states
            .get::<InterruptibleFsmState>(fsm_node, StateKey::Default)
            .ok()?;
        let actual_state = match &state.state {
            LowLevelStateId::HlState(state) => machine
                .states
                .get(state)
                .and_then(|state| grounded_state_index(&state.label)),
            LowLevelStateId::HlTransition(_) => None,
        };
        Some((
            actual_state,
            matches!(state.state, LowLevelStateId::HlTransition(_)),
            state.last_output_pose().is_some(),
        ))
    })
}

fn capture_grounded_graph_telemetry(
    mut commands: Commands,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    graph_assets: Res<Assets<AnimationGraph>>,
    state_machines: Res<Assets<StateMachine>>,
    handle: Res<GroundedGraphHandle>,
    mut characters: Query<Option<&mut AlsGroundedGraphTelemetry>, With<AlsCharacter>>,
) {
    for (parent, instance) in &avatars {
        let Ok(player) = players.get(instance.player_entity()) else {
            continue;
        };
        let actual = graph_assets.get(&handle.0).and_then(|graph| {
            actual_grounded_fsm_telemetry(player, graph, &handle.0, &state_machines)
        });
        let telemetry = AlsGroundedGraphTelemetry {
            actual_state: actual.and_then(|value| value.0),
            transition_active: actual.map(|value| value.1),
            pose_ready: actual.map(|value| value.2),
        };
        let Ok(current) = characters.get_mut(parent.parent()) else {
            continue;
        };
        if let Some(mut current) = current {
            *current = telemetry;
        } else {
            commands.entity(parent.parent()).insert(telemetry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grounded_fsm_telemetry_uses_the_authored_node_title() {
        assert_eq!(GROUNDED_STATES_NODE_NAME, "Grounded States\nState Machine");
        assert!(
            include_str!("../../../../assets/ALS/Character/AB_Als/AnimGraph.animgraph.ron")
                .contains(&format!("graph: \"{GROUNDED_GRAPH}\""))
        );
    }

    #[test]
    fn grounded_graph_telemetry_labels_match_the_loaded_fsm() {
        for (index, label) in [
            "Entry Conduit",
            "Standing",
            "Crouching",
            "Standing To Crouching",
            "Crouching To Standing",
            "Roll",
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(grounded_state_index(label), Some(index as u8));
        }
        assert_eq!(grounded_state_index("Unknown"), None);
    }
}
