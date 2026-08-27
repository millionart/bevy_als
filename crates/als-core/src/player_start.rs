use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

pub const ALS_PLAYER_START_SOURCE_CLEARANCE: f32 = 0.02;
pub const PLAYER_START_VERTICAL_ADJUSTMENTS: [f32; 8] =
    [0.0, 0.005, 0.01, 0.02, 0.04, 0.08, 0.16, 0.32];

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsPlayerStart;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsPlayerStartReady {
    pub transform: Transform,
    pub ground_y: f32,
    pub adjustment: f32,
}

#[derive(Component, Debug, Default, Clone, Copy)]
struct AlsPlayerStartProbe {
    ground_y: f32,
    adjustment: f32,
    stable_frames: u32,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct AlsPlayerStartSettings {
    pub capsule_half_height: f32,
    pub capsule_radius: f32,
    pub probe_height: f32,
    pub probe_distance: f32,
    pub required_stable_frames: u32,
}

impl Default for AlsPlayerStartSettings {
    fn default() -> Self {
        Self {
            capsule_half_height: crate::character::ALS_CHARACTER_CAPSULE_HALF_HEIGHT,
            capsule_radius: crate::character::ALS_CHARACTER_CAPSULE_RADIUS,
            probe_height: 5.0,
            probe_distance: 10.0,
            required_stable_frames: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsPlayerSpawnFit {
    pub transform: Transform,
    pub ground_y: f32,
    pub adjustment: f32,
}

pub fn player_start_grounded_height(ground_y: f32, capsule_half_height: f32) -> f32 {
    ground_y + capsule_half_height + ALS_PLAYER_START_SOURCE_CLEARANCE
}

pub fn fit_player_start(
    spatial: &SpatialQuery,
    start: Transform,
    ground_y: f32,
    capsule_half_height: f32,
    capsule_radius: f32,
) -> Option<AlsPlayerSpawnFit> {
    let capsule = crate::character::als_character_capsule(capsule_half_height, capsule_radius);
    let base_center_y = player_start_grounded_height(ground_y, capsule_half_height);
    for adjustment in PLAYER_START_VERTICAL_ADJUSTMENTS {
        let mut transform = start;
        transform.translation.y = base_center_y + adjustment;
        if spatial
            .shape_intersections(
                &capsule,
                transform.translation,
                transform.rotation,
                &SpatialQueryFilter::default(),
            )
            .is_empty()
        {
            return Some(AlsPlayerSpawnFit {
                transform,
                ground_y,
                adjustment,
            });
        }
    }
    None
}

pub struct AlsPlayerStartPlugin;

impl Plugin for AlsPlayerStartPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AlsPlayerStartSettings>()
            .add_systems(Update, prepare_player_starts);
    }
}

fn prepare_player_starts(
    mut commands: Commands,
    settings: Res<AlsPlayerStartSettings>,
    spatial: SpatialQuery,
    mut starts: Query<
        (Entity, &Transform, Option<&mut AlsPlayerStartProbe>),
        (With<AlsPlayerStart>, Without<AlsPlayerStartReady>),
    >,
) {
    for (entity, start, probe) in &mut starts {
        let probe_origin = start.translation + Vec3::Y * settings.probe_height;
        let Some(hit) = spatial.cast_ray(
            probe_origin,
            Dir3::NEG_Y,
            settings.probe_distance,
            true,
            &SpatialQueryFilter::default(),
        ) else {
            continue;
        };
        let ground_y = probe_origin.y - hit.distance;
        let Some(fit) = fit_player_start(
            &spatial,
            *start,
            ground_y,
            settings.capsule_half_height,
            settings.capsule_radius,
        ) else {
            continue;
        };
        let stable_frames = match probe {
            Some(mut probe)
                if (probe.ground_y - fit.ground_y).abs() <= 1e-4
                    && (probe.adjustment - fit.adjustment).abs() <= 1e-4 =>
            {
                probe.stable_frames += 1;
                probe.stable_frames
            }
            Some(mut probe) => {
                probe.ground_y = fit.ground_y;
                probe.adjustment = fit.adjustment;
                probe.stable_frames = 1;
                probe.stable_frames
            }
            None => {
                commands.entity(entity).insert(AlsPlayerStartProbe {
                    ground_y: fit.ground_y,
                    adjustment: fit.adjustment,
                    stable_frames: 1,
                });
                1
            }
        };
        if stable_frames < settings.required_stable_frames.max(1) {
            continue;
        }
        commands.entity(entity).insert(AlsPlayerStartReady {
            transform: fit.transform,
            ground_y: fit.ground_y,
            adjustment: fit.adjustment,
        });
        info!(
            "[PLAYERSTART] ground_y={:.4} center_y={:.4} adjustment={:.4} stable_frames={} -> PASS",
            fit.ground_y, fit.transform.translation.y, fit.adjustment, stable_frames
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_start_height_preserves_the_source_clearance() {
        assert!((player_start_grounded_height(0.0, 0.90) - 0.92).abs() < 1e-6);
        assert!((player_start_grounded_height(1.25, 0.90) - 2.17).abs() < 1e-6);
    }

    #[test]
    fn default_settings_match_the_als_capsule() {
        let settings = AlsPlayerStartSettings::default();
        assert_eq!(
            settings.capsule_half_height,
            crate::character::ALS_CHARACTER_CAPSULE_HALF_HEIGHT
        );
        assert_eq!(
            settings.capsule_radius,
            crate::character::ALS_CHARACTER_CAPSULE_RADIUS
        );
        assert_eq!(settings.required_stable_frames, 2);
    }
}
