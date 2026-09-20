use als_core::air::AlsAirState;
use als_core::gait::{
    GaitWeights, als_play_rate, calculate_gait_amount, stride_blend_amount_for_speed,
};
use als_core::graph_router::compute_graph_inputs;
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, Stance};
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::edge_data::DataValue;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionGraphInputs {
    pub speed: f32,
    pub actual_speed: f32,
    pub play_rate: f32,
    pub gait_move_weight: f32,
    pub gait_run_weight: f32,
    pub gait_sprint_weight: f32,
}

fn motion_gait_weights(
    params: &AlsMovementParams,
    stance: Stance,
    horizontal_speed: f32,
) -> GaitWeights {
    if horizontal_speed <= 0.05 {
        return GaitWeights::default();
    }
    match stance {
        value if value != Stance::Crouching => {
            let continuous = GaitWeights::from_gait_amount(calculate_gait_amount(
                horizontal_speed,
                params.stance_speeds(stance),
            ));
            GaitWeights {
                move_w: 1.0,
                run_w: continuous.run_w,
                sprint_w: continuous.sprint_w,
            }
        }
        _ => GaitWeights::targets(Gait::Walking),
    }
}

pub fn compute_motion_graph_inputs(
    params: &AlsMovementParams,
    stance: Stance,
    gait: Gait,
    horizontal_speed: f32,
) -> MotionGraphInputs {
    let gi = compute_graph_inputs(params, stance, gait, horizontal_speed);
    let graph_speed = horizontal_speed.max(gi.speed);
    let weights = motion_gait_weights(params, stance, graph_speed);
    let stride = stride_blend_amount_for_speed(weights.running_amount(), graph_speed);
    let pr = als_play_rate(params, stance, graph_speed, stride, weights);
    MotionGraphInputs {
        speed: gi.speed,
        actual_speed: gi.actual_speed,
        play_rate: pr.play_rate,
        gait_move_weight: weights.move_w,
        gait_run_weight: weights.run_w,
        gait_sprint_weight: weights.sprint_w,
    }
}

pub fn compute_motion_graph_inputs_blended(
    params: &AlsMovementParams,
    stance: Stance,
    gait: Gait,
    horizontal_speed: f32,
    speed_ema: f32,
    weights: GaitWeights,
) -> MotionGraphInputs {
    let gi = compute_graph_inputs(params, stance, gait, horizontal_speed);
    let stride = stride_blend_amount_for_speed(weights.running_amount(), speed_ema);
    let pr = als_play_rate(params, stance, speed_ema, stride, weights);
    MotionGraphInputs {
        speed: gi.speed,
        actual_speed: gi.actual_speed,
        play_rate: pr.play_rate,
        gait_move_weight: weights.move_w,
        gait_run_weight: weights.run_w,
        gait_sprint_weight: weights.sprint_w,
    }
}

pub fn write_motion_graph_pins(player: &mut AnimationGraphPlayer, inputs: &MotionGraphInputs) {
    player.set_input_data("speed", DataValue::from(inputs.speed));
    player.set_input_data("actual_speed", DataValue::from(inputs.actual_speed));
    player.set_input_data("play_rate", DataValue::from(inputs.play_rate));
    player.set_input_data("gait_move_weight", DataValue::from(inputs.gait_move_weight));
    player.set_input_data("gait_run_weight", DataValue::from(inputs.gait_run_weight));
    player.set_input_data(
        "gait_sprint_weight",
        DataValue::from(inputs.gait_sprint_weight),
    );
}

pub fn write_air_pins(player: &mut AnimationGraphPlayer, air: &AlsAirState) {
    write_air_pins_with_flail(player, air, &als_core::flail::AlsFlailState::default());
}

pub fn write_air_pins_with_flail(
    player: &mut AnimationGraphPlayer,
    air: &AlsAirState,
    flail: &als_core::flail::AlsFlailState,
) {
    player.set_input_data("air_weight", DataValue::from(air.air_weight));
    player.set_input_data("fall_fast_weight", DataValue::from(air.fall_fast_weight));
    player.set_input_data("land_weight", DataValue::from(air.land_weight));
    player.set_input_data("land_heavy_weight", DataValue::from(air.land_heavy_weight));
    player.set_input_data("flail_weight", DataValue::from(flail.weight));
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
        assert_eq!(m.gait_move_weight, 0.0);
        assert_eq!(m.play_rate, als_core::gait::MIN_PLAY_RATE);
    }

    #[test]
    fn walking_uses_als_walk_play_rate() {
        let p = AlsMovementParams::default();
        let m = compute_motion_graph_inputs(&p, Stance::Standing, Gait::Walking, 1.5);
        let stride = stride_blend_amount_for_speed(0.0, 1.5);
        assert!((stride - 1.0).abs() < 1e-3);
        assert!((m.play_rate - 1.5 / 1.5 / stride).abs() < 1e-4);
        assert!((m.gait_move_weight - 1.0).abs() < 1e-6);
        assert!((m.gait_run_weight - 0.0).abs() < 1e-6);
    }

    #[test]
    fn crouch_clamps_to_crouch_speed() {
        let p = AlsMovementParams::default();
        let m = compute_motion_graph_inputs(&p, Stance::Crouching, Gait::Sprinting, 1.5);
        assert!((m.speed - 1.5).abs() < 1e-5);
        let stride = stride_blend_amount_for_speed(0.0, 1.5);
        assert!((m.play_rate - 1.5 / 1.5 / stride).abs() < 1e-4);
        assert!((m.gait_move_weight - 1.0).abs() < 1e-6);
        assert!((m.gait_run_weight - 0.0).abs() < 1e-6);
        assert!((m.gait_sprint_weight - 0.0).abs() < 1e-6);
    }

    #[test]
    fn sprint_play_rate_never_exceeds_three() {
        let p = AlsMovementParams::default();
        let m = compute_motion_graph_inputs(&p, Stance::Standing, Gait::Sprinting, 6.5);
        assert!(m.play_rate <= 3.0 + 1e-5);
        assert!((m.gait_sprint_weight - 1.0).abs() < 1e-6);
    }

    #[test]
    fn blended_inputs_use_ema_and_weights() {
        let p = AlsMovementParams::default();
        let w = GaitWeights {
            move_w: 1.0,
            run_w: 0.5,
            sprint_w: 0.0,
        };
        let m =
            compute_motion_graph_inputs_blended(&p, Stance::Standing, Gait::Running, 3.75, 2.0, w);
        let expected_stride = stride_blend_amount_for_speed(0.5, 2.0);
        let walk_rate = 2.0_f32 / 1.5;
        let run_rate = 2.0 / 3.5;
        let expected_rate = (walk_rate + (run_rate - walk_rate) * 0.5) / expected_stride;
        assert!((m.play_rate - expected_rate).abs() < 1e-4);
        assert!((m.gait_move_weight - 1.0).abs() < 1e-6);
        assert!((m.gait_run_weight - 0.5).abs() < 1e-6);
    }

    #[test]
    fn ordinary_inputs_propagate_continuous_running_weight_to_play_rate() {
        let p = AlsMovementParams::default();
        let m = compute_motion_graph_inputs(&p, Stance::Standing, Gait::Running, 3.0);
        let expected_weights = GaitWeights {
            move_w: 1.0,
            run_w: 0.625,
            sprint_w: 0.0,
        };
        let expected_stride = stride_blend_amount_for_speed(expected_weights.running_amount(), 3.0);
        let expected_rate =
            als_play_rate(&p, Stance::Standing, 3.0, expected_stride, expected_weights).play_rate;
        println!(
            "[BF101.6] ordinary_run_weight={:.6} ordinary_play_rate={:.6}",
            m.gait_run_weight, m.play_rate
        );
        assert!((m.gait_run_weight - expected_weights.run_w).abs() < 1e-6);
        assert!((m.play_rate - expected_rate).abs() < 1e-6);
    }
}
