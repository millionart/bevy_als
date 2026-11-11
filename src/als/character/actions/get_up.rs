use std::sync::LazyLock;

use crate::als::{
    character::AlsCharacter, motion::AlsMovementInputBlocked, roll::AlsRollState,
};
use crate::extensions::{
    AlsAnimCurves,
    animation_context::{AnimationContextState, SourceRelevantAsset},
    profile_blend::{AlphaBlendMode, MontageBlend},
};
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_clip::GraphClip,
    animation_graph_player::AnimationGraphPlayer, edge_data::DataValue, plugin::AnimationGraphSet,
};
use serde::Deserialize;

use crate::{
    animation_events::{AlsAnimationEventSet, AlsNotifyLocomotionAction},
    character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar},
};

pub const POST_LOCOMOTION_POSE: &str = "post_locomotion.pose";
pub const POST_LOCOMOTION_WEIGHT: &str = "post_locomotion.weight";
pub const POST_LOCOMOTION_LAYER_WEIGHT: &str = "post_locomotion.layer_weight";

#[derive(Deserialize)]
pub struct GetUpClipSettings {
    pub source_montage: String,
    pub source_sequence: String,
    pub clip: String,
    pub curves: String,
    pub sequence_length: f32,
    pub play_rate: f32,
    pub blend_out_seconds: f32,
    pub notify_end_clip_seconds: f32,
}

#[derive(Deserialize)]
pub struct AlsRagdollSettings {
    #[serde(default)]
    pub interrupt_rolling_when_in_air: bool,
    pub start_on_land: bool,
    pub on_land_speed_threshold_mps: f32,
    pub minimum_initial_speed_limit_mps: f32,
    pub initial_speed_limit_refresh_frames: u8,
    pub reference_speed_mps: f32,
    pub angular_stiffness: f32,
    pub stop_montage_blend_seconds: f32,
    pub limit_initial_speed: bool,
    pub front: GetUpClipSettings,
    pub back: GetUpClipSettings,
}

pub static RAGDOLL_SETTINGS: LazyLock<AlsRagdollSettings> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../../../assets/ALS/Data/Character/als_ragdoll_settings.json"
    ))
    .expect("generated ragdoll settings parse")
});

pub fn get_up_settings(facing_up: bool) -> &'static GetUpClipSettings {
    if facing_up {
        &RAGDOLL_SETTINGS.back
    } else {
        &RAGDOLL_SETTINGS.front
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct AlsGetUpRequest(pub bool);

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsGetUpState {
    target_player: Entity,
    pub facing_up: bool,
    pub elapsed: f32,
    pub clip_time: f32,
    pub weight: f32,
    pub pose_ready: bool,
    pub(crate) interrupted: bool,
    stop: Option<MontageBlend>,
}

impl AlsGetUpState {
    pub(crate) fn new(facing_up: bool) -> Self {
        Self {
            target_player: Entity::PLACEHOLDER,
            facing_up,
            elapsed: 0.0,
            clip_time: 0.0,
            weight: 1.0,
            pose_ready: false,
            interrupted: false,
            stop: None,
        }
    }

    pub(crate) fn stop(&mut self, duration: f32) {
        self.interrupted = true;
        let configured_duration = self.stop.map(|blend| blend.duration);
        if configured_duration.is_none_or(|previous| duration < previous) {
            self.stop = Some(MontageBlend::new(self.weight, 0.0, duration, AlphaBlendMode::Cubic));
        }
    }

    pub(crate) fn advance(&mut self, delta: f32, settings: &GetUpClipSettings) {
        if let Some(blend) = &mut self.stop {
            self.weight = blend.advance(delta);
        }
        self.elapsed += delta;
        let mut time_remaining = delta;
        for _ in 0..9 {
            if time_remaining <= 1e-8 || self.clip_time >= settings.sequence_length { break; }
            let mut movement = time_remaining * settings.play_rate;
            if self.clip_time < settings.notify_end_clip_seconds
                && settings.notify_end_clip_seconds <= self.clip_time + movement {
                movement = settings.notify_end_clip_seconds - self.clip_time;
            }
            movement = movement.min(settings.sequence_length - self.clip_time);
            if movement <= 0.0 { break; }
            self.clip_time += movement;
            time_remaining = (time_remaining - movement / settings.play_rate).max(0.0);
            let to_end = (settings.sequence_length - self.clip_time) / settings.play_rate;
            if self.stop.is_none() && to_end <= settings.blend_out_seconds.max(1e-4) {
                self.stop = Some(MontageBlend::new(self.weight, 0.0, to_end, AlphaBlendMode::Cubic));
                if to_end <= 0.0 { self.weight = 0.0; }
            }
        }
    }
}

#[derive(Resource)]
struct GetUpAssets {
    clips: [Handle<GraphClip>; 2],
    curves: [Handle<AlsAnimCurves>; 2],
}

fn load_get_up_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GetUpAssets {
        clips: [
            assets.load(&RAGDOLL_SETTINGS.front.clip),
            assets.load(&RAGDOLL_SETTINGS.back.clip),
        ],
        curves: [
            assets.load(&RAGDOLL_SETTINGS.front.curves),
            assets.load(&RAGDOLL_SETTINGS.back.curves),
        ],
    });
}

fn start_get_up(
    mut commands: Commands,
    requests: Query<(Entity, &AlsGetUpRequest, Option<&AlsCharacterAssembly>, Option<&AlsGetUpState>)>,
    mut rolls: Query<&mut AlsRollState>,
    assets: Res<GetUpAssets>,
    clips: Res<Assets<GraphClip>>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
) {
    for (character, request, assembly, previous) in &requests {
        commands.entity(character).remove::<AlsGetUpRequest>();
        let Some(target_player) = assembly.and_then(|assembly| avatars.get(assembly.avatar).ok())
            .filter(|(avatar, instance)| avatar.character == character && players.contains(instance.player_entity()))
            .map(|(_, instance)| instance.player_entity()) else { continue; };
        if !clips.contains(&assets.clips[usize::from(request.0)]) {
            error!("[GET-UP] clip was not loaded when recovery requested character={character:?}");
            continue;
        }
        if let Ok(mut roll) = rolls.get_mut(character) {
            roll.active = false;
        }
        if let Some(previous) = previous && previous.target_player != target_player
            && let Ok(mut player) = players.get_mut(previous.target_player) {
            clear_get_up_slot(&mut player, source_inputs.get_mut(previous.target_player).ok());
        }
        commands
            .entity(character)
            .remove::<crate::roll_drive::AlsRollBlendOut>();
        commands.entity(character).insert((
            AlsGetUpState { target_player, ..AlsGetUpState::new(request.0) },
            AlsMovementInputBlocked,
            AlsNotifyLocomotionAction::GettingUp,
        ));
    }
}

fn clear_get_up_slot(player: &mut AnimationGraphPlayer, inputs: Option<Mut<crate::extensions::SourceAnimationInputs>>) {
    player.set_global_input_data(POST_LOCOMOTION_WEIGHT, 0.0_f32);
    player.set_global_input_data(POST_LOCOMOTION_LAYER_WEIGHT, 0.0_f32);
    if let Some(mut inputs) = inputs { inputs.set_slot_input_context(POST_LOCOMOTION_POSE, None); }
}

fn end_get_up_instance(commands: &mut Commands, character: Entity, action: Option<&AlsNotifyLocomotionAction>) {
    commands.entity(character).try_remove::<AlsGetUpState>();
    if action.is_none_or(|action| *action == AlsNotifyLocomotionAction::None || *action == AlsNotifyLocomotionAction::GettingUp) {
        commands.entity(character).try_remove::<AlsMovementInputBlocked>();
    }
    if action == Some(&AlsNotifyLocomotionAction::GettingUp) {
        commands.entity(character).try_insert(AlsNotifyLocomotionAction::None);
    }
}

fn discard_get_up_target(
    event: On<Discard, AnimationGraphPlayer>,
    states: Query<(Entity, &AlsGetUpState, Option<&AlsNotifyLocomotionAction>)>,
    mut commands: Commands,
) {
    for (character, state, action) in &states {
        if state.target_player == event.entity { end_get_up_instance(&mut commands, character, action); }
    }
}

pub(crate) fn sampled_curve_context(
    curves: &crate::extensions::AlsPoseCurves,
    time: f32,
) -> AnimationContextState {
    AnimationContextState {
        curves: curves
            .0
            .iter()
            .map(|(name, curve)| (name.clone(), curve.sample(time)))
            .collect(),
        relevant_asset: None,
        curves_are_authoritative: true,
        ..default()
    }
}

fn update_get_up_pose(
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GetUpAssets>,
    clips: Res<Assets<GraphClip>>,
    curves: Res<Assets<AlsAnimCurves>>,
    custom_dilations: Query<&crate::als_camera::AlsCameraCustomTimeDilation>,
    mut characters: Query<
        (
            Entity,
            Option<&AlsNotifyLocomotionAction>,
            &mut AlsGetUpState,
            Option<&AlsRollState>,
            Option<&AlsCharacterAssembly>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (character, action, mut state, roll, assembly) in &mut characters {
        let player_entity = state.target_player;
        if !assembly.and_then(|assembly| avatars.get(assembly.avatar).ok())
            .is_some_and(|(avatar, instance)| avatar.character == character && instance.player_entity() == player_entity)
            || !players.contains(player_entity) {
            if let Ok(mut player) = players.get_mut(player_entity) {
                clear_get_up_slot(&mut player, source_inputs.get_mut(player_entity).ok());
            }
            end_get_up_instance(&mut commands, character, action);
            continue;
        }
        let previous_clip_time = state.clip_time;
        if action == Some(&AlsNotifyLocomotionAction::Ragdolling) {
            state.stop(RAGDOLL_SETTINGS.stop_montage_blend_seconds);
        } else if roll.is_some_and(|roll| roll.active) {
            state.stop(crate::als::roll::ROLL_WEIGHT_IN_SECS);
        }
        let settings = get_up_settings(state.facing_up);
        let delta = time.delta_secs() * custom_dilations.get(character).map_or(1.0, |value| value.0);
        state.advance(delta, settings);
        let mut player = players.get_mut(player_entity).unwrap();
        if state.weight <= 0.0 {
            clear_get_up_slot(&mut player, source_inputs.get_mut(player_entity).ok());
            end_get_up_instance(&mut commands, character, action);
            continue;
        }
        let index = usize::from(state.facing_up);
        let (Some(clip), Some(curves)) = (
            clips.get(&assets.clips[index]),
            curves
                .get(&assets.curves[index])
                .and_then(AlsAnimCurves::first),
        ) else {
            state.pose_ready = false;
            continue;
        };
        let mut pose = crate::extensions::source_sequence_player::sample_pose(clip, state.clip_time);
        let mut context = sampled_curve_context(curves, state.clip_time);
        context.relevant_asset = Some(SourceRelevantAsset {
            duration: clip.duration,
            position: state.clip_time,
            previous_position: previous_clip_time,
            has_previous_position: true,
            delta_seconds: delta,
            playback_delta_seconds: state.clip_time - previous_clip_time,
            looping: false,
            weight: 1.0,
        });
        let mut clock = context.relevant_asset.clone().unwrap();
        clock.weight = state.weight;
        context.asset_samples.push(crate::extensions::animation_context::SourceAssetSample {
            identity: "get_up".to_owned(), clip: assets.clips[index].clone(), clock, montage: true,
            interrupted: state.interrupted, ..default()
        });
        let roll_weight = roll
            .filter(|roll| roll.active)
            .and_then(|_| player.get_global_input_data("roll.weight"))
            .and_then(|value| value.as_f32().ok())
            .unwrap_or(0.0);
        let mut weight = state.weight;
        if roll_weight > 0.0 {
            if let Some(roll_pose) = player
                .get_global_input_data("roll.pose")
                .and_then(|value| value.as_pose().ok())
            {
                let alpha = roll_weight / (weight + roll_weight);
                pose = crate::extensions::slot_node::blend_slot_pose(pose, Some(roll_pose), Some(alpha));
                if let Some(roll_context) = source_inputs.get(player_entity).ok().and_then(|inputs| inputs.slot_input_context("roll.pose",)) {
                    context = crate::extensions::source_pose_nodes::blend_animation_contexts([
                        (context, 1.0 - alpha),
                        (roll_context, alpha),
                    ]);
                }
                weight = (weight + roll_weight).min(1.0);
            }
        }
        player.set_global_input_data(POST_LOCOMOTION_POSE, DataValue::Pose(pose));
        player.set_global_input_data(POST_LOCOMOTION_WEIGHT, weight);
        player.set_global_input_data(POST_LOCOMOTION_LAYER_WEIGHT, 1.0_f32);
        source_inputs.get_mut(player_entity).unwrap().set_slot_input_context(POST_LOCOMOTION_POSE, Some(context));
        state.pose_ready = true;
    }
}

fn release_get_up_input(
    mut commands: Commands,
    characters: Query<(Entity, &AlsNotifyLocomotionAction), With<AlsMovementInputBlocked>>,
) {
    for (entity, action) in &characters {
        if *action == AlsNotifyLocomotionAction::None {
            commands.entity(entity).remove::<AlsMovementInputBlocked>();
        }
    }
}

pub struct AlsGetUpPlugin;

impl Plugin for AlsGetUpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_get_up_assets)
            .add_observer(discard_get_up_target)
            .add_systems(
                Update,
                start_get_up.after(crate::ragdoll_link::restore_ragdoll_player_parent),
            )
            .add_systems(
                PostUpdate,
                update_get_up_pose.before(AnimationGraphSet::PrePhysics),
            )
            .add_systems(
                PostUpdate,
                release_get_up_input
                    .after(AlsAnimationEventSet::Dispatch)
                    .before(crate::roll_drive::AlsRollDriveSet),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_audit_get_up_slot_uses_the_current_actor_clock() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, character, _) = crate::roll_drive::tests::loaded_roll_character();
        play_loaded_get_up(&mut app, character, false);
        let target = app.world().get::<AlsGetUpState>(character).unwrap().target_player;
        let mut failures = Vec::new();
        for scale in [0.0_f32, 0.5, 1.0, 2.0] {
            app.world_mut().entity_mut(character).insert(crate::als_camera::AlsCameraCustomTimeDilation(scale));
            let before = app.world().get::<AlsGetUpState>(character).unwrap().elapsed;
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
            app.world_mut().run_system_once(update_get_up_pose).unwrap();
            let state = app.world().get::<AlsGetUpState>(character).unwrap();
            let expected = before + 0.05 * scale;
            let context = app.world().get::<crate::extensions::SourceAnimationInputs>(target).unwrap()
                .slot_input_context(POST_LOCOMOTION_POSE).unwrap();
            let sampled = context.asset_samples[0].clock.position;
            if (state.elapsed - expected).abs() > 1e-6
                || (sampled - expected * get_up_settings(false).play_rate).abs() > 1e-6
                || (context.asset_samples[0].clock.delta_seconds - 0.05 * scale).abs() > 1e-6 {
                failures.push((scale, expected, state.elapsed, sampled, context.asset_samples[0].clock.delta_seconds));
            }
        }
        assert!(failures.is_empty(), "Get-up ignored actor time dilation: {failures:?}");
    }

    fn play_loaded_get_up(app: &mut App, character: Entity, facing_up: bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let assets = app.world().resource::<GetUpAssets>();
            if app.world().resource::<Assets<GraphClip>>().contains(&assets.clips[usize::from(facing_up)])
                && app.world().resource::<Assets<AlsAnimCurves>>().contains(&assets.curves[usize::from(facing_up)]) { break; }
            app.update();
            assert!(std::time::Instant::now() < deadline, "Get-up assets did not become ready");
        }
        app.world_mut().entity_mut(character).insert(AlsGetUpRequest(facing_up));
        for _ in 0..4 { app.update(); }
        let state = app.world().get::<AlsGetUpState>(character).unwrap();
        assert!(state.pose_ready && state.clip_time > 0.0 && state.weight > 0.9);
        assert!(app.world().get::<AlsMovementInputBlocked>(character).is_some());
        assert_eq!(app.world().get::<AlsNotifyLocomotionAction>(character), Some(&AlsNotifyLocomotionAction::GettingUp));
        let avatar = app.world().get::<crate::character_assembly::AlsCharacterAssembly>(character).unwrap().avatar;
        let target = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        let player = app.world().get::<AnimationGraphPlayer>(target).unwrap();
        assert!(player.get_error().is_none() && player.get_default_output_pose().is_some());
        assert!(player.get_global_input_data(POST_LOCOMOTION_POSE).and_then(|value| value.as_pose().ok()).is_some_and(|pose| !pose.bones.is_empty()));
        assert!(app.world().get::<crate::extensions::SourceAnimationInputs>(target).unwrap().slot_input_context(POST_LOCOMOTION_POSE).is_some());
    }

    fn assert_get_up_ended(app: &App, character: Entity) {
        assert!(app.world().get::<AlsGetUpState>(character).is_none(), "Old get-up survived the end of its player instance");
        assert!(app.world().get::<AlsMovementInputBlocked>(character).is_none());
        assert_ne!(app.world().get::<AlsNotifyLocomotionAction>(character), Some(&AlsNotifyLocomotionAction::GettingUp));
        assert!(app.world().get::<crate::animation_events::AlsAnimationEventPlaybacks>(character).unwrap().entries.iter()
            .all(|entry| !entry.source_asset.contains("GetUp")));
    }

    #[test]
    fn get_up_ends_when_its_loaded_player_is_replaced_or_destroyed() {
        use bevy_animation_graph::core::animation_graph_player::AnimationSource;
        use crate::animation_events::AlsAnimationEvent;
        for mode in 0..4 {
            let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let target = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
            play_loaded_get_up(&mut app, character, mode % 2 == 0);
            app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().clear();
            if mode < 2 {
                let player = app.world().get::<AnimationGraphPlayer>(target).unwrap();
                let AnimationSource::Graph(graph) = player.get_animation_source() else { panic!("Character must use its actual graph"); };
                let replacement = AnimationGraphPlayer::new(player.skeleton().clone()).with_graph(graph.clone());
                if mode == 1 { app.world_mut().entity_mut(target).remove::<AnimationGraphPlayer>(); }
                app.world_mut().entity_mut(target).insert(replacement);
            } else if mode == 2 {
                app.world_mut().despawn(target);
            } else {
                let children: Vec<_> = app.world().get::<Children>(avatar).unwrap().iter().rev().collect();
                app.world_mut().entity_mut(avatar).replace_children(&children);
                app.world_mut().despawn(avatar);
            }
            assert!(app.world().get::<AlsGetUpState>(character).is_none(), "Get-up survived player discard mode {mode}");
            assert!(app.world().get::<AlsMovementInputBlocked>(character).is_none());
            assert_ne!(app.world().get::<AlsNotifyLocomotionAction>(character), Some(&AlsNotifyLocomotionAction::GettingUp));
            app.update();
            assert_get_up_ended(&app, character);
            assert!(app.world_mut().resource_mut::<Messages<AlsAnimationEvent>>().drain().any(|event|
                event.character == character && event.source_asset.contains("GetUp") && event.phase == crate::extensions::event_lifecycle::EventPhase::End));
            for _ in 0..3 { app.update(); }
            assert_get_up_ended(&app, character);
            if mode >= 2 {
                use bevy::ecs::system::RunSystemOnce;
                app.world_mut().entity_mut(character).insert(AlsGetUpRequest(false));
                app.world_mut().run_system_once(start_get_up).unwrap();
                assert_get_up_ended(&app, character);
                assert!(app.world().get::<AlsGetUpRequest>(character).is_none());
            }
        }
    }

    #[test]
    fn get_up_replacement_request_clears_the_previous_players_slot() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::character_assembly::{ALS_LINKED_ANIMATED_SCENE, attach_avatar};
        let (mut app, character, old_avatar) = crate::roll_drive::tests::loaded_roll_character();
        let old_player = app.world().get::<AnimatedSceneInstance>(old_avatar).unwrap().player_entity();
        let avatar = app.world_mut().run_system_once_with(|In(character): In<Entity>, mut commands: Commands, assets: Res<AssetServer>| {
            attach_avatar(&mut commands, &assets, character, ALS_LINKED_ANIMATED_SCENE)
        }, character).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let target = loop {
            app.update();
            if let Some(instance) = app.world().get::<AnimatedSceneInstance>(avatar)
                && app.world().get::<AnimationGraphPlayer>(instance.player_entity()).is_some() {
                break instance.player_entity();
            }
            assert!(std::time::Instant::now() < deadline, "Replacement avatar did not become ready");
        };
        play_loaded_get_up(&mut app, character, false);
        app.world_mut().get_mut::<AlsCharacterAssembly>(character).unwrap().avatar = avatar;
        app.world_mut().entity_mut(character).insert(AlsGetUpRequest(true));
        app.world_mut().run_system_once(start_get_up).unwrap();
        assert_eq!(app.world().get::<AnimationGraphPlayer>(old_player).unwrap().get_global_input_data(POST_LOCOMOTION_WEIGHT).unwrap().as_f32().unwrap(), 0.0);
        assert!(app.world().get::<crate::extensions::SourceAnimationInputs>(old_player).unwrap().slot_input_context(POST_LOCOMOTION_POSE).is_none());
        assert_eq!(app.world().get::<AlsGetUpState>(character).unwrap().target_player, target);
        app.update();
        assert!(app.world().get::<AlsGetUpState>(character).unwrap().pose_ready);
        assert_eq!(app.world().get::<AlsNotifyLocomotionAction>(character), Some(&AlsNotifyLocomotionAction::GettingUp));
        assert!(app.world().get::<AlsMovementInputBlocked>(character).is_some());
        let player = app.world().get::<AnimationGraphPlayer>(target).unwrap();
        assert!(player.get_error().is_none() && player.get_default_output_pose().is_some());
    }

    #[test]
    fn get_up_follows_the_current_avatar_and_preserves_normal_overlay_changes() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::character_assembly::{AlsCharacterAssembly, ALS_LINKED_ANIMATED_SCENE, attach_avatar};
        for retain_owner in [false, true] {
        let (mut app, character, old_avatar) = crate::roll_drive::tests::loaded_roll_character();
        let old_player = app.world().get::<AnimatedSceneInstance>(old_avatar).unwrap().player_entity();
        play_loaded_get_up(&mut app, character, false);
        if !retain_owner { app.world_mut().entity_mut(old_avatar).remove::<AlsCharacterAvatar>(); }
        app.world_mut().get_mut::<AnimationGraphPlayer>(old_player).unwrap().pause();
        let avatar = app.world_mut().run_system_once_with(|In(character): In<Entity>, mut commands: Commands, assets: Res<AssetServer>| {
            attach_avatar(&mut commands, &assets, character, ALS_LINKED_ANIMATED_SCENE)
        }, character).unwrap();
        app.world_mut().get_mut::<AlsCharacterAssembly>(character).unwrap().avatar = avatar;
        let target = crate::roll_drive::tests::ready_roll_player(&mut app, avatar);
        assert_get_up_ended(&app, character);
        for player in [old_player, target] {
            assert_eq!(app.world().get::<AnimationGraphPlayer>(player).unwrap().get_global_input_data(POST_LOCOMOTION_WEIGHT)
                .and_then(|value| value.as_f32().ok()).unwrap_or(0.0), 0.0);
            assert!(app.world().get::<crate::extensions::SourceAnimationInputs>(player).unwrap().slot_input_context(POST_LOCOMOTION_POSE).is_none());
        }
        play_loaded_get_up(&mut app, character, true);
        let before = app.world().get::<AlsGetUpState>(character).unwrap().clip_time;
        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().overlay_state = Some(crate::als::state::OverlayState::Rifle);
        app.update();
        assert!(app.world().get::<AlsGetUpState>(character).unwrap().clip_time > before);
        app.world_mut().despawn(old_avatar);
        app.update();
        assert!(app.world().get::<AlsGetUpState>(character).unwrap().pose_ready);
        }
    }

    #[test]
    fn both_get_up_assets_load_sample_and_start_the_actual_request_system() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::skeleton::Skeleton;
        let (mut app, entity, _) = crate::roll_drive::tests::loaded_roll_character();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            app.update();
            let assets = app.world().resource::<GetUpAssets>();
            let clips = app.world().resource::<Assets<GraphClip>>();
            let curves = app.world().resource::<Assets<AlsAnimCurves>>();
            let skeletons = app.world().resource::<Assets<Skeleton>>();
            if assets.clips.iter().all(|handle| {
                clips
                    .get(handle)
                    .is_some_and(|clip| skeletons.contains(&clip.skeleton))
            }) && assets.curves.iter().all(|handle| curves.contains(handle))
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "get-up asset load made no progress within 30 seconds"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let mut signatures = Vec::new();
        for facing_up in [false, true] {
            let index = usize::from(facing_up);
            let assets = app.world().resource::<GetUpAssets>();
            let clip = app
                .world()
                .resource::<Assets<GraphClip>>()
                .get(&assets.clips[index])
                .unwrap();
            let pose = crate::extensions::source_sequence_player::sample_pose(clip, 0.25);
            assert_eq!(pose.paths.len(), 79);
            assert!(pose.paths.values().all(|index| {
                let bone = &pose.bones[*index];
                bone.translation.is_some_and(Vec3::is_finite)
                    && bone.rotation.is_some_and(Quat::is_finite)
                    && bone.scale.is_some_and(Vec3::is_finite)
            }));
            let pelvis = clip
                .curves
                .keys()
                .find_map(|id| {
                    let skeleton = app
                        .world()
                        .resource::<Assets<Skeleton>>()
                        .get(&clip.skeleton)?;
                    let bone_id = bevy_animation_graph::core::id::BoneId::from(*id);
                    let path = skeleton.id_to_path(bone_id)?;
                    (path.to_string().ends_with("/pelvis"))
                        .then(|| pose.get_bone(bone_id).unwrap().rotation.unwrap())
                })
                .expect("get-up clip must contain pelvis");
            signatures.push(pelvis);
            let curves = app
                .world()
                .resource::<Assets<AlsAnimCurves>>()
                .get(&assets.curves[index])
                .unwrap()
                .first()
                .unwrap();
            let context = sampled_curve_context(curves, 0.25);
            assert!(context.curves_are_authoritative && !context.curves.is_empty());
            let roll = AlsRollState {
                active: true,
                time: 0.3,
                ..default()
            };
            app
                .world_mut()
                .entity_mut(entity)
                .insert((
                    AlsGetUpRequest(facing_up),
                    roll,
                    crate::roll_drive::AlsRollBlendOut::new(&roll, crate::get_up::RAGDOLL_SETTINGS.stop_montage_blend_seconds),
                ));
            app.world_mut().run_system_once(start_get_up).unwrap();
            assert_eq!(
                app.world().get::<AlsGetUpState>(entity).unwrap().facing_up,
                facing_up
            );
            assert!(app.world().get::<AlsMovementInputBlocked>(entity).is_some());
            assert_eq!(
                app.world().get::<AlsNotifyLocomotionAction>(entity),
                Some(&AlsNotifyLocomotionAction::GettingUp)
            );
            assert!(!app.world().get::<AlsRollState>(entity).unwrap().active);
            assert!(
                app.world()
                    .get::<crate::roll_drive::AlsRollBlendOut>(entity)
                    .is_none()
            );
        }
        assert!(signatures[0].angle_between(signatures[1]) > 0.5);
    }

    #[test]
    fn input_unlock_waits_for_the_locomotion_action_to_clear() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let entity = world
            .spawn((
                AlsMovementInputBlocked,
                AlsNotifyLocomotionAction::GettingUp,
            ))
            .id();
        world.run_system_once(release_get_up_input).unwrap();
        assert!(world.get::<AlsMovementInputBlocked>(entity).is_some());
        world
            .entity_mut(entity)
            .insert(AlsNotifyLocomotionAction::None);
        world.run_system_once(release_get_up_input).unwrap();
        assert!(world.get::<AlsMovementInputBlocked>(entity).is_none());
    }

    #[test]
    fn browser_review_montage_natural_zero_blend_waits_for_its_end() {
        let settings = GetUpClipSettings {
            source_montage: String::new(), source_sequence: String::new(), clip: String::new(),
            curves: String::new(), sequence_length: 1.5, play_rate: 1.2,
            blend_out_seconds: 0.0, notify_end_clip_seconds: 1.0,
        };
        let mut state = AlsGetUpState::new(true);
        state.advance(1.0 / 60.0, &settings);
        assert_eq!(state.weight, 1.0, "Natural zero blend ended before the montage end window");
        state.advance(1.25 - state.elapsed - 0.01, &settings);
        assert_eq!(state.weight, 1.0);
        state.advance(0.01 - 0.00005, &settings);
        assert_eq!(state.weight, 1.0);
        let remaining = (settings.sequence_length - state.clip_time) / settings.play_rate;
        state.advance(remaining * 0.5, &settings);
        assert!((state.weight - 0.5).abs() < 1e-5,
            "Natural zero end window did not use the source remaining-time blend: {}", state.weight);
        assert!(!state.interrupted);
        state.advance(remaining * 0.5, &settings);
        assert_eq!(state.weight, 0.0);
        let mut stopped = AlsGetUpState::new(true);
        stopped.stop(0.0);
        stopped.advance(0.0, &settings);
        assert_eq!(stopped.weight, 0.0);
    }

    #[test]
    fn source_notify_releases_input_before_montage_blend_finishes() {
        for facing_up in [false, true] {
            let settings = get_up_settings(facing_up);
            let mut state = AlsGetUpState::new(facing_up);
            state.advance(settings.sequence_length / settings.play_rate - settings.blend_out_seconds + 0.001, settings);
            state.advance(settings.notify_end_clip_seconds / settings.play_rate - state.elapsed, settings);
            assert!(state.weight > 0.0 && state.weight < 1.0);
            assert!(state.clip_time < settings.sequence_length);
            state.advance(settings.sequence_length / settings.play_rate - state.elapsed, get_up_settings(state.facing_up));
            assert_eq!(state.weight, 0.0);
        }
    }

    #[test]
    fn interrupted_get_up_ends_on_the_source_blend_budget_boundary() {
        let (mut app,character,_)=crate::roll_drive::tests::loaded_roll_character();
        play_loaded_get_up(&mut app,character,true);
        {
            let mut state=app.world_mut().get_mut::<AlsGetUpState>(character).unwrap();
            state.elapsed=0.3;
            state.clip_time=0.3 * get_up_settings(state.facing_up).play_rate;
            state.weight=1.0;
            state.stop=None;
            state.stop(RAGDOLL_SETTINGS.stop_montage_blend_seconds);
        }
        for _ in 0..2 {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.1));
            app.world_mut().run_schedule(PostUpdate);
        }
        assert_get_up_ended(&app,character);
    }

    #[test]
    fn interruption_keeps_source_blend_duration_and_current_weight() {
        let mut state = AlsGetUpState::new(true);
        state.advance(0.3, get_up_settings(state.facing_up));
        state.stop(RAGDOLL_SETTINGS.stop_montage_blend_seconds);
        state.advance(0.1, get_up_settings(state.facing_up));
        assert!((state.weight - 0.5).abs() < 1e-5);
        state.advance(0.1, get_up_settings(state.facing_up));
        assert_eq!(state.weight, 0.0);
    }

    #[test]
    fn late_interruption_restarts_a_shorter_configured_blend_once() {
        let mut state = AlsGetUpState::new(true);
        state.advance(1.18, get_up_settings(state.facing_up));
        let before = state.weight;
        state.stop(0.2);
        state.advance(0.1, get_up_settings(state.facing_up));
        assert!((state.weight - before * 0.5).abs() < 1e-5);
        state.stop(0.2);
        state.advance(0.1, get_up_settings(state.facing_up));
        assert_eq!(state.weight, 0.0);
    }

    #[test]
    fn natural_get_up_uses_the_first_branching_or_frame_stop_budget() {
        for (rate, before, crossing, budget) in [
            (0.6, 2.1, 0.11, 0.29),
            (1.2, 0.9, 0.1, (1.5 - 1.166567) / 1.2),
            (2.4, 0.2, 0.3, (1.5 - 1.166567) / 2.4),
        ] {
            let settings = GetUpClipSettings {
                source_montage: String::new(), source_sequence: String::new(), clip: String::new(),
                curves: String::new(), sequence_length: 1.5, play_rate: rate,
                blend_out_seconds: 0.3, notify_end_clip_seconds: 1.166567,
            };
            let mut state = AlsGetUpState::new(false);
            state.advance(before, &settings);
            assert!(state.stop.is_none());
            state.advance(crossing, &settings);
            assert_eq!(state.weight, 1.0, "Natural Stop consumed weight in its trigger frame");
            assert!((state.stop.unwrap().duration - budget).abs() < 1e-6,
                "Natural GetUp ignored the first source substep that starts the blend");
            state.advance(budget * 0.5, &settings);
            assert!((state.weight - 0.5).abs() < 1e-5);
            assert!(!state.interrupted);
            state.advance(budget * 0.5, &settings);
            assert_eq!(state.weight, 0.0);
        }
    }

    #[test]
    fn get_up_uses_native_incremental_montage_time_and_blend_history() {
        let settings: AlsRagdollSettings = serde_json::from_str(include_str!(
            "../../../../assets/ALS/Data/Character/als_ragdoll_settings.json"
        )).unwrap();
        for (facing_up, mut settings, clock, notify) in [
            (false, settings.front, include_str!("../../../../docs/audit/bf144/d118a/front.native.json"),
                include_str!("../../../../docs/audit/bf144/d118a/front-notify.native.json")),
            (true, settings.back, include_str!("../../../../docs/audit/bf144/d118a/back.native.json"),
                include_str!("../../../../docs/audit/bf144/d118a/back-notify.native.json")),
        ] {
            let notify: serde_json::Value = serde_json::from_str(notify).unwrap();
            let mut ends = notify["notify_clock_probe"]["cases"].as_array().unwrap().iter()
                .flat_map(|case| case["events"].as_array().unwrap())
                .map(|event| event["end_trigger_time"].as_f64().unwrap() as f32);
            let marker = ends.next().unwrap();
            assert!(ends.all(|time| time == marker));
            settings.notify_end_clip_seconds = marker;

            let clock: serde_json::Value = serde_json::from_str(clock).unwrap();
            for sample in clock["montage_clock_probe"]["samples"].as_array().unwrap() {
                let name = sample["name"].as_str().unwrap();
                let mut state = AlsGetUpState::new(facing_up);
                for (index, frame) in sample["frames"].as_array().unwrap().iter().enumerate() {
                    state.advance(frame["delta_seconds"].as_f64().unwrap() as f32, &settings);
                    assert_eq!(state.weight, frame["weight"].as_f64().unwrap() as f32,
                        "Get-up blend history: facing_up={facing_up} sample={name} frame={index}");
                    if frame["playing"].as_bool().unwrap() {
                        assert_eq!(state.clip_time, frame["position"].as_f64().unwrap() as f32,
                            "Get-up clip clock: facing_up={facing_up} sample={name} frame={index}");
                    }
                }
            }
        }
    }
}
