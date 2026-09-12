use std::sync::LazyLock;

use als_core::{
    character::AlsCharacter, motion::AlsMovementInputBlocked, roll::AlsRollState,
};
use als_ext::{
    AlsAnimCurves,
    animation_context::{AnimationContextState, SourceRelevantAsset},
    profile_blend::{AlphaBlendMode, transition_alpha},
    source_gameplay_tag_input::set_slot_input_context,
};
use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_clip::GraphClip,
    animation_graph_player::AnimationGraphPlayer, edge_data::DataValue, plugin::AnimationGraphSet,
};
use serde::Deserialize;

use crate::{
    animation_events::{AlsAnimationEventSet, AlsNotifyLocomotionAction},
    character_assembly::AlsCharacterAvatar,
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
        "../assets/character/als_ragdoll_settings.json"
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
    pub facing_up: bool,
    pub elapsed: f32,
    pub clip_time: f32,
    pub weight: f32,
    pub pose_ready: bool,
    pub(crate) interrupted: bool,
    stop: Option<(f32, f32, f32)>,
}

impl AlsGetUpState {
    pub(crate) fn new(facing_up: bool) -> Self {
        Self {
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
        let configured_duration = self.stop.map(|(_, duration, _)| duration).or_else(|| {
            let settings = get_up_settings(self.facing_up);
            (self.elapsed
                >= settings.sequence_length / settings.play_rate - settings.blend_out_seconds)
                .then_some(settings.blend_out_seconds)
        });
        if configured_duration.is_none_or(|previous| duration < previous) {
            self.stop = Some((self.elapsed, duration.max(0.0), self.weight));
        }
    }

    pub(crate) fn advance(&mut self, delta: f32) {
        let settings = get_up_settings(self.facing_up);
        self.elapsed += delta;
        self.clip_time = (self.elapsed * settings.play_rate).min(settings.sequence_length);
        let duration = settings.sequence_length / settings.play_rate;
        let (start, blend_duration, start_weight) = self.stop.unwrap_or((
            duration - settings.blend_out_seconds,
            settings.blend_out_seconds,
            1.0,
        ));
        let alpha = if blend_duration > 0.0 {
            ((self.elapsed - start) / blend_duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.weight = start_weight * (1.0 - transition_alpha(alpha, AlphaBlendMode::Cubic, &[]));
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
    requests: Query<(Entity, &AlsGetUpRequest)>,
    mut rolls: Query<&mut AlsRollState>,
    assets: Res<GetUpAssets>,
    clips: Res<Assets<GraphClip>>,
) {
    for (character, request) in &requests {
        commands.entity(character).remove::<AlsGetUpRequest>();
        if !clips.contains(&assets.clips[usize::from(request.0)]) {
            error!("[GET-UP] clip was not loaded when recovery requested character={character:?}");
            continue;
        }
        if let Ok(mut roll) = rolls.get_mut(character) {
            roll.active = false;
        }
        commands
            .entity(character)
            .remove::<crate::roll_drive::AlsRollBlendOut>();
        commands.entity(character).insert((
            AlsGetUpState::new(request.0),
            AlsMovementInputBlocked,
            AlsNotifyLocomotionAction::GettingUp,
        ));
    }
}

pub(crate) fn sampled_curve_context(
    curves: &als_ext::AlsPoseCurves,
    time: f32,
) -> AnimationContextState {
    AnimationContextState {
        curves: curves
            .0
            .iter()
            .map(|(name, curve)| (name.to_string(), curve.sample(time)))
            .collect(),
        relevant_asset: None,
        curves_are_authoritative: true,
        ..default()
    }
}

fn update_get_up_pose(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GetUpAssets>,
    clips: Res<Assets<GraphClip>>,
    curves: Res<Assets<AlsAnimCurves>>,
    mut characters: Query<
        (
            Entity,
            Option<&AlsNotifyLocomotionAction>,
            &mut AlsGetUpState,
            Option<&AlsRollState>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (character, action, mut state, roll) in &mut characters {
        let previous_clip_time = state.clip_time;
        if action == Some(&AlsNotifyLocomotionAction::Ragdolling) {
            state.stop(RAGDOLL_SETTINGS.stop_montage_blend_seconds);
        } else if roll.is_some_and(|roll| roll.active) {
            state.stop(als_core::roll::ROLL_WEIGHT_IN_SECS);
        }
        state.advance(time.delta_secs());
        let Some(instance) = avatars
            .iter()
            .find_map(|(avatar, instance)| (avatar.character == character).then_some(instance))
        else {
            continue;
        };
        let player_entity = instance.player_entity();
        let Ok(mut player) = players.get_mut(player_entity) else {
            continue;
        };
        if state.weight <= 0.0 {
            player.set_global_input_data(POST_LOCOMOTION_WEIGHT, 0.0_f32);
            player.set_global_input_data(POST_LOCOMOTION_LAYER_WEIGHT, 0.0_f32);
            set_slot_input_context(player_entity, POST_LOCOMOTION_POSE, None);
            commands.entity(character).remove::<AlsGetUpState>();
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
        let mut pose = als_ext::source_sequence_player::sample_pose(clip, state.clip_time);
        let mut context = sampled_curve_context(curves, state.clip_time);
        context.relevant_asset = Some(SourceRelevantAsset {
            duration: clip.duration,
            position: state.clip_time,
            previous_position: previous_clip_time,
            has_previous_position: true,
            delta_seconds: time.delta_secs(),
            looping: false,
            weight: 1.0,
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
                pose = als_ext::slot_node::blend_slot_pose(pose, Some(roll_pose), Some(alpha));
                if let Some(roll_context) = als_ext::source_gameplay_tag_input::slot_input_context(
                    player_entity,
                    "roll.pose",
                ) {
                    context = als_ext::source_pose_nodes::blend_animation_contexts([
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
        set_slot_input_context(player_entity, POST_LOCOMOTION_POSE, Some(context));
        state.pose_ready = true;
    }
}

fn release_get_up_input(
    mut commands: Commands,
    characters: Query<(Entity, &AlsNotifyLocomotionAction), With<AlsMovementInputBlocked>>,
) {
    for (entity, action) in &characters {
        if *action != AlsNotifyLocomotionAction::GettingUp {
            commands.entity(entity).remove::<AlsMovementInputBlocked>();
        }
    }
}

pub struct AlsGetUpPlugin;

impl Plugin for AlsGetUpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_get_up_assets)
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
    fn both_get_up_assets_load_sample_and_start_the_actual_request_system() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::{
            animation_clip::loader::GraphClipLoader,
            skeleton::{Skeleton, loader::SkeletonLoader},
        };
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::gltf::GltfPlugin::default(),
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<bevy::animation::AnimationClip>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<GraphClip>()
        .init_asset_loader::<GraphClipLoader>()
        .init_asset::<Skeleton>()
        .init_asset_loader::<SkeletonLoader>()
        .add_plugins(als_ext::AlsExtPlugin)
        .add_systems(Startup, load_get_up_assets);
        app.finish();
        app.cleanup();
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
            let pose = als_ext::source_sequence_player::sample_pose(clip, 0.25);
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
            let entity = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    AlsGetUpRequest(facing_up),
                    roll,
                    crate::roll_drive::AlsRollBlendOut::new(&roll, crate::get_up::RAGDOLL_SETTINGS.stop_montage_blend_seconds),
                ))
                .id();
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
    fn source_notify_releases_input_before_montage_blend_finishes() {
        for facing_up in [false, true] {
            let settings = get_up_settings(facing_up);
            let mut state = AlsGetUpState::new(facing_up);
            state.advance(settings.notify_end_clip_seconds / settings.play_rate);
            assert!(state.weight > 0.0 && state.weight < 1.0);
            assert!(state.clip_time < settings.sequence_length);
            state.advance(settings.sequence_length / settings.play_rate - state.elapsed);
            assert_eq!(state.weight, 0.0);
        }
    }

    #[test]
    fn interruption_keeps_source_blend_duration_and_current_weight() {
        let mut state = AlsGetUpState::new(true);
        state.advance(0.3);
        state.stop(RAGDOLL_SETTINGS.stop_montage_blend_seconds);
        state.advance(0.1);
        assert!((state.weight - 0.5).abs() < 1e-5);
        state.advance(0.1);
        assert_eq!(state.weight, 0.0);
    }

    #[test]
    fn late_interruption_restarts_a_shorter_configured_blend_once() {
        let mut state = AlsGetUpState::new(true);
        state.advance(1.18);
        let before = state.weight;
        state.stop(0.2);
        state.advance(0.1);
        assert!((state.weight - before * 0.5).abs() < 1e-5);
        state.stop(0.2);
        state.advance(0.1);
        assert_eq!(state.weight, 0.0);
    }
}
