use als_core::input_request::AlsFirstPersonView;
use als_core::motion::AlsViewYaw;
use als_core::orientation::world_forward_to_yaw;
use als_core::rotate_in_place::{RotateInPlaceConfig, RotateInPlaceState, update_rotate_in_place};
use als_core::state::{LocomotionState, RotationMode, Stance};
use als_core::turn_in_place::{
    TurnInPlaceConfig, TurnInPlaceDetector, TurnVariant, wrap_angle_degrees,
};
use avian3d::prelude::LinearVelocity;
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
const STANDING_TURN_SLOT_GRAPH: &str = "graphs/ab_als/stances/standing/turn.animgraph.ron";
const STANDING_TURN_SLOT_POSE_KEY: &str = "als_standing_turn_pose";
const STANDING_TURN_SLOT_LAYER_WEIGHT_KEY: &str = "als_standing_turn_layer_weight";
const STANDING_TURN_INERTIALIZATION_EPOCH_KEY: &str = "als_standing_turn_inertialization_epoch";
const STANDING_TURN_INERTIALIZATION_DURATION_KEY: &str =
    "als_standing_turn_inertialization_duration";

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
            rotate_finishing: false,
            inertialization_request_epoch: 0.0,
            rotate_state: RotateInPlaceState::default(),
        }
    }
}

impl AlsTurnDrive {
    pub fn advance(&mut self, dt: f32) {
        if self.rotate_active {
            self.rotate_clip_time = (self.rotate_clip_time
                + dt.max(0.0) * self.rotate_rate.max(0.0))
            .rem_euclid(ROTATE_DUR);
            self.rotate_finishing = false;
        } else if self.rotate_finishing {
            self.rotate_clip_time =
                (self.rotate_clip_time + dt.max(0.0) * self.rotate_rate.max(0.0)).min(ROTATE_DUR);
            if self.rotate_clip_time >= ROTATE_DUR {
                self.rotate_finishing = false;
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
    pub blend_in_elapsed: f32,
    pub blend_out_elapsed: Option<f32>,
    pub blend_out_start_weight: f32,
}

impl AlsTurnMontage {
    pub fn weight(&self) -> f32 {
        if let Some(elapsed) = self.blend_out_elapsed {
            let alpha = (elapsed / TURN_MONTAGE_BLEND_DURATION).clamp(0.0, 1.0);
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
    next_id: u64,
}

impl AlsTurnMontageStack {
    fn begin(&mut self, lr: f32, is_180: bool, duration: f32, play_rate: f32) {
        for entry in &mut self.entries {
            if entry.blend_out_elapsed.is_none() {
                entry.blend_out_start_weight = entry.weight();
                entry.blend_out_elapsed = Some(0.0);
            }
        }
        self.entries.push(AlsTurnMontage {
            id: self.next_id,
            elapsed: 0.0,
            duration,
            play_rate,
            lr,
            is_180,
            blend_in_elapsed: 0.0,
            blend_out_elapsed: None,
            blend_out_start_weight: 1.0,
        });
        self.next_id = self.next_id.wrapping_add(1);
    }

    fn advance(&mut self, delta_seconds: f32) -> bool {
        let delta_seconds = delta_seconds.max(0.0);
        let mut natural_end = false;
        for entry in &mut self.entries {
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
        self.entries.retain(|entry| entry.weight() > f32::EPSILON);
        natural_end
    }

    fn clear(&mut self) {
        self.entries.clear();
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

fn standing_rotate_allowed(rotation_mode: RotationMode, first_person: bool) -> bool {
    rotation_mode == RotationMode::Aiming || first_person
}

fn standing_turn_allowed(rotation_mode: RotationMode, first_person: bool) -> bool {
    rotation_mode == RotationMode::ViewDirection && !first_person
}

#[derive(Component, Default)]
struct TurnDetectorComp(TurnInPlaceDetector);

#[derive(Component, Default)]
struct TurnYawSampler {
    prev_view: Option<(f64, f32)>,
}

#[derive(Component, Debug, Clone, Copy)]
struct AlsStandingTurnSlotSampler {
    avatar: Entity,
    target_player: Entity,
    montage_id: u64,
    reported_unready: bool,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsStandingTurnSlotRuntime {
    pub active: bool,
    pub weight: f32,
    pub pose_ready: bool,
    pub inertialization_request_epoch: f32,
}

pub struct TurnLayerPlugin;

impl Plugin for TurnLayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                ensure_turn_drive,
                ensure_turn_montage_stack,
                ensure_turn_slot_samplers,
                turn_pin_driver,
                prepare_turn_slot_samples,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            bridge_turn_slot_samples.after(AnimationGraphSet::Final),
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

fn request_natural_turn_inertialization(natural_end: bool, drive: &mut AlsTurnDrive) -> bool {
    if !natural_end {
        return false;
    }
    drive.inertialization_request_epoch += 1.0;
    true
}

fn ensure_turn_slot_samplers(
    asset_server: Res<AssetServer>,
    avatars: Query<
        (Entity, &AnimatedSceneInstance, &AlsTurnMontageStack),
        With<AlsCharacterAvatar>,
    >,
    samplers: Query<(Entity, &AlsStandingTurnSlotSampler)>,
    players: Query<&AnimationGraphPlayer>,
    mut commands: Commands,
) {
    for (avatar, instance, stack) in &avatars {
        let target_player = instance.player_entity();
        let Ok(player) = players.get(target_player) else {
            continue;
        };
        for montage in &stack.entries {
            if montage.weight() <= f32::EPSILON
                || samplers.iter().any(|(_, sampler)| {
                    sampler.target_player == target_player && sampler.montage_id == montage.id
                })
            {
                continue;
            }
            let mut sampler_player = AnimationGraphPlayer::new(player.skeleton().clone())
                .with_graph(asset_server.load(STANDING_TURN_SLOT_GRAPH));
            sampler_player.set_global_input_data(
                "als_standing_turn_index",
                turn_slot_index_for(montage.is_180, montage.lr),
            );
            sampler_player.set_global_input_data("als_standing_turn_weight", montage.weight());
            sampler_player.set_global_input_data("als_standing_turn_play_rate", montage.play_rate);
            commands.spawn((
                Name::new("als_standing_turn_slot_sampler"),
                Children::default(),
                ChildOf(avatar),
                sampler_player,
                AlsStandingTurnSlotSampler {
                    avatar,
                    target_player,
                    montage_id: montage.id,
                    reported_unready: false,
                },
            ));
        }
    }
    for (entity, sampler) in &samplers {
        let still_active = avatars.get(sampler.avatar).is_ok_and(|(_, _, stack)| {
            stack
                .entries
                .iter()
                .any(|entry| entry.id == sampler.montage_id && entry.weight() > f32::EPSILON)
        });
        if !still_active {
            commands.entity(entity).despawn();
        }
    }
}

fn prepare_turn_slot_samples(
    drives: Query<(&AlsTurnDrive, &AlsTurnMontageStack)>,
    mut samplers: Query<(&AlsStandingTurnSlotSampler, &mut AnimationGraphPlayer)>,
) {
    for (sampler, mut player) in &mut samplers {
        let Ok((drive, stack)) = drives.get(sampler.avatar) else {
            continue;
        };
        let Some(montage) = stack
            .entries
            .iter()
            .find(|entry| entry.id == sampler.montage_id)
        else {
            continue;
        };
        player.set_global_input_data(
            "als_standing_turn_index",
            turn_slot_index_for(montage.is_180, montage.lr),
        );
        player.set_global_input_data("als_standing_turn_weight", montage.weight());
        player.set_global_input_data("als_standing_turn_play_rate", montage.play_rate);
        player.set_global_input_data(
            STANDING_TURN_INERTIALIZATION_EPOCH_KEY,
            drive.inertialization_request_epoch,
        );
        player.set_global_input_data(
            STANDING_TURN_INERTIALIZATION_DURATION_KEY,
            TURN_INERTIALIZATION_DURATION,
        );
    }
}

fn compose_turn_montage_poses(
    stack: &AlsTurnMontageStack,
    poses: &HashMap<u64, bevy_animation_graph::core::pose::Pose>,
) -> Option<(bevy_animation_graph::core::pose::Pose, f32)> {
    let entries = stack
        .entries
        .iter()
        .filter_map(|entry| {
            let weight = entry.weight();
            (weight > f32::EPSILON)
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

fn bridge_turn_slot_samples(
    avatars: Query<
        (
            Entity,
            &AnimatedSceneInstance,
            &AlsTurnDrive,
            &AlsTurnMontageStack,
        ),
        With<AlsCharacterAvatar>,
    >,
    mut samplers: Query<(&mut AlsStandingTurnSlotSampler, &AnimationGraphPlayer)>,
    mut target_players: Query<&mut AnimationGraphPlayer, Without<AlsStandingTurnSlotSampler>>,
    mut commands: Commands,
) {
    let mut poses_by_avatar = HashMap::new();
    for (mut sampler, source_player) in &mut samplers {
        let Some(pose) = source_player.get_default_output_pose().cloned() else {
            if !sampler.reported_unready
                && let Some(error) = source_player.get_error()
            {
                warn!(
                    "[TURN-SLOT] sampler unavailable avatar={:?} montage={} error={:?}",
                    sampler.avatar, sampler.montage_id, error
                );
            }
            sampler.reported_unready = true;
            continue;
        };
        sampler.reported_unready = false;
        poses_by_avatar
            .entry(sampler.avatar)
            .or_insert_with(HashMap::new)
            .insert(sampler.montage_id, pose);
    }
    for (avatar, instance, drive, stack) in &avatars {
        let Ok(mut target_player) = target_players.get_mut(instance.player_entity()) else {
            continue;
        };
        target_player.set_global_input_data(
            STANDING_TURN_INERTIALIZATION_EPOCH_KEY,
            drive.inertialization_request_epoch,
        );
        target_player.set_global_input_data(
            STANDING_TURN_INERTIALIZATION_DURATION_KEY,
            TURN_INERTIALIZATION_DURATION,
        );
        let Some(poses) = poses_by_avatar.get(&avatar) else {
            target_player.set_global_input_data("als_standing_turn_weight", 0.0_f32);
            target_player.set_global_input_data(STANDING_TURN_SLOT_LAYER_WEIGHT_KEY, 0.0_f32);
            commands.entity(avatar).insert(AlsStandingTurnSlotRuntime {
                active: !stack.entries.is_empty(),
                weight: 0.0,
                pose_ready: stack.entries.is_empty(),
                inertialization_request_epoch: drive.inertialization_request_epoch,
            });
            continue;
        };
        let Some((pose, weight)) = compose_turn_montage_poses(stack, poses) else {
            target_player.set_global_input_data("als_standing_turn_weight", 0.0_f32);
            target_player.set_global_input_data(STANDING_TURN_SLOT_LAYER_WEIGHT_KEY, 0.0_f32);
            commands.entity(avatar).insert(AlsStandingTurnSlotRuntime {
                active: !stack.entries.is_empty(),
                weight: 0.0,
                pose_ready: false,
                inertialization_request_epoch: drive.inertialization_request_epoch,
            });
            continue;
        };
        target_player.set_global_input_data(STANDING_TURN_SLOT_POSE_KEY, DataValue::Pose(pose));
        target_player.set_global_input_data("als_standing_turn_weight", weight);
        target_player.set_global_input_data(STANDING_TURN_SLOT_LAYER_WEIGHT_KEY, 1.0_f32);
        commands.entity(avatar).insert(AlsStandingTurnSlotRuntime {
            active: true,
            weight,
            pose_ready: true,
            inertialization_request_epoch: drive.inertialization_request_epoch,
        });
    }
}

fn write_turn_pins(player: &mut AnimationGraphPlayer, drive: &AlsTurnDrive) {
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
    player.set_global_input_data("als_standing_turn_index", turn_slot_index(drive));
    player.set_global_input_data("als_standing_turn_weight", drive.weight);
    player.set_global_input_data("als_standing_turn_play_rate", drive.play_rate);
    player.set_global_input_data(
        STANDING_TURN_INERTIALIZATION_EPOCH_KEY,
        drive.inertialization_request_epoch,
    );
    player.set_global_input_data(
        STANDING_TURN_INERTIALIZATION_DURATION_KEY,
        TURN_INERTIALIZATION_DURATION,
    );
}

#[allow(clippy::type_complexity)]
fn turn_pin_driver(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &Transform,
            &AlsViewYaw,
            &LinearVelocity,
            &LocomotionState,
            &Stance,
            &RotationMode,
            Option<&AlsFirstPersonView>,
            Option<&mut TurnDetectorComp>,
            Option<&mut TurnYawSampler>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    mut drives: Query<
        (
            Entity,
            &'static ChildOf,
            &'static mut AlsTurnDrive,
            &'static mut AlsTurnMontageStack,
        ),
        With<AlsCharacterAvatar>,
    >,
    instances: Query<&'static AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (
        char_entity,
        body_tf,
        view,
        vel,
        loco,
        stance,
        rotation_mode,
        first_person,
        detector,
        sampler,
    ) in &mut q
    {
        let (Some(mut detector), Some(mut sampler)) = (detector, sampler) else {
            commands
                .entity(char_entity)
                .insert((TurnDetectorComp::default(), TurnYawSampler::default()));
            continue;
        };
        let hspeed = Vec2::new(vel.x, vel.z).length();
        let view_deg = view.0.to_degrees();
        let fwd = body_tf.forward();
        let body_deg = world_forward_to_yaw(*fwd).to_degrees();
        let rel = wrap_angle_degrees(view_deg - body_deg);
        let view_yaw_speed = sampler.update(view_deg, time.elapsed_secs_f64());

        let crouching = *stance == Stance::Crouching;
        let first_person = first_person.is_some();
        let stationary = *loco == LocomotionState::Grounded && hspeed < TURN_GROUND_SPEED_EPS;
        let rotate_allowed = stationary && standing_rotate_allowed(*rotation_mode, first_person);
        let turn_allowed = stationary && standing_turn_allowed(*rotation_mode, first_person);

        if rotate_allowed {
            detector.0 = TurnInPlaceDetector::default();
            let mut config = RotateInPlaceConfig::default();
            if first_person && *rotation_mode != RotationMode::Aiming {
                config.view_yaw_angle_threshold_degrees =
                    config.first_person_view_yaw_angle_threshold_degrees;
            }
            for (_drive_entity, parent, mut drive, mut stack) in drives.iter_mut() {
                if parent.parent() != char_entity {
                    continue;
                }
                stack.clear();
                synchronize_turn_drive(&mut drive, &stack);
                update_rotate_in_place(
                    &mut drive.rotate_state,
                    false,
                    true,
                    rel,
                    view_yaw_speed,
                    &config,
                    dt,
                );
                match drive.rotate_state.side {
                    Some(side) => {
                        let rotate_lr = if side == als_core::rotate_in_place::RotateSide::Left {
                            0.0
                        } else {
                            1.0
                        };
                        if !drive.rotate_active || drive.rotate_lr != rotate_lr {
                            drive.rotate_clip_time = 0.0;
                        }
                        drive.rotate_active = true;
                        drive.rotate_finishing = false;
                        drive.rotate_lr = rotate_lr;
                        drive.rotate_rate = drive.rotate_state.play_rate;
                        drive.weight = 0.0;
                        drive.elapsed = 0.0;
                        drive.clip_time = 0.0;
                        drive.rotate_weight = (drive.rotate_weight + TURN_WEIGHT_IN * dt).min(1.0);
                    }
                    None => {
                        drive.rotate_finishing = drive.rotate_active;
                        drive.rotate_active = false;
                        drive.rotate_rate = drive.rotate_state.play_rate;
                        drive.rotate_weight = (drive.rotate_weight - TURN_WEIGHT_OUT * dt).max(0.0);
                    }
                }
            }
        } else {
            for (_drive_entity, parent, mut drive, _stack) in drives.iter_mut() {
                if parent.parent() != char_entity {
                    continue;
                }
                drive.rotate_finishing |= drive.rotate_active;
                drive.rotate_active = false;
                if !drive.rotate_finishing {
                    drive.rotate_rate = 0.0;
                }
                drive.rotate_weight = (drive.rotate_weight - TURN_WEIGHT_OUT * dt).max(0.0);
            }
            if let Some(ev) = detector.0.update(
                rel,
                view_yaw_speed,
                dt,
                turn_allowed,
                crouching,
                &TurnInPlaceConfig::default(),
            ) {
                let (lr, is_180, duration) = variant_params(ev.variant, crouching);
                for (_drive_entity, parent, mut drive, mut stack) in drives.iter_mut() {
                    if parent.parent() != char_entity {
                        continue;
                    }
                    stack.begin(lr, is_180, duration, ev.play_rate);
                    synchronize_turn_drive(&mut drive, &stack);
                    drive.rotate_weight = 0.0;
                }
            }
        }

        for (drive_entity, parent, mut drive, mut stack) in drives.iter_mut() {
            if parent.parent() != char_entity {
                continue;
            }
            let natural_end = stack.advance(dt);
            synchronize_turn_drive(&mut drive, &stack);
            drive.advance(dt);
            if request_natural_turn_inertialization(natural_end, &mut drive) {
                warn!(
                    "[TURN-SLOT][INERTIALIZATION-REQUEST] avatar={drive_entity:?} epoch={:.0} duration={TURN_INERTIALIZATION_DURATION:.3}",
                    drive.inertialization_request_epoch
                );
            }
            let pin_target = instances
                .get(drive_entity)
                .ok()
                .and_then(|instance| players.get_mut(instance.player_entity()).ok());
            if let Some(mut player) = pin_target {
                write_turn_pins(&mut player, &drive);
            }
        }
    }
}

impl TurnYawSampler {
    fn update(&mut self, view_deg: f32, now: f64) -> f32 {
        let sample_dt = match self.prev_view {
            Some((pt, _)) => ((now - pt) as f32).max(1e-4),
            None => 0.0,
        };
        let inst = match self.prev_view {
            Some((_, pv)) if sample_dt > 1e-4 => wrap_angle_degrees(view_deg - pv) / sample_dt,
            _ => 0.0,
        };
        self.prev_view = Some((now, view_deg));
        inst.abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standing_rotate_uses_aiming_or_first_person() {
        assert!(standing_rotate_allowed(RotationMode::Aiming, false));
        assert!(standing_rotate_allowed(RotationMode::ViewDirection, true));
        assert!(!standing_rotate_allowed(RotationMode::ViewDirection, false));
    }

    #[test]
    fn standing_turn_uses_third_person_view_direction_only() {
        assert!(standing_turn_allowed(RotationMode::ViewDirection, false));
        assert!(!standing_turn_allowed(RotationMode::Aiming, false));
        assert!(!standing_turn_allowed(RotationMode::ViewDirection, true));
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
    fn natural_turn_end_requests_inertialization_and_removes_slot_weight() {
        let mut stack = AlsTurnMontageStack::default();
        stack.begin(0.0, false, TURN_DUR_90, 1.0);
        stack.entries[0].elapsed = TURN_DUR_90 - 0.01;
        let mut drive = AlsTurnDrive::default();
        assert!(request_natural_turn_inertialization(
            stack.advance(0.02),
            &mut drive
        ));
        synchronize_turn_drive(&mut drive, &stack);
        assert_eq!(drive.weight, 0.0);
        assert_eq!(drive.inertialization_request_epoch, 1.0);
    }

    #[test]
    fn source_retrigger_fades_the_prior_montage_while_starting_the_next_one() {
        let mut stack = AlsTurnMontageStack::default();
        stack.begin(0.0, false, TURN_DUR_90, 1.0);
        stack.advance(TURN_MONTAGE_BLEND_DURATION);
        stack.begin(1.0, false, TURN_DUR_90, 1.0);
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
        stack.begin(0.0, false, TURN_DUR_90, 1.0);
        stack.advance(TURN_MONTAGE_BLEND_DURATION);
        stack.begin(1.0, false, TURN_DUR_90, 1.0);
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
        let (result, weight) = compose_turn_montage_poses(&stack, &poses).unwrap();
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
    fn rotate_curve_time_loops_on_the_source_sequence_duration() {
        let mut drive = AlsTurnDrive {
            rotate_active: true,
            rotate_rate: 1.5,
            ..default()
        };
        drive.advance(1.0);
        assert!((drive.rotate_clip_time - 0.5).abs() < 1e-6);
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
    }

    #[test]
    fn turn_yaw_sampler_does_not_extend_stopped_view_speed() {
        let mut sampler = TurnYawSampler::default();
        let initial = sampler.update(0.0, 0.0);
        let fast = sampler.update(40.0, 0.1);
        let stopped = sampler.update(40.0, 0.2);
        let passed = initial == 0.0 && (fast - 400.0).abs() < 1e-4 && stopped == 0.0;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.39][VERDICT] initial={initial:.1} fast={fast:.1} stopped={stopped:.1} -> {verdict}"
        );
        assert!(passed);
    }
}
