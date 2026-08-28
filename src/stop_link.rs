use als_core::dynamics::{AlsGroundedDynamics, predict_grounded_stop_distance};
use als_core::input::AlsInputCommand;
use als_core::motion::{AlsInputOverride, AlsViewYaw, view_relative_move_direction};
use als_core::params::AlsMovementParams;
use als_core::start_stop::predict_stop_distance;
use als_core::state::{LocomotionState, RotationMode, Stance};
use als_core::transitions::{TransitionConfig, select_quick_stop};
use avian3d::prelude::LinearVelocity;
use bevy::ecs::message::{Message, MessageReader};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

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

    pub fn crouching_stop_right() -> Self {
        Self::new(
            AlsTransitionKind::CrouchingStop,
            1.0,
            STOP_TRANSITION_BLEND_IN,
            STOP_TRANSITION_BLEND_OUT,
            STOP_TRANSITION_PLAY_RATE,
            CROUCH_STOP_TRANSITION_START_TIME,
            TRANSITION_SEQUENCE_DURATION,
        )
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

    pub fn is_crouching(self) -> bool {
        matches!(
            self.kind,
            AlsTransitionKind::CrouchingStop
                | AlsTransitionKind::CrouchingQuickStop
                | AlsTransitionKind::CrouchingDynamic
        )
    }

    pub fn advance(&mut self, delta_seconds: f32) {
        if !self.active {
            return;
        }
        self.elapsed += delta_seconds.max(0.0);
        let play_rate = self.play_rate.max(f32::EPSILON);
        let playback_duration = (self.duration - self.start_time).max(0.0) / play_rate;
        self.clip_time = (self.start_time + self.elapsed * play_rate).min(self.duration);
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
pub const CROUCH_STOP_TRANSITION_START_TIME: f32 = 0.2;
pub const TRANSITION_SEQUENCE_DURATION: f32 = 2.3333333;
pub const CROUCH_DYNAMIC_TRANSITION_DURATION: f32 = 1.5333333;
pub const DYNAMIC_TRANSITION_BLEND: f32 = 0.2;
pub const DYNAMIC_TRANSITION_PLAY_RATE: f32 = 1.5;

pub fn quick_stop_selection(
    rotation_mode: RotationMode,
    input: &AlsInputCommand,
    view_yaw: f32,
    body: &Transform,
) -> (f32, f32) {
    if rotation_mode != RotationMode::VelocityDirection {
        return (0.0, QUICK_STOP_MIN_PLAY_RATE);
    }
    let forward = body.rotation * Vec3::NEG_Z;
    let body_yaw = f32::atan2(forward.x, -forward.z).to_degrees();
    let input_yaw = if input.move_dir.length_squared() > f32::EPSILON {
        let direction = view_relative_move_direction(input.move_dir, view_yaw);
        f32::atan2(direction.x, -direction.z).to_degrees()
    } else {
        body_yaw
    };
    let remaining_yaw = (input_yaw - body_yaw + 180.0).rem_euclid(360.0) - 180.0;
    let play_rate = QUICK_STOP_MIN_PLAY_RATE.lerp(
        QUICK_STOP_MAX_PLAY_RATE,
        (remaining_yaw.abs() / 180.0).clamp(0.0, 1.0),
    );
    let index = if remaining_yaw <= 0.0 { 0.0 } else { 1.0 };
    (index, play_rate)
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
        app.add_message::<AlsQuickStopNotify>().add_systems(
            Update,
            (
                ensure_stop_match,
                stop_link_system.after(ensure_stop_match),
                quick_stop_notify_system,
                transition_playback_system.after(quick_stop_notify_system),
            ),
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
            m.last_input_yaw = Some(f32::atan2(direction.x, -direction.z));
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
            let dynamic_distance =
                predict_grounded_stop_distance(p, *stance, *rotation_mode, hspeed);
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
        ),
        With<als_core::character::AlsCharacter>,
    >,
    mut commands: Commands,
) {
    for notice in notices.read() {
        let Ok((body, rotation_mode, stance, view_yaw, command, override_command)) =
            characters.get(notice.character)
        else {
            continue;
        };
        let input = override_command.map_or(command, |value| &value.0);
        let (index, play_rate) = quick_stop_selection(*rotation_mode, input, view_yaw.0, body);
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
            quick_stop_selection(RotationMode::VelocityDirection, &input, 0.0, &body);
        assert_eq!(index, 0.0);
        assert_eq!(play_rate, QUICK_STOP_MIN_PLAY_RATE);
        let playback = AlsTransitionPlayback::quick_stop(index, play_rate);
        assert_eq!(playback.blend_in, QUICK_STOP_BLEND_IN);
        assert_eq!(playback.blend_out, QUICK_STOP_BLEND_OUT);
        assert_eq!(playback.start_time, QUICK_STOP_START_TIME);
    }
}
