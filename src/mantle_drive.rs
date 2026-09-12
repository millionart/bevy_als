use als_core::character::{
    AlsCharacter, AlsCollisionLayerOwner, AlsCollisionLayerOwnership, set_collision_layer_owner,
};
use als_core::mantle::{AlsMantleState, AlsMantleTargetBase, MantleKind};
use als_ext::{
    animation_context::SourceRelevantAsset,
    profile_blend::transition_alpha,
    source_gameplay_tag_input::{set_slot_input_context, slot_input_context},
};
use avian3d::prelude::{CollisionLayers, LinearVelocity, Rotation};
use bevy::prelude::*;
use bevy_animation_graph::core::animation_clip::GraphClip;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
use bevy_animation_graph::core::pose::Pose;
use bevy_animation_graph::core::skeleton::Skeleton;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance, animation_graph_player::AnimationGraphPlayer,
    edge_data::DataValue,
};
use serde::Deserialize;
use std::sync::LazyLock;

use crate::animation_events::AlsNotifyLocomotionAction;
use crate::character_assembly::AlsCharacterAvatar;
use crate::get_up::{POST_LOCOMOTION_LAYER_WEIGHT, POST_LOCOMOTION_POSE, POST_LOCOMOTION_WEIGHT};
use crate::root_motion_link::mesh_to_entity_yaw;

#[derive(Clone, Debug)]
pub(crate) struct AlsMantleMontage {
    pub id: u64,
    pub kind: MantleKind,
    pub elapsed: f32,
    pub clip_time: f32,
    pub previous_clip_time: f32,
    pub weight: f32,
    pub pose_ready: bool,
    pub interrupted: bool,
    stop: Option<(f32, f32, f32)>,
}

impl AlsMantleMontage {
    fn stop(&mut self, duration: f32, interrupted: bool) {
        self.interrupted |= interrupted;
        if self.stop.is_none_or(|(_, previous, _)| duration < previous) {
            self.stop = Some((self.elapsed, duration.max(0.0), self.weight));
            if duration <= 0.0 {
                self.weight = 0.0;
            }
        }
    }

    fn advance(&mut self, delta: f32, root_time: Option<f32>) {
        let settings = mantle_montage_settings(self.kind);
        self.previous_clip_time = self.clip_time;
        self.elapsed += delta;
        self.clip_time = (root_time
            .map(|time| (time - delta).max(0.0))
            .unwrap_or(self.clip_time)
            + delta * settings.play_rate)
            .min(settings.sequence_length);
        self.weight = if let Some((start, duration, weight)) = self.stop {
            let alpha = if duration > 0.0 {
                (self.elapsed - start) / duration
            } else {
                1.0
            };
            weight * (1.0 - transition_alpha(alpha, settings.blend_out_mode, &[]))
        } else {
            als_core::mantle::mantle_blend_in_amount(self.kind, self.elapsed)
        };
        if self.stop.is_none() && self.clip_time >= settings.sequence_length {
            self.stop(settings.blend_out_seconds, false);
        }
    }
}

#[derive(Component, Default, Debug)]
pub(crate) struct AlsMantleMontages {
    pub entries: Vec<AlsMantleMontage>,
    next_id: u64,
}

impl AlsMantleMontages {
    fn begin(&mut self, mantle: &AlsMantleState) {
        for entry in &mut self.entries {
            entry.stop(mantle.kind.action_settings().blend_in_seconds, true);
        }
        self.next_id += 1;
        self.entries.push(AlsMantleMontage {
            id: self.next_id,
            kind: mantle.kind,
            elapsed: 0.0,
            clip_time: mantle.start_offset,
            previous_clip_time: mantle.start_offset,
            weight: 0.0,
            pose_ready: false,
            interrupted: false,
            stop: None,
        });
    }
}

#[derive(Resource)]
struct MantleAssets(Vec<Handle<GraphClip>>);

#[derive(Deserialize)]
pub(crate) struct MantleMontageSettings {
    pub source_montage: String,
    pub source_sequence: String,
    pub clip: String,
    pub sequence_length: f32,
    pub play_rate: f32,
    pub blend_in_seconds: f32,
    pub blend_out_seconds: f32,
    pub blend_out_mode: als_ext::profile_blend::AlphaBlendMode,
    sequence_curves: als_ext::AlsPoseCurves,
    montage_curves: als_ext::AlsPoseCurves,
}

#[derive(Deserialize)]
struct MantleSourceSettings {
    montages: Vec<MantleMontageSettings>,
    stop_blend_out_seconds: f32,
    start_ragdoll_on_target_destruction: bool,
}

static MANTLE_SETTINGS: LazyLock<MantleSourceSettings> = LazyLock::new(|| {
    ron::from_str(als_core::mantle::MANTLE_SOURCE_DATA)
        .expect("generated mantle montage settings are valid")
});

pub(crate) fn mantle_montage_settings(kind: MantleKind) -> &'static MantleMontageSettings {
    &MANTLE_SETTINGS.montages[kind.action_settings().montage_index]
}

impl MantleAssets {
    fn clip(&self, kind: MantleKind) -> &Handle<GraphClip> {
        &self.0[kind.action_settings().montage_index]
    }
}

fn load_mantle_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(MantleAssets(
        MANTLE_SETTINGS
            .montages
            .iter()
            .map(|settings| assets.load(&settings.clip))
            .collect(),
    ));
}

fn sample_mantle_pose(
    clip: &GraphClip,
    skeleton: &Skeleton,
    settings: &MantleMontageSettings,
    time: f32,
) -> Option<(Pose, als_ext::AnimationContextState)> {
    let mut pose = als_ext::source_sequence_player::sample_pose(clip, time);
    als_ext::source_sequence_player::apply_source_root_lock(
        &mut pose,
        clip,
        skeleton,
        als_ext::source_sequence_player::SourceRootLock::RefPose,
        false,
    )?;
    let mut context = crate::get_up::sampled_curve_context(&settings.sequence_curves, time);
    context.curves.extend(
        settings
            .montage_curves
            .0
            .iter()
            .map(|(name, curve)| (name.to_string(), curve.sample(time))),
    );
    Some((pose, context))
}

fn sample_mantle_root(clip: &GraphClip, skeleton: &Skeleton, time: f32) -> Option<Transform> {
    let pose = als_ext::source_sequence_player::sample_pose(clip, time);
    let root = als_ext::root_motion_defaults::derive_default_root_bone(clip, skeleton)?.id();
    let bone = pose.get_bone(root)?;
    let reference = skeleton.default_transforms(root)?.local;
    let basis = mesh_to_entity_yaw();
    Some(Transform {
        translation: basis * bone.translation.unwrap_or(reference.translation),
        rotation: (basis * bone.rotation.unwrap_or(reference.rotation) * basis.conjugate())
            .normalize(),
        ..default()
    })
}

pub struct AlsMantleDrivePlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsMantleDriveSet;

impl Plugin for AlsMantleDrivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_mantle_assets)
            .add_systems(Update, ensure_mantle_state);
        app.configure_sets(
            PostUpdate,
            AlsMantleDriveSet
                .after(AnimationGraphSet::PrePhysics)
                .before(crate::animation_events::AlsAnimationEventSet::Refresh),
        )
        .add_systems(
            PostUpdate,
            (
                prepare_mantle_montages,
                mantle_drive_system,
                mantle_pose_system,
            )
                .chain()
                .after(AnimationGraphSet::PrePhysics)
                .in_set(AlsMantleDriveSet),
        )
        .add_systems(
            PostUpdate,
            (
                handle_mantle_early_blend_out
                    .after(crate::animation_events::AlsAnimationEventSet::Dispatch),
                publish_mantle_slot
                    .after(handle_mantle_early_blend_out)
                    .after(crate::roll_drive::AlsRollDriveSet)
                    .before(als_ext::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
            ),
        );
    }
}

fn prepare_mantle_montages(
    time: Res<Time>,
    mut commands: Commands,
    assets: Res<MantleAssets>,
    clips: Res<Assets<GraphClip>>,
    skeletons: Res<Assets<Skeleton>>,
    mut roll_blend_outs: Query<&mut crate::roll_drive::AlsRollBlendOut>,
    mut characters: Query<
        (
            Entity,
            &AlsMantleState,
            Option<&mut AlsMantleMontages>,
            Option<&als_core::state::LocomotionState>,
            Option<&als_core::roll::AlsRollState>,
            Option<&mut crate::get_up::AlsGetUpState>,
        ),
        With<AlsCharacter>,
    >,
) {
    let delta = time.delta_secs();
    for (character, mantle, mut current, locomotion, roll, mut get_up) in &mut characters {
        let starting = mantle.active
            && !mantle.root_motion_ready
            && delta > 1e-8
            && clips
                .get(assets.clip(mantle.kind))
                .is_some_and(|clip| skeletons.contains(&clip.skeleton));
        let mut added = AlsMantleMontages::default();
        let montages = current.as_deref_mut().unwrap_or(&mut added);
        if starting {
            montages.begin(mantle);
            let duration = mantle.kind.action_settings().blend_in_seconds;
            if let Some(roll) = roll.filter(|roll| roll.active) {
                if let Ok(mut blend) = roll_blend_outs.get_mut(character) {
                    blend.stop(duration);
                } else {
                    commands
                        .entity(character)
                        .insert(crate::roll_drive::AlsRollBlendOut::new(roll, duration));
                }
            }
            if let Some(get_up) = get_up.as_deref_mut() {
                get_up.stop(duration);
            }
            commands
                .entity(character)
                .insert(AlsNotifyLocomotionAction::Mantling);
        }
        let last = montages.entries.len().saturating_sub(1);
        for (index, entry) in montages.entries.iter_mut().enumerate() {
            if locomotion == Some(&als_core::state::LocomotionState::Ragdolling) {
                entry.stop(
                    crate::get_up::RAGDOLL_SETTINGS.stop_montage_blend_seconds,
                    true,
                );
            } else if !mantle.active && get_up.is_some() {
                entry.stop(0.0, true);
            } else if !mantle.active && roll.is_some_and(|roll| roll.active) {
                entry.stop(als_core::roll::ROLL_WEIGHT_IN_SECS, true);
            } else if mantle.target_invalid {
                entry.stop(MANTLE_SETTINGS.stop_blend_out_seconds, true);
            }
            let root_time =
                (index == last && mantle.active).then(|| mantle.clip_time_at(mantle.time + delta));
            entry.advance(delta, root_time);
        }
        montages
            .entries
            .retain(|entry| entry.weight > 0.0 || entry.stop.is_none());
        if current.is_none() && starting {
            commands.entity(character).insert(added);
        }
    }
}

fn handle_mantle_early_blend_out(
    mut events: MessageReader<crate::animation_events::AlsAnimationEvent>,
    mut characters: Query<
        (
            &mut AlsMantleMontages,
            &AlsMantleState,
            &als_core::state::LocomotionState,
            &als_core::state::RotationMode,
            &als_core::state::Stance,
            &als_core::input::AlsInputCommand,
            Option<&als_core::motion::AlsInputOverride>,
        ),
        With<AlsCharacter>,
    >,
) {
    for event in events.read() {
        if event.class_name != "AlsAnimNotifyState_EarlyBlendOut"
            || event.phase != als_ext::EventPhase::Active
        {
            continue;
        }
        let Ok((mut montages, mantle, locomotion, rotation, stance, command, override_command)) =
            characters.get_mut(event.character)
        else {
            continue;
        };
        let command = override_command.map_or(command, |value| &value.0);
        if crate::animation_events::early_blend_out_matches(
            &event.parameters,
            command.move_dir.length_squared() > 1e-8,
            *locomotion,
            mantle.active,
            *rotation,
            *stance,
        ) {
            for entry in &mut montages.entries {
                if entry.id == event.instance_id
                    && mantle_montage_settings(entry.kind).source_montage == event.source_asset
                {
                    entry.stop(
                        crate::animation_events::parameter_f32(
                            &event.parameters,
                            "BlendOutDuration",
                            0.25,
                        ),
                        true,
                    );
                }
            }
        }
    }
}

fn publish_mantle_slot(
    time: Res<Time>,
    mut commands: Commands,
    assets: Res<MantleAssets>,
    clips: Res<Assets<GraphClip>>,
    skeletons: Res<Assets<Skeleton>>,
    mut characters: Query<
        (
            Entity,
            &mut AlsMantleMontages,
            Option<&crate::get_up::AlsGetUpState>,
            Option<&als_core::roll::AlsRollState>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (character, mut montages, get_up, roll) in &mut characters {
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
        let mut poses = Vec::new();
        let mut contexts = Vec::new();
        for entry in &mut montages.entries {
            entry.pose_ready = false;
            let Some(clip) = clips.get(assets.clip(entry.kind)) else {
                continue;
            };
            let Some(skeleton) = skeletons.get(&clip.skeleton) else {
                continue;
            };
            let Some((pose, mut context)) = sample_mantle_pose(
                clip,
                skeleton,
                mantle_montage_settings(entry.kind),
                entry.clip_time,
            ) else {
                continue;
            };
            context.relevant_asset = Some(SourceRelevantAsset {
                duration: clip.duration,
                position: entry.clip_time,
                previous_position: entry.previous_clip_time,
                has_previous_position: true,
                delta_seconds: time.delta_secs(),
                looping: false,
                weight: 1.0,
            });
            poses.push((pose, entry.weight));
            contexts.push((context, entry.weight));
            entry.pose_ready = true;
        }
        if get_up.is_some_and(|state| state.pose_ready && state.weight > 0.0)
            || roll.is_some_and(|state| state.active)
        {
            let weight = player
                .get_global_input_data(POST_LOCOMOTION_WEIGHT)
                .and_then(|value| value.as_f32().ok())
                .unwrap_or(0.0);
            if weight > 0.0
                && let Some(pose) = player
                    .get_global_input_data(POST_LOCOMOTION_POSE)
                    .and_then(|value| value.as_pose().ok())
            {
                poses.push((pose.clone(), weight));
                if let Some(context) = slot_input_context(player_entity, POST_LOCOMOTION_POSE) {
                    contexts.push((context, weight));
                }
            }
        }
        let total: f32 = poses.iter().map(|(_, weight)| weight).sum();
        if total > 0.0 {
            for (_, weight) in &mut poses {
                *weight /= total;
            }
            for (_, weight) in &mut contexts {
                *weight /= total;
            }
            player.set_global_input_data(
                POST_LOCOMOTION_POSE,
                DataValue::Pose(als_ext::source_pose_nodes::blend_source_poses(&poses)),
            );
            player.set_global_input_data(POST_LOCOMOTION_WEIGHT, total.min(1.0));
            player.set_global_input_data(POST_LOCOMOTION_LAYER_WEIGHT, 1.0_f32);
            set_slot_input_context(
                player_entity,
                POST_LOCOMOTION_POSE,
                Some(als_ext::source_pose_nodes::blend_animation_contexts(
                    contexts,
                )),
            );
        } else {
            player.set_global_input_data(POST_LOCOMOTION_WEIGHT, 0.0_f32);
            player.set_global_input_data(POST_LOCOMOTION_LAYER_WEIGHT, 0.0_f32);
            set_slot_input_context(player_entity, POST_LOCOMOTION_POSE, None);
        }
        if montages.entries.is_empty() {
            commands.entity(character).remove::<AlsMantleMontages>();
        }
    }
}

fn ensure_mantle_state(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<als_core::character::AlsCharacter>,
            Without<AlsMantleState>,
        ),
    >,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsMantleState::default());
    }
}

fn mantle_pose_system(
    mut commands: Commands,
    target_transforms: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    mut q: Query<
        (
            Entity,
            &mut AlsMantleState,
            &mut LinearVelocity,
            Option<&AlsNotifyLocomotionAction>,
            Option<&CollisionLayers>,
            &mut AlsCollisionLayerOwnership,
        ),
        With<AlsCharacter>,
    >,
) {
    for (entity, mut m, mut vel, action, layers, mut ownership) in &mut q {
        if m.root_motion_ready
            && action.is_some_and(|action| *action != AlsNotifyLocomotionAction::Mantling)
        {
            m.active = false;
        }
        let target_base = mantle_target_base(&m, &target_transforms);
        let target_valid = !m.active || m.refresh_target_base(target_base);
        if !target_valid {
            *vel = LinearVelocity(Vec3::ZERO);
        }
        set_collision_layer_owner(
            &mut commands,
            entity,
            &mut ownership,
            layers.copied(),
            AlsCollisionLayerOwner::Mantle,
            m.active,
        );
    }
}

#[allow(clippy::type_complexity)]
fn mantle_drive_system(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &mut AlsMantleState,
            &mut LinearVelocity,
            &mut Transform,
            Option<&mut Rotation>,
            Option<&CollisionLayers>,
            &mut AlsCollisionLayerOwnership,
        ),
        With<AlsCharacter>,
    >,
    target_transforms: Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
    assets: Res<MantleAssets>,
    clips: Res<Assets<GraphClip>>,
    skeletons: Res<Assets<Skeleton>>,
) {
    let dt = time.delta_secs();
    for (entity, mut m, mut vel, mut transform, rotation, layers, mut ownership) in &mut q {
        let target_base = mantle_target_base(&m, &target_transforms);
        let target_valid = !m.active || m.refresh_target_base(target_base);
        if !target_valid {
            *vel = LinearVelocity(Vec3::ZERO);
        }
        if !target_valid || !m.active || dt <= 1e-8 {
            set_collision_layer_owner(
                &mut commands,
                entity,
                &mut ownership,
                layers.copied(),
                AlsCollisionLayerOwner::Mantle,
                false,
            );
            continue;
        }

        let Some(clip) = clips.get(assets.clip(m.kind)) else {
            continue;
        };
        let Some(skeleton) = skeletons.get(&clip.skeleton) else {
            continue;
        };
        if !m.root_motion_ready {
            let (Some(start_root), Some(end_root)) = (
                sample_mantle_root(clip, skeleton, m.start_offset),
                sample_mantle_root(clip, skeleton, clip.duration),
            ) else {
                continue;
            };
            m.prepare_root_motion(start_root, end_root);
        }
        let previous_time = m.time;
        let current_time = (previous_time + dt).min(m.action_end());
        let Some(root) = sample_mantle_root(clip, skeleton, m.clip_time_at(current_time)) else {
            continue;
        };
        let target_actor_transform = m.target_actor_transform_at(current_time, root);
        *vel = LinearVelocity((target_actor_transform.translation - transform.translation) / dt);
        transform.rotation = target_actor_transform.rotation;
        if let Some(mut rotation) = rotation {
            rotation.0 = target_actor_transform.rotation;
        }
        m.time = current_time;
        let finishing = current_time >= m.action_end();
        if finishing {
            m.active = false;
        }
        set_collision_layer_owner(
            &mut commands,
            entity,
            &mut ownership,
            layers.copied(),
            AlsCollisionLayerOwner::Mantle,
            m.active,
        );
    }
}

fn mantle_target_base(
    state: &AlsMantleState,
    transforms: &Query<&GlobalTransform, Without<als_core::character::AlsCharacter>>,
) -> Option<AlsMantleTargetBase> {
    let base = state.target_base?;
    let transform = transforms.get(base.entity).ok()?;
    Some(AlsMantleTargetBase {
        entity: base.entity,
        translation: transform.translation(),
        rotation: transform.rotation(),
        scale: transform.scale(),
        relative_location: base.relative_location,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
    use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;

    #[test]
    fn mantle_action_end_releases_motion_before_the_montage_ends() {
        let mut world = World::new();
        let mut mantle = AlsMantleState::default();
        mantle.begin(MantleKind::Low, Vec3::ZERO, Vec3::Y, 1.0, 0.0, 0.0);
        mantle.time = 1.0;
        mantle.root_motion_ready = true;
        let actor = world
            .spawn((
                AlsCharacter,
                mantle,
                LinearVelocity::ZERO,
                crate::animation_events::AlsNotifyLocomotionAction::None,
            ))
            .id();
        world.run_system_once(mantle_pose_system).unwrap();
        assert!(!world.get::<AlsMantleState>(actor).unwrap().active);
    }

    #[test]
    fn mantle_montage_keeps_the_source_tail_and_each_interrupted_instance() {
        let mut mantle = AlsMantleState::default();
        mantle.begin(MantleKind::Low, Vec3::ZERO, Vec3::Y, 0.75, 0.0, 0.0);
        let mut montages = AlsMantleMontages::default();
        montages.begin(&mantle);
        let entry = &mut montages.entries[0];
        entry.advance(0.05, Some(mantle.clip_time_at(0.05)));
        assert!((entry.weight - 0.15625).abs() < 1e-6);
        entry.advance(0.75, Some(1.0));
        assert_eq!(entry.weight, 1.0);
        entry.advance(0.5, None);
        assert_eq!(entry.clip_time, 1.5);
        assert_eq!(entry.weight, 1.0);
        assert!(!entry.interrupted);
        entry.advance(0.1, None);
        assert!((entry.weight - 0.5).abs() < 1e-5);
        entry.stop(0.3, true);
        entry.advance(0.1, None);
        assert!(entry.weight < 1e-5);
        montages.entries.clear();
        montages.begin(&mantle);
        montages.entries[0].advance(0.5, None);
        let old_id = montages.entries[0].id;
        montages.begin(&mantle);
        assert_eq!(montages.entries.len(), 2);
        assert_ne!(montages.entries[1].id, old_id);
        assert!(montages.entries[0].interrupted);
        assert!(!montages.entries[1].interrupted);
        montages.entries[0].advance(0.1, None);
        assert!((montages.entries[0].weight - 0.5).abs() < 1e-5);
        assert!(montages.entries[0].clip_time > 0.75);
        montages.entries[0].stop(0.05, true);
        montages.entries[0].advance(0.025, None);
        let weight = montages.entries[0].weight;
        montages.entries[0].stop(0.05, true);
        montages.entries[0].advance(0.025, None);
        assert!(weight > 0.0 && montages.entries[0].weight < 1e-5);
        mantle.begin(MantleKind::High, Vec3::ZERO, Vec3::Y, 1.5, 0.0, 0.0);
        montages.entries.clear();
        montages.begin(&mantle);
        let root_time = mantle.clip_time_at(0.1);
        montages.entries[0].advance(0.1, Some(root_time));
        assert!((montages.entries[0].clip_time - (root_time - 0.1 + 0.1 * 1.2)).abs() < 1e-6);
        let previous = montages.entries[0].clip_time;
        montages.entries[0].advance(0.1, None);
        assert!((montages.entries[0].clip_time - previous - 0.12).abs() < 1e-6);
    }

    #[test]
    fn mantle_absolute_root_drives_actual_clips_to_source_endpoints() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::gltf::GltfPlugin::default(),
            TransformPlugin,
            bevy::gizmos::GizmoPlugin,
            bevy_animation_graph::AnimationGraphPlugin::default(),
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<bevy::animation::AnimationClip>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_systems(Startup, load_mantle_assets);
        app.finish();
        app.cleanup();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut previous = 0;
        loop {
            app.update();
            let assets = app.world().resource::<MantleAssets>();
            let clips = app.world().resource::<Assets<GraphClip>>();
            let skeletons = app.world().resource::<Assets<Skeleton>>();
            let loaded = assets
                .0
                .iter()
                .filter(|handle| {
                    clips
                        .get(*handle)
                        .and_then(|clip| skeletons.get(&clip.skeleton))
                        .is_some()
                })
                .count();
            if loaded != previous {
                eprintln!("mantle montage clips loaded {loaded}/6");
                previous = loaded;
            }
            if loaded == 6 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "mantle assets did not load within 30 seconds: {loaded}"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        for kind in [
            MantleKind::High,
            MantleKind::InAir,
            MantleKind::Low,
            MantleKind::LowLeft,
            MantleKind::LowRight,
            MantleKind::LowBothHands,
            MantleKind::LowBox,
        ] {
            let start = Vec3::new(-1.0, 0.9, 0.3);
            let target = Vec3::new(2.0, 2.0, -2.0);
            let mut mantle = AlsMantleState::default();
            mantle.begin(
                kind,
                start,
                target,
                if kind.is_high() { 1.5 } else { 0.75 },
                -0.6,
                0.8,
            );
            let duration = mantle.duration;
            {
                let assets = app.world().resource::<MantleAssets>();
                let clips = app.world().resource::<Assets<GraphClip>>();
                let clip = clips.get(assets.clip(kind)).unwrap();
                let skeletons = app.world().resource::<Assets<Skeleton>>();
                let skeleton = skeletons.get(&clip.skeleton).unwrap();
                let settings = mantle_montage_settings(kind);
                assert!((settings.sequence_length - clip.duration).abs() < 1e-5);
                assert_eq!(settings.play_rate, kind.action_settings().rate_scale);
                assert_eq!(
                    settings.blend_in_seconds,
                    kind.action_settings().blend_in_seconds
                );
                assert_eq!(settings.blend_out_seconds, 0.2);
                assert_eq!(
                    settings.blend_out_mode,
                    if kind.is_high() {
                        als_ext::profile_blend::AlphaBlendMode::Cubic
                    } else {
                        als_ext::profile_blend::AlphaBlendMode::HermiteCubic
                    }
                );
                let (pose, context) =
                    sample_mantle_pose(clip, skeleton, settings, clip.duration).unwrap();
                let root_id =
                    als_ext::root_motion_defaults::derive_default_root_bone(clip, skeleton)
                        .unwrap()
                        .id();
                let locked_root = pose.get_bone(root_id).unwrap();
                let reference = skeleton.default_transforms(root_id).unwrap().local;
                assert_eq!(locked_root.translation, Some(reference.translation));
                assert_eq!(locked_root.rotation, Some(reference.rotation));
                assert_eq!(pose.paths.len(), 79);
                assert!(context.curves_are_authoritative && context.curves.len() > 5);
                if kind == MantleKind::Low {
                    let (_, low) = sample_mantle_pose(clip, skeleton, settings, 0.8).unwrap();
                    let right_settings = mantle_montage_settings(MantleKind::LowRight);
                    let (_, right) =
                        sample_mantle_pose(clip, skeleton, right_settings, 0.8).unwrap();
                    assert_eq!(settings.clip, right_settings.clip);
                    assert_eq!(low.curves["LayerArmRight"], -1.0);
                    assert!(!right.curves.contains_key("LayerArmRight"));
                    assert_eq!(low.curves["PoseStanding"], 1.0);
                    let mut controlled: MantleSourceSettings =
                        ron::from_str(als_core::mantle::MANTLE_SOURCE_DATA).unwrap();
                    let controlled = &mut controlled.montages[kind.action_settings().montage_index];
                    controlled.sequence_curves.0.insert(
                        "LayerArmRight".into(),
                        als_ext::als_curves::AlsFloatCurve {
                            times: vec![0.0],
                            values: vec![0.5],
                            interps: vec![als_ext::als_curves::AlsKeyInterp::Linear],
                            ..default()
                        },
                    );
                    let (_, overridden) =
                        sample_mantle_pose(clip, skeleton, controlled, 0.8).unwrap();
                    assert_eq!(overridden.curves["LayerArmRight"], -1.0);
                    assert_eq!(overridden.curves["PoseStanding"], 1.0);
                }
                let root = sample_mantle_root(clip, skeleton, clip.duration).unwrap();
                assert!(
                    root.translation.distance(Vec3::new(
                        0.0,
                        if kind.is_high() { 2.0 } else { 1.0 },
                        -0.65
                    )) < 1e-5
                );
                let initial = sample_mantle_root(clip, skeleton, mantle.start_offset).unwrap();
                assert!(mantle.start_offset > 0.0);
                let mut prepared = mantle;
                prepared.prepare_root_motion(initial, root);
                assert!(
                    prepared
                        .target_actor_transform_at(0.0, initial)
                        .translation
                        .distance(start)
                        < 1e-5
                );
                let raw = Transform::from_translation(
                    mesh_to_entity_yaw().conjugate() * initial.translation,
                );
                let source_inverse_rotation = mesh_to_entity_yaw() * raw.rotation.conjugate();
                let source_warp_rotation = Quat::from_rotation_y(-0.6) * source_inverse_rotation;
                let source_warp_location = start - source_warp_rotation * raw.translation;
                assert!(
                    prepared
                        .warp_start
                        .translation
                        .distance(source_warp_location)
                        < 1e-5
                );
            }
            let entity = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    mantle,
                    LinearVelocity::ZERO,
                    Transform::from_translation(start).with_rotation(Quat::from_rotation_y(-0.6)),
                    Rotation(Quat::from_rotation_y(-0.6)),
                    AlsCollisionLayerOwnership::default(),
                ))
                .id();
            let clip_handle = app.world().resource::<MantleAssets>().clip(kind).clone();
            let skeleton = app
                .world()
                .resource::<Assets<GraphClip>>()
                .get(&clip_handle)
                .unwrap()
                .skeleton
                .clone();
            let avatar = app
                .world_mut()
                .spawn((
                    AlsCharacterAvatar { character: entity },
                    ChildOf(entity),
                    Transform::IDENTITY,
                    AnimatedSceneHandle::default(),
                ))
                .id();
            let player_entity = app
                .world_mut()
                .spawn((
                    AnimationGraphPlayer::new(skeleton),
                    Transform::IDENTITY,
                    ChildOf(avatar),
                ))
                .id();
            app.world_mut().trigger(WorldInstanceReady {
                entity: avatar,
                instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
            });
            app.world_mut().flush();
            let delta = 1.0 / 60.0;
            for _ in 0..=(duration / delta).ceil() as usize {
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_secs_f32(delta));
                app.world_mut()
                    .run_system_once(prepare_mantle_montages)
                    .unwrap();
                app.world_mut()
                    .run_system_once(mantle_drive_system)
                    .unwrap();
                app.world_mut()
                    .run_system_once(publish_mantle_slot)
                    .unwrap();
                let montage = &app
                    .world()
                    .get::<AlsMantleMontages>(entity)
                    .unwrap()
                    .entries[0];
                let player = app
                    .world()
                    .get::<AnimationGraphPlayer>(player_entity)
                    .unwrap();
                assert!(montage.pose_ready);
                let weight = player
                    .get_global_input_data(POST_LOCOMOTION_WEIGHT)
                    .unwrap()
                    .as_f32()
                    .unwrap();
                assert!((weight - montage.weight).abs() < 1e-6);
                assert_eq!(
                    player
                        .get_global_input_data(POST_LOCOMOTION_LAYER_WEIGHT)
                        .unwrap()
                        .as_f32()
                        .unwrap(),
                    1.0
                );
                let context = slot_input_context(player_entity, POST_LOCOMOTION_POSE).unwrap();
                assert!(context.curves_are_authoritative);
                assert_eq!(context.relevant_asset.unwrap().position, montage.clip_time);
                let clip = app
                    .world()
                    .resource::<Assets<GraphClip>>()
                    .get(&clip_handle)
                    .unwrap();
                let skeleton = app
                    .world()
                    .resource::<Assets<Skeleton>>()
                    .get(&clip.skeleton)
                    .unwrap();
                let (expected, expected_context) = sample_mantle_pose(
                    clip,
                    skeleton,
                    mantle_montage_settings(kind),
                    montage.clip_time,
                )
                .unwrap();
                let pose = player
                    .get_global_input_data(POST_LOCOMOTION_POSE)
                    .unwrap()
                    .as_pose()
                    .unwrap();
                assert_eq!(pose.paths.len(), 79);
                for id in expected.paths.keys() {
                    let actual = pose.get_bone(*id).unwrap();
                    let expected = expected.get_bone(*id).unwrap();
                    assert_eq!(actual.translation, expected.translation);
                    assert!(
                        actual
                            .rotation
                            .unwrap()
                            .dot(expected.rotation.unwrap())
                            .abs()
                            > 1.0 - 1e-5
                    );
                }
                assert_eq!(context.curves, expected_context.curves);
                let velocity = app.world().get::<LinearVelocity>(entity).unwrap().0;
                app.world_mut()
                    .get_mut::<Transform>(entity)
                    .unwrap()
                    .translation += velocity * delta;
                if !app.world().get::<AlsMantleState>(entity).unwrap().active {
                    break;
                }
            }
            let state = app.world().get::<AlsMantleState>(entity).unwrap();
            assert!(!state.active && state.root_motion_ready, "{kind:?}");
            let actor = app.world().get::<Transform>(entity).unwrap();
            assert!(
                actor.translation.distance(target) < 1e-4,
                "{kind:?}: {:?}",
                actor.translation
            );
            assert!(actor.rotation.angle_between(Quat::from_rotation_y(0.8)) < 1e-3);
            assert_eq!(
                app.world().get::<Rotation>(entity).unwrap().0,
                actor.rotation
            );
            if kind == MantleKind::Low {
                app.world_mut()
                    .get_mut::<AlsMantleState>(entity)
                    .unwrap()
                    .begin(MantleKind::LowLeft, target, target, 0.75, 0.8, 0.8);
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_secs_f32(0.1));
                app.world_mut()
                    .run_system_once(prepare_mantle_montages)
                    .unwrap();
                app.world_mut()
                    .run_system_once(mantle_drive_system)
                    .unwrap();
                app.world_mut()
                    .run_system_once(publish_mantle_slot)
                    .unwrap();
                let clips = app.world().resource::<Assets<GraphClip>>();
                let skeletons = app.world().resource::<Assets<Skeleton>>();
                let new_time = app
                    .world()
                    .get::<AlsMantleState>(entity)
                    .unwrap()
                    .clip_time_at(0.1);
                let samples = [(MantleKind::Low, 1.5), (MantleKind::LowLeft, new_time)].map(
                    |(kind, time)| {
                        let clip = clips
                            .get(app.world().resource::<MantleAssets>().clip(kind))
                            .unwrap();
                        sample_mantle_pose(
                            clip,
                            skeletons.get(&clip.skeleton).unwrap(),
                            mantle_montage_settings(kind),
                            time,
                        )
                        .unwrap()
                    },
                );
                let player = app
                    .world()
                    .get::<AnimationGraphPlayer>(player_entity)
                    .unwrap();
                assert!(
                    (player
                        .get_global_input_data(POST_LOCOMOTION_WEIGHT)
                        .unwrap()
                        .as_f32()
                        .unwrap()
                        - 1.0)
                        .abs()
                        < 1e-5
                );
                let output = player
                    .get_global_input_data(POST_LOCOMOTION_POSE)
                    .unwrap()
                    .as_pose()
                    .unwrap();
                let skeleton = skeletons
                    .get(&clips.get(&clip_handle).unwrap().skeleton)
                    .unwrap();
                let pelvis = output
                    .paths
                    .keys()
                    .find(|id| {
                        skeleton
                            .id_to_path(**id)
                            .unwrap()
                            .to_string()
                            .ends_with("/pelvis")
                    })
                    .copied()
                    .unwrap();
                let first = samples[0].0.get_bone(pelvis).unwrap().translation.unwrap();
                let second = samples[1].0.get_bone(pelvis).unwrap().translation.unwrap();
                assert!(first.distance(second) > 0.01);
                assert!(
                    output
                        .get_bone(pelvis)
                        .unwrap()
                        .translation
                        .unwrap()
                        .distance(first.lerp(second, 0.5))
                        < 1e-5
                );
                let context = slot_input_context(player_entity, POST_LOCOMOTION_POSE).unwrap();
                for (name, value) in &context.curves {
                    let expected = (samples[0].1.curves.get(name).copied().unwrap_or(0.0)
                        + samples[1].1.curves.get(name).copied().unwrap_or(0.0))
                        * 0.5;
                    assert!((value - expected).abs() < 1e-5);
                }
                app.world_mut()
                    .get_mut::<AlsMantleState>(entity)
                    .unwrap()
                    .active = false;
                for entry in &mut app
                    .world_mut()
                    .get_mut::<AlsMantleMontages>(entity)
                    .unwrap()
                    .entries
                {
                    entry.stop(0.2, true);
                }
            }
            for _ in 0..=13 {
                app.world_mut()
                    .run_system_once(prepare_mantle_montages)
                    .unwrap();
                app.world_mut()
                    .run_system_once(publish_mantle_slot)
                    .unwrap();
            }
            assert!(app.world().get::<AlsMantleMontages>(entity).is_none());
            assert_eq!(
                app.world()
                    .get::<AnimationGraphPlayer>(player_entity)
                    .unwrap()
                    .get_global_input_data(POST_LOCOMOTION_WEIGHT)
                    .unwrap()
                    .as_f32()
                    .unwrap(),
                0.0
            );
            assert!(slot_input_context(player_entity, POST_LOCOMOTION_POSE).is_none());
            app.world_mut().despawn(entity);
        }
        assert_eq!(MANTLE_SETTINGS.stop_blend_out_seconds, 0.3);
        assert!(MANTLE_SETTINGS.start_ragdoll_on_target_destruction);
        for interrupt in 0..4 {
            let mut mantle = AlsMantleState::default();
            mantle.begin(MantleKind::Low, Vec3::ZERO, Vec3::Y, 0.75, 0.0, 0.0);
            let mut montages = AlsMantleMontages::default();
            montages.begin(&mantle);
            montages.entries[0].advance(0.3, None);
            mantle.active = false;
            mantle.target_invalid = interrupt == 3;
            let actor = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    mantle,
                    montages,
                    if interrupt == 2 {
                        als_core::state::LocomotionState::Ragdolling
                    } else {
                        als_core::state::LocomotionState::Grounded
                    },
                ))
                .id();
            if interrupt == 0 {
                app.world_mut()
                    .entity_mut(actor)
                    .insert(als_core::roll::AlsRollState {
                        active: true,
                        time: 0.3,
                        ..default()
                    });
            }
            if interrupt == 1 {
                app.world_mut()
                    .entity_mut(actor)
                    .insert(crate::get_up::AlsGetUpState::new(false));
            }
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(0.05));
            app.world_mut()
                .run_system_once(prepare_mantle_montages)
                .unwrap();
            let montages = app.world().get::<AlsMantleMontages>(actor).unwrap();
            if interrupt == 1 {
                assert!(montages.entries.is_empty());
            } else {
                let duration = match interrupt {
                    0 => 0.1,
                    2 => 0.2,
                    _ => 0.3,
                };
                let alpha: f32 = 0.05 / duration;
                let expected = 1.0 - alpha * alpha * (3.0 - 2.0 * alpha);
                assert!(
                    (montages.entries[0].weight - expected).abs() < 1e-5,
                    "interruption {interrupt}"
                );
                assert!(montages.entries[0].interrupted && montages.entries[0].clip_time > 0.55);
            }
            app.world_mut().despawn(actor);
        }
        let mut mantle = AlsMantleState::default();
        mantle.begin(MantleKind::Low, Vec3::ZERO, Vec3::Y, 0.75, 0.0, 0.0);
        let actor = app
            .world_mut()
            .spawn((
                AlsCharacter,
                mantle,
                crate::get_up::AlsGetUpState::new(false),
                als_core::roll::AlsRollState {
                    active: true,
                    time: 0.3,
                    ..default()
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.1));
        app.world_mut()
            .run_system_once(prepare_mantle_montages)
            .unwrap();
        let montage = &app.world().get::<AlsMantleMontages>(actor).unwrap().entries[0];
        assert!((montage.weight - 0.5).abs() < 1e-5 && !montage.interrupted);
        assert!(
            app.world()
                .get::<crate::roll_drive::AlsRollBlendOut>(actor)
                .is_some()
        );
        let mut get_up = app
            .world_mut()
            .get_mut::<crate::get_up::AlsGetUpState>(actor)
            .unwrap();
        get_up.advance(0.1);
        assert!((get_up.weight - 0.5).abs() < 1e-5);
    }
}
