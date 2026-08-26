use als_core::motion::AlsViewYaw;
use als_core::rotate_in_place::{
    update_rotate_in_place, RotateInPlaceConfig, RotateInPlaceState,
};
use als_core::state::{LocomotionState, Stance};
use als_core::turn_in_place::{
    wrap_angle_degrees, TurnInPlaceConfig, TurnInPlaceDetector, TurnVariant,
};
use avian3d::prelude::LinearVelocity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

pub const TURN_GROUND_SPEED_EPS: f32 = 0.05;
pub const TURN_YAW_ENV_DECAY: f32 = 250.0;
pub const TURN_WEIGHT_IN: f32 = 10.0;
pub const TURN_WEIGHT_OUT: f32 = 8.0;
pub const TURN_DUR_90: f32 = 1.4;
pub const TURN_DUR_180: f32 = 2.0;

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
    pub rotate_lr: f32,
    pub rotate_rate: f32,
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
            rotate_lr: 0.0,
            rotate_rate: 0.0,
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
    }
}

fn variant_params(variant: TurnVariant) -> (f32, bool, f32) {
    match variant {
        TurnVariant::Left90 => (0.0, false, TURN_DUR_90),
        TurnVariant::Right90 => (1.0, false, TURN_DUR_90),
        TurnVariant::Left180 => (0.0, true, TURN_DUR_180),
        TurnVariant::Right180 => (1.0, true, TURN_DUR_180),
    }
}

#[derive(Component, Default)]
struct TurnDetectorComp(TurnInPlaceDetector);

pub struct TurnLayerPlugin;

impl Plugin for TurnLayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, turn_pin_driver);
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
    let rotate_w = if drive.rotate_active { 1.0f32 } else { 0.0f32 };
    player.set_input_data("rotate_weight", DataValue::from(rotate_w * drive.weight));
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
            &ChildOf,
            &Transform,
            &AlsViewYaw,
            &LinearVelocity,
            &LocomotionState,
            &Stance,
            Option<&mut TurnDetectorComp>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    mut drives: Query<(Entity, &'static ChildOf, Option<&'static mut AlsTurnDrive>)>,
    instances: Query<&'static AnimatedSceneInstance>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let Ok((char_entity, _child_of, body_tf, view, vel, loco, stance, mut detector)) =
        q.single_mut()
    else {
        return;
    };
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs().max(1e-4);
    let hspeed = Vec2::new(vel.x, vel.z).length();
    let view_deg = view.0.to_degrees();
    let fwd = body_tf.forward();
    let body_deg = f32::atan2(fwd.x, -fwd.z).to_degrees();
    let rel = wrap_angle_degrees(view_deg - body_deg);
    let allowed = *loco == LocomotionState::Grounded && hspeed < TURN_GROUND_SPEED_EPS;

    let detector = match detector {
        Some(ref mut d) => &mut d.0,
        None => {
            commands.entity(char_entity).insert(TurnDetectorComp::default());
            return;
        }
    };

    let mut sample = TurnYawSampler::default();
    let yaw_env = sample.update(view_deg, now, dt);
    let _ = yaw_env;

    let crouching = *stance == Stance::Crouching;
    if crouching {
        // ALS: crouched turns use RotateInPlace - continuous clip driven by
        // yaw speed mapped to play rate, no discrete variant selection.
        *detector = TurnInPlaceDetector::default();
        for (_e, parent, mut d) in drives.iter_mut() {
            if parent.parent() != char_entity {
                continue;
            }
            if let Some(ref mut drive) = d {
                drive.playing = false;
                let mut rs = RotateInPlaceState::default();
                update_rotate_in_place(
                    &mut rs,
                    hspeed >= TURN_GROUND_SPEED_EPS,
                    allowed,
                    rel,
                    sample.yaw_env,
                    &RotateInPlaceConfig::default(),
                    dt,
                );
                match rs.side {
                    Some(side) => {
                        drive.rotate_active = true;
                        drive.rotate_lr = if side == als_core::rotate_in_place::RotateSide::Left {
                            0.0
                        } else {
                            1.0
                        };
                        drive.rotate_rate = rs.play_rate;
                        drive.weight = (drive.weight + TURN_WEIGHT_IN * dt).min(1.0);
                    }
                    None => {
                        drive.rotate_active = false;
                        drive.rotate_rate = 0.0;
                        drive.weight = (drive.weight - TURN_WEIGHT_OUT * dt).max(0.0);
                    }
                }
            }
        }
    } else if let Some(ev) = detector.update(
        rel,
        sample.yaw_env,
        dt,
        allowed,
        false,
        &TurnInPlaceConfig::default(),
    ) {
        let (lr, is_180, duration) = variant_params(ev.variant);
        for (_e, parent, mut d) in drives.iter_mut() {
            if parent.parent() != char_entity {
                continue;
            }
            if let Some(ref mut drive) = d {
                drive.rotate_active = false;
                drive.playing = true;
                drive.elapsed = 0.0;
                drive.clip_time = 0.0;
                drive.duration = duration / ev.play_rate.max(0.01) * ev.play_rate;
                drive.play_rate = ev.play_rate;
                drive.lr = lr;
                drive.is_180 = is_180;
                drive.weight = drive.weight.max(0.01);
            }
        }
    }

    for (_e, parent, mut d) in drives.iter_mut() {
        if parent.parent() != char_entity {
            continue;
        }
        if let Some(ref mut drive) = d {
            drive.advance(dt);
            let pin_target = instances
                .get(parent.parent())
                .ok()
                .and_then(|i| players.get_mut(i.player_entity()).ok());
            if let Some(mut player) = pin_target {
                write_turn_pins(&mut player, &drive);
            }
        }
    }
}

#[derive(Default)]
struct TurnYawSampler {
    yaw_env: f32,
    prev_view: Option<(f64, f32)>,
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
