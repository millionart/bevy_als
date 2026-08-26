use als_core::air::AlsAirState;
use als_core::graph_router::compute_graph_inputs;
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, Stance};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

pub const STRIDE_RATE_MIN: f32 = 0.6;
pub const STRIDE_RATE_MAX: f32 = 1.6;
pub const STRIDE_IDLE_SPEED: f32 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionGraphInputs {
    pub speed: f32,
    pub actual_speed: f32,
    pub play_rate: f32,
}

pub fn compute_motion_graph_inputs(
    params: &AlsMovementParams,
    stance: Stance,
    gait: Gait,
    horizontal_speed: f32,
) -> MotionGraphInputs {
    let gi = compute_graph_inputs(params, stance, gait, horizontal_speed);
    MotionGraphInputs {
        speed: gi.speed,
        actual_speed: gi.actual_speed,
        play_rate: stride_play_rate(gi.speed, gi.actual_speed),
    }
}

pub fn stride_play_rate(speed_pin: f32, actual_speed: f32) -> f32 {
    if actual_speed < STRIDE_IDLE_SPEED || speed_pin < 1e-3 {
        return 1.0;
    }
    (actual_speed / speed_pin).clamp(STRIDE_RATE_MIN, STRIDE_RATE_MAX)
}

pub fn write_motion_graph_pins(player: &mut AnimationGraphPlayer, inputs: &MotionGraphInputs) {
    player.set_input_data("speed", DataValue::from(inputs.speed));
    player.set_input_data("actual_speed", DataValue::from(inputs.actual_speed));
    player.set_input_data("play_rate", DataValue::from(inputs.play_rate));
}

pub fn write_air_pins(player: &mut AnimationGraphPlayer, air: &AlsAirState) {
    player.set_input_data("air_weight", DataValue::from(air.air_weight));
    player.set_input_data("fall_fast_weight", DataValue::from(air.fall_fast_weight));
    player.set_input_data("land_weight", DataValue::from(air.land_weight));
    player.set_input_data("land_heavy_weight", DataValue::from(air.land_heavy_weight));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_inputs_are_neutral() {
        let p = AlsMovementParams::default();
        let m = compute_motion_graph_inputs(&p, Stance::Standing, Gait::Walking, 0.0);
        assert_eq!(m.speed, 0.0);
        assert_eq!(m.actual_speed, 0.0);
        assert_eq!(m.play_rate, 1.0);
    }

    #[test]
    fn matched_speed_keeps_native_play_rate() {
        let p = AlsMovementParams::default();
        let m = compute_motion_graph_inputs(&p, Stance::Standing, Gait::Running, 3.75);
        assert!((m.play_rate - 1.0).abs() < 1e-6);
    }

    #[test]
    fn crouch_clamps_and_preserves_actual() {
        let p = AlsMovementParams::default();
        let m = compute_motion_graph_inputs(&p, Stance::Crouching, Gait::Sprinting, 1.5);
        assert!((m.speed - 1.5).abs() < 1e-5);
        assert!((m.actual_speed - 1.5).abs() < 1e-5);
        assert!((m.play_rate - 1.0).abs() < 1e-6);
    }
}
