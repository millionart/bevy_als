use crate::state::LocomotionState;
use bevy::prelude::*;

pub const FLAIL_BLEND_START_DOWN_VY: f32 = 5.0;
pub const FLAIL_BLEND_FULL_DOWN_VY: f32 = 30.0;
pub const FLAIL_INTERP_SPEED: f32 = 5.0;

#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct AlsFlailState {
    pub weight: f32,
}

pub fn flail_target_weight(loco: &LocomotionState, vertical_velocity: f32) -> f32 {
    if *loco != LocomotionState::Air {
        return 0.0;
    }
    ((-vertical_velocity - FLAIL_BLEND_START_DOWN_VY)
        / (FLAIL_BLEND_FULL_DOWN_VY - FLAIL_BLEND_START_DOWN_VY))
        .clamp(0.0, 1.0)
}

impl AlsFlailState {
    pub fn update(&mut self, loco: &LocomotionState, vertical_velocity: f32, dt: f32) {
        if *loco != LocomotionState::Air {
            self.weight = 0.0;
            return;
        }
        let target = flail_target_weight(loco, vertical_velocity);
        let alpha = (FLAIL_INTERP_SPEED * dt.max(0.0)).clamp(0.0, 1.0);
        self.weight += (target - self.weight) * alpha;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flail_target_uses_als_vertical_velocity_range() {
        assert_eq!(flail_target_weight(&LocomotionState::Air, -5.0), 0.0);
        assert!((flail_target_weight(&LocomotionState::Air, -17.5) - 0.5).abs() < 1e-6);
        assert_eq!(flail_target_weight(&LocomotionState::Air, -30.0), 1.0);
        assert_eq!(flail_target_weight(&LocomotionState::Grounded, -30.0), 0.0);
    }

    #[test]
    fn flail_uses_als_interp_speed() {
        let mut state = AlsFlailState::default();
        state.update(&LocomotionState::Air, -17.5, 0.1);
        assert!((state.weight - 0.25).abs() < 1e-6);
    }

    #[test]
    fn flail_exits_with_the_air_state() {
        let mut state = AlsFlailState { weight: 1.0 };
        state.update(&LocomotionState::Grounded, -30.0, 0.1);
        assert_eq!(state.weight, 0.0);
    }
}
