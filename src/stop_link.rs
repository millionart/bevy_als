use als_core::input::AlsInputCommand;
use als_core::motion::AlsInputOverride;
use als_core::params::AlsMovementParams;
use als_core::start_stop::predict_stop_distance;
use als_core::state::{LocomotionState, RotationMode, Stance};
use avian3d::prelude::LinearVelocity;
use bevy::ecs::message::{Message, MessageReader};
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

use crate::character_assembly::AlsCharacterAvatar;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsStopMatch {
    pub active: bool,
    pub remaining: f32,
    pub total: f32,
    pub weight: f32,
    pub last_input_move: f32,
    pub quick_stop_active: bool,
    pub quick_stop_elapsed: f32,
    pub quick_stop_clip_time: f32,
    pub quick_stop_index: f32,
    pub quick_stop_play_rate: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsTransitionKind {
    Stop,
    QuickStop,
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
            duration: QUICK_STOP_DURATION,
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
        )
    }

    pub fn advance(&mut self, delta_seconds: f32) {
        if !self.active {
            return;
        }
        self.elapsed += delta_seconds.max(0.0);
        self.clip_time = self.start_time + self.elapsed * self.play_rate.max(f32::EPSILON);
        self.weight = transition_weight(
            self.elapsed,
            self.duration,
            self.start_time,
            self.play_rate,
            self.blend_in,
            self.blend_out,
        );
        if self.clip_time >= self.duration {
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

impl Default for AlsStopMatch {
    fn default() -> Self {
        Self {
            active: false,
            remaining: 0.0,
            total: 1.0,
            weight: 0.0,
            last_input_move: 0.0,
            quick_stop_active: false,
            quick_stop_elapsed: 0.0,
            quick_stop_clip_time: 0.0,
            quick_stop_index: 0.0,
            quick_stop_play_rate: 1.75,
        }
    }
}

pub const STOP_TRIGGER_MIN_SPEED: f32 = 1.0;
pub const STOP_RELEASE_INPUT_EPSILON: f32 = 0.05;
pub const QUICK_STOP_BLEND_IN: f32 = 0.1;
pub const QUICK_STOP_BLEND_OUT: f32 = 0.2;
pub const QUICK_STOP_START_TIME: f32 = 0.3;
pub const QUICK_STOP_DURATION: f32 = 2.33333325;
pub const QUICK_STOP_MIN_PLAY_RATE: f32 = 1.75;
pub const QUICK_STOP_MAX_PLAY_RATE: f32 = 3.0;
pub const STOP_TRANSITION_BLEND_IN: f32 = 0.2;
pub const STOP_TRANSITION_BLEND_OUT: f32 = 0.2;
pub const STOP_TRANSITION_PLAY_RATE: f32 = 1.5;
pub const STOP_TRANSITION_START_TIME: f32 = 0.4;

pub fn quick_stop_selection(
    rotation_mode: RotationMode,
    input: &AlsInputCommand,
    body: &Transform,
) -> (f32, f32) {
    if rotation_mode != RotationMode::VelocityDirection {
        return (0.0, QUICK_STOP_MIN_PLAY_RATE);
    }
    let input_yaw = if input.move_dir.length_squared() > f32::EPSILON {
        f32::atan2(input.move_dir.x, -input.move_dir.y).to_degrees()
    } else {
        let forward = body.rotation * Vec3::NEG_Z;
        f32::atan2(forward.x, -forward.z).to_degrees()
    };
    let forward = body.rotation * Vec3::NEG_Z;
    let body_yaw = f32::atan2(forward.x, -forward.z).to_degrees();
    let mut remaining = input_yaw - body_yaw;
    while remaining > 180.0 {
        remaining -= 360.0;
    }
    while remaining <= -180.0 {
        remaining += 360.0;
    }
    let rate = QUICK_STOP_MIN_PLAY_RATE.lerp(
        QUICK_STOP_MAX_PLAY_RATE,
        (remaining.abs() / 180.0).clamp(0.0, 1.0),
    );
    let index = if remaining <= 0.0 { 0.0 } else { 1.0 };
    (index, rate)
}

pub fn quick_stop_weight(elapsed: f32, play_rate: f32) -> f32 {
    transition_weight(
        elapsed,
        QUICK_STOP_DURATION,
        QUICK_STOP_START_TIME,
        play_rate,
        QUICK_STOP_BLEND_IN,
        QUICK_STOP_BLEND_OUT,
    )
}

pub fn transition_weight(
    elapsed: f32,
    duration: f32,
    start_time: f32,
    play_rate: f32,
    blend_in: f32,
    blend_out: f32,
) -> f32 {
    let playback_duration = (duration - start_time) / play_rate.max(f32::EPSILON);
    (elapsed / blend_in.max(f32::EPSILON))
        .min((playback_duration - elapsed) / blend_out.max(f32::EPSILON))
        .clamp(0.0, 1.0)
}

pub struct AlsStopLinkPlugin;

impl Plugin for AlsStopLinkPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AlsQuickStopNotify>().add_systems(
            Update,
            (
                ensure_stop_match,
                stop_link_system,
                quick_stop_notify_system,
                transition_playback_system,
            )
                .chain(),
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
            Entity,
            &LinearVelocity,
            &LocomotionState,
            &Stance,
            &als_core::character::AlsMovementParamsHandle,
            &mut AlsStopMatch,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    params: Res<Assets<AlsMovementParams>>,
    instances: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let dt = time.delta_secs();
    for (character, vel, loco, _stance, handle, mut m, cmd, ovr) in &mut q {
        let Some(p) = params.get(handle.id()) else {
            continue;
        };
        let effective = ovr.map(|o| &o.0).unwrap_or(cmd);
        let hspeed = Vec2::new(vel.x, vel.z).length();
        let input_move = effective.move_dir.length();

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
                m.total =
                    predict_stop_distance(hspeed, p.acceleration.braking_deceleration).max(0.3);
                m.remaining = m.total;
                m.active = true;
            }
        }
        m.weight = 0.0;
        m.quick_stop_active = false;
        m.quick_stop_elapsed = 0.0;
        m.quick_stop_clip_time = QUICK_STOP_START_TIME;
        m.quick_stop_index = 0.0;
        m.quick_stop_play_rate = QUICK_STOP_MIN_PLAY_RATE;
        m.last_input_move = input_move;

        for (child_of, instance) in instances.iter() {
            if child_of.parent() != character {
                continue;
            }
            if let Ok(mut player) = players.get_mut(instance.player_entity()) {
                player.set_input_data("stop_weight", DataValue::from(m.weight));
                player.set_input_data("stop_right_weight", DataValue::from(m.quick_stop_index));
                player.set_input_data("stop_remaining", DataValue::from(m.remaining));
                player.set_input_data("stop_total", DataValue::from(m.total));
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn quick_stop_notify_system(
    mut notices: MessageReader<AlsQuickStopNotify>,
    characters: Query<
        (
            &Transform,
            &RotationMode,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
        ),
        With<als_core::character::AlsCharacter>,
    >,
    mut commands: Commands,
) {
    for notice in notices.read() {
        let Ok((body, rotation_mode, command, override_command)) = characters.get(notice.character)
        else {
            continue;
        };
        let input = override_command.map_or(command, |value| &value.0);
        let (index, play_rate) = quick_stop_selection(*rotation_mode, input, body);
        commands
            .entity(notice.character)
            .insert(AlsTransitionPlayback::quick_stop(index, play_rate));
    }
}

fn transition_playback_system(time: Res<Time>, mut q: Query<&mut AlsTransitionPlayback>) {
    for mut playback in &mut q {
        playback.advance(time.delta_secs());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;

    #[derive(Resource)]
    struct NotifyTarget(Entity);

    fn emit_quick_stop(target: Res<NotifyTarget>, mut notices: MessageWriter<AlsQuickStopNotify>) {
        notices.write(AlsQuickStopNotify {
            character: target.0,
        });
    }

    #[test]
    fn quick_stop_latches_source_direction_and_rate() {
        let body = Transform::default();
        let mut input = AlsInputCommand::default();
        input.move_dir = Vec2::X;
        let (right, rate) = quick_stop_selection(RotationMode::VelocityDirection, &input, &body);
        assert_eq!(right, 1.0);
        assert!((rate - 2.375).abs() < 1e-6);

        input.move_dir = Vec2::NEG_X;
        let (left, rate) = quick_stop_selection(RotationMode::VelocityDirection, &input, &body);
        assert_eq!(left, 0.0);
        assert!((rate - 2.375).abs() < 1e-6);
    }

    #[test]
    fn quick_stop_weight_uses_source_blend_windows() {
        assert!((quick_stop_weight(0.05, QUICK_STOP_MIN_PLAY_RATE) - 0.5).abs() < 1e-6);
        assert!(
            (quick_stop_weight(QUICK_STOP_BLEND_IN, QUICK_STOP_MIN_PLAY_RATE) - 1.0).abs() < 1e-6
        );
        let duration = (QUICK_STOP_DURATION - QUICK_STOP_START_TIME) / QUICK_STOP_MIN_PLAY_RATE;
        assert!((quick_stop_weight(duration - 0.1, QUICK_STOP_MIN_PLAY_RATE) - 0.5).abs() < 1e-6);
        assert_eq!(quick_stop_weight(duration, QUICK_STOP_MIN_PLAY_RATE), 0.0);
    }

    #[test]
    fn stop_transition_uses_the_source_slot_parameters() {
        let playback = AlsTransitionPlayback::stop(1.0);
        assert_eq!(playback.kind, AlsTransitionKind::Stop);
        assert_eq!(playback.index, 1.0);
        assert_eq!(playback.blend_in, STOP_TRANSITION_BLEND_IN);
        assert_eq!(playback.blend_out, STOP_TRANSITION_BLEND_OUT);
        assert_eq!(playback.play_rate, STOP_TRANSITION_PLAY_RATE);
        assert_eq!(playback.start_time, STOP_TRANSITION_START_TIME);
        assert_eq!(playback.duration, QUICK_STOP_DURATION);
    }

    #[test]
    fn transition_playback_latches_and_ends_at_the_sequence_duration() {
        let mut playback = AlsTransitionPlayback::quick_stop(0.0, QUICK_STOP_MIN_PLAY_RATE);
        playback.advance(QUICK_STOP_BLEND_IN / 2.0);
        assert!((playback.weight - 0.5).abs() < 1e-6);
        assert!(playback.active);
        playback.advance(4.0);
        assert!(!playback.active);
        assert_eq!(playback.clip_time, QUICK_STOP_DURATION);
        assert_eq!(playback.weight, 0.0);
    }

    #[test]
    fn quick_stop_playback_starts_only_from_the_animation_notify_message() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<AlsQuickStopNotify>()
            .add_systems(Update, (emit_quick_stop, quick_stop_notify_system).chain());
        let character = app
            .world_mut()
            .spawn((
                als_core::character::AlsCharacter,
                Transform::default(),
                RotationMode::ViewDirection,
                AlsInputCommand::default(),
            ))
            .id();
        app.insert_resource(NotifyTarget(character));
        app.update();
        let playback = app.world().get::<AlsTransitionPlayback>(character).unwrap();
        assert_eq!(playback.kind, AlsTransitionKind::QuickStop);
        assert_eq!(playback.index, 0.0);
        assert_eq!(playback.play_rate, QUICK_STOP_MIN_PLAY_RATE);
        assert_eq!(playback.start_time, QUICK_STOP_START_TIME);
    }
}
