use als_core::input::AlsInputCommand;
use als_core::motion::AlsInputOverride;
use als_core::params::AlsMovementParams;
use als_core::start_stop::predict_stop_distance;
use als_core::state::{LocomotionState, Stance};
use avian3d::prelude::LinearVelocity;
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
        }
    }
}

pub const STOP_TRIGGER_MIN_SPEED: f32 = 1.0;
pub const STOP_RELEASE_INPUT_EPSILON: f32 = 0.05;
const WEIGHT_IN_SPEED: f32 = 8.0;
const WEIGHT_OUT_SPEED: f32 = 5.0;

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
        app.add_systems(
            Update,
            (ensure_stop_match, stop_link_system.after(ensure_stop_match)),
        );
    }
}

fn ensure_stop_match(
    mut commands: Commands,
    q: Query<Entity, (With<als_core::character::AlsCharacter>, Without<AlsStopMatch>)>,
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
            &als_core::character::AlsMovementParamsHandle,
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
    let Ok((vel, loco, _stance, handle, mut m, cmd, ovr)) = q.single_mut() else {
        return;
    };
    let Some(p) = params.get(handle.id()) else {
        return;
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
            m.left_weight = 1.0;
            m.right_weight = 0.0;
        }
    }
    m.last_input_move = input_move;

    let weight_target = if m.active { 1.0 } else { 0.0 };
    let w_speed = if m.active { WEIGHT_IN_SPEED } else { WEIGHT_OUT_SPEED };
    m.weight = move_toward(m.weight, weight_target, w_speed * dt);

    for instance in instances.iter() {
        if let Ok(mut player) = players.get_mut(instance.player_entity()) {
            player.set_input_data("stop_weight", DataValue::from(m.weight));
            player.set_input_data("stop_right_weight", DataValue::from(m.right_weight));
            player.set_input_data("stop_remaining", DataValue::from(m.remaining));
            player.set_input_data("stop_total", DataValue::from(m.total));
        }
    }
}
