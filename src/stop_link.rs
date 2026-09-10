use als_core::dynamics::{
    AlsGroundedDynamics, AlsLandingBrakingFriction,
    predict_grounded_stop_distance_with_braking_friction,
};
use als_core::input::AlsInputCommand;
use als_core::motion::{
    AlsInputOverride, AlsRotationResponseState, AlsViewYaw, view_relative_move_direction,
};
use als_core::orientation::world_forward_to_yaw;
use als_core::params::AlsMovementParams;
use als_core::start_stop::predict_stop_distance;
use als_core::state::{LocomotionState, RotationMode, Stance};
use als_core::transitions::{TransitionConfig, select_quick_stop};
use avian3d::prelude::LinearVelocity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::message::{Message, MessageReader};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::plugin::AnimationGraphSet;

use crate::character_assembly::AlsCharacterAvatar;

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
    StandingTransitionRight,
    StandingDynamic,
    CrouchingStop,
    CrouchingQuickStop,
    CrouchingDynamic,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsTransitionPlayback {
    pub active: bool,
    pub kind: AlsTransitionKind,
    pub elapsed: f32,
    pub clip_time: f32,
    pub index: f32,
    pub play_rate: f32,
    pub blend_in: f32,
    pub blend_out: f32,
    pub start_time: f32,
    pub duration: f32,
    pub weight: f32,
    pub stop_elapsed: Option<f32>,
    pub stop_start_weight: f32,
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
        Self {
            active: true,
            kind,
            elapsed: 0.0,
            clip_time: start_time,
            index,
            play_rate,
            blend_in,
            blend_out,
            start_time,
            duration,
            weight: 0.0,
            stop_elapsed: None,
            stop_start_weight: 0.0,
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
            AlsTransitionKind::StandingTransitionRight,
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
        if !self.active || self.stop_elapsed.is_some() {
            return;
        }
        self.stop_start_weight = transition_weight(
            self.elapsed,
            self.duration,
            self.start_time,
            self.play_rate,
            self.blend_in,
            self.blend_out,
        )
        .max(self.weight);
        self.stop_elapsed = Some(0.0);
    }

    pub fn advance(&mut self, delta_seconds: f32) {
        if !self.active {
            return;
        }
        self.elapsed += delta_seconds.max(0.0);
        let play_rate = self.play_rate.max(f32::EPSILON);
        self.clip_time = (self.start_time + self.elapsed * play_rate).min(self.duration);
        if let Some(stop_elapsed) = &mut self.stop_elapsed {
            *stop_elapsed += delta_seconds.max(0.0);
            let duration = self.blend_out.max(f32::EPSILON);
            self.weight = self.stop_start_weight * (1.0 - *stop_elapsed / duration).clamp(0.0, 1.0);
            if *stop_elapsed >= duration {
                self.active = false;
                self.weight = 0.0;
            }
            return;
        }
        let playback_duration = (self.duration - self.start_time).max(0.0) / play_rate;
        self.weight = transition_weight(
            self.elapsed,
            self.duration,
            self.start_time,
            play_rate,
            self.blend_in,
            self.blend_out,
        );
        if self.elapsed >= playback_duration {
            self.active = false;
            self.clip_time = self.duration;
            self.weight = 0.0;
        }
    }
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
const ROOT_TRANSITION_SLOT_GRAPH: &str = "graphs/ab_als/root_transition_slot.animgraph.ron";
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

pub fn transition_weight(
    elapsed: f32,
    duration: f32,
    start_time: f32,
    play_rate: f32,
    blend_in: f32,
    blend_out: f32,
) -> f32 {
    let playback_duration = (duration - start_time).max(0.0) / play_rate.max(f32::EPSILON);
    (elapsed / blend_in.max(f32::EPSILON))
        .min((playback_duration - elapsed) / blend_out.max(f32::EPSILON))
        .clamp(0.0, 1.0)
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
            .add_systems(
                Update,
                (
                    ensure_stop_match,
                    stop_link_system.after(ensure_stop_match),
                    quick_stop_notify_system,
                    transition_playback_system.after(quick_stop_notify_system),
                    ensure_root_transition_slot_samplers,
                    prepare_root_transition_slot_samples.after(transition_playback_system),
                ),
            )
            .add_systems(
                PostUpdate,
                bridge_root_transition_slot_samples.after(AnimationGraphSet::Final),
            );
    }
}

fn ensure_stop_match(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<als_core::character::AlsCharacter>,
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
                | AlsTransitionKind::StandingTransitionRight
                | AlsTransitionKind::StandingDynamic
                | AlsTransitionKind::CrouchingStop
                | AlsTransitionKind::CrouchingQuickStop
                | AlsTransitionKind::CrouchingDynamic
        )
}

fn root_transition_slot_index(playback: &AlsTransitionPlayback) -> f32 {
    match playback.kind {
        AlsTransitionKind::Stop | AlsTransitionKind::QuickStop => playback.index,
        AlsTransitionKind::StandingTransitionRight => 6.0,
        AlsTransitionKind::StandingDynamic => 7.0 + playback.index,
        AlsTransitionKind::CrouchingStop | AlsTransitionKind::CrouchingQuickStop => {
            2.0 + playback.index
        }
        AlsTransitionKind::CrouchingDynamic => 4.0 + playback.index,
    }
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
        AlsTransitionKind::StandingTransitionRight => Some("A_Als_Stand_Transition_Right"),
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
    asset_server: Res<AssetServer>,
    avatars: Query<(Entity, &AlsCharacterAvatar, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    samplers: Query<&AlsRootTransitionSlotSampler>,
    players: Query<&AnimationGraphPlayer>,
    mut commands: Commands,
) {
    for (avatar_entity, avatar, instance) in &avatars {
        let target_player = instance.player_entity();
        if samplers
            .iter()
            .any(|sampler| sampler.target_player == target_player)
        {
            continue;
        }
        let Ok(player) = players.get(target_player) else {
            continue;
        };
        commands.spawn((
            Name::new("als_root_transition_slot_sampler"),
            Children::default(),
            ChildOf(avatar_entity),
            AnimationGraphPlayer::new(player.skeleton().clone())
                .with_graph(asset_server.load(ROOT_TRANSITION_SLOT_GRAPH)),
            AlsRootTransitionSlotSampler {
                character: avatar.character,
                target_player,
                reported_unready: false,
            },
        ));
    }
}

fn prepare_root_transition_slot_samples(
    playbacks: Query<&AlsTransitionPlayback>,
    mut samplers: Query<(&AlsRootTransitionSlotSampler, &mut AnimationGraphPlayer)>,
) {
    for (sampler, mut player) in &mut samplers {
        let playback = playbacks
            .get(sampler.character)
            .ok()
            .filter(|playback| root_transition_slot_active(playback));
        player.set_global_input_data(
            "als_root_transition_slot_index",
            playback.map_or(0.0, root_transition_slot_index),
        );
        player.set_global_input_data(
            "als_root_transition_slot_weight",
            playback.map_or(0.0, |value| value.weight),
        );
        player.set_global_input_data(
            "als_root_transition_slot_play_rate",
            playback.map_or(1.75, |value| value.play_rate),
        );
        player.set_global_input_data(
            "als_root_transition_slot_start_time",
            playback.map_or(0.0, |value| value.start_time),
        );
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

fn bridge_root_transition_slot_samples(
    montage_assets: Res<crate::standing_source::SourceMontageAssets>,
    clips: Res<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>,
    skeletons: Res<Assets<bevy_animation_graph::core::skeleton::Skeleton>>,
    playbacks: Query<&AlsTransitionPlayback>,
    stances: Query<&Stance>,
    mut samplers: Query<(&mut AlsRootTransitionSlotSampler, &AnimationGraphPlayer)>,
    mut target_players: Query<&mut AnimationGraphPlayer, Without<AlsRootTransitionSlotSampler>>,
    mut commands: Commands,
) {
    for (mut sampler, source_player) in &mut samplers {
        let Ok(mut target_player) = target_players.get_mut(sampler.target_player) else {
            continue;
        };
        let playback = playbacks
            .get(sampler.character)
            .ok()
            .filter(|playback| root_transition_slot_active(playback));
        let crouching = playback.is_some_and(|value| value.is_crouching())
            || stances
                .get(sampler.character)
                .is_ok_and(|stance| *stance == Stance::Crouching);
        let Some(playback) = playback else {
            als_ext::source_gameplay_tag_input::set_slot_input_context(
                sampler.target_player,
                GENERATED_TRANSITION_SLOT_POSE_KEY,
                None,
            );
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
                .insert(AlsRootTransitionSlotRuntime::default());
            continue;
        };
        let source_sequence = source_transition_sequence(playback);
        let sample = source_sequence.and_then(|name| {
            crate::standing_source::sample_source_montage(
                name,
                playback.clip_time,
                &montage_assets,
                &clips,
                &skeletons,
            )
        });
        let pose = if source_sequence.is_some() {
            sample.as_ref().map(|(pose, _)| pose.clone())
        } else {
            source_player.get_default_output_pose().cloned()
        };
        let Some(pose) = pose else {
            if !sampler.reported_unready {
                warn!(
                    "[TRANSITION-SLOT] sampler unavailable character={:?} error={:?}",
                    sampler.character,
                    source_player.get_error()
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
        target_player.set_global_input_data(ROOT_TRANSITION_SLOT_WEIGHT_KEY, playback.weight);
        target_player.set_global_input_data(ROOT_TRANSITION_SLOT_LAYER_WEIGHT_KEY, 1.0_f32);
        target_player
            .set_global_input_data(GENERATED_TRANSITION_SLOT_POSE_KEY, DataValue::Pose(pose));
        target_player.set_global_input_data(GENERATED_TRANSITION_SLOT_WEIGHT_KEY, playback.weight);
        target_player.set_global_input_data(GENERATED_TRANSITION_SLOT_LAYER_WEIGHT_KEY, 1.0_f32);
        target_player
            .set_global_input_data(GENERATED_TRANSITION_SLOT_MESH_SPACE_ADDITIVE_KEY, 1.0_f32);
        write_root_transition_slot_curves(&mut target_player, crouching);
        sampler.reported_unready = false;
        als_ext::source_gameplay_tag_input::set_slot_input_context(
            sampler.target_player,
            GENERATED_TRANSITION_SLOT_POSE_KEY,
            sample.map(|(_, context)| context),
        );
        commands
            .entity(sampler.character)
            .insert(AlsRootTransitionSlotRuntime {
                active: true,
                weight: playback.weight,
                pose_ready: true,
            });
    }
}

#[allow(clippy::type_complexity)]
pub fn stop_link_system(
    time: Res<Time>,
    mut q: Query<
        (
            &LinearVelocity,
            &LocomotionState,
            &Stance,
            &RotationMode,
            &Transform,
            &AlsViewYaw,
            &als_core::character::AlsMovementParamsHandle,
            Option<&AlsGroundedDynamics>,
            Option<&AlsLandingBrakingFriction>,
            &mut AlsStopMatch,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<als_core::character::AlsCharacter>,
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
        view_yaw,
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
    let hspeed = Vec2::new(vel.x, vel.z).length();
    let input_move = effective.move_dir.length();
    if input_move > STOP_RELEASE_INPUT_EPSILON {
        let direction = view_relative_move_direction(effective.move_dir, view_yaw.0);
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
            &AlsViewYaw,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            Option<&AlsRotationResponseState>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    mut commands: Commands,
) {
    for notice in notices.read() {
        let Ok((body, rotation_mode, stance, view_yaw, command, override_command, rotation)) =
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
            quick_stop_selection(*rotation_mode, input, view_yaw.0, body, target_yaw);
        let playback = if *stance == Stance::Crouching {
            AlsTransitionPlayback::crouching_quick_stop(index, play_rate)
        } else {
            AlsTransitionPlayback::quick_stop(index, play_rate)
        };
        commands.entity(notice.character).insert(playback);
    }
}

fn transition_playback_system(time: Res<Time>, mut playbacks: Query<&mut AlsTransitionPlayback>) {
    for mut playback in &mut playbacks {
        playback.advance(time.delta_secs());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                    als_core::character::AlsCharacter,
                    Transform::IDENTITY,
                    RotationMode::VelocityDirection,
                    Stance::Standing,
                    AlsViewYaw(0.0),
                    AlsInputCommand {
                        move_dir: input,
                        ..default()
                    },
                    als_core::motion::AlsRotationResponseState {
                        initialized: true,
                        target_yaw: target_yaw.to_radians(),
                        ..default()
                    },
                ))
                .id();
            app.world_mut()
                .write_message(AlsQuickStopNotify { character });
            app.update();
            let playback = app.world().get::<AlsTransitionPlayback>(character).unwrap();
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
            root_transition_slot_index(&AlsTransitionPlayback::stop(1.0)),
            1.0
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
                Some("animations/Base/A_Als_Stand_Pose.anim.ron")
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
        assert_eq!(root_transition_slot_index(&quick_stop), 1.0);
        assert_eq!(quick_stop.start_time, QUICK_STOP_START_TIME);
        let crouching_stop = AlsTransitionPlayback::crouching_stop_right();
        assert_eq!(root_transition_slot_index(&crouching_stop), 3.0);
        assert_eq!(crouching_stop.start_time, RIGHT_TRANSITION_START_TIME);
        let crouching_quick_stop = AlsTransitionPlayback::crouching_quick_stop(0.0, 2.0);
        assert_eq!(root_transition_slot_index(&crouching_quick_stop), 2.0);
        assert_eq!(crouching_quick_stop.start_time, QUICK_STOP_START_TIME);
        let dynamic_left = AlsTransitionPlayback::crouching_dynamic(0.0);
        let dynamic_right = AlsTransitionPlayback::crouching_dynamic(1.0);
        assert_eq!(root_transition_slot_index(&dynamic_left), 4.0);
        assert_eq!(root_transition_slot_index(&dynamic_right), 5.0);
        assert_eq!(dynamic_left.start_time, 0.0);
    }

    #[test]
    fn roll_to_grounded_transition_selects_the_source_stance_asset_and_parameters() {
        let standing = AlsTransitionPlayback::roll_to_grounded_right(false);
        assert_eq!(standing.kind, AlsTransitionKind::StandingTransitionRight);
        assert_eq!(root_transition_slot_index(&standing), 6.0);
        assert_eq!(standing.blend_in, 0.2);
        assert_eq!(standing.blend_out, 0.2);
        assert_eq!(standing.play_rate, 1.5);
        assert_eq!(standing.start_time, 0.2);
        let crouching = AlsTransitionPlayback::roll_to_grounded_right(true);
        assert_eq!(crouching.kind, AlsTransitionKind::CrouchingStop);
        assert_eq!(root_transition_slot_index(&crouching), 3.0);
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
