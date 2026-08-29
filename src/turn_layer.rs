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
use bevy_animation_graph::core::edge_data::DataValue;

use crate::character_assembly::AlsCharacterAvatar;

pub const TURN_GROUND_SPEED_EPS: f32 = 0.05;
pub const TURN_YAW_ENV_DECAY: f32 = 250.0;
pub const TURN_WEIGHT_IN: f32 = 10.0;
pub const TURN_WEIGHT_OUT: f32 = 8.0;
pub const TURN_DUR_90: f32 = 2.0;
pub const TURN_DUR_180: f32 = 2.0;
pub const CROUCH_TURN_DUR_180: f32 = 2.33333325;
pub const ROTATE_DUR: f32 = 1.0;

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
            rotate_state: RotateInPlaceState::default(),
        }
    }
}

impl AlsTurnDrive {
    pub fn advance(&mut self, dt: f32) {
        if self.playing {
            self.elapsed += dt * self.play_rate.max(0.01);
            self.clip_time = self.elapsed.min(self.duration);
            self.weight = (self.weight + TURN_WEIGHT_IN * dt).min(1.0);
            if self.elapsed >= self.duration {
                self.playing = false;
            }
        } else {
            self.weight = (self.weight - TURN_WEIGHT_OUT * dt).max(0.0);
            if self.weight <= 0.0 {
                self.clip_time = 0.0;
                self.elapsed = 0.0;
            }
        }
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
    yaw_env: f32,
    prev_view: Option<(f64, f32)>,
}

pub struct TurnLayerPlugin;

impl Plugin for TurnLayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (ensure_turn_drive, turn_pin_driver.after(ensure_turn_drive)),
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
        (Entity, &'static ChildOf, &'static mut AlsTurnDrive),
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
        sampler.update(view_deg, time.elapsed_secs_f64(), dt);

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
            for (_drive_entity, parent, mut drive) in drives.iter_mut() {
                if parent.parent() != char_entity {
                    continue;
                }
                drive.playing = false;
                update_rotate_in_place(
                    &mut drive.rotate_state,
                    false,
                    true,
                    rel,
                    sampler.yaw_env,
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
            for (_drive_entity, parent, mut drive) in drives.iter_mut() {
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
                sampler.yaw_env,
                dt,
                turn_allowed,
                crouching,
                &TurnInPlaceConfig::default(),
            ) {
                let (lr, is_180, duration) = variant_params(ev.variant, crouching);
                for (_drive_entity, parent, mut drive) in drives.iter_mut() {
                    if parent.parent() != char_entity {
                        continue;
                    }
                    drive.playing = true;
                    drive.elapsed = 0.0;
                    drive.clip_time = 0.0;
                    drive.duration = duration;
                    drive.play_rate = ev.play_rate;
                    drive.lr = lr;
                    drive.is_180 = is_180;
                    drive.weight = drive.weight.max(0.01);
                    drive.rotate_weight = 0.0;
                }
            }
        }

        for (drive_entity, parent, mut drive) in drives.iter_mut() {
            if parent.parent() != char_entity {
                continue;
            }
            drive.advance(dt);
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
    fn update(&mut self, view_deg: f32, now: f64, dt: f32) -> f32 {
        let sample_dt = match self.prev_view {
            Some((pt, _)) => ((now - pt) as f32).max(1e-4),
            None => 0.0,
        };
        let inst = match self.prev_view {
            Some((_, pv)) if sample_dt > 1e-4 => wrap_angle_degrees(view_deg - pv) / sample_dt,
            _ => 0.0,
        };
        self.yaw_env = (self.yaw_env - TURN_YAW_ENV_DECAY * dt)
            .max(0.0)
            .max(inst.abs());
        self.prev_view = Some((now, view_deg));
        self.yaw_env
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
}
