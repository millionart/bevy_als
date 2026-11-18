use bevy::prelude::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlsMovementDirection {
    #[default]
    Forward,
    RightForward,
    RightBackward,
    Backward,
    LeftBackward,
    LeftForward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsGroundedMovementIntent {
    Forward,
    Right,
    Left,
    Backward,
}

pub const GROUNDED_MOVEMENT_FORWARD_HALF_ANGLE: f32 = 70.0;
pub const GROUNDED_MOVEMENT_ANGLE_THRESHOLD: f32 = 5.0;

pub const SECTOR_BOUNDARY_FORWARD_SIDE: f32 = 60.0;
pub const SECTOR_BOUNDARY_SIDE_DIAG: f32 = 120.0;
pub const SECTOR_BOUNDARY_DIAG_BACK: f32 = 150.0;
pub const SECTOR_HYSTERESIS: f32 = 5.0;

pub fn normalize_angle180(angle: f32) -> f32 {
    let mut a = angle;
    while a > 180.0 {
        a -= 360.0;
    }
    while a < -180.0 {
        a += 360.0;
    }
    a
}

pub fn classify_grounded_movement_intent(angle: f32) -> AlsGroundedMovementIntent {
    let angle = normalize_angle180(angle);
    let forward_limit = GROUNDED_MOVEMENT_FORWARD_HALF_ANGLE + GROUNDED_MOVEMENT_ANGLE_THRESHOLD;
    let side_start = GROUNDED_MOVEMENT_FORWARD_HALF_ANGLE - GROUNDED_MOVEMENT_ANGLE_THRESHOLD;
    let side_end = 180.0 - GROUNDED_MOVEMENT_FORWARD_HALF_ANGLE + GROUNDED_MOVEMENT_ANGLE_THRESHOLD;
    if (-forward_limit..=forward_limit).contains(&angle) {
        AlsGroundedMovementIntent::Forward
    } else if (side_start..=side_end).contains(&angle) {
        AlsGroundedMovementIntent::Right
    } else if (-side_end..=-side_start).contains(&angle) {
        AlsGroundedMovementIntent::Left
    } else {
        AlsGroundedMovementIntent::Backward
    }
}

pub fn advance_standing_movement_state(
    current: AlsMovementDirection,
    intent: AlsGroundedMovementIntent,
) -> AlsMovementDirection {
    match (current, intent) {
        (
            AlsMovementDirection::Forward
            | AlsMovementDirection::Backward
            | AlsMovementDirection::RightForward
            | AlsMovementDirection::LeftForward,
            AlsGroundedMovementIntent::Forward,
        ) => AlsMovementDirection::Forward,
        (AlsMovementDirection::RightBackward, AlsGroundedMovementIntent::Forward) => {
            AlsMovementDirection::RightForward
        }
        (AlsMovementDirection::LeftBackward, AlsGroundedMovementIntent::Forward) => {
            AlsMovementDirection::LeftForward
        }
        (
            AlsMovementDirection::Forward | AlsMovementDirection::Backward,
            AlsGroundedMovementIntent::Backward,
        ) => AlsMovementDirection::Backward,
        (
            AlsMovementDirection::RightForward | AlsMovementDirection::RightBackward,
            AlsGroundedMovementIntent::Backward,
        ) => AlsMovementDirection::RightBackward,
        (
            AlsMovementDirection::LeftForward | AlsMovementDirection::LeftBackward,
            AlsGroundedMovementIntent::Backward,
        ) => AlsMovementDirection::LeftBackward,
        (
            AlsMovementDirection::Forward
            | AlsMovementDirection::RightForward
            | AlsMovementDirection::LeftBackward,
            AlsGroundedMovementIntent::Right,
        ) => AlsMovementDirection::RightForward,
        (
            AlsMovementDirection::Backward | AlsMovementDirection::LeftForward,
            AlsGroundedMovementIntent::Right,
        ) => AlsMovementDirection::RightBackward,
        (AlsMovementDirection::RightBackward, AlsGroundedMovementIntent::Right) => {
            AlsMovementDirection::RightBackward
        }
        (
            AlsMovementDirection::Forward
            | AlsMovementDirection::LeftForward
            | AlsMovementDirection::RightBackward,
            AlsGroundedMovementIntent::Left,
        ) => AlsMovementDirection::LeftForward,
        (
            AlsMovementDirection::Backward | AlsMovementDirection::RightForward,
            AlsGroundedMovementIntent::Left,
        ) => AlsMovementDirection::LeftBackward,
        (AlsMovementDirection::LeftBackward, AlsGroundedMovementIntent::Left) => {
            AlsMovementDirection::LeftBackward
        }
    }
}

pub fn classify_direction(angle: f32) -> AlsMovementDirection {
    let a = normalize_angle180(angle);
    let abs = a.abs();
    if abs <= SECTOR_BOUNDARY_FORWARD_SIDE {
        AlsMovementDirection::Forward
    } else if abs <= SECTOR_BOUNDARY_SIDE_DIAG {
        if a > 0.0 {
            AlsMovementDirection::RightForward
        } else {
            AlsMovementDirection::LeftForward
        }
    } else if abs <= SECTOR_BOUNDARY_DIAG_BACK {
        if a > 0.0 {
            AlsMovementDirection::RightBackward
        } else {
            AlsMovementDirection::LeftBackward
        }
    } else {
        AlsMovementDirection::Backward
    }
}

pub fn update_direction(current: Option<AlsMovementDirection>, angle: f32) -> AlsMovementDirection {
    let a = normalize_angle180(angle);
    if let Some(cur) = current {
        let in_current = match cur {
            AlsMovementDirection::Forward => {
                a >= -SECTOR_BOUNDARY_FORWARD_SIDE - SECTOR_HYSTERESIS
                    && a <= SECTOR_BOUNDARY_FORWARD_SIDE + SECTOR_HYSTERESIS
            }
            AlsMovementDirection::RightForward => {
                a > SECTOR_BOUNDARY_FORWARD_SIDE - SECTOR_HYSTERESIS
                    && a <= SECTOR_BOUNDARY_SIDE_DIAG + SECTOR_HYSTERESIS
            }
            AlsMovementDirection::RightBackward => {
                a > SECTOR_BOUNDARY_SIDE_DIAG - SECTOR_HYSTERESIS
                    && a <= SECTOR_BOUNDARY_DIAG_BACK + SECTOR_HYSTERESIS
            }
            AlsMovementDirection::Backward => {
                a > SECTOR_BOUNDARY_DIAG_BACK - SECTOR_HYSTERESIS
                    || a <= -SECTOR_BOUNDARY_DIAG_BACK + SECTOR_HYSTERESIS
            }
            AlsMovementDirection::LeftBackward => {
                a < -SECTOR_BOUNDARY_SIDE_DIAG + SECTOR_HYSTERESIS
                    && a >= -SECTOR_BOUNDARY_DIAG_BACK - SECTOR_HYSTERESIS
            }
            AlsMovementDirection::LeftForward => {
                a < -SECTOR_BOUNDARY_FORWARD_SIDE + SECTOR_HYSTERESIS
                    && a >= -SECTOR_BOUNDARY_SIDE_DIAG - SECTOR_HYSTERESIS
            }
        };
        if in_current {
            return cur;
        }
    }
    classify_direction(a)
}

pub fn unwrap_angle_continuous(previous: f32, next: f32) -> f32 {
    let mut delta = normalize_angle180(next - previous);
    if delta > 180.0 - f32::EPSILON {
        delta = -180.0;
    }
    previous + delta
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsMovementDirectionState {
    pub direction: AlsMovementDirection,
    pub angle: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sectors_partition_circle() {
        assert_eq!(classify_direction(0.0), AlsMovementDirection::Forward);
        assert_eq!(classify_direction(45.0), AlsMovementDirection::Forward);
        assert_eq!(classify_direction(70.0), AlsMovementDirection::RightForward);
        assert_eq!(
            classify_direction(130.0),
            AlsMovementDirection::RightBackward
        );
        assert_eq!(classify_direction(170.0), AlsMovementDirection::Backward);
        assert_eq!(classify_direction(-170.0), AlsMovementDirection::Backward);
        assert_eq!(
            classify_direction(-130.0),
            AlsMovementDirection::LeftBackward
        );
        assert_eq!(classify_direction(-70.0), AlsMovementDirection::LeftForward);
        assert_eq!(classify_direction(180.0), AlsMovementDirection::Backward);
        assert_eq!(classify_direction(-180.0), AlsMovementDirection::Backward);
    }

    #[test]
    fn hysteresis_keeps_current_near_boundary() {
        let cur = AlsMovementDirection::Forward;
        assert_eq!(
            update_direction(Some(cur), SECTOR_BOUNDARY_FORWARD_SIDE + 2.0),
            AlsMovementDirection::Forward
        );
        assert_eq!(
            update_direction(Some(cur), SECTOR_BOUNDARY_FORWARD_SIDE + 8.0),
            AlsMovementDirection::RightForward
        );
    }

    #[test]
    fn hysteresis_wraps_around_backward() {
        let cur = AlsMovementDirection::Backward;
        assert_eq!(
            update_direction(Some(cur), 147.0),
            AlsMovementDirection::Backward
        );
        assert_eq!(
            update_direction(Some(cur), -147.0),
            AlsMovementDirection::Backward
        );
        assert_eq!(
            update_direction(Some(cur), 140.0),
            AlsMovementDirection::RightBackward
        );
    }

    #[test]
    fn first_update_classifies_directly() {
        assert_eq!(
            update_direction(None, 100.0),
            AlsMovementDirection::RightForward
        );
    }

    #[test]
    fn unwrap_keeps_continuity_across_pm180() {
        assert!((unwrap_angle_continuous(179.0, -179.0) - 181.0).abs() < 1e-4);
        assert!((unwrap_angle_continuous(-179.0, 179.0) + 181.0).abs() < 1e-4);
        assert!((unwrap_angle_continuous(10.0, -10.0) + 10.0).abs() < 1e-4);
    }

    #[test]
    fn normalize_maps_into_pm180() {
        assert!((normalize_angle180(270.0) + 90.0).abs() < 1e-4);
        assert!((normalize_angle180(-270.0) - 90.0).abs() < 1e-4);
        assert!((normalize_angle180(180.0) - 180.0).abs() < 1e-4);
    }

    #[test]
    fn grounded_intent_uses_the_source_forward_half_angle_and_threshold() {
        assert_eq!(
            classify_grounded_movement_intent(75.0),
            AlsGroundedMovementIntent::Forward
        );
        assert_eq!(
            classify_grounded_movement_intent(76.0),
            AlsGroundedMovementIntent::Right
        );
        assert_eq!(
            classify_grounded_movement_intent(115.0),
            AlsGroundedMovementIntent::Right
        );
        assert_eq!(
            classify_grounded_movement_intent(116.0),
            AlsGroundedMovementIntent::Backward
        );
        assert_eq!(
            classify_grounded_movement_intent(-90.0),
            AlsGroundedMovementIntent::Left
        );
    }

    #[test]
    fn standing_state_sequence_follows_source_transition_endpoints() {
        let mut state = AlsMovementDirection::Forward;
        for (intent, expected) in [
            (
                AlsGroundedMovementIntent::Backward,
                AlsMovementDirection::Backward,
            ),
            (
                AlsGroundedMovementIntent::Right,
                AlsMovementDirection::RightBackward,
            ),
            (
                AlsGroundedMovementIntent::Left,
                AlsMovementDirection::LeftForward,
            ),
            (
                AlsGroundedMovementIntent::Forward,
                AlsMovementDirection::Forward,
            ),
            (
                AlsGroundedMovementIntent::Left,
                AlsMovementDirection::LeftForward,
            ),
            (
                AlsGroundedMovementIntent::Backward,
                AlsMovementDirection::LeftBackward,
            ),
            (
                AlsGroundedMovementIntent::Right,
                AlsMovementDirection::RightForward,
            ),
        ] {
            state = advance_standing_movement_state(state, intent);
            assert_eq!(state, expected);
        }
    }
}
