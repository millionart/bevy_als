use bevy::prelude::*;

#[derive(Resource, Clone, Copy)]
pub(crate) struct SourceConvexMarginSettings {
    pub fraction: f32,
    pub maximum: f32,
    pub minimum: f32,
}

pub(crate) fn source_box_margin(half_extents: Vec3, settings: SourceConvexMarginSettings) -> f32 {
    (2.0 * half_extents.min_element() * settings.fraction).min(settings.maximum)
}

pub(crate) fn source_polygon_margins(
    first: f32,
    first_dynamic: bool,
    second: f32,
    second_dynamic: bool,
    minimum: f32,
) -> [f32; 2] {
    let first = if first_dynamic { first } else { 0.0 };
    let second = if second_dynamic { second } else { 0.0 };
    if first == 0.0 { [0.0, second.max(minimum)] }
    else if second == 0.0 { [first.max(minimum), 0.0] }
    else { [first.min(second); 2] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_box_and_polygon_margins_follow_dynamic_body_and_world_limit() {
        let [fraction, maximum, minimum] = include!("../als/settings/convex_margin.rs");
        let settings = SourceConvexMarginSettings { fraction, maximum, minimum };
        let foot = source_box_margin(Vec3::new(0.06, 0.06, 0.14), settings);
        let hand = source_box_margin(Vec3::new(0.08, 0.055, 0.035), settings);
        assert!((foot - 0.006).abs() < 1e-8);
        assert!((hand - 0.0035).abs() < 1e-8);
        assert_eq!(source_box_margin(Vec3::splat(4.0), settings), maximum);
        assert_eq!(source_polygon_margins(foot, true, 0.02, false, minimum), [foot, 0.0]);
        assert_eq!(source_polygon_margins(foot, false, hand, true, minimum), [0.0, hand]);
        assert_eq!(source_polygon_margins(foot, true, hand, true, minimum), [hand, hand]);
        assert_eq!(source_polygon_margins(foot, false, hand, false, minimum), [0.0, minimum]);
    }
}
