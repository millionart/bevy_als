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
}
