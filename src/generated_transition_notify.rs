use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance,
    animation_graph::{AnimationGraph, NodeId},
    animation_graph_player::AnimationGraphPlayer,
    context::graph_context_arena::GraphContextId,
    plugin::AnimationGraphSet,
};

use als_ext::{InterruptibleFsmNode, InterruptibleFsmState};

use crate::character_assembly::AlsCharacterAvatar;
use crate::standing_layer::AlsPivotNotify;
use crate::stop_link::{AlsQuickStopNotify, AlsTransitionPlayback};

#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct GeneratedTransitionNotifyTelemetry {
    pub activate_pivot_count: u32,
    pub stop_quick_count: u32,
    pub land_to_grounded_count: u32,
    pub observed_named_fsm_state_ids: Vec<String>,
    pub active_named_source_state_count: u32,
    pub last_active_named_source_state_id: String,
    pub active_named_transition_count: u32,
    pub last_active_named_transition_id: String,
}

#[derive(Resource, Default)]
struct GeneratedTransitionNotifyCursor {
    generations: HashMap<(Entity, GraphContextId, NodeId), u64>,
}

pub struct GeneratedTransitionNotifyPlugin;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransitionNotifyAction {
    ActivatePivot,
    StopQuick,
    LandToGrounded,
}

fn transition_notify_action(name: &str) -> Option<TransitionNotifyAction> {
    match name {
        "ActivatePivot" => Some(TransitionNotifyAction::ActivatePivot),
        "StopQuick" => Some(TransitionNotifyAction::StopQuick),
        "LandToGrounded" => Some(TransitionNotifyAction::LandToGrounded),
        _ => None,
    }
}

impl Plugin for GeneratedTransitionNotifyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GeneratedTransitionNotifyCursor>()
            .add_systems(PreUpdate, ensure_generated_transition_notify_telemetry)
            .add_systems(
                PostUpdate,
                dispatch_generated_transition_notifications
                    .after(AnimationGraphSet::PrePhysics)
                    .before(AnimationGraphSet::PostPhysics),
            );
    }
}

fn ensure_generated_transition_notify_telemetry(
    mut commands: Commands,
    characters: Query<
        Entity,
        (
            With<als_core::character::AlsCharacter>,
            Without<GeneratedTransitionNotifyTelemetry>,
        ),
    >,
) {
    for character in &characters {
        commands
            .entity(character)
            .insert(GeneratedTransitionNotifyTelemetry::default());
    }
}

fn dispatch_generated_transition_notifications(
    mut cursor: ResMut<GeneratedTransitionNotifyCursor>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    graphs: Res<Assets<AnimationGraph>>,
    mut telemetry: Query<&mut GeneratedTransitionNotifyTelemetry>,
    mut pivot_notices: MessageWriter<AlsPivotNotify>,
    mut quick_stop_notices: MessageWriter<AlsQuickStopNotify>,
    mut commands: Commands,
) {
    for (avatar, instance) in &avatars {
        let player_entity = instance.player_entity();
        let Ok(player) = players.get(player_entity) else {
            continue;
        };
        let Some(arena) = player.get_context_arena() else {
            continue;
        };
        for context_id in arena.iter_context_ids() {
            let Some(context) = arena.get_context(context_id) else {
                continue;
            };
            let Some(graph) = graphs.get(context.get_graph_id()) else {
                continue;
            };
            for node in graph.nodes.values() {
                let Some(fsm) = node.try_inner_downcast_ref::<InterruptibleFsmNode>() else {
                    continue;
                };
                let Ok(mut states) = context
                    .node_states
                    .get_all_upcoming_states::<InterruptibleFsmState>(node.id)
                else {
                    continue;
                };
                let Some(state) = states.find(|state| state.last_output_pose().is_some()) else {
                    continue;
                };
                if fsm
                    .source_transition_conditions
                    .iter()
                    .any(|condition| !condition.notifications.is_empty())
                    && let Some(state_id) = state.active_state_id()
                    && let Ok(mut telemetry) = telemetry.get_mut(avatar.character)
                    && !telemetry.observed_named_fsm_state_ids.contains(&state_id)
                {
                    telemetry.observed_named_fsm_state_ids.push(state_id);
                }
                if let Some(state_id) = state.active_state_id()
                    && fsm.source_transition_conditions.iter().any(|condition| {
                        condition.source_state_id == state_id && !condition.notifications.is_empty()
                    })
                    && let Ok(mut telemetry) = telemetry.get_mut(avatar.character)
                {
                    telemetry.active_named_source_state_count += 1;
                    telemetry.last_active_named_source_state_id = state_id;
                }
                if let Some(transition_id) = state.active_direct_transition_id()
                    && fsm.source_transition_conditions.iter().any(|condition| {
                        condition.transition_id == transition_id
                            && !condition.notifications.is_empty()
                    })
                    && let Ok(mut telemetry) = telemetry.get_mut(avatar.character)
                {
                    telemetry.active_named_transition_count += 1;
                    telemetry.last_active_named_transition_id = transition_id;
                }
                let key = (player_entity, context_id, node.id);
                if cursor.generations.get(&key) == Some(&state.notification_generation()) {
                    continue;
                }
                cursor
                    .generations
                    .insert(key, state.notification_generation());
                for notification in state.transition_notifications() {
                    match transition_notify_action(&notification.name) {
                        Some(TransitionNotifyAction::ActivatePivot) => {
                            if let Ok(mut telemetry) = telemetry.get_mut(avatar.character) {
                                telemetry.activate_pivot_count += 1;
                            }
                            pivot_notices.write(AlsPivotNotify {
                                character: avatar.character,
                            });
                        }
                        Some(TransitionNotifyAction::StopQuick) => {
                            if let Ok(mut telemetry) = telemetry.get_mut(avatar.character) {
                                telemetry.stop_quick_count += 1;
                            }
                            quick_stop_notices.write(AlsQuickStopNotify {
                                character: avatar.character,
                            });
                        }
                        Some(TransitionNotifyAction::LandToGrounded) => {
                            if let Ok(mut telemetry) = telemetry.get_mut(avatar.character) {
                                telemetry.land_to_grounded_count += 1;
                            }
                            commands
                                .entity(avatar.character)
                                .insert(AlsTransitionPlayback::standing_transition_right());
                        }
                        None => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_transition_notify_names_use_only_verified_actions() {
        assert_eq!(
            transition_notify_action("ActivatePivot"),
            Some(TransitionNotifyAction::ActivatePivot)
        );
        assert_eq!(
            transition_notify_action("StopQuick"),
            Some(TransitionNotifyAction::StopQuick)
        );
        assert_eq!(
            transition_notify_action("LandToGrounded"),
            Some(TransitionNotifyAction::LandToGrounded)
        );
        assert_eq!(transition_notify_action("Unknown"), None);
    }
}
