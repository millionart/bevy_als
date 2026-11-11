use crate::als::rotate_in_place::RotateInPlaceState;
use crate::als::state::Stance;
use crate::als::turn_in_place::{TurnInPlaceConfig, TurnVariant, wrap_angle_degrees};
use crate::extensions::source_slot::{MONTAGE_EVALUATION_WEIGHT_THRESHOLD, montage_evaluation_denominator};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;
use std::collections::HashMap;

use crate::character_assembly::AlsCharacterAvatar;

pub const TURN_GROUND_SPEED_EPS: f32 = 0.05;
pub const TURN_WEIGHT_IN: f32 = 5.0;
pub const TURN_WEIGHT_OUT: f32 = 5.0;
pub const TURN_INERTIALIZATION_DURATION: f32 = 0.2;
pub const TURN_MONTAGE_BLEND_DURATION: f32 = 0.2;
pub const TURN_DUR_90: f32 = 2.0;
pub const TURN_DUR_180: f32 = 2.0;
pub const CROUCH_TURN_DUR_180: f32 = 2.33333325;
pub const ROTATE_DUR: f32 = 1.0;
const STANDING_TURN_SLOT_POSE_KEY: &str = "als_standing_turn_pose";
const STANDING_TURN_SLOT_LAYER_WEIGHT_KEY: &str = "als_standing_turn_layer_weight";
const STANDING_TURN_INERTIALIZATION_EPOCH_KEY: &str = "als_standing_turn_inertialization_epoch";
const STANDING_TURN_INERTIALIZATION_DURATION_KEY: &str =
    "als_standing_turn_inertialization_duration";
const CROUCHING_TURN_SLOT_POSE_KEY: &str = "als_crouching_turn_pose";
const CROUCHING_TURN_SLOT_LAYER_WEIGHT_KEY: &str = "als_crouching_turn_layer_weight";
const CROUCHING_TURN_INERTIALIZATION_EPOCH_KEY: &str = "als_crouching_turn_inertialization_epoch";
const CROUCHING_TURN_INERTIALIZATION_DURATION_KEY: &str =
    "als_crouching_turn_inertialization_duration";

#[derive(Clone, Copy)]
struct TurnSlotContract {
    index_key: &'static str,
    weight_key: &'static str,
    play_rate_key: &'static str,
    pose_key: &'static str,
    layer_weight_key: &'static str,
    generated_pose_key: &'static str,
    generated_weight_key: &'static str,
    generated_layer_weight_key: &'static str,
    inertialization_epoch_key: &'static str,
    inertialization_duration_key: &'static str,
}

fn turn_slot_contract(crouching: bool) -> TurnSlotContract {
    if crouching {
        TurnSlotContract {
            index_key: "als_crouching_turn_index",
            weight_key: "als_crouching_turn_weight",
            play_rate_key: "als_crouching_turn_play_rate",
            pose_key: CROUCHING_TURN_SLOT_POSE_KEY,
            layer_weight_key: CROUCHING_TURN_SLOT_LAYER_WEIGHT_KEY,
            generated_pose_key: "turn_in_place_crouching.pose",
            generated_weight_key: "turn_in_place_crouching.weight",
            generated_layer_weight_key: "turn_in_place_crouching.layer_weight",
            inertialization_epoch_key: CROUCHING_TURN_INERTIALIZATION_EPOCH_KEY,
            inertialization_duration_key: CROUCHING_TURN_INERTIALIZATION_DURATION_KEY,
        }
    } else {
        TurnSlotContract {
            index_key: "als_standing_turn_index",
            weight_key: "als_standing_turn_weight",
            play_rate_key: "als_standing_turn_play_rate",
            pose_key: STANDING_TURN_SLOT_POSE_KEY,
            layer_weight_key: STANDING_TURN_SLOT_LAYER_WEIGHT_KEY,
            generated_pose_key: "turn_in_place_standing.pose",
            generated_weight_key: "turn_in_place_standing.weight",
            generated_layer_weight_key: "turn_in_place_standing.layer_weight",
            inertialization_epoch_key: STANDING_TURN_INERTIALIZATION_EPOCH_KEY,
            inertialization_duration_key: STANDING_TURN_INERTIALIZATION_DURATION_KEY,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsTurnDrive {
    pub playing: bool,
    pub elapsed: f32,
    pub clip_time: f32,
    pub duration: f32,
    pub play_rate: f32,
    pub lr: f32,
    pub is_180: bool,
    pub weight: f32,
    pub rotate_active: bool,
    pub rotate_weight: f32,
    pub rotate_lr: f32,
    pub rotate_rate: f32,
    pub rotate_clip_time: f32,
    pub rotate_cycle_completed: bool,
    pub rotate_completion_event: bool,
    pub rotate_finishing: bool,
    pub inertialization_request_epoch: f32,
    pub rotate_state: RotateInPlaceState,
}

impl Default for AlsTurnDrive {
    fn default() -> Self {
        Self {
            playing: false,
            elapsed: 0.0,
            clip_time: 0.0,
            duration: TURN_DUR_90,
            play_rate: 1.0,
            lr: 0.0,
            is_180: false,
            weight: 0.0,
            rotate_active: false,
            rotate_weight: 0.0,
            rotate_lr: 0.0,
            rotate_rate: 0.0,
            rotate_clip_time: 0.0,
            rotate_cycle_completed: false,
            rotate_completion_event: false,
            rotate_finishing: false,
            inertialization_request_epoch: 0.0,
            rotate_state: RotateInPlaceState::default(),
        }
    }
}

impl AlsTurnDrive {
    pub fn advance(&mut self, dt: f32) {
        self.rotate_cycle_completed = false;
        self.rotate_completion_event = false;
        if self.rotate_active {
            let next_clip_time = self.rotate_clip_time + dt.max(0.0) * self.rotate_rate.max(0.0);
            self.rotate_cycle_completed = next_clip_time >= ROTATE_DUR;
            self.rotate_completion_event = self.rotate_cycle_completed;
            self.rotate_clip_time = next_clip_time.rem_euclid(ROTATE_DUR);
            self.rotate_finishing = false;
        } else if self.rotate_finishing {
            self.rotate_clip_time =
                (self.rotate_clip_time + dt.max(0.0) * self.rotate_rate.max(0.0)).min(ROTATE_DUR);
            if self.rotate_clip_time >= ROTATE_DUR {
                self.rotate_finishing = false;
                self.rotate_completion_event = true;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlsTurnMontage {
    pub id: u64,
    pub elapsed: f32,
    pub duration: f32,
    pub play_rate: f32,
    pub lr: f32,
    pub is_180: bool,
    pub crouching: bool,
    pub sampling_ready: bool,
    pub blend_in_elapsed: f32,
    pub blend_out_elapsed: Option<f32>,
    pub blend_out_start_weight: f32,
    pub blend_out_duration: f32,
}

impl AlsTurnMontage {
    pub fn weight(&self) -> f32 {
        if let Some(elapsed) = self.blend_out_elapsed {
            return self.blend_out_start_weight
                * (1.0 - crate::stop_link::transition_blend_amount(elapsed, self.blend_out_duration));
        }
        crate::stop_link::transition_blend_amount(self.blend_in_elapsed, TURN_MONTAGE_BLEND_DURATION)
    }

    fn playing(&self) -> bool {
        self.blend_out_elapsed.is_none() && self.elapsed < self.duration
    }
}

#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct AlsTurnMontageStack {
    pub entries: Vec<AlsTurnMontage>,
    owner_player: Option<Entity>,
    next_id: u64,
    inertialization_pending: bool,
    had_instances_before_advance: bool,
}

fn reconcile_turn_player(
    stack: &mut AlsTurnMontageStack,
    drive: &mut AlsTurnDrive,
    current: Option<Entity>,
) -> Option<Entity> {
    if stack.owner_player == current {
        return None;
    }
    let previous = stack.owner_player;
    *stack = AlsTurnMontageStack {
        owner_player: current,
        ..default()
    };
    *drive = AlsTurnDrive::default();
    previous
}

impl AlsTurnMontageStack {
    pub(crate) fn has_montage_at_pose_tick(&self) -> bool {
        self.had_instances_before_advance || !self.entries.is_empty()
    }

    fn begin(&mut self, lr: f32, is_180: bool, duration: f32, play_rate: f32, crouching: bool) {
        for entry in &mut self.entries {
            if entry.blend_out_elapsed.is_none() {
                entry.blend_out_start_weight = entry.weight();
                entry.blend_out_elapsed = Some(0.0);
                entry.blend_out_duration = TURN_MONTAGE_BLEND_DURATION;
            }
        }
        self.entries.push(AlsTurnMontage {
            id: self.next_id,
            elapsed: 0.0,
            duration,
            play_rate,
            lr,
            is_180,
            crouching,
            sampling_ready: false,
            blend_in_elapsed: 0.0,
            blend_out_elapsed: None,
            blend_out_start_weight: 1.0,
            blend_out_duration: TURN_MONTAGE_BLEND_DURATION,
        });
        self.next_id = self.next_id.wrapping_add(1);
    }

    fn advance(&mut self, delta_seconds: f32) -> bool {
        self.had_instances_before_advance = !self.entries.is_empty();
        let delta_seconds = delta_seconds.max(0.0);
        let mut natural_end = std::mem::take(&mut self.inertialization_pending);
        for entry in &mut self.entries {
            if !entry.sampling_ready {
                continue;
            }
            let play_rate = entry.play_rate.max(0.01);
            entry.elapsed += delta_seconds * play_rate;
            entry.blend_in_elapsed += delta_seconds;
            if let Some(blend_out_elapsed) = &mut entry.blend_out_elapsed {
                *blend_out_elapsed += delta_seconds;
            } else if delta_seconds > 1e-8 && (entry.duration - entry.elapsed) / play_rate <= 1e-4 {
                natural_end = true;
                entry.blend_out_elapsed = Some(TURN_MONTAGE_BLEND_DURATION);
                entry.blend_out_start_weight = 0.0;
            }
        }
        self.entries.retain(|entry| entry.blend_out_elapsed.is_none() || entry.weight() > f32::EPSILON);
        natural_end
    }

    pub fn request_stop(&mut self) {
        self.inertialization_pending |= self
            .entries
            .iter()
            .any(|entry| entry.blend_out_elapsed.is_none());
        self.entries.retain(|entry| {
            entry.blend_out_elapsed.is_some()
                && entry.sampling_ready
                && entry.weight() > f32::EPSILON
        });
    }

    pub(crate) fn stop_for_group_replacement(&mut self, duration: f32) {
        let duration = duration.max(0.0);
        for entry in &mut self.entries {
            if entry.blend_out_elapsed.is_some() && duration >= entry.blend_out_duration {
                continue;
            }
            entry.blend_out_start_weight = entry.weight();
            entry.blend_out_elapsed = Some(0.0);
            entry.blend_out_duration = duration;
        }
        self.entries.retain(|entry| entry.weight() > f32::EPSILON);
    }

    fn latest_playing(&self) -> Option<&AlsTurnMontage> {
        self.entries.iter().rev().find(|entry| entry.playing())
    }
}

fn synchronize_turn_drive(drive: &mut AlsTurnDrive, stack: &AlsTurnMontageStack) {
    let Some(latest) = stack.latest_playing().or_else(|| stack.entries.last()) else {
        drive.playing = false;
        drive.elapsed = 0.0;
        drive.clip_time = 0.0;
        drive.weight = 0.0;
        return;
    };
    drive.playing = latest.playing();
    drive.elapsed = latest.elapsed;
    drive.clip_time = latest.elapsed.min(latest.duration);
    drive.duration = latest.duration;
    drive.play_rate = latest.play_rate;
    drive.lr = latest.lr;
    drive.is_180 = latest.is_180;
    drive.weight = latest.weight();
}

fn variant_params(variant: TurnVariant, crouching: bool) -> (f32, bool, f32) {
    match variant {
        TurnVariant::Left90 => (0.0, false, TURN_DUR_90),
        TurnVariant::Right90 => (1.0, false, TURN_DUR_90),
        TurnVariant::Left180 => (
            0.0,
            true,
            if crouching {
                CROUCH_TURN_DUR_180
            } else {
                TURN_DUR_180
            },
        ),
        TurnVariant::Right180 => (
            1.0,
            true,
            if crouching {
                CROUCH_TURN_DUR_180
            } else {
                TURN_DUR_180
            },
        ),
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceTurnRequest {
    pub variant: TurnVariant,
    pub yaw: f32,
    pub crouching: bool,
}

impl SourceTurnRequest {
    pub(crate) fn source_sequence(self) -> &'static str {
        let (lr, is_180, _) = variant_params(self.variant, self.crouching);
        source_turn_sequence(self.crouching, is_180, lr)
    }

    pub(crate) fn play(self, stack: &mut AlsTurnMontageStack) -> f32 {
        let (lr, is_180, _) = variant_params(self.variant, self.crouching);
        let duration = crate::standing_source::MONTAGES[self.source_sequence()].duration;
        let play_rate = TurnInPlaceConfig::default().play_rate;
        stack.begin(lr, is_180, duration, play_rate, self.crouching);
        if self.crouching {
            play_rate
        } else {
            play_rate * (self.yaw / if is_180 { 180.0 } else { 90.0 }).abs()
        }
    }
}

pub(crate) fn source_turn_sequence(crouching: bool, is_180: bool, lr: f32) -> &'static str {
    match (crouching, is_180, lr <= 0.5) {
        (false, false, true) => "A_Als_Turn_90_Left",
        (false, false, false) => "A_Als_Turn_90_Right",
        (false, true, true) => "A_Als_Turn_180_Left",
        (false, true, false) => "A_Als_Turn_180_Right",
        (true, false, true) => "A_Als_Crouch_Turn_90_Left",
        (true, false, false) => "A_Als_Crouch_Turn_90_Right",
        (true, true, true) => "A_Als_Crouch_Turn_180_Left",
        (true, true, false) => "A_Als_Crouch_Turn_180_Right",
    }
}

pub(crate) fn character_turn_angle_degrees(view_yaw_degrees: f32, body_yaw_degrees: f32) -> f32 {
    crate::als::head::unwind_degrees(wrap_angle_degrees(-view_yaw_degrees) + body_yaw_degrees)
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsStandingTurnSlotRuntime {
    pub active: bool,
    pub weight: f32,
    pub pose_ready: bool,
    pub inertialization_request_epoch: f32,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsTurnSlotBridgeSet;

pub struct TurnLayerPlugin;

impl Plugin for TurnLayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                ensure_turn_drive,
                ensure_turn_montage_stack,
                turn_pin_driver,
            )
                .chain(),
        )
        .configure_sets(
            PostUpdate,
            AlsTurnSlotBridgeSet
                .after(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs)
                .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
        )
        .add_systems(
            PostUpdate,
            bridge_turn_slot_samples.in_set(AlsTurnSlotBridgeSet),
        );
    }
}

fn ensure_turn_drive(
    mut commands: Commands,
    avatars: Query<
        Entity,
        (
            With<AlsCharacterAvatar>,
            With<AnimatedSceneHandle>,
            Without<AlsTurnDrive>,
        ),
    >,
) {
    for avatar in &avatars {
        commands.entity(avatar).insert(AlsTurnDrive::default());
    }
}

fn ensure_turn_montage_stack(
    mut commands: Commands,
    avatars: Query<Entity, (With<AlsTurnDrive>, Without<AlsTurnMontageStack>)>,
) {
    for avatar in &avatars {
        commands
            .entity(avatar)
            .insert(AlsTurnMontageStack::default());
    }
}

#[cfg(test)]
fn turn_slot_active(drive: &AlsTurnDrive) -> bool {
    !drive.rotate_active && (drive.playing || drive.weight > f32::EPSILON)
}

fn turn_slot_index(drive: &AlsTurnDrive) -> f32 {
    turn_slot_index_for(drive.is_180, drive.lr)
}

fn turn_slot_index_for(is_180: bool, lr: f32) -> f32 {
    match (is_180, lr <= 0.5) {
        (false, true) => 0.0,
        (false, false) => 1.0,
        (true, true) => 2.0,
        (true, false) => 3.0,
    }
}

#[cfg(test)]
fn source_turn_slot_is_active(stack: &AlsTurnMontageStack, crouching: bool) -> bool {
    stack.entries.iter().any(|entry| {
        entry.crouching == crouching && (entry.playing() || entry.weight() > f32::EPSILON)
    })
}

fn request_turn_inertialization(natural_end: bool, drive: &mut AlsTurnDrive) -> bool {
    if !natural_end {
        return false;
    }
    drive.inertialization_request_epoch += 1.0;
    true
}

fn compose_turn_montage_poses(
    stack: &AlsTurnMontageStack,
    poses: &HashMap<u64, bevy_animation_graph::core::pose::Pose>,
    crouching: bool,
) -> Option<(bevy_animation_graph::core::pose::Pose, f32, f32)> {
    let entries = stack
        .entries
        .iter()
        .filter_map(|entry| {
            let weight = entry.weight();
            (entry.crouching == crouching && weight > MONTAGE_EVALUATION_WEIGHT_THRESHOLD)
                .then(|| poses.get(&entry.id).cloned().map(|pose| (pose, weight)))
                .flatten()
        })
        .collect::<Vec<_>>();
    let total_weight = entries.iter().map(|(_, weight)| *weight).sum::<f32>();
    let slot_weight = total_weight.min(1.0);
    let denominator = slot_weight * montage_evaluation_denominator(total_weight);
    let output = entries.into_iter()
        .map(|(pose, weight)| pose.scalar_mult(weight / denominator))
        .reduce(|accumulated, pose| accumulated.linear_add(&pose))?;
    Some((output, slot_weight, denominator))
}

fn write_turn_inertialization_inputs(player: &mut AnimationGraphPlayer, drive: &AlsTurnDrive, requested: bool) {
    for crouching in [false, true] {
        let contract = turn_slot_contract(crouching);
        player.set_global_input_data(
            contract.inertialization_epoch_key,
            if requested { drive.inertialization_request_epoch } else { 0.0 },
        );
        player.set_global_input_data(
            contract.inertialization_duration_key,
            TURN_INERTIALIZATION_DURATION,
        );
    }
}

fn clear_turn_slot_inputs(player: &mut AnimationGraphPlayer) {
    for crouching in [false, true] {
        let contract = turn_slot_contract(crouching);
        player.set_global_input_data(contract.weight_key, 0.0_f32);
        player.set_global_input_data(contract.layer_weight_key, 0.0_f32);
        player.set_global_input_data(contract.generated_weight_key, 0.0_f32);
        player.set_global_input_data(contract.generated_layer_weight_key, 0.0_f32);
    }
}

fn clear_old_turn_player(
    player: &mut AnimationGraphPlayer,
    source_inputs: Option<&mut crate::extensions::SourceAnimationInputs>,
) {
    write_turn_pins(player, &AlsTurnDrive::default(), false);
    write_turn_inertialization_inputs(player, &AlsTurnDrive::default(), false);
    clear_turn_slot_inputs(player);
    if let Some(source_inputs) = source_inputs {
        for crouching in [false, true] {
            source_inputs.set_slot_input_context(turn_slot_contract(crouching).generated_pose_key, None);
        }
    }
}

fn bridge_turn_slot_samples(
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
    montage_assets: Res<crate::standing_source::SourceMontageAssets>,
    clips: Res<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>,
    skeletons: Res<Assets<bevy_animation_graph::core::skeleton::Skeleton>>,
    mut avatars: Query<
        (
            Entity,
            &AnimatedSceneInstance,
            &mut AlsTurnDrive,
            &mut AlsTurnMontageStack,
        ),
        With<AlsCharacterAvatar>,
    >,
    mut players: Query<&mut AnimationGraphPlayer>,
    mut commands: Commands,
) {
    for (avatar, instance, mut drive, mut stack) in &mut avatars {
        let player_entity = instance.player_entity();
        if let Some(old_player) = reconcile_turn_player(&mut stack, &mut drive, Some(player_entity))
            && let Ok(mut player) = players.get_mut(old_player)
        {
            let mut old_inputs = source_inputs.get_mut(old_player).ok();
            clear_old_turn_player(&mut player, old_inputs.as_deref_mut());
        }
        let Ok(mut player) = players.get_mut(player_entity) else {
            continue;
        };
        let mut poses = HashMap::new();
        let mut contexts = HashMap::new();
        for entry in &mut stack.entries {
            if let Some((pose, mut context)) = crate::standing_source::sample_source_montage(
                source_turn_sequence(entry.crouching, entry.is_180, entry.lr),
                entry.elapsed,
                &montage_assets,
                &clips,
                &skeletons,
            ) {
                let weight = entry.weight();
                for sample in &mut context.asset_samples {
                    sample.identity = format!("turn:{}", entry.id);
                    sample.clock.weight = weight;
                    sample.interrupted = entry.blend_out_elapsed.is_some();
                }
                entry.sampling_ready = true;
                poses.insert(entry.id, pose);
                if weight > MONTAGE_EVALUATION_WEIGHT_THRESHOLD {
                    contexts.insert(entry.id, context);
                }
            }
        }
        clear_turn_slot_inputs(&mut player);
        let mut runtime = AlsStandingTurnSlotRuntime {
            active: !stack.entries.is_empty(),
            pose_ready: stack
                .entries
                .iter()
                .all(|entry| poses.contains_key(&entry.id)),
            inertialization_request_epoch: drive.inertialization_request_epoch,
            ..default()
        };
        for crouching in [false, true] {
            let contract = turn_slot_contract(crouching);
            source_inputs.get_mut(player_entity).unwrap().set_slot_input_context(contract.generated_pose_key,
                None,);
            let Some((pose, weight, denominator)) = compose_turn_montage_poses(&stack, &poses, crouching) else {
                continue;
            };
            let mut context = crate::extensions::source_pose_nodes::blend_animation_contexts(
                stack
                    .entries
                    .iter()
                    .filter(|entry| entry.crouching == crouching)
                    .filter_map(|entry| {
                        contexts
                            .get(&entry.id)
                            .cloned()
                            .map(|context| (context, entry.weight() / denominator))
                    }),
            );
            context.slot_has_multiple_montages = stack.entries.iter()
                .filter(|entry| entry.crouching == crouching && contexts.contains_key(&entry.id))
                .nth(1).is_some();
            player.set_global_input_data(contract.pose_key, DataValue::Pose(pose.clone()));
            player.set_global_input_data(contract.weight_key, weight);
            player.set_global_input_data(contract.layer_weight_key, 1.0_f32);
            player.set_global_input_data(contract.generated_pose_key, DataValue::Pose(pose));
            player.set_global_input_data(contract.generated_weight_key, weight);
            player.set_global_input_data(contract.generated_layer_weight_key, 1.0_f32);
            source_inputs.get_mut(player_entity).unwrap().set_slot_input_context(contract.generated_pose_key,
                Some(context),);
            runtime.weight = (runtime.weight + weight).min(1.0);
        }
        commands.entity(avatar).insert(runtime);
    }
}

fn write_turn_pins(player: &mut AnimationGraphPlayer, drive: &AlsTurnDrive, crouching: bool) {
    player.set_input_data("turn_weight", DataValue::from(drive.weight));
    player.set_input_data("turn_lr", DataValue::from(drive.lr));
    player.set_input_data(
        "turn_180",
        DataValue::from(if drive.is_180 { 1.0f32 } else { 0.0f32 }),
    );
    player.set_input_data("turn_clip_time", DataValue::from(drive.clip_time));
    let rotate_w = if drive.rotate_active {
        drive.rotate_weight
    } else {
        0.0
    };
    player.set_input_data("rotate_weight", DataValue::from(rotate_w));
    player.set_input_data("rotate_lr", DataValue::from(drive.rotate_lr));
    player.set_input_data("rotate_play_rate", DataValue::from(drive.rotate_rate));
    for slot_crouching in [false, true] {
        let contract = turn_slot_contract(slot_crouching);
        player.set_global_input_data(contract.index_key, turn_slot_index(drive));
        player.set_global_input_data(
            contract.weight_key,
            if slot_crouching == crouching {
                drive.weight
            } else {
                0.0_f32
            },
        );
        player.set_global_input_data(contract.play_rate_key, drive.play_rate);
    }
}

#[allow(clippy::type_complexity)]
fn turn_pin_driver(
    time: Res<Time>,
    stances: Query<(&Stance, Option<&crate::als_camera::AlsCameraCustomTimeDilation>), With<crate::als::character::AlsCharacter>>,
    mut source_inputs: Query<&mut crate::extensions::SourceAnimationInputs>,
    mut drives: Query<
        (
            Entity,
            &ChildOf,
            &mut AlsTurnDrive,
            &mut AlsTurnMontageStack,
        ),
        With<AlsCharacterAvatar>,
    >,
    instances: Query<&AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    for (entity, parent, mut drive, mut stack) in &mut drives {
        let Ok((stance, custom_dilation)) = stances.get(parent.parent()) else {
            continue;
        };
        let dt = time.delta_secs() * custom_dilation.map_or(1.0, |value| value.0);
        let current_player = instances.get(entity).ok()
            .map(AnimatedSceneInstance::player_entity)
            .filter(|player| players.contains(*player));
        if let Some(old_player) = reconcile_turn_player(&mut stack, &mut drive, current_player)
            && let Ok(mut player) = players.get_mut(old_player)
        {
            let mut old_inputs = source_inputs.get_mut(old_player).ok();
            clear_old_turn_player(&mut player, old_inputs.as_deref_mut());
        }
        let natural_end = stack.advance(dt);
        synchronize_turn_drive(&mut drive, &stack);
        drive.advance(dt);
        let requested = request_turn_inertialization(natural_end, &mut drive);
        if requested {
            warn!(
                "[TURN-SLOT][INERTIALIZATION-REQUEST] avatar={entity:?} epoch={:.0} duration={TURN_INERTIALIZATION_DURATION:.3}",
                drive.inertialization_request_epoch
            );
        }
        if let Some(mut player) = current_player.and_then(|player| players.get_mut(player).ok()) {
            write_turn_pins(&mut player, &drive, *stance == Stance::Crouching);
            write_turn_inertialization_inputs(&mut player, &drive, requested);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
    use bevy_animation_graph::AnimationGraphPlugin;
    use crate::als::motion::AlsViewYaw;
    use crate::als::rotate_in_place::{RotateInPlaceConfig, RotateSide, update_rotate_in_place};
    use crate::als::state::{LocomotionState, RotationMode};
    use crate::als::turn_in_place::TurnInPlaceDetector;
    use avian3d::prelude::LinearVelocity;

    #[test]
    fn current_slot_samples_reach_post_physics_graph_evaluation() {
        use bevy_animation_graph::core::{animation_graph::AnimationGraph, animation_node::AnimationNode,
            edge_data::{DataSpec, DataValue}};
        use bevy_animation_graph::core::plugin::AnimationGraphSet;
        use crate::extensions::post_physics_animation::{evaluate_players, PostPhysicsAnimation, PostPhysicsAnimationSet};
        #[derive(Resource, Default)]
        struct EvalInputs { turn_clock: Option<f32>, transition_clock: Option<f32> }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            bevy::gizmos::GizmoPlugin, bevy::gltf::GltfPlugin::default(), TransformPlugin,
            AnimationGraphPlugin::default(), crate::extensions::AlsExtPlugin,
            TurnLayerPlugin, crate::stop_link::AlsStopLinkPlugin))
            .init_asset::<WorldAsset>()
            .init_asset::<crate::als::params::AlsMovementParams>()
            .init_asset::<bevy::animation::AnimationClip>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_resource::<EvalInputs>()
            .add_systems(Startup, crate::standing_source::load_source_montage_assets);
        app.configure_sets(PostUpdate, (
            PostPhysicsAnimationSet::Inputs.after(avian3d::prelude::PhysicsSystems::Last),
            PostPhysicsAnimationSet::Evaluate.after(PostPhysicsAnimationSet::Inputs)
                .before(AnimationGraphSet::PostPhysics),
            AnimationGraphSet::PostPhysics.before(AnimationGraphSet::Final),
        )).add_systems(PostUpdate, evaluate_players::<With<PostPhysicsAnimation>>
            .in_set(PostPhysicsAnimationSet::Evaluate));
        app.add_systems(PostUpdate, (|players: Query<Entity, (With<AnimationGraphPlayer>, With<PostPhysicsAnimation>)>,
            inputs: Query<&crate::extensions::SourceAnimationInputs>, mut seen: ResMut<EvalInputs>| {
            let Some(entity) = players.iter().next() else { return; };
            if let Ok(inputs) = inputs.get(entity) {
                seen.turn_clock = inputs.slot_input_context("turn_in_place_standing.pose")
                    .and_then(|context| context.asset_samples.first().map(|sample| sample.clock.position));
                seen.transition_clock = inputs.slot_input_context("transition.pose")
                    .and_then(|context| context.asset_samples.first().map(|sample| sample.clock.position));
            }
        }).in_set(PostPhysicsAnimationSet::Evaluate).before(evaluate_players::<With<PostPhysicsAnimation>>));
        app.finish();
        app.cleanup();
        let transition = crate::stop_link::AlsTransitionPlayback::standing_dynamic(0.0);
        let turn_name = source_turn_sequence(false, false, 0.0);
        let crouch_turn_name = source_turn_sequence(true, false, 0.0);
        let transition_name = crate::stop_link::source_transition_sequence(&transition).unwrap();
        let started = std::time::Instant::now();
        let watchdog_reason = |wall: std::time::Duration, stalled: std::time::Duration| {
            if wall.as_secs() >= 30 { "wall_timeout" }
            else if stalled.as_secs() >= 15 { "no_progress_timeout" } else { "" }
        };
        assert_eq!(watchdog_reason(std::time::Duration::ZERO, std::time::Duration::ZERO), "");
        assert_eq!(watchdog_reason(std::time::Duration::from_secs(30), std::time::Duration::ZERO), "wall_timeout");
        assert_eq!(watchdog_reason(std::time::Duration::ZERO, std::time::Duration::from_secs(15)), "no_progress_timeout");
        let mut last_progress = std::time::Instant::now();
        let mut loaded = 0;
        loop {
            app.update();
            let assets = app.world().resource::<crate::standing_source::SourceMontageAssets>();
            let clips = app.world().resource::<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>();
            let skeletons = app.world().resource::<Assets<bevy_animation_graph::core::skeleton::Skeleton>>();
            let ready = [turn_name, crouch_turn_name, transition_name].into_iter().filter(|name|
                crate::standing_source::sample_source_montage(name, 0.42, assets, clips, skeletons).is_some()).count();
            if ready > loaded {
                loaded = ready;
                last_progress = std::time::Instant::now();
            }
            if ready == 3 { break; }
            let reason = watchdog_reason(started.elapsed(), last_progress.elapsed());
            assert!(reason.is_empty(), "{}", serde_json::json!({"exit_reason":reason,
                "last_progress_loaded":loaded,"required":3,"wait_seconds":last_progress.elapsed().as_secs()}));
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let (old_turn, old_transition) = {
            let assets = app.world().resource::<crate::standing_source::SourceMontageAssets>();
            let clips = app.world().resource::<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>();
            let skeletons = app.world().resource::<Assets<bevy_animation_graph::core::skeleton::Skeleton>>();
            (crate::standing_source::sample_source_montage(turn_name, 0.40, assets, clips, skeletons).unwrap().0,
                crate::standing_source::sample_source_montage(transition_name, 0.40, assets, clips, skeletons).unwrap().0)
        };
        let mut graph = AnimationGraph::default();
        for (name, key, output) in [
            ("turn pose", turn_slot_contract(false).generated_pose_key, "pose"),
            ("transition pose", "transition.pose", "transition_pose"),
        ] {
            let node = AnimationNode::new(name, crate::extensions::GlobalPoseInputNode { key: key.into(), output: "Pose".into() });
            graph.add_output_data(output.into(), DataSpec::Pose);
            graph.add_output_data_edge(node.id, "Pose", output);
            graph.add_node(node);
        }
        for (name, key, output) in [
            ("turn weight", turn_slot_contract(false).generated_weight_key, "turn_weight"),
            ("transition weight", "transition.weight", "transition_weight"),
        ] {
            let node = AnimationNode::new(name, crate::extensions::GlobalF32InputNode { key: key.into() });
            graph.add_output_data(output.into(), DataSpec::F32);
            graph.add_output_data_edge(node.id, key, output);
            graph.add_node(node);
        }
        let graph = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(graph);
        let character = app.world_mut().spawn((crate::als::character::AlsCharacter, Stance::Standing)).id();
        let avatar = app.world_mut().spawn((AlsCharacterAvatar { character },
            AnimatedSceneHandle::default(), ChildOf(character),
            AlsTurnDrive::default(), AlsTurnMontageStack::default())).id();
        app.world_mut().entity_mut(character).insert(crate::character_assembly::AlsCharacterAssembly {
            avatar, camera: None,
        });
        let mut player = AnimationGraphPlayer::new(Handle::default()).with_graph(graph);
        player.set_global_input_data(turn_slot_contract(false).generated_pose_key, DataValue::Pose(old_turn));
        player.set_global_input_data(turn_slot_contract(false).generated_weight_key, 0.5_f32);
        player.set_global_input_data("transition.pose", DataValue::Pose(old_transition));
        player.set_global_input_data("transition.weight", 0.5_f32);
        let player_entity = app.world_mut().spawn((Name::new("slot_player"), player, PostPhysicsAnimation, Transform::IDENTITY,
            GlobalTransform::IDENTITY, ChildOf(avatar))).id();
        app.world_mut().spawn((Name::new("root"), Transform::IDENTITY, GlobalTransform::IDENTITY, ChildOf(player_entity)));
        app.world_mut().trigger(WorldInstanceReady {
            entity: avatar,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
        });
        app.world_mut().flush();
        app.world_mut().run_schedule(Update);
        let stack = app.world_mut().get_mut::<AlsTurnMontageStack>(avatar).unwrap().into_inner();
        stack.begin(0.0, false, TURN_DUR_90, 1.0, false);
        stack.owner_player = Some(player_entity);
        stack.entries[0].elapsed = 0.42;
        stack.entries[0].blend_in_elapsed = 0.42;
        stack.entries[0].sampling_ready = true;
        let drive = app.world_mut().get_mut::<AlsTurnDrive>(avatar).unwrap().into_inner();
        drive.playing = true;
        drive.clip_time = 0.42;
        let mut transition = transition;
        transition.clip_time = 0.42;
        transition.previous_clip_time = 0.40;
        transition.elapsed = 0.42;
        transition.weight = 1.0;
        app.world_mut().entity_mut(character).insert(crate::stop_link::AlsTransitionMontageStack { entries: vec![transition], ..default() });
        app.world_mut().run_schedule(PostUpdate);
        app.world_mut().schedule_scope(PostUpdate, |_, schedule| {
            use bevy::ecs::schedule::NodeId;
            let graph = schedule.graph();
            let dependencies = graph.dependency().graph();
            let find_set = |needle: &str| dependencies.nodes().filter(|node| matches!(node, NodeId::Set(_)))
                .find(|node| graph.get_node_name(node).contains(needle))
                .unwrap_or_else(|| panic!("missing schedule set: {needle}"));
            let transition = schedule.systems().unwrap()
                .find(|(_, system)| system.name().contains("bridge_root_transition_slot_samples"))
                .map(|(key, _)| NodeId::System(key)).expect("missing transition bridge system");
            let evaluate = find_set("Evaluate");
            let reaches_evaluate = |start| {
                let mut pending = vec![start];
                let mut visited = std::collections::HashSet::new();
                while let Some(node) = pending.pop() {
                    if node == evaluate { return true; }
                    if visited.insert(node) { pending.extend(dependencies.neighbors(node)); }
                }
                false
            };
            assert!(reaches_evaluate(find_set("AlsTurnSlotBridgeSet")), "turn bridge has no ordering path to late evaluate");
            assert!(reaches_evaluate(transition), "transition bridge has no ordering path to late evaluate");
        });
        let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
        assert!(player.get_error().is_none(), "late graph evaluation failed: {:?}", player.get_error());
        let outputs = player.get_outputs();
        let turn_weight = outputs.get("turn_weight").and_then(|value| value.as_f32().ok());
        let transition_weight = outputs.get("transition_weight").and_then(|value| value.as_f32().ok());
        let turn_pose_time = outputs.get("pose").cloned().and_then(|value| value.into_pose().ok()).map(|pose| pose.timestamp);
        let transition_pose_time = outputs.get("transition_pose").cloned().and_then(|value| value.into_pose().ok()).map(|pose| pose.timestamp);
        let seen = app.world().resource::<EvalInputs>();
        assert_eq!((seen.turn_clock, seen.transition_clock), (Some(0.42), Some(0.42)));
        assert_eq!((turn_weight, transition_weight), (Some(1.0), Some(1.0)));
        assert_eq!((turn_pose_time, transition_pose_time), (Some(0.42), Some(0.42)));

        let publish = |app: &mut App, crouching, weights: &[f32], positions: [f32; 2]| {
            let stack = app.world_mut().get_mut::<AlsTurnMontageStack>(avatar).unwrap().into_inner();
            stack.entries.clear();
            for (&weight, position) in weights.iter().zip(positions) {
                stack.begin(0.0, false, TURN_DUR_90, 1.0, crouching);
                let entry = stack.entries.last_mut().unwrap();
                entry.elapsed = position;
                entry.blend_out_elapsed = Some(0.0);
                entry.blend_out_start_weight = weight;
            }
            app.world_mut().run_system_once(bridge_turn_slot_samples).unwrap();
        };
        let yaw_speed = crate::extensions::SourceCurveName::from("RotationYawSpeed");
        for crouching in [false, true] {
            let contract = turn_slot_contract(crouching);
            for case in crate::extensions::source_slot::montage_eligibility_cases().into_iter().rev() {
                publish(&mut app, crouching, &case.weights, [0.15, 0.42]);
                let runtime = app.world().get::<AlsStandingTurnSlotRuntime>(avatar).unwrap();
                assert!(runtime.active && runtime.pose_ready);
                assert_eq!(runtime.weight, case.total, "crouching={crouching} {:?}", case.weights);
                let stack = app.world().get::<AlsTurnMontageStack>(avatar).unwrap();
                assert!(stack.entries.iter().all(|entry| entry.sampling_ready));
                let inputs = app.world().get::<crate::extensions::SourceAnimationInputs>(player_entity).unwrap();
                let context = inputs.slot_input_context(contract.generated_pose_key);
                if case.eligible == 0 {
                    assert!(context.is_none());
                } else {
                    let context = context.unwrap();
                    assert_eq!(context.asset_samples.len(), case.eligible);
                    let expected_position = if case.weights.len() == 1 { 0.15 } else { 0.42 };
                    assert_eq!(context.asset_samples[0].clock.position, expected_position);
                }
            }
            let layer_curve = crate::standing_source::sample_source_montage(
                source_turn_sequence(crouching, false, 0.0), 0.25,
                app.world().resource::<crate::standing_source::SourceMontageAssets>(),
                app.world().resource::<Assets<bevy_animation_graph::core::animation_clip::GraphClip>>(),
                app.world().resource::<Assets<bevy_animation_graph::core::skeleton::Skeleton>>(),
            ).unwrap().1.curves[&yaw_speed];
            let slot = if crouching { "crouching-turn" } else { "standing-turn" };
            for case in crate::extensions::source_slot::montage_normalization_cases(slot) {
                publish(&mut app, crouching, &case.weights, [0.25; 2]);
                let runtime = app.world().get::<AlsStandingTurnSlotRuntime>(avatar).unwrap();
                assert_eq!(runtime.weight, case.slot_weight);
                let context = app.world().get::<crate::extensions::SourceAnimationInputs>(player_entity).unwrap()
                    .slot_input_context(contract.generated_pose_key).unwrap();
                assert_eq!(context.slot_has_multiple_montages, case.weights.len() > 1);
                let actual = context.curves[&yaw_speed] * runtime.weight;
                let expected = case.effective.iter().map(|weight| layer_curve * weight).sum::<f32>();
                assert!((actual - expected).abs() <= 2.0 * f32::EPSILON * expected.abs().max(1.0),
                    "{slot} {:?}: {actual} != {expected}", case.weights);
            }
        }
    }

    #[test]
    fn turn_stack_does_not_cross_animation_player_replacement() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            AnimationGraphPlugin::default(),
            crate::extensions::SourceAnimationInputsPlugin,
        ));
        let character = app.world_mut().spawn((crate::als::character::AlsCharacter, Stance::Standing)).id();
        let avatar = app.world_mut().spawn((
            AlsCharacterAvatar { character },
            AnimatedSceneHandle::default(),
            ChildOf(character),
            AlsTurnDrive::default(),
            AlsTurnMontageStack::default(),
        )).id();
        let old_player = app.world_mut().spawn((
            AnimationGraphPlayer::new(Handle::default()),
            ChildOf(avatar),
        )).id();
        app.world_mut().trigger(WorldInstanceReady {
            entity: avatar,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
        });
        app.world_mut().flush();
        app.world_mut().run_system_once(turn_pin_driver).unwrap();
        let mut stack = app.world_mut().get_mut::<AlsTurnMontageStack>(avatar).unwrap();
        stack.begin(0.0, false, TURN_DUR_90, 1.0, false);
        stack.entries[0].sampling_ready = true;
        drop(stack);
        app.world_mut().get_mut::<AnimationGraphPlayer>(old_player).unwrap()
            .set_global_input_data(turn_slot_contract(false).generated_weight_key, 0.8_f32);
        app.world_mut().get_mut::<crate::extensions::SourceAnimationInputs>(old_player).unwrap()
            .set_slot_input_context(
                turn_slot_contract(false).generated_pose_key,
                Some(crate::extensions::animation_context::AnimationContextState::default()),
            );
        assert_eq!(app.world().get::<AlsTurnMontageStack>(avatar).unwrap().entries.len(), 1);

        app.world_mut().entity_mut(old_player).remove::<ChildOf>();
        let new_player = app.world_mut().spawn((
            AnimationGraphPlayer::new(Handle::default()),
            ChildOf(avatar),
        )).id();
        app.world_mut().trigger(WorldInstanceReady {
            entity: avatar,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
        });
        app.world_mut().flush();
        assert_eq!(app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity(), new_player);
        app.world_mut().run_system_once(crate::standing_source::load_source_montage_assets).unwrap();
        app.world_mut().run_system_once(bridge_turn_slot_samples).unwrap();
        assert!(app.world().get::<AlsTurnMontageStack>(avatar).unwrap().entries.is_empty());
        assert!(!app.world().get::<AlsStandingTurnSlotRuntime>(avatar).unwrap().active);
        assert_eq!(app.world().get::<AnimationGraphPlayer>(new_player).unwrap()
            .get_global_input_data(turn_slot_contract(false).generated_weight_key)
            .and_then(|value| value.as_f32().ok()), Some(0.0));
        app.world_mut().run_system_once(turn_pin_driver).unwrap();
        assert!(app.world().entities().contains(old_player));
        assert!(app.world().get::<AlsTurnMontageStack>(avatar).unwrap().entries.is_empty());
        assert_eq!(app.world().get::<AnimationGraphPlayer>(old_player).unwrap()
            .get_global_input_data(turn_slot_contract(false).generated_weight_key)
            .and_then(|value| value.as_f32().ok()), Some(0.0));
        assert!(app.world().get::<crate::extensions::SourceAnimationInputs>(old_player).unwrap()
            .slot_input_context(turn_slot_contract(false).generated_pose_key).is_none());

        let mut stack = app.world_mut().get_mut::<AlsTurnMontageStack>(avatar).unwrap();
        stack.begin(1.0, false, TURN_DUR_90, 1.0, false);
        stack.entries[0].sampling_ready = true;
        drop(stack);
        app.world_mut().get_mut::<AnimationGraphPlayer>(new_player).unwrap()
            .set_global_input_data(turn_slot_contract(false).generated_weight_key, 0.7_f32);
        app.world_mut().entity_mut(new_player).remove::<ChildOf>();
        let third_player = app.world_mut().spawn((
            AnimationGraphPlayer::new(Handle::default()),
            ChildOf(avatar),
        )).id();
        app.world_mut().trigger(WorldInstanceReady {
            entity: avatar,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
        });
        app.world_mut().flush();
        assert_eq!(app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity(), third_player);
        app.world_mut().run_system_once(turn_pin_driver).unwrap();
        assert!(app.world().get::<AlsTurnMontageStack>(avatar).unwrap().entries.is_empty());
        assert_eq!(app.world().get::<AnimationGraphPlayer>(new_player).unwrap()
            .get_global_input_data(turn_slot_contract(false).generated_weight_key)
            .and_then(|value| value.as_f32().ok()), Some(0.0));
    }

    #[test]
    fn live_turn_curve_survives_switch_to_velocity_direction() {
        use crate::als::{input::AlsInputCommand, motion::{AlsRotationAnimationState, AlsRotationResponseSet}};

        #[derive(Resource, Default)]
        struct Capture {
            armed: bool,
            value: Option<(f32, f32, f32, RotationMode)>,
        }
        let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
        app.init_resource::<Capture>().add_systems(PostUpdate, (move |time: Res<Time>, mut capture: ResMut<Capture>,
            actors: Query<(&Transform, &AlsRotationAnimationState, &RotationMode)>| {
            if !capture.armed { return; }
            let (transform, animation, mode) = actors.get(character).unwrap();
            capture.value = Some((crate::als::orientation::world_forward_to_yaw(transform.rotation * Vec3::NEG_Z),
                animation.yaw_speed_degrees, time.delta_secs(), *mode));
        }).after(crate::crouching_layer::AlsCrouchingLayerSet)
            .after(crate::als::aim::aim_rotation_mode_system).before(AlsRotationResponseSet));
        app.world_mut().run_system_once(move |mut commands: Commands| {
            crate::transition_notify::queue_turn_in_grounded_group(&mut commands, character, avatar,
                SourceTurnRequest { variant: TurnVariant::Right180, yaw: 120.0, crouching: false });
        }).unwrap();
        for _ in 0..18 { app.update(); }
        assert!(app.world().get::<AlsRotationAnimationState>(character)
            .is_some_and(|animation| animation.yaw_speed_degrees.abs() > 10.0),
            "The loaded turn did not produce its rotation curve");
        app.world_mut().get_mut::<AlsInputCommand>(character).unwrap().rotation_mode_pressed = true;
        app.world_mut().resource_mut::<Capture>().armed = true;
        app.update();
        let (before, curve, delta, mode) = app.world().resource::<Capture>().value.unwrap();
        assert_eq!(mode, RotationMode::VelocityDirection);
        assert!(curve.abs() > 1e-4 && delta > 0.0);
        let expected = before + curve.to_radians() * delta;
        let actual = crate::als::orientation::world_forward_to_yaw(
            app.world().get::<Transform>(character).unwrap().rotation * Vec3::NEG_Z);
        assert!(crate::als::motion::wrap_yaw_radians(actual - expected).abs() < 1e-5,
            "before={before} curve={curve} delta={delta} actual={actual} expected={expected}");
    }

    #[test]
    fn native_turn_blend_shape_reaches_the_actual_slot_pose() {
        use bevy_animation_graph::core::{animation_clip::GraphClip, skeleton::Skeleton};
        for frame_delta in [0.1_f32, 0.05] {
            let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
            app.add_systems(PostUpdate, (move |instances:Query<&AnimatedSceneInstance>,
                mut players:Query<&mut AnimationGraphPlayer>| {
                let player = instances.get(avatar).unwrap().player_entity();
                players.get_mut(player).unwrap().set_global_input_data("turn_in_place.allowed", false);
            }).after(crate::ab_als_provider::AbAlsProviderSet)
                .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));
            for _ in 0..3 { app.update(); }
            app.world_mut().run_system_once(move |mut commands:Commands| {
                crate::transition_notify::queue_turn_in_grounded_group(&mut commands, character, avatar,
                    SourceTurnRequest { variant:TurnVariant::Left90, yaw:-90.0, crouching:false });
            }).unwrap();
            for _ in 0..30 { app.update(); }
            let stack = app.world().get::<AlsTurnMontageStack>(avatar).unwrap();
            assert_eq!(stack.entries.len(), 1);
            assert!((stack.entries[0].weight()-1.0).abs()<1e-6);
            app.world_mut().run_system_once(move |mut commands:Commands| {
                crate::transition_notify::queue_turn_in_grounded_group(&mut commands, character, avatar,
                    SourceTurnRequest { variant:TurnVariant::Right180, yaw:150.0, crouching:false });
            }).unwrap();
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(frame_delta)));
            app.update();
            let stack = app.world().get::<AlsTurnMontageStack>(avatar).unwrap();
            assert_eq!(stack.entries.len(), 2);
            assert!(stack.entries.iter().all(|entry| entry.sampling_ready));
            assert!((stack.entries[1].blend_in_elapsed-frame_delta).abs()<1e-6);
            let mut source_poses = Vec::new();
            let mut source_weights = Vec::new();
            for entry in &stack.entries {
                let pose = crate::standing_source::sample_source_montage(
                    source_turn_sequence(entry.crouching, entry.is_180, entry.lr), entry.elapsed,
                    app.world().resource::<crate::standing_source::SourceMontageAssets>(),
                    app.world().resource::<Assets<GraphClip>>(), app.world().resource::<Assets<Skeleton>>()).unwrap().0;
                let elapsed = entry.blend_out_elapsed.unwrap_or(entry.blend_in_elapsed);
                let alpha = (elapsed/TURN_MONTAGE_BLEND_DURATION).clamp(0.0,1.0);
                let native_alpha = alpha*alpha*(3.0-2.0*alpha);
                source_weights.push(if entry.blend_out_elapsed.is_some() { entry.blend_out_start_weight*(1.0-native_alpha) } else { native_alpha });
                source_poses.push(pose);
            }
            let total = source_weights.iter().sum::<f32>();
            let source = source_poses[0].scalar_mult(source_weights[0] / total)
                .linear_add(&source_poses[1].scalar_mult(source_weights[1] / total));
            let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
            let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            assert!(player.get_error().is_none());
            let actual = player.get_global_input_data(turn_slot_contract(false).generated_pose_key).unwrap().as_pose().unwrap();
            let mut rotation_error = 0.0_f32;
            let mut location_error = 0.0_f32;
            for bone in source.paths.keys() {
                let a = actual.get_bone(*bone).unwrap();
                let s = source.get_bone(*bone).unwrap();
                if let (Some(a),Some(s))=(a.rotation,s.rotation) { rotation_error=rotation_error.max((a-s).length().min((a+s).length())); }
                if let (Some(a),Some(s))=(a.translation,s.translation) { location_error=location_error.max(a.distance(s)); }
            }
            assert!(rotation_error<1e-6 && location_error<1e-5,
                "Native Turn Hermite blend changes the actual slot pose: delta={frame_delta}, source_weights={source_weights:?}, rotation_error={rotation_error}, location_error={location_error}");
        }
    }

    #[test]
    fn turn_slot_pose_uses_the_source_actor_animation_clock() {
        use bevy_animation_graph::core::{animation_clip::GraphClip, skeleton::Skeleton};
        for (custom_dilation, world_paused) in [(1.0_f32, false), (0.5, false), (0.0, false), (1.0, true)] {
            let (mut app, character, avatar) = crate::roll_drive::tests::loaded_roll_character();
            let request = SourceTurnRequest { variant:TurnVariant::Right90, yaw:90.0, crouching:false };
            let sequence = request.source_sequence();
            app.world_mut().run_system_once(move |mut commands: Commands| {
                crate::transition_notify::queue_turn_in_grounded_group(&mut commands, character, avatar, request);
            }).unwrap();
            for _ in 0..18 { app.update(); }
            let stack = app.world().get::<AlsTurnMontageStack>(avatar).unwrap();
            assert_eq!(stack.entries.len(), 1);
            let before = stack.entries[0].clone();
            assert!(before.sampling_ready && before.blend_out_elapsed.is_none() && (before.weight()-1.0).abs()<1e-6);
            app.world_mut().entity_mut(character).insert(crate::als_camera::AlsCameraCustomTimeDilation(custom_dilation));
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
            if world_paused { app.world_mut().resource_mut::<Time<bevy::time::Virtual>>().pause(); }
            let mut delta = 0.0_f32;
            for _ in 0..if custom_dilation == 0.0 || world_paused { 100 } else { 1 } {
                app.update();
                delta += app.world().resource::<Time>().delta_secs() * custom_dilation;
            }
            if world_paused { assert_eq!(delta, 0.0); }
            let source_time = before.elapsed + delta * before.play_rate;
            let source_pose = crate::standing_source::sample_source_montage(sequence, source_time,
                app.world().resource::<crate::standing_source::SourceMontageAssets>(), app.world().resource::<Assets<GraphClip>>(),
                app.world().resource::<Assets<Skeleton>>()).unwrap().0;
            let player_entity = app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity();
            let player = app.world().get::<AnimationGraphPlayer>(player_entity).unwrap();
            assert!(player.get_error().is_none());
            let pose = player.get_global_input_data(turn_slot_contract(false).generated_pose_key).unwrap().as_pose().unwrap();
            let mut minimum_rotation_dot = 1.0_f32;
            let mut maximum_location_error = 0.0_f32;
            for bone in source_pose.paths.keys() {
                let actual = pose.get_bone(*bone).unwrap();
                let source = source_pose.get_bone(*bone).unwrap();
                if let (Some(a),Some(s))=(actual.rotation,source.rotation) { minimum_rotation_dot=minimum_rotation_dot.min(a.dot(s).abs()); }
                if let (Some(a),Some(s))=(actual.translation,source.translation) { maximum_location_error=maximum_location_error.max(a.distance(s)); }
            }
            assert!(minimum_rotation_dot>1.0-1e-5 && maximum_location_error<1e-5,
                "Turn slot pose uses the wrong actor clock: custom={custom_dilation}, world_paused={world_paused}, rotation_error_degrees={}, location_error={maximum_location_error}, source_time={source_time}, stack={:?}",
                (2.0*minimum_rotation_dot.clamp(-1.0,1.0).acos()).to_degrees(), app.world().get::<AlsTurnMontageStack>(avatar).unwrap());
            let after = &app.world().get::<AlsTurnMontageStack>(avatar).unwrap().entries[0];
            assert_eq!(after.id, before.id);
            assert!((after.elapsed-source_time).abs()<1e-6);
            if world_paused { app.world_mut().resource_mut::<Time<bevy::time::Virtual>>().unpause(); }
        }
    }

    #[test]
    fn turn_pause_resume_keeps_the_existing_turn_while_grounded() {
        let (mut app, character, avatar)=crate::roll_drive::tests::loaded_roll_character();
        app.add_systems(PostUpdate,(move |mut mode:Query<&mut LocomotionState>| {
            *mode.get_mut(character).unwrap()=LocomotionState::Grounded;
        }).after(crate::actual_motion::AlsActualMotionSet).before(crate::ab_als_root::AlsRootGraphSet));
        app.world_mut().run_system_once(move |mut commands:Commands| {
            crate::transition_notify::queue_turn_in_grounded_group(&mut commands, character, avatar,
                SourceTurnRequest { variant:TurnVariant::Right90,yaw:90.0,crouching:false });
        }).unwrap();
        let started = app.world().get::<AlsTurnMontageStack>(avatar).unwrap().entries[0].id;
        app.world_mut().entity_mut(character).insert(crate::als_camera::AlsCameraCustomTimeDilation(0.0));
        for _ in 0..2 { app.update(); }
        let held = &app.world().get::<AlsTurnMontageStack>(avatar).unwrap().entries[0];
        assert!(held.sampling_ready);
        assert_eq!((held.id, held.elapsed, held.weight()), (started, 0.0, 0.0));
        app.world_mut().entity_mut(character).insert(crate::als_camera::AlsCameraCustomTimeDilation(1.0));
        for _ in 0..18 {app.update();}
        let before=app.world().get::<AlsTurnMontageStack>(avatar).unwrap().entries[0].clone();
        let calls=app.world().get::<crate::transition_notify::TransitionNotifyTelemetry>(character).unwrap().stop_animations_count;
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
        app.world_mut().resource_mut::<Time<bevy::time::Virtual>>().pause();
        for _ in 0..100 {app.update();}
        assert_eq!(app.world().get::<AlsTurnMontageStack>(avatar).unwrap().entries[0].elapsed,before.elapsed);
        app.world_mut().resource_mut::<Time<bevy::time::Virtual>>().unpause();
        app.update();
        assert_eq!(app.world().get::<crate::transition_notify::TransitionNotifyTelemetry>(character).unwrap().stop_animations_count,calls,
            "Pause resume fabricated a state-machine stop notification");
        assert_eq!(*app.world().get::<LocomotionState>(character).unwrap(),LocomotionState::Grounded);
        let stack=app.world().get::<AlsTurnMontageStack>(avatar).unwrap();
        assert_eq!(stack.entries.len(),1);
        assert_eq!(stack.entries[0].id,before.id);
        assert!(stack.entries[0].blend_out_elapsed.is_none());
        assert!((stack.entries[0].elapsed-before.elapsed-0.1*before.play_rate).abs()<1e-6);
    }

    #[test]
    fn crouching_turn_driver_preserves_the_source_queued_montage() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .add_systems(Update, turn_pin_driver);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.1));
        let character = app
            .world_mut()
            .spawn((
                crate::als::character::AlsCharacter,
                Transform::default(),
                AlsViewYaw(0.0),
                LinearVelocity::ZERO,
                LocomotionState::Grounded,
                Stance::Crouching,
                RotationMode::Aiming,
            ))
            .id();
        let mut stack = AlsTurnMontageStack::default();
        SourceTurnRequest {
            variant: TurnVariant::Left90,
            yaw: -90.0,
            crouching: true,
        }
        .play(&mut stack);
        stack.entries[0].sampling_ready = true;
        let avatar = app
            .world_mut()
            .spawn((
                AlsCharacterAvatar { character },
                ChildOf(character),
                AlsTurnDrive::default(),
                stack,
            ))
            .id();
        app.update();
        app.update();
        let stack = app.world().get::<AlsTurnMontageStack>(avatar).unwrap();
        assert_eq!(stack.entries.len(), 1);
        assert!(stack.entries[0].elapsed > 0.0);
        assert!(stack.entries[0].blend_out_elapsed.is_none());
    }

    #[test]
    fn native_turn_request_keeps_clip_rate_separate_and_allows_retrigger() {
        let mut stack = AlsTurnMontageStack::default();
        let request = SourceTurnRequest {
            variant: TurnVariant::Left180,
            yaw: -135.0,
            crouching: false,
        };
        assert!((request.play(&mut stack) - 0.9).abs() < 1e-6);
        assert_eq!(stack.entries[0].play_rate, 1.2);
        stack.entries[0].sampling_ready = true;
        stack.advance(0.1);
        request.play(&mut stack);
        assert_eq!(stack.entries.len(), 2);
        assert!(stack.entries[0].blend_out_elapsed.is_some());
        assert!(stack.entries[1].blend_out_elapsed.is_none());
        assert_eq!(
            SourceTurnRequest {
                crouching: true,
                ..request
            }
            .play(&mut stack),
            1.2
        );
    }

    #[test]
    fn source_half_turn_reaches_head_with_correct_direction() {
        use crate::als::head::{AlsHeadSettings, AlsHeadState, AlsHeadUpdateInput, update_head_state};

        for (view, body, expected) in [
            (180.0, 0.0, 180.0), (-180.0, 0.0, 180.0),
            (170.0, 0.0, -170.0), (-170.0, 0.0, 170.0),
            (-90.0, 90.0, 180.0), (90.0, -90.0, -180.0),
            (180.0, 10.0, -170.0), (-180.0, -10.0, 170.0),
        ] {
            assert_eq!(character_turn_angle_degrees(view, body), expected);
        }
        let mut head = AlsHeadState { initialization_required: false, ..default() };
        update_head_state(&mut head, AlsHeadUpdateInput {
            delta_seconds: 1.0 / 60.0,
            delta_seconds_without_time_dilation: 1.0 / 60.0,
            first_person: false,
            rotation_mode: RotationMode::ViewDirection,
            has_input: false,
            input_yaw_world: 0.0,
            target_yaw_world: 0.0,
            body_yaw_world: 0.0,
            body_yaw_velocity_world: 0.0,
            view_yaw_angle: character_turn_angle_degrees(180.0, 0.0),
            view_pitch_angle: 0.0,
            movement_base_relative_yaw: 0.0,
        }, AlsHeadSettings::default());
        assert!(head.yaw_angle > 0.0 && head.yaw_amount > 0.5);
    }

    #[test]
    fn target_yaw_basis_selects_the_source_left_and_right_turn_variants() {
        let left_angle = character_turn_angle_degrees(90.0, 0.0);
        let right_angle = character_turn_angle_degrees(-90.0, 0.0);
        assert_eq!(left_angle, -90.0);
        assert_eq!(right_angle, 90.0);
        let mut left_rotate = RotateInPlaceState::default();
        let mut right_rotate = RotateInPlaceState::default();
        let rotate_config = RotateInPlaceConfig::default();
        update_rotate_in_place(
            &mut left_rotate,
            false,
            true,
            left_angle,
            0.0,
            &rotate_config,
            false,
            0.1,
        );
        update_rotate_in_place(
            &mut right_rotate,
            false,
            true,
            right_angle,
            0.0,
            &rotate_config,
            false,
            0.1,
        );
        assert_eq!(left_rotate.side, Some(RotateSide::Left));
        assert_eq!(right_rotate.side, Some(RotateSide::Right));
        let mut left_turn = TurnInPlaceDetector::default();
        let mut right_turn = TurnInPlaceDetector::default();
        let turn_config = TurnInPlaceConfig::default();
        let left_variant =
            (0..20).find_map(|_| left_turn.update(left_angle, 0.0, 0.1, true, true, &turn_config));
        let right_variant = (0..20)
            .find_map(|_| right_turn.update(right_angle, 0.0, 0.1, true, true, &turn_config));
        assert_eq!(left_variant.unwrap().variant, TurnVariant::Left90);
        assert_eq!(right_variant.unwrap().variant, TurnVariant::Right90);
    }

    #[test]
    fn source_standing_turn_sequences_are_two_seconds() {
        assert_eq!(TURN_DUR_90, 2.0);
        assert_eq!(TURN_DUR_180, 2.0);
    }

    #[test]
    fn source_turn_slot_uses_hermite_blend_in_and_inertialized_blend_out() {
        assert_eq!(TURN_WEIGHT_IN.recip(), 0.2);
        assert_eq!(TURN_INERTIALIZATION_DURATION, 0.2);
        let mut stack = AlsTurnMontageStack::default();
        stack.begin(0.0, false, TURN_DUR_90, 1.2, false);
        stack.entries[0].sampling_ready = true;
        stack.advance(0.05);
        assert!((stack.entries[0].weight() - 0.15625).abs() < 1e-6);
    }

    #[test]
    fn grounded_transition_play_stops_the_current_turn_montage() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let character = app.world_mut().spawn(crate::als::character::AlsCharacter).id();
        let mut turns = AlsTurnMontageStack::default();
        SourceTurnRequest {
            variant: TurnVariant::Left90,
            yaw: 90.0,
            crouching: false,
        }.play(&mut turns);
        turns.entries[0].blend_in_elapsed = 0.1;
        turns.entries[0].sampling_ready = true;
        let mut old_turns = turns.clone();
        old_turns.entries[0].id = 99;
        let old_avatar = app.world_mut().spawn((
            AlsCharacterAvatar { character }, ChildOf(character), old_turns,
        )).id();
        let avatar = app.world_mut().spawn((
            AlsCharacterAvatar { character }, ChildOf(character), turns,
        )).id();
        app.world_mut().entity_mut(character).insert(crate::character_assembly::AlsCharacterAssembly {
            avatar, camera: None,
        });
        app.world_mut().run_system_once(move |mut commands: Commands| {
            crate::stop_link::play_transition(&mut commands, character,
                crate::stop_link::AlsTransitionPlayback::quick_stop(0.0, 1.0));
        }).unwrap();
        app.world_mut().flush();
        let turns = app.world().get::<AlsTurnMontageStack>(avatar).unwrap();
        assert!(turns.entries[0].blend_out_elapsed.is_some());
        assert!((turns.entries[0].blend_out_duration - crate::stop_link::QUICK_STOP_BLEND_IN).abs() < 1e-6);
        assert!(!turns.inertialization_pending);
        assert!(app.world().get::<AlsTurnMontageStack>(old_avatar).unwrap().entries[0].blend_out_elapsed.is_none());
        let initial_weight = turns.entries[0].weight();
        let mut turns = app.world_mut().get_mut::<AlsTurnMontageStack>(avatar).unwrap();
        turns.advance(0.05);
        assert!((turns.entries[0].weight() - initial_weight * 0.5).abs() < 1e-5);

        let mut immediate = AlsTurnMontageStack::default();
        SourceTurnRequest {
            variant: TurnVariant::Right90, yaw: 90.0, crouching: false,
        }.play(&mut immediate);
        immediate.entries[0].blend_in_elapsed = 0.1;
        immediate.stop_for_group_replacement(0.0);
        assert!(immediate.entries.is_empty());
    }

    #[test]
    fn repeated_grounded_transition_preserves_the_native_turn_blend_out() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            cases: Vec<Case>,
        }

        #[derive(serde::Deserialize)]
        struct Case {
            name: String,
            crouching: bool,
            frames: Vec<Frame>,
        }

        #[derive(serde::Deserialize)]
        struct Frame {
            delta_seconds: f32,
            stop_seconds: Option<f32>,
            weight: f32,
        }

        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../../docs/audit/bf144/d171/replacement.replay.json"
        )).unwrap();
        let mut app = App::new();
        let character = app.world_mut().spawn(crate::als::character::AlsCharacter).id();
        let avatar = app.world_mut().spawn(AlsCharacterAvatar { character }).id();
        app.world_mut().entity_mut(character).insert(crate::character_assembly::AlsCharacterAssembly { avatar, camera: None });
        for case in fixture.cases {
            let mut turns = AlsTurnMontageStack::default();
            SourceTurnRequest {
                variant: TurnVariant::Left90, yaw: -90.0, crouching: case.crouching,
            }.play(&mut turns);
            turns.entries[0].sampling_ready = true;
            app.world_mut().entity_mut(avatar).insert(turns);
            app.world_mut().entity_mut(character).insert(crate::stop_link::AlsTransitionMontageStack::default());
            for (index, frame) in case.frames.into_iter().enumerate() {
                if let Some(duration) = frame.stop_seconds {
                    let mut transition = if case.crouching {
                        crate::stop_link::AlsTransitionPlayback::crouching_dynamic(0.0)
                    } else {
                        crate::stop_link::AlsTransitionPlayback::standing_dynamic(0.0)
                    };
                    transition.blend_in = duration;
                    app.world_mut().run_system_once(move |mut commands: Commands| {
                        crate::stop_link::play_transition(&mut commands, character, transition);
                    }).unwrap();
                    app.world_mut().flush();
                }
                let mut turns = app.world_mut().get_mut::<AlsTurnMontageStack>(avatar).unwrap();
                assert!(!turns.advance(frame.delta_seconds));
                let weight = turns.entries.first().map_or(0.0, AlsTurnMontage::weight);
                assert_eq!(weight, frame.weight, "{} crouching={} frame {index}", case.name, case.crouching);
            }
        }
    }

    #[test]
    fn grounded_turn_play_stops_the_current_transition_montage() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), AnimationGraphPlugin::default()));
        let mut transition = crate::stop_link::AlsTransitionPlayback::standing_dynamic(0.0);
        transition.weight = 0.75;
        let transitions = crate::stop_link::AlsTransitionMontageStack {
            entries: vec![transition],
            ..default()
        };
        let character = app.world_mut().spawn((crate::als::character::AlsCharacter, transitions)).id();
        let avatar = app.world_mut().spawn((
            AlsCharacterAvatar { character }, AnimatedSceneHandle::default(),
            ChildOf(character), AlsTurnMontageStack::default(),
        )).id();
        let player = app.world_mut().spawn((
            AnimationGraphPlayer::new(Handle::default()), ChildOf(avatar),
        )).id();
        app.world_mut().entity_mut(character).insert(crate::character_assembly::AlsCharacterAssembly {
            avatar, camera: None,
        });
        app.world_mut().trigger(WorldInstanceReady {
            entity: avatar,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
        });
        app.world_mut().flush();
        assert_eq!(app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity(), player);
        let request = SourceTurnRequest {
            variant: TurnVariant::Right90,
            yaw: 90.0,
            crouching: false,
        };
        app.world_mut().run_system_once(move |mut commands: Commands| {
            crate::transition_notify::queue_turn_in_grounded_group(
                &mut commands, character, avatar, request);
        }).unwrap();
        app.world_mut().flush();
        let transition = &app.world().get::<crate::stop_link::AlsTransitionMontageStack>(character).unwrap().entries[0];
        assert_eq!(transition.stop_elapsed, Some(0.0));
        assert!((transition.stop_duration - TURN_MONTAGE_BLEND_DURATION).abs() < 1e-6);
        assert_eq!(app.world().get::<AlsTurnMontageStack>(avatar).unwrap().entries.len(), 1);

        app.world_mut().entity_mut(character).insert(crate::stop_link::AlsTransitionMontageStack::default());
        app.world_mut().entity_mut(avatar).insert(AlsTurnMontageStack::default());
        app.world_mut().run_system_once(move |mut commands: Commands| {
            crate::stop_link::play_transition(&mut commands, character,
                crate::stop_link::AlsTransitionPlayback::quick_stop(0.0, 1.0));
            crate::transition_notify::queue_turn_in_grounded_group(
                &mut commands, character, avatar, request);
        }).unwrap();
        app.world_mut().flush();
        assert_eq!(app.world().get::<AlsTurnMontageStack>(avatar).unwrap().entries.len(), 1);
        let transition = &app.world().get::<crate::stop_link::AlsTransitionMontageStack>(character).unwrap().entries[0];
        assert_eq!(transition.stop_elapsed, Some(0.0));
        assert!((transition.stop_duration - TURN_MONTAGE_BLEND_DURATION).abs() < 1e-6);
        assert_eq!(app.world().get::<AnimationGraphPlayer>(player).unwrap()
            .get_global_input_data("turn_in_place_state.play_rate")
            .and_then(|value| value.as_f32().ok()), Some(TurnInPlaceConfig::default().play_rate));

        app.world_mut().entity_mut(player).remove::<ChildOf>();
        let new_player = app.world_mut().spawn((
            AnimationGraphPlayer::new(Handle::default()), ChildOf(avatar),
        )).id();
        app.world_mut().trigger(WorldInstanceReady {
            entity: avatar,
            instance_id: WorldInstanceSpawner::default().spawn(Handle::<WorldAsset>::default()),
        });
        app.world_mut().flush();
        assert_eq!(app.world().get::<AnimatedSceneInstance>(avatar).unwrap().player_entity(), new_player);
        app.world_mut().entity_mut(avatar).insert(AlsTurnMontageStack::default());
        app.world_mut().run_system_once(move |mut commands: Commands| {
            crate::transition_notify::queue_turn_in_grounded_group(
                &mut commands, character, avatar, SourceTurnRequest {
                    variant: TurnVariant::Left180, yaw: 120.0, crouching: false,
                });
        }).unwrap();
        app.world_mut().flush();
        let expected = TurnInPlaceConfig::default().play_rate * (120.0 / 180.0);
        assert_eq!(app.world().get::<AnimationGraphPlayer>(new_player).unwrap()
            .get_global_input_data("turn_in_place_state.play_rate")
            .and_then(|value| value.as_f32().ok()), Some(expected));
        assert_eq!(app.world().get::<AnimationGraphPlayer>(player).unwrap()
            .get_global_input_data("turn_in_place_state.play_rate")
            .and_then(|value| value.as_f32().ok()), Some(TurnInPlaceConfig::default().play_rate));
    }

    #[test]
    fn natural_turn_end_requests_inertialization_and_removes_slot_weight() {
        let mut stack = AlsTurnMontageStack::default();
        stack.begin(0.0, false, TURN_DUR_90, 1.0, false);
        stack.entries[0].sampling_ready = true;
        stack.entries[0].elapsed = TURN_DUR_90 - 0.01;
        let mut drive = AlsTurnDrive::default();
        assert!(request_turn_inertialization(
            stack.advance(0.02),
            &mut drive
        ));
        synchronize_turn_drive(&mut drive, &stack);
        assert_eq!(drive.weight, 0.0);
        assert_eq!(drive.inertialization_request_epoch, 1.0);
    }

    #[test]
    fn generated_turn_slots_publish_requests_to_the_parent_inertialization() {
        for (graph, idle, epoch, duration) in [
            ("assets/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron",
                "assets/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph/SM6_Standing_States/S0_Idle.animgraph.ron",
                STANDING_TURN_INERTIALIZATION_EPOCH_KEY, STANDING_TURN_INERTIALIZATION_DURATION_KEY),
            ("assets/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching/AnimGraph.animgraph.ron",
                "assets/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching/AnimGraph/SM2_Crouching_States/S8_Idle.animgraph.ron",
                CROUCHING_TURN_INERTIALIZATION_EPOCH_KEY, CROUCHING_TURN_INERTIALIZATION_DURATION_KEY),
        ] {
            let graph_text = std::fs::read_to_string(graph).unwrap();
            let node = graph_text.lines().find(|line| line.contains("InertializationPassthroughNode")
                && line.contains("filtered_curves: [\"RotationYawSpeed\"]")).unwrap();
            assert!(node.contains("request_epoch_key: \"\""), "{graph}");
            assert!(node.contains("request_duration_key: \"\""), "{graph}");
            let idle_text = std::fs::read_to_string(idle).unwrap();
            let slot = idle_text.lines().find(|line| line.contains("SourceSlotNode")
                && line.contains("TurnInPlace")).unwrap();
            assert!(slot.contains(&format!("request_epoch_key: \"{epoch}\"")), "{idle}");
            assert!(slot.contains(&format!("request_duration_key: \"{duration}\"")), "{idle}");
        }
    }

    #[test]
    fn source_turn_clock_waits_for_the_first_slot_pose() {
        let mut stack = AlsTurnMontageStack::default();
        stack.begin(0.0, false, TURN_DUR_90, 1.2, true);
        assert!(source_turn_slot_is_active(&stack, true));
        assert!(!stack.advance(0.5));
        assert_eq!(stack.entries[0].elapsed, 0.0);
        assert_eq!(stack.entries[0].weight(), 0.0);
        stack.entries[0].sampling_ready = true;
        assert!(!stack.advance(0.1));
        assert!((stack.entries[0].elapsed - 0.12).abs() < 1e-6);
        assert!((stack.entries[0].weight() - 0.5).abs() < 1e-6);
    }

    fn fixture_turn_request(angle: u16, right: bool, crouching: bool) -> SourceTurnRequest {
        let variant = match (angle, right) {
            (90, false) => TurnVariant::Left90,
            (90, true) => TurnVariant::Right90,
            (180, false) => TurnVariant::Left180,
            (180, true) => TurnVariant::Right180,
            _ => panic!("invalid turn angle"),
        };
        let yaw = f32::from(angle) * if right { 1.0 } else { -1.0 };
        SourceTurnRequest { variant, yaw, crouching }
    }

    #[test]
    fn ready_turn_instances_survive_native_zero_delta_and_resume_in_all_variants() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            cases: Vec<Case>,
        }

        #[derive(serde::Deserialize)]
        struct Case {
            sequence: String,
            crouching: bool,
            angle: u16,
            right: bool,
            length: f32,
            frames: Vec<Frame>,
        }

        #[derive(serde::Deserialize)]
        struct Frame {
            delta_seconds: f32,
            position: f32,
            weight: f32,
        }

        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../../docs/audit/bf144/d175/zero-delta.replay.json"
        )).unwrap();
        for case in fixture.cases {
            let request = fixture_turn_request(case.angle, case.right, case.crouching);
            assert_eq!(request.source_sequence(), case.sequence);
            let mut stack = AlsTurnMontageStack::default();
            request.play(&mut stack);
            let entry = &mut stack.entries[0];
            assert_eq!(entry.duration, case.length);
            entry.sampling_ready = true;
            let id = entry.id;
            for frame in case.frames {
                assert!(!stack.advance(frame.delta_seconds));
                assert_eq!(stack.entries.len(), 1, "{}", case.sequence);
                let entry = &stack.entries[0];
                assert_eq!(entry.id, id);
                assert!(entry.playing());
                assert_eq!((entry.elapsed, entry.weight()), (frame.position, frame.weight), "{}", case.sequence);
            }
        }
    }

    #[test]
    fn turn_natural_stop_requests_inertia_at_the_native_playback_window() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            cases: Vec<Case>,
        }

        #[derive(serde::Deserialize)]
        struct Case {
            sequence: String,
            crouching: bool,
            angle: u16,
            right: bool,
            samples: Vec<Sample>,
        }

        #[derive(serde::Deserialize)]
        struct Sample {
            name: String,
            start: f32,
            deltas: Vec<f32>,
            stop_frame: usize,
        }

        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../../docs/audit/bf144/d176/stop-window.replay.json"
        )).unwrap();
        for case in fixture.cases {
            let request = fixture_turn_request(case.angle, case.right, case.crouching);
            assert_eq!(request.source_sequence(), case.sequence);
            for sample in case.samples {
                let mut stack = AlsTurnMontageStack::default();
                request.play(&mut stack);
                stack.entries[0].sampling_ready = true;
                stack.entries[0].elapsed = sample.start;
                let mut drive = AlsTurnDrive::default();
                for (frame, delta) in sample.deltas.into_iter().enumerate() {
                    let requested = request_turn_inertialization(stack.advance(delta), &mut drive);
                    assert_eq!(requested, frame == sample.stop_frame, "{} {} frame {frame}", case.sequence, sample.name);
                    assert_eq!(stack.entries.is_empty(), frame >= sample.stop_frame);
                }
                assert_eq!(drive.inertialization_request_epoch, 1.0);
            }
        }
    }

    #[test]
    fn source_turn_stop_discards_an_unready_slot_entry() {
        let mut stack = AlsTurnMontageStack::default();
        stack.begin(0.0, false, TURN_DUR_90, 1.2, true);
        stack.request_stop();
        assert!(stack.entries.is_empty());
    }

    #[test]
    fn source_retrigger_fades_the_prior_montage_while_starting_the_next_one() {
        let mut stack = AlsTurnMontageStack::default();
        stack.begin(0.0, false, TURN_DUR_90, 1.0, false);
        stack.entries[0].sampling_ready = true;
        stack.advance(TURN_MONTAGE_BLEND_DURATION);
        stack.begin(1.0, false, TURN_DUR_90, 1.0, false);
        stack.entries[1].sampling_ready = true;
        assert_eq!(stack.entries.len(), 2);
        assert!(stack.entries[0].blend_out_elapsed.is_some());
        stack.advance(TURN_MONTAGE_BLEND_DURATION * 0.5);
        assert!((stack.entries[0].weight() - 0.5).abs() < 1e-6);
        assert!((stack.entries[1].weight() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn turn_stack_accumulates_native_unequal_rotation_weights() {
        use bevy_animation_graph::core::pose::{BoneId, BonePose, Pose};

        let bone = BoneId::default();
        for case in crate::extensions::profile_blend::native_weighted_rotation_cases() {
            let mut stack = AlsTurnMontageStack::default();
            let mut poses = HashMap::new();
            let crouching = case.stance == "crouching";
            for (rotation, weight) in [case.base, case.target].into_iter().zip(case.weights) {
                stack.begin(0.0, false, TURN_DUR_90, 1.2, crouching);
                let entry = stack.entries.last_mut().unwrap();
                entry.blend_out_elapsed = Some(0.0);
                entry.blend_out_start_weight = weight;
                let mut pose = Pose::default();
                pose.add_bone(BonePose { rotation: Some(Quat::from_array(rotation)), ..default() }, bone);
                poses.insert(entry.id, pose);
            }
            let (pose, weight, _) = compose_turn_montage_poses(&stack, &poses, crouching).unwrap();
            assert_eq!(weight, 1.0);
            let actual = pose.get_bone(bone).unwrap().rotation.unwrap();
            assert!((actual - Quat::from_array(case.raw_rotation)).length() < 3e-7, "{} {}", case.stance, case.bone);
            let output = pose.normalize_quat();
            let actual = output.get_bone(bone).unwrap().rotation.unwrap();
            assert!((actual - Quat::from_array(case.rotation)).length() < 3e-7, "{} {}", case.stance, case.bone);
        }
    }

    #[test]
    fn source_slot_stack_normalizes_overlapping_non_additive_montages() {
        use bevy_animation_graph::core::pose::{BoneId, BonePose, Pose};

        let mut stack = AlsTurnMontageStack::default();
        stack.begin(0.0, false, TURN_DUR_90, 1.0, false);
        stack.entries[0].sampling_ready = true;
        stack.advance(TURN_MONTAGE_BLEND_DURATION);
        stack.begin(1.0, false, TURN_DUR_90, 1.0, false);
        stack.entries[1].sampling_ready = true;
        stack.advance(TURN_MONTAGE_BLEND_DURATION * 0.5);
        let bone = BoneId::default();
        let mut first = Pose::default();
        first.add_bone(
            BonePose::from_transform(Transform::from_translation(Vec3::ZERO)),
            bone,
        );
        let mut second = Pose::default();
        second.add_bone(
            BonePose::from_transform(Transform::from_translation(Vec3::X * 10.0)),
            bone,
        );
        let poses = HashMap::from([(stack.entries[0].id, first), (stack.entries[1].id, second)]);
        let (result, weight, _) = compose_turn_montage_poses(&stack, &poses, false).unwrap();
        assert!((weight - 1.0).abs() < 1e-6);
        assert_eq!(
            result.get_bone(bone).and_then(|pose| pose.translation),
            Some(Vec3::X * 5.0)
        );
        for case in crate::extensions::source_slot::montage_normalization_cases("standing-turn") {
            let [first, second] = case.effective.as_slice() else { continue; };
            for (entry, weight) in stack.entries.iter_mut().zip(case.weights) {
                entry.blend_out_elapsed = Some(0.0);
                entry.blend_out_start_weight = weight;
            }
            let (pose, weight, _) = compose_turn_montage_poses(&stack, &poses, false).unwrap();
            assert_eq!(weight, case.slot_weight);
            let bone = pose.get_bone(bone).unwrap();
            let first = first / weight;
            let second = second / weight;
            assert_eq!(bone.translation, Some(Vec3::X * (10.0 * second)));
            assert_eq!(bone.scale, Some(Vec3::splat(first + second)));
        }
    }

    #[test]
    fn turn_stack_keeps_rotation_magnitude_until_the_base_pose_is_added() {
        use bevy_animation_graph::core::pose::{BoneId, BonePose, Pose};
        let bone = BoneId::default();
        let make_pose = |rotation| {
            let mut pose = Pose::default();
            pose.add_bone(BonePose::from_transform(Transform::from_rotation(rotation)), bone);
            pose
        };
        let base = make_pose(Quat::from_rotation_y(0.45));
        let distance = |a: &Pose, b: &Pose| {
            let a = a.get_bone(bone).unwrap().rotation.unwrap();
            let b = b.get_bone(bone).unwrap().rotation.unwrap();
            (a - b).length().min((a + b).length())
        };
        for crouching in [false, true] {
            for count in [2, 3] {
                let mut stack = AlsTurnMontageStack::default();
                let mut poses = HashMap::new();
                let mut samples = Vec::new();
                for (weight, rotation) in [
                    (0.22, Quat::from_rotation_x(1.7)),
                    (0.18, Quat::from_rotation_y(-1.6)),
                    (0.19, Quat::from_rotation_z(1.1)),
                ].into_iter().take(count) {
                    stack.begin(0.0, false, TURN_DUR_90, 1.0, crouching);
                    let entry = stack.entries.last_mut().unwrap();
                    entry.sampling_ready = true;
                    entry.blend_out_start_weight = weight;
                    entry.blend_out_elapsed = Some(0.0);
                    let pose = make_pose(rotation);
                    poses.insert(entry.id, pose.clone());
                    samples.push((pose, weight));
                }
                let (overlay, weight, _) = compose_turn_montage_poses(&stack, &poses, crouching).unwrap();
                assert_eq!(weight, samples.iter().map(|(_, weight)| weight).sum::<f32>());
                samples.push((base.clone(), 1.0 - weight));
                let expected = crate::extensions::source_pose_nodes::blend_source_poses(&samples);
                let actual = crate::extensions::slot_node::blend_slot_pose(base.clone(), Some(&overlay), Some(weight));
                let early = crate::extensions::slot_node::blend_slot_pose(base.clone(), Some(&overlay.normalize_quat()), Some(weight));
                assert!(distance(&actual, &expected) < 1e-6);
                assert!(distance(&early, &expected) > 1e-3);
            }
        }
    }

    #[test]
    fn turn_slot_samples_the_source_variant_and_excludes_rotate_in_place() {
        let drive = AlsTurnDrive {
            playing: true,
            weight: 0.5,
            lr: 1.0,
            is_180: true,
            ..default()
        };
        assert!(turn_slot_active(&drive));
        assert_eq!(turn_slot_index(&drive), 3.0);
        let rotate = AlsTurnDrive {
            rotate_active: true,
            weight: 1.0,
            ..default()
        };
        assert!(!turn_slot_active(&rotate));
    }

    #[test]
    fn source_crouching_turn_one_eighty_sequence_is_two_and_one_third_seconds() {
        let (_, is_180, duration) = variant_params(TurnVariant::Right180, true);
        assert!(is_180);
        assert!((duration - CROUCH_TURN_DUR_180).abs() < f32::EPSILON);
    }

    #[test]
    fn crouching_turn_slot_uses_its_source_slot_graph_and_global_boundary() {
        let contract = turn_slot_contract(true);
        assert_eq!(
            source_turn_sequence(true, false, 0.0),
            "A_Als_Crouch_Turn_90_Left"
        );
        assert_eq!(contract.index_key, "als_crouching_turn_index");
        assert_eq!(contract.weight_key, "als_crouching_turn_weight");
        assert_eq!(contract.pose_key, CROUCHING_TURN_SLOT_POSE_KEY);
        assert_eq!(
            contract.inertialization_epoch_key,
            CROUCHING_TURN_INERTIALIZATION_EPOCH_KEY
        );
    }

    #[test]
    fn source_turn_stop_request_uses_immediate_weight_and_inertialization() {
        let mut stack = AlsTurnMontageStack::default();
        stack.begin(1.0, true, CROUCH_TURN_DUR_180, 1.0, true);
        stack.entries[0].sampling_ready = true;
        stack.advance(TURN_MONTAGE_BLEND_DURATION);
        stack.request_stop();
        assert!(stack.entries.is_empty());
        let mut drive = AlsTurnDrive::default();
        assert!(request_turn_inertialization(stack.advance(0.0), &mut drive));
        assert_eq!(drive.inertialization_request_epoch, 1.0);
        let mut player = AnimationGraphPlayer::new(Handle::default());
        write_turn_inertialization_inputs(&mut player, &drive, true);
        assert_eq!(player.get_global_input_data(CROUCHING_TURN_INERTIALIZATION_EPOCH_KEY).unwrap().as_f32().unwrap(), 1.0);
        write_turn_inertialization_inputs(&mut player, &drive, false);
        assert_eq!(player.get_global_input_data(CROUCHING_TURN_INERTIALIZATION_EPOCH_KEY).unwrap().as_f32().unwrap(), 0.0);
        assert_eq!(drive.inertialization_request_epoch, 1.0);
        assert!(!stack.advance(0.0));
    }

    #[test]
    fn rotate_curve_time_loops_on_the_source_sequence_duration() {
        let mut drive = AlsTurnDrive {
            rotate_active: true,
            rotate_rate: 1.5,
            ..default()
        };
        drive.advance(1.0);
        assert!((drive.rotate_clip_time - 0.5).abs() < 1e-6);
        assert!(drive.rotate_cycle_completed);
        assert!(drive.rotate_completion_event);
    }

    #[test]
    fn rotate_curve_time_finishes_without_looping_after_rotation_stops() {
        let mut drive = AlsTurnDrive {
            rotate_rate: 1.5,
            rotate_clip_time: 0.5,
            rotate_finishing: true,
            ..default()
        };
        drive.advance(1.0);
        assert_eq!(drive.rotate_clip_time, ROTATE_DUR);
        assert!(!drive.rotate_finishing);
        assert!(drive.rotate_completion_event);
    }
}
