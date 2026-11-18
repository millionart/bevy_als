use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance,
    animation_graph::{AnimationGraph, NodeId},
    animation_graph_player::AnimationGraphPlayer,
    context::{graph_context_arena::GraphContextId, node_states::StateKey},
    plugin::AnimationGraphSet,
};

use crate::extensions::{
    InterruptibleFsmNode, InterruptibleFsmState, source_instance::SourceInstanceProperties,
};

use crate::character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar};
use crate::standing_layer::AlsPivotNotify;
use crate::stop_link::{AlsTransitionMontageStack, AlsTransitionPlayback, play_transition};
use crate::turn_layer::{
    AlsTurnDrive, AlsTurnMontageStack, SourceTurnRequest, TURN_MONTAGE_BLEND_DURATION,
};

#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct TransitionNotifyTelemetry {
    pub activate_pivot_count: u32,
    pub stop_quick_count: u32,
    pub land_to_grounded_count: u32,
    pub grounded_entry_reset_count: u32,
    pub roll_to_grounded_count: u32,
    pub roll_to_grounded_stance: Option<crate::als::state::Stance>,
    pub stop_animations_count: u32,
    pub crouching_stop_count: u32,
    pub transition_animation_call_count: u32,
    pub transition_animation_play_count: u32,
    pub observed_named_fsm_state_ids: Vec<String>,
    pub active_named_source_state_count: u32,
    pub last_active_named_source_state_id: String,
    pub active_named_transition_count: u32,
    pub last_active_named_transition_id: String,
}

#[derive(Resource, Default)]
struct TransitionNotifyCursor {
    generations: HashMap<(Entity, GraphContextId, NodeId), u64>,
    queued_transitions: HashMap<Entity, AlsTransitionPlayback>,
    queued_turns: HashMap<Entity, SourceTurnRequest>,
    source_frames: HashMap<Entity, (u64, u64)>,
}

impl TransitionNotifyCursor {
    fn take_transition(
        &mut self,
        character: Entity,
        stop_requested: bool,
    ) -> Option<AlsTransitionPlayback> {
        if stop_requested {
            None
        } else {
            self.queued_transitions.remove(&character)
        }
    }
}

pub struct TransitionNotifyPlugin;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransitionNotifyAction {
    ActivatePivot,
    StopQuick,
    LandToGrounded,
    StopLeft,
    StopRight,
    StopAnimations,
    TransitionRight,
}

fn transition_notify_action(name: &str) -> Option<TransitionNotifyAction> {
    match name {
        "ActivatePivot" => Some(TransitionNotifyAction::ActivatePivot),
        "StopQuick" => Some(TransitionNotifyAction::StopQuick),
        "LandToGrounded" => Some(TransitionNotifyAction::LandToGrounded),
        "PlayStopLeftTransitionAnimation" => Some(TransitionNotifyAction::StopLeft),
        "PlayStopRightTransitionAnimation" => Some(TransitionNotifyAction::StopRight),
        "StopTransitionAndTurnInPlaceAnimations" => Some(TransitionNotifyAction::StopAnimations),
        "PlayRollToGroundedTransitionAnimation" | "PlayStopTransitionAnimation" => {
            Some(TransitionNotifyAction::TransitionRight)
        }
        _ => None,
    }
}

fn transition_animation_call(
    notification: &crate::extensions::interruptible_fsm::SourceTransitionNotification,
    stance: crate::als::state::Stance,
    moving: bool,
) -> Option<AlsTransitionPlayback> {
    let left = match notification.function.as_str() {
        "PlayTransitionLeftAnimation" => true,
        "PlayTransitionRightAnimation" => false,
        _ => return None,
    };
    let argument = |name: &str| {
        notification
            .arguments
            .iter()
            .find(|argument| argument.property == name)
            .map(|argument| &argument.value)
    };
    if argument("bFromStandingIdleOnly")?.as_bool().ok()?
        && (moving || stance != crate::als::state::Stance::Standing)
    {
        return None;
    }
    Some(AlsTransitionPlayback::transition_animation(
        stance == crate::als::state::Stance::Crouching,
        left,
        argument("BlendInDuration")?.as_f32().ok()?,
        argument("BlendOutDuration")?.as_f32().ok()?,
        argument("PlayRate")?.as_f32().ok()?,
        argument("StartTime")?.as_f32().ok()?,
    ))
}

fn grounded_entry_reset_requested(source: &SourceInstanceProperties, frame: u64) -> bool {
    source
        .write_frames
        .get("grounded_entry_mode.reset_requested")
        == Some(&frame)
        && source
            .values
            .get("grounded_entry_mode.reset_requested")
            .and_then(|value| value.as_bool().ok())
            == Some(true)
}

impl Plugin for TransitionNotifyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TransitionNotifyCursor>()
            .add_systems(PreUpdate, ensure_transition_notify_telemetry)
            .add_systems(
                PostUpdate,
                dispatch_transition_notifications
                    .after(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate)
                    .before(AnimationGraphSet::PostPhysics),
            );
    }
}

fn ensure_transition_notify_telemetry(
    mut commands: Commands,
    characters: Query<
        Entity,
        (
            With<crate::als::character::AlsCharacter>,
            Without<TransitionNotifyTelemetry>,
        ),
    >,
) {
    for character in &characters {
        commands
            .entity(character)
            .insert(TransitionNotifyTelemetry::default());
    }
}

pub(crate) fn queue_turn_in_grounded_group(
    commands: &mut Commands,
    character: Entity,
    avatar_entity: Entity,
    request: SourceTurnRequest,
) {
    commands.queue(move |world: &mut World| {
        let Some(assembly) = world.get::<AlsCharacterAssembly>(character) else {
            return;
        };
        if assembly.avatar != avatar_entity
            || world
                .get::<AlsCharacterAvatar>(avatar_entity)
                .is_none_or(|avatar| avatar.character != character)
        {
            return;
        }
        let Some(player_entity) = world
            .get::<AnimatedSceneInstance>(avatar_entity)
            .map(AnimatedSceneInstance::player_entity)
        else {
            return;
        };
        if world.get::<AnimationGraphPlayer>(player_entity).is_none()
            || world.get::<AlsTurnMontageStack>(avatar_entity).is_none()
        {
            return;
        }
        let sampling_ready = world
            .get_resource::<crate::standing_source::SourceMontageAssets>()
            .zip(
                world
                    .get_resource::<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>(
                    ),
            )
            .zip(world.get_resource::<Assets<bevy_animation_graph::core::skeleton::Skeleton>>())
            .is_some_and(|((assets, clips), skeletons)| {
                crate::standing_source::sample_source_montage(
                    request.source_sequence(),
                    0.0,
                    assets,
                    clips,
                    skeletons,
                )
                .is_some()
            });
        if let Some(mut transitions) = world.get_mut::<AlsTransitionMontageStack>(character) {
            transitions.stop_with_duration(TURN_MONTAGE_BLEND_DURATION);
        }
        let curve_play_rate = {
            let mut turns = world.get_mut::<AlsTurnMontageStack>(avatar_entity).unwrap();
            let rate = request.play(&mut turns);
            if sampling_ready {
                turns.entries.last_mut().unwrap().sampling_ready = true;
            }
            rate
        };
        world
            .get_mut::<AnimationGraphPlayer>(player_entity)
            .unwrap()
            .set_global_input_data("turn_in_place_state.play_rate", curve_play_rate);
    });
}

fn dispatch_transition_notifications(
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
    mut cursor: ResMut<TransitionNotifyCursor>,
    avatars: Query<(Entity, &AlsCharacterAvatar, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    graphs: Res<Assets<AnimationGraph>>,
    mut telemetry: Query<&mut TransitionNotifyTelemetry>,
    mut pivot_notices: MessageWriter<AlsPivotNotify>,
    mut playbacks: Query<&mut AlsTransitionMontageStack>,
    mut turn_stacks: Query<&mut AlsTurnMontageStack>,
    mut turn_drives: Query<&mut AlsTurnDrive>,
    stances: Query<&crate::als::state::Stance, With<crate::als::character::AlsCharacter>>,
    assemblies: Query<&AlsCharacterAssembly>,
    transforms: bevy::transform::helper::TransformHelper,
    quick_stop_inputs: Query<(
        &crate::als::state::RotationMode,
        Option<&crate::actual_motion::AlsActualMotion>,
        Option<&crate::als::motion::AlsRotationResponseState>,
    )>,
    mut commands: Commands,
) {
    let active: HashMap<_, _> = avatars
        .iter()
        .filter(|(entity, avatar, instance)| {
            assemblies
                .get(avatar.character)
                .is_ok_and(|assembly| assembly.avatar == *entity)
                && stances.contains(avatar.character)
                && players.contains(instance.player_entity())
        })
        .map(|(_, avatar, instance)| (instance.player_entity(), avatar.character))
        .collect();
    cursor
        .source_frames
        .retain(|player, _| active.contains_key(player));
    cursor.generations.retain(|(player, context, _), _| {
        active.contains_key(player)
            && players
                .get(*player)
                .ok()
                .and_then(|player| player.get_context_arena())
                .is_some_and(|arena| arena.get_context(*context).is_some())
    });
    cursor
        .queued_transitions
        .retain(|character, _| active.values().any(|owner| owner == character));
    cursor
        .queued_turns
        .retain(|character, _| active.values().any(|owner| owner == character));
    for (avatar_entity, avatar, instance) in &avatars {
        let mut stop_requested = false;
        let mut game_thread_transition = None;
        let player_entity = instance.player_entity();
        if !active.contains_key(&player_entity) {
            continue;
        }
        let Ok(player) = players.get(player_entity) else {
            continue;
        };
        if player.is_paused() {
            continue;
        }
        let Some(arena) = player.get_context_arena() else {
            continue;
        };
        let source = arena
            .get_toplevel()
            .node_states
            .get::<SourceInstanceProperties>(NodeId::default(), StateKey::Default)
            .ok();
        if let Some(source) = source {
            let frame = (source.instance_id, arena.frame_index());
            let previous = cursor.source_frames.insert(player_entity, frame);
            if previous == Some(frame) {
                continue;
            }
            if previous.is_some_and(|previous| previous.0 != frame.0) {
                cursor.generations.retain(|key, _| key.0 != player_entity);
                cursor.queued_transitions.remove(&avatar.character);
                cursor.queued_turns.remove(&avatar.character);
            }
        }
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
                    if !notification.function.is_empty() {
                        if let Ok(mut telemetry) = telemetry.get_mut(avatar.character) {
                            telemetry.transition_animation_call_count += 1;
                        }
                        if let Ok(stance) = stances.get(avatar.character)
                            && let Some(moving) = player
                                .get_global_input_data("locomotion_state.b_moving")
                                .and_then(|value| value.as_bool().ok())
                            && let Some(playback) =
                                transition_animation_call(notification, *stance, moving)
                        {
                            game_thread_transition = Some(playback);
                            if let Ok(mut telemetry) = telemetry.get_mut(avatar.character) {
                                telemetry.transition_animation_play_count += 1;
                            }
                        }
                        continue;
                    }
                    match transition_notify_action(&notification.name) {
                        Some(TransitionNotifyAction::ActivatePivot) => {
                            if let Ok(mut telemetry) = telemetry.get_mut(avatar.character) {
                                telemetry.activate_pivot_count += 1;
                            }
                            let speed = crate::extensions::source_instance::source_player_property(
                                player,
                                "locomotion_state.speed",
                            )
                            .or_else(|| player.get_global_input_data("locomotion_state.speed"))
                            .and_then(|value| value.as_f32().ok())
                            .unwrap_or_default();
                            pivot_notices.write(AlsPivotNotify {
                                character: avatar.character,
                                activated: crate::standing_layer::source_pivot_activates(
                                    true, speed,
                                ),
                            });
                        }
                        Some(TransitionNotifyAction::StopQuick) => {
                            if let Ok(mut telemetry) = telemetry.get_mut(avatar.character) {
                                telemetry.stop_quick_count += 1;
                            }
                            if let Ok((rotation_mode, motion, rotation)) =
                                quick_stop_inputs.get(avatar.character)
                                && let Ok(stance) = stances.get(avatar.character)
                                && let Ok(body) =
                                    transforms.compute_global_transform(avatar.character)
                            {
                                let input_yaw = motion.and_then(|motion| {
                                    motion.has_input.then_some(motion.input_yaw_degrees)
                                });
                                let target_yaw = rotation
                                    .filter(|rotation| rotation.initialized)
                                    .map(|rotation| rotation.target_yaw);
                                let (index, play_rate) = crate::stop_link::quick_stop_selection(
                                    *rotation_mode,
                                    input_yaw,
                                    &body.compute_transform(),
                                    target_yaw,
                                );
                                game_thread_transition =
                                    Some(if *stance == crate::als::state::Stance::Crouching {
                                        AlsTransitionPlayback::crouching_quick_stop(
                                            index, play_rate,
                                        )
                                    } else {
                                        AlsTransitionPlayback::quick_stop(index, play_rate)
                                    });
                            }
                        }
                        Some(TransitionNotifyAction::LandToGrounded) => {
                            if let Ok(mut telemetry) = telemetry.get_mut(avatar.character) {
                                telemetry.land_to_grounded_count += 1;
                            }
                            if let Ok(stance) = stances.get(avatar.character) {
                                game_thread_transition =
                                    Some(AlsTransitionPlayback::transition_animation(
                                        *stance == crate::als::state::Stance::Crouching,
                                        false,
                                        0.1,
                                        0.2,
                                        1.4,
                                        0.0,
                                    ));
                            }
                        }
                        Some(TransitionNotifyAction::StopLeft) => {
                            cursor
                                .queued_transitions
                                .insert(avatar.character, AlsTransitionPlayback::stop(0.0));
                            info!(
                                "[SOURCE-STATE-CALL] function={} phase={} character={:?}",
                                notification.name, notification.phase, avatar.character
                            );
                        }
                        Some(TransitionNotifyAction::StopRight) => {
                            cursor
                                .queued_transitions
                                .insert(avatar.character, AlsTransitionPlayback::stop(1.0));
                            info!(
                                "[SOURCE-STATE-CALL] function={} phase={} character={:?}",
                                notification.name, notification.phase, avatar.character
                            );
                        }
                        Some(TransitionNotifyAction::StopAnimations) => {
                            stop_requested = true;
                            if let Ok(mut telemetry) = telemetry.get_mut(avatar.character) {
                                telemetry.stop_animations_count += 1;
                            }
                            info!(
                                "[SOURCE-STATE-CALL] function={} phase={} character={:?}",
                                notification.name, notification.phase, avatar.character
                            );
                        }
                        Some(TransitionNotifyAction::TransitionRight) => {
                            if let Ok(stance) = stances.get(avatar.character) {
                                cursor.queued_transitions.insert(
                                    avatar.character,
                                    AlsTransitionPlayback::roll_to_grounded_right(
                                        *stance == crate::als::state::Stance::Crouching,
                                    ),
                                );
                                if let Ok(mut telemetry) = telemetry.get_mut(avatar.character) {
                                    if notification.name == "PlayStopTransitionAnimation" {
                                        telemetry.crouching_stop_count += 1;
                                    } else {
                                        telemetry.roll_to_grounded_count += 1;
                                        telemetry.roll_to_grounded_stance = Some(*stance);
                                    }
                                }
                            }
                        }
                        None => {}
                    }
                }
            }
        }
        if let Some(source) = source {
            let scalar = |key| {
                source
                    .values
                    .get(key)
                    .and_then(|v| v.as_f32().ok())
                    .unwrap_or(0.0)
            };
            let boolean = |key| {
                source
                    .values
                    .get(key)
                    .and_then(|v| v.as_bool().ok())
                    .unwrap_or(false)
            };
            if grounded_entry_reset_requested(source, arena.frame_index()) {
                commands
                    .entity(avatar.character)
                    .insert(crate::grounded_layer::AlsGroundedEntryMode::None);
                source_inputs
                    .get_mut(player_entity)
                    .unwrap()
                    .set_gameplay_tag_input(
                        "grounded_entry_mode",
                        bevy_gameplay_tag::GameplayTag::new(""),
                    );
                if let Ok(mut telemetry) = telemetry.get_mut(avatar.character) {
                    telemetry.grounded_entry_reset_count += 1;
                }
            }
            if source
                .write_frames
                .get("rotate_in_place_state.b_updated_this_frame")
                == Some(&arena.frame_index())
                && let Ok(mut drive) = turn_drives.get_mut(avatar_entity)
            {
                drive.rotate_active = boolean("rotate_in_place_state.b_rotating_left")
                    || boolean("rotate_in_place_state.b_rotating_right");
                drive.rotate_lr = if boolean("rotate_in_place_state.b_rotating_right") {
                    1.0
                } else {
                    0.0
                };
                drive.rotate_rate = scalar("rotate_in_place_state.play_rate");
            }
            if source.write_frames.get("turn_in_place.queued_variant") == Some(&arena.frame_index())
            {
                use crate::als::turn_in_place::TurnVariant;
                let variant = match scalar("turn_in_place.queued_variant") as u8 {
                    0 => TurnVariant::Left90,
                    1 => TurnVariant::Right90,
                    2 => TurnVariant::Left180,
                    3 => TurnVariant::Right180,
                    _ => unreachable!(),
                };
                cursor.queued_turns.insert(
                    avatar.character,
                    SourceTurnRequest {
                        variant,
                        yaw: scalar("turn_in_place.queued_yaw"),
                        crouching: boolean("turn_in_place.queued_crouching"),
                    },
                );
            }
            if source
                .write_frames
                .get("dynamic_transitions.queued_sequence")
                == Some(&arena.frame_index())
            {
                let index = scalar("dynamic_transitions.queued_sequence");
                let playback = if index < 2.0 {
                    AlsTransitionPlayback::standing_dynamic(index)
                } else {
                    AlsTransitionPlayback::crouching_dynamic(index - 2.0)
                };
                cursor.queued_transitions.insert(avatar.character, playback);
            }
        }
        if stop_requested {
            if let Ok(mut playback) = playbacks.get_mut(avatar.character) {
                playback.request_stop();
            }
            if let Ok(mut stack) = turn_stacks.get_mut(avatar_entity) {
                stack.request_stop();
            }
        }
        if let Some(playback) = cursor.take_transition(avatar.character, stop_requested) {
            play_transition(&mut commands, avatar.character, playback);
        }
        if !stop_requested
            && turn_stacks.contains(avatar_entity)
            && let Some(request) = cursor.queued_turns.remove(&avatar.character)
        {
            queue_turn_in_grounded_group(&mut commands, avatar.character, avatar_entity, request);
        }
        if let Some(playback) = game_thread_transition {
            cursor.queued_transitions.remove(&avatar.character);
            play_transition(&mut commands, avatar.character, playback);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_landing_notify_uses_native_profile_and_current_stance() {
        use crate::als::{
            input::AlsInputCommand,
            state::{LocomotionState, Stance},
        };
        use bevy::ecs::system::RunSystemOnce;

        #[derive(serde::Deserialize)]
        struct Fixture {
            profile: Profile,
            cases: Vec<Case>,
        }

        #[derive(serde::Deserialize)]
        struct Profile {
            blend_in: f32,
            blend_out: f32,
            play_rate: f32,
            start: f32,
        }

        #[derive(serde::Deserialize)]
        struct Case {
            crouching: bool,
            sequence: String,
            duration: f32,
            first_position: f32,
            first_weight: f32,
            replaced_weight: f32,
        }

        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../tests/fixtures/actions/landing_notifications.json"
        ))
        .unwrap();
        for case in fixture.cases {
            let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().entity_mut(avatar).insert(
                crate::animation_visibility::AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones,
            );
            let before = app
                .world()
                .get::<TransitionNotifyTelemetry>(character)
                .unwrap()
                .land_to_grounded_count;
            app.world_mut()
                .get_mut::<AlsInputCommand>(character)
                .unwrap()
                .jump_pressed = true;
            app.update();
            app.world_mut()
                .get_mut::<AlsInputCommand>(character)
                .unwrap()
                .jump_pressed = false;
            let stance = if case.crouching {
                Stance::Crouching
            } else {
                Stance::Standing
            };
            app.add_systems(
                PostUpdate,
                (move |mut actors: Query<(&LocomotionState, &mut Stance)>| {
                    let (mode, mut current) = actors.get_mut(character).unwrap();
                    if *mode == LocomotionState::Grounded {
                        *current = stance;
                    }
                })
                .after(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate)
                .before(dispatch_transition_notifications),
            );
            let mut airborne =
                *app.world().get::<LocomotionState>(character).unwrap() == LocomotionState::Air;
            let mut landing = None;
            for _ in 0..240 {
                app.update();
                airborne |=
                    *app.world().get::<LocomotionState>(character).unwrap() == LocomotionState::Air;
                if app
                    .world()
                    .get::<TransitionNotifyTelemetry>(character)
                    .unwrap()
                    .land_to_grounded_count
                    > before
                {
                    landing = app
                        .world()
                        .get::<AlsTransitionMontageStack>(character)
                        .and_then(AlsTransitionMontageStack::latest)
                        .copied();
                    break;
                }
            }
            assert!(airborne);
            let playback =
                landing.expect("The generated Locomotion graph did not dispatch LandToGrounded");
            let profile = &fixture.profile;
            assert_eq!(
                crate::stop_link::source_transition_sequence(&playback),
                case.sequence.as_str()
            );
            assert_eq!(
                (
                    playback.blend_in,
                    playback.blend_out,
                    playback.play_rate,
                    playback.start_time,
                    playback.duration
                ),
                (
                    profile.blend_in,
                    profile.blend_out,
                    profile.play_rate,
                    profile.start,
                    case.duration
                )
            );
            let mut first = playback;
            first.advance(0.05);
            assert_eq!(
                (first.clip_time, first.weight),
                (case.first_position, case.first_weight)
            );

            first.advance(0.05);
            app.world_mut()
                .get_mut::<AlsTransitionMontageStack>(character)
                .unwrap()
                .entries = vec![first];
            app.world_mut()
                .run_system_once(move |mut commands: Commands| {
                    play_transition(&mut commands, character, playback);
                })
                .unwrap();
            app.world_mut().flush();
            let mut stack = app
                .world_mut()
                .get_mut::<AlsTransitionMontageStack>(character)
                .unwrap();
            assert_eq!(stack.entries.len(), 2);
            stack.entries[0].advance(0.05);
            assert_eq!(stack.entries[0].weight, case.replaced_weight);
        }
    }

    #[test]
    fn generated_pivot_notifications_capture_the_evaluated_instance_speed() {
        use avian3d::prelude::{LinearVelocity, RigidBody};
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().entity_mut(character).insert((
            RigidBody::Kinematic,
            bevy_tnua::TnuaToggle::Disabled,
            crate::als::state::RotationMode::ViewDirection,
        ));
        #[derive(Resource)]
        struct PivotFixtureInput(f32, f32);
        app.insert_resource(PivotFixtureInput(0.0, 0.0));
        app.add_systems(
            PostUpdate,
            (move |input: Res<PivotFixtureInput>,
                   mut motion: Query<(
                &mut crate::actual_motion::AlsActualMotion,
                &mut crate::als::state::LocomotionState,
                &mut crate::als::state::Gait,
            )>| {
                let (mut motion, mut mode, mut gait) = motion.get_mut(character).unwrap();
                *gait = crate::als::state::Gait::Running;
                motion.horizontal_speed = input.0;
                motion.world_velocity =
                    Quat::from_rotation_y(input.1.to_radians()) * Vec3::NEG_Z * input.0;
                motion.velocity_yaw_degrees = input.1;
                motion.source_velocity_yaw_degrees = None;
                motion.local_velocity =
                    Vec2::new(motion.world_velocity.x, -motion.world_velocity.z);
                motion.signed_direction_radians = input.1.to_radians();
                motion.has_input = input.0 > 0.0;
                motion.input_yaw_degrees = input.1;
                *mode = crate::als::state::LocomotionState::Grounded;
            })
            .after(crate::actual_motion::AlsActualMotionSet)
            .before(crate::gait_layer::AlsGaitLayerSet),
        );
        let mut observed = 0;
        let mut notice_cursor = bevy::ecs::message::MessageCursor::<AlsPivotNotify>::default();
        let mut run_observed = false;
        let mut pivot_consumed = false;
        for (frame, speed) in (0..240).map(|frame| (frame, if frame < 120 { 1.9_f32 } else { 2.1 }))
        {
            {
                let mut input = app.world_mut().resource_mut::<PivotFixtureInput>();
                input.0 = speed;
                input.1 = if frame % 120 < 60 { 0.0 } else { 180.0 };
            }
            app.world_mut()
                .get_mut::<LinearVelocity>(character)
                .unwrap()
                .0 = Vec3::NEG_Z * speed;
            app.world_mut()
                .get_mut::<crate::als::input::AlsInputCommand>(character)
                .unwrap()
                .move_dir = if frame % 120 < 60 {
                Vec2::Y
            } else {
                Vec2::new(0.8660254, 0.5)
            };
            app.update();
            let avatar = app
                .world()
                .get::<AlsCharacterAssembly>(character)
                .unwrap()
                .avatar;
            let player = app
                .world()
                .get::<AnimatedSceneInstance>(avatar)
                .unwrap()
                .player_entity();
            let instance = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            if let Some(telemetry) = app
                .world()
                .get::<crate::standing_layer::AlsStandingGraphTelemetry>(character)
            {
                run_observed |= telemetry.actual_detail_state == Some(1);
                if telemetry.actual_detail_state == Some(2) {
                    pivot_consumed = true;
                    assert!(
                        !crate::extensions::source_instance::source_player_property(
                            instance,
                            "standing_state.b_pivot_active"
                        )
                        .unwrap()
                        .as_bool()
                        .unwrap(),
                        "First Pivot did not consume ResetPivot"
                    );
                }
            }
            let snapshot = crate::extensions::source_instance::source_player_property(
                instance,
                "locomotion_state.speed",
            )
            .or_else(|| instance.get_global_input_data("locomotion_state.speed"))
            .unwrap()
            .as_f32()
            .unwrap();
            let notices = notice_cursor
                .read(app.world().resource::<Messages<AlsPivotNotify>>())
                .copied()
                .collect::<Vec<_>>();
            for notice in notices
                .into_iter()
                .filter(|notice| notice.character == character)
            {
                observed += 1;
                assert_eq!(
                    notice.activated,
                    snapshot < 2.0,
                    "Generated notify used another input frame"
                );
            }
        }
        let avatar = app
            .world()
            .get::<AlsCharacterAssembly>(character)
            .unwrap()
            .avatar;
        let player = app
            .world()
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let instance = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        assert!(
            observed > 0,
            "Actual generated Standing FSM did not emit ActivatePivot: mode={:?}, motion={:?}, speed={:?}, moving={:?}, telemetry={:?}",
            app.world()
                .get::<crate::als::state::LocomotionState>(character),
            app.world()
                .get::<crate::actual_motion::AlsActualMotion>(character),
            instance.get_global_input_data("locomotion_state.speed"),
            instance.get_global_input_data("locomotion_state.b_moving_smooth"),
            app.world().get::<TransitionNotifyTelemetry>(character)
        );
        assert!(
            app.world()
                .get::<TransitionNotifyTelemetry>(character)
                .unwrap()
                .activate_pivot_count
                > 0
        );
        assert!(
            run_observed && pivot_consumed,
            "Actual Run -> First Pivot transition was not consumed: run={run_observed}, pivot={pivot_consumed}, telemetry={:?}",
            app.world()
                .get::<crate::standing_layer::AlsStandingGraphTelemetry>(character)
        );
        app.world_mut()
            .run_system_once(dispatch_transition_notifications)
            .unwrap();
    }

    #[test]
    fn first_relevant_backward_movement_suppresses_the_native_pivot_start_notify() {
        use avian3d::prelude::{LinearVelocity, RigidBody};
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        app.world_mut().entity_mut(character).insert((
            RigidBody::Kinematic,
            bevy_tnua::TnuaToggle::Disabled,
            crate::als::state::RotationMode::ViewDirection,
        ));
        #[derive(Resource)]
        struct PivotFixtureInput(f32, f32);
        app.insert_resource(PivotFixtureInput(0.0, 0.0));
        app.add_systems(
            PostUpdate,
            (move |input: Res<PivotFixtureInput>,
                   mut motion: Query<(
                &mut crate::actual_motion::AlsActualMotion,
                &mut crate::als::state::LocomotionState,
                &mut crate::als::state::Gait,
            )>| {
                let (mut motion, mut mode, mut gait) = motion.get_mut(character).unwrap();
                *gait = crate::als::state::Gait::Running;
                motion.horizontal_speed = input.0;
                motion.world_velocity =
                    Quat::from_rotation_y(input.1.to_radians()) * Vec3::NEG_Z * input.0;
                motion.velocity_yaw_degrees = input.1;
                motion.source_velocity_yaw_degrees = None;
                motion.local_velocity =
                    Vec2::new(motion.world_velocity.x, -motion.world_velocity.z);
                motion.signed_direction_radians = input.1.to_radians();
                motion.has_input = input.0 > 0.0;
                motion.input_yaw_degrees = input.1;
                *mode = crate::als::state::LocomotionState::Grounded;
            })
            .after(crate::actual_motion::AlsActualMotionSet)
            .before(crate::gait_layer::AlsGaitLayerSet),
        );
        let mut observed = 0;
        let mut notice_cursor = bevy::ecs::message::MessageCursor::<AlsPivotNotify>::default();
        for _ in 0..4 {
            let speed = 1.9_f32;
            {
                let mut input = app.world_mut().resource_mut::<PivotFixtureInput>();
                input.0 = speed;
                input.1 = 180.0;
            }
            app.world_mut()
                .get_mut::<LinearVelocity>(character)
                .unwrap()
                .0 = Vec3::NEG_Z * speed;
            app.world_mut()
                .get_mut::<crate::als::input::AlsInputCommand>(character)
                .unwrap()
                .move_dir = Vec2::NEG_Y;
            app.update();
            let avatar = app
                .world()
                .get::<AlsCharacterAssembly>(character)
                .unwrap()
                .avatar;
            let player = app
                .world()
                .get::<AnimatedSceneInstance>(avatar)
                .unwrap()
                .player_entity();
            let instance = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            let snapshot = crate::extensions::source_instance::source_player_property(
                instance,
                "locomotion_state.speed",
            )
            .or_else(|| instance.get_global_input_data("locomotion_state.speed"))
            .unwrap()
            .as_f32()
            .unwrap();
            let notices = notice_cursor
                .read(app.world().resource::<Messages<AlsPivotNotify>>())
                .copied()
                .collect::<Vec<_>>();
            for notice in notices
                .into_iter()
                .filter(|notice| notice.character == character)
            {
                observed += 1;
                assert_eq!(
                    notice.activated,
                    snapshot < 2.0,
                    "Generated notify used another input frame"
                );
            }
        }
        assert_eq!(
            observed, 0,
            "First skipped Forward -> Backward transition must not dispatch ActivatePivot"
        );
        assert_eq!(
            app.world()
                .get::<TransitionNotifyTelemetry>(character)
                .unwrap()
                .activate_pivot_count,
            0
        );
    }

    #[test]
    fn generated_quick_stop_consumes_the_notifying_frame_inputs() {
        use avian3d::prelude::{LinearVelocity, RigidBody};
        fn run(
            change_input: bool,
            parented: bool,
            consumed_input: bool,
        ) -> (
            crate::stop_link::AlsTransitionPlayback,
            bevy_animation_graph::core::pose::Pose,
            String,
        ) {
            let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().entity_mut(character).insert((
                RigidBody::Kinematic,
                bevy_tnua::TnuaToggle::Disabled,
                crate::als::state::RotationMode::ViewDirection,
            ));
            if parented {
                let parent_transform = Transform::from_xyz(4.0, 2.0, -1.0)
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(1.5));
                let parent = app.world_mut().spawn(parent_transform).id();
                let world = *app.world().get::<Transform>(character).unwrap();
                app.world_mut().entity_mut(character).insert((
                    ChildOf(parent),
                    GlobalTransform::from(world)
                        .reparented_to(&GlobalTransform::from(parent_transform)),
                    crate::als::aim::AlsLastFreeRotationMode(
                        crate::als::state::RotationMode::VelocityDirection,
                    ),
                ));
            }
            if parented || consumed_input {
                app.world_mut().entity_mut(character).insert(
                    crate::als::aim::AlsLastFreeRotationMode(
                        crate::als::state::RotationMode::VelocityDirection,
                    ),
                );
                app.add_systems(
                    PostUpdate,
                    (move |mut actors: Query<(
                        &mut Transform,
                        &mut avian3d::prelude::Rotation,
                        &mut crate::als::motion::AlsRotationResponseState,
                    )>| {
                        let (mut transform, mut rotation, mut response) =
                            actors.get_mut(character).unwrap();
                        transform.rotation = Quat::from_rotation_y(if parented {
                            -std::f32::consts::FRAC_PI_2
                        } else {
                            0.0
                        });
                        rotation.0 = Quat::IDENTITY;
                        response.initialized = true;
                        response.target_yaw = if parented {
                            -std::f32::consts::FRAC_PI_4
                        } else {
                            0.0
                        };
                    })
                    .after(crate::als::motion::AlsRotationResponseSet)
                    .before(crate::ab_als_provider::AbAlsProviderSet),
                );
            }
            #[derive(Resource)]
            struct PivotFixtureInput(f32, f32);
            app.insert_resource(PivotFixtureInput(0.0, 0.0));
            app.add_systems(
                PostUpdate,
                (move |input: Res<PivotFixtureInput>,
                       mut motion: Query<(
                    &mut crate::actual_motion::AlsActualMotion,
                    &mut crate::als::state::LocomotionState,
                    &mut crate::als::state::Gait,
                )>| {
                    let (mut motion, mut mode, mut gait) = motion.get_mut(character).unwrap();
                    *gait = crate::als::state::Gait::Running;
                    motion.horizontal_speed = input.0;
                    motion.world_velocity =
                        Quat::from_rotation_y(input.1.to_radians()) * Vec3::NEG_Z * input.0;
                    motion.velocity_yaw_degrees = input.1;
                    motion.source_velocity_yaw_degrees = None;
                    motion.local_velocity =
                        Vec2::new(motion.world_velocity.x, -motion.world_velocity.z);
                    motion.signed_direction_radians = input.1.to_radians();
                    motion.has_input = consumed_input || input.0 > 0.0;
                    motion.input_yaw_degrees = if consumed_input { -90.0 } else { input.1 };
                    *mode = crate::als::state::LocomotionState::Grounded;
                })
                .after(crate::actual_motion::AlsActualMotionSet)
                .before(crate::gait_layer::AlsGaitLayerSet),
            );
            for frame in 0..110 {
                if frame == 94 {
                    if let Some(mut stack) = app
                        .world_mut()
                        .get_mut::<AlsTransitionMontageStack>(character)
                    {
                        stack.entries.clear();
                    }
                }
                let speed = if (96..98).contains(&frame) {
                    1.9_f32
                } else {
                    0.0
                };
                {
                    let mut input = app.world_mut().resource_mut::<PivotFixtureInput>();
                    input.0 = speed;
                    input.1 = 0.0;
                }
                app.world_mut()
                    .get_mut::<LinearVelocity>(character)
                    .unwrap()
                    .0 = Vec3::NEG_Z * speed;
                app.world_mut()
                    .get_mut::<crate::als::input::AlsInputCommand>(character)
                    .unwrap()
                    .move_dir = if speed > 0.0 { Vec2::Y } else { Vec2::ZERO };
                app.update();
                if app
                    .world()
                    .get::<TransitionNotifyTelemetry>(character)
                    .unwrap()
                    .stop_quick_count
                    > 0
                {
                    assert_eq!(
                        *app.world()
                            .get::<crate::als::state::RotationMode>(character)
                            .unwrap(),
                        if parented || consumed_input {
                            crate::als::state::RotationMode::VelocityDirection
                        } else {
                            crate::als::state::RotationMode::ViewDirection
                        }
                    );
                    if change_input {
                        *app.world_mut()
                            .get_mut::<crate::als::state::RotationMode>(character)
                            .unwrap() = crate::als::state::RotationMode::VelocityDirection;
                        app.world_mut()
                            .get_mut::<crate::als::input::AlsInputCommand>(character)
                            .unwrap()
                            .move_dir = Vec2::NEG_X;
                    }
                    for _ in 0..3 {
                        app.update();
                    }
                    let stack = app
                        .world()
                        .get::<AlsTransitionMontageStack>(character)
                        .unwrap();
                    let playback = stack
                        .entries
                        .iter()
                        .rev()
                        .find(|entry| {
                            matches!(
                                entry.kind,
                                crate::stop_link::AlsTransitionKind::QuickStop
                                    | crate::stop_link::AlsTransitionKind::CrouchingQuickStop
                            )
                        })
                        .expect("Actual generated StopQuick must create a transition montage");
                    let avatar = app
                        .world()
                        .get::<AlsCharacterAssembly>(character)
                        .unwrap()
                        .avatar;
                    let target = app
                        .world()
                        .get::<AnimatedSceneInstance>(avatar)
                        .unwrap()
                        .player_entity();
                    let player = app.world().get::<AnimationGraphPlayer>(target).unwrap();
                    let actual = player
                        .get_global_input_data("transition.pose")
                        .unwrap()
                        .as_pose()
                        .unwrap();
                    return (
                        *playback,
                        actual.clone(),
                        format!(
                            "entries={:?}, pose_timestamp={:?}, runtime={:?}",
                            stack.entries,
                            actual.timestamp,
                            app.world()
                                .get::<crate::stop_link::AlsRootTransitionSlotRuntime>(character)
                        ),
                    );
                }
            }
            panic!(
                "Actual generated Standing FSM did not emit StopQuick: telemetry={:?}",
                app.world().get::<TransitionNotifyTelemetry>(character)
            );
        }
        let (source, expected, source_stack) = run(false, false, false);
        let (actual, pose, actual_stack) = run(true, false, false);
        assert_eq!((source.index, source.play_rate), (0.0, 1.75));
        let mut dot = 1.0_f32;
        let mut distance = 0.0_f32;
        for bone in expected.paths.keys() {
            let source = expected.get_bone(*bone).unwrap();
            let target = pose.get_bone(*bone).unwrap();
            if let (Some(a), Some(b)) = (source.rotation, target.rotation) {
                dot = dot.min(a.dot(b).abs());
            }
            if let (Some(a), Some(b)) = (source.translation, target.translation) {
                distance = distance.max(a.distance(b));
            }
        }
        assert!(
            (actual.index, actual.play_rate) == (source.index, source.play_rate)
                && dot > 1.0 - 1e-5
                && distance < 1e-5,
            "Native notifying-frame QuickStop changed with later inputs: actual_rate={}, source_rate={}, actual_clock={}, source_clock={}, rotation_degrees={}, translation_meters={distance}, source_stack={source_stack}, actual_stack={actual_stack}",
            actual.play_rate,
            source.play_rate,
            actual.clip_time,
            source.clip_time,
            (2.0 * dot.clamp(-1.0, 1.0).acos()).to_degrees()
        );
        let (parented, _, details) = run(false, true, false);
        assert!(
            parented.index == 1.0 && (parented.play_rate - 2.0625).abs() < 1e-5,
            "Source generated QuickStop must use the 45-degree actor world difference: {details}"
        );
        let (consumed, _, details) = run(false, false, true);
        assert!(
            consumed.index == 1.0 && (consumed.play_rate - 2.375).abs() < 1e-5,
            "Source generated QuickStop must use consumed input when raw input is zero: {details}"
        );
    }

    #[test]
    fn game_thread_overlay_notify_plays_after_same_frame_worker_stop() {
        fn run(leave_ground: bool) {
            let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
            app.world_mut().entity_mut(avatar).insert(
                crate::animation_visibility::AlsAnimationTickOption::AlwaysTickPoseAndRefreshBones,
            );
            app.world_mut().entity_mut(character).insert((
                avian3d::prelude::RigidBody::Kinematic,
                bevy_tnua::TnuaToggle::Disabled,
                crate::als::state::RotationMode::ViewDirection,
            ));
            app.world_mut()
                .get_mut::<crate::als::transition::AlsStateRequest>(character)
                .unwrap()
                .overlay_state = Some(crate::als::state::OverlayState::Rifle);
            #[derive(Resource)]
            struct FrameInput(bool, bool);
            app.insert_resource(FrameInput(false, false));
            app.add_systems(
                PostUpdate,
                (move |input: Res<FrameInput>,
                       mut actors: Query<(
                    &mut crate::actual_motion::AlsActualMotion,
                    &mut crate::als::state::LocomotionState,
                    &mut crate::als::state::RotationMode,
                )>| {
                    let (mut motion, mut state, mut mode) = actors.get_mut(character).unwrap();
                    motion.horizontal_speed = 0.0;
                    motion.world_velocity = Vec3::ZERO;
                    motion.local_velocity = Vec2::ZERO;
                    motion.has_input = false;
                    *state = if input.0 {
                        crate::als::state::LocomotionState::Air
                    } else {
                        crate::als::state::LocomotionState::Grounded
                    };
                    *mode = if input.1 {
                        crate::als::state::RotationMode::Aiming
                    } else {
                        crate::als::state::RotationMode::ViewDirection
                    };
                })
                .after(crate::standing_layer::AlsStandingLayerSet)
                .before(crate::ab_als_root::AlsRootGraphSet),
            );
            for _ in 0..150 {
                app.update();
            }
            let before = app
                .world()
                .get::<TransitionNotifyTelemetry>(character)
                .unwrap()
                .clone();
            *app.world_mut().resource_mut::<FrameInput>() = FrameInput(leave_ground, true);
            app.world_mut()
                .get_mut::<crate::als::transition::AlsStateRequest>(character)
                .unwrap()
                .rotation_mode = Some(crate::als::state::RotationMode::Aiming);
            for _ in 0..4 {
                app.update();
                if app
                    .world()
                    .get::<TransitionNotifyTelemetry>(character)
                    .unwrap()
                    .transition_animation_play_count
                    > before.transition_animation_play_count
                {
                    break;
                }
            }
            let after = app
                .world()
                .get::<TransitionNotifyTelemetry>(character)
                .unwrap();
            let avatar = app
                .world()
                .get::<AlsCharacterAssembly>(character)
                .unwrap()
                .avatar;
            let target = app
                .world()
                .get::<AnimatedSceneInstance>(avatar)
                .unwrap()
                .player_entity();
            let player = app.world().get::<AnimationGraphPlayer>(target).unwrap();
            assert!(
                after.transition_animation_play_count > before.transition_animation_play_count,
                "Actual Rifle did not dispatch its ready notification: {after:?}, rotation={:?}, source_rotation={:?}, moving={:?}, player_error={:?}, paused={}",
                app.world()
                    .get::<crate::extensions::SourceAnimationInputs>(target)
                    .unwrap()
                    .gameplay_tag_input("rotation_mode"),
                crate::extensions::source_instance::source_player_property(player, "rotation_mode"),
                crate::extensions::source_instance::source_player_property(
                    player,
                    "locomotion_state.b_moving"
                ),
                player.get_error(),
                player.is_paused()
            );
            if leave_ground {
                assert!(
                    after.stop_animations_count > before.stop_animations_count,
                    "Actual Grounded exit did not queue its worker stop: {after:?}"
                );
            }
            assert!(
                app.world()
                    .get::<AlsTransitionMontageStack>(character)
                    .is_some_and(|stack| stack.entries.iter().any(|entry| entry.active
                        && crate::stop_link::source_transition_sequence(entry)
                            == "A_Als_Stand_Transition_Left")),
                "Game-thread notify must play in its frame after worker stop; leave_ground={leave_ground}, telemetry={after:?}"
            );
        }
        run(false);
        run(true);
    }

    #[test]
    fn queued_turn_with_loaded_clip_is_ready_for_the_next_update() {
        use crate::character_assembly::{AlsCharacterBlueprint, spawn_als_character};
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, _, _, _) = crate::camera_component::tests::loaded_camera_scene();
        let character = app
            .world_mut()
            .run_system_once(
                |mut commands: Commands,
                 assets: Res<AssetServer>,
                 mut params: ResMut<Assets<crate::als::params::AlsMovementParams>>,
                 mut configs: ResMut<
                    Assets<crate::als::control_scheme::AlsControlSchemeConfig>,
                >| {
                    spawn_als_character(
                        &mut commands,
                        &assets,
                        &mut params,
                        &mut configs,
                        AlsCharacterBlueprint {
                            spawn_input: false,
                            ..AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 3.0, 0.0))
                        },
                    )
                },
            )
            .unwrap();
        app.world_mut().entity_mut(character).insert((
            avian3d::prelude::RigidBody::Kinematic,
            bevy_tnua::TnuaToggle::Disabled,
        ));
        let avatar = app
            .world()
            .get::<AlsCharacterAssembly>(character)
            .unwrap()
            .avatar;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
        loop {
            app.update();
            let ready = app
                .world()
                .get::<AnimatedSceneInstance>(avatar)
                .is_some_and(|instance| {
                    app.world()
                        .get::<AnimationGraphPlayer>(instance.player_entity())
                        .is_some()
                })
                && app.world().get::<AlsTurnMontageStack>(avatar).is_some()
                && crate::standing_source::sample_source_montage(
                    "A_Als_Turn_90_Left",
                    0.0,
                    app.world()
                        .resource::<crate::standing_source::SourceMontageAssets>(),
                    app.world()
                        .resource::<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>(
                        ),
                    app.world()
                        .resource::<Assets<bevy_animation_graph::core::skeleton::Skeleton>>(),
                )
                .is_some();
            if ready {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Turn source clip did not load"
            );
        }
        app.world_mut()
            .run_system_once(move |mut commands: Commands| {
                queue_turn_in_grounded_group(
                    &mut commands,
                    character,
                    avatar,
                    SourceTurnRequest {
                        variant: crate::als::turn_in_place::TurnVariant::Left90,
                        yaw: -90.0,
                        crouching: false,
                    },
                );
            })
            .unwrap();
        app.world_mut().flush();
        let started = app
            .world()
            .get::<AlsTurnMontageStack>(avatar)
            .unwrap()
            .entries
            .last()
            .unwrap();
        assert!(started.sampling_ready);
        let id = started.id;
        app.update();
        assert!(
            app.world()
                .get::<AlsTurnMontageStack>(avatar)
                .unwrap()
                .entries
                .iter()
                .any(|entry| entry.id == id && entry.elapsed > 0.0)
        );

        app.world_mut()
            .remove_resource::<crate::standing_source::SourceMontageAssets>();
        app.world_mut()
            .entity_mut(avatar)
            .insert(AlsTurnMontageStack::default());
        app.world_mut()
            .run_system_once(move |mut commands: Commands| {
                queue_turn_in_grounded_group(
                    &mut commands,
                    character,
                    avatar,
                    SourceTurnRequest {
                        variant: crate::als::turn_in_place::TurnVariant::Right90,
                        yaw: 90.0,
                        crouching: false,
                    },
                );
            })
            .unwrap();
        app.world_mut().flush();
        assert!(
            !app.world()
                .get::<AlsTurnMontageStack>(avatar)
                .unwrap()
                .entries
                .last()
                .unwrap()
                .sampling_ready
        );
    }

    #[test]
    fn destroyed_animation_instances_release_all_notification_cursors() {
        use crate::character_assembly::{
            AlsCharacterAssembly, AlsCharacterBlueprint, spawn_als_character,
        };
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, _, _, _) = crate::camera_component::tests::loaded_camera_scene();
        for removal in 0..4 {
            let character = app
                .world_mut()
                .run_system_once(
                    |mut commands: Commands,
                     assets: Res<AssetServer>,
                     mut params: ResMut<Assets<crate::als::params::AlsMovementParams>>,
                     mut configs: ResMut<
                        Assets<crate::als::control_scheme::AlsControlSchemeConfig>,
                    >| {
                        spawn_als_character(
                            &mut commands,
                            &assets,
                            &mut params,
                            &mut configs,
                            AlsCharacterBlueprint {
                                spawn_input: false,
                                ..AlsCharacterBlueprint::new(Transform::from_xyz(0.0, 3.0, 0.0))
                            },
                        )
                    },
                )
                .unwrap();
            app.world_mut().entity_mut(character).insert((
                avian3d::prelude::RigidBody::Kinematic,
                bevy_tnua::TnuaToggle::Disabled,
            ));
            let avatar = app
                .world()
                .get::<AlsCharacterAssembly>(character)
                .unwrap()
                .avatar;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
            let player = loop {
                app.update();
                if let Some(instance) = app.world().get::<AnimatedSceneInstance>(avatar)
                    && app
                        .world()
                        .get::<AnimationGraphPlayer>(instance.player_entity())
                        .unwrap()
                        .get_default_output_pose()
                        .is_some()
                {
                    break instance.player_entity();
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "Character animation did not load"
                );
                std::thread::sleep(std::time::Duration::from_millis(5));
            };
            let previous_instance = app
                .world()
                .resource::<TransitionNotifyCursor>()
                .source_frames[&player]
                .0;
            let instance = app.world().get::<AnimationGraphPlayer>(player).unwrap();
            let bevy_animation_graph::core::animation_graph_player::AnimationSource::Graph(graph) =
                instance.get_animation_source()
            else {
                panic!("Character graph is missing");
            };
            let graph = graph.clone();
            let stale = (
                player,
                instance.get_context_arena().unwrap().get_toplevel_id(),
                NodeId::default(),
            );
            app.world_mut()
                .resource_mut::<TransitionNotifyCursor>()
                .generations
                .insert(stale, 1);
            app.world_mut()
                .get_mut::<AnimationGraphPlayer>(player)
                .unwrap()
                .start(graph);
            app.update();
            let cursor = app.world().resource::<TransitionNotifyCursor>();
            assert_ne!(cursor.source_frames[&player].0, previous_instance);
            assert!(
                !cursor.generations.contains_key(&stale),
                "Reinitialized instance retained old FSM generations"
            );
            {
                let mut cursor = app.world_mut().resource_mut::<TransitionNotifyCursor>();
                assert!(cursor.source_frames.contains_key(&player));
                assert!(cursor.generations.keys().any(|key| key.0 == player));
                cursor
                    .queued_transitions
                    .insert(character, AlsTransitionPlayback::stop(0.0));
                cursor.queued_turns.insert(
                    character,
                    SourceTurnRequest {
                        variant: crate::als::turn_in_place::TurnVariant::Left90,
                        yaw: -90.0,
                        crouching: false,
                    },
                );
            }
            match removal {
                0 => {
                    app.world_mut().despawn(character);
                }
                1 => {
                    app.world_mut().despawn(avatar);
                }
                2 => {
                    app.world_mut().despawn(player);
                }
                _ => {
                    app.world_mut()
                        .entity_mut(player)
                        .remove::<AnimationGraphPlayer>();
                }
            }
            app.world_mut()
                .run_system_once(dispatch_transition_notifications)
                .unwrap();
            let cursor = app.world().resource::<TransitionNotifyCursor>();
            assert!(
                !cursor.source_frames.contains_key(&player),
                "Removed instance retained its source frame: {removal}"
            );
            assert!(
                !cursor.generations.keys().any(|key| key.0 == player),
                "Removed instance retained FSM generations: {removal}"
            );
            assert!(!cursor.queued_transitions.contains_key(&character));
            assert!(!cursor.queued_turns.contains_key(&character));
            if app.world().get_entity(character).is_ok() {
                app.world_mut().despawn(character);
            }
            app.update();
        }
    }

    #[test]
    fn source_transition_call_preserves_arguments_and_standing_idle_guard() {
        use crate::als::state::Stance;
        use crate::extensions::interruptible_fsm::SourceTransitionNotification;
        use bevy_animation_graph::core::edge_data::DataValue;
        let mut registry = bevy::reflect::TypeRegistry::default();
        registry.register::<SourceTransitionNotification>();
        let mut deserializer = ron::de::Deserializer::from_str("(phase: \"TransitionStart\", name: \"CustomNotification\", function: \"PlayTransitionLeftAnimation\", arguments: [(property: \"BlendInDuration\", value: F32(0.2)), (property: \"BlendOutDuration\", value: F32(0.2)), (property: \"PlayRate\", value: F32(1.5)), (property: \"StartTime\", value: F32(0.3)), (property: \"bFromStandingIdleOnly\", value: Bool(true))])").unwrap();
        let reflected = serde::de::DeserializeSeed::deserialize(
            bevy::reflect::serde::TypedReflectDeserializer::new(
                registry
                    .get(std::any::TypeId::of::<SourceTransitionNotification>())
                    .unwrap(),
                &registry,
            ),
            &mut deserializer,
        )
        .unwrap();
        let mut notification =
            SourceTransitionNotification::from_reflect(reflected.as_partial_reflect()).unwrap();
        let playback = transition_animation_call(&notification, Stance::Standing, false).unwrap();
        assert_eq!(
            crate::stop_link::source_transition_sequence(&playback),
            "A_Als_Stand_Transition_Left"
        );
        assert_eq!(
            (
                playback.blend_in,
                playback.blend_out,
                playback.play_rate,
                playback.start_time,
                playback.clip_time
            ),
            (0.2, 0.2, 1.5, 0.3, 0.3)
        );
        assert!(transition_animation_call(&notification, Stance::Standing, true).is_none());
        assert!(transition_animation_call(&notification, Stance::Crouching, false).is_none());
        notification.name = "AnotherNotification".to_owned();
        let renamed = transition_animation_call(&notification, Stance::Standing, false).unwrap();
        assert_ne!(renamed.instance_id, playback.instance_id);
        let mut expected = playback;
        expected.instance_id = renamed.instance_id;
        assert_eq!(renamed, expected);
        notification.arguments[4].value = DataValue::Bool(false);
        notification.arguments[3].value = DataValue::F32(0.65);
        notification.function = "PlayTransitionRightAnimation".to_owned();
        let crouching = transition_animation_call(&notification, Stance::Crouching, true).unwrap();
        assert_eq!(
            crate::stop_link::source_transition_sequence(&crouching),
            "A_Als_Crouch_Transition_Right"
        );
        assert_eq!((crouching.start_time, crouching.clip_time), (0.65, 0.65));
        notification.arguments.pop();
        assert!(transition_animation_call(&notification, Stance::Standing, false).is_none());
    }

    #[test]
    fn state_function_transition_waits_until_the_source_stop_request_is_consumed() {
        let character = Entity::PLACEHOLDER;
        let mut cursor = TransitionNotifyCursor::default();
        cursor
            .queued_transitions
            .insert(character, AlsTransitionPlayback::stop(0.0));
        cursor
            .queued_transitions
            .insert(character, AlsTransitionPlayback::stop(1.0));
        assert!(cursor.take_transition(character, true).is_none());
        let playback = cursor.take_transition(character, false).unwrap();
        assert_eq!(playback.index, 1.0);
        assert_eq!(
            (
                playback.start_time,
                playback.play_rate,
                playback.blend_in,
                playback.blend_out
            ),
            (0.4, 1.5, 0.2, 0.2)
        );
        assert!(cursor.take_transition(character, false).is_none());
        assert_eq!(
            transition_notify_action("PlayStopLeftTransitionAnimation"),
            Some(TransitionNotifyAction::StopLeft)
        );
        assert_eq!(
            transition_notify_action("PlayStopRightTransitionAnimation"),
            Some(TransitionNotifyAction::StopRight)
        );
        assert_eq!(
            transition_notify_action("StopTransitionAndTurnInPlaceAnimations"),
            Some(TransitionNotifyAction::StopAnimations)
        );
    }

    #[test]
    fn crouching_stop_entry_has_a_real_dispatch_action() {
        assert!(transition_notify_action("PlayStopTransitionAnimation").is_some());
    }

    #[test]
    fn grounded_roll_exit_has_a_real_dispatch_action() {
        assert_eq!(
            transition_notify_action("PlayRollToGroundedTransitionAnimation"),
            Some(TransitionNotifyAction::TransitionRight)
        );
        for (stance, expected) in [
            (
                crate::als::state::Stance::Standing,
                "A_Als_Stand_Transition_Right",
            ),
            (
                crate::als::state::Stance::Crouching,
                "A_Als_Crouch_Transition_Right",
            ),
        ] {
            let playback = AlsTransitionPlayback::roll_to_grounded_right(
                stance == crate::als::state::Stance::Crouching,
            );
            assert_eq!(
                crate::stop_link::source_transition_sequence(&playback),
                expected
            );
            assert_eq!(
                (
                    playback.start_time,
                    playback.play_rate,
                    playback.blend_in,
                    playback.blend_out
                ),
                (0.2, 1.5, 0.2, 0.2)
            );
            assert!(crate::standing_source::MONTAGES.contains_key(expected));
        }
    }

    #[test]
    fn grounded_entry_reset_only_consumes_the_current_source_call() {
        let mut source = SourceInstanceProperties::default();
        let key = "grounded_entry_mode.reset_requested".to_owned();
        assert!(!grounded_entry_reset_requested(&source, 5));
        source.values.insert(key.clone(), true.into());
        source.write_frames.insert(key.clone(), 5);
        assert!(!grounded_entry_reset_requested(&source, 4));
        assert!(grounded_entry_reset_requested(&source, 5));
        assert!(!grounded_entry_reset_requested(&source, 6));
        source.values.insert(key, false.into());
        assert!(!grounded_entry_reset_requested(&source, 5));
    }

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
