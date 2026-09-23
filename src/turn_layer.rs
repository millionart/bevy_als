use als_core::rotate_in_place::RotateInPlaceState;
use als_core::state::Stance;
use als_core::turn_in_place::{TurnInPlaceConfig, TurnVariant, wrap_angle_degrees};
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneHandle;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::{DataValue, bone_mask::BoneMask};
use bevy_animation_graph::core::interpolation::linear::LinearInterpolator;
use bevy_animation_graph::core::plugin::AnimationGraphSet;
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
            let alpha = if self.blend_out_duration <= 0.0 { 1.0 }
                else { (elapsed / self.blend_out_duration).clamp(0.0, 1.0) };
            return self.blend_out_start_weight * (1.0 - alpha);
        }
        (self.blend_in_elapsed / TURN_MONTAGE_BLEND_DURATION).clamp(0.0, 1.0)
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
        let delta_seconds = delta_seconds.max(0.0);
        let mut natural_end = std::mem::take(&mut self.inertialization_pending);
        for entry in &mut self.entries {
            if !entry.sampling_ready {
                continue;
            }
            entry.elapsed += delta_seconds * entry.play_rate.max(0.01);
            entry.blend_in_elapsed += delta_seconds;
            if let Some(blend_out_elapsed) = &mut entry.blend_out_elapsed {
                *blend_out_elapsed += delta_seconds;
            } else if entry.elapsed >= entry.duration {
                natural_end = true;
                entry.blend_out_elapsed = Some(TURN_MONTAGE_BLEND_DURATION);
                entry.blend_out_start_weight = 0.0;
            }
        }
        self.entries.retain(|entry| {
            (!entry.sampling_ready && entry.blend_out_elapsed.is_none())
                || entry.weight() > f32::EPSILON
        });
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
        for entry in &mut self.entries {
            entry.blend_out_start_weight = entry.weight();
            entry.blend_out_elapsed = Some(0.0);
            entry.blend_out_duration = duration.max(0.0);
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
    pub(crate) fn play(self, stack: &mut AlsTurnMontageStack) -> f32 {
        let (lr, is_180, _) = variant_params(self.variant, self.crouching);
        let duration = crate::standing_source::MONTAGES
            [source_turn_sequence(self.crouching, is_180, lr)]
        .duration;
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
    -wrap_angle_degrees(view_yaw_degrees - body_yaw_degrees)
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
            AlsTurnSlotBridgeSet.after(AnimationGraphSet::Final),
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
) -> Option<(bevy_animation_graph::core::pose::Pose, f32)> {
    let entries = stack
        .entries
        .iter()
        .filter_map(|entry| {
            let weight = entry.weight();
            (entry.crouching == crouching && weight > f32::EPSILON)
                .then(|| poses.get(&entry.id).cloned().map(|pose| (pose, weight)))
                .flatten()
        })
        .collect::<Vec<_>>();
    let total_weight = entries.iter().map(|(_, weight)| *weight).sum::<f32>();
    let mut entries = entries.into_iter();
    let (mut output, mut accumulated) = entries.next()?;
    for (pose, weight) in entries {
        let next = accumulated + weight;
        LinearInterpolator {
            bone_mask: BoneMask::all(),
        }
        .interpolate_pose(&mut output, &pose, weight / next.max(f32::EPSILON));
        accumulated = next;
    }
    Some((output, total_weight.min(1.0)))
}

fn write_turn_inertialization_inputs(player: &mut AnimationGraphPlayer, drive: &AlsTurnDrive) {
    for crouching in [false, true] {
        let contract = turn_slot_contract(crouching);
        player.set_global_input_data(
            contract.inertialization_epoch_key,
            drive.inertialization_request_epoch,
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
    source_inputs: Option<&mut als_ext::SourceAnimationInputs>,
) {
    write_turn_pins(player, &AlsTurnDrive::default(), false);
    clear_turn_slot_inputs(player);
    if let Some(source_inputs) = source_inputs {
        for crouching in [false, true] {
            source_inputs.set_slot_input_context(turn_slot_contract(crouching).generated_pose_key, None);
        }
    }
}

fn bridge_turn_slot_samples(
    mut source_inputs: Query<&mut als_ext::SourceAnimationInputs>,
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
                for sample in &mut context.asset_samples {
                    sample.identity = format!("turn:{}", entry.id);
                    sample.clock.weight = entry.weight();
                }
                entry.sampling_ready = true;
                poses.insert(entry.id, pose);
                contexts.insert(entry.id, context);
            }
        }
        write_turn_inertialization_inputs(&mut player, &drive);
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
            let Some((pose, weight)) = compose_turn_montage_poses(&stack, &poses, crouching) else {
                continue;
            };
            let total = stack
                .entries
                .iter()
                .filter(|entry| entry.crouching == crouching && contexts.contains_key(&entry.id))
                .map(|entry| entry.weight())
                .sum::<f32>();
            let context = als_ext::source_pose_nodes::blend_animation_contexts(
                stack
                    .entries
                    .iter()
                    .filter(|entry| entry.crouching == crouching)
                    .filter_map(|entry| {
                        contexts
                            .get(&entry.id)
                            .cloned()
                            .map(|context| (context, entry.weight() / total))
                    }),
            );
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
    write_turn_inertialization_inputs(player, drive);
}

#[allow(clippy::type_complexity)]
fn turn_pin_driver(
    time: Res<Time>,
    stances: Query<&Stance, With<als_core::character::AlsCharacter>>,
    mut source_inputs: Query<&mut als_ext::SourceAnimationInputs>,
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
    let dt = time.delta_secs().max(1e-4);
    for (entity, parent, mut drive, mut stack) in &mut drives {
        let Ok(stance) = stances.get(parent.parent()) else {
            continue;
        };
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
        if request_turn_inertialization(natural_end, &mut drive) {
            warn!(
                "[TURN-SLOT][INERTIALIZATION-REQUEST] avatar={entity:?} epoch={:.0} duration={TURN_INERTIALIZATION_DURATION:.3}",
                drive.inertialization_request_epoch
            );
        }
        if let Some(mut player) = current_player.and_then(|player| players.get_mut(player).ok()) {
            write_turn_pins(&mut player, &drive, *stance == Stance::Crouching);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::world_serialization::{WorldAsset, WorldInstanceReady, WorldInstanceSpawner};
    use bevy_animation_graph::AnimationGraphPlugin;
    use als_core::motion::AlsViewYaw;
    use als_core::rotate_in_place::{RotateInPlaceConfig, RotateSide, update_rotate_in_place};
    use als_core::state::{LocomotionState, RotationMode};
    use als_core::turn_in_place::TurnInPlaceDetector;
    use avian3d::prelude::LinearVelocity;

    #[test]
    fn turn_stack_does_not_cross_animation_player_replacement() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            AnimationGraphPlugin::default(),
            als_ext::SourceAnimationInputsPlugin,
        ));
        let character = app.world_mut().spawn((als_core::character::AlsCharacter, Stance::Standing)).id();
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
        app.world_mut().get_mut::<als_ext::SourceAnimationInputs>(old_player).unwrap()
            .set_slot_input_context(
                turn_slot_contract(false).generated_pose_key,
                Some(als_ext::animation_context::AnimationContextState::default()),
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
        assert!(app.world().get::<als_ext::SourceAnimationInputs>(old_player).unwrap()
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
                als_core::character::AlsCharacter,
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
    fn source_turn_slot_uses_linear_blend_in_and_inertialized_blend_out() {
        assert_eq!(TURN_WEIGHT_IN.recip(), 0.2);
        assert_eq!(TURN_INERTIALIZATION_DURATION, 0.2);
    }

    #[test]
    fn grounded_transition_play_stops_the_current_turn_montage() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let character = app.world_mut().spawn(als_core::character::AlsCharacter).id();
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
    fn grounded_turn_play_stops_the_current_transition_montage() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), AnimationGraphPlugin::default()));
        let mut transition = crate::stop_link::AlsTransitionPlayback::standing_dynamic(0.0);
        transition.weight = 0.75;
        let transitions = crate::stop_link::AlsTransitionMontageStack {
            entries: vec![transition],
        };
        let character = app.world_mut().spawn((als_core::character::AlsCharacter, transitions)).id();
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
            crate::generated_transition_notify::queue_turn_in_grounded_group(
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
            crate::generated_transition_notify::queue_turn_in_grounded_group(
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
            crate::generated_transition_notify::queue_turn_in_grounded_group(
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
    fn generated_turn_inertialization_nodes_consume_runtime_requests() {
        for (graph, epoch, duration) in [
            ("assets/generated/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron",
                STANDING_TURN_INERTIALIZATION_EPOCH_KEY, STANDING_TURN_INERTIALIZATION_DURATION_KEY),
            ("assets/generated/ALS/Character/AnimationInstances/Stances/AB_Als_Crouching/AnimGraph.animgraph.ron",
                CROUCHING_TURN_INERTIALIZATION_EPOCH_KEY, CROUCHING_TURN_INERTIALIZATION_DURATION_KEY),
        ] {
            let graph_text = std::fs::read_to_string(graph).unwrap();
            let node = graph_text.lines().find(|line| line.contains("InertializationPassthroughNode")
                && line.contains("filtered_curves: [\"RotationYawSpeed\"]")).unwrap();
            assert!(node.contains(&format!("request_epoch_key: \"{epoch}\"")), "{graph}");
            assert!(node.contains(&format!("request_duration_key: \"{duration}\"")), "{graph}");
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
        let (result, weight) = compose_turn_montage_poses(&stack, &poses, false).unwrap();
        assert!((weight - 1.0).abs() < 1e-6);
        assert_eq!(
            result.get_bone(bone).and_then(|pose| pose.translation),
            Some(Vec3::X * 5.0)
        );
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
