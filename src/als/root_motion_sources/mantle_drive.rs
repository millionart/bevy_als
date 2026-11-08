use crate::als::character::AlsCharacter;
#[cfg(test)]
use crate::als::character::AlsCollisionLayerOwnership;
use crate::als::mantle::{AlsMantleState, AlsMantleTargetBase, MantleKind};
use crate::als::state::LocomotionState;
use crate::als::transition::{Dimension, StateTransitionEvent, StateValue};
use crate::extensions::{
    animation_context::SourceRelevantAsset,
    profile_blend::transition_alpha,
};
use avian3d::prelude::{
    CustomPositionIntegration, CustomVelocityIntegration, LinearVelocity, LockedAxes,
    PhysicsSystems, Position, Rotation,
};
#[cfg(test)]
use avian3d::prelude::CollisionLayers;
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
            crate::als::mantle::mantle_blend_in_amount(self.kind, self.elapsed)
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
    published_player: Option<Entity>,
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
pub(crate) struct MantleAssets(Vec<Handle<GraphClip>>);

#[derive(Component)]
pub(crate) struct AlsMantlePhysicsFrame {
    custom_position: bool,
    custom_velocity: bool,
    locked_axes: Option<LockedAxes>,
}

#[derive(Deserialize)]
pub(crate) struct MantleMontageSettings {
    pub source_montage: String,
    pub source_sequence: String,
    pub clip: String,
    pub sequence_length: f32,
    pub play_rate: f32,
    pub blend_in_seconds: f32,
    pub blend_out_seconds: f32,
    pub blend_out_mode: crate::extensions::profile_blend::AlphaBlendMode,
    sequence_curves: crate::extensions::AlsPoseCurves,
    montage_curves: crate::extensions::AlsPoseCurves,
}

#[derive(Deserialize)]
struct MantleSourceSettings {
    montages: Vec<MantleMontageSettings>,
    stop_blend_out_seconds: f32,
    start_ragdoll_on_target_destruction: bool,
}

static MANTLE_SETTINGS: LazyLock<MantleSourceSettings> = LazyLock::new(|| {
    ron::from_str(crate::als::mantle::MANTLE_SOURCE_DATA)
        .expect("generated mantle montage settings are valid")
});

pub(crate) fn mantle_montage_settings(kind: MantleKind) -> &'static MantleMontageSettings {
    &MANTLE_SETTINGS.montages[kind.action_settings().montage_index]
}

impl MantleAssets {
    pub(crate) fn clip(&self, kind: MantleKind) -> &Handle<GraphClip> {
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
) -> Option<(Pose, crate::extensions::AnimationContextState)> {
    let mut pose = crate::extensions::source_sequence_player::sample_pose(clip, time);
    crate::extensions::source_sequence_player::apply_source_root_lock(
        &mut pose,
        clip,
        skeleton,
        crate::extensions::source_sequence_player::SourceRootLock::RefPose,
        false,
    )?;
    let mut context = crate::get_up::sampled_curve_context(&settings.sequence_curves, time);
    context.curves.extend(
        settings
            .montage_curves
            .0
            .iter()
            .map(|(name, curve)| (name.clone(), curve.sample(time))),
    );
    Some((pose, context))
}

pub(crate) fn sample_mantle_root(clip: &GraphClip, skeleton: &Skeleton, time: f32) -> Option<Transform> {
    let pose = crate::extensions::source_sequence_player::sample_pose(clip, time);
    let root = crate::extensions::root_motion_defaults::derive_default_root_bone(clip, skeleton)?.id();
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
        app.add_message::<StateTransitionEvent>()
            .add_systems(Startup, load_mantle_assets)
            .add_systems(Update, ensure_mantle_state);
        app.configure_sets(
            PostUpdate,
            AlsMantleDriveSet
                .after(AnimationGraphSet::PrePhysics)
                .before(crate::source_physics_timing::SourcePhysicsTimingSet::Clamp)
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
                update_mantle_movement_base
                    .after(PhysicsSystems::Last)
                    .before(crate::als::motion::AlsActualVelocitySet)
                    .before(release_mantle_physics),
                release_mantle_physics
                    .after(PhysicsSystems::Last)
                    .after(crate::als::motion::AlsRotationResponseSet)
                    .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
                handle_mantle_early_blend_out
                    .after(crate::animation_events::AlsAnimationEventSet::Dispatch),
                publish_mantle_slot
                    .after(handle_mantle_early_blend_out)
                    .after(crate::roll_drive::AlsRollDriveSet)
                    .after(crate::source_physics_timing::SourcePhysicsTimingSet::Restore)
                    .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
            ),
        );
    }
}

fn prepare_mantle_montages(
    time: Res<Time>,
    custom_dilations: Query<&crate::als_camera::AlsCameraCustomTimeDilation>,
    mut commands: Commands,
    mut transitions: MessageWriter<StateTransitionEvent>,
    assets: Res<MantleAssets>,
    clips: Res<Assets<GraphClip>>,
    skeletons: Res<Assets<Skeleton>>,
    mut roll_blend_outs: Query<&mut crate::roll_drive::AlsRollBlendOut>,
    target_transforms: Query<&GlobalTransform, Without<AlsCharacter>>,
    mut characters: Query<
        (
            Entity,
            &mut AlsMantleState,
            Option<&mut AlsMantleMontages>,
            &mut LocomotionState,
            Option<&crate::als::roll::AlsRollState>,
            Option<&mut crate::get_up::AlsGetUpState>,
            Option<&AlsNotifyLocomotionAction>,
        ),
        With<AlsCharacter>,
    >,
) {
    for (character, mut mantle, mut current, mut locomotion, roll, mut get_up, action) in
        &mut characters
    {
        let delta = time.delta_secs() * custom_dilations.get(character).map_or(1.0, |value| value.0);
        if mantle.active
            && !mantle.root_motion_ready
            && (action == Some(&AlsNotifyLocomotionAction::Ragdolling)
                || mantle
                    .target_base
                    .is_some_and(|base| !target_transforms.contains(base.entity)))
        {
            mantle.active = false;
        }
        let starting = mantle.active
            && !mantle.root_motion_ready
            && time.delta_secs() > 1e-8
            && clips
                .get(assets.clip(mantle.kind))
                .is_some_and(|clip| skeletons.contains(&clip.skeleton));
        let mut added = AlsMantleMontages::default();
        let montages = current.as_deref_mut().unwrap_or(&mut added);
        if starting {
            if *locomotion != LocomotionState::Custom {
                transitions.write(StateTransitionEvent {
                    entity: character,
                    dimension: Dimension::LocomotionState,
                    from: StateValue::LocomotionState(*locomotion),
                    to: StateValue::LocomotionState(LocomotionState::Custom),
                });
                *locomotion = LocomotionState::Custom;
            }
            montages.begin(&mantle);
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
            if action == Some(&AlsNotifyLocomotionAction::Ragdolling) {
                entry.stop(
                    crate::get_up::RAGDOLL_SETTINGS.stop_montage_blend_seconds,
                    true,
                );
            } else if !mantle.active && get_up.is_some() {
                entry.stop(0.0, true);
            } else if !mantle.active && roll.is_some_and(|roll| roll.active) {
                entry.stop(crate::als::roll::ROLL_WEIGHT_IN_SECS, true);
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
            &LocomotionState,
            &crate::als::state::RotationMode,
            &crate::als::state::Stance,
            &crate::als::input::AlsInputCommand,
            Option<&crate::als::motion::AlsInputOverride>,
        ),
        With<AlsCharacter>,
    >,
) {
    for event in events.read() {
        if event.class_name != "AlsAnimNotifyState_EarlyBlendOut"
            || event.phase != crate::extensions::EventPhase::Active
        {
            continue;
        }
        let Ok((mut montages, locomotion, rotation, stance, command, override_command)) =
            characters.get_mut(event.character)
        else {
            continue;
        };
        let command = override_command.map_or(command, |value| &value.0);
        if crate::animation_events::early_blend_out_matches(
            &event.parameters,
            command.move_dir.length_squared() > 1e-8,
            *locomotion,
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
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
    time: Res<Time>,
    custom_dilations: Query<&crate::als_camera::AlsCameraCustomTimeDilation>,
    mut commands: Commands,
    assets: Res<MantleAssets>,
    clips: Res<Assets<GraphClip>>,
    skeletons: Res<Assets<Skeleton>>,
    mut characters: Query<
        (
            Entity,
            &mut AlsMantleMontages,
            Option<&crate::get_up::AlsGetUpState>,
            Option<&crate::als::roll::AlsRollState>,
            Option<&crate::character_assembly::AlsCharacterAssembly>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (character, mut montages, get_up, roll, assembly) in &mut characters {
        let delta = time.delta_secs() * custom_dilations.get(character).map_or(1.0, |value| value.0);
        let selected = assembly.and_then(|assembly| avatars.get(assembly.avatar).ok())
            .filter(|(avatar, _)| avatar.character == character)
            .map(|(_, instance)| instance.player_entity()).filter(|entity| players.contains(*entity));
        if montages.published_player != selected {
            if let Some(previous) = montages.published_player
                && !avatars.iter().any(|(avatar, instance)| avatar.character != character && instance.player_entity() == previous)
            {
                if let Ok(mut player) = players.get_mut(previous) {
                    player.set_global_input_data(POST_LOCOMOTION_POSE, DataValue::Pose(bevy_animation_graph::core::pose::Pose::default()));
                    player.set_global_input_data(POST_LOCOMOTION_WEIGHT, 0.0_f32);
                    player.set_global_input_data(POST_LOCOMOTION_LAYER_WEIGHT, 0.0_f32);
                }
                if let Ok(mut inputs) = source_inputs.get_mut(previous) {
                    inputs.set_slot_input_context(POST_LOCOMOTION_POSE, None);
                }
            }
            montages.published_player = selected;
        }
        let Some(player_entity) = selected else {
            for entry in &mut montages.entries { entry.pose_ready = false; }
            continue;
        };
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
                delta_seconds: delta,
                playback_delta_seconds: entry.clip_time - entry.previous_clip_time,
                looping: false,
                weight: 1.0,
            });
            let mut clock = context.relevant_asset.clone().unwrap();
            clock.weight = entry.weight;
            context.asset_samples.push(crate::extensions::animation_context::SourceAssetSample {
                identity: format!("mantle:{}", entry.id), clip: assets.clip(entry.kind).clone(), clock, montage: true, interrupted: entry.interrupted, ..default()
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
                if let Some(context) = source_inputs.get(player_entity).ok().and_then(|inputs| inputs.slot_input_context(POST_LOCOMOTION_POSE)) {
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
                DataValue::Pose(poses.iter().map(|(pose, weight)| pose.scalar_mult(*weight))
                    .reduce(|accumulator, pose| accumulator.linear_add(&pose)).unwrap()),
            );
            player.set_global_input_data(POST_LOCOMOTION_WEIGHT, total.min(1.0));
            player.set_global_input_data(POST_LOCOMOTION_LAYER_WEIGHT, 1.0_f32);
            source_inputs.get_mut(player_entity).unwrap().set_slot_input_context(POST_LOCOMOTION_POSE,
                Some(crate::extensions::source_pose_nodes::blend_animation_contexts(
                    contexts,
                )),);
        } else {
            player.set_global_input_data(POST_LOCOMOTION_WEIGHT, 0.0_f32);
            player.set_global_input_data(POST_LOCOMOTION_LAYER_WEIGHT, 0.0_f32);
            source_inputs.get_mut(player_entity).unwrap().set_slot_input_context(POST_LOCOMOTION_POSE, None);
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
            With<crate::als::character::AlsCharacter>,
            Without<AlsMantleState>,
        ),
    >,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsMantleState::default());
    }
}

fn mantle_pose_system(
    time: Res<Time<Virtual>>,
    mut commands: Commands,
    mut transitions: MessageWriter<StateTransitionEvent>,
    target_transforms: Query<&GlobalTransform, Without<crate::als::character::AlsCharacter>>,
    mut q: Query<
        (
            Entity,
            &mut AlsMantleState,
            &mut LocomotionState,
            &mut LinearVelocity,
            Option<&AlsNotifyLocomotionAction>,
            Option<&mut AlsMantleMontages>,
        ),
        With<AlsCharacter>,
    >,
) {
    if time.is_paused() { return; }
    for (entity, mut m, mut locomotion, mut vel, action, montages) in &mut q {
        if !m.active && !m.root_motion_ready {
            continue;
        }
        let action_ended = action != Some(&AlsNotifyLocomotionAction::Mantling);
        let mode_changed = *locomotion != LocomotionState::Custom;
        let target_valid = action_ended
            || mode_changed
            || !m.active
            || m.target_base
                .is_none_or(|base| target_transforms.contains(base.entity));
        if !target_valid {
            m.target_invalid = true;
            *vel = LinearVelocity(Vec3::ZERO);
        }
        if action_ended || mode_changed || !target_valid {
            let start_ragdoll =
                !target_valid && MANTLE_SETTINGS.start_ragdoll_on_target_destruction;
            if !action_ended && let Some(mut montages) = montages {
                for entry in &mut montages.entries {
                    if entry.kind.action_settings().montage_index
                        == m.kind.action_settings().montage_index
                    {
                        entry.stop(
                            if start_ragdoll {
                                crate::get_up::RAGDOLL_SETTINGS.stop_montage_blend_seconds
                            } else {
                                MANTLE_SETTINGS.stop_blend_out_seconds
                            },
                            true,
                        );
                    }
                }
            }
            m.active = false;
            m.root_motion_ready = false;
            if *locomotion != LocomotionState::Grounded {
                vel.y = 0.0;
                transitions.write(StateTransitionEvent {
                    entity,
                    dimension: Dimension::LocomotionState,
                    from: StateValue::LocomotionState(*locomotion),
                    to: StateValue::LocomotionState(LocomotionState::Grounded),
                });
                *locomotion = LocomotionState::Grounded;
            }
            if start_ragdoll {
                transitions.write(StateTransitionEvent {
                    entity,
                    dimension: Dimension::LocomotionState,
                    from: StateValue::LocomotionState(LocomotionState::Grounded),
                    to: StateValue::LocomotionState(LocomotionState::None),
                });
                *locomotion = LocomotionState::None;
                commands
                    .entity(entity)
                    .insert(AlsNotifyLocomotionAction::Ragdolling);
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn mantle_drive_system(
    time: Res<Time>,
    custom_dilations: Query<&crate::als_camera::AlsCameraCustomTimeDilation>,
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &mut AlsMantleState,
            &LocomotionState,
            &mut LinearVelocity,
            Option<&mut Rotation>,
            Option<&LockedAxes>,
            Option<&mut Position>,
            Has<CustomPositionIntegration>,
            Has<CustomVelocityIntegration>,
            Option<&mut crate::als::character::AlsCmcAdapterRuntime>,
        ),
        (With<AlsCharacter>, With<Transform>),
    >,
    parents: Query<&ChildOf>,
    mut transforms: ParamSet<(
        bevy::transform::helper::TransformHelper,
        Query<&mut Transform, With<AlsCharacter>>,
    )>,
    target_transforms: Query<&GlobalTransform, Without<crate::als::character::AlsCharacter>>,
    assets: Res<MantleAssets>,
    clips: Res<Assets<GraphClip>>,
    skeletons: Res<Assets<Skeleton>>,
) {
    let dt = time.delta_secs();
    for (
        entity,
        mut m,
        locomotion,
        mut vel,
        rotation,
        locked_axes,
        position,
        custom_position,
        custom_velocity,
        mut cmc,
    ) in &mut q
    {
        let actor_delta = dt * custom_dilations.get(entity).map_or(1.0, |value| value.0);
        if (!m.active && !m.root_motion_ready)
            || *locomotion != LocomotionState::Custom
            || dt <= 0.0
        {
            continue;
        }
        if let Some(cmc) = cmc.as_deref_mut() {
            vel.0 = cmc.grounded_collision_velocity.take().unwrap_or(vel.0);

        }
        commands.entity(entity).insert((
            AlsMantlePhysicsFrame {
                custom_position,
                custom_velocity,
                locked_axes: locked_axes.copied(),
            },
            CustomPositionIntegration,
            CustomVelocityIntegration,
            LockedAxes::ALL_LOCKED,
        ));
        if actor_delta <= 1e-8 && m.root_motion_ready {
            m.time += actor_delta;
            continue;
        }
        let target_base = mantle_target_base(&m, &target_transforms);
        if !m.active || (m.target_base.is_some() && target_base.is_none()) {
            *vel = LinearVelocity(Vec3::ZERO);
            continue;
        }
        m.refresh_target_base(target_base);

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
        let current_time = (m.time + actor_delta).min(m.action_end());
        let Some(root) = sample_mantle_root(clip, skeleton, m.clip_time_at(current_time)) else {
            continue;
        };
        let target_actor_transform = m.target_actor_transform_at(current_time, root);
        let Ok(actor_world) = transforms.p0().compute_global_transform(entity) else { continue; };
        let Ok(parent_world) = parents.get(entity).ok()
            .map(|parent| transforms.p0().compute_global_transform(parent.parent())).transpose() else { continue; };
        let mut query = transforms.p1();
        let mut transform = query.get_mut(entity).unwrap();
        let translation = target_actor_transform.translation - actor_world.translation();
        if actor_delta > 1e-8 {
            *vel = LinearVelocity(translation / actor_delta);
        }
        if actor_delta >= crate::als::movement_constants::MIN_MOVEMENT_DELTA_SECONDS {
            transform.translation = parent_world.map_or(target_actor_transform.translation,
                |parent| parent.affine().inverse().transform_point3(target_actor_transform.translation));
            if let Some(mut position) = position {
                position.0 = target_actor_transform.translation;
            }
        }
        transform.rotation = parent_world.map_or(target_actor_transform.rotation,
            |parent| parent.rotation().inverse() * target_actor_transform.rotation);
        if let Some(mut rotation) = rotation {
            rotation.0 = target_actor_transform.rotation;
        }
        m.time = current_time;
        m.active = current_time < m.action_end();
    }
}

fn release_mantle_physics(
    mut commands: Commands,
    characters: Query<(Entity, &AlsMantlePhysicsFrame)>,
) {
    for (entity, physics) in &characters {
        let mut entity_commands = commands.entity(entity);
        if !physics.custom_position {
            entity_commands.remove::<CustomPositionIntegration>();
        }
        if !physics.custom_velocity {
            entity_commands.remove::<CustomVelocityIntegration>();
        }
        if let Some(locked_axes) = physics.locked_axes {
            entity_commands.insert(locked_axes);
        } else {
            entity_commands.remove::<LockedAxes>();
        }
        entity_commands.remove::<AlsMantlePhysicsFrame>();
    }
}

fn update_mantle_movement_base(
    mut mantles: Query<(Entity, &mut AlsMantleState), With<AlsMantlePhysicsFrame>>,
    parents: Query<&ChildOf>,
    mut transforms: ParamSet<(
        bevy::transform::helper::TransformHelper,
        Query<(&mut Transform, Option<&mut Position>, Option<&mut Rotation>), With<AlsCharacter>>,
    )>,
    assets: Res<MantleAssets>,
    clips: Res<Assets<GraphClip>>,
    skeletons: Res<Assets<Skeleton>>,
) {
    for (entity, mut mantle) in &mut mantles {
        let Some(base) = mantle.target_base else { continue; };
        if !mantle.root_motion_ready { continue; }
        let Ok(latest) = transforms.p0().compute_global_transform(base.entity) else { continue; };
        let Some(clip) = clips.get(assets.clip(mantle.kind)) else { continue; };
        let Some(skeleton) = skeletons.get(&clip.skeleton) else { continue; };
        let Some(root) = sample_mantle_root(clip, skeleton, mantle.clip_time()) else { continue; };
        mantle.refresh_target_base(Some(AlsMantleTargetBase {
            translation: latest.translation(), rotation: latest.rotation(), scale: latest.scale(), ..base
        }));
        let target = mantle.target_actor_transform_at(mantle.time, root);
        let Ok(parent_world) = parents.get(entity).ok()
            .map(|parent| transforms.p0().compute_global_transform(parent.parent())).transpose() else { continue; };
        let mut query = transforms.p1();
        let Ok((mut transform, position, rotation)) = query.get_mut(entity) else { continue; };
        transform.translation = parent_world.map_or(target.translation,
            |parent| parent.affine().inverse().transform_point3(target.translation));
        transform.rotation = parent_world.map_or(target.rotation,
            |parent| parent.rotation().inverse() * target.rotation);
        if let Some(mut position) = position { position.0 = target.translation; }
        if let Some(mut rotation) = rotation { rotation.0 = target.rotation; }
    }
}

fn mantle_target_base(
    state: &AlsMantleState,
    transforms: &Query<&GlobalTransform, Without<crate::als::character::AlsCharacter>>,
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
pub(crate) mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
    use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;

    #[test]
    fn mantle_physics_moves_once_with_the_game_delta() {
        use avian3d::prelude::*;
        use bevy::animation::animated_field;
        use bevy_animation_graph::core::animation_clip::EntityPath;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            PhysicsPlugins::new(PostUpdate),
            AlsMantleDrivePlugin,
            crate::source_physics_timing::SourcePhysicsTimingPlugin,
            bevy_tnua_avian3d::TnuaAvian3dPlugin::new(Update),
            crate::als::character::AlsCharacterPlugin,
            crate::als::motion::AlsMotionPlugin,
            crate::ragdoll_link::AlsRagdollLinkPlugin,
        ))
        .init_asset::<GraphClip>()
        .init_asset::<Skeleton>()
        .init_asset::<Mesh>()
        .init_asset::<bevy_animation_graph::core::animated_scene::AnimatedScene>()
        .init_asset::<bevy_animation_graph::core::ragdoll::bone_mapping::RagdollBoneMap>()
        .init_asset::<crate::als::params::AlsMovementParams>()
        .add_message::<crate::animation_events::AlsAnimationEvent>();
        let path = EntityPath::default().child("root");
        let root = path.id();
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root);
        skeleton.add_bone(path, Transform::IDENTITY, Transform::IDENTITY);
        let skeleton = app
            .world_mut()
            .resource_mut::<Assets<Skeleton>>()
            .add(skeleton);
        let mut animation = bevy::animation::AnimationClip::default();
        animation.add_curve_to_target(
            root.animation_target_id(),
            AnimatableCurve::new(
                animated_field!(Transform::translation),
                UnevenSampleAutoCurve::new([(0.0, Vec3::ZERO), (1.5, Vec3::Y)]).unwrap(),
            ),
        );
        let clip =
            app.world_mut()
                .resource_mut::<Assets<GraphClip>>()
                .add(GraphClip::from_bevy_clip(
                    animation,
                    skeleton,
                    default(),
                    None,
                ));
        app.insert_resource(MantleAssets(vec![clip; 6]));
        app.finish();
        app.cleanup();
        let params = app
            .world_mut()
            .resource_mut::<Assets<crate::als::params::AlsMovementParams>>()
            .add(crate::als::params::AlsMovementParams::default());
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(20.0, 0.2, 20.0),
            Transform::from_xyz(0.0, 3.0, 0.0),
        ));
        for (delta, original) in [1.0 / 60.0, 0.08_f32]
            .into_iter()
            .flat_map(|dt| (0..4).map(move |mask| (dt, mask)))
        {
            let start = Vec3::new(1.0, 2.0, 3.0);
            let target = Vec3::new(3.0, 4.0, -2.0);
            let mut mantle = AlsMantleState::default();
            mantle.begin_with_start_time(MantleKind::Low, start, target, 0.0, 0.2, -0.5);
            let actor = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    mantle,
                    LocomotionState::Grounded,
                    RigidBody::Dynamic,
                    Collider::sphere(0.3),
                    Transform::from_translation(start).with_rotation(Quat::from_rotation_y(0.2)),
                    GravityScale(5.0),
                    AngularVelocity(Vec3::Y * 3.0),
                    CollisionLayers::ALL,
                    SweptCcd::default(),
                ))
                .id();
            app.world_mut()
                .entity_mut(actor)
                .insert(bevy_tnua::TnuaMotor {
                    lin: bevy_tnua::TnuaVelChange {
                        boost: Vec3::X * 2.0,
                        acceleration: Vec3::NEG_Y * 40.0,
                    },
                    ..default()
                });
            app.world_mut().entity_mut(actor).insert((
                crate::als::character::AlsMovementParamsHandle(params.clone()),
                crate::als::state::Gait::Running,
                crate::als::state::RotationMode::Aiming,
                crate::als::motion::AlsViewYaw(1.4),
                crate::actual_motion::AlsActualMotion::default(),
                crate::als::input::AlsInputCommand {
                    move_dir: Vec2::Y,
                    ..default()
                },
            ));
            if original & 1 != 0 {
                app.world_mut()
                    .entity_mut(actor)
                    .insert(CustomPositionIntegration);
            }
            if original & 2 != 0 {
                app.world_mut()
                    .entity_mut(actor)
                    .insert(CustomVelocityIntegration);
            }
            let mut frames = 0;
            let hold_expired = delta > 0.05 && original == 0;
            loop {
                let before = app.world().get::<Transform>(actor).unwrap().translation;
                if !hold_expired && (frames + 1) as f32 * delta >= 1.5 {
                    app.world_mut()
                        .entity_mut(actor)
                        .insert(AlsNotifyLocomotionAction::None);
                }
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_secs_f32(delta));
                app.world_mut().run_schedule(Update);
                app.world_mut().run_schedule(PostUpdate);
                frames += 1;
                let state = app.world().get::<AlsMantleState>(actor).unwrap();
                let expected_time = (frames as f32 * delta).min(1.5);
                assert!(
                    (state.time - expected_time).abs() < 1e-5,
                    "game clock delta={delta} frame={frames} actual={}",
                    state.time
                );
                let root = Transform::from_translation(Vec3::Y * (state.clip_time() / 1.5));
                let expected = state.target_actor_transform_at(state.time, root);
                let actual = app.world().get::<Transform>(actor).unwrap();
                assert!(
                    actual.translation.distance(expected.translation) < 1e-4,
                    "source move delta={delta} frame={frames}: {:?} expected {:?}",
                    actual.translation,
                    expected.translation
                );
                assert!(actual.rotation.angle_between(expected.rotation) < 1e-3);
                assert!(
                    app.world()
                        .get::<Position>(actor)
                        .unwrap()
                        .0
                        .distance(expected.translation)
                        < 1e-4
                );
                let mut expected_velocity = (expected.translation - before) / delta;
                if !state.active && !hold_expired {
                    expected_velocity.y = 0.0;
                }
                assert!(
                    app.world()
                        .get::<LinearVelocity>(actor)
                        .unwrap()
                        .0
                        .distance(expected_velocity)
                        < 1e-4
                );
                assert_eq!(
                    app.world().resource::<Time>().delta(),
                    std::time::Duration::from_secs_f32(delta)
                );
                assert_eq!(
                    app.world().resource::<Time<Physics>>().delta(),
                    std::time::Duration::from_secs_f32(delta)
                        .min(crate::source_physics_timing::SOURCE_MAX_PHYSICS_DELTA)
                );
                if !state.active {
                    assert!(actual.translation.distance(target) < 1e-4);
                    if hold_expired {
                        for end_action in [false, true] {
                            if end_action {
                                app.world_mut()
                                    .entity_mut(actor)
                                    .insert(AlsNotifyLocomotionAction::None);
                            }
                            app.world_mut()
                                .resource_mut::<Time>()
                                .advance_by(std::time::Duration::from_secs_f32(delta));
                            app.world_mut().run_schedule(Update);
                            app.world_mut().run_schedule(PostUpdate);
                            assert!(
                                app.world()
                                    .get::<Transform>(actor)
                                    .unwrap()
                                    .translation
                                    .distance(target)
                                    < 1e-4
                            );
                            assert_eq!(
                                app.world().get::<LinearVelocity>(actor),
                                Some(&LinearVelocity::ZERO)
                            );
                            assert_eq!(
                                app.world().get::<LocomotionState>(actor),
                                Some(&if end_action {
                                    LocomotionState::Grounded
                                } else {
                                    LocomotionState::Custom
                                })
                            );
                        }
                    }
                    break;
                }
                assert!(frames < 100, "mantle did not complete");
            }
            assert_eq!(
                app.world().get::<RigidBody>(actor),
                Some(&RigidBody::Dynamic)
            );
            assert_eq!(
                app.world().get::<CollisionLayers>(actor),
                Some(&CollisionLayers::ALL)
            );
            assert_eq!(
                app.world()
                    .get::<CustomPositionIntegration>(actor)
                    .is_some(),
                original & 1 != 0
            );
            assert_eq!(
                app.world()
                    .get::<CustomVelocityIntegration>(actor)
                    .is_some(),
                original & 2 != 0
            );
            assert!(app.world().get::<bevy_tnua::TnuaToggle>(actor).is_none());
            let previous_rotation = app.world().get::<Transform>(actor).unwrap().rotation;
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(delta));
            app.world_mut().run_schedule(Update);
            app.world_mut().run_schedule(PostUpdate);
            assert!(
                app.world()
                    .get::<Transform>(actor)
                    .unwrap()
                    .rotation
                    .angle_between(previous_rotation)
                    > 1e-3,
                "Completed mantle did not resume rotation: delta={delta} original={original}"
            );
            app.world_mut().despawn(actor);
        }
        for interruption in 0..5 {
            let target = app.world_mut().spawn(GlobalTransform::IDENTITY).id();
            let mut mantle = AlsMantleState::default();
            mantle.begin_with_target_base(
                MantleKind::Low,
                Vec3::Y * 2.0,
                Vec3::Y * 4.0,
                0.75,
                0.0,
                0.0,
                AlsMantleTargetBase {
                    entity: target,
                    translation: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::ONE,
                    relative_location: false,
                },
            );
            let actor = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    mantle,
                    LocomotionState::Grounded,
                    AlsNotifyLocomotionAction::None,
                    RigidBody::Dynamic,
                    Collider::sphere(0.3),
                    CollisionLayers::ALL,
                    Transform::from_translation(Vec3::Y * 2.0),
                    bevy_tnua::TnuaToggle::SenseOnly,
                ))
                .id();
            if interruption == 4 {
                app.world_mut().despawn(target);
            }
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(80));
            app.world_mut().run_schedule(Update);
            app.world_mut().run_schedule(PostUpdate);
            if interruption == 4 {
                assert!(!app.world().get::<AlsMantleState>(actor).unwrap().active);
                assert_eq!(
                    app.world().get::<LocomotionState>(actor),
                    Some(&LocomotionState::Grounded)
                );
                assert_eq!(
                    app.world().get::<AlsNotifyLocomotionAction>(actor),
                    Some(&AlsNotifyLocomotionAction::None)
                );
                assert!(app.world().get::<AlsMantleMontages>(actor).is_none());
                app.world_mut().despawn(actor);
                continue;
            }
            assert_eq!(
                app.world().get::<LocomotionState>(actor),
                Some(&LocomotionState::Custom)
            );
            match interruption {
                0 => {
                    app.world_mut()
                        .entity_mut(actor)
                        .insert(AlsNotifyLocomotionAction::None);
                }
                1 => {
                    app.world_mut()
                        .entity_mut(actor)
                        .insert(LocomotionState::Air);
                }
                2 => {
                    app.world_mut()
                        .entity_mut(actor)
                        .insert(AlsNotifyLocomotionAction::Ragdolling);
                }
                _ => {
                    app.world_mut().despawn(target);
                }
            }
            app.world_mut().run_schedule(Update);
            app.world_mut().run_schedule(PostUpdate);
            let ragdolling = interruption >= 2;
            assert!(!app.world().get::<AlsMantleState>(actor).unwrap().active);
            assert_eq!(
                app.world().get::<LocomotionState>(actor),
                Some(&if interruption == 3 {
                    LocomotionState::None
                } else {
                    LocomotionState::Grounded
                })
            );
            assert_eq!(
                app.world().get::<RigidBody>(actor),
                Some(&if ragdolling {
                    RigidBody::Kinematic
                } else {
                    RigidBody::Dynamic
                })
            );
            assert_eq!(
                app.world().get::<bevy_tnua::TnuaToggle>(actor),
                Some(&if ragdolling {
                    bevy_tnua::TnuaToggle::Disabled
                } else {
                    bevy_tnua::TnuaToggle::SenseOnly
                })
            );
            assert_eq!(
                app.world().get::<CollisionLayers>(actor),
                Some(&if ragdolling {
                    CollisionLayers::NONE
                } else {
                    CollisionLayers::ALL
                })
            );
            assert!(
                app.world()
                    .get::<CustomPositionIntegration>(actor)
                    .is_none()
            );
            assert!(
                app.world()
                    .get::<CustomVelocityIntegration>(actor)
                    .is_none()
            );
            if interruption == 3 {
                let entry = &app.world().get::<AlsMantleMontages>(actor).unwrap().entries[0];
                assert!(entry.interrupted);
                assert_eq!(entry.stop.unwrap().1, 0.2);
            }
            if ragdolling {
                app.world_mut()
                    .entity_mut(actor)
                    .insert((LocomotionState::Air, AlsNotifyLocomotionAction::None));
                app.world_mut().run_schedule(Update);
                assert_eq!(
                    app.world().get::<RigidBody>(actor),
                    Some(&RigidBody::Dynamic)
                );
                assert_eq!(
                    app.world().get::<bevy_tnua::TnuaToggle>(actor),
                    Some(&bevy_tnua::TnuaToggle::SenseOnly)
                );
                assert_eq!(
                    app.world().get::<CollisionLayers>(actor),
                    Some(&CollisionLayers::ALL)
                );
            }
            app.world_mut().despawn(actor);
            if interruption != 3 {
                app.world_mut().despawn(target);
            }
        }
    }

    #[test]
    fn mantle_action_end_releases_motion_before_the_montage_ends() {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world.init_resource::<Messages<crate::als::transition::StateTransitionEvent>>();
        let mut mantle = AlsMantleState::default();
        mantle.begin(MantleKind::Low, Vec3::ZERO, Vec3::Y, 1.0, 0.0, 0.0);
        mantle.time = 1.0;
        mantle.root_motion_ready = true;
        let actor = world
            .spawn((
                AlsCharacter,
                mantle,
                crate::als::state::LocomotionState::Custom,
                LinearVelocity::ZERO,
                crate::animation_events::AlsNotifyLocomotionAction::None,
            ))
            .id();
        world.run_system_once(mantle_pose_system).unwrap();
        assert!(!world.get::<AlsMantleState>(actor).unwrap().active);
        assert_eq!(
            world.get::<crate::als::state::LocomotionState>(actor),
            Some(&crate::als::state::LocomotionState::Grounded)
        );
        assert!(
            !world
                .get::<AlsMantleState>(actor)
                .unwrap()
                .root_motion_ready
        );
    }

    #[test]
    fn paused_mantle_owner_defers_target_destruction_until_resume() {
        let mut world = World::new();
        world.init_resource::<Messages<StateTransitionEvent>>();
        world.init_resource::<Time<Virtual>>();
        let target = world.spawn(GlobalTransform::IDENTITY).id();
        let mut mantle = AlsMantleState::default();
        mantle.begin(MantleKind::Low, Vec3::ZERO, Vec3::Y, 0.75, 0.0, 0.0);
        mantle.root_motion_ready = true;
        mantle.target_base = Some(AlsMantleTargetBase {
            entity: target, translation: Vec3::ZERO, rotation: Quat::IDENTITY,
            scale: Vec3::ONE, relative_location: false,
        });
        let mut montages = AlsMantleMontages::default();
        montages.begin(&mantle);
        montages.entries[0].advance(0.3, None);
        let actor = world.spawn((AlsCharacter, mantle, montages, LocomotionState::Custom,
            AlsNotifyLocomotionAction::Mantling, LinearVelocity(Vec3::Y))).id();
        world.run_system_once(mantle_pose_system).unwrap();
        assert!(world.get::<AlsMantleState>(actor).unwrap().active);
        world.resource_mut::<Time<Virtual>>().pause();
        world.despawn(target);
        world.run_system_once(mantle_pose_system).unwrap();
        assert!(world.get::<AlsMantleState>(actor).unwrap().active,
            "Source paused Character must retain its active mantle after target destruction");
        assert_eq!(world.get::<LocomotionState>(actor), Some(&LocomotionState::Custom));
        assert_eq!(world.get::<AlsNotifyLocomotionAction>(actor), Some(&AlsNotifyLocomotionAction::Mantling));
        assert!(world.get::<AlsMantleMontages>(actor).unwrap().entries[0].stop.is_none());
        world.resource_mut::<Time<Virtual>>().unpause();
        world.run_system_once(mantle_pose_system).unwrap();
        assert!(!world.get::<AlsMantleState>(actor).unwrap().active);
        assert!(world.get::<AlsMantleState>(actor).unwrap().target_invalid);
        assert_eq!(world.get::<AlsNotifyLocomotionAction>(actor), Some(&AlsNotifyLocomotionAction::Ragdolling));
    }

    #[test]
    fn mantle_custom_mode_uses_source_stop_priority() {
        use crate::als::state::LocomotionState;
        use crate::als::transition::{Dimension, StateTransitionEvent, StateValue};
        for (mode, action, missing_target, expected, interrupted) in [
            (
                LocomotionState::Custom,
                AlsNotifyLocomotionAction::Ragdolling,
                false,
                LocomotionState::Grounded,
                false,
            ),
            (
                LocomotionState::Custom,
                AlsNotifyLocomotionAction::Mantling,
                false,
                LocomotionState::Custom,
                false,
            ),
            (
                LocomotionState::Custom,
                AlsNotifyLocomotionAction::None,
                false,
                LocomotionState::Grounded,
                false,
            ),
            (
                LocomotionState::Air,
                AlsNotifyLocomotionAction::Mantling,
                false,
                LocomotionState::Grounded,
                true,
            ),
            (
                LocomotionState::Grounded,
                AlsNotifyLocomotionAction::Mantling,
                false,
                LocomotionState::Grounded,
                true,
            ),
            (
                LocomotionState::Custom,
                AlsNotifyLocomotionAction::None,
                true,
                LocomotionState::Grounded,
                false,
            ),
            (
                LocomotionState::Air,
                AlsNotifyLocomotionAction::Mantling,
                true,
                LocomotionState::Grounded,
                true,
            ),
            (
                LocomotionState::Custom,
                AlsNotifyLocomotionAction::Mantling,
                true,
                LocomotionState::None,
                true,
            ),
        ] {
            let mut world = World::new();
            world.init_resource::<Time<Virtual>>();
            world.init_resource::<Messages<StateTransitionEvent>>();
            let mut mantle = AlsMantleState::default();
            mantle.begin(MantleKind::Low, Vec3::ZERO, Vec3::Y, 0.75, 0.0, 0.0);
            mantle.root_motion_ready = true;
            if missing_target {
                let target = world.spawn_empty().id();
                world.despawn(target);
                mantle.target_base = Some(AlsMantleTargetBase {
                    entity: target,
                    translation: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::ONE,
                    relative_location: false,
                });
            }
            let mut montages = AlsMantleMontages::default();
            montages.begin(&mantle);
            montages.entries[0].advance(0.3, None);
            let invalid = missing_target
                && mode == LocomotionState::Custom
                && action == AlsNotifyLocomotionAction::Mantling;
            let actor = world
                .spawn((
                    AlsCharacter,
                    mantle,
                    montages,
                    mode,
                    action,
                    LinearVelocity(Vec3::new(1.0, 2.0, 3.0)),
                ))
                .id();
            world.run_system_once(mantle_pose_system).unwrap();
            assert_eq!(world.get::<LocomotionState>(actor), Some(&expected));
            assert_eq!(
                world.get::<AlsNotifyLocomotionAction>(actor),
                Some(&if invalid {
                    AlsNotifyLocomotionAction::Ragdolling
                } else {
                    action
                })
            );
            assert_eq!(
                world.get::<AlsMantleState>(actor).unwrap().active,
                expected == LocomotionState::Custom
            );
            assert_eq!(
                world.get::<AlsMantleState>(actor).unwrap().target_invalid,
                invalid
            );
            assert_eq!(
                world.get::<LinearVelocity>(actor).unwrap().0,
                if invalid {
                    Vec3::ZERO
                } else if mode != expected {
                    Vec3::new(1.0, 0.0, 3.0)
                } else {
                    Vec3::new(1.0, 2.0, 3.0)
                }
            );
            let entry = &world.get::<AlsMantleMontages>(actor).unwrap().entries[0];
            assert_eq!(entry.interrupted, interrupted);
            assert_eq!(
                entry.stop.map(|(_, duration, _)| duration),
                interrupted.then_some(if invalid { 0.2 } else { 0.3 })
            );
            let events: Vec<_> = world
                .resource_mut::<Messages<StateTransitionEvent>>()
                .drain()
                .collect();
            assert_eq!(
                events.len(),
                if invalid {
                    2
                } else {
                    usize::from(mode != expected)
                }
            );
            if let Some(event) = events.first() {
                assert_eq!(event.entity, actor);
                assert_eq!(event.dimension, Dimension::LocomotionState);
                assert_eq!(event.from, StateValue::LocomotionState(mode));
                assert_eq!(
                    event.to,
                    StateValue::LocomotionState(if invalid {
                        LocomotionState::Grounded
                    } else {
                        expected
                    })
                );
            }
            if invalid {
                assert_eq!(
                    events[1].from,
                    StateValue::LocomotionState(LocomotionState::Grounded)
                );
                assert_eq!(
                    events[1].to,
                    StateValue::LocomotionState(LocomotionState::None)
                );
            }
            world.run_system_once(mantle_pose_system).unwrap();
            assert_eq!(
                world
                    .resource_mut::<Messages<StateTransitionEvent>>()
                    .drain()
                    .count(),
                0
            );
        }
    }


    pub(crate) fn current_post_locomotion_base(app: &App, player: Entity) -> Pose {
        use bevy_animation_graph::core::{animation_graph::{AnimationGraph, SourcePin, TargetPin},
            context::node_states::StateKey};
        let player = app.world().get::<AnimationGraphPlayer>(player).unwrap();
        let arena = player.get_context_arena().unwrap();
        let graphs = app.world().resource::<Assets<AnimationGraph>>();
        let poses = arena.iter_context_ids().filter_map(|id| {
            let context = arena.get_context(id)?;
            let graph = graphs.get(context.get_graph_id())?;
            let slot = graph.nodes.values().find(|node| node.name == "Slot 'PostLocomotion'\nGroup 'Locomotion'")?;
            let SourcePin::NodeData(node, pin) = graph.edges_inverted.get(&TargetPin::NodeData(slot.id, "Source".into()))? else { return None; };
            context.node_caches.get_output_data(*node, StateKey::Default, pin.clone())?.into_pose().ok()
        }).collect::<Vec<_>>();
        assert_eq!(poses.len(), 1, "Actual PostLocomotion source pose must be unique");
        poses.into_iter().next().unwrap()
    }

    #[test]
    fn current_mantle_slot_uses_native_weighted_quaternion_blending() {
        let (app, _, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        let base = current_post_locomotion_base(&app, player_entity);
        let handle = app.world().resource::<MantleAssets>().clip(MantleKind::Low).clone();
        let clips = app.world().resource::<Assets<GraphClip>>();
        let clip = clips.get(&handle).unwrap();
        let skeleton = app.world().resource::<Assets<Skeleton>>().get(&clip.skeleton).unwrap();
        let overlay = sample_mantle_pose(clip, skeleton, mantle_montage_settings(MantleKind::Low), 0.35).unwrap().0;
        let weight = 0.15625;
        let actual = crate::extensions::slot_node::blend_slot_pose(base.clone(), Some(&overlay), Some(weight));
        let expected = crate::extensions::source_pose_nodes::blend_source_poses(&[(overlay, weight), (base, 1.0 - weight)]);
        assert_eq!(actual.skeleton, expected.skeleton);
        assert_eq!(actual.paths.len(), expected.paths.len());
        let mut maximum = 0.0_f32;
        for bone in actual.paths.keys() {
            if let (Some(a), Some(b)) = (actual.get_bone(*bone).unwrap().rotation, expected.get_bone(*bone).unwrap().rotation) {
                maximum = maximum.max((a - b).length().min((a + b).length()));
            }
        }
        assert!(maximum < 1e-6, "Current Mantle slot used spherical instead of native weighted blending: error={maximum}");
    }

    #[test]
    fn overlapping_current_mantles_keep_the_native_slot_base_quaternion_weight() {
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        let base = current_post_locomotion_base(&app, player_entity);
        let mut old = AlsMantleState::default();
        old.begin(MantleKind::Low, Vec3::ZERO, Vec3::Y, 0.75, 0.0, 0.0);
        let mut montages = AlsMantleMontages::default();
        montages.begin(&old);
        montages.entries[0].advance(1.5, Some(1.5));
        montages.entries[0].advance(0.05, None);
        let mut new = AlsMantleState::default();
        new.begin(MantleKind::LowLeft, Vec3::ZERO, Vec3::Y, 0.75, 0.0, 0.0);
        montages.begin(&new);
        for entry in &mut montages.entries { entry.advance(0.05, None); }
        let total: f32 = montages.entries.iter().map(|entry| entry.weight).sum();
        assert!(total > 0.0 && total < 1.0);
        let mut samples = Vec::new();
        for entry in &montages.entries {
            let handle = app.world().resource::<MantleAssets>().clip(entry.kind).clone();
            let clips = app.world().resource::<Assets<GraphClip>>();
            let clip = clips.get(&handle).unwrap();
            let skeleton = app.world().resource::<Assets<Skeleton>>().get(&clip.skeleton).unwrap();
            samples.push((sample_mantle_pose(clip, skeleton, mantle_montage_settings(entry.kind), entry.clip_time).unwrap().0, entry.weight));
        }
        app.world_mut().entity_mut(character).insert(montages);
        app.world_mut().run_system_once(publish_mantle_slot).unwrap();
        let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
        let overlay = player.get_global_input_data(POST_LOCOMOTION_POSE).unwrap().as_pose().unwrap();
        let weight = player.get_global_input_data(POST_LOCOMOTION_WEIGHT).unwrap().as_f32().unwrap();
        let actual = crate::extensions::slot_node::blend_slot_pose(base.clone(), Some(overlay), Some(weight));
        let mut native_inputs = samples;
        native_inputs.push((base, 1.0 - total));
        let expected = crate::extensions::source_pose_nodes::blend_source_poses(&native_inputs);
        let mut maximum = 0.0_f32;
        for bone in actual.paths.keys() {
            if let (Some(a), Some(b)) = (actual.get_bone(*bone).unwrap().rotation, expected.get_bone(*bone).unwrap().rotation) {
                maximum = maximum.max((a - b).length().min((a + b).length()));
            }
        }
        assert!(maximum < 1e-6, "Current Mantle slot normalized before adding its base: error={maximum}");
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
    fn fresh_audit_mantle_montages_and_published_slots_use_owner_time() {
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;
        let mut app = mantle_clip_test_app();
        let clip = app.world().resource::<MantleAssets>().clip(MantleKind::High).clone();
        let skeleton = app.world().resource::<Assets<GraphClip>>().get(&clip).unwrap().skeleton.clone();
        let mut failures = Vec::new();
        for dilation in [0.0_f32, 0.5, 1.0, 2.0] {
            let mut mantle = AlsMantleState::default();
            mantle.begin(MantleKind::High, Vec3::ZERO, Vec3::Y, 1.5, 0.0, 0.0);
            mantle.root_motion_ready = true;
            mantle.time = 0.2;
            let mut montages = AlsMantleMontages::default();
            montages.begin(&mantle);
            for entry in &mut montages.entries {
                entry.elapsed = 0.2;
                entry.clip_time = mantle.clip_time();
                entry.previous_clip_time = entry.clip_time;
                entry.weight = 1.0;
            }
            montages.begin(&mantle);
            let entry = montages.entries.last_mut().unwrap();
            entry.elapsed = 0.2;
            entry.clip_time = mantle.clip_time();
            entry.previous_clip_time = entry.clip_time;
            entry.weight = 1.0;
            let previous = mantle.clip_time();
            let delta = 0.05 * dilation;
            let expected_current = (mantle.clip_time_at(0.2 + delta) - delta).max(0.0)
                + delta * mantle_montage_settings(MantleKind::High).play_rate;
            let actor = app.world_mut().spawn((AlsCharacter, mantle, montages,
                LocomotionState::Custom, crate::als_camera::AlsCameraCustomTimeDilation(dilation))).id();
            let avatar = app.world_mut().spawn((AlsCharacterAvatar { character: actor },
                AnimatedSceneHandle::default(), Transform::IDENTITY, ChildOf(actor))).id();
            let player = app.world_mut().spawn((AnimationGraphPlayer::new(skeleton.clone()),
                Transform::IDENTITY, ChildOf(avatar))).id();
            app.world_mut().trigger(WorldInstanceReady { entity: avatar,
                instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()) });
            app.world_mut().flush();
            app.world_mut().entity_mut(actor).insert(crate::character_assembly::AlsCharacterAssembly { avatar, camera: None });
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
            app.world_mut().run_system_once(prepare_mantle_montages).unwrap();
            app.world_mut().run_system_once(publish_mantle_slot).unwrap();
            let entries = &app.world().get::<AlsMantleMontages>(actor).unwrap().entries;
            let context = app.world().get::<crate::extensions::SourceAnimationInputs>(player).unwrap()
                .slot_input_context(POST_LOCOMOTION_POSE).unwrap();
            let correct = (entries[0].elapsed - 0.2 - delta).abs() < 1e-6
                && (entries[1].elapsed - 0.2 - delta).abs() < 1e-6
                && (entries[0].clip_time - previous - delta * 1.2).abs() < 1e-6
                && (entries[1].clip_time - expected_current).abs() < 1e-6
                && context.asset_samples.iter().all(|sample| (sample.clock.delta_seconds - delta).abs() < 1e-6);
            if !correct { failures.push((dilation, entries[0].elapsed, entries[1].clip_time,
                context.asset_samples.iter().map(|sample| sample.clock.delta_seconds).collect::<Vec<_>>())); }
            app.world_mut().despawn(actor);
        }
        assert!(failures.is_empty(), "Mantle montage and slot clocks ignored owner time: {failures:?}");
    }

    #[test]
    fn loaded_mantle_zero_clock_entry_preserves_pending_source_velocity() {
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let origin = app.world().get::<Position>(actor).unwrap().0;
        let source_velocity = Vec3::new(0.7, 0.0, -0.4);
        let mut mantle = AlsMantleState::default();
        mantle.begin(MantleKind::High, origin, origin + Vec3::new(0.0, 1.5, -0.8), 1.5, 0.0, 0.0);
        app.world_mut().entity_mut(actor).insert((mantle, LinearVelocity(source_velocity),
            crate::als_camera::AlsCameraCustomTimeDilation(0.0)));
        for _ in 0..3 {
            app.update();
            let actual = app.world().get::<crate::actual_motion::AlsActualMotion>(actor).unwrap().world_velocity;
            assert!(actual.abs_diff_eq(source_velocity, 0.005),
                "Source zero-delta RMS entry retains pending CMC velocity instead of the physical bridge: actual={actual:?}");
            assert!(app.world().get::<Position>(actor).unwrap().0.abs_diff_eq(origin, 1e-5));
            assert_eq!(app.world().get::<AlsMantleState>(actor).unwrap().time, 0.0);
        }
    }

    #[test]
    fn loaded_mantle_keeps_queries_and_restores_physics_axes() {
        use avian3d::prelude::{PhysicsSystems, SpatialQueryFilter};
        use crate::als::character::AlsCollisionLayer;
        use crate::extensions::RagdollSpatialQuery;
        use std::sync::{Arc, Mutex};
        let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
        let origin = app.world().get::<Position>(actor).unwrap().0;
        let original_layers = *app.world().get::<CollisionLayers>(actor).unwrap();
        let original_axes = app.world().get::<LockedAxes>(actor).map(LockedAxes::to_bits);
        let samples = Arc::new(Mutex::new(Vec::new()));
        let captured = samples.clone();
        app.add_systems(PostUpdate, (move |spatial: RagdollSpatialQuery,
            actors: Query<(&Position, &CollisionLayers, &LockedAxes), With<AlsMantlePhysicsFrame>>| {
            let Ok((position, layers, axes)) = actors.get(actor) else { return };
            let hit = spatial.cast_ray_predicate(position.0 + Vec3::X, Dir3::NEG_X, 2.0, true,
                &SpatialQueryFilter::from_mask(AlsCollisionLayer::Default), &|entity| entity == actor).is_some();
            captured.lock().unwrap().push((hit, *layers, axes.to_bits(), position.0));
        }).after(PhysicsSystems::StepSimulation).before(PhysicsSystems::Writeback));
        let mut mantle = AlsMantleState::default();
        mantle.begin(MantleKind::High, origin, origin + Vec3::new(0.0, 1.5, -0.8), 1.5, 0.0, 0.0);
        app.world_mut().entity_mut(actor).insert(mantle);
        for _ in 0..10 {
            app.update();
            if !samples.lock().unwrap().is_empty() { break; }
        }
        let samples = samples.lock().unwrap();
        let (hit, layers, axes, position) = samples.last().copied().expect("Mantle physics did not run");
        assert!(hit, "The character capsule must remain queryable during Mantle physics");
        assert_eq!(layers, original_layers);
        assert_eq!(axes, LockedAxes::ALL_LOCKED.to_bits());
        assert!(app.world().get::<Position>(actor).unwrap().0.abs_diff_eq(position, 1e-4));
        assert_eq!(app.world().get::<LockedAxes>(actor).map(LockedAxes::to_bits), original_axes);
        assert_eq!(*app.world().get::<CollisionLayers>(actor).unwrap(), original_layers);
    }

    #[test]
    fn loaded_mantle_publishes_owner_velocity_through_physics_and_freeze() {
        use crate::actual_motion::AlsActualMotion;
        for (parented, moving_base) in [(false, false), (true, false), (true, true)] {
            for dilation in [0.5_f32, 1.0, 2.0] {
                let (mut app, actor, _) = crate::roll_drive::tests::loaded_roll_character();
                let origin = app.world().get::<Position>(actor).unwrap().0;
                if parented {
                    let parent_transform = Transform::from_xyz(5.0, 2.0, -3.0)
                        .with_rotation(Quat::from_euler(EulerRot::YXZ, 1.1, 0.15, -0.1)).with_scale(Vec3::splat(2.0));
                    let parent = app.world_mut().spawn((parent_transform, GlobalTransform::from(parent_transform))).id();
                    let local = GlobalTransform::from(*app.world().get::<Transform>(actor).unwrap())
                        .reparented_to(&GlobalTransform::from(parent_transform));
                    app.world_mut().entity_mut(actor).insert((local, ChildOf(parent)));
                }
                let target = origin + Vec3::new(0.0, 1.5, -0.8);
                let mut mantle = AlsMantleState::default();
                if moving_base {
                    let base = app.world_mut().spawn((avian3d::prelude::RigidBody::Kinematic,
                        Transform::from_translation(target), GlobalTransform::from_translation(target),
                        LinearVelocity(Vec3::X * 0.75))).id();
                    mantle.begin_with_target_base(MantleKind::High, origin, target, 1.5, 0.0, 0.0,
                        AlsMantleTargetBase { entity: base, translation: target, rotation: Quat::IDENTITY,
                            scale: Vec3::ONE, relative_location: true });
                } else {
                    mantle.begin(MantleKind::High, origin, target, 1.5, 0.0, 0.0);
                }
                app.world_mut().entity_mut(actor).insert((mantle, crate::als_camera::AlsCameraCustomTimeDilation(dilation)));
                let trace = std::sync::Arc::new(std::sync::Mutex::new([Vec3::ZERO; 3]));
                let before = trace.clone();
                app.add_systems(PostUpdate, (move |helper: bevy::transform::helper::TransformHelper| {
                    before.lock().unwrap()[0] = helper.compute_global_transform(actor).unwrap().translation();
                }).before(AlsMantleDriveSet));
                let after = trace.clone();
                app.add_systems(PostUpdate, (move |actors: Query<(&Position, &LinearVelocity)>| {
                    let (position, velocity) = actors.get(actor).unwrap();
                    let mut values = after.lock().unwrap();
                    values[1] = position.0;
                    values[2] = velocity.0;
                }).after(AlsMantleDriveSet).before(crate::source_physics_timing::SourcePhysicsTimingSet::Clamp));
                let mut samples = 0;
                for frame in 0..12 {
                    app.update();
                    let state = app.world().get::<AlsMantleState>(actor).unwrap();
                    if !state.active || !state.root_motion_ready { continue; }
                    let values = *trace.lock().unwrap();
                    let world_delta = app.world().resource::<Time<Virtual>>().delta_secs();
                    let expected = (values[1] - values[0]) / (world_delta * dilation);
                    assert!(values[2].abs_diff_eq(expected, 0.005), "Source RMS publishes owner velocity before Custom physics");
                    let actual = app.world().get::<AlsActualMotion>(actor).unwrap().world_velocity;
                    assert!(actual.abs_diff_eq(expected, 0.005),
                        "Source RMS velocity divides the consumed world delta by owner time: parented={parented} base={moving_base} dilation={dilation} frame={frame} actual={actual:?} expected={expected:?}");
                    samples += 1;
                }
                assert!(samples >= 6, "Loaded Mantle did not produce enough source velocity samples");
                let velocity = app.world().get::<AlsActualMotion>(actor).unwrap().world_velocity;
                let clip_time = app.world().get::<AlsMantleState>(actor).unwrap().time;
                app.world_mut().entity_mut(actor).insert(crate::als_camera::AlsCameraCustomTimeDilation(0.0));
                app.update();
                assert_eq!(app.world().get::<AlsMantleState>(actor).unwrap().time, clip_time);
                assert!(app.world().get::<AlsActualMotion>(actor).unwrap().world_velocity.abs_diff_eq(velocity, 0.005),
                    "Source zero-delta RMS clears its override and retains current velocity");
                let values = *trace.lock().unwrap();
                assert!(values[1].abs_diff_eq(values[0], 1e-5), "Frozen Mantle must not integrate its retained source velocity");
            }
        }
    }

    #[test]
    fn mantle_root_and_physics_keep_current_actor_parent_space() {
        let mut app = mantle_clip_test_app();
        let handle = app.world().resource::<MantleAssets>().clip(MantleKind::High).clone();
        for parent_transform in [Transform::IDENTITY, Transform::from_xyz(5.0, 2.0, -3.0),
            Transform::from_xyz(5.0, 2.0, -3.0).with_rotation(Quat::from_euler(EulerRot::YXZ, 1.1, 0.15, -0.1))
                .with_scale(Vec3::splat(2.0))] {
            let parent = app.world_mut().spawn((parent_transform, GlobalTransform::IDENTITY)).id();
            for (dilation, near_end) in [0.0_f32, 0.5, 1.0, 2.0].into_iter()
                .flat_map(|dilation| [false, true].map(move |near_end| (dilation, near_end))) {
                let clip = app.world().resource::<Assets<GraphClip>>().get(&handle).unwrap();
                let skeleton = app.world().resource::<Assets<Skeleton>>().get(&clip.skeleton).unwrap();
                let mut mantle = AlsMantleState::default();
                mantle.begin(MantleKind::High, Vec3::ZERO, Vec3::Y, 1.5, 0.0, 0.0);
                mantle.prepare_root_motion(sample_mantle_root(clip, skeleton, mantle.start_offset).unwrap(),
                    sample_mantle_root(clip, skeleton, clip.duration).unwrap());
                mantle.time = if near_end { mantle.action_end() - 0.075 } else { 0.2 };
                let before = mantle.target_actor_transform_at(mantle.time,
                    sample_mantle_root(clip, skeleton, mantle.clip_time()).unwrap());
                let next_time = (mantle.time + 0.05 * dilation).min(mantle.action_end());
                let expected = mantle.target_actor_transform_at(next_time,
                    sample_mantle_root(clip, skeleton, mantle.clip_time_at(next_time)).unwrap());
                let local = GlobalTransform::from(before).reparented_to(&GlobalTransform::from(parent_transform));
                let source_initial_velocity = Vec3::new(0.7, 0.2, -0.4);
                let actor = app.world_mut().spawn((AlsCharacter, mantle, LocomotionState::Custom,
                    LinearVelocity(source_initial_velocity), local, ChildOf(parent), GlobalTransform::IDENTITY,
                    Position(before.translation), Rotation(before.rotation), AlsCollisionLayerOwnership::default(),
                    crate::als_camera::AlsCameraCustomTimeDilation(dilation),
                    crate::als::character::AlsCmcAdapterRuntime { grounded_move_start: Some(before.translation),
                        grounded_collision_velocity: Some(source_initial_velocity), ..default() })).id();
                let before_world = app.world_mut().run_system_once(move |helper: bevy::transform::helper::TransformHelper| {
                    helper.compute_global_transform(actor).unwrap()
                }).unwrap();
                assert!(before_world.translation().distance(before.translation) < 1e-5);
                app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
                app.world_mut().run_system_once(mantle_drive_system).unwrap();
                let actual = app.world_mut().run_system_once(move |helper: bevy::transform::helper::TransformHelper| {
                    helper.compute_global_transform(actor).unwrap()
                }).unwrap();
                let current = app.world().get::<AlsMantleState>(actor).unwrap();
                assert!((current.time - next_time).abs() < 1e-6 && current.active == (next_time < current.action_end()));
                assert!(actual.translation().distance(expected.translation) < 1e-5,
                    "Mantle wrote a world location into actor local space: parent={parent_transform:?} actual={actual:?} expected={expected:?}");
                let rotation = actual.rotation();
                assert!(rotation.abs_diff_eq(expected.rotation, 1e-5) || rotation.abs_diff_eq(-expected.rotation, 1e-5));
                assert!(app.world().get::<Transform>(actor).unwrap().scale.abs_diff_eq(local.scale, 1e-6));
                assert!(app.world().get::<Position>(actor).unwrap().0.distance(expected.translation) < 1e-5);
                assert!(app.world().get::<Rotation>(actor).unwrap().0.abs_diff_eq(expected.rotation, 1e-5));
                let expected_velocity = if dilation == 0.0 { source_initial_velocity }
                    else { (expected.translation - before_world.translation()) / (0.05 * dilation) };
                let actual_velocity = app.world().get::<LinearVelocity>(actor).unwrap().0;
                assert!(actual_velocity.distance(expected_velocity) < 1e-5,
                    "Source RMS must use the consumed actor world location and owner clock: parent={parent_transform:?} dilation={dilation} near_end={near_end} actual={actual_velocity:?} expected={expected_velocity:?}");
                if !current.active {
                    app.world_mut().entity_mut(actor).insert(AlsNotifyLocomotionAction::None);
                    app.world_mut().run_system_once(mantle_pose_system).unwrap();
                    app.world_mut().run_system_once(release_mantle_physics).unwrap();
                    assert_eq!(app.world().get::<LocomotionState>(actor), Some(&LocomotionState::Grounded));
                    assert!(app.world().get::<LinearVelocity>(actor).unwrap().0.abs_diff_eq(actual_velocity.with_y(0.0), 1e-6),
                        "Source Walking handoff must preserve world horizontal root velocity");
                    assert_eq!(app.world().get::<Position>(actor).unwrap().0, expected.translation);
                    assert!(app.world().get::<CustomPositionIntegration>(actor).is_none());
                    assert!(app.world().get::<CustomVelocityIntegration>(actor).is_none());
                }
                app.world_mut().despawn(actor);
            }
            app.world_mut().despawn(parent);
        }
    }

    #[test]
    fn mantle_moving_base_writes_the_current_actor_parent_space() {
        let mut app = mantle_clip_test_app();
        for parent_transform in [Transform::IDENTITY, Transform::from_xyz(5.0, 2.0, -3.0)
            .with_rotation(Quat::from_rotation_y(1.1)).with_scale(Vec3::splat(2.0))] {
            let parent = app.world_mut().spawn((parent_transform, GlobalTransform::IDENTITY)).id();
            let old_base = Transform::from_xyz(0.0, 2.0, 0.0);
            let latest_base = old_base.with_translation(Vec3::new(0.4, 2.3, -0.2)).with_rotation(Quat::from_rotation_y(0.3));
            let base = app.world_mut().spawn((latest_base, GlobalTransform::from(old_base))).id();
            let mut mantle = AlsMantleState::default();
            mantle.begin_with_target_base(MantleKind::High, Vec3::new(0.0, 1.0, 1.0), Vec3::new(0.0, 2.5, 0.0),
                1.5, 0.0, 0.0, AlsMantleTargetBase { entity: base, translation: old_base.translation,
                    rotation: old_base.rotation, scale: old_base.scale, relative_location: true });
            let clip = app.world().resource::<Assets<GraphClip>>().get(app.world().resource::<MantleAssets>().clip(MantleKind::High)).unwrap();
            let skeleton = app.world().resource::<Assets<Skeleton>>().get(&clip.skeleton).unwrap();
            mantle.prepare_root_motion(sample_mantle_root(clip, skeleton, mantle.start_offset).unwrap(),
                sample_mantle_root(clip, skeleton, clip.duration).unwrap());
            mantle.time = 0.4;
            let root = sample_mantle_root(clip, skeleton, mantle.clip_time()).unwrap();
            let before = mantle.target_actor_transform_at(mantle.time, root);
            let mut reference = mantle.clone();
            reference.refresh_target_base(Some(AlsMantleTargetBase { entity: base, translation: latest_base.translation,
                rotation: latest_base.rotation, scale: latest_base.scale, relative_location: true }));
            let expected = reference.target_actor_transform_at(reference.time, root);
            let local = GlobalTransform::from(before).reparented_to(&GlobalTransform::from(parent_transform));
            let actor = app.world_mut().spawn((AlsCharacter, mantle, local, ChildOf(parent), GlobalTransform::IDENTITY,
                Position(before.translation), Rotation(before.rotation), AlsMantlePhysicsFrame { custom_position: false, custom_velocity: false, locked_axes: None })).id();
            app.world_mut().run_system_once(update_mantle_movement_base).unwrap();
            let actual = app.world_mut().run_system_once(move |helper: bevy::transform::helper::TransformHelper| {
                helper.compute_global_transform(actor).unwrap()
            }).unwrap();
            assert!(actual.translation().distance(expected.translation) < 1e-5,
                "Moving Mantle base must write the target world pose into actor parent space: actual={actual:?} expected={expected:?}");
            assert!(actual.rotation().abs_diff_eq(expected.rotation, 1e-5) || actual.rotation().abs_diff_eq(-expected.rotation, 1e-5));
            assert!(app.world().get::<Transform>(actor).unwrap().scale.abs_diff_eq(local.scale, 1e-6));
            assert!(app.world().get::<Position>(actor).unwrap().0.distance(expected.translation) < 1e-5);
            assert!(app.world().get::<Rotation>(actor).unwrap().0.abs_diff_eq(expected.rotation, 1e-5));
            app.world_mut().despawn(actor);
            app.world_mut().despawn(base);
            app.world_mut().despawn(parent);
        }
    }

    #[test]
    fn fresh_audit_mantle_root_moves_and_finishes_on_owner_time() {
        let mut app = mantle_clip_test_app();
        let handle = app.world().resource::<MantleAssets>().clip(MantleKind::High).clone();
        let mut failures = Vec::new();
        for (dilation, near_end) in [0.0_f32, 0.5, 1.0, 2.0].into_iter()
            .flat_map(|dilation| [false, true].map(move |near_end| (dilation, near_end))) {
            let clip = app.world().resource::<Assets<GraphClip>>().get(&handle).unwrap();
            let skeleton = app.world().resource::<Assets<Skeleton>>().get(&clip.skeleton).unwrap();
            let mut mantle = AlsMantleState::default();
            mantle.begin(MantleKind::High, Vec3::ZERO, Vec3::Y, 1.5, 0.0, 0.0);
            mantle.prepare_root_motion(sample_mantle_root(clip, skeleton, mantle.start_offset).unwrap(),
                sample_mantle_root(clip, skeleton, clip.duration).unwrap());
            mantle.time = if near_end { mantle.action_end() - 0.075 } else { 0.2 };
            let before = mantle.target_actor_transform_at(mantle.time,
                sample_mantle_root(clip, skeleton, mantle.clip_time()).unwrap());
            let expected_time = (mantle.time + 0.05 * dilation).min(mantle.action_end());
            let expected = mantle.target_actor_transform_at(expected_time,
                sample_mantle_root(clip, skeleton, mantle.clip_time_at(expected_time)).unwrap());
            let expected_active = expected_time < mantle.action_end();
            let actor = app.world_mut().spawn((AlsCharacter, mantle, LocomotionState::Custom,
                LinearVelocity::ZERO, before, Position(before.translation), Rotation(before.rotation),
                AlsCollisionLayerOwnership::default(), crate::als_camera::AlsCameraCustomTimeDilation(dilation))).id();
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
            app.world_mut().run_system_once(mantle_drive_system).unwrap();
            let current = app.world().get::<AlsMantleState>(actor).unwrap();
            let actual = app.world().get::<Transform>(actor).unwrap();
            let expected_velocity = if dilation == 0.0 { Vec3::ZERO }
                else { (expected.translation - before.translation) / (0.05 * dilation) };
            if (current.time - expected_time).abs() > 1e-6 || current.active != expected_active
                || actual.translation.distance(expected.translation) > 1e-5
                || app.world().get::<Position>(actor).unwrap().0.distance(expected.translation) > 1e-5
                || app.world().get::<LinearVelocity>(actor).unwrap().0.distance(expected_velocity) > 1e-5 {
                failures.push((dilation, near_end, current.time, expected_time, current.active, actual.translation, expected.translation));
            }
            app.world_mut().despawn(actor);
        }
        assert!(failures.is_empty(), "Mantle root time, final pose, or completion ignored owner time: {failures:?}");
    }

    #[test]
    fn fresh_audit_mantle_zero_clock_entry_initializes_only_once_before_resume() {
        let mut app = mantle_clip_test_app();
        let mut mantle = AlsMantleState::default();
        mantle.begin(MantleKind::High, Vec3::ZERO, Vec3::Y, 1.5, 0.0, 0.0);
        let source_velocity = Vec3::new(0.7, 0.2, -0.4);
        let actor = app.world_mut().spawn((AlsCharacter, mantle, LocomotionState::Grounded,
            LinearVelocity(source_velocity), Transform::IDENTITY, GlobalTransform::IDENTITY,
            Position(Vec3::ZERO), Rotation(Quat::IDENTITY), AlsCollisionLayerOwnership::default(),
            crate::als_camera::AlsCameraCustomTimeDilation(0.0), crate::als::character::AlsCmcAdapterRuntime::default())).id();
        let mut identities = Vec::new();
        for _ in 0..3 {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(0.05));
            app.world_mut().run_system_once(prepare_mantle_montages).unwrap();
            app.world_mut().run_system_once(mantle_drive_system).unwrap();
            let montage = &app.world().get::<AlsMantleMontages>(actor).unwrap().entries[0];
            identities.push(montage.id);
            assert_eq!(montage.elapsed, 0.0);
            assert_eq!(app.world().get::<AlsMantleState>(actor).unwrap().time, 0.0);
            assert_eq!(app.world().get::<Transform>(actor).unwrap().translation, Vec3::ZERO);
            assert!(app.world().get::<LinearVelocity>(actor).unwrap().0.abs_diff_eq(source_velocity, 1e-6),
                "Source zero-delta RMS entry must retain current velocity before its first root preparation");
        }
        let prepared = app.world().get::<AlsMantleState>(actor).unwrap().root_motion_ready;
        app.world_mut().entity_mut(actor).insert(crate::als_camera::AlsCameraCustomTimeDilation(0.5));
        app.world_mut().run_system_once(prepare_mantle_montages).unwrap();
        app.world_mut().run_system_once(mantle_drive_system).unwrap();
        let current = app.world().get::<AlsMantleState>(actor).unwrap();
        let montage = &app.world().get::<AlsMantleMontages>(actor).unwrap().entries[0];
        assert!(prepared && identities == [1, 1, 1] && montage.id == 1 && (current.time - 0.025).abs() < 1e-6,
            "Zero-clock Mantle entry restarted before resume: prepared={prepared} ids={identities:?} current_id={} root_time={}",
            montage.id, current.time);
    }

    fn mantle_clip_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            crate::extensions::SourceAnimationInputsPlugin,
            AssetPlugin::default(),
            bevy::gltf::GltfPlugin::default(),
            TransformPlugin,
            bevy::gizmos::GizmoPlugin,
            bevy_animation_graph::AnimationGraphPlugin::default(),
            avian3d::prelude::PhysicsPlugins::new(PostUpdate),
            AlsMantleDrivePlugin,
            crate::source_physics_timing::SourcePhysicsTimingPlugin,
            bevy_tnua_avian3d::TnuaAvian3dPlugin::new(Update),
            bevy_tnua::TnuaControllerPlugin::<crate::als::control_scheme::AlsControlScheme>::new(Update),
            crate::als::transition::AlsStateTransitionPlugin,
            crate::als::character::AlsCharacterPlugin,
            crate::als::motion::AlsMotionPlugin,
        ))
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_asset::<bevy::animation::AnimationClip>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_message::<crate::als::transition::StateTransitionEvent>()
        .add_message::<crate::animation_events::AlsAnimationEvent>();
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
        app
    }

    #[test]
    fn mantle_completion_keeps_the_current_moving_support() {
        use avian3d::prelude::*;
        use crate::als::{character::AlsCharacterBundle, control_scheme::AlsControlSchemeConfig, params::AlsMovementParams};
        let mut app = mantle_clip_test_app();
        let mut failures = Vec::new();
        for (child, delta) in [false, true].into_iter().flat_map(|child| [1.0 / 60.0, 0.05_f32].map(move |delta| (child, delta))) {
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
            let center = Vec3::Y * 2.0;
            let body = app.world_mut().spawn((RigidBody::Kinematic, Transform::from_translation(center), GlobalTransform::from_translation(center))).id();
            let target = if child {
                app.world_mut().spawn((ChildOf(body), Collider::cuboid(4.0, 0.5, 4.0), Transform::IDENTITY, GlobalTransform::from_translation(center))).id()
            } else {
                app.world_mut().entity_mut(body).insert(Collider::cuboid(4.0, 0.5, 4.0));
                body
            };
            let params = AlsMovementParams::default();
            let config = app.world_mut().resource_mut::<Assets<AlsControlSchemeConfig>>().add(crate::als::motion::initial_als_control_scheme_config(&params));
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(params);
            let start = Vec3::new(0.0, 2.424, 1.5);
            let mut actor_bundle = AlsCharacterBundle::new(params, config);
            actor_bundle.transform.translation = start;
            let actor = app.world_mut().spawn(actor_bundle).id();
            let mut mantle = AlsMantleState::default();
            mantle.begin_with_target_base(MantleKind::Low, start, Vec3::new(0.0, 3.174, 0.0), 0.75, 0.0, 0.0,
                AlsMantleTargetBase { entity:target, translation:center, rotation:Quat::IDENTITY, scale:Vec3::ONE, relative_location:true });
            app.world_mut().entity_mut(actor).insert(mantle);
            app.update();
            app.world_mut().get_mut::<LinearVelocity>(body).unwrap().0 = Vec3::X;
            for _ in 0..240 {
                app.update();
                if !app.world().get::<AlsMantleState>(actor).unwrap().active { break; }
            }
            assert!(!app.world().get::<AlsMantleState>(actor).unwrap().active);
            app.update();
            let offset = app.world().get::<Position>(actor).unwrap().0 - app.world().get::<Position>(body).unwrap().0;
            app.world_mut().entity_mut(actor).insert(AlsNotifyLocomotionAction::None);
            for frame in 0..20 {
                app.update();
                let actual = app.world().get::<Position>(actor).unwrap().0 - app.world().get::<Position>(body).unwrap().0;
                let error = (actual - offset).with_y(0.0).length();
                let mode = *app.world().get::<LocomotionState>(actor).unwrap();
                let base = app.world().get::<crate::als::dynamics::AlsMovementBaseDynamicsState>(actor).and_then(|base| base.entity);
                if error > 1e-4 || mode != LocomotionState::Grounded || base != Some(target) {
                    failures.push((child, delta, frame, error, mode, base == Some(target)));
                    break;
                }
            }
            app.world_mut().despawn(actor);
            app.world_mut().despawn(body);
        }
        assert!(failures.is_empty(), "Mantle lost the current movement base at completion: {failures:?}");
    }

    #[test]
    fn moving_mantle_targets_are_sampled_after_their_current_frame_motion() {
        use avian3d::prelude::*;
        use bevy::time::TimeUpdateStrategy;
        let mut app = mantle_clip_test_app();
        let mut failures = Vec::new();
        for child in [false, true] {
            for (linear, angular) in [(Vec3::X, Vec3::ZERO), (Vec3::ZERO, Vec3::Y)] {
                let transform = Transform::from_xyz(0.0, 4.0, 0.0);
                let body = app.world_mut().spawn((RigidBody::Kinematic, transform, GlobalTransform::from(transform),
                    LinearVelocity(linear), AngularVelocity(angular))).id();
                let target = if child {
                    app.world_mut().spawn((ChildOf(body), Transform::from_xyz(0.2, 0.0, 0.0),
                        GlobalTransform::from(Transform::from_xyz(0.2, 4.0, 0.0)), Collider::cuboid(2.0, 0.4, 2.0))).id()
                } else {
                    app.world_mut().entity_mut(body).insert(Collider::cuboid(2.0, 0.4, 2.0));
                    body
                };
                let base_transform = *app.world().get::<GlobalTransform>(target).unwrap();
                let start = Vec3::new(-0.5, 4.9, 1.5);
                let mut mantle = AlsMantleState::default();
                mantle.begin_with_target_base(MantleKind::Low, start, Vec3::new(0.0, 5.2, 0.0), 0.75, 0.0, 0.0,
                    AlsMantleTargetBase { entity:target, translation:base_transform.translation(), rotation:base_transform.rotation(),
                        scale:base_transform.scale(), relative_location:true });
                let clip_handle = app.world().resource::<MantleAssets>().clip(MantleKind::Low).clone();
                let clip = app.world().resource::<Assets<GraphClip>>().get(&clip_handle).unwrap();
                let skeleton = app.world().resource::<Assets<Skeleton>>().get(&clip.skeleton).unwrap();
                let mut reference = mantle.clone();
                reference.prepare_root_motion(sample_mantle_root(clip, skeleton, mantle.start_offset).unwrap(),
                    sample_mantle_root(clip, skeleton, clip.duration).unwrap());
                let actor = app.world_mut().spawn((AlsCharacter, mantle, LocomotionState::Grounded,
                    RigidBody::Dynamic, Transform::from_translation(start), GlobalTransform::from(Transform::from_translation(start)),
                    Collider::sphere(0.3))).id();
                let mut maximum = 0.0_f32;
                let mut elapsed = 0.0_f32;
                for frame in 0..120 {
                    let delta = [1.0/60.0, 0.05][frame % 2];
                    elapsed += delta;
                    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(delta)));
                    app.update();
                    let current = app.world().get::<AlsMantleState>(actor).unwrap();
                    assert!((current.time - elapsed.min(current.action_end())).abs() < 1e-5);
                    let latest = app.world().get::<GlobalTransform>(target).unwrap();
                    reference.refresh_target_base(Some(AlsMantleTargetBase { entity:target, translation:latest.translation(),
                        rotation:latest.rotation(), scale:latest.scale(), relative_location:true }));
                    let clip = app.world().resource::<Assets<GraphClip>>().get(&clip_handle).unwrap();
                    let skeleton = app.world().resource::<Assets<Skeleton>>().get(&clip.skeleton).unwrap();
                    let root = sample_mantle_root(clip, skeleton, current.clip_time()).unwrap();
                    let expected = reference.target_actor_transform_at(current.time, root);
                    let actual = app.world().get::<Transform>(actor).unwrap();
                    maximum = maximum.max(actual.translation.distance(expected.translation));
                    assert!(actual.rotation.angle_between(expected.rotation) < 1e-3);
                    if !current.active { break; }
                }
                assert!(!app.world().get::<AlsMantleState>(actor).unwrap().active);
                if maximum > 1e-4 { failures.push((child, linear, angular, maximum)); }
                app.world_mut().entity_mut(actor).insert(AlsNotifyLocomotionAction::None);
                app.update();
                assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), LocomotionState::Grounded);
                assert!(app.world().get::<CustomPositionIntegration>(actor).is_none());
                assert!(app.world().get::<CustomVelocityIntegration>(actor).is_none());
                app.world_mut().despawn(actor);
                app.world_mut().despawn(body);
            }
        }
        assert!(failures.is_empty(), "Mantle used an old movement-base transform: {failures:?}");
    }

    #[test]
    fn mantle_slot_follows_current_assembly_and_cleans_only_its_old_player() {
        use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
        use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;
        use crate::character_assembly::AlsCharacterAssembly;
        let mut app = mantle_clip_test_app();
        let clip = app.world().resource::<MantleAssets>().clip(MantleKind::Low).clone();
        let skeleton = app.world().resource::<Assets<GraphClip>>().get(&clip).unwrap().skeleton.clone();
        for current_first in [false, true] {
            let actor = app.world_mut().spawn((AlsCharacter, AlsMantleMontages {
                entries: vec![AlsMantleMontage { id: 1, kind: MantleKind::Low, elapsed: 0.2,
                    clip_time: 0.6, previous_clip_time: 0.5, weight: 1.0, pose_ready: false, interrupted: false, stop: None }],
                ..default()
            })).id();
            let spawn_avatar = |app: &mut App| {
                let avatar = app.world_mut().spawn((AlsCharacterAvatar { character: actor },
                    AnimatedSceneHandle::default(), Transform::IDENTITY, ChildOf(actor))).id();
                let player = app.world_mut().spawn((AnimationGraphPlayer::new(skeleton.clone()),
                    Transform::IDENTITY, ChildOf(avatar))).id();
                app.world_mut().trigger(WorldInstanceReady { entity: avatar,
                    instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()) });
                app.world_mut().flush();
                (avatar, player)
            };
            let pair = [spawn_avatar(&mut app), spawn_avatar(&mut app)];
            let current = pair[usize::from(!current_first)];
            let old = pair[usize::from(current_first)];
            app.world_mut().entity_mut(actor).insert(AlsCharacterAssembly { avatar: current.0, camera: None });
            let unrelated = app.world_mut().spawn(AnimationGraphPlayer::new(skeleton.clone())).id();
            app.world_mut().get_mut::<AnimationGraphPlayer>(unrelated).unwrap().set_global_input_data(POST_LOCOMOTION_WEIGHT, 0.37_f32);
            app.world_mut().run_system_cached(publish_mantle_slot).unwrap();
            assert_eq!(app.world().get::<AnimationGraphPlayer>(current.1).unwrap()
                .get_global_input_data(POST_LOCOMOTION_WEIGHT).and_then(|v| v.as_f32().ok()), Some(1.0),
                "The current Assembly player must receive the mantle slot in both creation orders");
            assert_eq!(app.world().get::<AnimationGraphPlayer>(current.1).unwrap()
                .get_global_input_data(POST_LOCOMOTION_POSE).unwrap().as_pose().unwrap().paths.len(), 79);
            assert!(app.world().get::<AnimationGraphPlayer>(old.1).unwrap().get_global_input_data(POST_LOCOMOTION_WEIGHT).is_none());
            assert!(app.world().get::<crate::extensions::SourceAnimationInputs>(current.1).unwrap().slot_input_context(POST_LOCOMOTION_POSE).is_some());
            app.world_mut().get_mut::<AnimationGraphPlayer>(old.1).unwrap().pause();
            app.world_mut().get_mut::<AlsCharacterAssembly>(actor).unwrap().avatar = old.0;
            app.world_mut().get_mut::<AlsMantleMontages>(actor).unwrap().entries[0].weight = 0.5;
            app.world_mut().run_system_cached(publish_mantle_slot).unwrap();
            assert_eq!(app.world().get::<AnimationGraphPlayer>(current.1).unwrap()
                .get_global_input_data(POST_LOCOMOTION_WEIGHT).and_then(|v| v.as_f32().ok()), Some(0.0));
            assert!(app.world().get::<AnimationGraphPlayer>(current.1).unwrap()
                .get_global_input_data(POST_LOCOMOTION_POSE).unwrap().as_pose().unwrap().paths.is_empty());
            assert_eq!(app.world().get::<AnimationGraphPlayer>(current.1).unwrap()
                .get_global_input_data(POST_LOCOMOTION_LAYER_WEIGHT).and_then(|v| v.as_f32().ok()), Some(0.0));
            assert!(app.world().get::<crate::extensions::SourceAnimationInputs>(current.1).unwrap().slot_input_context(POST_LOCOMOTION_POSE).is_none());
            assert_eq!(app.world().get::<AnimationGraphPlayer>(old.1).unwrap()
                .get_global_input_data(POST_LOCOMOTION_WEIGHT).and_then(|v| v.as_f32().ok()), Some(0.5));
            app.world_mut().entity_mut(old.1).remove::<ChildOf>();
            let rebuilt = app.world_mut().spawn((AnimationGraphPlayer::new(skeleton.clone()), Transform::IDENTITY, ChildOf(old.0))).id();
            app.world_mut().trigger(WorldInstanceReady { entity: old.0,
                instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()) });
            app.world_mut().flush();
            app.world_mut().run_system_cached(publish_mantle_slot).unwrap();
            assert_eq!(app.world().get::<AnimationGraphPlayer>(rebuilt).unwrap()
                .get_global_input_data(POST_LOCOMOTION_WEIGHT).and_then(|v| v.as_f32().ok()), Some(0.5));
            assert_eq!(app.world().get::<AnimationGraphPlayer>(old.1).unwrap()
                .get_global_input_data(POST_LOCOMOTION_WEIGHT).and_then(|v| v.as_f32().ok()), Some(0.0));
            assert!(app.world().get::<crate::extensions::SourceAnimationInputs>(old.1).unwrap().slot_input_context(POST_LOCOMOTION_POSE).is_none());
            let foreign = app.world_mut().spawn((AlsCharacter, AlsCharacterAssembly { avatar: old.0, camera: None })).id();
            app.world_mut().get_mut::<AlsCharacterAvatar>(old.0).unwrap().character = foreign;
            app.world_mut().get_mut::<AnimationGraphPlayer>(rebuilt).unwrap().set_global_input_data(POST_LOCOMOTION_WEIGHT, 0.37_f32);
            app.world_mut().run_system_cached(publish_mantle_slot).unwrap();
            assert_eq!(app.world().get::<AnimationGraphPlayer>(rebuilt).unwrap()
                .get_global_input_data(POST_LOCOMOTION_WEIGHT).and_then(|v| v.as_f32().ok()), Some(0.37));
            assert_eq!(app.world().get::<AnimationGraphPlayer>(unrelated).unwrap()
                .get_global_input_data(POST_LOCOMOTION_WEIGHT).and_then(|v| v.as_f32().ok()), Some(0.37));
            app.world_mut().despawn(actor);
            app.world_mut().despawn(foreign);
            app.world_mut().despawn(old.1);
            app.world_mut().despawn(unrelated);
        }
    }

    #[test]
    fn mantle_absolute_root_drives_actual_clips_to_source_endpoints() {
        let mut app = mantle_clip_test_app();
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
                        crate::extensions::profile_blend::AlphaBlendMode::Cubic
                    } else {
                        crate::extensions::profile_blend::AlphaBlendMode::HermiteCubic
                    }
                );
                let (pose, context) =
                    sample_mantle_pose(clip, skeleton, settings, clip.duration).unwrap();
                let root_id =
                    crate::extensions::root_motion_defaults::derive_default_root_bone(clip, skeleton)
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
                    assert_eq!(low.curves[&crate::extensions::SourceCurveName::from("LayerArmRight")], -1.0);
                    assert!(!right.curves.contains_key(&crate::extensions::SourceCurveName::from("LayerArmRight")));
                    assert_eq!(low.curves[&crate::extensions::SourceCurveName::from("PoseStanding")], 1.0);
                    let mut controlled: MantleSourceSettings =
                        ron::from_str(crate::als::mantle::MANTLE_SOURCE_DATA).unwrap();
                    let controlled = &mut controlled.montages[kind.action_settings().montage_index];
                    controlled.sequence_curves.0.insert(
                        "LayerArmRight".into(),
                        crate::extensions::als_curves::AlsFloatCurve {
                            times: vec![0.0],
                            values: vec![0.5],
                            interps: vec![crate::extensions::als_curves::AlsKeyInterp::Linear],
                            ..default()
                        },
                    );
                    let (_, overridden) =
                        sample_mantle_pose(clip, skeleton, controlled, 0.8).unwrap();
                    assert_eq!(overridden.curves[&crate::extensions::SourceCurveName::from("LayerArmRight")], -1.0);
                    assert_eq!(overridden.curves[&crate::extensions::SourceCurveName::from("PoseStanding")], 1.0);
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
                    crate::als::state::LocomotionState::Grounded,
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
            app.world_mut().entity_mut(entity).insert(crate::character_assembly::AlsCharacterAssembly { avatar, camera: None });
            app.world_mut().entity_mut(entity).insert((
                crate::als::transition::AlsStateRequest::default(),
                crate::als::state::Stance::Standing,
                crate::als::state::Gait::Walking,
                crate::als::state::RotationMode::ViewDirection,
                crate::als::state::OverlayState::Default,
            ));
            let delta = 1.0 / 60.0;
            for _ in 0..=(duration / delta).ceil() as usize {
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_secs_f32(delta));
                app.world_mut()
                    .run_system_once(prepare_mantle_montages)
                    .unwrap();
                assert_eq!(
                    app.world().get::<crate::als::state::LocomotionState>(entity),
                    Some(&crate::als::state::LocomotionState::Custom)
                );
                let owner = app.world().get::<AlsMantleState>(entity).unwrap();
                if owner.active || owner.root_motion_ready {
                    for requested in [LocomotionState::Grounded, LocomotionState::Air] {
                        app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(entity).unwrap().locomotion_state = Some(requested);
                        app.world_mut().run_schedule(Update);
                        assert_eq!(*app.world().get::<LocomotionState>(entity).unwrap(), LocomotionState::Custom,
                            "Source active Mantle owns the mode lock: kind={kind:?} requested={requested:?}");
                        assert!(app.world().get::<crate::als::transition::AlsStateRequest>(entity).unwrap().locomotion_state.is_none());
                    }
                }
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
                let context = app.world().get::<crate::extensions::SourceAnimationInputs>(player_entity).unwrap().slot_input_context(POST_LOCOMOTION_POSE).unwrap();
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
                let context = app.world().get::<crate::extensions::SourceAnimationInputs>(player_entity).unwrap().slot_input_context(POST_LOCOMOTION_POSE).unwrap();
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
            assert!(app.world().get::<crate::extensions::SourceAnimationInputs>(player_entity).unwrap().slot_input_context(POST_LOCOMOTION_POSE).is_none());
            app.world_mut().despawn(entity);
        }
        assert_eq!(MANTLE_SETTINGS.stop_blend_out_seconds, 0.3);
        assert!(MANTLE_SETTINGS.start_ragdoll_on_target_destruction);
        let mut pending = AlsMantleState::default();
        pending.begin(MantleKind::Low, Vec3::ZERO, Vec3::Y, 0.75, 0.0, 0.0);
        let actor = app
            .world_mut()
            .spawn((
                AlsCharacter,
                pending,
                LocomotionState::None,
                AlsNotifyLocomotionAction::Ragdolling,
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.05));
        app.world_mut()
            .run_system_once(prepare_mantle_montages)
            .unwrap();
        assert_eq!(
            app.world().get::<LocomotionState>(actor),
            Some(&LocomotionState::None)
        );
        assert!(app.world().get::<AlsMantleMontages>(actor).is_none());
        app.world_mut().despawn(actor);
        let missing = app.world_mut().spawn_empty().id();
        app.world_mut().despawn(missing);
        pending.active = false;
        pending.target_base = Some(AlsMantleTargetBase {
            entity: missing,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            relative_location: false,
        });
        for mode in [LocomotionState::Grounded, LocomotionState::None] {
            let actor = app
                .world_mut()
                .spawn((
                    AlsCharacter,
                    pending,
                    mode,
                    Transform::IDENTITY,
                    LinearVelocity(Vec3::X),
                ))
                .id();
            app.world_mut()
                .run_system_once(mantle_drive_system)
                .unwrap();
            assert_eq!(app.world().get::<LinearVelocity>(actor).unwrap().0, Vec3::X);
            app.world_mut().despawn(actor);
        }
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
                    LocomotionState::Grounded,
                    if interrupt == 2 {
                        AlsNotifyLocomotionAction::Ragdolling
                    } else {
                        AlsNotifyLocomotionAction::None
                    },
                ))
                .id();
            if interrupt == 0 {
                app.world_mut()
                    .entity_mut(actor)
                    .insert(crate::als::roll::AlsRollState {
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
                crate::als::state::LocomotionState::Grounded,
                crate::get_up::AlsGetUpState::new(false),
                crate::als::roll::AlsRollState {
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
        let settings = crate::get_up::get_up_settings(get_up.facing_up);
        get_up.advance(0.1, settings);
        assert!((get_up.weight - 0.5).abs() < 1e-5);
    }
}
