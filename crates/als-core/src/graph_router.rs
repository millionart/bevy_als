use crate::motion::effective_gait;
use crate::params::AlsMovementParams;
use crate::state::{Gait, Stance};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphInputs {
    pub speed: f32,
    pub actual_speed: f32,
}

pub fn compute_graph_inputs(
    params: &AlsMovementParams,
    stance: Stance,
    gait: Gait,
    horizontal_speed: f32,
) -> GraphInputs {
    let effective = effective_gait(stance, gait);
    let target = crate::motion::gait_max_speed(params, &stance, &effective);
    let speed = if horizontal_speed.abs() > 1e-3 {
        horizontal_speed.abs().clamp(0.0, target)
    } else {
        0.0
    };
    GraphInputs {
        speed,
        actual_speed: horizontal_speed.abs(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> AlsMovementParams {
        AlsMovementParams::default()
    }

    #[test]
    fn standing_still_is_zero() {
        let g = compute_graph_inputs(&params(), Stance::Standing, Gait::Walking, 0.0);
        assert_eq!(g.speed, 0.0);
        assert_eq!(g.actual_speed, 0.0);
    }

    #[test]
    fn walk_speed_passes_through_when_under_target() {
        let g = compute_graph_inputs(&params(), Stance::Standing, Gait::Walking, 1.2);
        assert!((g.speed - 1.2).abs() < 1e-5);
        assert_eq!(g.actual_speed, 1.2);
    }

    #[test]
    fn sprint_clamps_to_gait_target() {
        let g = compute_graph_inputs(&params(), Stance::Standing, Gait::Running, 6.0);
        assert!((g.speed - 3.75).abs() < 1e-5);
        assert_eq!(g.actual_speed, 6.0);
    }

    #[test]
    fn crouching_clamps_walk_input_to_crouch_speed() {
        let p = params();
        let crouch_walk = p.crouch.max_walk_speed_crouched;
        let g = compute_graph_inputs(&p, Stance::Crouching, Gait::Walking, 10.0);
        assert!((g.speed - crouch_walk).abs() < 1e-5);
    }
}
