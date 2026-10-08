use crate::als::dynamics::{
    AlsGroundedDynamics, AlsLandingBrakingFriction,
    predict_grounded_stop_distance_with_braking_friction,
};
use crate::als::input::AlsInputCommand;
use crate::als::motion::{
    AlsActualVelocity, AlsInputOverride, AlsRotationResponseState, AlsViewYaw, view_relative_move_direction,
};
use crate::als::orientation::world_forward_to_yaw;
use crate::als::params::AlsMovementParams;
use crate::als::start_stop::predict_stop_distance;
use crate::als::state::{LocomotionState, RotationMode, Stance};
use crate::als::transitions::{TransitionConfig, select_quick_stop};
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::message::{Message, MessageReader};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

use crate::character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar};

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsStopMatch {
    pub active: bool,
    pub remaining: f32,
    pub total: f32,
    pub left_weight: f32,
    pub right_weight: f32,
    pub weight: f32,
    pub last_input_move: f32,
    pub last_input_yaw: Option<f32>,
    pub quick_stop_play_rate: f32,
    pub quick_stop_start_time: f32,
    pub quick_stop_blend_in_duration: f32,
    pub quick_stop_blend_out_duration: f32,
    pub quick_stop_remaining_yaw_degrees: f32,
}

impl Default for AlsStopMatch {
    fn default() -> Self {
        Self {
            active: false,
            remaining: 0.0,
            total: 1.0,
            left_weight: 0.0,
            right_weight: 0.0,
            weight: 0.0,
            last_input_move: 0.0,
            last_input_yaw: None,
            quick_stop_play_rate: 1.75,
            quick_stop_start_time: 0.3,
            quick_stop_blend_in_duration: 0.1,
            quick_stop_blend_out_duration: 0.2,
            quick_stop_remaining_yaw_degrees: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsTransitionKind {
    Stop,
    QuickStop,
    StandingTransition,
    StandingDynamic,
    CrouchingStop,
    CrouchingQuickStop,
    CrouchingDynamic,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsTransitionPlayback {
    pub instance_id: u64,
    pub active: bool,
    pub kind: AlsTransitionKind,
    pub elapsed: f32,
    pub clip_time: f32,
    pub previous_clip_time: f32,
    pub index: f32,
    pub play_rate: f32,
    pub blend_in: f32,
    pub blend_out: f32,
    pub start_time: f32,
    pub duration: f32,
    pub weight: f32,
    pub stop_elapsed: Option<f32>,
    pub stop_start_weight: f32,
    pub stop_duration: f32,
}

impl AlsTransitionPlayback {
    fn new(
        kind: AlsTransitionKind,
        index: f32,
        blend_in: f32,
        blend_out: f32,
        play_rate: f32,
        start_time: f32,
        duration: f32,
    ) -> Self {
        static NEXT_INSTANCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            instance_id: NEXT_INSTANCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            active: true,
            kind,
            elapsed: 0.0,
            clip_time: start_time,
            previous_clip_time: start_time,
            index,
            play_rate,
            blend_in,
            blend_out,
            start_time,
            duration,
            weight: if blend_in <= 0.0 { 1.0 } else { 0.0 },
            stop_elapsed: None,
            stop_start_weight: 0.0,
            stop_duration: 0.0,
        }
    }

    pub fn stop(index: f32) -> Self {
        Self::new(
            AlsTransitionKind::Stop,
            index,
            STOP_TRANSITION_BLEND_IN,
            STOP_TRANSITION_BLEND_OUT,
            STOP_TRANSITION_PLAY_RATE,
            STOP_TRANSITION_START_TIME,
            TRANSITION_SEQUENCE_DURATION,
        )
    }

    pub fn quick_stop(index: f32, play_rate: f32) -> Self {
        Self::new(
            AlsTransitionKind::QuickStop,
            index,
            QUICK_STOP_BLEND_IN,
            QUICK_STOP_BLEND_OUT,
            play_rate,
            QUICK_STOP_START_TIME,
            TRANSITION_SEQUENCE_DURATION,
        )
    }

    pub fn standing_transition_right() -> Self {
        Self::new(
            AlsTransitionKind::StandingTransition,
            6.0,
            STOP_TRANSITION_BLEND_IN,
            STOP_TRANSITION_BLEND_OUT,
            STOP_TRANSITION_PLAY_RATE,
            RIGHT_TRANSITION_START_TIME,
            TRANSITION_SEQUENCE_DURATION,
        )
    }

    pub fn crouching_stop_right() -> Self {
        Self::new(
            AlsTransitionKind::CrouchingStop,
            1.0,
            STOP_TRANSITION_BLEND_IN,
            STOP_TRANSITION_BLEND_OUT,
            STOP_TRANSITION_PLAY_RATE,
            RIGHT_TRANSITION_START_TIME,
            TRANSITION_SEQUENCE_DURATION,
        )
    }

    pub fn transition_animation(crouching: bool, left: bool, blend_in: f32, blend_out: f32, play_rate: f32, start_time: f32) -> Self {
        let mut playback = Self::new(
            if crouching { AlsTransitionKind::CrouchingStop } else { AlsTransitionKind::StandingTransition },
            if left { 0.0 } else { 1.0 }, blend_in, blend_out, play_rate, start_time, TRANSITION_SEQUENCE_DURATION,
        );
        playback.duration = crate::standing_source::MONTAGES[source_transition_sequence(&playback).unwrap()].duration;
        playback
    }

    pub fn roll_to_grounded_right(crouching: bool) -> Self {
        if crouching {
            Self::crouching_stop_right()
        } else {
            Self::standing_transition_right()
        }
    }

    pub fn crouching_quick_stop(index: f32, play_rate: f32) -> Self {
        Self::new(
            AlsTransitionKind::CrouchingQuickStop,
            index,
            QUICK_STOP_BLEND_IN,
            QUICK_STOP_BLEND_OUT,
            play_rate,
            QUICK_STOP_START_TIME,
            TRANSITION_SEQUENCE_DURATION,
        )
    }

    pub fn crouching_dynamic(index: f32) -> Self {
        Self::new(
            AlsTransitionKind::CrouchingDynamic,
            index,
            DYNAMIC_TRANSITION_BLEND,
            DYNAMIC_TRANSITION_BLEND,
            DYNAMIC_TRANSITION_PLAY_RATE,
            0.0,
            CROUCH_DYNAMIC_TRANSITION_DURATION,
        )
    }

    pub fn standing_dynamic(index: f32) -> Self {
        Self::new(
            AlsTransitionKind::StandingDynamic,
            index,
            DYNAMIC_TRANSITION_BLEND,
            DYNAMIC_TRANSITION_BLEND,
            DYNAMIC_TRANSITION_PLAY_RATE,
            0.0,
            crate::standing_source::MONTAGES["A_Als_Stand_DynamicTransition_Left"].duration,
        )
    }

    pub fn is_crouching(self) -> bool {
        matches!(
            self.kind,
            AlsTransitionKind::CrouchingStop
                | AlsTransitionKind::CrouchingQuickStop
                | AlsTransitionKind::CrouchingDynamic
        )
    }

    pub fn request_stop(&mut self) {
        self.stop_with_duration(self.blend_out);
    }

    fn stop_with_duration(&mut self, duration: f32) {
        if !self.active || (self.stop_elapsed.is_some() && duration >= self.stop_duration) {
            return;
        }
        self.stop_start_weight = self.weight;
        self.stop_duration = duration.max(0.0);
        self.stop_elapsed = Some(0.0);
        if duration <= 0.0 {
            self.active = false;
            self.weight = 0.0;
        }
    }

    pub fn advance(&mut self, delta_seconds: f32) {
        if !self.active {
            return;
        }
        self.previous_clip_time = self.clip_time;
        self.elapsed += delta_seconds.max(0.0);
        let play_rate = if self.play_rate.abs() <= 1e-8 { 0.0 } else { self.play_rate };
        self.clip_time = (self.start_time + self.elapsed * play_rate).clamp(0.0, self.duration);
        let playback_duration = if play_rate == 0.0 {
            f32::INFINITY
        } else if play_rate < 0.0 {
            self.start_time.max(0.0) / -play_rate
        } else {
            (self.duration - self.start_time).max(0.0) / play_rate
        };
        if let Some(stop_elapsed) = &mut self.stop_elapsed {
            *stop_elapsed += delta_seconds.max(0.0);
        } else {
            self.weight = transition_blend_amount(self.elapsed.min(playback_duration), self.blend_in);
            if self.elapsed < playback_duration {
                return;
            }
            self.request_stop();
            self.stop_elapsed = Some(self.elapsed - playback_duration);
        }
        let stop_elapsed = self.stop_elapsed.unwrap();
        self.weight = self.stop_start_weight * (1.0 - transition_blend_amount(stop_elapsed, self.stop_duration));
        if stop_elapsed >= self.stop_duration {
            self.active = false;
            self.weight = 0.0;
        }
    }
}

#[derive(Component, Debug, Default)]
pub struct AlsTransitionMontageStack {
    pub entries: Vec<AlsTransitionPlayback>,
}

impl AlsTransitionMontageStack {
    pub fn latest(&self) -> Option<&AlsTransitionPlayback> { self.entries.last() }

    fn begin(&mut self, playback: AlsTransitionPlayback) {
        for entry in &mut self.entries {
            entry.stop_with_duration(playback.blend_in);
        }
        self.entries.retain(|entry| entry.active && entry.weight > 0.0);
        self.entries.push(playback);
    }

    pub fn request_stop(&mut self) {
        for entry in &mut self.entries { entry.request_stop(); }
    }

    pub(crate) fn stop_with_duration(&mut self, duration: f32) {
        for entry in &mut self.entries { entry.stop_with_duration(duration); }
    }

    fn advance(&mut self, delta_seconds: f32) {
        for entry in &mut self.entries { entry.advance(delta_seconds); }
        let last = self.entries.len().saturating_sub(1);
        let mut index = 0;
        self.entries.retain(|entry| {
            let keep = entry.active || index == last;
            index += 1;
            keep
        });
    }
}

pub(crate) fn play_transition(commands: &mut Commands, character: Entity, playback: AlsTransitionPlayback) {
    commands.queue(move |world: &mut World| {
        if let Some(assembly) = world.get::<crate::character_assembly::AlsCharacterAssembly>(character).copied()
            && world.get::<crate::character_assembly::AlsCharacterAvatar>(assembly.avatar)
                .is_some_and(|avatar| avatar.character == character)
            && let Some(mut turns) = world.get_mut::<crate::turn_layer::AlsTurnMontageStack>(assembly.avatar)
        {
            turns.stop_for_group_replacement(playback.blend_in);
        }
        let Ok(mut entity) = world.get_entity_mut(character) else { return; };
        if let Some(mut stack) = entity.get_mut::<AlsTransitionMontageStack>() {
            stack.begin(playback);
        } else {
            entity.insert(AlsTransitionMontageStack { entries: vec![playback] });
        }
    });
}

#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct AlsQuickStopNotify {
    pub character: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
struct AlsRootTransitionSlotSampler {
    character: Entity,
    target_player: Entity,
    reported_unready: bool,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsRootTransitionSlotRuntime {
    pub active: bool,
    pub weight: f32,
    pub pose_ready: bool,
}

pub const STOP_TRIGGER_MIN_SPEED: f32 = 1.0;
pub const STOP_RELEASE_INPUT_EPSILON: f32 = 0.05;
pub const QUICK_STOP_BLEND_IN: f32 = 0.1;
pub const QUICK_STOP_BLEND_OUT: f32 = 0.2;
pub const QUICK_STOP_START_TIME: f32 = 0.3;
pub const QUICK_STOP_MIN_PLAY_RATE: f32 = 1.75;
pub const QUICK_STOP_MAX_PLAY_RATE: f32 = 3.0;
pub const STOP_TRANSITION_BLEND_IN: f32 = 0.2;
pub const STOP_TRANSITION_BLEND_OUT: f32 = 0.2;
pub const STOP_TRANSITION_PLAY_RATE: f32 = 1.5;
pub const STOP_TRANSITION_START_TIME: f32 = 0.4;
pub const RIGHT_TRANSITION_START_TIME: f32 = 0.2;
pub const TRANSITION_SEQUENCE_DURATION: f32 = 2.3333333;
pub const CROUCH_DYNAMIC_TRANSITION_DURATION: f32 = 1.5333333;
pub const DYNAMIC_TRANSITION_BLEND: f32 = 0.2;
pub const DYNAMIC_TRANSITION_PLAY_RATE: f32 = 1.5;
const ROOT_TRANSITION_SLOT_POSE_KEY: &str = "als_root_transition_slot_pose";
const ROOT_TRANSITION_SLOT_WEIGHT_KEY: &str = "als_root_transition_slot_weight";
const ROOT_TRANSITION_SLOT_LAYER_WEIGHT_KEY: &str = "als_root_transition_slot_layer_weight";
const GENERATED_TRANSITION_SLOT_POSE_KEY: &str = "transition.pose";
const GENERATED_TRANSITION_SLOT_WEIGHT_KEY: &str = "transition.weight";
const GENERATED_TRANSITION_SLOT_LAYER_WEIGHT_KEY: &str = "transition.layer_weight";
const GENERATED_TRANSITION_SLOT_MESH_SPACE_ADDITIVE_KEY: &str = "transition.mesh_space_additive";
const ROOT_TRANSITION_SLOT_CURVE_PREFIX: &str = "als_root_transition_slot_curve_";
const ROOT_TRANSITION_SLOT_CURVE_NAMES: [&str; 12] = [
    "PoseStanding",
    "PoseCrouching",
    "PoseMoving",
    "RotationYawOffset",
    "FootLeftLock",
    "FootRightLock",
    "AllowTransitions",
    "RotationYawSpeed",
    "PoseGrounded",
    "FootLeftIk",
    "FootRightIk",
    "PoseInAir",
];

pub fn quick_stop_selection(
    rotation_mode: RotationMode,
    input: &AlsInputCommand,
    view_yaw: f32,
    body: &Transform,
    target_yaw: f32,
) -> (f32, f32) {
    let forward = body.rotation * Vec3::NEG_Z;
    let body_yaw = world_forward_to_yaw(forward).to_degrees();
    let input_yaw = if input.move_dir.length_squared() > f32::EPSILON {
        let direction = view_relative_move_direction(input.move_dir, view_yaw);
        world_forward_to_yaw(direction).to_degrees()
    } else {
        target_yaw.to_degrees()
    };
    let selection = select_quick_stop(
        rotation_mode,
        -body_yaw,
        -input_yaw,
        &TransitionConfig::default(),
    );
    (if selection.right { 1.0 } else { 0.0 }, selection.play_rate)
}

pub(crate) fn transition_blend_amount(elapsed: f32, duration: f32) -> f32 {
    if duration <= 0.0 { return 1.0; }
    let alpha = (elapsed / duration.max(f32::EPSILON)).clamp(0.0, 1.0);
    alpha * alpha * (3.0 - 2.0 * alpha)
}

fn move_toward(current: f32, target: f32, max_delta: f32) -> f32 {
    let diff = target - current;
    if diff.abs() <= max_delta {
        target
    } else {
        current + diff.signum() * max_delta
    }
}

pub struct AlsStopLinkPlugin;

impl Plugin for AlsStopLinkPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AlsQuickStopNotify>()
            .add_observer(discard_transition_target)
            .add_systems(
                Update,
                (
                    ensure_stop_match,
                    stop_link_system.after(ensure_stop_match),
                    quick_stop_notify_system,
                    transition_playback_system.after(quick_stop_notify_system),
                    ensure_root_transition_slot_samplers
                        .before(stop_link_system)
                        .before(quick_stop_notify_system)
                        .before(transition_playback_system),
                ),
            )
            .add_systems(
                PostUpdate,
                bridge_root_transition_slot_samples
                    .after(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs)
                    .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
            );
    }
}

fn ensure_stop_match(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<crate::als::character::AlsCharacter>,
            Without<AlsStopMatch>,
        ),
    >,
) {
    for entity in &q {
        commands.entity(entity).insert(AlsStopMatch::default());
    }
}

fn root_transition_slot_active(playback: &AlsTransitionPlayback) -> bool {
    playback.active
        && matches!(
            playback.kind,
            AlsTransitionKind::Stop
                | AlsTransitionKind::QuickStop
                | AlsTransitionKind::StandingTransition
                | AlsTransitionKind::StandingDynamic
                | AlsTransitionKind::CrouchingStop
                | AlsTransitionKind::CrouchingQuickStop
                | AlsTransitionKind::CrouchingDynamic
        )
}

pub(crate) fn source_transition_sequence(playback: &AlsTransitionPlayback) -> Option<&'static str> {
    let left = playback.index <= 0.5;
    match playback.kind {
        AlsTransitionKind::Stop => Some(if left {
            "A_Als_Stop_Left"
        } else {
            "A_Als_Stop_Right"
        }),
        AlsTransitionKind::QuickStop => Some(if left {
            "A_Als_Stand_Transition_Left"
        } else {
            "A_Als_Stand_Transition_Right"
        }),
        AlsTransitionKind::StandingDynamic => Some(if left {
            "A_Als_Stand_DynamicTransition_Left"
        } else {
            "A_Als_Stand_DynamicTransition_Right"
        }),
        AlsTransitionKind::StandingTransition => Some(if left { "A_Als_Stand_Transition_Left" } else { "A_Als_Stand_Transition_Right" }),
        AlsTransitionKind::CrouchingDynamic => Some(if left {
            "A_Als_Crouch_DynamicTransition_Left"
        } else {
            "A_Als_Crouch_DynamicTransition_Right"
        }),
        AlsTransitionKind::CrouchingStop | AlsTransitionKind::CrouchingQuickStop => Some(if left {
            "A_Als_Crouch_Transition_Left"
        } else {
            "A_Als_Crouch_Transition_Right"
        }),
    }
}

fn ensure_root_transition_slot_samplers(
    characters: Query<(Entity, &AlsCharacterAssembly), With<crate::als::character::AlsCharacter>>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    samplers: Query<(Entity, &AlsRootTransitionSlotSampler)>,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
    mut states: Query<(Option<&mut AlsTransitionMontageStack>, Option<&mut AlsRootTransitionSlotRuntime>)>,
    mut commands: Commands,
) {
    let current_target = |character| {
        let (_, assembly) = characters.get(character).ok()?;
        let (avatar, instance) = avatars.get(assembly.avatar).ok()?;
        (avatar.character == character).then_some(instance.player_entity())
    };
    for (entity, sampler) in &samplers {
        if current_target(sampler.character) == Some(sampler.target_player) && players.contains(sampler.target_player) { continue; }
        if let Ok((stack, runtime)) = states.get_mut(sampler.character) { end_transition_instance(stack, runtime); }
        if let Ok(mut player) = players.get_mut(sampler.target_player) {
            clear_transition_slot(&mut player, source_inputs.get_mut(sampler.target_player).ok());
        }
        commands.entity(entity).try_despawn();
    }
    for (character, _) in &characters {
        let Some(target_player) = current_target(character).filter(|player| players.contains(*player)) else { continue; };
        if samplers
            .iter()
            .any(|(_, sampler)| sampler.character == character && sampler.target_player == target_player)
        {
            continue;
        }
        commands.spawn((
            Name::new("als_root_transition_slot_sampler"),
            ChildOf(character),
            AlsRootTransitionSlotSampler {
                character,
                target_player,
                reported_unready: false,
            },
        ));
    }
}

fn end_transition_instance(stack: Option<Mut<AlsTransitionMontageStack>>, runtime: Option<Mut<AlsRootTransitionSlotRuntime>>) {
    if let Some(mut stack) = stack { stack.entries.clear(); }
    if let Some(mut runtime) = runtime { *runtime = default(); }
}

fn clear_transition_slot(player: &mut AnimationGraphPlayer, inputs: Option<Mut<crate::extensions::SourceAnimationInputs>>) {
    for key in [ROOT_TRANSITION_SLOT_WEIGHT_KEY, ROOT_TRANSITION_SLOT_LAYER_WEIGHT_KEY,
        GENERATED_TRANSITION_SLOT_WEIGHT_KEY, GENERATED_TRANSITION_SLOT_LAYER_WEIGHT_KEY, GENERATED_TRANSITION_SLOT_MESH_SPACE_ADDITIVE_KEY] {
        player.set_global_input_data(key, 0.0_f32);
    }
    if let Some(mut inputs) = inputs { inputs.set_slot_input_context(GENERATED_TRANSITION_SLOT_POSE_KEY, None); }
}

fn discard_transition_target(
    event: On<Discard, AnimationGraphPlayer>,
    samplers: Query<(Entity, &AlsRootTransitionSlotSampler)>,
    mut states: Query<(Option<&mut AlsTransitionMontageStack>, Option<&mut AlsRootTransitionSlotRuntime>)>,
    mut commands: Commands,
) {
    for (entity, sampler) in &samplers {
        if sampler.target_player != event.entity { continue; }
        if let Ok((stack, runtime)) = states.get_mut(sampler.character) { end_transition_instance(stack, runtime); }
        commands.entity(entity).try_despawn();
    }
}

fn root_transition_curve_key(name: &str, crouching: bool) -> Option<&'static str> {
    match (crouching, name) {
        (true, "PoseCrouching") => Some("als_crouching_curve_pose_crouching"),
        (true, "PoseMoving") => Some("als_crouching_curve_pose_moving"),
        (true, "RotationYawOffset") => Some("als_crouching_curve_rotation_yaw_offset"),
        (true, "FootLeftLock") => Some("als_crouching_curve_foot_left_lock"),
        (true, "FootRightLock") => Some("als_crouching_curve_foot_right_lock"),
        (true, "AllowTransitions") => Some("als_crouching_curve_allow_transitions"),
        (true, "RotationYawSpeed") => Some("als_crouching_curve_rotation_yaw_speed"),
        (false, "PoseStanding") => Some("als_standing_curve_pose_standing"),
        (false, "PoseMoving") => Some("als_standing_curve_pose_moving"),
        (false, "RotationYawOffset") => Some("als_standing_curve_rotation_yaw_offset"),
        (false, "FootLeftLock") => Some("als_standing_curve_foot_left_lock"),
        (false, "FootRightLock") => Some("als_standing_curve_foot_right_lock"),
        (false, "AllowTransitions") => Some("als_standing_curve_allow_transitions"),
        (false, "RotationYawSpeed") => Some("als_standing_curve_rotation_yaw_speed"),
        _ => None,
    }
}

fn write_root_transition_slot_curves(player: &mut AnimationGraphPlayer, crouching: bool) {
    for name in ROOT_TRANSITION_SLOT_CURVE_NAMES {
        let value = root_transition_curve_key(name, crouching)
            .and_then(|key| player.get_global_input_data(key))
            .and_then(|value| value.as_f32().ok())
            .filter(|value| value.is_finite())
            .unwrap_or_default();
        player.set_global_input_data(format!("{ROOT_TRANSITION_SLOT_CURVE_PREFIX}{name}"), value);
    }
}

fn sample_transition_stack(
    stack: &AlsTransitionMontageStack,
    assets: &crate::standing_source::SourceMontageAssets,
    clips: &Assets<bevy_animation_graph::core::animation_clip::GraphClip>,
    skeletons: &Assets<bevy_animation_graph::core::skeleton::Skeleton>,
) -> Option<(bevy_animation_graph::core::pose::Pose, crate::extensions::AnimationContextState, f32)> {
    let samples = stack.entries.iter().filter(|entry| entry.active && entry.weight > 1e-6)
        .map(|entry| crate::standing_source::sample_source_montage(
            source_transition_sequence(entry)?, entry.clip_time, assets, clips, skeletons,
        ).map(|(pose, mut context)| {
            for sample in &mut context.asset_samples {
                sample.identity = format!("transition:{}", entry.instance_id);
                sample.clock.previous_position = entry.previous_clip_time;
                sample.clock.has_previous_position = true;
                sample.clock.playback_delta_seconds = sample.clock.position - entry.previous_clip_time;
                sample.clock.weight = entry.weight;
            }
            (pose, context, entry.weight)
        }))
        .collect::<Option<Vec<_>>>()?;
    let total = samples.iter().map(|(_, _, weight)| weight).sum::<f32>();
    let denominator = total.max(1.0);
    let pose = crate::extensions::mesh_space_additive::compose_mesh_space_additives(
        samples.iter().map(|(pose, _, weight)| (pose, weight / denominator)),
    )?;
    let context = crate::extensions::source_pose_nodes::blend_animation_contexts(
        samples.into_iter().map(|(_, context, weight)| (context, weight / denominator)),
    );
    Some((pose, context, total.min(1.0)))
}

fn bridge_root_transition_slot_samples(
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
    montage_assets: Res<crate::standing_source::SourceMontageAssets>,
    clips: Res<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>,
    skeletons: Res<Assets<bevy_animation_graph::core::skeleton::Skeleton>>,
    playbacks: Query<&AlsTransitionMontageStack>,
    stances: Query<&Stance>,
    mut samplers: Query<&mut AlsRootTransitionSlotSampler>,
    mut target_players: Query<&mut AnimationGraphPlayer>,
    mut commands: Commands,
) {
    for mut sampler in &mut samplers {
        let Ok(mut target_player) = target_players.get_mut(sampler.target_player) else {
            continue;
        };
        let stack = playbacks
            .get(sampler.character)
            .ok()
            .filter(|stack| stack.entries.iter().any(root_transition_slot_active));
        let playback = stack.and_then(AlsTransitionMontageStack::latest);
        let crouching = playback.is_some_and(|value| value.is_crouching())
            || stances
                .get(sampler.character)
                .is_ok_and(|stance| *stance == Stance::Crouching);
        let Some(stack) = stack else {
            clear_transition_slot(&mut target_player, source_inputs.get_mut(sampler.target_player).ok());
            write_root_transition_slot_curves(&mut target_player, crouching);
            commands
                .entity(sampler.character)
                .insert(AlsRootTransitionSlotRuntime::default());
            continue;
        };
        let sample = sample_transition_stack(stack, &montage_assets, &clips, &skeletons);
        let Some((pose, context, weight)) = sample else {
            if stack.entries.iter().any(|entry| entry.weight > 1e-6) && !sampler.reported_unready {
                warn!(
                    "[TRANSITION-SLOT] sampler unavailable character={:?} error={:?}",
                    sampler.character,
                    target_player.get_error()
                );
                sampler.reported_unready = true;
            }
            target_player.set_global_input_data(ROOT_TRANSITION_SLOT_WEIGHT_KEY, 0.0_f32);
            target_player.set_global_input_data(ROOT_TRANSITION_SLOT_LAYER_WEIGHT_KEY, 0.0_f32);
            target_player.set_global_input_data(GENERATED_TRANSITION_SLOT_WEIGHT_KEY, 0.0_f32);
            target_player
                .set_global_input_data(GENERATED_TRANSITION_SLOT_LAYER_WEIGHT_KEY, 0.0_f32);
            target_player
                .set_global_input_data(GENERATED_TRANSITION_SLOT_MESH_SPACE_ADDITIVE_KEY, 0.0_f32);
            write_root_transition_slot_curves(&mut target_player, crouching);
            commands
                .entity(sampler.character)
                .insert(AlsRootTransitionSlotRuntime {
                    active: true,
                    weight: 0.0,
                    pose_ready: false,
                });
            continue;
        };
        target_player
            .set_global_input_data(ROOT_TRANSITION_SLOT_POSE_KEY, DataValue::Pose(pose.clone()));
        target_player.set_global_input_data(ROOT_TRANSITION_SLOT_WEIGHT_KEY, 1.0_f32);
        target_player.set_global_input_data(ROOT_TRANSITION_SLOT_LAYER_WEIGHT_KEY, 1.0_f32);
        target_player
            .set_global_input_data(GENERATED_TRANSITION_SLOT_POSE_KEY, DataValue::Pose(pose));
        target_player.set_global_input_data(GENERATED_TRANSITION_SLOT_WEIGHT_KEY, 1.0_f32);
        target_player.set_global_input_data(GENERATED_TRANSITION_SLOT_LAYER_WEIGHT_KEY, 1.0_f32);
        target_player
            .set_global_input_data(GENERATED_TRANSITION_SLOT_MESH_SPACE_ADDITIVE_KEY, 1.0_f32);
        write_root_transition_slot_curves(&mut target_player, crouching);
        sampler.reported_unready = false;
        source_inputs.get_mut(sampler.target_player).unwrap().set_slot_input_context(GENERATED_TRANSITION_SLOT_POSE_KEY,
            Some(context),);
        commands
            .entity(sampler.character)
            .insert(AlsRootTransitionSlotRuntime {
                active: true,
                weight,
                pose_ready: true,
            });
    }
}

#[allow(clippy::type_complexity)]
pub fn stop_link_system(
    time: Res<Time>,
    mut q: Query<
        (
            &AlsActualVelocity,
            &LocomotionState,
            &Stance,
            &RotationMode,
            &Transform,
            (&AlsViewYaw, Option<&crate::als::view::AlsPlayerViewYaw>),
            &crate::als::character::AlsMovementParamsHandle,
            Option<&AlsGroundedDynamics>,
            Option<&AlsLandingBrakingFriction>,
            &mut AlsStopMatch,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<crate::als::character::AlsCharacter>,
    >,
    params: Res<Assets<AlsMovementParams>>,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    let Ok((
        vel,
        loco,
        stance,
        rotation_mode,
        transform,
        (view_yaw, player_view),
        handle,
        dynamics,
        landing_braking,
        mut m,
        cmd,
        ovr,
    )) = q.single_mut()
    else {
        return;
    };
    let Some(p) = params.get(handle.id()) else {
        return;
    };
    let effective = ovr.map(|o| &o.0).unwrap_or(cmd);
    let hspeed = vel.horizontal_speed;
    let input_move = effective.move_dir.length();
    if input_move > STOP_RELEASE_INPUT_EPSILON {
        let direction = view_relative_move_direction(effective.move_dir, player_view.map_or(view_yaw.0, |view| view.0));
        if direction.length_squared() > 1e-6 {
            m.last_input_yaw = Some(world_forward_to_yaw(direction));
        }
    }

    if m.active {
        m.remaining = (m.remaining - hspeed * dt).max(0.0);
        let stopped = hspeed <= 0.05;
        let aborted = input_move > 0.30 || *loco != LocomotionState::Grounded;
        if m.remaining <= 0.005 || stopped || aborted {
            if stopped {
                m.remaining = 0.0;
            }
            m.active = false;
        }
    } else {
        let released_while_fast = m.last_input_move > 0.30
            && input_move <= STOP_RELEASE_INPUT_EPSILON
            && *loco == LocomotionState::Grounded
            && hspeed >= STOP_TRIGGER_MIN_SPEED;
        if released_while_fast {
            let config = TransitionConfig::default();
            let (actor_yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
            let target_yaw = m.last_input_yaw.unwrap_or(actor_yaw);
            let quick_stop = select_quick_stop(
                *rotation_mode,
                actor_yaw.to_degrees(),
                target_yaw.to_degrees(),
                &config,
            );
            let fallback_braking = dynamics
                .map(|value| value.0.braking_deceleration)
                .unwrap_or(p.acceleration.braking_deceleration);
            let dynamic_distance = predict_grounded_stop_distance_with_braking_friction(
                p,
                *stance,
                *rotation_mode,
                hspeed,
                landing_braking.map_or(0.0, |state| state.factor),
            );
            m.total = if dynamic_distance > 0.0 {
                dynamic_distance
            } else {
                predict_stop_distance(hspeed, fallback_braking)
            }
            .max(0.3);
            m.remaining = m.total;
            m.active = true;
            m.left_weight = if quick_stop.right { 0.0 } else { 1.0 };
            m.right_weight = if quick_stop.right { 1.0 } else { 0.0 };
            m.quick_stop_play_rate = quick_stop.play_rate;
            m.quick_stop_start_time = quick_stop.start_time;
            m.quick_stop_blend_in_duration = quick_stop.blend_in_duration;
            m.quick_stop_blend_out_duration = quick_stop.blend_out_duration;
            m.quick_stop_remaining_yaw_degrees = quick_stop.remaining_yaw_degrees;
        }
    }
    m.last_input_move = input_move;

    let weight_target = if m.active { 1.0 } else { 0.0 };
    let w_speed = if m.active {
        m.quick_stop_blend_in_duration.recip()
    } else {
        m.quick_stop_blend_out_duration.recip()
    };
    m.weight = move_toward(m.weight, weight_target, w_speed * dt);

    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("stop_weight", DataValue::from(m.weight));
            player.set_input_data("stop_right_weight", DataValue::from(m.right_weight));
            player.set_input_data("stop_remaining", DataValue::from(m.remaining));
            player.set_input_data("stop_total", DataValue::from(m.total));
            player.set_input_data("stop_play_rate", DataValue::from(m.quick_stop_play_rate));
            player.set_input_data("stop_start_time", DataValue::from(m.quick_stop_start_time));
        }
    }
}

fn quick_stop_notify_system(
    mut notices: MessageReader<AlsQuickStopNotify>,
    characters: Query<
        (
            &Transform,
            &RotationMode,
            &Stance,
            (&AlsViewYaw, Option<&crate::als::view::AlsPlayerViewYaw>),
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            Option<&AlsRotationResponseState>,
        ),
        With<crate::als::character::AlsCharacter>,
    >,
    mut commands: Commands,
) {
    for notice in notices.read() {
        let Ok((body, rotation_mode, stance, (view_yaw, player_view), command, override_command, rotation)) =
            characters.get(notice.character)
        else {
            continue;
        };
        let input = override_command.map_or(command, |value| &value.0);
        let target_yaw = rotation
            .filter(|rotation| rotation.initialized)
            .map_or_else(
                || world_forward_to_yaw(*body.forward()),
                |rotation| rotation.target_yaw,
            );
        let (index, play_rate) =
            quick_stop_selection(*rotation_mode, input, player_view.map_or(view_yaw.0, |view| view.0), body, target_yaw);
        let playback = if *stance == Stance::Crouching {
            AlsTransitionPlayback::crouching_quick_stop(index, play_rate)
        } else {
            AlsTransitionPlayback::quick_stop(index, play_rate)
        };
        play_transition(&mut commands, notice.character, playback);
    }
}

fn transition_playback_system(
    time: Res<Time>,
    mut playbacks: Query<(&mut AlsTransitionMontageStack, Option<&crate::als_camera::AlsCameraCustomTimeDilation>)>,
) {
    for (mut playback, custom_dilation) in &mut playbacks {
        playback.advance(time.delta_secs() * custom_dilation.map_or(1.0, |value| value.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play_loaded_transition(app: &mut App, character: Entity) -> Entity {
        use bevy::ecs::system::RunSystemOnce;
        app.world_mut().run_system_once_with(|In(character): In<Entity>, mut commands: Commands| {
            play_transition(&mut commands, character, AlsTransitionPlayback::standing_dynamic(0.0));
        }, character).unwrap();
        for _ in 0..6 { app.update(); }
        let playback = app.world().get::<AlsTransitionMontageStack>(character).unwrap().latest().unwrap();
        assert!(playback.active && playback.clip_time > 0.0 && playback.weight > 0.1);
        let runtime = app.world().get::<AlsRootTransitionSlotRuntime>(character).unwrap();
        assert!(runtime.active && runtime.pose_ready && runtime.weight > 0.1);
        let avatar = app.world().get::<crate::character_assembly::AlsCharacterAssembly>(character).unwrap().avatar;
        let target = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        let player = app.world().get::<AnimationGraphPlayer>(target).unwrap();
        assert!(player.get_error().is_none() && player.get_default_output_pose().is_some(), "Transition target pose failed: error={:?} paused={}", player.get_error(), player.is_paused());
        assert!(player.get_global_input_data(GENERATED_TRANSITION_SLOT_POSE_KEY).and_then(|value| value.as_pose().ok()).is_some_and(|pose| !pose.bones.is_empty()));
        assert!(app.world().get::<crate::extensions::SourceAnimationInputs>(target).unwrap().slot_input_context(GENERATED_TRANSITION_SLOT_POSE_KEY).is_some());
        app.world_mut().query::<(Entity, &AlsRootTransitionSlotSampler)>().iter(app.world())
            .find(|(_, sampler)| sampler.character == character && sampler.target_player == target).unwrap().0
    }

    fn assert_transition_ended(app: &App, character: Entity) {
        assert!(app.world().get::<AlsTransitionMontageStack>(character).is_none_or(|stack| stack.entries.is_empty()), "Transition stack survived its player instance: {:?}", app.world().get::<AlsTransitionMontageStack>(character));
        assert!(app.world().get::<AlsRootTransitionSlotRuntime>(character).is_none_or(|runtime| !runtime.active && runtime.weight == 0.0 && !runtime.pose_ready));
    }

    #[test]
    fn transition_sampler_ends_with_loaded_player_replacement_and_destruction() {
        use bevy_animation_graph::core::animation_graph_player::AnimationSource;
        for mode in 0..5 {
            let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let target = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
            let sampler = play_loaded_transition(&mut app, character);
            let old_ids: Vec<_> = app.world().get::<AlsTransitionMontageStack>(character).unwrap().entries.iter().map(|entry| entry.instance_id).collect();
            let replacement_boundary = AlsTransitionPlayback::standing_dynamic(1.0).instance_id;
            if mode < 2 {
                let player = app.world().get::<AnimationGraphPlayer>(target).unwrap();
                let AnimationSource::Graph(graph) = player.get_animation_source() else { panic!("Character must use its actual graph"); };
                let replacement = AnimationGraphPlayer::new(player.skeleton().clone()).with_graph(graph.clone());
                if mode == 1 { app.world_mut().entity_mut(target).remove::<AnimationGraphPlayer>(); }
                app.world_mut().entity_mut(target).insert(replacement);
            } else if mode == 2 {
                app.world_mut().despawn(target);
            } else {
                if mode == 4 {
                    let children: Vec<_> = app.world().get::<Children>(avatar).unwrap().iter().rev().collect();
                    app.world_mut().entity_mut(avatar).replace_children(&children);
                }
                app.world_mut().despawn(avatar);
            }
            assert!(app.world().get_entity(sampler).is_err(), "Old transition sampler survived discard mode {mode}");
            assert_transition_ended(&app, character);
            for _ in 0..4 { app.update(); }
            if let Some(stack) = app.world().get::<AlsTransitionMontageStack>(character) {
                assert!(stack.entries.iter().all(|entry| !old_ids.contains(&entry.instance_id)));
                assert!(stack.entries.iter().all(|entry| mode < 2 && entry.instance_id > replacement_boundary && entry.kind == AlsTransitionKind::StandingDynamic && entry.elapsed <= 4.0 / 60.0));
            }
            if let Some(context) = app.world().get::<crate::extensions::SourceAnimationInputs>(target).and_then(|inputs| inputs.slot_input_context(GENERATED_TRANSITION_SLOT_POSE_KEY)) {
                assert!(context.asset_samples.iter().all(|sample| old_ids.iter().all(|id| sample.identity != format!("transition:{id}"))));
            }
        }
    }

    #[test]
    fn transition_sampler_uses_only_the_current_avatar() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::character_assembly::{AlsCharacterAssembly, ALS_LINKED_ANIMATED_SCENE, attach_avatar};
        for retain_owner in [false, true] {
            let (mut app, character, old_avatar) = crate::roll_drive::tests::loaded_roll_character();
            let old_target = app.world().get::<AnimatedSceneInstance>(old_avatar).unwrap().player_entity();
            let old_sampler = play_loaded_transition(&mut app, character);
            if !retain_owner { app.world_mut().entity_mut(old_avatar).remove::<AlsCharacterAvatar>(); }
            app.world_mut().get_mut::<AnimationGraphPlayer>(old_target).unwrap().pause();
            let avatar = app.world_mut().run_system_once_with(|In(character): In<Entity>, mut commands: Commands, assets: Res<AssetServer>| {
                attach_avatar(&mut commands, &assets, character, ALS_LINKED_ANIMATED_SCENE)
            }, character).unwrap();
            app.world_mut().get_mut::<AlsCharacterAssembly>(character).unwrap().avatar = avatar;
            let target = crate::roll_drive::tests::ready_roll_player(&mut app, avatar);
            assert!(app.world().get_entity(old_sampler).is_err(), "Retained old avatar kept its transition sampler");
            assert_transition_ended(&app, character);
            for player in [old_target, target] {
                assert_eq!(app.world().get::<AnimationGraphPlayer>(player).unwrap().get_global_input_data(GENERATED_TRANSITION_SLOT_WEIGHT_KEY)
                    .and_then(|value| value.as_f32().ok()).unwrap_or(0.0), 0.0);
                assert!(app.world().get::<crate::extensions::SourceAnimationInputs>(player).unwrap().slot_input_context(GENERATED_TRANSITION_SLOT_POSE_KEY).is_none());
            }
            let sampler = play_loaded_transition(&mut app, character);
            app.world_mut().get_mut::<crate::als::transition::AlsStateRequest>(character).unwrap().overlay_state = Some(crate::als::state::OverlayState::Rifle);
            app.update();
            assert!(app.world().get::<AlsRootTransitionSlotRuntime>(character).unwrap().pose_ready);
            app.world_mut().despawn(old_avatar);
            app.update();
            assert!(app.world().get_entity(sampler).is_ok());
            assert!(app.world().get::<AlsTransitionMontageStack>(character).unwrap().latest().unwrap().active);
            let current: Vec<_> = app.world_mut().query::<(Entity, &AlsRootTransitionSlotSampler)>().iter(app.world())
                .filter(|(_, sampler)| sampler.character == character).map(|(entity, sampler)| (entity, sampler.target_player)).collect();
            assert_eq!(current, [(sampler, target)]);
        }
    }

    #[test]
    fn transition_replacement_accepts_a_new_request_after_ending_the_old_stack() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::character_assembly::{ALS_LINKED_ANIMATED_SCENE, attach_avatar};
        let (mut app, character, old_avatar) = crate::roll_drive::tests::loaded_roll_character();
        play_loaded_transition(&mut app, character);
        let old_ids: Vec<_> = app.world().get::<AlsTransitionMontageStack>(character).unwrap().entries.iter().map(|entry| entry.instance_id).collect();
        let avatar = app.world_mut().run_system_once_with(|In(character): In<Entity>, mut commands: Commands, assets: Res<AssetServer>| {
            attach_avatar(&mut commands, &assets, character, ALS_LINKED_ANIMATED_SCENE)
        }, character).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while app.world().get::<AnimatedSceneInstance>(avatar).is_none() {
            app.update();
            assert!(std::time::Instant::now() < deadline, "Replacement transition avatar did not load");
        }
        let old_player = app.world().get::<AnimatedSceneInstance>(old_avatar).unwrap().player_entity();
        app.world_mut().get_mut::<AnimationGraphPlayer>(old_player).unwrap().pause();
        app.world_mut().get_mut::<AlsCharacterAssembly>(character).unwrap().avatar = avatar;
        app.world_mut().write_message(AlsQuickStopNotify { character });
        app.update();
        let stack = app.world().get::<AlsTransitionMontageStack>(character).unwrap();
        assert!(stack.entries.iter().all(|entry| !old_ids.contains(&entry.instance_id)));
        assert!(stack.entries.iter().any(|entry| entry.active && entry.kind == AlsTransitionKind::QuickStop), "Instance cleanup erased a new transition request");
        assert!(app.world().get::<AlsRootTransitionSlotRuntime>(character).unwrap().pose_ready);
    }

    #[test]
    fn transition_slot_pose_uses_the_source_actor_animation_clock() {
        use bevy_animation_graph::core::{animation_clip::GraphClip, skeleton::Skeleton};
        for (custom_dilation, reverse) in [(1.0_f32, false), (0.5, false), (0.0, false), (0.5, true)] {
            let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
            play_loaded_transition(&mut app, character);
            for _ in 0..12 { app.update(); }
            assert_eq!(app.world().get::<AlsTransitionMontageStack>(character).unwrap().entries.len(), 1,
                "Clock fixture must sample a single settled montage instance");
            if reverse {
                let mut stack = app.world_mut().get_mut::<AlsTransitionMontageStack>(character).unwrap();
                let playback = stack.entries.last_mut().unwrap();
                playback.start_time = 1.0 + playback.elapsed;
                playback.clip_time = 1.0;
                playback.play_rate = -1.0;
            }
            let before = *app.world().get::<AlsTransitionMontageStack>(character).unwrap().latest().unwrap();
            assert!(before.active && before.stop_elapsed.is_none());
            app.world_mut().entity_mut(character).insert(crate::als_camera::AlsCameraCustomTimeDilation(custom_dilation));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
            app.update();
            let delta = app.world().resource::<Time>().delta_secs() * custom_dilation;
            let source_time = (before.clip_time + delta * before.play_rate).clamp(0.0, before.duration);
            let (source_pose, _) = crate::standing_source::sample_source_montage(source_transition_sequence(&before).unwrap(), source_time,
                app.world().resource::<crate::standing_source::SourceMontageAssets>(), app.world().resource::<Assets<GraphClip>>(),
                app.world().resource::<Assets<Skeleton>>()).unwrap();
            let target = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
            let player = app.world().get::<AnimationGraphPlayer>(target).unwrap();
            assert!(player.get_error().is_none());
            let actual = player.get_global_input_data(GENERATED_TRANSITION_SLOT_POSE_KEY).unwrap().as_pose().unwrap();
            let mut minimum_rotation_dot = 1.0_f32;
            let mut maximum_location_error = 0.0_f32;
            for bone in source_pose.paths.keys() {
                let a = actual.get_bone(*bone).unwrap();
                let s = source_pose.get_bone(*bone).unwrap();
                if let (Some(a), Some(s)) = (a.rotation, s.rotation) {
                    minimum_rotation_dot = minimum_rotation_dot.min(a.dot(s).abs());
                }
                if let (Some(a), Some(s)) = (a.translation, s.translation) {
                    maximum_location_error = maximum_location_error.max(a.distance(s));
                }
            }
            assert!(minimum_rotation_dot > 1.0 - 1e-5 && maximum_location_error < 1e-5,
                "Transition slot pose uses the wrong actor clock: custom={custom_dilation}, rotation_error_degrees={}, location_error={maximum_location_error}, source_time={source_time}, stack={:?}",
                (2.0 * minimum_rotation_dot.clamp(-1.0, 1.0).acos()).to_degrees(), app.world().get::<AlsTransitionMontageStack>(character).unwrap());
            let after = app.world().get::<AlsTransitionMontageStack>(character).unwrap().latest().unwrap();
            assert_eq!(after.instance_id, before.instance_id);
            assert!((after.clip_time - source_time).abs() < 1e-6);
        }
    }

    #[test]
    fn transition_stack_sampling_advances_without_an_auxiliary_player() {
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        play_loaded_transition(&mut app, character);
        let target = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
        let before = app.world().get::<AnimationGraphPlayer>(target).unwrap().get_global_input_data(GENERATED_TRANSITION_SLOT_POSE_KEY).unwrap().clone().into_pose().unwrap();
        let time = app.world().get::<AlsTransitionMontageStack>(character).unwrap().latest().unwrap().clip_time;
        for _ in 0..6 { app.update(); }
        let after = app.world().get::<AnimationGraphPlayer>(target).unwrap().get_global_input_data(GENERATED_TRANSITION_SLOT_POSE_KEY).unwrap().as_pose().unwrap();
        assert!(before.paths.keys().any(|bone| before.get_bone(*bone).unwrap().rotation != after.get_bone(*bone).unwrap().rotation));
        let context = app.world().get::<crate::extensions::SourceAnimationInputs>(target).unwrap().slot_input_context(GENERATED_TRANSITION_SLOT_POSE_KEY).unwrap();
        assert!(context.asset_samples.iter().any(|sample| sample.identity.starts_with("transition:") && sample.clock.position > time));
        for sample in &context.asset_samples {
            if sample.identity.starts_with("transition:") {
                assert!(sample.clock.has_previous_position);
                assert!(sample.clock.playback_delta_seconds > 0.0);
                assert!((sample.clock.playback_delta_seconds
                    - (sample.clock.position - sample.clock.previous_position)).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn transition_signed_playback_preserves_direction_and_boundary_lifetime() {
        for rate in [-2.0_f32, -1.0, -0.5, 0.5, 1.0, 2.0] {
            for start in [0.0_f32, 0.5, 1.0] {
                let mut playback = AlsTransitionPlayback::new(AlsTransitionKind::StandingTransition, 1.0, 0.0, 0.2, rate, start, 1.0);
                playback.advance(0.0);
                assert_eq!(playback.clip_time, start);
                let lifetime = if rate < 0.0 { start / -rate } else { (1.0 - start) / rate };
                let dt = lifetime.min(0.1);
                playback.advance(dt);
                assert!((playback.clip_time - (start + dt * rate).clamp(0.0, 1.0)).abs() < 1e-6, "Signed time failed: rate={rate}, start={start}, actual={}", playback.clip_time);
                playback.advance((lifetime - dt).max(0.0));
                assert!((playback.clip_time - if rate < 0.0 { 0.0 } else { 1.0 }).abs() < 1e-6);
                assert!(playback.active && playback.stop_elapsed.is_some());
                playback.advance(0.1);
                assert!((playback.weight - 0.5).abs() < 1e-5);
                playback.advance(0.1);
                assert!(!playback.active && playback.weight == 0.0);
            }
        }
        for rate in [0.0, 1e-9, -1e-9] {
            let mut playback = AlsTransitionPlayback::new(AlsTransitionKind::StandingTransition, 1.0, 0.1, 0.2, rate, 0.5, 1.0);
            playback.advance(100.0);
            assert_eq!(playback.clip_time, 0.5);
            assert!(playback.active && playback.weight == 1.0 && playback.stop_elapsed.is_none());
            playback.request_stop();
            playback.advance(0.2);
            assert!(!playback.active && playback.clip_time == 0.5);
        }
        let mut stack = AlsTransitionMontageStack::default();
        stack.begin(AlsTransitionPlayback::new(AlsTransitionKind::StandingTransition, 1.0, 0.0, 0.2, -1.0, 0.5, 1.0));
        stack.advance(0.1);
        stack.begin(AlsTransitionPlayback::new(AlsTransitionKind::StandingTransition, 0.0, 0.1, 0.2, 1.0, 0.2, 1.0));
        stack.advance(0.05);
        assert_eq!(stack.entries.len(), 2);
        assert!((stack.entries[0].clip_time - 0.35).abs() < 1e-6);
        assert!((stack.latest().unwrap().clip_time - 0.25).abs() < 1e-6);
        stack.advance(0.05);
        assert_eq!(stack.entries.len(), 1);
    }

    #[test]
    fn transition_signed_playback_reaches_actual_slot_pose_curves_and_event_clock() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy_animation_graph::core::{animation_clip::GraphClip, skeleton::Skeleton};
        for crouching in [false, true] {
            for rate in [-2.0_f32, -1.0, -0.5, 0.0, 1.0] {
                let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
                play_loaded_transition(&mut app, character);
                app.world_mut().get_mut::<Stance>(character).map(|mut stance| *stance = if crouching { Stance::Crouching } else { Stance::Standing });
                app.world_mut().run_system_once(move |mut commands: Commands| {
                    play_transition(&mut commands, character, AlsTransitionPlayback::transition_animation(crouching, false, 0.0, 0.2, rate, 0.5));
                }).unwrap();
                let target = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
                let mut last_time = 0.5;
                let mut pose_changed = false;
                let mut last_pose = None;
                for _ in 0..6 {
                    app.update();
                    let entry = app.world().get::<AlsTransitionMontageStack>(character).unwrap().latest().unwrap();
                    let dt = app.world().resource::<Time>().delta_secs();
                    let expected_time = (0.5 + entry.elapsed * rate).clamp(0.0, entry.duration);
                    assert!((entry.clip_time - expected_time).abs() < 1e-6);
                    let context = app.world().get::<crate::extensions::SourceAnimationInputs>(target).unwrap().slot_input_context(GENERATED_TRANSITION_SLOT_POSE_KEY).unwrap();
                    let sample = context.asset_samples.iter().find(|sample| sample.identity == format!("transition:{}", entry.instance_id)).unwrap();
                    assert!((sample.clock.position - expected_time).abs() < 1e-6);
                    assert!((sample.clock.previous_position - last_time).abs() < 1e-6);
                    assert!((sample.clock.playback_delta_seconds - rate * dt).abs() < 1e-6);
                    if rate != 0.0 {
                        let clock = crate::extensions::EventClock {
                            previous_time: sample.clock.previous_position, current_time: sample.clock.position,
                            duration: sample.clock.duration, wrapped: false,
                            direction: if rate < 0.0 { crate::extensions::PlaybackDirection::Backward } else { crate::extensions::PlaybackDirection::Forward },
                        };
                        assert!(crate::extensions::event_lifecycle::crossed_time((last_time + expected_time) * 0.5, clock));
                        assert!(!crate::extensions::event_lifecycle::crossed_time(if rate < 0.0 { last_time + 0.01 } else { last_time - 0.01 }, clock));
                    }
                    let (expected_pose, expected_context) = crate::standing_source::sample_source_montage(source_transition_sequence(entry).unwrap(), expected_time,
                        app.world().resource::<crate::standing_source::SourceMontageAssets>(), app.world().resource::<Assets<GraphClip>>(), app.world().resource::<Assets<Skeleton>>()).unwrap();
                    for (curve, expected) in &expected_context.curves {
                        assert!((context.curves[curve] - expected).abs() < 1e-5);
                    }
                    let player = app.world().get::<AnimationGraphPlayer>(target).unwrap();
                    assert!(player.get_error().is_none() && player.get_default_output_pose().is_some());
                    let actual_pose = player.get_global_input_data(GENERATED_TRANSITION_SLOT_POSE_KEY).unwrap().as_pose().unwrap();
                    for bone in expected_pose.paths.keys() {
                        let actual = actual_pose.get_bone(*bone).unwrap();
                        let expected = expected_pose.get_bone(*bone).unwrap();
                        if let (Some(a), Some(e)) = (actual.rotation, expected.rotation) { assert!(a.dot(e).abs() > 1.0 - 1e-5); }
                        if let (Some(a), Some(e)) = (actual.translation, expected.translation) { assert!((a-e).length() < 1e-5); }
                    }
                    if let Some(previous) = last_pose.as_ref() {
                        let previous: &bevy_animation_graph::core::pose::Pose = previous;
                        pose_changed |= actual_pose.paths.keys().any(|bone| actual_pose.get_bone(*bone).unwrap().rotation != previous.get_bone(*bone).unwrap().rotation);
                    }
                    last_pose = Some(actual_pose.clone());
                    last_time = expected_time;
                }
                assert_eq!(pose_changed, rate != 0.0);
            }
        }
    }

    #[test]
    fn transition_stack_preserves_interruptions_shorter_fades_and_natural_end() {
        let mut stack = AlsTransitionMontageStack::default();
        stack.begin(AlsTransitionPlayback::standing_dynamic(0.0));
        stack.advance(0.05);
        assert!((stack.latest().unwrap().weight - 0.15625).abs() < 1e-6);
        stack.advance(0.31);
        stack.begin(AlsTransitionPlayback::standing_dynamic(1.0));
        assert_eq!(stack.entries.len(), 2);
        assert_eq!(stack.entries[0].weight, 1.0);
        assert_eq!(stack.latest().unwrap().weight, 0.0);
        stack.advance(0.05);
        assert!((stack.entries[0].weight - 0.84375).abs() < 1e-6);
        assert!((stack.latest().unwrap().weight - 0.15625).abs() < 1e-6);
        let retained = stack.entries.iter().map(|entry| (entry.clip_time, entry.weight)).collect::<Vec<_>>();
        stack.begin(AlsTransitionPlayback::quick_stop(0.0, 1.75));
        assert_eq!(stack.entries.len(), 3);
        for (entry, (time, weight)) in stack.entries.iter().zip(retained) {
            assert_eq!(entry.clip_time, time);
            assert_eq!(entry.stop_start_weight, weight);
            assert_eq!(entry.stop_duration, 0.1);
        }
        stack.advance(0.05);
        assert!((stack.entries[0].weight - 0.84375 * 0.5).abs() < 1e-6);
        stack.advance(0.05);
        assert_eq!(stack.entries.len(), 1);
        assert_eq!(stack.latest().unwrap().weight, 1.0);
        stack.request_stop();
        stack.advance(0.2);
        assert!(!stack.latest().unwrap().active);
        stack.begin(AlsTransitionPlayback::crouching_dynamic(1.0));
        let end = stack.latest().unwrap().duration / stack.latest().unwrap().play_rate;
        stack.advance(end);
        assert_eq!(stack.latest().unwrap().weight, 1.0);
        assert_eq!(stack.latest().unwrap().clip_time, stack.latest().unwrap().duration);
        stack.advance(0.05);
        assert!((stack.latest().unwrap().weight - 0.84375).abs() < 1e-6);
        stack.advance(0.15);
        assert!(!stack.latest().unwrap().active);
        let instant = AlsTransitionPlayback::transition_animation(false, true, 0.0, 0.2, 1.5, 0.0);
        stack.begin(instant);
        stack.advance(0.0);
        assert_eq!(stack.latest().unwrap().weight, 1.0);
    }

    #[test]
    fn interrupted_transition_keeps_its_sampled_slot_contribution() {
        let mut app = crate::standing_source::tests::loaded_montage_app();
        app.add_message::<AlsQuickStopNotify>().add_systems(
            Update,
            (quick_stop_notify_system, bridge_root_transition_slot_samples).chain(),
        );
        let mut previous = AlsTransitionPlayback::standing_dynamic(0.0);
        previous.advance(0.24);
        let character = app.world_mut().spawn((
            crate::als::character::AlsCharacter,
            Transform::IDENTITY,
            RotationMode::VelocityDirection,
            Stance::Standing,
            AlsViewYaw(0.0),
            AlsInputCommand::default(),
            AlsTransitionMontageStack { entries: vec![previous] },
        )).id();
        let player = app.world_mut().spawn(AnimationGraphPlayer::new(Handle::default())).id();
        app.world_mut().spawn((
            AlsRootTransitionSlotSampler { character, target_player: player, reported_unready: false },
            AnimationGraphPlayer::new(Handle::default()),
        ));
        app.update();
        let before = *app.world().get::<AlsRootTransitionSlotRuntime>(character).unwrap();
        assert_eq!(before.weight, 1.0);
        let curves_before = app.world().get::<crate::extensions::SourceAnimationInputs>(player).unwrap().slot_input_context(GENERATED_TRANSITION_SLOT_POSE_KEY).unwrap().curves;
        assert!(curves_before.values().any(|value| value.abs() > 0.5));
        let pose_before = app.world().get::<AnimationGraphPlayer>(player).unwrap()
            .get_global_input_data(GENERATED_TRANSITION_SLOT_POSE_KEY).unwrap().clone().into_pose().unwrap();
        app.world_mut().write_message(AlsQuickStopNotify { character });
        app.update();
        let after = app.world().get::<AlsRootTransitionSlotRuntime>(character).unwrap();
        assert_eq!(after.weight, before.weight, "interrupting a full-weight transition discarded its contribution");
        assert_eq!(app.world().get::<crate::extensions::SourceAnimationInputs>(player).unwrap().slot_input_context(GENERATED_TRANSITION_SLOT_POSE_KEY).unwrap().curves, curves_before);
        let pose_after = app.world().get::<AnimationGraphPlayer>(player).unwrap()
            .get_global_input_data(GENERATED_TRANSITION_SLOT_POSE_KEY).unwrap().clone().into_pose().unwrap();
        for bone in pose_before.paths.keys() {
            let before = pose_before.get_bone(*bone).unwrap();
            let after = pose_after.get_bone(*bone).unwrap();
            assert_eq!(before.translation, after.translation);
            assert_eq!(before.rotation, after.rotation);
            assert_eq!(before.scale, after.scale);
        }
        app.world_mut().get_mut::<AlsTransitionMontageStack>(character).unwrap().advance(0.05);
        app.update();
        let stack = app.world().get::<AlsTransitionMontageStack>(character).unwrap();
        assert_eq!(stack.entries.len(), 2);
        let actual = app.world().get::<crate::extensions::SourceAnimationInputs>(player).unwrap().slot_input_context(GENERATED_TRANSITION_SLOT_POSE_KEY).unwrap();
        let mut expected = std::collections::HashMap::<crate::extensions::SourceCurveName, f32>::new();
        let total = stack.entries.iter().map(|entry| entry.weight).sum::<f32>().max(1.0);
        for entry in &stack.entries {
            let settings = &crate::standing_source::MONTAGES[source_transition_sequence(entry).unwrap()];
            for (name, value) in crate::get_up::sampled_curve_context(&settings.curves, entry.clip_time).curves {
                *expected.entry(name).or_default() += value * entry.weight / total;
            }
        }
        for (name, value) in expected {
            assert!((actual.curves[&name] - value).abs() < 1e-6, "curve {name} discarded a fading instance");
        }
    }

    #[test]
    fn stop_transition_uses_the_source_blueprint_pin_values() {
        let playback = AlsTransitionPlayback::stop(0.0);
        assert_eq!(playback.blend_in, 0.2);
        assert_eq!(playback.blend_out, 0.2);
        assert_eq!(playback.play_rate, 1.5);
        assert_eq!(playback.start_time, 0.4);
        assert_eq!(playback.duration, TRANSITION_SEQUENCE_DURATION);
    }

    #[test]
    fn quick_stop_uses_the_source_settings_range() {
        let body = Transform::IDENTITY;
        let input = AlsInputCommand::default();
        let (index, play_rate) =
            quick_stop_selection(RotationMode::VelocityDirection, &input, 0.0, &body, 0.0);
        assert_eq!(index, 0.0);
        assert_eq!(play_rate, QUICK_STOP_MIN_PLAY_RATE);
        let playback = AlsTransitionPlayback::quick_stop(index, play_rate);
        assert_eq!(playback.blend_in, QUICK_STOP_BLEND_IN);
        assert_eq!(playback.blend_out, QUICK_STOP_BLEND_OUT);
        assert_eq!(playback.start_time, QUICK_STOP_START_TIME);
    }

    #[test]
    fn quick_stop_notice_uses_retained_target_and_target_yaw_basis() {
        for (input, target_yaw, index, rate) in [
            (Vec2::ZERO, -90.0_f32, 1.0, 2.375),
            (Vec2::ZERO, 90.0, 0.0, 2.375),
            (Vec2::X, 90.0, 1.0, 2.375),
            (Vec2::NEG_X, -90.0, 0.0, 2.375),
            (Vec2::ZERO, -176.0, 0.0, 3.0277777),
        ] {
            let mut app = App::new();
            app.add_message::<AlsQuickStopNotify>()
                .add_systems(Update, quick_stop_notify_system);
            let character = app
                .world_mut()
                .spawn((
                    crate::als::character::AlsCharacter,
                    Transform::IDENTITY,
                    RotationMode::VelocityDirection,
                    Stance::Standing,
                    AlsViewYaw(0.0),
                    AlsInputCommand {
                        move_dir: input,
                        ..default()
                    },
                    crate::als::motion::AlsRotationResponseState {
                        initialized: true,
                        target_yaw: target_yaw.to_radians(),
                        ..default()
                    },
                ))
                .id();
            app.world_mut()
                .write_message(AlsQuickStopNotify { character });
            app.update();
            let playback = app.world().get::<AlsTransitionMontageStack>(character).unwrap().latest().unwrap();
            assert_eq!(playback.index, index, "quick stop target yaw {target_yaw}");
            assert!(
                (playback.play_rate - rate).abs() < 1e-5,
                "quick stop rate {target_yaw}: {}",
                playback.play_rate
            );
        }
    }

    #[test]
    fn root_transition_slot_accepts_the_source_dynamic_transition_montages() {
        let quick_stop = AlsTransitionPlayback::quick_stop(0.0, 1.75);
        let standing_transition = AlsTransitionPlayback::standing_transition_right();
        let crouching_stop = AlsTransitionPlayback::crouching_stop_right();
        let crouching_quick_stop = AlsTransitionPlayback::crouching_quick_stop(0.0, 1.75);
        let crouching_dynamic = AlsTransitionPlayback::crouching_dynamic(1.0);
        assert!(root_transition_slot_active(&quick_stop));
        assert!(root_transition_slot_active(&standing_transition));
        assert!(root_transition_slot_active(&crouching_stop));
        assert!(root_transition_slot_active(&crouching_quick_stop));
        assert!(root_transition_slot_active(&crouching_dynamic));
        assert!(root_transition_slot_active(&AlsTransitionPlayback::stop(
            0.0
        )));
        assert_eq!(
            source_transition_sequence(&AlsTransitionPlayback::stop(1.0)),
            Some("A_Als_Stop_Right")
        );
        let dynamic = AlsTransitionPlayback::standing_dynamic(1.0);
        assert!(root_transition_slot_active(&dynamic));
        assert_eq!(
            source_transition_sequence(&dynamic),
            Some("A_Als_Stand_DynamicTransition_Right")
        );
        assert_eq!(
            source_transition_sequence(&quick_stop),
            Some("A_Als_Stand_Transition_Left")
        );
        for (index, expected) in [
            (0.0, "A_Als_Crouch_Transition_Left"),
            (1.0, "A_Als_Crouch_Transition_Right"),
        ] {
            let playback = AlsTransitionPlayback::crouching_quick_stop(index, 1.75);
            assert_eq!(source_transition_sequence(&playback), Some(expected));
            assert_eq!(
                crate::standing_source::MONTAGES[expected]
                    .reference_clip
                    .as_deref(),
                Some("ALS/Animations/Base/A_Als_Stand_Pose.anim.ron")
            );
            assert!(
                (crate::standing_source::MONTAGES[expected].duration - playback.duration).abs()
                    < 1e-6
            );
        }
        assert_eq!(
            source_transition_sequence(&AlsTransitionPlayback::stop(1.0)),
            Some("A_Als_Stop_Right")
        );
        assert_eq!(
            (dynamic.start_time, dynamic.play_rate, dynamic.duration),
            (0.0, 1.5, 1.533333)
        );
        let mut inactive = AlsTransitionPlayback::quick_stop(0.0, 1.75);
        inactive.active = false;
        assert!(!root_transition_slot_active(&inactive));
    }

    #[test]
    fn root_transition_slot_preserves_the_source_variant_and_start_time() {
        let quick_stop = AlsTransitionPlayback::quick_stop(1.0, 2.0);
        assert_eq!(source_transition_sequence(&quick_stop), Some("A_Als_Stand_Transition_Right"));
        assert_eq!(quick_stop.start_time, QUICK_STOP_START_TIME);
        let crouching_stop = AlsTransitionPlayback::crouching_stop_right();
        assert_eq!(source_transition_sequence(&crouching_stop), Some("A_Als_Crouch_Transition_Right"));
        assert_eq!(crouching_stop.start_time, RIGHT_TRANSITION_START_TIME);
        let crouching_quick_stop = AlsTransitionPlayback::crouching_quick_stop(0.0, 2.0);
        assert_eq!(source_transition_sequence(&crouching_quick_stop), Some("A_Als_Crouch_Transition_Left"));
        assert_eq!(crouching_quick_stop.start_time, QUICK_STOP_START_TIME);
        let dynamic_left = AlsTransitionPlayback::crouching_dynamic(0.0);
        let dynamic_right = AlsTransitionPlayback::crouching_dynamic(1.0);
        assert_eq!(source_transition_sequence(&dynamic_left), Some("A_Als_Crouch_DynamicTransition_Left"));
        assert_eq!(source_transition_sequence(&dynamic_right), Some("A_Als_Crouch_DynamicTransition_Right"));
        assert_eq!(dynamic_left.start_time, 0.0);
    }

    #[test]
    fn roll_to_grounded_transition_selects_the_source_stance_asset_and_parameters() {
        let standing = AlsTransitionPlayback::roll_to_grounded_right(false);
        assert_eq!(standing.kind, AlsTransitionKind::StandingTransition);
        assert_eq!(source_transition_sequence(&standing), Some("A_Als_Stand_Transition_Right"));
        assert_eq!(standing.blend_in, 0.2);
        assert_eq!(standing.blend_out, 0.2);
        assert_eq!(standing.play_rate, 1.5);
        assert_eq!(standing.start_time, 0.2);
        let crouching = AlsTransitionPlayback::roll_to_grounded_right(true);
        assert_eq!(crouching.kind, AlsTransitionKind::CrouchingStop);
        assert_eq!(source_transition_sequence(&crouching), Some("A_Als_Crouch_Transition_Right"));
        assert_eq!(crouching.start_time, 0.2);
    }

    #[test]
    fn root_transition_slot_uses_crouching_curve_context_for_crouching_playback() {
        assert_eq!(
            root_transition_curve_key("RotationYawSpeed", true),
            Some("als_crouching_curve_rotation_yaw_speed")
        );
        assert_eq!(
            root_transition_curve_key("RotationYawSpeed", false),
            Some("als_standing_curve_rotation_yaw_speed")
        );
    }

    #[test]
    fn source_stop_request_fades_the_active_transition_with_its_source_blend_out() {
        for mut playback in [AlsTransitionPlayback::standing_transition_right(), AlsTransitionPlayback::crouching_stop_right()] {
            playback.advance(STOP_TRANSITION_BLEND_IN);
            assert_eq!(playback.weight, 1.0);
            let start = playback.clip_time;
            playback.request_stop();
            playback.advance(STOP_TRANSITION_BLEND_OUT * 0.5);
            assert!((playback.weight - 0.5).abs() < 1e-6);
            assert!((playback.clip_time - start - STOP_TRANSITION_BLEND_OUT * 0.5 * playback.play_rate).abs() < 1e-6);
            playback.advance(STOP_TRANSITION_BLEND_OUT * 0.5);
            assert!(!playback.active);
            assert_eq!(playback.weight, 0.0);
            assert!((playback.clip_time - start - STOP_TRANSITION_BLEND_OUT * playback.play_rate).abs() < 1e-6);
            let end = playback.clip_time;
            playback.advance(1.0);
            assert_eq!(playback.clip_time, end);
        }
    }
}
